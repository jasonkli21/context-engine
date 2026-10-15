# Phase 2 review fixes

**Date:** 2026-10-14  
**Baseline:** `16f7088`  
**Review:** [Independent Phase 2 review](../reviews/phase-2-independent-review.md)  
**Disposition:** all seven findings addressed; local verification complete;
independent final disposition pending. This does not authorize Phase 3.

## Findings and changes

| Finding | Disposition |
| --- | --- |
| CE-P2-001 | Canonical discovery and ignore-free preflight now enforce entry/path work limits during traversal. Warning retention, ignore-control content and aggregate line-count I/O are bounded. Fatal overflow returns no partial repository. |
| CE-P2-002 | Both symbol-only and graph analysis check shared repository symbol/name/path/diagnostic limits before extending retained vectors. Overflow is a repository error, not a per-file parse failure. Index JSON also has an output byte ceiling. |
| CE-P2-003 | Root matching retains at most 128 canonical roots plus one temporary overflow sentinel, without allocating the full match set. `roots_truncated` distinguishes omitted root groups from each returned group's traversal flag; overall `truncated` includes either cause. |
| CE-P2-004 | Graph analysis checks aggregate candidate target/alias and repeated evidence-path bytes before retaining each file. Construction rechecks the complete text budget, including containment paths. Record caps are reduced; full graph/query JSON serialization is bounded after escaping and before stdout. |
| CE-P2-005 | Removed file-wide unique-local reference resolution. Only unique exact same-file qualified-name matches resolve; bare method/nested names remain unresolved. Query root lookup retains qualified-first/local-fallback behavior. |
| CE-P2-006 | Call evidence uses the callee node, excluding call parentheses and arguments, for both Rust and Python. Supported syntax forms are unchanged. |
| CE-P2-007 | Grouped Rust `self` denotes the accumulated group prefix, including aliases and nested groups. Standalone `self::x`, `super::x` and `crate::x::Y` retain their segments. |

## Limits and failure semantics

Production constants remain internal; small injected-limit tests exercise boundaries.
These limits supplement the existing independent per-file limits.

| Layer | Aggregate ceiling |
| --- | --- |
| Discovery and preflight, independently | 50,000 non-root entries; 4 MiB relative-path bytes, including symlinks and ignored preflight entries |
| Scanner warnings | 1,000 records; 1 MiB message/path bytes |
| Ignore-control preflight | 1 MiB file-size accounting across consulted ancestor candidates and discovered ignore files |
| Line counting | 256 MiB reserved I/O, including one growth-detection byte per counted file |
| Repository index | 50,000 symbols; 8 MiB local/qualified-name bytes; 16 MiB symbol-path/diagnostic detail bytes; 10,000 detailed diagnostics |
| Graph | 50,000 entities; 100,000 relations; 16 MiB target/alias and repeated evidence-path bytes |
| Query | 128 roots; existing shared 10,000 visited entity occurrences / 25,000 emitted relation occurrences; maximum depth 8 |
| Index and graph JSON | 64 MiB serialized UTF-8 bytes before the CLI's trailing newline |

Count ceilings now bound fixed record overhead to tens of thousands of entities
and at most one hundred thousand relations, rather than millions. Text limits
also account for paths repeated per symbol/relation, so one deep path cannot
multiply unchecked through graph evidence. Per-file parse/extraction allocations
remain bounded separately; a file is checked before joining repository retention.
The JSON writer bounds the serialized buffer, including escaping and whitespace,
instead of allocating an arbitrarily large pretty-printed string. DTO preparation
still uses bounded in-memory vectors; this is not a streaming/pagination redesign.

Discovery includes ignored content in its safety preflight. A very large ignored
build tree can therefore cause a clear fatal limit error; it is not silently
excluded from safety accounting. Ignore-control metadata checks remain subject to
the documented filesystem mutation/TOCTOU limitations.

Fatal scanner/index/graph/output overflow returns an error before stdout receives
results. Per-file analysis/relation overflow still produces the established
honest file status, with that file's affected facts discarded. Query omissions
are explicitly truncated results rather than fatal incomplete repositories.

## Validation actually run

Used the existing isolated aarch64 macOS Rust toolchain and cached dependencies:

```bash
export PATH=/tmp/context-engine-cargo/bin:$PATH
export CARGO_HOME=/tmp/context-engine-cargo
export RUSTUP_HOME=/tmp/context-engine-rustup
export CARGO_NET_OFFLINE=true
./scripts/verify.sh
cargo check --all-targets
cargo build --release
```

- Canonical verifier passed: context checks, formatting, warnings-denied Clippy,
  41 unit tests and 33 integration tests (17 scanner, 7 symbols, 9 graph).
- Injected tests cover exact/over discovery entries/paths, ignored preflight work,
  warning/detail/ignore/line budgets, repository symbols/names/details/diagnostics,
  cross-file relation bytes, root retention/filtering and escaped JSON bytes.
- CLI regressions verify fatal scan overflow produces empty stdout for every
  command, and production root overflow yields 128 deterministic groups with
  explicit truncation for all three graph queries.
- Rust/Python regressions verify unresolved unqualified methods/nested declarations,
  preserved exact matches, exact callee spans, and grouped `self` alias normalization.
- All-target check and optimized release build passed.
- 28 release smoke invocations passed: human/JSON `scan`, `index`, `symbols`,
  `graph`, `deps`, `callers`, and `related` against this Rust repository and a
  temporary mixed Rust/Python fixture. Repository query root: `scan_repository`;
  fixture query root: `helper`. Fixture JSON assertions check grouped imports,
  unresolved bare methods and callee evidence spans.
- `git diff --check` passed.

The original independent review is preserved without changing its verdict. No
cloud/provider checks, cross-platform CI, independent executable verification or
final Phase 2 acceptance is claimed. Git subprocess limits and stronger filesystem
snapshot guarantees remain future considerations from the review.
