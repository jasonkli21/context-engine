# Context Engine — Phase 7 Implementation Plan

**Phase:** 7 — Incremental Indexing
**Status:** Planned; implementation requires maintainer authorization if Phase 6 fresh independent re-review is still pending
**Date:** 2026-10-16
**Repository:** `jasonkli21/context-engine`
**Planning baseline inspected:** `5e3ae18d1510501e177aac2d9021fc01d8534435` (`fix: address phase 6 review findings`)
**Primary language:** Rust
**Binary:** `ctx`
**Primary new capability:** update the validated persistent current-state analysis artifact by reanalyzing only files whose analysis inputs changed, and keep that artifact warm from filesystem events without turning Context Engine into a daemon/service
**Explicitly deferred:** async/Tokio, HTTP/IPC/MCP, background service installation, history/evolution intelligence, cross-repository indexing, compiler/LSP/SCIP resolution, embeddings/model calls, full-text search, parallel parser workers, persistent graph IDs/adjacency, remote/team synchronization, cache GC/compaction policy, stable symbol lineage

---

## 1. Planning Baseline and Gate

This plan is based on the actual `main` state inspected on 2026-10-16.

Latest inspected commit:

```text
5e3ae18d1510501e177aac2d9021fc01d8534435
fix: address phase 6 review findings
```

Its parent is the initial Phase 6 implementation:

```text
ef91ac426e45bfd566d339653875c001e9b1b69f
feat: implement phase 6 persistent analysis index
```

The Phase 6 review-fix commit is the Phase 7 planning baseline. It materially changed the persistence seam specifically to make file-local updates possible:

- SQLite schema v2 uses path-derived internal file keys;
- symbol/diagnostic/candidate ordinals restart within each file;
- stored structural rows are covered by a versioned artifact-integrity digest;
- freshness failures on automatic commands fail closed for cache reuse and return uncached best-effort analysis;
- analyzer compatibility and store-schema compatibility are distinct contracts.

`docs/current-state.md` still says Phase 6 requires fresh independent re-review, and several earlier phase dispositions remain pending. Therefore:

> **Creating or implementing this Phase 7 plan does not accept Phase 6 or any earlier pending phase.**

Implementation may begin when either:

1. the required Phase 6 re-review/disposition has completed; or
2. the maintainer explicitly directs an implementation session to proceed from the current baseline despite that pending disposition.

Such direction is authorization to proceed, not independent acceptance.

If `main` advances beyond the planning baseline, do not reset or recreate this snapshot. Inspect the actual `HEAD`, worktree, code, tests, current-state document, reports/reviews, and any new durable decisions. Reconcile this plan to the real repository before editing, and record material deviations in the Phase 7 implementation report.

---

## 2. Authority and Alignment

### 2.1 PRD intent

The PRD defines Phase 7 as **Incremental Indexing** with the goal:

> Update only affected state when files change.

The PRD explicitly names:

- filesystem watch;
- changed-file queue;
- parse invalidation;
- graph edge update;
- dependency invalidation;
- threads;
- channels;
- synchronization;
- shared state / work queues.

It also says concurrency should be introduced only at this phase unless earlier profiling proved a need.

The broader PRD remains binding:

- deterministic evidence first;
- local-first and offline by default;
- incremental updates are a core long-term product requirement;
- persisted evidence must retain inspectable provenance;
- query and storage internals must not become tied to one agent/transport;
- Context Integrity remains a downstream consumer of descriptive evidence, not a policy layer inside this engine.

### 2.2 Architecture intent

The current architecture already gives Phase 7 the correct base:

```text
current repository
      │
      ▼
 scan + safe source reads
      │
      ▼
RepositoryIndex + FileRelations
      │
      ├── persisted in SQLite
      │
      ▼
graph::from_analysis(...)
      │
      ▼
query / ranking / rendering
```

Durable architecture points that Phase 7 must preserve:

- `persistence.rs` owns current-state load/build/freshness composition;
- `store.rs` owns the SQLite schema/codecs and artifact integrity;
- the persisted artifact is pre-resolution analysis, not graph/query state;
- `graph::from_analysis` remains the canonical graph constructor;
- renderers do not scan, watch, parse, or access SQLite;
- source bodies are not persisted;
- Git operational metadata and scan warnings remain live;
- `scan`/`stats`/`tree` retain live filesystem semantics;
- `diff`/`impact` retain committed-revision semantics.

The architecture document explicitly says schema v2 was chosen to give Phase 7 a file-local row-replacement seam.

### 2.3 Newer repository guidance

Implementation must also respect:

- `AGENTS.md`;
- `docs/current-state.md`;
- `docs/development/llm-workflow.md`;
- `docs/reports/phase-6-implementation-report.md`;
- `docs/reports/phase-6-review-fix-report.md`;
- the actual Phase 6 source/tests at implementation time.

Important practical rules:

- code/tests and verified behavior determine what is delivered;
- plans are future intent, not proof;
- extend existing seams instead of creating a second indexer/store/query path;
- keep state ownership singular and explicit;
- preserve aggregate resource limits and fail-closed semantics;
- unsupported or uncertain evidence must stay explicit;
- a review must reconstruct the architecture independently rather than merely map code to plan bullets.

---

## 3. Current Phase 6 Reality

Phase 6 eliminated repeated parsing/extraction for an unchanged repository, but it still validates freshness globally on every current-state request.

Today, roughly:

```text
query
  │
  ▼
scan current inventory
  │
  ▼
hash every parse-eligible Rust/Python body
  │
  ├── global fingerprint matches ──► load whole cached artifact
  │
  └── fingerprint differs ─────────► rebuild whole repository analysis
                                      │
                                      ▼
                                   save whole DB
```

Important current implementation facts:

- `persistence::FINGERPRINT_VERSION` is `freshness-v1`;
- `store::SCHEMA_VERSION` is `2`;
- `INDEX_COMPAT_VERSION` is `1` unless analyzer/classifier/extraction semantics change;
- `store::save` writes a complete sibling SQLite database and atomically replaces the prior one;
- `store::load` requires a current scan/fingerprint match before returning the artifact;
- `index::analyze_scan` analyzes an entire `RepositoryScan` and enforces aggregate limits while accumulating results;
- there is not yet a reusable file-analysis unit/API;
- graph commands always reconstruct from the loaded/built analysis artifact;
- cache rows are already path-local enough for file replacement, but no partial transaction exists;
- there are no watchers, event queues, worker loops, threads/channels owned by Context Engine, or incremental write conflicts to resolve.

Phase 7 should remove **full reanalysis** for localized changes. It does not have to eliminate every byte read needed to prove freshness on a one-shot CLI request.

---

## 4. Phase 7 Objective

After Phase 7, a local edit such as:

```text
src/a.rs changed
```

should behave conceptually as:

```text
validated cached artifact
        +
current repository inputs
        │
        ▼
file-level delta
  unchanged: many
  modified: 1
  added:    0
  removed:  0
        │
        ▼
analyze only src/a.rs
        │
        ▼
replace src/a.rs structural records
        │
        ▼
validate combined artifact + commit atomically
        │
        ▼
reconstruct canonical graph
        │
        ▼
existing query/ranking/output
```

A foreground watcher should be able to keep the same artifact current while files change:

```text
filesystem events (hints)
        │
        ▼
bounded event queue
        │
        ▼
coalesced reconciliation trigger
        │
        ▼
scan/fingerprint actual repository state
        │
        ▼
file-local incremental update
```

The filesystem event stream is an optimization/trigger, **not** the authority for what changed.

---

## 5. Phase 7 Success Definition

Phase 7 succeeds if:

1. modifying one analyzable file does not re-run language parsing/relation extraction for unrelated unchanged files;
2. add/remove/rename/classification/role/eligibility changes produce the same semantic artifact as a clean full rebuild;
3. stale symbols, diagnostics, relation candidates, and graph edges from changed/removed files cannot survive an incremental update;
4. repository-wide aggregate safety limits are enforced over the **final combined artifact**, not just the changed files;
5. repository mutation during reconciliation is never knowingly committed as a fresh generation;
6. concurrent incremental writers cannot silently overwrite each other from stale bases;
7. a foreground watch path uses bounded concurrency and degrades to deterministic reconciliation on ambiguous/lost event conditions;
8. all existing query/ranking outputs retain their semantics regardless of whether the artifact came from full build, warm load, on-demand incremental reconciliation, or watcher reconciliation;
9. explicit `ctx index` remains a reliable full rebuild/repair path;
10. the phase does not prematurely become Phase 9's daemon/API architecture.

---

## 6. Core Design Decision: Increment the Analysis Artifact, Rebuild the Graph

Do **not** make `RepositoryGraph` the persistent incremental state.

The incremental unit should be a file-owned slice of the existing analysis artifact:

```text
FileAnalysisUnit
  - FileAnalysis
  - symbols[]
  - diagnostics[]
  - optional FileRelations / extraction
  - file input fingerprint metadata
```

The exact type/name may differ.

When one file changes:

1. retain unchanged file analysis units;
2. recompute the changed/new file unit through the same language analyzers;
3. remove units for deleted paths;
4. validate the assembled repository-wide artifact under existing limits;
5. persist the changed rows transactionally;
6. reconstruct `RepositoryGraph` from the full updated analysis artifact through `graph::from_analysis`.

This intentionally avoids incremental adjacency mutation in Phase 7.

Why this still satisfies graph/dependency invalidation:

- all resolved edges are derived from the current file/symbol/relation-candidate set;
- removing a symbol/candidate cannot leave a persisted resolved edge because no graph edge is persisted;
- if later cross-file resolution is added, rebuilding from all current candidates naturally re-resolves unchanged callers against changed definitions;
- graph IDs remain generation-local rather than being falsely stabilized by the database.

A reviewer should reject a second graph implementation or persisted adjacency unless real implementation evidence proves it is materially safer/simpler and preserves these invariants.

---

## 7. File-Local Input Fingerprints

### 7.1 Why `freshness-v1` must evolve

The current global fingerprint can answer only:

```text
same repository analysis inputs?
yes / no
```

Phase 7 needs:

```text
which file analysis inputs differ?
```

Introduce a new explicitly versioned freshness contract. `freshness-v2` is the expected name unless implementation finds a better clear versioning scheme.

Do not silently reinterpret `freshness-v1`.

### 7.2 File input stamp

For every discovered file, derive a deterministic file input stamp from the facts that determine its stored analysis state.

At minimum include:

- lossless scan-root-relative native path bytes;
- language classification;
- file role;
- file size / parse-eligibility facts;
- analysis option compatibility relevant to that file;
- raw content bytes for parse-eligible supported Rust/Python files.

Continue hashing raw bytes rather than decoded UTF-8 so invalid UTF-8 remains a deterministic analysis input.

For unsupported, generated/vendor, or over-limit files whose bodies do not influence stored parser-derived evidence, the file input stamp may omit body content exactly as Phase 6 does. Their inventory/classification/eligibility still participates.

A same-length edit to an eligible Rust/Python file must change its stamp.

### 7.3 Repository generation stamp

Derive a repository generation stamp from a canonical sorted sequence of file paths + file input stamps plus the freshness-contract/options identity.

This stamp serves two roles:

- summarize which repository generation the cache represents;
- act as the optimistic concurrency token for file-local store updates.

It must not be used as a substitute for validating per-file records.

### 7.4 Persist only digests, not source bodies

Persist each file's input digest/stamp, not source text.

The artifact-integrity digest must cover the stored input-stamp bytes as well as the structural records so local cache corruption cannot silently change incremental invalidation decisions.

### 7.5 Resource bounds

Preserve the current bounded source-read posture.

A one-shot CLI reconciliation may still need to read all parse-eligible source bytes to prove which files changed. That is acceptable for Phase 7 as long as:

- the read budget remains bounded;
- parsing/extraction is limited to changed/new files;
- the limitation is documented honestly;
- no unverified metadata shortcut creates false cache hits.

Do not use mtime alone as proof of unchanged source. Metadata may be used as an optimization only if correctness still falls back to content validation.

---

## 8. Storage Evolution: Schema v3 / Incremental Snapshot API

Phase 7 changes the persistent record contract and therefore should bump the store schema, expected:

```text
SCHEMA_VERSION = 3
```

Schema v2 remains an incompatible derivable cache and should rebuild rather than require a migration framework.

`INDEX_COMPAT_VERSION` should **not** be bumped merely because Phase 7 exists. Bump it only if scanner/classifier/analyzer/relation/persisted-enum semantics materially change during implementation, following the rule already documented in `AGENTS.md`.

### 8.1 Persist per-file freshness state

Each file record must carry or be associated with its file input stamp.

Possible shape:

```text
files
  id            path-derived internal key
  path          native BLOB
  input_stamp   BLOB
  payload       FileAnalysis JSON
```

Exact SQL may differ, but the input stamp must be a validated, bounded, integrity-covered value.

### 8.2 Load a structurally valid stale snapshot

The current `store::load` is intentionally all-or-nothing on the current global fingerprint. Phase 7 needs a narrower API conceptually like:

```text
load_snapshot(path, root/options/compat)
  -> validated persisted artifact
     + persisted file input stamps
     + persisted repository generation stamp
```

This load must:

- validate schema, analysis compatibility, path codec, root identity, parse limit/options;
- enforce DB/row/aggregate bounds;
- verify artifact integrity;
- validate native paths, ownership, ordinals, ranges, enums, counts, and foreign keys;
- **not** require the persisted inventory to equal the current scan before it can be used as an incremental base.

A corrupt or incompatible snapshot is not an incremental base; rebuild it fully.

### 8.3 Transactional file patch

Add a store operation conceptually like:

```text
apply_patch(
  expected_generation,
  new_generation,
  removed_paths,
  changed_or_added_units
)
```

Within one SQLite transaction:

1. confirm metadata still matches `expected_generation`;
2. delete child records for removed/changed paths;
3. delete removed file records;
4. insert/update changed/new file record + input stamp;
5. insert that file's symbols/diagnostics/extraction/candidates with file-local ordinals;
6. re-run store preflight/resource validation;
7. recompute the artifact-integrity digest across the resulting committed rows;
8. update the repository generation stamp and digest;
9. commit atomically.

If the expected generation no longer matches, return a typed write-conflict result. Do not apply a patch derived from a stale store generation.

### 8.4 Why optimistic compare-and-swap is required

Two CLI processes may reconcile the same repository concurrently.

Without a generation check:

```text
P1 reads S0
P2 reads S0
P1 commits change A -> S1
P2 commits stale patch derived from S0 -> may erase/revive state
```

With an expected-generation check, P2 must reload/reconcile instead of overwriting S1 blindly.

Keep retries bounded. Do not add a long-lived global file-lock/daemon ownership protocol in Phase 7.

### 8.5 Full replacement still has a role

Retain the existing safe full-store replacement path for:

- explicit `ctx index` force rebuild;
- missing store;
- incompatible schema/compatibility contract;
- corrupt/untrusted store;
- incremental state that cannot be safely reconciled.

Incremental mutation is an optimization over a trusted cache generation, not the only recovery mechanism.

### 8.6 No cache compaction requirement

SQLite page reuse/fragmentation and cache GC are not Phase 7 product scope. Continue enforcing the DB ceiling. If repeated churn can eventually require a full rebuild because the store reaches a bound, document that rather than adding a maintenance subsystem.

---

## 9. Refactor the Analyzer Around a Shared File Unit

`index::analyze_scan` currently owns per-file analysis and repository-wide aggregation in one loop. Phase 7 should extract a reusable per-file primitive without creating duplicate analysis semantics.

A good conceptual decomposition is:

```text
AnalyzerRegistry
    │
    ▼
analyze_file(file, source loader, graph=true)
    │
    ▼
FileAnalysisUnit

Repository accumulator/validator
    │
    ├── full build: add every file unit
    └── incremental: retain unchanged + replace changed units
    │
    ▼
RepositoryIndex + Vec<FileRelations>
```

Requirements:

- full analysis and incremental analysis call the same file-analysis code;
- Rust/Python parser behavior, diagnostics, statuses, ranges, and relation extraction stay identical;
- one batch of changed files should reuse one analyzer registry where practical rather than initializing parsers per file;
- aggregate limits are checked over the final repository state;
- changed-file analysis cannot bypass symbol/name/detail/diagnostic/relation/entity ceilings merely because each file fits alone;
- retained cache data is validated under the same aggregate invariants before persistence/query use.

Avoid a public trait hierarchy solely to support this refactor. A plain struct/function boundary is preferable unless current code proves otherwise.

---

## 10. Incremental Reconciliation Algorithm

Introduce one application-level reconciliation path in or adjacent to `persistence.rs`. A separate `incremental.rs` module is reasonable if it clarifies ownership; do not create a second persistence service.

Conceptual flow for automatic current-state commands:

```text
scan A
  │
  ▼
compute file input stamps A
  │
  ▼
locate store
  │
  ├── missing/incompatible/corrupt ──► full build + full save
  │
  ▼
load validated persisted snapshot S
  │
  ▼
compare S file stamps to A
  │
  ├── no delta ──────────────────────► attach live Git/warnings; return
  │
  └── delta
        │
        ├── removed paths
        ├── changed paths
        └── added paths
        │
        ▼
analyze only changed/added files
        │
        ▼
merge with retained file units
        │
        ▼
validate repository-wide limits
        │
        ▼
scan B + compute file stamps B
        │
        ├── A != B ──► discard candidate / bounded retry
        │
        ▼
apply SQLite patch expecting S generation
        │
        ├── write conflict ──► reload / bounded retry
        │
        └── success
        ▼
return updated artifact
```

### 10.1 Delta rules

Treat path identity as the primary incremental ownership key.

- same path + same input stamp -> retain persisted unit;
- same path + changed input stamp -> recompute unit;
- current path absent from store -> add/recompute unit;
- stored path absent from current scan -> remove unit;
- rename -> delete old + add new unless a future lineage phase proves a stronger identity;
- role/language/eligibility change -> recompute unit because stored analysis status may change;
- ignored/unignored file -> remove/add according to the canonical current scan.

Do not infer symbol rename lineage in this phase.

### 10.2 Read/freshness failure

Preserve the conservative Phase 6 behavior.

If required current source freshness cannot be established safely:

- do not trust cached rows as current;
- automatic commands may return bounded live best-effort analysis with explicit `ReadFailed` evidence;
- do not persist that incomplete generation;
- explicit `ctx index` remains an error;
- watcher mode should report the problem and leave the last committed valid cache generation intact.

Do not merge an unreadable changed file with stale cached symbols and call the result current.

### 10.3 Repository mutation during reconciliation

The pre/post generation check must cover the whole analysis-relevant input set, not only the files initially believed to be changed.

Otherwise an unrelated file could change while the worker is parsing the first file and the engine could persist a mixed generation.

As in Phase 6:

- allow at most one bounded automatic retry unless implementation evidence justifies a similarly small explicit bound;
- never loop indefinitely;
- never knowingly commit a mixed snapshot;
- if a stable generation cannot be established, fail explicit index/watch update appropriately or return uncached best-effort analysis for an automatic one-shot query without corrupting JSON stdout.

### 10.4 Incremental write failure

If fresh in-memory reconciliation succeeds but the automatic incremental SQLite patch fails:

- rollback/preserve the prior committed cache;
- return the fresh in-memory result where existing Phase 6 semantics allow it;
- emit a sanitized warning to stderr;
- do not claim the cache advanced;
- the next command may retry.

Explicit `ctx index` remains stronger: persistence failure is command failure.

---

## 11. Graph and Dependency Invalidation

The graph remains derived state.

After an incremental artifact update:

```rust
graph::from_analysis(updated_index, updated_relation_files)
```

must be the only production graph reconstruction path unless a later measured phase explicitly changes that decision.

This provides correct invalidation for:

- removed symbols;
- changed symbol ownership/ranges;
- added/removed relation candidates;
- changed extraction status;
- future cross-file resolution improvements.

The phase should prove that a changed file cannot leave stale graph relations that existed only in the prior generation.

Do not persist dependency invalidation queues or resolved graph IDs in SQLite.

---

## 12. User Surface: On-Demand Incrementality

All existing current-state intelligence commands should benefit automatically:

```text
index          force full rebuild/repair (unchanged strong semantics)
symbols        incremental reconcile when needed
symbol         incremental reconcile when needed
graph          incremental reconcile when needed
deps           incremental reconcile when needed
callers        incremental reconcile when needed
related        incremental reconcile when needed
search         incremental reconcile when needed
neighborhood   incremental reconcile when needed
hotspots       incremental reconcile when needed
context        incremental reconcile when needed
```

Keep unchanged:

```text
scan / stats / tree    live scanner views
diff / impact          committed Git revision analyses
```

`context --change` must retain all Phase 5 snapshot gates and post-content recheck semantics. Incremental current-state reuse must not weaken the clean/current-HEAD requirement.

No new flag should be required to get incremental behavior.

---

## 13. User Surface: Foreground `ctx watch`

Phase 7 should expose filesystem watching as a small foreground operational command rather than inventing a daemon.

Expected shape:

```bash
ctx watch [PATH]
ctx watch [PATH] --max-parse-file-size <bytes>
```

Exact help wording may vary.

### 13.1 Semantics

`ctx watch` should:

1. canonicalize/validate the requested root using existing index semantics;
2. perform an initial full/incremental reconciliation so the external cache starts from a valid generation;
3. recursively watch the requested repository scope;
4. coalesce filesystem activity into bounded reconciliation triggers;
5. reconcile against actual scan/fingerprint state;
6. commit file-local updates transactionally;
7. continue until process termination or a fatal watcher/configuration error.

It is acceptable for the process to be terminated by the user's normal signal without a bespoke signal framework because each committed update is transactional. Do not add a signal-handling dependency unless needed for a concrete correctness/usability reason.

### 13.2 Events are hints, never evidence

Do not directly trust watcher event kinds/paths to update SQLite.

Filesystem backends can:

- coalesce events;
- emit duplicate events;
- represent rename differently;
- report paths after they moved;
- overflow/error;
- vary across platforms.

Therefore:

> Any event means “repository state may have changed; reconcile.”

The canonical scan + file input stamps determine the actual delta.

This also correctly handles ignore-rule changes, atomic-save rename patterns, create/delete bursts, and editor-specific writes.

### 13.3 Bounded queue and ownership

Use a bounded synchronous queue, preferably standard-library primitives unless the watcher API makes another small dependency materially cleaner.

Recommended ownership:

```text
notify callback thread(s)
        │
        ├── try_send lightweight event signal
        └── set overflow/resync flag if queue is full/error
        │
        ▼
single watch/reconcile owner loop
        │
        ▼
persistence reconciler / store
```

A small `Arc<AtomicBool>` or equally narrow synchronization primitive for overflow/resync state is reasonable. Avoid an `Arc<Mutex<RepositoryGraph>>` architecture: there is no in-process query server in Phase 7.

The worker/reconcile loop should own mutable incremental state. The watcher callback should not parse, scan, access SQLite, or render.

### 13.4 Event coalescing

Use a small internal debounce/coalescing window so an editor save does not trigger several complete reconciliations.

The precise duration is not a public product contract and should not dominate tests.

If the queue overflows or the watcher reports an event-delivery error:

- mark a full reconciliation required;
- do not drop the condition silently;
- once the worker wakes, reconcile from actual repository state.

Do not implement an unbounded event queue.

### 13.5 Watch output contract

Keep this intentionally small.

Recommended behavior:

- operational startup/update/warning messages are human-readable and written by the CLI layer to stderr;
- stdout is unused by `watch` in Phase 7;
- do not expose an unversioned streaming JSON protocol just to make watch machine-readable;
- Phase 9 can expose a deliberate service/event API over the same reconciler later.

The underlying watcher/reconciler module must not print directly. It should return/callback structured status to the CLI boundary.

### 13.6 Watch reliability claims

Do not claim that OS watcher delivery itself is a perfect atomic source of truth.

Correctness guardrails are:

- initial reconciliation;
- full deterministic reconciliation after every received/overflow/error trigger;
- transactional cache commits;
- ordinary one-shot current-state commands still perform freshness reconciliation before trusting the store.

A silent kernel/backend event loss that is not surfaced until a later query is a known watcher limitation unless a measured periodic resync is deliberately added. Do not add polling complexity merely to overclaim perfection.

---

## 14. Concurrency and Synchronization Boundaries

Phase 7 is the first concurrency milestone, but concurrency should remain narrow.

### Allowed/expected

- filesystem watcher callback threads owned by the watcher library;
- bounded channel from event callbacks to one reconciliation owner;
- small atomics/synchronization for queue-overflow/resync state;
- SQLite transactional exclusion and optimistic generation checking;
- bounded retry after repository/store races.

### Not required

- parallel parsing;
- Rayon;
- Tokio;
- async functions;
- shared mutable graph across threads;
- worker pools;
- daemon lifecycle;
- interprocess lock service.

Sequential changed-file analysis is preferable until profiling demonstrates parser parallelism is worthwhile.

The main Rust learning objective is to introduce channels/thread ownership safely, not maximize concurrency.

---

## 15. Failure Semantics

Keep failures explicit and centralized.

Potential new typed outcomes/errors include conceptually:

```text
StoreConflict
WatcherUnavailable
WatcherEventError / ResyncRequired
IncrementalValidationError
```

Exact names are flexible.

Requirements:

- no panic on malformed repository/cache/watcher input;
- no indefinite SQLite/channel waits;
- no partial committed file patch;
- no stale cache promoted to current after unknown freshness;
- no JSON stdout contamination on existing JSON commands;
- watcher warnings/errors must be control-character escaped/sanitized at the CLI boundary;
- a failed incremental transaction leaves the prior committed generation usable;
- a store-generation conflict triggers bounded reconcile/reload, not blind overwrite.

---

## 16. Security and Path Safety

Watcher input is untrusted operational input, just like repository paths.

Rules:

- do not read/analyze a raw watcher path directly;
- event paths do not bypass canonical scan-root containment;
- source reads continue through the safe bounded loader/no-follow behavior;
- ignore symlink/event tricks by reconciling through the canonical scanner;
- cache remains outside the analyzed root;
- target repository code is never executed;
- no network access is needed;
- no source body is stored in SQLite;
- native non-UTF-8 relative paths remain lossless internally.

If adding `notify`, configure recursive watching of the intended root but avoid depending on platform-specific event names for correctness.

Repository `.git/` activity may be filtered as an operational optimization because it is outside the indexed inventory and Git metadata is not persisted; ordinary repository-path events must not be over-filtered in ways that miss ignore or classification changes.

---

## 17. Code-Level Change Map

Expected production changes are approximately:

```text
Cargo.toml / Cargo.lock
  + filesystem watcher dependency (likely notify)
  no Tokio/Rayon requirement

src/
├── cli.rs
│   add small `watch` command args/help
│
├── main.rs
│   route watch through an operational CLI loop/callback
│   keep rendering/output ownership at the boundary
│
├── index.rs
│   extract reusable per-file analysis unit
│   make full and incremental assembly share the same analysis/validation logic
│
├── persistence.rs
│   replace global stale=>full-rebuild behavior with trusted-snapshot reconciliation
│   file-input stamp computation / repository generation
│   bounded mutation/store-conflict retry
│   retain explicit full-rebuild path
│
├── store.rs
│   schema v3
│   persist per-file input stamps
│   validated stale-snapshot load
│   transactional file-local patch with expected-generation check
│   artifact digest includes incremental metadata
│
└── incremental.rs          (new; name flexible)
    watcher adapter / bounded event queue / reconciliation loop
    no parsing/query/rendering duplication
```

Tests may add:

```text
tests/incremental.rs
```

and focused unit tests in `index.rs`, `persistence.rs`, `store.rs`, and the watcher module.

Do not split into a multi-crate workspace.

---

## 18. Implementation Stages

### Stage A — Reconfirm baseline and invariants

Before editing:

1. inspect actual `HEAD`, `git status`, and any user changes;
2. read `AGENTS.md`;
3. read `docs/current-state.md`;
4. read this Phase 7 plan;
5. read Phase 6 implementation + review-fix reports;
6. inspect `persistence.rs`, `store.rs`, `index.rs`, `graph.rs`, `main.rs`, `cli.rs`, `source.rs`, `scan.rs`, models and persistence tests;
7. read PRD Phase 7 and architecture persistence/incremental sections;
8. run the canonical verifier once if practical.

Do not reset unrelated changes.

### Stage B — Factor one canonical file-analysis primitive

Refactor `index.rs` so:

- full repository analysis uses a reusable file-analysis unit;
- incremental batches can analyze only selected current `FileRecord`s;
- analyzer registry setup is reused within a batch;
- aggregate validation can validate a final assembled artifact;
- current full-build outputs remain identical.

Run focused symbol/graph/index tests before touching storage semantics further.

### Stage C — Introduce file freshness model

Implement:

- `freshness-v2` (or equivalent distinct contract);
- bounded per-file input stamps;
- canonical repository generation digest;
- deterministic delta classification;
- native-path-safe map keys.

Prove same-length eligible edits, classification changes, add/remove/rename, skipped-body behavior, invalid UTF-8, and aggregate read limits.

### Stage D — Schema v3 and snapshot load

Implement:

- per-file input stamps in storage;
- schema v3 compatibility;
- artifact digest coverage of stamps;
- validated snapshot loading independent of current-inventory equality;
- full save using the new schema.

Old schema v2 should rebuild safely.

### Stage E — Transactional file patch + CAS

Implement file-local store mutation:

- add;
- modify;
- remove;
- child-row replacement/removal;
- aggregate preflight;
- artifact digest recomputation;
- expected-generation compare-and-swap;
- rollback preservation;
- typed conflict.

Use direct store tests before CLI wiring.

### Stage F — Automatic on-demand incremental reconciliation

Replace stale=>full-rebuild for trusted compatible stores with:

- delta;
- changed/new file analysis only;
- retained-unit merge;
- final aggregate validation;
- whole-input post-validation;
- transactional patch;
- bounded repository/store-race retry.

Keep missing/corrupt/incompatible as full rebuild.

Wire all existing current-state consumers through this single path.

### Stage G — Watcher adapter and foreground command

Add:

- watcher dependency;
- bounded queue;
- single owner/reconcile loop;
- overflow/error resync flag;
- event coalescing;
- structured watch status surfaced by CLI;
- foreground `ctx watch` command.

Do not make watcher events the source of the delta.

### Stage H — Adversarial/race verification

Exercise:

- file changes during incremental parse;
- second unrelated file changes during first-file analysis;
- concurrent store generation conflict;
- modified file becomes unreadable/non-regular;
- delete/create/rename bursts;
- ignore-file change;
- store corruption;
- schema-v2 cache;
- watcher queue overflow/error path via deterministic injection;
- non-Git repository;
- clean and dirty `context --change` regression behavior.

### Stage I — Full verification, release smoke, docs/report

Run one final full quality pass, smoke real binaries, then update truthful documentation and write the Phase 7 implementation report.

---

## 19. Focused Testing Strategy

Testing should prove incremental correctness, not multiply every command by every event sequence.

### 19.1 Full vs incremental file-analysis equivalence

Create a compact multi-file Rust/Python fixture.

Prove that:

- full build and incremental assembly produce equivalent `RepositoryIndex` and `FileRelations` for the same final repository state;
- file order does not change semantic results;
- parse failure/invalid UTF-8/too-large/skipped role statuses match full analysis.

### 19.2 Deterministic proof that unchanged files are not re-parsed

Do not rely on timing.

Use an injected analyzer/loader counter or equivalent seam to prove:

- edit one eligible file -> only that file is analyzed;
- add one eligible file -> only the new file is analyzed;
- remove a file -> no unrelated parser work;
- rename -> old removed + new analyzed;
- no delta -> zero parser work;
- changed unsupported/skipped/over-limit inputs do not invoke an analyzer incorrectly.

This is a central Phase 7 acceptance property.

### 19.3 Stale child-data removal

For a changed file that previously had symbols, diagnostics, and relation candidates, update it to a state that removes them.

Prove the incremental result/store contains no stale:

- symbol rows;
- diagnostic rows;
- extraction/candidate rows;
- graph edges;
- query results caused solely by the old file generation.

### 19.4 Repository-wide limits after patch

Use injected small limits.

Prove retained + changed data can fail:

- symbol count;
- diagnostic count;
- name/detail bytes;
- relation/entity counts/bytes;
- encoded store payload;
- path/store ceilings.

A changed file that fits alone must not bypass an aggregate repository limit.

### 19.5 Snapshot mutation race

Inject a mutation after initial file stamps and during changed-file analysis.

Prove:

- the candidate generation is discarded;
- one bounded retry can succeed when the repo stabilizes;
- repeated mutation fails without committing a mixed generation;
- prior cache remains valid/unchanged.

Also mutate an initially “unchanged” second file during the first-file parse to prove post-validation covers the whole relevant input set.

### 19.6 Store CAS / multi-process-equivalent conflict

Without requiring flaky real processes, open two store views/generations and prove:

1. writer A commits from generation S0 to S1;
2. writer B attempts patch with expected S0;
3. writer B gets a typed conflict;
4. S1 remains intact;
5. a reload/reconcile can subsequently produce the correct final generation.

### 19.7 Watch queue/worker tests

Make event handling testable through an injected event source/status callback rather than depending entirely on OS timing.

Prove:

- duplicate events coalesce into one reconciliation opportunity;
- queue full/overflow marks resync rather than silently dropping correctness state;
- watcher error triggers resync/fatal behavior as designed;
- raw event path is never treated as authoritative analysis input;
- ignored/unignored/create/delete/rename final state is determined by scan reconciliation;
- callback thread does not perform parsing/store writes.

One real `notify` smoke on the local platform is useful if stable, but it should not be the sole correctness test and should not make the canonical suite flaky.

### 19.8 Command semantic regression

Representative cold/full, warm, and incremental generations should produce identical semantic output for unchanged final repository state across:

- `symbols`;
- `graph`;
- `search`;
- `neighborhood`/`hotspots` where relevant;
- `context`.

Continue existing JSON determinism checks.

### 19.9 Explicit rebuild recovery

Prove `ctx index` still:

- forces full analysis rather than trusting stale incremental state;
- replaces schema-v2/corrupt cache safely;
- fails if persistence promised by the command cannot commit;
- leaves repository Git status unchanged.

### 19.10 Native-path fidelity

Retain Phase 6 non-UTF-8 path tests and add file-local update coverage where the platform permits it.

Incremental path maps/deltas must not round-trip through lossy UTF-8.

---

## 20. Verification and Smoke Plan

During implementation, use focused test targets for the active workstream.

Near completion run:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
git diff --check
```

Use an isolated external cache:

```bash
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx index .
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx symbols . --format json
CONTEXT_ENGINE_CACHE_DIR=<tmp> ./target/release/ctx graph . --format json
```

Then on a small temp repository:

1. full index;
2. modify exactly one Rust/Python file;
3. run a current-state command and verify updated output;
4. independently verify unchanged files were not reanalyzed through test instrumentation (release timing is not proof);
5. add/remove/rename files and verify correct final state;
6. corrupt or downgrade the cache and verify full rebuild recovery;
7. verify cache remains outside the repository and Git cleanliness is unchanged.

For watcher smoke, use a temp repository and a bounded manual/scripted run if the environment permits:

- start `ctx watch`;
- edit/add/remove a file;
- observe successful reconciliation status;
- terminate the foreground process;
- run a normal one-shot query and confirm the cache represents the final repository state.

Do not add a flaky wall-clock performance threshold. Optional measurements may be reported, but the deterministic acceptance fact is parser/extractor invocation count for unchanged files.

Record exact commands/results and any platform limitations.

---

## 21. Documentation and Reporting

Implementation completion must create:

```text
docs/reports/phase-7-implementation-report.md
```

The report must include:

- starting and ending commits/worktree state;
- authorization/gating status relative to Phase 6 review;
- final freshness/schema/analysis-compat versions;
- file input stamp algorithm;
- repository generation/CAS semantics;
- exact incremental unit and analyzer refactor;
- store patch transaction semantics;
- repository mutation retry behavior;
- multi-writer conflict behavior;
- watcher dependency and concurrency ownership;
- queue/overflow/error behavior;
- user-facing `ctx watch` semantics;
- resource/safety bounds;
- command wiring;
- test and release-smoke evidence;
- observed limitations/deviations;
- explicit Phase 8/9+ remaining work.

Update after the capability actually ships:

```text
README.md
docs/current-state.md
docs/architecture.md
```

Potential stale statements that must be reconsidered include:

- “Phase 7 incremental indexing and watchers remain future work”;
- “No incremental indexing / watchers / concurrency exists”;
- `SCHEMA_VERSION = 2` descriptions if schema v3 ships;
- `freshness-v1` as the current freshness contract;
- stale behavior descriptions saying any changed source causes whole-repository reanalysis;
- status headings that still call Phase 6 the active/latest implementation;
- CLI command listings that omit `watch` if it ships.

Do **not** rewrite the PRD just to say Phase 7 happened. Update it only if product intent genuinely changes.

Do **not** create `handoff.md` or another parallel living-status file.

Update `AGENTS.md`, `docs/development/llm-workflow.md`, or the Context Integrity integration doc only if Phase 7 creates a durable rule/boundary those files must actually carry; do not copy implementation detail into them reflexively.

---

## 22. Phase 7 Acceptance Criteria

### Incremental correctness

- A trusted compatible stale snapshot can be compared to current file-level analysis inputs.
- Only changed/new analyzable files are parsed/extracted during incremental reconciliation.
- Removed files and all dependent stored child records disappear.
- Final incremental artifact is semantically equivalent to a full rebuild of the same stable repository state.
- Rename/classification/role/parse-eligibility changes are handled conservatively and correctly.
- Repository-wide limits are enforced after merging retained + changed file units.

### Freshness and races

- Same-length eligible edits are detected.
- Unknown freshness never becomes an incremental cache hit.
- Whole-input post-validation prevents committing a known mixed snapshot.
- Repository mutation retries are bounded.
- Store writes use an expected-generation check so stale concurrent patches cannot blindly overwrite newer generations.
- Incremental write failure preserves the prior committed generation.

### Storage

- Per-file input stamps are persisted and integrity-covered.
- Schema v2 is handled as incompatible derivable cache and rebuilds safely.
- Changed-file structural rows are replaced atomically.
- Unchanged file row identities/child ordinals do not needlessly renumber.
- No source bodies, graph IDs, adjacency, query results, or ranking output are persisted.
- Native path identity remains lossless.

### Graph/query integration

- Graphs are still reconstructed through the canonical analysis-to-graph path.
- No stale graph edge survives a changed/removed file generation.
- Existing current-state commands automatically benefit from incremental reconciliation.
- `scan`/`stats`/`tree`, `diff`/`impact`, and `context --change` retain their established semantics.
- `ctx index` remains a force-full-rebuild/repair path.

### Watcher/concurrency

- A foreground `ctx watch` performs initial reconciliation and then reacts to filesystem activity.
- Event delivery uses a bounded queue/coalescing design.
- Raw event paths/kinds are not authoritative; actual repository reconciliation is.
- Queue overflow/watcher errors trigger explicit resynchronization or clear fatal behavior, never silent state loss.
- Mutable incremental state has one clear owner; no unnecessary shared mutable graph architecture is introduced.
- No Tokio/daemon/API layer is added.

### Quality

- focused incremental/store/watcher race tests pass;
- canonical verifier passes;
- all-target check passes;
- release build passes;
- diff whitespace check passes;
- release-binary incremental/watch smokes pass where the local platform supports them;
- implementation report is complete;
- public/current architecture docs are truthful and no longer describe Phase 7 as future work after it ships.

Independent review is still required after local implementation verification.

---

## 23. Explicit Non-Goals / Phase Stop Boundary

Do **not** add in Phase 7 unless a concrete correctness blocker forces a narrowly documented exception:

- Tokio/async runtime;
- HTTP server;
- Unix-socket/IPC service;
- MCP adapter;
- installed/background daemon or OS service manager integration;
- parallel parser worker pool/Rayon;
- persistent in-memory query server;
- historical revision indexing/cache;
- blame/co-change/evolution model;
- cross-repository registry or contract graph;
- compiler/LSP/SCIP semantic backend;
- cross-file resolution work merely to make Phase 7 appear richer;
- Tantivy/full-text index;
- embeddings/vector search/model calls;
- Context Integrity authority/policy/drift semantics;
- stable symbol rename/refactor lineage;
- persistent graph IDs/adjacency;
- cache GC/compaction subsystem;
- remote/team cache synchronization;
- multi-crate restructuring.

Phase 8 owns history/evolution intelligence. Phase 9 owns the local service + agent adapter and is the proper point for async/network/IPC concerns.

---

## 24. Downstream Compatibility Requirements

### 24.1 Phase 8 — History and Evolution

Incremental current-state generations must remain distinct from historical revision lineage.

Do not treat a file storage key, file input stamp, or per-generation symbol position as a stable identity across refactors/commits.

### 24.2 Phase 9 — Local Service + Agent Adapter

The reconciler should be callable as application logic without depending on Clap or stderr.

A future service should be able to:

- keep one validated artifact/graph warm;
- subscribe to the same reconciliation engine;
- expose queries concurrently;

without rewriting file invalidation or SQLite semantics.

Phase 7 should not pre-build the service, but it should avoid burying incremental logic inside the `watch` CLI handler.

### 24.3 Phase 10 — Cross-Repository Intelligence

Repository generation/file input identities remain repository-local. Do not create accidental global identities from cache locators or path hashes.

### 24.4 Phase 11 — Context Integrity Evidence Adapter

Context Integrity must consume stable evidence/query contracts, not SQLite schema v3, watcher events, or internal generation tokens directly.

Incremental updates may improve freshness, but they do not move authority/policy semantics into this engine.

---

## 25. Key Risks the Implementation and Review Must Challenge

This plan is specific, but it is not infallible. Implementation and independent review must challenge at least:

1. **False unchanged classification:** can any file content/path/classification/option change retain an old analysis unit incorrectly?
2. **Mixed generation:** can another file change during incremental analysis and escape post-validation?
3. **Stale child data:** can old symbols/diagnostics/candidates survive a changed or removed file?
4. **Aggregate-limit bypass:** do retained + changed records still pass the same global limits as a clean full build?
5. **Store race:** can two processes patch from the same base and lose one another's updates?
6. **Artifact integrity:** are file input stamps and structural rows both covered by the corruption check?
7. **Graph invalidation:** can derived graph/query state still expose an edge/entity from the prior generation?
8. **Path fidelity:** do incremental maps/store operations preserve native non-UTF-8 paths without lossy keys?
9. **Watcher trust:** does any code directly mutate analysis from an event path/kind without canonical reconciliation?
10. **Queue behavior:** can event bursts cause unbounded memory, deadlock the callback, or silently miss required resync?
11. **Watcher portability:** are exact event kinds/rename sequences assumed across macOS/Linux/Windows?
12. **Concurrency ownership:** did Phase 7 introduce broad `Arc<Mutex<...>>` state that Phase 9 will immediately replace?
13. **Full-build divergence:** do full and incremental paths share one file analyzer and one aggregate validator?
14. **Explicit index semantics:** did optimization accidentally make `ctx index` stop being a reliable full rebuild/repair path?
15. **Failure semantics:** does unreadable current source ever cause stale cached evidence to be presented as current?
16. **Freshness cost:** is file-level hashing still honest about its per-request cost, and is parsing reduction actually proven independently of timing?
17. **Public contracts:** did `watch` accidentally create an unversioned machine stream or leak storage implementation details?
18. **Phase creep:** did implementation slide into daemon/API/history/cross-repo/semantic-resolution work?
19. **Documentation truth:** are schema/freshness/status/limitations/help descriptions consistent with actual shipped behavior?
20. **Plan error:** is there a simpler/safer design in the actual code that achieves the phase goal while preserving downstream seams? A reviewer should say so rather than reward plan conformity.

A conforming implementation should still be rejected if these questions reveal a correctness or architectural flaw.

---

## 26. Summary

Phase 7 should add one new property to Context Engine:

> **A validated persisted analysis snapshot can advance from generation to generation by reanalyzing only files whose analysis inputs changed, with bounded race-safe storage updates and an optional foreground watcher that triggers the same deterministic reconciliation engine.**

The intended progression remains:

```text
Phase 6
validated persistent whole-analysis snapshot
        │
        ▼
Phase 7
file-local incremental generations
+ foreground watch trigger
        │
        ▼
Phase 8
history/evolution intelligence
        │
        ▼
Phase 9
long-lived service + agent adapters
```

The phase succeeds by reducing redundant analysis without weakening freshness, evidence correctness, path fidelity, resource bounds, failure semantics, or the separation between descriptive code intelligence and future Context Integrity policy.
