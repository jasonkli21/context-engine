# Phase 5 independent-review fixes

**Date:** 2026-10-16
**Reviewed baseline:** `c99c255c3e90e39cd350859228239e8ef7f6f8bb`
**Status:** findings addressed locally; fresh independent re-review required

This report records fixes for the findings in the maintainer-provided Phase 5
independent-review handoff. It preserves the distinction between local delivery
evidence and independent acceptance.

## Findings addressed

The original fixture reported 12/12 ranker hits, 0/12 path-order hits, and 4/12
path-only hits. Those values are retained here as historical author-side results,
not acceptance evidence: four expected tests each declared the task-named helper
locally, and the strongest baseline omitted the existing symbol search capability.
The corrected benchmark below is the current evidence.

### CE-P5-003 — seed identity is not graph evidence

Traversal still reuses the canonical `RepositoryGraph`, but candidate scoring now
ignores distance-zero visits and visits whose owner is the seed's own file. A
graph score requires reaching a distinct file owner through an actual relation;
distance 1 scores 20 and distance 2 scores 10. Changed-file and changed-symbol
owner evidence remains in the change component without a duplicate distance-zero
boost.

Direct path and symbol matches remain direct when traversal roots are truncated.
A direct test match receives `direct_test_match`; `related_test_candidate` is
reserved for a non-direct test reached from qualifying graph evidence. Current
resolved relationships are same-file only, so the current model produces no
cross-file graph boost or related-test candidate. No cross-file resolver was
added to satisfy the benchmark.

### CE-P5-002 — candidate loading has an operation-wide safety cap

Selection preserves the exact selected UTF-8 content-byte budget and separately
limits content-load work to 256 candidate attempts and 64 MiB of conservatively
reserved read allowance. Before each safe-loader call, selection reserves
`min(remaining content budget, per-file limit) + 1` bytes. The extra byte accounts
for the loader's bounded growth probe. The operation stops before either cap would
be exceeded and reports attempted loads, reserved bytes, safety truncation, and
unattempted candidate count in human and schema-v1 JSON output.

Injected-limit tests cover invalid UTF-8, a file removed after indexing, aggregate
allowance exhaustion, and zero-byte successful files. Existing regressions cover
over-budget files, skip-and-continue behavior, and exact selected-byte accounting.

### CE-P5-004 — `file-ranker-v1` pins its public behavior

Scoring weights and caps, reason codes, graph/change distances, root priorities,
fallback size, and ordering key are centralized in `ranker_v1`. A contract test
asserts the version string, exact score breakdowns, reasons, order, and centrality
fallback. Human and JSON renderers use the same version constant. The corrected
pre-acceptance behavior remains `file-ranker-v1`; the first reviewed implementation
was not accepted as a stable contract.

### CE-P5-001 — expected-file benchmark uses a stronger baseline

The revised six-scenario fixture has 22 files of 128 bytes each and keeps Rust,
Python, tests, docs, configuration, and schema examples. Test fixtures import and
exercise the implementation; they do not define duplicate task-named helpers.
Expected paths are literal fixture labels. Each expected test has a direct path
match but no direct symbol match, which the test asserts. It is labeled as a
direct test match, not as a graph-related test or coverage result.

The five strategies share production file eligibility, `source::load`, complete
file packing, and a 256-byte budget: production ranking, existing path-and-symbol
search, canonical path order, path-only lexical ranking, and production ranking
with file-role scores removed.

| Task | Production | Existing path + symbol search | Path order | Path-only | Without role |
| --- | ---: | ---: | ---: | ---: | ---: |
| cache | 2/2 | 1/2 | 0/2 | 0/2 | 2/2 |
| router | 2/2 | 1/2 | 0/2 | 0/2 | 2/2 |
| retry | 2/2 | 1/2 | 0/2 | 0/2 | 2/2 |
| codec | 2/2 | 1/2 | 0/2 | 0/2 | 2/2 |
| setup | 2/2 | 1/2 | 0/2 | 1/2 | 1/2 |
| protocol | 2/2 | 1/2 | 0/2 | 1/2 | 1/2 |
| **Total / macro recall** | **12/12; 1.0** | **6/12; 0.5** | **0/12; 0.0** | **2/12; 0.166667** | **10/12; 0.833333** |

Every strategy selects 256 bytes in each scenario, or 1,536 bytes across the
six scenarios. Removing role scores drops recall in both supporting-file
scenarios. The fixture supports this bounded result only; it does not claim that
the existing graph retrieves cross-file tests or source files.

### CE-P5-DOC-001 — status and architecture describe current evidence

The original implementation report now marks its first benchmark as superseded
and records the corrected fixture, baseline, measurements, score semantics, and
load-work limits. The README and architecture describe direct versus related
test evidence and the current absence of cross-file graph retrieval. The original
Phase 5 implementation plan remains intact as historical requirements evidence.

## Verification

Using the existing isolated macOS Rust toolchain and cached dependencies
(`PATH=/tmp/context-engine-cargo/bin:$PATH`, `CARGO_HOME=/tmp/context-engine-cargo`,
`RUSTUP_HOME=/tmp/context-engine-rustup`):

- `./scripts/verify.sh` passed: 71 unit tests and 56 integration tests, including
  formatting, Clippy with warnings denied, repository context checks, and all
  prior suites.
- `cargo check --all-targets` passed.
- `cargo build --release` passed.
- The focused context tests passed, including the six-scenario benchmark,
  ranker-v1 contract, distinct-file graph evidence, direct-test/root truncation,
  and injected load-work limits.
- Human and JSON release smokes passed on this repository. JSON selected 23,663
  of 24,000 bytes across three files and reported 11 content-load attempts.
- Human and JSON `--change BASE..HEAD` smokes passed on a clean isolated Git
  fixture; one changed file and its current mapping were reported.
- `git diff --check` passed.

These are local execution results. No cloud/provider or cross-platform
verification is claimed, and they do not replace the requested fresh independent
review.

## Limits

This is local implementation evidence, not an independent acceptance. The
benchmark is synthetic and measures path, symbol, and role ranking. The current
graph has no cross-file resolved reference edges, so this fix deliberately makes
no graph-retrieval quality claim. The filesystem is still not an atomic snapshot.
