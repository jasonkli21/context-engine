# Contributing

Thanks for contributing to Context Engine.

## Before changing code

Use [`docs/README.md`](docs/README.md) to select the smallest relevant context set. For ordinary implementation work, read:

1. [`AGENTS.md`](AGENTS.md)
2. [`docs/current-state.md`](docs/current-state.md)
3. the active/relevant phase plan
4. affected code and tests

Read the full PRD or architecture only when the change actually touches their scope.

## Development setup

Install stable Rust with `rustfmt` and `clippy`, then run:

```bash
./scripts/verify.sh
```

The repository intentionally uses one canonical verification command so human and AI contributors do not accumulate subtly different test rituals.

## Change discipline

- Keep changes scoped to the active plan or clearly documented issue.
- Prefer small, reviewable commits.
- Do not add speculative abstractions for future phases.
- Do not mix architecture changes with unrelated cleanup.
- Keep public behavior and docs accurate; never advertise planned features as implemented.
- Add tests for behavior and invariants, not only implementation details.
- If a durable architecture decision changes, add or update an ADR under `docs/decisions/`.

## Phase workflow

For substantial phase implementation:

```text
plan
  ↓
implementation
  ↓
implementation report
  ↓
independent review
  ↓
fixes
  ↓
verification
  ↓
current-state update
```

Use the templates under `docs/templates/`.

A phase plan is a requirements source, not proof of delivery. The implementation report should state exactly what changed, what was verified, any deviations, and anything still externally/unverified.

## Pull requests

Pull requests should include:

- scope and plan/issue reference;
- significant architectural decisions or deviations;
- tests/verification actually run;
- documentation changed;
- unverified areas;
- implementation report/review links when closing a phase.

## Generated code and AI assistance

AI-assisted contributions are welcome, but the contributor remains responsible for correctness. Agents should inspect the repository and actual execution path rather than assuming plans or generated summaries are current. Do not commit session transcripts or model scratchpads as architecture documentation.
