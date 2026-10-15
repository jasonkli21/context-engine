# Phase 2 Independent Review — Repository Relationship Graph
## With Phase 0–1 Cross-Phase Audit and Sol Medium Fix Handoff

**Repository:** `jasonkli21/context-engine`  
**Reviewed commit:** `16f7088b5e44621f0e4a3b076c6528003adbba9e` (`feat: implement phase 2 repository relationship graph`)  
**Parent / Phase 2 baseline:** `b132b8e61843918e374598f05f17edef3862e174` (`fix: address phase 1 review findings`)  
**Review date:** 2026-10-14 / 2026-10-15 UTC boundary  
**Reviewer role:** independent post-implementation review  
**Review scope:** Phase 2 implementation plus a fresh cross-phase audit of the delivered Phase 0–2 stack  
**Intended next consumer:** fresh **Sol Medium Codex** fix session  
**Repository artifact target:** `docs/reviews/phase-2-independent-review.md`

---

# 1. Executive assessment

## Verdict

**NOT READY FOR PHASE 3**

Phase 2 is architecturally strong in several important ways:

- graph mode reuses the canonical Phase 0 scanner;
- each supported file is loaded once and parsed once for graph mode;
- symbols and relation candidates are derived from the same source/tree;
- Tree-sitter types remain inside language analysis;
- graph identity is separate from `qualified_name`;
- containment is lexical rather than fabricated semantic ownership;
- unresolved evidence remains first class;
- imports remain unresolved;
- adjacency is built only after final deterministic relation sorting;
- traversal is iterative and cycle-safe;
- Phase 1 per-file symbol/name/qualification limits remain present;
- Phase 2 adds real per-file relation budgets and fail-closed relation extraction;
- existing Phase 0/1 public commands are not rewritten around the graph.

However, the graph is not yet a sufficiently trustworthy substrate for Phase 3 because the review found **four High-severity resource-boundary defects and three Medium-severity evidence/correctness defects**.

The most important cross-phase result is that the current resource story is only locally bounded:

- Phase 0 bounds line counting **per file**, but repository discovery/materialization has no global budget;
- Phase 1 bounds symbol work **per file**, but standalone `index` / `symbols` / `symbol` have no repository-wide symbol/name/output ceiling;
- Phase 2 bounds relation work **per file** and counts graph entities/relations, but those caps are applied after unbounded scanning and do not bound aggregate relation text or full JSON output size;
- Phase 2 traversal budgets do not bound the number of matching roots or the number of root result groups emitted.

There are also three evidence correctness issues:

- the unique-local fallback can resolve an unqualified call such as `unique()` to a class/impl method merely because that method name is unique in the file;
- call evidence ranges point at the entire call expression, not the callee syntax required by the plan;
- Rust grouped imports containing `self` normalize the target incorrectly.

These are not reasons to discard the Phase 2 architecture. The core design is good. They are reasons to close the remaining trust-boundary and evidence gaps before Phase 3 starts building change intelligence or higher-level repository reasoning on top of the graph.

## Finding counts

| Severity | Count |
| --- | ---: |
| Critical | 0 |
| High | 4 |
| Medium | 3 |
| Low | 0 |
| Future considerations | 4 |

## Phase 3 readiness

**NOT READY FOR PHASE 3**

Blocking findings:

- `CE-P2-001`
- `CE-P2-002`
- `CE-P2-003`
- `CE-P2-004`
- `CE-P2-005`
- `CE-P2-006`
- `CE-P2-007`

The four High findings must be fixed before Phase 3.

The three Medium findings should also be fixed before Phase 3 because Phase 3 will depend on relationship identity/evidence being trustworthy rather than merely labeled best-effort.

## Highest-risk themes

1. **End-to-end resource limits are incomplete.** Per-file limits do not bound repository-wide discovery, indexing, graph text, query roots, or serialized output.
2. **Heuristic resolution is too eager for methods/nested symbols.** The graph can create a resolved edge where the syntax does not justify one.
3. **Evidence spans are not exact.** Call relation ranges point at the whole call rather than the callee.
4. **One common Rust grouped-import form is normalized incorrectly.**
5. **Tests are strong but currently encode one incorrect semantic assumption and miss the cross-phase aggregate-limit cases.**

## Independent verification status

### Static review

Completed against the reviewed commit and its Phase 0–2 plans/reports/code/tests.

### Reviewer-side executable verification

**Blocked by the review environment.**

Attempted:

```bash
cargo --version
```

Result:

```text
cargo: command not found
```

Attempted:

```bash
git clone --no-checkout https://github.com/jasonkli21/context-engine.git ...
```

Result:

```text
Could not resolve host: github.com
```

The connected GitHub repository interface was available for source/commit inspection.

No workflow run or combined commit status was associated with the reviewed commit at review time.

Therefore this review does **not** independently claim execution of:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

The Phase 2 implementation report states that those local checks passed using the author's isolated Rust toolchain, including 32 unit tests and 31 integration tests. Those are delivery claims, not independently reproduced reviewer evidence.

---

# 2. Review scope and evidence

## Reviewed revision

Target commit:

```text
16f7088b5e44621f0e4a3b076c6528003adbba9e
feat: implement phase 2 repository relationship graph
```

Parent:

```text
b132b8e61843918e374598f05f17edef3862e174
fix: address phase 1 review findings
```

The Phase 2 commit is exactly one commit ahead of the Phase 1 review-fix baseline.

The Phase 2 change adds or materially changes:

```text
README.md
docs/current-state.md
docs/plans/phase-2-implementation-plan.md
docs/reports/phase-2-implementation-report.md
src/cli.rs
src/graph.rs
src/index.rs
src/language/mod.rs
src/language/python.rs
src/language/relations.rs
src/language/rust.rs
src/main.rs
src/model.rs
src/output.rs
src/output_graph.rs
src/output_index.rs
src/relations.rs
tests/graph.rs
tests/graph_fixtures/relations.py
tests/graph_fixtures/relations.rs
```

## Canonical documents consulted

- `AGENTS.md`
- `docs/README.md`
- `docs/current-state.md`
- `docs/prd.md`
- `docs/architecture.md`
- `docs/plans/phase-0-implementation-plan.md`
- `docs/plans/phase-1-implementation-plan.md`
- `docs/plans/phase-2-implementation-plan.md`
- `docs/reports/phase-0-implementation-report.md`
- `docs/reports/phase-1-implementation-report.md`
- `docs/reviews/phase-1-independent-review.md`
- `docs/reports/phase-1-review-fix-report.md`
- `docs/reports/phase-2-implementation-report.md`
- `README.md`

## Production code inspected

Phase 0 / common substrate:

- `src/scan.rs`
- `src/git.rs`
- `src/model.rs`
- `src/output.rs`

Phase 1:

- `src/source.rs`
- `src/index.rs`
- `src/language/mod.rs`
- `src/language/rust.rs`
- `src/language/python.rs`
- symbol queries
- `src/output_index.rs`

Phase 2:

- `src/relations.rs`
- `src/language/relations.rs`
- `src/graph.rs`
- `src/output_graph.rs`
- `src/cli.rs`
- `src/main.rs`
- `tests/graph.rs`
- graph fixtures

## External grammar evidence checked

The Tree-sitter Rust grammar/corpus corresponding to the repository's Rust grammar dependency was inspected for grouped-use syntax.

The upstream corpus includes a valid form equivalent to:

```rust
use sayings::english::{self, greetings as en_greetings, farewells as en_farewells};
```

This matters to `CE-P2-007`: the current implementation treats the grouped `self` child as an ordinary path segment and produces a target ending in `::self`, which is not the imported target represented by that use tree.

## Phase 0/1 prior review status

Phase 0's earlier five review findings were reported fixed and Phase 0 was explicitly accepted by the maintainer.

Phase 1's earlier independent review found:

- missing per-file symbol/qualification resource bounds;
- misleading Rust reference/pointer impl qualification;
- one source-loading documentation overclaim.

The parent baseline `b132b8e...` contains the reported fixes:

- max 4,096 symbols/file;
- 4 MiB local/qualified-name bytes/file;
- 25,000 qualification-node visits/file;
- Tree-sitter match-state guard kept separate;
- fail-closed file behavior on symbol analysis budget overflow;
- conservative Rust reference/pointer impl-target handling;
- narrowed filesystem mutation wording.

This Phase 2 review did **not** find those specific Phase 1 fixes reverted.

The new cross-phase findings are additional system-level limits not covered by the earlier per-file review.

---

# 3. Delivered architecture reconstruction

## Production flow

Actual graph/query path:

```text
CLI
 ↓
main::graph_command
 ↓
graph::build
 ↓
index::graph_analysis
 ↓
scan::scan_repository
 ↓
for each supported analyzable file
    source::load
     ↓
    LanguageAnalyzer::analyze_graph
     ↓
    one Tree-sitter parse
     ├── symbol query / qualification
     ├── syntax diagnostics
     └── relation extraction from same Tree
 ↓
RepositoryIndex + per-file relation candidates
 ↓
graph::construct
 ├── repository/file/symbol entities
 ├── lexical containment
 ├── same-file target maps
 ├── reference resolution
 ├── relation sorting
 └── final forward/reverse adjacency
 ↓
graph::query for deps/callers/related
 ↓
output_graph DTO adapters
 ↓
human or schema-v1 JSON
```

## Same-source / same-tree invariant

**Pass.**

`SyntaxAnalyzer::analyze_mode(..., graph = true)` parses one `text` value once.

It performs symbol extraction and then:

```rust
result.relations = Some(relations::extract(tree.root_node(), text, &language));
```

against the same `tree` and `text`.

The implementation does not reopen/reparse each file in a separate graph pass.

This avoids merging symbols from working-tree content A with relations from content B.

## Domain boundaries

### Phase 1 domain

`RepositoryIndex` owns:

- file analysis records;
- symbols;
- diagnostics;
- scan warnings;
- optional Git metadata.

It does not retain Tree-sitter trees.

### Phase 2 domain

`RepositoryGraph` owns:

- `RepositoryIndex`;
- entity vector;
- relation vector;
- relation-file accounting;
- forward adjacency;
- reverse adjacency.

`EntityId` and `RelationId` are newtype integer indexes.

`Target` is either:

```text
Resolved(EntityId)
Unresolved(String)
```

Extraction derivation and resolution quality are separate enums.

Tree-sitter `Node` values remain in `src/language/...`.

## Entity construction

Entity ordering is:

```text
0              repository
1..F           files sorted by relative path
F+1..F+S       canonical sorted symbols
```

Symbol entity identity is represented by the canonical symbol position, whose structural meaning is file + kind + declaration range.

Names are used for lookup/resolution, not node identity.

## Relation construction

For each file:

1. repository → file containment;
2. lexical symbol containment;
3. import/reference candidate ownership by lexical range;
4. same-file reference resolution;
5. relation accumulation;
6. deterministic relation sort;
7. relation IDs assigned by final sorted vector position;
8. adjacency built from final IDs.

This ordering correctly avoids stale adjacency IDs after sorting.

## Query semantics

`deps`:

- follows outgoing references;
- includes unresolved imports/references attached to visited source entities;
- does not follow containment.

`callers`:

- follows incoming resolved call-context references only.

`related`:

- follows resolved relations in both directions;
- therefore includes containment and references;
- attaches unresolved outgoing facts as leaves.

Traversal is iterative `VecDeque` BFS with per-root visited/emitted sets plus shared counters.

The root list itself is not bounded; see `CE-P2-003`.

---

# 4. Acceptance-criteria coverage

| Area | Status | Review conclusion |
| --- | --- | --- |
| Phase 1 baseline | **Pass** | Phase 2 starts from `b132b8e...`; prior Phase 1 fixes remain present. |
| Phase 0 trust-boundary regressions | **Partial** | No Phase 2 regression found in old fixes, but fresh cross-phase review finds missing repository-wide discovery budgets (`CE-P2-001`). |
| Same source / same tree | **Pass** | Symbols and relations derive from one source/tree in graph mode. |
| Architecture separation | **Pass** | Canonical scanner reused; graph does not print; output does not parse; Tree-sitter types stay internal. |
| Entity identity | **Pass** | Names are not graph identity; duplicate names remain distinct; IDs are explicitly index-local. |
| Relation occurrence identity | **Pass** | Repeated syntactic occurrences remain separate; relation sort precedes ID/adjacency assignment. |
| Evidence model | **Partial** | Derivation vs resolution is clean, but call range is wrong (`CE-P2-006`). |
| Lexical containment | **Pass (static)** | Nearest lexical parent is used; Rust struct does not fake-own separate impl methods. |
| Containment complexity | **Pass (static)** | Range index avoids pairwise O(S²) source ownership. |
| Rust imports | **Partial** | Main matrix is handled, but grouped `self` is normalized incorrectly (`CE-P2-007`). |
| Python imports | **Pass (static)** | Dotted/from/relative/alias/wildcard forms are conservatively normalized; future imports explicitly unsupported. |
| Reference extraction | **Partial** | Conservative syntax set, but evidence range is whole call (`CE-P2-006`). |
| Same-file resolution | **Fail** | Unique-local fallback can resolve methods/nested symbols without binding evidence (`CE-P2-005`). |
| `deps` honesty | **Pass** | Help/README describe best-effort outgoing call references rather than a complete semantic dependency graph. |
| `callers` honesty | **Pass** | Human/help wording says syntactic/best-effort callers. |
| `related` | **Pass (static)** | Resolved bidirectional graph traversal with unresolved leaves. |
| Forward/reverse adjacency | **Pass (static)** | Final sorted relation IDs are used; unresolved edges are forward-only. |
| Self-loops / cycles | **Pass (static)** | Relations preserved; iterative visited-state traversal. |
| Per-file symbol limits | **Pass** | Prior Phase 1 fix remains. |
| Per-file relation limits | **Pass (static)** | Candidate/byte/visit/segment/target limits and fail-closed extraction present. |
| Repository-wide scanner limits | **Fail** | No global discovery/path/work bound; graph cap is post-scan (`CE-P2-001`). |
| Repository-wide Phase 1 index limits | **Fail** | Standalone index commands aggregate unbounded bounded files (`CE-P2-002`). |
| Global graph memory/output limits | **Fail** | Entity/relation counts do not bound relation text / serialized output (`CE-P2-004`). |
| Traversal budgets | **Fail** | Edge/entity walk is bounded, root fan-out/output is not (`CE-P2-003`). |
| Determinism | **Pass (static)** | Explicit sorts restore deterministic entity/relation/query ordering for bounded inputs. |
| JSON contract | **Partial** | Explicit DTOs/no source bodies, but worst-case full/query JSON output is not meaningfully resource-bounded. |
| No target-code execution | **Pass (static)** | No Python imports, Cargo target builds, language server calls, shell evaluation, or network behavior. |
| Human control escaping | **Pass (static)** | Repository-controlled graph labels/targets/query text pass through control escaping. |
| Existing Phase 0/1 DTO compatibility | **Pass (static)** | New graph DTOs are separate. |
| Test quality | **Partial** | Strong happy/adversarial coverage, but one test encodes the false unique-local resolution and aggregate/root/evidence/grouped-self cases are missing. |
| Documentation truthfulness | **Partial** | Mostly careful; multi-root amplification and graph "safety ceiling" claims are stronger than implementation guarantees. |
| Phase 3 readiness | **Fail** | Resource safety and evidence correctness require fixes. |

---

# 5. Findings

## CE-P2-001 — Repository discovery is unbounded before graph/index safety checks

**Severity:** High  
**Category:** security / resource behavior / cross-phase architecture  
**Blocks Phase 3:** yes  
**Origin:** inherited Phase 0 behavior exposed by Phase 1/2 scale

### Evidence

Primary code:

```text
src/scan.rs
  scan_repository
  reject_symlink_ignore_files

src/index.rs
  index_repository
  graph_analysis
  analyze_scan

src/graph.rs
  build
```

Observed behavior:

- `scan_repository` appends every discovered regular file to `scan.files`;
- it appends every discovered directory to `scan.directories`;
- recoverable errors may append warnings;
- all three collections are sorted after traversal;
- there is no project-owned maximum discovered entry count;
- there is no cumulative path-byte budget;
- there is no repository-level warning/materialization budget;
- line counting is bounded per file but not by an aggregate repository read budget;
- `reject_symlink_ignore_files` performs a separate full preflight traversal with no project-owned repository entry/work ceiling;
- only **after** `scan_repository` returns does graph mode check:

```text
scan.files.len() + 1 > MAX_ENTITIES
```

Thus the Phase 2 graph entity cap cannot protect the memory/work used to construct the Phase 0 `RepositoryScan`.

`scan`, `stats`, and `tree` have no later graph cap at all.

### Concrete failure mode

A repository-controlled directory tree containing a very large number of:

- empty directories;
- tiny regular files;
- long relative paths;
- recoverable traversal warnings;

can make the process allocate and sort very large vectors before any graph safety ceiling is consulted.

A repository containing mostly directories is especially important:

- directories consume Phase 0 memory/work;
- directories are not graph entities;
- therefore the Phase 2 file/entity cap does not even correlate with that amplification.

This is an end-to-end trust-boundary gap.

### Why it matters

The project treats repository contents as untrusted input.

A per-file byte cap does not bound:

```text
number of files × per-file work
number of directories
total path bytes
warning count
sorting work
```

Phase 3 will add more repository-scale processing. Carrying an unbounded canonical discovery substrate forward would make later "safe" limits misleading.

### Expected behavior

Canonical repository discovery should have explicit project-owned aggregate ceilings.

If discovery cannot complete within those ceilings, commands that require a complete inventory should fail clearly rather than silently return a partial repository.

The limit must apply **before or while** entries are accumulated, not after the full scan is already materialized.

### Recommended fix

Introduce a small internal scan-budget model, for example conceptually:

```rust
struct ScanLimits {
    max_entries: usize,
    max_path_bytes: usize,
    max_warnings: usize,
    max_line_count_bytes: u64,
}
```

Exact fields/constants may differ, but the fix must meaningfully bound repository-controlled aggregate work.

Requirements:

1. Bound total materialized file + directory entries.
2. Bound aggregate owned relative-path bytes or an equivalent memory dimension.
3. Bound warning accumulation or fail once the scan itself is no longer trustworthy.
4. Apply compatible work ceilings to `reject_symlink_ignore_files`.
5. Consider aggregate line-count I/O rather than relying only on the per-file 1 MiB cap.
6. Check/reserve before pushing owned records where practical.
7. Return a typed deterministic fatal error for completeness-required commands.
8. Do not silently truncate `scan`, `tree`, `index`, or graph input and then claim a complete repository.
9. Keep limits internal in this phase unless there is a concrete reason to expose new CLI knobs.
10. Preserve all prior ignore/symlink semantics.

A test-only `scan_with_limits(...)` or equivalent injection seam is preferable to production tests that create enormous repositories.

### Required validation

Targeted tests should cover:

- exact total-entry budget succeeds;
- one entry over fails before returning partial output;
- directories count toward the budget;
- files count toward the budget;
- cumulative path-byte exact/over boundary;
- preflight ignore-control traversal respects a bounded work limit;
- warning amplification cannot grow without bound;
- ordinary small repositories remain byte-deterministic;
- Phase 0 existing symlink/ignore regression tests continue passing.

Then run the full repository gate.

---

## CE-P2-002 — Phase 1 standalone index commands have no repository-wide symbol/name/output ceiling

**Severity:** High  
**Category:** security / resource behavior / cross-phase Phase 1  
**Blocks Phase 3:** yes  
**Origin:** Phase 1 per-file fix did not close aggregate repository amplification

### Evidence

Primary code:

```text
src/index.rs
  index_repository
  analyze_scan

src/language/mod.rs
  AnalysisLimits
  AnalysisBudget
  SyntaxAnalyzer::analyze_mode

src/model.rs
  RepositoryIndex

src/output_index.rs
  json
  human
```

The prior Phase 1 fix correctly adds per-file ceilings:

```text
max_symbols = 4,096
max_name_bytes = 4 MiB
max_qualification_nodes = 25,000
```

However `analyze_scan` only applies repository-global entity/relation checks inside:

```rust
if graph { ... }
```

Standalone:

```text
ctx index
ctx symbols
ctx symbol
```

use:

```rust
analyze_scan(..., graph = false)
```

and repeatedly:

```rust
index.symbols.extend(result.symbols);
index.diagnostics.extend(result.diagnostics);
```

with no project-owned repository-global ceiling.

After accumulation:

```rust
index.sort();
```

sorts the complete aggregate result.

`output_index::json` can then serialize:

- every symbol for `symbols`;
- every exact query match for `symbol`;
- all diagnostics;
- all scan warnings.

### Concrete failure mode

A repository can contain many individually valid, individually bounded files.

Each file may legally produce thousands of symbols and MiBs of materialized names.

The Phase 1 per-file fix therefore changes:

```text
unbounded per file
```

into:

```text
bounded per file × unbounded number of files
```

for standalone index commands.

This remains unbounded repository-controlled memory/work/output.

### Why it matters

This is the same security lesson as Phase 1's original `CE-P1-001`, just one layer higher:

> bounded input units do not imply bounded aggregate derived output.

Phase 3 will reuse the repository index. The common index substrate should have repository-level budgets before more derived analyses are added.

### Expected behavior

Repository indexing should have an aggregate project-owned budget independent from graph mode.

When a global indexing budget is exceeded:

- fail clearly;
- do not return a silently partial `RepositoryIndex`;
- do not label all prior files as a complete repository index;
- retain deterministic behavior.

### Recommended fix

Add shared repository-analysis limits used by both `index_repository` and `graph_analysis`.

At minimum bound aggregate:

- symbol count;
- materialized local + qualified-name bytes or equivalent;
- diagnostics/detail count/bytes if they can aggregate materially;
- any other retained Phase 1 owned text whose per-file ceiling can multiply across files.

Possible shape:

```rust
struct RepositoryAnalysisLimits {
    max_symbols: usize,
    max_symbol_name_bytes: usize,
    max_diagnostics: usize,
}
```

Do not simply reuse `MAX_ENTITIES` by accident unless the semantics are deliberately unified and documented.

The implementation should reserve/check before extending the canonical vectors.

Prefer a typed error such as:

```text
repository analysis safety limit exceeded
```

rather than misclassifying this as a parse failure on one file.

### Required validation

Add small injected-limit tests covering:

- exact global symbol boundary;
- one symbol over;
- many files each below the per-file limit but over the global limit;
- aggregate name-byte boundary;
- no partial `symbols`/`symbol` JSON on fatal repository-global overflow;
- Phase 1 per-file overflow behavior remains unchanged;
- graph mode still respects the stricter relevant graph caps.

---

## CE-P2-003 — Multi-root query output bypasses traversal budgets

**Severity:** High  
**Category:** security / resource behavior / query correctness  
**Blocks Phase 3:** yes

### Evidence

Primary code:

```text
src/graph.rs
  RepositoryGraph::lookup
  RepositoryGraph::query_with_limits

src/output_graph.rs
  json
```

`lookup` scans the complete symbol index and collects **all** matching roots:

```rust
.collect()
```

into a `Vec<EntityId>`.

`query_with_limits` shares:

```text
total_entities
total_relations
```

across roots.

That correctly limits graph expansion.

However, after the entity budget is exhausted, the outer loop continues through every remaining root and still appends:

```rust
RootResult {
    root,
    entities: ...,
    relations: ...,
    truncated: true,
}
```

The complete root vector also remains in:

```text
QueryResult.roots
```

`output_graph::json` serializes:

```text
roots: every root ID
results: one result object per root
```

even when most roots were never traversed.

The production graph cap allows up to roughly one million entities.

### Documentation mismatch

The Phase 2 implementation report states:

```text
Shared counters bound multi-root result amplification.
```

That is only true for visited entities and emitted relations.

It is not true for:

- root-ID collection;
- number of root groups;
- root-group JSON objects;
- root grouping/sorting/materialization.

README wording similarly presents the traversal budgets as bounding query results generally.

### Concrete failure mode

Construct many same-file or cross-file symbols with the same local name, for example thousands/tens of thousands of `helper` definitions while staying under other repository caps.

Then:

```bash
ctx deps helper ...
ctx callers helper ...
ctx related helper ...
```

may:

1. collect all matching roots;
2. preserve all roots in the response;
3. emit one result group for every root;
4. set many groups to truncated after the shared visited budget is exhausted.

The query has exceeded its intended result bound even though edge traversal is bounded.

### Why it matters

A query budget should bound the complete attacker-controlled response surface, not only BFS expansion.

The current behavior can still produce a very large allocation/JSON response using only root multiplicity.

### Expected behavior

Root matching must be explicitly bounded.

If not all matching roots are returned, output must say so.

The implementation must preserve the product invariant that returned roots remain distinct groups.

### Recommended fix

Introduce a root-match/result-group budget.

A robust design should distinguish:

```text
root matching truncated
```

from:

```text
graph traversal truncated
```

Possible approach:

- scan the symbol index while counting all matches;
- retain only up to `MAX_QUERY_ROOTS`;
- expose an additive machine field such as:

```text
root_match_count
roots_truncated
```

- set overall `truncated = true` whenever roots or traversal are truncated;
- build `RootResult` only for retained roots.

If changing the public DTO minimally is preferable, at least:

- cap roots;
- set overall truncation truthfully;
- document that `roots` may be a bounded prefix.

However, an explicit `roots_truncated` flag is clearer for downstream consumers.

The root budget may be shared conceptually with `MAX_VISITED`, but do not let implementation convenience blur the difference between:

```text
matched query roots
```

and:

```text
entities reached by traversal
```

### Required validation

Add tests for:

- exact root cap;
- one root over;
- many duplicate local names;
- deterministic retained-root ordering;
- overall truncation;
- per-root result count never exceeds returned root count;
- JSON output does not contain a result object for omitted roots;
- `--file` narrowing is applied before root-budget retention if that preserves the intended filtering semantics;
- zero and ordinary multiple roots remain unchanged.

---

## CE-P2-004 — Entity/relation count caps do not meaningfully bound graph memory or full JSON output

**Severity:** High  
**Category:** security / resource behavior / output  
**Blocks Phase 3:** yes

### Evidence

Primary code:

```text
src/graph.rs
  MAX_ENTITIES = 1,000,000
  MAX_RELATIONS = 2,000,000
  construct

src/language/relations.rs
  per-file aggregate bytes = 4 MiB
  per-target bytes = 4 KiB

src/relations.rs
  Candidate
  Relation target/metadata

src/output_graph.rs
  json
```

The graph has project-owned **record-count** ceilings:

```text
1,000,000 entities
2,000,000 relations
```

Per file, relation normalization also caps materialized target/binding bytes.

But there is no repository-global cap on:

- aggregate unresolved target string bytes;
- aggregate alias string bytes;
- aggregate relation-owned textual material;
- repeated evidence/path text cost;
- serialized full graph JSON bytes.

A count of two million relations is not by itself a realistic memory/output safety bound.

`output_graph::json` additionally builds full entity/relation DTO vectors and then uses:

```rust
serde_json::to_string_pretty(...)
```

which materializes the complete JSON string in memory before stdout writing.

### Concrete failure mode

The current allowed state space can contain very large amounts of owned relation text.

Even if every individual file obeys the 4 MiB relation-text budget, many files can collectively retain a very large graph.

Two million relation records also imply a large JSON document even when targets are short.

Thus:

```text
graph construction limit passed
```

does not imply:

```text
resident memory/output size is safely bounded
```

The README calls the entity/relation caps:

```text
safety ceilings
```

which is stronger than the actual guarantee.

### Why it matters

Phase 2 explicitly adopted the lesson:

> parse-size bounds do not bound derived output/work.

The same principle applies one aggregation level higher:

> relation-count bounds do not bound aggregate relation text or serialized output bytes.

Phase 3 should not inherit graph "safety" constants that permit impractically large materialized graphs.

### Expected behavior

Global graph safety should bound the dimensions that dominate actual memory/output.

The exact constants should be conservative and based on plausible workstation operation, not theoretical integer capacity.

### Recommended fix

Add at least one repository-global materialized-text budget during graph analysis/construction.

Account before storing graph-owned strings where practical:

```text
unresolved target bytes
import binding bytes
other relation-owned text
```

Also make full graph output meaningfully bounded.

Acceptable strategies include a combination of:

1. a global graph text-byte cap;
2. a materially lower relation-record cap;
3. an explicit output-record/output-byte cap;
4. streaming JSON instead of building a second full output string;
5. avoiding repeated owned path text inside internal relations where a file/entity reference is sufficient.

Do **not** perform a broad storage refactor solely for elegance. A targeted global byte budget plus a realistic output ceiling is sufficient for the review fix if it creates a real finite worst-case bound.

Update documentation so "safety ceiling" describes what is actually bounded.

### Required validation

Use injected small limits to cover:

- exact aggregate graph-text boundary;
- one byte over;
- many files each below per-file relation-byte limits but over the repository-global byte cap;
- no partial graph returned on global overflow;
- full graph JSON cannot exceed the intended configured output record/byte envelope without clear failure/truncation semantics;
- ordinary fixtures retain byte-identical JSON.

---

## CE-P2-005 — Unique local-name fallback can resolve unqualified calls to methods/nested symbols without binding evidence

**Severity:** Medium  
**Category:** correctness / evidence semantics / plan assumption  
**Blocks Phase 3:** yes

### Evidence

Primary code:

```text
src/graph.rs
  construct
  per-file `qualified` and `local` maps
```

Resolution logic:

1. try exact same-file `qualified_name`;
2. if target is unqualified, look up the target in the per-file **local name** map;
3. if exactly one symbol anywhere in that file has that local name, resolve it.

The local map contains all symbol kinds/scopes, including:

- Python class methods;
- Python nested functions;
- Rust impl/trait methods;
- Rust nested functions represented by Phase 1;
- other declarations whose local name is not sufficient binding evidence.

The current unit test explicitly endorses this case:

```python
class C:
    def unique(self): pass

def a():
    unique()
```

and asserts that the unqualified `unique()` call resolves to:

```text
C.unique
```

because `unique` is the only local-name match in the file.

That is not justified by Python binding semantics.

The same structural problem exists for Rust impl methods.

### Why it matters

The graph does label this resolution as:

```text
unique_same_file_local_name
```

so it does not claim compiler certainty.

However, the relation is still moved from:

```text
unresolved name evidence
```

to:

```text
resolved EntityId edge
```

and therefore participates in:

- `deps`;
- reverse adjacency;
- `callers`;
- `related`;
- future Phase 3 structural/change analysis.

This converts a weak name coincidence into a concrete graph edge.

For methods/nested declarations this can be a systematic false-positive pattern, not merely an obscure shadowing corner case.

### Plan weakness

The Phase 2 plan explicitly allowed a unique same-file local-name fallback.

The implementation follows that rule.

The review prompt explicitly requires challenging whether that rule becomes too strong once implemented.

It does.

The test above demonstrates that file-wide uniqueness is not a sufficient proxy for lexical binding.

### Expected behavior

Phase 2 should prefer unresolved evidence over a resolved edge when binding scope is not known.

A valid Phase 2 resolved edge should be supported by syntax-local evidence strong enough for the advertised resolution kind.

### Recommended fix

**Preferred Phase 2 fix: remove the file-wide unique-local fallback for call references.**

Keep:

- exact same-file qualified-name matching;
- unresolved target evidence.

This still resolves useful cases:

```python
def helper(): ...
helper()
```

because the top-level function's qualified name is already exactly `helper`.

It also resolves explicit qualified syntax such as:

```python
C.unique()
```

when `C.unique` is an exact same-file qualified name.

It stops guessing:

```python
unique()
```

→ `C.unique`.

Do not add a partial lexical-binding engine merely to preserve the existing fallback unless the implementation remains small, obviously correct, and thoroughly tested.

Lexical scope/binding analysis is a larger semantic feature and can be added later with its own evidence model.

The `UniqueSameFileLocalName` enum/schema value may remain for compatibility/future use even if Phase 2 stops emitting it.

### Required validation

Add tests:

Python:

```python
class C:
    def unique(self): pass

def caller():
    unique()
```

Expected: `unique` remains unresolved.

```python
class C:
    def unique(self): pass

def caller():
    C.unique()
```

Expected: exact qualified match may resolve to `C.unique`.

```python
def helper(): pass

def caller():
    helper()
```

Expected: exact qualified match resolves to top-level `helper`.

Rust:

```rust
struct C;
impl C {
    fn unique() {}
}
fn caller() { unique(); }
```

Expected: unqualified call does not resolve to `C::unique`.

Also verify:

- callers no longer reports false incoming edges from the removed fallback;
- ambiguous same-file names stay unresolved;
- cross-file names stay unresolved.

Update README/report wording if unique-local resolution is no longer emitted in Phase 2.

---

## CE-P2-006 — Call reference evidence range points to the whole call, not the callee

**Severity:** Medium  
**Category:** correctness / evidence contract  
**Blocks Phase 3:** yes

### Evidence

Primary code:

```text
src/language/relations.rs
  Extractor::run
  Extractor::emit
```

For a call node, the implementation does:

```rust
let callee = node.child_by_field_name("function") ...;
callee
    .and_then(|n| self.path(n, python))
    .and_then(|parts| {
        self.emit(
            &parts,
            ...,
            node,
            ...,
        )
    })
```

The `node` passed to `emit` is the **call expression**.

`emit` stores:

```rust
range: source_range(node)
```

Therefore a relation for:

```python
helper(secret, other())
```

has evidence range covering the whole call expression, not just:

```text
helper
```

The Phase 2 plan explicitly specifies:

```text
target_text = normalized callee
evidence range = callee syntax range
```

Existing tests verify that repeated call ranges differ, but do not verify that the range equals the callee span.

### Why it matters

Evidence ranges are part of the graph's provenance contract.

Future consumers may use the range to:

- highlight the syntax that justified the edge;
- slice local evidence;
- compare structural changes;
- hand evidence to Context Integrity.

A range that includes arguments is broader than the syntax used to derive the target.

It also weakens the "do not expose arbitrary call body/arguments" boundary if a future consumer slices source by the relation span.

### Expected behavior

For call references:

```text
range == normalized callee syntax
```

not the entire invocation.

Examples:

```rust
module::helper(arg)
^^^^^^^^^^^^^^
```

```python
Outer.helper(arg)
^^^^^^^^^^^^
```

### Recommended fix

Retain the callee `Node` and pass that node to `emit`.

Do not change target normalization semantics.

Do not use source slicing to reconstruct target text; keep the existing accepted-node normalization.

### Required validation

Add exact-range tests for:

- Rust `helper()`;
- Rust `module::helper(arg)`;
- Python `helper()`;
- Python `Outer.helper(arg)`;
- repeated call sites;
- nested accepted inner calls inside unsupported outer dynamic calls.

Tests should assert exact byte start/end against the callee text, not merely that ranges are distinct.

---

## CE-P2-007 — Rust grouped `self` imports are normalized as `prefix::self`

**Severity:** Medium  
**Category:** correctness / Rust syntax normalization  
**Blocks Phase 3:** yes

### Evidence

Primary code:

```text
src/language/relations.rs
  Extractor::rust_import
  Extractor::path
```

For:

```rust
use sayings::english::{self, greetings};
```

the parser represents the outer grouped prefix separately and the group contains a `self` node.

Current normalization:

1. computes prefix:

```text
sayings::english
```

2. treats group child `self` as an ordinary path segment:

```text
self
```

3. appends it to the prefix;
4. emits:

```text
sayings::english::self
```

That is not the imported target represented by grouped `self`.

In a use tree:

```rust
use sayings::english::{self, ...};
```

`self` imports the prefix itself:

```text
sayings::english
```

not a child literally named:

```text
sayings::english::self
```

The upstream Tree-sitter Rust corpus contains this grouped form, so it is not a fabricated syntax case.

### Documentation mismatch

The implementation report broadly states that Rust import handling covers:

- groups;
- nested groups;
- `crate` / `self` / `super`;
- leading-root paths.

The grouped `self` case is therefore more than an undocumented unsupported form: it can produce a wrong relation target.

### Why it matters

Imports are deliberately unresolved in Phase 2, but their normalized target text is still evidence.

A wrong unresolved target corrupts the observable syntax fact and will also corrupt later module-resolution work if Phase 3+ treats Phase 2 target text as trusted input.

### Expected behavior

At minimum:

```rust
use sayings::english::{self, greetings};
```

should emit atomic import facts equivalent to:

```text
sayings::english
sayings::english::greetings
```

For:

```rust
use sayings::english::{self as en, greetings};
```

the `self as en` target should remain:

```text
sayings::english
```

with alias/binding metadata:

```text
en
```

Top-level:

```rust
use self::x;
```

must remain:

```text
self::x
```

The special handling is contextual to grouped-prefix `self`.

### Recommended fix

In Rust use-tree flattening, distinguish:

```text
grouped `self` meaning "the current prefix"
```

from:

```text
`self` as the root of a normal scoped path
```

Do not globally remove `self` segments.

Handle explicit aliasing of grouped `self` consistently.

Keep the iterative/bounded traversal model.

### Required validation

Add tests for:

```rust
use foo::{self, bar};
use foo::{self as f, bar};
use foo::nested::{self, Thing};
use self::x;
use super::x;
use crate::x::Y;
```

Assert:

- exact target strings;
- alias metadata;
- wildcard behavior unchanged;
- no extra `::self` target for grouped `self`;
- deterministic candidate ordering;
- relation budgets still apply.

---

# 6. Graph identity assessment

## Conclusion

**Graph identity design is sound for Phase 2.**

No actionable identity collision was found.

### Positive findings

- `EntityId` is not derived from name.
- File identity is canonical relative path within the built index.
- Symbol entity position maps to the canonical sorted `Symbol`.
- Duplicate qualified/local names remain separate entity IDs.
- Relation targets use IDs only after explicit resolution.
- IDs are documented as index-local and non-lineage.
- JSON exposes the caveat.
- relation IDs are assigned after deterministic sorting.
- adjacency is constructed from final relation positions.

### Important carry-forward rule

Do not let Phase 3 reinterpret:

```text
EntityId(u32)
```

as an identity stable across edits or rebuilds.

Phase 3 structural change analysis will need a distinct matching/lineage concept rather than persisting Phase 2 vector indexes.

---

# 7. Relationship and evidence assessment

## Containment

The delivered containment model is appropriately lexical.

Notably:

```rust
struct Foo {}

impl Foo {
    fn bar() {}
}
```

does not cause:

```text
Foo --contains--> Foo::bar
```

merely because the qualified name uses `Foo`.

That is correct.

## Imports

### Python

Static inspection indicates the main planned forms are handled conservatively:

- import;
- dotted import;
- alias;
- from-import;
- multiple names;
- relative dots;
- wildcard.

Imports remain unresolved.

### Rust

The main planned simple/scoped/group/wildcard forms are present.

`CE-P2-007` is the important exception for grouped `self`.

## References

The syntax surface is intentionally conservative:

Rust accepts:

- identifier callee;
- scoped identifier callee.

Python accepts:

- identifier;
- identifier-rooted dotted attribute chains.

Dynamic/field/indexed outer calls are counted unsupported rather than guessed.

This is the correct Phase 2 direction.

## Resolution

The exact-qualified match is honest enough because the resolution enum states what happened:

```text
exact_same_file_qualified_name
```

It still does not imply compiler resolution.

The file-wide local fallback is too broad; see `CE-P2-005`.

## Evidence

Derivation and resolution are correctly separated.

The main evidence defect is the overly broad call source range (`CE-P2-006`).

---

# 8. Resource / performance assessment

## Per-file Phase 1 symbol analysis

**Good.**

Prior review fixes remain:

- symbol count ceiling;
- aggregate name-byte ceiling;
- qualification-node ceiling;
- fail-closed file result;
- independent Tree-sitter query match-state guard.

## Per-file Phase 2 relation extraction

**Good in concept and mostly good in implementation.**

Bounded dimensions include:

- candidate count;
- target/binding bytes;
- normalization visits;
- target segments;
- target/alias bytes.

On relation-only limit overflow:

- relation candidates are discarded;
- Phase 1 symbols survive;
- structural containment survives;
- relation status records the failure.

## Repository-wide discovery

**Not bounded.**

See `CE-P2-001`.

## Repository-wide standalone index

**Not bounded.**

See `CE-P2-002`.

## Graph-wide retained text / JSON

**Not meaningfully bounded by record counts.**

See `CE-P2-004`.

## Complexity

No obvious Phase 2 O(R×S) or O(S²) global resolution loop was found.

Positive points:

- symbols are grouped per file;
- same-file name maps avoid scanning all repository symbols per candidate;
- lexical ownership uses an interval/range index;
- reverse adjacency is constructed directly;
- relation sorting is O(R log R), expected for deterministic IDs.

The highest-value performance fixes are therefore aggregate resource budgets, not algorithm replacement.

---

# 9. Traversal / query assessment

## Cycle safety

Pass on static inspection.

Traversal uses:

- iterative `VecDeque`;
- visited sets;
- emitted relation sets;
- depth checks.

Self-loops and mutual cycles are preserved as evidence rather than deleted.

## Edge traversal budget

Pass in isolation.

Shared counters bound:

- newly visited entity occurrences across root groups;
- emitted relation occurrences across root groups.

## Root fan-out

Fail.

`CE-P2-003` shows that root matching/result groups are outside those budgets.

## `deps`

The implementation is narrower than a semantic dependency graph, but public wording is sufficiently careful:

- CLI help says best-effort outgoing call-site references and unresolved facts;
- README explains imports are unresolved;
- containment is not treated as dependency;
- file imports are not inherited by every symbol.

No rename is required in this review.

## `callers`

Public wording is appropriately qualified as syntactic/best-effort.

The important correctness issue is upstream resolution (`CE-P2-005`), not the callers traversal implementation itself.

## `related`

Semantics are coherent for the current graph:

- containment + resolved references;
- both directions;
- unresolved leaves do not become fake graph nodes.

---

# 10. Security / Phase 0–1 regression assessment

## Preserved protections

Static inspection indicates Phase 2 preserves:

- no target code execution;
- no package-manager execution;
- no language-server invocation;
- no source upload/network;
- no parser types in durable public graph objects;
- bounded source load;
- component/symlink checks in Phase 1 loader;
- Phase 0 ignore-control preflight;
- Git clean/process filter safety check;
- human control-character escaping;
- per-file Phase 1 analysis limits.

## New cross-phase issues

The new findings are primarily missing **aggregate** budgets, not regressions of the old local fixes.

That distinction matters:

```text
old fix remains correct
+
system composition still has a larger unbounded dimension
```

Both can be true.

## Accepted limitations not promoted to findings

The following remain documented limitations / future hardening rather than actionable findings in this review:

1. Filesystem operations are not an atomic repository snapshot.
2. Metadata-check/open TOCTOU races are not completely eliminated.
3. Git subprocesses have no timeout.
4. Cross-platform CI was not independently run.

These should remain visible in docs, but this review does not require a Phase 2 fix unless scope changes or public claims become stronger.

---

# 11. Documentation / contract assessment

Documentation is generally unusually explicit for an early-phase project.

Positive wording includes:

- syntax/evidence graph;
- unresolved imports;
- best-effort reference resolution;
- syntactic callers;
- index-local IDs;
- graph rebuilt per command;
- no compiler completeness;
- no atomic working-tree snapshot.

Required corrections after fixes:

## Multi-root resource claim

Current implementation report says:

```text
Shared counters bound multi-root result amplification.
```

That is incomplete until `CE-P2-003` is fixed.

## Graph safety-ceiling wording

README calls:

```text
1,000,000 entities
2,000,000 relations
```

global safety ceilings.

Until `CE-P2-004` is fixed, these are only record-count ceilings.

## Unique-local resolution

If the recommended fix removes file-wide unique-local reference resolution, update:

- README Phase 2 relationship section;
- fix report;
- current state.

Do not retroactively rewrite the independent review or original Phase 2 plan to hide the plan weakness.

---

# 12. Test quality assessment

The Phase 2 tests are stronger than a typical happy-path suite.

Good coverage includes:

- deterministic graph JSON;
- distinct repeated relation occurrences;
- lexical containment;
- Rust separate impl containment;
- alias/wildcard metadata;
- per-file relation limits;
- no partial relation facts after relation limit overflow;
- exact/over traversal entity/relation budgets;
- cycles;
- code execution marker;
- terminal escaping;
- symlink omission;
- file filtering;
- zero roots;
- multiple roots;
- depth zero;
- graph entity/relation cap injection.

Important missing/incorrect tests:

1. No repository-wide scanner entry/path budget test (`CE-P2-001`).
2. No repository-wide standalone Phase 1 symbol/name budget test (`CE-P2-002`).
3. No query root-count overflow test (`CE-P2-003`).
4. No aggregate graph relation-text/output budget test (`CE-P2-004`).
5. Existing resolution test incorrectly expects `unique()` to resolve to `C.unique` (`CE-P2-005`).
6. Call tests check range distinctness, not exact callee span (`CE-P2-006`).
7. Rust import matrix omits grouped `self` (`CE-P2-007`).

The test suite should be changed to detect these bugs rather than only adding tests around existing behavior.

---

# 13. Plan assumptions challenged

## 13.1 Unique same-file local-name resolution is too strong

The Phase 2 plan's Rule 2 says an unqualified target may resolve when one same-file symbol has that local name.

Implementation evidence shows this can create a false resolved edge to:

- a class method;
- an impl method;
- a nested declaration;

without lexical binding evidence.

**Recommendation:** remove the file-wide local fallback in Phase 2 and keep the relation unresolved unless an exact qualified syntactic match exists.

Reintroduce lexical local resolution only when there is an explicit scope/binding model.

## 13.2 Resource limits need a hierarchy, not isolated local caps

Phase 1 and Phase 2 plans correctly learned to bound per-file derived work.

The next design correction is to make limits hierarchical:

```text
per file
repository discovery
repository index
graph retained text/records
query roots
query traversal
output
```

A lower-level bound does not automatically bound the next aggregation level.

This should become a durable implementation principle for later phases.

## 13.3 `deps` naming is acceptable if qualification remains prominent

No rename is recommended now.

The shipped help/README explain the command as best-effort syntactic reference/dependency evidence.

Phase 3 should not silently broaden the meaning of existing results without new derivation/resolution labels.

## 13.4 Manual relation tree walk is an acceptable plan deviation

The Phase 2 plan suggested separate Tree-sitter relation query assets.

The implementation instead performs a project-owned iterative node-kind walk in `src/language/relations.rs`.

That is not inherently a defect:

- Tree-sitter types stay internal;
- normalization is explicit;
- traversal is bounded;
- no upstream query semantics define the public contract.

Do not rewrite this solely to match the plan.

If later relation extraction becomes much larger or harder to audit, separate query assets can be reconsidered.

---

# 14. Phase 3 readiness

## Determination

**NOT READY FOR PHASE 3**

## Safe to carry forward

The following are sound foundations:

- one canonical scanner;
- same-source/same-tree graph analysis;
- language-neutral symbol/graph domain;
- index-local entity IDs;
- structural symbol identity separate from names;
- lexical containment;
- unresolved targets as first-class evidence;
- explicit derivation vs resolution quality;
- final-sort-before-adjacency;
- iterative cycle-safe traversal;
- explicit graph/output DTOs;
- no target execution;
- per-file Phase 1/2 fail-closed analysis budgets.

## Must be fixed before Phase 3

Resource boundaries:

- `CE-P2-001`
- `CE-P2-002`
- `CE-P2-003`
- `CE-P2-004`

Evidence/correctness:

- `CE-P2-005`
- `CE-P2-006`
- `CE-P2-007`

## Do not generalize these Phase 2 assumptions into Phase 3

1. `EntityId` is not lineage.
2. exact same-file name matching is not compiler binding.
3. unresolved import target text is not a resolved module.
4. call references are not a complete call graph.
5. a passed per-file limit is not proof that repository-wide materialization is bounded.
6. a relation-count cap is not a memory/output byte cap.

---

# 15. Sol Medium Fix Handoff

This section is intended to be handed directly to a **fresh Sol Medium Codex session**.

## Mission

Address all actionable findings in this review with targeted changes.

Do **not** implement Phase 3.

Do **not** re-review the entire project unless needed to understand a finding.

Preserve:

- all Phase 0 behavior and prior review fixes;
- all Phase 1 review fixes;
- Phase 2 product scope;
- same-source/same-tree analysis;
- deterministic schema-v1 behavior;
- evidence honesty;
- no target-code execution;
- no async/concurrency/persistence/model work.

## Starting point

Reviewed implementation:

```text
16f7088b5e44621f0e4a3b076c6528003adbba9e
```

Expected parent:

```text
b132b8e61843918e374598f05f17edef3862e174
```

Before editing:

1. inspect current `HEAD`;
2. confirm it contains or descends from the reviewed implementation;
3. if later commits already address any finding, verify them rather than reimplementing;
4. do not discard unrelated later work.

## Required read order

Read:

1. `AGENTS.md`
2. `docs/current-state.md`
3. `docs/reviews/phase-2-independent-review.md`
4. `docs/plans/phase-2-implementation-plan.md`
5. `docs/reports/phase-2-implementation-report.md`
6. `docs/reviews/phase-1-independent-review.md`
7. `docs/reports/phase-1-review-fix-report.md`
8. affected production code/tests
9. `docs/architecture.md` only where a changed limit/contract crosses a durable boundary

Do not load the entire PRD unless a product decision becomes ambiguous.

---

## Fix order

Recommended order:

```text
1. CE-P2-001 — canonical scan aggregate budget
2. CE-P2-002 — repository-wide index aggregate budget
3. CE-P2-004 — graph aggregate text/output budget
4. CE-P2-003 — bounded query roots/result groups
5. CE-P2-005 — conservative reference resolution
6. CE-P2-006 — exact callee evidence range
7. CE-P2-007 — grouped Rust `self` import normalization
```

Rationale:

- 001/002 establish shared end-to-end resource semantics.
- 004 should reuse the same style of aggregate-budget accounting.
- 003 is query/output-specific and should be tested against the corrected repository limits.
- 005–007 are semantic fixes largely independent from the resource work.

The semantic fixes may be implemented in parallel locally, but commit/review them as one coherent review-fix pass unless project workflow requires otherwise.

---

## CE-P2-001 fix handoff — canonical scan aggregate budget

### Desired end state

No command can materialize an unbounded number of repository entries/paths/warnings before higher-level limits run.

### Likely files

```text
src/scan.rs
src/model.rs
src/index.rs
tests/cli.rs
tests/graph.rs
README.md
```

### Important invariants

- scanner remains the single discovery path;
- ignore semantics unchanged;
- symlink semantics unchanged;
- no partial repository silently presented as complete;
- deterministic behavior;
- no user-facing knobs unless clearly justified.

### Implementation constraints

Prefer a simple internal `ScanLimits` / budget struct.

Make production constants explicit and test with injected small limits.

Check before owned vector growth where practical.

Apply a compatible work limit to ignore-control preflight.

Do not merely add a post-scan `len()` check.

### Targeted tests

- exact/over entry count;
- directories included;
- files included;
- path-byte exact/over;
- preflight bounded;
- warnings cannot explode;
- no partial scan/index/graph JSON.

---

## CE-P2-002 fix handoff — repository-wide index budget

### Desired end state

`index`, `symbols`, and `symbol` have finite repository-wide retained symbol/name/diagnostic bounds independent of per-file limits.

### Likely files

```text
src/index.rs
src/model.rs
src/language/mod.rs
src/output_index.rs
tests/symbols.rs
tests/graph.rs
```

### Important invariants

- keep per-file Phase 1 limits unchanged;
- global overflow is not mislabeled as one file's parse failure;
- no partial repository index on fatal aggregate overflow;
- graph mode remains compatible with its stricter graph caps.

### Implementation constraints

Use a shared repository-level analysis budget.

At minimum bound:

```text
symbols
symbol name/qualified-name bytes
diagnostic detail aggregation if materially unbounded
```

Check before extending canonical vectors.

### Targeted tests

Many tiny files, each valid and below per-file limits, should exceed the injected repository budget deterministically.

---

## CE-P2-004 fix handoff — graph aggregate text/output budget

### Desired end state

Passing graph entity/relation caps implies a realistic finite bound on retained graph text and output materialization.

### Likely files

```text
src/index.rs
src/graph.rs
src/relations.rs
src/output_graph.rs
tests/graph.rs
README.md
```

### Important invariants

- per-file relation budgets stay independent;
- global overflow fails clearly;
- no partial graph;
- stable fixtures remain deterministic.

### Implementation constraints

Add a repository-global relation-text/materialization budget.

Count at least unresolved target + alias bytes before retaining the graph.

Also make full `graph --format json` bounded in a meaningful way.

A targeted cap is preferred over a broad storage redesign.

If keeping `serde_json::to_string_pretty`, choose record/text ceilings that make worst-case output realistic.

If streaming is introduced, keep output DTO/schema ownership clear and do not broaden scope unnecessarily.

### Targeted tests

- exact global target/binding byte boundary;
- over by one;
- many per-file-safe files exceeding aggregate graph bytes;
- no partial graph;
- full graph output bound.

---

## CE-P2-003 fix handoff — query root budget

### Desired end state

A query with huge duplicate-name fan-out cannot emit unbounded root IDs/result groups.

### Likely files

```text
src/graph.rs
src/output_graph.rs
tests/graph.rs
README.md
```

### Important invariants

- qualified-first/local-fallback root lookup semantics remain;
- file filter remains a filter;
- returned roots remain separate;
- ordinary zero/multiple root behavior unchanged;
- truncation is explicit.

### Implementation constraints

Introduce a `MAX_QUERY_ROOTS` or equivalent.

Prefer output that distinguishes:

```text
roots_truncated
traversal truncated
```

while retaining overall `truncated`.

Do not allocate one empty `RootResult` for every omitted root.

### Targeted tests

- exactly at root limit;
- one over;
- deterministic retained roots;
- root truncation JSON/human semantics;
- `--file` narrowing;
- zero roots.

---

## CE-P2-005 fix handoff — remove false file-wide local binding

### Desired end state

An unqualified call is not resolved to a method/nested declaration merely because that local name is unique somewhere in the file.

### Likely files

```text
src/graph.rs
tests/graph.rs
tests/graph_fixtures/*
README.md
docs/current-state.md
```

### Preferred implementation

For Phase 2, remove the file-wide `UniqueSameFileLocalName` fallback from call-reference resolution.

Keep exact same-file qualified-name resolution.

Leave uncertain unqualified targets unresolved.

Do not implement a partial compiler/binding engine in this fix pass.

### Targeted tests

Python:

```text
unique() must not resolve to C.unique
C.unique() may resolve to C.unique
top-level helper() still resolves through exact qualified name
```

Rust:

```text
unique() must not resolve to C::unique impl method
```

Ensure callers/deps reflect the corrected resolution.

---

## CE-P2-006 fix handoff — callee evidence span

### Desired end state

Call relation evidence range covers exactly the syntax used to normalize the callee.

### Likely files

```text
src/language/relations.rs
tests/graph.rs
```

### Implementation constraint

Pass the callee node to `emit`, not the enclosing call node.

Do not change accepted callee forms.

### Targeted tests

Exact byte ranges for Rust/Python simple and qualified calls.

---

## CE-P2-007 fix handoff — grouped Rust `self`

### Desired end state

Grouped `self` imports flatten to the group prefix, not `prefix::self`.

### Likely files

```text
src/language/relations.rs
tests/graph.rs
tests/graph_fixtures/relations.rs
```

### Implementation constraints

Special-case grouped-prefix `self` context only.

Do not break:

```rust
use self::x;
use super::x;
use crate::x::Y;
```

Support:

```rust
use foo::{self, bar};
use foo::{self as f, bar};
use foo::nested::{self, Thing};
```

### Targeted tests

Assert exact targets and alias metadata.

---

# 16. Full verification required from the Sol Medium fix session

Run targeted tests while editing, then:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

Run release smoke:

```bash
./target/release/ctx scan .
./target/release/ctx index .
./target/release/ctx symbols . --format json
./target/release/ctx graph .
./target/release/ctx graph . --format json
```

Choose actual symbols from the repository and run:

```bash
./target/release/ctx deps <SYMBOL> .
./target/release/ctx callers <SYMBOL> .
./target/release/ctx related <SYMBOL> .
```

Also run a Python repository/fixture smoke because `CE-P2-005` and `CE-P2-006` affect Python reference evidence.

For Rust, explicitly smoke grouped `self` imports through a committed or temporary fixture.

If the environment permits, add adversarial small-limit tests rather than constructing production-scale repositories.

---

# 17. Required documentation / evidence updates from the fix session

Create:

```text
docs/reports/phase-2-review-fix-report.md
```

The report should:

- identify this independent review;
- list every finding and disposition;
- explain chosen aggregate limits and why they are meaningful;
- explain the reference-resolution change;
- explain the callee-range fix;
- explain grouped `self` import semantics;
- record exact verification actually run;
- distinguish local verification from independent acceptance.

Update:

```text
docs/current-state.md
```

Truthfully state:

```text
Phase 2 review findings fixed and locally verified;
independent final disposition pending
```

or equivalent.

Update README only where public behavior changed, especially:

- repository/global safety-limit semantics;
- root query truncation fields/behavior;
- unique-local resolution behavior;
- graph safety ceiling wording.

Do not rewrite:

```text
docs/reviews/phase-2-independent-review.md
```

to hide findings.

Do not retroactively mark Phase 2 independently accepted merely because the fix session passes tests.

Do not implement Phase 3.

---

# 18. Future considerations — not required in this fix pass

These are not actionable Phase 2 findings unless scope changes.

## A. Git subprocess timeout/output streaming

`git` probing currently has no subprocess timeout and uses `Command::output()`.

This is already documented as a limitation.

A later hardening pass may want:

- timeout/cancellation;
- bounded stderr capture;
- dirty-state checks that do not materialize large porcelain output solely to test non-emptiness.

Do not broaden the Phase 2 review fix unless measurements or requirements justify it.

## B. Stronger filesystem no-follow primitives

Phase 0/1 metadata checks reduce symlink/path races but cannot make a live filesystem an atomic snapshot.

A later Unix-specific `openat`/`O_NOFOLLOW`-style hardening could reduce check/open races.

Do not turn this Phase 2 fix into a portability-heavy filesystem rewrite.

## C. Relation extraction query assets

The original Phase 2 plan suggested separate Tree-sitter relation query files.

The current manual iterative walk is acceptable.

Revisit only if relation extraction complexity grows materially.

## D. Streaming large machine output

Even after realistic safety caps, later persistence/daemon phases may prefer streaming machine output or pagination instead of one giant JSON string.

That is not required to complete Phase 2 if the new caps make worst-case output safe.

---

# 19. Final verdict

The Phase 2 implementation is a good architectural foundation with several unusually careful choices:

- same-tree evidence;
- structural IDs;
- lexical containment;
- explicit unresolved targets;
- derivation vs resolution;
- deterministic final relation IDs;
- cycle-safe traversal;
- fail-closed per-file relation budgets.

The main problem is not that the architecture is misguided.

The main problem is that the resource model stops one aggregation layer too early, while one heuristic resolver and two syntax/evidence details create false or imprecise graph facts.

After `CE-P2-001` through `CE-P2-007` are fixed and the full verifier/smoke suite passes, the repository should be suitable for a short independent final disposition before starting Phase 3.

Until then:

> **Phase 2 is implemented, but the Phase 0–2 evidence pipeline is not yet fully bounded or precise enough to treat the relationship graph as a trusted Phase 3 substrate.**
