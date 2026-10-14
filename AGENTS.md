# Agent instructions

This is the canonical repository-level instruction file. Keep it compact. Do not create a second independent ruleset in `CLAUDE.md`, Copilot instructions, handoff docs, or session notes.

## Read order

For any code task:

1. `docs/current-state.md`
2. the active/relevant file under `docs/plans/`
3. affected code and tests
4. `docs/architecture.md` only when the task touches a boundary, data model, public contract, or future seam
5. `docs/prd.md` only when product scope or long-term intent is materially relevant

Use `docs/README.md` to route other task types.

## Source-of-truth rules

- Code/tests + verified behavior are evidence of what is delivered.
- `docs/current-state.md` is the single concise living status snapshot.
- Accepted ADRs record durable decisions.
- `docs/architecture.md` and `docs/prd.md` define durable intent, not proof of implementation.
- `docs/plans/` authorizes future work; a plan is never evidence that work shipped.
- `docs/reports/` and `docs/reviews/` are delivery/review evidence.
- `docs/archive/` is historical context and non-authoritative by default.
- Session transcripts, temporary handoffs, and generated summaries must not become hidden canonical state.

## Implementation rules

- Preserve the engine's descriptive boundary: observable code facts belong here; Context Integrity owns authority/policy/drift judgments.
- Keep Phase 0 synchronous and small. Do not add Tree-sitter, persistence, async, threads, MCP, embeddings, model calls, or graph databases unless the active plan explicitly advances to that phase.
- Prefer a plain struct/function/module before a trait, framework, service, or new crate.
- Do not execute analyzed repository code as part of scanning/indexing.
- Keep machine-readable contracts explicitly versioned once exposed.
- Unsupported/uncertain evidence must be represented honestly rather than guessed.
- No `unsafe` without measured need, documented rationale, and review.

## Rust quality gate

Run the canonical verifier before claiming completion:

```bash
./scripts/verify.sh
```

Do not claim cloud/provider/external verification that was not actually run.

## LLM development hygiene

- Keep mandatory context small; follow links instead of copying durable facts into multiple files.
- Add local README or nested `AGENTS.md` only when a subsystem has stable ownership/invariants that cannot be explained locally in code.
- Update existing canonical docs rather than adding another handoff/status file.
- When a phase closes, create/update implementation evidence and then update `docs/current-state.md`.
- Reviews should reconstruct the real architecture independently; passing tests alone do not prove correct integration.
