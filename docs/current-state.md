# Current state

**Last updated:** 2026-10-21

**Active phase:** Phase 9 — Local Service + Agent Adapter

**Overall status:** Phase 9 implemented; local verification recorded below; independent review required

## Delivered

- Phase 9 adds a synchronous application seam for search, exact symbol lookup,
  neighborhood, committed impact and full-file task context. CLI, HTTP and MCP
  invoke the same use cases and schema-v1 DTO builders; reconciliation warnings
  are returned as data and CLI alone renders them on stderr.
- `ctx serve` pins one canonical root, binds loopback only, requires a generated
  environment bearer token for intelligence, checks exact Host/rejects Origin,
  and exposes health plus five read-only v1 operations. Connections, readers,
  bodies, responses and deadlines are bounded. One completion-owned blocking
  permit survives caller timeout/disconnect; current queries reconcile each time.
- `ctx mcp` exposes the same five operations over stdio using official rmcp 3.6.0,
  MCP 2026-07-28 only, modern metadata/discovery and no initialize handshake.
  A narrow metadata guard/direct SDK service avoids dual-era bootstrap assumptions.
  Frame/output limits and outstanding-response bounds apply; stdout is protocol-only.
- The existing cache/schema/analysis versions and descriptive boundary are unchanged.
  Phase 9 also repairs inherited Phase 8 compile/test fixture errors discovered by
  executable regression verification. See the [Phase 9 report](reports/phase-9-implementation-report.md)
  for exact scope, SDK interoperability, security/concurrency tests and limitations.

- Phase 8 `history SYMBOL [PATH]` resolves committed HEAD symbols and follows
  bounded first-parent, exact-descriptor file lineage through Git rename evidence.
  `evolution TARGET --repo PATH` aggregates historical file/directory changes,
  available symbol/relation deltas and scoped same-commit co-change evidence.
- Shared Phase 4 snapshots/comparisons serve both commands. Dirty source and the
  current SQLite cache are excluded. Metadata, diff/path/source, structural work,
  event text, retention and output have command-wide bounds; commit-window
  truncation and coverage gaps are explicit. Ambiguous/unavailable lineage stops;
  shallow/grafted histories are rejected. Historical JSON is schema v2; persisted
  schema, analysis compatibility and freshness versions are unchanged.
- Phase 8 review fixes qualify path-lineage starts without claiming global symbol
  origin, distinguish partial observed symbol totals, expose native path identities,
  fail closed on incomplete supported-source HEAD lookup, report explicit history
  stop reasons, and reuse tree-provided blob sizes. See the
  [Phase 8 review-fix report](reports/phase-8-review-fix-report.md) and the original
  [Phase 8 implementation report](reports/phase-8-implementation-report.md).

- Phase 7 automatically reconciles trusted stale snapshots by native path and
  file-input stamp. Only changed/new eligible files are parsed; deleted groups
  disappear and renames become delete-plus-add. Full and incremental analysis use
  the same per-file primitive and aggregate-limit accounting.
- Schema v3 / `freshness-v2` / analysis compatibility v1 persist integrity-covered
  file stamps and sizes. Repository generations derive from canonical sorted
  stamps. SQLite file-local transactions validate the expected generation and
  roll back failures; conflicts and whole-input pre/post mutation retry once.
- Graphs reconstruct canonically from merged evidence. `index` remains force-full
  repair. `watch` registers before initial reconciliation, monitors parent ignore
  controls, coalesces filesystem hints through a bounded queue, and runs a 60-second
  safety reconciliation; overflow/errors rescan deterministically. The CLI
  alone emits watch status/warnings to stderr. No async/service/parallel parsing.
  Evidence: [Phase 7 report](reports/phase-7-implementation-report.md).

- Phase 6 adds a versioned external SQLite cache of file analysis, symbols,
  diagnostics and parser-extracted relation candidates. All current intelligence
  consumers share validated full-analysis reuse; graphs reconstruct through the
  existing constructor. `index` forces rebuild and requires a committed cache.
- The Phase 6 schema-v2 / analysis-compatibility-v1 / `freshness-v1` baseline validated native root/path
  identities, options, current inventory and eligible Rust/Python bytes. Pre/post
  build validation retries once on mutation. Corrupt/incompatible stores rebuild;
  automatic write failures warn on stderr while explicit indexing fails.
- No source bodies or graph IDs are stored. Caches stay outside the analyzed root.
  Scan/stats/tree remain live; diff/impact remain committed-revision analyses.
  `context --change` retains its snapshot gates. Phase 7 extends this cache with
  incremental updates. Evidence: [Phase 6 report](reports/phase-6-implementation-report.md)
  and [Phase 6 review-fix report](reports/phase-6-review-fix-report.md).

- Phase 5 `context [PATH] --task TASK --budget BYTES` assembles deterministic
  full-file UTF-8 bundles with contract-tested `file-ranker-v1` scores/reasons,
  matched symbols, exact content-byte accounting and skip-and-continue packing.
- Path/symbol relevance, depth-2 traversal, supporting file-role evidence and
  capped secondary centrality reuse the current graph. Graph scores exclude seed
  ownership and require a distinct file owner; current same-file-only resolution
  produces no cross-file graph boost or related-test candidate.
- Candidate content loads are capped at 256 attempts and 64 MiB of conservative
  read allowance, separate from selected-content bytes. Safety truncation and
  unattempted candidates are reported in human and schema-v1 JSON.
  Explicit centrality fallback, root/traversal/explanation/selection omissions,
  analysis gaps and content failures are reported in human/schema-v1 JSON.
- Optional `--change BASE..HEAD` requires known clean current HEAD matching the
  resolved range, rejects hidden index entries/scoped gitlinks, maps structural
  descriptors rather than numeric IDs, and rechecks compatibility after loading.
- Revised six-scenario benchmark: macro expected-file recall 1.0 for production,
  0.5 for existing path-and-symbol search, 0.0 for path order, 0.166667 for
  path-only, and 0.833333 with role scores removed. Each strategy uses the same
  loader, packing and byte budget. This synthetic fixture does not establish
  cross-file graph retrieval or general natural-language understanding.

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
- Synchronous `graph`, `deps`, `callers` and `related` commands reconstruct the
  in-memory graph from validated cached analysis or the current analysis build.
- Explicit index-local repository/file/symbol IDs; lexical range containment;
  atomic Rust/Python import facts; conservative call-site reference facts.
- Conservative same-file exact-qualified resolution, unresolved targets, separate
  extraction/resolution evidence, occurrence-preserving relations and adjacency.
- Cycle-safe iterative traversal, grouped exact-name roots, file filtering,
  bounded depth/results, explicit truncation and schema-v1 graph/query DTOs.
- Independent fail-closed relation budgets and hierarchical discovery/index/graph
  text, record and output limits; explicit query root omission accounting;
  relation-file accounting alongside every file's Phase 1 analysis status.

Evidence: [Phase 5 review-fix report](reports/phase-5-review-fix-report.md),
[Phase 5 implementation report](reports/phase-5-implementation-report.md),
[Phase 4 report](reports/phase-4-implementation-report.md),
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

- The maintainer directly authorized Phase 9 from `6693892`, despite pending Phase 8
  and earlier reviews, and requested one commit. This authorizes implementation,
  not acceptance. Canonical verification passed locally: formatting, Clippy,
  context checks, 106 unit / 78 integration tests. All-target check and release
  build passed. CLI/HTTP/MCP parity and release smokes passed on both real
  repositories' `src` scopes; the full engine root hit existing scanner bounds
  due to ignored build artifacts. Two APFS native-name fixture cases returned
  early; injected native-path tests ran. Details are recorded in the [Phase 9 report](reports/phase-9-implementation-report.md). Fresh independent
  Phase 9 review remains required. No earlier review disposition is changed.

- The maintainer directly authorized Phase 8 from `ef44712` despite earlier pending
  reviews. Local canonical verification and real-repository smoke evidence are
  recorded in the [Phase 8 report](reports/phase-8-implementation-report.md).
  Fresh independent Phase 8 review is required; no earlier phase is accepted here.

- The attached Phase 8 review findings were addressed in one follow-up change.
  Formatting, context checks and diff whitespace checks passed. The canonical
  verifier could not reach compilation because this environment cannot resolve
  crates.io and the available offline Cargo indexes lack required metadata. No new
  test, Clippy or cross-platform pass is claimed. Fresh independent Phase 8 review
  remains required; Phase 6/7 and earlier dispositions are unchanged.

- The Phase 7 independent-review handoff findings are addressed in one follow-up
  commit. Its canonical verifier passed with 99 unit / 61 integration tests. Fresh
  independent re-review remains required; this does not accept Phase 7. See the
  [Phase 7 review-fix report](reports/phase-7-review-fix-report.md).

- The maintainer directly authorized Phase 7 from `5e3ae18` while Phase 6 fresh
  re-review and earlier dispositions remain pending. This is implementation
  authorization, not independent acceptance. Phase 7 verification is recorded in
  the [implementation report](reports/phase-7-implementation-report.md).

- The maintainer directly authorized Phase 6 from `28e2f8a` despite pending earlier
  dispositions. This is implementation authorization, not independent acceptance.
- Phase 6 canonical verifier passed: 77 unit / 59 integration tests, formatting,
  Clippy and context checks. All-target check, release build, whitespace check and
  twenty isolated-cache release smoke invocations passed locally on macOS.
  Deterministic builder injection proves warm hits bypass analysis. Cold/warm
  command equivalence, mutation retry, storage validation/rollback, source edits,
  corruption recovery, clean Git `--change` and dirty rejection were exercised.
  No cloud, cross-platform or independent executable verification is claimed.
- The independent Phase 6 review findings have been addressed in a single follow-up:
  schema-v2 file-local storage keys, derived-payload integrity checking, best-effort
  automatic fallback on freshness read failures, compatibility guardrails, and the
  native-path/documentation gaps. Canonical verification for this follow-up is
  recorded in the [review-fix report](reports/phase-6-review-fix-report.md).

- The maintainer explicitly authorized Phase 5 from `4579d5b` despite pending
  Phase 4 fresh re-review and earlier final dispositions; this is not acceptance.
- Phase 5 canonical verifier passed: 65 unit / 56 integration tests, including
  all previous suites. All-target check, release build, diff whitespace check
  and eleven release smoke invocations passed locally on macOS using the existing
  isolated toolchain. Both formats were checked on real Rust/Python repositories
  and a clean Git fixture; dirty/mismatched snapshots failed with empty stdout.
  Fresh independent Phase 5 review remains required.
- The independent Phase 5 handoff findings are addressed in a follow-up: graph
  seed ownership is no longer scored as graph evidence, content loads have
  operation-wide caps, `file-ranker-v1` has a pinned contract, and the benchmark
  includes existing path-and-symbol search. The canonical verifier passes with
  71 unit / 56 integration tests; all-target check, release build, whitespace
  check, human/JSON repository smokes, and clean Git `--change` smokes pass
  locally. See the [Phase 5 review-fix report](reports/phase-5-review-fix-report.md).
  Fresh independent re-review remains required; this is not Phase 5 acceptance.

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

Phase 9 is local, single-root and serialized. Adapter context budgets stop at 1 MiB
of full-file content; response limits can reject larger evidence sets. HTTP timeouts
do not kill blocking work. Shutdown waits for that work without a hard deadline.
MCP is 2026-07-28 only; no legacy or HTTP MCP. Its 16 outstanding-response slots
include cancelled calls that produce no response, so repeated cancellations may
require reconnecting. Same-user/token-authorized HTTP clients and stdio clients
can obtain sensitive repository metadata/source; this is not a multi-user sandbox.
No third-party MCP client, IPv6, Linux/Windows or external deployment execution is
claimed. Native textual selectors retain existing UTF-8/lossy-path limitations.

Phase 8 uses a bounded first-parent window and exact current-HEAD descriptors;
full-DAG lineage, deleted-symbol lookup, symbol renames/moves, historical centrality
and historical persistence remain absent. HEAD lookup analyzes its scoped committed
inventory; older comparisons analyze selected file pairs. Git rename classification
uses the existing fixed 50% similarity policy. Evolution selectors retain their path
rather than follow file lineage. Coverage gaps and hard resource failures remain
explicit; co-change is only scoped commit co-occurrence. No Linux/Windows or
independent Phase 8 execution is claimed.

Phase 7 still scans and reads eligible source bytes on each request. Freshness
reserves at most 256 MiB per pass; stores have additional size/record limits.
If source freshness cannot be established, automatic current-state queries bypass
the cache and return best-effort analysis with per-file `ReadFailed` status; if a
read fails after snapshot units were retained, reconciliation discards them and
rebuilds from a fresh live scan. Explicit `index` fails and incomplete results are
never persisted. All cache writers share a bounded per-cache sidecar lock; complete
filesystem snapshots remain absent. Watch monitors parent `.gitignore` and `.ignore`
controls for nested roots and performs a canonical full reconciliation at least
every 60 seconds to recover from missed backend events. There is no persisted event log or
service/query server. Windows/native-wide support is implemented but
not locally executed; non-UTF-8 store tests use injected native inventory records.

Phase 5 packs complete files under a byte budget, not tokens/snippets. With
repository containment excluded, resolved graph edges stay within files. Ranking
does not score a seed's own file as graph evidence and currently has no distinct
cross-file graph candidates. Directly matched tests are not coverage. No atomic
filesystem snapshot is promised. `--change` uses conservative Git cleanliness
metadata and rejects hidden index flags/submodules within scope. Content loads
reserve at most 64 MiB across 256 attempts; over-budget files may be skipped
before UTF-8 validation. Conservative 64 MiB output preflight can reject a large
otherwise-renderable bundle.

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
and may renumber across edits. Current-state graphs reconstruct from validated persisted analysis; revision commands
build immutable Git-object models. Watch callback/channel concurrency is narrow; Phase 9 isolates synchronous core
work behind async adapters. No embeddings, model calls or policy behavior exists.
Relation budgets discard a failed file's extracted facts while retaining valid
symbols and structural containment. Queries expose truncation on safety overflow.
UTF-8 Rust/Python only; generated/vendor source skipped. Non-UTF-8 paths are
presented lossily and omitted from textual file-path search, with that condition
reported in query coverage. Existing parse recovery, scanner/Git limitations and
filesystem TOCTOU races remain; results are not atomic snapshots.

## Next step

Fresh independent Phase 9 review of MCP interoperability, HTTP local security,
blocking/response ownership, cache races, disclosure boundaries and evidence parity.
Fresh independent Phase 8 review of immutable history, lineage, scope, bounds and
production integration. Fresh independent Phase 7 review of incremental storage, freshness, races, watcher
ownership and integration. Phase 6 fresh re-review remains pending.
Phase 5 fresh re-review remains required. Phase 4 and Phase 3 fixes still require
fresh independent re-review; Phase 1/2 final independent dispositions also remain
pending. Local verification does not constitute independent acceptance.
