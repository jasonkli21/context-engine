# Phase 3 review fixes

**Date:** 2026-10-15
**Baseline:** `ff9ad64bd290f71c3c1b6536f0907ca32dfc351b`
**Review:** [Independent Phase 3 review](../reviews/phase-3-independent-review.md)
**Fix commit:** the single commit containing this report (resolve with
`git log -1 --format=%H -- docs/reports/phase-3-review-fix-report.md`).
**Disposition:** all four blocking review findings addressed; local canonical
verification passed. Fresh independent re-review remains required; this report
does not constitute independent acceptance.

## Findings and changes

| Finding | Disposition |
| --- | --- |
| CE-P3-001 | All three Phase 3 query outputs now identify the analyzed root and state index-local, non-lineage ID semantics. Human output includes the escaped repository root. |
| CE-P3-002 | Query outputs reuse the existing index and graph output summaries, add `analysis_incomplete`, diagnostic/scan-warning counts, relation status counts for graph-backed commands, and explicit detail-omission fields. Empty results remain successful and carry their coverage context. |
| CE-P3-003 | Added a shared normalized `/`-separated path helper. File search matches only UTF-8 paths using that emitted representation, reports omitted non-UTF-8 candidates, and marks lossy path presentation in coverage. |
| CE-P3-004 | Human neighborhood output assigns each visit an entity ID and predecessor/relation evidence, then names candidate test entities and repeats their evidence trace. JSON association membership is unchanged. |

The query commands no longer probe Git because their outputs do not consume Git
metadata. The Phase 3 plan and README now state the output-scope, coverage, and
normalized-path requirements. The supplied independent review is preserved at
`docs/reviews/phase-3-independent-review.md` without changing its verdict.

## Validation actually run

Used the existing isolated aarch64 macOS Rust toolchain and cached dependencies:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
export CARGO_NET_OFFLINE=true
./scripts/verify.sh
```

- Canonical verifier passed: context checks, formatting, warnings-denied Clippy,
  47 unit tests and 40 integration tests.
- Integration coverage checks repository scope, complete/incomplete file
  analysis, skipped files, failed UTF-8 loading, scan/diagnostic omission flags,
  normalized nested paths, human root escaping, and named test-association
  predecessor/relation evidence.
- Unix unit tests construct two distinct non-UTF-8 path values that collapse
  under lossy display and verify they are not matched as textual file identity.
  The sandbox filesystem rejected creation of such filenames, so the behavior
  and output omission count are verified with a synthetic in-memory index.
- No cross-platform CI, cloud/provider verification, or independent executable
  verification was run.

## Remaining review status

The non-blocking future considerations in the independent review remain
considerations. Phase 3 is not independently accepted until a fresh reviewer
reconstructs and evaluates the fixed implementation. Phase 1 and Phase 2 final
independent dispositions remain pending.
