# Context Engine — Phase 9 Implementation Plan

**Phase:** 9 — Local Service + Agent Adapter
**Repository:** https://github.com/jasonkli21/context-engine
**Planning baseline:** `main` at `6693892ca20cb1511305243b589f66d1f767d6c1` (2026-10-21; Phase 8 review-fix commit)
**Status:** Proposed implementation authorization, **not** delivery, verification, or independent acceptance
**Primary outcomes:** `ctx serve`, a supported MCP agent entry point, and one transport-independent query/application seam shared with the CLI
**Expected Rust learning:** Tokio runtime, async I/O versus blocking work, ownership across tasks, bounded concurrency, cancellation, connection lifecycles, and protocol testing

> **Rebase gate:** The SHA above is a point-in-time reference, not permission to reset or overwrite a newer tree. Before coding, inspect actual `HEAD`, worktree, `AGENTS.md`, `docs/current-state.md`, reports, plans, accepted ADRs, code and tests. Resolve substantive drift in the phase contract before modifying code. Do not assume this plan is the complete or infallible design.

## 1. Authoritative intent, current reality, and advancement gate

### Product and architecture alignment

- `docs/prd.md` §16.1–16.4: CLI remains first-class; a local `ctx serve` supports persistent consumers; HTTP/IPC and MCP are external adapters, not independent intelligence engines. The provisional daemon endpoint examples are **illustrative**, not a mandate to implement all of them.
- `docs/prd.md` §27, **Phase 9 — Local Service + Agent Adapter**: support persistent consumers; `ctx serve`; HTTP/IPC and MCP adapters; introduce Tokio/async service design and concurrency boundaries; **exit: CLI and external consumers use the same core query layer**.
- `docs/prd.md` §§9, 17, 21–24 and `docs/architecture.md` §§2–4, 12–17, 20–23, 27–29: local-first, determinism, inspectable evidence, stable/versioned external contracts, explicit identity/revision/coverage, sane resource budgets, no repository code execution, and descriptive-only Context Engine semantics. Do not add Context Integrity authority, policy, supersession, or drift judgments.
- `docs/integrations/context-integrity.md`: preserve future evidence/provenance interoperability; the first explicit cross-project adapter remains **Phase 11**, not Phase 9.
- `AGENTS.md` and `docs/README.md`: status is not implementation evidence, plans are not delivered work, review findings must be independently reproducible, and existing canonical docs should be updated rather than creating duplicate status sources.

### What actually exists at the baseline

- **One binary crate** (`Cargo.toml`, `[[bin]] ctx`, `src/main.rs`), synchronous Clap dispatch in `src/cli.rs`/`src/main.rs`, no `lib.rs`, Tokio, HTTP server, or MCP transport yet.
- Existing code paths already compute real results: `scan`, `index`, `symbols`, `symbol`, graph queries (`deps`, `callers`, `related`), `search`, `neighborhood`, `hotspots`, `diff`, `impact`, `context`, `history`, and `evolution`. Presentation is split across `src/output*.rs` but some execution orchestration and warning printing remain in `main.rs`.
- `persistence::reconcile` owns the Phase 6–7 versioned SQLite-derived current-state index (`SCHEMA_VERSION` v3; `INDEX_COMPAT_VERSION` v1; `freshness-v2`) and returns a `Reconciled { artifact, warnings, committed }`. It may perform substantial **blocking** scan, source reads, parsing, SQLite and cache-lock work. The cache stays **outside** the source repository.
- `watch` is foreground, synchronous, event-hint driven, single-owner reconciliation; it is **not** a ready-made background service lifecycle. Queries already reconcile for freshness; do not quietly reinterpret event hints or retain a stale process-global graph.
- `change` and `history` use bounded, on-demand, **committed Git objects**; `history`/`evolution` use first-parent evidence with known lineage/coverage limitations, schema-v2 JSON, and no historical persistence. Phase 5 bundles are full-file UTF-8, byte-budgeted, potentially sensitive source contents—not arbitrary snippets or token budgets.
- External JSON is versioned **per command** (primarily schema v1; history/evolution schema v2). Entity/graph IDs are index- or revision-scoped. Rust/Python syntax and conservative same-file reference resolution are not compiler-level semantics; unsupported/ambiguous evidence is reported.
- Phase 8 original implementation was locally tested, **but** the subsequent review fixes at `6693892` could not compile or complete canonical verification due to Cargo/Rustup dependency availability; fresh Phase 8 independent re-review is pending. Multiple earlier phases also have pending dispositions. See `docs/current-state.md` and `docs/reports/phase-8-{implementation,review-fix}-report.md`.

### Acceptance/advancement rule

The instruction to begin Phase 9 authorizes **work**, not retrospective acceptance of Phase 8 or any older phase. At start, record current unresolved reviews and restore a workable verification environment. Baseline failures must be separated from Phase 9 regressions. If baseline compilation or essential contracts cannot be validated, label Phase 9 **unverified/blocked** and do not claim completion. Do not silently fix unrelated previous phases merely to make the report appear green; fix genuine shared-seam regressions or escalate explicit blockers.

## 2. Phase 9 goal and scoped user journeys

### Goal

A local developer can leave a single, safely configured process running, and a local agent can request codebase intelligence over a documented, versioned protocol without launching the CLI for every query or reimplementing indexing, parsing, ranking, change logic, or output semantics. Existing CLI queries remain correct and testably equivalent.

### Phase 9 must deliver

1. **Local HTTP service** via `ctx serve [PATH]`, bound to loopback only, repository root pinned at startup, explicit authentication, versioned read-only operations and a minimal health endpoint.
2. **MCP adapter** via `ctx mcp [PATH]` (or `ctx serve --transport stdio-mcp` if coherently justified), using stdio and the same application/query layer. No transport logs on MCP stdout.
3. **One reusable application seam** for the exposed operations, invoked directly by CLI, HTTP and MCP. No `Command`/Clap request objects, subprocess-to-`ctx`, JSON-output scraping, or independent parser/index/query implementations in transports.
4. **A representative, end-to-end useful tool set**: repository search, symbol lookup, dependency/neighborhood exploration, committed change impact, task-context assembly. It may expose more existing read operations if low-risk, but not at the expense of this complete vertical slice.
5. **Service security, correctness and concurrency**: fixed scope, least privilege, per-request bounds, admission control, async/blocking separation, explicit freshness, shutdown and error mapping, operational diagnostics.
6. **CLI compatibility and real interoperability evidence**, including an actual MCP client/conformance exercise if available (otherwise honestly report protocol verification scope).

### Explicitly out of scope

- Cross-repository registry/traversal/impact (Phase 10), Context Integrity evidence export/authority (Phase 11), editable/executing agent tools, LLM calls, embeddings, cloud hosting, public binding, OAuth/remote multi-tenant auth, persistent chat/session memory, learned ranking, editor extensions, historical index/daemon-owned history database, replacing the cache store, multi-crate refactor, and an independently optimized streaming indexer.
- Do not implement the PRD's entire *illustrative* endpoint list merely to fill out REST coverage. Do not graft an unfinished MCP protocol approximation into production just to satisfy a checkbox.

### Acceptance journeys

- `ctx search "store" . --format json` and the service/MCP search of the same fixed repository yield the same ordered entity selections, matching reasons, truncation and coverage under stable inputs; any envelope/presentation differences are documented.
- An agent queries an ambiguous symbol and sees multiple matches or an explicit ambiguity rather than one fabricated “unique” declaration; walks its dependency neighborhood with the same bounded resolution/traversal contract as the CLI.
- An agent asks for `impact BASE..HEAD` and obtains exact committed revisions, scoped change facts and limits **independent of dirty working-tree changes**.
- An agent obtains a ranked task-context bundle with explicit full-file source exposure, scoring reasons, `file-ranker-v1`, exact byte budget, skipped candidates and source/analysis warnings; no privilege/scope widening through request arguments.
- Concurrent HTTP calls cannot corrupt cache state, exceed admitted blocking capacity, leak source to unauthenticated local clients, hang indefinitely as an unbounded pending work queue, or keep serving a silently stale snapshot after filesystem changes.
- On shutdown, no new requests are accepted; in-flight operations have a documented drain bound and no unsupported claim that Tokio timeout kills blocking Git/SQLite work.

## 3. Architectural decision: one application layer, two transports

Recommended incremental shape (exact file placement may evolve):

```text
                              ctx CLI
                                 │
 local authenticated HTTP ───────┼─────── MCP stdio (agent)
             │                   │               │
      request validation        CLI           JSON-RPC + tools
      auth/version/bounds      argument        validation
             └──────────────────┼───────────────┘
                                ▼
                   application/query facade
                 typed requests/results/errors
                                │
             ┌──────────────────┼─────────────────────────┐
             ▼                  ▼                         ▼
      persistence/reconcile   graph/query/rank        change/history
      current repository      source bundles         immutable Git
             └──────────────────┼─────────────────────────┘
                                ▼
                   explicit coverage/evidence
                                │
                   versioned output adapters
```

**Build order matters:** first extract a **small, synchronous** application seam covering the chosen operations, migrate their CLI callers to it, and prove unchanged CLI behavior. Only then add HTTP and MCP adapters. Keep the core synchronous: Tokio is appropriate for network/stdio I/O and lifecycle, **not** a justification to rewrite parser/SQLite/Git code as async. Keep one binary crate unless production testing or reuse materially requires a library crate.

### Suggested code ownership (not immutable filenames)

| Area | Responsibility | Rules |
|---|---|---|
| `src/application.rs` or `src/app/` | Typed use cases: search, symbol, neighborhood, impact, context; common input validation; return domain results, warnings and snapshot scope | No Clap, Tokio, HTTP, MCP, printing, response bodies or transport auth |
| `src/main.rs`, `src/cli.rs` | Preserve existing CLI dispatch, flags, exit behavior; add `ServeArgs` and MCP entry | Call the application seam for shared operations; preserve historical defaults/output shape |
| `src/service.rs` or `src/server/` | Loopback binding, HTTP routes, auth, request limits, error/status mapping, graceful shutdown | No parser/index/ranker algorithms; never accept arbitrary repository root from caller |
| `src/mcp.rs` | MCP protocol bridge, declared tool schemas, tool dispatch and stdio framing | Protocol-correct, stable tool naming, no non-protocol stdout |
| `src/output_service.rs` or `src/transport_dto.rs` | Stable versioned HTTP/MCP representations and bounded serialization | Preserve provenance, native-path identity where necessary, uncertainty and omission data |
| `src/persistence.rs` and existing domain modules | Existing reconciliation/analysis and evidence semantics | Minimal refactor; no schema or semantic-version bump unless existing persisted analyses change |
| `tests/service.rs`, `tests/mcp.rs`, CLI/regression tests | Socket and subprocess black-box tests, parity, security, hostile inputs, concurrency | No tests that pass only because mocked transport bypasses production dispatch |

### Core request API

Define a deliberately small, typed operation vocabulary, e.g. `Search { query, kind, limit }`, `FindSymbol { name, file? }`, `Neighborhood { symbol?/file?, depth }`, `Impact { range, depth }`, `TaskContext { task, budget, change? }`. Adapt names to existing domain APIs rather than add speculative abstraction. Domain requests must not carry arbitrary cache directories, executables, Git flags, network URLs, absolute source paths, or roots supplied by remote callers. The trusted process constructor owns `RepositoryConfig { canonical_root, parse_limit, ... }`.

- Shared validation: UTF-8 query/task budgets, allowed enum and depth bounds, exact relative file selectors, Git range grammar and committed revisions, context output budget, excessive JSON fields, duplicate/unknown arguments.
- Shared application results should retain the original command's coverage, warnings, scope, output ordering and truncation. If sharing a DTO implementation with existing `output*.rs` is worthwhile, extract it consciously; do **not** call the CLI renderer and parse its JSON back into transport state.
- Because cache reconciliation currently returns warnings while CLI `main.rs` prints them, move this side effect **out** of the shared core. CLI prints to stderr; HTTP/MCP return structured warnings and log operational diagnostics separately. Never write uncontrolled client-derived strings to logs/stdout without escaping/redaction.
- **Scope semantics**: current-state queries use a best-effort validated live repository generation, which is not a global atomic filesystem snapshot; change/history use immutable committed Git objects. Never brand both with an unqualified single “revision.” Expose revision, state kind, and whether freshness could be fully established; preserve fallback/incomplete flags.
- Externally exposed numeric entity IDs remain scoped/ephemeral. Include grounded selectors (native path identity if needed, symbol name/kind/range, Git commit) and explicitly label ID scope; do not market Phase 8 heuristics as stable symbol lineage.
- Avoid a single mega-result allocated twice. Existing command-specific output preflight and per-query bounds must still apply; impose an additional service-level response bound before sending bytes.

## 4. HTTP local service contract

### CLI and lifecycle

Proposed minimal UX (final spelling may change to fit Clap style):

```bash
# Corrective note added 2026-10-21: the server requires 64 hexadecimal characters.
# Supply a strong token through the process environment (not CLI argv).
CONTEXT_ENGINE_SERVE_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')" \
  ctx serve . --listen 127.0.0.1:0

# Discover the selected loopback address in an escaped startup line on stderr;
# obtain the secret out of band from the launching user's environment.
ctx mcp .
```

- Resolve/canonicalize the **single repository root once at startup**; reject nonexistent or invalid roots. Support non-Git directories where operations permit; Git-only operations return precise errors on non-Git inputs.
- Bind to `127.0.0.1` (optionally `::1` after explicit coverage); reject wildcard, public IPv4/IPv6, Unix-socket paths outside trusted configured locations, and hostname tricks. Default an OS-assigned port (`:0`) rather than assuming a fixed port is available. Do not enable CORS or cross-origin credentials.
- Require a sufficiently strong per-process bearer token from `CONTEXT_ENGINE_SERVE_TOKEN` before accepting any intelligence endpoint (prefer fail-closed startup if absent; avoid logging the token). A dedicated env var is intentionally simpler than OAuth or a hand-rolled secret file; note that same-user processes are within the local trust boundary. Validate the `Host` authority and reject unauthorized `Origin` headers to mitigate DNS rebinding / cross-origin browser access even if authentication is configured. Constant-time compare for secrets if practical, and never include secret values in errors, URLs or JSON.
- Restrict endpoint methods; send `Cache-Control: no-store` and cautious content-type/anti-sniffing headers. Do not expose an all-files API, arbitrary file read, command execution, git fetch, filesystem write, root override or write operation. No external network egress is required.
- SIGINT/SIGTERM graceful shutdown: stop accepting connections, enforce a documented drain interval, and communicate what happens to already-running blocking tasks; no zombie watcher threads or unexpected cache mutation. No permanently running watcher is necessary in Phase 9.

### Suggested HTTP v1 surface

| Route | Request | Shared domain operation | Key semantics |
|---|---|---|---|
| `GET /health` | no repository data, bounded | process readiness | No source/paths/secrets in unauthenticated response; clarify whether readiness includes index availability |
| `POST /v1/search` | `{ "query": "...", "kind": "all", "limit": 20 }` | Phase 3 search | Stable ranking/tie break, truncation, non-UTF-8 path omissions |
| `POST /v1/symbol` | `{ "name": "...", "file": "src/lib.rs" }` | Phase 1/2 exact symbol discovery | Multiple matches and explicit ambiguity, provenance and scoped IDs |
| `POST /v1/neighborhood` | `{ "symbol": "...", "depth": 1 }` or `{ "file": "...", "depth": 1 }` | Phase 2/3 graph navigation | Bounded BFS, edges/status, unresolved relations, coverage |
| `POST /v1/impact` | `{ "range": "BASE..HEAD", "depth": 1 }` | Phase 4 committed impact | Immutable Git scope and exact base/head, no dirty checkout dependency |
| `POST /v1/context` | `{ "task": "...", "budget": 131072, "change": null }` | Phase 5 task context | Full-file content sensitivity, ranker version, exact budget, ranking/omissions |

Optional low-risk endpoints such as history, evolution, stats or hotspots may be added only if there is genuine time and protocol testing. Explicitly decide whether the externally exposed `find_symbol` operation should list all exact matches, return an ambiguity error, or both; never silently choose the first.

A versioned result envelope should contain at least: `schema_version`, `operation`, `repository` (scope/identity), `snapshot` (`current_worktree` or `committed_range`, revision metadata where known), `data`, `coverage`/`warnings`, `truncated`/`omitted` as appropriate. Keep *per-operation* domain DTOs stable and accurately qualified; avoid claiming that every operation has a meaningful `git_head` or that the index generation is a universal symbol identity. Adopt fixed JSON field casing/order/encoding and stable machine error codes. Keep human CLI output free to evolve separately.

**Error classes and example status mapping:** malformed JSON/unknown field/out-of-bounds/invalid path → 400 (or 413 for oversized body); no permitted symbol/path → 404 where semantically valid; ambiguous selector → 409 or a deterministic multiple-match result; overload → 429/503 with explicit retryability; expired/invalid bearer → 401; rejected Origin/Host → 403/400; analyzer/source/cache issues → typed 4xx or 5xx according to fault, retaining coverage when partial success is supported; internal failures → 500 with redacted message. Auth checks must precede any analysis or identifiable data disclosure. Distinguish operational error vs partial evidence in success payloads. Do not convert safety-limit failures into silent empty results.

### Resource budgets

Use implementation constants with unit tests and clear defaults, not an unbounded user-configurable framework. Suggested conservative starting points, to tune with real tests:

- Request body <= **64 KiB**; HTTP header count/size controlled by framework; expected paths and queries still obey the existing 4,096-byte query/task contracts.
- Response serialized bytes <= **8 MiB** by default for HTTP/MCP; context raw-content budget <= **1 MiB by default for service** (may be configured up to the existing 16 MiB CLI hard maximum but cannot override output bound). Service-specific cap differs explicitly from CLI; it must never pretend it packed more than it did.
- At most **1 actively executing repository job per served root** initially (safe single owner); at most a small, fixed number of admitted pending requests (prefer fail-fast when full). Handle socket concurrency asynchronously while doing scan/parse/SQLite/Git/JSON-heavy work only through a bounded `spawn_blocking` admission gate.
- The Tokio task timeout must **not** be advertised as stopping a `spawn_blocking` operation: blocking tasks are generally not force-cancellable after start. Prefer a single-owner bounded queue with bounded internal work, explicit timeout/drain accounting and overload refusal. Do not spawn unlimited detached tasks or drop admission permits before blocking work ends.
- Continue to enforce existing source, traversal, graph, history, index, output and Git record caps. Avoid accidental N× allocations when multiple consumers request large context bundles; test pathological workload limits and partial-result semantics.

## 5. MCP adapter: useful agent tools without a second protocol engine

### Protocol/version policy (verify before implementing)

As of **2026-10-21**, the official MCP specification release is **`2026-07-28`**, which replaces the old `initialize`/`notifications/initialized` stateful handshake with per-request `_meta` and changes tool/result and caching conventions. The `2025-11-25` protocol is a *different*, legacy wire contract. Read the **normative specification and changelog at implementation time**, not tutorials with unversioned examples:

- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/basic/index.mdx
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/basic/transports/stdio.mdx
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/server/tools.mdx
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/changelog.mdx
- Legacy compatibility reference: https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2025-11-25/basic/transports.mdx

**Decision gate:** inventory maintained Rust MCP SDKs, compatible protocol revisions and real clients. Prefer a maintained SDK whose actual behavior can be independently tested. The recommended primary target is `2026-07-28`; if SDK/client support is insufficient, ship **explicitly labeled, interoperable `2025-11-25` compatibility** first *with tests* and record why the current revision is deferred, or implement both through a tested compatibility adapter. Do **not** mix the 2025 initialize flow with 2026 `_meta` handling, pretend protocol versions are interchangeable, or handwave conformance. No full MCP-over-HTTP requirement for Phase 9: stdio is enough for the agent adapter, alongside the native local HTTP API.

### Transport and tool contract

- `ctx mcp [PATH]` starts an agent-launched stdio server; read/write **newline-delimited UTF-8 JSON-RPC** as required by the pinned protocol. Only MCP messages on stdout; escape/log diagnostics on stderr. Enforce message size, UTF-8, invalid JSON, unknown request IDs/methods, notification semantics, EOF, repeated requests, mixed success/failure and version negotiation correctly for the chosen spec.
- Stable tool catalog (names can be pinned before implementation): `search_codebase`, `find_symbol`, `get_dependency_neighborhood`, `get_change_impact`, `build_task_context`.
- Every tool publishes correct JSON Schema input properties, required fields, valid ranges, bounded descriptions, deterministic tool order, and read-only annotations **only when supported by the pinned revision**. Avoid misleading claims of compiler-verified call resolution or test coverage.
- Tool success carries **structured JSON** if supported by the revision/SDK, plus clear text as needed for client compatibility. Preserve result `coverage`, warnings, truncation, source scope, identities and ranker reasons, not just a string of pretty CLI output. Map tool-execution errors to protocol error results per the chosen spec; malformed JSON-RPC versus a valid tool call that fails are **different** cases.
- The adapter is **read-only** and fixed-root; it may return **full source files** from `context`, so process launch and trusted client are part of its confidentiality boundary. No `write_file`, `apply_patch`, `run_command`, arbitrary `read_file`, or agent-requested server launch flags.
- Test with a real MCP client/official inspector or interoperable SDK where available, as well as subprocess JSON-line tests and adversarial protocol vectors. State which protocol version(s), SDK version(s), and client(s) actually passed.

## 6. Freshness, isolation, correctness and security invariants

1. **One source of truth for analysis:** all adapters invoke the existing `persistence::reconcile` + graph/query/change/context pipelines. Do not duplicate indexing or maintain a second service-owned SQL schema.
2. **Never trust path input from a request:** canonical root is launch-owned; selectors are normalized repository-relative and checked before access. Preserve exact native paths when identity matters; lossy UTF-8 rendering cannot authorize access to a different native path.
3. **No repository code execution or network source upload.** Git is read-only via existing hardened Git layer; do not accept arbitrary revision flags/command arguments.
4. **No durable snapshot illusion:** dirty current-tree queries and committed revision queries are different types of evidence; no claims of atomicity beyond what reconciliation actually guarantees.
5. **No fake cross-file semantic precision:** same-file resolved calls remain syntax heuristics; unsupported languages, relation gaps, parse failures, truncated traversals and other conservative omissions remain visible.
6. **Lock/ownership clarity:** share one immutable configuration and one authoritative blocking-work admission path; do not run a parallel watcher that races cache writes. Existing sidecar lock and generation checks remain safety backstops.
7. **Do not leak diagnostics:** errors, HTTP logs and MCP stderr escape hostile control characters and avoid accidental printing of task-context file bodies, request tokens, arbitrary headers and potentially sensitive paths.
8. **No implicit public service:** loopback, bearer auth, strict Origin/Host, limited read-only surface and no unsafe cross-user assumptions. Treat localhost as potentially hostile (browsers and other local processes can connect).
9. **No speculative output unification:** retain existing CLI JSON contracts; new transport schema versions must change deliberately and be tested with golden fixtures. If shared semantic fixes require changes, document/justify compatibility impact.
10. **Versioning discipline:** bump `INDEX_COMPAT_VERSION` only for changes that alter scanner/classifier/parser/relation/persisted enum meaning; bump `SCHEMA_VERSION` only for derivative cache format changes. A mere service/APIs addition should require neither.

## 7. Implementation sequence with checkpoints

### Step 0 — Baseline reconnaissance and verification gate

- Inspect actual HEAD/worktree, `AGENTS.md`, canonical docs, related reports, new decisions and existing tests.
- Confirm baseline `./scripts/verify.sh` status; attempt to resolve environment dependency issue **without modifying machine-global configuration**. Run basic CLI JSON smoke. Record failures as pre-existing or implementation-introduced; do not claim a passing baseline if unavailable.
- Verify official MCP version against current normative docs, test SDK options, and record the chosen supported protocol(s), explicit compatibility posture and security model in the implementation report/accepted ADR **only if this becomes a durable decision**.

### Step 1 — Transport-free application seam and CLI migration

- Refactor only the five selected operations into a shared typed request/result path. `main.rs` still handles Clap and rendering; warnings become structured rather than core `eprintln!` side effects.
- Preserve all CLI flags/defaults/JSON and errors under current tests. Add parity tests on stable Rust/Python fixtures for search/symbol/neighborhood/impact/context. Include ambiguous resolution, unresolvable references, warnings, dirty tree and exact context budget.
- Keep history/evolution and watch CLI untouched unless small changes are essential for separating responsibilities.

### Step 2 — Safe lifecycle and bounded blocking executor

- Introduce Tokio/runtime and a minimal HTTP stack, pinned to versions compatible with Rust edition/toolchain and project dependency strategy. Keep blocking analyzer/SQLite/Git outside reactor threads.
- Bind fixed root, loopback-only address and bearer token config. Implement admission gate, operational error types, signal shutdown and stderr lifecycle logs. Reject overload **before** creating expensive work.
- Exercise same-root concurrent requests and mutation races. Ensure admission permits live through completion of blocking work, including timeout/error paths.

### Step 3 — Native HTTP vertical slices

- Wire health + five operations through the common seam; validate input before analysis, use strict JSON, versioned envelopes and bounded serialization. No command dispatch duplication.
- Add socket-level tests using ephemeral port: bearer missing/wrong/correct, Origin/Host, non-loopback bind refusal, unknown route/method, large payload/response, body/content-type mismatch, repo-boundary path attempts, non-Git paths, stable result ordering, error statuses, graceful shutdown.
- Compare HTTP responses against CLI results semantically (excluding deliberately different envelopes), including warning/truncation/source scope and mutable-worktree scenarios.

### Step 4 — MCP stdio agent adapter

- Integrate the pinned protocol using a tested SDK or a narrowly reviewed implementation, with explicit decision for 2026 modern versus 2025 legacy compatibility.
- Register tools that call the **same** application functions; test their schemas, exact argument rejection, tool results and errors.
- Run subprocess/real-client interoperability checks. Ensure no non-protocol stdout, no missed notification framing, no invented `initialize` requirement for a 2026-only adapter, no incorrect legacy fallback, and no filesystem/root override.

### Step 5 — Real-world performance, reliability and end-to-end examples

- On the engine repository and at least one *other* real Rust/Python repository, check cold/warm query behavior, multiple sequential agent calls, concurrent socket clients, mutation between requests, provenance, resource refusal and output byte caps. Record observations, not unmeasured SLA claims.
- Verify HTTP-only versus MCP-only startup/shutdown and combined CLI regression. Ensure the index cache remains external and that enabling the service does not modify scanned repository files.
- Confirm no dependencies or processes remain after normal termination, and no permanently running watcher is needed.

### Step 6 — Documentation, report and phase gate

- Update `README.md` with secure `ctx serve` and `ctx mcp` usage, endpoint/table schema examples, MCP protocol version/support matrix, source-content confidentiality warning and troubleshooting. Explain known semantic limits plainly.
- Update `docs/architecture.md` only to describe **delivered** service/application/async/concurrency boundaries, shared query layer and future seams; remove stale “Phase 9 future” claims only when shipped. Keep original architectural principles intact.
- Update `docs/current-state.md` as the **single** status snapshot: what shipped, what did not, verification matrix, Phase 8 acceptance state and Phase 9 independent review pending. Do not silently mark Phase 8 or earlier work accepted.
- Create `docs/reports/phase-9-implementation-report.md` with baseline SHA, actual code/module changes, decisions/deviations, commands/results, protocol/version/client matrix, security assumptions, limitations, performance/resource observations and pending independent-review status.
- Only change `docs/prd.md`, `docs/integrations/context-integrity.md` or add an ADR if a **durable** product/architecture decision truly changes; do not rewrite PRD as delivery evidence. Sweep README, CLI help, `AGENTS.md` references, docs index and previous “future” phrasing for staleness; avoid duplicate policy guides and session notes.

## 8. Focused validation matrix

| Layer | Required scenarios | Minimum evidence |
|---|---|---|
| CLI preservation | Old flags/defaults/JSON/golden behavior; all command families unaffected | Existing canonical suite + service-operation parity tests |
| Shared app | Search, exact/ambiguous symbol, bounded neighborhood, committed impact, explainable context | Unit + integration tests against production path |
| Local HTTP | Token, strict Origin/Host, bind enforcement, methods, payload caps, paths, typed errors, response version | Real socket tests using ephemeral loopback |
| MCP | Pinned protocol revision(s), stdio framing, tool listing/calls, schema validation, metadata/handshake correctness, errors, EOF/stdout purity | Subprocess + actual client/conformance check where possible |
| Concurrency | simultaneous requests, resource overload, source mutation, cache lock/generation races, shutdown | Deterministic contention/failure-injection tests; no hang/deadlock |
| Fidelity | ordered ranked results, provenance, unsupported/ambiguous evidence, truncation, non-UTF-8 path identity, committed vs dirty distinction | CLI/HTTP/MCP semantic comparison |
| Safety | path traversal, symlink boundaries, hostile control chars, malicious Git metadata, huge request/source/result, token leaks | Negative/adversarial tests and redacted diagnostics |
| Reality | cold/warm queries on this repository + separate real Rust/Python repo | Commands, observed counts, limitations, independent-review status |

Run, to the extent a valid environment supports it:

```bash
./scripts/verify.sh
cargo check --all-targets
cargo build --release
git diff --check
# plus the concrete, documented HTTP socket and MCP client smoke commands
```

Tests that are **not run** must be labeled so. Successful typechecking, unit tests or a hand-crafted JSON-RPC request are not a substitute for interoperability, security or repository end-to-end verification. Record Linux/Windows/macOS environments actually tested; do not imply cross-platform approval without execution.

## 9. Risk register and trade-offs

| Risk | Mitigation / explicit test |
|---|---|
| Transport implements logic differently from CLI | Move selected CLI uses through one application seam first; semantic parity fixture |
| `spawn_blocking` timeout leaves unbounded work behind | Queue/admission cap and completion-held permit; no false cancellation claim; bounded core algorithms |
| Multiple HTTP requests race cache/SQLite | Single-root worker owner initially, existing lock/generation checks; deterministic contention test |
| Long-lived process serves silently stale index | Per-request authoritative reconciliation; don't cache `Arc<Graph>` without generation invalidation proof |
| Local HTTP leaks source through DNS rebinding/CSRF/other users | loopback-only, bearer required, strict Host/Origin, no CORS, no network binding, no secrets in logs |
| Agent can choose paths outside intended root | immutable launch-owned root and checked native relative selectors; no absolute arbitrary file reads |
| MCP protocol silently uses obsolete handshake | explicit release pin + SDK/client verification and legacy mode decisions |
| Full context bundle blows memory or model budgets | preserve existing 16 MiB CLI cap; lower service default; preflight before serialization; label full-file outputs |
| JSON shape or ID semantics mislead downstream agents | versioned DTOs, scope/revision/coverage, stability disclaimer, deterministic results |
| Git range/dirty checkout conflation | committed impact validated against Git objects; current-state queries labeled distinctly |
| Phase 8 code remains uncompiled after review fix | baseline compile gate, separate inherited failures, no acceptance implication |
| Speculative migration to multi-crate / framework | retain single crate unless concrete tests demand otherwise; defer editor/API expansion |
| Documentation says shipped before tests | report exact commands, current-state flags pending review, docs staleness sweep |

## 10. Phase 9 exit criteria

Phase 9 is **implementation-complete**, but **not yet independently accepted**, only when all are true:

1. `ctx serve` starts a loopback-only, fixed-repository, authenticated read-only HTTP service and terminates predictably.
2. A pinned, real MCP stdio adapter can list and invoke the defined tools through protocol-accurate request/response handling; actual client compatibility is evidenced or transparently qualified.
3. All five minimum operations are available and correct across **CLI and both adapters**, with no transport-specific intelligence logic, subprocess delegation or JSON scraping.
4. Existing CLI contracts remain intact; meaningful differences in new HTTP/MCP wrappers are explicitly versioned/documented. Current versus committed snapshot semantics, warnings, truncation, ambiguity and ranking reasons survive transport boundaries.
5. Resource/admission concurrency is bounded, blocking work never executes on the async reactor, and failure/overload/shutdown paths do not abandon unbounded tasks or silently corrupt cache state.
6. Scope and confidentiality threats are tested: no public binds, unauthenticated access, Origin/Host bypass, traversal, arbitrary file execution/write, source egress or stdout contamination.
7. Canonical verification, release build, relevant socket/MCP black-box tests and two real-repo smokes are executed or exceptions clearly documented **without false success claims**.
8. Report, README, architecture and current-state docs match **verified** behavior; Phase 8 and earlier pending review dispositions remain accurately represented.
9. A subsequent **independent review** can discover plan defects as well as implementation defects; this plan must not be used to exclude problems outside its checklist.

**Suggested next step after delivery:** hand an independent reviewer a Phase 9 review prompt with the implementation diff/SHA, not merely this plan. Fix findings in a separate focused pass and verify again; only the maintainer should choose whether to accept Phase 9.
