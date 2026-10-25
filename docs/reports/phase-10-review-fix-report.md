# Phase 10 review-fix report

Date: 2026-10-24. Implementation evidence for the CE10-01–CE10-05 findings in
the attached independent Phase 10 review. Fresh independent re-review and
maintainer acceptance remain pending; this report does not change prior phase
dispositions.

## Baseline and ownership

Fixes start from `42cf009` (`Implement Phase 10 local workspace intelligence`).
The pre-existing untracked `docs/plans/phase-10-implementation-plan.md` was kept
unchanged and excluded from the commit. No subagents were used. This report's
implementation SHA is the commit containing this file; resolve it with
`git log -1 --format=%H -- docs/reports/phase-10-review-fix-report.md`.

## Findings addressed

- **CE10-01 — competing direct evidence:** Impact still emits one candidate and
  one representative shortest witness chain per repository. It now retains up to
  64 sorted direct relation IDs per candidate, counts excess IDs in
  `direct_triggers_omitted`, and gathers the direct trigger paths. Exact declared
  contract evidence ranks ahead of coarse package evidence for the candidate's
  summary reason and representative first hop. Alternate transitive chains remain
  summarized by `alternative_witnesses_omitted`; they are not expanded into all
  possible paths.
- **CE10-02 — relation identity:** Contract kind is part of the structural
  relationship hash. IDs remain independent of canonical root locations, volatile
  manifest fingerprints and manifest ordering. Tests cover API/event kind changes,
  root relocation and manifest reorder.
- **CE10-03 — unmodeled changes:** Impact JSON now reports each excluded changed
  side in `trigger_omissions` with its path when representable, side, and reason:
  `role_not_package_trigger`, `non_regular` or `non_utf8_omitted`. Exact contract
  matching remains available for a path outside the coarse package classifier.
  The list is bounded by the existing Git diff/path limits and final 8 MiB output
  cap.
- **CE10-04 — root package with workspace:** A root with an explicit `[package]`
  name and direct dependency may also contain `[workspace]`. The root package is
  inspected, while member discovery is a typed coverage gap. Virtual workspace
  roots and inherited `package.workspace` identity remain unsupported; dependency
  entries using `workspace = true` remain coverage gaps.
- **CE10-05 — human evidence:** Human relationship rows now print the relation ID,
  consumer path, contract kind and ID, source location, and observed manifest
  fingerprints. Dynamic text uses the existing control-character escaping.

Workspace output is now `workspace_schema_version: 2`; manifest input remains
version 1. This explicit bump covers the new candidate/omission fields and changed
relation-ID semantics while the Phase 10 evidence contract is still under review.
No cache, index compatibility, HTTP, or MCP schema changed. The original Phase 10
implementation report is marked as describing the v1 baseline.

## Regression coverage

The production-binary workspace integration suite now has 14 cases. New coverage
checks multiple changed contracts and an overlapping Cargo edge for one consumer,
exact-evidence summary precedence, relation ID invariance, typed `.proto` and
non-regular-path omissions, source and `build.rs` package triggers, explicit
root-package-plus-workspace behavior, and human/JSON evidence parity. A unit test
exercises the 64-trigger cap and confirms contract triggers survive coarse-trigger
overflow.

A disposable `target/release/ctx` smoke passed with two temporary Git repositories.
It exercised workspace JSON v2, an explicit root package with `[workspace]`, a
Cargo relation overlapping two exact contract triggers, a custom `.proto` omission,
and the human consumer provenance fields. The temporary repositories were removed.

## Verification

Environment: aarch64 macOS 27.0.1; `rustc 1.99.1
(58cf8f9c3, 2026-10-13)`. Existing offline dependencies and toolchain were used.

- `cargo test --offline --test workspace`: passed, 14 tests.
- `./scripts/verify.sh`: passed context checks, formatting, Clippy, 110 unit tests
  and 93 integration tests, including all 8 HTTP/MCP adapter tests. The first
  sandboxed invocation could not start the loopback HTTP fixtures; the canonical
  rerun with local loopback access completed successfully.
- `cargo check --all-targets --offline`: passed.
- `cargo build --release --offline`: passed.
- Disposable release-binary multi-repository smoke: passed as described above.
- `git diff --check`: passed.

No Linux/Windows, cloud/provider, third-party MCP, external deployment or fresh
independent executable review is claimed.
