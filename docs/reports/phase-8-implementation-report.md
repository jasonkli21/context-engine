# Phase 8 implementation report

This report records the initial Phase 8 implementation and its original contract.
The subsequent independent-review fixes revise path-origin certainty, history JSON
schema and coverage fields; see the [Phase 8 review-fix report](phase-8-review-fix-report.md)
for the current contract and verification status.

**Date:** 2026-10-16
**Status:** implemented and locally verified; fresh independent review required

## Baseline and authorization

Started at `ef44712858f02b34f1fd4b43201ad6f26bbc3cf2`
(`fix: address phase 7 review findings`), matching the planning baseline. The only
initial worktree entry was the supplied untracked Phase 8 plan. It is included in
the single implementation commit with only Markdown trailing whitespace normalized.
No user source changes were reset or discarded. The maintainer explicitly requested
Phase 8 implementation and one commit; this authorizes progression without accepting
Phase 7 or any earlier pending phase.

The resulting commit is the single implementation commit containing this report,
directly on the baseline above; obtain its identity with `git log -1`. No push or
independent acceptance is part of this implementation.

## Production architecture and contracts

`ctx history SYMBOL [PATH] [--file RELATIVE_FILE]` and
`ctx evolution TARGET [--repo PATH]` are wired through Clap, application/domain
functions, explicit output DTOs and stdout. Both accept `--max-commits`,
`--max-parse-file-size` and `--format human|json`.

`git_revision.rs` extends the established hardened read-only adapter with typed,
NUL-delimited commit metadata and first-parent enumeration, root-commit raw diffs,
and unscoped raw-diff usage accounting. Metadata includes OID, first parent,
committer timestamp and subject. Git walk order is authoritative; output is newest
first. Merge results compare against their first parent, not every branch parent.
One lookahead commit establishes window truncation. Subjects are bounded untrusted
text, serialized normally and control-escaped for humans. Shallow repositories and
repositories with an `info/grafts` entry fail clearly. Claimed roots are also
checked against immutable commit headers to prevent a shallow-boundary race from
fabricating introduction evidence.

All historical commands inherit literal paths, sanitized inherited `GIT_*`
environment, optional locks disabled, prompts/replace objects/lazy fetching disabled,
fixed rename detection and no external diff/textconv. No checkout, reset, stash,
temporary historical checkout, hooks or analyzed repository execution is used.
Git's existing fixed 50% file-rename similarity classification remains evidence,
not compiler-level semantic identity.

The minimal shared seam exposes `change::snapshot`, `compare_file` and file usability
inside the crate and returns snapshot source-byte usage. Diff/impact retain their
production paths and their existing scoped boundary presentation. History-specific
raw-diff adaptation preserves `entered_scope` / `left_scope` evidence for clipped
renames. Historical snapshots still use centralized classification,
`index::analyze_loaded`, existing analyzers and `graph::from_analysis`. There is no
copied parser, resolver or comparison algorithm. Historical graphs contain selected
files, not full repository snapshots per commit. HEAD symbol lookup inventories and
analyzes the committed scoped tree once.

Current SQLite schema **3**, `INDEX_COMPAT_VERSION=1`, and **freshness-v2** are
unchanged because stored/analyzer semantics did not change. Historical application
functions never invoke persistence, reconcile, store, watch or live scanning.

## Identity and coverage semantics

HEAD lookup tries exact qualified name, falls back to exact local name only if no
qualified match exists, then applies the optional file filter. Zero/multiple matches
fail with the match count and disambiguation guidance. Incomplete seed-file analysis
also fails; HEAD inventory analysis gaps are counted explicitly.

Lineage uses language, kind, qualified name and relative file path, never numeric
IDs or offsets. Each adjacent first-parent pair is inspected for a delta arriving
at the current lineage path; unrelated files are not parsed. Before/after exact
matches must be unique. An explicit Git rename maps the prior path only while this
continuity holds. Source-slice comparison comes from Phase 4. Rename events also
carry `lineage_symbol_modified`, distinguishing pure rename from rename plus edit.
A trustworthy absent prior descriptor yields `introduced`; duplicates or unavailable
analysis stop the lineage with an explicit boundary and no introduction claim.
Entering a nested scope stops rather than fabricating introduction. There is no
same-name cross-file move inference, symbol-rename identity or global ID promise.

Evolution validates targets lexically, rejects absolute/parent paths, accepts `.`
and does not canonicalize or require target existence. It aggregates matching file
events, including additions, deletions, modifications, mode/type changes and renames
entering/leaving selectors. Selectors remain fixed paths; they do not follow file
lineage or infer directory identity. Symbol and relation occurrence totals use only
available comparisons; unavailable/unsupported/oversized/non-UTF-8/syntax-damaged
sides retain Git evidence and coverage gaps. Ambiguous keys are counted. File totals
in output describe reported events, including the whole compared file for history.

Co-change counts each other scoped path once per inspected target-event commit;
rename endpoints in the tracked file pair are excluded. A change to another symbol
in the lineage file alone does not produce symbol co-change. Evolution excludes all
selector descendants. Ordering is count descending, native path ascending; the
inspected target-event-commit denominator, retained-result omissions and same-commit
meaning are explicit. This is not dependency, causality or test coverage.

## Bounds

| Command-wide resource | Limit |
|---|---:|
| Inspected commits | default 128; valid 1–512, plus one metadata lookahead |
| Commit metadata, including immutable root verification | 1 MiB |
| Unscoped diff records | 50,000 |
| Unscoped diff path slots | 16 MiB |
| Raw diff bytes | 64 MiB |
| Historical source bytes, including committed HEAD lookup | 256 MiB |
| Snapshot file entries, including repeated sides | 20,000 |
| Cumulative symbols plus graph relation records | 200,000 |
| Selected file-pair comparisons | 4,096 |
| Events | 8,192 |
| Repeated event subject/path/gap text before cloning | 2 MiB |
| Distinct retained co-change candidates | 4,096 |
| Rendered co-change candidates | 100, with omitted count |
| Human / JSON output | 16 MiB, conservative preflight and bounded serialization |

Existing per-diff/tree/source/index/graph bounds additionally apply. Unrelated raw
deltas are charged before scope filtering. Actual source loads spend one shared
command budget; duplicate snapshots do not reset it. Structural records and event
text are bounded separately from source bytes. A user window exhaustion yields a
valid truncated result; hard safety exhaustion fails before stdout. Lineage stopping
at introduction or a coverage boundary before exhausting the window is not labeled
window truncation. Limits are plain test-injectable values, without a service or
concurrency framework.

## Verification

Used the existing isolated macOS toolchain; no global setup changed:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
./scripts/verify.sh
cargo check --all-targets
cargo build --release
git diff --check
```

Baseline verifier passed **99 unit / 61 integration tests**. Final verifier passed
**104 unit / 64 integration tests**, formatting, Clippy with warnings denied and
context checks. All-target check, release build and whitespace check passed.
Intermediate fixture/Clippy issues were corrected before the final gate.

Focused evidence uses one deterministic multi-commit Git fixture across three
integration tests, plus compact adapter/budget/output unit tests:

- introduction, unrelated commits, source modification, co-change, pure file rename,
  rename plus modification, merge-first-parent behavior and branch-commit exclusion;
- qualified/local/file lookup, ambiguity/not-found errors, dirty source disagreement,
  byte-identical JSON, escaped hostile subjects and nested scope boundaries;
- deleted selectors, directory aggregation, unsupported/too-large/invalid-UTF-8/
  syntax-damaged history, historical duplicate descriptors and fail-closed lineage;
- relation occurrence additions and Unix mode/type/symlink evidence;
- injected metadata/diff record/path/raw/source/file/structural/comparison/event/
  event-text/co-change limits, retained co-change omission and output preflight;
- injected non-UTF-8 native paths without depending on filesystem name acceptance;
- populated cache file bytes unchanged after both historical queries;
- all existing diff/impact and Phase 0–7 regression suites.

Release smokes used `/tmp/context-phase8-smoke.py`, with an external isolated cache,
for **15 successful invocations**: seven fixture/window/dirty checks and four each
on this repository and the real sibling Context Integrity repository. Both commands
and both output formats passed. The fixture had identical results after contradictory
dirty source; first-parent branch exclusion, introduction, rename and window
truncation assertions passed. Queries created no cache files. Debug CLI fixture
execution is also covered by the integration suite.

At engine baseline `ef44712`, `history snapshot` reached introduction after eight
commits with two events; `evolution src` inspected 16 commits and emitted 111 file
events. At Context Integrity `cdee5befa78ccac3bbc4825ad7e0a943e406596a`,
`history parse_config` inspected 14 commits with ten events; evolution of
`src/context_integrity` inspected 16 commits with 85 events. Relation coverage was
frequently unavailable and remained explicit rather than inferred. These are local
observations, not a general performance benchmark. No cloud/provider, Linux,
Windows, cross-platform CI or independent executable verification was performed.

## Deviations and limitations

The plan's architecture is retained with smaller shared functions rather than a
new revision framework. Both commands share a plain domain result with optional
symbol descriptor and explicit separate rendering DTOs. There is no new dependency,
crate, temporal schema, async/runtime, MCP, worker pool or historical centrality.

Additional conservative choices: reject shallow/grafted ancestry; charge unscoped
raw changes; bound repeated event text before cloning; preserve scope-entry rename
boundaries; expose whole-file structural totals alongside the tracked-symbol edit
flag. Evolution reports file events and aggregate counts, rather than introducing
an additional temporal entity model. Source-body comparison intentionally counts
formatting/source-slice edits under the existing Phase 4 contract.

HEAD resolution reads its scoped committed inventory, and selected before/after
snapshots may reread the same immutable blob. Existing per-snapshot graph
construction is reused for consistency, even though history does not traverse it.
No historical persistence or performance optimization beyond selective comparisons
is promised. Co-change covers inspected target-event commits only, and unsupported
analysis can limit conclusions. Git objects are immutable inputs; this does not
promise an atomic snapshot of concurrent Git administrative metadata changes.

Fresh independent Phase 8 review is required. Earlier pending Phase 7/6 re-reviews
and other phase dispositions remain pending; local verification does not accept them.
