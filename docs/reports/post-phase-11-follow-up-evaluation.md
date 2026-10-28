# Post-Phase-11 follow-up evaluation

Date: 2026-10-28. Repository: Context Engine only.

Input: `/Users/jasonkli/Downloads/context-engine-post-phase-11-codex-handoff.md`,
targeting `bfdc98bc4aedd5e7d4a77ba5293e59a3530cdf47`. The handoff was treated as
conditional scope and evidence to verify against the repository, not as authority
to begin Phase 12 or change the Context Integrity consumer. The pre-existing
untracked Phase 10 and Phase 11 plans were preserved. No production code,
dependencies or remote services were changed; this report and its current-state
entry are the only retained edits.

## Summary

- Follow-up A remains applicable. `source::open_checked` fails closed on non-Unix
  after metadata checks, so supported source content and line counts are
  unavailable there. This is a support limitation, not evidence that the current
  branch reads outside the root. A bounded Windows design and native acceptance
  proposal are below; production reader work remains unauthorized by the handoff.
- Follow-up B is reproducible. An unused global clean/process filter makes
  `git_dirty` null, and an applicable repository filter also makes it null. Neither
  filter ran in either fixture. The current broad guard skips status for filter
  configuration observed by its probe and remains the conservative policy.
  A local prototype used Git's `check-attr` to resolve static attribute sources,
  but it was reverted: the path inventory, attribute resolution, config query and
  later `git status` are separate observations. A concurrent path, attribute or
  config change can invalidate the preflight and let `git status` start a filter.
  Without a way to pin those inputs for status, the prototype does not preserve the
  filter non-execution boundary under races. No production change is recommended
  until that boundary can be maintained.

No evidence supports disabling the filter guard or reporting a false-clean value.
No filter ran in the reproduced fixtures. No compatibility or wire version
changed.

## Follow-up A — non-Unix checked reads

### Verified behavior and threat model

At this revision, `src/source.rs::open_checked` checks each path component with
`symlink_metadata`, then returns `AnalysisStatus::ReadFailed` on `cfg(not(unix))`.
It does not call `File::open` there. Unix opens the canonical root and each path
component relative to held directory descriptors with `NOFOLLOW`, then validates
the opened final handle. Scanner source reads, line counts, ignore controls and
workspace declaration checks share this acquisition path.

The Windows race that motivated the fail-closed branch is still relevant to any
future reader: an ordinary path open can follow a final symlink or a reparse point
installed after metadata inspection. Protecting only the final component is not
enough because an ancestor junction, mount point or other reparse point can
redirect path traversal. Opening a handle-relative child and checking the opened
handle avoids relying on a separate path metadata/open pair. Root ancestry and
unsupported root forms (including volume and UNC handling) also need an explicit
policy.

The existing Unix containment is path/reparse containment; hard links are not
reparse points and are not rejected by the current reader. If the desired boundary
also forbids hard-linked files whose content originated elsewhere, that changes
the threat model and needs separate approval.

### Bounded design and acceptance proposal

1. Keep all acquisition in `source::open_checked`. On Windows, open the canonical
   root and walk every root and relative component by held handles using a
   reviewed handle-relative API that does not traverse reparse points. Obtain type,
   reparse tag and size from the opened handles. Reject any reparse component and
   any final handle that is not a regular file. Do not use metadata-then-ordinary
   `File::open` as a fallback. If a supported safe handle-relative API cannot be
   selected, retain fail-closed behavior.
2. Specify supported Windows roots before implementation. The first version may
   fail closed for unreviewed volume/UNC forms; it must not silently route those
   forms through ordinary path opens.
3. Run deterministic native Windows swap tests after all path checks and before
   acquisition. Cover a final-file symlink/reparse swap, each relative parent
   replaced with a junction, and a reparse point in a root ancestor. Use a distinct
   outside sentinel and assert its bytes are never returned. Stable in-root files
   must still read successfully.
4. Exercise the same acquisition through an analyzed source file, a line-counted
   file, root and nested `.ignore`/`.gitignore` controls, and workspace declaration
   validation. A raced counted file must keep an unknown count and warning; an
   unsafe consulted control must fail closed with empty CLI stdout.
5. Execute the tests on a native Windows runner with symlink/junction creation
   enabled, then run `cargo test --bin ctx source::tests -- --nocapture`, the
   relevant scanner/CLI integration tests, and `bash scripts/verify.sh`. Record the
   Windows edition/runner, filesystem and exact test results. A macOS or Linux run
   does not satisfy this acceptance gate.
6. If the reader changes which source bytes are analyzed, review and bump
   `INDEX_COMPAT_VERSION` from 2 (or document why a platform-specific compatibility
   discriminator is sufficient). No SQLite layout change is implied, so
   `SCHEMA_VERSION` remains 3 unless storage fields change. Evidence schema v1 can
   remain wire version 1, but its producer compatibility validator and contract
   must accept the new analysis compatibility value.

This proposal needs maintainer approval for platform API/dependency design and a
reviewer able to inspect the Windows handle boundary before production work.
Windows execution was not available in this evaluation.

## Follow-up B — configured Git filters

### Reproduction

The current `src/git.rs::filters_configured` checks effective system, global,
repository and command-scope config for any `filter.*.clean` or
`filter.*.process` entry. When one exists, `probe` skips `git status`, returns
`is_dirty: null`, and emits a warning. It does not inspect whether any current
path has the corresponding `filter` attribute.

On this macOS host, the pre-existing system/global config contained zero matching
entries. I then used disposable repositories and an explicit temporary
`GIT_CONFIG_GLOBAL` with `GIT_CONFIG_NOSYSTEM=1`:

```sh
GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL="$TMP/global.config" \
  target/debug/ctx scan "$TMP/unused-global-repo" --format json
GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null \
  target/debug/ctx scan "$TMP/applicable-filter-repo" --format json
```

The first temporary config defined `filter.lfs.clean` and `filter.lfs.process`
without any matching path attribute. The second repository had
`.gitattributes` `*.txt filter=probe` and local `filter.probe.clean` configuration.
Both contained a dirty tracked file and marker paths in the filter commands.

| Fixture | Observation | Filter execution |
| --- | --- | --- |
| Global `filter.lfs.clean` and `.process`, no path attributes, dirty tracked Rust file | `git_dirty: null`; generic configured-filter warning | No marker created |
| Tracked `*.txt` path with `.gitattributes` `filter=probe` and a configured clean command, dirty contents | `git_dirty: null`; generic configured-filter warning | No marker created |

The second case is also covered by the existing `git_filter_configuration_skips_dirty_probe_without_running_filter` integration test. The handoff's described global-filter usability issue therefore remains applicable; the reported Linux outcome was not repeated on Linux here.

### Safety decision

Git's `check-attr` can resolve the effective static `filter` value without
invoking a filter. Its documented resolution includes `$GIT_DIR/info/attributes`,
nested and root `.gitattributes`, configured global/system attribute files,
per-attribute precedence, and top-level custom macros. Git's `filter` driver
documentation identifies `clean` and `process` as check-in filters. See the
[gitattributes documentation](https://git-scm.com/docs/gitattributes) and
[check-attr documentation](https://git-scm.com/docs/git-check-attr).

An exploratory local prototype also enumerated tracked and non-ignored untracked
paths and queried `check-attr` in bounded NUL-delimited input/output. Its static
fixtures passed for an unused global LFS filter and applicable filters from nested
attributes, `info/attributes`, a configured global attributes file, a custom
macro, and a process filter; markers showed that the filter commands did not run.
The prototype and those temporary tests were not retained as production changes.

The unresolved issue is not attribute syntax coverage: Git already resolves that.
The probe and `git status` do not share a frozen path inventory, attribute set, or
effective config. A concurrent change after `check-attr` but before `status` can
  make the preflight miss a filter that status then runs. Fail-closed behavior on an
observed command error or output cap cannot close that time-of-check/time-of-use
window. Since Engine must not execute analyzed repository code and no atomic
snapshot or status mode preserves clean-filter semantics while disabling filters,
the conditional “safe, narrowly provable” gate is not met. Keep `dirty: null` when
any clean/process filter configuration is observed until a separately reviewed
design closes that boundary. This decision does not claim an atomic Git status
snapshot or resolve the existing race between config inspection and status.

### Diagnostic proposal

The current warning says only that external filters are configured, which does
not tell a user whether the repository uses them. If retained as the final policy,
consider this clearer warning text:

> Git dirty state is unavailable because clean/process filter configuration is present; Context Engine does not invoke filters.

This is a diagnostic-only proposal. No warning text or output contract was changed.

## Verification and limits

- Reproduced both filter cases against the unmodified binary `target/debug/ctx`;
  the controlled global configuration used disposable paths and no filter
  executable was invoked.
- The exploratory attribute prototype's four focused CLI tests passed locally,
  then all prototype source/test edits were reversed. Those results are not
  represented as verification of shipped behavior.
- Focused final-tree tests passed with isolated Git config:
  `source::tests::post_metadata_file_and_ancestor_swaps_fail_before_opening_sentinel`,
  `scan::tests::metadata_to_open_swaps_cannot_read_outside_controls_or_counted_files`,
  `git_filter_configuration_skips_dirty_probe_without_running_filter`,
  `git_filter_safety_check_failure_skips_status`,
  `non_utf8_history_paths_have_distinct_native_identities`, and
  `raw_blobs_ignore_attributes_filters_and_git_command_overrides`. The first
  sandboxed history attempt was denied while creating its native filename; it
  passed on host execution.
- Two default-parallel canonical runs timed out in
  `mcp_wire_metadata_errors_notifications_and_eof`; that test passed alone. The
  canonical script then passed serially with:
  `RUST_TEST_THREADS=1 GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null ./scripts/verify.sh`
  — 126 unit tests and 107 integration tests, plus formatting, Clippy and context
  checks.
- `cargo check --all-targets --all-features` and
  `cargo build --locked --release` passed with system/global Git config isolated.
  `git diff --check` passed.
- The host is macOS/aarch64 with Rust 1.99.1 and Git 2.39.5. Windows/Linux runs,
  independent review, maintainer acceptance, Context Integrity integration and
  cloud CI were not performed as part of this evaluation.
- The two pre-existing untracked plan files remain unmodified and outside any
  commit.

The non-Unix support decision and broader safe filter-snapshot design both remain
maintainer/reviewer gates. Phase 12 remains unauthorized.
