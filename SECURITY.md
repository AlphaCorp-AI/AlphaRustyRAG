# Security Policy

We take the security of RustyRAG seriously. This document explains which versions receive security fixes and how to report a vulnerability privately.

## Supported versions

Security fixes are issued for the latest minor release on `main`. Older releases are supported on a best-effort basis only.

| Version | Supported          |
| ------- | ------------------ |
| `main`  | :white_check_mark: |
| Latest released minor | :white_check_mark: |
| Older   | :x:                |

## Reporting a vulnerability

**Please do not file public GitHub issues for security vulnerabilities.**

Report suspected vulnerabilities privately to **security@example.com**. If you prefer, you can also use [GitHub's private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability) on this repository.

Please include:

- A description of the issue and its impact.
- Steps to reproduce, or a proof-of-concept.
- Affected versions or commit SHAs.
- Any suggested mitigation, if known.

## What to expect

- **Acknowledgement** within **48 hours** of your report.
- An initial assessment within **5 business days**, including severity and a tentative fix timeline.
- Regular status updates until the issue is resolved.
- Public disclosure only after a fix is available, coordinated with the reporter.

## Disclosure process

1. You report the issue privately.
2. We confirm the vulnerability and determine its scope.
3. We develop and test a fix on a private branch.
4. We release a patched version and publish a [GitHub Security Advisory](../../security/advisories) crediting the reporter (unless anonymity is requested).
5. After a reasonable embargo period (typically 7–30 days, depending on severity), full technical details may be disclosed.

## Out of scope

The following are **not** considered vulnerabilities:

- Issues that require physical access to a user's machine.
- Self-XSS or social-engineering attacks.
- Findings from automated scanners without a working proof-of-concept.
- Denial-of-service via resource exhaustion in self-hosted deployments where the operator controls the workload.

Thank you for helping keep RustyRAG and its users safe.
