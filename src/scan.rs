use crate::{
    classify::{classify_language, classify_role},
    git,
    model::{FileRecord, RepositoryScan, ScanOptions, ScanWarning},
};
use ignore::WalkBuilder;
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScanError {
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

pub fn scan_repository(options: &ScanOptions) -> Result<RepositoryScan, ScanError> {
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
    reject_symlink_ignore_files(&root)?;
    let mut scan = RepositoryScan {
        root: root.clone(),
        files: vec![],
        directories: vec![],
        git: None,
        warnings: vec![],
    };
    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .git_global(false)
        .git_exclude(false)
        .filter_entry(|entry| entry.depth() == 0 || entry.file_name() != ".git");
    for result in builder.build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(source) => {
                if source.depth() == Some(0) {
                    return Err(ScanError::Traversal { path: root, source });
                }
                scan.warnings.push(ScanWarning {
                    path: source
                        .path_in_error()
                        .and_then(|path| path.strip_prefix(&root).ok())
                        .map(Path::to_path_buf),
                    message: format!("traversal: {source}"),
                });
                continue;
            }
        };
        if let Some(error) = entry.error() {
            scan.warnings.push(ScanWarning {
                path: entry.path().strip_prefix(&root).ok().map(Path::to_path_buf),
                message: format!("ignore rules: {error}"),
            });
        }
        if entry.depth() == 0 {
            continue;
        }
        let relative_path = match entry.path().strip_prefix(&root) {
            Ok(path) => path.to_path_buf(),
            Err(error) => {
                scan.warnings.push(ScanWarning {
                    path: None,
                    message: format!("skipped path outside scan root: {error}"),
                });
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
                scan.warnings.push(ScanWarning {
                    path: Some(relative_path),
                    message: format!("metadata: {error}"),
                });
                continue;
            }
        };
        let language = classify_language(&relative_path);
        let role = classify_role(&relative_path, &language);
        let line_count = if options.count_lines
            && language.is_text()
            && metadata.len() <= options.max_line_count_file_bytes
        {
            match count_lines(entry.path(), options.max_line_count_file_bytes) {
                Ok(lines) => Some(lines),
                Err(error) => {
                    scan.warnings.push(ScanWarning {
                        path: Some(relative_path.clone()),
                        message: format!("line count unavailable: {error}"),
                    });
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
    scan.files
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    scan.directories.sort();
    if options.probe_git {
        let (metadata, warnings) = git::probe(&root);
        scan.git = metadata;
        scan.warnings.extend(warnings);
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

fn reject_symlink_ignore_files(root: &Path) -> Result<(), ScanError> {
    fn check_candidate(directory: &Path) -> Result<(), ScanError> {
        for name in [".gitignore", ".ignore"] {
            let path = directory.join(name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(ScanError::SymlinkIgnoreFile { path });
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(ScanError::IgnorePreflight { path, source });
                }
            }
        }
        Ok(())
    }

    // WalkBuilder can read parent ignore files, so validate those candidates too.
    for directory in root.ancestors() {
        if !directory
            .components()
            .any(|component| component.as_os_str() == ".git")
        {
            check_candidate(directory)?;
        }
    }

    // Traverse without ignore processing so no ignore control file is opened before
    // its file type is checked. Symlinked directories are never followed.
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
        .filter_entry(|entry| entry.depth() == 0 || entry.file_name() != ".git");
    for result in builder.build() {
        let entry = match result {
            Ok(entry) => entry,
            // The regular traversal reports recoverable child failures. A directory
            // that cannot be entered cannot expose an ignore control file to either
            // traversal, so it is safe to defer its diagnostic.
            Err(error) if error.depth() != Some(0) => continue,
            Err(source) => {
                return Err(ScanError::Traversal {
                    path: root.to_path_buf(),
                    source,
                });
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink()
            && matches!(entry.file_name().to_str(), Some(".gitignore" | ".ignore"))
        {
            return Err(ScanError::SymlinkIgnoreFile {
                path: entry.path().to_path_buf(),
            });
        }
    }
    Ok(())
}

fn count_lines(path: &Path, max_bytes: u64) -> io::Result<u64> {
    let file = File::open(path)?;
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
        assert!(count_lines(Path::new("/nonexistent/ctx-file.rs"), 100).is_err());
    }
}
