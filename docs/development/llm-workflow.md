# LLM-assisted development workflow

This repository is organized so coding agents can work with **bounded, explicit context** instead of repeatedly ingesting the whole project history.

## 1. Progressive disclosure

Every session starts with a small mandatory set:

```text
AGENTS.md
    ↓
docs/current-state.md
    ↓
relevant phase plan
    ↓
affected code/tests
```

Only expand into `architecture.md`, ADRs, reports/reviews, or the PRD when the task requires them.

This keeps stable instructions cacheable, reduces contradictory context, and leaves more budget for source code and reasoning.

## 2. Keep context classes separate

### Durable intent

- `docs/prd.md`
- `docs/architecture.md`
- accepted ADRs

### Current status

- `docs/current-state.md`

### Future work

- `docs/plans/`

### Delivery evidence

- code/tests;
- `docs/reports/`;
- commits and verification output.

### Independent review evidence

- `docs/reviews/`

### Historical context

- `docs/archive/`

Never blur these classes. In particular, an implementation plan is not evidence that the feature exists.

## 3. Recommended phase loop

Use a fresh/bounded implementation context for a phase or coherent sub-phase:

```text
plan
  ↓
implement
  ↓
run canonical verification
  ↓
write implementation report
  ↓
independent review in a fresh review context
  ↓
fix findings
  ↓
final verification
  ↓
update current-state
```

The independent reviewer should reconstruct how the change fits the repository rather than simply checking whether plan bullets have corresponding files/tests.

## 4. Implementation-session contract

Before editing:

- inspect the actual code/tests;
- identify the current architecture seam;
- reconcile the plan with what is already present;
- preserve explicit non-goals;
- state any plan deviation in the implementation report.

While editing:

- prefer extending established abstractions over creating parallel paths;
- avoid name-specific branching in generic layers unless the domain truly requires it;
- keep state ownership singular and explicit;
- make deterministic/offline tests the default where possible;
- do not hide external/unverified acceptance areas.

At completion:

- run `./scripts/verify.sh`;
- produce/update the implementation report;
- do not self-promote the phase to complete in `current-state.md` until required review/fix/verification is done.

## 5. Review-session contract

Review against:

- written plan;
- actual architecture;
- durable long-term intent;
- preceding abstractions;
- downstream consumers;
- code/tests/production composition.

Look specifically for:

- requirements implemented but not wired into the real path;
- duplicate abstractions or state owners;
- bypasses around shared boundaries;
- incorrect error/failure semantics;
- stale fallback paths;
- tests that only validate synthetic composition;
- architecture likely to be undone in the next few phases;
- docs claiming more than code proves.

Record all actionable findings with severity, affected components, why it matters, expected behavior, fix direction, and verification.

## 6. Handoff discipline

Do not create `handoff.md`, session scratchpads, or model-specific status files as durable repository state.

If work must continue in another session, the handoff should be reconstructable from:

- `current-state.md`;
- active plan;
- implementation report/review;
- commits/code/tests.

If temporary notes are necessary, keep them untracked or move durable conclusions into the proper canonical document before ending the session.

## 7. Local context only when earned

Do not pre-create READMEs or nested `AGENTS.md` files for every directory.

Add local guidance when a subsystem becomes large enough to have stable:

- ownership;
- invariants;
- extension seams;
- verification commands;
- non-obvious failure semantics.

Local docs should explain **why and where**, not copy the root instructions.

## 8. Keep the repository truthful

After meaningful changes:

- update public README only for shipped user-facing capability;
- update `current-state.md` for current status;
- update architecture/ADR only when a durable decision changed;
- archive/supersede obsolete context instead of adding a competing current document.

The repository should be understandable from current code plus a small number of canonical documents, not from archaeological reconstruction of old agent sessions.
