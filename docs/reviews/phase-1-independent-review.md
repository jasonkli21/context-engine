# Phase 1 Independent Review — Syntax and Symbol Index

**Repository:** `jasonkli21/context-engine`<br>
**Reviewed commit:** `f6f39f4e91c94fc480c3a1c69248589bf8bbdbe6` (`implement phase 1`)<br>
**Parent / Phase 0 baseline:** `47bc1bd0ac051f0647986bbd34e2a643821a388c`<br>
**Review date:** 2026-10-14<br>
**Reviewer role:** independent post-implementation review<br>
**Intended next consumer:** fresh Luna XHigh Codex fix session<br>
**Required fix artifact target:** this document should be placed at `docs/reviews/phase-1-independent-review.md`

---

## 1. Executive assessment

### Verdict

**NOT READY FOR PHASE 2**

Phase 1 is substantially well-structured and, on static inspection, implements most of the intended syntax/symbol foundation faithfully. The production path reuses the Phase 0 scanner, keeps Tree-sitter types inside the analysis layer, uses owned language-neutral symbol records, performs bounded UTF-8 source reads, preserves explicit per-file accounting, keeps the index process-local/in-memory, and uses explicit output DTOs. The Rust/Python fixture coverage is materially stronger than a happy-path-only implementation.

However, one **High** severity resource-safety defect should be fixed before Phase 2 builds graph construction on this substrate:

- **CE-P1-001:** the analyzer has no bound on total extracted symbols or aggregate qualified-name work/output. The configured Tree-sitter `QueryCursor::match_limit` is only a limit on **in-progress** query matches, not a cap on total matches. Because qualification walks ancestor chains for every symbol and materializes owned qualified names, a small-but-deep hostile source file can cause superlinear work and output growth despite the parse-size cap.

There is also one **Medium** Rust evidence-correctness defect and one **Low** documentation mismatch:

- **CE-P1-002:** Rust impl-target qualification erases reference/pointer wrappers, so a method syntactically belonging to an impl for `&Foo` or `*const Foo` can be reported as `Foo::method`.
- **CE-P1-003:** README wording says changed files fail recoverably after discovery, but ordinary same-size/in-place concurrent changes are not detected; only unsafe/unavailable/non-regular/oversized states are rejected. The surrounding TOCTOU disclaimer is correct, but the stronger sentence should be narrowed.

### Finding counts

| Severity | Count |
| --- | ---: |
| Critical | 0 |
| High | 1 |
| Medium | 1 |
| Low | 1 |
| Future considerations | 3 |

### Phase 2 readiness

**Blocked by CE-P1-001.**

CE-P1-002 should be fixed in the same review-fix pass before Phase 2 begins depending on or exposing Rust qualified-name evidence more broadly. It is not classified High because Phase 1 explicitly states that `qualified_name` is syntax-local display/query evidence, not globally unique identity.

### Highest-risk themes

1. **Resource amplification from untrusted source:** parse-size bounds do not by themselves bound symbol count, qualification work, or emitted symbol text.
2. **Rust syntax evidence fidelity:** impl target normalization removes meaningful non-generic syntax and can produce misleading qualified names.
3. **Contract precision:** one README sentence overstates what filesystem revalidation proves.

### Independent verification status

**Static review completed; reviewer-side executable verification is blocked by the review environment.**

Reviewer execution blockers:

- `cargo` is not installed in this environment (`cargo: command not found`).
- Direct repository cloning is blocked by DNS/network isolation (`Could not resolve host: github.com`).
- GitHub reports no workflow run or combined status associated with the reviewed commit.

Therefore this reviewer did **not** independently run:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
./target/release/ctx index .
./target/release/ctx symbols .
./target/release/ctx symbol RepositoryScan .
./target/release/ctx index . --format json
./target/release/ctx symbols . --format json
```

The Phase 1 implementation report states that these local checks passed on aarch64 macOS with stable Rust 1.99.0, including 19 unit tests and 22 integration tests. Those claims are delivery evidence, not independently reproduced review evidence.

---

## 2. Review scope and evidence

### Reviewed revision

- Target commit: `f6f39f4e91c94fc480c3a1c69248589bf8bbdbe6`
- Parent: `47bc1bd0ac051f0647986bbd34e2a643821a388c`
- Git comparison: target is exactly one commit ahead of the parent.
- Phase 1 changes are concentrated in parser/index/domain/CLI/output/test/docs additions. The target commit does not modify the Phase 0 scanner, Git adapter, classifier, or original `tests/cli.rs` regression suite.

### Documents consulted

Read as requirements/status/architecture evidence:

- `AGENTS.md`
- `docs/README.md`
- `docs/current-state.md`
- `docs/plans/phase-1-implementation-plan.md`
- `docs/architecture.md`
- `docs/reports/phase-1-implementation-report.md`
- `docs/reports/phase-0-implementation-report.md` at the parent baseline
- `docs/development/llm-workflow.md`
- `docs/prd.md` for long-term evidence/Phase 2 intent
- `README.md`
- `scripts/verify.sh`
- `.github/workflows/ci.yml`

There are no accepted ADR files under `docs/decisions/` beyond its routing README at the reviewed revision.

### Source/test areas inspected

- `src/model.rs`
- `src/source.rs`
- `src/index.rs`
- `src/language/mod.rs`
- `src/language/rust.rs`
- `src/language/python.rs`
- `src/language/queries/rust-symbols.scm`
- `src/language/queries/python-symbols.scm`
- `src/language/tests.rs`
- `src/cli.rs`
- `src/main.rs`
- `src/output.rs`
- `src/output_index.rs`
- relevant Phase 0 scanner path in `src/scan.rs`
- `tests/symbols.rs`
- Rust/Python symbol fixtures, including malformed fixtures
- dependency versions in the commit/lockfile

### External API semantics checked

Tree-sitter 0.27.1 documentation for `QueryCursor` was checked independently. It defines `match_limit` / `set_match_limit` as the maximum number of **in-progress matches**, and `did_exceed_match_limit` reports whether that in-progress limit was exceeded. It is not a total-result cap.

Reference:

- <https://docs.rs/tree-sitter/0.27.1/tree_sitter/struct.QueryCursor.html>

This distinction is material to CE-P1-001.

### Reviewer commands / environment checks

Attempted:

```text
cargo --version
```

Result: blocked — `cargo: command not found`.

Attempted:

```text
git clone --no-checkout https://github.com/jasonkli21/context-engine.git ...
```

Result: blocked — `Could not resolve host: github.com`.

GitHub commit metadata/status was inspected through the connected repository interface. No workflow runs or combined statuses were attached to the target commit at review time.

### Phase 0 baseline/gate assessment

The Phase 1 plan originally required independent Phase 0 re-review before Phase 1 implementation. The parent Phase 0 implementation report still described independent re-review as pending. The Phase 1 implementation report and current state record that the user explicitly accepted Phase 0 and waived that re-review on 2026-10-14, superseding the original gate.

Repository history cannot independently prove the claimed local ordering “status was updated before code edits” because the Phase 1 code and canonical status update landed in the same final commit. That is an evidence limitation, not by itself a code defect.

More importantly for correctness, the target commit does not alter the Phase 0 scanner/Git/classifier implementation where the five prior Phase 0 review remediations live. Static composition also preserves:

- shared `scan_repository(...)` discovery;
- no-follow scanner symlink behavior;
- fail-closed ignore-control checks;
- Phase 0 Git filter hardening;
- warning-path conventions;
- human control-character escaping;
- `ctx tree` with line counting and Git probing disabled;
- separate explicit schema-v1 DTOs.

Given the explicit user waiver plus lack of a concrete Phase 0 regression in this commit, this review does **not** promote the formal gate deviation into a Phase 1 blocking finding. It remains a documented review-evidence limitation.

---

## 3. Acceptance-criteria coverage

| Area | Status | Notes |
| --- | --- | --- |
| Phase 0 gate / regression | **Partial** | Original independent-re-review gate was explicitly waived by the user and documented. Static diff shows Phase 1 does not modify the scanner/Git/classifier core; no regression found. Reviewer could not rerun the Phase 0 suite. |
| Architecture | **Pass** | Canonical scanner reused; no second walker; analyzers do not traverse/render; output does not read/parse source; domain is Clap/Tree-sitter independent; one package remains. |
| Source loading | **Pass (static)** | Relative-component validation, symlink/regular-file rechecks, pre-open and bounded-read size checks, strict UTF-8, recoverable states. Residual check/open TOCTOU is honestly documented and within plan scope. Runtime tests not independently rerun. |
| Rust analysis | **Partial** | Required kinds and major nesting cases are present, but CE-P1-002 produces misleading qualification for reference/pointer impl targets. |
| Python analysis | **Pass (static)** | Class/function/method/nested/decorated/async logic is coherent; nearest-definition logic correctly avoids treating a nested function inside a method as another method. |
| Source ranges | **Pass (static)** | Domain uses UTF-8 byte offsets, zero-based rows/byte columns, half-open ranges; fixture tests independently recompute columns and slice name ranges. |
| Malformed-source diagnostics | **Pass (static)** | Error/missing nodes are iteratively counted; details capped at 20/file; recovered symbols retained; messages are fixed/bounded. |
| Analysis accounting | **Pass (static)** | Every scanned file gets a status; supported/generated/vendor/too-large/UTF-8/read/parse states are separately represented; summary arithmetic is explicit. |
| Repository orchestration | **Pass (static)** | Line counting disabled; one file loaded/analyzed at a time; source/tree not retained in `RepositoryIndex`; analyzer registry reused for invocation. |
| CLI | **Pass (static)** | Three commands, path defaults, format/max-size args, exact qualified lookup then exact local fallback, zero/multiple-match behavior, fresh/in-memory help for `index`. |
| JSON machine contract | **Pass (static)** | Explicit DTOs/envelope; no domain auto-serialization; source ranges documented; ordering explicit; no raw source bodies. Runtime byte determinism not independently rerun. |
| Security / hostile input | **Fail** | Filesystem/source execution boundaries are good, but CE-P1-001 leaves unbounded per-file symbol/qualification amplification under hostile input. |
| Tests | **Partial** | Existing tests are strong but omit the adversarial resource-amplification and non-nominal impl-target cases behind CE-P1-001/002. Reviewer execution blocked. |
| Documentation | **Partial** | Generally precise and appropriately scoped, with one overbroad mutation claim in README (CE-P1-003). |
| Scope discipline | **Pass** | No graph/import/reference/call/persistence/cache/watcher/async/concurrency/MCP/LSP/SCIP/TS/embeddings/model/policy work introduced. |
| Phase 2 readiness | **Fail** | CE-P1-001 must be fixed before using Phase 1 as the graph/evidence foundation. |

---

## 4. Findings

### CE-P1-001 — Per-file symbol and qualified-name work is not resource-bounded

**Severity:** High<br>
**Category:** security / resource behavior / analyzer correctness<br>
**Blocks Phase 2:** yes

#### Evidence

- **Files/modules/symbols:**
  - `src/language/mod.rs` — `SyntaxAnalyzer::analyze`
  - `src/language/rust.rs` — `qualify`
  - `src/language/python.rs` — `qualify`
  - `src/model.rs` — `Symbol`
  - `src/source.rs` — parse-size bounding
  - `src/language/tests.rs`, `tests/symbols.rs` — current coverage
- **Observed behavior from code:**
  - source reads are bounded by `max_parse_file_bytes`;
  - the query cursor is configured with `cursor.set_match_limit(65_536)`;
  - every query match is accepted and materialized into an owned `Symbol` until iteration ends;
  - Rust and Python qualification walk ancestors for each symbol and allocate/join owned qualified names;
  - there is no explicit maximum symbol count, maximum aggregate qualified-name bytes, maximum qualification ancestry work, or per-file output budget;
  - `RepositoryIndex` retains every extracted symbol.
- **Relevant requirement/invariant:**
  - arbitrary repository source is hostile input;
  - max parse size is intended to bound resource behavior per source file;
  - query result explosion must not trivially exhaust memory/output;
  - review guidance explicitly asks for obvious quadratic behavior in qualified-name ancestry to be identified;
  - partial/truncated analyzer results must not be silently accepted.
- **External API evidence:**
  - Tree-sitter 0.27.1 documents `QueryCursor::match_limit` as the maximum number of **in-progress matches**, not a maximum total result count. `did_exceed_match_limit()` only reports that in-progress limit.
- **Static adversarial construction:**
  - a Rust file can contain many nested inline modules/declarations while remaining below the default 1 MiB parse-size limit;
  - for each declaration, `qualify` walks back through its ancestor chain and constructs a qualified-name string containing that ancestry;
  - with declarations at increasing nesting depth, total ancestry traversal and materialized qualified-name bytes grow superlinearly (quadratically in the straightforward nested construction) relative to declaration count;
  - the query cursor may never exceed its *in-progress* match limit in this pattern, so the current `did_exceed_match_limit()` guard does not provide the intended resource ceiling.
- **Command/test/scratch evidence:**
  - runtime scratch execution was not possible in this reviewer environment because Cargo is absent;
  - no existing test constructs deep nested declarations or enough flat declarations to exercise a total analysis/output budget.

#### Expected behavior / invariant

For a source file that is already bounded by `max_parse_file_bytes`, **Phase 1 analysis itself must also have deterministic input-triggered resource ceilings**. A repository-controlled file must not be able to make symbol extraction or qualified-name materialization grow without an explicit bound.

The implementation must distinguish:

1. Tree-sitter's in-progress query-state limit, which remains a correctness/truncation signal; and
2. project-owned limits on total symbols / aggregate qualification output or equivalent analyzer work.

If a project-owned analysis budget is exceeded, the file must fail recoverably and deterministically. It must **not** emit a silently partial symbol set.

#### Why it matters

Phase 1 establishes the evidence layer that Phase 2 will graph. Without a real per-file extraction/output budget, an untrusted repository can force large CPU/memory allocations despite the apparently safe source-size limit. Phase 2 would increase downstream amplification because every accepted symbol can become graph entities/relations/evidence.

This is more than a performance optimization: it is a hostile-input trust-boundary issue and makes the current resource-safety story incomplete.

#### Plan weakness

The Phase 1 plan correctly required:

- bounded source reads;
- a Tree-sitter query match limit/truncation check;
- review of query-result explosion;
- sequential per-file processing.

But it did not clearly distinguish Tree-sitter's **in-progress match-state limit** from a total-result/output budget, and it did not specify an explicit project-owned bound on total symbol/qualification work.

**Corrected requirement:** parse-size limiting and query-state limiting are necessary but insufficient. Phase 1 must additionally bound the total per-file symbol/evidence materialization work it owns.

#### Required fix

- **Intended outcome:**
  - add a deterministic project-owned per-file analysis budget that bounds at least:
    1. total extracted symbol count; and
    2. aggregate qualified-name materialization / equivalent ancestry work;
  - preserve Tree-sitter `did_exceed_match_limit()` checking as a separate truncation/correctness guard;
  - if a project-owned budget is exceeded, discard that file's symbol results and expose one bounded recoverable failure/status/diagnostic;
  - do not silently truncate and report the file as `analyzed`.
- **Likely code/doc locations:**
  - `src/language/mod.rs`
  - `src/language/rust.rs`
  - `src/language/python.rs`
  - `src/model.rs` if a dedicated status/diagnostic kind is added
  - `src/index.rs` for recoverable mapping
  - `src/output_index.rs` if a new stable status/diagnostic value is exposed
  - `src/language/tests.rs`
  - `tests/symbols.rs`
  - `README.md`
  - `docs/current-state.md`
  - `docs/reports/phase-1-implementation-report.md` or the repository's review-fix evidence according to workflow
- **Constraints to preserve:**
  - keep Phase 1 sequential;
  - do not add Rayon/Tokio/threads/timeouts/daemon infrastructure;
  - do not add persistence or Phase 2 graph types;
  - do not retain source or syntax trees beyond one file;
  - ordinary repositories below limits must retain identical symbol semantics and ordering;
  - no partial result should be mislabeled as successful analysis;
  - diagnostics/messages remain bounded and contain no source body.
- **Approach guidance:**
  - prefer a small explicit `AnalysisLimits`/constants mechanism over framework-level complexity;
  - a practical minimal design is a maximum symbols-per-file plus a maximum cumulative qualified-name/evidence byte budget (or an equivalent work counter that provably bounds ancestor traversal/materialization);
  - check limits during extraction, before unbounded accumulation;
  - because each ancestry component contributes at least a name/separator to the materialized qualified name, an aggregate qualified-name byte/work budget can also cap the repeated deep-ancestry pattern;
  - keep the existing Tree-sitter match-limit check, but update comments/docs/tests so nobody treats it as the total-result budget;
  - for public accounting, prefer a truthful dedicated status such as `analysis_limit_exceeded` / bounded diagnostic kind. If adding a new schema-v1 status value is judged incompatible with the repository's current contract policy, use the existing recoverable failure channel only if the emitted diagnostic is stable and documentation makes the resource-limit cause explicit. Do not call successful-but-truncated output `analyzed`.

#### Validation

- **Tests to add/update:**
  1. deep nested Rust modules/functions that remain under parse-size limit but exceed the qualification budget;
  2. many flat declarations that exceed the symbol-count budget;
  3. just-below-limit cases that still analyze successfully and deterministically;
  4. assert over-budget files contribute **zero partial symbols** and one bounded recoverable failure/status;
  5. repeated JSON for the over-budget fixture is byte-identical on stable non-Git input;
  6. existing Tree-sitter query-limit test remains and is clearly testing query-state truncation, not total-result count;
  7. ordinary Rust/Python fixture outputs remain unchanged.
- **Commands to run:**

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
./target/release/ctx index .
./target/release/ctx symbols . --format json
```

- **Expected observable result:**
  - adversarial deep/flat fixtures terminate under the explicit budget;
  - no partial symbols from the budget-exceeded file enter `RepositoryIndex`;
  - file accounting truthfully records the recoverable limit failure;
  - ordinary repositories retain expected symbol counts/order.

#### Scope guard

Do **not** implement while fixing:

- global graph budgets;
- Phase 2 entity IDs/edges;
- concurrency;
- parser subprocess isolation;
- OS sandboxing;
- persistence/caching;
- user-configurable general resource policy framework;
- model/LLM behavior.

A small deterministic Phase 1 analyzer budget is sufficient.

#### Dependencies / interactions

- **Related finding IDs:** none required.
- **Required ordering:** fix CE-P1-001 first because it changes analyzer failure/accounting semantics and may affect tests/docs touched by later findings.

---

### CE-P1-002 — Rust impl-target qualification erases reference/pointer wrappers

**Severity:** Medium<br>
**Category:** implementation defect / symbol evidence correctness<br>
**Blocks Phase 2:** no (but should be fixed before Phase 2 relies on/exposes this evidence)

#### Evidence

- **Files/modules/symbols:**
  - `src/language/rust.rs` — `impl_target`, `qualify`
  - `src/language/tests.rs` — `impl_targets_omit_generic_expressions_and_decorators_are_in_whole_range`
  - `README.md` — documented Rust qualified-name limitations
- **Observed behavior from code:**
  - `impl_target` treats `generic_type`, `reference_type`, and `pointer_type` identically by replacing the target node with `child_by_field_name("type")`;
  - generic arguments are intentionally omitted, which is documented;
  - reference/pointer wrappers are also discarded, which is not the same operation as omitting generic arguments;
  - therefore a trait impl such as `impl LocalTrait for &Foo { fn run(...) {} }` can contribute the container `Foo`, yielding `Foo::run` rather than evidence that the syntactic target was a reference type;
  - a direct `impl LocalTrait for Foo` method can therefore receive the same qualified display/query name even though the syntax-local impl targets differ.
- **Relevant requirement/invariant:**
  - qualified names are syntax-local evidence and must not falsely imply compiler resolution;
  - unsupported impl targets should contribute no guessed container;
  - duplicate names are allowed, but the analyzer should not manufacture equivalence by deleting semantically relevant non-generic target syntax;
  - public docs specifically say generic arguments are omitted; they do not say reference/pointer wrappers are erased.
- **Command/test/scratch evidence:**
  - current fixtures cover `impl<T> Engine<T>` and trait impls for `Engine<T>`, but do not cover reference/pointer target forms;
  - reviewer runtime execution was blocked by missing Cargo.

#### Expected behavior / invariant

Phase 1 may intentionally normalize **generic arguments** for nominal impl targets, but it must not turn a syntactically non-nominal target into a nominal container name that claims more precision than the parser evidence supports.

For unsupported non-nominal targets, the safest Phase 1 behavior is the already documented one: **do not guess a container**.

#### Why it matters

The defect affects exact symbol display/query evidence and can collapse distinct impl contexts into the same apparently nominal target. Phase 2 must already avoid treating `qualified_name` as identity, so this is not independently a High-severity identity failure, but carrying misleading qualification into graph evidence would make later debugging and relation derivation less trustworthy.

#### Required fix

- **Intended outcome:**
  - keep current generic nominal behavior such as `Engine<T>` → `Engine` if that is the documented Phase 1 contract;
  - stop stripping `reference_type` and `pointer_type` wrappers into a guessed nominal container;
  - for these non-nominal target forms, return `None` from `impl_target` unless a separately documented syntax-preserving representation is deliberately added;
  - minimal preferred Phase 1 fix: treat reference/pointer impl targets as unsupported and leave the method without a guessed impl container.
- **Likely code/doc locations:**
  - `src/language/rust.rs`
  - `src/language/tests.rs`
  - optionally Rust fixture file(s)
  - README only if behavior wording needs clarification
- **Constraints to preserve:**
  - do not add compiler/type resolution;
  - do not invent stable IDs;
  - do not encode trait/reference semantics into graph types;
  - preserve generic nominal target behavior already covered by fixtures;
  - preserve duplicate declarations rather than deduplicating by name.
- **Approach guidance:**
  - remove `reference_type` / `pointer_type` from the generic-unwrapping arm;
  - let unsupported target kinds return `None`;
  - if Tree-sitter wraps `&mut Foo` or raw pointers in nested nodes, tests should prove no `Foo::...` container is guessed.

#### Validation

- **Tests to add/update:**
  - direct nominal target: `impl LocalTrait for Foo` still yields `Foo::run` (or current documented nominal form);
  - generic nominal target: `impl<T> LocalTrait for Wrapper<T>` retains current normalized container behavior;
  - reference target: `impl LocalTrait for &Foo` does **not** yield `Foo::run`;
  - mutable-reference target: `impl LocalTrait for &mut Foo` does **not** yield `Foo::run`;
  - raw pointer target(s), if accepted by the grammar, do **not** yield a guessed `Foo::run`;
  - when direct and reference-target impls coexist, their output does not falsely imply that both were nominal `Foo` impls.
- **Commands to run:**

```bash
cargo test --all-targets --all-features
./scripts/verify.sh
```

- **Expected observable result:**
  - generic argument omission remains as documented;
  - non-nominal wrappers are not erased into misleading nominal qualification.

#### Scope guard

Do **not** implement:

- trait-resolution identity;
- fully canonical Rust type printing;
- compiler/HIR integration;
- graph IDs;
- cross-file package/type resolution.

#### Dependencies / interactions

- **Related finding IDs:** none.
- **Required ordering:** can follow CE-P1-001.

---

### CE-P1-003 — README overstates detection of files changed after discovery

**Severity:** Low<br>
**Category:** documentation mismatch / trust-boundary wording<br>
**Blocks Phase 2:** no

#### Evidence

- **Files/modules/symbols:**
  - `README.md` — Syntax and symbol indexing / source-loading bullet
  - `src/source.rs` — `load`
  - `src/index.rs` — scan-to-read flow
  - `docs/reports/phase-1-implementation-report.md` — known TOCTOU limitation
- **Observed behavior:**
  - README says: `Files changed/unavailable since discovery fail recoverably.`
  - production revalidation rejects observed symlinks/special files, missing files, and files that exceed the bound;
  - it does **not** and cannot detect every ordinary content mutation between scan and read. A regular file that is modified in place or replaced with another same-size regular file may be parsed successfully;
  - the very next README sentence correctly acknowledges separate metadata/open operations and residual live-filesystem races;
  - the implementation report is more precise and explicitly says concurrent writers can race the checks.
- **Relevant requirement/invariant:**
  - docs must not claim atomic snapshot/change detection that Phase 1 does not implement;
  - residual TOCTOU is allowed by the plan if documented honestly.

#### Expected behavior / invariant

Public documentation should say that **unsafe/unavailable/oversized states observed at load time fail recoverably**, while ordinary concurrent content changes may still be analyzed because Phase 1 does not provide an atomic snapshot.

#### Why it matters

This is a small wording issue, but the source-loader trust boundary is security-sensitive. Precise wording prevents users and later implementation agents from inferring snapshot/change-detection guarantees that do not exist.

#### Required fix

- **Intended outcome:** narrow the README sentence; preserve the correct residual-TOCTOU disclaimer.
- **Likely code/doc locations:**
  - `README.md`
  - no production-code change required for this finding.
- **Suggested wording:**

> Files that become unavailable, unsafe/non-regular, or oversized before loading fail recoverably. Ordinary concurrent content changes may still be analyzed because scan/metadata/open are not an atomic snapshot.

- **Constraints to preserve:**
  - do not claim no-follow atomic open semantics;
  - do not add OS-specific sandboxing solely for this wording fix.

#### Validation

- README source-loading section no longer implies generic post-discovery mutation detection.
- `docs/current-state.md` and implementation/review-fix report use compatible wording.
- `./scripts/check-context.sh` and `./scripts/verify.sh` pass.

#### Scope guard

Documentation only. Do not turn CE-P1-003 into an `openat2`/sandbox/platform-specific redesign.

#### Dependencies / interactions

- **Related finding IDs:** none.
- **Required ordering:** after functional findings.

---

## 5. Architecture assessment

### Domain boundaries

The main Phase 1 architecture is sound.

Observed production flow matches the intended shape:

```text
CLI
  ↓
index_repository(...)
  ↓
scan_repository(...)
  ↓
RepositoryScan.files
  ↓
source::load(...)
  ↓
AnalyzerRegistry
  ├── RustAnalyzer
  └── PythonAnalyzer
  ↓
owned language-neutral symbols + diagnostics
  ↓
RepositoryIndex
  ↓
human / explicit JSON DTO renderers
```

Strengths:

- no second repository walker was introduced;
- scanner-produced `FileRecord` paths are the production source of analysis inputs;
- line counting is disabled for index commands;
- analyzers neither traverse the filesystem nor render output;
- output modules do not parse/read source;
- Clap structs do not become domain types;
- Tree-sitter `Tree`, `Node`, `Point`, queries/captures and parser lifetimes do not escape `src/language/`;
- `RepositoryIndex` owns only paths, ranges, strings, statuses, diagnostics and optional Git metadata — not source bodies/trees;
- grammar-specific node-kind handling stays inside Rust/Python analysis modules;
- no unnecessary multi-crate/plugin architecture was added.

### Analyzer abstraction

The single `LanguageAnalyzer` trait is justified by two concrete analyzers and remains small. The registry owns one mutable analyzer per language, allowing parser/query reuse within a command invocation without introducing shared ownership, threads, async, or dynamic plugin discovery.

### Source ownership

Source strings live only through one file's analysis call. Tree-sitter trees are local to `SyntaxAnalyzer::analyze` and are dropped when the call returns. Durable domain results own their names/ranges.

### Phase 0 reuse

The target commit does not modify `src/scan.rs`, `src/git.rs`, or `src/classify.rs`. `index_repository` constructs `ScanOptions` with `count_lines = false` and delegates discovery to `scan_repository`, preserving the reviewed Phase 0 traversal/ignore/symlink/Git behavior.

### Phase 2 extension seam

After CE-P1-001 is fixed, the Phase 1 model is a reasonable substrate for Phase 2:

- path/language/kind/range/name facts are parser-independent;
- duplicate symbols are preserved;
- exact display/query qualified names are separate from future identity;
- a future `EntityId` and `EvidenceRef` can be layered on without storing Tree-sitter objects;
- containment/relationship types can remain separate from extraction.

Phase 2 should not require rewriting the indexing core merely to introduce graph IDs/evidence, assuming the resource/fidelity issues above are addressed.

---

## 6. Rust / maintainability assessment

### Ownership and lifetimes

Good:

- no self-referential source/tree containers;
- no `Node<'tree>` / `Tree` / borrowed source slices in durable domain records;
- parser/query objects are reused per analyzer invocation, while trees remain per file;
- strings/ranges in `Symbol` are owned and simple;
- source is read sequentially and not retained repository-wide.

### Error design

The high-level split is appropriate:

- scan/analyzer initialization failure can fail the command;
- source/read/UTF-8/analyze failures can be represented per file;
- malformed syntax is recovery evidence rather than command failure.

CE-P1-001 may justify refining the recoverable analyzer failure/status path so an input resource-budget breach is not conflated with a generic parser failure.

### Trait use

The analyzer trait is proportionate. There is no speculative backend hierarchy or runtime plugin system.

### Duplication / language divergence

Rust and Python share:

- parser/query machinery;
- capture cardinality checks;
- Tree-sitter-to-domain range conversion;
- syntax diagnostic traversal;
- `AnalyzedFile` result semantics.

Language-specific code is concentrated in qualification/target semantics where divergence is real.

### Parser/query reuse

One parser/query per Rust/Python analyzer is constructed through `AnalyzerRegistry::new()` and reused over files in the command. Trees are not retained.

### Unsafe / panics

No project `unsafe` was found in the reviewed Phase 1 production source. Tests use `unwrap` for assertions/fixture setup; production repository input flows through explicit errors/statuses. Tree-sitter dependencies internally use FFI/unsafe as expected, but those internals do not leak into the project domain.

### Maintainability concern

The main maintainability risk is not abstraction count; it is semantic comments/contracts around resource bounds and Rust target normalization. Fixing CE-P1-001/002 should include comments that state what is and is not normalized/bounded so future language work does not inherit false assumptions.

---

## 7. Parser / symbol correctness assessment

### Tree-sitter integration

Dependency set at the reviewed commit:

- `tree-sitter = 0.27.1`
- `tree-sitter-rust = 0.24.2`
- `tree-sitter-python = 0.25.0`

Initialization uses the grammar `LANGUAGE` constants and current parser/query APIs. Queries are embedded with `include_str!`, so target repositories cannot supply or execute parser queries.

The query capture contract validates labels/cardinality rather than depending on capture order.

### Rust

Static confidence is generally good for promised kinds:

- module
- struct
- enum
- union
- trait
- free function
- impl/trait methods
- type aliases / associated types per the chosen Phase 1 semantics
- const
- static

Existing fixture coverage includes:

- generic structs/impls;
- trait declaration/default methods;
- trait implementation;
- inline/nested modules;
- nested functions;
- attributes;
- raw identifiers;
- Unicode names/byte columns;
- malformed recovery.

`name_range` checks slice exact source bytes. Method classification only promotes functions that are direct members of an impl/trait declaration list, avoiding the obvious nested-function-as-method error.

**Limitation/finding:** CE-P1-002 shows that impl-target normalization is too lossy for reference/pointer target forms.

### Python

Static confidence is high for the Phase 1 syntactic contract:

- class/function query patterns are intentionally small;
- async definitions are represented through the same function definition node path;
- decorated definition range expands to the immediate `decorated_definition` wrapper;
- nearest-definition ancestry means a nested function inside a method remains `Function` rather than becoming another `Method`;
- nested classes/functions construct deterministic lexical qualified names;
- Unicode range semantics are tested in bytes.

### Qualified names

Correctly documented safe uses:

- display;
- exact lookup;
- debugging/navigation within current syntax evidence.

Unsafe uses that must remain prohibited:

- global uniqueness;
- cross-file identity;
- compiler/type identity;
- refactor-stable lineage;
- graph entity identity by itself.

### Ranges

The durable range contract is coherent:

- UTF-8 byte offsets;
- zero-based rows;
- zero-based byte columns;
- half-open `[start, end)`;
- one-based human display based on `name_range.start`.

No Tree-sitter range type leaks into JSON/domain.

### Malformed source

The iterative tree cursor avoids recursive Rust call-stack traversal when counting syntax nodes. Error/missing nodes are counted fully while detailed diagnostics are capped at 20. This is consistent with the documented definition of syntax issues.

---

## 8. Security / trust-boundary assessment

### Source path and filesystem boundary

Good static properties:

- production indexing starts from canonical Phase 0 scan records;
- `source::load` rejects empty, absolute/root/prefix/current/parent components by requiring every relative component to be `Component::Normal`;
- each joined path component is checked with `symlink_metadata` and observed symlinks are rejected;
- final target must be a regular file;
- size is checked before open and against opened metadata;
- bounded reading uses `take(max + 1)` semantics (with saturating max handling) so growth cannot trivially bypass the read cap;
- invalid UTF-8 is rejected, not lossy-decoded into the parser.

### Residual TOCTOU

There is still a check/open race: a path can change after metadata checks and before/during `File::open`. The opened file is rechecked as a regular file and read is bounded, but a concurrent adversary can potentially redirect which regular file is read during the race.

This is **not a required Phase 1 finding** because:

- the plan explicitly permits residual platform-level TOCTOU if honestly documented;
- README/report/current-state state that this is not an atomic snapshot/platform sandbox;
- the review prompt explicitly says not to demand OS-specific sandboxing merely for Phase 1.

Keep this as a future hardening consideration if the threat model later requires atomic no-follow open primitives.

### No execution / network

No Phase 1 code path imports Python, compiles target Rust, runs target Cargo/build scripts/package managers/LSPs, evaluates repository config, loads repository-supplied queries/plugins, or makes analysis network calls.

Git probing is inherited Phase 0 behavior and its filter/process hardening remains in the unchanged Git adapter.

### Output leakage

Stable symbol outputs contain symbol names/ranges/paths/diagnostics but no source bodies/signatures/previews. Human repository-controlled strings pass through control-character escaping. JSON relies on standard string escaping and preserves machine strings rather than terminal rendering.

### Resource safety

**Not sufficient until CE-P1-001 is fixed.** Source-byte bounding is good, but project-owned symbol/qualification materialization remains unbounded relative to that source cap.

### Query origin

Queries are project-owned compile-time assets under `src/language/queries/` and not loaded from analyzed repositories.

---

## 9. Test / verification assessment

### Existing test quality

The Phase 1 suite is stronger than average for an initial parser phase. It tests real production CLI composition in addition to analyzer helpers.

Notable coverage:

- parser/query initialization and unsupported registry lookup;
- exact source-size boundaries including zero;
- invalid UTF-8;
- absolute/parent/empty/missing paths;
- bounded growth and synthetic read error;
- file/parent symlink rejection on Unix;
- scan-to-read disappearance, growth and symlink substitution;
- all required Rust symbol kinds;
- Rust generic impls, nested modules/functions, raw and Unicode identifiers;
- Python decorated/async/nested definitions and method-vs-nested-function distinction;
- name-range source slicing and independent byte-column recomputation;
- malformed recovery, missing/error nodes and 20-detail diagnostic cap;
- mixed supported/unsupported/generated/vendor/too-large/invalid files;
- supported non-source roles;
- exact lookup precedence/local fallback/case sensitivity/no fuzzy matching;
- zero/multiple/duplicate results;
- deterministic repeated JSON on non-Git fixtures;
- no target source execution and no arbitrary source-body output;
- human control escaping.

### Missing high-value cases

Required by findings:

- adversarial deep-nesting / aggregate qualification budget;
- adversarial many-flat-symbols budget;
- Rust `&Foo`, `&mut Foo`, pointer/non-nominal impl target qualification.

Useful but non-blocking future tests:

- cross-platform no-follow/path mutation behavior under Linux/Windows when CI is available;
- very large-but-below-budget Unicode identifier/name cases;
- property/fuzz testing for parser input once the parser surface stabilizes.

### Reviewer execution status

**Blocked, not failed.**

- Cargo is absent in this execution environment.
- Direct clone/network is DNS-blocked.
- No GitHub Actions run/status is attached to the target commit.

### Implementation-report discrepancy assessment

No direct contradiction was found between the implementation report and inspected code for the claimed implemented features. The important caveat is that the report's “query match-limit” protection should not be read as a total symbol/result resource budget; Tree-sitter's API defines it only for in-progress matches. That gap is CE-P1-001.

---

## 10. Documentation assessment

### README

Strong points:

- clearly says the index is fresh/in-memory and not persistent;
- clearly limits parsing to Rust/Python;
- clearly distinguishes syntax-aware evidence from future graphs/relationships;
- explicitly says qualified names are syntax-local and not stable identities;
- documents UTF-8 and source-range byte semantics;
- documents malformed partial recovery;
- documents output status categories and no source bodies;
- documents residual filesystem race/non-sandbox behavior.

Required correction:

- CE-P1-003 narrows one overbroad “changed files fail” sentence.

### `docs/current-state.md`

Appropriately says Phase 1 is implemented/locally verified while independent review is pending. It records the Phase 0 user waiver explicitly and does not claim independent Phase 1 acceptance.

After the fix pass, it should not mark Phase 1 closed until CE-P1-001 and the chosen fix set are implemented and independently/finally verified per repository workflow.

### Implementation report

Generally candid:

- calls itself delivery evidence rather than independent acceptance;
- records the Phase 0 gate waiver as a deviation;
- records toolchain/platform conditions;
- states no cross-platform/cloud/independent verification;
- acknowledges residual TOCTOU and no global repository symbol/output budget.

It should be amended or accompanied by review-fix evidence after CE-P1-001 so the query match-limit is not mistaken for a total resource bound.

### Machine-contract documentation

Explicit DTOs and README field/range semantics are good. If the CE-P1-001 fix introduces a new analysis status/diagnostic value, update the schema-v1 contract documentation intentionally rather than relying on derived serialization.

---

## 11. Phase 2 readiness

### Safe to carry forward

After required fixes, Phase 2 may rely on these Phase 1 properties:

- canonical Phase 0 repository discovery remains the only walker;
- file paths are scan-root-relative domain evidence;
- Rust/Python source loading is strict UTF-8 and bounded;
- parser trees/source are ephemeral per file;
- `RepositoryIndex` contains owned, parser-independent file/symbol/diagnostic facts;
- `Symbol` has explicit local name, qualified display/query name, language, kind, path, whole range and name range;
- duplicate legitimate symbols are preserved;
- malformed files can produce partial **syntax** evidence while syntax issues remain explicit;
- unsupported/skipped/failed files are explicitly accounted for;
- external machine output is defined by DTOs rather than domain serialization.

### Must fix before Phase 2

- **CE-P1-001** — add a real project-owned per-file extraction/qualification resource budget and fail closed on budget exhaustion without partial symbols.

### Should fix

- **CE-P1-002** — stop collapsing reference/pointer impl targets into nominal Rust qualified names.
- **CE-P1-003** — narrow README mutation wording.

### Do not generalize from Phase 1

Phase 2 must not assume:

- `qualified_name` is a unique or stable entity ID;
- two symbols with the same qualified name are the same entity;
- Tree-sitter syntax is compiler/type/import resolution;
- optional Git HEAD identifies the exact dirty working-tree source snapshot;
- malformed-source recovered symbols are compiler-valid;
- the in-memory index is persistence or incremental state;
- Rust/Python extraction patterns prove every future language should share identical containment rules;
- Phase 1's source-loader metadata checks are an atomic filesystem sandbox.

### Phase 2 design implications

Supported by this review only:

1. Add an explicit Phase 2 entity identity separate from `qualified_name`.
2. Build `EvidenceRef` from existing repository path/range/language facts; do not leak Tree-sitter nodes into the graph.
3. Preserve derivation quality so syntax-based relations are not presented as compiler-resolved facts.
4. Carry Phase 1 analysis status/resource failures into graph completeness/accounting rather than silently omitting files.
5. Keep relationships separate from symbol extraction; do not retrofit graph semantics into `Symbol` merely for convenience.

This review does **not** define the Phase 2 implementation plan.

---

## 12. Luna XHigh Fix Handoff

> Implement the findings below in the listed order unless repository evidence requires a safer dependency order. Do not re-review the entire phase unless needed to understand a finding. Preserve Phase 1 scope and all reviewed Phase 0 behavior. Run each finding's targeted validation plus the full repository verification gate. Update current-state/report/review-fix evidence according to repository workflow. If a finding cannot be implemented as written, document the conflict and the safest equivalent outcome rather than silently skipping it.

### Fresh-session read order

Before editing, read:

1. `AGENTS.md`
2. `docs/current-state.md`
3. `docs/plans/phase-1-implementation-plan.md`
4. `docs/reviews/phase-1-independent-review.md` (this file)
5. affected production code/tests named below
6. `docs/architecture.md` only as needed to preserve boundaries
7. `docs/reports/phase-1-implementation-report.md` as delivery evidence, not authority

Do **not** broaden the fix pass into Phase 2.

### Must fix before Phase 2

#### Finding ID: CE-P1-001

**Desired outcome:**<br>
Introduce deterministic project-owned per-file limits that bound total symbol extraction and aggregate qualified-name/ancestry materialization. Preserve Tree-sitter's in-progress match limit as a separate correctness guard. If the project-owned budget is exceeded, fail that file recoverably with no partial symbols and a bounded, deterministic status/diagnostic.

**Likely files/modules:**

- `src/language/mod.rs`
- `src/language/rust.rs`
- `src/language/python.rs`
- `src/model.rs`
- `src/index.rs`
- `src/output_index.rs` if public status/diagnostic shape changes
- `src/language/tests.rs`
- `tests/symbols.rs`
- `README.md`
- `docs/current-state.md`
- implementation/review-fix report evidence

**Prerequisites:**

- understand that Tree-sitter `match_limit` is only an **in-progress matches** limit;
- preserve current parse-size bound and query truncation behavior;
- choose small explicit Phase 1 constants/`AnalysisLimits`, not a general resource-policy framework.

**Validation:**

- deep-nesting over-budget fixture → recoverable limit failure, zero partial symbols;
- flat-many-symbols over-budget fixture → recoverable limit failure, zero partial symbols;
- below-budget fixtures → unchanged correct symbol output;
- deterministic JSON/status/diagnostic;
- full canonical verifier + release build/smoke commands.

### Should fix

#### Finding ID: CE-P1-002

**Desired outcome:**<br>
Do not erase reference/pointer wrappers into a guessed nominal impl container. Preserve the existing documented generic nominal normalization, but treat `reference_type` / `pointer_type` targets as unsupported unless a syntax-preserving representation is deliberately documented. Minimal preferred fix: return no impl container for those target forms.

**Likely files/modules:**

- `src/language/rust.rs`
- `src/language/tests.rs`
- Rust fixture(s) as useful
- README only if wording changes

**Prerequisites:**

- preserve existing direct/generic nominal cases;
- remember `qualified_name` remains syntax-local display/query evidence, not identity.

**Validation:**

- direct/generic nominal target tests still pass;
- `&Foo`, `&mut Foo`, pointer target cases do not become `Foo::method`;
- full verifier passes.

#### Finding ID: CE-P1-003

**Desired outcome:**<br>
Make README source-loading wording accurately distinguish recoverable unsafe/unavailable/oversized states from ordinary concurrent content mutation, while retaining the explicit non-atomic-snapshot disclaimer.

**Likely files/modules:**

- `README.md`
- optionally status/report docs for wording consistency

**Prerequisites:** none beyond preserving the existing threat model.

**Validation:**

- documentation no longer claims generic post-discovery mutation detection;
- context check + full verifier pass.

### Optional cleanup

None required by this review. Avoid opportunistic refactoring.

### Fix-session completion requirements

Before ending the Luna XHigh fix session:

1. run every targeted regression test added for CE-P1-001/002;
2. run:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
./target/release/ctx index .
./target/release/ctx symbols .
./target/release/ctx symbol RepositoryScan .
./target/release/ctx index . --format json
./target/release/ctx symbols . --format json
```

3. preserve all Phase 0 regression tests and trust-boundary behavior;
4. update documentation/report evidence according to `AGENTS.md` and `docs/development/llm-workflow.md`;
5. explicitly record any deliberate deviation from the fix guidance and why the resulting invariant is equivalent or safer;
6. do not mark Phase 1 accepted solely because the implementation session's tests pass — leave final verification/review state truthful.

---

## 13. Final verdict

**Phase status:** **NOT READY FOR PHASE 2**<br>
**Critical:** 0<br>
**High:** 1<br>
**Medium:** 1<br>
**Low:** 1<br>
**Unresolved blocker IDs:** `CE-P1-001`<br>
**Should-fix IDs:** `CE-P1-002`, `CE-P1-003`<br>
**Reviewed commit:** `f6f39f4e91c94fc480c3a1c69248589bf8bbdbe6`

**Verification status:** static review complete; reviewer-side Rust execution/build/test/release smoke verification was blocked because Cargo is not installed and direct repository cloning/network resolution is unavailable in the review runtime. GitHub showed no workflow run/status for the target commit. The implementation report's local verification claims were inspected but not independently reproduced.

**Handoff sufficiency:** yes. This document specifies the required end state, affected code, constraints, ordering and validation for every actionable finding and is intended to be handed directly to a fresh Luna XHigh Codex session without additional reviewer prompting.

**Repository path:**

```text
docs/reviews/phase-1-independent-review.md
```
