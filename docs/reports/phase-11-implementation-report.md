# Phase 11 implementation report

Date: 2026-10-24 (maintainer local date). Phase 11 implemented locally;
fresh independent review and maintainer acceptance pending.

## Baseline, authorization and ownership

Starting HEAD: `05b9b10e6e1cd6cab7c5d2391f7827b05a9a211e`, exactly the planning
baseline. Initial worktree contained only two untracked user files:
`docs/plans/phase-10-implementation-plan.md` and
`docs/plans/phase-11-implementation-plan.md`. Both were read, preserved unchanged
and excluded from the implementation commit. The maintainer explicitly requested
implementation in this session, one commit and no subagents. This satisfies the
Phase 11 advancement gate despite pending Phase 10 re-review, and does not accept
Phase 10 or any earlier phase. No subagents or separate chats were used. The
separate Context Integrity repository was inspected read-only; its code was not
edited. No dependency, global toolchain, extraction or cache-format change was made.

Final implementation HEAD is the single commit containing this report, resolvable
without a self-referential hash using:

```sh
git log -1 --format=%H -- docs/reports/phase-11-implementation-report.md
```

The final handoff supplies that exact SHA. There is no intervening implementation
commit or second status/handoff ruleset.

## Plan reconciliation and production architecture

Repository truth matched the companion plan. Phase 10 output is v2, relation
identity includes contract kind, explicit root Cargo packages may include
`[workspace]`, and direct triggers are capped at 64. All these semantics are
preserved. Existing Context Integrity contracts consume graph/context schema v1,
not the new evidence envelope. Only the application graph accessor and additional
body-free/committed use cases were needed; no scanner or parser changes were needed.
There was no material design deviation requiring a new product choice.

Production paths:

- Clap `Command::Evidence` in `src/cli.rs` → `main.rs` → `evidence::command`.
  Errors are replaced with a fixed, root-free stderr diagnostic. Successful
  serialization completes before the shared stdout writer is entered.
- Structure → `Application::graph_with_probe(true)` → reconciliation → existing
  `graph::from_analysis` → explicit evidence File/Symbol/Relation DTO projection.
- Changes → `Application::changes` → `change::analyze(..., None)` → immutable
  Git snapshots and structural comparisons → side-specific projections/deltas.
  Checkout source bodies do not participate in this feed.
- Candidates → `Application::candidates` → Task::parse → existing graph → optional
  compatible_change → context::rank → snapshot recheck → metadata projection →
  another compatibility recheck after DTO construction. Neither context::select
  nor its full-file selection loader is called. Ordinary parser/freshness source
  reads remain necessary; body-free is an output/ranking boundary.
- Workspace → existing application workspace_inspect or explicitly requested
  workspace_impact → existing explicit `output_workspace::Document` → evidence
  envelope. JsonSchema derives added to existing DTO dependencies do not alter
  their Serialize behavior or domain logic.

`src/evidence.rs` owns v1 DTOs, reference derivation/validation, projection and
bounded serialization. It never reparses legacy CLI JSON. HTTP/MCP request types
and route/tool vocabularies were not edited. `tests/evidence.rs` invokes the actual
binary and a separate Python process, with golden fixtures and a deliberately
independent schema/semantic validator. The validator uses the Python standard
library; it does not deserialize Rust DTOs or import the engine or Python provider.
The checked-in schema is frozen against serialize-mode schemars reflection plus
explicit v1 version/enum constraints. Existing schemars dependency is reused.

## Contract and uncertainty

See the normative [evidence v1 contract](../contracts/context-integrity-evidence-v1.md)
and adjacent machine-checkable JSON Schema for exact required/nullable fields,
enums, ordering, IDs, offsets, multiplicity and compatibility rules.

Caller/manifest logical IDs replace physical-root identities. References use
versioned domain-separated, length-prefixed BLAKE3 inputs. Worktree scope is an
opaque invocation namespace; it is explicitly not a content digest, cache
generation, timestamp, atomic snapshot or continuity claim. Committed scopes use
actual commit IDs plus the Git scan prefix. Every exported structural endpoint
resolves; duplicate IDs fail. Repeated relations retain occurrence ordinals.
Workspace relations keep their distinct, unchanged Phase 10 identity contract.

Source points retain half-open byte ranges and zero-based row/byte columns.
Resolution/derivation distinguish lexical containment, imports, syntactic calls,
exact same-file qualified matches and unresolved names. Parser version is null,
not invented; producer version and analysis compatibility are actual constants.
Coverage always declines global completeness. Observed analysis/relation statuses,
syntax errors, unsupported candidates, withheld diagnostic/warning counts and
native/path omissions remain explicit. Nonrepresentable structural paths cascade
omissions without dangling endpoints. Essential Git/workspace paths fail closed.
Changes preserve rename/delete/mode/oid sides, ambiguity, incomplete comparison
and bounded occurrence examples. No raw task, source body, root/cache path or
lower-level stderr appears in the export. Names and task digests can still reveal
sensitive metadata; this is not secret detection or policy evaluation.

Candidate limit is metadata cardinality, not a selected content-byte budget.
Ranker score/reason/tie order is reused, including sensitive-path and role
exclusions, centrality fallback, term/root/traversal/limit omissions and optional
clean matching HEAD compatibility. Workspace inspection and explicit provider
impact both ship; no narrowing/deferment approval was needed. They retain
observed-vs-declared evidence, mixed-time HEAD/dirty/unknown, typed change-trigger
omissions, direct trigger IDs/cap/excess counts and representative/alternate/depth
witness limits.

The final fully escaped response, including LF, is bounded to 8,388,608 bytes.
The shared bounded serializer rejects before stdout, tested at exact/one-over and
with escaping. A large real-binary structure fixture also verifies overflow with
empty stdout. Lower-layer analysis bounds remain active; the exporter builds
bounded DTO vectors and ordered lookup maps, without quadratic symbol-reference
searches. Like all stdout programs, it cannot retract bytes after an OS write
failure once writing has begun.

## Executed verification

Environment: macOS / Darwin arm64, rustc `1.99.1 (58cf8f9c3 2026-10-13)`;
existing offline dependency cache. Test validator runs with system Python 3.9;
existing consumer smokes use the sibling project's existing newer `.venv/bin/python`.
No packages were installed. Commands below were actually run; no cloud/provider,
Linux/Windows or independent-review pass is implied.

| Literal command | Result |
| --- | --- |
| `cargo test --offline --test evidence -- --nocapture` | Passed all 10 integration tests after fixture corrections |
| `UPDATE_EVIDENCE_SCHEMA=1 cargo test --offline --bin ctx evidence::tests::schema_is_frozen` | Generated/froze the committed schema; passed |
| `cargo test --offline --bin ctx evidence::tests -- --nocapture` | Initial four exporter tests passed; the final verifier also covers the fifth application-level test |
| `./scripts/verify.sh` | Final pass: context checks, fmt, Clippy with warnings denied, 115 unit and 103 integration tests, including all 8 HTTP/MCP adapter tests |
| `cargo check --all-targets --offline` | Passed on final code |
| `cargo build --release --offline` | Passed on final code |
| `PYTHONDONTWRITEBYTECODE=1 /Users/jasonkli/projects/context-platform/context-integrity/.venv/bin/python /tmp/context-engine-phase11-smoke.py` | Real release evidence feeds independently decoded; legacy consumer results/limitations below |
| `git diff --check` | Passed |

The first canonical verifier invocation was sandboxed: all 114 then-existing unit
tests passed, but six HTTP tests could not launch loopback servers. An approved
local-loopback rerun passed, and the final rerun after the fifth application test
passed with the counts above. Logs were captured in
`/tmp/context-engine-phase11-final-verify.log`. Early new-test failures were fixture
issues: goldens had not yet been generated, APFS rejected a native filename,
manifest fingerprints correctly changed after reorder, and a single-file stress
fixture hit the per-file analyzer cap before the output cap. Goldens were generated,
workspace assertions now compare stable IDs rather than mutable manifest hashes,
and the output stress case uses ten eligible files. These corrected tests passed.

New automated coverage includes Rust/Python/non-Git/empty inventory, resolved and
unresolved relations, duplicate call occurrences, invalid UTF-8 bodies, syntax
recovery, unsupported/generated roles, cold/warm coverage, candidate score/reason
parity with context, sensitive filenames, fallback, cardinality omissions, clean/
dirty/mismatched change gates, actual commit hashes, rename/delete/executable modes,
ambiguous/incomplete comparisons, nested Git scope, unsafe/control/native paths,
workspace order/relocation and observed/declared impact, malformed versions,
duplicate JSON keys/IDs, dangling references, wrong counts, invalid nullability,
escaping and output overflow. Existing suites exercise the lower-level ranker
root/traversal/term/fallback caps, Git bounds, symlink containment, cache races and
workspace direct-trigger/witness limits without changing those implementations.

APFS rejected the invalid-byte path creation; that one native filesystem subcase
is explicitly reported rather than counted as executed. Control/backslash-path
production tests still ran. An injected native-path graph unit test exercised
actual exporter omission accounting and dependent relation removal without
requiring filesystem support. No Windows/native-wide execution is claimed.

Release smokes used real `context-engine/src` and sibling `context-integrity/src`
scopes, external temporary caches, and the independent validator's full schema/
reference/count checks. Structure/candidate outputs were respectively
4,204,402/40,809 and 4,541,242/29,757 bytes; the actual engine `HEAD~1..HEAD`
committed changes feed was 474,591 bytes. All five decoded successfully and
retained explicit non-atomic/incomplete markers. Source scopes avoid the existing
full-root scanner ceiling caused by ignored build artifacts. Disposable workspace
production cases are automated in the ten-test evidence suite.

## Legacy consumer interoperability and remaining gaps

Existing graph/context/diff/workspace shapes are unchanged; all prior tests pass.
The separate Python provider's `relations` path accepted real release output.
A broad `index repository` context request was rejected by its existing
`CandidateRelevance` validation: matched entity qualified names must match a
lowercase identifier token there, which rejects Rust qualified/capitalized names
(and Python qualified names). This rule was inspected directly; the schema-v1
producer still returns qualified syntax names as before. No consumer rule or
legacy output was quietly changed to make the smoke green. The compatible no-match task `zzzzzzqqqqzzzz` exercised centrality fallback;
both legacy graph and context provider subprocess paths accepted the real Rust
and Python scopes (final smoke command exit 0). The new evidence candidate feed
also successfully handled the broad task; the rejection above belongs to the
existing legacy consumer normalization layer.

System Python 3.9 cannot import that project's StrEnum-based models; its existing
virtual environment was used for real provider checks. No Python migration is
claimed. A future independently authorized consumer change must opt into evidence
v1 and decide normalization/policy semantics; qualified-name acceptance is a
separate existing consumer issue.

Current references intentionally vary per invocation; deterministic current
comparison requires normalizing scope/reference values. Committed output is
byte-stable for repeated immutable inputs. Cache generation authenticity and
atomic source snapshots remain unsupported. Native/unsafe essential changes
reject the entire feed instead of returning a lossy partial revision result.
Coverage categories are sparse and globally incomplete. Existing filesystem/Git
races and per-phase unsupported languages/Cargo shapes remain limits. Not every
lower-level boundary fixture is duplicated in the new exporter suite; unchanged
lower-layer tests remain part of the canonical verifier.

## Independent review focus and disposition

Review actual production composition, schema/independent decoder agreement,
opaque observation scope vs revision identity, reference integrity/multiplicity,
path omission cascades, privacy and bounded allocation/output, candidate ranker
parity and clean-HEAD rechecks, preserved Phase 10 typed trigger/witness semantics,
and legacy compatibility. Review must reconstruct the architecture and behavior,
not infer correctness from this report or passing tests.

Phase 11 requires fresh independent review and maintainer acceptance. Phase 10
fresh re-review of CE10-01–05 remains pending. Phase 9/8/7 and Phase 6 fresh reviews,
Phase 5/4/3 re-reviews and Phase 1/2 outstanding independent dispositions remain
unchanged. Implementation authorization and local verification accept none of them.
