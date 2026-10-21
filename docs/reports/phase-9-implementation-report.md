# Phase 9 implementation report

Date: 2026-10-21. Implementation evidence; independent review and owner acceptance
remain pending. Phase 8 and all earlier pending review dispositions are unchanged.

## Baseline and commit identity

Baseline: `6693892ca20cb1511305243b589f66d1f767d6c1` on `main`. The only initial
worktree change was the supplied untracked Phase 9 plan; it is included in the
single implementation commit. No reset/rebase or unrelated user-file overwrite.
The final SHA is the single commit containing this report, available with
`git log -1 --format=%H -- docs/reports/phase-9-implementation-report.md` and in
the delivery message. A report cannot embed its own content-addressed commit SHA.

Initial `./scripts/verify.sh` passed context/format checks but registry access was
blocked by sandbox DNS. `cargo fetch --locked` with approved network access fetched
existing locked dependencies. The next baseline verifier reached compilation and
exposed inherited Phase 8 failures: `history::Work::snapshot` sorted a non-mutable
vector; its source-budget test multiplied an `Option<u64>`. Minimal fixes make the
vector mutable and unwrap the fixture's known blob size.

The first full unit run then exposed two inherited fixture problems: the empty-blob
inventory test reused a four-byte path budget for `empty.py`, and APFS rejected an
invalid UTF-8 native filename with EILSEQ (92). The empty case now has its own exact
path budget; the filesystem-dependent test recognizes that specific macOS refusal.
Injected native path/SQLite round-trip tests remain active. A subsequent integration run exposed the same APFS issue in a historical fixture
and an obsolete assertion that global and file-scoped history envelopes must be
byte-identical. That assertion now compares symbol/event/co-change evidence and
explicitly checks their intentionally different HEAD lookup scope. These six fixes are
separate from Phase 9 behavior in the diff. No scanner/parser/relation semantics,
`INDEX_COMPAT_VERSION`, `SCHEMA_VERSION` or freshness version changed.

## Delivered code and contracts

- `src/application.rs`: synchronous typed search, symbol, neighborhood, committed
  impact and context use cases; structured warning ownership; trusted pinned root.
- `src/model.rs`: cloneable symbol descriptors for an owned exact-lookup result;
  serialized fields and analysis semantics remain unchanged.
- `src/main.rs`, `src/cli.rs`: all five CLI operations use the same seam; existing
  human/JSON renderers and flags remain. `serve [PATH] --listen IP:PORT` and
  `mcp [PATH]` are explicit entry points. Other commands remain synchronous.
- `src/output{,_index,_query,_change,_context}.rs`: shared serializable DTO builders
  used directly by all adapters; no CLI subprocess, JSON scraping or second engine.
- `src/adapter.rs`: strict typed adapter arguments, runtime constraints, schema-v1
  response/state/warnings envelope, serialization counting before Value allocation,
  redacted typed error categories and completion-owned blocking admission.
- `src/service.rs`: Tokio/Axum loopback server, bearer/Host/Origin/method/content
  validation, bounded connections/readers/body/response, signal shutdown and draining.
- `src/mcp.rs`: official SDK tools, modern metadata guard and bounded stdio wrapper.
- `tests/adapters.rs`: seven actual subprocess/socket/official-client integrations;
  two additional adapter unit tests cover cancellation ownership and escaped size.
- `Cargo.toml`, `Cargo.lock`: Tokio 1.54.0, Axum 0.8.9, schemars 1.2.3 and pinned
  official Rust MCP SDK rmcp **3.6.0** (same version as the actual test client).
- README, architecture, current state, supplied plan and this report document delivery.

HTTP API v1 exposes POST `/v1/search`, `/v1/symbol`, `/v1/neighborhood`, `/v1/impact`
and `/v1/context`. MCP tools respectively: `search_codebase`, `find_symbol`,
`get_dependency_neighborhood`, `get_change_impact`, `build_task_context`. CLI
counterparts: `search`, `symbol`, `neighborhood`, `impact`, `context`. Neighborhood
provides the dependency/exploration vertical slice; legacy `deps` remains CLI-only.

Adapter success: `api_version: 1`, `state`, structured `warnings`, and `result`
containing the existing schema-v1 command envelope. Current results are explicitly
`current_working_tree_best_effort`; impact is `committed_revisions`. Existing ranker,
coverage, truncation, ambiguity, occurrence evidence, byte accounting and ephemeral
ID semantics survive. No historical endpoints, edit tools or execution tools added.

The HTTP status/error mapping is documented in README: invalid input 400, missing
credentials 401, unsafe authority/origin 403, unknown route 404, method 405, body
read timeout 408, payload/result limit 413, content type 415, query/Git unavailable
422, overload 503, query timeout 504, internal failure 500. Fixed messages omit raw
filesystem/Git errors. Operational reconciliation warnings remain structured data;
CLI warnings are escaped on stderr. Token/header/body/source logging is absent.

## Protocol evidence and design corrections

Normative sources checked before SDK selection:

- [2026-07-28 versioning](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
- [2026-07-28 tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)
- [2026-07-28 transports](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)
- [official changelog](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/changelog.mdx)

Supported revision is **2026-07-28 only**, not the legacy initialization format.
The official rmcp client exercises `ClientLifecycleMode::Discover` against the real
`ctx mcp` subprocess, listing/calling all five tools and receiving modern results.
Raw subprocess tests also exercise required metadata, unsupported 2025 version,
malformed/oversized input recovery, notifications, multiple requests, IDs and EOF.
No independent third-party client, external certification or cross-SDK conformance
is claimed. SDK client execution is materially stronger than hand-written frames
alone but does not prove complete protocol conformance.

The SDK's generic dual-era `serve` bootstrap handles discovery but remains in its
bootstrap loop until the first valid non-discovery request. Testing revealed that
notifications during that interval and discovery-only EOF produce initialization
errors. This modern-only server therefore uses SDK `serve_directly`, guarded by
mandatory per-request modern metadata validation and explicit initialize rejection.
The SDK still owns framing, request/result parsing, dispatch, cancellation and
server metadata. No custom JSON-RPC engine was introduced.

The SDK drops syntactically malformed JSON and oversized lines, then continues.
Tests verify recovery; they do not invent a parse-error reply the SDK never sends.
The adapter limits outstanding requests to 16, rejects duplicate outstanding IDs
by closing stdio, and applies a ten-second stdout write deadline. Cancelled calls
that produce no response conservatively retain a slot until process exit. A client
issuing many cancellations must reconnect. This avoids unlimited pending SDK
responses while keeping the work-owner boundary independent of client cancellation.

All five tool input schemas derive from the same strict serde structs. A separate
`jsonschema` 4.25.1 Draft 2020-12 validation run checked every exported schema,
a valid instance and rejection of an unknown `root` field. It ran with
`UV_CACHE_DIR=/tmp/context-phase9-uv-cache uv run --no-project --with jsonschema python /tmp/context-phase9-schemas.py`
in an isolated temporary environment; all five passed. Numeric
and string schema bounds are declared; runtime checks additionally enforce UTF-8
byte limits, range grammar and exactly one neighborhood selector. Source paths
are only selectors over the canonical inventory; no arbitrary read endpoint exists.

## Security and concurrency model

Launch configuration pins one canonical directory and default parse ceiling.
HTTP clients cannot choose root, cache, executable, environment or arbitrary source
paths. Startup rejects non-loopback/wildcard IPs and missing/malformed tokens;
expected tokens are 64 hex characters generated from 32 random bytes. Format
validation cannot prove entropy; the launch user must generate a real secret.
Authorization uses a fixed-size digest comparison. Only health omits authentication,
and health exposes no root/revision/source. Exact bound Host authority is required;
all Origin headers are rejected. No CORS. Application responses are no-store/nosniff.
HTTP parser errors may happen before application response headers are attached.

HTTP has 32 connection slots with 45-second maximum lifetimes, 16 body readers,
16 KiB body ceiling, ten-second body timeout, 8 MiB response limit, and a 30-second
query timeout. MCP has a 16 KiB incoming frame and 8 MiB complete result ceiling,
including both structured data and text plus a reserve for outer protocol framing.
Adapter context budget is 1 MiB; CLI retains its 16 MiB budget. Legacy core safety
limits and preflights remain. No schema/persisted-analysis version bump is needed.

One admitted blocking job owns a semaphore permit inside `spawn_blocking` through
query, DTO construction and result preparation. New work fails immediately while
busy. A dropped/timed-out caller does not release the permit. Current operations
invoke normal reconciliation every time; graphs reconstruct through the canonical
constructor. The existing external cache generation/sidecar lock remains the only
writer coordination path. No service-owned graph, parallel parser or automatic
watch process exists. Committed impact bypasses current source/cache analysis.

SIGINT/SIGTERM stops accepting HTTP, permits five seconds of socket draining, then
waits for admitted blocking work with no hard deadline. EOF does the same work
drain for MCP. CPU/data work is bounded, but existing filesystem/Git I/O has no
hard wall-time guarantee. This is an honest drain policy, not killable analysis.

Same-user processes and trusted MCP clients are inside the trust boundary. A valid
token or stdio client can obtain repository metadata and full-file context. This
is not a multi-user sandbox. Existing generated/vendor/hidden/ignore exclusions,
Unix descriptor-relative safe reads and external-cache separation remain active.
Current DTO lossy-path warnings are preserved; no new stable native-path selector
or cross-edit identity contract is claimed.

## Verification

Executed locally on aarch64 macOS with the existing stable Rust 1.99.1 toolchain:

| Command | Final result |
| --- | --- |
| `./scripts/verify.sh` | PASS: context checks, formatting, Clippy with warnings denied, 106 unit / 78 integration tests |
| `cargo check --all-targets` | PASS |
| `cargo build --release` | PASS |
| `git diff --check` | PASS |
| `cargo test --test adapters` | PASS: seven real adapter integrations (also included in canonical verification) |
| Exported tool JSON Schema validation | PASS: jsonschema 4.25.1, all five schemas / valid instances / unknown-field rejection |
| Release CLI/HTTP/MCP smoke | PASS on both repositories' `src` scopes; five operations with full nested-DTO equality, repeated nonempty context, discovery/tool list, no-store and clean exits |

Two filesystem-dependent native-name cases return early on APFS's EILSEQ; this
is not evidence that APFS supports non-UTF-8 filenames. Injected native path,
identity and store tests execute. Existing Phase 0–8 suites all run. Adapter tests
cover ambiguity, limit truncation, depth, exact UTF-8 content bytes, dirty current
updates versus committed impact, unknown tools/arguments, valid Git tool failure,
metadata/version errors, malformed/oversize-frame recovery, notifications and EOF.
Socket tests cover auth, duplicate/hostile Host, Origin, methods/content type,
malformed/duplicate/unknown JSON, excessive body/response, relative path escape,
non-Git/redacted errors, source/cache boundaries and SIGTERM. A real SQLite writer
reservation stalls a request while competing requests receive 503; disconnect
retains work ownership and later queries see the updated generation. A controlled
blocking-job unit test verifies cancellation-held admission independently of timing.

Intermediate failures were corrected, not reported as green: registry/sandbox
restrictions, inherited compile/test issues above, fixture path lengths, initial
SDK discovery-field assumptions, SDK bootstrap behavior, Clippy suggestions and
contention-fixture scheduling. Loopback tests require approved execution outside
the sandbox network restriction. No machine-global toolchain/config was edited.

### Real repositories

Release executions use external temporary caches, ephemeral authenticated loopback
HTTP, and modern MCP stdio. Context Engine is the actual modified checkout at
baseline `6693892`; Context Integrity is the actual independent Python checkout at
`917a9d318824d4fa67a7206dd3784068dabc7a21`. Both are scoped to `src`; no repository
source was changed by smoke scenarios. Search/symbol/neighborhood use `snapshot`
(engine) and `parse_config` (Integrity). Context tasks are `source` and
`parse_config`, with 16,000-byte budgets and nonzero selected content. Impact uses
`HEAD..HEAD`; nonempty committed deltas and dirty isolation are additionally covered
by the existing change suites and the adapter fixture.

An initial whole-engine-root smoke failed honestly with `repository scan safety
limit exceeded`. The existing ignored-file preflight also visits build artifacts:
`target` contained 58,708 files (5.5 GiB), already beyond the 50,000-entry ceiling.
The root can exhaust entry/path bounds before analysis. Nested scopes preserve
real repository/Git semantics while excluding that build tree; Phase 9 does not
weaken the scanner's pre-existing guard or clean user build artifacts. This is a
known root-scope operational limit, not a passing whole-root smoke.

Measured final release observations follow; times are individual local invocations,
not latency guarantees or an optimization benchmark. CLI is followed by HTTP/MCP
against the same cache. MCP size includes its duplicate text representation and is
measured from reserialized result data, not the full wire frame.

| Repository / src operation | CLI s | HTTP s | MCP s | HTTP bytes | MCP result bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| context-engine / search | 0.206 | 0.084 | 0.075 | 4266 | 9775 |
| context-engine / symbol | 0.078 | 0.076 | 0.072 | 1154 | 2689 |
| context-engine / neighborhood | 0.025 | 0.075 | 0.081 | 13782 | 31416 |
| context-engine / impact | 1.140 | 1.105 | 1.162 | 1872 | 4150 |
| context-engine / context | 0.085 | 0.075 | 0.072 | 16848 | 35206 |
| context-integrity / search | 0.129 | 0.074 | 0.075 | 1756 | 4007 |
| context-integrity / symbol | 0.081 | 0.087 | 0.070 | 1110 | 2577 |
| context-integrity / neighborhood | 0.024 | 0.074 | 0.073 | 95020 | 214994 |
| context-integrity / impact | 0.788 | 0.783 | 0.818 | 1824 | 4046 |
| context-integrity / context | 0.080 | 0.073 | 0.076 | 19336 | 40741 |

## Limitations and independent review priorities

The service is local/read-only, single-root and deliberately serialized. It does
not expose all CLI operations, legacy MCP, HTTP MCP, OAuth, remote hosting, a
watcher service, cross-repository queries, snippets/tokens, policy or authority.
HTTP responses may time out while blocking work continues. Stdio cancellation
can consume an outstanding-response slot; reconnect is the recovery. No Linux,
Windows, IPv6 socket, cloud/provider or external deployment execution is claimed.

Highest-priority independent review: modern-only SDK direct-entry metadata/error
semantics and third-party interoperability; HTTP authority/auth/header handling;
connection/stdio resource ownership and shutdown under stalled I/O; concurrent
cache generations across other processes; intentional full-file disclosure versus
scope/ignore/symlink escape; and CLI/adapter evidence consistency on incomplete,
ambiguous, lossy-path or dirty/committed inputs. Phase 9 is not self-accepted.
