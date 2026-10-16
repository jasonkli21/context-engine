//! Shared path text rules for query matching and output presentation.
use std::path::Path;

/// Formats a relative path with `/` separators, replacing non-UTF-8 components.
pub fn display_relative(path: &Path) -> String {
    path.iter()
        .map(|component| component.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Formats a relative path with `/` separators only when every component is UTF-8.
pub fn utf8_relative(path: &Path) -> Option<String> {
    path.iter()
        .map(|component| component.to_str())
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn normalized_relative_path_uses_slashes_on_every_platform() {
        let path = PathBuf::from("nested").join("inner.rs");
        assert_eq!(display_relative(&path), "nested/inner.rs");
        assert_eq!(utf8_relative(&path).as_deref(), Some("nested/inner.rs"));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_relative_paths_are_not_searchable_text() {
        use std::os::unix::ffi::OsStrExt;

        let path = Path::new(std::ffi::OsStr::from_bytes(b"odd\xff.rs"));
        assert_eq!(display_relative(path), "odd\u{fffd}.rs");
        assert_eq!(utf8_relative(path), None);
    }
}
