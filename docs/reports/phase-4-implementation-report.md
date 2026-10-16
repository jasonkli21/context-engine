# Phase 4 implementation report

**Date:** 2026-10-15 (America/Los_Angeles)
**Actual starting commit:** `865f5a4ddb103529966a2310f1607d636d8afd15`
**Implementation commit:** the single commit containing this report; resolve with
`git log -1 --format=%H -- docs/reports/phase-4-implementation-report.md`.
A commit cannot embed its own final hash.
**Disposition:** implemented and locally verified; fresh independent review required.
This report does not independently accept Phase 4 or any previous phase.

## Authorization and baseline

The maintainer explicitly requested Phase 4 implementation according to the
attached prompt and a single commit. This authorizes proceeding while Phase 3
re-review and Phase 1/2 final independent dispositions remain pending. The actual
HEAD matched the plan's baseline. The only pre-existing working-tree change was
the supplied untracked Phase 4 plan, included in this implementation commit with only trailing Markdown whitespace
normalized for the whitespace gate. No newer fixes were reset or overwritten. Existing Phase 3 coverage,
path normalization and named predecessor/test evidence remain intact.

## Delivered architecture

Production CLI composition is:

```text
main / Clap diff|impact
  -> change::analyze
  -> git::revision: resolve explicit range, bounded raw diff and tree/blob reads
  -> classify + index::analyze_loaded (shared closure-based source-loader seam)
  -> graph::from_analysis (same construction, resolution and budgets)
  -> file-paired symbol slices + structural relation occurrence multisets
  -> impact only: graph::traverse_roots (existing BFS)
  -> output::change explicit DTOs / shared bounded JSON writer
```

`src/git_revision.rs` is a narrow submodule of the existing Git boundary; old
metadata probing and old output DTOs are unchanged. `src/change.rs` owns the
change application/domain types; `src/output_change.rs` is an output submodule.
No new dependency, trait hierarchy, parser, BFS, crate or service was introduced.

Diff analyzes only changed sides. It uses two small changed-inventory graphs for
existing same-file relation resolution/containment semantics, rather than full
revision graphs. Impact inventories both full scoped committed trees. Both modes
use the same registry, extraction, role/language rules, symbol/relation budgets
and graph constructor as current-state analysis. Snapshot metadata identifies
its commit; dirty/branch metadata is inapplicable. Source bodies are retained
only for changed-side slice comparison and cleared before returning the change
set. Output never accesses sources or Git.

Per-file maps avoid rescanning all graph symbols/relations for every change;
file/relation analysis lookups use sorted path identity. Structural endpoint
names share immutable `Rc<str>` storage, so repeated references do not copy a
potentially large qualified name for every occurrence. Numeric symbol-vector
indices address a single snapshot internally; pairing uses `(kind,
qualified_name)` content within the Git-evidenced file pair. No base/head numeric
IDs are compared for lineage.

Unique matched symbols are modified only when their validated UTF-8 source slices
differ. Added/removed keys are reported on their own side. Duplicate keys are
counted as ambiguous without matching them. Symbol renames remain removed/added.
Relation signatures include source/target structural descriptors, kind,
resolution, metadata and derivation, excluding occurrence position and numeric
IDs. File endpoints normalize within the Git pair, including renames. Deltas
retain counts and up to three source-range examples per side, with omitted
example counts. Repository/file containment is represented by file changes.

Impact seeds directly changed file entities and confidently changed symbols.
Seeds select IDs from their own already-built revision graph, without name
fallback or binding guesses. Each output seed identifies its file change and,
when applicable, symbol change. Neighborhood traversal excludes the repository
hub and preserves BFS distances, predecessor relation occurrences and truncation.
The base traversal consumes a shared budget first; head receives the remainder.
Reached test-role visits and directly changed test/doc sides are explicit
candidates. Imports remain unresolved, and references retain only the existing
unique exact same-file qualified-name heuristic.

## CLI and contracts

```bash
ctx diff BASE..HEAD [PATH] --format human|json --max-parse-file-size BYTES
ctx impact BASE..HEAD [PATH] --depth 0..8 --format human|json --max-parse-file-size BYTES
```

`PATH` defaults to `.`, format to human, parse size to 1 MiB, impact depth to 1.
Both commands require an explicit two-dot range. Empty endpoints, controls,
leading option-like revisions, repeated separators and triple-dot are rejected.
Each endpoint is resolved separately to a full commit object ID before large
Git operations. Equal endpoints succeed with no changes.

Nested scopes emit normalized scope-relative paths. Native non-UTF-8 path
identity is retained through parsing/matching; lossy output is flagged and those
paths are not structurally parsed. File evidence includes status, before/after
paths, modes, object IDs and rename similarity. Pure mode changes are labeled
`mode_changed`; mixed content/mode changes retain both modes. Symlinks/gitlinks
remain file records and are never followed. Cross-scope renames become additions
or deletions within the selected scope.

Schema-v1 `diff`/`impact` envelopes include repository/scope, original range,
resolved commits, committed semantics, fixed rename settings, ID semantics,
analysis scope, per-side status/coverage, unavailable comparison counts and
ambiguity counts. Impact adds side-tagged roots, seed omissions, visits,
predecessors, relation DTOs and test flags. Human output escapes controlled text
and includes paths, ranges, status, relation metadata and predecessor evidence.
All old command schema-v1 shapes remain unchanged.

## Safety and bounds

| Resource | Ceiling |
| --- | --- |
| Range text | 4 KiB |
| Changed records | 10,000 |
| Changed path bytes | 4 MiB, conservatively counting before/after paths |
| Raw diff stdout | 16 MiB |
| Revision inventory | 50,000 entries / 4 MiB scope-relative path bytes |
| Raw tree stdout | 16 MiB per revision |
| Loaded Git source | 256 MiB per revision |
| Source file parsing | Existing configurable size ceiling, default 1 MiB |
| Analyzer / graph | Existing Phase 1/2 per-file and repository ceilings |
| Impact traversal | 128 roots / 10,000 visits / 25,000 relations total across both sides |
| Depth | Existing maximum 8 |
| JSON | Existing 64 MiB bounded writer before stdout |

Every new Git subprocess has a stdout byte ceiling. Large outputs use piped
`Read::take(limit+1)`; overflow kills/reaps the child and fails the operation.
Stderr is discarded rather than captured without a bound. Commit-resolution,
root and size responses also have small explicit ceilings. Raw blobs bypass
checkout filters. External diff/textconv are disabled, submodule modes retained,
rename detection fixed at 50% with limit 10,000 and the Myers algorithm selected.
Ambient `GIT_*` overrides are removed; locale is fixed, optional locks/fsmonitor/
prompts disabled, replacement objects ignored and lazy object fetching disabled.
There is no shell interpolation, analyzed-code execution, checkout/stash/reset,
worktree creation, index mutation, network requirement or repository write path.

Git/object failures and fatal revision/index/graph/output safety failures do not
produce partial JSON. Unsupported/generated/vendor, too-large, invalid UTF-8,
non-regular and non-UTF-8 path sides remain visible as unavailable structural
analysis. Recovered syntax errors suppress symbol comparison for that pair.
Ambiguous symbols or unsupported/failed relation extraction suppress that pair's
relation comparison instead of inventing removals. Impact may still expose
retained graph facts with explicit coverage incompleteness. Safety truncation
is distinct from the requested normal depth cutoff.

## Verification actually run

Used the pre-existing isolated aarch64 macOS toolchain and cached dependencies;
no global installation/configuration was changed:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
export CARGO_NET_OFFLINE=true
./scripts/verify.sh
cargo check --all-targets
cargo build --release
git diff --check
```

The canonical verifier passed with **57 unit and 50 integration tests**, including
all unchanged Phase 0–3 regressions. All-target check, release build and whitespace
check passed. An earlier verifier also passed all 47 existing unit / 40 existing
integration tests after the first production implementation pass. This was not a
pre-edit baseline run. No cloud/provider, cross-platform CI or independent
executable verification was performed.

New focused tests cover:

- exact/invalid range grammar and revision text ceilings;
- raw NUL add/delete/modify/rename/type/mode parsing and malformed records;
- exact/over changed count/path, tree entry/path, raw diff/tree stdout and aggregate
  source boundaries using injected small limits and tiny real Git fixtures;
- subprocess stdout and escaped DTO serialization exact/over byte ceilings;
- conservative body edits, symbol add/remove, duplicates, relation multiplicity,
  source-position movement and pure same-blob file rename;
- committed-blob isolation after dirty edits, unchanged Git index/HEAD bytes,
  nested scopes and cross-boundary renames;
- real base removed/head added/both modified symbol seeds, shortest predecessor
  references, depth zero, repository-hub exclusion, reached tests and changed docs;
- shared traversal budgets within and across both snapshots, explicit production
  root omission counts, and deterministic human/JSON;
- too-large/invalid UTF-8, unsupported/generated/vendor, recovered syntax,
  unsupported relation candidates, non-regular mode and control escaping;
- distinct invalid UTF-8 native Git path identities and explicit file-level
  omission coverage via synthetic inventories (the local filesystem need not
  support creation of such filenames);
- malicious Git filters/external diff/textconv/fsmonitor and ambient Git redirection
  cannot affect the committed output or create the fixture sentinel.

## Release smoke observations

Eight release invocations passed: both commands in human and JSON on each of:

1. A deterministic temporary Git fixture with a deliberately dirty checkout,
   before/after functions, repeated calls, a pure rename, changed test and README.
2. This repository's actual
   `ff9ad64bd290f71c3c1b6536f0907ca32dfc351b..865f5a4ddb103529966a2310f1607d636d8afd15`
   Phase 3 fix range, while the Phase 4 checkout was dirty.

Impact used depth 2. Fixture: 4 files, 5 symbol changes, 5 relation count deltas;
one documentation pair structurally unavailable, zero ambiguity. The pure rename
had no symbol/relation churn. Each revision had 8 seeds, 25 visits, zero omissions
and no traversal truncation. The real range: 14 files, 40 symbol changes, 6
unavailable symbol comparisons, zero ambiguity; no relation deltas were inferred
where relation comparisons were incomplete. Base: 32 seeds / 554 visits. Head:
54 seeds / 951 visits. No safety truncation. This repository contains unsupported
syntax candidates, so sparse relation results are explicitly qualified, not a
claim of no relation change. Outputs were inspected under
`/tmp/context-engine-phase4-smoke`; these are temporary observations rather than
canonical state or performance benchmarks.

## Plan adaptations and limitations

- The Git object adapter is split into a narrow submodule to keep the existing
  metadata code separate. The planned shared analyzer and BFS seams are delivered
  without a broad refactor or a new identity-layout formula.
- Diff uses changed-file-only graphs to reuse exact relation resolution; it does
  not build full repository graphs. Snapshot trees include tracked entries
  irrespective of checkout ignore controls, as intended by the plan.
- Diff enumeration is repository-wide and bounded before scope filtering. This
  preserves cross-scope Git rename pairing but means unrelated changes can exhaust
  the diff record/path/output limit. Output and revision tree inventories remain
  scoped. This is an intentional, documented scope/budget tradeoff.
- Incomplete relation extraction or symbol ambiguity suppresses all relation
  deltas for that file pair, a conservative choice rather than guessing which
  omitted facts would have changed. Syntax-damaged pairs suppress all symbol
  deltas even when the parser recovered some symbols.
- Impact uses the existing neighborhood semantics, not a separate dependency
  prediction. File seeds intentionally expose broader local neighbors, including
  unchanged symbols; modified symbol seeds still provide their own groups.
  Base-first allocation can exhaust the shared budget before head traversal;
  head omissions and truncation are explicit.
- There is no history/blame, dirty/staged/untracked diff, triple-dot merge base,
  compiler/cross-file binding, symbol rename lineage, semantic compatibility,
  policy, task score, persistent index, async/concurrency, MCP or model behavior.
  Non-UTF-8 paths can collide in lossy display only; flags qualify presentation.

No durable PRD/architecture decision changed, so those intent documents were not
rewritten. Phase 5 can consume the in-memory change set, revision-local symbol
changes and bounded impact evidence as deterministic ranking signals. Source
bodies, task ranking, token budgets, persistence and a service contract are not
added to that seam. Fresh independent Phase 4 review is the next step; Phase 3
re-review and Phase 1/2 final dispositions remain pending.
