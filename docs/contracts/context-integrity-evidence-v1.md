# Context Integrity evidence v1

Normative producer contract for `ctx evidence`. This is descriptive, body-free
metadata, not authority, policy, drift, claim verification or architectural intent.
The [JSON Schema](context-integrity-evidence-v1.schema.json) defines required keys,
nullability, feed variants, scalar types and enum constraints. The separate
[Python conformance consumer](../../tests/fixtures/evidence/validate.py) additionally
checks paths, identity uniqueness, references, counts and observation coherence.
Golden valid/invalid documents live beside that consumer. Metadata (including
names and paths) may itself be sensitive; this is not secret detection.

## Invocation and wire behavior

```sh
ctx evidence structure . --repository-id engine --format json
ctx evidence changes HEAD~1..HEAD . --repository-id engine --format json
ctx evidence candidates . --repository-id engine --task 'indexing' --limit 50 --format json
ctx evidence candidates . --repository-id engine --task 'indexing' --change HEAD~1..HEAD --format json
ctx evidence workspace --manifest ./workspace.json --format json
ctx evidence workspace --manifest ./workspace.json --repo engine --change HEAD~1..HEAD --depth 2 --format json
```

JSON is the only format and the default. Single-root IDs are required and match
`[a-z][a-z0-9_-]{0,63}`; they name the caller's selected scan scope, not a physical
location. Workspace IDs come from the validated Phase 10 manifest. No enrollment
or network access occurs. Analysis may reconcile the existing external cache;
the analyzed roots are read-only.

Success writes one complete pretty JSON document followed by LF. **8,388,608
bytes including LF and JSON escaping** is the hard maximum. Serialization is
bounded into memory before writing stdout. Overflow, invalid identity, invalid
reference, analysis or Git failures return nonzero with empty stdout. Evidence
errors use a fixed diagnostic on stderr, withholding lower-layer raw messages and
physical paths. OS write failures after writing begins cannot retract bytes.
No output files are written. Lower scanner/graph/change/ranking limits apply:
50,000 graph entities, 100,000 relations, 16 MiB graph text, and lower parser,
Git and freshness bounds. Identity lookup is linear or ordered-map based;
there is no all-pairs reference validation. Output overflow fails the entire feed
rather than silently trimming facts. Candidate output accepts limits 1–200.

## Common envelope

All keys shown in the schema's `required` arrays must appear, including nullable
keys. `evidence_schema_version` is integer 1. `feed` discriminates the exact
`data` type: `structure`, `changes`, `candidates` or `workspace`.

`producer` names `context-engine`, its actual Cargo package version,
`analysis_compat_version: 1`, `identity_version: evidence-id-v1`, and
`parser_version: null`. There is no runtime parser-version metadata available;
no grammar version is invented. Actual parser dependencies remain in Cargo.lock.
`subject` is either `{kind: repository, repository_id}` or
`{kind: workspace, workspace_id}`.

`observation` always includes basis, scope, atomic, git_head, git_dirty, base and
head. `atomic` is false in every feed. HEAD/dirty values are nullable and never
certify analyzed content. Current feeds use `best_effort_worktree`; base/head are
null. Changes use `committed_changed_pairs`, actual immutable base/head Git object
IDs and null current HEAD/dirty. Workspace uses `mixed_time_workspace` even for
inspection: independent current repository observations are not synchronized.
Optional impact adds committed provider base/head alongside current relationship
and consumer observations, retaining their separate Phase 10 time basis.

`coverage.complete` is always false: excluded scanner paths, syntax limitations
and best-effort observations preclude a global completeness assertion. `counts`
is a sparse map of nonnegative integers; absent counters mean zero. `limitations`
is an ordered array of machine codes, not arbitrary diagnostics. It currently
uses `scanner_exclusions_apply`, `syntax_not_compiler_binding`,
`raw_diagnostics_withheld`, `best_effort_non_atomic`, `changed_pairs_only`,
`git_rename_heuristic`, `no_cross_refactor_lineage`,
`ranked_metadata_not_content_selection`, `mixed_time_not_synchronized`, and
`candidate_consumers_only`, as applicable. Unknown codes must be treated as
additional limitations rather than proof of completeness.

## Exact paths and identities

Paths are exact UTF-8 POSIX relative paths: nonempty, <=4096 UTF-8 bytes,
components <=255 bytes, no empty/dot/parent components, backslash, colon, control
characters, surrogate or replacement characters. Absolute paths are forbidden.
Nonrepresentable structure paths and their dependent facts are omitted and
counted; essential changed Git sides or workspace paths fail closed. Names are
syntax metadata, not path identities. No lossy string is an identity input.

Every structural reference is a 64-character lowercase BLAKE3 hex digest.
Define `H(domain, fields...)`: hash UTF-8 strings
`context-engine-evidence-id-v1`, `domain`, then every field in order; prefix
each with its byte length as an unsigned 64-bit little-endian integer.
No delimiters, native roots, cache paths or graph numeric IDs are included.

- Worktree scope is an opaque invocation nonce `H(observation, internal_nonce_inputs...)`.
  Its inputs are local clock/process/sequence entropy, never published raw. This
  is only a namespace: no timestamp, source digest, persisted generation,
  authenticity or atomicity claim. Separate invocations intentionally get separate
  references, even if source is unchanged. Compare descriptors after normalizing
  scope/references when comparing current results.
- Revision scope is `H(revision_scope, commit_oid, scope_prefix_or_empty)`.
  `scope_prefix` is the exact Git-root-relative scan directory, nullable for root.
  The changes envelope scope is `H(comparison, base_scope, head_scope)`.
- Repository reference: `H(repository, logical_repo, scope)`.
- File reference: `H(file, logical_repo, scope, path)`.
- Symbol reference: `H(symbol, logical_repo, scope, path, kind, name,
  qualified_name, compact_range_json)`. Range JSON uses `start`, `end` in that
  order, with points ordered `byte`, `row`, `column`, decimal integers, no spaces.
- Relation descriptor: `H(relation_descriptor, logical_repo, scope,
  compact_relation_json)`. The JSON is the schema's Relation record with an empty
  `id`; field order is id, source_ref, target_ref, unresolved_name, kind,
  resolution, metadata, path, range, language, derivation. Metadata order is kind,
  then binding/wildcard or context. Strings use serde_json compact JSON escaping.
  Relation reference is `H(relation, descriptor, occurrence_ordinal_decimal)`.
  Ordinal starts at zero per identical descriptor in canonical graph order.

All IDs must be unique within a projected graph. Every source and resolved target
must resolve to an exported repository/file/symbol; symbols must refer to their
own exported file. Dangling references and collisions fail. Repeated relation
occurrences are preserved with ordinal disambiguation. IDs do not establish
continuity across refactors, renames, observations or differing revisions.
Workspace relationship IDs remain in the distinct Phase 10 namespace, unchanged
from workspace JSON v2 (including contract kind); they are not graph references.

## Structure

`data` contains repository_ref, files, symbols and relations, ordered by the
canonical graph inventory (native path order; symbols by path, range, kind and
qualified name; relations by canonical graph order). Omissions retain that order.
All covered files appear, including unsupported/skipped/invalid files and files
with no relations. Each File has id, path, language, role, analysis_status,
syntax_errors, nullable relation_status, unsupported_relation_candidates.
Language/role use the engine's display names (`Rust`, `Python`, `Source`, `Test`,
etc.). Language labels are descriptive strings, not parser availability claims.

Symbol includes id, file_ref, path, language, kind, exact local/qualified names,
range, name_range and `derivation: parser_definition`. Points are UTF-8 byte
offsets, zero-based row and **byte** column; ranges are half-open [start,end).
Kinds: module, class, struct, enum, union, trait, function, method, type_alias,
constant, static. Files with recovered syntax errors may retain partial facts.

Relation includes id, source_ref, nullable target_ref/unresolved_name, kind,
resolution, typed metadata, path, nullable range/language, derivation. Exactly one
of target_ref/unresolved_name is nonnull. Kinds: contains, imports, references.
Resolution: direct_structural, exact_same_file_qualified_name, unresolved.
Derivation: repository_contains_file, lexical_range_containment, rust_use_syntax,
python_import_syntax, rust_call_syntax, python_call_syntax. Same-file call matches
are syntactic qualified-name heuristics, never compiler binding. Imports remain
unresolved names. Metadata is tagged none, import (nullable binding, wildcard),
or reference (context call).

Analysis statuses: analyzed, unsupported_language, skipped_role, too_large,
invalid_utf8, read_failed, parse_failed, analysis_limit_exceeded. Relation status
is null when extraction was not performed, otherwise analyzed, extraction_failed
or analysis_limit_exceeded. Null does not mean zero relations were found.

Coverage counts include files_observed, files_exported, symbols_exported,
relations_exported, analysis_STATUS, relations_STATUS, syntax_errors,
unsupported_relation_candidates, unresolved_names, diagnostics_withheld,
scan_warnings_withheld, reconciliation_warnings_withheld and
unrepresentable_{files,symbols,relations}_omitted. files_observed equals
files_exported + unrepresentable_files_omitted. Status totals count observed
files, including those omitted for path representation. Raw diagnostic messages
and scanner warning paths are withheld.

## Changes

`data` contains separate base/head structures, their base_coverage/head_coverage,
files and nullable scope_prefix. These graphs cover changed pairs only. They are
built from immutable Git objects, never dirty checkout source. Every changed file
retains status, nullable similarity (Git rename evidence), nullable before/after
sides with exact path/mode/object_id, symbols, relations, ambiguous_keys,
comparison_available and relation_comparison_available. Missing sides mean
add/delete or out-of-scope rename sides, not missing current files.

Symbol deltas have kind added/removed/modified and nullable before_ref/after_ref
pointing into their own revision's exported symbols. Unchanged symbols do not
appear as deltas. Ambiguity or unavailable comparison suppresses unsupported
conclusions. Relation deltas preserve signature source/target descriptors
(state/kind/name), kind, resolution, metadata, derivation and before/after
occurrences (count, bounded example ranges, examples_omitted). Their endpoints
are comparison signature descriptors, not structural foreign keys. Occurrence
count equals examples length + examples_omitted. Symbol/body matching and relation
multiset comparison retain the existing Phase 4 rules, including ambiguity and
range-shift behavior; this feed promises no symbol-rename lineage.

Top coverage counts changed_files, ambiguous_symbol_keys,
unavailable_symbol_comparisons and unavailable_relation_comparisons. Side coverage
retains every structure counter. A malformed/unresolvable range or essential
unsafe/native Git path rejects the entire response; no positive completeness
assertion replaces it.

## Candidates

The shared Task parser bounds input to 4096 bytes and 64 normalized terms. The
feed never echoes raw task/terms: task_digest is
`H(normalized_task, normalized_phrase)` and may permit dictionary inference.
`data` includes ranker_version file-ranker-v1, its exact sort_order, semantics
ranked_metadata_not_content_selection, task_digest, fallback, nullable change,
and candidates. It calls Task::parse and context::rank, never context::select or
the full-file selection loader. Parsing/freshness reads are still required to
build the graph; body-free does not mean zero analysis reads.

Each candidate contains original rank (1-based, possibly gaps after path
omissions), file metadata, score, sorted unique reasons, matched_symbols,
matched_symbols_omitted and nullable graph/change distances. Matched symbols have
the same reference/provenance rules as structure. No body, content size, selected
byte-budget or token claim is present. Candidates are sorted by descending total,
symbol, path, change, graph, centrality scores, then ascending native path. Role
is included in total. The existing ranker-v1 contract owns score constants and
reason vocabulary in `src/context.rs`; this export does not rerank them. It omits
conventional sensitive filenames and generated/vendor roles using that ranker.

Coverage retains observed analysis counts and adds eligible_files,
excluded_role_files, sensitive_files_omitted, task_terms, task_terms_truncated
(0/1), ranking_roots, ranking_roots_omitted, graph_truncated (0/1),
fallback_candidates_omitted, ranking_candidates, candidates_exported,
candidate_limit_omitted and unrepresentable_candidates_omitted. Output cardinality
plus limit/path omissions equals ranking_candidates. Projection omission counters
describe the analyzed inventory; files/symbols/relations_exported counters are
absent because this feed exports only candidate facts. Ranking omissions,
fallback omissions and incompleteness are not zero evidence assertions.

Optional change carries actual base/head and existing observed/mapped/unmapped,
ambiguous, unavailable-comparison and deleted-only counts, with no raw requested
range. Existing clean matching HEAD, hidden-index/submodule and pre/post snapshot
checks apply; dirty/mismatched/unknown incompatible states fail closed.

## Workspace

`data` reuses the explicit Phase 10 output DTO (workspace_schema_version 2,
manifest_schema_version 1), with no root disclosure. The exact existing inspection
or explicitly requested impact payload is preserved: registered logical repos,
HEAD/dirty/unknown observations, manifest fingerprint, typed relations and gaps.
Observed Cargo dependencies and declared API/event mappings retain different
kind/basis/validation, source locators/fingerprints and contract kinds.

Impact retains actual base/head, typed trigger omissions, exact vs coarse reasons,
one representative witness chain, alternative/depth omissions, at most 64 direct
trigger relation IDs per candidate and excess counts. Witness edge indices address
this data.relations vector; direct trigger IDs address its relationship IDs.
Provider and consumer observations remain mixed-time. Neither relationships nor
candidate consumers prove runtime use, compatibility or breakage. Manifest bytes
fingerprints may change on reorder/relocation; logical relationship IDs do not.
Top coverage adds workspace_gaps; detailed typed gaps/trigger limits remain in data.

## Compatibility and evolution

| Contract | Version | Phase 11 effect |
| --- | --- | --- |
| Evidence | 1 | New opt-in namespace |
| graph/context/diff and existing CLI | schema_version 1 | Unchanged |
| Workspace | workspace_schema_version 2 / manifest 1 | Unchanged, reused |
| SQLite / analysis / freshness | 3 / 1 / freshness-v2 | Unchanged |
| HTTP/MCP vocabulary | Phase 9 | Unchanged, no evidence routes/tools |

Consumers reject unknown evidence major versions and duplicate JSON keys. They
may ignore unknown extra fields, but must retain unknown limitations and handle
unknown count categories conservatively. Additive optional/nullable fields or new
coverage codes may be v1 additions; changing required fields, null meanings,
identity derivation, enums, multiplicity, ranking semantics or provenance requires
v2. JSON Schema alone cannot prove semantic reference integrity; validate both.
The existing Python Context Integrity provider continues to use ctx graph/context
schema v1. No production consumer migration is included or implied.
