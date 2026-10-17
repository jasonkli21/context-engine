# Context Engine — Phase 8 Implementation Plan

**Phase:** 8 — History and Evolution Intelligence
**Status:** Planned; implementation requires maintainer authorization if Phase 7 fresh independent re-review is still pending
**Date:** 2026-10-16
**Repository:** `jasonkli21/context-engine`
**Planning baseline inspected:** `ef44712858f02b34f1fd4b43201ad6f26bbc3cf2` (`fix: address phase 7 review findings`)
**Primary language:** Rust
**Binary:** `ctx`
**Primary new capability:** bounded, deterministic historical/evolution queries over immutable Git objects, including current-symbol history, path evolution, rename-aware lineage where evidence is strong, and explicit co-change evidence
**Explicitly deferred:** daemon/service/API/MCP, Tokio/async, cross-repository history, persistent historical index/cache, historical graph snapshots, compiler/LSP/SCIP resolution, embeddings/model calls, learned ranking, full-DAG semantic lineage, blame/ownership intelligence, historical centrality unless it is demonstrably cheap and isolated, arbitrary policy/drift judgments, Context Integrity semantics

---

## 1. Planning Baseline and Gate

This plan is based on the actual `main` state inspected on 2026-10-16.

Latest inspected commit:

```text
ef44712858f02b34f1fd4b43201ad6f26bbc3cf2
fix: address phase 7 review findings
```

Its parent is the initial Phase 7 implementation:

```text
0f9662b882b7dc45c2865c1418a8b35629c0a6d7
feat: implement phase 7 incremental indexing and foreground watch
```

The baseline already contains:

- schema-v3 / `freshness-v2` persistent current-state analysis;
- file-local incremental reconciliation;
- bounded all-writer coordination and CAS conflict handling;
- a foreground watcher with canonical periodic reconciliation;
- immutable committed-revision analysis for `diff` / `impact`;
- task-context ranking and current graph/query surfaces.

`docs/current-state.md` still says Phase 7 requires a fresh independent re-review and several earlier phase dispositions remain pending. Therefore:

> **Planning or implementing Phase 8 does not accept Phase 7 or any earlier pending phase.**

Implementation may begin when either:

1. the required Phase 7 re-review/disposition has completed; or
2. the maintainer explicitly directs an implementation session to proceed from the current baseline despite the pending disposition.

Such direction is authorization to proceed, not independent acceptance.

If `main` advances beyond the planning baseline, do not reset or recreate this snapshot. Inspect the actual `HEAD`, worktree, current docs, implementation/review reports, source, tests, and durable decisions. Reconcile this plan to the real repository before editing and record material deviations in the Phase 8 implementation report.

---

## 2. Authority and Alignment

### 2.1 PRD intent

The PRD defines Phase 8 as **History and Evolution Intelligence**.

Its goal is:

> Model how abstractions change over time.

The roadmap names:

```bash
ctx history <symbol>
ctx evolution <path>
```

and potential features including:

- introduction commit;
- major modifications;
- co-change relationships;
- lineage heuristics;
- historical centrality.

The broader PRD also requires:

- deterministic evidence before probabilistic inference;
- structural + historical context rather than plain text alone;
- inspectable provenance;
- local-first operation with no mandatory network/model cost;
- honest uncertainty;
- stable domain boundaries;
- revision-scoped identity until robust lineage exists;
- history as descriptive evidence, not architectural/policy judgment.

### 2.2 Architecture intent

Current architecture already has the right immutable-history foundation:

```text
Git commit objects
      │
      ▼
git_revision.rs
  - resolve commits
  - diff-tree
  - ls-tree
  - cat-file
      │
      ▼
change.rs
  - build revision snapshots
  - analyze source through existing index pipeline
  - compare symbols/relations
  - optional impact traversal
```

Phase 8 should extend this seam rather than create a second Git/parser/index architecture.

Durable boundaries to preserve:

- target repository code is never executed;
- historical source comes from immutable Git objects, not checkout mutation;
- current-state SQLite remains a current repository derivative cache;
- historical queries do **not** write old revisions into that cache;
- parser/classifier/relation semantics stay shared with current-state analysis;
- renderers do not invoke Git, scan, parse, or access SQLite;
- CLI types do not become domain types;
- numeric graph IDs never imply cross-revision lineage;
- Context Integrity policy/authority/drift semantics remain downstream;
- no Phase 9 service/async architecture is pulled forward.

### 2.3 Newer repository guidance

Implementation must also respect:

- `AGENTS.md`;
- `docs/current-state.md`;
- `docs/development/llm-workflow.md`;
- `docs/architecture.md`;
- `docs/reports/phase-7-implementation-report.md`;
- `docs/reports/phase-7-review-fix-report.md`;
- actual Phase 7 source/tests at implementation time.

The repository’s own workflow requires implementation to reconcile the plan against real code and requires independent review to reconstruct the architecture rather than merely map plan bullets to files.

---

## 3. Current Phase 7 Reality Relevant to Phase 8

### 3.1 What exists

The current tree already has:

- `src/git_revision.rs` — hardened read-only immutable Git-object access;
- `src/change.rs` — revision snapshots and symbol/relation comparison;
- `src/index.rs` — canonical per-file and repository analysis;
- `src/graph.rs` — canonical graph reconstruction;
- `src/persistence.rs` / `src/store.rs` — current-state caching only;
- `src/cli.rs` / `src/main.rs` — command parsing/dispatch;
- separate output modules for change/query/context/index/graph;
- explicit aggregate limits and versioned JSON surfaces.

`change::analyze()` currently compares one explicit committed `BASE..HEAD` range. For `diff`, it analyzes changed sides. For `impact`, it inventories full revision-local scoped trees and reconstructs revision graphs.

### 3.2 Important current limitations

Current code does **not** provide:

- commit-history enumeration;
- commit metadata timelines;
- reusable historical lineage types;
- symbol introduction/modification history;
- path evolution summaries;
- co-change statistics;
- rename-aware symbol history across multiple commits;
- persistent history data;
- arbitrary revision DAG traversal semantics.

`docs/current-state.md` explicitly says stable symbol lineage remains absent. Phase 8 may introduce **bounded heuristics with explicit evidence**, but it must not silently upgrade revision-scoped IDs into stable global identity.

---

## 4. Phase 8 Objective

After Phase 8, a developer should be able to ask:

```bash
ctx history ModelRouter
ctx history ModelRouter --file src/router.rs
ctx evolution src/providers
```

and receive bounded, deterministic, explainable evidence such as:

```text
ModelRouter
start revision: <HEAD>
history policy: first-parent
lineage: exact kind + qualified name, rename-aware

introduced:
  <commit>

modified:
  <commit>
  <commit>

file lineage:
  src/router_old.rs -> src/router.rs

co-changed most often:
  tests/router.rs  4
  src/providers.rs 3

coverage:
  inspected 87 commits
  truncated: false
  lineage complete through introduction: true
```

For path evolution:

```text
src/providers/

inspected commits: 128
commits touching target: 19
files added: ...
files removed: ...
renames entering/leaving target: ...
symbol additions/removals/modifications: ...
common co-change paths: ...
history truncated: true
```

The output must distinguish:

- facts directly derived from Git;
- structural facts derived from existing analyzers;
- lineage heuristics;
- ambiguity/incompleteness/truncation.

---

## 5. Phase 8 Success Definition

Phase 8 succeeds if:

1. `history` can resolve a current committed symbol and produce a bounded chronological lineage over immutable Git history;
2. `evolution` can summarize bounded path evolution without mutating the checkout;
3. file renames are followed where Git supplies explicit rename evidence;
4. symbol continuity is based on explicit descriptors/evidence, never graph numeric IDs;
5. introduction/modification/removal/rename events are not invented when comparison is ambiguous or unavailable;
6. co-change is reported as commit co-occurrence evidence, not causality;
7. dirty working-tree content cannot contaminate committed history results;
8. nested scan scopes remain respected;
9. malformed/unsupported/too-large historical source produces explicit coverage gaps rather than false lineage;
10. aggregate Git/history/source/output work is bounded;
11. the current-state cache is not repurposed as a historical store and Phase 8 queries do not write historical snapshots into it;
12. existing `diff` / `impact` semantics and all Phase 0–7 commands remain intact;
13. JSON output is deterministic, versioned, and explicit about history policy, coverage, truncation, and heuristic status;
14. the implementation remains synchronous and local-first;
15. the phase does not introduce Phase 9 service/MCP/async or Phase 10 cross-repository behavior.

---

## 6. Core Design Decision: Historical Queries Are On-Demand Git Evidence

Phase 8 should **not** expand schema v3 into a temporal database.

Conceptually:

```text
Git HEAD
  │
  ▼
bounded commit walk
  │
  ├── immutable commit metadata
  ├── diff/rename facts
  └── selected historical blobs
         │
         ▼
shared classifier / index analyzer
         │
         ▼
history/evolution domain model
         │
         ▼
human / schema-v1 JSON
```

Reasons:

- Git already owns historical object storage;
- the current SQLite store is optimized for current-state warm queries;
- caching N historical snapshots would create migration/eviction/invalidation policy prematurely;
- Phase 9 needs a clean query layer, not a historical cache accident;
- Phase 10 cross-repository intelligence will likely reshape persistence needs.

**Expected versioning impact:** no SQLite schema bump merely for Phase 8. Do not bump `INDEX_COMPAT_VERSION` unless parser/classifier/relation/persisted-enum semantics actually change.

---

## 7. History Traversal Contract

### 7.1 Default history model: first-parent ancestry

Use a deterministic linear history policy for Phase 8:

```text
resolved HEAD
   │
   ▼
HEAD
parent1
parent1(parent1)
...
```

This should be represented explicitly in output, e.g.:

```text
history_policy = "first_parent"
```

Why first-parent initially:

- gives one deterministic sequence;
- avoids pretending a DAG has a single unambiguous symbol lineage;
- bounds complexity;
- makes adjacent-revision comparison straightforward;
- preserves merge results as mainline state transitions.

A merge commit is compared with its first parent. Internal feature-branch commits are not independently traversed unless they are on the first-parent chain.

This limitation must be documented.

### 7.2 No working-tree dependency

History/evolution data must come from Git objects:

- `rev-parse`;
- `log` / equivalent commit enumeration;
- `diff-tree`;
- `ls-tree`;
- `cat-file`.

Do not checkout revisions, stash, reset, or mutate the analyzed repository.

### 7.3 Hardened Git invocation

Extend the existing hardened Git adapter rather than adding ad hoc `Command::new("git")` calls elsewhere.

Preserve:

- `--no-optional-locks`;
- literal pathspec behavior;
- no terminal prompts;
- no replace objects;
- no lazy fetch;
- no external diff/textconv where applicable;
- bounded stdout;
- cleared/untrusted `GIT_*` environment handling;
- no hooks/project-code execution.

Commit subjects/messages, if exposed, are untrusted bounded text and must flow through normal escaping/serialization rules.

### 7.4 Commit metadata

A minimal history commit record should include:

```text
CommitRef
- oid
- first_parent_oid?
- committer_timestamp
- bounded subject?       # optional but useful
```

Do not use wall-clock timestamps as ordering authority. Preserve Git walk order.

### 7.5 Aggregate limits

History must not be unbounded.

Recommended public control:

```text
--max-commits
```

Suggested policy:

- default: 128;
- hard CLI maximum: 512;
- internal test-injectable limit.

Also enforce aggregate limits for:

- commit metadata bytes;
- diff/change records;
- changed-path bytes;
- historical source/blob bytes;
- analyzed historical files;
- emitted history events;
- co-change candidates;
- serialized output.

Reuse existing Phase 4 limits where they naturally apply, but add **history-wide** accounting so a per-pair limit cannot be multiplied across hundreds of commits into unbounded work.

If the commit budget is exhausted, return a valid result with explicit truncation. If a hard safety/resource bound is exceeded before a safe result can be assembled, fail rather than emit misleading partial structural claims.

---

## 8. Reuse/Refactor the Existing Revision Analysis Seam

Do not implement a second historical parser.

Today `change.rs` has private snapshot construction that already:

- classifies Git tree entries;
- reads immutable blob bytes;
- routes source through `index::analyze_loaded`;
- reconstructs the canonical graph;
- records analysis/relation coverage.

Phase 8 should extract or expose the **smallest justified reusable revision-analysis seam**.

Conceptual options:

```rust
analyze_revision_files(...)
analyze_revision_snapshot(...)
analyze_entry_pair(...)
```

Exact names are implementation choices.

The important invariants:

1. current `diff` / `impact` continue using the same production path;
2. Phase 8 uses the same parser/classifier/relation code;
3. source limits remain aggregate and explicit;
4. revision paths remain scan-scope-relative;
5. no renderer reads blobs or invokes Git;
6. refactoring must not weaken Phase 4 safety/coverage semantics.

A reviewer should reject copied snapshot/parser logic unless the implementation demonstrates a compelling reason.

---

## 9. `ctx history <symbol>` Semantics

### 9.1 CLI

Expected shape:

```bash
ctx history <SYMBOL> [PATH] \
  [--file <RELATIVE_FILE>] \
  [--max-commits <N>] \
  [--max-parse-file-size <BYTES>] \
  [--format human|json]
```

Mirror existing symbol/graph lookup behavior where practical.

`--file` is an optional disambiguator and must be an exact scan-root-relative path with the same lexical safety rules used by current graph queries.

### 9.2 Starting revision

Resolve the requested repository/scope and committed `HEAD`.

Resolve the symbol **from the committed HEAD snapshot**, not from dirty working-tree state.

This avoids a hidden clean-worktree precondition and ensures:

```text
history result == committed Git history
```

If the symbol is not present at committed HEAD, Phase 8 may fail with an actionable error. Historical lookup for symbols already deleted from HEAD is deferred unless implementation can add it without complicating identity semantics.

### 9.3 Initial symbol identity descriptor

Use revision-local descriptors, for example:

```text
SymbolDescriptor
- language
- kind
- qualified_name
- relative_path
```

The stable lineage key should not contain numeric graph IDs or source byte offsets.

For current exact lookup:

1. exact qualified name;
2. fallback exact local name;
3. optional file filter;
4. if still zero => not found;
5. if still multiple => ambiguous; fail with candidate count/paths rather than guessing.

### 9.4 Backward lineage algorithm

Walk adjacent first-parent pairs backward.

For each pair:

1. obtain Git file-change/rename facts;
2. determine whether the current lineage file changed;
3. if unchanged, continue without parsing unrelated files;
4. if modified/renamed, analyze only the relevant before/after file evidence needed to compare the lineage;
5. match symbol by exact `(kind, qualified_name)` in the file pair;
6. compare source slice/body using the existing structural comparison seam;
7. classify a bounded event.

Expected event kinds:

```text
introduced
modified
file_renamed
removed_from_lineage
ambiguous
comparison_unavailable
```

A rename and modification may be represented either as one event with multiple flags or deterministic adjacent facts; the machine contract must make the choice explicit.

### 9.5 Rename-aware continuity

If Git emits an explicit rename pairing:

```text
old/path.rs -> new/path.rs
```

and the exact symbol descriptor is uniquely present on both sides, follow the old path backward.

Do not infer a rename solely because the same local symbol name appears elsewhere.

### 9.6 Introduction

If the symbol is present after a commit but has no unique matching symbol before, this may be an introduction **only if** comparison coverage is sufficient to support that conclusion for the tracked file lineage.

If analysis is unavailable/ambiguous at that boundary, report incomplete lineage instead of claiming introduction.

### 9.7 Cross-file symbol moves

A non-Git-rename symbol move such as:

```text
src/a.rs: Foo
  -> src/b.rs: Foo
```

is difficult to establish deterministically.

Phase 8 should default to:

- **do not guess**;
- report lineage loss / possible move only if useful;
- optionally expose a conservative unique exact-descriptor candidate among files changed in the same commit, but mark it explicitly as a heuristic and never as guaranteed identity.

If implemented, this heuristic must have:

- uniqueness requirements;
- bounded changed-file search;
- explicit derivation label;
- tests for duplicates/false-positive resistance.

It is acceptable to defer this heuristic.

---

## 10. `ctx evolution <path>` Semantics

### 10.1 CLI

Because the first positional argument is the target historical path, avoid ambiguous positional reuse of the repository root.

Recommended shape:

```bash
ctx evolution <TARGET> \
  [--repo <PATH>] \
  [--max-commits <N>] \
  [--max-parse-file-size <BYTES>] \
  [--format human|json]
```

`--repo` may default to `.`.

Exact flag naming may be adjusted for consistency after inspecting Clap composition.

### 10.2 Target path handling

The target is a **Git-relative historical selector**, not necessarily an existing current filesystem path.

Therefore:

- do not require `canonicalize()` on the target;
- reject absolute paths;
- reject `..`;
- normalize safe lexical components;
- allow `.` as the requested repository/scope root if explicitly supported;
- preserve lossless/native path handling internally where current Git adapters support it.

### 10.3 Evolution output

For the bounded first-parent window, report:

- resolved start revision;
- history policy;
- commits inspected;
- commits affecting target;
- target/file additions;
- deletions;
- modifications;
- type/mode changes where existing Git facts expose them;
- explicit Git renames entering/leaving/within target;
- structural symbol additions/removals/modifications when analysis is available;
- relation additions/removals when existing comparison coverage supports them;
- co-change summary;
- coverage gaps;
- truncation.

For a directory target, aggregate descendant file events.

Do not claim a Git “directory rename” unless it is derived from explicit file rename evidence and clearly presented as aggregation rather than a native Git directory identity.

### 10.4 Structural work should be selective

Do not build a full repository graph for every historical commit merely to produce path evolution.

Prefer:

```text
commit pair
  │
  ├── Git says target unaffected -> skip structural analysis
  │
  └── target affected -> analyze only relevant changed sides
```

Historical centrality/full graph snapshots are deferred unless profiling and design prove they are necessary.

---

## 11. Co-Change Evidence

Co-change means:

> two paths were changed by the same inspected commit.

It does **not** mean:

- runtime dependency;
- architectural coupling;
- test coverage;
- causality.

For history/evolution commits that affect the target:

1. collect other changed paths from the same scoped commit delta;
2. exclude the tracked target/lineage path itself;
3. count deterministic occurrences;
4. rank by count desc, then path asc;
5. cap retained candidates and rendered results;
6. expose inspected-commit denominator and truncation.

Possible machine form:

```json
{
  "path": "tests/router.rs",
  "cochange_commit_count": 4,
  "target_change_commit_count": 9
}
```

Do not label this as a dependency relation.

---

## 12. Domain Model

Keep history types independent from Clap and rendering.

A reasonable conceptual model:

```text
HistoryResult
- repository_root/scope identity
- start_revision
- history_policy
- query
- resolved_symbol_descriptor
- events[]
- cochanges[]
- coverage
- truncation

HistoryEvent
- commit
- kind
- before_descriptor?
- after_descriptor?
- git_change_evidence?
- structural_evidence?
- derivation
- comparison_status

EvolutionResult
- target
- start_revision
- history_policy
- commit/event summaries
- aggregate structural counts
- cochanges[]
- coverage
- truncation

HistoryCoverage
- commits_inspected
- target_change_commits
- structural_comparisons_attempted
- structural_comparisons_available
- ambiguous_boundaries
- unavailable_boundaries
- history_truncated
- lineage_complete_to_introduction
```

Exact structs may differ.

Avoid speculative general temporal frameworks. Add only types needed by the two commands.

---

## 13. Output Contracts

### 13.1 Human output

Human output should answer, quickly:

- what was queried;
- what start revision/policy was used;
- what changed;
- where lineage is strong;
- where evidence is incomplete/heuristic;
- whether history was truncated.

Escape control characters exactly as existing output code requires.

### 13.2 JSON

Use the repository’s existing schema-v1 envelope style unless a real compatibility reason requires another version.

JSON should expose:

- `command`;
- `schema_version`;
- repository/scope;
- start revision;
- history policy (`first_parent`);
- query/target;
- event list;
- evidence/derivation labels;
- co-change results;
- coverage;
- truncation.

Do not serialize internal structs directly merely because they implement `Serialize`.

### 13.3 Determinism

For unchanged Git objects and identical CLI options:

- commit/event order must be defined;
- co-change ties must be stable;
- candidate/event ordering must be stable;
- no current time should appear;
- dirty working tree should not change committed history output.

---

## 14. Failure and Ambiguity Semantics

### Fatal user/config errors

Examples:

- not a Git repository;
- invalid root/scope;
- invalid target path;
- invalid `--max-commits`;
- current committed symbol not found;
- current committed symbol ambiguous after optional file filter;
- requested Git revision cannot be established.

### Explicit non-fatal/in-result coverage gaps

Examples:

- historical file unsupported;
- historical file too large;
- invalid UTF-8;
- syntax damage;
- ambiguous same-key symbols;
- relation extraction unavailable;
- lineage cannot safely cross a boundary;
- history truncated by user commit limit.

### Hard safety failures

If aggregate safety bounds are exceeded in a way that makes a safe result impossible, fail clearly. Never continue with silently dropped historical commits and then claim a complete introduction/evolution result.

---

## 15. Persistence and Compatibility Rules

Phase 8 history is on-demand and should not change current-state persistence by default.

Expected:

```text
SCHEMA_VERSION = 3          # unchanged
INDEX_COMPAT_VERSION = 1   # unchanged unless analyzer semantics change
freshness-v2               # unchanged
```

Do not:

- store historical commits in current file rows;
- add “current vs historical” flags to current-state records;
- persist graph IDs across revisions;
- treat current cache generation as a Git-history identity;
- let history queries mutate/watch/reconcile current-state cache as a side effect.

It is acceptable for shared helper refactors to touch persistence-independent analysis APIs.

---

## 16. Performance Strategy

Phase 8 can be useful without indexing all history.

Prioritize:

- path-filtered Git operations when correctness is preserved;
- skip parsing when a commit pair does not touch the target;
- analyze only relevant changed files/sides;
- reuse existing source-size and analysis limits;
- avoid full graph construction unless a result actually needs it;
- stop at `--max-commits`;
- keep co-change maps bounded.

No Rayon/Tokio/parallel history walkers in this phase.

Performance claims should be measured with at least a few real repositories and synthetic history fixtures rather than inferred.

---

## 17. Security and Repository-Safety Requirements

Historical repositories are untrusted input.

Phase 8 must not:

- checkout revisions;
- execute hooks;
- run project binaries;
- run package managers;
- invoke external diff/textconv;
- load source through language runtime imports;
- follow filesystem symlinks to historical content;
- upload source/network by default.

Use immutable blob reads and existing parser isolation.

Pay special attention to:

- hostile commit subjects;
- very long commit metadata;
- large histories;
- rename storms;
- huge changed-path sets;
- unusual modes/gitlinks/symlinks;
- non-UTF-8 paths on supported platforms;
- nested scan scopes;
- merge commits.

---

## 18. Focused Test Strategy

Testing should be strong but not excessive.

### 18.1 Deterministic Git-history fixture

Create a compact repository history that includes:

1. symbol introduction;
2. unchanged commit elsewhere;
3. symbol body modification;
4. co-changed test/source file;
5. file rename with symbol preserved;
6. rename + modification;
7. symbol removal or an ambiguity boundary;
8. merge commit demonstrating first-parent semantics;
9. unrelated dirty working-tree change after commits.

Use this fixture across both `history` and `evolution` tests where practical.

### 18.2 Symbol history tests

Verify:

- exact qualified lookup;
- local-name fallback;
- file disambiguation;
- ambiguity rejection;
- introduction detection;
- modification detection;
- explicit Git rename continuity;
- no false event for unchanged lineage file;
- comparison unavailable behavior;
- `--max-commits` truncation;
- dirty worktree independence.

### 18.3 Evolution tests

Verify:

- file target;
- directory target;
- additions/deletions/modifications;
- rename crossing target boundary;
- structural counts where available;
- deterministic co-change ranking;
- nested repository scope;
- truncation/coverage.

### 18.4 Safety/limits

Use injected small limits where possible for:

- commit count;
- commit metadata bytes;
- aggregate diff/path bytes;
- aggregate historical source bytes;
- event/co-change retention;
- output bounds.

### 18.5 Regression

Run all existing tests and targeted checks proving:

- `diff` / `impact` still use immutable Git objects correctly;
- current-state cache bytes/generation are not modified by history/evolution;
- scan/index/watch/query/context behavior is unchanged;
- machine-output versioning and escaping remain intact.

Avoid dozens of redundant end-to-end cases when a smaller fixture plus focused unit tests proves the invariant.

---

## 19. Documentation Updates Required by Implementation

At implementation completion:

### `docs/reports/phase-8-implementation-report.md`

Record:

- actual baseline/commit;
- architecture/refactor choices;
- exact history policy;
- identity/lineage semantics;
- limits;
- verification commands/results;
- real-repository observations;
- deviations from this plan;
- known limitations;
- explicit unverified areas.

### `docs/current-state.md`

Update only with verified shipped behavior. State that Phase 8 is locally verified / pending independent review as appropriate; do not claim independent acceptance.

### `docs/architecture.md`

Update the Git/history boundary if Phase 8 creates a durable new seam. Replace stale “future” wording with accurate current architecture without rewriting unrelated sections.

### `README.md`

Add user-facing `history` / `evolution` examples and limitations only after they actually work.

### `docs/prd.md`

Do not rewrite the roadmap simply to mirror implementation details. Update only if a durable product/phase correction is genuinely required.

Also sweep canonical docs for statements made stale by Phase 8 and correct them in the proper source-of-truth document rather than adding another status/handoff file.

---

## 20. Suggested Implementation Sequence

### Step 1 — Reinspect and protect the baseline

- read `AGENTS.md`;
- read `docs/current-state.md`;
- read this plan;
- inspect Phase 7 reports/review fixes;
- inspect `git_revision.rs`, `change.rs`, CLI/main/output/tests;
- run baseline verifier;
- preserve user worktree changes.

### Step 2 — Extract the minimal reusable revision-analysis seam

Refactor existing Phase 4 revision snapshot logic only as needed.

Before adding new behavior, ensure existing `diff` / `impact` tests still pass.

### Step 3 — Add bounded commit enumeration

Extend the hardened Git adapter with first-parent history enumeration and typed/bounded commit metadata.

Add parsing/limit unit tests before higher-level history logic.

### Step 4 — Implement symbol history domain logic

- resolve committed HEAD symbol;
- walk backward;
- skip unaffected commits;
- compare relevant file pairs;
- follow explicit rename evidence;
- surface ambiguity/incompleteness;
- collect co-change evidence.

### Step 5 — Implement path evolution

Reuse the same commit/diff infrastructure.

Keep structural analysis selective and bounded.

### Step 6 — Add renderers/CLI wiring

Wire production paths end-to-end:

```text
Clap -> application function -> domain result -> renderer -> stdout
```

No synthetic-only entrypoint.

### Step 7 — Add focused fixtures and regressions

Prioritize lineage correctness, merge policy, limits, dirty-worktree independence, nested scopes, output determinism, and no current-cache mutation.

### Step 8 — Real-repository smokes

Exercise both commands on:

- this Rust repository;
- at least one additional Git repository with enough history to demonstrate useful output.

Do not claim cross-platform verification unless actually run.

### Step 9 — Docs/report and canonical verification

Run:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
git diff --check
```

Record actual results. Then update implementation report/current state/architecture/README as justified.

---

## 21. Expected Code Touches

Likely, not mandatory:

```text
src/cli.rs
src/main.rs
src/git_revision.rs
src/change.rs
src/history.rs              # likely new application/domain module
src/output_history.rs       # or output/history.rs, whichever fits existing layout
tests/...                   # history/evolution integration fixture coverage
docs/reports/phase-8-implementation-report.md
docs/current-state.md
docs/architecture.md
README.md
```

The implementation may choose different module names if the actual repository structure suggests a cleaner fit.

Do not create a new crate, service, storage engine, or framework merely to match this list.

---

## 22. Explicit Non-Goals

Phase 8 does **not** require:

- arbitrary Git DAG lineage;
- branch-by-branch history UI;
- deleted-symbol lookup with `--at`;
- blame/author ownership;
- semantic commit-message analysis;
- stable global symbol IDs;
- compiler-grade rename detection;
- cross-file symbol-move inference unless conservative and clearly heuristic;
- historical PageRank/centrality;
- historical persistence/index compaction;
- full-text search;
- embeddings;
- LLM summaries;
- MCP/HTTP/IPC;
- Tokio;
- cross-repository contracts/history;
- Context Integrity drift/policy decisions.

If an implementation adds one of these, it must justify why it is necessary for Phase 8 correctness rather than convenience.

---

## 23. Risk Register

### Risk: false symbol lineage

Mitigation:

- exact descriptor matching;
- explicit Git rename evidence;
- uniqueness requirements;
- stop/report ambiguity rather than guess.

### Risk: history work multiplies existing per-range limits

Mitigation:

- add history-wide aggregate accounting;
- bounded commit walk;
- skip unaffected commits;
- selective blob analysis.

### Risk: merges create misleading chronology

Mitigation:

- explicit first-parent policy;
- compare merge commit to first parent;
- report policy in machine/human output.

### Risk: dirty worktree leaks into history

Mitigation:

- resolve/analyze immutable Git objects only;
- tests with contradictory dirty content.

### Risk: duplicated Phase 4 snapshot logic

Mitigation:

- extract one reusable revision-analysis seam;
- regression-test `diff` / `impact`.

### Risk: current cache becomes accidental temporal store

Mitigation:

- no historical writes to SQLite;
- no schema bump for history alone;
- test cache immutability.

### Risk: co-change is overinterpreted

Mitigation:

- name it co-change;
- define it as same-commit occurrence;
- do not emit dependency semantics.

### Risk: path evolution mishandles historical/deleted targets

Mitigation:

- lexical Git-relative target validation;
- do not require current filesystem existence;
- explicit rename/coverage semantics.

---

## 24. Phase 8 Exit Criteria

Phase 8 is implementation-complete when:

- both `history` and `evolution` are wired to real CLI production paths;
- symbol lineage is bounded, deterministic, rename-aware where evidence is explicit, and honest about ambiguity;
- path evolution and co-change evidence are useful and explainable;
- merge/first-parent semantics are explicit;
- aggregate history limits are enforced;
- dirty worktree cannot alter committed history results;
- no historical snapshot is written into the current-state cache;
- existing Phase 0–7 behavior remains green;
- canonical verifier and additional required local checks pass;
- implementation report and living docs are truthful;
- a fresh independent review can reconstruct the implementation without relying on this plan as an oracle.

Independent review/acceptance is a separate step.
