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
    pub size: u64,
}
impl Entry {
    pub fn regular(&self) -> bool {
        matches!(self.mode.as_str(), "100644" | "100755")
    }
}
#[derive(Debug)]
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
        let out = run(
            &self.root,
            &[
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
                &self.base,
                &self.head,
                "--",
            ],
            limits.diff_bytes,
            "raw diff bytes",
        )?;
        let mut changes = parse_diff(&out, limits)?;
        for change in &mut changes {
            for side in [&mut change.before, &mut change.after] {
                if let Some(e) = side {
                    if let Ok(p) = e.path.strip_prefix(&self.prefix) {
                        e.path = p.to_owned();
                    } else {
                        *side = None;
                    }
                }
            }
            if change.before.is_none() {
                change.status = "added".into();
                change.similarity = None;
            }
            if change.after.is_none() {
                change.status = "deleted".into();
                change.similarity = None;
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
        Ok(changes)
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
                size: 0,
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
        let p = p
            .strip_prefix(prefix)
            .map_err(|_| Error::Git("tree scope"))?
            .to_path_buf();
        paths = paths.saturating_add(p.as_os_str().len());
        if result.len() >= l.entries || paths > l.tree_paths {
            return Err(Error::Limit("inventory records/paths"));
        }
        result.push(Entry {
            path: p,
            mode: h[0].into(),
            oid: h[2].into(),
            size: if h[3] == "-" {
                0
            } else {
                h[3].parse().map_err(|_| Error::Git("tree size"))?
            },
        });
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(e[0].size, 9);
        assert_eq!(e[0].path, Path::new("a.py"));
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
