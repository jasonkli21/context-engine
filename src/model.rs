use crate::classify::{FileRole, Language};
use std::{collections::BTreeMap, path::PathBuf};

pub const DEFAULT_MAX_LINE_BYTES: u64 = 1024 * 1024;
pub struct ScanOptions {
    pub root: PathBuf,
    pub count_lines: bool,
    pub max_line_count_file_bytes: u64,
    pub probe_git: bool,
}
#[derive(Debug)]
pub struct RepositoryScan {
    pub root: PathBuf,
    pub files: Vec<FileRecord>,
    // Retain empty directories so tree rendering never needs another walker.
    pub directories: Vec<PathBuf>,
    pub git: Option<GitMetadata>,
    pub warnings: Vec<ScanWarning>,
}
#[derive(Debug)]
pub struct FileRecord {
    pub relative_path: PathBuf,
    pub language: Language,
    pub role: FileRole,
    pub size_bytes: u64,
    pub line_count: Option<u64>,
}
#[derive(Debug)]
pub struct GitMetadata {
    pub repository_root: PathBuf,
    pub head_commit: Option<String>,
    pub branch: Option<String>,
    pub is_dirty: Option<bool>,
}
#[derive(Debug)]
pub struct ScanWarning {
    pub path: Option<PathBuf>,
    pub message: String,
}
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub files: usize,
    pub bytes: u64,
    pub counted_lines: u64,
    pub files_with_line_counts: usize,
}
impl Counts {
    fn add(&mut self, file: &FileRecord) {
        self.files += 1;
        self.bytes += file.size_bytes;
        if let Some(lines) = file.line_count {
            self.counted_lines += lines;
            self.files_with_line_counts += 1;
        }
    }
}
pub struct Statistics<'a> {
    pub totals: Counts,
    pub languages: Vec<(Language, Counts)>,
    pub roles: Vec<(FileRole, usize)>,
    pub largest_files: Vec<&'a FileRecord>,
}
pub fn aggregate(files: &[FileRecord]) -> Statistics<'_> {
    let mut totals = Counts::default();
    let mut languages = BTreeMap::<Language, Counts>::new();
    let mut roles = BTreeMap::<FileRole, usize>::new();
    for file in files {
        totals.add(file);
        languages
            .entry(file.language.clone())
            .or_default()
            .add(file);
        *roles.entry(file.role).or_default() += 1;
    }
    let mut languages: Vec<_> = languages.into_iter().collect();
    languages.sort_by(|a, b| {
        b.1.files
            .cmp(&a.1.files)
            .then_with(|| a.0.name().cmp(b.0.name()))
    });
    let mut roles: Vec<_> = roles.into_iter().collect();
    roles.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name().cmp(b.0.name())));
    let mut largest_files: Vec<_> = files.iter().collect();
    largest_files.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    largest_files.truncate(10);
    Statistics {
        totals,
        languages,
        roles,
        largest_files,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregation_and_ties() {
        let files = vec![
            FileRecord {
                relative_path: "b.rs".into(),
                language: Language::Rust,
                role: FileRole::Source,
                size_bytes: 8,
                line_count: Some(2),
            },
            FileRecord {
                relative_path: "a.py".into(),
                language: Language::Python,
                role: FileRole::Test,
                size_bytes: 8,
                line_count: None,
            },
            FileRecord {
                relative_path: "c.rs".into(),
                language: Language::Rust,
                role: FileRole::Source,
                size_bytes: 2,
                line_count: Some(0),
            },
        ];
        let stats = aggregate(&files);
        assert_eq!(
            stats.totals,
            Counts {
                files: 3,
                bytes: 18,
                counted_lines: 2,
                files_with_line_counts: 2
            }
        );
        assert_eq!(
            stats.languages[0],
            (
                Language::Rust,
                Counts {
                    files: 2,
                    bytes: 10,
                    counted_lines: 2,
                    files_with_line_counts: 2
                }
            )
        );
        assert_eq!(
            stats.roles,
            vec![(FileRole::Source, 2), (FileRole::Test, 1)]
        );
        assert_eq!(stats.largest_files[0].relative_path, PathBuf::from("a.py"));
        let tie = aggregate(&files[..2]);
        assert_eq!(tie.languages[0].0, Language::Python);
        assert_eq!(aggregate(&[]).totals, Counts::default());
    }
}
