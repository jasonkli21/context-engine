# Phase 7 independent-review fixes

**Date:** 2026-10-16
**Status:** findings addressed and locally verified; fresh independent re-review required

## Findings addressed

- Analyzer-stage `ReadFailed` no longer returns a merge containing retained cache
  units. Automatic reconciliation discards the candidate, rebuilds from a fresh
  canonical scan, preserves live per-file failure evidence, and does not persist it.
  Explicit `index` still fails on incomplete analysis.
- Full replacement and incremental patch writers now share a per-cache SQLite
  sidecar reservation with the existing two-second busy timeout. Reconciliation
  rechecks all source inputs while holding the reservation through commit or path
  replacement. A changed input retries once; stale incremental generations retain
  their compare-and-swap conflict path. The reservation releases when the process
  exits.
- Watch registers ancestor directories for parent `.gitignore` and `.ignore`
  controls, filters unrelated parent activity, and performs a canonical full
  reconciliation at least every 60 seconds. Event paths remain wakeup/filter hints;
  only the scanner determines inventory. Tests cover parent ignore changes and
  timer recovery without backend events.
- Store loading and patching validate SQLite page geometry. Incremental patches
  derive `max_page_count` from the database's actual page size, reject oversized or
  non-regular write targets before opening, and validate the resulting geometry.
  A database using 8192-byte pages remains patchable within the 128 MiB ceiling.
- Living documentation now reflects the current Phase 7 behavior, all-writer
  coordination and watcher safety net. The architecture's stale Phase 0 next-step
  section is evergreen, and the PRD learning progression matches the Phase 8/9
  roadmap.

## Verification

The canonical `./scripts/verify.sh` passed locally on macOS with the isolated Rust
toolchain: formatting, Clippy with warnings denied, context checks, **99 unit tests**
and **61 integration tests**. No cross-platform or independent verifier execution
is claimed.

Focused regression coverage includes retained-cache fallback after analyzer read
failure and a concurrent initially-unchanged file edit; shared writer-lock blocking;
noncanonical SQLite page sizes; ancestor ignore-control wakeups; unrelated parent
event filtering; and periodic reconciliation without backend events.

This fixes the handoff findings. It does not constitute a fresh independent Phase 7
review or acceptance.
