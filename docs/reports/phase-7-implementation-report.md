# Phase 7 implementation report

**Date:** 2026-10-16
**Status:** implemented and locally verified; fresh independent review required

## Baseline and authorization

Started at `5e3ae18d1510501e177aac2d9021fc01d8534435`
(`fix: address phase 6 review findings`). The only initial worktree entry was the
user-provided, untracked `docs/plans/phase-7-implementation-plan.md`; its content is preserved (with trailing-whitespace normalization)
and included in this implementation commit. No newer commits or user edits were
reset/discarded. The maintainer directly requested Phase 7 implementation and one
commit, authorizing progression despite pending Phase 6 fresh re-review and earlier
phase dispositions. This does not accept those phases.

The end state is the single implementation commit containing this report, directly
on that baseline; its identity is available from `git log -1`. No push is performed.
The final deliverable includes production changes, tests, the supplied plan and
updates to README/current-state/architecture; there is no second status/handoff file.

## Contracts and file inputs

- SQLite schema **3** replaces schema 2 without migration. Analysis compatibility
  remains **1**: scanner/classifier/parser/extractor/enum semantics are unchanged.
- Freshness is **`freshness-v2`**. Each discovered file gets a BLAKE3 stamp over
  length-delimited freshness version, native-path codec, compatibility, parse limit,
  lossless native relative path, serialized language, role, size and eligible raw
  Rust/Python bytes. Fixed integers use little-endian encoding. Generated/vendor,
  unsupported and over-limit bodies remain excluded; their inventory facts still
  participate. Invalid UTF-8 is hashed as raw bytes. Mtime is never freshness proof.
- The repository generation hashes version/codec/compatibility/options and the
  canonical native-path-sorted map of path/stamp pairs. Root identity is validated
  separately in cache metadata. File input records persist only size and digest.
- Integrity **`blake3-rows-v2`** covers ordered keys, native paths, file input records,
  child associations/ordinals and structural payload bytes. Loaded generations are
  recomputed from their stamps. This detects corruption, not an attacker able to
  rewrite both data and digest. No source bodies, graph IDs, adjacency, query or
  ranking results are persisted. Path-derived store keys remain internal.

## Shared analysis and reconciliation

`index::analyze_file` returns a plain file-owned unit containing analysis status,
symbols, diagnostics and optional extracted relations. Full builds and changed-file
batches call this same primitive with one analyzer registry per batch. There is no
second parser/status/diagnostic implementation. `AnalysisTotals` provides linear
aggregate accounting during full accumulation and validation after incremental merge.

`store::load_snapshot` no longer requires the current inventory/generation. It still
validates schema, analysis/options/codec/root identity, bounded DB/row/payload/path
sizes, integrity, native paths, enums, source ranges against persisted sizes, ownership,
foreign keys, file-local ordinals and aggregate structural limits. An untrusted or
incompatible snapshot is fully rebuilt.

The reconciler compares current inputs with the trusted snapshot by native path.
Unchanged units are retained; changed/new units use the canonical analyzer; absent
paths and their evidence disappear. Renames are delete-plus-add. The merged artifact
passes the same symbol/name/detail/diagnostic/entity/relation limits as full analysis.
A complete second scan/input pass covers **all** eligible files, including those
initially unchanged. A mutation discards the candidate and retries once; a second
mutation fails without committing it. Filesystem reads remain non-atomic and do not
protect against arbitrary transient changes that revert between observations.

`index` forces full analysis and committed full-store repair. All other current-state
intelligence commands automatically reconcile incrementally. Scan/stats/tree remain
live, diff/impact retain immutable committed objects, and context-change gates remain
unchanged. Every current graph still comes from `graph::from_analysis`, so removed
symbols/candidates cannot leave stale adjacency. No cross-file resolver changes ship.

## Transaction and failure semantics

`store::patch` opens an IMMEDIATE SQLite transaction, validates the existing snapshot
under the write reservation, and compares its generation with the expected base.
A mismatch returns typed `StoreError::Conflict`; reconciliation reloads/retries once.
A second conflict fails rather than applying a stale patch.

For removed/replaced paths, candidates/extractions/symbols/diagnostics are deleted
before the file owner. Only changed/new ownership groups are inserted; unrelated
rows and ordinals are untouched. Whole-store preflight, updated input stamps,
recomputed artifact integrity/generation and final structural validation precede
atomic commit. Serialization, constraint, bound or I/O failure rolls back. Full
repair retains the Phase 6 synced sibling-file atomic replacement path; this is
optimistic incremental-writer coordination, not a daemon/global lock protocol.

Unknown source freshness never authorizes cached reuse. Automatic queries return
uncached live best-effort analysis with per-file failures; explicit index fails.
If the post-pass cannot establish freshness, automatic reconciliation rebuilds live
rather than returning retained cached units that are now unverified. These results
are never persisted. Automatic patch failure can return the fresh in-memory artifact
and a structured warning; the CLI emits it only on stderr. Watch reports failures
and leaves the committed generation intact. Persistence/reconciliation do not print.

## Foreground watcher

The only new direct dependency is `notify = "8"` (locked **8.2.0**), providing native
recursive events without an application async runtime. `watch.rs` holds the backend,
a standard one-slot synchronous channel, and narrow shared atomic dirty/resync/error
flags. Callback work only classifies access/rescan/error hints and signals; it never
reads source, scans, parses, accesses SQLite, uses event paths as evidence or renders.
The backend owns its platform event threads. One foreground CLI owner performs all
reconciliation and discards each artifact after the update; no shared mutable graph
or parser pool is introduced.

Registration precedes initial reconciliation to retain startup edits. A bounded
100 ms coalescing window prevents continuous events from postponing work forever.
Overflow, backend rescan hints and errors cause a deterministic full inventory/stamp
pass. Activity arriving during work remains pending. Read-only access events do not
cause self-trigger loops. Error details/paths are not retained in the callback queue.

`ctx watch [PATH] [--max-parse-file-size BYTES]` runs until normal process termination
or fatal watcher setup failure. Startup/current/warning messages are human stderr
only; stdout is empty and `--format json` is deliberately unsupported. Failed updates
warn and continue. No signal-handling framework or event-stream contract is added.

## Bounds and limitations

Existing scanner/source/graph/output safety boundaries remain. Freshness reserves at
most **256 MiB per pass**, including growth probes, and still reads eligible bytes
on each one-shot request. Store bounds remain **128 MiB DB**, **32 MiB aggregate
encoded payload** (now including inputs), **1 MiB per record**, **4 MiB native paths**,
plus existing record and structural ceilings. SQLite busy timeout is two seconds;
patches enforce its page ceiling. Retained plus changed analysis is bounded and
validated before query use. No analyzed repository code executes.

No cache GC/compaction, persisted event log, atomic filesystem snapshot or guaranteed
recovery of an OS watcher after its watched root is replaced is promised. Backend
errors/rescan hints cause reconciliation; event delivery still depends on the host.
Repeated SQLite churn can exhaust its ceiling and require explicit index repair.
Concurrent full repair still replaces the derivative store; incremental CAS applies
to the SQLite generation it opens. A subsequent request always revalidates current
inputs. Full graph reconstruction, sequential parsers and whole-inventory freshness
reads are intentional Phase 7 tradeoffs.

## Verification and evidence

Used the existing isolated macOS environment without changing global toolchain setup:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
./scripts/verify.sh
cargo check --all-targets
cargo build --release
git diff --check
```

Baseline verifier passed **82 unit / 59 integration tests**. Final verifier passed
**93 unit / 61 integration tests**, context checks, formatting and Clippy with warnings
denied. All-target check, optimized release build and whitespace check passed.
Focused tests were run while editing; intermediate assertion/Clippy failures were
corrected before the final gate.

New/adapted deterministic evidence covers:

- instrumented source/analyzer-entry work: warm zero work, one edited/added file,
  deletion without unrelated work, rename/new ownership, skipped/unsupported/large
  files and transitions through the parse-size threshold;
- full/incremental symbol and graph equivalence after every delta, representative
  symbols/graph/search/neighborhood/hotspots/context CLI equivalence, and stale
  symbol/diagnostic/candidate deletion in SQLite and public outputs;
- a trigger forbidding deletion of an unchanged file, proving file-local writes;
- retained-plus-changed aggregate limits with injected small ceilings;
- mutation of the changed file and a second initially unchanged file, bounded
  retry, stable retry success and preservation of the prior committed bytes;
- stale-writer typed conflict, reconciliation reload/warm-hit after another writer,
  rollback after child deletion/insertion, and fresh-result fallback on patch failure;
- schema-2 rejection/recovery, compatibility and digest/row/stamp/size corruption,
  unknown-freshness fallback and post-failure live reload, native path codecs and
  native-path patching (injected records); the guarded real native-name test retains
  its existing filesystem-rejection skip;
- deterministic callback/coalescing/overflow/error/access tests, events during work,
  ambiguous event paths with final analysis from the actual scan, and watch CLI bounds.

Release smokes used `python3 /tmp/context-phase7-smoke.py` and isolated external cache
directories. **19 successful one-shot invocations plus one foreground watcher**
covered force index, warm no-write reuse, edit/add/remove/rename, graph/search/
neighborhood/hotspots/context, corrupt-cache repair, schema-2 repair, Git cleanliness
and cache externality. The watcher started, committed a changed symbol plus a
create/rename/delete burst, and was terminated normally. SQLite was inspected for
that final state **before** the post-watch one-shot query; watch stdout was zero
bytes. Four invocations scanned/indexed/queried this real Rust repository.

The sandboxed native watcher smoke initially received no events. The same smoke
passed outside the sandbox with approved local execution on macOS. This is local
platform evidence, not a timing-based parser-work claim. No Linux/Windows watcher,
cloud/provider, cross-platform CI or independent executable verification was run.

## Deviations and remaining phases

The implementation follows the plan's file-owned evidence/graph reconstruction
architecture. Inputs are an explicit bounded JSON record in each file row rather
than a raw digest column, allowing stale range validation from persisted sizes.
Warnings moved to structured reconciliation results and CLI rendering. Watch
registration deliberately precedes initial reconciliation to close the startup gap.
The one-slot channel coalesces repository-wide hints instead of retaining file events;
this preserves correctness because every trigger scans authoritative state.

Phase 8 history/evolution, Phase 9 async service and adapters, Phase 10 cross-repository
intelligence, and Phase 11 Context Integrity evidence contracts remain future work.
Independent Phase 7 review and prior pending dispositions remain required.
