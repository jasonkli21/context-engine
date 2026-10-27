//! Reads only scanner-produced paths. Metadata checks reduce, but cannot eliminate,
//! races with concurrent filesystem mutation; this is not an atomic snapshot.
use crate::model::AnalysisStatus;
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path},
};

pub fn load(root: &Path, relative: &Path, max: u64) -> Result<String, AnalysisStatus> {
    String::from_utf8(load_raw(root, relative, max)?).map_err(|_| AnalysisStatus::InvalidUtf8)
}

pub(crate) fn load_raw(root: &Path, relative: &Path, max: u64) -> Result<Vec<u8>, AnalysisStatus> {
    bounded_bytes(open_checked(root, relative, max)?, max)
}

/// Validate and open an exact regular file without following symlink components.
/// Callers checking declarations can drop the handle without reading file bodies.
pub(crate) fn open_checked(root: &Path, relative: &Path, max: u64) -> Result<File, AnalysisStatus> {
    open_checked_with(root, relative, max, || {})
}
fn open_checked_with(
    root: &Path,
    relative: &Path,
    max: u64,
    before_open: impl FnOnce(),
) -> Result<File, AnalysisStatus> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(AnalysisStatus::ReadFailed);
    }
    // Inspect every component so a directory replaced with a symlink is rejected too.
    let mut path = root.to_path_buf();
    let root_meta = fs::symlink_metadata(&path).map_err(|_| AnalysisStatus::ReadFailed)?;
    if !root_meta.is_dir() {
        return Err(AnalysisStatus::ReadFailed);
    }
    for component in relative.components() {
        path.push(component);
        let meta = fs::symlink_metadata(&path).map_err(|_| AnalysisStatus::ReadFailed)?;
        if meta.file_type().is_symlink() {
            return Err(AnalysisStatus::ReadFailed);
        }
    }
    let meta = fs::symlink_metadata(&path).map_err(|_| AnalysisStatus::ReadFailed)?;
    if !meta.is_file() {
        return Err(AnalysisStatus::ReadFailed);
    }
    if meta.len() > max {
        return Err(AnalysisStatus::TooLarge);
    }
    #[cfg(not(unix))]
    {
        // A metadata check followed by an ordinary path open can follow a
        // reparse point installed between the two operations. Until this
        // platform has a handle-relative no-reparse implementation, fail closed.
        let _ = before_open;
        Err(AnalysisStatus::ReadFailed)
    }
    #[cfg(unix)]
    {
        before_open();
        // Open each component relative to a held directory descriptor: parent swaps
        // cannot redirect this read through a symlink outside the scan root.
        use rustix::fs::{Mode, OFlags, open, openat};
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        // Canonical callers use absolute roots. Anchor from / and reject links in
        // every root component too, including ancestors of a nested scan scope.
        if !root.is_absolute() {
            return Err(AnalysisStatus::ReadFailed);
        }
        let mut dir = open("/", flags | OFlags::DIRECTORY, Mode::empty())
            .map_err(|_| AnalysisStatus::ReadFailed)?;
        for component in root.components() {
            if let Component::Normal(part) = component {
                dir = openat(&dir, part, flags | OFlags::DIRECTORY, Mode::empty())
                    .map_err(|_| AnalysisStatus::ReadFailed)?;
            }
        }
        let parts: Vec<_> = relative.components().collect();
        for part in &parts[..parts.len() - 1] {
            dir = openat(
                &dir,
                part.as_os_str(),
                flags | OFlags::DIRECTORY,
                Mode::empty(),
            )
            .map_err(|_| AnalysisStatus::ReadFailed)?;
        }
        let file: File = openat(
            &dir,
            parts.last().unwrap().as_os_str(),
            flags | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| AnalysisStatus::ReadFailed)?
        .into();
        let opened = file.metadata().map_err(|_| AnalysisStatus::ReadFailed)?;
        if !opened.is_file() {
            return Err(AnalysisStatus::ReadFailed);
        }
        if opened.len() > max {
            return Err(AnalysisStatus::TooLarge);
        }
        Ok(file)
    }
}
#[cfg(test)]
fn bounded_utf8(reader: impl Read, max: u64) -> Result<String, AnalysisStatus> {
    String::from_utf8(bounded_bytes(reader, max)?).map_err(|_| AnalysisStatus::InvalidUtf8)
}
fn bounded_bytes(reader: impl Read, max: u64) -> Result<Vec<u8>, AnalysisStatus> {
    let mut bytes = Vec::new();
    reader
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_: io::Error| AnalysisStatus::ReadFailed)?;
    if bytes.len() as u64 > max {
        return Err(AnalysisStatus::TooLarge);
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_and_invalid_inputs() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "é").unwrap();
        assert_eq!(
            load(&fs::canonicalize(dir.path()).unwrap(), Path::new("a.rs"), 2).unwrap(),
            "é"
        );
        assert_eq!(
            load(&fs::canonicalize(dir.path()).unwrap(), Path::new("a.rs"), 1),
            Err(AnalysisStatus::TooLarge)
        );
        fs::write(dir.path().join("bad.py"), [255]).unwrap();
        assert_eq!(
            load(
                &fs::canonicalize(dir.path()).unwrap(),
                Path::new("bad.py"),
                10
            ),
            Err(AnalysisStatus::InvalidUtf8)
        );
        for path in ["../a.rs", "/a.rs", "", "missing.rs"] {
            assert_eq!(
                load(&fs::canonicalize(dir.path()).unwrap(), Path::new(path), 10),
                Err(AnalysisStatus::ReadFailed)
            );
        }
        assert_eq!(
            bounded_utf8(&b"grown"[..], 4),
            Err(AnalysisStatus::TooLarge)
        );
        assert_eq!(bounded_utf8(&b""[..], 0).unwrap(), "");
    }
    #[cfg(unix)]
    #[test]
    fn post_metadata_file_and_ancestor_swaps_fail_before_opening_sentinel() {
        use std::os::unix::fs::symlink;
        for swap_parent in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(dir.path()).unwrap();
            let outside = tempfile::tempdir().unwrap();
            fs::create_dir(root.join("nested")).unwrap();
            fs::write(root.join("nested/a.py"), "in\n").unwrap();
            fs::write(outside.path().join("a.py"), "out").unwrap();
            let opened = open_checked_with(&root, Path::new("nested/a.py"), 100, || {
                if swap_parent {
                    fs::rename(root.join("nested"), root.join("held")).unwrap();
                    symlink(outside.path(), root.join("nested")).unwrap();
                } else {
                    fs::remove_file(root.join("nested/a.py")).unwrap();
                    symlink(outside.path().join("a.py"), root.join("nested/a.py")).unwrap();
                }
            });
            // NOFOLLOW rejects at openat itself, after all metadata checks. No
            // descriptor for the outside sentinel can be returned or read.
            assert!(matches!(opened, Err(AnalysisStatus::ReadFailed)));
        }
    }
    #[cfg(unix)]
    #[test]
    fn rejects_file_and_parent_symlinks() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        symlink(dir.path().join("a.rs"), dir.path().join("link.rs")).unwrap();
        symlink(dir.path(), dir.path().join("nested")).unwrap();
        for path in ["link.rs", "nested/a.rs"] {
            assert_eq!(
                load(&fs::canonicalize(dir.path()).unwrap(), Path::new(path), 100),
                Err(AnalysisStatus::ReadFailed)
            );
        }
    }
    #[cfg(not(unix))]
    #[test]
    fn non_unix_checked_reads_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        assert_eq!(
            load_raw(
                &fs::canonicalize(dir.path()).unwrap(),
                Path::new("a.rs"),
                100
            ),
            Err(AnalysisStatus::ReadFailed)
        );
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;
    struct Failed;
    impl Read for Failed {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("unreadable"))
        }
    }
    #[test]
    fn read_error_and_bounded_growth() {
        assert_eq!(bounded_utf8(Failed, 10), Err(AnalysisStatus::ReadFailed));
        // The read bound is independent of metadata, including when a source grows.
        let mut input = io::Cursor::new(vec![b'a'; 100]);
        assert_eq!(bounded_utf8(&mut input, 10), Err(AnalysisStatus::TooLarge));
        assert_eq!(input.position(), 11);
    }
}
