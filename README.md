# Context Engine

> **Working name.** A local-first codebase / context intelligence engine written in Rust.

Context Engine is an early-stage open-source project for turning software repositories into structured, queryable evidence: files, symbols, relationships, changes, history, and eventually task-specific context for developers and coding agents.

The core question is:

> **What is true about this codebase, how is it connected, how has it changed, and what context is most relevant to a given task?**

The project is intentionally **descriptive rather than normative**. It aims to model observable code facts and provenance. A separate project, **Context Integrity**, may eventually consume that evidence to reason about authority, architectural intent, stale documentation, and drift.

## Status

**Pre-alpha / Phase 1 implemented; independent Phase 1 review pending.**
The synchronous CLI scans repositories and builds a fresh in-memory Rust/Python
symbol inventory, with source ranges, syntax diagnostics, deterministic human
reports and versioned JSON. Phase 0 was accepted by the maintainer on 2026-10-14.
Graphs, relationships, persistence, async, MCP, embeddings and model calls remain
future work.

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
- Human symbol locations use one-based lines and byte columns at the symbol name.
  All repository-controlled human text has control characters escaped.

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

## Future direction

Later phases may add symbol relationships, graphs, structural Git changes,
persistence and task-context queries. These capabilities are not implemented.

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
- [Phase 0 implementation plan](docs/plans/phase-0-implementation-plan.md)
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
