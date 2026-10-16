# Phase 6 independent-review fixes

**Date:** 2026-10-16
**Status:** fixes implemented and locally verified; fresh independent re-review pending

This follow-up addresses the findings in the supplied Phase 6 review handoff.
It does not change earlier phase dispositions or claim independent acceptance.

## Changes

- SQLite schema v2 uses deterministic path-derived internal file keys, native path
  uniqueness, and per-file child ordinals. Loads reconcile by native path and then
  rebuild canonical in-memory ordering. Schema-v1 caches are incompatible and rebuild.
- A versioned BLAKE3 digest covers sorted storage keys, native path bytes, foreign-key
  associations, local ordinals, and exact payload bytes. The loader validates bounds,
  verifies the digest, then decodes records. Source freshness and artifact integrity
  remain separate checks. The digest detects accidental/corrupt changes; it is not a
  MAC against a process that can modify the digest too.
- Automatic commands bypass cache reuse when eligible-source freshness reads fail,
  return best-effort analysis with per-file `ReadFailed` evidence, and do not persist
  that result. Explicit `index` still fails when freshness cannot be established.
- The schema writer uses `SCHEMA_VERSION`; `INDEX_COMPAT_VERSION` now documents the
  analyzer/classifier/extraction changes that require a bump. Tests pin the saved
  SQLite version, reject incompatible analysis versions, and retain representative
  symbol/relation codec round trips.
- Added a real scan/fingerprint/store/warm-load non-UTF-8 filename test on Unix.
  It skips only when the filesystem rejects creation of such a path.
- Updated the README, current-state snapshot, architecture, and this delivery record
  to describe validated cache reuse and the corrected Phase 7 file-local storage seam.
  The original implementation report remains historical evidence.

## Verification

`./scripts/verify.sh` passed: **82 unit / 59 integration tests**, formatting,
Clippy with warnings denied, and repository context checks. Store/persistence tests
cover schema-v2 key stability, valid JSON payload tampering, source-freshness fallback,
compatibility metadata, and native-path codecs. The integrated real invalid-byte
filename test is guarded; this sandbox rejected creating that filename, so it skipped.
The separate native non-UTF-8 store codec test passed. No cloud, cross-platform, or
independent executable verification is claimed.
