# Phase 6 — Persistent index implementation

**Date:** 2026-10-16
**Status:** implemented and locally verified; fresh independent review required

## Baseline and authorization

Implementation started on `main` at
`28e2f8a8bf975669be0923204b4b70473c5bdecf` (`fix: address phase 5 review findings`),
matching the planning baseline. The only initial worktree change was the untracked
`docs/plans/phase-6-implementation-plan.md`; that provided plan is included in the
single delivery commit. No newer work was reset or discarded. The maintainer
explicitly requested implementation from the attached prompt and one commit.
This authorizes Phase 6 despite pending earlier reviews; it does not accept Phase 5
or alter earlier independent dispositions.

## Delivered architecture

`src/persistence.rs` provides one synchronous load/build path for `index`,
`symbols`, `symbol`, `graph`, `deps`, `callers`, `related`, `search`, `neighborhood`,
`hotspots` and `context`. `index` forces analysis and requires successful persistent
replacement before rendering success. Automatic consumers scan and fingerprint,
load compatible fresh analysis, or rebuild. Missing, stale, incompatible and invalid
stores are typed internal rebuild reasons. Cache configuration/I/O/write failure
can return fresh automatic results with sanitized stderr warnings; explicit indexing
fails with empty stdout. Unknown source freshness fails closed, including a
read-failed analysis artifact.

The production seam is `index::analyze_scan`, using the existing analyzer registry
and graph-capable per-file parse. The artifact contains `RepositoryIndex` structural
records and `FileRelations`, not a second query model. `graph::from_analysis`
reconstructs all entities, resolution, relation IDs and adjacency on every graph
request. The old fresh build helpers remain only in unit fixtures. Rendering,
querying and ranking do not access SQLite. Symbol-only cold requests now also
extract relation candidates so all consumers share one reusable artifact.

`scan`, `stats` and `tree` remain live filesystem commands. `diff` and `impact`
continue analyzing immutable Git objects independently. `context --change` uses
the current cached graph with the existing clean/current-HEAD checks, structural
mapping, committed analysis and post-content snapshot recheck.

## Storage and compatibility

Added dependencies: `rusqlite 0.38` with bundled SQLite and limits, `blake3`,
`dirs 6`, and `rustix` filesystem operations. Existing `tempfile` moved from dev
to runtime dependencies for safe sibling temporary replacement. The lockfile pins
actual resolved versions. New dependencies were fetched using the existing isolated
Rust installation after sandbox DNS initially prevented the fetch; no global
Rust configuration changed.

`src/store.rs` defines:

- SQLite schema version **1** (`PRAGMA user_version`);
- analysis compatibility **1** (`INDEX_COMPAT_VERSION`);
- fingerprint contract **`freshness-v1`**;
- native path codecs **`unix_bytes_v1`** / **`windows_wide_le_v1`**.

Metadata verifies full canonical native root identity, path codec, parse limit,
analysis version and fingerprint version/value. File, symbol, diagnostic,
extraction and candidate rows are normalized by file ordinal and foreign keys.
Each structural row has a bounded JSON record with explicit named enum tags;
unknown values fail decoding. Path fields are excluded from JSON and reconstructed
from validated BLOB paths and file foreign keys. No Rust discriminants are stored.
`Language::Other`, optional diagnostic ranges, aliases, wildcards, reference context,
derivation, extraction status and unsupported counts round-trip.

No source bodies, Git status, warnings, graph IDs/adjacency, queries, ranking results
or selected context content are persisted. `context` loads selected content live.
Native relative paths must match current sorted inventory and reject absolute,
parent/dot traversal and invalid encodings. Integer ordinals use checked conversion;
row counts, file associations, symbol counts, ranges and metadata consistency are
validated before trusted reconstruction. Current Git metadata and scan warnings
replace operational metadata on every load; native-path qualification is reapplied.

## Location, freshness and mutation

Default: `<platform-cache>/context-engine/indexes/<key>.sqlite3`.
`CONTEXT_ENGINE_CACHE_DIR=/absolute/external/path` overrides the cache base.
Relative/traversing overrides and locations inside the analyzed root are rejected.
The nearest existing ancestor is canonicalized before creating directories, so a
rejected repository-local override does not dirty the target. Locator hashing uses
length-delimited native canonical root bytes, platform codec and parse limit;
metadata checks root identity again rather than trusting the locator alone.

Each sorted discovered file contributes native relative path, language, role and
size. Supported Rust/Python source within the parse limit, excluding generated and
vendor roles, contributes raw content bytes even when invalid UTF-8. Eligible
same-length source edits invalidate reuse. Inventory changes invalidate reuse;
same-length edits to skipped bodies do not. Ignore controls influence the freshly
scanned inventory rather than being treated as parser source bodies.

Fingerprint reads share the safe bounded source loader. On Unix, descriptor-relative
opens with `NOFOLLOW` protect every path component, including parent swaps. The
existing metadata checks remain on other platforms. Fingerprinting reserves each
scanned eligible size plus one growth-probe byte, with a 256 MiB total per pass.
Unknown/unreadable eligible source fails rather than producing a fresh hit.

A cold build fingerprints scan A, analyzes that scan, then scans/fingerprints B.
Different fingerprints/root identity discard the artifact and retry once. A second
change fails without replacement. This bounds retries and never knowingly persists
a mixed generation. It does not make filesystem operations atomic or detect every
possible concurrent change-and-revert race. A cache can become stale immediately
after validation; the next request validates again.

## Transaction and resource safety

Writers create a sibling temporary DB, enable foreign keys, write all records in
one transaction, preflight aggregate sizes, commit and sync the file, then atomically
replace the destination. Failures leave the prior snapshot intact. Readers open
read-only transactions; replacement exposes a complete file rather than a partial
schema. Concurrent complete writers may replace each other; every subsequent request
still checks freshness. There is no migration or destructive recovery of unrelated
files. Old incompatible/corrupt databases are replaced with a fresh derivable artifact.

Limits include a two-second SQLite busy timeout, 128 MiB DB ceiling, SQLite 2 MiB
value limit, 1 MiB encoded record ceiling, 32 MiB aggregate encoded payload,
49,999 file/extraction rows, 50,000 symbol rows, 10,000 diagnostics and 100,000
candidate rows. Loaded graphs also enforce 50,000 total entities and 100,000 total
relations including structural facts; names, details, paths and relation text retain
the existing 8/16/4/16 MiB aggregate ceilings. These may reject artifacts that fit
one limit but exceed another. The temporary store also has a 128 MiB page ceiling.
No schema-controlled executable code or analyzed repository code is invoked.

## Local verification

Using `PATH=/tmp/context-engine-cargo/bin:$PATH`,
`CARGO_HOME=/tmp/context-engine-cargo`, and `RUSTUP_HOME=/tmp/context-engine-rustup`:

- Focused persistence, store and CLI integration targets passed.
- Final `./scripts/verify.sh` passed: **77 unit / 59 integration tests**, including
  formatting, Clippy with warnings denied, context checks and all prior suites.
- `cargo check --all-targets --offline` passed.
- `cargo build --release --offline` passed.
- Twenty release-binary smoke invocations passed with an isolated external cache.
- `git diff --check` passed, including final documentation updates.

The first full verifier run found one stale help assertion expecting the removed
“fresh in-memory” wording. It was updated to assert persistent-index help and the
complete canonical verifier rerun passed.

Warm-hit proof injects a builder that panics if invoked; a warm load succeeds, while
force/stale paths invoke the builder. Mutation injection verifies exactly two
attempts, successful single retry, and prior DB preservation after repeated change.
Freshness tests cover Rust/Python same-length edits, rename/add/remove, parse option
changes, invalid UTF-8, generated/vendor/large-file exclusions and injected byte limits.
Store tests cover metadata variants, diagnostics, `Language::Other`, unknown
schema/compat/codec/enums, foreign keys, ordinals, traversal, ranges, missing extraction,
injected aggregate payload limits, a sparse over-ceiling DB and transaction rollback.
Native non-UTF-8 paths are injected through inventory/domain records and round-tripped
through SQLite; actual creation of non-UTF-8 filenames is not claimed.

CLI tests isolate cache storage for every invocation, including existing integration
suites through `tests/support/mod.rs`. New tests compare cold/warm output across all
current consumers, check that warm calls do not replace the DB, exercise source
changes/add/delete/corrupt DB in non-Git mode, validate override/write-failure stdout
behavior, and preserve clean Git `context --change` behavior and dirty rejection.

## Release smoke results

The optimized binary indexed this repository in human format and produced repeated,
byte-identical JSON for `symbols`, `graph`, `search graph` and `context --task
"persistent index" --budget 24000`. An isolated non-Git source fixture exercised
indexing, same-length Rust and Python edits, file addition/deletion and corrupt DB
replacement. The fixture was then initialized as a clean Git repository with two
commits; forced indexing preserved cleanliness, repeated `context --change
HEAD~1..HEAD` returned identical JSON, and a dirty source edit was rejected with
empty stdout. All twenty invocations behaved as expected. The original repository's
Git status was identical before/after the smokes, and the fixture contained only
its authored source files before Git initialization. No repository-local cache or
journal appeared. Temporary smoke repositories/caches were removed afterward.

## Adaptations and remaining limits

The schema uses named JSON record payloads within normalized SQLite rows instead of
one SQL column per range/enum field. This keeps a single explicit codec per domain
record while retaining file-scoped ownership, bounded reads and the Phase 7 update
seam. Replacement uses a complete transactional sibling DB and atomic rename rather
than mutating the current DB in place, simplifying corrupt-store recovery and failed
write preservation. A sparse oversized database checks the real DB ceiling without
allocating its contents; aggregate/freshness bounds also have injected small limits.

Freshness still reads eligible source on every request; no timing/performance claim
is made. Parsing/extraction reuse is proven deterministically. The cache is local
and derivable, with no incremental mutation, watchers, workers, async, daemon,
historical caching, new resolution, policy logic, full-text search or remote sync.
Phase 7 can transactionally replace a file's analysis and dependent candidate rows
and reuse the same graph-construction boundary. No cache GC is implemented.
Cross-platform/Windows, provider/cloud CI, power-loss behavior and independent
executable/concurrent-process verification were not run. Existing non-atomic filesystem,
Git probing, parser and same-file resolution limitations remain. Fresh independent
Phase 6 review is required; earlier pending dispositions remain pending.
