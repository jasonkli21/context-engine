# Phase 1 Review Fix Report

**Date:** 2026-10-14<br>
**Review:** [Phase 1 Independent Review](../reviews/phase-1-independent-review.md)<br>
**Reviewed implementation commit:** `f6f39f4e91c94fc480c3a1c69248589bf8bbdbe6`

## Summary

Addressed all three actionable findings in the independent Phase 1 review. The
fix adds bounded, fail-closed per-file symbol analysis; preserves Rust
reference/pointer impl-target syntax by declining guessed nominal qualification;
and narrows the README's concurrent-file-change claim.

## Findings addressed

### CE-P1-001 — Per-file analysis resource limits

Each source file now has project-owned ceilings of 4,096 symbols, 4 MiB of
materialized local and qualified-name bytes, and 25,000 qualification-node
visits. The node count includes Rust impl-target normalization. Limits are
charged before copying names or joining qualified names. The existing
Tree-sitter 65,536 in-progress match-state limit remains a separate query
truncation guard.

When a project-owned limit is exceeded, analysis returns no file result. The
repository index records status and diagnostic kind `analysis_limit_exceeded`
with the fixed message `per-file symbol analysis limit exceeded`; no partial
symbols enter the index. Tree-sitter match-state exhaustion continues to use
`parse_failed`. The schema-v1 envelope is unchanged, and the new status is
documented in README.

Regression coverage exercises exact/over custom boundaries for symbol count,
name bytes and qualification visits; over-budget flat and deeply nested Rust
files through the CLI; zero partial symbols; one diagnostic per failed file;
and byte-identical repeated JSON.

### CE-P1-002 — Rust reference and pointer impl targets

Rust impl-target normalization still drops generic arguments from nominal
targets. Reference and pointer wrappers now make the target unsupported for
qualification, so methods receive no guessed `Foo` container. Tests cover
direct and generic nominal targets alongside `&Foo`, `&mut Foo`, and `*const
Foo`.

### CE-P1-003 — Source-loading wording

README now says unavailable, symlink/non-regular and oversized file states
observed while loading fail recoverably. It also states that ordinary concurrent
content changes may still be analyzed because the filesystem checks are not an
atomic snapshot.

## Verification

Using the existing isolated aarch64 macOS Rust 1.99.0 toolchain and cached
Cargo registry in offline mode; no global toolchain setup changed:

- `./scripts/verify.sh` — passed; 21 unit tests, 16 Phase 0 CLI tests and 7
  Phase 1 symbol CLI tests.
- `cargo check --all-targets` — passed.
- `cargo build --release` — completed. The local toolchain emitted a non-fatal
  `rust-objcopy` warning because its `libLLVM.dylib` is absent while stripping
  debug information.
- Release CLI smoke checks passed for engine `index`, `symbols`, exact
  `RepositoryScan` lookup, JSON `index`/`symbols`, and Python Context Integrity
  `index`/JSON `symbols`.
- The canonical verifier's context, formatting, Clippy and all-target test
  checks passed.

The independent reviewer's executable verification was blocked in its review
environment. These local checks do not claim cross-platform CI or independent
post-fix acceptance.

## Plan deviation and status

The original Phase 1 plan did not list a dedicated analysis-budget status. The
review recommended a truthful status rather than labeling budget-exceeded files
as successfully analyzed or conflating them with parse failures. The additive
`analysis_limit_exceeded` status and diagnostic retain the schema-v1 envelope
and are explicitly documented.

The findings are addressed and locally verified. Phase 1 remains pending
independent final disposition; this report does not mark the phase accepted.
