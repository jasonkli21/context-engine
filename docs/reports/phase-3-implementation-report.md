# Phase 3 implementation report

**Date:** 2026-10-15 (America/Los_Angeles)  
**Starting commit:** `09efd226d49357869af4f9dcf66997eefced5a8a`  
**Implementation commit:** the single commit containing this report (resolve with
`git log -1 --format=%H -- docs/reports/phase-3-implementation-report.md`). A commit
cannot contain its own final hash.  
**Disposition:** implemented and locally verified; independent Sol High review
required. This report is implementation evidence, not independent acceptance.

## Gate and baseline

The maintainer explicitly requested execution of the supplied Phase 3 prompt and
one commit. That prompt authorizes starting from the Phase 2 review-fix baseline
while final independent Phase 1/2 dispositions remain pending. No newer explicit
blocking instruction was found. The earlier Phase 2 review verdict is preserved.
The actual HEAD matches the planning baseline exactly. The only starting working
tree change was the supplied untracked Phase 3 plan; it is included in this commit.
No material baseline drift occurred.

The first verifier invocation could not find Cargo on the default PATH. Reusing
the existing isolated toolchain passed the pre-edit canonical verifier: 41 unit
and 33 integration tests. No global toolchain configuration changed.

## Delivered composition

`main` routes search through `index_repository` (symbol-only analysis, no relation
extraction). `query::search` borrows the canonical bounded index. Neighborhood and
hotspots use the existing scanner → shared symbol/relation analysis → graph path.
`query` owns search and structural aggregation; `graph` retains its existing shared
iterative BFS, with a separate neighborhood eligibility policy. Output adapters
borrow these results and reuse entity/symbol/range/relation DTO vocabulary and the
existing bounded JSON serializer. Domain types have no Serde or Clap dependency.

No filesystem walk, source load, parse, Git call, or source execution occurs in
query/output code. No second repository state owner or relation representation was
introduced. Existing `deps`, `callers`, `related` DTOs and eligibility rules remain
unchanged. Their internal root results also retain bounded distance/predecessor
maps, without adding those fields to existing public JSON.

## CLI and semantics

All commands accept `[PATH]` defaulting to `.`, `--format human|json`, and the
existing `--max-parse-file-size BYTES` (default 1 MiB).

| Command | Additional options |
| --- | --- |
| `ctx search QUERY [PATH]` | `--kind all|symbol|file` (all), `--limit 1..200` (20) |
| `ctx neighborhood [PATH]` | exactly one of `--symbol NAME`, `--file RELATIVE_PATH`; `--depth 0..8` (1) |
| `ctx hotspots [PATH]` | `--limit 1..200` (20, applied independently to symbols and files) |

Search is case-sensitive metadata search. Tiers are exact qualified symbol, exact
local symbol, exact file path; then qualified/local/file prefix; then
qualified/local/file substring. One entity retains its strongest tier. Ties use
canonical index order (path, range, kind, qualified name for symbols). IDs follow
the graph's canonical repository/file/symbol layout and remain index-local. No
source-body, fuzzy, semantic, or history search exists. Empty/oversized queries
fail; zero matches succeed.

Neighborhood roots use existing qualified-first/local-fallback bounded lookup or
exact native relative file identity. Empty, absolute, parent and explicit leading
current-directory file selectors fail validation. File roots have zero or one
match. Traversal follows file/symbol and symbol/symbol containment, references in
both directions, and unresolved outgoing facts as leaves. Every repository-sourced
edge is excluded; repository containment cannot bridge unrelated files. BFS
records shortest distance and one deterministic predecessor/relation chain per
visited entity, per root. Depth zero retains the root only. Normal depth cutoffs
are query scope, not truncation; safety/root omissions set truncation flags.

Candidate test associations are reached entities whose owning indexed file has
`FileRole::Test`, including a test-role root itself. Their IDs refer to visits with
predecessor/relation evidence in the same group. No filename similarity, unresolved
name binding, or directory proximity becomes a coverage claim.

Metrics make one relation pass. BTree path lookup and distinct source/target sets
add logarithmic factors; there is no per-symbol full-graph rescan. Incoming and
outgoing resolved reference occurrences remain separate from distinct fan-in/out.
Unresolved references/imports and direct child symbols are separate counts.
File symbol counts come from canonical indexed symbols. Each non-containment
occurrence contributes activity once to each associated file; same-file references
increment both incoming and outgoing component counts, but activity only once.
Distinct fan-in/out are symbol metrics and are omitted on file JSON entries.

Central symbols rank by `(fan_in, incoming occurrences, fan_out, outgoing
occurrences)` descending, canonical symbol order ascending. Files rank by
`(activity, incoming+outgoing occurrences, unresolved references+imports,
symbol count)` descending, canonical path ascending. No floats or hidden weights.
Symbols without resolved evidence and files without structural activity are
omitted. Empty lists succeed. These are current syntactic graph summaries, not
historical hotspots or semantic importance.

## Resources and contracts

All Phase 2 scanner, index, graph, text, diagnostic and fail-closed protections
remain intact. New query text is limited to 4096 UTF-8 bytes; returned search
matches, central symbols and hotspot files are independently capped at 200.
Candidate sorting uses bounded index/graph IDs rather than copied source strings.

Neighborhood reuses the same 128 roots, 10,000 visited entity occurrences and
25,000 emitted relation occurrences shared across root groups, maximum depth 8.
Distances/predecessors are bounded by those visits. Test association output is a
subset of retained visits. Metrics allocate per bounded entity and retain at most
two distinct-set entries per resolved reference occurrence.

New explicit command DTOs use `{schema_version: 1, command, data}` envelopes.
Existing command shapes are unchanged. New JSON uses the existing 64 MiB bounded
UTF-8 writer, including pretty-printing/escaping, before stdout. Human output
escapes repository/query-controlled text. Search and hotspots expose list
truncation; neighborhood preserves both root and traversal truncation.

## Local verification

Used the existing cached aarch64 macOS toolchain:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
export CARGO_NET_OFFLINE=true
./scripts/verify.sh
cargo check --all-targets
cargo build --release
```

Final canonical gate: context checks, formatting, warnings-denied Clippy, **43 unit
and 37 integration tests passed** (17 scanner, 7 symbols, 9 graph, 4 intelligence).
All-target check and release build passed. `git diff --check` passed.

Focused production-path tests in `tests/intelligence.rs` cover strongest-match
deduplication, exact/prefix/substring reasons, deterministic ties/output, filters,
zero matches, exact/over result bounds including the 200 ceiling, 4096-byte query
boundary, duplicate roots, depth 0/1/8, self-cycles, locality, shortest predecessor
chains, reached/unrelated test entities, repeated versus distinct references,
unresolved components, containment exclusion, zero-evidence output, CLI errors,
and control escaping. The shared traversal unit test injects exact/over root,
visited and emitted budgets. A new adapter test checks exact/over escaped serialized
bytes through the existing writer. Existing Phase 0–2 tests remain passing.

No cloud/provider, cross-platform CI, independent executable verification, or
independent acceptance was run or claimed.

## Release smoke observations

Ran 20 successful final release invocations: each of the following in human and
JSON forms on this repository and the real sibling Python Context Integrity
repository. The latter used `load_inventory` and `src/context_integrity/cli.py`;
this repository used `RepositoryGraph` and `src/graph.rs`.

```text
search graph|context PATH
search RepositoryGraph|load_inventory PATH --kind symbol
neighborhood --symbol RepositoryGraph|load_inventory PATH --depth 2
neighborhood --file src/graph.rs|src/context_integrity/cli.py PATH --depth 2
hotspots PATH --limit 10
```

This repository's `graph` search returned 16 matches and `RepositoryGraph` search
7, neither truncated. Its symbol neighborhood visited 31 entities and its file
neighborhood 44, all in `src/graph.rs`, with no safety truncation or test
associations. Central entries included `write` (fan-in 16, incoming 39), `json`
(10, 20), and `run` (8, 16). Structural file leaders included `tests/cli.rs`
(activity 185, incoming/outgoing 73/73, unresolved references/imports 94/18),
`src/scan.rs` (91, 10/10, 63/18), and `src/graph.rs` (90, 18/18, 62/10).
The limit-10 hotspot lists reported truncation.

Context Integrity's `context` search returned 15 matches without truncation. Its
file neighborhood visited 13 entities, all in `src/context_integrity/cli.py`, with
no test associations or safety truncation. Structural file leaders included
`tests/test_lifecycle.py` (activity 315, incoming/outgoing 97/97, unresolved
references/imports 185/33), `tests/test_authority.py` (189, 17/17, 145/27), and
`src/context_integrity/cli.py` (143, 11/11, 104/28). Central entries included
`write` (fan-in 15, incoming 17), `config` (14, 14), and `edge` (13, 31).
The limit-10 lists reported truncation. These are single-run functional
observations, not performance benchmarks.

## Deviations, limitations and Phase 4 seam

The neighborhood policy stays in the existing graph traversal rather than moving
or duplicating BFS into `query.rs`. This is the smallest way to reuse exactly the
Phase 2 accounting and preserve existing query results. The new query module owns
search/metrics; new output adapters reuse the graph DTO helpers. File ranking omits
zero-activity files rather than filling the limit with containment-only entries.
These are implementation choices permitted by the plan, with no material scope
expansion. Architecture/PRD durable intent did not change.

Existing extraction/parse recovery and unsupported-language limitations remain.
Relation-budget failures discard that file's relation facts; symbol information
can remain. Rankings describe retained evidence, not complete semantic behavior.
Imports remain unresolved; only unique exact same-file qualified references
resolve. Test associations are therefore usually absent across files. Test-role
classification itself remains the scanner's path heuristic. IDs may renumber,
paths remain lossy at presentation boundaries, and filesystem scans are not atomic.

Phase 4 can reuse search and bounded graph traversal/metrics over future revision
models. No revision identity, structural diff, impact command, history, persistence,
concurrency, MCP, network, model calls, or Context Integrity policy behavior was
added. Independent Sol High review remains required before Phase 3 acceptance.

## Implementation acceptance audit

| Area | Actual code/tests/evidence |
| --- | --- |
| Gate | Explicit user prompt authorization; matching starting HEAD; passing baseline verifier above |
| Search | `query::search`, index-only main dispatch; intelligence search/filter/bounds tests |
| Neighborhood | Shared `RepositoryGraph::query_with_limits`; duplicate-root/depth/cycle integration checks |
| Repository-hub policy | Neighborhood excludes repository-sourced edges; unrelated file/test exclusion assertions |
| Metrics | One relation pass in `query::hotspots`; repeated/distinct/unresolved count assertions |
| Central symbols | Explicit lexicographic tuple; deterministic and 200-result checks |
| Structural hotspot files | Per-file occurrence components; current-graph labels and activity assertions |
| Test associations | Reached test-role visits with predecessor/relation IDs; unrelated test exclusion |
| Resource bounds | Existing lower limits unchanged; injected shared traversal bounds; query/list/JSON byte tests |
| Determinism | Canonical IDs, sorted sets/maps, explicit tuple ties; repeated byte-identical JSON checks |
| JSON contract | Explicit DTOs and schema-v1 envelopes; reused entity/relation meanings and bounded writer |
| Security/read-only behavior | Query/output borrow in-memory data; no source execution/network/persistence added; escaping tests |
| Regression compatibility | All unchanged Phase 0–2 suites pass; old query DTOs/eligibility preserved |
| Verification | Canonical verifier, all-target check, release build, 20 final release smoke invocations |
| Documentation | README commands/semantics, supplied active plan committed, this report, current-state updated |
