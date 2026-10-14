# Phase 0 — Repository Scanner implementation plan

**Status:** Ready for implementation
**Date:** 2026-10-13
**Primary language:** Rust
**Binary:** `ctx`
**Prerequisites:** `docs/prd.md`, `docs/architecture.md`

---

## 1. Objective

Build a small, polished, synchronous Rust CLI that inspects a local repository and returns a deterministic structural inventory **without parsing source syntax**.

The completed phase should support:

```bash
ctx scan .
ctx stats .
ctx tree .
```

and report useful information about file/directory counts, repository size, physical text lines, language distribution, file-role distribution, largest files, directory structure, and basic Git metadata.

Phase 0 is deliberately not a code graph. Its purpose is to establish a useful Rust program, clear boundaries, traversal semantics, versioned machine output, reliable tests, and a clean launch point for Phase 1 parsing/symbol work.

---

## 2. Learning objectives

By phase end, implementation should provide hands-on experience with:

- Cargo and modules;
- structs/enums/pattern matching;
- ownership and borrowing;
- `Path` / `PathBuf`;
- iterators/collections;
- `Option` / `Result` / `?`;
- file I/O;
- process invocation;
- Serde serialization;
- unit/integration tests;
- CLI parsing.

Explicit non-objectives: async/Tokio, threads, `Arc`/`Mutex`, unsafe, FFI, complex generic frameworks, graph algorithms.

---

## 3. User stories

1. `ctx scan .` provides a concise overview.
2. `ctx scan ~/code/project` works from another directory.
3. `ctx stats .` gives detailed statistics.
4. `ctx tree .` produces a deterministic relative tree.
5. `--format json` returns versioned machine-readable output.
6. non-Git directories still scan successfully.
7. ignore rules are respected.
8. one unreadable file degrades to a warning where practical rather than aborting the scan.

---

## 4. Scope

### In scope

- one local scan root;
- recursive traversal;
- `.gitignore`/`.ignore` aware scanning;
- name/extension language detection;
- path/name role heuristics;
- file sizes;
- physical line counting for recognized text files;
- deterministic aggregation;
- largest-file calculation;
- directory statistics/tree;
- basic Git metadata through read-only Git commands;
- human and JSON output;
- tests and public docs.

### Out of scope

Do **not** implement Tree-sitter, ASTs, symbols, imports/references, graphs, Git structural diffs/history, persistence, SQLite/Tantivy, caching, threads/Rayon, async/Tokio, daemon/API, MCP/editor integration, embeddings/model calls, Context Integrity policy, or multi-repository workspaces.

---

## 5. Scaffold reconciliation

The repository scaffold already contains:

- Cargo package + `ctx` binary shell;
- `scan`, `stats`, `tree` placeholder subcommands;
- CLI smoke tests;
- documentation/context architecture;
- verification scripts/CI.

Before implementing, inspect rather than recreating these files. Keep the existing public/documentation structure unless a real implementation need requires adjustment.

The scaffold is not proof that Phase 0 behavior exists.

---

## 6. Expected dependencies

Add dependencies only as each step uses them.

Runtime:

```toml
clap = { version = "4.5", features = ["derive"] }
ignore = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

Dev:

```toml
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

Use compatible stable releases at implementation time; do not add future-phase crates.

---

## 7. Target module shape

By phase end, a reasonable single-package layout is:

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

Do not create these modules until the vertical slice needs them. Do not split into multiple crates.

---

## 8. CLI contract

### `ctx scan [PATH]`

Defaults:

- path: `.`
- format: human

Add:

```text
--format human|json
--no-lines
--max-line-file-size <bytes>
```

Required human sections:

- Repository
- Summary
- Languages
- Roles
- Largest files
- Git
- Warnings (only when non-empty)

### `ctx stats [PATH]`

Emphasize totals, language/role aggregation, largest files, and optionally largest directories if simple.

### `ctx tree [PATH]`

Render deterministic relative paths. JSON should use a simple flat stable list rather than a deeply nested public schema.

All three commands may re-run the scan in Phase 0; persistence arrives later.

---

## 9. Machine-output contract

Version JSON from the start:

```json
{
  "schema_version": 1,
  "command": "scan",
  "data": {}
}
```

Human output is not a compatibility contract. JSON is.

Do not serialize arbitrary internal structs as the external API without deciding the fields intentionally.

---

## 10. Recommended domain types

Conceptually:

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

Use enums for common `Language` and `FileRole` variants with `Other`/`Unknown` escape hatches. Do not create language parser traits in this phase.

---

# 11. Work breakdown

Implement as vertical slices.

## 0.1 — Reconcile/bootstrap

### Work

- inspect the existing scaffold;
- verify package name/binary/docs paths;
- add only currently needed dependencies;
- ensure CI and scripts match the actual package.

### Acceptance

With a Rust toolchain:

```bash
cargo check
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

pass before substantive scanning work.

---

## 0.2 — Finalize CLI skeleton

The scaffold already exposes placeholder subcommands. Replace placeholder dispatch gradually with real application calls.

Requirements:

- `ctx --help` / `--version`;
- `scan`, `stats`, `tree` help;
- path defaults to `.`;
- command handling stays thin.

Tests: CLI help and default-path smoke tests.

---

## 0.3 — Root validation and traversal

Implement:

```rust
scan_repository(&ScanOptions) -> Result<RepositoryScan, ScanError>
```

First collect only root, relative file paths, directory count, and size.

Requirements:

- root exists and is a directory;
- preserve scan-root semantics distinct from Git root;
- use `ignore` traversal;
- respect ignore files;
- exclude `.git`;
- do not follow directory symlinks;
- output paths relative to scan root;
- deterministic sorting independent of filesystem traversal order.

Tests:

- nested fixture;
- missing root;
- file passed as root;
- ignored file;
- `.git` exclusion;
- symlink behavior where portable.

---

## 0.4 — Language classification

Implement one centralized function:

```rust
classify_language(path: &Path) -> Language
```

At minimum recognize:

```text
rs py ts tsx js jsx md mdx json yaml yml toml sh bash sql
```

Special names such as `Dockerfile`/`Makefile` may use `Other(...)` initially.

Use table-style unit tests without adding a test-macro crate solely for convenience.

Unknown extensions remain visible.

---

## 0.5 — File-role classification

Implement:

```rust
classify_role(path: &Path, language: &Language) -> FileRole
```

Required cases:

```text
src/main.rs             → Source
tests/cli.rs            → Test
test_auth.py            → Test
docs/architecture.md    → Documentation
README.md               → Documentation
Cargo.toml              → Configuration
migrations/001.sql      → Migration
schemas/openapi.yaml    → Schema
```

Document precedence for conflicting heuristics.

Do not infer more than path/name evidence supports.

---

## 0.6 — Line counting

Count physical lines for recognized text-like files under a configurable size threshold (recommended initial default: 1 MiB).

Use buffered streaming I/O; do not read entire files into memory just to count.

Define/test behavior for:

- empty file;
- final line with/without newline;
- over-threshold file;
- unreadable file.

Failure to count one file becomes warning + `None`, not whole-scan failure.

---

## 0.7 — Aggregation

Calculate:

- total files;
- total bytes;
- total counted lines;
- file/byte/line counts by language;
- file counts by role;
- largest files.

Use pure functions over file records where practical.

Deterministic sorting:

- language counts descending, display name tie-break;
- largest files size descending, path tie-break.

Percentages belong in presentation, not stored domain state.

---

## 0.8 — Git metadata

Implement a small module using `std::process::Command`.

Probe:

- repository root;
- HEAD;
- current branch;
- dirty state.

Non-Git path or missing Git binary must not make the repository scan fail. Convert optional enrichment failures into absence/warning as appropriate.

Use temporary initialized repositories for integration tests when Git is available; do not rely on the developer's working repository.

---

## 0.9 — Human `scan` renderer

Render stable readable sections. Example target:

```text
Repository: /path/to/repo

Summary
  Files:          27
  Directories:     8
  Size:         142 KB
  Text lines:    3,912

Languages
  Rust          11 files    41%
  Markdown       7 files    26%

Roles
  Source         11
  Tests           5
  Docs            7

Largest files
  docs/prd.md                     48 KB
  src/scan.rs                     14 KB

Git
  Branch: main
  Head:   abc1234
  Dirty:  yes
```

No terminal UI framework required.

---

## 0.10 — JSON output

Introduce explicit output DTO/envelope types.

Requirements:

- valid JSON;
- schema version `1`;
- command field;
- correct counts;
- relative paths where file paths are emitted;
- deterministic for stable input.

Integration tests should deserialize the JSON and assert structure rather than snapshot the entire string.

---

## 0.11 — `stats`

Reuse the same scan pipeline. Do not duplicate traversal.

Human output should emphasize totals and aggregate tables. JSON may use a command-specific data shape if needed rather than artificially forcing all commands into one DTO.

---

## 0.12 — `tree`

Derive tree output from canonical scan entries or a shared traversal result; the renderer must not run a second independent filesystem walker.

Requirements:

- stable alphabetical ordering;
- relative paths;
- ignored entries absent;
- JSON as flat path/kind entries initially.

---

## 0.13 — Error/warning polish

Audit all error paths and every `unwrap`/`expect`.

Requirements:

- invalid root gives an actionable message;
- per-file failures name the path;
- Git failures degrade gracefully;
- JSON errors propagate;
- non-UTF-8 paths do not panic.

Keep `PathBuf` internally; use explicit lossy string conversion only at presentation boundaries when necessary.

---

## 0.14 — Public docs

Update README when actual Phase 0 behavior is available. Remove the scaffold-only warning only when scanning is genuinely implemented and verified.

Update `docs/current-state.md` as the single status snapshot.

Do not add another implementation handoff file.

---

## 0.15 — Quality gate and real-repository checks

Run:

```bash
./scripts/verify.sh
cargo build --release
```

Then run the release binary against at least:

1. this repository;
2. one Python-heavy repository;
3. one TypeScript/mixed repository.

Record classification surprises and obvious performance/path issues in the implementation report. Optimize only if measurements show a real problem.

---

# 12. Acceptance criteria

## CLI

- [ ] help/version work;
- [ ] scan/stats/tree default to current directory;
- [ ] explicit path works;
- [ ] human output works;
- [ ] JSON output works.

## Traversal

- [ ] recursive discovery correct;
- [ ] `.git/` excluded;
- [ ] ignore semantics documented/tested;
- [ ] symlinks not followed by default;
- [ ] emitted file paths relative;
- [ ] output order deterministic.

## Classification

- [ ] common languages recognized;
- [ ] unknown files remain visible;
- [ ] test/docs/config/schema/migration cases covered.

## Statistics

- [ ] total file/byte counts correct;
- [ ] directory-count definition documented;
- [ ] text-line fixtures correct;
- [ ] large-file threshold works;
- [ ] language/role aggregation correct;
- [ ] largest-file ordering deterministic.

## Git

- [ ] Git root, HEAD, branch, dirty state supported;
- [ ] non-Git directories succeed;
- [ ] missing/failing Git does not crash core scan.

## Errors

- [ ] missing root returns non-zero;
- [ ] file-as-root returns non-zero;
- [ ] recoverable per-file errors do not normally abort;
- [ ] no normal repository condition panics.

## Machine output

- [ ] valid JSON;
- [ ] `schema_version = 1`;
- [ ] command field exists;
- [ ] stable input produces deterministic output;
- [ ] contract has integration tests.

## Quality / phase discipline

- [ ] canonical verifier passes;
- [ ] release build succeeds;
- [ ] no async runtime;
- [ ] no concurrency;
- [ ] no AST/parser dependency;
- [ ] no persistence;
- [ ] no LLM/model dependency;
- [ ] no Context Integrity policy logic.

---

# 13. Test matrix

| Area | Test | Level |
|---|---|---|
| CLI | help/version | integration |
| CLI | default path / invalid path | integration |
| traversal | nested files | fixture |
| traversal | ignored file / `.git` | fixture |
| paths | relative/stable output | unit/integration |
| language | known + unknown extensions | unit |
| role | source/test/docs/config/schema/migration | unit |
| lines | empty/trailing/no-trailing/threshold | unit |
| stats | language/role aggregation | unit |
| stats | largest-file tie | unit |
| Git | non-Git / initialized / dirty repo | integration |
| output | required human sections | integration |
| JSON | deserializable/versioned envelope | integration |
| tree | deterministic structure | integration |

Avoid flaky platform-specific permission tests if portability cost exceeds their value.

---

# 14. Fixtures

Keep fixtures tiny and explanatory.

Suggested mixed fixture:

```text
tests/fixtures/mixed-repo/
├── .gitignore
├── Cargo.toml
├── README.md
├── config.yaml
├── docs/architecture.md
├── migrations/001_init.sql
├── schemas/api.yaml
├── src/lib.rs
├── src/main.rs
├── src/helper.py
├── tests/cli.rs
└── ignored.log
```

`.gitignore` excludes `ignored.log`.

Use `tempfile` for dynamic Git repositories and threshold cases when committed fixtures are awkward.

---

# 15. Implementation-order rationale

Use this order:

```text
CLI shell
 ↓
walk files
 ↓
classify
 ↓
measure
 ↓
aggregate
 ↓
Git enrich
 ↓
render human
 ↓
versioned JSON
 ↓
stats/tree
 ↓
polish / real-repo checks
```

Do not design all final domain types before the first vertical slice. Let real scan data inform the model.

JSON arrives before phase end because it forces a healthy data/presentation boundary that future agents and Context Integrity will depend on.

---

# 16. Rust learning checkpoints

| Slice | Main Rust concepts |
|---|---|
| CLI | enums, derive, match |
| traversal | `Path`, `PathBuf`, iterators, `Result` |
| classification | enums/pattern matching |
| line counting | buffered I/O, `Option`, error propagation |
| aggregation | `HashMap`, sorting, borrowing |
| Git | `Command`, exit status, UTF-8 boundaries |
| rendering | references/formatting |
| JSON | Serde + small practical generic envelope |
| tree | recursive/stack traversal |
| polish | typed errors + ownership review |

When the borrow checker blocks progress, understand ownership/lifetime first. Do not introduce `Arc`, `Mutex`, `Rc`, or `RefCell` merely to escape ordinary Phase 0 ownership questions.

---

# 17. Common traps

- **Trait explosion:** do not create walker/detector/counter/provider traits before multiple implementations exist.
- **Path cloning obsession:** a few `PathBuf` clones are fine; clarity matters more than eliminating every allocation.
- **Printing inside scan logic:** return data, render later.
- **Internal structs as API:** Serde derive does not automatically define a stable public contract.
- **Phase 1 in disguise:** do not regex imports/symbols while walking files.
- **Premature parallelism:** sequential correctness is the benchmark baseline.
- **False precision:** extension/role classification is heuristic; say so.

---

# 18. Learning definition of done

Before Phase 1, the implementer should be able to explain:

1. owned `PathBuf` vs borrowed `&Path`;
2. where ownership transfers during scanning;
3. `Option` vs `Result` choices;
4. why non-Git scanning succeeds;
5. why renderers are separate from scanning;
6. why JSON is versioned;
7. why concurrency is deferred;
8. why `Language` is an enum in Phase 0;
9. what behavior is delegated to `ignore`;
10. how Tree-sitter can be added without rewriting scan/output boundaries.

---

# 19. Post-implementation review checklist

Review:

- plan completeness and actual wiring;
- scan/domain/output separation;
- CLI thinness;
- no Phase 1/Context Integrity leakage;
- understandable ownership/cloning;
- contextual errors/no user-input panics;
- ignore/path/line-count/Git edge cases;
- deterministic output;
- tests on real command path;
- README/current-state accuracy.

Passing tests do not prove the architecture is correctly integrated.

---

# 20. Phase exit and Phase 1 handoff

After implementation:

1. write `docs/reports/phase-0-implementation-report.md` from the template;
2. perform an independent review under `docs/reviews/`;
3. fix required findings;
4. run final verification;
5. update `docs/current-state.md`;
6. then write a focused Phase 1 design/implementation plan.

Questions to answer before Phase 1:

- Is `Language` ready to attach analyzers?
- Are relative path semantics stable?
- Is scan data reusable outside the CLI?
- Does JSON need compatibility correction before consumers exist?
- Which language should be parsed first?
- Should source text be read on demand through a `SourceFile` abstraction?
- Did real repos reveal generated/vendor handling gaps?
- Is the single-package layout still comfortable?

---

# 21. Suggested commits

A reasonable sequence:

```text
1. reconcile scaffold and dependencies
2. implement root validation/traversal
3. add language classification
4. add role classification
5. add line counting/warnings
6. add aggregation
7. add Git metadata
8. add human scan/stats rendering
9. add versioned JSON output
10. add tree rendering
11. complete fixture/CLI coverage
12. polish errors/docs/quality gate
```

Avoid one giant Phase 0 commit.

---

# 22. Optional stretch work

Only after every required criterion passes, choose at most one or two:

- `--top <N>` largest-file count;
- explicit hidden-file option if ignore semantics are fully understood;
- simple benchmark script.

Do not use stretch work to pull parsing, caching, threads, or persistence forward.

---

# 23. Final acceptance commands

```bash
./scripts/verify.sh
cargo build --release

./target/release/ctx --help
./target/release/ctx scan .
./target/release/ctx scan . --format json
./target/release/ctx stats .
./target/release/ctx tree .
```

Then run against at least two external repositories with different language mixes.

Phase 0 should feel like a complete small utility, not merely scaffolding.
