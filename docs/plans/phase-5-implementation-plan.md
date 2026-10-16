# Context Engine — Phase 5 Implementation Plan

**Phase:** 5 — Task Context Ranking\
**Status:** Planned; implementation gated on final Phase 4 acceptance or explicit maintainer authorization\
**Date:** 2026-10-15\
**Repository:** `jasonkli21/context-engine`\
**Planning baseline inspected:** `4579d5bcfef0a7b8e4bada93da8b0722c39d37d3` (`fix: address phase 4 review findings`)\
**Primary language:** Rust\
**Binary:** `ctx`\
**Primary new capability:** deterministic, explainable task-specific repository context selection under a hard content budget\
**Explicitly deferred:** embeddings/model calls, semantic vector search, persistence, incremental indexing, async/concurrency, daemon/API/MCP, token-model-specific budgeting, cross-repository context, history/blame, compiler/LSP-grade resolution

---

# 1. Planning Baseline and Gate

This plan is based on the actual `main` state inspected on 2026-10-15.

Latest inspected commit:

```text
4579d5bcfef0a7b8e4bada93da8b0722c39d37d3
fix: address phase 4 review findings
```

The initial Phase 4 implementation commit immediately before the review-fix pass is:

```text
c4ca002149c9d92ba8c0cb92091b85f98a8fc727
feat: implement phase 4 git and structural change intelligence
```

The current codebase already provides the Phase 5 substrate:

- bounded repository discovery and file-role/language classification;
- Rust/Python Tree-sitter symbol indexing with source ranges;
- fresh synchronous in-memory repository graphs;
- index-local file/symbol entities and explicit relationship evidence;
- deterministic path/symbol search;
- bounded graph neighborhood traversal;
- structural hotspot metrics;
- test-role awareness;
- safe bounded UTF-8 file loading through `source::load`;
- committed two-dot structural diff/change evidence;
- explicit Phase 4 base/head revision identities and conservative structural matching;
- shared versioned machine-output conventions and bounded output handling.

Current project status is intentionally more conservative than the code delta alone:

- Phase 4 independent review found three issues;
- commit `4579d5b` addresses them and passes local verification;
- fresh independent Phase 4 re-review/final disposition is still pending;
- earlier phase final dispositions also remain partially pending as recorded in `docs/current-state.md`.

Therefore:

> **Creating this plan does not accept Phase 4 and does not itself authorize Phase 5 implementation.**

Implementation may begin after either:

1. final Phase 4 independent acceptance/closure; or
2. explicit maintainer authorization to proceed despite the pending disposition.

A direct maintainer instruction to execute the Phase 5 implementation prompt is sufficient authorization, but the implementation report and `docs/current-state.md` must preserve the distinction between authorization and independent acceptance.

Do not reset, rewrite, or silently discard newer commits if the implementation session starts from a later `main`. Reconcile this plan with the actual code first.

---

# 2. Authority and Alignment

## 2.1 PRD intent

The PRD defines Phase 5 as:

```text
Task Context Ranking
```

with the user-facing capability:

```bash
ctx context --task "..." --budget ...
```

The expected deterministic signals are:

- lexical;
- symbol;
- graph;
- test;
- change proximity;
- centrality.

The Phase 5 exit criterion is not merely that the command runs. It is:

> A benchmark shows better expected-file recall under a fixed context budget than naive file selection.

The PRD explicitly says:

> No embedding requirement.

The broader context-bundle design also requires:

- structured context rather than an opaque concatenated dump;
- selection reasons;
- budget/cost accounting;
- repository/revision provenance where available;
- a future path to adapters without designing the core around any one LLM or MCP.

## 2.2 Architecture intent

The architecture describes the Phase 1–5 direction as:

```text
scan / parse / graph / Git-change
             ↓
      repository model
             ↓
      retrieval/ranking
             ↓
       context bundles
```

and the task-context flow as:

```text
task
 ↓
candidate retrieval
 ├── lexical
 ├── symbol
 ├── graph neighborhood
 ├── tests/docs
 └── change proximity
 ↓
scoring with reasons
 ↓
budget selection
 ↓
ContextBundle
```

The architectural principles remain binding:

- deterministic before probabilistic;
- evidence and provenance are inspectable;
- local-first;
- synchronous/process-local at this phase;
- repository code is untrusted input;
- machine-readable output is versioned;
- CLI/output concerns stay outside ranking/domain logic;
- Context Integrity remains a downstream policy/authority consumer, not part of ranking.

## 2.3 Newer repository guidance

`AGENTS.md`, `docs/README.md`, `docs/development/llm-workflow.md`, recent reports, and current code add practical constraints that the original roadmap did not spell out:

- extend existing production seams rather than building parallel abstractions;
- preserve bounded input, traversal, and output behavior;
- represent incomplete/unsupported evidence explicitly;
- keep current status, plans, delivery evidence, and independent review evidence separate;
- do not treat a plan as proof of implementation;
- do not create a new crate, daemon, framework, or trait hierarchy without present need;
- update `README.md` only for capability that actually ships;
- update `docs/current-state.md` only to describe the true post-implementation/review status.

---

# 3. Phase 5 Objective

Phase 4 answers:

> What changed structurally between committed revisions, and what structural impact evidence exists?

Phase 5 should answer:

> For a concrete engineering task, which repository files are the most useful context under a fixed budget, and why were they selected?

The first Phase 5 context bundle should be deliberately **file-granular**.

That means:

- files are the budgeted content units;
- symbol matches, graph relationships, test associations, change proximity, and centrality are **ranking evidence attached to files**;
- selected files include their bounded UTF-8 content;
- relevant symbols and structural evidence are reported as focus metadata;
- Phase 5 does not yet implement arbitrary snippet packing or a token-model-specific context compressor.

This is intentionally narrower than the mature conceptual `ContextCandidate` model. It directly matches the PRD's expected-file-recall acceptance metric, gives agents immediately usable source context, and avoids prematurely creating a complicated mixed file/symbol packing system before there is benchmark evidence for it.

Conceptually:

```text
task
 │
 ├── normalize bounded lexical terms
 │
 ▼
current RepositoryGraph
 │
 ├── path matches
 ├── symbol matches
 ├── graph proximity
 ├── test/docs role evidence
 ├── centrality
 └── optional reconciled committed-change proximity
             │
             ▼
      per-file score components
             │
             ▼
       deterministic ranking
             │
             ▼
   bounded safe content loading
             │
             ▼
       greedy budget selection
             │
             ▼
         ContextBundle
```

---

# 4. User Journey

Primary command:

```bash
ctx context [PATH] \
  --task "Add another model provider" \
  --budget 24000
```

Optional committed-change signal:

```bash
ctx context [PATH] \
  --task "Update impact reporting for changed methods" \
  --budget 24000 \
  --change main..HEAD
```

Machine-readable output:

```bash
ctx context . \
  --task "Add another model provider" \
  --budget 24000 \
  --format json
```

A useful human result should make it obvious:

- what task was interpreted;
- what budget unit is being used;
- what repository/snapshot was ranked;
- whether optional change evidence was used;
- which files were selected;
- how many content bytes each selected file consumes;
- why each file ranked highly;
- what high-value symbols were matched;
- whether graph/test/change/centrality evidence contributed;
- which high-ranked files were skipped because they did not fit or could not be loaded;
- whether ranking/traversal/output was truncated or incomplete.

The command must remain useful with no Git repository and no model/API key. `--change` is optional and Git-specific.

---

# 5. Product Boundary

## 5.1 In scope

Phase 5 implements:

1. a new `ctx context` CLI command;
2. bounded task validation and deterministic lexical normalization;
3. file-granular candidate ranking;
4. lexical path relevance;
5. symbol-name relevance aggregated to owning files;
6. graph-neighborhood relevance from task-matching seeds;
7. conservative test-role relevance;
8. documentation/config/schema role evidence where direct lexical relevance exists;
9. structural centrality as a bounded secondary signal;
10. optional committed change proximity from Phase 4 evidence;
11. deterministic integer score components with human-readable reasons;
12. stable rank/tie-breaking rules;
13. exact UTF-8 content-byte budgeting;
14. safe bounded loading of selected file contents through the established source boundary;
15. explicit skip/omission accounting;
16. human and schema-v1 JSON context-bundle rendering;
17. a deterministic offline ranking benchmark;
18. focused unit/integration/CLI coverage;
19. an implementation report and truthful status/docs updates.

## 5.2 Explicitly not in scope

Phase 5 must not add:

- embeddings;
- vector search;
- model calls;
- semantic summaries generated by an LLM;
- an external tokenizer dependency;
- model-specific token counting;
- arbitrary snippet extraction/packing;
- AST body summarization;
- persistent indexes;
- file watchers;
- background workers;
- Rayon/thread pools;
- Tokio/async;
- a daemon/API;
- MCP;
- editor integration;
- cross-repository context;
- history/blame/co-change intelligence;
- compiler/LSP/SCIP binding;
- new cross-file import/reference resolution;
- architectural policy or drift judgments;
- document authority/freshness semantics;
- automatic working-tree diff/change inference;
- a new graph database;
- a multi-crate workspace;
- a generalized retrieval framework with speculative providers/traits.

Phase 5 must not silently claim that its ranking is semantic understanding. It is deterministic evidence-based retrieval over the repository model that exists today.

---

# 6. Key Design Decisions

## 6.1 File-granular bundle v1

Phase 5 ranks **files**, not arbitrary chunks.

Reasons:

- the PRD acceptance criterion is expected-file recall;
- file ownership already exists for graph/symbol evidence;
- content loading can reuse `source::load`;
- full-file context is easy for humans and agents to interpret;
- byte cost is exact;
- mixed file/symbol deduplication and snippet replacement logic can be deferred until benchmark evidence shows it is necessary.

Symbol evidence remains first-class ranking metadata. A selected file should expose the strongest matching symbols/reasons so downstream consumers can focus quickly.

Do not reshape core symbol identity merely to support ranking.

## 6.2 Exact byte budget, not guessed tokens

For Phase 5:

```text
--budget N
```

means:

> maximum total UTF-8 bytes of selected raw file content.

Machine output must explicitly report:

```text
budget.unit = "utf8_content_bytes"
```

Rationale:

- exact and deterministic;
- no tokenizer/model dependency;
- no misleading approximate token claim;
- works for any future LLM/provider;
- keeps Phase 5 local/offline;
- the PRD conceptual model already allows token **or byte** cost.

Metadata/rendering overhead is **not** counted inside the context-content budget. It is separately subject to the repository's global output materialization/serialization safety bounds.

A future phase may add tokenizer-aware budgeting as another strategy without changing the core ranking model.

## 6.3 Scoring must be explainable and versioned

Do not use one opaque floating-point relevance number.

Represent score components explicitly, for example:

```rust
pub struct ScoreBreakdown {
    pub lexical_path: u16,
    pub lexical_symbol: u16,
    pub graph: u16,
    pub role: u16,
    pub change: u16,
    pub centrality: u16,
}
```

and expose a ranking version such as:

```text
file-ranker-v1
```

The total may be the saturating sum of bounded integer components.

Every non-zero component should have one or more stable reason codes such as:

```text
path_component_exact
path_substring
symbol_local_exact
symbol_qualified_prefix
graph_distance_1
related_test
direct_task_doc_match
changed_file
changed_symbol_owner
change_distance_1
centrality_fan_in
centrality_activity
centrality_fallback
```

The exact internal type names may differ, but the output contract must be explicit enough to explain selection without reverse-engineering weights.

## 6.4 Centrality is secondary

Centrality must not overpower task relevance.

A globally central file should not outrank a strong exact task match merely because it has many relationships.

Use centrality as:

- a small bounded score component;
- a tie-break/secondary relevance signal;
- an explicit fallback only when no task/change seeds exist.

When fallback is active, output must say so.

## 6.5 Reuse existing graph/query primitives

Do not build a second graph or duplicate traversal logic.

Phase 5 should reuse/refactor established helpers where sensible:

- `query::file_owners`;
- the structural metrics currently used by `query::hotspots`;
- `RepositoryGraph::traverse_roots` / existing bounded BFS semantics;
- existing repository-hub exclusion;
- existing traversal limits and predecessor/relation evidence.

A small refactor such as extracting reusable `structural_metrics(&RepositoryGraph)` from `hotspots` is appropriate if it reduces duplicate logic.

Do not change old query semantics merely to make ranking easier unless the change is independently correct and regression-tested.

## 6.6 Change evidence is optional and must be snapshot-compatible

Phase 4 deliberately exposes deterministic committed change evidence. Phase 5 should use it only when the user explicitly supplies:

```text
--change BASE..HEAD
```

The ranked content remains the **current filesystem snapshot** in Phase 5.

To avoid mixing incompatible snapshots, `--change` is valid only when:

1. the scoped repository is Git-backed;
2. the Phase 4 range resolves successfully;
3. the resolved range head equals the current repository `HEAD`;
4. the current worktree is known clean.

If any compatibility condition is false or cannot be established, fail clearly rather than mixing committed change facts with unrelated working-tree content.

This conservative restriction may be relaxed in a future committed-snapshot context mode.

Do not compare Phase 4 numeric entity IDs with current graph IDs.

Map change evidence structurally using:

- current/head path;
- symbol kind;
- qualified name;
- unique match requirements.

Ambiguous mappings must not receive a symbol-specific change boost.

Direct changed files can still receive file-level change evidence.

## 6.7 Content loading stays behind the existing safety boundary

Selected content must be loaded through the established path-safe bounded source loader or a very small shared extension of it.

Do not:

- call `std::fs::read_to_string` directly from output code;
- follow symlinks;
- execute repository code;
- invoke language runtimes;
- bypass size checks;
- read content from renderer code.

Content load failures are expected candidate-level outcomes:

- too large;
- invalid UTF-8;
- disappeared/unreadable;
- non-regular/race-related read failure.

They should not corrupt the entire ranking. Skip the file, record the reason, and continue to the next candidate unless a repository-wide safety invariant failed.

---

# 7. CLI Contract

Add:

```rust
Command::Context(ContextArgs)
```

Suggested shape:

```rust
pub struct ContextArgs {
    /// Natural-language task used for deterministic repository ranking.
    #[arg(long)]
    pub task: String,

    /// Maximum UTF-8 bytes of selected raw file content.
    #[arg(long)]
    pub budget: u64,

    /// Optional explicit committed two-dot range used as an additional ranking signal.
    #[arg(long)]
    pub change: Option<String>,

    #[command(flatten)]
    pub analysis: AnalysisArgs,
}
```

Resulting CLI:

```bash
ctx context [PATH] --task TASK --budget BYTES
```

with inherited:

```text
--format human|json
--max-parse-file-size BYTES
```

and optional:

```text
--change BASE..HEAD
```

### Validation

Recommended Phase 5 constants:

```text
MAX_TASK_BYTES                 4 KiB
MAX_TASK_TERMS                 64
MAX_CONTEXT_BUDGET_BYTES       16 MiB
MAX_CONTEXT_GRAPH_DEPTH        2
MAX_EXPLAINED_SYMBOLS_PER_FILE 8
MAX_REPORTED_SKIPS             50
```

Exact values may be adjusted if implementation evidence shows a better bounded value, but all externally meaningful limits must be documented and tested.

Validation rules:

- task must contain 1–4096 UTF-8 bytes;
- budget must be `>= 1`;
- budget must be `<= MAX_CONTEXT_BUDGET_BYTES`;
- `--change` must use the existing explicit two-dot grammar;
- no triple-dot/implicit working-tree change mode;
- path semantics remain those of `AnalysisArgs`.

Do not silently truncate the task string.

---

# 8. Domain Model

Prefer a new focused module:

```text
src/context.rs
```

or, if naming would conflict conceptually, `src/rank.rs`.

A practical internal model:

```rust
pub struct ContextRequest {
    pub task: String,
    pub budget_bytes: u64,
    pub change_range: Option<String>,
}

pub struct ContextBundle {
    pub task: String,
    pub ranker_version: &'static str,
    pub budget: BudgetUsage,
    pub repository: RepositoryContext,
    pub change: Option<ChangeContext>,
    pub selected: Vec<SelectedFile>,
    pub skipped: Vec<SkippedCandidate>,
    pub coverage: ContextCoverage,
}

pub struct FileCandidate {
    pub file_index: usize,
    pub score: ScoreBreakdown,
    pub total_score: u16,
    pub reasons: Vec<Reason>,
    pub matched_symbols: Vec<MatchedSymbol>,
    pub graph_distance: Option<u8>,
    pub change_distance: Option<u8>,
}

pub struct SelectedFile {
    pub file_index: usize,
    pub rank: usize,
    pub content_bytes: usize,
    pub content: String,
    pub score: ScoreBreakdown,
    pub reasons: Vec<Reason>,
    pub matched_symbols: Vec<MatchedSymbol>,
}

pub struct BudgetUsage {
    pub unit: &'static str,
    pub max: u64,
    pub used: u64,
}
```

Do not derive the external JSON contract directly from these domain structs merely because `serde` is available. Continue using explicit DTOs in output code.

The bundle should retain enough internal identity to avoid repeated path scans, but external output must keep current index-local identity semantics explicit.

---

# 9. Task Normalization

Task parsing must be deterministic and dependency-free.

Recommended algorithm:

1. validate task byte length;
2. preserve the original task for output;
3. lowercase Unicode deterministically using standard-library character lowering;
4. split on characters that are not alphanumeric and are not `_`, `-`, `.`, `/`, or `:`;
5. additionally split path-like/name-like separators into searchable components;
6. drop empty components;
7. drop one-character terms unless the full task itself is one character;
8. deduplicate terms while preserving a deterministic canonical order;
9. cap at `MAX_TASK_TERMS`;
10. preserve the normalized whole-task string as an additional phrase signal.

Do not introduce a large hard-coded English stopword list in Phase 5. It is easy to bias and difficult to justify. Unmatched generic task words naturally contribute zero.

If normalization produces no usable terms, ranking may use the documented centrality fallback.

Expose term-count/truncation information in coverage if a safety cap is reached.

---

# 10. Candidate Retrieval and Lexical Evidence

The candidate universe is the current graph's file entities.

Generated/vendor files should not be selected by default. They remain visible in coverage as excluded roles.

Other roles may be eligible:

- source;
- test;
- documentation;
- configuration;
- schema;
- migration;
- unknown, if safely loadable text.

## 10.1 Path matching

For UTF-8 paths, compare normalized task terms against:

- full normalized relative path;
- path components;
- file name;
- file stem.

Use simple deterministic tiers:

1. exact component/stem;
2. prefix;
3. substring.

Do not use edit distance, regexes from user input, or fuzzy libraries in Phase 5.

Non-UTF-8 paths are not eligible for lexical path matching because their lossy display is not a unique identity. They may still receive structural/change relevance through native internal identity.

## 10.2 Symbol matching

Match task terms against:

- symbol local name;
- symbol qualified name.

Aggregate symbol evidence to the owning file.

Important:

- cap the contribution from many matching symbols in one file;
- preserve the strongest few matched symbols for explanation;
- do not let a file with hundreds of similar generated symbols dominate only because of count;
- symbol name matching must not claim semantic equivalence.

A small refactor can reuse matching-tier helpers from `query.rs`, but Phase 5 may need case-normalized multi-term matching while the existing `search` command intentionally remains case-sensitive. Do not accidentally change `ctx search` semantics.

---

# 11. Recommended Ranker v1 Score

Use bounded integer components. The exact constants below are the preferred starting point; minor tuning is allowed only when benchmark evidence justifies it and the implementation report records the deviation.

## 11.1 Lexical path

Suggested capped contribution:

```text
exact stem/component        30
prefix                      18
substring                   10
normalized whole-task hit   +12
component cap               60
```

## 11.2 Symbol

Suggested tiers per strong symbol match:

```text
qualified exact             45
local exact                 40
qualified/local prefix      26
substring                   15
```

Aggregate only the strongest few matches and cap the file component around `80–90`.

## 11.3 Graph proximity

From the strongest lexical/change seeds:

```text
distance 0                  30
distance 1                  20
distance 2                  10
```

Use minimum observed distance for the file.

Repository containment edges must not make every file a distance-2 neighbor. Reuse current repository-hub exclusion semantics.

## 11.4 Role/test signal

Role is supporting evidence, not a generic popularity boost.

Suggested behavior:

- test file reached structurally from a task-relevant seed: `+12`;
- test file with direct symbol/path lexical relevance: `+8`;
- documentation/schema/config file with direct lexical relevance: `+6`;
- no unconditional test/doc bonus when otherwise unrelated.

## 11.5 Change proximity

When `--change` is valid and reconciled:

```text
current file directly changed          32
owner of uniquely mapped changed symbol 40
distance 1 from changed seed           20
distance 2 from changed seed           10
```

A single file should receive the strongest applicable direct change reason, not unbounded repeated boosts from every relation occurrence.

## 11.6 Centrality

Centrality should contribute only a small bounded amount, for example `0–10`, derived from saturated/bucketed existing metrics:

- fan-in;
- incoming references;
- fan-out/outgoing;
- structural activity.

Do not make raw degree directly additive.

## 11.7 Eligibility and fallback

Normal mode:

- include a file in ranked candidates only if it has lexical, symbol, graph, role-linked, or change relevance;
- centrality alone does not make every repository file a normal candidate.

Fallback mode:

- if there are no lexical seeds and no change seeds, rank a small bounded set of central files;
- mark `fallback_mode = "centrality"` explicitly;
- do not claim task-specific evidence that does not exist.

## 11.8 Stable ordering

Sort candidates by a documented tuple similar to:

```text
1. total score descending
2. lexical-symbol component descending
3. lexical-path component descending
4. change component descending
5. graph component descending
6. centrality component descending
7. native relative path ascending
```

Do not tie-break by hash-map iteration order.

---

# 12. Graph Relevance

## 12.1 Seeds

Create bounded ranking seeds from:

- direct task-matching symbol entities;
- direct task-matching file entities;
- optional current entities mapped from Phase 4 change evidence.

Prefer stronger symbol seeds before broad file seeds when root capacity is limited.

Deduplicate roots.

## 12.2 Traversal

Reuse existing bounded graph traversal semantics.

Phase 5 ranking depth should be fixed to:

```text
2
```

for the first implementation.

Do not expose another CLI depth knob until benchmark/user evidence shows it is useful.

Requirements:

- repository hub excluded;
- cycles handled by current traversal logic;
- minimum distance retained;
- traversal/root truncation exposed;
- traversal limits are global for the ranking operation, not reset per root;
- graph evidence never upgrades unresolved imports/references into resolved facts.

## 12.3 Mapping graph entities to files

Reuse a single owner map, not repeated full scans.

For symbol entities, map through symbol relative path to owning file.

For file entities, ownership is direct.

For graph results, a file's graph score is based on its minimum distance from any relevant root. Do not multiply graph score by the number of duplicate routes.

---

# 13. Test and Documentation Signals

Phase 5 should use what the current model can actually prove.

## Tests

The engine already classifies test-role files and can reach them through graph traversal.

A test candidate may be boosted when:

- it is directly task-matched;
- it is structurally reached from a task-relevant source/symbol;
- it is close to an optional reconciled change seed.

Output language must say:

```text
related test candidate
```

not:

```text
covers this code
```

unless actual coverage evidence exists in a future phase.

## Documentation/config/schema

Documentation is often not parsed into the current structural graph. Phase 5 should therefore rely primarily on:

- direct path lexical relevance;
- file-role classification;
- future structural evidence only where it actually exists.

Do not invent a `documents` relation or imply freshness/authority. Those concepts belong to later evidence and Context Integrity.

---

# 14. Optional Phase 4 Change Proximity

## 14.1 Invocation

```bash
ctx context . \
  --task "..." \
  --budget 24000 \
  --change main..HEAD
```

## 14.2 Compatibility gate

Before using change evidence:

1. build/obtain the current graph with Git metadata enabled;
2. analyze the explicit range using the Phase 4 path/scope semantics;
3. compare resolved range head commit to current graph/index Git `head_commit`;
4. require `is_dirty == Some(false)`.

If:

- Git metadata is unavailable;
- worktree cleanliness is unknown;
- worktree is dirty;
- range head differs from current `HEAD`;

return a clear command error.

Do not silently disable the requested change signal.

## 14.3 Structural mapping

Use only current/head-side facts that can be mapped safely.

File mapping:

```text
head/current relative path
```

Symbol mapping:

```text
relative path + kind + qualified_name
```

and only when unique in the current graph.

Do not compare index-local numeric IDs.

For modified/added symbols, current mapping can seed graph proximity.

For deleted-only symbols/files, report that the change exists but cannot directly select absent current content. The containing current file may receive a file-level boost only if it still exists under the head path.

Expose counts such as:

- changed files observed;
- changed symbols observed;
- current file mappings;
- current symbol mappings;
- ambiguous/unmapped symbols;
- deleted-only entities unavailable for content ranking.

## 14.4 No Phase 4 behavior regression

Do not alter `ctx diff` or `ctx impact` output semantics solely for Phase 5 convenience.

If a small reusable helper is extracted from `change.rs`, keep its existing behavior covered.

---

# 15. Budget Selection

After ranking, greedily walk candidates in deterministic rank order.

For each candidate:

1. determine remaining content budget;
2. attempt a bounded safe UTF-8 load;
3. if the complete file cannot fit, skip with `budget_exceeded`;
4. if loading returns `TooLarge`, skip with `too_large`;
5. if invalid UTF-8, skip with `invalid_utf8`;
6. if read failed/raced/non-regular, skip with `read_failed`;
7. otherwise select the whole file and subtract its exact UTF-8 byte length.

Do **not** truncate files mid-byte or mid-source in Phase 5.

Do **not** stop after the first over-budget file. Continue down the ranking so smaller useful files can fill the remaining budget.

Empty files cost zero content bytes but should not be selected unless they carry meaningful relevance; avoid a zero-cost flood by normal candidate/output count bounds.

Recommended candidate/report bounds:

```text
MAX_RANKED_CANDIDATES_INTERNAL <= repository file ceiling
MAX_SELECTED_FILES            <= 200
MAX_REPORTED_SKIPS            <= 50
```

Selection stops when:

- ranked candidates are exhausted;
- selected-file count reaches its safety limit; or
- no positive-budget candidate can be loaded.

Record counts for unreported skips.

---

# 16. Context Bundle Output Contract

Add a focused output module:

```text
src/output_context.rs
```

wired as a submodule of `output`, matching the existing index/query/change pattern.

## 16.1 JSON

Keep the existing envelope convention:

```json
{
  "schema_version": 1,
  "command": "context",
  "data": {}
}
```

Recommended `data` fields:

```json
{
  "repository": {
    "root": "...",
    "git_head": "...",
    "git_dirty": false,
    "id_semantics": "current_index_local"
  },
  "task": "Add another model provider",
  "ranker_version": "file-ranker-v1",
  "fallback_mode": null,
  "budget": {
    "unit": "utf8_content_bytes",
    "max": 24000,
    "used": 18342,
    "remaining": 5658
  },
  "change": null,
  "coverage": {
    "files_considered": 0,
    "files_eligible": 0,
    "task_terms": 0,
    "task_terms_truncated": false,
    "lexical_non_utf8_path_omissions": 0,
    "ranking_roots": 0,
    "ranking_roots_omitted": 0,
    "graph_truncated": false,
    "content_load_failures": 0,
    "budget_skips": 0,
    "reported_skips_truncated": false
  },
  "selected": [
    {
      "rank": 1,
      "path": "src/providers/registry.rs",
      "path_lossy": false,
      "language": "rust",
      "role": "source",
      "content_bytes": 2310,
      "score": {
        "total": 0,
        "lexical_path": 0,
        "lexical_symbol": 0,
        "graph": 0,
        "role": 0,
        "change": 0,
        "centrality": 0
      },
      "reasons": [],
      "matched_symbols": [],
      "content": "..."
    }
  ],
  "skipped": []
}
```

The exact field shape may be refined to fit current output conventions, but preserve:

- versioning;
- deterministic ordering;
- explicit budget unit;
- score breakdown;
- reasons;
- snapshot/Git identity;
- truncation/incompleteness;
- path-lossiness qualification;
- no false stable-ID claim.

## 16.2 Human

Human output should be useful as an inspectable context bundle.

Suggested structure:

```text
task: Add another model provider
ranker: file-ranker-v1
budget: 18342 / 24000 utf8_content_bytes
repository: ...
change signal: none
selected files: 5

1. src/providers/registry.rs
   score: 132
   reasons: symbol_local_exact, graph_distance_1, centrality_fan_in
   matched symbols: ProviderRegistry, register_provider
   bytes: 2310

--- content ---
...
--- end content ---
```

Repository content is untrusted.

Human rendering may preserve normal newlines/tabs for readability, but must escape terminal-dangerous control characters. Reuse or add a focused sanitizer rather than writing raw arbitrary control bytes to the terminal.

Do not let repository content inject fake top-level metadata headings without visible file boundaries.

## 16.3 Output bounds

The existing global 64 MiB output materialization/serialization ceiling remains binding.

Before constructing large owned DTO/rendered structures, estimate or enforce bounded growth in the same spirit as Phase 4.

The `--budget` maximum is intentionally far below the output ceiling, but metadata/reasons/skips still need explicit bounds.

Fatal output-size failure must not emit partial JSON.

---

# 17. Code-Level Change Map

Expected production changes are approximately:

```text
src/
├── cli.rs              add ContextArgs / Command::Context
├── main.rs             dispatch context command
├── context.rs          new ranking + bundle application/domain logic
├── query.rs            small reusable metrics/matching refactor if justified
├── graph.rs            reuse existing bounded traversal; avoid semantic change
├── change.rs           expose only a narrow reusable change-evidence seam if needed
├── source.rs           reuse current safe loader; tiny generalization only if required
├── output.rs           register context output submodule/shared bound helper
└── output_context.rs   human + schema-v1 context DTO/rendering
```

Tests:

```text
tests/
├── context.rs
├── context_fixtures/
│   └── ...
└── intelligence.rs     only if a shared regression belongs here
```

Documentation:

```text
README.md
docs/current-state.md
docs/reports/phase-5-implementation-report.md
```

Potentially update `docs/architecture.md` or create an ADR **only** if implementation discovers a genuinely durable architectural decision that changes the existing intent. Do not edit durable-intent docs just to say Phase 5 shipped.

Do not create a new handoff/status document.

---

# 18. Implementation Stages

## Stage A — Baseline and shared seams

Before edits:

1. record actual `HEAD`;
2. inspect `docs/current-state.md`;
3. inspect this plan;
4. inspect `cli.rs`, `main.rs`, `query.rs`, `graph.rs`, `change.rs`, `source.rs`, output modules, and relevant tests;
5. confirm whether Phase 4 has received a newer re-review/fix since this planning baseline;
6. run the canonical verifier if the environment supports it.

If newer changes exist, adapt. Do not reset to `4579d5b`.

Refactor only what is necessary to expose:

- reusable structural metrics;
- file ownership;
- bounded traversal;
- optional Phase 4 change descriptors.

## Stage B — Context domain and task parsing

Implement:

- request validation;
- task term normalization;
- ranker constants/version;
- candidate/reason/score types;
- deterministic ordering.

Add focused unit tests.

## Stage C — Lexical/symbol/graph/role/centrality ranking

Implement the current-state ranker without change evidence first.

Validate on deterministic fixtures.

At this point:

```bash
ctx context . --task "..." --budget ...
```

should work without Git.

## Stage D — Budgeted content loading and output

Implement:

- safe content selection;
- exact byte accounting;
- skip accounting;
- human output;
- schema-v1 JSON;
- output bound handling.

Add exact-boundary tests with injected small limits rather than huge fixtures.

## Stage E — Optional committed-change signal

Integrate `--change` behind the snapshot compatibility gate.

Use structural mapping, not numeric ID reuse.

Add small real-Git fixtures that verify:

- clean matching `HEAD` accepted;
- dirty worktree rejected;
- mismatched range head rejected;
- direct changed file/symbol boost;
- ambiguous symbol mapping does not invent a boost;
- deleted-only facts are qualified.

## Stage F — Benchmark

Add the deterministic benchmark fixture/runner described below.

Tune only bounded ranker constants if necessary. Do not add new feature classes merely to make the benchmark pass.

## Stage G — Verification, docs, report

Run canonical verification and release smoke checks.

Write the implementation report.

Update shipped user docs/status truthfully.

---

# 19. Focused Testing Strategy

Testing should be sufficient to prove ranking, safety, and production wiring without exploding into exhaustive combinatorics.

## 19.1 Unit coverage

Focus on:

- task validation and normalization;
- exact/prefix/substring tiering;
- score caps;
- deterministic tie-breaking;
- centrality saturation;
- candidate deduplication;
- byte-budget exact boundary;
- skip-and-continue behavior.

## 19.2 Integration coverage

Use small production-composition fixtures to prove:

- a symbol-name task selects the correct owning source file;
- graph proximity raises a structurally related file;
- a related test is surfaced without claiming coverage;
- direct documentation-path relevance can select a doc;
- centrality does not overwhelm a strong exact match;
- no-match fallback is explicit and deterministic;
- non-UTF-8 lexical omission is accounted for where platform tests permit;
- invalid UTF-8/unreadable/too-large content is skipped explicitly;
- selection continues after a large high-ranked file does not fit;
- human/JSON output is deterministic;
- repository control characters cannot inject unsafe terminal output;
- existing commands retain their old behavior.

## 19.3 Change-signal coverage

Use a tiny Git repository fixture.

Verify:

- explicit two-dot parsing remains Phase 4 compatible;
- matching clean head succeeds;
- dirty/unknown worktree compatibility fails closed;
- mismatched range head fails;
- modified/added symbol owners get change evidence;
- no numeric-ID cross-snapshot comparison;
- rename/head paths map correctly where unambiguous;
- deleted-only content is not fabricated.

## 19.4 Safety boundary tests

Use injected low limits for:

- task bytes;
- term count;
- budget bytes;
- candidate count;
- selected count;
- reported skip count;
- output bytes.

Do not allocate giant repositories only to hit limits.

---

# 20. Deterministic Ranking Benchmark

The benchmark is an explicit Phase 5 product acceptance artifact, not a micro-performance benchmark.

## 20.1 Goal

Measure:

> expected-file recall at an identical content-byte budget.

## 20.2 Dataset

Create a small dedicated benchmark fixture set with at least **6 task scenarios** spanning:

- Rust;
- Python;
- symbol-centric tasks;
- graph-dependent tasks;
- test-related tasks;
- documentation/config/schema relevance where realistic.

Each scenario defines:

```text
task
budget_bytes
expected_files[]
```

Expected files must be manually justified by the fixture's feature/task, not generated from the ranker.

Do not use the Context Engine production repository itself as the only benchmark. A self-referential benchmark is too easy to overfit and changes with development.

## 20.3 Baselines

Compare against at least:

### Baseline A — deterministic path order

Eligible UTF-8 files in canonical path order, greedily packed under the same byte budget.

### Baseline B — path-lexical-only

Use the same task normalization and file-path lexical tiers, but:

- no symbol evidence;
- no graph evidence;
- no test role signal;
- no change evidence;
- no centrality.

Pack under the same full-file byte budget.

Baseline B is the more important comparison because it tests whether Phase 5 intelligence beats simple task/path matching rather than merely beating alphabetical dumping.

## 20.4 Metric

For each scenario:

```text
recall = selected_expected_files / expected_files
```

Report:

- per-scenario recall;
- macro-average recall;
- total expected-file hits;
- budget used.

## 20.5 Exit requirement

The Phase 5 ranker must:

1. beat path-order baseline macro recall;
2. beat path-lexical-only baseline macro recall;
3. use exactly the same eligible-file/content-byte budget semantics;
4. report the benchmark result in the implementation report.

Do not manipulate baseline eligibility, file loading, or budget accounting differently from the Phase 5 ranker.

The benchmark may be implemented as:

- a deterministic Rust integration test plus fixture manifest; and/or
- a small repo script that calls the same production ranking code.

Prefer one source of ranking logic. The benchmark must not contain a shadow ranker.

No latency/performance claim is required in Phase 5.

---

# 21. Safety and Resource Bounds

Preserve all existing scanner/index/graph/change limits.

Add explicit Phase 5 ceilings for:

```text
task text
task terms
context content budget
ranking roots
graph visits/relations
selected files
reported skipped candidates
matched symbols/reasons per file
materialized output
```

Requirements:

- all user-controlled text is bounded before expensive work;
- task text never becomes a regex or shell argument;
- repository content is never executed;
- source paths remain scanner/index-produced identities;
- symlink/path traversal protections remain in the loader;
- non-UTF-8 display is qualified;
- map/set iteration never determines output order;
- ranking integer math is bounded/saturating;
- failure to load one candidate does not create false content;
- no partial JSON on fatal output failure;
- change mode fails closed on snapshot mismatch.

---

# 22. Documentation and Reporting

Implementation completion should produce:

```text
docs/reports/phase-5-implementation-report.md
```

The report must state:

- actual starting commit;
- implementation commit rule/identifier;
- authorization vs acceptance state;
- delivered architecture;
- exact CLI contract;
- ranker version and actual score constants;
- byte-budget semantics;
- optional change compatibility semantics;
- benchmark dataset/baselines/results;
- tests/verification actually run;
- release-smoke observations;
- known limitations;
- plan deviations;
- next independent review step.

Update:

### `README.md`

Only after the command actually ships. Document:

- `ctx context`;
- byte budget unit;
- deterministic reasons;
- optional `--change`;
- no embedding/model/tokenizer requirement;
- key limitations.

### `docs/current-state.md`

Record the actual Phase 5 status without declaring independent acceptance prematurely.

### `docs/prd.md` / `docs/architecture.md`

Do not rewrite merely because implementation exists.

Update only if implementation establishes a durable decision that makes the existing intent materially false or incomplete. If so, explain why in the implementation report.

### Historical docs

Do not rewrite prior phase reports/reviews to make them look current.

---

# 23. Acceptance Criteria

Phase 5 implementation is ready for independent review when all of the following are true.

## Product

- `ctx context [PATH] --task ... --budget ...` works on real Rust/Python repositories.
- It works without Git when `--change` is absent.
- It returns selected file contents under the exact byte budget.
- Every selected file has explainable ranking evidence.
- Results are deterministic for unchanged inputs/options.
- Strong task matches outrank unrelated central files.
- related tests are surfaced as candidates without false coverage claims.
- optional change proximity works only under snapshot-compatible conditions.
- no embeddings, model calls, persistence, or async are required.

## Ranking

- score components and reason codes are explicit;
- candidate/order caps are explicit;
- graph traversal is bounded and repository-hub-safe;
- numeric IDs are not compared across revisions;
- centrality is bounded;
- fallback mode is explicit.

## Budget/output

- selected raw content bytes never exceed `--budget`;
- a too-large candidate does not prevent later smaller candidates from being selected;
- invalid/unreadable content is explicitly skipped;
- human output is control-safe;
- JSON remains schema-v1/versioned and deterministic;
- fatal output overflow emits no partial JSON.

## Benchmark

- at least six deterministic scenarios exist;
- expected files are fixture-authored;
- both baselines use identical eligibility and budget semantics;
- Phase 5 beats both path-order and path-lexical-only macro recall;
- exact results are recorded.

## Regression

- canonical verifier passes;
- all-target check passes if used by current phase practice;
- release build passes;
- `git diff --check` passes;
- release smoke exercises human and JSON output;
- existing Phase 0–4 behavior remains green.

## Documentation

- implementation report exists;
- README describes shipped behavior only;
- current-state reflects implementation/review state truthfully;
- no competing handoff/status file is added.

---

# 24. Known Phase 5 Limitations to Preserve Honestly

Even after successful implementation:

- Rust/Python are still the structural parser set;
- imports remain unresolved;
- references are primarily same-file exact-qualified heuristics;
- graph relevance is syntactic/structural evidence, not semantic correctness;
- file-granular packing can waste budget on large files;
- byte budget is not a model token budget;
- docs are mostly path/role-relevant rather than structurally linked;
- test relevance is not coverage;
- current filesystem ranking is not an atomic snapshot;
- optional change ranking requires a clean current `HEAD` matching the range head;
- no embeddings/semantic vector retrieval;
- no history/co-change;
- no cross-repository context;
- no persistent index;
- no background refresh;
- no external agent protocol.

These are not Phase 5 failures unless the implementation claims otherwise.

---

# 25. Future Seams

Phase 5 should make later work easier without implementing it now.

## Phase 6 — Persistent index

The ranker should consume the repository/query model through ordinary data structures so persistence can replace rebuild cost without rewriting ranking.

Do not embed filesystem reads throughout scoring.

## Phase 7 — Incremental indexing

Candidate scoring should not require mutable global state. Later index generations should be able to feed the same ranking logic.

## Phase 8 — History/evolution

Historical/co-change signals can become additional explicit score components rather than hidden changes to lexical behavior.

## Phase 9 — service/adapters

`ContextBundle` should be usable by future API/MCP adapters without parsing human CLI text.

## Phase 10+ — cross-repository

Repository identity/provenance should remain explicit so multiple repository bundles can eventually be composed.

## Phase 11 — Context Integrity evidence

Ranking reasons and evidence should remain descriptive. Context Integrity may later decide whether selected context is authoritative/fresh/compliant, but Phase 5 must not import those policy concepts.

---

# 26. Do-Not-Overengineer Guardrails

Prefer:

- one new ranking/application module;
- one new output module;
- small helper extraction from existing query/change code;
- integer scores;
- `BTreeMap`/`BTreeSet` or explicitly sorted vectors where output order matters;
- existing file/symbol/graph identities;
- exact bytes;
- production-composition fixtures.

Avoid:

- a retrieval-provider trait;
- a ranker plugin framework;
- a generic feature-vector engine;
- dynamic weight configuration;
- YAML/TOML ranking configs;
- a tokenizer abstraction with no tokenizer;
- a vector-store seam;
- a new crate;
- a DAG/workflow engine;
- a background index;
- a second source loader;
- duplicate graph traversals;
- excessive snapshot abstractions.

Build what Phase 5 needs and keep the seams clean.

---

# 27. Verification

Use the repository's canonical verifier first:

```bash
./scripts/verify.sh
```

Then, consistent with recent phases:

```bash
cargo check --all-targets
cargo build --release
git diff --check
```

Run focused release smoke examples in both human and JSON forms, including:

```bash
ctx context . --task "context ranking" --budget 24000
ctx context . --task "context ranking" --budget 24000 --format json
```

and a deterministic clean Git fixture using:

```bash
--change BASE..HEAD
```

Do not claim cross-platform/cloud/provider/model verification that was not run.

---

# 28. Implementation Completion Handoff

The implementation session should end with:

1. implementation code/tests;
2. `docs/reports/phase-5-implementation-report.md`;
3. truthful README/current-state updates;
4. canonical verification results;
5. benchmark results;
6. one coherent implementation commit unless the maintainer explicitly requests another structure;
7. a fresh independent Phase 5 review as the next step.

The implementation report must make it possible for a fresh reviewer to reconstruct:

- what Phase 5 actually does;
- how ranking is composed;
- what snapshot is ranked;
- what budget means;
- how change evidence is reconciled;
- what the benchmark proves;
- what remains incomplete.

It must not claim independent acceptance.
