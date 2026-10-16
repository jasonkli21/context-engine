# Context Engine — Phase 6 Implementation Plan

**Phase:** 6 — Persistent Index
**Status:** Planned; implementation requires maintainer authorization if Phase 5 independent acceptance is still pending
**Date:** 2026-10-16
**Repository:** `jasonkli21/context-engine`
**Planning baseline inspected:** `28e2f8a8bf975669be0923204b4b70473c5bdecf` (`fix: address phase 5 review findings`)
**Primary language:** Rust
**Binary:** `ctx`
**Primary new capability:** reuse a validated local persistent repository-analysis artifact instead of reparsing/re-extracting the same unchanged repository for every current-state intelligence command
**Explicitly deferred:** incremental file-by-file updates/watchers, async/concurrency architecture, daemon/API/MCP, history/evolution indexing, cross-repository storage, embeddings/model calls, compiler/LSP/SCIP resolution, graph database, source-content cache, remote/team cache

---

## 1. Planning Baseline and Gate

This plan is based on the actual `main` state inspected on 2026-10-16.

Latest inspected commit:

```text
28e2f8a8bf975669be0923204b4b70473c5bdecf
fix: address phase 5 review findings
```

Its parent is the initial Phase 5 implementation:

```text
c99c255c3e90e39cd350859228239e8ef7f6f8bb
feat: implement phase 5 task context ranking
```

The Phase 5 review-fix pass is therefore the Phase 6 implementation baseline, not the earlier implementation commit.

At this baseline the repository already provides:

- bounded repository discovery and safe source loading;
- Rust/Python symbol extraction;
- parser-independent relation candidates;
- a canonical in-memory `RepositoryIndex` and `RepositoryGraph`;
- deterministic graph construction from analysis results;
- path/symbol search, graph traversal, neighborhoods, hotspots, structural change analysis, impact analysis, and task-context ranking;
- versioned machine-readable output;
- extensive aggregate safety bounds;
- a clear split between extraction evidence and relation target resolution;
- current-state documentation that explicitly says graphs/indexes are rebuilt per command and no persistence exists.

Phase 5 is locally implemented and its first independent-review findings were fixed, but `docs/current-state.md` still says a fresh independent Phase 5 re-review is required. Earlier phase dispositions are also not all final.

Therefore:

> **Creating this plan does not accept Phase 5 or any earlier pending phase.**

Implementation may begin when either:

1. the required Phase 5 review/disposition has completed; or
2. the maintainer explicitly directs an implementation session to execute this Phase 6 plan/prompt despite the pending disposition.

Such direction is authorization to proceed, not independent acceptance of Phase 5.

If `main` advances beyond the planning baseline, do not reset or recreate this exact snapshot. Inspect the new state and reconcile this plan against the actual code, tests, reports, and durable docs before editing.

---

## 2. Authority and Alignment

### 2.1 PRD intent

The PRD defines Phase 6 as:

```text
Persistent Index
```

with the goal:

> Avoid rebuilding repository intelligence for every query.

The PRD names possible technologies including SQLite and custom serialized artifacts and requires:

- schema/index versioning;
- stale-index detection;
- a rebuild path;
- persistence/serialization ownership boundaries;
- migration/version design.

The PRD also establishes long-lived constraints that remain binding:

- local-first operation;
- deterministic evidence before probabilistic reasoning;
- inspectable provenance;
- stable domain models independent of a specific tool/LLM/transport;
- persistence must not force a graph database;
- incremental indexing is a later phase, not something to smuggle into Phase 6;
- Context Integrity is a downstream consumer of evidence, not the owner of this store.

### 2.2 Architecture intent

`docs/architecture.md` deliberately deferred persistence until real query patterns existed. Those patterns now exist across:

- index/symbol lookup;
- graph queries;
- search/neighborhood/hotspots;
- task-context ranking.

The architecture specifically suggests:

- SQLite for durable metadata/entities/relations;
- compact serialized artifacts where useful;
- Tantivy only if later full-text requirements justify it.

It also says storage should not reshape early domain types around a database schema.

That means Phase 6 should **cache the established analysis substrate** and keep the current query/ranking layer operating over the in-memory domain model.

### 2.3 Newer repository guidance

The implementation must also respect newer guidance in:

- `AGENTS.md`;
- `docs/current-state.md`;
- `docs/development/llm-workflow.md`;
- the Phase 5 plan/report/review-fix report;
- actual source/tests at the implementation baseline.

Important practical rules include:

- extend existing production seams rather than create parallel indexing/query paths;
- preserve explicit aggregate bounds;
- distinguish unsupported/incomplete evidence from success;
- keep output rendering independent from repository/storage traversal;
- keep plans, delivery evidence, review evidence, and living status separate;
- do not create another crate/service/framework merely because persistence is being added;
- reviews must reconstruct the real architecture rather than check only plan bullets.

---

## 3. Phase 6 Objective

Phase 5 can answer:

> Which current repository files are most relevant to this task under a fixed content budget?

But every current-state intelligence command still performs a full scan and then reparses/re-extracts repository structure.

Phase 6 should answer a new operational question:

> Can the engine reuse previously computed structural analysis when the inputs that determine that analysis are unchanged, while detecting staleness conservatively and rebuilding safely when they are not?

The Phase 6 product should behave conceptually as:

```text
current-state intelligence request
              │
              ▼
       discover current repo
              │
              ▼
     compute freshness fingerprint
              │
       ┌──────┴─────────┐
       │                │
       ▼                ▼
 persistent hit    missing/stale/
       │           incompatible/corrupt
       │                │
       ▼                ▼
load analysis       rebuild analysis
artifact             + validate
       │                │
       └──────┬─────────┘
              ▼
  canonical in-memory model
      ┌───────┴──────────┐
      ▼                  ▼
RepositoryIndex    RepositoryGraph
      │                  │
      └────────┬─────────┘
               ▼
 existing queries/ranking/rendering
```

The persisted representation is a cache of deterministic derived evidence. It is not a new source of truth.

---

## 4. Core Design Decision: Persist the Analysis Artifact, Not the Query Graph

The most important Phase 6 boundary is what is actually persisted.

### 4.1 Persist

Persist the reusable output of repository analysis:

```text
RepositoryIndex
+
per-file parser-extracted FileRelations / relation candidates
+
store/freshness metadata
```

This includes facts such as:

- analyzed file identities;
- language/role/status;
- symbols and ranges;
- analysis diagnostics;
- relation-extraction status;
- unresolved/import/reference candidate facts and their ranges/metadata/derivation.

### 4.2 Do not persist as authoritative state

Do **not** make the current `RepositoryGraph` representation the persistence contract.

In particular, do not make persisted correctness depend on:

- `EntityId` numeric values;
- `RelationId` numeric values;
- in-memory forward/reverse adjacency vectors;
- today’s exact relation-resolution implementation;
- query-result IDs;
- ranking output;
- selected context content.

Those are revision/index-local derived structures.

### 4.3 Reconstruction

A warm graph-backed command should:

1. load persisted index + parser-extracted relation candidates;
2. reconstruct the canonical `RepositoryGraph` through the existing graph-construction seam (`from_analysis` or a carefully refactored equivalent);
3. run the same current query/ranking code as a cold build.

Benefits:

- no second graph implementation;
- no database-specific query layer in Phase 6;
- existing query/ranker semantics remain shared;
- graph-resolution improvements can apply to cached extraction without persisting obsolete numeric IDs;
- Phase 7 can invalidate/recompute analysis per file without requiring a complete storage redesign;
- Context Integrity remains decoupled from the store.

If implementation evidence shows a narrower or cleaner artifact boundary, it may deviate, but the implementation report must explain why and preserve these architectural properties.

---

## 5. User Journey and Command Semantics

Phase 6 should avoid adding a large new CLI surface.

### 5.1 Explicit rebuild

`ctx index` becomes the explicit rebuild/persist path:

```bash
ctx index .
ctx index /path/to/repo --max-parse-file-size 2097152
```

Expected semantics:

- perform a fresh graph-capable repository analysis sufficient to populate the shared persistent artifact;
- validate that the repository did not change across the build window;
- atomically replace/update the cached artifact only after validation;
- render the existing index-facing result only after persistence succeeds.

Do not require users to run a separate migration or database command.

### 5.2 Automatic reuse/rebuild

Commands that consume the current repository index/graph should use a shared `load_or_build` path:

```text
index
symbols
symbol
graph
deps
callers
related
search
neighborhood
hotspots
context
```

For these commands:

- fresh compatible cache → load it;
- missing cache → build and persist;
- stale cache → rebuild and persist;
- known incompatible schema/compatibility version → rebuild;
- corrupt/untrusted cache → ignore/rebuild safely, never panic or trust partial state.

The semantic command result must not depend on whether it was a cache hit or cold rebuild.

### 5.3 Commands intentionally not cached in Phase 6

Keep these live/current behavior paths:

```text
scan
stats
tree
```

They are inexpensive filesystem-oriented views and have separate line-count/tree semantics.

Keep committed revision analysis independent:

```text
diff
impact
```

Phase 4 reads immutable Git objects and may analyze arbitrary base/head revisions. Phase 6 is the current-worktree/current-scope persistent index; historical/revision artifact caching is not required.

`context --change BASE..HEAD` may reuse the persisted current graph, but its Phase 4 committed-change computation and clean/head compatibility gate remain separate and unchanged in meaning.

---

## 6. Storage Technology and Location

### 6.1 SQLite

Use SQLite through `rusqlite` unless implementation discovers a concrete blocker.

Rationale:

- local and embedded;
- transactions provide a reliable snapshot-update boundary;
- schema/version handling is explicit;
- normalized rows can preserve native-path bytes and bounded field validation;
- later Phase 7 can update file-scoped records without inventing a second persistence system;
- no service/process is required.

Prefer a bundled SQLite feature if it materially improves reproducibility/portability and the repository’s build environment supports it. Record the actual dependency choice in the implementation report.

Do not introduce a graph database.

### 6.2 Cache location

Do **not** write the persistent database into the analyzed repository.

Repository-local storage such as `.ctx/` would:

- make clean Git repositories dirty unless separately ignored;
- pollute repository scans;
- complicate Phase 5 `--change` cleanliness semantics;
- create recursion/self-indexing risks;
- couple source repositories to an implementation cache.

Instead use the platform’s user cache directory, for example conceptually:

```text
<platform-cache>/context-engine/indexes/<repository-key>.sqlite3
```

Use a standard platform-directory helper rather than hard-coded macOS/Linux paths.

Support one explicit environment override for deterministic tests/CI and advanced local use:

```text
CONTEXT_ENGINE_CACHE_DIR=/absolute/path
```

Requirements:

- the override must be validated;
- tests must use an isolated temporary cache root;
- production tests must never write into the developer’s normal cache directory;
- no source repository file is modified merely by indexing.

### 6.3 Repository key

The store locator should be derived from:

- a lossless canonical scan-root identity;
- platform/path-encoding tag;
- the analysis option that changes index eligibility (`max_parse_file_bytes`).

A BLAKE3 digest is an appropriate compact key and can also support freshness hashing.

The database must still store and verify the full canonical root identity. The hash is a locator, not proof of identity.

Do not rely on lossy Unicode rendering for cache identity.

---

## 7. Lossless Native Path Persistence

Current code intentionally preserves native `PathBuf` identities internally and only uses lossy Unicode at presentation boundaries.

Phase 6 must not regress that behavior.

SQLite path fields should therefore be stored as BLOBs with an explicit platform encoding contract, for example:

```text
unix_bytes_v1
windows_wide_le_v1
```

On Unix:

- encode `OsStr` with raw bytes;
- decode with `OsStringExt`.

On Windows:

- encode the native wide sequence deterministically;
- decode through the Windows `OsString` extension API.

The cache metadata must record the encoding/platform. A cache written under an incompatible platform/path codec is unusable and must rebuild.

Every decoded relative path must be validated before use:

- non-empty where required;
- relative, not absolute;
- no `.` / `..` traversal components;
- consistent with the current discovered inventory;
- aggregate path-byte limits enforced before large allocations.

Do not silently convert persisted paths through UTF-8.

---

## 8. Persistent Schema v1

Exact SQL names may vary, but keep the conceptual schema explicit and normalized.

### 8.1 Metadata

Store at least:

```text
schema_version
index_compat_version
fingerprint_version
path_encoding
canonical_root_native_bytes
max_parse_file_bytes
freshness_fingerprint
artifact counts / sanity metadata
```

Do not require a wall-clock timestamp for correctness.

### 8.2 File analysis

One row per discovered/indexed file, with stable persisted ordinal only for reconstruction:

```text
file_ordinal
relative_path_blob
language_tag
language_other_value
role_tag
analysis_status
symbol_count
syntax_error_count
```

The ordinal is a serialization/reconstruction detail, not a public stable ID.

### 8.3 Symbols

Persist:

```text
symbol_ordinal
file_ordinal
kind
name
qualified_name
range start/end byte,row,column
name_range start/end byte,row,column
```

### 8.4 Diagnostics

Persist analysis diagnostics with bounded text and optional source range.

### 8.5 Relation extraction

Persist parser-independent extraction results rather than final graph IDs:

```text
file_ordinal
relation status
unsupported candidate count
candidate ordinal
relation kind
target text
source range
metadata kind / import binding / wildcard / reference context
derivation
```

### 8.6 Stable enum codecs

Do not persist Rust enum discriminants or rely on derived binary layout.

Use explicit stable string/integer tags with decode validation for:

- `Language`;
- `FileRole`;
- `AnalysisStatus`;
- `DiagnosticKind`;
- `SymbolKind`;
- `RelationKind`;
- `RelationStatus`;
- `Metadata` variants;
- `ReferenceContext`;
- `Derivation`.

Unknown values make the cache incompatible/corrupt; they must never be guessed into a default variant.

### 8.7 Do not persist live operational metadata as truth

Git metadata and scan warnings can change independently of parser-derived evidence.

On a cache hit, use the **current validation scan’s**:

- canonical root;
- Git metadata requested by the caller;
- scan warnings.

Reconstruct the loaded `RepositoryIndex` with that current operational metadata plus the persisted structural analysis.

This avoids returning stale Git cleanliness/head/warning information merely because symbol facts are reusable.

---

## 9. Versioning and Compatibility

Phase 6 needs two distinct compatibility concepts.

### 9.1 SQLite schema version

Use SQLite `PRAGMA user_version` (or an equally explicit mechanism).

Initial state:

```text
STORE_SCHEMA_VERSION = 1
```

A v0/uninitialized DB may be initialized to v1.

Because this database is a derivable cache, Phase 6 does **not** need a complex migration framework for preserving old cache data. For an unsupported older/newer schema:

- treat it as incompatible;
- rebuild safely into the current schema;
- never attempt heuristic row interpretation.

Leave one clean migration/rebuild dispatch seam for future versions.

### 9.2 Analysis compatibility version

The SQL schema can remain unchanged while parser/classification/extraction semantics change.

Add an explicit compatibility constant, conceptually:

```text
INDEX_COMPAT_VERSION = 1
```

Bump it when a code change can make previously stored analysis semantically incompatible, including material changes to:

- scanner/classification semantics that affect the indexed inventory;
- language analyzer symbol extraction;
- relation-candidate extraction;
- persisted enum semantics;
- freshness rules.

Do not rely solely on package version `0.1.0` or a Git commit hash.

The code should make this compatibility boundary difficult to overlook in future phases, but avoid creating an elaborate plugin-version framework.

---

## 10. Freshness / Staleness Model

A cache hit is allowed only when the current repository inputs that determine persisted analysis match the cached inputs.

### 10.1 Fingerprint v1

Define a deterministic `freshness-v1` digest over a canonical sorted inventory.

Include for every discovered regular file at least:

- lossless relative path bytes;
- language classification;
- file role;
- size or an equivalent eligibility marker.

For files whose **contents affect parser-derived analysis** under the current options, also hash the bounded raw bytes.

That means content hashing is required for supported files that are:

- not generated/vendor-skipped;
- within `max_parse_file_bytes`;
- otherwise eligible for analysis.

Hash raw bytes, not decoded UTF-8, so invalid-UTF-8 analysis state can still be fingerprinted.

Files that are unsupported, role-skipped, or above the parse limit need not have full contents hashed because their contents do not affect persisted symbols/relations. Their inventory/path/classification/eligibility facts still participate.

A same-length edit to a parse-eligible Rust/Python file **must** invalidate the cache.

### 10.2 Bounded safe fingerprint reads

Fingerprinting must preserve the existing trust boundary:

- no symlink following;
- regular files only;
- bounded per-file reads;
- bounded aggregate fingerprint I/O;
- repository contents never executed;
- no unbounded buffering of file contents.

A small safe raw-byte hashing helper may live next to `source::load` if that keeps filesystem safety rules singular.

If a required fingerprint cannot be established safely because a relevant file is racing/unreadable/non-regular, the persisted artifact is **not proven fresh**. Do not use it.

### 10.3 What does not need to invalidate by content

Do not force a reparse merely because a same-path unsupported/documentation file changes bytes if that content is not part of the persisted structural analysis. `context` still loads selected file content live.

Conservative extra invalidation is acceptable if implementation simplicity requires it, but document the performance tradeoff.

### 10.4 Parse-limit compatibility

The cache key/metadata must account for `max_parse_file_bytes`.

A cache built with a different parse limit is not interchangeable unless the implementation can prove equivalence. The simplest correct implementation is to maintain distinct keyed variants or rebuild.

### 10.5 Compatibility-code changes

Even an identical repository fingerprint is not a hit when `INDEX_COMPAT_VERSION` differs.

---

## 11. Build Consistency and Concurrent Repository Mutation

Persisting a derived artifact creates a new risk: accidentally making a transient mixed filesystem snapshot durable.

A cold/rebuild path should therefore use a bounded pre/post validation protocol.

Recommended flow:

```text
scan A
  ↓
fingerprint A
  ↓
analyze scan A
  ↓
scan B
  ↓
fingerprint B
  ↓
A == B ?
  ├── yes → persist artifact
  └── no  → discard artifact / bounded retry
```

Allow at most one automatic retry in Phase 6.

If the repository changes again during the retry:

- do not write a cache claimed to be fresh;
- return a clear typed `repository changed during indexing` error for explicit `ctx index`;
- for an automatic query rebuild, prefer correctness over persistence and do not loop indefinitely. If the implementation returns the best-effort live analysis under existing non-atomic semantics, it must not persist it and must surface the cache-write omission without corrupting JSON stdout.

The exact fallback behavior can be refined during implementation, but **never persist an artifact whose pre/post fingerprint is known to differ**.

Phase 6 still does not claim an atomic filesystem snapshot for query execution after validation.

---

## 12. Load / Rebuild / Recovery Lifecycle

Introduce one shared application-level persistence service rather than duplicating cache logic in each command.

Conceptually:

```rust
struct AnalysisArtifact {
    index: RepositoryIndex,
    relation_files: Vec<FileRelations>,
}

enum CacheOutcome {
    Hit,
    RebuiltMissing,
    RebuiltStale,
    RebuiltIncompatible,
    RebuiltCorrupt,
}
```

Exact names are flexible.

### 12.1 Warm load

1. canonicalize/scan requested root using existing scanner semantics;
2. compute bounded `freshness-v1`;
3. locate database from lossless root/options key;
4. validate schema/compat/root/options/fingerprint metadata;
5. load rows under strict aggregate limits;
6. validate path identities, counts, enum values, ranges, foreign-key/ordinal consistency;
7. reconstruct `RepositoryIndex` with current scan warnings/Git metadata;
8. reconstruct `RepositoryGraph` only for graph-backed consumers.

### 12.2 Missing/stale/incompatible/corrupt

1. build a fresh graph-capable analysis artifact;
2. perform post-build fingerprint revalidation;
3. write the artifact transactionally;
4. use the same in-memory artifact to satisfy the current command.

Do not build once for cache storage and independently build again for the command.

### 12.3 Explicit `ctx index`

`ctx index` should always force a new validated build and replace the current keyed artifact. This is the user-visible rebuild/repair path.

A persistence failure for explicit `ctx index` is a command error; do not print a success result if the promised persistent index was not updated.

### 12.4 Automatic query write failure

The query result itself should remain usable when analysis succeeded but an optional cache refresh cannot be committed, unless implementation evidence shows that this creates an unsafe ambiguity.

If using graceful degradation:

- return the fresh analysis result;
- emit a sanitized warning to **stderr**, not JSON stdout;
- do not claim persistence succeeded;
- do not mutate `RepositoryIndex.scan_warnings` merely to carry a storage implementation warning.

Keep this behavior centralized in the CLI/application composition layer.

---

## 13. Transaction and Multi-Process Semantics

SQLite should protect the cache from partial replacement.

Requirements:

- enable foreign-key checking;
- write the entire new compatible artifact in one transaction;
- old committed state remains readable until the new transaction commits;
- failure/rollback leaves the prior committed artifact intact;
- readers never observe a half-deleted/half-inserted artifact;
- use a bounded busy timeout; do not block indefinitely on another process;
- close/rollback cleanly on errors.

A simple delete-and-reinsert **inside one SQLite transaction** is sufficient for schema-compatible replacement because SQLite readers see a consistent committed snapshot.

For an incompatible/corrupt database that cannot be opened/migrated safely, rebuilding the derivative cache is acceptable. Do not add a complex recovery journal outside SQLite in this phase.

Two processes may redundantly analyze the same repository. Preventing duplicate compute with a daemon/file-lock protocol is not required in Phase 6; correctness is required.

---

## 14. Resource, Security, and Privacy Requirements

Persistence must preserve the repository’s existing fail-closed resource posture.

### 14.1 Bounded database load

Before/while materializing rows, enforce existing or tighter limits for:

- files/entities;
- symbols;
- relations/candidates;
- diagnostics;
- name/detail/relation bytes;
- path bytes;
- source ranges / integer conversions;
- database file size.

A corrupt database that claims billions of rows/huge strings must not cause unbounded allocation.

A conservative database file-size ceiling (for example 256 MiB, adjusted only with evidence) is appropriate for Phase 6.

### 14.2 Treat the cache as untrusted derived input

Even though it is local, loaded data must be validated:

- no `unwrap` on database values;
- no unchecked integer casts;
- no absolute/traversal paths;
- no invalid enum fallback;
- no out-of-range symbol/relation ownership;
- no duplicated ordinals silently accepted;
- no invalid half-open source ranges accepted as trustworthy facts.

Corruption → rebuild, not panic.

### 14.3 No source-content persistence

Do **not** persist complete source file bodies in Phase 6.

The cache may contain structural metadata derived from source, including:

- symbol names/signatures represented by current models;
- relation targets/import bindings;
- diagnostics;
- paths/ranges.

Task-context selected content remains loaded live through the safe source loader.

### 14.4 Local only

No network, cloud database, telemetry, model API, or external service.

---

## 15. Current Command Composition After Phase 6

The intended architecture should look approximately like:

```text
CLI
 │
 ├── scan/stats/tree ───────────────→ live scanner
 │
 ├── diff/impact ───────────────────→ committed Git revision analysis
 │
 └── current-state intelligence
         │
         ▼
   persistence service
     │          │
  fresh hit   rebuild
     │          │
     └────┬─────┘
          ▼
  AnalysisArtifact
   │            │
   ▼            ▼
RepositoryIndex RepositoryGraph
   │            │
   └──────┬─────┘
          ▼
 existing query/rank/output modules
```

There must still be one canonical:

- scanner;
- language-analysis pipeline;
- relation-extraction pipeline;
- graph constructor;
- query/ranker implementation.

The cache is not a second code-intelligence engine.

---

## 16. Code-Level Change Map

Expected production changes are approximately:

```text
Cargo.toml / Cargo.lock
  rusqlite
  platform cache-directory helper
  blake3 (or equally justified bounded digest dependency)

src/
├── main.rs
│   route current-state analysis through shared persistence service
│   make `ctx index` an explicit force rebuild/persist path
│
├── cli.rs
│   little/no new surface; help text must stop claiming fresh in-memory indexing
│
├── index.rs
│   expose/refactor one graph-capable analyze-from-existing-scan seam
│   do not duplicate parser analysis
│
├── graph.rs
│   reuse `from_analysis`; small visibility/validation refactor only if needed
│
├── source.rs
│   optionally add safe bounded raw-byte hashing primitive beside existing loader
│
├── persistence.rs        (new; name flexible)
│   cache location/key
│   freshness-v1
│   load-or-build/rebuild orchestration
│   compatibility and typed errors
│
└── store.rs              (new; name flexible)
    SQLite schema v1
    stable enum/path codecs
    bounded artifact load/write
    transactional replacement
```

Keeping persistence orchestration and raw SQLite row encoding separate is reasonable if it keeps each module understandable. Do not create a crate hierarchy.

Tests may add:

```text
tests/persistence.rs
```

and focused unit tests adjacent to persistence/store/source modules.

Documentation after implementation:

```text
README.md
docs/current-state.md
docs/architecture.md
docs/reports/phase-6-implementation-report.md
```

Update other docs only where actual shipped semantics make them stale.

---

## 17. Implementation Stages

### Stage A — Reconfirm baseline and extraction seam

Before editing:

1. inspect actual `HEAD` and worktree;
2. read `AGENTS.md` and `docs/current-state.md`;
3. read this Phase 6 plan;
4. inspect the Phase 5 implementation/review-fix reports;
5. inspect `main.rs`, `cli.rs`, `index.rs`, `graph.rs`, `model.rs`, `relations.rs`, `source.rs`, relevant outputs/tests;
6. inspect architecture/PRD persistence and downstream Phase 7 sections;
7. run the canonical verifier once if environment permits.

Establish exactly what artifact can be reused without duplicating graph/query logic.

### Stage B — Persistence codecs and SQLite schema

Implement:

- stable enum codecs;
- lossless native-path codec;
- SQLite v1 schema;
- bounded row loading;
- artifact round-trip;
- compatibility/root/options validation;
- transactional replacement.

Use small direct unit tests.

Do not wire every CLI command yet.

### Stage C — Freshness-v1

Implement:

- platform cache path/key;
- current inventory fingerprint;
- safe raw-byte hashing for parse-relevant files;
- explicit aggregate fingerprint limits;
- parse-limit/index-compat metadata;
- stale/missing/incompatible/corrupt classifications.

Test same-length source edits and add/remove/path changes explicitly.

### Stage D — Shared load/build application path

Refactor analysis so one discovered scan can feed:

- fingerprinting;
- canonical index/relation extraction;
- cache persistence.

Implement pre/post fingerprint validation and bounded retry.

Avoid analyzing the same cold repository twice merely because one copy is needed for the command and another for the cache.

### Stage E — Wire explicit `index`

Make `ctx index` force rebuild/persist.

Verify:

- no cache file appears inside the source repository;
- successful command means cache commit succeeded;
- failed cache commit does not print a success result;
- existing human/JSON semantic index data remains correct.

### Stage F — Wire current-state consumers

Migrate one shared path at a time:

1. `symbols` / `symbol` / `search`;
2. `graph` / `deps` / `callers` / `related`;
3. `neighborhood` / `hotspots`;
4. `context`.

After each coherent workstream, run focused tests only.

Do not change `scan/stats/tree` or Phase 4 `diff/impact` merely to route everything through one abstraction.

### Stage G — Recovery/adversarial behavior

Exercise:

- corrupt DB;
- unknown schema/compat version;
- wrong root metadata;
- invalid enum/path rows;
- truncated/oversized DB;
- repository mutation during indexing;
- cache directory write failure;
- non-Git repository;
- dirty Git repository;
- clean `context --change` path with cache stored outside the repository.

### Stage H — Verification, docs, report

Run one complete quality pass near the end, then update shipped documentation and write the implementation report.

---

## 18. Focused Testing Strategy

Testing should prove persistence correctness and real command wiring without multiplying every existing command test by every cache state.

### 18.1 Store round-trip

A compact fixture must round-trip:

- supported/unsupported languages;
- `Language::Other`;
- all file roles/statuses relevant to current code;
- symbols/ranges/name ranges;
- diagnostics;
- relation statuses;
- import metadata with/without bindings/wildcards;
- reference metadata;
- derivations;
- non-UTF-8 path bytes on supported test platforms.

Loaded artifact should reconstruct byte-for-byte/field-for-field equivalent domain facts after canonical sorting.

### 18.2 Freshness

Prove at least:

- unchanged repo → hit;
- same-length parse-eligible Rust edit → stale;
- same-length parse-eligible Python edit → stale;
- added file → stale;
- removed file → stale;
- renamed file → stale;
- role/language/path classification change → stale;
- parse-limit mismatch → incompatible/distinct variant;
- compatibility-version mismatch → rebuild;
- invalid UTF-8 parse-eligible file can be fingerprinted without pretending it is analyzable;
- generated/vendor/too-large content behavior matches documented fingerprint scope.

### 18.3 Warm path avoids parser work

Do not rely only on timing.

Use a test seam/counter/injected builder to prove a valid cache hit does **not** invoke Tree-sitter analysis or relation extraction again.

This is a central Phase 6 acceptance property.

### 18.4 Graph/query equivalence

From the same fixture, compare cold and warm paths for representative operations:

- symbol lookup;
- graph relation/traversal;
- search;
- neighborhood/hotspots;
- context ranking.

The semantic output should be identical for unchanged state.

Do not create a shadow query implementation inside persistence tests.

### 18.5 Corruption and bounds

Use deliberately malformed small databases rather than giant fixtures.

Test:

- unknown schema;
- unknown enum;
- bad ordinal/foreign key;
- invalid native path;
- integer overflow/range fields;
- aggregate row/text limit;
- DB-size limit via injected small threshold;
- rollback on write failure where practical.

Expected result is typed recovery/rebuild/error, never panic or partial trusted graph.

### 18.6 Multi-process/transaction semantics

One focused concurrency/transaction test is enough if deterministic:

- a reader must not observe half-replaced rows;
- failed transaction preserves prior committed artifact.

Do not turn Phase 6 into a distributed-locking project.

### 18.7 CLI integration

Use an isolated `CONTEXT_ENGINE_CACHE_DIR`.

At minimum prove:

1. `ctx index` creates/updates external cache;
2. repository `git status` remains unchanged by cache creation;
3. a subsequent current-state command can consume the cached artifact;
4. editing a source invalidates/rebuilds and updated symbols appear;
5. corrupt cache is rebuilt without stdout corruption;
6. human and JSON results remain deterministic;
7. `context --change` still requires/observes the existing clean/head gate and the external cache does not make the worktree dirty;
8. non-Git repository works.

---

## 19. Verification and Smoke Plan

During implementation, use focused tests for the workstream being edited.

Near completion run the full gate once:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
git diff --check
```

Release smoke tests should use a temporary cache root and at least:

```bash
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx index .
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx symbols . --format json
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx graph . --format json
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx search graph . --format json
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx context . --task "persistent index" --budget 24000 --format json
```

Then repeat representative warm commands and confirm semantic/byte determinism where expected.

Create a tiny temp repository and:

- index it;
- make a same-length source edit;
- verify stale detection and updated output;
- add/remove a file;
- verify another rebuild;
- corrupt the SQLite file;
- verify safe recovery;
- if Git-backed, verify the cache itself never dirties the repository.

Optional timing observations may be recorded, but no flaky wall-clock speedup threshold is required. The deterministic acceptance fact is that a warm hit bypasses parsing/extraction.

Record exact commands/results and anything that could not be executed.

---

## 20. Documentation and Reporting

Implementation completion must produce:

```text
docs/reports/phase-6-implementation-report.md
```

The report must include:

- actual starting/ending commits or worktree state;
- whether Phase 5 acceptance was still pending and what authorization allowed Phase 6 to proceed;
- final storage technology/dependencies;
- exact cache location/key semantics;
- schema/index/fingerprint versions;
- persisted artifact boundary;
- staleness algorithm;
- path encoding;
- load/rebuild/recovery behavior;
- resource limits;
- command wiring;
- test/verification results;
- observed limitations/deviations;
- explicit remaining Phase 7+ work.

Update `README.md` after the capability actually ships. Remove statements such as:

```text
Nothing is saved to .ctx/ or any cache.
Current-state commands perform a fresh scan/index on every invocation.
Persistence remains future work.
```

Replace them with accurate behavior without implying incremental indexing.

Update `docs/current-state.md` to describe the implementation truth and review status.

Update the persistence section of `docs/architecture.md` because the storage choice, artifact boundary, freshness contract, and Phase 7 seam are durable architecture once shipped.

Do not rewrite the PRD merely to say Phase 6 happened.

Do not create `handoff.md` or another living status document.

---

## 21. Phase 6 Acceptance Criteria

Phase 6 is implementation-complete when all of the following are true.

### Persistence correctness

- A repository analysis artifact survives process exit and is reusable by a later process.
- Warm reuse bypasses language parsing/relation extraction.
- `RepositoryGraph` is reconstructed through the canonical graph path rather than loaded as a competing graph representation.
- No index-local graph/query ID is falsely promoted to a cross-generation stable identity.

### Freshness

- Unchanged relevant inputs hit the cache.
- Same-length edits to parse-relevant Rust/Python source invalidate it.
- add/remove/rename/classification changes invalidate it.
- parse-option/compatibility changes do not reuse incompatible analysis.
- unknown freshness never becomes a false hit.
- a known repository mutation during build is never persisted as a valid fresh cache.

### Safety/recovery

- corrupt/incompatible cache cannot panic or create trusted bogus evidence;
- persisted rows are loaded under aggregate bounds;
- native non-UTF-8 paths remain lossless internally;
- no source code is executed;
- no source file bodies are stored;
- no cache file is written into the analyzed repository by default.

### Integration

- `ctx index` is a working explicit rebuild path;
- current-state intelligence commands use the shared persistent artifact;
- `scan/stats/tree` keep their live semantics;
- `diff/impact` keep their Phase 4 committed-revision semantics;
- `context --change` still obeys the Phase 5 snapshot compatibility gate;
- cold/warm semantic outputs match for unchanged repositories.

### Quality

- canonical verifier passes;
- all-target check passes;
- release build passes;
- diff whitespace check passes;
- release-binary persistence/staleness/corruption smokes pass;
- implementation report and user/current architecture docs are truthful.

Independent review is still required after local implementation verification.

---

## 22. Explicit Non-Goals / Phase Stop Boundary

Do **not** add in Phase 6:

- filesystem watchers;
- per-file incremental mutation of an existing cache in response to events;
- background indexing workers;
- threads/channels solely for indexing;
- async/Tokio;
- daemon/service process;
- HTTP/IPC;
- MCP;
- cross-repository workspace database;
- historical revision cache;
- Git blame/history/co-change model;
- Tantivy/full-text index;
- embeddings/vector search/model calls;
- token budgeting;
- compiler/LSP/SCIP analysis;
- cross-file resolution solely to make cached graphs more impressive;
- Context Integrity policy/authority/drift semantics;
- a multi-crate workspace;
- remote/team synchronization;
- cache garbage collection policy beyond what is necessary for safe current operation.

Phase 7 is specifically where changed-file invalidation, queues, watchers, and synchronization belong.

---

## 23. Downstream Compatibility Requirements

### 23.1 Phase 7 — Incremental Indexing

Phase 6 should leave a clean seam for:

```text
changed file
  ↓
recompute file analysis + relation candidates
  ↓
replace file-scoped persisted records
  ↓
reconstruct/update graph
```

Do not require Phase 7 to abandon SQLite or throw away the analysis-artifact boundary just to update one file.

However, Phase 6 does not need to implement that incremental mutation now.

### 23.2 Phase 8 — History/Evolution

Current-state cache identity must not be conflated with historical symbol lineage. Persisted ordinals remain local serialization details.

### 23.3 Phase 9 — Local Service

A future daemon may keep the same artifact/domain model warm in memory. Phase 6 must not force CLI-specific storage APIs into the domain.

### 23.4 Phase 11 — Context Integrity evidence adapter

Context Integrity should consume stable evidence/query contracts, **not this SQLite schema directly**.

The database remains an internal implementation detail of Context Engine.

---

## 24. Key Risks the Implementation and Review Must Challenge

This plan makes concrete design choices, but they are not exempt from review.

The implementation and independent reviewer should challenge at least:

1. **Fingerprint correctness:** can any content/path/option change produce a false fresh hit?
2. **Fingerprint cost:** does proving freshness erase most of the benefit of persistence?
3. **Artifact boundary:** is persisting pre-resolution analysis truly the cleanest reuse unit?
4. **Path fidelity:** are non-UTF-8/native paths round-tripped losslessly across supported platforms?
5. **Database corruption:** can malformed rows bypass current graph/index resource invariants?
6. **Transactional replacement:** can a crash/concurrent process leave a partially trusted artifact?
7. **Command coverage:** are any current-state commands still silently rebuilding outside the shared path?
8. **Cold-build duplication:** does cache integration accidentally scan/parse the repository twice unnecessarily beyond the deliberate pre/post validation work?
9. **Phase 7 seam:** will per-file invalidation require a database/schema rewrite?
10. **Public behavior:** did persistence accidentally change query semantics, output IDs, or Phase 5 snapshot guarantees?
11. **Cache privacy:** are more source-derived details persisted than necessary?
12. **Documentation truth:** do README/current-state/architecture/source help still claim fresh-only/no-persistence behavior after shipping?

A conforming implementation can still be rejected if the review finds a simpler/safer architecture or a plan assumption that is wrong in the real code.

---

## 25. Summary

Phase 6 should add one thing to the product architecture:

> **a validated, versioned local cache of the repository-analysis artifact that existing query/ranking code can reuse safely across processes.**

The intended progression remains:

```text
Phase 0–5
fresh synchronous intelligence
        │
        ▼
Phase 6
persistent validated analysis snapshot
        │
        ▼
Phase 7
incremental updates to that snapshot
        │
        ▼
Phase 8+
history/service/cross-repo/integration
```

The phase succeeds by removing redundant reparsing without weakening evidence correctness, path fidelity, resource bounds, or the clean boundary between observable code intelligence and later policy/agent layers.
