# Phase 2 implementation report

**Date:** 2026-10-14<br>
**Status:** implementation evidence; independent Sol High review pending

## 1. Summary

Implemented `graph`, `deps`, `callers` and `related` as fresh synchronous in-memory
commands over the canonical scanner and Rust/Python analyzers. The graph retains
lexical structure, unresolved imports, conservative call-site references, explicit
evidence and limited same-file resolution. No Phase 3 functionality was added.

## 2. Phase 1 acceptance and baseline

Baseline commit: `b132b8e61843918e374598f05f17edef3862e174`.
The user explicitly authorized Phase 2 despite pending final Phase 1 acceptance,
before production edits. The three Phase 1 review fixes remain in place; this
session does not independently close that review.

An initial verifier invocation could not find Cargo on PATH. With the existing
isolated toolchain, the pre-edit `./scripts/verify.sh` passed: 21 unit tests,
16 Phase 0 integration tests and 7 Phase 1 integration tests. No toolchain install
or global configuration change was needed.

## 3. Entity identity

`EntityId(u32)` and `RelationId(u32)` address canonical vectors. Entity order is
repository first, files by relative path, then symbols in `RepositoryIndex` order.
`Entity::Symbol` references the owned canonical symbol record rather than copying
its key; its structural identity is path + kind + whole declaration range.
Qualified/local names are display/query evidence, never graph keys. Duplicate
names retain separate IDs. IDs are deterministic for unchanged input, index-local,
not stable across edits and not lineage identifiers.

## 4. Relations and evidence

Relations are `contains`, `imports` or `references`. Targets are either an entity
ID or an unresolved normalized name. Imports are always unresolved. References
carry `ReferenceContext::Call`; there is no `calls` relation. Repeated occurrences,
self-loops and cycles are preserved. Sorting precedes relation ID assignment;
forward/reverse adjacency lists are consequently ordered by relation ID.
Unresolved targets have forward adjacency only.

Every relation has a path, optional source range/language and project-owned
extraction derivation. `direct_structural`, `exact_same_file_qualified_name`,
`unique_same_file_local_name` and `unresolved` describe resolution separately.
Git metadata retains existing optional semantics and does not identify exact
working-tree source content.

## 5. Containment and source ownership

Repository contains every scanned file. Strict whole-range containment chooses a
symbol's nearest lexical parent, otherwise file. Rust impl methods are not
fake-owned by separate nominal type declarations. Import/reference ownership uses
the innermost declaration containing the entire occurrence range.

Sorted-range stack validation detects crossing/equal ambiguous intervals. A
max-end segment tree over unambiguous ranges serves O(log S) owner lookups after
O(S log S) preparation per file. Ambiguous regions conservatively fall back to
file ownership, even when this sacrifices an otherwise plausible outer parent.
There are no pairwise symbol/relation scans.

## 6. Rust and Python imports

Rust handles simple/scoped `use`, aliases, groups, nested groups, wildcards,
crate/self/super and leading-root paths. Groups flatten to atomic target facts.
Python handles import/dotted/aliased/from/multiple/relative/wildcard forms;
relative leading dots remain visible. Alias and wildcard metadata are retained.
Imports do not resolve repository files, symbols, packages or later aliases.
Unsupported syntax is counted and omitted. Python future-import statements are
currently counted as unsupported.

Normalization is iterative and builds only accepted syntax segments. It never
copies arbitrary expressions, call arguments or source previews into targets.

## 7. References and syntactic callers

Rust accepts identifier/scoped callees; field-expression and dynamic callees are
unsupported. Python accepts identifier-rooted dotted chains. Inner accepted calls
remain facts even if their outer dynamic call is unsupported. `self`/`Self` names
are not rewritten to class/type names. `callers` follows incoming resolved
call-context references and explicitly calls them syntactic/best-effort callers.
This is not a compiler-complete call graph.

## 8. Resolution and query behavior

Per-file maps retain all IDs for each qualified/local name. A unique exact
qualified match wins; otherwise an unqualified target may match a unique local
name in that file. Ambiguous names and cross-file targets remain unresolved.
No import aliases, lexical bindings, types or dispatch are inferred.

Root lookup reuses Phase 1 qualified-first/local-fallback semantics, then applies
`--file`. Zero roots succeed; multiple roots remain grouped. `deps` follows
outgoing references and attaches unresolved imports/references owned by visited
entities. It neither traverses containment nor inherits file imports. `callers`
traverses incoming resolved call references. `related` traverses resolved edges
both ways, including containment, with unresolved attached facts as leaves.
Depth 0 returns roots without relations. Boundary entities are included but their
outgoing facts are not expanded beyond the requested depth.

## 9. Resource limits

| Scope | Ceiling |
|---|---|
| Per-file relation candidates | 16,384 |
| Aggregate normalized target + alias bytes | 4 MiB |
| Normalization node/segment visits | 50,000 |
| Target segments | 64 |
| One target or alias | 4 KiB |
| Graph entities | 1,000,000 |
| Graph relations | 2,000,000 |
| Default / maximum depth | 1 / 8 |
| Visited entity occurrences across all root groups | 10,000 |
| Emitted relation occurrences across all root groups | 25,000 |

Relation exhaustion clears all extracted facts from that file, records one fixed
`analysis_limit_exceeded` status, and preserves Phase 1 symbols/diagnostics and
structural containment. Counts on failed files describe only pre-failure work;
import/reference/unresolved output counts are zero. Graph cap exhaustion fails the
command explicitly. Traversal overflow marks group and aggregate `truncated`.
Exact budgets are allowed; only omitted eligible work causes truncation.

Tree-sitter query-state and Phase 1 symbol/name/qualification limits remain
independent and unchanged. Graph analysis checks cumulative entity/relation
ceilings while accumulating analyzer output, before graph construction. Per-file
maps, range indexes and adjacency avoid global R*S resolution/ownership work.

## 10. Code organization

- `src/relations.rs`: owned parser-independent candidates, accounting and evidence.
- `src/language/relations.rs`: shared budget and language-specific syntax normalization.
- `src/language/mod.rs`, Rust/Python adapters, `src/index.rs`: graph-mode analysis
  from the same loaded text/tree; symbol-only mode skips relation extraction.
- `src/graph.rs`: entity/relation construction, containment, resolution, adjacency
  and iterative BFS queries with shared result budgets.
- `src/cli.rs`, `src/main.rs`: argument validation and command composition.
- `src/output_graph.rs`: explicit schema-v1 DTOs and escaped human output, with
  existing symbol/range DTO reuse. Output performs no source reads/parsing.
- `tests/graph.rs`, graph fixtures and internal tests: semantic/resource/security
  coverage. Existing Phase 0/1 tests were not weakened or rewritten.

## 11. Plan deviations

- Proceeded under explicit maintainer authorization while Phase 1 final independent
  acceptance remains pending, as permitted by the prompt's gate.
- Stored entity keys by reference to canonical file/symbol positions, avoiding
  duplicate path/kind/range storage; IDs remain separate from names.
- Used an interval index in addition to a sorted stack for efficient arbitrary
  occurrence ownership, including shared-boundary and malformed overlap defense.
- Traversal budgets are shared across root groups, preventing many matching roots
  from multiplying the output ceiling. Per-root results still preserve grouping.
- Relation byte budgets also charge alias materialization and bound individual
  aliases. Structural containment survives relation extraction failure; zero-partial
  behavior applies to extracted imports/references, preserving graph structure.
- Retained the supplied previously untracked Phase 2 plan in the single requested
  implementation commit. No architecture/PRD/ADR changes were needed.

## 12. Verification

Using the existing isolated aarch64 macOS Rust 1.99.0 toolchain, cached registry
and offline mode:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
export CARGO_NET_OFFLINE=true
./scripts/verify.sh
cargo check --all-targets
cargo build --release
```

- Canonical verifier passed: context checks, rustfmt, warnings-denied Clippy,
  32 unit tests and 31 integration tests (16 Phase 0, 7 Phase 1, 8 Phase 2).
- All-target check and optimized release build passed.
- All 20 human/JSON release smoke invocations below passed.
- `git diff --check` passed.
- Initial missing-Cargo failure was resolved by selecting the already installed
  isolated toolchain. No remaining blocked/failed checks. No cloud/provider,
  cross-platform CI or independent review was performed.

## 13. Real repository smoke observations

Executed these commands in both human and JSON formats:

```bash
./target/release/ctx graph .
./target/release/ctx index .
./target/release/ctx deps construct .
./target/release/ctx callers construct .
./target/release/ctx related construct .
```

For Python, used `/Users/jasonkli/projects/context-platform/context-integrity`
as PATH and `normalize_path` as NAME. Smoke output and command/time metadata
were captured under `/tmp/context-engine-phase2-*`; these temporary outputs
are not canonical project state.

| Observation | Context Engine | Context Integrity |
|---|---:|---:|
| Scanned files / symbols | 79 / 384 | 65 / 241 |
| Entities | 464 | 307 |
| Relations | 1,378 | 2,085 |
| Contains / imports / references | 463 / 194 / 721 | 306 / 283 / 1,496 |
| Resolved call references | 267 | 282 |
| Unresolved call references | 454 | 1,214 |
| Unresolved imports | 194 | 283 |
| Relation files analyzed | 30 | 23 |
| Relation failures / limit failures | 0 / 0 | 0 / 0 |
| Unsupported candidates | 1,805 | 106 |
| Unsupported-language files | 49 | 42 |
| Query deps / callers / related relation counts | 22 / 1 / 24 | 4 / 5 / 10 |
| Graph human / JSON local wall time | 0.175 / 0.156 s | 0.113 / 0.121 s |

All selected queries matched one root and reported `truncated: false`.
No symbol-analysis failures occurred. Engine syntax diagnostics included two
issues from intentionally malformed fixtures; Integrity had zero. Unsupported
candidates include field/dynamic callees and unsupported import forms, rather
than silently guessed targets. Timings are single local smoke observations,
not benchmarks. Counts include source fixtures/tests and reflect the dirty
working tree at smoke time, not a promised immutable revision snapshot.

## 14. Acceptance audit

| Plan acceptance area | Actual implementation / evidence |
|---|---|
| Gate | Explicit user authorization; passing pre-edit baseline |
| Identity | Newtype IDs; canonical vector order; duplicate/determinism/cap tests |
| Evidence | Required evidence/derivation/resolution DTO fields; unresolved facts |
| Containment | Lexical fixtures, shared endpoints, oracle and synthetic overlap tests |
| Imports | Rust/Python matrix fixtures and metadata/relative/wildcard assertions |
| References | Accepted/unsupported call forms; exact/local/ambiguous/cross-file tests |
| Resource behavior | Independent exact/over budgets; fail-closed CLI; retained symbols; caps |
| Graph | Forward/reverse/unresolved adjacency; repeated occurrences; self/mutual cycles |
| CLI | All commands, defaults, zero/multiple roots, file filters and depth validation |
| JSON | Explicit v1 DTOs, deterministic graph/query fixtures, no source bodies |
| Architecture | One scanner/read/parse; no Tree-sitter outside language; output only adapts |
| Security | No execution, escaped controls, symlink omission; prior scanner regressions |
| Verification | Canonical verifier, all-target check, release build and two-repo smoke |
| Documentation | README, current-state and this report; independent acceptance pending |

The audit checks delivery criteria against code/tests; it does not grant Phase 2
independent acceptance.

## 15. Known limitations

Only UTF-8 Rust/Python are parsed. Names remain syntax-local; malformed syntax
recovery is not compiler validity. Scope shadowing, type/trait/method dispatch,
package/module resolution and cross-file import/reference resolution are absent.
Alias metadata is not a resolver. Python future imports are unsupported. Graphs
are fresh and nonpersistent; IDs can renumber. Some valid large/deep files fail
budgets honestly. Ambiguous ranges choose file ownership. Existing native path
lossiness, Git probing/ignore costs and filesystem TOCTOU limitations remain.
No atomic repository snapshot, cloud verification or independent review is claimed.

## 16. Phase 3 handoff observations

1. `EntityId` is sufficient for this rebuilt index; do not treat it as lineage.
2. A future external `EntityRef` would need file/kind/range plus content/revision
   context before it could safely survive across commands.
3. Lexical containment provides useful neighborhoods, including file context.
4. Cross-file project imports are valuable unresolved targets; external imports
   should remain distinguishable rather than guessed.
5. Keep module/package resolution a separate explicit feature with its own evidence.
6. Same-file resolved-reference observations are recorded in the smoke table below.
7. Dotted receiver calls and Rust method calls remain predominantly unresolved or
   unsupported; no runtime false-positive rate was measured.
8. Expand references beyond calls only with a concrete query need and budget tests.
9. File/entity/path search could make neighborhoods easier to enter.
10. Current commands provide inspectable foundations; usefulness is bounded by
    deliberately limited resolution and no automatic inheritance of file imports.
11. These smoke runs alone do not justify centrality/hotspot work.
12. Custom adjacency was sufficient; no graph-crate comparison benchmark was run.
13. Global caps were validated with injected small ceilings; production-scale
    memory/performance has not been benchmarked.
14. Traversal caps are tested at exact/over injected boundaries and shared across
    roots. User-scale tuning remains future evidence gathering.
15. TypeScript versus richer queries remains a product decision; this implementation
    provides no comparative demand evidence.

## 17. Rust learning notes

Newtype IDs separate addresses from names without introducing UUID dependencies.
Canonical vectors own records; adjacency lists borrow no parser/source lifetime.
Per-file maps borrow owned index names only while constructing relations. Sorted
vectors/BTreeSets restore deterministic output after HashMap lookups. VecDeque BFS
plus per-root visited/emitted sets preserves cycles and repeated edge occurrences
without infinite traversal. Shared counters bound multi-root result amplification.
Tree cursors and bounded normalization worklists avoid recursive parser walks.
