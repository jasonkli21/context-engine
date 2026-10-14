# Phase 0 implementation report

**Plan:** `docs/plans/phase-0-implementation-plan.md`  
**Commit/range:** review remediation committed together with this report as one commit
**Date:** 2026-10-14  
**Status:** implemented, review findings addressed and locally verified; independent re-review pending

## Summary

Phase 0 now provides a synchronous local `ctx scan`, `ctx stats`, and `ctx tree`
utility. It inventories regular files and descendant directories, applies ignore
rules, classifies language and role, counts bounded physical text lines, aggregates
statistics, and optionally probes basic Git metadata. Human output and explicit
schema-v1 JSON are wired through the actual CLI. No later-phase analysis or
infrastructure was introduced.

## Independent review remediation

The attached Phase 0 independent review identified five findings. All five were
addressed in this remediation pass:

| Finding | Disposition |
| --- | --- |
| CE-P0-001 — Git filters could run during `git status` | Query effective Git configuration first; skip dirty probing with a pathless warning when clean/process filters exist or safety cannot be checked. Regression coverage confirms the marker command is not run while root, HEAD and branch remain available. |
| CE-P0-002 — ignore-control symlinks could be opened | Add a no-follow metadata preflight for root, descendant and parent `.gitignore`/`.ignore` candidates; fail with a typed error naming a symlink. Disable `.git/info/exclude`. |
| CE-P0-003 — warning paths mixed namespaces | Define warning paths as scan-root-relative entries, normalize JSON paths with `/`, give Git/global warnings null paths, and preserve dependency-provided traversal paths structurally. |
| CE-P0-004 — `tree` did unused enrichment | Give `tree` only path/format CLI arguments and disable line reads and Git probing while retaining the shared traversal. |
| CE-P0-005 — human output could emit terminal controls | Escape controls in human paths/messages and fatal CLI errors while keeping JSON strings unchanged. |

Regression tests cover all five findings. The fixes have not been independently
re-reviewed and cross-platform CI was not run.

## Implemented requirements and acceptance audit

The Phase 0 acceptance checklist was walked against code and executable tests:

| Acceptance area | Implementation and evidence |
| --- | --- |
| CLI help/version, defaults, explicit paths | Clap shell extended; integration tests run all three commands with default/current-directory and explicit roots, help/version, invalid format/limit input |
| Recursive traversal, relative paths, ordering | `scan_repository`; mixed fixture asserts exact file list, totals, directories, and repeat JSON equality |
| Ignore rules and `.git` exclusion | `ignore::WalkBuilder`; fixture covers `.gitignore`, `.ignore`, fake `.git`, and hidden ignore files; nested test covers parent rules, negation, and `.ignore` precedence |
| Symlinks and ignore controls | File/directory links are skipped; symlinked `.gitignore`/`.ignore` controls fail closed for scan-root, nested and parent candidates |
| Known and unknown languages | Centralized table covers required extensions, TSX/JSX/MDX, case normalization, special names, and Unknown |
| Roles | Table tests required source/test/docs/config/schema/migration cases and vendor/generated/conflicting-rule precedence |
| Counts and aggregation | Pure aggregation unit test covers files/bytes/lines/coverage, role counts, language counts and tie ordering; mixed CLI fixture independently sums filesystem sizes and asserts line totals |
| Directory definition | Discovered descendant directories, including empty directories, excluding root; fixture and README document/test this |
| Physical lines and limits | Unit tests cover empty/final/newline/CRLF/lone-CR/multibyte text, split UTF-8, invalid UTF-8, NUL, bounded reads and I/O failures; CLI tests cover exact/custom/default 1 MiB limits and disabled counts |
| Largest files | Pure function sorts bytes descending/path ascending and truncates at 10; unit and JSON tests check size/path ties |
| Optional Git | Temporary real Git repository test covers unborn HEAD, clean commit, dirty state, detached HEAD, and subdirectory scan root distinct from Git root |
| Missing/failing Git and non-Git inputs | CLI tests exercise absent executable, failing executable, and non-Git directories; inventory remains usable |
| Recoverable filesystem errors | Unreadable file/directory integration test passed under enforced permissions; file remains with absent lines and warnings, directory traversal failure is a warning |
| Actionable fatal errors / panic audit | Missing/file roots tested nonzero; production `unwrap`/`expect` absent; arbitrary paths retained natively |
| Human sections and tree | CLI tests assert scan sections, detailed stats, empty directory paths, ignored entries absent, and repeated tree equality |
| JSON contract | Intentional DTOs/envelope; all commands deserialized with schema version 1 and command name; counts, paths, flat tree kinds, nullability and repeated outputs tested |
| Non-UTF-8 paths | Unix output unit test verifies replacement characters and ambiguity warning without relying on unsupported macOS filenames; actual filename integration test enabled on other Unix platforms |
| No repository-helper execution | Git fsmonitor is disabled; dirty probing skips configured clean/process filters, with a marker integration test proving no filter command is run |
| Tree work | Tree shares the traversal but skips line-count reads and Git subprocesses; integration tests use invalid UTF-8 and a marker Git executable |
| Warning paths and terminal safety | Warning JSON paths are relative `/` paths or null; human control characters are visibly escaped |
| Quality / phase discipline | Canonical verifier and release build pass; one package, sequential walker, no async, parsing, persistence, model or policy implementation |

No known Phase 0 review finding remains unaddressed in the locally exercised macOS
implementation. Independent re-review of the fixes and cross-platform CI remain
unperformed.

## Code organization

- `src/main.rs`: thin argument-to-domain dispatch, rendering selection, stdout/error
  handling; broken stdout pipes end cleanly.
- `src/cli.rs`: shared path/format/line-count options for scan and stats; tree has
  only path and format arguments.
- `src/scan.rs`: typed root errors, synchronous ignore-aware traversal, symlinked
  ignore-control preflight, recoverable warnings, file measurements, bounded buffered
  text counting.
- `src/classify.rs`: language/role enums, centralized heuristics and precedence.
- `src/model.rs`: CLI-independent records/options, Git/warnings, pure aggregation.
- `src/git.rs`: optional read-only Git enrichment with nullable partial results.
- `src/output.rs`: explicit schema-v1 DTOs, human reports, flat tree generation
  from scan entries, lossy-path qualification at the presentation boundary.
- `tests/cli.rs` and `tests/fixtures/mixed-repo/`: small mixed fixture plus dynamic
  threshold, permission, symlink, ignore and Git scenarios.
- `Cargo.toml`/`Cargo.lock`: only Phase 0 dependencies, application lockfile generated.
- `README.md` and `docs/current-state.md`: public behavior and concise current status.

## Architecture decisions

1. Keep `PathBuf` in the domain and sort native paths before presentation. Relative
   output uses `/`; non-UTF-8 conversion is explicit and warned, not guessed.
2. Retain directory paths, including empty directories, in the shared scan result.
   Tree rendering does not traverse again and directory count is derived from this
   single collection.
3. Global Git ignore configuration and local Git excludes are disabled;
   repository/parent ignore files remain active. `require_git(false)` makes `.gitignore` useful
   for plain directories (including parent rules above Git boundaries).
4. Include hidden files unless ignored. Skip every symlink; preflight fails closed
   on symlinked `.gitignore`/`.ignore` controls before the ignore crate can open them.
   No broad hard-coded vendor or build exclusions were added; those names only
   influence role classification.
5. Count only recognized, valid UTF-8 text without NUL bytes. A bounded reader
   checks content while counting LF delimiters and a final unterminated line.
   Files that grow over the limit become warnings. Coverage accompanies totals.
6. Keep DTOs separate from domain types. Scan/stats share aggregate fields, while
   only scan emits the full inventory. Tree emits a flat path/kind list. No timestamps
   or percentages are stored in the machine/domain model.
7. Git probing disables optional locks and fsmonitor, removes common ambient Git
   directory/index overrides, and degrades failures to warnings. Before dirty
   status, it checks effective config for clean/process filters and skips status if
   any are configured or the check fails. Dirty status is repository-wide, with
   submodule worktree contents excluded.

These are delivery choices for the independent reviewer to assess, not a claim
that the architecture is flawless.

## Deviations from the Phase 0 plan

The tree command now accepts only path and format, as review CE-P0-004 showed that
line-count flags were not applicable to it. Git's local `info/exclude` is disabled
because no-follow protection is not implemented for that source. The plan's
unspecified defaults are explicit in README: hidden files included, Git global and
local exclude files disabled, all symlinks skipped, top 10 largest files, and
UTF-8/NUL validation for recognized text. A no-follow metadata preflight checks
ignore controls, including under ignored directories, before normal traversal. The
human tree uses a flat alphabetically ordered relative-path listing.

The review findings are addressed, but this does not certify a separate re-review
of the fixes. No PRD or historical documents were rewritten.

## Verification performed

Rust was initially unavailable (`./scripts/verify.sh` failed with `cargo is not
installed`; `cargo check` failed with command not found). An isolated toolchain was
installed under `/tmp`, without modifying global shell/toolchain configuration.
Dependency fetching needed network-enabled execution; an initial offline check
could not find the not-yet-downloaded `ignore` crate. These setup blockers were
resolved. Stable Rust 1.99.0 was used on aarch64 macOS.

Commands were run with:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
```

| Command | Final status |
| --- | --- |
| `./scripts/verify.sh` on original scaffold | Pass after toolchain setup |
| `cargo check` on scaffold and scanner | Pass |
| `cargo fmt --all` | Applied |
| `./scripts/verify.sh` after review remediation | Pass: context check, format, Clippy, 7 unit + 16 integration tests |
| `cargo fmt --all -- --check` (inside verifier) | Pass |
| `cargo clippy --all-targets --all-features -- -D warnings` (inside verifier) | Pass |
| `cargo test --all-targets --all-features` (inside verifier) | Pass |
| `cargo build --release` | Pass |
| `./target/release/ctx --help` | Pass |
| `./target/release/ctx scan .` | Pass |
| `./target/release/ctx scan . --format json` | Pass; JSON parsed and inspected |
| `./target/release/ctx stats .` | Pass |
| `./target/release/ctx tree .` | Pass |
| `./target/release/ctx scan /Users/jasonkli/projects/axiom-pipeline-interview-main --format json` | Pass; Python-heavy directory |
| `./target/release/ctx scan /Users/jasonkli/projects/health-app --format json` | Pass; TypeScript/Python mixed Git repository |

An early implementation test run attempted a non-UTF-8 filename on macOS and failed with illegal
byte sequence (and operation not permitted under sandbox). That filesystem case
is now platform-gated; the raw native-path output boundary is still tested locally
without filesystem creation. The final suite has no failures or ignored tests on
this platform. Linux-only arbitrary-byte filename integration was not run here.

Observed external smoke scans: Python project had 21 files (8 Python), 4 descendant
directories, and no warnings; mixed project had 384 files (141 TypeScript, 127
Python), 84 directories, and no warnings. Git root/HEAD/main/dirty were populated
for the mixed project. Approximate single-run process times were 0.022 seconds and
0.157 seconds respectively; these are smoke timings, not a benchmark. The earlier
implementation self-scan had no warnings and no Git metadata because it was run
before this checkout carried Git metadata.

Classification observations: unknown extensions and extensionless manifests,
lockfiles, notebooks and data remain visible but uncounted as text. For example,
the Python-heavy fixture has 11 unknown files. No guessed language or hidden
filter was added to improve those numbers.

No cloud/provider verification, independent re-review of the fixes, Windows
verification, or Linux execution is claimed. No remaining local verification
command is blocked.

## Known limitations / technical debt

- Classification is name/path evidence, not syntax analysis; unsupported languages
  remain Unknown, and unknown text does not receive line counts.
- UTF-16 and other non-UTF-8 text formats are not counted; lossy output for native
  non-UTF-8 names can collide and is explicitly warned.
- Every command rescans; live modifications can yield mixed-time metadata/content.
  There is no atomic snapshot or protection against adversarial path replacement
  during traversal/open. Ignore-control preflight performs an additional metadata
  traversal and may reject symlinks inside ignored subtrees.
- Git is optional. Dirty state excludes submodule working contents; missing/failed
  Git can yield absent or partially unavailable metadata. Dirty state is absent
  when Git clean/process filters are configured or filter safety cannot be checked.
  Probe subprocesses have no timeout, so unusually slow local Git commands can delay
  completion.
- Permission/byte-path integration coverage is platform-dependent; macOS was the
  actual local verification platform. CI configuration was inspected, not run.

## Phase 1 handoff observations

- File inventory, directory collection, and classification are usable independently
  of presentation; parser work should consume native relative paths and preserve
  absent/unsupported evidence rather than treating line counts as parser readiness.
- Scan-root identity is distinct from enclosing Git metadata, tested with a subdirectory.
- Keep the schema-v1 DTO contract intentional as future domain types grow; adding
  syntax facts does not require serializing the entire internal model.
- Independent re-review should verify production integration and ignore/Git failure
  semantics as well as passing tests. No Phase 1 design or infrastructure was added.

## Rust learning notes

`Result` and `?` handle fatal root/output failures; `Option` distinguishes unavailable
line/Git evidence from zero/false. Scanner records own paths and metadata, while
aggregation borrows file records for largest-file reporting. Enum matches centralize
classification; `BTreeMap` plus explicit sorting gives deterministic aggregate
order. Buffered `fill_buf`/`consume` counts without retaining file contents, and a
small carry handles UTF-8 sequences across chunks. DTO conversion performs string
allocation only at the output boundary. Tests use `unwrap` for fixture assertions;
production input paths use explicit failure handling.
