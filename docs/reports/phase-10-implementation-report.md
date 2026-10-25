# Phase 10 implementation report

Date: 2026-10-21. Local implementation evidence; **independent review and maintainer
acceptance remain pending**. Phase 9 and earlier pending dispositions are unchanged.

The workspace JSON v1 and first-witness behavior recorded below describe this
implementation baseline. The CE10-01–CE10-05 follow-up changes the current workspace
contract to JSON v2; see the [Phase 10 review-fix report](phase-10-review-fix-report.md).

## Baseline and commit ownership

Baseline HEAD was `39c87cfa4dd4e51b7ef8780e7ed96072fe5aa59d`, matching the supplied
prompt/plan. The only pre-existing worktree item was the untracked
`docs/plans/phase-10-implementation-plan.md`. It was read, preserved byte-for-byte,
and deliberately excluded from the implementation commit. No subagents were used.
The user directly authorized implementation and one commit in this session.

Final implementation SHA is the commit containing this report: resolve with
`git log -1 --format=%H -- docs/reports/phase-10-implementation-report.md`.
The final session response records its literal SHA. Review the entire range from
the baseline above to that commit, rather than just the new modules. A literal
self-referencing commit hash cannot be embedded in its own committed report.

Reconnaissance covered current state, Phase 9 plan/implementation/review-fix
reports, the Phase 10 plan, workflow, relevant PRD/architecture and Context Integrity
boundary, existing Clap/application/output, path/source/classification, Git/change,
cache versions and adapter tests. No prior review was treated as acceptance.

## Delivered composition

- `workspace.rs`: strict manifest v1, bounded IDs/entries/reads, canonical native
  registration ownership, preflight endpoint identity, static TOML parsing, typed
  observed/declared relationships and sorted typed coverage. Missing/unreadable
  evidence yields omissions; identity violations and aggregate excess fail closed.
- `workspace_impact.rs`: existing `git::revision::Revisions` committed raw changes,
  both-sided exact contract triggers, coarse classified-source/package triggers,
  package-only transitive breadth-first traversal, dedup and first witness chains.
- `application.rs`: transport-neutral inspect/consumers/impact functions. Existing
  single-root application behavior is intact. No new HTTP/MCP request type/tool.
- `output_workspace.rs`: pure serializable workspace-v1 document and bounded
  JSON/human rendering, without absolute registered roots or source bodies.
- `cli.rs` / `main.rs`: the three actual `workspace` CLI entry points.
- `source.rs`: narrow extraction of the existing regular-file opening primitive
  into `open_checked`; declaration checks drop the handle without reading bodies.
  Existing source loading still uses exactly that checked open plus bounded read.
- Cargo adds maintained `toml` 0.9.12+spec-1.1.0 and its locked dependencies, resolved
  from the existing offline cache. No analyzed Cargo process runs.
- `tests/workspace.rs`: ten real-binary multi-repository integration tests; module
  tests exercise native omission identity, deterministic witness ties/truncation
  and escaped output materialization limits. README, architecture and current
  state describe delivered behavior and pending review.

No scanner, parser, relation-extraction, cache enum or store layout semantics
changed. `INDEX_COMPAT_VERSION=1`, `SCHEMA_VERSION=3`, freshness-v2 are unchanged.
PRD and Context Integrity integration intent are unchanged. No ADR is needed for
an additional invocation-local layer that follows those established boundaries.

## User journeys and evidence contract

Commands exercised through the real binary:

```bash
ctx workspace inspect --manifest workspace.json --format json
ctx workspace consumers --manifest workspace.json --repo provider --path schemas/api.yaml --format json
ctx workspace impact --manifest workspace.json --repo provider --change BASE..HEAD --depth 2 --format json
```

Inspection returns logical registrations (including non-Git), actual available
HEAD/dirty/unknown metadata, supported edges and coverage. Observed Cargo edges
require an exact canonical registered root and matching package name, including
`package` aliases. They identify the consumer Cargo key and both actual manifest
byte fingerprints. Unsupported workspace/target/dev/build/registry/inherited
forms, mismatches, unregistered targets, missing/oversized/invalid TOML do not
become edges. Optional declarations do not prove enabled features/version fit.

Declared API/event mappings identify manifest bytes, contract ID, exact provider
and consumer selectors, and file availability only. Source bodies are neither
interpreted nor output. Equivalent duplicate mappings and same-repo contracts
are rejected. Validating a declaration never proves runtime API/event use.

Impact uses resolved immutable base/head and exact regular paths from both sides
of the existing bounded Git diff, including nested registered scopes. A deleted
or renamed provider endpoint can be recovered from its regular committed diff
side when absent currently; the validation field explicitly distinguishes this
from current endpoint availability. Consumer files must still be readable. The
current provider-file coverage gap remains visible. A non-regular Git side does
not supply an exact contract trigger.

Package candidates require regular source-role changes under the existing
classifier, or Cargo.toml/build.rs. Other changes alone do not trigger them.
Traversal after the first hop follows package links only, not unrelated contracts.
In this implementation baseline, candidates deduplicate by logical repo, with
shortest first witness and stable provider/consumer/kind/path/contract/locator tie
order. Alternate witness and depth omission counts remain visible; depth is 1–3.
Relation IDs hash structural logical keys, while witness array indices are
response-local. No candidate proves breakage. The review-fix report replaces the
single-trigger summary with bounded direct-trigger evidence and includes semantic
contract kind in relation identity.

JSON is separately versioned `workspace_schema_version: 1`. It exposes manifest
version/fingerprint, command/selectors, sorted nodes/edges/coverage, current
HEAD/dirty/unknown, committed base/head, changed sides, trigger paths, candidate
reasons, witness edges and omission/limit fields. Reordering the manifest preserves
logical evidence and ordering, while its actual byte fingerprint correctly changes.
Relocation preserves logical identities; roots never appear as authorization text
or output identifiers. Human rendering escapes controls and conveys uncertainty.

## Trust, resource and failure decisions

Manifest configuration is trusted local launch input. Absolute and relative roots
are supported, relative to the manifest's directory; root symlink aliases may be
canonicalized once. Duplicate/overlapping canonical ownership fails. Exact file
selectors reject absolute/dot/parent/empty components, backslash, colon, symlinks
including in-root aliases, and existing-path normalization to a different identity.
Text v1 selectors cannot name non-UTF-8 files; committed path omissions are counted
using native identity before presentation. Unix opens hold directory descriptors
and reject symlinks; existing non-Unix race limitations remain.

Named limits are 1 MiB JSON; 16 repos; 256 contracts; 1,024 declaration consumers;
1,024 aggregate direct Cargo dependency checks; 256 KiB per Cargo manifest (at most
4 MiB aggregate); 1,024 validated relations; 4,096 coverage gaps; 4,096-byte selectors
with 255-byte components; 4,096 eligible edge examinations; depth three; 8 MiB escaped
output. Declaration file opens are bounded by 256 providers plus 1,024 consumers;
up to 1,024 unavailable-provider mappings can await committed-side validation.
Git retains its existing 10,000 changes / 4 MiB paths / 16 MiB raw diff caps before
scope filtering. Up to 16 deduplicated candidates can repeat bounded trigger paths;
complete output is independently counted before materialization. Metadata probing
has the existing per-Git-operation 15-second limit; there is no whole-command
filesystem deadline. No network, clone, source execution, automatic enrollment,
workspace persistence, watcher or writes into analyzed roots are introduced.

Manifest identity/path/permission errors, Git failure and aggregate excess are
fatal with empty stdout. Missing supported Cargo/file evidence is explicit usable
coverage; traversal limits and withheld witnesses are result fields. Git probe
warnings are reduced to fixed coverage categories, avoiding raw local paths or
stderr leakage in workspace DTOs. Consumer and provider current manifest evidence
can change during an invocation and can differ from committed provider history;
all outputs explicitly disclose the mixed, best-effort, non-atomic time basis.

## Verification and actual smokes

Environment: aarch64 macOS 27.0.1 (26A434); rustc 1.99.1
(58cf8f9c3, 2026-10-13), LLVM 23.1.1. Existing toolchain/dependency caches were
used; no global setup or network change was made.

- Baseline `./scripts/verify.sh`: context, formatting and Clippy passed; all 106
  unit tests passed. Adapter integration suite had two passes and six startup
  failures at tests/adapters.rs:141. A direct `ctx serve` probe confirmed sandbox
  loopback binding failed with `Operation not permitted`. This was an environment
  restriction, not a failing Phase 10 code assertion. Baseline verifier stopped
  there; later baseline integration suites were not run by that invocation.
- An interim canonical verifier with approved loopback access passed all suites.
  This was after implementation edits and is not relabeled as baseline evidence.
- Focused `cargo test --offline --test workspace`: passed the expanded production
  paths; an initial run exposed use of the TOML value parser rather than document
  parser. Corrected to `toml::from_str`; subsequent runs passed.
- Final `./scripts/verify.sh` with loopback access: passed context checks,
  formatting, Clippy with warnings denied, 109 unit tests and 89 integration tests
  (including all eight unchanged HTTP/MCP adapter tests).
- `cargo check --all-targets --offline`: passed.
- `cargo build --release --offline`: passed.
- `git diff --check`: passed.

Production integration exercises cover real Git Rust provider/consumer/downstream
and Python client repositories plus non-Git registration; alias/name checks;
api/event declarations; unsupported/invalid/missing/oversized Cargo; strict schema,
unknown fields/version, duplicates/overlap, symlink file/parent escapes and root
alias policy; missing endpoints; physical relocation; manifest reorder/repeat
output; hostile controls; exact 1 MiB manifest vs one-byte excess; registration,
contract, consumer and dependency-count ceilings; committed schema edits, delete,
rename, unrelated docs, source changes, dirty provider/consumer; nested Git roots;
non-Git/unresolvable-range failures; cycles, depth and deterministic witnesses.
Native-path collision tests inject native byte paths (independent of APFS invalid
name creation); existing Git/store native tests also remain in the canonical run.
No new filesystem-invalid native filename fixture is claimed.

Release binary manual smoke used a temporary Python driver with temporary local
Git repos and local fixture-only Git user config. `inspect` returned one observed
aliased Cargo path edge, and a committed provider src/lib.rs change returned its
consumer as `coarse_package_candidate` at depth two. The temp roots were removed.
The equivalent checked-in production fixture is reproducible with
`cargo test --test workspace package_transitivity_cycles_depth_and_witnesses`.

The same release driver registered the real sibling context-engine and
context-integrity repositories via temporary absolute-root manifest. It declared
src/adapter.rs -> src/context_integrity/context_engine.py solely as a test mapping.
`inspect` and exact `consumers` returned that one declared edge; `impact --repo
engine --change HEAD~1..HEAD` returned integrity as `declared_contract_candidate`.
No mapping was saved in either repository and no runtime use assertion is made.
This tests existing real Rust/Python trees without scanning or executing them.

No cloud/provider, external deployment, Linux/Windows or independent executable
verification is claimed. Existing HTTP authorization/single-root and official MCP
catalog/wire/client tests run unchanged in the canonical verifier.

## Deviations, limitations and review focus

- The pre-existing plan is not committed: the attachment explicitly conditions
  committing planning artifacts on authorization; user work remains preserved.
- Exact package trigger policy is deliberately narrower than every changed path:
  existing source-role classification plus Cargo.toml/build.rs. This avoids treating
  docs-only changes as package evidence, but custom Cargo inputs and generated/test
  changes can remain uncovered. The coarse rule is documented, not compiler proof.
- Deleted provider declarations use exact regular committed diff sides rather
  than requiring current provider existence, which would erase deletion impact.
- Workspace consumers is a typed application use case; rendering is pure. Existing
  source open logic is shared rather than adding a separate path-access pipeline.
- No output/storage/adapter compatibility version bump outside workspace v1 is
  justified. No multi-root service, broader context disclosure or Phase 11 work.
- Independent review should reconstruct exact ownership, source/manifest provenance,
  stale/missing side semantics, first-witness dedup, hard bounds and current/committed
  mixing. Root/package/version/feature limitations and filesystem races are real;
  candidate absence must never be interpreted as safety or proven lack of impact.

README/current-state stale cancellation and “no service” text was corrected only
after checking the existing MCP cancellation removal and delivered service path.
No Phase 9 implementation change was made. No prior-phase regression was observed
in the executed final verification. Review and maintainer acceptance remain pending.
