# Context Engine

> **Working name.** A local-first codebase / context intelligence engine written in Rust.

Context Engine is an early-stage open-source project for turning software repositories into structured, queryable evidence: files, symbols, relationships, changes, history, and eventually task-specific context for developers and coding agents.

The core question is:

> **What is true about this codebase, how is it connected, how has it changed, and what context is most relevant to a given task?**

The project is intentionally **descriptive rather than normative**. It aims to model observable code facts and provenance. A separate project, **Context Integrity**, may eventually consume that evidence to reason about authority, architectural intent, stale documentation, and drift.

## Status

**Pre-alpha / Phase 2 implemented; independent Phase 2 review pending.**
The synchronous CLI scans repositories, indexes Rust/Python symbols, and builds a
fresh in-memory graph of lexical containment, imports and call-site references.
The maintainer authorized Phase 2 despite pending final Phase 1 acceptance.
Phase 0 was accepted on 2026-10-14. Persistence, async, MCP, embeddings and model
calls remain future work.

See [`docs/current-state.md`](docs/current-state.md) for implementation and review status.

## Build and use

Install stable Rust with `rustfmt` and `clippy`:

```bash
cargo build --release
./target/release/ctx scan .
./target/release/ctx stats /path/to/project
./target/release/ctx tree .
./target/release/ctx scan . --format json
./target/release/ctx scan . --no-lines
./target/release/ctx scan . --max-line-file-size 2097152
./target/release/ctx index .
./target/release/ctx symbols . --format json
./target/release/ctx symbol RepositoryScan .
./target/release/ctx symbol Router.route /path/to/python-project
./target/release/ctx graph . --format json
./target/release/ctx deps construct . --depth 2
./target/release/ctx callers construct .
./target/release/ctx related construct . --file src/graph.rs
```

All commands default to the current directory and accept `--format human|json`.
`scan` and `stats` also accept `--no-lines` and `--max-line-file-size <bytes>`
(default 1 MiB). Every command performs a fresh scan. `scan` shows an overview;
`stats` adds per-language byte and line totals; `tree` lists alphabetically ordered
relative paths, with `/` suffixes for directories. Tree does not read file contents
or probe Git. Help and version: `ctx --help`, `ctx scan --help`, `ctx --version`.

### Scan semantics

- The requested root is canonicalized and must be a readable directory. It may
  differ from the enclosing Git root; non-Git directories are valid targets.
- `.ignore`, `.gitignore` (also outside Git), and parent ignore files use the
  `ignore` crate's precedence. Global Git ignores and Git's local `info/exclude`
  are disabled so unrelated configuration does not change the inventory.
- Hidden files are included unless ignored. `.git` entries are always excluded.
  Symlinks (both files and directories) and special filesystem entries are skipped.
  A symlinked `.gitignore` or `.ignore` in the scan tree or a consulted parent path
  fails the scan before ignore-aware traversal. A no-follow metadata preflight checks
  the tree, including ignored directories, and adds one traversal's metadata cost.
  Unknown regular files remain visible and contribute to file/byte totals.
- Directory count includes discovered, non-ignored descendant directories,
  including empty directories; it excludes the scan root. An unreadable discovered
  directory can still be counted, with its missing contents reported as a warning.
- Languages use names/extensions: Rust, Python, TypeScript/TSX, JavaScript/JSX,
  Markdown/MDX, JSON, YAML, TOML, shell, SQL, Dockerfile, Makefile, or Unknown.
  Roles use path/name heuristics with precedence: vendor, generated, test,
  migration, schema, documentation, configuration, source, unknown.
- Physical text lines count LF delimiters plus a final unterminated line. Empty
  files have zero lines; CRLF counts normally; a lone CR is not a delimiter.
  Only recognized text files at or below the limit are counted. Unknown, oversized,
  and disabled counts are absent; unreadable, invalid UTF-8, or NUL-containing
  recognized text files produce a warning and absent count. This is not semantic LOC.
  Totals explicitly report how many files have line counts.
- Largest files are the top 10 by bytes descending, with relative-path tie-breaking.
  Language/role tables sort by file count descending, then display name.
- Git reports enclosing root, HEAD, branch, and dirty state when available. Unborn
  HEAD and detached branch are absent values. Git failure becomes a warning.
  Dirty status covers the enclosing repository; submodule working contents are
  not inspected. Dirty probing is skipped with a warning if executable Git clean or
  process filters are configured, or if that configuration cannot be checked safely.
  Git probing disables filesystem-monitor commands and optional locks.
- Paths remain native internally. Output uses lossy Unicode at the presentation
  boundary (relative paths use `/`); non-UTF-8 paths trigger a warning because
  replacement characters may make displayed names ambiguous. Human output escapes
  control characters from paths and warning messages. In JSON schema v1, warning
  paths identify scan-root-relative entries with `/`; repository-wide warnings use
  `null`.
- Scans are local and read-only, with no execution of analyzed code or network calls.
  Results reflect live filesystem state, not an atomic snapshot.
- Discovery has internal aggregate limits: 50,000 entries (files, directories and
  symlinks), 4 MiB of relative-path bytes per walk, 1,000 warnings / 1 MiB of warning
  details, 1 MiB of consulted ignore-control content, and 256 MiB of line-count I/O.
  The ignore-free safety preflight has the same entry/path limits, including ignored
  entries. Exceeding a limit fails the command without a partial inventory.

### Syntax and symbol indexing

`index`, `symbols` and `symbol <NAME>` reuse the canonical scanner with line
counting disabled and optional safe Git metadata. Each command builds a fresh
in-memory index. Nothing is saved to `.ctx/` or any cache. These commands accept
`--max-parse-file-size <BYTES>` (default 1 MiB), separately from scan line counting.

- Rust: modules, structs, enums, unions, traits, functions, impl/trait methods,
  type aliases (including associated types), constants and statics.
- Python: classes, functions, methods, nested definitions, decorators and async
  definitions. A function nested inside a method is a function, not another method.
- Qualified names describe syntax in a single file: `parser::parse`,
  `Engine::run`, `Router.route`, `outer.inner`. Rust nominal impl targets omit
  generic arguments; reference/pointer and other unsupported impl targets
  contribute no guessed container.
  File paths do not infer package/module names. Duplicate declarations remain
  distinct. Names are not stable identities across edits.
- `symbol` first matches exact case-sensitive qualified names. Only when none
  match does it try exact local names. Multiple matches and zero matches succeed.
  There is no substring, fuzzy or semantic search.
- Every discovered file receives a status: `analyzed`, `unsupported_language`,
  `skipped_role`, `too_large`, `invalid_utf8`, `read_failed`, `parse_failed` or
  `analysis_limit_exceeded`.
  Supported files in generated/vendor roles are skipped. Other supported roles,
  including tests, migrations and configuration, are eligible.
- Source loading validates relative paths, checks every path component for
  symlinks, requires regular files, bounds reads before and during loading, and
  requires UTF-8. Files observed as unavailable, symlinks/non-regular, or oversized
  while loading fail recoverably. Ordinary concurrent content changes may still
  be analyzed because scan, metadata checks and opening are not an atomic snapshot.
  Python encoding-cookie decoding is unsupported; these checks do not provide a
  platform sandbox.
- Recoverable syntax errors retain available symbols. Error/missing nodes are
  counted, with at most 20 detailed syntax diagnostics per file. Tree-sitter's
  in-progress query-state limit remains a separate guard; exhaustion discards
  that file's symbols and records `parse_failed`. Project-owned per-file limits
  cap analysis at 4,096 symbols, 4 MiB of local/qualified name materialization,
  and 25,000 qualification-node visits. Exceeding a limit discards all symbols
  from that file and records `analysis_limit_exceeded`. Diagnostics contain
  locations and fixed descriptions, not source bodies.
- Repository analysis independently caps retained symbols at 50,000, local plus
  qualified names at 8 MiB, symbol paths plus diagnostic details at 16 MiB, and
  detailed diagnostics at 10,000. Aggregate overflow fails the whole command;
  it is distinct from a recoverable per-file analysis failure. Index and graph
  JSON serialization has a 64 MiB byte limit, checked before writing stdout.
- Human symbol locations use one-based lines and byte columns at the symbol name.
  All repository-controlled human text has control characters escaped.

### Repository relationship graph

`graph`, `deps <NAME>`, `callers <NAME>` and `related <NAME>` rebuild a synchronous,
in-memory graph per command using the same scanner, loading protections and
`--max-parse-file-size` limit as symbol indexing. Symbols and relation candidates
come from one loaded source and one parse tree per analyzed file. Symbol-only
commands skip relation work. No target source is executed.

- Entities are the repository, every scanned file, and each canonical symbol
  record. Symbol identity is structural (file, kind, declaration range), separate
  from its name. IDs are index-local integers: repository first, files by path,
  then symbols in canonical index order. Stable input gives deterministic IDs;
  edits can renumber them. IDs are not persistent or lineage identifiers.
- `contains` is lexical source-range containment. A symbol's parent is its nearest
  strictly containing declaration, otherwise its file. Rust `impl Foo` methods
  are not contained by the separate `struct Foo` declaration. Ambiguous overlapping
  ranges fall back conservatively to file ownership.
- `imports` are unresolved syntactic facts. Rust grouped `use` trees and Python
  dotted/from/relative imports become atomic targets, preserving aliases and
  wildcard metadata. Imports are never resolved to files or symbols, and aliases
  are not used to resolve references. Rust grouped `self` denotes its group prefix:
  `use foo::{self as f, bar}` emits `foo` (binding `f`) and `foo::bar`.
- `references` carry `reference_context: call`. Rust supports identifier/scoped
  callees; Python supports identifier-rooted dotted chains. Complex/dynamic
  callees are counted as unsupported and omitted. Accepted inner calls still
  appear when an outer dynamic call is unsupported. Targets contain normalized
  name segments, never arguments or arbitrary expressions. Evidence ranges cover
  exactly the callee syntax, excluding arguments and call parentheses.
- Reference resolution requires a unique exact same-file qualified-name match.
  A bare call never resolves to a method/nested declaration solely because its
  local name is unique. Ambiguous and cross-file names remain unresolved. This does not model lexical
  binding/shadowing, runtime types, dispatch, `self`/`Self` rewriting or compiler
  resolution. Extraction derivation and resolution quality are separate evidence.
- Queries first match exact qualified names, falling back to exact local names.
  Zero matches succeed; multiple roots remain separate groups. `--file <PATH>`
  filters matches afterward using a scan-root-relative path. `--depth` defaults to
  1 and accepts 0 through 8; depth 0 returns roots without relation occurrences.
- `deps` follows outgoing resolved references and includes unresolved imports or
  references owned by visited entities as leaves. It does not follow containment
  or inherit a file's imports for each symbol. `callers` follows incoming resolved
  call-context references: these are **syntactic callers**, not a compiler-complete
  call graph. `related` follows resolved edges in both directions, including
  containment, and shows unresolved facts as attached leaves.
- Repeated occurrences and self-loops remain distinct relations. Iterative traversal
  handles cycles. Query budgets, shared across root groups, cap visited entity
  occurrences at 10,000 and emitted relation occurrences at 25,000; exceeding a
  budget sets `truncated: true` for the affected group and overall result.
  At most 128 matching roots are retained in canonical order; omitted roots set
  `roots_truncated: true` and overall `truncated: true`. Returned root groups
  retain their independent traversal truncation flags.
- Each file has an independent relation budget: 16,384 candidates, 4 MiB aggregate
  normalized target/binding bytes, 50,000 normalization node/segment visits, 64
  segments per target and 4 KiB per target or alias. Exceeding any limit discards
  all extracted imports/references for that file, records
  `analysis_limit_exceeded`, and retains valid Phase 1 symbols and structural
  containment. Unsupported counts on failed files cover only work before failure.
  Global ceilings of 50,000 entities, 100,000 relations and 16 MiB of aggregate
  candidate target/alias plus repeated evidence-path bytes fail the graph command
  clearly. These complement the scanner/index limits. Full graph and query JSON
  is capped at 64 MiB after escaping; overflow fails before emitting any JSON.
  These are safety ceilings, not performance promises.

`graph` human output is a count summary; JSON includes the full graph. These
commands have no persistence, caching, cross-file import resolution or compiler
integration. Existing filesystem snapshot and Git-metadata limitations apply.

### JSON schema v1

All JSON output is wrapped as `{"schema_version":1,"command":"scan","data":{...}}`.
`scan` and `stats` data contain `root`, `directory_count`, `totals`, `languages`,
`roles`, `largest_files`, `git`, and `warnings`. Only `scan` includes the full
`files` inventory. File records contain `path`, `language`, `role`, `size_bytes`,
and nullable `line_count`. Counts contain `files`, `bytes`, `counted_lines`, and
`files_with_line_counts`. Git is nullable; its fields are `repository_root`,
`head_commit`, `branch`, and `is_dirty` (the latter three nullable).
Warnings contain nullable `path` and `message`. A non-null warning path is the
scan-root-relative identity of an entry, normalized with `/`; repository-wide Git
warnings have a null path.

`tree` data contain `root`, a flat sorted `entries` list (`path`, `kind` of
`file` or `directory`), and `warnings`. Relative file/tree paths are rooted in
`data.root`; root and Git root are absolute. Stable input yields deterministic
output; live filesystem and Git changes can alter results. Fatal errors go to
stderr with a nonzero exit status.

Phase 1 keeps the same schema-v1 envelope, with commands `index`, `symbols`, and
`symbol`. Each data object includes `root`, nullable `git`, `summary`, `languages`
(symbol counts), `kinds` (symbol counts), bounded `diagnostics`, and `scan_warnings`.
The summary counts discovered/supported/analyzable/analyzed/skipped/failed files,
syntax-error files, full syntax issues, symbols, and file statuses. `analyzable_files`
means supported non-generated/non-vendor files before size and read checks.

`index` adds `files` with path, language, role, status and symbol/syntax counts.
`symbols` adds `symbols`. `symbol` adds `query`, `match_mode` (`qualified_name`,
`local_name`, `none`) and `matches`. Symbol records contain `name`, `qualified_name`,
`kind`, lowercase `language`, repository-relative `path`, `range`, and `name_range`.
Each range contains `start` and `end` points with UTF-8 `byte` offsets, zero-based
`row`, and zero-based **byte** `column`; the byte interval is half-open `[start,end)`.
Whole ranges follow declaration nodes (Python includes decorator wrappers;
Rust preceding attribute siblings are outside the declaration node).
No source bodies are emitted. Files sort by path; symbols sort by path, start byte,
end byte, kind name and qualified name; diagnostics sort by path, range, kind and
message. Non-UTF-8 paths remain lossy and explicitly warned.

Graph commands also use schema v1. `graph` data include `root`, nullable `git`,
`id_semantics`, `summary`, `entities`, `relations`, `files`, `relation_files`,
`diagnostics` and `scan_warnings`. Entities expose `id`, `kind`, nullable `path`,
and nullable `symbol` (the existing symbol shape). Relation IDs are deterministic
vector positions after sorting by evidence path/range, kind, source, target,
metadata and derivation; distinct occurrences are retained. Relations expose
`id`, `kind`, `source_id`, `target`, `resolution`, `metadata` and `evidence`.
Targets are tagged `{ "state": "resolved", "entity_id": 1 }` or
`{ "state": "unresolved", "name": "foo.bar" }`. Evidence contains `path`, nullable
`range`, nullable `language`, and `derivation`. Metadata is tagged by `kind`:
`none`, `import` (nullable `binding`, `wildcard`), or `reference`
(`reference_context: "call"`). Resolution values are `direct_structural`,
`exact_same_file_qualified_name` and `unresolved`.

`files` accounts for all discovered files with `symbol_status` and symbol/syntax
counts. `relation_files` accounts for files whose symbol analysis succeeded,
including relation `status` (`analyzed`, `extraction_failed`, or
`analysis_limit_exceeded`), import/reference/unresolved counts and
`unsupported_candidate_count`. Thus unavailable symbol analysis is not confused
with successful relation extraction. Summary exposes both categories.

Query commands add `query`, `match_mode`, root ID list `roots`, `depth`,
`roots_truncated`, `truncated`, and `results` instead of top-level entity/relation arrays. Each result
contains `root_id`, its entity/relationship records and its own `truncated` flag.
Output IDs reference the enclosing graph, not positions in a query subset.
Source bodies are never serialized; optional Git HEAD is repository metadata,
not an identity for exact dirty working-tree content.

## Future direction

Later phases may add richer repository queries, structural Git changes,
persistence and task-context compilation. These capabilities are not implemented.

## Why Rust?

Rust is a strong fit for a local developer-intelligence engine because the long-term workload is dominated by filesystem traversal, parsing, graph operations, indexing, incremental updates, concurrency, and low-overhead distribution. Rust is also an explicit learning goal for this project, so advanced language/runtime features are introduced only when a product requirement justifies them.

The roadmap intentionally progresses from ownership and `Result` handling, through parsing and graph modeling, to persistence, concurrency, and async services.

## Design principles

- **Deterministic first.** Prefer parsing, Git, graphs, search, and explicit metadata before probabilistic inference.
- **LLM optional.** The core must remain useful with zero model calls or API keys.
- **Evidence is inspectable.** Results should retain source paths, revisions, ranges, and derivation where possible.
- **Local first.** No source upload or cloud service should be required for the core engine.
- **One reusable model.** The engine should not be designed around one coding agent, IDE, or transport.
- **Small phases.** Avoid pulling persistence, async, graph databases, or agent protocols forward before they are needed.
- **Policy stays separate.** Context Engine reports facts; Context Integrity may later interpret them against intended architecture and authority.

## Repository map

```text
.
├── AGENTS.md                 # canonical compact instructions for coding agents
├── CLAUDE.md                 # pointer only; no duplicate instructions
├── README.md                 # public project overview
├── CONTRIBUTING.md
├── Cargo.toml
├── rust-toolchain.toml
├── src/                      # Rust implementation
├── tests/                    # integration tests / fixtures
├── scripts/                  # canonical verification and context checks
├── docs/
│   ├── README.md             # task-based document router
│   ├── current-state.md      # single concise living status snapshot
│   ├── prd.md                # long-term product requirements
│   ├── architecture.md       # durable technical boundaries
│   ├── plans/                # future/authorized work
│   ├── reports/              # implementation evidence
│   ├── reviews/              # independent review evidence
│   ├── decisions/            # durable ADRs
│   ├── development/          # development workflows and conventions
│   ├── integrations/         # cross-project contracts
│   ├── templates/            # repeatable artifact templates
│   └── archive/              # historical/superseded context
└── .github/
    └── workflows/ci.yml
```

The documentation layout is intentionally designed for human and LLM development. Start with the smallest relevant context set rather than loading the entire repository narrative into every session. [`docs/README.md`](docs/README.md) defines the reading routes and source-of-truth hierarchy.

## Development

Install a stable Rust toolchain with `rustfmt` and `clippy`, then run:

```bash
./scripts/verify.sh
```

The canonical verification path runs repository-context checks plus:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Release verification also runs `cargo build --release` and exercises the binary against real repositories.

## Documentation

Start here:

- [Documentation router](docs/README.md)
- [Current state](docs/current-state.md)
- [Product requirements](docs/prd.md)
- [Architecture](docs/architecture.md)
- [Phase 2 implementation plan](docs/plans/phase-2-implementation-plan.md)
- [Phase 2 implementation report](docs/reports/phase-2-implementation-report.md)
- [LLM-assisted development workflow](docs/development/llm-workflow.md)
- [Context Integrity integration boundary](docs/integrations/context-integrity.md)

## Contributing

Contributions are welcome while the project is still young. Please read [`CONTRIBUTING.md`](CONTRIBUTING.md) and the relevant task route in [`docs/README.md`](docs/README.md) first.

For substantial phase work, the expected workflow is:

```text
plan → implementation → implementation report → independent review → fixes → verification → current-state update
```

Plans describe authorized future work; reports and tests provide evidence of what was actually delivered.

## Security and privacy

The core project is intended to operate locally and read-only by default. It should not execute repository code or upload source as part of normal analysis. See [`SECURITY.md`](SECURITY.md).

## License

MIT. See [`LICENSE`](LICENSE).
