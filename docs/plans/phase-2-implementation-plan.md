# Codebase / Context Intelligence Engine — Phase 2 Implementation Plan

**Phase:** 2 — Repository Relationship Graph<br>
**Status:** Planned; implementation gated on final Phase 1 acceptance<br>
**Date:** 2026-10-14<br>
**Primary language:** Rust<br>
**Binary:** `ctx`<br>
**Repository:** `jasonkli21/context-engine`<br>
**Planning baseline inspected:** `b132b8e61843918e374598f05f17edef3862e174` (`fix: address phase 1 review findings`)<br>
**Prerequisites:** accepted Phase 1, `docs/prd.md`, `docs/architecture.md`, current Phase 1 code/tests<br>
**Primary new capabilities:** repository entities, containment/import/reference relations, forward/reverse traversal, graph serialization

---

# 1. Current Repository Baseline

This plan is based on the repository state actually present on `main` when the plan was authored.

Latest inspected commit:

```text
b132b8e61843918e374598f05f17edef3862e174
fix: address phase 1 review findings
```

The previous implementation commit was:

```text
f6f39f4e91c94fc480c3a1c69248589bf8bbdbe6
implement phase 1
```

The Phase 1 review-fix commit addresses:

- project-owned per-file symbol/name/qualification resource limits;
- Rust reference/pointer impl-target qualification;
- documentation wording around concurrent source mutation.

The repository currently contains:

- the Phase 0 scanner;
- Rust/Python Tree-sitter analyzers;
- bounded source loading;
- language-neutral `Symbol` and source ranges;
- `RepositoryIndex`;
- exact symbol lookup;
- `index`, `symbols`, and `symbol` CLI commands;
- deterministic schema-v1 JSON DTOs;
- Phase 1 independent review and review-fix evidence.

At planning time, `docs/current-state.md` still says:

```text
Phase 1 review findings fixed and locally verified;
independent final acceptance pending.
```

Therefore Phase 2 is **planned but not implementation-ready until final Phase 1 disposition is explicitly accepted**.

---

# 2. Phase 2 Implementation Gate

Before any Phase 2 code changes:

1. read `docs/current-state.md`;
2. confirm Phase 1 is explicitly accepted/closed or explicitly authorized for Phase 2 by the user;
3. confirm no unresolved Phase 1 blocker remains;
4. run:

```bash
./scripts/verify.sh
```

5. inspect the final Phase 1 review/fix disposition;
6. confirm the Phase 1 resource-safety fixes remain present.

If final Phase 1 acceptance is still pending, stop before editing code.

Do not silently treat the existence of commit `b132b8e` as final acceptance.

---

# 3. Phase Objective

Phase 2 turns the Phase 1 symbol inventory into the first repository relationship graph.

The engine should be able to answer:

> What entities exist, how are they structurally related, what syntactic dependency/reference facts are observable, and what evidence supports each relation?

The new conceptual pipeline is:

```text
repository scan
      ↓
syntax + symbol analysis
      ↓
relation candidate extraction
      ↓
entity construction
      ↓
relation resolution
      ↓
forward/reverse adjacency
      ↓
repository graph
      ↓
graph / deps / callers / related
```

The graph remains:

- in-memory;
- rebuilt per command;
- synchronous;
- local;
- read-only;
- non-persistent;
- syntax/evidence based;
- separate from Context Integrity policy.

---

# 4. Product Boundary

Phase 2 should represent **observable relationships**, not claim compiler certainty.

It should distinguish:

```text
known structural fact
vs
syntactic relation candidate
vs
heuristically resolved target
vs
unresolved target
```

Examples:

### Structural fact

```text
File src/foo.py lexically contains class Router.
```

### Syntactic fact

```text
Router.route contains a call-site reference whose target text is "helper".
```

### Heuristic resolution

```text
"helper" uniquely matches a symbol named helper in the same file.
```

### Unresolved fact

```text
File src/foo.rs imports crate::storage::Store,
but Phase 2 does not claim which repository entity that path resolves to.
```

The graph must not turn a Tree-sitter name match into:

```text
compiler-resolved dependency
```

unless a later backend actually provides that evidence.

---

# 5. Phase 2 Product Goals

By completion, Context Engine should:

1. construct deterministic repository, file, and symbol graph entities;
2. assign explicit graph entity identity separate from `qualified_name`;
3. create lexical containment relations;
4. extract Rust import relations;
5. extract Python import relations;
6. extract bounded call-site reference relations;
7. resolve only explicitly supported same-file reference cases;
8. preserve unresolved import/reference targets as first-class evidence;
9. attach evidence and derivation metadata to every relation;
10. build forward and reverse adjacency for resolved relations;
11. handle cycles and self-loops safely;
12. expose full graph serialization;
13. expose forward dependency traversal;
14. expose reverse syntactic-caller traversal;
15. expose general related-entity traversal;
16. preserve Phase 0 and Phase 1 behavior/contracts;
17. remain bounded under hostile/unusually dense source.

---

# 6. Rust Learning Objectives

Phase 2 is intentionally the collections/graph phase.

The implementation should provide practice with:

- newtype IDs;
- ownership of graph entities;
- `HashMap` / `BTreeMap`;
- `HashSet`;
- adjacency lists;
- forward and reverse indexes;
- iterative BFS/DFS;
- cycle detection/visited sets;
- deterministic ordering after hash-based computation;
- graph construction in multiple passes;
- domain identity vs display names;
- enums for resolved/unresolved states;
- evidence modeling;
- resource budgeting;
- avoiding O(N²) graph construction where N can be attacker-influenced.

Still defer:

- threads;
- async;
- shared mutable concurrency;
- database graph storage;
- persistent indexes;
- network services.

---

# 7. User Stories

## US-2.1 — Build a repository graph

```bash
ctx graph .
```

shows a graph summary.

```bash
ctx graph . --format json
```

serializes the complete Phase 2 graph.

## US-2.2 — Inspect dependencies

```bash
ctx deps Router.route .
```

shows direct outgoing Phase 2 dependency/reference facts for every matching root symbol.

With depth:

```bash
ctx deps Router.route . --depth 2
```

follows resolved dependency/reference edges while remaining cycle-safe.

Unresolved leaves are displayed but not traversed.

## US-2.3 — Inspect syntactic callers

```bash
ctx callers helper .
```

shows incoming **resolved call-context reference** edges.

This command must be documented as syntactic/best-effort, not a compiler call graph.

## US-2.4 — Inspect related entities

```bash
ctx related Router .
```

shows graph neighbors using:

- containment;
- resolved references;
- resolved relation edges available in this phase.

It traverses edges in both directions.

## US-2.5 — Preserve ambiguous symbols

If:

```text
a.py contains helper
b.py contains helper
```

then:

```bash
ctx deps helper .
```

does not collapse them into one entity.

Results are grouped by each matched root.

An optional file filter may narrow the query.

## US-2.6 — Preserve unresolved imports

For:

```rust
use crate::storage::Store;
```

Phase 2 may produce:

```text
imports → crate::storage::Store
resolution: unresolved
derivation: rust_use_syntax
```

without claiming which file/symbol it resolves to.

## US-2.7 — Handle recursive/cyclic relationships

Self-recursion and mutual references do not cause infinite traversal.

---

# 8. Scope

## 8.1 In Scope

- graph-specific entity identity;
- repository entity;
- file entities;
- symbol entities;
- relation IDs;
- `contains`;
- `imports`;
- `references`;
- call-context reference extraction;
- relation derivation metadata;
- resolved vs unresolved target state;
- same-file exact/unique symbol resolution;
- repository/file/symbol containment;
- forward adjacency;
- reverse adjacency;
- iterative traversal;
- graph query limits;
- `ctx graph`;
- `ctx deps`;
- `ctx callers`;
- `ctx related`;
- optional `--file` query disambiguation;
- bounded `--depth`;
- human output;
- schema-v1 JSON for new commands;
- deterministic graph serialization;
- graph fixtures;
- security/resource tests;
- Phase 2 implementation report.

## 8.2 Explicitly Out of Scope

Do not implement:

- compiler/type resolution;
- full Rust module resolution;
- full Python import resolution;
- cross-file import target resolution;
- true semantic call graph;
- method dispatch resolution;
- trait dispatch resolution;
- Python dynamic dispatch resolution;
- `self.method()` → class method inference unless exact syntax-local resolution is trivial and explicitly justified;
- `Self::method()` inference;
- inheritance relations;
- implementation relations;
- type-reference relations;
- field access relations;
- variable/dataflow relations;
- cross-repository relationships;
- Git structural diff;
- history;
- impact analysis;
- centrality/PageRank;
- hotspots;
- ranking/context bundles;
- TypeScript parsing;
- persistence;
- SQLite;
- Tantivy;
- graph database;
- incremental indexing;
- filesystem watching;
- threads;
- Rayon;
- async/Tokio;
- MCP;
- server/API;
- LSP/SCIP/compiler backends;
- embeddings;
- LLMs;
- Context Integrity policy.

---

# 9. Critical Design Principle: Identity Is Not `qualified_name`

Phase 1 explicitly established:

```text
qualified_name
```

as syntax-local display/query evidence, not stable identity.

Phase 2 must not use it as a graph node key.

Two symbols with identical:

```text
qualified_name
```

must remain distinct when they differ by:

- file;
- range;
- kind.

---

# 10. Entity Model

## 10.1 Internal `EntityId`

Recommended:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(u32);
```

`EntityId` is:

- deterministic within one built graph;
- opaque;
- index-local;
- not stable across repository edits;
- not lineage identity.

Document this explicitly.

## 10.2 Entity kinds

```rust
pub enum EntityKind {
    Repository,
    File,
    Symbol,
}
```

## 10.3 Entity key

Use an owned structural key independent of names.

Conceptually:

```rust
pub enum EntityKey {
    Repository,
    File {
        path: PathBuf,
    },
    Symbol {
        path: PathBuf,
        kind: SymbolKind,
        start_byte: usize,
        end_byte: usize,
    },
}
```

The symbol key may use `name_range` or whole range.

Preferred:

```text
path + kind + whole declaration range
```

because the declaration range identifies duplicate same-name declarations in one file.

This is revision-scoped, not refactor-stable.

## 10.4 Graph entity

Conceptually:

```rust
pub struct GraphEntity {
    pub id: EntityId,
    pub key: EntityKey,
}
```

Do not duplicate full source bodies.

For symbol entities, output/adapters can map the key back to the canonical `Symbol`.

---

# 11. Deterministic Entity Assignment

Assign IDs only after the canonical Phase 1 index is sorted.

Recommended ordering:

```text
0                repository
1..F             files by relative path
F+1..F+S         symbols in existing RepositoryIndex sort order
```

This makes IDs deterministic for unchanged input.

However:

> IDs are not stable if an earlier-sorted entity is added or removed.

Machine output must state that IDs are index-local.

Do not add hashing/UUID dependencies solely to make IDs look stable.

---

# 12. Relation Model

## 12.1 Relation ID

```rust
pub struct RelationId(u32);
```

Assigned after deterministic relation sorting.

Same stability caveat as `EntityId`.

## 12.2 Relation kind

```rust
pub enum RelationKind {
    Contains,
    Imports,
    References,
}
```

Do not add `Calls`.

Phase 2 callers are derived from `References` with:

```text
ReferenceContext::Call
```

This keeps the product honest that compiler-resolved call semantics do not yet exist.

## 12.3 Relation target

Relations may not always resolve to a graph entity.

Conceptually:

```rust
pub enum RelationTarget {
    Resolved(EntityId),
    Unresolved(NamedTarget),
}
```

## 12.4 Named target

```rust
pub struct NamedTarget {
    pub text: String,
}
```

The target text must be:

- normalized from accepted syntax forms;
- bounded;
- not arbitrary full source expressions.

## 12.5 Relation metadata

Recommended:

```rust
pub enum RelationMetadata {
    None,
    Import {
        binding: Option<String>,
        wildcard: bool,
    },
    Reference {
        context: ReferenceContext,
    },
}
```

## 12.6 Reference context

Phase 2 initially requires:

```rust
pub enum ReferenceContext {
    Call,
}
```

Later phases may expand.

---

# 13. Evidence Model

Every relation needs evidence.

Introduce an explicit Phase 2 `EvidenceRef`.

Recommended:

```rust
pub struct EvidenceRef {
    pub relative_path: PathBuf,
    pub range: Option<SourceRange>,
    pub language: Option<Language>,
    pub derivation: RelationDerivation,
}
```

## 13.1 Derivation

Recommended:

```rust
pub enum RelationDerivation {
    RepositoryContainsFile,
    LexicalRangeContainment,
    RustUseSyntax,
    PythonImportSyntax,
    RustCallSyntax,
    PythonCallSyntax,
}
```

Keep **extraction derivation** separate from **resolution quality**.

## 13.2 Resolution

Recommended:

```rust
pub enum ResolutionKind {
    DirectStructural,
    ExactSameFileQualifiedName,
    UniqueSameFileLocalName,
    Unresolved,
}
```

Examples:

```text
contains
  derivation = lexical_range_containment
  resolution = direct_structural

reference
  derivation = rust_call_syntax
  resolution = unique_same_file_local_name

import
  derivation = python_import_syntax
  resolution = unresolved
```

This distinction is important for future Context Integrity consumers.

---

# 14. Revision / Repository Evidence

The graph should retain existing optional Git metadata.

Do **not** claim:

```text
HEAD == exact source snapshot
```

when the working tree can be dirty.

Graph JSON may include:

- scan root;
- Git repository root;
- HEAD;
- branch;
- dirty state;

using existing semantics.

Do not embed HEAD into each relation as if it uniquely identified the source content.

---

# 15. Repository Graph

Recommended:

```rust
pub struct RepositoryGraph {
    pub index: RepositoryIndex,
    pub entities: Vec<GraphEntity>,
    pub relations: Vec<Relation>,
    pub relation_files: Vec<RelationFileAnalysis>,

    // internal/query indexes
    forward: HashMap<EntityId, Vec<RelationId>>,
    reverse: HashMap<EntityId, Vec<RelationId>>,
}
```

Exact visibility may vary.

Key invariant:

> The externally meaningful facts are entities, relations, evidence, and relation-analysis accounting. Adjacency indexes are implementation details.

---

# 16. Relation-Analysis Accounting

Phase 1 carefully accounts for every scanned file.

Phase 2 must preserve that discipline.

Add a relation-analysis record for supported analyzed files.

Recommended:

```rust
pub enum RelationAnalysisStatus {
    Analyzed,
    ParseFailed,
    AnalysisLimitExceeded,
}
```

Files unavailable to relation analysis because Phase 1 analysis did not succeed remain visible through the existing:

```text
FileAnalysis.status
```

Graph summary should distinguish:

- symbol analysis unavailable;
- relation extraction failed;
- relation extraction limit exceeded;
- relation extraction succeeded.

## 16.1 Relation file record

```rust
pub struct RelationFileAnalysis {
    pub relative_path: PathBuf,
    pub status: RelationAnalysisStatus,
    pub import_count: usize,
    pub reference_count: usize,
    pub unresolved_count: usize,
    pub unsupported_candidate_count: usize,
}
```

No source body.

---

# 17. Avoiding Double-Parse Snapshot Mismatch

Do not implement Phase 2 graph construction as:

```text
Phase 1 full index pass
↓
reopen every file
↓
second independent parse pass
↓
merge relations into earlier symbols
```

That would create a larger within-command race:

```text
symbols from content A
relations from content B
```

because the filesystem is not an atomic snapshot.

Instead, refactor the common analysis pipeline so a **graph analysis mode** can extract:

- Phase 1 symbols;
- Phase 1 diagnostics;
- Phase 2 relation candidates;

from the same loaded source/tree for that file.

Existing Phase 1 commands should still be able to skip relation extraction.

Recommended conceptual mode:

```rust
enum AnalysisMode {
    Symbols,
    Graph,
}
```

Exact implementation may differ.

Required invariant:

> When graph mode succeeds for a file, its symbols and relation candidates come from the same parse tree/source read.

---

# 18. Analyzer Boundary Evolution

Current Phase 1:

```rust
LanguageAnalyzer::analyze(...)
    -> AnalyzedFile
```

Phase 2 may evolve this cleanly.

Possible design:

```rust
fn analyze(
    &mut self,
    path: &Path,
    text: &str,
    mode: AnalysisMode,
) -> Result<AnalyzedFile, AnalysisError>
```

with:

```rust
pub struct AnalyzedFile {
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub syntax_error_count: usize,

    // Phase 2 internal
    pub relation_candidates: Vec<RelationCandidate>,
    pub relation_status: RelationAnalysisStatus,
    pub unsupported_relation_candidates: usize,
}
```

Alternative structures are acceptable if they preserve:

- one parse/source read in graph mode;
- symbol results surviving relation-specific limit failure where safe;
- Tree-sitter types confined to language modules;
- Phase 1 command output unchanged.

---

# 19. Relation Extraction Budgets

Phase 1 review established an important security principle:

> Parse-size bounds do not bound extracted output/work.

Phase 2 must apply the same lesson to relationships.

Use project-owned per-file relation budgets.

Recommended initial constants:

```text
max relation candidates:          16,384
max normalized target bytes:       4 MiB aggregate
max relation-normalization visits: 50,000
max target segments:               64
max one target text:               4 KiB
```

Exact constants may be adjusted during implementation, but explicit limits must exist.

If a relation budget is exceeded:

- discard **all Phase 2 relation candidates for that file**;
- preserve already-valid Phase 1 symbol results;
- mark relation analysis as `analysis_limit_exceeded`;
- emit one bounded deterministic relation diagnostic/status;
- do not return partial graph relations from that file.

Do not reuse Tree-sitter query `match_limit` as a total result bound.

---

# 20. Repository-Level Graph Limits

Add conservative global graph-construction limits.

Recommended initial ceilings:

```text
max graph entities:   1,000,000
max graph relations:  2,000,000
```

These are safety ceilings, not expected ordinary sizes.

Exceeding one should fail the graph command clearly and deterministically rather than allocate until exhaustion.

Do not add CLI knobs for these in Phase 2 unless a real need appears.

---

# 21. Containment Relations

Containment is the most trustworthy Phase 2 relation.

## 21.1 Repository → File

Every scanned file entity:

```text
Repository --contains--> File
```

Evidence:

```text
relative path
derivation = repository_contains_file
```

No source range required.

## 21.2 File/Symbol → Symbol

For every symbol:

- find the nearest lexical symbol whose whole declaration range strictly contains the symbol;
- if found:

```text
ParentSymbol --contains--> ChildSymbol
```

- otherwise:

```text
File --contains--> Symbol
```

## 21.3 Semantics

`contains` means:

> lexical source-range containment

It does **not** mean:

- semantic ownership;
- type membership;
- module export;
- implementation membership;
- package membership.

Important Rust consequence:

```rust
struct Foo {}

impl Foo {
    fn bar() {}
}
```

`Foo`'s struct declaration range does not contain the `impl`.

Therefore Phase 2 may correctly produce:

```text
file contains Foo
file contains Foo::bar
```

and **not**:

```text
Foo contains Foo::bar
```

A future `member_of`/`implements` relation may model that semantics.

Do not fake semantic containment using `qualified_name`.

---

# 22. Efficient Containment Construction

Do not use an obvious:

```text
for every symbol:
    scan every other symbol
```

O(S²) implementation.

Phase 1 allows up to thousands of symbols/file.

Use a range-aware stack or interval-based algorithm.

Recommended:

1. group symbols by file;
2. sort by:
   - start byte ascending;
   - end byte descending;
3. maintain stack of currently containing ranges;
4. pop until top strictly contains current;
5. assign top as parent or file;
6. push current where appropriate.

Handle malformed/overlapping ranges defensively.

If ranges overlap without proper nesting:

- choose no symbol parent or the narrowest unambiguous parent;
- never invent contradictory containment;
- cover with a unit test using synthetic ranges.

---

# 23. Rust Import Extraction

Extract syntactic `use` facts.

Required cases where grammar support is straightforward:

```rust
use foo::bar;
use foo::bar as baz;
use foo::{bar, baz};
use foo::{bar as b, nested::Thing};
use foo::*;
use crate::x::Y;
use self::x;
use super::x;
```

Normalize grouped imports into atomic relation candidates where safely possible.

Examples:

```text
use foo::{bar, baz};

imports foo::bar
imports foo::baz
```

Alias:

```text
target = foo::bar
binding = baz
```

Wildcard:

```text
target = foo
wildcard = true
```

Do not attempt crate/file resolution in Phase 2.

## 23.1 Rust import normalization safety

Rust use trees can nest.

Use iterative/bounded traversal.

Do not copy arbitrary source expressions.

Accepted target segments should come from syntactic identifier/path components.

Unsupported use syntax:

- increments `unsupported_candidate_count`;
- does not produce a guessed relation.

---

# 24. Python Import Extraction

Required:

```python
import foo
import foo.bar
import foo as f
import foo.bar as fb

from foo import Bar
from foo.bar import Baz
from foo import A, B
from foo import A as Alias
from . import sibling
from ..pkg import Name
from foo import *
```

Normalize into one relation candidate per imported name where appropriate.

Examples:

```text
from foo import A, B

imports foo.A
imports foo.B
```

Alias:

```text
target = foo.A
binding = Alias
```

Wildcard:

```text
target = foo
wildcard = true
```

Relative import leading dots should remain visible in normalized text.

Do not resolve to files/packages in Phase 2.

---

# 25. Call-Site Reference Extraction

Phase 2 `references` should begin conservatively with **call-site references**.

This supports the PRD's early graph/callers use case without claiming a full call graph.

## 25.1 Rust accepted forms

At minimum:

```rust
helper()
module::helper()
crate::module::helper()
```

Optionally support generic-call wrappers when the callee can be normalized without copying arbitrary expressions.

Do not normalize:

```rust
obj.method()
(factory())()
arr[i]()
```

unless there is a clearly bounded syntax representation.

Unsupported forms should be counted, not guessed.

## 25.2 Python accepted forms

At minimum:

```python
helper()
module.helper()
Outer.helper()
```

Only accept dotted attribute chains rooted in an identifier.

Do not normalize arbitrary expression callees such as:

```python
factory()()
items[0]()
```

unless a later phase explicitly expands semantics.

## 25.3 Reference evidence

For a normalized call target:

```text
kind = references
context = call
target_text = normalized callee
evidence range = callee syntax range
```

Do not output the full call arguments/body.

---

# 26. Relation Source Ownership

Import/reference candidates need a source entity.

Use lexical source ownership:

1. find the innermost symbol whose declaration range contains the candidate range;
2. if none, source is the file entity.

Examples:

```python
def f():
    import x
    helper()
```

produces source:

```text
f
```

for both relation candidates.

Module-level import/call source:

```text
file entity
```

Use the same containment index/data structure where possible.

Do not derive source ownership from `qualified_name`.

---

# 27. Reference Resolution Rules

Phase 2 only resolves references in the **same file**.

This is intentional.

Do not claim cross-file resolution without import/module semantics.

Given normalized target text:

## Rule 1 — exact same-file qualified name

If target text exactly equals exactly one same-file symbol:

```text
qualified_name
```

resolve:

```text
ExactSameFileQualifiedName
```

If multiple symbols have that same qualified name, do not choose one.

## Rule 2 — unique same-file local name

If no exact qualified match exists and the target is an unqualified name:

```text
helper
```

and exactly one same-file symbol has local name `helper`, resolve:

```text
UniqueSameFileLocalName
```

This is heuristic syntactic resolution.

It does not account for:

- local variables;
- shadowing;
- imports;
- runtime rebinding.

The resolution metadata must preserve that limitation.

## Rule 3 — otherwise unresolved

Keep:

```text
target_text
resolution = unresolved
```

Do not discard the relation merely because it did not resolve.

---

# 28. Why Imports Stay Unresolved in Phase 2

Rust module/crate resolution requires awareness of:

- crate roots;
- `mod`;
- `mod.rs`;
- path attributes;
- workspaces;
- reexports;
- aliases.

Python import resolution requires:

- package roots;
- `__init__.py`;
- namespace packages;
- runtime `sys.path`;
- relative levels;
- environment.

Implementing these accurately is not a small graph primitive.

Therefore Phase 2 records import facts without falsely mapping them to files.

Later phases can add a separate resolver and retain the original syntax evidence.

---

# 29. Relation Deduplication

Do not collapse distinct source occurrences merely because they share:

```text
source
target
kind
```

Two call sites are two pieces of evidence.

Keep one relation per syntactic occurrence.

Containment should naturally be unique.

Deterministic relation identity/order includes:

```text
kind
source
target
evidence path
evidence start/end range
resolution kind
```

---

# 30. Self-Loops and Cycles

Allow legitimate self-reference:

```rust
fn recurse() {
    recurse();
}
```

Graph:

```text
recurse --references--> recurse
```

Do not drop self-loops.

Traversal must use visited state to avoid infinite loops.

Test:

- self-recursion;
- A references B;
- B references A.

---

# 31. Adjacency Model

No graph library is required.

Recommended internal representation:

```rust
HashMap<EntityId, Vec<RelationId>> // forward
HashMap<EntityId, Vec<RelationId>> // reverse
```

Only resolved-target relations belong in reverse adjacency.

Forward adjacency may include relation IDs whose target is unresolved, because they are still outgoing facts.

Unresolved relations:

- can be displayed;
- cannot be traversed to another entity.

Sort each adjacency relation list deterministically after construction.

Do not add `petgraph` unless current implementation pressure clearly justifies it.

---

# 32. Graph Query Budgets

Graph traversal is another attacker-influenced expansion surface.

Use explicit limits.

Recommended:

```text
default depth:                 1
maximum requested depth:       8
maximum visited entities: 10,000
maximum emitted relations:25,000
```

If a traversal budget is exceeded:

- stop deterministically;
- mark result as truncated;
- do not silently pretend traversal is complete.

Depth `0` may mean root only.

Do not use recursive traversal; use iterative BFS/DFS.

---

# 33. Graph Query Root Resolution

Reuse Phase 1 exact symbol semantics.

Given `<NAME>`:

1. exact case-sensitive `qualified_name`;
2. only if none, exact case-sensitive local `name`;
3. zero matches is valid;
4. multiple matches remain distinct roots.

Do not merge multiple root symbols.

Add optional:

```bash
--file <RELATIVE_PATH>
```

to narrow roots.

The file filter:

- applies after name matching;
- uses scan-root-relative path semantics;
- must not allow path traversal;
- does not change graph entity identity.

If multiple roots remain, output grouped results for each.

Do not make ambiguity an excuse to guess.

---

# 34. CLI Contract

## 34.1 `ctx graph`

```bash
ctx graph [PATH]
    [--format human|json]
    [--max-parse-file-size BYTES]
```

Human output is summary-oriented.

JSON serializes the full Phase 2 graph.

## 34.2 `ctx deps`

```bash
ctx deps <NAME> [PATH]
    [--file RELATIVE_PATH]
    [--depth N]
    [--format human|json]
    [--max-parse-file-size BYTES]
```

Semantics:

- roots are matching symbols;
- traverse outgoing:
  - resolved `references`;
  - future resolved dependency relation types;
- include direct unresolved `imports` / `references` owned by visited entities as leaf facts;
- do not traverse `contains` as a dependency edge.

Module/file-scoped imports are not automatically inherited by every symbol.

This prevents overstating dependency semantics.

## 34.3 `ctx callers`

```bash
ctx callers <NAME> [PATH]
    [--file RELATIVE_PATH]
    [--depth N]
    [--format human|json]
    [--max-parse-file-size BYTES]
```

Semantics:

- reverse traversal only over resolved:
  - `RelationKind::References`
  - `ReferenceContext::Call`

Help/output must say:

```text
syntactic callers / call-site references
```

not:

```text
compiler-resolved call graph
```

## 34.4 `ctx related`

```bash
ctx related <NAME> [PATH]
    [--file RELATIVE_PATH]
    [--depth N]
    [--format human|json]
    [--max-parse-file-size BYTES]
```

Traverse resolved graph relations in both directions.

Include:

- containment;
- resolved references.

Unresolved relations may be included as attached facts but are not traversal nodes.

---

# 35. `ctx graph` Human Output

Example shape:

```text
Repository graph: /path/to/repo

Entities
  Repository:      1
  Files:          84
  Symbols:       312
  Total:         397

Relations
  Contains:      396
  Imports:        44
  References:    87
  Resolved:      451
  Unresolved:     76

Relation analysis
  Files analyzed:               27
  Relation-limit failures:       0
  Other relation failures:       0
  Unsupported call candidates:  13

Evidence
  Rust files: ...
  Python files: ...

Git
  ...
```

Do not print every entity/relation in human `graph`.

Use JSON for full serialization.

---

# 36. Graph JSON Contract

Use the existing envelope style:

```json
{
  "schema_version": 1,
  "command": "graph",
  "data": {}
}
```

Do not mutate existing Phase 0/1 DTOs merely to expose graph state.

## 36.1 Entities

Example:

```json
{
  "id": 42,
  "kind": "symbol",
  "path": "src/model.rs",
  "symbol": {
    "kind": "struct",
    "name": "RepositoryIndex",
    "qualified_name": "RepositoryIndex",
    "range": { ... }
  }
}
```

Repository/file entities have appropriate reduced shapes.

Document:

```text
id is index-local and not stable across edits/rebuilds
```

## 36.2 Relations

Resolved:

```json
{
  "id": 100,
  "kind": "references",
  "source": 42,
  "target": {
    "state": "resolved",
    "entity_id": 55
  },
  "resolution": "unique_same_file_local_name",
  "metadata": {
    "reference_context": "call"
  },
  "evidence": {
    "path": "src/foo.rs",
    "range": { ... },
    "language": "rust",
    "derivation": "rust_call_syntax"
  }
}
```

Unresolved:

```json
{
  "id": 101,
  "kind": "imports",
  "source": 3,
  "target": {
    "state": "unresolved",
    "text": "crate::storage::Store"
  },
  "resolution": "unresolved",
  "metadata": {
    "binding": null,
    "wildcard": false
  },
  "evidence": { ... }
}
```

No source body.

---

# 37. Query JSON Contract

For:

```text
deps
callers
related
```

return:

```text
query
match_mode
roots[]
depth
truncated
results grouped by root
```

Each result group should identify:

- root entity;
- visited entities;
- traversed relations;
- unresolved leaf relations where relevant.

Do not flatten roots together.

---

# 38. Existing Schema Compatibility

Existing commands must remain semantically compatible:

```text
scan
stats
tree
index
symbols
symbol
```

Do not add graph fields to their schema-v1 output unless clearly additive, intentional, documented, and needed.

Preferred:

> keep existing DTOs unchanged and add graph-specific DTOs.

---

# 39. Relation Extraction Query Assets

Add separate project-owned query assets:

```text
src/language/queries/rust-relations.scm
src/language/queries/python-relations.scm
```

Do not overload symbol queries into an unreadable all-purpose file.

Recommended captures:

```text
@relation.import
@relation.call
```

Language-specific code may then normalize nested syntax.

Embedded query compilation failure remains a fatal implementation/dependency error.

---

# 40. Tree-sitter Types Stay Internal

As in Phase 1:

- no `Node`;
- no `Tree`;
- no `Point`;
- no query capture type;

may appear in:

- graph domain;
- output DTO;
- query traversal layer.

Convert to owned relation candidates and `SourceRange` before leaving language modules.

---

# 41. Raw Relation Candidate

Recommended internal language-neutral type:

```rust
pub struct RelationCandidate {
    pub kind: RelationKind,
    pub target_text: String,
    pub target_range: SourceRange,
    pub metadata: RelationCandidateMetadata,
    pub derivation: RelationDerivation,
}
```

It intentionally does **not** contain:

- source `EntityId`;
- resolved target `EntityId`.

Those are assigned after all symbols/entities exist.

---

# 42. Normalized Target Rules

Do not copy arbitrary expressions.

A normalized target is composed from accepted identifier/path segments.

Examples:

Rust:

```text
crate::foo::Bar
foo::helper
helper
```

Python:

```text
foo.bar.Baz
module.helper
helper
..pkg.Name
```

Target normalization must:

- enforce segment count;
- enforce total bytes;
- reject control characters or escape at output boundary;
- not include call arguments;
- not include arbitrary generic expressions.

---

# 43. Import Alias Handling

Alias is metadata, not target identity.

Example:

```python
from foo import Bar as B
```

record:

```text
target = foo.Bar
binding = B
```

Do not resolve later calls to `B()` through alias in Phase 2.

That requires import/name-resolution semantics and is intentionally deferred.

Likewise:

```rust
use foo::Bar as B;
```

does not make `B()` resolve to `foo::Bar` in Phase 2.

---

# 44. Same-File Reference Resolver

Build per-file lookup tables after entities exist.

Recommended:

```text
qualified_name -> Vec<EntityId>
local_name     -> Vec<EntityId>
```

Only symbol entities from that file participate.

Resolution:

```text
exact qualified if exactly one
else unique local for unqualified target
else unresolved
```

Never choose one of multiple candidates.

Do not use global repository name uniqueness as a substitute for module/import resolution.

---

# 45. Graph Construction Pipeline

Recommended:

```text
GraphOptions
   ↓
analyze_repository(mode = graph)
   ↓
RepositoryIndex
+ relation candidates
+ relation-file accounting
   ↓
assign deterministic entities
   ↓
build repository/file/symbol containment
   ↓
build same-file symbol lookup
   ↓
map candidate source owner
   ↓
resolve candidate target conservatively
   ↓
construct relations
   ↓
sort relations
   ↓
assign RelationId
   ↓
build forward adjacency
   ↓
build reverse adjacency
   ↓
RepositoryGraph
```

No persistence.

---

# 46. Graph Options

Recommended:

```rust
pub struct GraphOptions {
    pub root: PathBuf,
    pub max_parse_file_bytes: u64,
    pub probe_git: bool,
}
```

Traversal options should remain command/query specific:

```rust
pub struct TraversalOptions {
    pub depth: usize,
    pub max_entities: usize,
    pub max_relations: usize,
}
```

Do not put query strings into graph construction options.

---

# 47. Graph Errors

Typed errors should distinguish:

```text
scan/index error
analyzer initialization
graph entity limit exceeded
graph relation limit exceeded
internal inconsistent entity mapping
invalid traversal depth
```

Per-file relation extraction failures should normally be represented in graph accounting rather than aborting the entire graph.

Programmer invariants may be fatal if graph references an impossible missing entity.

No panic on arbitrary repository source.

---

# 48. Work Breakdown

Implement in vertical slices.

---

## Step 2.0 — Final Phase 1 acceptance preflight

Before edits:

1. read current state;
2. confirm Phase 1 is accepted;
3. run canonical verifier;
4. inspect `b132b8e` fix behavior still present;
5. record baseline in implementation report.

If not accepted, stop.

---

## Step 2.1 — Add graph domain IDs and evidence types

Add:

- `EntityId`;
- `RelationId`;
- `EntityKind`;
- `EntityKey`;
- `GraphEntity`;
- `RelationKind`;
- `RelationTarget`;
- `NamedTarget`;
- relation metadata;
- evidence/derivation/resolution enums;
- relation-analysis accounting types.

No CLI yet.

Tests:

- IDs compare/order;
- duplicate qualified symbols create distinct entity keys;
- output name strings stable.

---

## Step 2.2 — Add relation-analysis mode and candidate type

Refactor the analyzer boundary minimally so:

```text
Symbols mode
Graph mode
```

can share one source read/tree.

Existing Phase 1 behavior must not change.

Add relation candidate structures but no extraction yet.

Tests:

- symbol-only mode returns identical Phase 1 symbols;
- graph mode without relation query still preserves identical symbols.

---

## Step 2.3 — Add relation budgets

Implement a project-owned relation budget separate from Phase 1 symbol budget.

Track at minimum:

- candidate count;
- normalized target bytes;
- normalization node/segment visits.

Exact-bound and over-bound unit tests.

On over-bound:

- no partial relation candidates for file;
- Phase 1 symbols remain valid;
- relation file status = limit exceeded.

---

## Step 2.4 — Rust import extraction

Add:

```text
rust-relations.scm
```

and Rust normalizer.

Cover:

- simple;
- scoped;
- grouped;
- nested grouped;
- alias;
- wildcard;
- `crate`;
- `self`;
- `super`.

Use iterative bounded use-tree traversal.

Do not resolve targets.

---

## Step 2.5 — Python import extraction

Add:

```text
python-relations.scm
```

and normalizer.

Cover:

- import;
- dotted import;
- aliases;
- from import;
- multiple names;
- relative import;
- wildcard.

No package/file resolution.

---

## Step 2.6 — Rust call-reference extraction

Capture conservative normalized call targets.

Required:

- identifier;
- scoped identifier.

Optional only if clearly safe:

- generic wrapper around accepted target.

Reject/count unsupported:

- field-expression calls;
- expression calls;
- complicated dynamic forms.

---

## Step 2.7 — Python call-reference extraction

Required:

- identifier calls;
- identifier-rooted dotted attribute chains.

Reject/count:

- call-return-call;
- subscript callable;
- arbitrary expression callable.

No `self`/class inference.

---

## Step 2.8 — Build deterministic entities

Given sorted index:

1. repository entity;
2. files;
3. symbols.

Create:

```text
EntityKey -> EntityId
```

lookup.

Enforce global entity cap.

Test deterministic IDs on identical fixture.

Test duplicate qualified names get different IDs.

---

## Step 2.9 — Build containment graph

Implement efficient lexical containment.

Add:

```text
repository -> file
file -> top-level symbol
symbol -> nested symbol
```

Tests:

- Python class -> method;
- nested function;
- Rust inline module -> function;
- Rust impl method remains file-contained;
- malformed synthetic overlapping range behavior;
- no O(S²) obvious construction.

---

## Step 2.10 — Map relation source owners

For each relation candidate:

- innermost containing symbol;
- otherwise file.

Reuse containment/index structures.

Test:

- function-local import;
- function-local call;
- module-level import;
- nested function call source ownership.

---

## Step 2.11 — Same-file target resolution

Build per-file:

```text
qualified map
local map
```

Resolve according to Section 27.

Tests:

- exact qualified;
- unique local;
- duplicate local unresolved;
- duplicate qualified unresolved;
- cross-file same name unresolved;
- recursive self-reference;
- mutual references.

---

## Step 2.12 — Construct/sort relations

Create resolved/unresolved relations.

Do not dedupe distinct evidence occurrences.

Sort deterministically before assigning `RelationId`.

Enforce global relation cap.

Tests:

- repeated call sites remain separate;
- relation IDs stable on stable input;
- evidence ranges distinct.

---

## Step 2.13 — Build forward/reverse adjacency

Resolved:

- forward + reverse.

Unresolved:

- forward only.

Sort adjacency lists.

Tests:

- expected outgoing;
- expected incoming;
- unresolved absent from reverse entity lookup;
- self-loop;
- cycle.

---

## Step 2.14 — Implement graph traversal primitives

Implement iterative traversal.

Required:

- forward dependency traversal;
- reverse call-reference traversal;
- undirected related traversal;
- visited sets;
- depth;
- entity/relation result budgets;
- truncation signal.

Tests:

- depth 0/1/2;
- cycles;
- self-loop;
- truncation;
- deterministic traversal result ordering.

---

## Step 2.15 — Add `ctx graph`

CLI + summary human output + full JSON.

Do not make human output enormous.

Test:

- help;
- default path;
- JSON entity/relation shape;
- deterministic JSON;
- index-local ID documentation.

---

## Step 2.16 — Add graph symbol query args

Create graph query arguments with:

```text
name
path
format
max parse file size
depth
optional --file
```

Validate:

```text
depth <= max supported depth
```

Reuse existing symbol lookup semantics.

Do not modify existing `ctx symbol`.

---

## Step 2.17 — Add `ctx deps`

Implement grouped per-root results.

Traversal:

- forward `references`;
- unresolved imports/references attached as leaves;
- no containment dependency traversal.

Test:

- resolved direct ref;
- depth;
- unresolved import leaf;
- duplicate root grouping;
- file filter.

---

## Step 2.18 — Add `ctx callers`

Reverse only:

```text
references + call context
```

Output calls itself:

```text
syntactic callers
```

Test:

- direct;
- recursive;
- mutual cycle;
- unresolved call omitted from resolved caller traversal;
- multiple roots.

---

## Step 2.19 — Add `ctx related`

Traverse resolved relations in both directions.

Include containment.

Test:

- file/symbol containment neighborhood;
- reference neighbor;
- depth 2;
- cycles;
- unresolved relation not traversed.

---

## Step 2.20 — Security/resource regression coverage

Add hostile/dense fixtures:

- many call sites;
- deep Rust use tree;
- deeply nested Python attributes/imports where grammar allows;
- over-budget relation candidate file;
- repeated unresolved target text;
- cyclic reference graph;
- huge graph query bounded.

Verify:

- no partial file relations after per-file relation-budget failure;
- no infinite traversal;
- no arbitrary source expression emitted;
- source never executed.

---

## Step 2.21 — Preserve Phase 0/1 regressions

Run all existing tests unchanged.

Do not weaken Phase 1 resource limits.

Do not change:

```text
symbol lookup semantics
source range semantics
Phase 1 JSON
```

to make graph implementation easier.

---

## Step 2.22 — Real repository smoke tests

Run release binary on:

1. Context Engine;
2. Python Context Integrity repository;
3. optionally another Rust/Python repository.

Commands:

```bash
ctx graph .
ctx graph . --format json
ctx deps <known-symbol> .
ctx callers <known-symbol> .
ctx related <known-symbol> .
```

Record:

- entities;
- relation counts;
- resolved/unresolved counts;
- relation-analysis failures;
- unsupported call candidates;
- local wall-clock observations.

Do not call smoke results benchmarks.

---

## Step 2.23 — Documentation and report

Update:

```text
README.md
docs/current-state.md
```

Create:

```text
docs/reports/phase-2-implementation-report.md
```

Do not rewrite PRD/architecture unless real implementation reveals a durable correction.

---

# 49. Test Matrix

| Area | Case | Type |
|---|---|---|
| entity | deterministic repository/file/symbol IDs | unit |
| entity | duplicate qualified symbols distinct | unit |
| entity | entity cap | unit |
| evidence | derivation names | unit |
| containment | repository→file | unit |
| containment | file→top symbol | unit |
| containment | symbol→nested symbol | unit |
| containment | Rust impl method not fake-contained | fixture |
| containment | overlap defense | unit |
| Rust import | simple | fixture |
| Rust import | grouped | fixture |
| Rust import | alias | fixture |
| Rust import | wildcard | fixture |
| Rust import | nested group | fixture |
| Python import | import | fixture |
| Python import | from import | fixture |
| Python import | alias | fixture |
| Python import | relative | fixture |
| Python import | wildcard | fixture |
| Rust ref | identifier call | fixture |
| Rust ref | scoped call | fixture |
| Rust ref | unsupported field call counted | fixture |
| Python ref | identifier call | fixture |
| Python ref | dotted call | fixture |
| Python ref | unsupported dynamic call counted | fixture |
| resolution | exact same-file qualified | unit/integration |
| resolution | unique same-file local | unit/integration |
| resolution | duplicate unresolved | unit/integration |
| resolution | cross-file not guessed | integration |
| cycle | self-loop | unit |
| cycle | mutual refs | unit |
| relation | repeated call sites preserved | unit |
| relation | deterministic IDs | integration |
| relation budget | exact limit | unit |
| relation budget | over limit zero partial relations | integration |
| graph budget | entity limit | unit |
| graph budget | relation limit | unit |
| adjacency | forward | unit |
| adjacency | reverse | unit |
| adjacency | unresolved not reverse-resolved | unit |
| traversal | depth | unit |
| traversal | cycle-safe | unit |
| traversal | truncation | unit |
| CLI | graph help/human/json | integration |
| CLI | deps | integration |
| CLI | callers | integration |
| CLI | related | integration |
| CLI | file filter | integration |
| CLI | zero roots | integration |
| CLI | multiple roots grouped | integration |
| JSON | graph schema v1 | integration |
| JSON | deterministic stable fixture | integration |
| JSON | no source bodies | integration |
| regression | Phase 0 suite | integration |
| regression | Phase 1 suite | integration |
| security | target code not executed | integration |
| security | control escaping | integration |

---

# 50. Exact Acceptance Criteria

## Gate

- [ ] Phase 1 final acceptance/authorization recorded before Phase 2 code changes.
- [ ] canonical verifier passed at Phase 2 baseline.

## Identity

- [ ] `qualified_name` is not graph identity.
- [ ] repository/file/symbol entities have explicit IDs.
- [ ] duplicate qualified names remain distinct.
- [ ] IDs deterministic for stable graph input.
- [ ] IDs documented as index-local/non-lineage.

## Evidence

- [ ] every relation has evidence.
- [ ] derivation is explicit.
- [ ] resolution quality is explicit.
- [ ] unresolved target facts are preserved.
- [ ] optional Git HEAD is not represented as exact dirty-worktree content identity.

## Containment

- [ ] repository contains every file entity.
- [ ] file/symbol lexical containment constructed.
- [ ] nearest lexical parent used.
- [ ] Rust impl methods are not fake-contained by nominal struct declarations.
- [ ] algorithm is not obvious O(S²).

## Imports

- [ ] Rust imports extracted.
- [ ] Python imports extracted.
- [ ] grouped imports normalized.
- [ ] aliases retained as metadata.
- [ ] wildcard retained.
- [ ] import targets remain unresolved in Phase 2.
- [ ] unsupported syntax not guessed.

## References

- [ ] Rust accepted call targets extracted.
- [ ] Python accepted call targets extracted.
- [ ] call references labeled as `references` + call context.
- [ ] no compiler-call-graph claim.
- [ ] exact same-file qualified resolution implemented.
- [ ] unique same-file local resolution implemented.
- [ ] ambiguous cases remain unresolved.
- [ ] cross-file matching is not guessed.
- [ ] unsupported call forms counted.

## Resource behavior

- [ ] relation candidate count bounded per file.
- [ ] relation target bytes bounded.
- [ ] normalization traversal bounded.
- [ ] over-budget file emits no partial relations.
- [ ] Phase 1 symbols survive relation-specific limit failure.
- [ ] graph entities globally capped.
- [ ] graph relations globally capped.
- [ ] traversal depth capped.
- [ ] traversal result size capped.
- [ ] truncation explicit.

## Graph

- [ ] forward adjacency works.
- [ ] reverse adjacency works.
- [ ] unresolved relations are not reverse-resolved.
- [ ] self-loops preserved.
- [ ] cycles safe.
- [ ] deterministic relation sorting/IDs.
- [ ] distinct occurrences preserved.

## CLI

- [ ] `ctx graph` works.
- [ ] `ctx deps` works.
- [ ] `ctx callers` works.
- [ ] `ctx related` works.
- [ ] path defaults consistent with prior commands.
- [ ] `--file` disambiguation works.
- [ ] depth behavior works.
- [ ] zero matches succeed cleanly.
- [ ] multiple roots remain grouped.

## JSON

- [ ] schema-v1 graph envelope.
- [ ] full graph serializable.
- [ ] evidence serialized.
- [ ] resolved/unresolved target state explicit.
- [ ] entity ID semantics documented.
- [ ] no source body.
- [ ] deterministic fixture output.
- [ ] prior JSON commands remain compatible.

## Architecture

- [ ] one scan path.
- [ ] graph mode uses same source/tree for symbols+relation candidates.
- [ ] Tree-sitter types do not leak.
- [ ] graph logic does not print.
- [ ] output does not parse/read repository source.
- [ ] no graph DB.
- [ ] no persistence.
- [ ] no concurrency.
- [ ] no Context Integrity policy types.
- [ ] no `unsafe`.

## Verification

- [ ] `./scripts/verify.sh` passes.
- [ ] `cargo check --all-targets` passes.
- [ ] formatter passes.
- [ ] Clippy warnings-denied passes.
- [ ] all tests pass.
- [ ] release build passes.
- [ ] real Rust repository smoke passes.
- [ ] real Python repository smoke passes.

## Documentation

- [ ] README accurately describes syntactic graph limitations.
- [ ] `callers` explicitly labeled best-effort/syntactic.
- [ ] current state updated truthfully.
- [ ] Phase 2 implementation report created.
- [ ] deviations recorded.

---

# 51. Quality Gate

Required:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
```

The canonical verifier should already cover:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Then smoke:

```bash
./target/release/ctx graph .
./target/release/ctx graph . --format json

./target/release/ctx deps <KNOWN_SYMBOL> .
./target/release/ctx callers <KNOWN_SYMBOL> .
./target/release/ctx related <KNOWN_SYMBOL> .
```

Run analogous graph/index commands on a Python-heavy repository.

Do not claim a verification step that did not run.

---

# 52. Phase 2 Implementation Report

Create:

```text
docs/reports/phase-2-implementation-report.md
```

Required sections:

## 1. Summary

What Phase 2 actually ships.

## 2. Baseline / acceptance gate

Record:

- Phase 1 final disposition;
- baseline commit;
- baseline verifier.

## 3. Graph model

Document:

- entity identity;
- relation identity;
- ID stability semantics;
- evidence;
- resolution.

## 4. Relationship semantics

Exact shipped behavior for:

- containment;
- Rust imports;
- Python imports;
- references;
- callers.

## 5. Resolution rules

Document:

- same-file only;
- qualified/local rules;
- ambiguity;
- unresolved state.

## 6. Resource limits

Record final constants and fail-closed behavior.

## 7. Code organization

Explain graph/analyzer/query/output modules.

## 8. Deviations

Every material deviation from this plan.

## 9. Verification

Commands/results.

## 10. Real repository observations

Counts + local smoke timings.

## 11. Known limitations

Be explicit about syntax vs compiler semantics.

## 12. Phase 3 handoff

Answer Section 54.

## 13. Rust learning notes

Cover:

- IDs;
- adjacency;
- traversal;
- cycles;
- ownership;
- deterministic output.

---

# 53. Post-Implementation Review Focus

Independent Phase 2 review should scrutinize:

1. graph identity not based on names;
2. evidence/derivation honesty;
3. same-file resolution false-certainty risks;
4. containment semantics;
5. import normalization correctness;
6. relation resource amplification;
7. graph/traversal limits;
8. cycles/self-loops;
9. deterministic graph serialization;
10. no hidden O(N²) behavior;
11. source/tree same-pass invariant;
12. Phase 0/1 regressions;
13. public wording around "callers" and "dependencies";
14. whether graph structure is suitable for Phase 3 queries.

---

# 54. Phase 3 Handoff Questions

At Phase 2 completion, answer:

1. Is `EntityId` sufficient as an index-local identifier?
2. Does Phase 3 need a richer external `EntityRef`?
3. Are containment semantics useful enough for query neighborhoods?
4. Which unresolved import targets are most valuable to resolve next?
5. Should Phase 3 add Rust/Python module resolution or keep that separate?
6. How many call references resolved under same-file rules on real repos?
7. What false/ambiguous patterns were observed?
8. Should `references` expand beyond call sites?
9. Do query commands need file/entity/path search?
10. Are `deps`, `callers`, and `related` sufficiently useful or mostly foundation?
11. Is a centrality/hotspot layer now justified?
12. Did custom adjacency outperform the need for a graph crate?
13. Are graph entity/relation caps reasonable?
14. Are traversal budgets reasonable?
15. Is TypeScript more valuable now than richer query intelligence?

The default roadmap says:

> Phase 3 improves query/repository intelligence before Git change analysis.

---

# 55. Known Phase 2 Limitations That Are Acceptable

The following are expected and should not be "fixed" by guessing:

- imports unresolved to files;
- cross-file call/reference resolution absent;
- dynamic Python calls unresolved;
- Rust trait/method dispatch unresolved;
- `self.method()` / `Self::method()` generally unresolved;
- field-expression calls unresolved;
- syntax-local qualified names remain display evidence;
- graph IDs not stable across edits;
- no persistence;
- graph rebuilt each command;
- no atomic filesystem snapshot;
- only Rust/Python analyzers;
- malformed Tree-sitter syntax is not compiler validity;
- generated/vendor files remain skipped.

---

# 56. Suggested Commit Sequence

Recommended:

```text
1. add Phase 2 graph/evidence domain types
2. add graph analysis mode and relation budgets
3. add Rust import/reference extraction
4. add Python import/reference extraction
5. add deterministic entity construction
6. add containment graph
7. add relation source ownership and same-file resolution
8. add relation construction + forward/reverse adjacency
9. add cycle-safe traversal primitives
10. add graph command + JSON
11. add deps/callers/related commands
12. add hostile-input/resource/security coverage
13. preserve Phase 0/1 regression suite
14. update docs + Phase 2 implementation report
15. run final verifier/release smoke
```

Avoid a single monolithic commit if the implementation workflow supports clean intermediate commits.

---

# 57. Definition of Done for Learning

Before moving to Phase 3, the implementer should be able to explain:

1. why `qualified_name` is not an entity ID;
2. how graph IDs are assigned and why they are not stable across edits;
3. why unresolved imports remain graph facts;
4. difference between extraction derivation and resolution quality;
5. why same-file local resolution is heuristic;
6. how containment is built without quadratic pairwise comparison;
7. why Rust impl methods are not lexically contained by the struct declaration;
8. how reverse adjacency is constructed;
9. why unresolved edges have no reverse entity target;
10. how cycle-safe BFS/DFS works;
11. why self-loops are retained;
12. what graph/traversal budgets prevent;
13. why Phase 2 does not use a graph database;
14. why symbol+relation extraction should share the same source/tree in graph mode;
15. what would be required for real cross-file/compiler resolution.

---

# 58. Final Phase 2 Product Shape

At completion:

```text
                        repository
                            │
                            ▼
                    canonical scanner
                            │
                            ▼
                 Rust/Python analyzer
                   symbols + relation
                       candidates
                            │
                            ▼
                  deterministic entities
                            │
                    ┌───────┴───────┐
                    ▼               ▼
               containment      resolution
                    │               │
                    └───────┬───────┘
                            ▼
                     RepositoryGraph
                   forward + reverse
                            │
           ┌────────────────┼────────────────┐
           ▼                ▼                ▼
       ctx graph         ctx deps       ctx callers
                                             │
                                             ▼
                                        ctx related
```

Phase 2 succeeds when Context Engine can truthfully answer:

> **What repository/file/symbol entities exist, what lexical/import/call-reference relationships are observable, which relationship targets can be resolved under explicitly limited syntactic rules, and what evidence supports each edge?**

It should still refuse to imply:

> **the compiler/runtime guarantees these relationships.**

That semantic honesty is foundational for every later phase.
