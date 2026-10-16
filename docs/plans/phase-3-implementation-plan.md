# Codebase / Context Intelligence Engine — Phase 3 Implementation Plan

**Phase:** 3 — Query and Repository Intelligence  
**Status:** Planned; implementation gated on final Phase 2 acceptance or explicit maintainer authorization  
**Date:** 2026-10-15  
**Primary language:** Rust  
**Binary:** `ctx`  
**Repository:** `jasonkli21/context-engine`  
**Planning baseline inspected:** `09efd226d49357869af4f9dcf66997eefced5a8a` (`fix: address phase 2 review findings`)  
**Prerequisites:** Phase 2 review-fix baseline, `docs/prd.md`, `docs/architecture.md`, current code/tests, Phase 2 implementation/review evidence  
**Primary new capabilities:** deterministic path/symbol search, generalized bounded neighborhoods, structural centrality/hotspot summaries, evidence-backed test associations

---

# 1. Current Repository Baseline

This plan is based on the actual `main` state inspected on 2026-10-15.

Latest inspected commit:

```text
09efd226d49357869af4f9dcf66997eefced5a8a
fix: address phase 2 review findings
```

Its parent is the Phase 2 implementation commit:

```text
16f7088b5e44621f0e4a3b076c6528003adbba9e
feat: implement phase 2 repository relationship graph
```

The repository currently has:

- the bounded Phase 0 scanner and Git metadata probe;
- Rust/Python Tree-sitter symbol indexing;
- project-owned per-file and repository-wide analysis limits;
- a fresh synchronous in-memory repository graph;
- repository/file/symbol entities with index-local IDs;
- lexical `contains` relations;
- unresolved Rust/Python import facts;
- conservative call-site reference facts;
- unique exact same-file qualified-name reference resolution only;
- forward/reverse graph adjacency;
- `graph`, `deps`, `callers`, and `related` commands;
- deterministic schema-v1 output and bounded graph/query JSON;
- explicit query root/traversal truncation;
- Phase 2 review fixes covering aggregate scanner/index/graph/query limits and evidence correctness.

The current state document still records Phase 2 final independent disposition as pending. Therefore:

> **The existence of the Phase 2 review-fix commit is not itself final Phase 2 acceptance.**

Phase 3 implementation may begin only after either:

1. final Phase 2 acceptance/closure; or
2. explicit maintainer authorization to proceed despite the pending final disposition.

Receiving a future implementation prompt directly from the maintainer may serve as that explicit authorization, but it must not be rewritten as evidence that Phase 2 was independently accepted.

---

# 2. Phase 3 Objective

Phase 2 made repository structure queryable. Phase 3 should make that structure **genuinely useful for interactive repository exploration** without yet introducing Git structural diffs, task-context ranking, persistence, concurrency, or model-assisted semantics.

The core question becomes:

> **Given the bounded repository/index/graph we already have, can a developer quickly find relevant entities, identify structurally important areas, and inspect a useful local neighborhood without manually browsing the repository?**

Phase 3 should turn the current low-level graph primitives into an explicit query/intelligence layer.

Conceptually:

```text
scan / index / graph
        │
        ▼
 bounded repository model
        │
        ▼
 Phase 3 query/intelligence layer
   ├── lexical entity search
   ├── structural metrics
   ├── neighborhood exploration
   └── evidence-backed test associations
        │
        ▼
 CLI + schema-v1 DTOs
```

This is still:

- local-first;
- synchronous;
- read-only;
- rebuilt per command;
- deterministic;
- evidence-oriented;
- LLM-free;
- non-persistent.

---

# 3. Product Boundary

Phase 3 is **query intelligence over the current repository model**.

It is not a semantic compiler or historical analyzer.

The phase may derive useful structural summaries from existing evidence, but it must not relabel heuristics as facts.

Examples of valid Phase 3 output:

```text
ModelRouter.route
  exact symbol search match
  incoming resolved call references: 6
  distinct resolved callers: 4
  outgoing resolved call references: 2
```

```text
src/router.rs
  structural activity: 19 non-containment relation occurrences
  unresolved imports: 4
  unresolved call references: 7
```

```text
Neighborhood of ModelRouter.route
  distance 0: ModelRouter.route
  distance 1: same-file referenced symbols + lexical parent/children
  attached unresolved evidence: provider.route
```

Invalid Phase 3 claims include:

```text
"This is the most important symbol in the codebase."
```

when the engine only has a sparse syntactic graph.

```text
"This file is a historical hotspot."
```

before Git/change history exists.

```text
"This test definitely covers this production function."
```

without explicit structural evidence.

Use terms such as:

- `structural centrality`;
- `structural activity`;
- `candidate test association`;
- `syntactic reference`;
- `current graph`;

rather than stronger semantic or historical wording.

---

# 4. Phase 3 Goals

By completion, Context Engine should:

1. provide deterministic file/symbol search without requiring an exact symbol name;
2. expose match reasons rather than a single opaque relevance score;
3. let a user enter the graph from either a symbol or a file;
4. provide a bounded local neighborhood that does not explode through the repository root containment hub;
5. expose useful per-symbol structural metrics;
6. identify structurally central symbols using explicit, inspectable ranking rules;
7. expose structurally active files using explicit component counts;
8. surface test-role entities only when the current evidence supports an association;
9. reuse existing scanner/index/graph construction rather than creating a parallel repository model;
10. preserve all Phase 0–2 safety, determinism, evidence, and output contracts;
11. add no persistence, async, threads, model calls, embeddings, or network requirements;
12. preserve a clean Phase 4 seam for Git/change intelligence and Phase 5 seam for task-context ranking.

---

# 5. Explicit Non-Goals

Phase 3 must not implement:

- Git structural diffing;
- `ctx diff` or `ctx impact`;
- history/blame/evolution analysis;
- task-specific context bundles;
- BM25, embeddings, vector search, learned ranking, or LLM ranking;
- full-text source-content search;
- fuzzy edit-distance search unless later evidence justifies it;
- cross-file module/package resolution;
- import alias binding;
- type inference or method dispatch;
- new language support solely for Phase 3;
- persistent indexes or `.ctx/` state;
- filesystem watchers;
- threads, Rayon, Tokio, async, daemon/API, or MCP;
- graph databases;
- architecture policy, authority, claims, or drift judgments;
- probabilistic test coverage claims;
- PageRank/betweenness merely to make the feature sound sophisticated.

Do not expand relation extraction just to make hotspot output look richer. Phase 3 should first prove value from the graph that Phase 2 actually delivers.

---

# 6. Rust Learning Objectives

Phase 3 should emphasize API/query design over new infrastructure.

Useful Rust practice:

- borrowing a repository/index/graph without duplicating ownership;
- small query-domain structs/enums;
- explicit ordering and tie-breaking;
- bounded result collection;
- reusable traversal policies;
- maps/sets for distinct fan-in/fan-out counts;
- deterministic aggregation;
- separation of domain/query results from serialization DTOs;
- avoiding lifetime complexity unless it naturally improves ownership.

Avoid introducing traits simply because a "query engine" sounds abstract. Plain functions and structs are preferred unless multiple implementations genuinely exist.

---

# 7. User Journeys

## US-3.1 — Find a partially remembered symbol or path

```bash
ctx search graph .
ctx search RepositoryGraph .
ctx search output_graph . --kind file
ctx search route /path/to/project --kind symbol --limit 10
```

The user should see deterministic file/symbol matches with explicit match reasons.

## US-3.2 — Inspect a symbol neighborhood

```bash
ctx neighborhood --symbol RepositoryGraph . --depth 2
```

The result should show the local graph neighborhood without using the repository node to fan out into unrelated files.

## US-3.3 — Inspect a file neighborhood

```bash
ctx neighborhood --file src/graph.rs . --depth 2
```

The result should expose contained symbols, nearby resolved references, and attached unresolved evidence.

## US-3.4 — Find structurally central symbols

```bash
ctx hotspots .
ctx hotspots . --limit 10 --format json
```

The user should see central symbols ranked by explicit fan-in/fan-out evidence, plus structurally active files.

## US-3.5 — Find evidence-backed test context

A neighborhood should surface test-role files/symbols only when they are genuinely reached through the allowed structural/reference neighborhood.

If no supported evidence connects a test entity, the result should be empty rather than inventing a path/name heuristic and presenting it as coverage.

---

# 8. Proposed CLI Surface

Add three commands:

```text
ctx search <QUERY> [PATH]
ctx neighborhood (--symbol <NAME> | --file <RELATIVE_PATH>) [PATH]
ctx hotspots [PATH]
```

Existing commands remain unchanged:

```text
ctx graph
ctx deps
ctx callers
ctx related
ctx index
ctx symbols
ctx symbol
ctx scan
ctx stats
ctx tree
```

Do not repurpose or silently change Phase 2 command semantics.

## 8.1 `ctx search`

Suggested options:

```text
<QUERY>                       required non-empty query text
[PATH]                        repository/directory, default .
--kind all|symbol|file        default all
--limit N                     default 20, maximum 200
--format human|json
--max-parse-file-size BYTES   existing analysis option
```

Phase 3 search is repository-metadata/symbol search, not source-text search.

## 8.2 `ctx neighborhood`

Exactly one root selector:

```text
--symbol <NAME>
--file <RELATIVE_PATH>
```

Plus:

```text
[PATH]
--depth N                     default 1, maximum 8
--format human|json
--max-parse-file-size BYTES
```

Symbol selection retains the existing qualified-first/local-fallback behavior and existing bounded multiple-root semantics.

File selection uses exact scan-root-relative identity. No fuzzy file root selection in the neighborhood command; use `search` first.

## 8.3 `ctx hotspots`

Suggested options:

```text
[PATH]
--limit N                     default 20, maximum 200
--format human|json
--max-parse-file-size BYTES
```

Human output must call these **structural** hotspots/central symbols.

---

# 9. Query Module Boundary

Phase 3 is the first point where a dedicated query/intelligence module is justified.

Recommended direction:

```text
src/query.rs
```

It may own:

- search enums/results;
- generic entity-root resolution helpers;
- neighborhood traversal policy/results;
- structural metric aggregation;
- hotspot ranking;
- evidence-backed test association summaries.

Recommended dependency direction:

```text
scan/index/graph domain
        │
        ▼
      query
        │
        ▼
 output DTOs / CLI
```

The query layer must not:

- read source files;
- invoke Tree-sitter;
- walk the filesystem;
- call Git;
- print;
- serialize itself directly merely by deriving `Serialize` on domain structs.

`graph.rs` should remain responsible for graph construction/storage/adjacency. Phase 2 traversal may be factored into reusable internal helpers if doing so reduces duplication, but do not perform a broad refactor solely to move code between files.

---

# 10. Search Semantics

Phase 3 search should be simple, deterministic, and explainable.

## 10.1 Searchable entities

Search:

- file entities by the normalized `/`-separated scan-root-relative path emitted by
  the CLI. Only UTF-8 paths participate in textual path matching; any omitted
  non-UTF-8 file paths must be reported in output coverage context;
- symbol entities by `qualified_name`;
- symbol entities by local `name`.

Do not search:

- source bodies;
- diagnostic messages;
- import target text;
- Git history;
- docs content.

Those are separate future features.

## 10.2 Match tiers

Use explicit match classes rather than an opaque floating-point score.

Recommended tiers, strongest first:

```text
1. exact_qualified_symbol
2. exact_local_symbol
3. exact_file_path
4. qualified_symbol_prefix
5. local_symbol_prefix
6. file_path_prefix
7. qualified_symbol_substring
8. local_symbol_substring
9. file_path_substring
```

Within the same tier:

- preserve deterministic canonical ordering;
- use entity ID only as a final tie-break for the current graph/index;
- do not imply cross-command ID stability.

Case sensitivity should remain exact/case-sensitive in Phase 3. Do not add Unicode case-folding or fuzzy matching unless there is a concrete requirement.

## 10.3 Duplicate match handling

A symbol that matches both local and qualified forms should appear once with its strongest match reason.

## 10.4 Result bounds

Recommended limits:

```text
MAX_SEARCH_QUERY_BYTES = 4096
DEFAULT_SEARCH_LIMIT = 20
MAX_SEARCH_LIMIT = 200
```

The existing repository/index ceilings bound the candidate universe. Search should avoid duplicating candidate strings unnecessarily; retain entity IDs/references plus match reason.

Return:

```text
truncated: true
```

when additional eligible matches exist beyond the requested/capped limit.

A zero-match search succeeds.

---

# 11. Search Data Model

Illustrative domain types:

```rust
pub enum SearchKind {
    All,
    Symbol,
    File,
}

pub enum SearchMatchKind {
    ExactQualifiedSymbol,
    ExactLocalSymbol,
    ExactFilePath,
    QualifiedSymbolPrefix,
    LocalSymbolPrefix,
    FilePathPrefix,
    QualifiedSymbolSubstring,
    LocalSymbolSubstring,
    FilePathSubstring,
}

pub struct SearchMatch {
    pub entity: EntityId,
    pub match_kind: SearchMatchKind,
}

pub struct SearchResult {
    pub query: String,
    pub kind: SearchKind,
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
}
```

The exact shape may change, but preserve the principles:

- entity identity is separate from search text;
- reason is explicit;
- results are bounded;
- no opaque semantic score.

`search` can operate on `RepositoryIndex` plus deterministic file/symbol-to-entity mapping if the graph is unnecessary. Prefer the cheaper index path when relationship data is not needed.

---

# 12. Generalized Neighborhood Semantics

The existing `related` command is a useful Phase 2 graph primitive, but it is symbol-only and can become unhelpfully broad through repository containment.

Phase 3 should add a purpose-built neighborhood traversal while preserving `related` unchanged.

## 12.1 Valid roots

- exact file entity;
- exact qualified/local symbol roots under existing bounded semantics.

## 12.2 Traversed evidence

Neighborhood traversal may follow:

- resolved `references` in both directions;
- lexical `contains` between files and symbols;
- lexical `contains` between symbols;
- unresolved imports/references attached to a visited source entity as leaves.

## 12.3 Repository-hub rule

The repository entity must **not** be used as a bridge from one file to every other file.

A neighborhood may expose the repository entity only if useful for representation, but traversal must not expand:

```text
symbol -> file -> repository -> every file
```

Recommended policy:

- skip repository→file containment as a traversable neighborhood edge;
- or stop expansion at the repository entity.

This is a Phase 3 query policy, not a mutation of the underlying Phase 2 graph.

## 12.4 Distance

New neighborhood results should retain distance from each root for resolved entity visits.

Distance is graph-query distance under the Phase 3 traversal policy, not semantic dependency distance.

## 12.5 Multiple symbol roots

Reuse the Phase 2 query-root ceiling:

```text
MAX_QUERY_ROOTS = 128
```

If more symbol roots match, retain canonical roots and expose `roots_truncated`.

An exact file root yields zero or one file entity.

## 12.6 Traversal bounds

Reuse the established Phase 2 limits unless measurement gives a concrete reason to change them:

```text
MAX_DEPTH = 8
MAX_VISITED = 10,000 entity occurrences across root groups
MAX_EMITTED = 25,000 relation occurrences across root groups
```

Do not create a second independent traversal-budget system.

---

# 13. Neighborhood Result Shape

Recommended conceptual result:

```text
NeighborhoodResult
- selector
- match_mode
- roots[]
- root_results[]
  - root
  - entities[]
      - entity_id
      - distance
  - relations[]
  - test_associations[]
  - truncated
- roots_truncated
- truncated
```

The new schema may include more explicit entity summaries than Phase 2 `related`, but should reuse the same graph entity/relation DTO concepts rather than invent incompatible meanings.

No source body is required.

---

# 14. Structural Metrics

Phase 3 should compute metrics from the current graph without implying compiler or historical truth.

## 14.1 Symbol metrics

For each symbol entity, compute at least:

- incoming resolved reference occurrence count;
- distinct incoming source entity count (`fan_in`);
- outgoing resolved reference occurrence count;
- distinct outgoing resolved target count (`fan_out`);
- unresolved outgoing reference occurrence count;
- unresolved outgoing import occurrence count where the symbol lexically owns imports;
- direct lexical child symbol count.

Containment should not inflate reference centrality.

## 14.2 File metrics

For each file, aggregate at least:

- symbol count;
- resolved incoming reference occurrences targeting symbols in the file;
- resolved outgoing reference occurrences whose source belongs to the file;
- unresolved reference occurrences owned by file/symbols in the file;
- unresolved import occurrences owned by file/symbols in the file;
- total non-containment relation occurrences associated with the file (`structural_activity`).

Define exactly whether a cross-entity relation contributes to both source and target file counts. Prefer explicit component fields over a hidden weighted score.

## 14.3 Complexity

Metrics should be computable in roughly:

```text
O(entities + relations)
```

plus bounded sorting for returned rankings.

Do not perform per-symbol full-graph scans.

---

# 15. Central Symbols

Phase 3 may call a symbol "central" only according to a transparent current-graph rule.

Recommended ranking tuple:

```text
fan_in descending
incoming resolved reference occurrences descending
fan_out descending
outgoing resolved reference occurrences descending
canonical symbol order ascending
```

This favors symbols observed as referenced by multiple distinct source entities.

Do **not** collapse the tuple into an unexplained weighted float.

Do not include zero-evidence symbols merely to fill a requested limit. An empty central-symbol list is valid when the current graph has no resolved reference evidence.

The output should explicitly note that unresolved/cross-file references are not treated as resolved fan-in.

---

# 16. Structural Hotspot Files

"Hotspot" in Phase 3 means current **structural activity**, not historical churn.

Recommended ranking:

```text
structural_activity descending
resolved incoming + outgoing reference occurrences descending
unresolved reference/import occurrences descending
symbol count descending
canonical file path ascending
```

Output all component counts.

Human heading:

```text
Structural hotspots (current graph)
```

Never:

```text
Historical hotspots
```

until Git/history phases exist.

---

# 17. Evidence-Backed Test Associations

The PRD says test associations "where feasible." Phase 3 should interpret that conservatively.

## 17.1 What qualifies

A test association may be surfaced when:

- an entity in a file classified `FileRole::Test` is actually reached by the allowed Phase 3 neighborhood traversal; or
- the queried root itself is in a test-role file.

The result should carry the graph/path evidence that caused it to appear.

## 17.2 What does not qualify

Do not claim a test association merely because:

- filenames look similar;
- both names contain the same token;
- an unresolved bare call happens to equal a local symbol name;
- a test imports a module-like string that might contain the target;
- both files live in the same directory.

Those may become future **test candidate ranking** signals, but they are not Phase 3 structural evidence.

## 17.3 Expected sparsity

Because Phase 2 intentionally lacks cross-file resolution, Phase 3 test associations may be sparse. That is acceptable and should be documented honestly.

The correct Phase 3 response to missing evidence is:

```text
no supported test association found
```

not invented coverage.

---

# 18. Output Contracts

All new machine-readable output uses the existing schema-v1 envelope:

```json
{
  "schema_version": 1,
  "command": "search",
  "data": {}
}
```

New commands may define new command-specific v1 data shapes without mutating existing Phase 0–2 command shapes.

Requirements:

- use explicit DTOs;
- do not derive serialization on core graph/query structs solely for convenience;
- reuse existing entity/symbol/range/evidence vocabulary where meanings are identical;
- stable deterministic order;
- no source bodies;
- human output escapes repository-controlled control characters;
- query/path text rendered in human output is escaped too;
- reuse the existing bounded JSON serialization path / 64 MiB ceiling;
- output overflow fails before partial JSON reaches stdout.
- every new query result identifies its analyzed root and states that entity IDs
  are index-local and have no edit or lineage stability;
- every new query result reports compact file-analysis coverage, diagnostic and
  scan-warning counts, and explicit omission of their details; graph-backed
  results also report relation-analysis status counts;
- negative or sparse results expose skipped/failed analysis and relation failures
  through an `analysis_incomplete` signal;
- human neighborhood output names each candidate test entity and shows the visit
  ID plus predecessor/relation evidence that makes the association traceable.

Do not create a second unbounded JSON serializer for the new commands.

---

# 19. Error and Empty-Result Semantics

Normal empty results are success:

- search with zero matches;
- neighborhood with zero matching roots;
- hotspots with no resolved reference evidence;
- no test associations.

Errors include:

- empty search query;
- query text over internal safety ceiling;
- invalid `--limit`;
- both/neither neighborhood root selectors supplied;
- neighborhood depth over maximum;
- malformed/absolute file selector if the implementation requires a scan-root-relative path;
- underlying scan/index/graph failure;
- output-size overflow.

Do not convert "nothing found" into a process failure unless the existing CLI convention explicitly requires it.

---

# 20. Resource-Safety Requirements

Phase 2 review demonstrated that per-file limits are insufficient. Phase 3 must preserve the full end-to-end bounded story.

## 20.1 Reuse existing lower-layer limits

Do not bypass:

- discovery/preflight limits;
- warning/detail limits;
- line/source loading limits;
- repository symbol/name/diagnostic limits;
- graph entity/relation/text limits;
- query root/traversal limits;
- JSON serialization limit.

## 20.2 New Phase 3 limits

Recommended:

```text
max search query bytes: 4 KiB
max returned search results: 200
max returned hotspot symbols: 200
max returned hotspot files: 200
```

Defaults should be smaller, e.g. 20.

## 20.3 Work bounds

Given the existing repository caps, a linear scan over files/symbols/relations is acceptable.

Avoid:

- all-pairs similarity;
- per-symbol rescanning of every relation;
- unbounded result groups;
- copying every candidate name/path into intermediate owned strings;
- recursive graph traversal;
- quadratic path/symbol matching.

If a sort needs all bounded matches/metrics, that is acceptable. Prefer clarity over premature heap/top-k machinery unless measurement justifies it.

---

# 21. Determinism Requirements

For unchanged repository state and identical CLI arguments:

- search match order is stable;
- search match reasons are stable;
- central symbol order is stable;
- hotspot file order is stable;
- neighborhood entity/relation order is stable;
- distance values are stable;
- test association order is stable;
- JSON field meanings/order from serialized structs remain consistent;
- human output uses stable tie-breaking.

Do not rely on `HashMap` iteration order.

---

# 22. Security and Trust Boundary

Phase 3 must preserve the existing read-only trust boundary.

It must not:

- execute project code;
- import Python modules;
- run Cargo/npm/project tests from the analyzed repository as part of analysis;
- evaluate config;
- invoke language servers;
- make network calls;
- follow external symlinks;
- upload code;
- trust repository-controlled control characters in terminal output.

Search and ranking operate only on already bounded in-memory metadata/evidence.

---

# 23. Recommended Code Organization

Likely touched/new files:

```text
src/cli.rs
src/main.rs
src/query.rs                 # new, if earned by implementation
src/graph.rs                 # small reusable traversal/lookup support only
src/output.rs                # shared helpers only if appropriate
src/output_query.rs          # new Phase 3 DTO/human rendering if useful
src/model.rs                 # only if a shared domain type truly belongs here
tests/intelligence.rs        # focused Phase 3 integration coverage
README.md
docs/current-state.md
docs/reports/phase-3-implementation-report.md
```

Possible existing test updates:

```text
tests/graph.rs
tests/cli.rs
```

Do not force this exact file layout if the actual code suggests a cleaner minimal composition. Any material deviation should be explained in the implementation report.

---

# 24. Implementation Work Packages

## WP-3.0 — Gate and baseline

Before edits:

1. read `AGENTS.md`;
2. read `docs/current-state.md`;
3. confirm explicit authorization if final Phase 2 acceptance is still pending;
4. run `./scripts/verify.sh`;
5. inspect the current `main` commit and reconcile any drift from `09efd226...`;
6. inspect Phase 2 graph/query/output code and tests;
7. preserve all seven Phase 2 review fixes.

Do not begin code changes if the baseline verifier fails for an unexplained reason or a newer blocking review exists.

## WP-3.1 — Query-domain seam

- create the minimal query module/domain types needed by Phase 3;
- reuse `RepositoryIndex`/`RepositoryGraph` ownership;
- add shared entity/file lookup helpers where needed;
- separate root resolution from traversal where that reduces duplication;
- preserve Phase 2 `related` behavior.

Focused tests:

- entity mapping;
- exact file lookup;
- multiple symbol root semantics;
- existing query regression.

## WP-3.2 — Deterministic search

- add `search` CLI arguments;
- implement file/symbol match tiers;
- deduplicate entities by strongest match;
- bound query length/result count;
- expose truncation;
- use index-only analysis path where graph relations are unnecessary.

Focused tests:

- exact/prefix/substring tier precedence;
- duplicate local/qualified match dedupe;
- deterministic ties;
- kind filter;
- exact result limit vs overflow;
- zero matches;
- control characters / odd Unicode are rendered safely without panics.

## WP-3.3 — Generalized neighborhood

- add symbol/file root selectors;
- implement reusable bounded traversal policy;
- record entity distance;
- block repository-hub expansion;
- attach unresolved evidence leaves;
- retain root/traversal truncation semantics.

Focused tests:

- file root;
- symbol duplicate roots;
- cycles/self-loops;
- depth 0/1/2;
- repository hub does not expose unrelated files;
- exact/over visited and emitted budgets with injected small limits;
- root truncation still explicit.

## WP-3.4 — Structural metrics and hotspots

- aggregate symbol fan-in/fan-out counts in one graph pass;
- aggregate file-level non-containment activity;
- rank central symbols by documented tuple;
- rank structural hotspot files by documented tuple;
- bound returned lists;
- expose all component counts.

Focused tests:

- repeated occurrences vs distinct fan-in/fan-out;
- containment excluded from centrality;
- unresolved facts do not become resolved fan-in;
- deterministic ties;
- zero-evidence graph;
- result cap/truncation.

## WP-3.5 — Test associations

- identify reachable test-role file/symbol entities in a neighborhood;
- carry the structural evidence/path that connects them;
- do not add filename or unresolved-name guessing;
- document expected sparsity.

Focused tests:

- explicit reachable test entity appears;
- unrelated test file remains absent;
- repository-hub traversal cannot make every test look related.

## WP-3.6 — Output/CLI integration

- add explicit schema-v1 DTOs;
- reuse existing symbol/entity/relation DTO meaning;
- ensure human control escaping;
- reuse bounded JSON serialization;
- update help text to distinguish structural/syntactic evidence from semantic/historical claims.

Focused tests:

- deterministic JSON snapshots/assertions for new shapes;
- output byte overflow uses existing failure behavior;
- human headings/labels are honest;
- existing Phase 0–2 JSON remains unchanged.

## WP-3.7 — Documentation and evidence

Update:

- `README.md` with shipped Phase 3 commands and limitations;
- `docs/current-state.md` to "Phase 3 implemented; independent review pending" or equivalent truthful state;
- `docs/reports/phase-3-implementation-report.md` with:
  - baseline and implementation commit;
  - actual architecture;
  - plan deviations;
  - limits;
  - test counts;
  - real-repository smoke observations;
  - known limitations;
  - Phase 4 handoff observations.

Do not change PRD/architecture merely to mirror implementation details. Update durable docs only if a real architectural decision changed.

---

# 25. Focused Testing Strategy

Testing should be strong but not excessive.

Use:

1. small unit tests for ordering/aggregation/traversal policies;
2. one focused Phase 3 integration fixture covering search, neighborhood, hotspots, and test-role evidence;
3. CLI tests for parsing/help/error/output boundaries;
4. existing Phase 0–2 regression suites unchanged where possible;
5. one final full repository verification.

Avoid:

- large combinatorial fixture matrices;
- duplicating the same behavior at three test layers;
- broad property/fuzz testing unless a concrete bug class justifies it;
- tests that only construct synthetic query objects while bypassing production composition.

Production-path integration matters more than test count.

---

# 26. Real-Repository Smoke Validation

After focused tests pass, build release and exercise at least:

```bash
ctx search graph .
ctx search RepositoryGraph . --kind symbol --format json
ctx neighborhood --symbol RepositoryGraph . --depth 2
ctx neighborhood --file src/graph.rs . --depth 2 --format json
ctx hotspots . --limit 10
ctx hotspots . --limit 10 --format json
```

Run equivalent useful commands against one additional real Rust/Python repository already available locally, preferably the sibling Context Integrity repository if present.

Record observations, not benchmark claims.

Useful observations:

- number of search matches and truncation;
- whether neighborhood results are actually local rather than repository-wide;
- central/hotspot entries and their component counts;
- test associations found or absent;
- single-run wall time only if clearly labeled as a smoke observation.

Do not infer performance SLAs from these runs.

---

# 27. Verification Gate

During implementation, run targeted tests for the active work package.

At completion run the canonical gate once:

```bash
./scripts/verify.sh
```

Then, if supported by the existing local environment:

```bash
cargo check --all-targets
cargo build --release
```

`./scripts/verify.sh` remains authoritative for the repository quality gate.

Do not claim CI/cloud/cross-platform verification that was not actually run.

---

# 28. Acceptance Criteria

Phase 3 implementation is ready for independent review only when all of the following are true.

## Gate / regression

- [ ] Phase 2 was accepted or explicit maintainer authorization to proceed was recorded.
- [ ] Pre-edit baseline verification passed or any unrelated baseline failure is documented.
- [ ] Phase 0–2 public behavior remains intact.
- [ ] All seven Phase 2 review-fix protections remain intact.

## Search

- [ ] `ctx search` searches file paths and symbol names without source-body scanning.
- [ ] exact/prefix/substring tiers are explicit and deterministic.
- [ ] duplicate entity matches collapse to the strongest reason.
- [ ] `--kind` works for all/symbol/file.
- [ ] result limits are bounded and truncation is explicit.
- [ ] zero results succeed.
- [ ] search does not build the graph unnecessarily.

## Neighborhood

- [ ] `ctx neighborhood --symbol` supports existing qualified-first/local-fallback roots.
- [ ] `ctx neighborhood --file` supports exact relative file roots.
- [ ] traversal is iterative/cycle-safe.
- [ ] entity distance is exposed.
- [ ] unresolved evidence is retained honestly.
- [ ] repository containment cannot fan the query out to all files.
- [ ] root/visited/relation limits are explicit and reuse established budgets.
- [ ] existing `related` semantics remain unchanged.

## Structural intelligence

- [ ] symbol fan-in/fan-out metrics distinguish distinct entities from repeated occurrences.
- [ ] unresolved evidence is not counted as resolved fan-in/fan-out.
- [ ] containment does not inflate reference centrality.
- [ ] central symbols use a documented deterministic tuple, not an opaque score.
- [ ] file hotspot output is labeled structural/current-graph, not historical.
- [ ] file output exposes component counts behind its ordering.
- [ ] zero-evidence repositories behave honestly.

## Test associations

- [ ] test associations require supported structural reachability or root membership.
- [ ] unrelated test files are not included by repository-hub traversal.
- [ ] filename/token similarity alone is not presented as coverage.
- [ ] empty test association output is allowed/documented.

## Contracts / safety

- [ ] new JSON uses schema v1 command envelopes with explicit DTOs.
- [ ] existing command schema shapes are not silently broken.
- [ ] new output reuses the existing bounded JSON path.
- [ ] human output escapes repository-controlled text.
- [ ] no source execution/network/persistence/concurrency was added.
- [ ] query work remains bounded by repository and Phase 3 limits.
- [ ] no obvious O(S×R) / O(N²) per-entity full-graph scanning exists.

## Documentation / evidence

- [ ] README documents shipped commands/limitations.
- [ ] implementation report records actual code, tests, deviations, limits, and smoke evidence.
- [ ] current-state truthfully records review status.
- [ ] PRD/architecture were changed only if durable intent/boundaries actually changed.
- [ ] canonical verification passes.

---

# 29. Review Risk Areas

The independent Phase 3 review should pay special attention to:

1. search matching that silently becomes fuzzy/semantic or non-deterministic;
2. search paths that accidentally perform graph construction/source rereads;
3. result truncation that omits evidence without reporting it;
4. neighborhood traversal accidentally crossing repository→all-files containment;
5. off-by-one depth/distance behavior;
6. multiple-root budgets multiplying work;
7. hotspot metrics counting containment or unresolved targets as resolved centrality;
8. repeated relations incorrectly treated as distinct source entities for fan-in;
9. an opaque score whose meaning cannot be audited;
10. test associations that overclaim coverage;
11. query code duplicating graph/index ownership or parser logic;
12. new JSON paths bypassing the Phase 2 output-size fix;
13. `HashMap` iteration leaking nondeterminism;
14. plan-compliant code that nevertheless weakens Phase 0–2 safety boundaries;
15. architecture that would obviously be thrown away when Phase 4/5 arrive.

---

# 30. Phase 4 Handoff Boundary

Phase 3 should leave Phase 4 with:

- a reusable query/intelligence layer;
- explicit entity search;
- generic local neighborhoods;
- structural metrics;
- deterministic output contracts.

Phase 4 may then add Git/change intelligence:

```text
base/head revisions
      ↓
structural change set
      ↓
changed entities/relations
      ↓
impact traversal
```

Phase 3 must not pre-build fake revision identities, Git diff models, or change scores merely in anticipation.

The only Phase 4-oriented design requirement is:

> **Do not make the Phase 3 query layer impossible to reuse over a future base/head structural model.**

That does not justify adding the Phase 4 model now.

---

# 31. Definition of Done

Phase 3 is implementation-complete when:

- the three proposed user-facing capabilities (`search`, `neighborhood`, `hotspots`) are integrated through the real scanner/index/graph path;
- the engine can answer useful repository exploration questions without exact file browsing;
- structural importance and test evidence are labeled honestly;
- resource/determinism/security guarantees from Phase 0–2 are preserved;
- focused tests and final canonical verification pass;
- a Phase 3 implementation report exists;
- `docs/current-state.md` records implementation complete / independent review pending;
- no Phase 4+ functionality has been smuggled into the phase.

Independent Sol High review remains required before Phase 3 should be treated as accepted.
