//! Immutable Git-object adapter. No checkout, filters, or repository writes.
use std::{
    ffi::OsString,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid explicit BASE..HEAD range")]
    Range,
    #[error("Git {0} failed or returned unsupported data")]
    Git(&'static str),
    #[error("Phase 4 {0} safety limit exceeded")]
    Limit(&'static str),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub revision: usize,
    pub changes: usize,
    pub change_paths: usize,
    pub diff_bytes: usize,
    pub entries: usize,
    pub tree_paths: usize,
    pub tree_bytes: usize,
    pub source_bytes: u64,
}
pub(crate) const LIMITS: Limits = Limits {
    revision: 4096,
    changes: 10_000,
    change_paths: 4 * 1024 * 1024,
    diff_bytes: 16 * 1024 * 1024,
    entries: 50_000,
    tree_paths: 4 * 1024 * 1024,
    tree_bytes: 16 * 1024 * 1024,
    source_bytes: 256 * 1024 * 1024,
};
pub fn range(text: &str, max: usize) -> Result<(&str, &str), Error> {
    if text.len() > max {
        return Err(Error::Limit("revision text"));
    }
    let (a, b) = text.split_once("..").ok_or(Error::Range)?;
    if [a, b].iter().any(|s| {
        s.is_empty() || s.starts_with('-') || s.contains("..") || s.chars().any(char::is_control)
    }) || text.contains("...")
    {
        return Err(Error::Range);
    }
    Ok((a, b))
}
fn command(root: &Path) -> Command {
    let mut c = Command::new("git");
    c.args([
        "--no-optional-locks",
        "--literal-pathspecs",
        "-c",
        "core.fsmonitor=false",
        "-C",
    ])
    .arg(root)
    .env("LC_ALL", "C")
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("GIT_NO_REPLACE_OBJECTS", "1")
    .env("GIT_NO_LAZY_FETCH", "1");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            c.env_remove(key);
        }
    }
    c.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_NO_LAZY_FETCH", "1");
    c
}
fn bounded(mut c: Command, max: usize, operation: &'static str) -> Result<Vec<u8>, Error> {
    let mut child = c
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or(Error::Git(operation))?
        .take(max.saturating_add(1) as u64)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > max {
        let _ = child.kill();
        let _ = child.wait();
        read?;
        return Err(Error::Limit(operation));
    }
    if !child.wait()?.success() {
        return Err(Error::Git(operation));
    }
    Ok(bytes)
}
fn run(root: &Path, args: &[&str], max: usize, op: &'static str) -> Result<Vec<u8>, Error> {
    let mut c = command(root);
    c.args(args);
    bounded(c, max, op)
}
fn text(bytes: &[u8]) -> Result<&str, Error> {
    std::str::from_utf8(bytes).map_err(|_| Error::Git("text parsing"))
}
fn path(bytes: &[u8]) -> Result<PathBuf, Error> {
    #[cfg(unix)]
    let p = {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(OsString::from_vec(bytes.to_vec()))
    };
    #[cfg(not(unix))]
    let p = PathBuf::from(OsString::from(text(bytes)?));
    if p.as_os_str().is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(Error::Git("path parsing"));
    }
    Ok(p)
}
fn oid(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub mode: String,
    pub oid: String,
    /// `Some` when supplied by `ls-tree -l`; raw diff records do not carry sizes.
    pub size: Option<u64>,
}
impl Entry {
    pub fn regular(&self) -> bool {
        matches!(self.mode.as_str(), "100644" | "100755")
    }
}
#[derive(Clone, Debug)]
pub struct FileChange {
    pub status: String,
    pub similarity: Option<u8>,
    pub before: Option<Entry>,
    pub after: Option<Entry>,
}
pub struct Revisions {
    pub root: PathBuf,
    pub scope: PathBuf,
    pub prefix: PathBuf,
    pub base: String,
    pub head: String,
}
impl Revisions {
    pub fn open(scope: &Path, input: &str, limits: Limits) -> Result<Self, Error> {
        let (a, b) = range(input, limits.revision)?;
        let scope = scope.canonicalize()?;
        if !scope.is_dir() {
            return Err(Error::Git("scope must be a directory"));
        }
        let mut bytes = run(
            &scope,
            &["rev-parse", "--show-toplevel"],
            64 * 1024,
            "repository discovery",
        )?;
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        // Git returns an absolute root; unlike tree paths it can contain a root component.
        #[cfg(unix)]
        let root = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(OsString::from_vec(bytes)).canonicalize()?
        };
        #[cfg(not(unix))]
        let root = { PathBuf::from(text(&bytes)?).canonicalize()? };
        let prefix = scope
            .strip_prefix(&root)
            .map_err(|_| Error::Git("scope"))?
            .to_path_buf();
        let resolve = |rev: &str| -> Result<String, Error> {
            let out = run(
                &root,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{rev}^{{commit}}"),
                ],
                256,
                "commit resolution",
            )?;
            let s = text(&out)?.trim();
            if !oid(s) {
                return Err(Error::Git("commit resolution"));
            }
            Ok(s.into())
        };
        Ok(Self {
            base: resolve(a)?,
            head: resolve(b)?,
            root,
            scope,
            prefix,
        })
    }
    pub fn changes(&self, limits: Limits) -> Result<Vec<FileChange>, Error> {
        Ok(self.changes_inner(limits, false)?.0)
    }
    /// Usage is measured before scope filtering so unrelated changes cannot
    /// multiply a per-diff ceiling across a history walk.
    pub(crate) fn changes_with_usage(
        &self,
        limits: Limits,
    ) -> Result<(Vec<FileChange>, [usize; 3]), Error> {
        self.changes_inner(limits, true)
    }
    fn changes_inner(
        &self,
        limits: Limits,
        history: bool,
    ) -> Result<(Vec<FileChange>, [usize; 3]), Error> {
        let mut args = vec![
            "-c",
            "diff.renameLimit=10000",
            "-c",
            "diff.algorithm=myers",
            "diff-tree",
            "-r",
            "--raw",
            "-z",
            "--no-commit-id",
            "--no-abbrev",
            "--find-renames=50%",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ];
        if self.base.is_empty() {
            args.extend(["--root", &self.head, "--"]);
        } else {
            args.extend([&self.base, &self.head, "--"]);
        }
        let out = run(&self.root, &args, limits.diff_bytes, "raw diff bytes")?;
        let mut changes = parse_diff(&out, limits)?;
        let usage = [
            changes.len(),
            changes
                .iter()
                .map(|c| {
                    // parse_diff accounts both path slots, including non-renames.
                    c.before
                        .as_ref()
                        .or(c.after.as_ref())
                        .map_or(0, |e| e.path.as_os_str().len())
                        + c.after
                            .as_ref()
                            .or(c.before.as_ref())
                            .map_or(0, |e| e.path.as_os_str().len())
                })
                .sum(),
            out.len(),
        ];
        for change in &mut changes {
            for side in [&mut change.before, &mut change.after] {
                if let Some(e) = side {
                    if let Some(p) = scope_relative(&e.path, &self.prefix) {
                        e.path = p;
                    } else {
                        *side = None;
                    }
                }
            }
            if change.before.is_none() {
                change.status = if history && change.similarity.is_some() {
                    "entered_scope"
                } else {
                    "added"
                }
                .into();
                if !history {
                    change.similarity = None;
                }
            }
            if change.after.is_none() {
                change.status = if history && change.similarity.is_some() {
                    "left_scope"
                } else {
                    "deleted"
                }
                .into();
                if !history {
                    change.similarity = None;
                }
            }
        }
        changes.retain(|c| c.before.is_some() || c.after.is_some());
        changes.sort_by(|a, b| {
            a.before
                .as_ref()
                .or(a.after.as_ref())
                .unwrap()
                .path
                .cmp(&b.before.as_ref().or(b.after.as_ref()).unwrap().path)
        });
        Ok((changes, usage))
    }
    /// Git walk order is authoritative; timestamps are metadata only.
    pub(crate) fn commits(&self, max: usize, metadata_bytes: usize) -> Result<Vec<Commit>, Error> {
        // Shallow/grafted boundaries are not immutable root-commit evidence.
        if run(
            &self.root,
            &["rev-parse", "--is-shallow-repository"],
            16,
            "shallow history check",
        )? != b"false\n"
        {
            return Err(Error::Git(
                "history requires a complete, non-shallow repository",
            ));
        }
        let mut graft_path = run(
            &self.root,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "info/grafts",
            ],
            64 * 1024,
            "history graft check",
        )?;
        if graft_path.last() == Some(&b'\n') {
            graft_path.pop();
        }
        #[cfg(unix)]
        let graft_path = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(OsString::from_vec(graft_path))
        };
        #[cfg(not(unix))]
        let graft_path = PathBuf::from(text(&graft_path)?);
        match std::fs::symlink_metadata(graft_path) {
            Ok(_) => return Err(Error::Git("history does not support grafted ancestry")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let out = run(
            &self.root,
            &[
                "log",
                "--first-parent",
                "--no-show-signature",
                "--no-decorate",
                "-z",
                "--format=%H%x00%P%x00%ct%x00%s",
                &format!("--max-count={}", max + 1),
                &self.head,
                "--",
            ],
            metadata_bytes,
            "history metadata bytes",
        )?;
        let commits = parse_commits(&out)?;
        let mut remaining = metadata_bytes.saturating_sub(out.len());
        // Verify claimed roots against immutable object headers as well. A live
        // shallow/graft metadata change during enumeration cannot fabricate a root.
        for commit in commits.iter().filter(|c| c.first_parent.is_none()) {
            let raw = run(
                &self.root,
                &["cat-file", "commit", &commit.oid],
                remaining,
                "history root metadata",
            )?;
            remaining = remaining.saturating_sub(raw.len());
            if raw
                .split(|b| *b == b'\n')
                .take_while(|line| !line.is_empty())
                .any(|line| line.starts_with(b"parent "))
            {
                return Err(Error::Git("incomplete history ancestry boundary"));
            }
        }
        if commits.is_empty() {
            return Err(Error::Git("empty committed history"));
        }
        Ok(commits)
    }
    pub fn inventory(&self, commit: &str, limits: Limits) -> Result<Vec<Entry>, Error> {
        let mut c = command(&self.root);
        c.args(["ls-tree", "-r", "-z", "-l", commit, "--"]);
        if !self.prefix.as_os_str().is_empty() {
            c.arg(&self.prefix);
        }
        let out = bounded(c, limits.tree_bytes, "tree bytes")?;
        parse_tree(&out, &self.prefix, limits)
    }
    pub fn size(&self, e: &Entry) -> Result<u64, Error> {
        let out = run(&self.root, &["cat-file", "-s", &e.oid], 64, "blob size")?;
        text(&out)?
            .trim()
            .parse()
            .map_err(|_| Error::Git("blob size"))
    }
    pub fn blob(&self, e: &Entry, max: u64) -> Result<Vec<u8>, Error> {
        run(
            &self.root,
            &["cat-file", "blob", &e.oid],
            max.min(usize::MAX as u64) as usize,
            "blob bytes",
        )
    }
}
#[derive(Clone, Debug)]
pub struct Commit {
    pub oid: String,
    pub first_parent: Option<String>,
    pub committer_timestamp: i64,
    pub subject: String,
}
fn parse_commits(bytes: &[u8]) -> Result<Vec<Commit>, Error> {
    let f = fields(bytes)?;
    if f.len() % 4 != 0 {
        return Err(Error::Git("history records"));
    }
    f.as_chunks::<4>()
        .0
        .iter()
        .map(|r| {
            let id = text(r[0])?;
            let parents: Vec<_> = text(r[1])?.split_whitespace().collect();
            if !oid(id) || parents.iter().any(|p| !oid(p)) {
                return Err(Error::Git("history commit identity"));
            }
            Ok(Commit {
                oid: id.into(),
                first_parent: parents.first().map(|p| (*p).into()),
                committer_timestamp: text(r[2])?
                    .parse()
                    .map_err(|_| Error::Git("history timestamp"))?,
                subject: String::from_utf8_lossy(r[3]).into_owned(),
            })
        })
        .collect()
}
fn fields(bytes: &[u8]) -> Result<Vec<&[u8]>, Error> {
    if bytes.is_empty() {
        return Ok(vec![]);
    }
    if bytes.last() != Some(&0) {
        return Err(Error::Git("NUL records"));
    }
    Ok(bytes[..bytes.len() - 1].split(|b| *b == 0).collect())
}
fn parse_diff(bytes: &[u8], l: Limits) -> Result<Vec<FileChange>, Error> {
    let fields = fields(bytes)?;
    let mut i = 0;
    let mut result = vec![];
    let mut paths = 0usize;
    while i < fields.len() {
        let h: Vec<_> = text(fields[i])?.split(' ').collect();
        i += 1;
        if h.len() != 5 || !h[0].starts_with(':') || !oid(h[2]) || !oid(h[3]) {
            return Err(Error::Git("raw diff"));
        }
        let status = h[4];
        let mut kind = match status.as_bytes().first() {
            Some(b'A') => "added",
            Some(b'D') => "deleted",
            Some(b'M') => "modified",
            Some(b'T') => "type_changed",
            Some(b'R') => "renamed",
            _ => return Err(Error::Git("diff status")),
        };
        if kind == "modified" && &h[0][1..] != h[1] && h[2] == h[3] {
            kind = "mode_changed";
        }
        let first = *fields.get(i).ok_or(Error::Git("diff path"))?;
        i += 1;
        let second = if kind == "renamed" {
            let p = *fields.get(i).ok_or(Error::Git("rename path"))?;
            i += 1;
            p
        } else {
            first
        };
        paths = paths
            .saturating_add(first.len())
            .saturating_add(second.len());
        if result.len() >= l.changes || paths > l.change_paths {
            return Err(Error::Limit("changed records/paths"));
        }
        let entry = |p, mode: &str, id: &str| -> Result<Option<Entry>, Error> {
            if mode == "000000" {
                return Ok(None);
            }
            if !matches!(mode, "100644" | "100755" | "120000" | "160000") {
                return Err(Error::Git("object mode"));
            }
            Ok(Some(Entry {
                path: path(p)?,
                mode: mode.into(),
                oid: id.into(),
                size: None,
            }))
        };
        let similarity = if kind == "renamed" {
            Some(
                status[1..]
                    .parse::<u8>()
                    .ok()
                    .filter(|n| *n <= 100)
                    .ok_or(Error::Git("rename similarity"))?,
            )
        } else {
            if status.len() != 1 {
                return Err(Error::Git("diff status"));
            }
            None
        };
        result.push(FileChange {
            status: kind.into(),
            similarity,
            before: entry(first, &h[0][1..], h[2])?,
            after: entry(second, h[1], h[3])?,
        });
    }
    Ok(result)
}
fn parse_tree(bytes: &[u8], prefix: &Path, l: Limits) -> Result<Vec<Entry>, Error> {
    let mut result = vec![];
    let mut paths = 0usize;
    for record in fields(bytes)? {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or(Error::Git("tree record"))?;
        let h: Vec<_> = text(&record[..tab])?.split_whitespace().collect();
        if h.len() != 4
            || !oid(h[2])
            || !matches!(
                (h[0], h[1]),
                ("100644" | "100755" | "120000", "blob") | ("160000", "commit")
            )
        {
            return Err(Error::Git("tree record"));
        }
        let p = path(&record[tab + 1..])?;
        let Some(p) = scope_relative(&p, prefix) else {
            // A historical file/symlink/gitlink exactly at the current directory
            // scope is not an entry inside that directory.
            continue;
        };
        paths = paths.saturating_add(p.as_os_str().len());
        if result.len() >= l.entries || paths > l.tree_paths {
            return Err(Error::Limit("inventory records/paths"));
        }
        result.push(Entry {
            path: p,
            mode: h[0].into(),
            oid: h[2].into(),
            size: if h[3] == "-" {
                None
            } else {
                Some(h[3].parse().map_err(|_| Error::Git("tree size"))?)
            },
        });
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

/// Convert a Git-root path to a strict descendant of the requested directory.
/// The prefix itself is excluded because it is not a file within that scope.
fn scope_relative(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(prefix).ok()?;
    (!relative.as_os_str().is_empty()).then(|| relative.to_path_buf())
}

/// Context ranks filesystem bytes, so hidden index entries and gitlinks cannot
/// establish the clean snapshot required for committed-change ranking. This is
/// metadata-only and bounded; it never descends into a submodule or runs filters.
pub fn validate_context_index(scope: &Path) -> Result<(), Error> {
    let bytes = run(
        scope,
        &["ls-files", "--cached", "--stage", "-v", "-z"],
        LIMITS.tree_bytes,
        "context index compatibility",
    )?;
    for row in bytes.split(|&b| b == 0).filter(|row| !row.is_empty()) {
        let header = row
            .split(|&b| b == b'\t')
            .next()
            .ok_or(Error::Git("context index metadata"))?;
        // Lowercase status tags hide assume-unchanged entries; S hides sparse /
        // skip-worktree entries. Only ordinary tracked H entries are accepted.
        let fields: Vec<_> = header.split(|&b| b == b' ').collect();
        if fields.len() != 4 || fields[0] != b"H" || fields[3] != b"0" {
            return Err(Error::Git(
                "--change requires an index without hidden or unmerged entries",
            ));
        }
        if fields[1] == b"160000" {
            return Err(Error::Git(
                "--change cannot establish clean submodule content",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_metadata_is_typed_bounded_and_not_timestamp_sorted() {
        let id = "a".repeat(40);
        let parent = "b".repeat(40);
        let bytes = format!(
            "{id}\0{parent} {}\0-10\0hostile\x1b[31m\0{parent}\0\0999\0root\0",
            "c".repeat(40)
        );
        let c = parse_commits(bytes.as_bytes()).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].oid, id);
        assert_eq!(c[0].first_parent.as_deref(), Some(parent.as_str()));
        assert_eq!(c[0].committer_timestamp, -10);
        assert!(c[1].first_parent.is_none());
        for bad in [b"bad\0".as_slice(), b"bad\0\x000\0text\0", b"no terminator"] {
            assert!(parse_commits(bad).is_err());
        }
    }
    #[test]
    fn explicit_range_grammar_and_exact_text_budget() {
        for valid in ["main..HEAD", "HEAD~1..v1.0", "a^{commit}..b"] {
            assert!(range(valid, valid.len()).is_ok());
            assert!(matches!(
                range(valid, valid.len() - 1),
                Err(Error::Limit(_))
            ));
        }
        for invalid in [
            "", "HEAD", "..HEAD", "HEAD..", "a...b", "a..b..c", "-a..b", "a..-b", "a\n..b",
            "a..b\0",
        ] {
            assert!(range(invalid, 100).is_err(), "{invalid:?}");
        }
    }
    fn raw(status: &str, old: &str, new: &str, paths: &str) -> Vec<u8> {
        format!(
            ":{old} {new} {} {} {status}\0{paths}\0",
            "1".repeat(40),
            "2".repeat(40)
        )
        .into_bytes()
    }
    #[test]
    fn raw_records_modes_renames_and_exact_count_path_limits() {
        for (status, old, new, paths, kind) in [
            ("A", "000000", "100644", "a.py", "added"),
            ("D", "100644", "000000", "a.py", "deleted"),
            ("M", "100644", "100755", "a.py", "modified"),
            ("T", "100644", "120000", "a.py", "type_changed"),
            ("R100", "100644", "100644", "a.py\0b.py", "renamed"),
        ] {
            let bytes = raw(status, old, new, paths);
            let exact = Limits {
                changes: 1,
                change_paths: 8,
                ..LIMITS
            };
            let v = parse_diff(&bytes, exact).unwrap();
            assert_eq!(v[0].status, kind);
            assert!(
                v[0].before
                    .as_ref()
                    .or(v[0].after.as_ref())
                    .unwrap()
                    .size
                    .is_none()
            );
            assert!(
                parse_diff(
                    &bytes,
                    Limits {
                        changes: 0,
                        ..exact
                    }
                )
                .is_err()
            );
            assert!(
                parse_diff(
                    &bytes,
                    Limits {
                        change_paths: 7,
                        ..exact
                    }
                )
                .is_err()
            );
            assert!(parse_diff(&bytes[..bytes.len() - 1], exact).is_err());
            if kind == "renamed" {
                assert_eq!(v[0].similarity, Some(100));
                assert_eq!(v[0].after.as_ref().unwrap().path, Path::new("b.py"));
            }
        }
        for bytes in [
            raw("X", "100644", "100644", "a.py"),
            raw("R101", "100644", "100644", "a.py\0b.py"),
            raw("M", "100644", "100644", "../x"),
        ] {
            assert!(parse_diff(&bytes, LIMITS).is_err());
        }
    }
    #[test]
    fn inventory_exact_entry_path_limits_and_malformed_records() {
        let bytes = format!("100644 blob {} 9\tscope/a.py\0", "1".repeat(40));
        let l = Limits {
            entries: 1,
            tree_paths: 4,
            ..LIMITS
        };
        let e = parse_tree(bytes.as_bytes(), Path::new("scope"), l).unwrap();
        assert_eq!(e[0].size, Some(9));
        assert_eq!(e[0].path, Path::new("a.py"));
        let empty_blob = format!("100644 blob {} 0\tscope/empty.py\0", "2".repeat(40));
        let empty = parse_tree(empty_blob.as_bytes(), Path::new("scope"), l).unwrap();
        assert_eq!(empty[0].size, Some(0));
        assert!(
            parse_tree(
                bytes.as_bytes(),
                Path::new("scope"),
                Limits { entries: 0, ..l }
            )
            .is_err()
        );
        assert!(
            parse_tree(
                bytes.as_bytes(),
                Path::new("scope"),
                Limits { tree_paths: 3, ..l }
            )
            .is_err()
        );
        assert!(parse_tree(b"not a tree\0", Path::new(""), l).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_keep_native_identity_through_git_parsing() {
        let mut bytes = raw("A", "000000", "100644", "placeholder");
        let pos = bytes.iter().position(|b| *b == 0).unwrap() + 1;
        bytes.truncate(pos);
        bytes.extend_from_slice(b"a\xff.py\0");
        let first = parse_diff(&bytes, LIMITS)
            .unwrap()
            .pop()
            .unwrap()
            .after
            .unwrap();
        bytes[pos + 1] = 0xfe;
        let second = parse_diff(&bytes, LIMITS)
            .unwrap()
            .pop()
            .unwrap()
            .after
            .unwrap();
        assert_ne!(first.path, second.path);
        assert_eq!(first.path.to_string_lossy(), second.path.to_string_lossy());
    }
    #[test]
    fn subprocess_bytes_are_bounded_before_returning() {
        // git --version is tiny, deterministic within this test and needs no repository.
        let c = || {
            let mut c = Command::new("git");
            c.arg("--version");
            c
        };
        let bytes = bounded(c(), 1024, "test").unwrap();
        assert_eq!(bounded(c(), bytes.len(), "test").unwrap(), bytes);
        assert!(matches!(
            bounded(c(), bytes.len() - 1, "test"),
            Err(Error::Limit("test"))
        ));
    }
}
