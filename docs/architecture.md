# Codebase / Context Intelligence Engine — Architecture

**Status:** Draft v1
**Date:** 2026-10-13
**Working binary:** `ctx`
**Primary language:** Rust
**Related:** `prd.md`, `plans/phase-0-implementation-plan.md`, Context Integrity

---

## 1. Purpose

This document defines durable technical boundaries for Context Engine. It supports two horizons:

1. a small Rust CLI that is understandable while learning Rust; and
2. a reusable local-first code-intelligence substrate that can later support syntax/symbol analysis, repository graphs, change intelligence, context ranking, cross-repository reasoning, coding-agent adapters, and Context Integrity.

The architectural rule that matters most is:

> **Model observable software facts cleanly and independently of the policy that interprets those facts.**

Context Engine should answer what exists, where it is, how it relates, what changed, and what evidence supports the result. Context Integrity may later decide whether those facts violate an ADR, are stale, or conflict with intended architecture.

---

## 2. Architecture goals

In priority order:

1. **Correctness and evidence.** Facts should be traceable to repository paths, source ranges, Git revisions, analyzers, or deterministic derivations.
2. **Small early architecture.** Phase 0 must not require async, persistence, parsing frameworks, graph databases, servers, embeddings, or model calls.
3. **Stable domain boundaries.** CLI concerns, scanning, language analysis, graph/query logic, persistence, and rendering should remain separable.
4. **Local-first operation.** The core requires no network or cloud service.
5. **Deterministic before probabilistic.** Parsing, Git, graph traversal, and search precede model inference.
6. **Replaceable external surfaces.** CLI, MCP, editors, local APIs, and Context Integrity should adapt the same core rather than duplicate it.
7. **Evolution without speculative abstraction.** Preserve useful seams, but add traits/crates/services only when current requirements justify them.

---

## 3. System shape

### Phase 0

```text
CLI
 │
 ▼
command dispatch
 │
 ▼
repository scan
 ├── traversal
 ├── language/role classification
 ├── measurements
 └── optional Git probe
 │
 ▼
RepositoryScan
 ├── human renderer
 └── versioned JSON renderer
```

Everything is synchronous and process-local.

### Phase 1–5 direction

```text
CLI / future adapters
        │
        ▼
application/query layer
        │
 ┌──────┼─────────────┬──────────────┐
 ▼      ▼             ▼              ▼
scan   parse         graph         Git/change
 │      │             │              │
 └──────┴──────┬──────┴──────────────┘
               ▼
        repository model
               │
               ▼
        retrieval/ranking
               │
               ▼
         context bundles
```

### Mature direction

```text
filesystem / Git / language analyzers
                 │
                 ▼
          ingestion / indexing
                 │
       ┌─────────┼─────────┐
       ▼         ▼         ▼
    syntax     change    search
     index     model     index
       │         │         │
       └─────────┼─────────┘
                 ▼
        repository/system graph
                 │
                 ▼
          query + ranking layer
                 │
      ┌──────────┼───────────────┐
      ▼          ▼               ▼
     CLI       MCP/API     Context Integrity
```

The mature diagram is directional, not permission to build those components early.

---

## 4. Dependency rules

Dependencies should flow from external interfaces toward application/domain logic, never the reverse.

Key restrictions:

- domain types must not depend on Clap;
- scan/index logic must not print directly;
- Git adapters must not choose presentation formatting;
- output rendering must not walk the filesystem;
- parser/graph code must not be designed around one CLI command or agent protocol;
- Context Integrity concepts such as claims, authority, policy, and drift findings must not become core code-intelligence types.

Prefer a function/module boundary over dependency injection or a trait hierarchy until multiple implementations or testing pressure justify polymorphism.

---

## 5. Repository organization

Start as one package and one binary target.

```text
src/
├── main.rs
└── cli.rs
```

Phase 0 grows toward:

```text
src/
├── main.rs
├── cli.rs
├── model.rs
├── scan.rs
├── classify.rs
├── git.rs
└── output.rs
```

Later, once actual size/ownership justifies it, modules may group under `domain/`, `language/`, `graph/`, `query/`, `rank/`, and `store/`.

Do not create a multi-crate workspace until a real library consumer, dependency boundary, or compile-time concern exists.

---

## 6. Phase 0 domain model

Recommended conceptual types:

```rust
pub struct ScanOptions {
    pub root: PathBuf,
    pub count_lines: bool,
    pub max_line_count_file_bytes: u64,
}

pub struct RepositoryScan {
    pub root: PathBuf,
    pub files: Vec<FileRecord>,
    pub directory_count: usize,
    pub git: Option<GitMetadata>,
    pub warnings: Vec<ScanWarning>,
}

pub struct FileRecord {
    pub relative_path: PathBuf,
    pub language: Language,
    pub role: FileRole,
    pub size_bytes: u64,
    pub line_count: Option<u64>,
}
```

### Language

Use an enum with an escape hatch rather than a trait hierarchy in Phase 0:

```rust
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Markdown,
    Json,
    Yaml,
    Toml,
    Shell,
    Sql,
    Other(String),
    Unknown,
}
```

### File role

```rust
pub enum FileRole {
    Source,
    Test,
    Documentation,
    Configuration,
    Schema,
    Migration,
    Generated,
    Vendor,
    Unknown,
}
```

Classification is heuristic. Do not imply compiler-level certainty.

### Git metadata

```rust
pub struct GitMetadata {
    pub repository_root: PathBuf,
    pub head_commit: Option<String>,
    pub branch: Option<String>,
    pub is_dirty: Option<bool>,
}
```

Git enrichment is optional; non-Git directories remain valid scan targets.

---

## 7. Identity and path semantics

Phase 0 file identity is the normalized path relative to the scan root. No global file ID is needed.

Rules:

1. user path defaults to `.`;
2. validate/canonicalize the root for internal identity where appropriate;
3. the **scan root** remains what the user requested and need not equal the Git root;
4. emitted file/tree paths are relative to the scan root;
5. traversal must not escape above the scan root;
6. directory symlinks are not followed by default.

Example:

```bash
ctx scan ./backend
```

should inspect `backend`, even if Git discovers a parent repository root.

Phase 1+ symbol identity should initially be revision-scoped. Do not promise stable identity through rename/refactor until lineage support exists.

---

## 8. Filesystem traversal

Use the Rust `ignore` crate for Phase 0 rather than reimplementing ignore semantics.

Expected default behavior:

- respect `.gitignore` and `.ignore` through the traversal backend;
- exclude `.git/`;
- do not follow directory symlinks;
- retain unsupported/unknown files as visible records rather than silently discarding them;
- treat unreadable individual files as warnings where practical, not whole-scan failures.

Potential large generated/vendor directories may later gain configurable treatment. Do not hide broad directories through undocumented hard-coded rules.

Fatal errors include invalid root / traversal cannot start. Recoverable errors include one unreadable file or optional Git probe failure.

---

## 9. Classification

Phase 0 language detection is filename/extension based.

Ordering:

1. exact special name;
2. compound extension;
3. simple extension;
4. unknown.

Role classification uses explicit path/name heuristics. Priority should be documented and deterministic, for example vendor/generated → test → migration/schema → docs → config → source → unknown.

Centralize classification in one module so Phase 1 analyzers can evolve independently from CLI/output code.

---

## 10. Line counting and measurements

Phase 0 counts physical text lines only. It does not claim semantic/source LOC.

Recommended behavior:

- count only recognized text/code/config/doc languages;
- use buffered streaming I/O;
- skip line counting above a configured file-size threshold;
- line-count failure records a warning and `None` rather than failing the scan;
- do not retain full file contents solely for counting.

Aggregate statistics should be computed deterministically from file records and use stable tie-breaking for sorting.

---

## 11. Git adapter

Use read-only `git` subprocess calls for basic Phase 0 metadata:

```text
git rev-parse --show-toplevel
git rev-parse HEAD
git branch --show-current
git status --porcelain
```

Rationale: the CLI is already ubiquitous, the metadata need is small, and a Git library would add complexity before history/diff requirements exist.

Failure to find Git, a detached HEAD, or a non-Git directory must degrade gracefully. A future phase may switch to `gix`/`git2` if query/history requirements justify it.

Do not introduce a `GitProbe` trait until a second implementation or testing difficulty makes it valuable.

---

## 12. Application boundary

The scanner should return structured data, never print directly.

Recommended API:

```rust
pub fn scan_repository(options: &ScanOptions) -> Result<RepositoryScan, ScanError>;
```

The CLI is responsible for:

1. parsing arguments;
2. invoking application functions;
3. selecting an output renderer;
4. printing the resulting string;
5. translating errors to exit status/messages.

No dependency-injection container is needed.

---

## 13. CLI and output contracts

Phase 0 commands:

```text
ctx scan [PATH]
ctx stats [PATH]
ctx tree [PATH]
```

Add `--format human|json` when real scan output is implemented.

Human output may evolve freely. Machine output should be explicitly versioned from its first public implementation:

```json
{
  "schema_version": 1,
  "command": "scan",
  "data": {}
}
```

Do not expose every internal struct merely because it derives `Serialize`; use output DTOs/envelopes as the external contract.

Determinism requirements:

- sort file paths;
- define ordering/tie breaks for language/role/largest-file views;
- avoid embedding current timestamps unless semantically necessary;
- repeated scans of unchanged state should produce stable machine output except explicit volatile metadata.

---

## 14. Error model

Use typed internal errors (`thiserror` is appropriate once scan modules are implemented). User-controlled filesystem/Git state should not cause panic.

Distinguish at least:

- invalid root;
- traversal/metadata/I/O error;
- optional enrichment failure;
- output serialization failure.

`unwrap`/`expect` are acceptable in tests and true programmer invariants, not for arbitrary repository input.

Recoverable scan problems should be surfaced in a warning collection or equivalent rather than silently swallowed.

---

## 15. Testing architecture

Use several test layers:

### Unit

- language classification;
- file-role classification;
- aggregation;
- path normalization;
- formatting helpers.

### Fixture integration

Small committed repository trees validate traversal, ignore rules, classification, tree shape, and determinism.

### CLI integration

`assert_cmd` verifies help, errors, human output, and versioned JSON.

### Later

- golden structural diff fixtures;
- graph invariant tests;
- retrieval benchmarks;
- parser fuzz/property tests where justified.

Tests should validate actual production composition rather than only hand-wired test objects.

---

## 16. Performance and concurrency

Phase 0 is sequential by design.

Do not add Rayon, Tokio, worker pools, custom mmap, or caching before a correct sequential baseline exists and real repository measurements show a need.

Safe early performance practices:

- stream traversal;
- avoid retaining file contents;
- skip large-file line counts;
- collect only metadata needed for outputs.

Performance claims must be benchmarked rather than inferred from the use of Rust.

Concurrency is deliberately introduced later as a learning milestone for incremental indexing.

---

## 17. Security boundary

Phase 0 is read-only. It may:

- list files/directories;
- read file metadata;
- read text for line counting;
- run read-only Git metadata commands.

It must not:

- execute project code;
- run package-manager scripts;
- import target modules;
- evaluate target configuration;
- follow symlinks outside the boundary by default;
- upload source.

Treat repository contents as untrusted input.

---

## 18. Future language-analysis architecture

Phase 1 may introduce a language analyzer abstraction only when parser implementations exist:

```rust
pub trait LanguageAnalyzer {
    fn language(&self) -> Language;
    fn analyze(&self, file: &SourceFile) -> Result<FileAnalysis, AnalysisError>;
}
```

A language-neutral result can contain `Symbol`, `Relation`, source ranges, evidence, and diagnostics. Tree-sitter node kinds should not leak throughout the rest of the domain.

Potential later evidence backends include Tree-sitter, LSP, compiler metadata, SCIP, and language-specific analyzers. Relationships should preserve derivation/quality so syntactic evidence is not falsely presented as compiler-resolved truth.

Do **not** implement this trait in Phase 0.

---

## 19. Future graph architecture

Phase 2 introduces:

```text
Repository
File
Symbol
Relation
EvidenceRef
```

An in-memory adjacency representation is sufficient initially. No graph database is required.

The graph needs both forward and reverse traversal, explicit relation evidence, and honest handling of uncertain/missing relationships.

Persistence is a later concern and should be selected from real query patterns rather than shaping early domain types around a database schema.

---

## 20. Future persistence / incremental architecture

Persistence begins only after repeated rebuild cost is demonstrated.

Candidate direction:

- SQLite for durable metadata/entities/relations;
- compact serialized indexes where useful;
- Tantivy only if full-text requirements justify it.

Before selecting storage, define actual query patterns, index sizes, invalidation rules, schema versioning, and rebuild behavior.

Incremental indexing later introduces:

```text
filesystem event
      ↓
change coalescing
      ↓
affected file analysis
      ↓
entity/edge invalidation
      ↓
re-index
      ↓
new index generation
```

Concurrency belongs there, not in Phase 0.

---

## 21. Future Git/change intelligence

Structural change analysis should eventually compare entity models across base/head revisions:

```text
Git diff
   ↓
changed files
   ↓
base/head analyses
   ↓
entity matching
   ↓
symbol/relation changes
   ↓
impact traversal
```

Phase 0 Git metadata is only a thin foundation and must not prematurely encode the future diff model.

---

## 22. Future task-context ranking

Task context should be compiled from evidence rather than repository dumping:

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

Ranking should expose why an item was selected. Embeddings may become an optional signal later; they are not a foundational requirement.

---

## 23. Context Integrity integration

Context Integrity is a downstream consumer, not an internal module.

### Context Engine owns observable evidence

- repository/file/symbol identities;
- source ranges;
- code relationships;
- revisions and structural changes;
- impact evidence;
- context candidates.

### Context Integrity owns normative semantics

- source authority;
- claims;
- supersession;
- intended architecture;
- policies/invariants;
- stale documentation judgments;
- drift/governance.

Future integration should occur through a stable evidence/query contract, not a shared database requirement.

Externally consumed evidence should eventually carry repository identity, revision, entity identity, file/range, derivation method, and analyzer version where relevant.

No duplicate policy layer should emerge in this Rust engine for convenience.

---

## 24. Dependency strategy

Add dependencies only when current code uses them.

### Phase 0 expected

- `clap` — CLI;
- `ignore` — traversal/ignore rules;
- `serde` / `serde_json` — JSON contract;
- `thiserror` — typed errors;
- `assert_cmd`, `predicates`, `tempfile` — tests.

The scaffold initially includes only dependencies needed by the code already present; Phase 0 implementation adds the rest when used.

### Explicitly deferred

- `tree-sitter` — Phase 1;
- graph library/database — Phase 2 only if useful;
- `rusqlite` — persistence phase;
- `notify` — incremental phase;
- `tokio` — daemon/service phase;
- `rayon` — only after measurement;
- embeddings/model SDKs — optional much later.

---

## 25. Rust engineering conventions

- stable Rust, no nightly requirement;
- safe Rust by default;
- `unsafe` requires measured need, rationale, and review;
- `cargo fmt --check`, Clippy with warnings denied, and tests form the quality gate;
- comments explain invariants/tradeoffs, not ordinary syntax;
- public contracts get documentation when non-obvious;
- avoid using `Arc`, `Mutex`, `Rc`, or `RefCell` to bypass ordinary Phase 0 ownership design problems.

The repository's canonical command is:

```bash
./scripts/verify.sh
```

---

## 26. Provisional architecture decisions

### ADR direction A — single package first

Start as one package/binary. Split only for real reuse or dependency boundaries.

### ADR direction B — synchronous first

Build a correct sequential baseline before concurrency/async.

### ADR direction C — use `ignore` for traversal

Ignore semantics are complex but not product differentiation.

### ADR direction D — Git CLI probe initially

Use read-only subprocess metadata until Git history requirements justify a library.

### ADR direction E — version machine output immediately

Future agents/Context Integrity need explicit compatibility boundaries.

### ADR direction F — no persistence in Phase 0

Full scan per command is acceptable initially.

### ADR direction G — LLM-free core early

The learning/product MVP derives value from deterministic evidence.

### ADR direction H — Context Integrity downstream

Authority/policy/drift semantics remain outside the engine.

These become formal ADRs only when the implementation reaches the decision and the choice remains durable.

---

## 27. Architecture invariants

Review every phase against these invariants:

1. scanning/indexing returns data; it does not print;
2. renderers do not walk/analyze repositories;
3. CLI structs do not become the core domain model;
4. externally emitted paths/revisions are explicit and stable enough for their advertised contract;
5. non-Git directories remain valid inputs;
6. one bad file should not normally abort a whole repository;
7. target repository code is not executed;
8. network access is not required for core intelligence;
9. machine-readable output is versioned;
10. ordering is deterministic for stable input;
11. Phase 0 remains sequential;
12. authority/policy semantics remain outside the core;
13. future adapters use shared application/query APIs rather than duplicate analysis;
14. unsupported evidence remains visible/qualified rather than guessed;
15. ordinary repository input errors produce actionable diagnostics, not panics.

---

## 28. Phase boundaries

### Phase 0 may introduce

CLI, filesystem scan, extension-based language classification, role heuristics, physical line counts, basic Git metadata, human/JSON output, typed errors, and tests.

### Phase 0 must not introduce

AST parsing, symbols, code relations, graph structures, persistent indexes, caching, threads, async, HTTP, MCP, embeddings, vector search, LLMs, or Context Integrity policy.

### Phase 1 may introduce

Tree-sitter, `Symbol`, `SourceRange`, parser/analyzer boundaries, and parser fixtures.

### Phase 2 may introduce

entity IDs, `Relation`, and graph traversal.

This sequencing is deliberate.

---

## 29. Architecture review checklist

Before accepting a change, ask:

- Is this observable code intelligence, or normative Context Integrity policy?
- Is it needed by the current phase?
- Could a plain function/struct solve it before a trait/framework/service?
- Can the result be traced to evidence?
- Are uncertainty and unsupported cases honest?
- Is the domain independent from CLI/transport?
- Is a public machine contract versioned?
- Is state ownership singular?
- Will this architecture obviously be undone in the next few planned phases?
- Were real production paths tested, not only synthetic wiring?

---

## 30. Immediate next step

Implement Phase 0 against this architecture. Do not add Phase 1 infrastructure beyond preserving these seams:

```text
scan produces domain data
classification is centralized
output is separate
Git is separate
machine output is versioned
```

Revise this architecture when actual code proves a boundary wrong; do not preserve speculative design merely because it was written first.
