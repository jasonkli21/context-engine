# Context Engine — Phase 4 Implementation Plan

**Phase:** 4 — Git and Structural Change Intelligence
**Status:** Planned; implementation gated on final Phase 3 acceptance or explicit maintainer authorization
**Date:** 2026-10-15
**Repository:** `jasonkli21/context-engine`
**Planning baseline inspected:** `865f5a4ddb103529966a2310f1607d636d8afd15` (`fix: address phase 3 review findings`)
**Primary language:** Rust
**Binary:** `ctx`
**Primary new capabilities:** committed-revision structural diffing and evidence-backed change impact
**Explicitly deferred:** task-context ranking, persistence, incremental indexing, async/concurrency, MCP/API, embeddings/model calls, history/blame, compiler-grade resolution

---

# 1. Planning Baseline and Gate

This plan is based on the actual `main` state inspected on 2026-10-15.

Latest inspected commit:

```text
865f5a4ddb103529966a2310f1607d636d8afd15
fix: address phase 3 review findings
```

The Phase 3 implementation commit immediately before the review-fix pass is:

```text
ff9ad64bd290f71c3c1b6536f0907ca32dfc351b
feat: implement phase 3 query and repository intelligence
```

Current repository state includes:

- bounded Phase 0 repository discovery and Git metadata probing;
- Rust/Python Tree-sitter symbol extraction;
- bounded source loading and repository-wide analysis budgets;
- a fresh synchronous in-memory repository graph;
- lexical containment, unresolved imports, and conservative call-site reference facts;
- exact same-file qualified-name reference resolution only;
- deterministic `graph`, `deps`, `callers`, `related`;
- deterministic `search`, `neighborhood`, and `hotspots`;
- explicit analysis coverage/incompleteness in Phase 3 query output;
- normalized external path semantics and explicit non-UTF-8 omissions;
- index-local repository/file/symbol IDs that may renumber across edits;
- no persistence, async, threads, daemon/API, MCP, embeddings, model calls, or policy semantics.

`docs/current-state.md` records the Phase 3 review findings as fixed and locally verified, but the fresh independent re-review is still pending.

Therefore:

> **Creating this plan does not itself authorize Phase 4 implementation and does not convert local verification into independent acceptance.**

Implementation may begin after either:

1. final Phase 3 independent acceptance/closure; or
2. explicit maintainer authorization to proceed despite the pending disposition.

A future direct maintainer instruction to execute the Phase 4 implementation prompt is sufficient authorization to start, but implementation/reporting must preserve the distinction between authorization and independent acceptance.

---

# 2. Alignment With Durable Intent

Phase 4 is the first major change-intelligence milestone in both the PRD and architecture.

The PRD roadmap defines Phase 4 as:

```text
Git and Structural Change Intelligence
```

with:

```bash
ctx diff main..HEAD
ctx impact main..HEAD
```

and expected reporting of:

- changed symbols;
- new/removed relations;
- likely affected dependents;
- test/doc candidates.

The architecture describes the intended flow:

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

The durable project boundary remains:

```text
Context Engine
"What is observably true?"

          ↓ structured evidence

Context Integrity
"Does that evidence align with intended/authoritative state?"
```

Phase 4 therefore adds **revision-aware descriptive evidence**, not policy judgments.

It must preserve the existing principles:

- deterministic before probabilistic;
- local-first and read-only;
- explicit provenance;
- uncertainty represented honestly;
- synchronous/process-local execution;
- no mandatory network;
- no analyzed-code execution;
- no false promise of stable entity lineage across edits.

---

# 3. Phase 4 Objective

Phase 3 answers:

> What exists in the current repository, and how is it structurally connected?

Phase 4 should answer:

> What observably changed between two committed Git revisions, and which current/prior structural neighbors are plausible impact candidates?

Conceptually:

```text
Git range
  │
  ├── resolve immutable base/head commits
  ├── enumerate changed paths/blob IDs
  └── read bounded Git blobs
          │
          ▼
base/head structural analysis
          │
          ├── file changes
          ├── symbol changes
          └── relation occurrence deltas
          │
          ▼
StructuralChangeSet
          │
          ├── ctx diff
          │
          └── impact seeds
                  │
        base/head revision graphs
                  │
                  ▼
          bounded impact traversal
                  │
                  ▼
             ctx impact
```

Phase 4 should be useful without Phase 5 task ranking.

---

# 4. Product Boundary

## 4.1 In scope

Phase 4 implements committed-revision change intelligence for a local Git repository:

1. parse and resolve an explicit two-dot commit range;
2. enumerate changed files safely and deterministically;
3. preserve Git file status/mode/blob provenance;
4. analyze old/new changed-file blobs using the existing Rust/Python analyzers;
5. identify added/removed/modified symbols conservatively;
6. identify added/removed relation occurrences using stable structural comparison keys;
7. handle Git-detected file renames without pretending to infer arbitrary symbol lineage;
8. construct immutable base/head repository graphs from Git snapshots for `impact`;
9. traverse existing structural/reference evidence to produce likely impact candidates;
10. surface directly changed test/doc files and structurally reached test candidates;
11. expose explicit revision identity, coverage, ambiguity, and truncation;
12. preserve schema-v1 versioned machine output and the shared bounded serializer;
13. keep all work local, synchronous, read-only, and LLM-free.

## 4.2 Explicitly not in scope

Phase 4 must not add:

- working-tree/staged/untracked diff modes;
- `HEAD~1` implicit defaults when the user did not provide a range;
- triple-dot / merge-base semantics;
- merge history interpretation;
- `git blame`;
- long-term history/evolution queries;
- rename lineage for symbols beyond a Git-evidenced file rename pair;
- compiler/LSP-grade binding;
- new cross-file reference resolution;
- import resolution;
- type inference or dispatch;
- semantic breaking-change classification;
- API compatibility judgments;
- architecture-policy/drift judgments;
- task-specific ranking or `ctx context`;
- persistent indexes;
- filesystem watchers;
- async/Tokio;
- worker pools/Rayon;
- daemon/API/MCP;
- embeddings;
- model calls;
- cross-repository impact;
- a graph database;
- a multi-crate workspace.

Phase 4 must not silently turn unresolved syntax facts into "affected dependents."

---

# 5. Key Design Decisions

## 5.1 Analyze committed snapshots, not the dirty working tree

`ctx diff A..B` and `ctx impact A..B` must describe commits `A` and `B`, even if the checkout is dirty.

Do **not** implement Phase 4 by:

- checking out the base revision;
- modifying the worktree;
- creating a Git worktree;
- reading the current filesystem and assuming it equals `HEAD`;
- mixing uncommitted source with committed revision evidence.

Resolve both endpoints to immutable commit SHAs and read repository data from Git objects.

This gives Phase 4 stronger snapshot semantics than the live filesystem commands.

## 5.2 Reuse analyzers and graph construction; do not create a second parser model

Historical Git blobs must flow through the same:

- language classification;
- file-role classification;
- `AnalyzerRegistry`;
- symbol model;
- relation candidate extraction;
- repository analysis limits;
- graph construction semantics

used by current-state indexing.

A small refactor is expected so `index.rs` can analyze a bounded `RepositoryScan`-like inventory using a supplied source loader instead of always calling `source::load`.

Prefer an internal function/closure boundary over a new trait hierarchy.

Illustrative shape:

```rust
fn analyze_scan_with_loader<F>(
    scan: RepositoryScan,
    max_parse_file_bytes: u64,
    graph: bool,
    loader: F,
) -> Result<(RepositoryIndex, Vec<FileRelations>), IndexError>
where
    F: FnMut(&FileRecord) -> Result<String, AnalysisStatus>;
```

The exact signature may differ.

Existing filesystem indexing should call the same function with `source::load`.

Git snapshot indexing should call it with a bounded blob loader.

## 5.3 Do not compare index-local IDs across revisions

Current `EntityId`/`RelationId` values are deterministic only within one constructed graph.

Phase 4 must never infer:

```text
base EntityId(42) == head EntityId(42)
```

Instead, compare revision-scoped structural descriptors.

IDs may still appear inside one revision's traversal evidence, but output must label them as revision/index-local.

## 5.4 Structural matching is conservative, not lineage inference

Initial symbol comparison key within a paired file:

```text
(kind, qualified_name)
```

File identity comes from the Git file-change pair.

Rules:

- match only when the key is unique on both sides;
- do not guess when duplicate keys make matching ambiguous;
- ambiguous keys must be explicitly reported/qualified;
- range changes do not by themselves create a different matched symbol;
- exact source slice comparison determines whether a uniquely matched symbol's declaration/body changed;
- a rename of the containing file may pair the old/new file using Git rename evidence, but does not establish general symbol lineage;
- a symbol rename remains removed + added unless future lineage work explicitly supports more.

This intentionally does not promise identity through arbitrary refactors.

## 5.5 Relations compare as occurrence multisets, not relation IDs/ranges

A relation can move in source without changing structurally.

Compare a canonical relation signature that excludes volatile occurrence location:

```text
source structural descriptor
relation kind
target descriptor or unresolved target text
resolution class
metadata
derivation
```

Track occurrence counts:

```text
before_count
after_count
```

A delta exists when counts differ.

Retain bounded example evidence ranges from the relevant side for inspection.

This prevents harmless line shifts from becoming remove/add churn while preserving repeated calls/imports.

Repository→file containment should remain represented by file changes rather than duplicated as structural relation deltas.

## 5.6 Pure Git file renames should not create artificial structural churn

Use fixed, explicit Git rename detection for file pairing.

Recommended:

```text
--find-renames=50%
```

A pure rename with unchanged blob content should report the file rename, but should not manufacture every unchanged symbol/relation as removed+added.

When comparing a Git-evidenced rename pair:

- use old/new path in evidence;
- compare unique symbol keys within the paired file;
- normalize file-local endpoints through the paired file for relation comparison;
- do not extend this into arbitrary symbol-rename inference.

The chosen rename threshold must be fixed and reported or documented.

## 5.7 `ctx diff` and `ctx impact` may have different analysis cost

`ctx diff` should be diff-first:

- inspect the Git file-change set;
- load/analyze only changed old/new blobs;
- produce the structural change set.

It should **not** build two whole-repository graphs merely to list changes.

When converting Git-root paths to a directory scope, accept strict descendants
only. A historical file, symlink, or gitlink exactly at the scope prefix is not
a scope-relative file and must not become an empty path identity.

`ctx impact` may build bounded full base/head revision graphs because dependents can live outside changed files.

This keeps the common structural-diff path smaller.

---

# 6. Proposed CLI Surface

Add:

```text
ctx diff <BASE>..<HEAD> [PATH]
ctx impact <BASE>..<HEAD> [PATH]
```

Existing commands remain unchanged.

## 6.1 Shared options

```text
[PATH]                      repository/directory scope, default .
--format human|json         default human
--max-parse-file-size N     existing default 1 MiB
```

## 6.2 `ctx diff`

Suggested shape:

```bash
ctx diff main..HEAD .
ctx diff v0.1.0..HEAD /path/to/repo --format json
```

No implicit range.

No working-tree mode.

No triple-dot mode in Phase 4.

## 6.3 `ctx impact`

Suggested options:

```text
--depth N                   default 1, maximum existing graph max (8)
```

Example:

```bash
ctx impact main..HEAD . --depth 2
```

Impact depth is graph traversal depth under the existing syntactic evidence model, not predicted runtime propagation.

Avoid introducing an opaque impact score.

---

# 7. Range Semantics

## 7.1 Input grammar

Phase 4 accepts exactly one explicit two-dot range:

```text
BASE..HEAD
```

Reject:

- empty base;
- empty head;
- `...`;
- multiple `..` separators;
- NUL/control-containing revision text;
- revision tokens beginning with `-` unless safely handled by a guaranteed end-of-options strategy;
- excessively long revision strings.

Recommended maximum input length:

```text
MAX_REVISION_TEXT_BYTES = 4096
```

## 7.2 Resolution

Resolve endpoints separately to commit objects:

```text
BASE -> full commit SHA
HEAD -> full commit SHA
```

Subsequent large-output Git commands should use the resolved SHAs, not raw user revision tokens.

Output must include:

- original range text;
- resolved base commit SHA;
- resolved head commit SHA.

If both resolve to the same commit, the command succeeds with an empty change set.

---

# 8. Git Adapter Evolution

`src/git.rs` currently owns small read-only metadata probes.

Phase 4 justifies expanding that boundary, but not replacing it with a service/framework.

Recommended responsibilities:

```text
git.rs
├── existing metadata probe
├── repository/scope discovery
├── revision-range resolution
├── raw diff enumeration
├── revision tree inventory
└── bounded blob loading
```

If the file becomes unwieldy, split a narrow internal submodule, but avoid a multi-crate redesign.

## 8.1 Read-only commands

A likely implementation can use:

```text
git rev-parse --show-toplevel
git rev-parse --verify <revision>^{commit}
git diff-tree -r --raw -z --no-commit-id --no-abbrev --find-renames=50% ...
git ls-tree -r -z -l <commit> -- <scope>
git cat-file -s <blob>
git cat-file blob <blob>
```

Exact commands may vary.

Large diff commands must disable external presentation hooks where applicable:

```text
--no-ext-diff
--no-textconv
```

Continue:

- `--no-optional-locks`;
- `core.fsmonitor=false`;
- `LC_ALL=C`;
- `GIT_TERMINAL_PROMPT=0`;
- removing ambient Git redirection variables as the existing adapter does.

Do not invoke shell interpolation.

## 8.2 Large-output subprocesses must be bounded

Do not use an unbounded `Command::output()` for:

- whole diff enumeration;
- full tree listing;
- blob contents.

Read large stdout with an explicit byte ceiling.

If the ceiling is exceeded:

- terminate/fail the operation;
- emit no partial structural result;
- return a typed, clear error.

Small metadata commands may retain a simpler path when their successful output is inherently tiny.

Do not introduce threads merely to capture arbitrarily large Git stderr. For repository-sized streaming commands, a generic typed Git failure is preferable to unbounded stderr capture.

## 8.3 Blob reads

Use object IDs returned by Git enumeration.

Before reading a blob:

1. determine/validate object type/mode;
2. obtain size;
3. skip structural parsing when size exceeds `--max-parse-file-size`;
4. stream at most the allowed bytes;
5. require UTF-8 for existing analyzers.

Reading raw Git blobs avoids smudge/clean/process filters and keeps committed evidence immutable.

---

# 9. Repository Scope and Paths

Existing commands accept a scan root that may be nested below the Git root.

Phase 4 should preserve that model.

For `[PATH]`:

1. validate/canonicalize the requested existing directory;
2. discover enclosing Git root;
3. require the requested root to be inside that Git repository;
4. compute its Git-root-relative prefix;
5. constrain diff/tree enumeration to that prefix;
6. emit file paths relative to the requested Phase 4 scope, not the Git root.

Example:

```bash
ctx diff main..HEAD ./backend
```

should not report unrelated `frontend/` changes.

Path output must keep the established normalized `/` representation.

Non-UTF-8 Git paths:

- remain distinguishable internally where the platform supports them;
- must not be converted into false exact identities using lossy strings;
- may be reported lossily at presentation boundaries with an explicit coverage flag;
- are not structurally analyzed if current classifier/analyzer path handling cannot represent them safely.

Do not silently drop them from file-change counts.

---

# 10. Git File-Change Model

Recommended internal types are illustrative, not mandatory:

```rust
enum FileChangeKind {
    Added,
    Deleted,
    Modified,
    Renamed,
    ModeChanged,
}

struct RevisionFile {
    path: PathBuf,
    mode: GitMode,
    blob_id: Option<String>,
    size_bytes: Option<u64>,
}

struct FileChange {
    kind: FileChangeKind,
    before: Option<RevisionFile>,
    after: Option<RevisionFile>,
    rename_similarity: Option<u8>,
    structural_status: StructuralAnalysisStatus,
}
```

Handle at least:

- added regular file;
- deleted regular file;
- modified regular file;
- Git-detected rename;
- file mode change;
- regular↔symlink transition;
- gitlink/submodule entry.

Only ordinary blob-backed files eligible under the existing analyzer policy should receive structural symbol/relation analysis.

Symlinks and gitlinks are reported at file level and are not executed/followed.

---

# 11. Revision Snapshot Inventory for `impact`

`ctx impact` needs repository-wide evidence at base/head.

Build a synthetic immutable inventory from the Git tree:

```text
resolved commit
    ↓
bounded ls-tree inventory
    ↓
scan-root-relative FileRecord values
    ↓
existing role/language classification
    ↓
shared repository analyzer with Git blob loader
    ↓
RepositoryIndex + relation extraction
    ↓
existing graph construction
```

Important differences from live filesystem scanning:

- only committed tracked entries exist;
- no `.gitignore` traversal is necessary to discover committed files;
- no symlink following occurs;
- generated/vendor role skipping remains identical;
- file sizes come from Git object metadata;
- source bytes come from blobs, not checkout files;
- `GitMetadata.head_commit` for the synthetic snapshot should identify the analyzed commit;
- dirty-state metadata is not applicable to an immutable snapshot.

The synthetic snapshot must preserve current repository analysis ceilings.

Add a Phase 4-specific aggregate Git blob-read ceiling so a historical graph cannot force tens of gigabytes of object reads merely because every file is individually under 1 MiB.

Recommended initial ceiling:

```text
MAX_GIT_SOURCE_BYTES_PER_REVISION = 256 MiB
```

Treat overflow as a fatal incomplete revision graph, not a partial authoritative graph.

---

# 12. Structural File Analysis

For each old/new changed file side:

1. classify language and role using existing functions;
2. reject generated/vendor structural analysis as current indexing does;
3. enforce blob size limit;
4. require UTF-8;
5. run the same analyzer used by filesystem indexing;
6. retain:
   - symbols;
   - diagnostics/syntax status;
   - relation extraction;
   - source text only as long as needed to compare symbol slices;
7. discard source text before returning the final public result.

Do not emit source bodies in Phase 4 JSON.

## 12.1 Per-file fail-closed semantics

If an existing side should be analyzable but cannot be analyzed because of:

- invalid UTF-8;
- parse failure;
- analysis limit;
- relation extraction limit;
- unsafe/unavailable Git object;
- unsupported object mode,

do not invent structural deltas from the other side.

Report:

```text
file change: known
symbol/relation structural diff: incomplete/unavailable
reason: explicit
```

File-level Git evidence remains valid.

Unsupported languages are "structural analysis unavailable," not fatal repository errors.

---

# 13. Symbol Change Semantics

## 13.1 Matching key

Within one Git file-change pair, use:

```text
SymbolMatchKey {
    kind,
    qualified_name,
}
```

Do not use:

- `EntityId`;
- absolute byte range;
- row/column range;
- local name alone.

## 13.2 Unique matching only

For each key:

- one base + one head => eligible match;
- base only => removed;
- head only => added;
- duplicates on either side => ambiguous; do not guess one-to-one identity.

Ambiguity should be explicit in coverage/output.

## 13.3 Modified symbol detection

For a uniquely matched symbol:

- validate its source range against the side's UTF-8 text;
- compare the exact symbol source slices;
- if bytes differ, report `modified`;
- if identical, omit it from changed-symbol output.

This detects:

- body changes;
- parameter/signature text changes;
- visibility/modifier changes when included in the declaration range;
- nested content changes that fall inside the symbol's syntax range.

This is textual structural modification, not semantic compatibility analysis.

## 13.4 Added/deleted files

For a successfully analyzed added file, all symbols are added.

For a successfully analyzed deleted file, all symbols are removed.

## 13.5 File rename

Git rename evidence pairs the files.

A pure rename with identical blob content should not mark every contained symbol modified.

Do not infer a renamed symbol from similarity of two differently named declarations.

---

# 14. Relation Change Semantics

Relations in different revision graphs have different IDs and source ranges.

Define a revision-independent comparison signature over one file-change pair.

Illustrative:

```rust
struct RelationSignature {
    source: StableEndpoint,
    kind: RelationKind,
    target: StableTarget,
    resolution: Resolution,
    metadata: Metadata,
    derivation: Derivation,
}
```

`StableEndpoint` / `StableTarget` should use:

- file-local paired identity;
- unique matched symbol descriptor;
- unresolved target text where resolution is unresolved.

Exclude:

- relation IDs;
- absolute occurrence range;
- repository-local numeric entity IDs.

## 14.1 Occurrence counts

Build a multiset:

```text
signature -> occurrence count
```

Compare base/head counts.

Emit a relation delta only when counts differ:

```text
before_count
after_count
delta
```

Retain one or a small bounded number of evidence examples from the side responsible for the delta.

This preserves repeated call/import facts.

## 14.2 Containment

Include lexical file→symbol and symbol→symbol containment when structural analysis is complete.

Do not emit repository→file containment deltas; the file-change list already carries that information.

## 14.3 Resolution honesty

Do not upgrade relation quality in Phase 4.

Current semantics remain:

- imports unresolved;
- references resolve only to a unique exact same-file qualified name;
- bare method/nested calls may remain unresolved;
- cross-file calls remain unresolved.

Relation deltas must preserve:

- `resolution`;
- `derivation`;
- metadata.

---

# 15. `StructuralChangeSet`

Recommended conceptual model:

```text
StructuralChangeSet
- repository scope
- input range
- base revision
- head revision
- file changes
- symbol changes
- relation deltas
- coverage
- ambiguity
- truncation / safety status
```

This becomes the reusable Phase 4 domain object consumed by both output and impact analysis.

It must not depend on Clap or Serde.

Use explicit output DTOs rather than serializing domain structs directly.

---

# 16. Impact Analysis Semantics

`ctx impact` is evidence-backed traversal, not failure prediction.

Phrase output as:

- "impact candidates";
- "syntactic dependents";
- "reached test candidates";
- "changed documentation candidates";

not:

- "will break";
- "affected runtime behavior";
- "test coverage";
- "must update."

## 16.1 Impact seeds

Seeds come from the structural change set.

At minimum:

- added symbols;
- removed symbols;
- modified symbols;
- changed files whose structural symbol diff is unavailable.

Map:

- removed symbols to the base graph;
- added symbols to the head graph;
- modified symbols to both base and head graphs when uniquely resolvable;
- file-only changes to the corresponding base/head file entity.

Use exact structural descriptors, not name-only search.

## 16.2 Traversal

For symbol seeds, the primary dependent traversal should reuse the graph's resolved incoming call/reference evidence.

For file seeds or broader local context, a bounded neighborhood may be used if it preserves the Phase 3 repository-hub exclusion.

Phase 4 should add an internal graph API that can traverse from already-resolved `EntityId` roots, avoiding a second ambiguous name lookup.

Reuse existing:

- iterative traversal;
- cycle handling;
- depth maximum;
- root accounting;
- visited/relation budgets;
- predecessor evidence;
- deterministic ordering.

Impact traversal budgets must be shared across all change seeds for one revision side, not reset independently per seed.

When both revisions contain impact seeds, allocate root, visit, and relation
budgets deterministically across base and head before traversal. A changed-symbol
seed has priority over a changed-file fallback; use file neighborhoods when no
confident changed-symbol seed is available for that side. Output identifies the
seed kind and traversal policy. Rendering must check a conservative materialized
size bound before allocating DTOs or human output, while retaining the final
bounded JSON writer.

## 16.3 Impact result

A practical result can remain grouped by seed:

```text
changed seed
  ├── base-side dependent evidence
  └── head-side dependent evidence
```

Each visited entity should carry:

- side/revision;
- revision-local entity ID if useful;
- file/symbol descriptor;
- distance;
- predecessor/relation evidence;
- traversal truncation.

Do not join base/head entities merely because their numeric IDs match.

## 16.4 Test candidates

Surface:

1. directly changed files classified as `Test`;
2. test-role entities actually reached by allowed impact/neighborhood traversal.

Preserve Phase 3 language:

```text
candidate test association
```

not coverage.

Because cross-file resolution remains intentionally weak, test candidates may often be sparse. Sparse truthful output is preferred over filename/directory heuristics disguised as evidence.

## 16.5 Documentation candidates

Phase 4 does not yet have code↔documentation relation extraction.

Therefore surface only:

- directly changed files classified as `Documentation`;
- any future/current explicit graph evidence if it genuinely exists.

Do not invent doc associations from name similarity.

## 16.6 Removed entities

Removed symbols exist only in the base graph.

Use the base graph to find prior syntactic dependents.

This is one reason `impact` needs both revision snapshots.

---

# 17. Graph/Internal API Adjustments

Phase 4 may justify small internal APIs such as:

```text
build graph from already analyzed revision inventory
resolve exact structural symbol descriptor -> EntityId
traverse from explicit root IDs
```

Do not:

- expose graph internals publicly merely for Phase 4;
- make `EntityId` cross-revision stable;
- duplicate BFS in `change.rs`;
- duplicate graph construction in `git.rs`.

Address the Phase 3 future consideration about duplicated index→entity layout only if Phase 4 actually touches that mapping.

If a small authoritative helper removes duplicate ID-layout formulas, factor it now.

Do not perform an unrelated graph refactor.

---

# 18. Suggested Module Layout

Likely additions:

```text
src/
├── change.rs          # structural change + impact domain/application logic
├── output_change.rs   # Phase 4 human/JSON adapters
```

Likely modifications:

```text
src/git.rs             # revision/diff/tree/blob adapter
src/index.rs           # shared analysis over pluggable internal loader
src/graph.rs           # construct/traverse from explicit roots
src/cli.rs             # diff/impact arguments
src/main.rs            # dispatch/orchestration
src/output.rs          # shared DTO/helpers only if genuinely shared
src/model.rs           # only durable/shared types that belong here
README.md
docs/current-state.md
```

Tests:

```text
tests/change.rs
```

Keep Phase 4 concerns out of `query.rs` unless a truly shared traversal primitive belongs there.

---

# 19. Resource and Safety Limits

Phase 4 expands the trust boundary to Git object data.

It must add explicit limits rather than assume Git output is small.

Recommended initial production ceilings:

```text
revision input text                 4 KiB
changed file records               10,000
changed-path bytes                  4 MiB
raw Git diff output                 bounded (e.g. 16 MiB)
revision inventory entries          50,000
revision inventory path bytes       4 MiB
Git source bytes per revision       256 MiB
per-file parse bytes                existing default/configurable limit
repository symbols/names/details    existing Phase 1 limits
graph entities/relations/text       existing Phase 2 limits
impact roots                        existing 128-root ceiling, fairly split
impact visited entities             existing 10,000 shared ceiling, fairly split
impact relation occurrences         existing 25,000 shared ceiling, fairly split
human/JSON materialized output       existing 64 MiB ceiling
```

Exact constants may be adjusted if implementation evidence justifies a better value.

Requirements:

- exact-boundary and over-boundary tests use injected small limits;
- do not create huge physical fixtures merely to hit limits;
- fatal Git/revision/index/graph/output overflow emits no partial JSON;
- JSON and human output check a conservative materialization estimate before
  building owned DTO collections or rendered strings;
- query/traversal truncation remains explicit where omission is a normal bounded-query behavior;
- structural analysis omission for an individual changed file is represented in coverage rather than silently discarded.

---

# 20. Security and Trust Boundary

Phase 4 remains read-only.

It may:

- inspect Git repository metadata;
- resolve revisions;
- read Git trees/blobs;
- parse committed source;
- build in-memory structural models.

It must not:

- check out revisions;
- reset/stash/modify the worktree;
- write to `.git`;
- run project code;
- run package managers;
- import analyzed modules;
- execute Git external diff helpers;
- invoke textconv;
- invoke source filters to obtain committed contents;
- use the shell to interpolate revision/path text;
- upload source;
- make network calls.

Treat repository content, Git paths, refs, and commit data as untrusted input.

Human output must continue escaping control characters.

---

# 21. Output Contracts

Add explicit schema-v1 envelopes:

```json
{
  "schema_version": 1,
  "command": "diff",
  "data": {}
}
```

```json
{
  "schema_version": 1,
  "command": "impact",
  "data": {}
}
```

Do not change existing Phase 0–3 JSON shapes.

## 21.1 Required revision context

Every Phase 4 result must identify:

- requested scope root;
- Git repository root in presentation-safe form;
- original range text;
- resolved base commit;
- resolved head commit;
- two-dot committed-revision semantics;
- rename-detection behavior.

## 21.2 Coverage

Include enough context to interpret sparse/negative results:

- total file changes;
- structurally analyzed file sides;
- unsupported/skipped/too-large/invalid/failed counts;
- symbol-match ambiguity count;
- relation-analysis incomplete count;
- lossy/non-UTF-8 path count;
- base/head revision graph completeness for impact;
- root/traversal truncation;
- omitted detail counts when details are intentionally summarized.

A zero structural delta with incomplete analysis must not look like proof that "nothing structural changed."

## 21.3 File changes

Expose:

- kind;
- before/after path;
- before/after object mode;
- before/after blob ID when applicable;
- rename similarity when applicable;
- structural analysis status.

## 21.4 Symbol changes

Expose:

- added/removed/modified;
- before and/or after symbol descriptor;
- revision-scoped source range;
- file path;
- ambiguity only when necessary.

Do not expose a fake stable symbol ID.

## 21.5 Relation deltas

Expose:

- source descriptor;
- relation kind;
- target descriptor/unresolved text;
- resolution;
- metadata;
- derivation;
- before/after occurrence counts;
- bounded example evidence.

## 21.6 Impact

Expose:

- seed change;
- revision side;
- candidate descriptor;
- graph distance;
- predecessor/relation evidence;
- candidate test/doc classifications;
- truncation.

All JSON must use the existing bounded writer.

---

# 22. Error Semantics

Fatal Phase 4 errors include:

- target path is not a valid directory;
- no enclosing Git repository;
- invalid range grammar;
- base/head cannot resolve to commits;
- unsupported Git output shape;
- Git diff/tree output exceeds safety limit;
- revision inventory exceeds entry/path limits;
- aggregate Git blob read budget exceeded;
- repository-wide index/graph limit exceeded;
- serialization exceeds output ceiling.

Recoverable per-file structural statuses include:

- unsupported language;
- generated/vendor role;
- too large;
- invalid UTF-8;
- parse failure;
- analysis limit;
- relation extraction limit;
- non-regular Git mode.

A recoverable per-file status does not erase the Git file change.

Do not silently convert fatal incomplete revision graphs into partial impact output.

---

# 23. Determinism

For identical repository objects and CLI options:

- resolved commit SHAs are fixed;
- file changes sort canonically;
- symbol changes sort by path/kind/qualified name and side;
- relation signatures use explicit ordered fields;
- relation occurrence counts are deterministic;
- impact roots and visits use existing canonical graph order;
- sets/maps used for output should not leak randomized iteration order;
- repeated JSON runs should be byte-identical.

Git rename detection must use an explicit fixed threshold rather than user/global configuration.

---

# 24. Implementation Sequence

## Step 0 — Reconfirm the gate and baseline

Before editing:

- read `AGENTS.md`;
- read `docs/current-state.md`;
- read this plan;
- inspect latest commit/code/tests;
- read Phase 3 implementation/review/fix evidence;
- inspect architecture/PRD change-intelligence sections;
- verify whether Phase 3 has since received an independent disposition;
- reconcile any newer repository state.

Do not blindly force this plan onto a materially changed repository.

## Step 1 — Add revision/range and Git-object primitives

Implement and unit-test:

- two-dot range parsing;
- commit resolution;
- repository/scope mapping;
- bounded raw diff parsing;
- Git mode/status modeling;
- rename pairing;
- bounded tree inventory;
- bounded blob-size/content reads.

No public `diff` command yet.

## Step 2 — Reuse index analysis for Git snapshots

Refactor the current index analysis minimally so both:

```text
filesystem FileRecord + source::load
Git FileRecord + blob loader
```

share the same analyzer/budget path.

Prove all existing index/graph tests still pass before layering change logic.

## Step 3 — Implement changed-file structural analysis

Build old/new file analysis for changed blobs.

Add:

- explicit per-side analysis status;
- source slice validation;
- unique symbol matching;
- added/removed/modified symbol output;
- ambiguous key handling.

## Step 4 — Implement relation multiset diff

Reuse extraction/resolution semantics.

Add:

- stable endpoint/signature representation;
- count-based relation comparison;
- rename normalization;
- bounded evidence examples.

At this point a `StructuralChangeSet` should exist independent of CLI/output.

## Step 5 — Wire `ctx diff`

Add:

- CLI parsing;
- main dispatch;
- human output;
- schema-v1 JSON;
- coverage context;
- output bounding.

Run focused integration tests before impact work.

## Step 6 — Build immutable revision graphs

Implement bounded base/head snapshot inventories and graph construction from Git blobs.

Do not duplicate graph construction.

Add aggregate Git source I/O limit.

## Step 7 — Implement impact traversal

Add exact seed→entity mapping and traversal from explicit entity roots.

Reuse the graph traversal core.

Produce:

- base/head dependent evidence;
- test candidates;
- directly changed docs;
- explicit truncation/incompleteness.

## Step 8 — Wire `ctx impact`

Add human/JSON output and focused CLI tests.

## Step 9 — Documentation/status

Update:

- `README.md` with shipped semantics and limitations;
- `docs/reports/phase-4-implementation-report.md`;
- `docs/current-state.md` truthfully.

Do not alter PRD/architecture unless implementation discovers a durable decision that truly changes them.

Do not claim independent acceptance.

---

# 25. Focused Test Strategy

Testing should target new correctness boundaries, not duplicate every Phase 0–3 assertion.

## 25.1 Unit tests

Prioritize:

- range parser exact valid/invalid forms;
- raw NUL-delimited diff parser;
- add/delete/modify/rename/mode statuses;
- path normalization/scope stripping;
- symbol unique/ambiguous matching;
- symbol body-only modification;
- relation multiset count delta;
- pure file rename normalization;
- small injected resource limits.

## 25.2 Local Git integration fixture

Create a temporary repository through Git CLI in the test, with deterministic author/date config if needed.

A compact fixture should cover:

Base:

```text
src/lib.rs
src/removed.rs
tests/change_test.rs
docs/guide.md
```

Head:

- modify a function body;
- change a declaration/signature;
- add a symbol;
- remove a symbol;
- add/remove a call occurrence;
- add/delete a file;
- rename one file without changing its blob;
- modify a test file;
- modify a documentation file.

Assertions should validate:

- resolved revision SHAs;
- file statuses;
- changed symbols;
- relation occurrence deltas;
- pure rename behavior;
- directly changed test/doc candidates;
- deterministic human/JSON.

## 25.3 Impact fixture

Use same-file call relationships that the existing resolver can genuinely resolve.

Verify:

- removed symbol dependents come from base graph;
- modified symbol can produce base/head evidence;
- added symbol uses head graph;
- distance and predecessor evidence;
- cycle safety;
- shared traversal bounds;
- no fake cross-file resolution.

Do not create a test that passes only because a production symbol and a test file have similar names.

## 25.4 Snapshot integrity

After creating the head commit, dirty the worktree.

`ctx diff base..head` and `ctx impact base..head` must still describe committed blobs, not the dirty checkout.

## 25.5 Scope

Run commands from a nested repository path and verify unrelated sibling changes are excluded.

## 25.6 Incomplete analysis

At least one focused case should verify:

- too-large or invalid structural source;
- file change still reported;
- structural coverage marked incomplete;
- no false symbol/relation delta inferred.

## 25.7 Regression gate

All existing Phase 0–3 tests must remain passing.

---

# 26. Verification

Canonical required gate:

```bash
./scripts/verify.sh
```

Also run when the environment supports them:

```bash
cargo check --all-targets
cargo build --release
git diff --check
```

Release smoke should include:

```bash
ctx diff <known-base>..<known-head> . --format human
ctx diff <known-base>..<known-head> . --format json
ctx impact <known-base>..<known-head> . --depth 1
ctx impact <known-base>..<known-head> . --format json
```

Use this repository's real recent commits for one smoke path when practical, in addition to deterministic temporary fixtures.

Do not claim cross-platform, CI, cloud/provider, or independent verification unless actually run.

---

# 27. Phase 4 Acceptance Criteria

Phase 4 is implementation-complete when:

1. `ctx diff BASE..HEAD [PATH]` works on committed revisions;
2. `ctx impact BASE..HEAD [PATH]` works on committed revisions;
3. dirty checkout content cannot contaminate revision results;
4. both endpoints are resolved and reported as immutable commits;
5. changed-file evidence is deterministic and bounded;
6. Git-detected pure file renames do not explode into false structural churn;
7. added/removed/modified symbols are derived conservatively without cross-revision numeric IDs;
8. body-only symbol edits are detectable;
9. duplicate/ambiguous symbol keys are not guessed into lineage;
10. relation deltas compare structural occurrence multisets, not relation IDs/source positions;
11. relation resolution/derivation quality remains explicit;
12. `impact` uses base evidence for removed entities and head evidence for added entities;
13. impact candidates remain explicitly syntactic/evidence-based;
14. test candidates are evidence-backed or directly changed;
15. doc candidates are directly changed unless genuine structural evidence exists;
16. revision/scope/coverage/truncation are self-identifying in human and JSON output;
17. large Git outputs/blob reads are bounded;
18. no checkout/worktree mutation/project-code execution/network access occurs;
19. existing Phase 0–3 command contracts remain unchanged;
20. canonical verification passes;
21. implementation report and current-state are truthful;
22. independent review is requested before final Phase 4 acceptance.

---

# 28. Review Risks to Expect

The independent reviewer should be especially skeptical of:

- accidental current-worktree reads;
- `EntityId` comparison across revisions;
- symbol matching by local name alone;
- ambiguous duplicate symbol keys;
- false "modified" symbols caused only by file rename;
- false relation churn caused only by moved source ranges;
- relation deltas that lose occurrence multiplicity;
- impact traversal that silently performs name-based cross-file guessing;
- base-only removed symbols incorrectly looked up in the head graph;
- Git option injection through raw revision text;
- external diff/textconv/filter execution;
- unbounded Git subprocess output;
- unbounded aggregate blob reads;
- path-scope mistakes for nested scan roots;
- non-UTF-8/lossy path identity confusion;
- incomplete structural analysis presented as a clean zero-delta;
- duplicated parser/index/graph pipelines;
- tests that only exercise synthetic structs instead of real Git→analysis composition;
- Phase 5 ranking or Phase 6 persistence leaking into this phase.

---

# 29. Downstream Seams

## Phase 5 — Task Context Ranking

Phase 5 should be able to use:

- `StructuralChangeSet`;
- changed entities;
- impact candidates;
- change proximity;
- current structural centrality

as deterministic ranking signals.

Phase 4 should not itself add task text, scoring, token budgets, or context bundles.

## Phase 6 — Persistence

Phase 4 revision/change domain types should remain serializable through future adapters, but must not be shaped around SQLite today.

## Historical intelligence

Future history/evolution work can reuse:

- revision identity;
- Git object adapter;
- structural change model.

Do not add blame/evolution lineage now.

## Context Integrity

Phase 4 is an important future integration seam.

External evidence should eventually be able to identify:

```text
repository
base/head revision
file
symbol descriptor
source range
relation kind
resolution
derivation
change kind
```

Context Integrity may interpret those facts later.

Context Engine must not add authority, supersession, policy, or drift findings in Phase 4.

---

# 30. Final Scope Summary

Phase 4 should make this command meaningful:

```bash
ctx diff main..HEAD
```

by turning a Git range into explicit file/symbol/relation change evidence.

It should make this command useful:

```bash
ctx impact main..HEAD
```

by traversing the actual base/head structural models and reporting bounded, inspectable impact candidates.

The phase succeeds when it can answer:

> **What structurally changed between these commits, what evidence supports that conclusion, and what known structural neighbors should a developer inspect?**

It does not need to answer:

> **Was the change correct, architecturally allowed, semantically breaking, or sufficient for a specific coding task?**

Those belong to later ranking, semantic, historical, or Context Integrity layers.
