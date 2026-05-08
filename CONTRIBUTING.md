# Contributing to RustyRAG

Thanks for your interest in contributing! This document explains how to file issues, propose changes, and get a pull request merged.

By participating in this project you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

> **License notice.** RustyRAG is licensed under the [Elastic License 2.0](LICENSE). By submitting a contribution, you agree that your contribution is licensed under ELv2 and that you have the right to submit it. If you cannot agree to this, please do not contribute.

---

## Table of contents

- [Ways to contribute](#ways-to-contribute)
- [Reporting bugs](#reporting-bugs)
- [Requesting features](#requesting-features)
- [Reporting security issues](#reporting-security-issues)
- [Development setup](#development-setup)
- [Making changes](#making-changes)
- [Coding standards](#coding-standards)
- [Commit messages](#commit-messages)
- [Submitting a pull request](#submitting-a-pull-request)
- [Review process](#review-process)

---

## Ways to contribute

- **Report bugs** you've hit while running RustyRAG.
- **Suggest features** or improvements.
- **Improve docs** — README, SETUP, code comments, examples.

Found a problem or have an idea? Just contribute — open an issue, or send a PR straight away if the fix is small. For non-trivial changes, please open an issue first so we can discuss design before you spend time on a PR.

---

## Reporting bugs

Before opening a bug:

1. Search [existing issues](../../issues) to avoid duplicates.
2. Reproduce on the latest `main` if possible.
3. Gather the info the bug template asks for (Rust version, OS, env, logs).

Then open a new issue using the **Bug report** template.

## Requesting features

Open an issue using the **Feature request** template. Describe the problem you're trying to solve, not just the solution — context helps us evaluate trade-offs.

## Reporting security issues

**Do not file public issues for security vulnerabilities.** See [SECURITY.md](SECURITY.md) for private disclosure instructions.

---

## Development setup

RustyRAG targets stable Rust. See [SETUP.md](SETUP.md) for the full environment guide (Docker, Milvus, GPU services). For quick local iteration:

```bash
# Clone
git clone https://github.com/AlphaCorp-AI/RustyRAG.git
cd RustyRAG

# Configure
cp .env.example .env
# edit .env with your API keys

# Bring up Milvus + auxiliary services
docker compose up -d

# Build & run
cargo run --release
```

Useful commands:

```bash
cargo build              # debug build
cargo run                # run server
cargo test               # run tests
cargo fmt --all          # format
cargo clippy --all-targets --all-features -- -D warnings   # lint
cargo doc --no-deps --open                                 # local docs
```

---

## Making changes

1. **Fork** the repository and create a branch from `main`:
   ```bash
   git checkout -b fix/short-description
   ```
   Use prefixes: `feat/`, `fix/`, `docs/`, `refactor/`, `test/`, `chore/`.

2. **Keep changes focused.** One logical change per PR. Unrelated cleanups belong in separate PRs.

3. **Add tests** for new behaviour or bug fixes. If a change is hard to test, explain why in the PR.

4. **Update docs** when you change user-visible behaviour, env vars, API surface, or CLI flags.

5. **Run the full check suite locally** before pushing:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ```

---

## Coding standards

- **Format with `rustfmt`.** No exceptions; CI rejects unformatted code.
- **Pass `clippy` with `-D warnings`.** If you must allow a lint, justify it inline.
- **Prefer `Result`/`thiserror` over panics** in library code. `unwrap()`/`expect()` is acceptable in tests, examples, and `main.rs` startup paths only.
- **Use `tracing`** for logging — never `println!` in handlers or services.
- **Public APIs need doc comments.** Use `///` and include an example where it isn't obvious.
- **No `unsafe`** unless absolutely necessary, and never without a `// SAFETY:` comment explaining the invariants.
- **Keep dependencies lean.** Prefer the existing crate set; if you need a new dep, justify it in the PR.

### Project-specific guidelines

- Handlers live in `src/handlers/`, services in `src/services/`, schemas (request/response types) in `src/schemas/`.
- Anything that calls an external service (LLM, embeddings, Milvus, reranker) belongs behind a typed client in `services/`.
- Configuration goes through `src/config.rs` and `.env` — don't hard-code endpoints or model names.
- New API endpoints must be annotated with `utoipa` so they show up in Swagger UI.

---

## Commit messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short summary>

<body — optional, wrap at 72 chars>

<footer — optional, e.g. "Closes #123">
```

Common types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`, `build`, `ci`.

Examples:

```
feat(reranker): add Jina v4 support
fix(ingest): handle empty PPTX slides without panicking
docs(readme): clarify reranker latency trade-off
```

Breaking changes: append `!` to the type (`feat!:`) and add a `BREAKING CHANGE:` footer.

---

## Submitting a pull request

1. Push your branch and open a PR against `main`.
2. Fill out the PR template — what changed, why, how it was tested.
3. Link related issues with `Closes #N` or `Refs #N`.
4. Make sure CI is green. PRs with failing CI won't be reviewed.
5. Mark the PR as a draft if it isn't ready for review yet.

Keep PRs small. A good rule of thumb: a reviewer should be able to read it in under 30 minutes. Large changes are easier to review when split into logical commits.

---

## Review process

- A maintainer will review your PR, usually within a few days.
- Address review feedback by pushing additional commits — don't force-push during review unless asked.
- Once approved and CI is green, a maintainer will squash-merge.
- After merge, your contribution will appear in the next release. Thank you!

---

## Questions?

Open a [Discussion](../../discussions) or comment on a related issue. We're happy to help.
