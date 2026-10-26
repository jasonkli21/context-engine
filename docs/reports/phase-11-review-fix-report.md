# Phase 11 independent review fixes

**Date:** 2026-10-24
**Status:** Review findings addressed locally; fresh independent re-review and
maintainer acceptance remain pending.

## Findings addressed

- **CE11-01:** `safe_path` validates the original native spelling before
  formatting. Windows slash and backslash separators are checked as components
  and emitted in POSIX form; Unix literal backslash names remain unrepresentable.
  Added a Windows-gated executable regression covering nested structure,
  candidates and committed changes. This macOS host did not execute that test,
  so Windows behavior still needs platform-run confirmation.
- **CE11-02:** The independent stdlib Python consumer now checks relation source
  types, endpoint/path consistency, lexical containment, observation scope
  format, candidate rank bounds, revision-side symbol references, and workspace
  trigger ownership and witness-chain continuity. Added six invalid golden
  fixtures for wrong source type, evidence path mismatch, empty scope, invalid
  containment endpoint, impossible rank and wrong change-side reference. The
  workspace integration test also mutates a real impact response to verify
  rejection of a wrong trigger owner. The validator does not recompute BLAKE3
  identities; this limit is explicit in the contract.
- **CE11-03:** README and `docs/current-state.md` now identify Phase 11 as an
  evidence export and state that production Context Integrity evidence-v1
  consumption is not integrated. The README and current-state snapshot retain
  the pending review and acceptance status.

## Verification

- `./scripts/verify.sh` passed: context checks, formatting, Clippy, 115 unit
  tests and 103 integration tests.
- The four valid evidence goldens were accepted and all 13 invalid goldens were
  rejected by the independent validator. Evidence integration coverage also
  passed, including the live workspace trigger-owner mutation.
- `git diff --check` passed.
- Windows-specific runtime verification was not available on this macOS host.
  The Windows-gated test is present for a Windows runner; no Windows pass is
  claimed here.

Phase 11 remains pending fresh independent review and maintainer acceptance.
Phase 10 CE10 re-review and prior phase dispositions are unchanged.

## 2026-10-26 supplemental handoff findings

The subsequent independent re-verification handoff identified false acceptance
in the Python consumer for wrong-type and wrong-file change references, plus
inexpensive change-observation, delta-shape and ranker invariants. The fixes are
limited to the independent consumer and its conformance tests; this review did
not demonstrate a malformed Rust producer response.

- `structure()` now retains typed symbol maps with owning paths as well as the
  general entity set. `changes` requires each reference to be a symbol from the
  correct revision and binds its path to the matching Git side. For nested
  scopes, the exported symbol path is joined with the checked `scope_prefix`;
  absent sides remain valid when a reference is absent, including out-of-scope
  rename sides.
- `added`, `removed` and `modified` deltas now require their normative reference
  shape. Committed changes require null `git_head` and `git_dirty`. Candidates
  require the exact `file-ranker-v1` sort order while preserving legal rank gaps
  from path omissions.
- Workspace inspection requires null base/head revisions. Impact observations
  must match the embedded base/head; direct triggers must belong to the selected
  provider and candidate consumer, and each emitted candidate must have a
  nonempty witness chain that starts at that provider, remains continuous and
  ends at the candidate at the reported distance.
- Added seven invalid goldens for the reproduced wrong file/repository references,
  unrelated side path, impossible added shape, dirty committed observation,
  same-revision wrong-file symbol and arbitrary candidate sort order. The earlier
  wrong-revision golden remains in the suite. Tests also mutate a real emitted
  rename response and a four-repository, multi-hop workspace impact response to
  exercise wrong trigger ownership, chain discontinuity, wrong final consumer,
  empty witness chain, direct-trigger/provider mismatch and observation revision
  mismatch. An independent working-diff review found the empty-chain case; the
  consumer invariant and regression were added before final verification.
- Positive producer cases cover rename sides, modified/added/removed symbol
  deltas, nested scope, incomplete and ambiguous comparisons, workspace inspection
  without impact, and multi-hop impact. No BLAKE3 identity recomputation was
  added; its stated consumer limitation remains.

## Verification for the supplemental fixes

Environment: macOS / Darwin arm64 with the repository's existing Rust toolchain
and offline dependencies. No packages or toolchains were installed.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --offline --test evidence -- --nocapture` | Passed: 10 integration tests |
| `./scripts/verify.sh` | Passed: context checks, formatting, Clippy, 115 unit tests and 103 integration tests |
| Evidence conformance goldens | 4/4 valid accepted; 20/20 invalid rejected |
| `git diff --check` | Passed |

The first verifier attempt ran under the default sandbox and its six existing
HTTP adapter cases could not bind loopback sockets. The canonical verifier was
rerun with local-loopback access and passed. The Windows-gated native-path test
was not run on this macOS host. An independent post-fix review found no remaining
findings. No Windows pass, external production consumer interoperability or
maintainer acceptance is claimed.

Phase 11 remains pending fresh independent review and maintainer acceptance.
Phase 10 CE10 re-review and all earlier dispositions remain unchanged.
