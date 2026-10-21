# Phase 8 Review Fix Report

**Date:** 2026-10-21
**Status:** review findings addressed; canonical verification blocked; fresh independent review required

## Changes

- Replaced the unqualified `introduced` history event with `path_lineage_start`.
  Events identify `tracked_file_first_parent_path` as the evidence scope and mark
  cross-file origin as unknown. The result reports why traversal stopped and does
  not claim global symbol introduction.
- Versioned history/evolution JSON as schema v2. Structural events distinguish
  available syntax comparison from complete symbol-delta accounting. Totals are
  named as observed deltas and expose completeness, incomplete-file counts and
  ambiguous keys omitted.
- Added lossless native path identities alongside display paths: Unix path bytes
  are lowercase hex with encoding `unix_bytes`; Windows wide strings are encoded
  as little-endian UTF-16 bytes with encoding `windows_utf16le`. Other platforms
  emit a null identity rather than imply a portable encoding.
- Replaced the inferred history truncation flag with an explicit stop reason.
  A path-lineage start or coverage gap at the final allowed commit leaves
  `history_truncated` false; an unresolved lineage stopped by the commit window
  reports `window_limit` and remains truncated.
- HEAD lookup now separates supported Rust/Python analysis gaps from expected
  unsupported-language or skipped-role exclusions. Whole supported-inventory
  lookup fails closed when an eligible source file is incomplete; `--file` remains
  usable when its selected file is fully analyzed.
- Git tree entries retain their known blob sizes as `Some(size)`, including zero;
  raw diff entries carry an unknown size. Shared snapshot construction asks Git for
  a size only for unknown regular entries, so full HEAD inventories reuse
  `ls-tree -l` sizes.
- Corrected README future-direction wording and updated the architecture and current
  state contracts. The initial implementation report remains historical evidence.

## Focused regression coverage added

- File move with substantial rewrite producing an unpaired addition, copied symbol,
  move into an existing file, and first-parent root all report path-scoped origin.
- Path-lineage, source-coverage and tracked-descriptor ambiguity stops at the exact
  end of a commit window do not report window truncation; continuing lineage at
  the window limit still does.
- Duplicate unrelated symbol groups produce partial observed totals while a unique
  tracked descriptor continues.
- An oversized Rust/Python source blocks whole-inventory HEAD uniqueness, while a
  fully analyzed `--file` selection succeeds; README is counted as an expected
  exclusion rather than an analysis gap.
- Two Unix non-UTF-8 paths with the same lossy display retain distinct identities
  in event entries and co-change candidates.
- A counted Git wrapper verifies that a 24-file committed HEAD inventory reuses
  `ls-tree -l` sizes and issues only the targeted size query for the changed raw
  diff entry.

## Verification

- `cargo fmt --all -- --check` — passed.
- `./scripts/check-context.sh` — passed.
- `git diff --check` — passed.
- `./scripts/verify.sh` — attempted, but Cargo could not complete dependency
  resolution. The default Cargo home tried to reach crates.io and DNS resolution
  failed. Retrying with the Phase 8 report's isolated Cargo/Rustup paths attempted
  an unavailable Rustup channel sync. Offline retries found missing registry index
  metadata (`dirs` / `blake3`). Therefore the new Rust code has not been compiled,
  and tests or Clippy are not claimed as passed.

No cross-platform execution or independent acceptance is claimed. This change does
not alter the status of Phase 6, Phase 7 or earlier pending reviews.
