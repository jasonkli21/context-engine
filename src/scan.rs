use crate::{
    classify::{classify_language, classify_role},
    git,
    model::{FileRecord, RepositoryScan, ScanOptions, ScanWarning},
    source,
};
use ignore::{
    WalkBuilder,
    gitignore::{Gitignore, GitignoreBuilder},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("repository scan safety limit exceeded")]
    LimitExceeded,
    #[error("cannot access scan root {path}: {source}; provide an existing readable directory")]
    Root { path: PathBuf, source: io::Error },
    #[error("scan root {0} is not a directory; provide a directory path")]
    NotDirectory(PathBuf),
    #[error(
        "symlinked ignore file {path:?} is unsupported because it could escape the scan boundary; replace it with a regular file"
    )]
    SymlinkIgnoreFile { path: PathBuf },
    #[error("cannot safely inspect ignore-control file {path:?}: {source}")]
    IgnorePreflight { path: PathBuf, source: io::Error },
    #[error("cannot start traversal of {path}: {source}")]
    Traversal {
        path: PathBuf,
        source: ignore::Error,
    },
}

#[derive(Clone, Copy)]
struct ScanLimits {
    entries: usize,
    path_bytes: usize,
    warnings: usize,
    warning_bytes: usize,
    line_bytes: u64,
    ignore_bytes: u64,
}
const SCAN_LIMITS: ScanLimits = ScanLimits {
    entries: 50_000,
    path_bytes: 4 * 1024 * 1024,
    warnings: 1_000,
    warning_bytes: 1024 * 1024,
    line_bytes: 256 * 1024 * 1024,
    ignore_bytes: 1024 * 1024,
};
struct ScanBudget {
    limits: ScanLimits,
    entries: usize,
    paths: usize,
    warnings: usize,
    details: usize,
    lines: u64,
    ignores: u64,
}
impl ScanBudget {
    fn new(limits: ScanLimits) -> Self {
        Self {
            limits,
            entries: 0,
            paths: 0,
            warnings: 0,
            details: 0,
            lines: 0,
            ignores: 0,
        }
    }
    fn entry(&mut self, path: &Path) -> Result<(), ScanError> {
        self.entries = self.entries.saturating_add(1);
        self.paths = self.paths.saturating_add(path.as_os_str().len());
        if self.entries > self.limits.entries || self.paths > self.limits.path_bytes {
            return Err(ScanError::LimitExceeded);
        }
        Ok(())
    }
    fn warning(
        &mut self,
        warnings: &mut Vec<ScanWarning>,
        warning: ScanWarning,
    ) -> Result<(), ScanError> {
        self.warnings += 1;
        self.details = self
            .details
            .saturating_add(warning.message.len())
            .saturating_add(warning.path.as_ref().map_or(0, |p| p.as_os_str().len()));
        if self.warnings > self.limits.warnings || self.details > self.limits.warning_bytes {
            return Err(ScanError::LimitExceeded);
        }
        warnings.push(warning);
        Ok(())
    }
    fn ignore(&mut self, bytes: u64) -> Result<(), ScanError> {
        self.ignores = self.ignores.saturating_add(bytes);
        if self.ignores > self.limits.ignore_bytes {
            return Err(ScanError::LimitExceeded);
        }
        Ok(())
    }
}
pub fn scan_repository(options: &ScanOptions) -> Result<RepositoryScan, ScanError> {
    scan_with_limits(options, SCAN_LIMITS)
}
fn scan_with_limits(
    options: &ScanOptions,
    limits: ScanLimits,
) -> Result<RepositoryScan, ScanError> {
    scan_with_open_hook(options, limits, |_| {})
}
// The callback is a deterministic test seam at the metadata/open boundary.
fn scan_with_open_hook(
    options: &ScanOptions,
    limits: ScanLimits,
    before_open: impl Fn(&Path),
) -> Result<RepositoryScan, ScanError> {
    let root = fs::canonicalize(&options.root).map_err(|source| ScanError::Root {
        path: options.root.clone(),
        source,
    })?;
    if !root.is_dir() {
        return Err(ScanError::NotDirectory(root));
    }
    // Verify traversal can start; individual child failures below remain recoverable.
    fs::read_dir(&root).map_err(|source| ScanError::Root {
        path: root.clone(),
        source,
    })?;
    let rules = std::sync::Arc::new(std::sync::Mutex::new(checked_ignore_rules(
        &root,
        limits,
        &before_open,
    )?));
    let mut budget = ScanBudget::new(limits);
    let mut scan = RepositoryScan {
        root: root.clone(),
        files: vec![],
        directories: vec![],
        git: None,
        warnings: vec![],
    };
    let rule_warnings: Vec<_> = rules
        .lock()
        .unwrap()
        .warnings
        .iter()
        .map(|warning| ScanWarning {
            path: warning.path.clone(),
            message: warning.message.clone(),
        })
        .collect();
    for warning in &rule_warnings {
        budget.warning(
            &mut scan.warnings,
            ScanWarning {
                path: warning.path.clone(),
                message: warning.message.clone(),
            },
        )?;
    }
    let walk_rules = rules.clone();
    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false)
        .git_ignore(false)
        .parents(false)
        .filter_entry(move |entry| {
            entry.depth() == 0
                || (entry.file_name() != ".git"
                    && !walk_rules
                        .lock()
                        .unwrap()
                        .ignored(entry.path(), entry.file_type().is_some_and(|t| t.is_dir())))
        });
    for result in builder.build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(source) => {
                if source
                    .path_in_error()
                    .is_some_and(|p| rules.lock().unwrap().blocked.contains(p))
                {
                    continue;
                }
                if source.depth() == Some(0) {
                    return Err(ScanError::Traversal { path: root, source });
                }
                budget.entry(Path::new(""))?;
                budget.warning(
                    &mut scan.warnings,
                    ScanWarning {
                        path: source
                            .path_in_error()
                            .and_then(|path| path.strip_prefix(&root).ok())
                            .map(Path::to_path_buf),
                        message: format!("traversal: {source}"),
                    },
                )?;
                continue;
            }
        };
        if entry.depth() != 0 {
            budget.entry(entry.path().strip_prefix(&root).unwrap_or(entry.path()))?;
        }
        if let Some(error) = entry.error() {
            budget.warning(
                &mut scan.warnings,
                ScanWarning {
                    path: entry.path().strip_prefix(&root).ok().map(Path::to_path_buf),
                    message: format!("ignore rules: {error}"),
                },
            )?;
        }
        if entry.depth() == 0 {
            continue;
        }
        let relative_path = match entry.path().strip_prefix(&root) {
            Ok(path) => path.to_path_buf(),
            Err(error) => {
                budget.warning(
                    &mut scan.warnings,
                    ScanWarning {
                        path: None,
                        message: format!("skipped path outside scan root: {error}"),
                    },
                )?;
                continue;
            }
        };
        let Some(kind) = entry.file_type() else {
            continue;
        };
        // Skip all symlinks, including file links, rather than reading outside the root.
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            scan.directories.push(relative_path);
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => continue,
            Err(error) => {
                budget.warning(
                    &mut scan.warnings,
                    ScanWarning {
                        path: Some(relative_path),
                        message: format!("metadata: {error}"),
                    },
                )?;
                continue;
            }
        };
        let language = classify_language(&relative_path);
        let role = classify_role(&relative_path, &language);
        let line_count = if options.count_lines
            && language.is_text()
            && metadata.len() <= options.max_line_count_file_bytes
        {
            // Reserve the extra byte used to detect growth after discovery.
            budget.lines = budget
                .lines
                .saturating_add(metadata.len().saturating_add(1));
            if budget.lines > limits.line_bytes {
                return Err(ScanError::LimitExceeded);
            }
            before_open(entry.path());
            match count_lines(&root, &relative_path, metadata.len()) {
                Ok(lines) => Some(lines),
                Err(error) => {
                    budget.warning(
                        &mut scan.warnings,
                        ScanWarning {
                            path: Some(relative_path.clone()),
                            message: format!("line count unavailable: {error}"),
                        },
                    )?;
                    None
                }
            }
        } else {
            None
        };
        scan.files.push(FileRecord {
            relative_path,
            language,
            role,
            size_bytes: metadata.len(),
            line_count,
        });
    }
    drop(builder);
    drop(rules);
    scan.files
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    scan.directories.sort();
    if options.probe_git {
        let (metadata, warnings) = git::probe(&root);
        scan.git = metadata;
        for warning in warnings {
            budget.warning(&mut scan.warnings, warning)?;
        }
    }
    scan.warnings
        .sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.message.cmp(&b.message)));
    Ok(scan)
}

trait ErrorPath {
    fn path_in_error(&self) -> Option<&Path>;
}

impl ErrorPath for ignore::Error {
    fn path_in_error(&self) -> Option<&Path> {
        match self {
            ignore::Error::Partial(errors) => errors.iter().find_map(ErrorPath::path_in_error),
            ignore::Error::WithLineNumber { err, .. } | ignore::Error::WithDepth { err, .. } => {
                err.path_in_error()
            }
            ignore::Error::WithPath { path, .. } => Some(path),
            ignore::Error::Loop { child, .. } => Some(child),
            ignore::Error::Io(_)
            | ignore::Error::Glob { .. }
            | ignore::Error::UnrecognizedFileType(_)
            | ignore::Error::InvalidDefinition => None,
        }
    }
}

// Immutable matchers built from checked descriptors. WalkBuilder's own ignore
// ingestion MUST remain disabled: a metadata preflight cannot authorize a reopen.
#[derive(Default)]
struct IgnoreRules {
    directories: BTreeMap<PathBuf, DirectoryRules>,
    blocked: BTreeSet<PathBuf>,
    warnings: Vec<ScanWarning>,
}
struct DirectoryRules {
    ignore: Gitignore,
    gitignore: Gitignore,
    git_boundary: bool,
}
impl IgnoreRules {
    fn ignored(&self, path: &Path, is_dir: bool) -> bool {
        let mut ignore = ignore::Match::None;
        let mut gitignore = ignore::Match::None;
        let mut saw_git = false;
        for directory in path.parent().into_iter().flat_map(Path::ancestors) {
            if self.blocked.contains(directory) {
                return true;
            }
            if let Some(rules) = self.directories.get(directory) {
                if ignore.is_none() {
                    ignore = rules.ignore.matched(path, is_dir);
                }
                if !saw_git && gitignore.is_none() {
                    gitignore = rules.gitignore.matched(path, is_dir);
                }
                saw_git |= rules.git_boundary;
            }
        }
        // .ignore takes precedence over .gitignore, even across directories.
        ignore.or(gitignore).is_ignore()
    }
}
fn checked_ignore_rules(
    root: &Path,
    limits: ScanLimits,
    before_open: &impl Fn(&Path),
) -> Result<IgnoreRules, ScanError> {
    let mut budget = ScanBudget::new(limits);
    let mut rules = IgnoreRules::default();
    fn add_directory(
        directory: &Path,
        budget: &mut ScanBudget,
        rules: &mut IgnoreRules,
        before_open: &impl Fn(&Path),
    ) -> Result<(), ScanError> {
        let mut matchers = Vec::new();
        for name in [".ignore", ".gitignore"] {
            let path = directory.join(name);
            let fail = |source| ScanError::IgnorePreflight {
                path: path.clone(),
                source,
            };
            let metadata = match fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(ScanError::SymlinkIgnoreFile { path });
                }
                Ok(meta) => Some(meta),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(fail(e)),
            };
            let mut builder = GitignoreBuilder::new(directory);
            if let Some(metadata) = &metadata {
                // Nonregular controls fail closed, including FIFOs/devices.
                if !metadata.is_file() {
                    return Err(fail(io::Error::other(
                        "ignore control is not a regular file",
                    )));
                }
                budget.ignore(metadata.len())?;
                before_open(&path);
                let file = source::open_checked(directory, Path::new(name), metadata.len())
                    .map_err(|_| {
                        fail(io::Error::other(
                            "ignore control changed or cannot be opened safely",
                        ))
                    })?;
                let mut bytes = Vec::new();
                file.take(metadata.len().saturating_add(1))
                    .read_to_end(&mut bytes)
                    .map_err(fail)?;
                if bytes.len() as u64 > metadata.len() {
                    return Err(ScanError::LimitExceeded);
                }
                // Same BOM/CRLF and per-line glob grammar as GitignoreBuilder::add.
                // Invalid controls fail closed instead of silently losing exclusions.
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| fail(io::Error::other("ignore control is not UTF-8")))?;
                for (i, line) in text.lines().enumerate() {
                    let line = if i == 0 {
                        line.trim_start_matches('\u{feff}')
                    } else {
                        line
                    };
                    builder
                        .add_line(Some(path.clone()), line)
                        .map_err(|e| fail(io::Error::other(e)))?;
                }
            }
            matchers.push(if metadata.is_some() {
                builder.build().map_err(|e| fail(io::Error::other(e)))?
            } else {
                Gitignore::empty()
            });
        }
        let git_boundary = fs::symlink_metadata(directory.join(".git")).is_ok();
        if git_boundary || matchers.iter().any(|m| !m.is_empty()) {
            rules.directories.insert(
                directory.to_path_buf(),
                DirectoryRules {
                    ignore: matchers.remove(0),
                    gitignore: matchers.remove(0),
                    git_boundary,
                },
            );
        }
        Ok(())
    }
    for directory in root.ancestors() {
        if !directory.components().any(|c| c.as_os_str() == ".git") {
            add_directory(directory, &mut budget, &mut rules, before_open)?;
        }
    }
    let rules = std::sync::Arc::new(std::sync::Mutex::new(rules));
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .filter_entry({
            let rules = rules.clone();
            move |entry| {
                entry.depth() == 0
                    || (entry.file_name() != ".git"
                        && !rules.lock().unwrap().ignored(
                            entry.path(),
                            entry.file_type().is_some_and(|kind| kind.is_dir()),
                        ))
            }
        });
    for result in builder.build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(error) if error.depth() != Some(0) => {
                budget.entry(Path::new(""))?;
                continue;
            }
            Err(source) => {
                return Err(ScanError::Traversal {
                    path: root.into(),
                    source,
                });
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        budget.entry(entry.path().strip_prefix(root).unwrap_or(entry.path()))?;
        let add_result = if entry.file_type().is_some_and(|t| t.is_dir()) {
            Some(add_directory(
                entry.path(),
                &mut budget,
                &mut rules.lock().unwrap(),
                before_open,
            ))
        } else {
            None
        };
        if let Some(Err(error)) = add_result {
            if matches!(&error, ScanError::IgnorePreflight { source, .. } if source.kind() == io::ErrorKind::PermissionDenied)
            {
                let mut rules = rules.lock().unwrap();
                rules.blocked.insert(entry.path().to_path_buf());
                budget.warning(
                    &mut rules.warnings,
                    ScanWarning {
                        path: entry.path().strip_prefix(root).ok().map(Path::to_path_buf),
                        message: "traversal unavailable during ignore snapshot".into(),
                    },
                )?;
            } else {
                return Err(error);
            }
        }
        if entry.file_type().is_some_and(|t| t.is_symlink())
            && matches!(entry.file_name().to_str(), Some(".gitignore" | ".ignore"))
        {
            return Err(ScanError::SymlinkIgnoreFile {
                path: entry.path().into(),
            });
        }
    }
    drop(builder);
    match std::sync::Arc::try_unwrap(rules) {
        Ok(rules) => Ok(rules.into_inner().unwrap()),
        Err(_) => unreachable!("control discovery walker released its ignore rules"),
    }
}
fn count_lines(root: &Path, relative: &Path, max_bytes: u64) -> io::Result<u64> {
    let file = source::open_checked(root, relative, max_bytes)
        .map_err(|_| io::Error::other("file changed or cannot be opened safely"))?;
    count_reader(
        BufReader::new(file.take(max_bytes.saturating_add(1))),
        max_bytes,
    )
}
// Bound work even if a file grows after metadata was read. Validate UTF-8 incrementally
// using a short carry buffer, so a mislabeled binary is never presented as text evidence.
fn count_reader(mut reader: impl BufRead, max_bytes: u64) -> io::Result<u64> {
    let mut total = 0_u64;
    let mut lines = 0_u64;
    let mut last = None;
    let mut carry = Vec::new();
    loop {
        let bytes = reader.fill_buf()?;
        if bytes.is_empty() {
            break;
        }
        total = total.saturating_add(bytes.len() as u64);
        if total > max_bytes {
            return Err(io::Error::other("file exceeds line-count size limit"));
        }
        if bytes.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "NUL byte in recognized text file",
            ));
        }
        let mut text = std::mem::take(&mut carry);
        text.extend_from_slice(bytes);
        if let Err(error) = std::str::from_utf8(&text) {
            if error.error_len().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "recognized text file is not UTF-8",
                ));
            }
            carry.extend_from_slice(&text[error.valid_up_to()..]);
        }
        lines += bytes.iter().filter(|&&byte| byte == b'\n').count() as u64;
        last = bytes.last().copied();
        let length = bytes.len();
        reader.consume(length);
    }
    if !carry.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete UTF-8 sequence",
        ));
    }
    Ok(lines + u64::from(last.is_some_and(|byte| byte != b'\n')))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn metadata_to_open_swaps_cannot_read_outside_controls_or_counted_files() {
        use std::os::unix::fs::symlink;
        // Each swap is injected after metadata inspection, before the real read.
        // The sentinel is only created by this fixture; no genuine directory races.
        for name in ["a.py", ".ignore", ".gitignore", "nested/.ignore"] {
            let dir = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let sentinel = outside.path().join("sentinel");
            fs::write(&sentinel, "x\n").unwrap();
            fs::create_dir(dir.path().join("nested")).unwrap();
            fs::write(dir.path().join(name), "\n\n").unwrap();
            fs::write(dir.path().join("outside.py"), "pass\n").unwrap();
            let options = ScanOptions {
                root: dir.path().into(),
                count_lines: true,
                max_line_count_file_bytes: 100,
                probe_git: false,
            };
            let swapped = std::cell::Cell::new(false);
            let scan = scan_with_open_hook(&options, SCAN_LIMITS, |path| {
                if path.ends_with(name) && !swapped.replace(true) {
                    fs::remove_file(path).unwrap();
                    symlink(&sentinel, path).unwrap();
                }
            });
            assert!(swapped.get());
            if name == "a.py" {
                let scan = scan.unwrap();
                assert!(
                    scan.files
                        .iter()
                        .find(|f| f.relative_path == Path::new(name))
                        .unwrap()
                        .line_count
                        .is_none()
                );
                assert!(
                    scan.warnings
                        .iter()
                        .any(|w| w.message.contains("line count unavailable"))
                );
            } else {
                assert!(matches!(scan, Err(ScanError::IgnorePreflight { .. })));
            }
            assert_eq!(fs::read_to_string(sentinel).unwrap(), "x\n");
        }
    }
    #[test]
    fn malformed_and_growing_ignore_controls_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let options = ScanOptions {
            root: dir.path().into(),
            count_lines: false,
            max_line_count_file_bytes: 100,
            probe_git: false,
        };
        fs::write(dir.path().join(".ignore"), [255]).unwrap();
        assert!(matches!(
            scan_with_limits(&options, SCAN_LIMITS),
            Err(ScanError::IgnorePreflight { .. })
        ));
        fs::write(dir.path().join(".ignore"), "a").unwrap();
        assert!(
            scan_with_open_hook(&options, SCAN_LIMITS, |p| {
                if p.file_name().is_some_and(|n| n == ".ignore") {
                    fs::write(p, "growing").unwrap();
                }
            })
            .is_err()
        );
    }
    #[test]
    fn discovery_budgets_prune_ignored_subtrees_before_loading_nested_controls() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("d")).unwrap();
        fs::write(dir.path().join("d/a.rs"), "fn f() {}\n").unwrap();
        let options = ScanOptions {
            root: dir.path().into(),
            count_lines: false,
            max_line_count_file_bytes: 100,
            probe_git: false,
        };
        let exact = ScanLimits {
            entries: 2,
            path_bytes: "d".len() + "d/a.rs".len(),
            ..SCAN_LIMITS
        };
        let scan = scan_with_limits(&options, exact).unwrap();
        assert_eq!((scan.files.len(), scan.directories.len()), (1, 1));
        for limits in [
            ScanLimits {
                entries: 1,
                ..exact
            },
            ScanLimits {
                path_bytes: exact.path_bytes - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                scan_with_limits(&options, limits),
                Err(ScanError::LimitExceeded)
            ));
        }
        fs::write(dir.path().join(".ignore"), "d/\n").unwrap();
        fs::write(dir.path().join("d/.ignore"), [255]).unwrap();
        let ignored = scan_with_limits(
            &options,
            ScanLimits {
                entries: 2,
                ..SCAN_LIMITS
            },
        )
        .expect("ignored content and its controls are not consulted");
        assert_eq!(ignored.files.len(), 1);
        assert_eq!(ignored.files[0].relative_path, Path::new(".ignore"));
        assert!(ignored.directories.is_empty());

        fs::remove_file(dir.path().join(".ignore")).unwrap();
        fs::remove_file(dir.path().join("d/.ignore")).unwrap();
        for name in ["b.rs", "c.rs"] {
            fs::write(dir.path().join("d").join(name), "fn f() {}\n").unwrap();
        }
        assert!(matches!(
            scan_with_limits(
                &options,
                ScanLimits {
                    entries: 2,
                    ..SCAN_LIMITS
                }
            ),
            Err(ScanError::LimitExceeded)
        ));
    }
    #[test]
    fn warning_ignore_and_line_work_budgets_fail_closed() {
        let mut budget = ScanBudget::new(ScanLimits {
            warnings: 1,
            warning_bytes: 2,
            ..SCAN_LIMITS
        });
        let mut warnings = vec![];
        budget
            .warning(
                &mut warnings,
                ScanWarning {
                    path: None,
                    message: "ab".into(),
                },
            )
            .unwrap();
        assert!(
            budget
                .warning(
                    &mut warnings,
                    ScanWarning {
                        path: None,
                        message: "".into()
                    }
                )
                .is_err()
        );
        assert_eq!(warnings.len(), 1);
        let mut budget = ScanBudget::new(ScanLimits {
            warning_bytes: 1,
            ..SCAN_LIMITS
        });
        assert!(
            budget
                .warning(
                    &mut vec![],
                    ScanWarning {
                        path: None,
                        message: "ab".into()
                    }
                )
                .is_err()
        );
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "ab").unwrap();
        fs::write(dir.path().join("b.rs"), "cd").unwrap();
        let options = ScanOptions {
            root: dir.path().into(),
            count_lines: true,
            max_line_count_file_bytes: 100,
            probe_git: false,
        };
        assert!(
            scan_with_limits(
                &options,
                ScanLimits {
                    line_bytes: 6,
                    ..SCAN_LIMITS
                }
            )
            .is_ok()
        );
        assert!(matches!(
            scan_with_limits(
                &options,
                ScanLimits {
                    line_bytes: 5,
                    ..SCAN_LIMITS
                }
            ),
            Err(ScanError::LimitExceeded)
        ));
        let mut budget = ScanBudget::new(ScanLimits {
            ignore_bytes: 3,
            ..SCAN_LIMITS
        });
        budget.ignore(3).unwrap();
        assert!(budget.ignore(1).is_err());
    }
    #[test]
    fn physical_lines() {
        for (bytes, expected) in [
            ("", 0),
            ("a", 1),
            ("a\n", 1),
            ("a\nb", 2),
            ("\n\n", 2),
            ("a\r\nb\r\n", 2),
            ("a\rb", 1),
            ("é\n", 1),
        ] {
            assert_eq!(count_reader(io::Cursor::new(bytes), 100).unwrap(), expected);
        }
        assert!(count_reader(io::Cursor::new(b"\xff"), 100).is_err());
        assert!(count_reader(io::Cursor::new(b"a\0"), 100).is_err());
        assert!(count_reader(io::Cursor::new("abc"), 2).is_err());
        let reader = BufReader::with_capacity(1, io::Cursor::new("é\n"));
        assert_eq!(count_reader(reader, 3).unwrap(), 1);
    }
    #[test]
    fn read_failure_is_an_error() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "test read denied",
                ))
            }
        }
        assert!(count_reader(BufReader::new(Broken), 100).is_err());
        assert!(count_lines(Path::new("/nonexistent"), Path::new("ctx-file.rs"), 100).is_err());
    }
}
