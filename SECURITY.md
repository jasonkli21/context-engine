# Security policy

Context Engine is intended to analyze source repositories locally and read-only by default.

## Security principles

- Do not execute target-repository code as part of normal scanning/indexing.
- Do not upload source code without explicit user configuration and consent.
- Do not follow filesystem links outside the requested scan boundary by default.
- Treat repository contents as untrusted input.
- Avoid logging source contents or secrets unnecessarily.
- Keep future network/model integrations opt-in and explicit about transmitted data.

## Reporting a vulnerability

For a public GitHub repository, use GitHub's private security advisory mechanism when available rather than opening a public issue for sensitive vulnerabilities. Non-sensitive bugs can use the normal issue tracker.

Please include the affected version/commit, reproduction steps, impact, and any suggested mitigation.
