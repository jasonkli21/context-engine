# Phase 3 Independent Review — Query and Repository Intelligence

**Repository:** `jasonkli21/context-engine`
**Reviewed commit:** `ff9ad64bd290f71c3c1b6536f0907ca32dfc351b` (`feat: implement phase 3 query and repository intelligence`)
**Baseline / parent:** `09efd226d49357869af4f9dcf66997eefced5a8a` (`fix: address phase 2 review findings`)
**Review target:** Phase 3 — Query and Repository Intelligence
**Review type:** independent static architecture/correctness review with executable-verification attempt
**Intended next consumer:** fresh Luna XHigh Codex fix session
**Recommended repository artifact path after handoff:** `docs/reviews/phase-3-independent-review.md`

---

## 1. Executive assessment

### Verdict

**READY AFTER FIXES**

The Phase 3 implementation is strong in its core algorithms and layering. Static reconstruction shows that:

- `search` uses the canonical bounded `RepositoryIndex` and does not construct relation/graph state;
- `neighborhood` reuses the Phase 2 iterative BFS and shared root/visited/relation budgets rather than creating a second traversal engine;
- the repository containment hub is excluded from neighborhood traversal without mutating the underlying graph;
- `hotspots` performs one relation pass and distinguishes resolved occurrence counts from distinct fan-in/fan-out;
- unresolved relations do not silently become resolved centrality evidence;
- containment does not inflate centrality or structural activity;
- existing `deps`, `callers`, and `related` schema-v1 result shapes remain unchanged despite the internal addition of distance/predecessor state;
- Phase 2's reviewed fixes for global resource limits, 128-root truncation, conservative same-file resolution, exact callee spans, and grouped Rust `self` normalization remain present in the inspected code;
- no Phase 4+ Git diff model, persistence, async/concurrency, server/MCP, embeddings, model calls, or policy semantics were introduced.

The blocking problems are instead concentrated in the **new Phase 3 output contracts and path-search semantics**. The new standalone query commands drop repository/analysis context that the existing index/graph outputs carried, which makes index-local IDs and negative/sparse results unsafe to interpret in isolation. There is also a cross-platform/lossy path-search mismatch and a human-output defect in evidence-backed test associations.

These defects should be fixed before Phase 3 acceptance and before Phase 4 builds change intelligence on top of the query layer.

### Finding counts

| Severity | Count |
| --- | ---: |
| Critical | 0 |
| High | 2 |
| Medium | 2 |
| Low | 0 |
| Non-blocking future considerations | 3 |

### Phase 4 readiness

**READY AFTER FIXES**

Blocking findings:

- `CE-P3-001` — Phase 3 query outputs are not repository-scoped/self-identifying.
- `CE-P3-002` — Phase 3 query outputs hide analysis incompleteness, making negative/sparse results ambiguous.
- `CE-P3-003` — file-path search uses native/lossy display text rather than the normalized path contract.
- `CE-P3-004` — human neighborhood output does not actually identify or evidence candidate test associations.

### Highest-risk themes

1. **Standalone query outputs lost evidence context.** Existing index/graph JSON carries repository root and analysis state; the new query DTOs do not.
2. **Negative results can look authoritative when analysis was incomplete.** A skipped/too-large/failed file can remove symbols/relations while `search`, `hotspots`, or `neighborhood` provide no coverage warning.
3. **“Exact file path” is not actually a stable normalized path identity.** Search matches `PathBuf::to_string_lossy()` rather than the normalized `/`-separated path representation emitted by the product.
4. **Human test-association output is not usable evidence.** It prints raw IDs without mapping those IDs to labeled visits or predecessor evidence.

### Executable verification status

Independent executable verification was **attempted but blocked by the review environment**.

Attempted:

```bash
cargo --version
rustc --version
```

Result: neither tool is installed in the review container.

Attempted:

```bash
git clone https://github.com/jasonkli21/context-engine.git ...
```

Result:

```text
Could not resolve host: github.com
```

The connected GitHub repository interface remained available for exact commit/file inspection. GitHub reported no workflow runs and no combined status checks for the reviewed commit.

Therefore this review does **not** independently claim that `./scripts/verify.sh`, `cargo check --all-targets`, or `cargo build --release` passed. The Phase 3 implementation report states that its local canonical verifier passed with 43 unit and 37 integration tests, followed by all-target check, release build, and 20 release smoke invocations. Those remain implementation-session evidence rather than reviewer-executed evidence.

---

## 2. Reviewed scope and revisions

### Provenance

The reviewed Phase 3 implementation is exactly one commit ahead of the documented Phase 2 review-fix baseline:

```text
09efd226d49357869af4f9dcf66997eefced5a8a
  -> ff9ad64bd290f71c3c1b6536f0907ca32dfc351b
```

The repository's `main` branch currently points to `ff9ad64...`. No later commits were present in the connected repository at review time.

The implementation/current-state documents correctly preserve the distinction between maintainer authorization and independent Phase 1/2 acceptance. No provenance finding was identified.

The connected repository interface cannot expose a local developer working tree, and cloning was blocked by DNS, so current `git status` cleanliness was not independently verified.

### Phase 3 changed files

```text
README.md
docs/current-state.md
docs/plans/phase-3-implementation-plan.md
docs/reports/phase-3-implementation-report.md
src/cli.rs
src/graph.rs
src/main.rs
src/output.rs
src/output_graph.rs
src/output_query.rs
src/query.rs
tests/intelligence.rs
```

### Mandatory documents inspected

- `AGENTS.md`
- `docs/current-state.md`
- `docs/plans/phase-3-implementation-plan.md`
- `docs/reports/phase-3-implementation-report.md`
- Phase 3 production code/tests
- `docs/architecture.md`
- Phase 3 / query portions of `docs/prd.md`
- `docs/reports/phase-2-implementation-report.md`
- `docs/reviews/phase-2-independent-review.md`
- `docs/reports/phase-2-review-fix-report.md`
- `docs/development/llm-workflow.md`

Additional cross-phase source inspected where needed included `src/index.rs`, `src/output_index.rs`, and the prior Phase 2 `src/graph.rs` query implementation.

---

## 3. Independent delivered-architecture reconstruction

### Search flow

Actual composition:

```text
CLI `ctx search`
  -> `main::search_command`
  -> validate query length
  -> `index::index_repository`
       -> bounded scanner
       -> supported Rust/Python source loading/parsing
       -> canonical `RepositoryIndex`
       -> no relation extraction
  -> `query::search`
       -> scan canonical file records
       -> scan canonical symbols
       -> exact/prefix/substring match tiers
       -> strongest reason per symbol
       -> deterministic tier + canonical-ID sort
       -> result truncation
  -> `output_query::{search_human, search_json}`
  -> shared bounded JSON writer for JSON
```

Important properties:

- no graph construction for search;
- no source-body/full-text search;
- no fuzzy/semantic scoring;
- candidate universe is bounded by the Phase 0/1 repository limits;
- returned matches are bounded to 200;
- search currently constructs graph-compatible `EntityId`s from the index layout without constructing a graph.

### Neighborhood flow

Actual composition:

```text
CLI `ctx neighborhood`
  -> `main::neighborhood_command`
       -> validate exact relative file selector when used
  -> `graph::build`
       -> canonical scanner/index/relation extraction
       -> canonical `RepositoryGraph`
  -> `RepositoryGraph::query(QueryKind::Neighborhood, ...)`
       -> file-root lookup OR qualified-first/local-fallback symbol lookup
       -> max 128 canonical roots
       -> existing iterative per-root BFS
       -> shared 10,000 visit / 25,000 relation-occurrence budgets
       -> neighborhood eligibility policy
            * repository-sourced containment excluded
            * file/symbol and symbol/symbol containment allowed
            * references both directions
            * imports forward only
       -> per-root shortest distance
       -> deterministic predecessor entity/relation
       -> unresolved outgoing evidence retained as leaves
  -> output adapter
       -> explicit visits/relations/test candidates
```

Repository-hub behavior is sound in the inspected implementation. All repository→file containment relations have source `EntityId(0)`, and the neighborhood eligibility rule excludes every relation sourced by the repository. A file/symbol cannot therefore travel backward into the repository and then out to unrelated files through containment.

The inverse behavior is also preserved: same-file lexical context remains traversable because file→symbol and symbol→symbol containment has non-repository sources.

### Hotspot flow

Actual composition:

```text
CLI `ctx hotspots`
  -> `graph::build`
  -> `query::hotspots`
       -> build entity->owning-file table
       -> allocate bounded per-entity metrics
       -> one graph relation pass
            * containment -> direct child counts only
            * resolved references -> occurrence + distinct fan-in/out
            * unresolved refs/imports -> unresolved components only
            * structural activity -> non-containment file incidence
       -> deterministic central-symbol sort
       -> deterministic structural-file sort
       -> independent symbol/file truncation
  -> explicit human/JSON output adapters
```

The metric arithmetic inspected is internally consistent:

- repeated resolved references increment occurrence counts but not distinct fan-in/out after the first distinct entity;
- self-loops count as one incoming and one outgoing occurrence and one distinct source/target where appropriate;
- unresolved targets never enter resolved fan-in/fan-out;
- same-file references increment both file incoming and outgoing component counts but structural activity only once;
- cross-file resolved evidence would count structural activity once for each associated file;
- containment is excluded from structural activity and reference centrality;
- counts cannot approach `usize` overflow under the current 100,000-relation cap.

### State ownership and resource boundaries

State ownership remains singular:

- scan/index owns repository analysis;
- graph owns entity/relation/adjacency state;
- query borrows index/graph state;
- output adapts domain/query results without parsing or walking the repository.

No second graph/index is introduced.

Resource controls retained/added in the inspected code include:

- Phase 0 discovery/warning/line-work ceilings;
- Phase 1 repository symbol/name/detail/diagnostic ceilings;
- Phase 2 graph entity/relation/text ceilings;
- 128 query roots;
- 10,000 visited entity occurrences across root groups;
- 25,000 emitted relation occurrences across root groups;
- 4 KiB search query text;
- maximum 200 search results;
- maximum 200 central symbols and 200 hotspot files;
- 64 MiB bounded JSON writer for the new query outputs.

No new obviously unbounded relation/entity vector or all-pairs algorithm was found.

---

## 4. Plan quality assessment

### What the plan got right

The Phase 3 product slice is coherent and appropriate for the PRD milestone:

- path/symbol search provides an entry point without pretending to be semantic search;
- neighborhood adds a safer, more useful local traversal while preserving Phase 2 `related`;
- structural centrality/hotspots expose transparent counts rather than opaque scores;
- test associations are deliberately conservative and allowed to be sparse;
- the phase does not smuggle in Git/change modeling or task-context ranking;
- the implementation seam (`scan/index/graph -> query -> output/CLI`) aligns with the durable architecture and is reusable for later consumers.

The repository-hub policy is a good Phase 3-specific traversal policy. It prevents containment explosion without changing the graph itself.

The centrality/hotspot definitions are honest enough for the current sparse resolver because the output explicitly labels them current/syntactic/structural and keeps unresolved evidence separate. They are not yet a strong semantic importance model, but that limitation is appropriately deferred to richer resolution/ranking phases.

### Plan gaps

The plan under-specified the **self-contained context required by a machine-readable query result**.

Its proposed Search/Neighborhood result shapes focused on matches/traversal but did not require the new standalone commands to carry the same repository scope and analysis-completeness context that existing index/graph v1 output already exposed. This omission directly enabled `CE-P3-001` and `CE-P3-002`.

The plan also described file-path matching as scan-root-relative path matching without resolving the existing distinction between:

- native `PathBuf` identity;
- normalized `/`-separated external path text; and
- lossy non-UTF-8 presentation.

That omission enabled `CE-P3-003`.

### Over-/under-scoping

No material Phase 4/5 over-scope was found.

The candidate-test-association feature is somewhat low-yield under same-file-only resolution, but the conservative policy is safe and does not require a separate future architecture. It should remain Phase 3 once its human evidence rendering is fixed.

### Downstream rewrite risk

After the four findings below are fixed, the architecture should survive Phase 4 and Phase 5 without an obvious query-layer rewrite.

Without those fixes, Phase 4/5 would likely have to retrofit repository/evidence scope into every query contract precisely when revision identity and ranking make that scope more important.

---

## 5. Acceptance-area audit

| Area | Status | Review conclusion |
| --- | --- | --- |
| Phase gate / provenance | **Pass** | Actual parent is `09efd22`; authorization is not rewritten as acceptance; `main` points at reviewed commit. |
| Query architecture / state ownership | **Pass** | Canonical index/graph reused; no parallel parser/graph/state owner. |
| Search candidate/match logic | **Partial** | Tiering/dedup/bounds are sound, but file-path matching violates normalized/lossless path semantics (`CE-P3-003`). |
| Search graph avoidance | **Pass** | Search uses `index_repository`, not relation extraction/graph construction. |
| Neighborhood root semantics | **Pass (static)** | File roots exact; symbols qualified-first/local-fallback; duplicate roots bounded. |
| Repository-hub exclusion | **Pass (static)** | Repository-sourced containment cannot bridge unrelated files. |
| Neighborhood distance/predecessor | **Pass (static)** | Iterative BFS records shortest distance and deterministic predecessor relation. |
| Shared traversal budgets | **Pass (static)** | Root/visit/relation budgets remain shared across root groups. |
| Structural metrics | **Pass (static)** | Occurrence/distinct semantics, unresolved handling, containment exclusion, ownership accounting are coherent. |
| Central-symbol ranking | **Pass (static)** | Documented tuple and deterministic ID tie break are implemented; zero resolved-evidence symbols omitted. |
| Structural hotspot ranking | **Pass (static)** | Current-graph structural semantics and component counts are explicit. |
| Test association derivation | **Partial** | JSON derivation is conservative; human presentation does not identify/evidence candidates (`CE-P3-004`). |
| Resource boundaries | **Pass (static)** | New work/result limits reuse lower-layer caps; no new unbounded graph/query collection found. |
| Complexity | **Pass (static)** | Search linear + sort; hotspot aggregation one relation pass; BFS iterative; no per-symbol full relation scan. |
| Determinism | **Pass (static)** | BTree collections/canonical IDs/explicit sort tuples prevent hash iteration leakage in inspected paths. |
| Output/API contracts | **Fail** | New query outputs lack required repository/ID scope and analysis completeness (`CE-P3-001`, `CE-P3-002`). |
| JSON byte ceiling | **Pass (static)** | New JSON routes through existing bounded writer before stdout. |
| Human escaping | **Pass (static)** | Query/repository-controlled text inspected is escaped. |
| Security / read-only trust boundary | **Pass (static)** | No target-code execution, network, language server, persistence, or write path added. |
| Phase 2 regression fixes | **Pass (static)** | Global limits, 128 roots, conservative resolver, callee spans, grouped `self` fix remain present. |
| Phase boundary | **Pass** | No accidental Phase 4+ features. |
| Test quality | **Partial** | Production-path Phase 3 fixture is useful, but it misses the four defects identified below. |
| Documentation truthfulness | **Partial** | Status/provenance is truthful; implementation report does not identify the new query-context omissions. |
| Reviewer executable verification | **Blocked** | No Rust toolchain; Git clone DNS blocked; no CI/status checks were present. |

---

## 6. Findings

### CE-P3-001 — New query outputs are not repository-scoped/self-identifying

**Severity:** High
**Origin:** `plan_gap`
**Affected files/components:** `src/output_query.rs`; `src/main.rs`; all Phase 3 human/JSON command contracts (`search`, `neighborhood`, `hotspots`)

#### Problem

The new standalone Phase 3 result DTOs omit the repository/scan-root identity that scopes every relative path and index-local ID.

`SearchDto` includes `query`, `kind`, `id_semantics`, matches, and truncation, but no root. `NeighborhoodDto` includes selector/traversal fields and raw entity/relation IDs, but no root and no explicit ID-semantics field. `HotspotDto` likewise includes entity DTOs/metrics without root or ID semantics.

The human renderers also do not print the repository root.

By contrast, the pre-existing index and graph v1 outputs include the repository root, and graph JSON explicitly includes the semantics of index-local IDs.

#### Why it matters

These IDs are explicitly not stable across edits and are only meaningful inside one rebuilt index/graph. Paths are scan-root-relative. A machine consumer cannot safely store, compare, join, or even identify a standalone Phase 3 result unless it knows which scan root produced it.

This is particularly important because Phase 4 will introduce base/head structural models and Phase 5 will consume query results for ranked context. Retrofitting scope later would create avoidable contract churn exactly when identity becomes more complex.

Severity is High because this is a core machine-contract/provenance defect in the query layer intended to become an agent substrate.

#### Concrete evidence

- `src/output_query.rs::SearchDto` has no repository root.
- `src/output_query.rs::NeighborhoodDto` emits `roots`, predecessor IDs, and relation IDs without repository scope or an ID-semantics note.
- `src/output_query.rs::HotspotDto` emits entity IDs through `EntityDto` but has no root/ID scope.
- Existing `src/output_index.rs::ReportDto` includes `root` and Git context.
- Existing `src/output_graph.rs::ReportDto` includes `root`, Git context, and `id_semantics`.

#### Expected behavior

Every standalone Phase 3 output must be self-scoped enough that a caller can correctly interpret relative paths and IDs without relying on the invocation shell/session.

At minimum, JSON should carry:

```text
root
id_semantics
```

for all three commands. Human output should also identify the repository root.

Optional Git metadata may be reused if useful, but do **not** claim it is an exact working-tree revision or invent Phase 4 revision semantics.

#### Fix direction

Introduce a small shared query-output context rather than duplicating fields independently, for example conceptually:

```text
QueryContext
- root
- id_semantics
- optional existing Git metadata (only if kept honestly qualified)
```

Reuse the existing root/path/ID wording where meanings are unchanged.

Do not change old Phase 0–2 schema-v1 command shapes.

#### Required regression test / validation

Add focused CLI/DTO assertions that:

1. `search`, `neighborhood`, and `hotspots` JSON each identify the analyzed root;
2. `neighborhood` and `hotspots` explicitly state index-local/non-lineage ID semantics;
3. two otherwise-identical temporary repositories produce results whose repository scope is distinguishable;
4. human output includes the repository identity without introducing control-character regressions.

#### Dependencies

Fix this first. `CE-P3-002` should extend the same shared query context/output seam rather than creating a second metadata block.

---

### CE-P3-002 — Negative/sparse query results hide incomplete repository analysis

**Severity:** High
**Origin:** `plan_gap`
**Affected files/components:** `src/output_query.rs`; `src/main.rs`; index/graph coverage presentation for `search`, `neighborhood`, `hotspots`; Phase 3 tests/docs

#### Problem

The new Phase 3 outputs drop the analysis-status, diagnostic, and scan-warning context that existing `index` and `graph` outputs provide.

As a result, a zero/sparse result cannot distinguish between:

- genuine absence under complete supported analysis; and
- missing evidence because relevant files were unsupported, generated/vendor-skipped, above `--max-parse-file-size`, parse-failed, analysis-limit-failed, relation-limit-failed, or affected by scan warnings.

A concrete static reproducer is:

```bash
ctx search hidden . --max-parse-file-size 0 --format json
```

against a non-empty Rust/Python file defining `hidden`.

The index will retain the file with a `too_large`-style status and no symbol, while `SearchDto` can return:

```text
matches = []
truncated = false
```

with no indication that the file was not analyzed.

Similarly, `hotspots` can return empty lists when all supported files were skipped, and `neighborhood` can expose only a file root with no relation/symbol evidence, again without any analysis-coverage signal.

#### Why it matters

Phase 3 is explicitly evidence-oriented. A coding agent or human can reasonably interpret an empty search as "not found" or an empty test association as "no related tests." Those interpretations are not justified when the underlying analysis was incomplete.

The existing graph/index commands already preserve this distinction through summaries, per-file statuses, diagnostics, and scan warnings. The new query layer silently loses that honesty.

Severity is High because this produces misleading absence semantics, one of the review prompt's explicit High-severity categories.

#### Concrete evidence

- `src/output_index.rs::ReportDto` carries `summary`, diagnostics, and scan warnings.
- `src/output_graph.rs::ReportDto` carries graph/index summaries, file/relation-file status, diagnostics, and scan warnings.
- `src/output_query.rs::{SearchDto, NeighborhoodDto, HotspotDto}` carry none of those coverage indicators.
- `query::search` and `query::hotspots` correctly operate on retained evidence; the defect is that output does not tell the consumer how complete that evidence is.

#### Expected behavior

A Phase 3 result must make incomplete analysis observable without requiring the caller to separately run `ctx index` or `ctx graph`.

The fix does not need to dump every file/diagnostic into every result. A compact, bounded summary is sufficient if it preserves the critical distinction.

At minimum expose counts/status such as:

```text
analysis_incomplete
supported/analyzed/skipped/failed file counts
relation-analysis failures/limit failures for graph-backed commands
scan warning count
```

If omitting warning/diagnostic detail, make that omission explicit and provide enough counts/status for a consumer to know the result may be incomplete.

For non-UTF-8/lossy path conditions, ensure the query result also surfaces the existing warning/qualification needed to interpret paths safely.

#### Fix direction

Factor/reuse compact coverage-summary helpers from the existing index/graph output logic rather than reimplementing competing status semantics in `output_query.rs`.

Keep query-domain structs independent of presentation/Serde. This is an output/application concern.

Do not turn ordinary per-file incompleteness into a fatal command error; the existing design intentionally retains partial supported evidence. The requirement is **honest qualification**, not fail-whole-repository behavior.

#### Required regression test / validation

Add production-path tests using real CLI composition:

1. fixture contains a Rust/Python symbol;
2. run `search` with `--max-parse-file-size 0`;
3. assert zero match is accompanied by explicit incomplete/skipped coverage;
4. run `hotspots` under the same condition and assert empty lists are qualified as incomplete analysis;
5. create a parse/analysis-failure fixture if existing fixtures make that easy and assert the failure count is visible;
6. verify complete fixtures report complete/zero-failure coverage deterministically;
7. verify JSON remains under the shared bounded serializer and human text remains escaped.

#### Dependencies

Implement on top of the shared output-context seam introduced for `CE-P3-001`.

---

### CE-P3-003 — File-path search does not use the normalized path contract and can overstate “exact” matches

**Severity:** Medium
**Origin:** `architecture_conflict`
**Affected files/components:** `src/query.rs::search`; shared path normalization/presentation utilities; search docs/tests

#### Problem

File candidates are matched using:

```text
file.relative_path.to_string_lossy()
```

while public output normalizes path components to `/` through the shared `relative(...)` formatter.

This creates two problems:

1. **Platform separator mismatch.** On Windows, the internal native path text may contain `\` while JSON/human output advertises `/`-separated paths. A user/agent can copy an emitted `src/graph.rs` path into `ctx search` and fail to receive the same prefix/exact semantics because search is comparing a different textual representation.
2. **Lossy non-UTF-8 semantics.** On platforms that permit non-UTF-8 paths, distinct byte paths can collapse to the same replacement-character text. The search result can label such a comparison `exact_file_path` even though the UTF-8 query is not an exact identity for the underlying native path.

The repository already recognizes that lossy presented paths may not be unique; Phase 3 search currently turns that presentation lossiness into domain matching semantics without qualification.

#### Why it matters

`search` is intended to be the Phase 3 entry point into repository exploration and should compose with the paths emitted by the rest of the CLI. “Exact file path” must have a stable, documented representation.

This is narrower than `CE-P3-001/002`, hence Medium severity, but it should be fixed before acceptance because it is a direct correctness/contract issue and would otherwise become part of the v1 query behavior.

#### Concrete evidence

- `src/query.rs::search` uses `relative_path.to_string_lossy()` for file matching.
- `src/output.rs::relative` and the graph/index/query output adapters emit component-joined `/` paths.
- Existing output code explicitly warns that lossy non-UTF-8 paths may not be unique, but the new search DTO drops that warning (`CE-P3-002`).

#### Expected behavior

Define one explicit textual path-search representation and use it consistently.

Recommended direction:

- match UTF-8 paths using the same normalized `/`-separated relative representation emitted to users;
- do not claim byte-identity `exact_file_path` for a lossy non-UTF-8 conversion without qualification;
- either omit non-UTF-8 file candidates from exact textual path matching with an explicit coverage signal, or add an explicit lossy/display-path distinction to the contract.

Do not introduce fuzzy filesystem lookup or source reads.

#### Fix direction

Create/reuse one pure normalized-relative-path helper at a domain-neutral boundary so query and output cannot diverge.

Avoid making `query` depend on the output module merely to reuse a renderer helper.

For non-UTF-8 paths, choose an honest minimal policy and document it. Do not silently preserve `exact_file_path` terminology over lossy text.

#### Required regression test / validation

- Nested relative paths must search by the same `/` representation that JSON emits.
- Add a Windows-targeted test or cross-platform helper unit test proving separator normalization.
- On Unix, construct two distinct non-UTF-8 path values that collapse under lossy display and verify the chosen policy does not present both as unqualified byte-exact identity.
- Ensure file search remains index-only and deterministic.

#### Dependencies

`CE-P3-002` should expose any non-UTF-8 omission/lossiness status in the result context.

---

### CE-P3-004 — Human neighborhood output does not provide usable evidence-backed test associations

**Severity:** Medium
**Origin:** `implementation_deviation`
**Affected files/components:** `src/output_query.rs::neighborhood_human`; Phase 3 human-output tests

#### Problem

The JSON neighborhood representation is joinable: every visit includes entity ID, distance, predecessor ID, and via-relation ID, and `candidate_test_associations` contains entity IDs.

The human renderer does not preserve that usability. It currently prints visit lines as labels/distances without entity IDs or predecessor information, then prints candidate associations as a debug-style vector of raw numeric IDs.

Conceptually the result looks like:

```text
distance 1: test_name (...)
candidate test associations: [17]
```

but the user is not shown that the labeled visit is entity `17`, nor the predecessor/via-relation evidence that makes it a candidate.

#### Why it matters

Evidence-backed test association is an explicit Phase 3 capability. The human CLI is a primary product surface. A numeric ID that cannot be mapped to a labeled visit in the same rendering does not actually tell a developer which test entity was associated, and it does not surface the structural evidence promised by the plan.

The implementation report's smoke runs did not expose this because the sampled neighborhoods contained no test associations.

#### Concrete evidence

- `src/output_query.rs::neighborhood_human` prints visits as `distance N: <label>` without entity IDs/predecessors.
- The same function prints `candidate test associations: {:?}` over a vector of numeric IDs.
- `NeighborhoodRootDto`/`VisitDto` in JSON already contain the information required to produce a useful human rendering.

#### Expected behavior

Human output should identify each candidate by label and make its evidence traceable.

A minimal coherent presentation could include:

```text
#17 distance 2: test_run (tests/test_x.py:10) via relation #42 from #9
...
candidate test association: #17 test_run (tests/test_x.py:10)
```

A full expanded chain is optional if predecessor/via IDs are printed consistently enough to reconstruct it from the same output.

Do not add filename/name-similarity heuristics or strengthen the association claim to coverage.

#### Fix direction

Reuse the already-retained `RootResult::predecessors` and entity labels.

Prefer an intentional human format over debug `Vec<u32>` output.

#### Required regression test / validation

Extend the existing fixture where the `helper` symbol has a root in `test_a.py`:

- run human `neighborhood`;
- assert the candidate association is named/labeled, not only numeric;
- assert the association can be tied to a visit/predecessor/via relation where applicable;
- assert repository-controlled text remains escaped;
- keep the JSON association semantics unchanged except for any metadata additions required by `CE-P3-001/002`.

#### Dependencies

None beyond avoiding conflicting formatting work with `CE-P3-001/002`.

---

## 7. Non-blocking future considerations

### FC-P3-001 — Avoid unnecessary Git probing if Phase 3 outputs do not use Git metadata

`main::options` sets `probe_git: true` for new query commands. Today the new outputs do not consume that Git metadata. After `CE-P3-001`, either reuse existing Git context honestly or consider disabling the probe for commands that do not need it. This is a latency/trust-surface cleanup, not a current correctness blocker.

### FC-P3-002 — Consider centralizing the index→entity ID layout before revision models expand

Index-only search manually constructs graph-compatible `EntityId`s from the current repository/file/symbol ordering, while graph construction/output decoding also know that layout. The plan explicitly allowed a deterministic mapping, so this is not a Phase 3 defect. If Phase 4 changes entity/revision layout, factor a small authoritative mapping primitive rather than letting the formula proliferate.

### FC-P3-003 — Re-evaluate hotspot usefulness after cross-file resolution exists

The current structural metrics are honest, but same-file-only resolution and many unresolved references mean file activity can be dominated by syntactic density rather than architecture importance. Do not change Phase 3's transparent metric into an opaque score now; use Phase 4/5 evaluation data to decide whether additional signals improve utility.

---

## 8. Reviewed areas with no findings

The following areas were explicitly checked and no actionable Phase 3 finding was identified:

- exact/prefix/substring search tier ordering;
- strongest-reason deduplication for symbols matching both local and qualified names;
- file/symbol `--kind` filtering;
- search result-limit and query-byte bounds;
- zero search result process semantics;
- absence of source-body/fuzzy/semantic search;
- search avoiding relation extraction/graph construction;
- qualified-first/local-fallback symbol neighborhood root behavior;
- exact file neighborhood root lookup;
- 128-root retention/truncation logic;
- depth 0/1/max BFS semantics in the inspected code;
- iterative cycle-safe traversal;
- repeated relation occurrence preservation;
- unresolved outgoing evidence as leaves;
- repository containment hub exclusion;
- same-file lexical containment preservation;
- shortest-distance/predecessor recording;
- shared visit/relation budgets across roots;
- incoming occurrence versus distinct fan-in accounting;
- outgoing occurrence versus distinct fan-out accounting;
- self-loop accounting;
- unresolved target exclusion from resolved centrality;
- containment exclusion from reference centrality and structural activity;
- file source/target ownership accounting;
- module/file-owned unresolved evidence accounting;
- central-symbol tuple/tie determinism;
- zero-resolved-evidence central-symbol omission;
- hotspot current-graph/structural wording;
- same-file reference activity semantics;
- result truncation for central symbols/hotspot files;
- query/hotspot use of bounded candidate universe;
- absence of per-symbol full relation rescans;
- deterministic BTree/canonical-ID ordering in inspected query paths;
- shared bounded JSON serialization for all three new query commands;
- stdout-on-fatal-error behavior inherited through `main::run`/`write_output`;
- target-code execution/network/server/persistence/concurrency non-goals;
- Phase 2 root cap, resolver, callee span, grouped Rust `self`, and aggregate graph/index safety fixes;
- absence of accidental Phase 4+ implementation.

---

## 9. Verification evidence

### Reviewer-executed repository inspection

Verified through the connected GitHub repository interface:

- `main` points to `ff9ad64bd290f71c3c1b6536f0907ca32dfc351b`;
- its parent is `09efd226d49357869af4f9dcf66997eefced5a8a`;
- the compare range is one commit;
- the 12 changed files listed above match Phase 3 scope;
- there are no later commits in the connected repository at review time;
- no GitHub workflow runs were associated with the reviewed commit;
- no combined commit status checks were associated with the reviewed commit.

### Reviewer-executed local attempts

```bash
cargo --version
rustc --version
```

Blocked: commands not installed.

```bash
git clone https://github.com/jasonkli21/context-engine.git ...
```

Blocked: DNS/network unavailable in the execution container.

Therefore no independent local build/test/smoke command is claimed as passing.

### Implementation-report claims not independently reproduced

The Phase 3 implementation report records:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
```

as passing locally with 43 unit tests and 37 integration tests, plus 20 human/JSON release smoke invocations against Context Engine and Context Integrity. This review treats those statements as delivery evidence, not independent execution evidence.

### Static review confidence

Static confidence is high for the four findings because each follows directly from the delivered DTO/rendering/path-matching code and does not depend on runtime timing or platform-specific parser behavior. `CE-P3-003` includes a Windows-specific separator consequence that should be validated on a Windows target after the normalization fix.

---

## 10. Phase 4 readiness / final disposition

### Disposition

**READY AFTER FIXES**

The core Phase 3 algorithms do not need redesign. The required fix pass should be narrow and concentrated on:

1. making standalone query outputs self-scoped;
2. preserving analysis-completeness honesty;
3. making file-path search use one explicit normalized/lossless-enough contract;
4. making human test-association evidence usable.

After those changes and focused regression tests, Phase 3 should be suitable for a separate final independent verification/acceptance pass.

Do not begin Phase 4 merely because the implementation report's local verifier passed. Phase 4 should start only after these findings are fixed and Phase 3 receives its final disposition under the repository's review workflow.

---

## 11. Luna XHigh Fix Handoff

### Objective

Fix the four Phase 3 review findings with minimal coherent changes. Preserve the delivered query algorithms and all Phase 0–2 safety/evidence behavior. Do not expand into Phase 4 Git/change intelligence or Phase 5 ranking.

### Required read order

Before editing, read:

1. `AGENTS.md`
2. `docs/current-state.md`
3. `docs/plans/phase-3-implementation-plan.md`
4. this Phase 3 independent review
5. `docs/reports/phase-3-implementation-report.md`
6. affected production code:
   - `src/main.rs`
   - `src/query.rs`
   - `src/output.rs`
   - `src/output_index.rs`
   - `src/output_graph.rs`
   - `src/output_query.rs`
   - `src/graph.rs`
   - `src/index.rs`
7. `tests/intelligence.rs`
8. Phase 2 review-fix report/code only as needed to avoid regressions

Reproduce/understand each finding before editing.

### Ordered execution plan

#### Step 1 — Fix `CE-P3-001`: repository/ID scope

Likely targets:

```text
src/output_query.rs
src/main.rs                 # only if composition needs additional context
src/output.rs               # shared DTO/helper visibility only if justified
```

Implement one small shared query-output context carrying at least:

```text
root
id_semantics
```

for `search`, `neighborhood`, and `hotspots`.

Human output should identify the repository root as well.

If reusing current Git metadata, keep its current best-effort/non-atomic meaning. Do not invent a revision snapshot or Phase 4 identity model.

Do **not** mutate existing Phase 0–2 schema shapes.

Focused tests:

- all three new JSON outputs include root;
- neighborhood/hotspots state ID semantics;
- identical contents in two temp roots remain distinguishable by output scope;
- human root text remains escaped.

#### Step 2 — Fix `CE-P3-002`: analysis coverage honesty

Likely targets:

```text
src/output_query.rs
src/output_index.rs         # factor reusable summary logic only if clean
src/output_graph.rs         # factor reusable summary logic only if clean
src/index.rs / model types  # only if a tiny existing-status helper belongs there
```

Add a compact bounded coverage summary to each Phase 3 command.

For index-backed `search`, expose enough status to distinguish complete analyzed evidence from unsupported/skipped/too-large/failed analysis.

For graph-backed `neighborhood`/`hotspots`, also expose relation-analysis incompleteness/limit failures and scan warning presence.

Prefer reusing established status meanings over inventing new ones.

Do not turn recoverable per-file incompleteness into a fatal command.

Focused production-path tests:

```text
fixture defines hidden()
ctx search hidden ROOT --max-parse-file-size 0 --format json
```

must return zero matches **and** explicit incomplete/skipped coverage.

Likewise verify an empty hotspot result under skipped analysis is qualified.

#### Step 3 — Fix `CE-P3-003`: path-search representation

Likely targets:

```text
src/query.rs
possibly a small shared path utility module/helper
src/output.rs only if an existing pure helper can be moved/reused cleanly
```

Use one documented normalized `/`-separated relative text representation for searchable UTF-8 file paths, matching the product's emitted path representation.

Choose an honest non-UTF-8 policy:

- either omit those files from exact textual path matching and report that through coverage metadata; or
- explicitly label matching as lossy/display-path matching rather than byte-exact path identity.

Do not silently call lossy replacement text an exact native path.

Focused tests:

- nested `/` path round-trip from output to search;
- cross-platform normalization helper behavior;
- Unix non-UTF-8 ambiguity policy if supported by the test environment.

#### Step 4 — Fix `CE-P3-004`: human test-association evidence

Target:

```text
src/output_query.rs::neighborhood_human
```

Print visit entity IDs and predecessor/via-relation evidence consistently enough to make candidate IDs traceable, and print candidate associations by meaningful label rather than debug numeric vector only.

Do not change association derivation semantics.

Focused test:

- use the existing `test_a.py` fixture;
- assert human output names the candidate test entity/file and ties it to the retained structural evidence.

#### Step 5 — Focused regression pass

Re-run focused Phase 3 tests after each relevant fix, but do not create a large redundant test matrix.

Specifically preserve:

- strongest search tier/dedup;
- search index-only composition;
- 4 KiB query bound and 200-result cap;
- repository-hub exclusion;
- qualified-first/local-fallback roots;
- 128-root semantics;
- shared 10k/25k traversal budgets;
- deterministic distances/predecessors;
- metric occurrence/distinct counts;
- unresolved vs resolved separation;
- 64 MiB JSON ceiling;
- existing Phase 0–2 command schemas and tests.

#### Step 6 — Final verification

At the end of the fix session, run once:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
```

If the repository workflow/environment supports it, also run representative release commands:

```bash
./target/release/ctx search graph . --format json
./target/release/ctx neighborhood --symbol RepositoryGraph . --depth 2 --format json
./target/release/ctx neighborhood --file src/graph.rs . --depth 2
./target/release/ctx hotspots . --limit 10 --format json
```

Add one deliberately incomplete-analysis smoke case to confirm `CE-P3-002` behavior.

Do not claim any command passed unless it was actually run.

### Documentation updates

Create:

```text
docs/reports/phase-3-review-fix-report.md
```

It must record:

- baseline review/fix commit range;
- disposition for `CE-P3-001` through `CE-P3-004`;
- exact output contract changes;
- chosen non-UTF-8/path-search policy;
- focused tests added/changed;
- verification commands actually run and results;
- any environment blockers;
- confirmation that existing Phase 0–2 schemas/semantics were preserved.

Update:

```text
docs/current-state.md
```

to truthfully say Phase 3 review findings are fixed and final independent verification/disposition is pending.

Update `README.md` only if the public query path/output semantics materially changed in a way users need to know (particularly non-UTF-8 path-search behavior and incomplete-analysis qualification).

Do not rewrite this independent review's original verdict. Preserve it as review evidence.

### What not to change

Do not:

- add cross-file resolution to make associations/hotspots richer;
- add fuzzy/BM25/embedding/model ranking;
- add Git diff/change models;
- add persistent IDs/storage;
- add async/threads/server/MCP;
- alter Phase 2 `deps`/`callers`/`related` semantics;
- weaken any scanner/index/graph/query/output safety limit;
- turn test candidates into coverage claims;
- broaden the fix into a repository architecture rewrite.

### Definition of done

The fix pass is complete when:

- all four findings are resolved with focused production-path tests;
- every Phase 3 standalone result is repository-scoped and explicit about ID semantics;
- negative/sparse results expose analysis incompleteness instead of silently implying full coverage;
- file-path search uses one normalized, honestly documented path-text contract;
- human test associations identify the actual test entities and retained evidence;
- `./scripts/verify.sh` passes;
- all-target check and release build pass if the environment supports them;
- `docs/reports/phase-3-review-fix-report.md` exists;
- `docs/current-state.md` is truthful;
- no Phase 4+ scope is added;
- final independent acceptance remains a separate step.
