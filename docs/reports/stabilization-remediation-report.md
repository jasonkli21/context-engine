# Stabilization remediation report

Date: 2026-10-26. Repository: Context Engine only.

Baseline: `231c0256500fad1599259f9c97284ce74285946d` (`main`), exactly the
repo-wide and cross-project audit's Engine SHA. There were no intervening commits
or tracked edits. The pre-existing untracked Phase 10 and Phase 11 plans were read
and preserved outside the commits. The maintainer authorized handoff 1A using both
complete audits; their review-only instructions describe the original audits,
not this implementation. No subagents, remote writes, services, dependencies,
Context Integrity edits or Phase 12 implementation were added.

Implementation commit: `a43649a2205b83c77ab42159199c18811d05610d`, on local
branch `codex/stabilization-remediation`. A following documentation-only commit
records this report. No push, PR, merge or release was performed. This report is
local implementation evidence, not independent acceptance.

## Finding ledger and before/after evidence

| ID / owner / risk | Current-code verification and original reproduction | Regression / final disposition |
| --- | --- | --- |
| CE-STAB-01 / Engine scanner and source / P1 | Both audited path reopens existed. Audit was code-path-only, without Rust execution. Deterministic metadata/open injections now exercise counted files, root/nested ignore controls, source-file and parent-directory swaps in disposable fixtures. | **Unix fixed and verified; non-Unix P1 remains open.** Swapped count is unknown/warned; unsafe controls fail closed; checked descriptor acquisition refuses the outside sentinel. See residual risk below. |
| CE-STAB-02 / Engine graph and committed history / P2 | Exact audited logic remained. New actual CLI regressions failed before the fix: graph selected qualified mode outside `a.py`; committed history returned 0 matches. | **Fixed and verified.** File scoping precedes qualified/local precedence. Scoped `run` resolves `A.run`, unscoped `run` retains top-level precedence; same-file ambiguity remains explicit. |
| CE-STAB-03 / Engine Git adapters / P2 | Readers stopped at cap while owner waited for exit. Audit was code-path-only. Disabling the new early signal reproduces a 15.01-second fixture wait and fails the elapsed-time assertion. | **Fixed and verified.** Both adapters use one capture owner; stdout/stderr cap detection kills/reaps the child promptly, joins both readers, and preserves typed limit errors. |
| CE-STAB-04 / Engine MCP framing/admission / P2 | Audited duplicate/capacity branch returned transport EOF. Audit was code-path-only. Restoring EOF-at-cap fails the new pipe-backed regression. | **Fixed and verified.** Capacity rejection returns -32600 with the rejected ID; duplicate rejection returns -32600 with null ID, without releasing/correlating the admitted original. Later valid requests survive. |
| CE-BND-03 / Engine evidence conformance / P2 | The audit's Python mutation was reproduced here: `not-a-blake3-digest` was accepted. No malformed real producer digest was observed. | **Fixed and verified for shape.** Schema and independent stdlib decoder reject wrong-length/non-lowerhex values. New negative golden plus live-producer mutations pass. Independent cryptographic derivation remains explicitly outside the decoder's guarantee. |
| CI-BND-01 / Context Integrity / P1 | Consumer qualified-name rejection is independently reproduced in the supplied boundary audit and historically recorded in the Phase 11 report. | **Consumer-owned, handoff 1B.** No producer name mangling or consumer edits. |
| CI-BND-02 / Context Integrity / P2 | Consumer drops `sensitive_files_omitted`, per the supplied audit's executable Python reproduction. | **Consumer-owned, handoff 1B.** Engine's exclusion and emitted count are retained. |

No additional independently confirmed high-risk defect is claimed. The non-Unix
fallback below is an explicit residual of CE-STAB-01, previously noted as a
cross-platform audit suspicion; no Windows exploit execution is claimed.

### CE-STAB-01 containment and compatibility

`scan.rs` no longer delegates control-file opens to WalkBuilder. Both walks have
ignore ingestion, parent ingestion, global rules and Git exclude ingestion off.
Controls are read once, through `source::open_checked`, under the existing 1 MiB
aggregate budget; an extra byte detects growth. GitignoreBuilder parses lines
from those checked bytes, including the crate's BOM/CRLF handling. Matchers are
frozen before the actual inventory walk. Closest directory wins within each
control class; `.ignore` precedes `.gitignore`, and nested `.git` boundaries stop
inherited Git rules. Root/parent rules, anchored patterns, directory pruning,
negations and ordinary scans have executable coverage.

On Unix, source opens now also anchor the canonical absolute root from `/`,
opening each root and relative directory component with NOFOLLOW and a held
file descriptor; the final regular file is opened NOFOLLOW/NONBLOCK. A descriptor
for a post-metadata symlink sentinel is never acquired. The source regression
places the swap after all metadata checks and would return a regular outside
handle if NOFOLLOW were absent. Scanner regressions use a same-size outside
sentinel so size validation cannot mask an unsafe open. Restoring raw opens
separately for counts and controls makes that scanner regression fail. Only
isolated temporary directories/files are mutated; no analyzed code is executed.

Raced/unreadable counted files retain unknown counts plus warnings. Symlinked,
nonregular, malformed, invalid-UTF-8, unsafe or growing ignore controls fail the
scan, with empty CLI stdout. This deliberately strengthens prior partial-ignore
error handling. Inaccessible child directories retain a warning and their
children stay excluded even if permissions later change. Missing/newly created
controls are part of the next best-effort scan, not an atomic snapshot promise.
Matching uses the existing crate grammar; no new parser/framework/crate was added.
Only nonempty matchers and Git boundaries are retained, with bounded discovery
and control-byte work. Descriptor acquisition adds work proportional to path depth;
no general performance or ranking-quality acceptance is inferred from smoke timing.

**Open P1 / exact non-Unix residual:** `source.rs` still uses `File::open(path)`
after metadata inspection under `cfg(not(unix))`. A file or ancestor swapped to a
link between that check and the open can redirect a read. Routing scanner reads
through that function does not establish containment on that branch. This host
cannot execute Windows reparse-point/handle-relative tests; no new unsafe FFI or
untested platform rewrite is introduced. Next work is a Windows handle-relative,
no-reparse open (or an explicit fail-closed support decision), followed by the
same after-metadata sentinel tests for final files and parent components on a
Windows runner. This risk is **not accepted by this report**; Phase 12 stays gated.

Scanner semantics changed, so `INDEX_COMPAT_VERSION` is **2**. SQLite
`SCHEMA_VERSION` remains **3**, and `freshness-v2` is unchanged. The existing
invalidation test now seeds actual baseline compatibility **1**, proves a full
rebuild, and checks stored compatibility **2**. No store-layout migration is needed.
Evidence wire version stays **1**; its schema/decoder accept analysis compatibility
1 and 2 so historical goldens remain valid while current exports report 2.

### Scoped lookup, Git and MCP compatibility

Graph lookup and committed HEAD history filter by file before choosing exact
qualified-name or local-name matches. Unscoped ordering, root limits, JSON shapes,
human output and ambiguity handling retain their established contracts. Python
method/top-level collisions are checked through all three graph CLI operations
and history, in JSON and human forms. Existing Rust fixtures remain in the suites.

Git capture shares bounded stdout/stderr readers and an overflow/error signal.
The owning loop checks that signal while polling the child; every early failure
kills/reaps before joining both readers. Successful bytes/status remain intact.
Metadata cap failures remain `InvalidData` with typed `CaptureError::Limit` as the
source; revision calls return their existing `revision::Error::Limit(operation)`.
Revision stderr is now captured under a 1 MiB cap rather than discarded, never
exposed raw by evidence output. Timeout remains 15 seconds for genuinely stalled
children. Tests use 1 KiB caps and 2 MiB output from the trusted test executable,
separately on each stream, assert <3 seconds and (Unix) no surviving fixture PID.
The real Git tree fixture checks reduced-cap failure and normal-cap success.
These guarantees cover the owned child/readers, not arbitrary descendant process
groups or hard cancellation of filesystem stalls.

MCP admission errors are written directly and bounded by existing framing/write
limits, without insertion into the outstanding set. Duplicate errors use null ID
so they cannot remove the original pending slot via `send`. Cancellation retains
existing matching-slot release and completion-owned worker permit semantics. The
transport regression withholds all responses while piping 19 requests (16 admitted,
a duplicate and two excess), then checks errors, cancellation, original response
flush, pending counts and later valid admission. Existing real SDK/stdio and
HTTP/MCP parity/admission/cancellation tests are retained. No vocabulary, successful
tool payload or MCP version change occurred.

### Evidence conformance guarantee

`task_digest` now has JSON Schema type string, min/max length 64 and lowercase
hex pattern, plus stdlib semantic full-match validation. A negative golden and
mutations for empty, short, long, uppercase, nonhex and trailing-newline values
are rejected. Real Rust candidate exports still validate. A test explicitly
accepts a different well-formed digest to demonstrate the limitation: the decoder
has neither the normalized task phrase nor a stdlib BLAKE3 implementation and
**does not independently prove cryptographic derivation**. Contract documentation
states this separately from producer identity tests. No wire-version churn,
production Integrity evidence-v1 consumer or governance behavior is introduced.

## Verification

Host: macOS/Darwin 27.0.0, aarch64-apple-darwin; rustc 1.99.1
(`58cf8f9c3`, 2026-10-13), Cargo 1.99.1 (`5f94df478`), Apple Git 2.39.5; Python 3.9.6.
All fixtures are local/disposable. No network, hosted provider or analyzed-source
execution was used.

| Command / check | Exit / result | Duration and scope |
| --- | --- | --- |
| `./scripts/verify.sh` (host execution) | **0, PASS** | 157.14s including compilation/lock wait; context check, format, Clippy `-D warnings`, **124 unit + 107 integration tests** |
| `cargo check --all-targets --all-features` | **0, PASS** | 16.46s |
| `cargo build --locked --release` | **0, PASS** | 97.77s |
| Independent Python loop calling `validate.validate` over every checked-in `valid-*.json` / `invalid-*.json` | **0, PASS** | 0.135s; **4 valid accepted / 21 invalid rejected** |
| Release CLI/Git/evidence smoke matrix below | **0, PASS** (one intended CLI exit 1) | 5.85s overall, **14 invocations** |
| `cargo fmt --all -- --check`, `./scripts/check-context.sh`, `git diff --check` | **0, PASS** | Rechecked after final documentation edits |
| `git status --short` | **0** | Final status preserves only the two original untracked plans |

The canonical integration breakdown is: adapters 8 (5.69s), change 13 (9.52s),
CLI 18 (1.43s), context 3 (13.66s), evidence 11 (17.29s), graph 10 (16.03s),
history 11 (23.08s), intelligence 7 (1.06s), persistence 5 (5.14s), symbols 7
(4.20s), workspace 14 (11.02s). Unit execution was 11.77s. This includes actual
HTTP/MCP, dirty/rename/delete, nested scopes and producer/independent-decoder paths.

The first sandboxed `./scripts/verify.sh` exited **101** in 47.97s: context,
format, Clippy and 124 unit tests passed, but six HTTP fixture startups could not
bind loopback. A direct socket probe confirms sandbox `PermissionError`/errno 1.
A separate targeted history run also hit EPERM while creating an existing native
filename fixture, before `ctx` invocation. The native-filename test passed outside
the sandbox (1/1, 0.11s), followed by the full host verifier above. These initial
restricted runs are not reported as passes; the full host run used the same
production source and tests, without disabling assertions.

Release smokes used the absolute `target/release/ctx` executable, isolated cache,
two disposable repositories and Git commits, with no fixture builds. Commands
below use `$R` for the temporary provider root, `$M` for an explicit v1 workspace
manifest and `$REV` for the exact immutable fixture range
`9f8f1f49b045b8f90aef9f6468f9a91037f8a5c5..c002ccabc0fe299f735313a4bd07abb8b0230570`.
All JSON commands used `--format json`; single-repository evidence additionally
used `--repository-id fixture`:

| Release command | Exit | Seconds |
| --- | --- | --- |
| `ctx --version` | 0 (`ctx 0.1.0`) | 0.025 |
| `ctx scan $R` | 0 | 0.227 |
| `ctx related run $R --file a.py` | 0 | 0.524 |
| `ctx history run $R --file a.py` | 0 (`A.run`) | 0.694 |
| `ctx graph $R` | 0 | 0.202 |
| `ctx context $R --task A.run --budget 4096` | 0 | 0.180 |
| `ctx evidence structure $R` | 0, independent decoder accepted | 0.223 |
| `ctx evidence candidates --task A.run $R` | 0, accepted | 0.161 |
| `ctx evidence changes $REV $R` | 0, accepted; rename and delete asserted | 0.394 |
| `ctx evidence candidates --task target --change $REV $R` | 0, accepted | 1.110 |
| `ctx evidence workspace --manifest $M` | 0, accepted; observed Cargo + declared API | 0.193 |
| `ctx evidence workspace --manifest $M --repo provider --change $REV` | 0, accepted | 0.390 |
| `ctx evidence structure $R` after dirty malformed Python edit | 0, accepted; dirty=true | 0.716 |
| `ctx context $R --task target --budget 4096 --change $REV` after that edit | **1, expected rejection**, empty stdout | 0.254 |

The initial smoke harness assumed raw Git status letters; its assertion was
corrected to the existing normalized `renamed`/`deleted` contract before the
successful matrix. No production change was made for that harness mistake.


Deliberate regression mutations were restored immediately using `try/finally`
before final verification. Each exited 101 with a failed assertion (no compilation
error): raw count open 4.22s; raw ignore open 12.39s; Git early signal disabled
31.08s including build, 15.01s actual test; EOF-at-cap 16.97s including build.
Commands were `cargo test --bin ctx TEST -- --exact --nocapture`, selecting
`scan::tests::metadata_to_open_swaps_cannot_read_outside_controls_or_counted_files`,
`git::tests::output_limits_stop_and_reap_both_streams_promptly` and
`mcp::transport_tests::duplicate_and_capacity_errors_preserve_admitted_work_and_transport`.
These are fault-injection evidence, not a claim to have run an unmodified audited
binary's race under an operating-system scheduler. The original scoped graph and
history test failures and Python malformed-digest acceptance were directly observed.

Windows and Linux Rust execution, third-party MCP clients, live production
Integrity binary round-trip, cloud CI and independent post-fix acceptance were
**not run**. The real Rust producer → separate stdlib Python evidence decoder
was run; that is distinct from production Context Integrity integration.

## Backlog and acceptance gates

- **P1:** complete non-Unix read containment, or obtain an explicit fail-closed
  platform/support disposition. Windows final-component and ancestor reparse
  races must run on Windows; a macOS pass is not a substitute.
- **P2 platform coverage:** current CI is Ubuntu-only. Add Windows/macOS matrix
  coverage for native path encoding, no-follow/reparse behavior, Git paths and
  evidence path normalization; this session's macOS results do not activate CI.
- **P2 ranking evaluation:** fixed realistic Rust/Python repos with expected-file
  sets and negative controls, import-boundary ambiguity, rename/delete impact,
  budget stability and p50/p95 CLI latency. The six synthetic ranker scenarios
  remain narrow; this remediation does not become an evaluation project.
- **P3/future:** privacy-preserving stable evidence error codes; public Rust
  library only for an actual consumer. Existing first-parent, non-atomic,
  heuristic syntax, current/committed workspace timing and filesystem-stall limits
  remain. No policy/authority/drift logic belongs in Engine.
- **Consumer handoff:** CI-BND-01/02 belong to Integrity remediation 1B. Production
  Integrity still consumes legacy context/graph. Evidence v1 is producer-only;
  a later opt-in consumer requires separately bound identities and acceptance.

Historical dispositions are unchanged: Phase 0 accepted by maintainer with
re-review waived; Phases 1–2 final independent disposition pending; Phases 3–9
fresh independent re-review pending; Phase 10 fresh re-review/maintainer acceptance
pending; Phase 11's narrow semantic re-review does not confer formal independent
or maintainer acceptance. Original reports remain intact.

**Independent re-review requested:** reconstruct the production paths and verify
scanner containment/ignore precedence and compatibility invalidation, scoped
lookup ambiguity, Git child/thread ownership, MCP rejection/cancellation behavior,
evidence schema/decoder guarantees and preserved cross-project boundaries. Review
actual fixes and residual P1 independently of green self-tests. Maintainer
acceptance and Phase 12 authorization remain separate decisions.
