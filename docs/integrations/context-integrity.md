# Context Integrity integration boundary

Context Engine and Context Integrity should develop independently until the code-intelligence engine exposes a stable evidence/query layer.

## Responsibility split

### Context Engine — descriptive evidence

Owns or may own:

- files and repository identity;
- source ranges;
- symbols;
- structural relationships;
- Git revisions and structural changes;
- impact evidence;
- context candidates and ranking evidence;
- cross-repository code/contract relationships.

Primary question:

> **What is true in the codebase?**

### Context Integrity — normative interpretation

Owns or may own:

- source authority;
- claims;
- validity/freshness;
- supersession;
- architectural intent and policies;
- contradictions;
- drift judgments;
- context contracts and governance.

Primary question:

> **Does the observed evidence align with what should be trusted or should hold?**

## Evidence v1 opt-in contract

Phase 11 adds `ctx evidence structure|changes|candidates|workspace` with logical IDs,
scoped references, nullable revision metadata and explicit coverage. See the
[normative contract](../contracts/context-integrity-evidence-v1.md) and JSON Schema.
Facts carry provenance to identify:

```text
repository
revision
entity identity
file
source range
relation/derivation type
analyzer/parser version where relevant
```

Context Integrity should consume this via an adapter/query contract. The projects should not require a shared database or shared implementation language.

## Guardrail

Do not add document authority, supersession, architectural rule ownership, or drift policy to Context Engine merely to make a future integration convenient. Likewise, Context Integrity should not permanently duplicate the parser/index/graph engine once this project exposes stable evidence.

The existing Python provider still consumes unchanged `ctx context` and `ctx graph`
schema v1. Evidence v1 is an additive opt-in seam; no Python consumer migration or
policy implementation is included. The four body-free feeds are validated by an
independent offline subprocess decoder in the engine tests.
