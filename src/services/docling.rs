use std::collections::BTreeMap;

use anyhow::Context;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::document::PageText;

/// Supported file extensions for Docling conversion.
const DOCLING_EXTENSIONS: &[&str] = &["pdf", "docx", "pptx", "xlsx", "html"];

#[derive(Clone)]
pub struct DoclingClient {
    http: Client,
    base_url: String,
    groq_api_key: String,
    vision_model: String,
}

// ── Groq vision API types ───────────────────────────────────────────

#[derive(Serialize)]
struct VisionRequest {
    model: String,
    messages: Vec<VisionMessage>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct VisionMessage {
    role: String,
    content: Vec<VisionContent>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum VisionContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Deserialize)]
struct VisionResponse {
    choices: Vec<VisionChoice>,
}

#[derive(Deserialize)]
struct VisionChoice {
    message: VisionChoiceMessage,
}

#[derive(Deserialize)]
struct VisionChoiceMessage {
    content: Option<String>,
}

impl DoclingClient {
    pub fn new(base_url: &str, groq_api_key: &str, vision_model: &str) -> Self {
        Self {
            http: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(300)) // PDFs can be slow
                .pool_idle_timeout(std::time::Duration::from_secs(90))
                .pool_max_idle_per_host(8)
                .build()
                .expect("Failed to build reqwest client for Docling"),
            base_url: base_url.trim_end_matches('/').to_string(),
            groq_api_key: groq_api_key.to_string(),
            vision_model: vision_model.to_string(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty()
    }

    /// Returns true if this extension should be routed through Docling.
    pub fn supports_extension(ext: &str) -> bool {
        DOCLING_EXTENSIONS.contains(&ext)
    }

    /// Convert a document via the Docling API.
    ///
    /// We request BOTH `md` and `json` representations. The structured JSON
    /// (`json_content.texts[*].prov[0].page_no`) is the authoritative source
    /// for per-chunk page numbers — every text item carries its real PDF page
    /// extracted by Docling, so we no longer need to guess from markdown
    /// `<!-- page N -->` markers (which Docling does not emit by default and
    /// which are brittle even when present).
    ///
    /// Inputs without inherent pagination (HTML, plain text) get `page_no =
    /// None`, which we map to page 1 — an honest fallback rather than an
    /// inferred value.
    ///
    /// The markdown is kept around for embedded-image processing: base64
    /// images are extracted from the markdown body and described via the
    /// vision model, then attached to the appropriate page.
    pub async fn convert_document(
        &self,
        file_bytes: Vec<u8>,
        filename: &str,
    ) -> anyhow::Result<Vec<PageText>> {
        let body = self.call_docling_api(file_bytes, filename).await?;

        let document = &body["document"];
        let json_content = &document["json_content"];
        // `md_content` is searched across known Docling response shapes; it is
        // only used for fallback when `json_content` is empty.
        let md_content = extract_markdown_from_response(&body).unwrap_or_default();

        // Group text + tables by page from the structured JSON.
        let mut by_page: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        collect_texts_by_page(json_content, &mut by_page);
        collect_tables_by_page(json_content, &mut by_page);

        // Describe pictures via the vision model and attach them to their page.
        if let Some(pictures) = json_content["pictures"].as_array() {
            for pic in pictures {
                let page = page_no_of(pic).unwrap_or(1);
                let uri = pic["image"]["uri"]
                    .as_str()
                    .or_else(|| pic["image"].as_str())
                    .or_else(|| pic["uri"].as_str());
                let Some(uri) = uri else { continue };
                if !uri.starts_with("data:image/") {
                    continue;
                }
                let desc = match self.describe_image(uri).await {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!("Failed to describe image: {e}");
                        "description unavailable".to_string()
                    }
                };
                by_page
                    .entry(page)
                    .or_default()
                    .push(format!("[Image: {desc}]"));
            }
        }

        let mut pages: Vec<PageText> = by_page
            .into_iter()
            .map(|(page, segs)| PageText {
                text: segs.join("\n\n"),
                page_number: Some(page),
            })
            .collect();

        // Fallback when json_content is empty (older Docling versions or
        // non-supported corner cases): fall back to the legacy markdown path
        // so we never lose data, even if page metadata is unknown.
        if pages.is_empty() {
            let md = self.replace_images_with_descriptions(&md_content).await;
            let text = md.trim().to_string();
            if !text.is_empty() {
                pages.push(PageText {
                    text,
                    page_number: None,
                });
            }
        }

        Ok(pages)
    }

    /// POST file to Docling /v1/convert/file and return the full response body.
    ///
    /// Requests both `md` and `json` output formats so callers can:
    ///   - process embedded images via `md_content` (data URIs are easier to
    ///     extract from markdown),
    ///   - read per-item page numbers from `json_content.texts[*].prov[0].page_no`.
    ///
    /// Retries with exponential backoff on connection errors (e.g. Docling
    /// restarting) and 5xx responses.
    async fn call_docling_api(
        &self,
        file_bytes: Vec<u8>,
        filename: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/v1/convert/file", self.base_url);

        let max_retries = 8u32;
        let mut backoff = std::time::Duration::from_secs(2);

        for attempt in 0..=max_retries {
            let file_part = reqwest::multipart::Part::bytes(file_bytes.clone())
                .file_name(filename.to_string())
                .mime_str("application/octet-stream")
                .context("Failed to create multipart part")?;

            let form = reqwest::multipart::Form::new()
                .part("files", file_part)
                .text("image_export_mode", "embedded")
                .text("do_table_structure", "true")
                .text("to_formats", "md")
                .text("to_formats", "json");

            let result = self.http.post(&url).multipart(form).send().await;

            let resp = match result {
                Ok(r) => r,
                Err(e) if attempt < max_retries => {
                    tracing::warn!(
                        "Docling connection failed for '{filename}', retrying in {}s ({}/{max_retries}): {e}",
                        backoff.as_secs(),
                        attempt + 1,
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(std::time::Duration::from_secs(60));
                    continue;
                }
                Err(e) => return Err(e).context("Docling: failed to send request"),
            };

            let status = resp.status();

            if status.is_server_error() && attempt < max_retries {
                let text = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    "Docling returned {status} for '{filename}', retrying in {}s ({}/{max_retries}): {text}",
                    backoff.as_secs(),
                    attempt + 1,
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(std::time::Duration::from_secs(60));
                continue;
            }

            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("Docling returned {status}: {text}");
            }

            let body: serde_json::Value = resp
                .json()
                .await
                .context("Docling: failed to parse response")?;

            return Ok(body);
        }

        anyhow::bail!("Docling still unavailable for '{filename}' after {max_retries} retries")
    }

    /// Find base64-embedded images in markdown and replace with text descriptions.
    async fn replace_images_with_descriptions(&self, markdown: &str) -> String {
        if self.groq_api_key.is_empty() {
            return markdown.to_string();
        }

        let re = Regex::new(r"!\[([^\]]*)\]\((data:image/[^;]+;base64,[^)]+)\)")
            .expect("Invalid regex");

        let mut result = markdown.to_string();
        let matches: Vec<(String, String)> = re
            .captures_iter(markdown)
            .map(|cap| (cap[0].to_string(), cap[2].to_string()))
            .collect();

        if matches.is_empty() {
            return result;
        }

        tracing::info!("Describing {} embedded images via vision model…", matches.len());

        for (full_match, data_url) in &matches {
            match self.describe_image(data_url).await {
                Ok(description) => {
                    result = result.replace(
                        full_match,
                        &format!("[Image: {description}]"),
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to describe image: {e}");
                    result = result.replace(full_match, "[Image: description unavailable]");
                }
            }
        }

        result
    }

    /// Call Groq with Llama 4 Scout to describe a base64-encoded image.
    async fn describe_image(&self, data_url: &str) -> anyhow::Result<String> {
        let body = VisionRequest {
            model: self.vision_model.clone(),
            messages: vec![VisionMessage {
                role: "user".into(),
                content: vec![
                    VisionContent::Text {
                        text: "Describe this image concisely for a document search system. \
                               Include all visible text, numbers, data, and structural information."
                            .into(),
                    },
                    VisionContent::ImageUrl {
                        image_url: ImageUrl {
                            url: data_url.to_string(),
                        },
                    },
                ],
            }],
            max_tokens: 512,
        };

        let resp: VisionResponse = self
            .http
            .post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(&self.groq_api_key)
            .json(&body)
            .send()
            .await
            .context("Vision API: failed to send request")?
            .error_for_status()
            .context("Vision API: server returned error")?
            .json()
            .await
            .context("Vision API: failed to parse response")?;

        resp.choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("Vision API returned no content"))
    }
}

// ── Response parsing helpers ────────────────────────────────────────

/// Try multiple known paths to extract markdown from a Docling response.
fn extract_markdown_from_response(body: &serde_json::Value) -> Option<String> {
    // Path 1: document.md_content
    if let Some(md) = body["document"]["md_content"].as_str() {
        return Some(md.to_string());
    }
    // Path 2: document.export_to_markdown (some versions)
    if let Some(md) = body["document"]["markdown"].as_str() {
        return Some(md.to_string());
    }
    // Path 3: rendered.md
    if let Some(md) = body["rendered"]["md"].as_str() {
        return Some(md.to_string());
    }
    // Path 4: md_content at top level
    if let Some(md) = body["md_content"].as_str() {
        return Some(md.to_string());
    }
    // Path 5: content at top level
    if let Some(md) = body["content"].as_str() {
        return Some(md.to_string());
    }
    // Path 6: output.md
    if let Some(md) = body["output"]["md"].as_str() {
        return Some(md.to_string());
    }
    None
}

// ── Page assembly from DoclingDocument JSON ─────────────────────────
//
// The DoclingDocument JSON exposes `texts`, `tables`, and `pictures`. Every
// item carries a `prov[]` array; the first entry's `page_no` is the 1-based
// physical PDF page (or `null` for inputs without inherent pagination, e.g.
// HTML). We group items by page_no and join them into one block of text per
// page.

/// Extract the 1-based `page_no` from an item's `prov[0]`. Returns None when
/// the input format has no notion of pages (HTML, plain text).
fn page_no_of(item: &serde_json::Value) -> Option<u32> {
    item["prov"][0]["page_no"]
        .as_u64()
        .map(|n| n as u32)
}

/// Append every text item in `json_content.texts` to `by_page`, keyed by its
/// page_no (defaults to 1 when unknown).
fn collect_texts_by_page(
    json_content: &serde_json::Value,
    by_page: &mut BTreeMap<u32, Vec<String>>,
) {
    let Some(texts) = json_content["texts"].as_array() else {
        return;
    };
    for t in texts {
        let Some(s) = t["text"].as_str() else { continue };
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        let page = page_no_of(t).unwrap_or(1);
        by_page.entry(page).or_default().push(s.to_string());
    }
}

/// Append every table in `json_content.tables` as text to `by_page`.
///
/// DoclingDocument tables can be represented in multiple ways across Docling
/// versions. We try (in order): a flat `text` field, a structured `data.grid`
/// of cells (joined with ` | ` per row), and finally a flattened concatenation
/// of any nested `text` fields. Whichever is found is good enough for chunk
/// indexing; the LLM judge sees the page reference, not the formatting.
fn collect_tables_by_page(
    json_content: &serde_json::Value,
    by_page: &mut BTreeMap<u32, Vec<String>>,
) {
    let Some(tables) = json_content["tables"].as_array() else {
        return;
    };
    for tb in tables {
        let page = page_no_of(tb).unwrap_or(1);
        let text = if let Some(s) = tb["text"].as_str() {
            s.to_string()
        } else if let Some(grid) = tb["data"]["grid"].as_array() {
            let rows: Vec<String> = grid
                .iter()
                .filter_map(|row| row.as_array())
                .map(|cells| {
                    cells
                        .iter()
                        .filter_map(|c| c["text"].as_str())
                        .collect::<Vec<_>>()
                        .join(" | ")
                })
                .filter(|r| !r.is_empty())
                .collect();
            rows.join("\n")
        } else {
            String::new()
        };
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            by_page.entry(page).or_default().push(trimmed.to_string());
        }
    }
}
