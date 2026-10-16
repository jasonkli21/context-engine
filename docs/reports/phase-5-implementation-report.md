# Phase 5 — Task Context Ranking implementation

**Date:** 2026-10-16\
**Starting commit:** `4579d5bcfef0a7b8e4bada93da8b0722c39d37d3`\
**Disposition:** implemented and locally verified; fresh independent review required

The first independent review found ranking-evidence, load-work-bound, ranker-version,
and benchmark-validity issues. The original benchmark metrics are recorded as
superseded in the [Phase 5 review-fix report](phase-5-review-fix-report.md); the
corrected benchmark below replaces them. This report's initial verification
figures remain author-side evidence for the original implementation.

The maintainer directly authorized Phase 5 while Phase 4 fresh re-review and
prior final dispositions remain pending. Authorization is not independent
acceptance of either phase. The scheduled implementation began at the requested
October 16, 2026, 01:30 PDT / 08:30 UTC run.

## Baseline and architecture

HEAD matched the planning baseline. The only pre-existing worktree addition was
`docs/plans/phase-5-implementation-plan.md`. It is included with this implementation
as its requirements document; only Markdown hard-break whitespace was normalized
to pass the diff whitespace gate. No unrelated changes were present.
The attached implementation prompt was read from the maintainer-provided Downloads
path; it is not copied into a second repository ruleset.

`src/context.rs` consumes the current canonical `RepositoryGraph`. It normalizes
bounded task text, collects file/path and symbol evidence, computes explicit
integer components, reuses bounded neighborhood traversal, and selects complete
UTF-8 files through `source::load`. Scoring performs no filesystem reads.
`src/output_context.rs` owns separate schema-v1 DTOs and human rendering; output
code never reads files. `cli.rs`/`main.rs` compose validation, one current graph,
optional committed-change analysis, selection and rendering.

`query::structural_metrics` is extracted from hotspots without changing its
algorithm. Ranker metrics and traversal share one entity-to-file owner map.
Existing query coverage adapters are reused for analysis/incompleteness reporting.
There is no second current index/graph, new dependency, framework, persistence,
async, thread, model call, embedding, or execution of analyzed code.

## CLI and task contract

```bash
ctx context [PATH] --task TASK --budget BYTES \
  [--change BASE..HEAD] [--format human|json] \
  [--max-parse-file-size BYTES]
```

Tasks require 1–4096 original UTF-8 bytes. Whitespace-normalized Unicode lowercase
text supplies a phrase signal. Tokens retain `_ - . / :` and additionally split
those separators into components. Empty and one-character terms are omitted,
except a single-character task. Unique terms are sorted canonically; the first
64 are retained and term truncation is explicit. A task with no usable evidence
can enter fallback; no stopword list or semantic interpretation is added.

Budgets require 1–16,777,216 bytes. `budget.unit = "utf8_content_bytes"` measures
only selected raw UTF-8 contents; metadata and human/JSON framing do not consume
that content budget. Complete files are greedily considered in rank order. An
oversized high-ranked file is skipped while later smaller files remain eligible.
Empty relevant files cost zero and are subject to count bounds.

## Ranker version, scores and ordering

**Version:** `file-ranker-v1`. Recommended weights were retained; no benchmark
weight tuning was performed.

| Component | Rules | Cap |
| --- | --- | --- |
| lexical_path | exact path component/stem 30; prefix 18; substring 10; whole normalized phrase hit +12 | 60 |
| lexical_symbol | qualified exact 45; local exact 40; qualified/local prefix 26; substring 15; strongest tier per symbol, aggregate strongest eight | 90 |
| graph | minimum distance 1 → 20 or 2 → 10 to a distinct file owner; seed-owner identity receives no graph score | 20 |
| role | non-direct test reached through qualifying graph evidence 12; direct test match 8; directly matched doc/config/schema 6 | 12 |
| change | unique changed-symbol owner 40; direct changed file 32; distinct-owner changed-root distance 1 → 20, 2 → 10; strongest applicable value | 40 |
| centrality | min(file structural activity, 4) + min(incoming references, 3) + min(outgoing references, 3) | 10 |

Each nonzero component has stable reason codes, including `path_component_exact`,
`symbol_qualified_exact`, `graph_distance_1`, `related_test_candidate`,
`direct_task_role_match`, `changed_symbol_owner`, and `centrality_activity`.
A candidate's total is the bounded sum (maximum 232). The v1 weights, reason
vocabulary, root priorities, fallback cap, and sorting key are centralized beside
the ranker and pinned by a contract regression. Sorting descends by total,
symbol, path, change, graph, centrality and finally ascends by native relative path.
Matched symbols are sorted by strength then canonical symbol order; all matching
symbols can seed traversal, independently of the eight-symbol explanation cap.

Generated/vendor roles are excluded. Normal candidates require task, graph or
change relevance; centrality alone does not make unrelated files candidates.
If no lexical/change seeds exist, explicit `fallback_mode = "centrality"` retains
the first 20 centrality-ranked files, with canonical path ties and omitted counts.
Zero-activity fallback files may have score zero; no task evidence is invented.

Symbol roots have priority over file roots; exact/prefix/substring strength and
current changed-symbol seeds determine deterministic root priority. One traversal
call uses fixed depth 2 and shares existing ceilings of 128 roots, 10,000 visits,
and 25,000 relation occurrences across lexical and change roots. Change-origin
roots are tagged so lexical-only roots cannot create change-distance evidence.
Traversal/root omissions and fallback omissions are exposed. Repository containment
is excluded, and unresolved imports/calls remain unresolved.

## Optional committed-change gate

Only explicit `--change` invokes Phase 4. The current graph must have Git metadata,
a current HEAD, and `is_dirty == Some(false)`. Phase 4 resolves the two-dot range;
its head must equal current HEAD. A bounded metadata-only index listing rejects
assume-unchanged/skip-worktree/unmerged entries and scoped gitlinks; submodule
content is not traversed to establish cleanliness. The metadata/head/cleanliness gate is rechecked
after content loading, before output. A failing/unknown gate produces a command
error with empty stdout rather than silently dropping the requested signal.

File mapping uses native head-relative path. Changed-symbol mapping reads head
snapshot descriptors and looks up unique current `(path, kind, qualified_name)`
keys. Numeric IDs are never reused across snapshots. Phase 4 ambiguity suppresses
symbol boosts; file-level changes remain eligible. Output exposes observed files
and changed-symbol descriptors, current mappings, unmapped symbols, ambiguous
keys, unavailable symbol comparisons and deleted-only entities. Deleted-only
files/symbols never fabricate content. `diff` and `impact` are unchanged.

## Content and output safety

The existing source loader retains relative-path, parent/file symlink,
regular-file, bounded read and UTF-8 checks. Reads use the smaller of remaining
content budget and the 16 MiB per-file ceiling; no mid-file truncation is selected.
`budget_exceeded` takes precedence over decoding an oversized candidate. Otherwise
`too_large`, `invalid_utf8` and `read_failed` are recorded and selection continues.
This avoids materializing a large rejected file merely to learn it cannot fit.

Selected count is capped at 200; matched symbol details at eight per file; skip
details at 50. Omission counters survive those caps. Candidate count inherits the
existing 50,000-entity graph ceiling. Reasons have a fixed finite code vocabulary.
Non-UTF-8 paths retain native internal identity and explicit lossy display flags;
textual path evidence omits them and reports the count.

Candidate loading has separate operation-wide safety limits: at most 256 attempts
and at most 64 MiB of conservatively reserved read allowance. Before each call to
`source::load`, selection reserves `min(remaining content budget, per-file limit)
+ 1` bytes, matching the loader's maximum read including its over-limit probe.
This allowance is charged even when metadata rejects a file before reading or a
read fails early. If another reservation would exceed either cap, selection stops
and human/JSON coverage reports the attempts, reserved bytes, safety truncation,
and number of unattempted candidates. The public content budget still counts only
selected UTF-8 bytes.

A conservative escaped-size estimate runs before allocating output DTO collections
or a human buffer. Both formats obey the shared 64 MiB materialization ceiling;
JSON also uses the bounded serializer. Overflow fails before stdout. The estimate
can reject a large otherwise-renderable bundle because it reserves worst-case
escaping. Human source retains tabs/newlines, escapes other controls, and prefixes
every source line with `|` inside file boundaries. JSON contains the exact decoded
source string. Current index-local IDs and full analysis/graph coverage are explicit.

## Deterministic expected-file-recall benchmark

The benchmark is the production-composition unit test
`context::tests::expected_file_recall_benchmark`, reproducible with:

```bash
cargo test --bin ctx expected_file_recall_benchmark -- --nocapture
```

The corrected fixture contains 22 files of exactly 128 bytes. It covers four
source/test tasks in Rust and Python plus setup and protocol docs/config/schema
tasks. Each source/test pair contains a source declaration and a test that imports
and exercises it; the test does not define a duplicate task-named helper. Expected
paths are literal values authored in the test. The test file itself has no task
symbol match, which the benchmark asserts through the existing symbol-search API.
It is expected because its path directly matches the task and its file role is
useful. It is not presented as a graph-reached test or as coverage evidence.

All five strategies use the same eligible-file policy, production
`select`/`source::load` behavior, full-file packing and 256-byte budget. The
pre-Phase-5 baseline uses the existing path-and-symbol `query::search` results
mapped to owning files. Other baselines use canonical path order and path-only
lexical scoring. A role-removed production ranking is also included as a
mutation-style check that the measured incremental signal affects recall.

| Task | Expected files | Production | Existing path+symbol search | Path order | Path-only | Production without role | Bytes per strategy |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cache | src/cache.py; tests/test_cache_contract.py | 2/2, 1.0 | 1/2, 0.5 | 0/2, 0.0 | 0/2, 0.0 | 2/2, 1.0 | 256 |
| router | src/router.rs; tests/router_contract.rs | 2/2, 1.0 | 1/2, 0.5 | 0/2, 0.0 | 0/2, 0.0 | 2/2, 1.0 | 256 |
| retry | src/retry.py; tests/test_retry_policy.py | 2/2, 1.0 | 1/2, 0.5 | 0/2, 0.0 | 0/2, 0.0 | 2/2, 1.0 | 256 |
| codec | src/codec.rs; tests/codec_roundtrip.rs | 2/2, 1.0 | 1/2, 0.5 | 0/2, 0.0 | 0/2, 0.0 | 2/2, 1.0 | 256 |
| setup | docs/setup.md; config/setup.toml | 2/2, 1.0 | 1/2, 0.5 | 0/2, 0.0 | 1/2, 0.5 | 1/2, 0.5 | 256 |
| protocol | docs/protocol.md; schema/protocol.json | 2/2, 1.0 | 1/2, 0.5 | 0/2, 0.0 | 1/2, 0.5 | 1/2, 0.5 | 256 |
| **Aggregate** | **12 files / six scenarios** | **12/12; macro 1.0** | **6/12; macro 0.5** | **0/12; macro 0.0** | **2/12; macro 0.166667** | **10/12; macro 0.833333** | **1536 per strategy** |

The corrected fixture supports a bounded claim: in these six authored scenarios,
production ranking beats existing path-and-symbol search, path order, and
path-only retrieval under identical byte budgets. Removing file-role scores drops
recall from 1.0 to 0.833333, with both supporting-file scenarios losing one hit.
It does **not** demonstrate cross-file graph retrieval:
the current resolver supplies same-file relationships, and graph scores now
require reaching a distinct file owner. These results are synthetic and do not
establish broad natural-language or real-world retrieval quality.

## Verification actually run

Using the existing isolated macOS stable Rust toolchain and cached dependencies
(`PATH=/tmp/context-engine-cargo/bin:$PATH`, `CARGO_HOME=/tmp/context-engine-cargo`,
`RUSTUP_HOME=/tmp/context-engine-rustup`), without changing global setup:

- Baseline `./scripts/verify.sh`: passed, 58 unit / 53 integration tests.
- Final `./scripts/verify.sh`: passed, 65 unit / 56 integration tests.
- `cargo check --all-targets`: passed.
- `cargo build --release`: passed.
- `git diff --check`: passed.

Focused tests cover task/term bounds, lexical tiers, caps/ties, symbol owners,
graph/test/role signals, centrality/fallback, root/explanation/count omissions,
exact multibyte budgets, skip-and-continue, invalid UTF-8/disappeared files,
human controls, JSON determinism/schema, output preflight boundaries, and actual
Git clean/dirty/unknown/head mismatch/rename/deletion/ambiguity mappings, plus
hidden index flags and scoped submodule rejection. A
non-UTF-8 filename could not be created under the desktop sandbox; its coverage
regression injects the native-path index model instead, like the existing search
regression. It does not claim filesystem creation of such a name was verified.
All Phase 0–4 suites remain green. No cloud, provider, cross-platform, or
independent executable verification is claimed.

## Release smoke observations

Eleven release-binary invocations covered human plus repeated deterministic JSON
on this Rust repository, the real sibling Python Context Integrity repository,
and a clean temporary Git fixture. JSON checks recomputed selected UTF-8 byte
sums and checked reasons, schema, and budget bounds. The fixture accepted
`HEAD~1..HEAD` in both formats and rejected a mismatched head and dirty checkout
with empty stdout. On the fixture, `hydrate` selected `z.py`, 45/128 bytes, with
unique changed-symbol evidence. The Python repository's `evidence validation`
task selected `integrity.py` and `evidence.py`, using 16,611/24,000 bytes.
The final Rust `context ranking` smoke selected `src/main.rs` and
`src/output_context.rs`, using 23,014/24,000 bytes with nine budget skips.
Repeated JSON runs were byte-identical within each unchanged repository/scope.

## Adaptations and limits

The review-fix pass corrected seed-owner graph scoring and bounded selection work;
details and verification are recorded in the linked review-fix report. Durable
architecture and PRD intent remain unchanged. The current constraints are:

1. Content reads are bounded by remaining bytes, so over-budget rejection precedes
   full UTF-8 validation instead of reading complete oversized files.
2. With repository containment excluded, existing resolved graph edges are
   confined to a single file. The review fix excludes the seed's own file from
   graph evidence and requires a traversed relation to a distinct file owner.
   Current data therefore supplies no cross-file graph boosts or related-test
   candidates. The corrected benchmark exercises path, symbol and file-role
   evidence and makes no graph-retrieval claim.

3. Candidate content loading reserves a separate operation-wide allowance before
   calling the safe loader: at most 256 attempts and 64 MiB of potential read
   bytes. Exact selected content bytes remain governed by the public budget.

4. A context-only bounded Git index check strengthens the existing metadata
   probe: hidden index flags, unmerged entries and scoped submodules fail closed.
   Ordinary `diff`/`impact` and other metadata probe behavior remain unchanged.

The filesystem is not an atomic snapshot. Git cleanliness uses the existing
metadata probe plus the context-specific index check; ignored files may still
exist, and cleanliness is not a byte identity guarantee against concurrent
mutation. Rust/Python remain the parser set; imports are unresolved and calls resolve only by unique exact same-file
qualified name. Docs/config/schema have direct path/role evidence, not document
relations or freshness/authority. Tests are candidates, not coverage. Byte budgets
are not tokens. Full-file packing may waste space or skip a highly relevant large
file. Root/visit/output caps may omit useful evidence; those conditions are
reported. No persistence, semantic vector search, history or cross-repository
context is implemented.

## Next step

A fresh independent Phase 5 review must challenge both implementation and plan,
including benchmark representativeness, snapshot compatibility, graph usefulness
at file granularity and safety accounting. Phase 5 is not independently accepted;
Phase 4 re-review and earlier final dispositions remain pending.
