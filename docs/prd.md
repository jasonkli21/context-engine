# Codebase / Context Intelligence Engine — Product Requirements Document

**Status:** Draft v1
**Date:** 2026-10-12
**Working names:** `context-engine`, `context-core`, `ctx`
**Primary implementation language:** Rust
**Primary mode:** Local-first CLI/library, with daemon/API support later
**Related project:** Context Integrity
**Audience:** Project owner, future contributors, implementation/review agents, prospective users

---

## 1. Executive Summary

The Codebase / Context Intelligence Engine is a local-first system for building and querying a machine-readable model of software repositories.

Its purpose is to answer, deterministically and efficiently:

> **What is true about this codebase, how is it connected, how has it changed, and what context is most relevant to a given task?**

The engine should ingest source repositories and progressively model:

- files and directories;
- languages and source roots;
- symbols and declarations;
- imports, references, calls, inheritance, implementations, and other relationships;
- tests and their relation to production code;
- documentation and code associations;
- Git history, diffs, authorship, and change evolution;
- APIs, schemas, and cross-repository contracts where feasible;
- task-relevant structural neighborhoods;
- repository and system-level context packages for human and agent consumption.

The system is intentionally **descriptive rather than normative**.

It should report facts such as:

- `TravelService` directly depends on `DynamoTripStore`;
- `ModelRouter` has five dependents;
- this pull request introduced three new cross-package edges;
- these seven files and eleven symbols are most structurally relevant to a task;
- this API schema changed at commit X and is consumed by repositories A, B, and C.

It should **not**, by itself, decide that those facts violate an architecture policy, contradict an ADR, or represent documentation drift. Those higher-level judgments belong primarily to the separate **Context Integrity** project.

This creates a deliberate long-term relationship:

```text
Codebase / Context Intelligence Engine
"What is true?"

              ↓ structured evidence

Context Integrity
"Does what is true align with what should be true?"
```

The two projects should develop separately at first. The Codebase / Context Intelligence Engine must be independently useful as a CLI, library, and future local service. However, its data model and APIs should preserve a clean future integration seam so Context Integrity can consume its evidence without reimplementing repository parsing, dependency graphing, change analysis, or task-context selection.

The project has a second explicit objective: **serve as a progressive Rust learning project**. The implementation roadmap should introduce language and systems concepts only when product requirements justify them. Early milestones should be intentionally small and synchronous. Async runtimes, complex persistence, multi-process services, distributed infrastructure, embeddings, and model calls should not be introduced merely because they may exist in the eventual architecture.

---

# 2. Problem Statement

Modern software repositories are difficult for both humans and AI agents to reason about because the relevant context for a change is distributed across:

- source files;
- symbol definitions;
- call/reference relationships;
- interfaces and implementations;
- tests;
- configuration;
- database schemas and migrations;
- API schemas;
- documentation;
- ADRs;
- prior commits and pull requests;
- neighboring repositories;
- generated artifacts;
- runtime contracts.

Existing developer tools expose many fragments of this information, but they usually optimize for a particular interaction surface:

- textual search;
- IDE navigation;
- language-server symbol lookup;
- static analysis;
- AST-aware search;
- repository packing for LLM prompts;
- code graph exploration;
- agent-oriented retrieval;
- code editing;
- documentation drift checks.

Coding agents often compensate by repeatedly reading large files, running broad text searches, or rebuilding an approximate mental model from scratch. This wastes tokens and time, and it disproportionately hurts weaker or inexpensive models.

At the same time, repository-scale architectural review is difficult because there is often no stable, queryable evidence layer that can answer:

- what changed structurally;
- which architectural neighborhoods are affected;
- whether a new dependency edge is actually new;
- which tests cover a symbol;
- which external repositories consume a changed contract;
- which historical decisions and files are likely relevant.

A reusable codebase intelligence layer can make these relationships explicit.

---

# 3. Product Thesis

> **A software repository should have a continuously queryable structural and evolutionary model that is independent of any specific LLM, IDE, or policy engine.**

The engine should convert repositories from collections of files into structured evidence.

At maturity:

```text
Repository / Repositories
        │
        ├── source
        ├── tests
        ├── schemas
        ├── docs
        ├── git history
        └── configuration
                │
                ▼
      Codebase Intelligence Engine
                │
        ┌───────┼────────┐
        │       │        │
        ▼       ▼        ▼
      Graph   Search   Change Model
        │       │        │
        └───────┼────────┘
                ▼
      Query / Context Layer
                │
      ┌─────────┼─────────┐
      ▼         ▼         ▼
     CLI      Agents    Context Integrity
```

The engine should create leverage primarily through:

1. **Deterministic understanding before probabilistic reasoning.**
2. **Reusable representation rather than one-off prompts.**
3. **Incremental updates rather than full rescans.**
4. **Structural + historical context rather than plain text alone.**
5. **Task-specific context selection rather than repository dumping.**
6. **Explicit evidence and provenance rather than opaque similarity scores.**
7. **Local-first operation with no mandatory model/API cost.**

---

# 4. Why This Project Exists Separately from Context Integrity

The projects have overlapping long-term concerns but different responsibilities.

## 4.1 Codebase / Context Intelligence Engine

Primary question:

> **What is true in the codebase?**

Owns or may own:

- filesystem discovery;
- language detection;
- parsing;
- symbol extraction;
- references and structural relations;
- dependency graphs;
- Git/diff analysis;
- historical evolution;
- code search;
- structural search;
- task-relevance ranking;
- context bundle construction;
- cross-repository code/contract relationships;
- incremental indexing;
- stable machine-readable query APIs.

## 4.2 Context Integrity

Primary question:

> **Does the current and assembled context accurately reflect the system's intended state, authority, and constraints?**

Owns or may own:

- source authority;
- provenance;
- validity and freshness;
- supersession;
- contradictions;
- stale documentation;
- architectural intent;
- policy/invariant representation;
- context contracts;
- evidence-to-claim relationships;
- drift judgments;
- task-context policy;
- governance;
- integrity checks and CI enforcement.

## 4.3 Shared Boundary

The projects intentionally overlap at the point where structural evidence becomes architectural meaning.

Example:

```text
Rust engine reports:

TravelService
  └── imports / depends_on → DynamoTripStore

Context Integrity knows:

"Application services must access persistence
 through the TripRepository abstraction."

Context Integrity result:

Potential architectural drift.
```

The codebase engine must not require Context Integrity to be useful.

Context Integrity should not need to implement another parser/indexer once the engine exposes stable evidence.

---

# 5. Goals

## 5.1 Near-Term Goals

1. Build a useful Rust CLI while learning idiomatic Rust.
2. Scan and summarize a local repository.
3. Parse a small number of languages structurally.
4. Extract symbols and basic relationships.
5. Build a queryable repository graph.
6. Add Git-aware change analysis.
7. Produce useful task-specific context without requiring an LLM.
8. Define stable domain models that can later support persistence and external consumers.

## 5.2 Medium-Term Goals

1. Persist repository intelligence efficiently.
2. Incrementally update indexes after file changes.
3. Rank context using multiple deterministic signals.
4. Expose a library and daemon/query API.
5. Support coding-agent integrations such as MCP without making MCP the core architecture.
6. Model cross-repository contracts and dependency relationships.
7. Provide evidence consumable by Context Integrity.

## 5.3 Long-Term Goals

1. Maintain a continuously updated model of a multi-repository software system.
2. Answer impact-analysis and evolutionary questions cheaply.
3. Build high-quality, evidence-backed context bundles for weaker or expensive-to-query models.
4. Enable architectural and documentation integrity systems without coupling the core to one policy model.
5. Potentially provide a commercial local/enterprise developer-intelligence product while preserving a meaningful open-source core.

---

# 6. Non-Goals

The project is not initially:

- an autonomous coding agent;
- a general LLM orchestration framework;
- an architecture-policy engine;
- a replacement for Context Integrity;
- a general-purpose IDE;
- a code editor;
- a code-generation product;
- a hosted SaaS;
- a vector database;
- a replacement for language servers;
- a full compiler;
- a security sandbox;
- a requirement to support every language;
- a justification to rewrite unrelated systems in Rust;
- dependent on embeddings;
- dependent on cloud services;
- dependent on paid models;
- dependent on MCP.

These may become integrations or adjacent capabilities, but they should not distort the core.

---

# 7. Target Users

## 7.1 Primary Initial User

A developer working across one or more repositories who wants:

- fast repository understanding;
- dependency queries;
- change impact;
- better AI context;
- a learning-oriented systems project.

## 7.2 Future Users

### Software engineers

Questions:

- What calls this function?
- What will likely break if I change this interface?
- Which tests touch this subsystem?
- What did this area look like three months ago?
- What are the most central abstractions in this package?

### Coding agents

Need:

- precise symbol lookup;
- dependency neighborhoods;
- relevant tests;
- compact context under a token budget;
- evidence about changed code;
- cross-repository consumers.

### Review systems

Need:

- added/removed dependency edges;
- changed public symbols;
- affected contracts;
- structural risk areas;
- likely missing test updates.

### Context Integrity

Need:

- stable symbol identities;
- structural relationships;
- change sets;
- provenance to code locations/commits;
- cross-repo relationships;
- task-relevant code evidence.

---

# 8. Jobs to Be Done

## 8.1 Understand a Repository

```bash
ctx scan .
ctx tree
ctx stats
ctx symbols
```

Answer:

- What languages are here?
- What are the likely source/test roots?
- What are the largest modules?
- What are the key symbols?
- What are the dependency hotspots?

## 8.2 Understand a Symbol

```bash
ctx symbol ModelRouter
ctx callers ModelRouter.route
ctx deps ModelRouter
ctx related ModelRouter
```

Answer:

- Where is it defined?
- What does it depend on?
- What depends on it?
- What tests and docs are related?
- What changed recently?

## 8.3 Understand a Change

```bash
ctx impact HEAD~1..HEAD
```

Answer:

- What symbols changed?
- What relationships changed?
- What dependents are affected?
- Were public contracts modified?
- Which tests/docs may need review?

## 8.4 Assemble Agent Context

```bash
ctx context \
  --task "Add another model provider" \
  --budget 12000
```

Return a ranked, explainable set of:

- files;
- symbols;
- tests;
- schemas;
- docs;
- dependency paths;
- change/history snippets.

## 8.5 Understand Evolution

```bash
ctx history MemoryStore
ctx evolution src/memory/
```

Answer:

- when was this abstraction introduced?
- what major changes occurred?
- which dependencies accumulated?
- which symbols replaced older ones?

## 8.6 Understand Cross-Repository Impact

```bash
ctx impact personal-ai-system:RouteDecision --workspace ~/code
```

Return likely consumers and affected contracts across repositories.

---

# 9. Product Principles

## 9.1 Deterministic First

If parsing, graph traversal, exact search, Git, or explicit metadata can answer a question reliably, use them before an LLM.

## 9.2 LLM Optional

The core must remain useful with zero model calls.

Model-assisted features may later enrich:

- ambiguous relationship classification;
- semantic summaries;
- inferred architectural concepts;
- task-intent expansion.

But their output must be distinguishable from deterministic evidence.

## 9.3 Evidence Is Inspectable

Every result should ideally be traceable to:

- file path;
- source range;
- symbol;
- parser;
- commit;
- index version;
- relation derivation.

## 9.4 Stable Domain Model over Tool-Specific APIs

Do not model the core around Claude, Codex, ChatGPT, Cursor, or MCP.

Prefer domain types such as:

```text
Repository
File
Symbol
Relation
ChangeSet
Contract
ContextCandidate
ContextBundle
EvidenceRef
```

Adapters can expose these to tools later.

## 9.5 Local-First and Cheap

The expected default should be:

- local repository;
- local CPU;
- local persistent index;
- no mandatory network;
- no mandatory API keys.

## 9.6 Incrementality Is a Long-Term Requirement, Not an MVP Requirement

The data model should not prevent incremental updates, but the first implementation can rebuild from scratch.

## 9.7 Cross-Language Architecture, Small Initial Language Set

The internal model should be language-neutral.

The first parser implementations should support only enough languages to validate the abstraction.

## 9.8 Explain Ranking

Task-context ranking should expose why items were selected.

Example:

```text
src/providers/registry.py
score: 0.91
reasons:
- exact task keyword match
- defines ProviderRegistry
- 2-hop dependency from ModelRouter
- referenced by 4 provider tests
```

---

# 10. Conceptual Data Model

The exact implementation can evolve, but the conceptual model should remain explicit.

## 10.1 Repository

```text
Repository
- id
- root
- vcs identity
- revision
- languages
- source roots
- metadata
```

## 10.2 File

```text
File
- id
- repository_id
- path
- language
- content hash
- role
- generated?
- source range metadata
```

Potential roles:

- source;
- test;
- documentation;
- config;
- schema;
- migration;
- generated;
- unknown.

## 10.3 Symbol

```text
Symbol
- id
- repository_id
- file_id
- language
- kind
- qualified_name
- display_name
- source range
- signature
- visibility
- stable identity metadata
```

Potential kinds:

- function;
- method;
- class;
- struct;
- enum;
- trait/interface;
- module;
- constant;
- field;
- endpoint;
- schema;
- test.

## 10.4 Relation

```text
Relation
- from
- to
- kind
- evidence
- confidence / derivation type
```

Potential relation kinds:

- contains;
- imports;
- references;
- calls;
- implements;
- extends;
- returns;
- accepts;
- tests;
- documents;
- configures;
- migrates;
- consumes_contract;
- produces_contract.

Not all relations will be available in every language.

## 10.5 Evidence Reference

```text
EvidenceRef
- repository
- revision
- file
- source span
- parser / analyzer
- derivation
```

This is important for later Context Integrity integration.

## 10.6 Change Set

```text
ChangeSet
- base revision
- head revision
- files added/removed/modified
- symbols added/removed/modified
- relations added/removed
- contracts changed
```

## 10.7 Context Candidate

```text
ContextCandidate
- entity
- score
- reasons[]
- token / byte cost
- evidence
```

## 10.8 Context Bundle

```text
ContextBundle
- task
- repository revision
- budget
- selected candidates
- selection reasons
- omitted candidates
- ranking version
```

This bundle should be consumable without requiring a specific model vendor.

---

# 11. Intelligence Layers

The system should grow in layers.

```text
L0  Filesystem / repository inventory
L1  Syntax / AST
L2  Symbols
L3  Static relationships
L4  Repository graph
L5  Git/change intelligence
L6  Retrieval / ranking
L7  Historical evolution
L8  Cross-repository contracts
L9  Optional semantic enrichment
```

Context Integrity may consume Layers 2–8 while adding authority, intent, claims, and drift semantics.

---

# 12. Core Capabilities

## 12.1 Repository Inventory

Discover:

- files;
- directories;
- ignored paths;
- likely generated paths;
- language mix;
- source roots;
- test roots;
- documentation roots;
- package manifests;
- VCS metadata.

MVP can be heuristic and transparent.

## 12.2 Syntax Parsing

Use Tree-sitter or another well-justified parser backend for supported languages.

Requirements:

- parser abstraction;
- syntax errors should not crash whole indexing;
- unsupported files remain visible as text/file entities;
- parser version recorded;
- source locations preserved.

## 12.3 Symbol Extraction

Extract high-value symbols first.

Initial examples:

- Rust: modules, structs, enums, traits, impls, functions;
- Python: modules, classes, functions/methods;
- TypeScript: modules, classes, interfaces, functions, methods, exported values.

## 12.4 Relationship Extraction

Start simple:

- containment;
- imports;
- definitions;
- simple references where reliable.

Later:

- call graph;
- implementations;
- inheritance;
- type references;
- schema consumers;
- endpoint relationships.

The engine must represent uncertainty honestly. A Tree-sitter-derived syntactic relation should not be presented as equivalent to a compiler-resolved semantic relation.

## 12.5 Repository Graph

Create a graph over:

- repositories;
- files;
- symbols;
- contracts;
- relations.

The first implementation need not use a graph database.

An in-memory adjacency representation may be preferable for learning and simplicity.

## 12.6 Git Intelligence

Support:

- repository revision;
- diff ranges;
- changed files;
- line diffs;
- symbol-aware changes;
- added/removed relations;
- blame/history later.

The key value is converting text diffs into structural diffs.

## 12.7 Search

Search modes may include:

- exact symbol;
- fuzzy symbol;
- text;
- path;
- structural;
- relation traversal.

Full-text indexing can be introduced later if needed.

## 12.8 Impact Analysis

Given:

- symbol;
- file;
- diff;
- contract;

produce likely impact based on:

- reverse dependencies;
- reference graph;
- tests;
- public API changes;
- graph centrality;
- cross-repo consumers.

Impact is evidence, not proof of runtime breakage.

## 12.9 Task-Relevant Context

Given a task and a budget:

1. identify lexical matches;
2. resolve matching symbols;
3. expand graph neighborhood;
4. include associated tests/docs/schemas;
5. rank by structural importance and task relevance;
6. select under budget;
7. retain explanation/provenance.

The initial version should avoid embeddings.

## 12.10 Cross-Repository Intelligence

Long-term support:

- workspace manifests;
- repository registry;
- package dependencies;
- API schema relationships;
- shared libraries;
- message/event schemas;
- explicit mappings;
- cross-repo call/consumer evidence.

This is strategically important because many agent context systems remain repository-centric.

---

# 13. Context Bundle Design

A context bundle should be structured, not simply concatenated text.

Possible representation:

```yaml
task: "Add OpenRouter provider"
revision: "abc123"
budget:
  unit: tokens
  max: 12000

entities:
  - type: symbol
    id: "ProviderRegistry"
    reason:
      - exact_match
      - architectural_centrality
  - type: file
    path: "src/providers/gemini.py"
    reason:
      - sibling_implementation
  - type: test
    path: "tests/providers/test_registry.py"
    reason:
      - tests_selected_symbol

relationships:
  - from: ModelRouter
    to: ProviderRegistry
    kind: references
```

Output adapters can render:

- Markdown;
- JSON;
- XML;
- MCP resource/tool response;
- editor-friendly representations.

---

# 14. Change Intelligence

Change intelligence is a major differentiator from simple repository packing.

Given:

```bash
ctx impact main..HEAD
```

the engine should eventually produce:

```text
Changed:
- 11 files
- 24 symbols
- 7 relations

Added edges:
- TravelService -> DynamoTripStore
- RoutePlanner -> OpenRouterProvider

Removed edges:
- RoutePlanner -> ProviderRegistry

Potential high-impact changes:
- ProviderRegistry interface changed
- 4 known implementations
- 9 dependent call sites
- 3 affected tests
```

Future extensions:

- edge novelty;
- historical frequency;
- hotspot weighting;
- public API compatibility;
- ownership/history;
- change coupling;
- likely forgotten consumers.

---

# 15. Historical / Evolution Intelligence

A codebase's current state is not enough for many difficult tasks.

Long-term queries:

```bash
ctx history MemoryStore
ctx why ProviderRegistry
ctx evolution src/storage
```

Possible evidence:

- first introduction;
- major signature changes;
- rename lineage;
- dependency additions/removals;
- associated ADR/doc references;
- commits touching both entities;
- tests added after regressions.

The engine should distinguish deterministic historical evidence from inferred narratives.

---

# 16. External Interfaces

## 16.1 CLI

The CLI is the first-class early surface.

Illustrative commands:

```bash
ctx scan .
ctx stats
ctx tree

ctx index .
ctx symbols
ctx symbol <name>

ctx deps <symbol>
ctx callers <symbol>ctx related <entity>

ctx diff <range>
ctx impact <range-or-entity>

ctx search <query>

ctx context --task "<task>" --budget <n>

ctx history <entity>

ctx serve
```

Commands should be composable and support machine-readable output later.

## 16.2 Library

The core should eventually be exposed as Rust crates/modules that do not depend on CLI concerns.

Possible packages:

```text
ctx-core
ctx-parse
ctx-graph
ctx-git
ctx-rank
ctx-store
ctx-cli
ctx-server
```

This is aspirational; do not create a multi-crate workspace prematurely.

## 16.3 Daemon/API

Later:

```text
ctx serve
```

Potential endpoints:

```text
GET  /symbols/{id}
GET  /symbols/{id}/dependencies
GET  /symbols/{id}/dependents
GET  /changes?base=&head=
POST /context
POST /search
```

Transport could be HTTP, Unix socket, or another local IPC mechanism.

No decision is required in early phases.

## 16.4 MCP Adapter

MCP should be an adapter over the query layer, not the internal architecture.

Potential tools:

- `find_symbol`
- `find_references`
- `get_dependency_neighborhood`
- `get_change_impact`
- `build_task_context`

---

# 17. Context Integrity Integration Contract

This section is a long-term architectural constraint even though integration is not an MVP feature.

## 17.1 Engine Responsibilities

The engine should expose facts with provenance.

Example:

```json
{
  "relation": {
    "from": "TravelService",
    "to": "DynamoTripStore",
    "kind": "imports"
  },
  "evidence": {
    "repository": "travel-app",
    "revision": "abc123",
    "path": "src/travel_service.py",
    "range": {"start": 8, "end": 8}
  }
}
```

## 17.2 Context Integrity Responsibilities

Context Integrity may add:

```json
{
  "claim": "Application services must not directly depend on persistence adapters",
  "authority": "ADR-0012",
  "status": "active",
  "evaluation": "violated"
}
```

## 17.3 Identity Requirements

The engine should aim for stable identities for:

- repository;
- revision;
- file;
- symbol;
- relation;
- contract.

Symbol identity is difficult across refactors. Early versions may use revision-scoped identity and explicitly defer robust lineage.

## 17.4 Provenance Requirements

Every exported fact should indicate:

- source revision;
- source location;
- derivation method;
- analyzer/parser version where relevant.

## 17.5 No Duplicate Policy Layer

The engine should not develop a second competing system for:

- document authority;
- supersession;
- architectural rule ownership;
- claims;
- governance.

If simple boundary rules are eventually useful for standalone operation, they should remain clearly separate from the richer Context Integrity semantic model.

---

# 18. Architecture Direction

## 18.1 Early Architecture

```text
CLI
 │
 ▼
Application services
 │
 ├── repository scanner
 ├── parser registry
 ├── symbol extractor
 ├── graph builder
 └── query services
        │
        ▼
  in-memory model
```

This is intentionally simple.

## 18.2 Later Architecture

```text
Filesystem / Git
      │
      ▼
 Incremental Indexer
      │
 ┌────┴────────┐
 ▼             ▼
Syntax Index   Change Index
 │             │
 └──────┬──────┘
        ▼
 Repository Graph
        │
 ┌──────┼─────────┐
 ▼      ▼         ▼
Search  Rank    History
 │      │         │
 └──────┼─────────┘
        ▼
 Query Surface
        │
 ┌──────┼──────────────┐
 ▼      ▼              ▼
CLI    MCP/API    Context Integrity
```

## 18.3 Storage

No graph database should be required initially.

Possible future options:

- SQLite for durable metadata;
- compact custom serialized indexes;
- Tantivy for full-text search;
- embedded key-value structures;
- optional graph DB only if it creates measurable value.

The architecture must not assume Neo4j or another external service.

---

# 19. Language Support Strategy

## 19.1 Phase Strategy

Do not begin with broad language support.

Suggested validation sequence:

1. Rust — useful for dogfooding.
2. Python — relevant to many existing repositories.
3. TypeScript — relevant to web/application repositories.

However, implementation order may be adjusted to maximize simplicity.

## 19.2 Parser Abstraction

```text
LanguageAdapter
- detect
- parse
- extract_symbols
- extract_relations
```

The common model should not leak Tree-sitter node kinds throughout the rest of the system.

## 19.3 Semantic Backends Later

Potentially support multiple evidence sources:

- Tree-sitter;
- LSP;
- compiler metadata;
- SCIP;
- language-specific analyzers.

Relations should record derivation so higher layers can judge quality.

---

# 20. Ranking Strategy

Initial ranking should be deterministic.

Possible signals:

- task term overlap;
- exact symbol match;
- path match;
- graph distance;
- reverse-reference count;
- PageRank-like centrality;
- test association;
- recent change relevance;
- changed-symbol proximity;
- public/exported status;
- file role;
- explicit user inclusions/exclusions.

Later optional signals:

- BM25;
- embeddings;
- learned ranking;
- historical task feedback.

The ranking system should be modular and evaluable.

---

# 21. Performance Requirements

These are directional, not premature hard SLAs.

## 21.1 Early

- scan medium personal repositories interactively;
- no obvious quadratic behavior over files;
- bounded memory for normal repositories;
- actionable errors rather than panics.

## 21.2 Mature

Aim for:

- fast warm queries;
- incremental updates proportional primarily to changed code;
- parallel parsing where beneficial;
- lazy expensive analyses;
- index size small enough for laptop-scale multi-repo use.

Performance claims should be benchmarked rather than assumed from using Rust.

---

# 22. Reliability Requirements

- malformed source must not corrupt the entire index;
- one unsupported language must not fail repository indexing;
- stale index state must be detectable;
- repository revision must be explicit;
- machine-readable output should be versioned;
- errors should differentiate:
  - user/config error;
  - parse error;
  - unsupported capability;
  - internal error.

No `unsafe` code should be introduced without a concrete measured need.

---

# 23. Security and Privacy

Default posture:

- local processing;
- no source upload;
- no network required;
- respect `.gitignore` and explicit exclusions;
- support secrets-sensitive exclusions;
- avoid automatically indexing large binary/generated/vendor trees;
- make external model calls opt-in if they are ever added.

Future agent integrations should expose read operations before edit operations.

This project is primarily an intelligence provider, not an execution sandbox.

---

# 24. Local-First / Free-Tier Economics

The core product should be usable with effectively zero marginal infrastructure cost.

Expected default:

```text
Laptop
  ├── Rust binary
  ├── repository
  └── embedded index
```

Cloud may later be useful for:

- team sharing;
- CI;
- remote indexing;
- cross-machine synchronization;
- enterprise deployment.

But cloud should be additive.

This also aligns with the goal of improving inexpensive/free-model effectiveness through a stronger harness rather than expensive inference.

---

# 25. Evaluation Framework

The project needs more than "the CLI works."

## 25.1 Extraction Accuracy

Fixtures should validate:

- symbols;
- ranges;
- relations;
- language edge cases.

## 25.2 Graph Correctness

Test:

- traversal;
- cycles;
- reverse edges;
- deletion;
- deduplication.

## 25.3 Change Accuracy

Golden tests for:

- symbol addition;
- symbol removal;
- rename;
- signature change;
- relation addition/removal.

## 25.4 Retrieval Quality

Build a small benchmark:

```text
Task → expected useful entities
```

Examples:

- "add provider"
- "change auth callback"
- "modify storage schema"
- "fix scheduler retry"

Measure:

- recall of expected files/symbols;
- budget efficiency;
- ranking stability.

## 25.5 Agent Utility Later

Compare:

- plain grep / repository dump;
- structural context bundle;
- structural + history bundle.

Measure:

- task success;
- tokens;
- tool calls;
- incorrect architectural assumptions.

---

# 26. Rust Learning Objectives

Learning Rust is an explicit project requirement.

## 26.1 Principles

- prefer safe Rust;
- learn ownership before async;
- learn simple modules before advanced abstraction;
- avoid macros unless they solve a real problem;
- avoid premature trait hierarchies;
- introduce concurrency only after a sequential implementation exists;
- profile before performance optimization.

## 26.2 Learning Progression

| Stage | Product capability | Rust focus |
|---|---|---|
| 0 | Repository scanner | ownership, borrowing, `Result`, iterators, modules |
| 1 | Parser + symbols | enums, traits, Tree-sitter FFI/API usage |
| 2 | Repository graph | collections, graph traversal, domain modeling |
| 3 | Query CLI | API design, lifetimes where naturally needed |
| 4 | Git/change model | external libraries, error boundaries, data modeling |
| 5 | Context ranking | algorithms, scoring, testing |
| 6 | Persistence | Serde, SQLite/embedded storage, migrations/versioning |
| 7 | Incremental updates | channels, threads, `Arc`, synchronization |
| 8 | History and evolution | Git history modeling, temporal data |
| 9 | Local service and agent adapter | Tokio/async, networking/IPC |
| 10+ | Scale | profiling, allocations, parallelism, caching |

---

# 27. Phased Roadmap

The roadmap intentionally separates **learning milestones** from the long-term platform vision.

## Phase 0 — Repository Scanner

### Goal

Create a small, polished Rust CLI that can inspect a repository without syntax parsing.

### Capabilities

```bash
ctx scan .
ctx stats
ctx tree
```

Output:

- file counts;
- language estimates;
- directory tree;
- ignored paths;
- likely source/test/docs roots;
- largest files;
- basic Git metadata.

### Rust focus

- Cargo;
- project structure;
- ownership/borrowing;
- iterators;
- `Path` / filesystem APIs;
- `Result`;
- error reporting;
- Clap;
- tests.

### Non-goals

- Tree-sitter;
- SQLite;
- async;
- threads;
- LLMs;
- graph DB.

### Exit criteria

- useful on at least three real repositories;
- deterministic tests;
- installable binary;
- clean CLI errors;
- no architecture that blocks parser integration.

---

## Phase 1 — Syntax and Symbol Index

### Goal

Turn files into language-aware symbol inventories.

### Capabilities

```bash
ctx index .
ctx symbols
ctx symbol <name>
```

### Scope

Start with 1–2 languages.

### Rust focus

- parser integration;
- enums;
- traits only where justified;
- source ranges;
- domain modeling.

### Exit criteria

- stable `File` and `Symbol` concepts;
- symbol fixtures;
- syntax errors tolerated;
- machine-readable output.

---

## Phase 2 — Repository Relationship Graph

### Goal

Represent useful code relationships.

### Capabilities

```bash
ctx deps <symbol>
ctx callers <symbol>
ctx related <symbol>
```

Initial relation types:

- contains;
- imports;
- basic references.

### Rust focus

- `HashMap`, `HashSet`;
- graph representation;
- traversals;
- cycles;
- IDs and ownership design.

### Exit criteria

- forward and reverse traversal;
- graph serialization;
- evidence attached to edges;
- no mandatory graph database.

---

## Phase 3 — Query and Repository Intelligence

### Goal

Make the graph genuinely useful for interactive exploration.

### Capabilities

- central symbols;
- hotspots;
- path/symbol search;
- neighborhoods;
- test associations where feasible.

### Exit criteria

A developer can answer meaningful repository questions faster than with file browsing alone.

---

## Phase 4 — Git and Structural Change Intelligence

### Goal

Convert Git diffs into structural change sets.

### Capabilities

```bash
ctx diff main..HEAD
ctx impact main..HEAD
```

Report:

- changed symbols;
- new/removed relations;
- likely affected dependents;
- test/doc candidates.

### Exit criteria

Structural diff fixtures work across realistic changes.

### First major product milestone

At this point the tool should already be independently useful even if no AI-specific features are built.

---

## Phase 5 — Task Context Ranking

### Goal

Generate compact task-specific context.

### Capabilities

```bash
ctx context --task "..." --budget ...
```

Signals:

- lexical;
- symbol;
- graph;
- test;
- change proximity;
- centrality.

### Exit criteria

A benchmark shows better expected-file recall under a fixed context budget than naive file selection.

### Important

No embedding requirement.

---

## Phase 6 — Persistent Index

### Goal

Avoid rebuilding repository intelligence for every query.

### Potential technologies

- SQLite;
- custom serialized artifacts;
- optional Tantivy for text.

### Requirements

- schema/index version;
- stale index detection;
- rebuild path.

### Rust focus

- persistence;
- serialization;
- ownership boundaries;
- migration/version design.

---

## Phase 7 — Incremental Indexing

### Goal

Update only affected state when files change.

### Capabilities

- filesystem watch;
- changed-file queue;
- parse invalidation;
- graph edge update;
- dependency invalidation.

### Rust focus

- threads;
- channels;
- synchronization;
- shared state;
- work queues.

### Important

Introduce concurrency only now unless profiling proves earlier need.

---

## Phase 8 — History and Evolution Intelligence

### Goal

Model how abstractions change over time.

### Capabilities

```bash
ctx history <symbol>
ctx evolution <path>
```

### Potential features

- introduction commit;
- major modifications;
- co-change relationships;
- lineage heuristics;
- historical centrality.

---

## Phase 9 — Local Service + Agent Adapter

### Goal

Support persistent consumers.

### Capabilities

```bash
ctx serve
```

Adapters:

- HTTP/IPC;
- MCP;
- editor plugins later.

### Rust focus

- Tokio;
- async service design;
- concurrency boundaries.

### Exit criteria

CLI and external consumers use the same core query layer.

---

## Phase 10 — Cross-Repository Intelligence

### Goal

Move from repository intelligence to system intelligence.

### Capabilities

- workspace registry;
- repository identities;
- package/contract relationships;
- API/schema consumers;
- cross-repo impact analysis.

### Strategic importance

This becomes highly relevant to multi-app systems and Context Integrity.

---

## Phase 11 — Context Integrity Evidence Adapter

### Goal

Create the first deliberate integration with Context Integrity while keeping repositories and ownership separate.

### Deliverables

- evidence export schema;
- revision identities;
- symbol/relation provenance;
- structural diff feed;
- context-candidate feed.

### Non-goal

Do not move authority or policy semantics into the Rust engine.

---

## Phase 12+ — Advanced Intelligence

Possible directions:

- LSP/compiler-backed semantic resolution;
- SCIP support;
- embeddings as optional ranking signal;
- learned ranking;
- runtime traces;
- ownership data;
- PR review integrations;
- architectural visualization;
- richer contract inference;
- organizational multi-repo graphs.

These are intentionally not committed MVP scope.

---

# 28. MVP Definition

There are two useful MVP lines.

## 28.1 Learning MVP

Phases 0–2.

Success means:

- real Rust experience;
- reusable parser/domain design;
- symbol + graph queries;
- small enough to complete without platform sprawl.

## 28.2 Product MVP

Phases 0–5.

Success means:

- repository indexing;
- structural queries;
- Git change intelligence;
- task-specific context ranking;
- useful without an LLM.

This is the preferred point to reassess long-term investment.

---

# 29. Suggested Initial Repository Layout

Do not over-modularize initially.

Example:

```text
context-engine/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── model.rs
│   ├── scan.rs
│   ├── language/
│   ├── graph.rs
│   └── output.rs
├── tests/
└── docs/
    ├── prd.md
    ├── architecture.md
    └── plans/
```

Split into crates only when package boundaries are real.

---

# 30. Open-Source Landscape

This project intentionally overlaps with existing work. Replication has learning value, and mature differentiation should come from combining capabilities rather than pretending basic code graphing is novel.

## 30.1 Repomix

Focus:

- packages repositories into AI-friendly output;
- token counting;
- Git-aware inclusion;
- Tree-sitter-based compression.

Overlap:

- context packaging;
- repository summarization;
- AI context efficiency.

Opportunity:

- persistent graph rather than primarily packed output;
- relationship queries;
- structural change intelligence;
- cross-repo system model;
- Context Integrity evidence integration.

Source: https://github.com/yamadashy/repomix

## 30.2 Aider Repository Map

Focus:

- builds a repository map;
- ranks important repository content using dependency graph signals;
- operates under token budgets.

Overlap:

- graph-based importance;
- token-aware context selection.

Opportunity:

- general reusable engine independent of one coding agent;
- persistent indexes;
- richer change/history intelligence;
- evidence/provenance export.

Source: https://aider.chat/docs/repomap.html

## 30.3 Serena

Focus:

- semantic code retrieval/editing for agents;
- symbol-aware tools;
- LSP/IDE semantic backends;
- MCP integration.

Overlap:

- symbol navigation;
- relationships;
- agent retrieval.

Opportunity:

- engine is read/intelligence-first rather than editing-agent-first;
- explicit Git evolution model;
- deterministic context-bundle ranking;
- Context Integrity integration.

Source: https://github.com/oraios/serena

## 30.4 CodeGraphContext

Focus:

- local code indexing into a graph;
- CLI + MCP;
- Tree-sitter-based code graph.

This is one of the closest comparables.

Overlap:

- local graph;- parsing;
- AI context;
- CLI/MCP.

Opportunity:

- Rust-first learning/performance implementation;
- explicit evidence/provenance semantics;
- first-class structural Git diffs;
- historical/evolution intelligence;
- cross-repository architecture;
- clean separation between evidence and Context Integrity policy.

Source: https://github.com/CodeGraphContext/CodeGraphContext

## 30.5 Potpie

Focus:

- context graph for AI-native SDLC;
- code plus source history and engineering context;
- local context engine/daemon;
- task resolution and agent integrations.

This significantly overlaps with the long-term vision.

Opportunity:

- narrower deterministic core with fewer platform assumptions;
- explicit reusable evidence contract;
- local-first Rust implementation;
- fine-grained structural change model;
- intentional pairing with a separate authority/integrity system.

Source: https://github.com/potpie-ai/potpie

## 30.6 ast-grep

Focus:

- fast structural code search/lint/rewrite;
- Tree-sitter;
- implemented in Rust.

Overlap:

- AST parsing;
- structural matching;
- high-performance local code analysis.

Opportunity:

- ast-grep is primarily structural query/rewrite, not a persistent multi-layer repository/evolution context graph.

Source: https://github.com/ast-grep/ast-grep

---

# 31. Differentiation

The engine should not claim:

> "We build a code graph for AI."

That space already has substantial OSS coverage.

A stronger long-term position is:

> **A local-first, deterministic software-system evidence engine that unifies structural code intelligence, change/evolution intelligence, explainable context compilation, and cross-repository relationships, with stable provenance suitable for downstream integrity and agent systems.**

The strongest differentiators to explore are:

1. **Change as a first-class model**
   - structural diffs;
   - relation diffs;
   - contract diffs;
   - change neighborhoods.

2. **Evidence provenance**
   - where every relationship came from;
   - revision-scoped truth;
   - analyzer derivation.

3. **Cross-repository system model**
   - not merely a single codebase graph.

4. **Context compilation**
   - explicit selection reasons;
   - deterministic budget optimization;
   - reusable output independent of one agent.

5. **Context Integrity compatibility**
   - upstream evidence substrate for claims/authority/drift.

6. **Rust implementation**
   - not sufficient differentiation by itself, but valuable for learning, local distribution, concurrency, and performance.

---

# 32. Monetization Possibilities — Long Term Only

Monetization is not an early requirement.

Potential later models:

## Open Core

Free:

- local single-user engine;
- CLI;
- core parsers;
- local graph;
- context packing.

Paid:

- team/shared index;
- organization-scale cross-repo graph;
- CI history retention;
- hosted synchronization;
- governance;
- enterprise auth;
- dashboards.

## Enterprise Developer Intelligence

Possible value:

- architectural impact analysis;
- multi-repo migration planning;
- agent governance;
- context quality analytics;
- repository knowledge retention.

## Context Integrity Bundle

A commercial offering could eventually combine:

```text
Codebase Intelligence
        +
Context Integrity
```

where the intelligence engine supplies evidence and Context Integrity provides governance, claims, authority, drift, and policy evaluation.

No monetization architecture should be built until core utility is proven.

---

# 33. Key Risks

## 33.1 Reinventing Existing Tools

Risk is real because the ecosystem is active.

Mitigation:

- treat overlap as learning value;
- continuously benchmark against existing tools;
- focus differentiation on evolution, provenance, cross-repo, and integrity integration.

## 33.2 Multi-Language Complexity

Semantic code analysis becomes difficult quickly.

Mitigation:

- small language set;
- tiered relation confidence;
- accept syntactic evidence where semantic certainty is unavailable;
- add LSP/compiler backends later.

## 33.3 Premature Platform Scope

Risk:

- daemon;
- MCP;
- embeddings;
- vector DB;
- graph DB;
- web UI;
- cloud.

Mitigation:

- hard phase boundaries;
- CLI first;
- no infrastructure without demonstrated need.

## 33.4 Symbol Identity Across Refactors

Stable identity is hard.

Mitigation:

- revision-scoped identity first;
- explicit lineage heuristics later;
- do not falsely promise stability.

## 33.5 Context Ranking Becomes Subjective

Mitigation:

- explain scores;
- benchmark;
- keep multiple ranking strategies pluggable;
- compare against baselines.

## 33.6 Rust Learning Gets Buried Under Product Complexity

Mitigation:

- small Phase 0;
- sequential before concurrent;
- use real requirements to introduce advanced features.

---

# 34. Open Questions

1. What should the project be called publicly?
2. Which first two languages provide the best combination of learning value and immediate utility?
3. Should Tree-sitter be the only initial parser backend?
4. How should stable symbol IDs be represented?
5. Should the graph be persisted relationally, serialized directly, or stored in an embedded graph-like format?
6. How much call-graph precision is possible without compiler/LSP integration?
7. When should full-text search justify Tantivy?
8. What context-bundle format should become the stable external contract?
9. How should cross-repository contracts be declared vs inferred?
10. Which evidence fields are mandatory for future Context Integrity consumption?
11. Which ranking benchmark best reflects agent usefulness?
12. When does MCP become worth adding?
13. Should Context Integrity consume a local API, file export, library binding, or event/change feed?

These should not block Phase 0.

---

# 35. Immediate Next Steps

After this PRD:

1. Create `docs/architecture.md`.
   - module boundaries;
   - initial domain model;
   - parser abstraction;
   - output/versioning strategy;
   - future Context Integrity seam.

2. Create a detailed **Phase 0 implementation plan**.
   - CLI commands;
   - exact outputs;
   - crate dependencies;
   - test plan;
   - acceptance criteria.

3. Create a lighter **Phase 1 design note**.
   - parser selection;
   - first languages;
   - `Symbol` model;
   - source ranges;
   - fixtures.

4. Implement Phase 0 before over-designing later phases.

5. Revisit this PRD after Phase 2 and again after Phase 5.

---

# 36. Summary

The project should begin as a small Rust CLI and be allowed to earn its way into a platform.

The near-term project is:

> **A Rust repository scanner and structural code intelligence CLI.**

The medium-term project is:

> **A local repository graph, change-analysis engine, and task-context compiler.**

The long-term project is:

> **A local-first software-system evidence layer that helps humans and agents understand code, evolution, and impact—and provides the deterministic substrate that Context Integrity can use to detect architectural, documentation, and contextual drift.**

The most important architectural rule is therefore:

> **Build a strong model of observable software facts without prematurely owning the policy for what those facts should mean.**

That boundary lets the project remain independently useful while preserving a natural long-term integration with Context Integrity.

---

# Appendix A — Research References

Reviewed for the October 2026 PRD:

- Repomix — https://github.com/yamadashy/repomix
- Aider repository map — https://aider.chat/docs/repomap.html
- Serena — https://github.com/oraios/serena
- CodeGraphContext — https://github.com/CodeGraphContext/CodeGraphContext
- Potpie — https://github.com/potpie-ai/potpie
- ast-grep — https://github.com/ast-grep/ast-grep
- Tree-sitter — https://tree-sitter.github.io/
- Tantivy — https://github.com/quickwit-oss/tantivy

These projects are references and possible integration/inspiration sources, not product boundaries that must be avoided.
