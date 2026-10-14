# Codebase / Context Intelligence Engine — Phase 1 Implementation Plan

**Phase:** 1 — Syntax and Symbol Index  
**Status:** Planned; implementation gated on Phase 0 closure  
**Date:** 2026-10-14  
**Primary language:** Rust  
**Binary:** `ctx`  
**Required language analyzers:** Rust + Python  
**Prerequisites:** `docs/prd.md`, `docs/architecture.md`, completed/re-reviewed Phase 0  
**Planning baseline:** repository `jasonkli21/context-engine`, main after Phase 0 review-fix commit `47bc1bd0ac051f0647986bbd34e2a643821a388c`

---

## 1. Phase Objective

Phase 1 turns the Phase 0 repository inventory into the first true code-intelligence layer.

The engine should be able to parse supported source files, extract a deterministic inventory of source symbols, preserve source locations, tolerate malformed code, and expose that information through:

```bash
ctx index [PATH]
ctx symbols [PATH]
ctx symbol <NAME> [PATH]
```

Phase 1 remains:

- local;
- synchronous;
- read-only;
- in-memory;
- deterministic;
- independent of LLMs;
- independent of Context Integrity policy.

The central product transition is:

```text
Phase 0
files + metadata
      ↓
Phase 1
syntax + symbols + source evidence
```

The central architecture transition is:

```text
RepositoryScan
      ↓
bounded source loading
      ↓
language analyzer
      ↓
Tree-sitter syntax tree
      ↓
language-neutral Symbol records
      ↓
RepositoryIndex
      ↓
human / versioned JSON output
```

Phase 1 deliberately does **not** create the repository relationship graph. Imports, references, calls, dependency edges, `contains` graph edges, impact analysis, persistence, incremental indexing, and agent-facing retrieval remain later work.

---

# 2. Implementation Gate: Phase 0 Must Be Closed First

At planning time, the Phase 0 review findings have been addressed locally, but `docs/current-state.md` still says independent re-review is pending.

Phase 1 implementation must not begin until the repository's current canonical state explicitly indicates that Phase 0 is accepted/closed and safe to build upon.

Before Phase 1 implementation:

1. Phase 0 review-fix commit must be independently re-reviewed.
2. Any remaining blocking findings must be resolved.
3. `./scripts/verify.sh` must pass.
4. `docs/current-state.md` must no longer describe Phase 0 as awaiting required re-review/fixes.
5. The Phase 0 trust-boundary fixes must remain intact.

If those conditions are not true, stop before editing Phase 1 code.

This plan can exist in the repository before that gate is cleared.

---

# 3. Phase 1 Product Goals

By the end of Phase 1, the engine should:

1. identify source symbols from Rust files;
2. identify source symbols from Python files;
3. represent symbols with language-neutral domain types;
4. preserve deterministic source ranges and name ranges;
5. produce syntax diagnostics without failing the entire repository;
6. keep unsupported languages visible as skipped analysis inputs rather than pretending they do not exist;
7. expose symbol-index summaries;
8. list all extracted symbols;
9. query exact symbol names deterministically;
10. provide versioned JSON for all new commands;
11. preserve all Phase 0 commands and contracts;
12. create a clean extension seam for Phase 2 graph relationships.

---

# 4. Rust Learning Objectives

Phase 1 is the first parser integration phase.

The implementation should provide practical experience with:

- integrating a native/parser library safely;
- Tree-sitter `Parser`, `Tree`, `Node`, `Query`, and `QueryCursor`;
- converting library-specific syntax objects into project domain types;
- traits where there are now genuinely multiple implementations;
- source ranges and byte offsets;
- borrowing source text while constructing owned domain results;
- error boundaries between fatal configuration errors and recoverable file errors;
- deterministic collection/sorting;
- module organization;
- testing parser fixtures;
- avoiding library lifetime types in durable domain models.

This phase should **not** use advanced Rust complexity merely to demonstrate it.

In particular, avoid:

- `unsafe`;
- async;
- threads;
- `Arc`/`Mutex`;
- self-referential data structures;
- long-lived Tree-sitter node lifetimes;
- generic abstraction layers that exist only for imagined future backends.

---

# 5. User Stories

## US-1.1 — Build a syntax/symbol index

```bash
ctx index .
```

returns a concise summary containing:

- files discovered;
- files analyzable;
- files analyzed;
- files skipped and why;
- files with syntax errors;
- symbols extracted;
- symbols by language;
- symbols by kind.

Important:

> In Phase 1, `index` means "build a fresh in-memory index for this command invocation." It does **not** persist anything.

Persistent indexes remain Phase 6.

## US-1.2 — List symbols

```bash
ctx symbols .
```

lists deterministic symbol records with at least:

- language;
- kind;
- qualified name;
- relative file path;
- source location.

## US-1.3 — Inspect a symbol

```bash
ctx symbol RepositoryScan .
```

returns exact matching symbol records.

## US-1.4 — Query a qualified symbol

```bash
ctx symbol 'RepositoryScan::some_method' .
```

returns qualified-name matches before falling back to local-name matches.

## US-1.5 — Analyze malformed source

A Python/Rust file containing syntax errors:

- does not crash the repository analysis;
- records syntax diagnostics;
- extracts symbols from valid portions where Tree-sitter can recover them.

## US-1.6 — Analyze a non-Git directory

All Phase 1 commands work without Git.

Git metadata remains optional evidence.

## US-1.7 — Keep unsupported files visible

A TypeScript or Markdown file in the repository is not parsed in Phase 1, but the index summary accounts for it as unsupported/skipped rather than implying the repository contained only Rust/Python.

## US-1.8 — Consume deterministic JSON

```bash
ctx symbols . --format json
```

returns stable, versioned machine-readable output with no source bodies.

---

# 6. Phase Scope

## 6.1 In Scope

- Tree-sitter integration;
- Rust grammar;
- Python grammar;
- bounded source reads;
- UTF-8 source handling;
- syntax tree construction;
- project-owned symbol queries;
- language-neutral symbol model;
- source ranges;
- name ranges;
- best-effort syntax-local qualified names;
- syntax diagnostics;
- fresh in-memory repository index;
- `ctx index`;
- `ctx symbols`;
- `ctx symbol`;
- human output;
- JSON schema v1 output for new commands;
- deterministic ordering;
- parser/unit/fixture/CLI tests;
- README/current-state/report updates;
- Phase 0 regression preservation.

## 6.2 Explicitly Out of Scope

Do **not** implement:

- imports as relations;
- references;
- call graphs;
- dependency graphs;
- `contains` graph edges;
- reverse lookup/callers;
- `ctx deps`;
- `ctx callers`;
- `ctx related`;
- graph databases;
- `petgraph` unless Phase 2 explicitly chooses it;
- Git structural diffs;
- symbol history/lineage;
- stable identity across refactors;
- persistence;
- index files on disk;
- SQLite;
- Tantivy;
- caching;
- incremental parsing between CLI invocations;
- filesystem watches;
- concurrency;
- Rayon;
- Tokio;
- daemon/server;
- MCP;
- editor protocol support;
- LSP/compiler resolution;
- SCIP;
- embeddings/vector search;
- fuzzy semantic search;
- LLM/model calls;
- Context Integrity policy/claims/drift logic;
- cross-repository analysis;
- TypeScript/JavaScript parsing;
- Python non-UTF-8 encoding-cookie support;
- source code previews/signatures in stable output.

Phase 1 is a **symbol inventory phase**, not a graph/query platform phase.

---

# 7. Architectural Decisions for Phase 1

## 7.1 Reuse Phase 0 discovery

Phase 1 must not introduce a second filesystem walker.

Use:

```text
scan_repository(...)
```

as the canonical repository discovery path.

Phase 1 consumes `RepositoryScan.files`.

This preserves:

- ignore semantics;
- symlink semantics;
- relative paths;
- warnings;
- language classification;
- role classification;
- Git behavior.

## 7.2 No line counting during index commands

Phase 1 index commands need source contents for parsing but do not need Phase 0 physical line-count computation.

When constructing scan options for:

```text
index
symbols
symbol
```

use:

```text
count_lines = false
```

This avoids reading each source twice for two unrelated purposes.

## 7.3 Optional Git metadata remains useful

Phase 1 should normally retain `probe_git = true` for index/symbol commands so output can identify:

- HEAD commit when available;
- branch;
- dirty state where safe/available.

Do not treat HEAD as proof that working-tree source equals that commit.

The analyzed truth is:

> current filesystem contents at scan/read time, accompanied by optional Git metadata.

## 7.4 Language analyzer abstraction is now justified

Phase 0 correctly avoided a language-analyzer trait.

Phase 1 has two real implementations:

- Rust;
- Python.

A small trait is now justified.

Recommended conceptual interface:

```rust
pub trait LanguageAnalyzer {
    fn language(&self) -> Language;

    fn analyze(
        &mut self,
        source: &SourceFile<'_>,
    ) -> Result<AnalyzedFile, AnalysisError>;
}
```

Exact method signatures may vary.

Requirements:

- no CLI dependency;
- no output formatting;
- no filesystem traversal;
- no Context Integrity semantics;
- no Tree-sitter types in returned durable domain structures.

## 7.5 Tree-sitter types stay inside analysis modules

`tree_sitter::Tree`, `Node`, `Point`, query captures, and parser-specific node kinds must not become public domain/API types.

Convert immediately to:

- `SourcePoint`;
- `SourceRange`;
- `Symbol`;
- analysis diagnostics.

This preserves the ability to use another backend later.

## 7.6 Project-owned symbol queries

Use minimal project-owned Tree-sitter query files, for example:

```text
src/language/queries/rust-symbols.scm
src/language/queries/python-symbols.scm
```

Embed them at compile time with `include_str!`.

Why project-owned queries:

- symbol semantics belong to this project;
- grammar-provided tags queries may contain broader definitions/references than Phase 1 needs;
- output should not silently change because an upstream tags query changes;
- query fixtures are easy to review.

Grammar-provided `NODE_TYPES` / `TAGS_QUERY` may be used as references while authoring queries.

## 7.7 No syntax tree persistence

A Tree-sitter syntax tree exists only while analyzing the file.

Store:

- extracted symbols;
- diagnostics;
- ranges.

Do not store `Tree` objects in `RepositoryIndex`.

Incremental tree reuse belongs later.

---

# 8. Proposed Repository Layout

Keep one package / one binary.

Recommended Phase 1 additions:

```text
src/
├── main.rs
├── cli.rs
├── classify.rs
├── git.rs
├── model.rs
├── output.rs
├── scan.rs
├── index.rs
├── source.rs
└── language/
    ├── mod.rs
    ├── rust.rs
    ├── python.rs
    └── queries/
        ├── rust-symbols.scm
        └── python-symbols.scm
```

Tests:

```text
tests/
├── cli.rs
├── fixtures/
│   └── ...
└── symbol_fixtures/
    ├── rust/
    │   ├── basic.rs
    │   ├── nested.rs
    │   ├── unicode.rs
    │   └── malformed.rs
    └── python/
        ├── basic.py
        ├── nested.py
        ├── unicode.py
        └── malformed.py
```

Exact fixture placement may be adjusted to fit existing repository conventions.

Do not create a multi-crate workspace.

---

# 9. Dependency Plan

Add only parser dependencies required by Phase 1.

Required categories:

```toml
tree-sitter = "compatible current stable version"
tree-sitter-rust = "compatible current stable version"
tree-sitter-python = "compatible current stable version"
```

Use mutually compatible current stable versions at implementation time and let `Cargo.lock` record the resolved set.

Do not blindly copy versions from this plan.

The current Tree-sitter Rust API exposes a `LANGUAGE` constant from the Rust/Python grammar crates, a `Parser`, source `Node` ranges, and `QueryCursor`; the implementation should use the current supported APIs rather than deprecated ones.

No new dependency is automatically approved for:

- graphing;
- persistence;
- async;
- concurrency;
- source execution.

If query iteration requires importing a trait re-exported by `tree-sitter`, use the re-export rather than adding a redundant direct dependency unless necessary.

---

# 10. Phase 1 Domain Model

Exact field names can evolve, but these semantics should remain.

## 10.1 Parse-size constant

Recommended:

```rust
pub const DEFAULT_MAX_PARSE_BYTES: u64 = 1024 * 1024;
```

Phase 1 should bound source reads.

The default may be adjusted after real-repository smoke tests, but the limit must exist.

## 10.2 `IndexOptions`

```rust
pub struct IndexOptions {
    pub root: PathBuf,
    pub max_parse_file_bytes: u64,
    pub probe_git: bool,
}
```

Do not reuse line-count settings as parse settings.

## 10.3 `SourcePoint`

```rust
pub struct SourcePoint {
    pub byte: usize,
    pub row: usize,
    pub column: usize,
}
```

Semantics:

- `byte` is UTF-8 byte offset from start of file;
- `row` is zero-based;
- `column` is zero-based byte column, matching Tree-sitter semantics.

## 10.4 `SourceRange`

```rust
pub struct SourceRange {
    pub start: SourcePoint,
    pub end: SourcePoint,
}
```

Semantics:

- byte interval is half-open `[start.byte, end.byte)`;
- end row/column follows Tree-sitter's end position;
- domain type does not depend on Tree-sitter.

Human output may convert row/column to one-based display.

JSON must document zero-based machine semantics.

## 10.5 `SymbolKind`

Required Phase 1 variants:

```rust
pub enum SymbolKind {
    Module,
    Class,
    Struct,
    Enum,
    Union,
    Trait,
    Function,
    Method,
    TypeAlias,
    Constant,
    Static,
}
```

Do not add speculative variants unless a supported-language fixture requires them.

Macro definitions may be deferred.

## 10.6 `Symbol`

Recommended:

```rust
pub struct Symbol {
    pub relative_path: PathBuf,
    pub language: Language,
    pub kind: SymbolKind,
    pub name: String,
    pub qualified_name: String,
    pub range: SourceRange,
    pub name_range: SourceRange,
}
```

Do not add a globally stable `SymbolId` in Phase 1.

Identity for sorting/query purposes is currently the tuple of:

```text
relative path
+ kind
+ name range
+ qualified name
```

Phase 2 will revisit entity IDs for graph construction.

## 10.7 `AnalysisStatus`

Recommended:

```rust
pub enum AnalysisStatus {
    Analyzed,
    UnsupportedLanguage,
    SkippedRole,
    TooLarge,
    InvalidUtf8,
    ReadFailed,
    ParseFailed,
}
```

Names may differ.

Each scanned file should have an explicit analysis status.

This prevents the index summary from pretending unsupported/skipped files did not exist.

## 10.8 `FileAnalysis`

Recommended:

```rust
pub struct FileAnalysis {
    pub relative_path: PathBuf,
    pub language: Language,
    pub role: FileRole,
    pub status: AnalysisStatus,
    pub symbol_count: usize,
    pub syntax_error_count: usize,
}
```

Do not store source contents.

## 10.9 `AnalysisDiagnostic`

Recommended conceptual fields:

```rust
pub struct AnalysisDiagnostic {
    pub relative_path: PathBuf,
    pub kind: DiagnosticKind,
    pub message: String,
    pub range: Option<SourceRange>,
}
```

Required diagnostic categories should be small, for example:

```text
SyntaxError
MissingSyntax
ReadError
InvalidUtf8
ParseFailure
```

Unsupported/too-large/generated/vendor files can be represented by `AnalysisStatus` without producing noisy diagnostics.

## 10.10 `RepositoryIndex`

Recommended:

```rust
pub struct RepositoryIndex {
    pub root: PathBuf,
    pub git: Option<GitMetadata>,
    pub scan_warnings: Vec<ScanWarning>,
    pub files: Vec<FileAnalysis>,
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}
```

All collections must have deterministic ordering before rendering.

---

# 11. Source Loading and Trust Boundary

Phase 1 reads full source contents, so this is an important extension of the Phase 0 trust boundary.

## 11.1 Source files are untrusted input

The indexer must not:

- import Python modules;
- compile Rust code;
- run `cargo`;
- execute `build.rs`;
- run `python`;
- invoke language servers;
- evaluate project config;
- execute shebangs;
- load plugins from the target repository;
- use network access.

Only parse source bytes with trusted compiled Tree-sitter grammar dependencies.

## 11.2 Analyze only Phase 0 discovered files

Never accept a path from syntax content and then open it.

The source loader receives:

```text
canonical scan root
+ scan-produced repository-relative path
```

Validate the relative path before joining.

Reject unexpected:

- absolute paths;
- parent traversal;
- path escaping the root.

## 11.3 Revalidate file type before reading

Between scan and parse, filesystem state can change.

Before reading:

1. inspect the target with no intentional symlink traversal;
2. ensure it is still a regular file;
3. ensure it remains within the expected scan-root path model;
4. enforce the parse-size limit;
5. read at most `max + 1` bytes so file growth cannot bypass the limit.

If state changed:

- mark the file read/analysis failure;
- record a relative-path diagnostic;
- continue the repository analysis where safe.

Do not add `unsafe` or an OS-specific sandbox framework in Phase 1.

If a residual platform-level TOCTOU limitation remains, document it rather than claiming perfect atomic snapshot semantics.

## 11.4 UTF-8 policy

Phase 1 supports UTF-8 Rust/Python source.

If a supported-language file is not valid UTF-8:

- do not parse it lossy;
- mark `InvalidUtf8`;
- record a bounded diagnostic;
- continue.

Python encoding-cookie decoding is out of scope.

## 11.5 Parse-size limit

Before and during read:

```text
size <= max_parse_file_bytes
```

Files larger than the threshold get:

```text
AnalysisStatus::TooLarge
```

No diagnostic body or source preview is emitted.

---

# 12. Analyzer Registry

Create a small language registry.

Conceptual:

```text
AnalyzerRegistry
├── RustAnalyzer
└── PythonAnalyzer
```

Required behavior:

```rust
registry.analyzer_for(&Language::Rust)   -> Some(...)
registry.analyzer_for(&Language::Python) -> Some(...)
registry.analyzer_for(&Language::TypeScript) -> None
```

The registry can use:

- trait objects;
- an enum;
- another simple implementation.

Prefer the clearest ownership model.

A trait is appropriate because there are two real analyzer implementations.

Avoid a plugin framework.

---

# 13. Analyzer Result Boundary

A language analyzer consumes a source file and returns language-neutral results.

Conceptual:

```rust
pub struct AnalyzedFile {
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub syntax_error_count: usize,
}
```

Fatal initialization errors:

- grammar cannot be assigned to parser;
- embedded project-owned query does not compile;
- incompatible grammar/parser ABI.

These are implementation/dependency failures and should fail command execution clearly.

Recoverable per-file problems:

- invalid UTF-8;
- source changed/unreadable;
- Tree-sitter recoverable syntax errors;
- parse failure for one input.

Do not silently emit an incomplete symbol list when the analyzer itself failed.

---

# 14. Tree-sitter Query Contract

Use one minimal symbol query per supported language.

Recommended capture convention:

```text
@symbol.struct
@symbol.function
@symbol.method
@symbol.class
...
@name
```

For each query match:

- exactly one declaration capture identifies the whole symbol node;
- exactly one name capture identifies the name node.

If a project query violates this expectation:

- treat it as a programmer/query bug;
- catch it in analyzer tests.

Do not depend on capture order without validating capture names.

After query execution:

1. create symbol kind from capture type;
2. obtain exact name bytes from the `@name` node;
3. derive source/name ranges;
4. derive qualified name from syntax-local ancestry;
5. create owned `Symbol`.

Set/check a reasonable query match limit if the Tree-sitter API makes that practical.

If the query engine reports that matches were truncated/exceeded:

- do not silently accept partial results;
- treat file analysis as failed or produce an explicit diagnostic according to the cleanest API.

---

# 15. Rust Symbol Extraction Requirements

Required Rust symbols:

## Module

Examples:

```rust
mod parser;
mod inline {
}
```

Output kind:

```text
Module
```

## Struct

```rust
struct RepositoryScan {
}
```

Output kind:

```text
Struct
```

## Enum

```rust
enum Language {
}
```

Output:

```text
Enum
```

## Union

If present in grammar/fixtures:

```text
Union
```

## Trait

```rust
trait LanguageAnalyzer {
}
```

Output:

```text
Trait
```

## Function

Top-level or module/nested function:

```rust
fn scan_repository(...) { ... }
```

Output:

```text
Function
```

unless syntax ancestry makes it a method.

## Method

Functions/signatures inside:

- `impl`;
- trait bodies.

Examples:

```rust
impl RepositoryScan {
    fn files(&self) { ... }
}
```

and:

```rust
trait LanguageAnalyzer {
    fn analyze(&mut self, ...);
}
```

Output:

```text
Method
```

## Type alias

```rust
type SymbolId = ...;
```

Output:

```text
TypeAlias
```

## Constant

```rust
const MAX: usize = ...;
```

Output:

```text
Constant
```

## Static

```rust
static VALUE: ...;
```

Output:

```text
Static
```

### Rust raw identifiers

Where supported by grammar, extraction should not crash on raw identifiers such as:

```rust
fn r#type() {}
```

The exact displayed name should be consistent with the syntax node text and covered by a fixture if practical.

---

# 16. Python Symbol Extraction Requirements

Required Python symbols:

## Class

```python
class Router:
    ...
```

Output:

```text
Class
```

## Function

Module-level or nested function:

```python
def route(...):
    ...
```

Output:

```text
Function
```

unless inside a class.

## Method

A function defined lexically within a class:

```python
class Router:
    def route(self):
        ...
```

Output:

```text
Method
```

Async forms should be handled if the grammar represents them through the same definition node or a straightforward adjacent pattern.

## Decorated definitions

Decorators must not hide a definition:

```python
@decorator
def route():
    ...
```

and:

```python
@dataclass
class Config:
    ...
```

should still produce the function/class symbol.

## Nested definitions

Nested classes/functions should produce deterministic qualified names.

---

# 17. Qualified Name Rules

Qualified names in Phase 1 are **syntax-local**, not fully compiler/import resolved.

This limitation must be explicit.

## 17.1 Rust

Separator:

```text
::
```

Examples:

```text
parse
parser::parse
RepositoryScan::files
LanguageAnalyzer::analyze
outer::inner
```

Sources of container qualification:

- inline module ancestry;
- trait ancestry;
- `impl` target, best effort;
- nested function ancestry where unambiguous.

For an `impl<T> SomeType<T>` target, a compact source-text form may be used.

Do not build package/module resolution across files yet.

For example, a function in `src/foo.rs` does not automatically become `crate::foo::function` unless syntax in that file provides the container.

## 17.2 Python

Separator:

```text
.
```

Examples:

```text
route
Router.route
outer.inner
Outer.Inner.method
```

Do not infer import/package module paths from filesystem layout in Phase 1.

## 17.3 Ambiguity is allowed

Multiple symbols may have the same local or qualified name.

Do not deduplicate legitimate duplicate definitions.

Deterministic path/range ordering resolves display order.

---

# 18. Syntax Error Handling

Tree-sitter can create useful trees even when source contains errors.

Requirements:

1. `root_node.has_error()` does not make the repository command fail.
2. Traverse error/missing syntax nodes to count syntax issues.
3. Extract symbols from the recoverable tree.
4. Record bounded diagnostics.

Recommended diagnostic cap:

```text
20 detailed syntax diagnostics per file
```

Still record the full count if cheaply available.

This prevents one malformed/malicious file from exploding output size.

Human `index` output should show counts, not dump all diagnostics by default.

JSON may include bounded diagnostic records.

---

# 19. Index Orchestration

Recommended data flow:

```text
IndexOptions
    ↓
scan_repository(
    count_lines = false,
    probe_git = true
)
    ↓
RepositoryScan
    ↓
for each FileRecord
    ├── analyzer supported?
    │      no → UnsupportedLanguage
    │
    ├── generated/vendor?
    │      yes → SkippedRole
    │
    ├── over parse size?
    │      yes → TooLarge
    │
    ├── safe bounded source read
    │      fail → ReadFailed / InvalidUtf8
    │
    └── analyzer.analyze(...)
           ├── symbols
           ├── syntax diagnostics
           └── FileAnalysis
    ↓
sort symbols + files + diagnostics
    ↓
RepositoryIndex
```

Analyze supported languages in:

- source files;
- test files;
- migration/config/etc. files if they happen to be Rust/Python;

but skip roles explicitly classified as:

- `Generated`;
- `Vendor`.

Do not silently skip all non-`Source` roles.

---

# 20. Deterministic Ordering

## Files

Sort:

```text
relative path ascending
```

## Symbols

Sort by:

1. relative path ascending;
2. `range.start.byte` ascending;
3. `range.end.byte` ascending;
4. symbol kind stable name;
5. qualified name.

This keeps symbols in source order within files.

## Diagnostics

Sort by:

1. relative path;
2. optional range start byte;
3. diagnostic kind;
4. message.

Do not depend on query-cursor incidental ordering as the external contract.

---

# 21. CLI Contract

## 21.1 `ctx index`

```bash
ctx index [PATH] [--format human|json] [--max-parse-file-size BYTES]
```

Defaults:

```text
PATH = .
format = human
max parse size = DEFAULT_MAX_PARSE_BYTES
```

Help text must say this is a **fresh in-memory** symbol index.

### Human output

Example:

```text
Repository index: /path/to/repo

Files
  Discovered:              214
  Supported language:       91
  Analyzed:                 88
  Unsupported language:    113
  Skipped generated/vendor:  4
  Too large:                 2
  Read/encoding failures:    1
  Syntax-error files:        3

Symbols
  Total:                    742

By language
  Rust                      691
  Python                     51

By kind
  Function                  312
  Method                    204
  Struct                     61
  ...
```

Optional Git section may reuse Phase 0 conventions.

No source body.

## 21.2 `ctx symbols`

```bash
ctx symbols [PATH] [--format human|json] [--max-parse-file-size BYTES]
```

Human example:

```text
rust    struct    RepositoryScan          src/model.rs:12:1
rust    function  scan_repository         src/scan.rs:24:1
python  class     Router                  tools/router.py:8:1
python  method    Router.route            tools/router.py:12:5
```

The exact column layout may evolve.

Requirements:

- stable order;
- human rows/columns one-based;
- escape repository-controlled control characters;
- no source preview.

## 21.3 `ctx symbol`

```bash
ctx symbol <NAME> [PATH] [--format human|json] [--max-parse-file-size BYTES]
```

Matching semantics:

1. case-sensitive exact `qualified_name`;
2. if no qualified-name match exists, case-sensitive exact local `name`;
3. no fuzzy/substr search in Phase 1;
4. multiple results are valid;
5. zero matches are a valid query result, not an operational failure.

Human output should clearly indicate:

```text
0 matches
1 match
N matches
```

No source preview.

---

# 22. JSON Output Contract

Existing Phase 0 JSON contracts must remain compatible.

New Phase 1 commands should use the same envelope concept:

```json
{
  "schema_version": 1,
  "command": "symbols",
  "data": {}
}
```

Adding new commands does not require changing Phase 0 schema v1.

Do not serialize internal structs automatically as the public contract.

Use DTOs.

## 22.1 Symbol JSON

Recommended shape:

```json
{
  "name": "scan_repository",
  "qualified_name": "scan_repository",
  "kind": "function",
  "language": "rust",
  "path": "src/scan.rs",
  "range": {
    "start": {
      "byte": 123,
      "row": 10,
      "column": 0
    },
    "end": {
      "byte": 456,
      "row": 24,
      "column": 1
    }
  },
  "name_range": {
    "start": {
      "byte": 126,
      "row": 10,
      "column": 3
    },
    "end": {
      "byte": 141,
      "row": 10,
      "column": 18
    }
  }
}
```

Document:

- row is zero-based;
- column is zero-based UTF-8 byte column;
- byte range is half-open.

## 22.2 `index` JSON

Recommended data sections:

```text
root
git
summary
languages
kinds
files
diagnostics
scan_warnings
```

No source text.

## 22.3 `symbols` JSON

Recommended:

```text
root
git
symbols
diagnostics summary or bounded diagnostics
scan_warnings
```

Avoid duplicating enormous unsupported-file detail if not needed.

## 22.4 `symbol` JSON

Recommended:

```text
root
query
match_mode
matches
diagnostics summary
```

`match_mode`:

```text
qualified_name
local_name
none
```

---

# 23. Error Semantics

Fatal command errors:

- invalid root / Phase 0 `ScanError`;
- analyzer initialization failure;
- incompatible parser grammar;
- invalid embedded query;
- internal output serialization error.

Recoverable per-file states:

- unsupported language;
- generated/vendor skip;
- too large;
- invalid UTF-8;
- read failure;
- recoverable parse/syntax errors.

The repository should still produce an index when some files are recoverably unavailable.

Do not use panic for arbitrary source input.

---

# 24. Work Breakdown

Implement in vertical slices.

---

## Step 1.0 — Phase 0 closure preflight

Before editing code:

1. read `docs/current-state.md`;
2. verify Phase 0 is closed/accepted;
3. inspect the final Phase 0 review/fix evidence;
4. run:

```bash
./scripts/verify.sh
```

If Phase 0 is not closed, stop.

Do not "fix Phase 0 while implementing Phase 1" unless the user explicitly changes scope.

---

## Step 1.1 — Add parser dependencies and empty language module

Add:

- Tree-sitter core;
- Rust grammar;
- Python grammar.

Create:

```text
src/language/mod.rs
src/language/rust.rs
src/language/python.rs
src/language/queries/...
```

At first:

- initialize parsers;
- compile project queries;
- add small unit tests proving parser/query initialization.

Do not wire CLI yet.

### Acceptance

- project compiles;
- existing Phase 0 tests still pass;
- Rust/Python analyzer initialization succeeds;
- invalid query test or compile-time fixture catches malformed query design if practical.

---

## Step 1.2 — Add language-neutral source range / symbol domain

Add:

- `SourcePoint`;
- `SourceRange`;
- `SymbolKind`;
- `Symbol`;
- file-analysis/status types;
- `RepositoryIndex`.

Keep Tree-sitter types out of domain structs.

Add unit tests for:

- ordering helpers;
- kind display names;
- range conversion where appropriate.

### Acceptance

Domain can represent Rust and Python symbols without parser-specific variants.

---

## Step 1.3 — Implement bounded source loader

Create `source.rs` or equivalent.

Responsibilities:

- validate repository-relative path;
- recheck regular-file/symlink state;
- enforce size;
- bounded read;
- UTF-8 validation.

Return owned source text only for the duration needed by analysis.

Do not retain source text in `RepositoryIndex`.

### Tests

- regular UTF-8 file;
- size boundary exactly at max;
- growth/over-max detection;
- invalid UTF-8;
- absolute/parent path rejection through direct unit test;
- symlink rejection where portable.

---

## Step 1.4 — Implement analyzer trait + registry

Add the minimum polymorphism needed for Rust/Python.

The registry should:

- return analyzer for supported `Language`;
- return none for unsupported languages;
- reuse parser/query objects within one command invocation when appropriate.

Do not create runtime plugins.

### Acceptance

Tests show:

```text
Rust -> Rust analyzer
Python -> Python analyzer
TypeScript -> unsupported
```

---

## Step 1.5 — Implement Rust analyzer

Build Rust symbol query/extraction.

Required symbol kinds:

- module;
- struct;
- enum;
- union where grammar supports it;
- trait;
- function;
- method;
- type alias;
- constant;
- static.

Implement qualified names.

### Rust fixtures

At minimum:

1. top-level declarations;
2. inline nested module;
3. impl methods;
4. trait method declaration + default method;
5. nested functions if grammar supports them cleanly;
6. generics;
7. attributes;
8. raw/unicode identifier edge case where legal;
9. malformed source with recoverable symbols.

### Assertions

For every expected symbol:

- kind;
- local name;
- qualified name;
- path;
- whole range;
- name range.

Verify `name_range` slices exactly to the symbol name bytes.

---

## Step 1.6 — Implement Python analyzer

Required:

- classes;
- functions;
- methods;
- decorated class/function;
- nested definitions;
- async definitions if straightforward.

### Python fixtures

At minimum:

1. module functions;
2. class methods;
3. nested class;
4. nested function;
5. decorators;
6. async function/method;
7. Unicode identifier;
8. malformed source with recoverable symbols.

Assert same evidence properties as Rust.

---

## Step 1.7 — Implement syntax diagnostics

Add helper to inspect the syntax tree for:

- error nodes;
- missing nodes.

Requirements:

- count complete syntax issues where reasonable;
- retain max 20 detail diagnostics/file;
- source ranges;
- deterministic order;
- no source body in messages.

Malformed source still returns recoverable symbols.

### Acceptance

Malformed Rust/Python fixture:

- `syntax_error_count > 0`;
- command succeeds;
- valid declaration(s) outside broken region remain present where Tree-sitter recovers them.

---

## Step 1.8 — Implement `index_repository`

Create:

```rust
pub fn index_repository(options: &IndexOptions) -> Result<RepositoryIndex, IndexError>;
```

Responsibilities:

1. call canonical Phase 0 scan;
2. choose supported files;
3. skip generated/vendor;
4. enforce size;
5. load source;
6. analyze;
7. accumulate statuses;
8. merge scan warnings;
9. sort deterministically.

It must not render/print.

### Acceptance

A mixed fixture contains:

- Rust;
- Python;
- TypeScript;
- Markdown;
- generated Rust/Python;
- malformed file;
- too-large file.

Index statuses/counts are correct.

---

## Step 1.9 — Add CLI commands

Extend `Command`:

```text
Index
Symbols
Symbol
```

Recommended shared args:

```text
path
format
max_parse_file_size
```

`Symbol` adds:

```text
name
```

Do not reuse `PathArgs` if doing so would couple parse and line-count options awkwardly.

A new `AnalysisArgs` / flattened args structure is justified.

### Acceptance

Help text makes ephemeral/in-memory semantics clear.

---

## Step 1.10 — Human `index` output

Add summary renderer.

Must include:

- discovered count;
- analyzed count;
- skip reasons;
- syntax-error files;
- symbol total;
- by-language symbol counts;
- by-kind counts;
- Git metadata when available;
- bounded warnings summary.

Do not dump all syntax diagnostics by default.

---

## Step 1.11 — Human `symbols` and `symbol` output

`symbols`:

- one deterministic row/block per symbol;
- human 1-based line/column;
- escaped control characters.

`symbol`:

- exact-match semantics;
- zero/multiple matches handled cleanly.

No source preview.

---

## Step 1.12 — JSON DTOs

Add explicit DTOs for:

- index;
- symbol;
- range;
- diagnostics/status summaries.

Preserve existing Phase 0 JSON.

Integration tests must deserialize output rather than only string-match it.

Run same command twice against a stable fixture and assert byte-identical JSON if no explicitly volatile Git state is included.

For deterministic fixture tests, use a non-Git fixture or controlled Git state.

---

## Step 1.13 — Security / trust regression tests

Required coverage:

### No source execution

Create source containing executable-looking code, e.g. Python top-level file write.

Indexing must not create the marker.

### No toolchain execution

No test should require:

- `python`;
- `cargo` on the target repo;
- language server;
- target build system.

### Symlink behavior

Phase 1 source loading must not intentionally follow file symlinks.

### Source output

Stable JSON/human symbol output must not contain arbitrary source bodies.

### Control characters

Repository-controlled path/name text rendered to human output must not permit raw terminal control injection.

---

## Step 1.14 — Phase 0 regression preservation

The implementation must keep:

```bash
ctx scan
ctx stats
ctx tree
```

working exactly as documented.

Especially preserve:

- tree does not read file contents or probe Git;
- ignore-control hardening;
- Git clean/process filter safety;
- warning-path semantics;
- schema-v1 Phase 0 output behavior.

Do not "simplify" Phase 0 code in a way that reopens reviewed defects.

---

## Step 1.15 — Real-repository smoke tests

Run release binary against:

1. `context-engine` itself;
2. at least one Python-heavy repository;
3. optionally one mixed repository.

Example commands:

```bash
./target/release/ctx index .
./target/release/ctx symbols .
./target/release/ctx symbol RepositoryScan .

./target/release/ctx index /path/to/python-project
./target/release/ctx symbols /path/to/python-project
```

Record:

- files discovered;
- files analyzed;
- symbols;
- syntax errors;
- unexpected skips;
- rough wall-clock time.

These are smoke observations, not benchmark claims.

---

## Step 1.16 — Documentation + implementation report

Update:

```text
README.md
docs/current-state.md
```

Create:

```text
docs/reports/phase-1-implementation-report.md
```

Do not edit PRD/architecture unless implementation reveals a genuine durable correction.

---

# 25. Test Matrix

| Area | Case | Type |
|---|---|---|
| analyzer init | Rust parser/query | unit |
| analyzer init | Python parser/query | unit |
| registry | supported/unsupported languages | unit |
| source | regular UTF-8 file | unit |
| source | exact size boundary | unit |
| source | over size | unit |
| source | invalid UTF-8 | unit |
| source | path traversal rejected | unit |
| source | symlink rejected | integration/platform |
| Rust | struct/enum/trait | fixture |
| Rust | free function | fixture |
| Rust | impl method | fixture |
| Rust | trait methods | fixture |
| Rust | module qualification | fixture |
| Rust | generics/attributes | fixture |
| Rust | malformed syntax | fixture |
| Python | class/function | fixture |
| Python | method | fixture |
| Python | nested definitions | fixture |
| Python | decorators | fixture |
| Python | async | fixture |
| Python | malformed syntax | fixture |
| ranges | name range exact | unit/fixture |
| ranges | Unicode byte positions | fixture |
| index | mixed supported/unsupported | integration |
| index | generated/vendor skip | integration |
| index | too-large skip | integration |
| index | partial read failure | integration |
| diagnostics | bounded malformed details | unit/integration |
| CLI | `index --help` | integration |
| CLI | `symbols --help` | integration |
| CLI | `symbol --help` | integration |
| CLI | human index | integration |
| CLI | human symbols | integration |
| CLI | exact qualified lookup | integration |
| CLI | local-name fallback | integration |
| CLI | zero matches | integration |
| JSON | schema version | integration |
| JSON | symbol/range shape | integration |
| JSON | deterministic order | integration |
| regression | scan/stats/tree | integration |
| security | Python code not executed | integration |
| security | no source bodies | integration |
| security | control escaping | integration |

---

# 26. Fixture Expectations

## Rust basic fixture

Include:

```rust
const DEFAULT: usize = 1;
static ENABLED: bool = true;

type Name = String;

struct Engine {}

enum Mode {
    Fast,
}

union Bits {
    value: u32,
}

trait Analyzer {
    fn analyze(&self);

    fn defaulted(&self) {}
}

impl Engine {
    fn run(&self) {}
}

fn top_level() {}
```

## Rust nested fixture

Include:

- inline modules;
- generic impl;
- nested function;
- trait impl.

## Python basic fixture

Include:

```python
def top_level():
    pass

class Router:
    def route(self):
        pass

    async def route_async(self):
        pass
```

## Python nested/decorated

Include:

- decorator;
- nested class;
- nested function;
- async definition.

## Malformed fixtures

Keep small and intentionally malformed.

The expected recovery behavior should be documented in fixture tests, not inferred from huge real code.

---

# 27. Exact Acceptance Criteria

Phase 1 is accepted only when all are true.

## Precondition

- [ ] Phase 0 independently closed/accepted before implementation began.
- [ ] Phase 0 canonical verification passed before Phase 1 changes.

## Architecture

- [ ] Phase 0 scan remains the only repository discovery path.
- [ ] parser logic does not walk filesystem.
- [ ] output logic does not parse source.
- [ ] CLI types do not become domain model.
- [ ] Tree-sitter types do not leak into durable domain/output models.
- [ ] Rust/Python analyzers share a language-neutral result contract.
- [ ] no Context Integrity policy types introduced.
- [ ] single package remains sufficient.
- [ ] no `unsafe`.

## Parsing

- [ ] Rust grammar works.
- [ ] Python grammar works.
- [ ] project-owned symbol queries compile.
- [ ] malformed files do not crash whole repository.
- [ ] syntax errors are counted.
- [ ] diagnostic details are bounded.
- [ ] parser/query truncation is not silently accepted.

## Symbols

- [ ] Rust modules extracted.
- [ ] Rust structs extracted.
- [ ] Rust enums extracted.
- [ ] Rust traits extracted.
- [ ] Rust functions extracted.
- [ ] Rust methods extracted.
- [ ] Rust type aliases extracted.
- [ ] Rust constants/statics extracted.
- [ ] Python classes extracted.
- [ ] Python functions extracted.
- [ ] Python methods extracted.
- [ ] nested/decorated Python definitions work.
- [ ] qualified names deterministic.
- [ ] duplicate legitimate names preserved.
- [ ] symbol whole range correct.
- [ ] symbol name range correct.

## Source safety

- [ ] only scan-produced relative paths analyzed.
- [ ] absolute/parent traversal rejected by source loader.
- [ ] generated/vendor files skipped by default.
- [ ] max parse size enforced before/while read.
- [ ] invalid UTF-8 is recoverable.
- [ ] source code is not executed/imported/compiled.
- [ ] source bodies are not emitted.
- [ ] source-file symlinks are not intentionally followed.

## CLI

- [ ] `ctx index [PATH]` works.
- [ ] `ctx symbols [PATH]` works.
- [ ] `ctx symbol <NAME> [PATH]` works.
- [ ] all default PATH to `.` where applicable.
- [ ] human output works.
- [ ] JSON output works.
- [ ] exact qualified-name matching works.
- [ ] exact local-name fallback works.
- [ ] zero matches succeeds cleanly.
- [ ] multiple matches deterministic.

## JSON

- [ ] `schema_version == 1` for new commands.
- [ ] existing Phase 0 schema remains compatible.
- [ ] symbol range semantics documented.
- [ ] paths repository-relative where expected.
- [ ] no raw source content.
- [ ] deterministic fixture output.

## Regression

- [ ] `ctx scan` remains correct.
- [ ] `ctx stats` remains correct.
- [ ] `ctx tree` remains correct and lightweight.
- [ ] Phase 0 security review fixes remain intact.

## Verification

- [ ] `./scripts/verify.sh` passes.
- [ ] `cargo check --all-targets` passes.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] Clippy with warnings denied passes.
- [ ] all tests pass.
- [ ] `cargo build --release` passes.
- [ ] release binary smoke-tested against Rust repo.
- [ ] release binary smoke-tested against Python repo.

## Documentation

- [ ] README accurately describes shipped Phase 1.
- [ ] README does not claim graphs/imports/references/persistence.
- [ ] `docs/current-state.md` updated truthfully.
- [ ] Phase 1 implementation report created.
- [ ] plan deviations recorded.

---

# 28. Quality Gate

Required repository canonical gate:

```bash
./scripts/verify.sh
```

Also ensure equivalents of:

```bash
cargo check --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

Then:

```bash
./target/release/ctx index .
./target/release/ctx symbols .
./target/release/ctx symbol RepositoryScan .
./target/release/ctx index . --format json
./target/release/ctx symbols . --format json
```

No check may be claimed if not actually run.

---

# 29. Performance Expectations

No hard SLA yet.

Phase 1 should remain responsive on ordinary personal repositories.

Rules:

- sequential;
- read one source file at a time;
- do not retain source text after extraction;
- bounded parse file size;
- reuse initialized parser/query per language within one invocation if cleanly possible;
- do not retain Tree-sitter trees after file analysis;
- no speculative parallelism.

Record smoke times.

Do not market them as benchmarks.

---

# 30. Security Review Checklist

Before completion, explicitly audit:

- [ ] no project code execution;
- [ ] no project config evaluation;
- [ ] no target build commands;
- [ ] no Python imports;
- [ ] no network access;
- [ ] no runtime query files loaded from target repo;
- [ ] parser queries are embedded/trusted project assets;
- [ ] file read size bounded;
- [ ] malformed source bounded in diagnostics;
- [ ] no source body in JSON;
- [ ] human repo-controlled strings escaped;
- [ ] Phase 0 symlink/ignore/Git hardening preserved;
- [ ] no `unsafe`.

---

# 31. Known Limitations to Document

Phase 1 may truthfully retain these limitations:

1. only Rust and Python parsed;
2. parsing is syntax-aware, not compiler/type resolved;
3. qualified names are syntax-local, not cross-file package resolved;
4. source identity is revision/path/range based, not stable through refactors;
5. no imports/references/call relationships;
6. no persistence;
7. no incremental parse reuse;
8. UTF-8 only;
9. large files skipped by configured limit;
10. Tree-sitter recovery may yield partial symbols from malformed code;
11. live filesystem is not an atomic snapshot.

These are expected, not Phase 1 defects.

---

# 32. Phase 2 Handoff Requirements

Phase 1 should leave Phase 2 with:

```text
FileRecord
Symbol
SourceRange
qualified names
per-file analysis status
optional Git metadata
deterministic symbol index
```

Phase 2 will add relationships:

```text
contains
imports
basic references
```

Do not prebuild those edges now.

At Phase 1 completion, answer in the implementation report:

1. Is `Symbol` stable enough to become a graph entity?
2. Does Phase 2 need an explicit `EntityId`?
3. Are qualified names reliable enough for display only vs identity?
4. Which symbol kinds need refinement?
5. Did Rust/Python extraction expose grammar-specific leakage?
6. Should containment edges be derived from ranges or directly during parse?
7. Should source evidence gain an explicit `EvidenceRef` type in Phase 2?
8. How often do malformed files produce misleading partial symbols?
9. Are parse-size defaults appropriate?
10. Is TypeScript the next useful analyzer, or should graph work come first?

The expected default is:

> graph work comes next; do not automatically add TypeScript before Phase 2.

---

# 33. Suggested Commit Sequence

A clean sequence:

```text
1. add Phase 1 parser dependencies and language module
2. add symbol/source-range domain model
3. add bounded source loader
4. add analyzer registry
5. add Rust symbol extraction + fixtures
6. add Python symbol extraction + fixtures
7. add syntax diagnostics
8. add repository index orchestration
9. add index/symbols/symbol CLI commands
10. add human output
11. add versioned JSON output
12. add security and mixed-repo integration coverage
13. preserve Phase 0 regressions and polish docs
14. write Phase 1 implementation report and run final verification
```

Do not make one giant commit if the environment/workflow allows coherent intermediate commits.

---

# 34. Phase 1 Implementation Report

Create:

```text
docs/reports/phase-1-implementation-report.md
```

Required sections:

## 1. Summary

What Phase 1 actually ships.

## 2. Preconditions / Phase 0 baseline

State:

- how Phase 0 closure was verified;
- baseline commit/state.

## 3. Implemented requirements

Map to:

- Rust analysis;
- Python analysis;
- symbol model;
- ranges;
- diagnostics;
- CLI;
- JSON;
- security.

## 4. Code organization

Explain:

- `index`;
- `source`;
- `language`;
- analyzers;
- query assets;
- output changes.

## 5. Symbol semantics

Document exact supported kinds and qualified-name rules.

## 6. Architecture decisions

Only decisions worth preserving.

## 7. Deviations

Every meaningful deviation from this plan.

## 8. Verification

Exact commands + outcomes.

## 9. Real-repo observations

Rust/Python smoke results.

## 10. Known limitations / debt

Real issues only.

## 11. Phase 2 handoff observations

Answer the questions in Section 32.

## 12. Rust learning notes

Explain useful lessons around:

- Tree-sitter lifetimes;
- owned vs borrowed source;
- trait boundary;
- error recovery;
- range conversion.

The implementation report is delivery evidence, not an independent review.

---

# 35. Post-Implementation Review Focus

The independent Phase 1 review should scrutinize:

1. source parser trust boundary;
2. Tree-sitter lifetime leakage;
3. symbol range correctness;
4. malformed-source semantics;
5. query completeness and duplication;
6. qualified-name correctness;
7. analyzer abstraction quality;
8. no hidden graph semantics;
9. output contract determinism;
10. Phase 0 regressions;
11. tests that exercise real CLI composition;
12. whether the symbol model is genuinely suitable for Phase 2.

---

# 36. Definition of Done for Learning

Before proceeding to Phase 2, the implementer should be able to explain:

1. why Tree-sitter nodes cannot be stored freely after their tree/source lifetime;
2. why the domain model owns strings/ranges instead;
3. how Rust and Python analyzers implement the same abstraction;
4. how syntax errors differ from analyzer failures;
5. why invalid UTF-8 is recoverable;
6. why file-size limits exist;
7. how a symbol's byte range differs from human line/column display;
8. why qualified names are not stable IDs;
9. why imports/references are deferred to Phase 2;
10. why no persistent index exists despite the `ctx index` command name;
11. how source parsing preserves the Phase 0 no-execution trust boundary;
12. what architectural change Phase 2 will introduce.

---

# 37. Final Phase 1 Product Shape

At completion:

```text
                       repository
                           │
                           ▼
                    Phase 0 scanner
                           │
                    discovered files
                           │
                           ▼
                   bounded source read
                           │
             ┌─────────────┴─────────────┐
             ▼                           ▼
       Rust analyzer                Python analyzer
             │                           │
             └─────────────┬─────────────┘
                           ▼
                 language-neutral
                     symbols/ranges
                           │
                           ▼
                    RepositoryIndex
                           │
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
     ctx index        ctx symbols      ctx symbol
```

Phase 1 succeeds when Context Engine can truthfully answer:

> **What source declarations exist in this repository, where exactly are they, and what syntax evidence produced them?**

It should **not yet** answer:

> What depends on them?

That is Phase 2.
