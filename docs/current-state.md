# Current state

**Last updated:** 2026-10-15

**Active phase:** Phase 4 — Git and Structural Change Intelligence

**Overall status:** Phase 4 review findings fixed and locally verified; fresh independent re-review pending

## Delivered

- Phase 4 `diff BASE..HEAD` and `impact BASE..HEAD` read bounded committed Git
  snapshots, resolve immutable commits, honor nested scopes, and ignore dirty
  checkout content. Fixed Git rename evidence and file modes/object IDs remain
  visible even when structural comparison is unavailable.
- Unique file-paired kind/qualified-name symbol matching with source-slice body
  comparison; explicit ambiguity, fail-closed incomplete comparisons and relation
  occurrence multiset deltas without numeric-ID lineage or range-shift churn.
- Revision graphs reuse the existing analyzer/graph pipeline. Impact reuses BFS
  with exact revision-local seeds, distance/predecessor evidence, reached test
  candidates, directly changed test/docs and shared base/head safety budgets.
- New escaped human and bounded schema-v1 JSON outputs expose revision/scope,
  analysis coverage, unavailable comparisons, omitted seeds and truncation.
- Review fixes prioritize changed-symbol incoming caller evidence, reserve impact
  budgets across base/head, use changed-file neighborhoods as fallback, preflight
  a conservative 64 MiB human/JSON materialization bound, and exclude historical
  entries exactly at a nested scope prefix.


- Phase 3 index-only deterministic path/symbol `search`, generalized bounded
  `neighborhood` with per-root distances/predecessor evidence and repository-hub
  exclusion, and current-graph structural `hotspots` with distinct fan-in/out.
- Conservative reached test-role candidate associations; explicit ranking tuples,
  result/root/traversal truncation, escaped human and bounded schema-v1 JSON.
- New query outputs identify repository scope and index-local IDs, summarize
  analysis coverage/incompleteness, and expose relation statuses where applicable.
- File search uses normalized `/` paths and excludes non-UTF-8 paths from textual
  matching with explicit omission counts; human test associations show named
  visits and predecessor/relation evidence.

- Phase 0 scanner and Phase 1 Rust/Python symbol indexing remain intact, including
  bounded loading, fail-closed symbol budgets and deterministic schema-v1 output.
- Fresh synchronous in-memory `graph`, `deps`, `callers` and `related` commands.
- Explicit index-local repository/file/symbol IDs; lexical range containment;
  atomic Rust/Python import facts; conservative call-site reference facts.
- Conservative same-file exact-qualified resolution, unresolved targets, separate
  extraction/resolution evidence, occurrence-preserving relations and adjacency.
- Cycle-safe iterative traversal, grouped exact-name roots, file filtering,
  bounded depth/results, explicit truncation and schema-v1 graph/query DTOs.
- Independent fail-closed relation budgets and hierarchical discovery/index/graph
  text, record and output limits; explicit query root omission accounting;
  relation-file accounting alongside every file's Phase 1 analysis status.

Evidence: [Phase 4 report](reports/phase-4-implementation-report.md),
[Phase 3 report](reports/phase-3-implementation-report.md),
[Phase 3 review-fix report](reports/phase-3-review-fix-report.md),
[independent Phase 3 review](reviews/phase-3-independent-review.md),
[Phase 0 report](reports/phase-0-implementation-report.md),
[Phase 1 report](reports/phase-1-implementation-report.md),
[Phase 1 review-fix report](reports/phase-1-review-fix-report.md),
[independent Phase 1 review](reviews/phase-1-independent-review.md),
[Phase 2 report](reports/phase-2-implementation-report.md),
[independent Phase 2 review](reviews/phase-2-independent-review.md), and
[Phase 2 review-fix report](reports/phase-2-review-fix-report.md).

## Acceptance and verification

- The maintainer directly authorized Phase 4 from `865f5a4` while Phase 3 fresh
  re-review and Phase 1/2 final dispositions remain pending. Authorization does
  not constitute independent acceptance.
- Phase 4 canonical verifier passed: 57 unit and 50 integration tests, including
  all prior suites. All-target check, release build and diff whitespace checks
  passed. Eight release smoke invocations covered both commands and formats on
  a deterministic dirty-worktree Git fixture and the real Phase 3 fix range.
  Existing isolated macOS toolchain reused; no global setup changed. See report
  for observations, bounds, conservative omissions and audit evidence.
- Independent Phase 4 review identified three findings. All three are addressed
  in one follow-up change. The canonical verifier passed with 58 unit and 53
  integration tests; all-target check, release build and diff whitespace checks
  passed. See [Phase 4 review-fix report](reports/phase-4-review-fix-report.md).
  Fresh independent re-review remains pending; this is not Phase 4 acceptance.


- The maintainer explicitly authorized Phase 3 from `09efd22` while final
  independent Phase 1/2 dispositions remain pending. This is not acceptance.
- Phase 3 baseline verifier passed (41 unit / 33 integration). Final canonical
  verifier passed (43 unit / 37 integration), all-target check and release build
  passed, with 20 human/JSON release smoke invocations against this repository
  and the real sibling Python Context Integrity repository. The existing isolated
  macOS Rust toolchain was reused; no global setup changed.
- The independent Phase 3 review found four blocking output/path/evidence issues.
  All four were addressed in one follow-up commit. The canonical verifier passed
  with 47 unit and 40 integration tests. This is local implementation evidence;
  fresh independent re-review is still required and has not been claimed.

- Phase 0 was accepted by the user on 2026-10-14 with independent re-review waived.
- The user explicitly authorized Phase 2 despite Phase 1's remaining final
  acceptance label, before Phase 2 code changes. Phase 1 final independent
  disposition remains pending; authorization is not an independent acceptance.
- Phase 2 baseline canonical verifier passed: 21 unit tests and 23 integration
  tests, using the existing isolated `/tmp` Rust toolchain and cached dependencies.
- Initial Phase 2 verifier, all-target check, release build and release smoke
  passed locally on aarch64 macOS: 32 unit and 31 integration tests, including all
  unchanged Phase 0/1 regressions. Real Rust/Python repository observations are
  recorded in the Phase 2 report. Global toolchain setup unchanged.
- Independent Phase 2 static review found seven blocking findings; all are
  addressed in the review-fix pass. Final canonical verifier passed with 41 unit
  and 33 integration tests; all-target check, release build and 28 repository /
  Rust-Python fixture smoke invocations passed locally.
- No cloud/provider, cross-platform CI or independent executable verification
  claimed. The independent review could not run the Rust toolchain.

## Known limitations

Phase 4 is committed two-dot comparison only, with no working-tree/triple-dot,
compiler-grade impact, symbol-rename lineage or cross-file binding. Diff analyzes
changed sides only; impact inventories tracked scoped trees regardless of checkout
ignore rules. Incomplete/syntax-damaged sides suppress symbol comparison;
ambiguity/incomplete relation extraction suppresses relation comparison for that
file pair. Impact includes changed-file neighborhoods, processes base first, and
shares 128 roots / 10,000 visits / 25,000 relation occurrences across both sides.
Repository-wide diff enumeration precedes scope filtering to retain cross-boundary
rename evidence, so unrelated changes can exhaust the diff safety ceiling.


Imports remain unresolved. Unqualified method/nested calls remain unresolved
unless their syntax exactly matches a unique qualified name. Call-site references
and same-file matches are syntax heuristics, not binding/type/dispatch or compiler guarantees. IDs are index-local
and may renumber across edits. Current-state graphs are rebuilt per command; revision commands build immutable
Git-object models. No persistence,
concurrency, async, MCP, embeddings, model calls or policy behavior exists.
Relation budgets discard a failed file's extracted facts while retaining valid
symbols and structural containment. Queries expose truncation on safety overflow.
UTF-8 Rust/Python only; generated/vendor source skipped. Non-UTF-8 paths are
presented lossily and omitted from textual file-path search, with that condition
reported in query coverage. Existing parse recovery, scanner/Git limitations and
filesystem TOCTOU races remain; results are not atomic snapshots.

## Next step

Fresh independent Phase 4 re-review and final disposition. Phase 3 fixes still
require fresh independent re-review; Phase 1/2 final independent
dispositions also remain pending. Local verification does not constitute
independent acceptance of any phase.
