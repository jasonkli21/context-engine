# Post-stabilization engine re-verification follow-up

Date: 2026-10-26. Repository: Context Engine only.

Review input: `post-stabilization-engine-reverification.md`, targeting `aaf7f3f`.
The attached review's findings were treated as evidence to verify against this
checkout. Its embedded copy-ready fixer block was not treated as a separate
authority. No Context Integrity consumer, Phase 12, network operation, or
unrelated plan file was changed.

## Finding dispositions

| Finding | Disposition |
| --- | --- |
| RV-00 — non-Unix path containment | **Fail-closed support decision.** `source::open_checked` no longer performs an ordinary path open on non-Unix targets; it returns `ReadFailed`. Content parsing and line counting are unavailable there, and a scan with a consulted ignore control can fail closed. This prevents a metadata/open swap from acquiring an outside file handle. Native Windows execution was not available, so Windows behavior remains unexecuted. |
| RV-01 — ignored-tree preflight | **Fixed.** Control discovery now applies loaded ancestor rules as it walks and prunes ignored directories before descending. Nested controls inside an ignored directory are not consulted; an invalid nested control there does not abort the scan. Entry/path limits remain enforced over the traversed tree, and included trees still fail at their configured bounds. |
| RV-02 — cancellation and reused MCP IDs | **Fixed.** The stdio transport maps each client wire ID to a unique private SDK ID, rewrites cancellations to that ID, and maps replies back. If cancellation arrives before sending, the wire slot can be reused while the SDK's distinct ID isolates any late completion. Once response sending starts, the wire slot stays reserved through the write. Unmapped stale replies are suppressed. A real `rmcp` service test covers late completion, numeric/string ID distinction, unknown cancellation, duplicate admission, response ownership, and recovery. |
| RV-03 — descendant-held Git pipes | **Documented limitation.** The 15-second deadline applies to the direct Git child. A descendant inheriting a pipe can delay reader joins beyond that deadline. README, architecture and current-state documentation now state this boundary. A Unix helper test exercises a descendant-held pipe after the direct child exits. |
| RV-04 — current version wording | **Fixed.** Present-tense status identifies SQLite schema 3, analysis compatibility 2 and `freshness-v2`; Phase 8, 9 and 11 version statements are explicitly scoped to their delivery baselines. |

## Verification

Host: macOS, aarch64. No Windows or Linux run is claimed.

| Command | Result |
| --- | --- |
| Focused scanner ignored-tree regression | Passed |
| `cargo test --bin ctx mcp::transport_tests -- --nocapture` | Passed; 2 tests, including the real rmcp cancellation/reuse sequence |
| Focused descendant-held pipe test | Passed; direct helper exited while its inherited pipe remained open for about 0.25 seconds |
| Focused Unix post-metadata source/ancestor swap test | Passed |
| `./scripts/verify.sh` | Passed: context checks, formatting, Clippy with `-D warnings`, 126 unit tests and 107 integration tests |
| `cargo check --all-targets --all-features` | Passed |
| `cargo build --locked --release` | Passed |
| `git diff --check` | Passed |

The first sandboxed verifier run could not start six HTTP fixtures because the
child processes did not emit the expected loopback listening line. The first
host-access retry had one timeout in the MCP adapter integration test while the
suite ran concurrently; the test passed alone and the next full host verifier
passed. This report records the final passing verifier, not the earlier attempt as
a pass.

## Remaining limits and re-review

Non-Unix content reads intentionally remain unsupported until a handle-relative,
no-reparse open is implemented. The fail-closed branch and its target-specific
test were not executed on Windows. The Git direct-child deadline does not bound
descendant-held pipes. Neither choice changes SQLite schema v3, analysis
compatibility v2, `freshness-v2`, CLI/adapter wire schemas, or the descriptive
Engine / Context Integrity boundary.

Independent re-review and maintainer acceptance remain pending. Phase 12 remains
unauthorized. The next step is fresh independent review of the changed checkout
and the non-Unix fail-closed decision.
