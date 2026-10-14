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

## Future evidence contract

Externally consumed code facts should eventually carry enough provenance to identify:

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

The first deliberate integration is a later roadmap phase, not a Phase 0 requirement.
