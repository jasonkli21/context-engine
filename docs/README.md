# Documentation router

This file routes humans and coding agents to the **smallest context set needed for the task**. Do not load every document by default.

## Canonical context layers

| Layer | Purpose | Default authority |
|---|---|---|
| `../AGENTS.md` | compact repository-wide operating rules | mandatory instructions |
| `current-state.md` | single concise living status snapshot | current status |
| code + tests | delivered behavior / executable evidence | implementation truth |
| `decisions/` | accepted durable decisions | architecture decisions |
| `architecture.md` | durable technical boundaries and intended seams | architecture intent |
| `prd.md` | long-term product scope and thesis | product intent |
| `plans/` | authorized future work | requirements, **not delivery evidence** |
| `reports/` | what an implementation actually changed and verified | delivery evidence |
| `reviews/` | independent findings and verification | review evidence |
| `integrations/` | cross-project contracts and boundaries | integration intent/contracts |
| `archive/` | superseded/historical material | non-authoritative by default |

## Task routes

### Implement the active phase

Read:

1. `../AGENTS.md`
2. `current-state.md`
3. the relevant file in `plans/`
4. affected code/tests
5. `architecture.md` only for boundaries/contracts/data model decisions

Do **not** read the entire PRD unless scope/long-term intent is genuinely ambiguous.

### Plan a future phase

Read:

1. `current-state.md`
2. `prd.md`
3. `architecture.md`
4. relevant accepted ADRs
5. recent reports/reviews that constrain the seam being extended

Write the new plan under `plans/`; do not claim planned capability is implemented.

### Perform an independent implementation review

Read:

1. `current-state.md`
2. relevant phase plan
3. implementation report, if present
4. actual diff/code/tests and production path
5. `architecture.md`
6. relevant ADRs and downstream plan sections

Treat the plan as a requirements source, not as proof that its architecture was complete. Record findings under `reviews/`.

### Fix review findings

Read the review, relevant code/tests, architecture/ADR sources, and original plan. Make targeted fixes; do not expand into unrelated cleanup.

### Work on Context Integrity integration

Read:

- `integrations/context-integrity.md`
- `architecture.md`
- relevant evidence/data-model sections of `prd.md`

Keep authority/policy/drift semantics outside the Rust engine.

### Update project status

Update `current-state.md`. Do not add a new root-level handoff/status file.

## Context hygiene rules

- Prefer links over copied durable facts.
- Keep `current-state.md` short enough to read every session.
- Plans live only under `plans/`; reports/reviews are separate evidence classes.
- When a document is superseded, move it to `archive/` or mark it explicitly; do not let multiple "current" status files accumulate.
- Local READMEs/nested `AGENTS.md` should be added only when a subsystem has stable invariants/ownership worth routing locally.
- Agent-specific files may point to `AGENTS.md` but must not become independent rule sources.
