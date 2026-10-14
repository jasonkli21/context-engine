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

pub const DEFAULT_MAX_PARSE_BYTES: u64 = 1024 * 1024;
pub struct IndexOptions {
    pub root: PathBuf,
    pub max_parse_file_bytes: u64,
    pub probe_git: bool,
}
/// UTF-8 byte offsets, zero-based rows and byte columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourcePoint {
    pub byte: usize,
    pub row: usize,
    pub column: usize,
}
/// Half-open byte interval [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceRange {
    pub start: SourcePoint,
    pub end: SourcePoint,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Class,
    Struct,
    Enum,
    Union,
    Trait,
    Function,
    Method,
    TypeAlias,
    Constant,
    Static,
}
impl SymbolKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Union => "union",
            Self::Trait => "trait",
            Self::Function => "function",
            Self::Method => "method",
            Self::TypeAlias => "type_alias",
            Self::Constant => "constant",
            Self::Static => "static",
        }
    }
}
#[derive(Debug)]
pub struct Symbol {
    pub relative_path: PathBuf,
    pub language: Language,
    pub kind: SymbolKind,
    pub name: String,
    pub qualified_name: String,
    pub range: SourceRange,
    pub name_range: SourceRange,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisStatus {
    Analyzed,
    UnsupportedLanguage,
    SkippedRole,
    TooLarge,
    InvalidUtf8,
    ReadFailed,
    ParseFailed,
    AnalysisLimitExceeded,
}
impl AnalysisStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Analyzed => "analyzed",
            Self::UnsupportedLanguage => "unsupported_language",
            Self::SkippedRole => "skipped_role",
            Self::TooLarge => "too_large",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::ReadFailed => "read_failed",
            Self::ParseFailed => "parse_failed",
            Self::AnalysisLimitExceeded => "analysis_limit_exceeded",
        }
    }
}
#[derive(Debug)]
pub struct FileAnalysis {
    pub relative_path: PathBuf,
    pub language: Language,
    pub role: FileRole,
    pub status: AnalysisStatus,
    pub symbol_count: usize,
    pub syntax_error_count: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    SyntaxError,
    MissingSyntax,
    ReadError,
    InvalidUtf8,
    ParseFailure,
    AnalysisLimitExceeded,
}
impl DiagnosticKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::SyntaxError => "syntax_error",
            Self::MissingSyntax => "missing_syntax",
            Self::ReadError => "read_error",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::ParseFailure => "parse_failure",
            Self::AnalysisLimitExceeded => "analysis_limit_exceeded",
        }
    }
}
#[derive(Debug)]
pub struct AnalysisDiagnostic {
    pub relative_path: PathBuf,
    pub kind: DiagnosticKind,
    pub message: String,
    pub range: Option<SourceRange>,
}
pub struct AnalyzedFile {
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub syntax_error_count: usize,
}
pub struct RepositoryIndex {
    pub root: PathBuf,
    pub git: Option<GitMetadata>,
    pub scan_warnings: Vec<ScanWarning>,
    pub files: Vec<FileAnalysis>,
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}
impl RepositoryIndex {
    pub fn sort(&mut self) {
        self.files
            .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        self.symbols.sort_by(|a, b| {
            a.relative_path
                .cmp(&b.relative_path)
                .then(a.range.start.byte.cmp(&b.range.start.byte))
                .then(a.range.end.byte.cmp(&b.range.end.byte))
                .then(a.kind.name().cmp(b.kind.name()))
                .then(a.qualified_name.cmp(&b.qualified_name))
        });
        self.diagnostics.sort_by(|a, b| {
            a.relative_path
                .cmp(&b.relative_path)
                .then(a.range.cmp(&b.range))
                .then(a.kind.name().cmp(b.kind.name()))
                .then(a.message.cmp(&b.message))
        });
        self.scan_warnings
            .sort_by(|a, b| a.path.cmp(&b.path).then(a.message.cmp(&b.message)));
    }
    pub fn lookup(&self, query: &str) -> (&'static str, Vec<&Symbol>) {
        let qualified: Vec<_> = self
            .symbols
            .iter()
            .filter(|s| s.qualified_name == query)
            .collect();
        if !qualified.is_empty() {
            return ("qualified_name", qualified);
        }
        let local: Vec<_> = self.symbols.iter().filter(|s| s.name == query).collect();
        (
            if local.is_empty() {
                "none"
            } else {
                "local_name"
            },
            local,
        )
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    #[test]
    fn stable_kind_names_and_explicit_sort_ties() {
        let kinds = [
            SymbolKind::Module,
            SymbolKind::Class,
            SymbolKind::Struct,
            SymbolKind::Enum,
            SymbolKind::Union,
            SymbolKind::Trait,
            SymbolKind::Function,
            SymbolKind::Method,
            SymbolKind::TypeAlias,
            SymbolKind::Constant,
            SymbolKind::Static,
        ];
        let names: std::collections::BTreeSet<_> = kinds.iter().map(|kind| kind.name()).collect();
        assert_eq!(names.len(), 11);
        assert!(names.contains("type_alias"));
        let range = SourceRange {
            start: SourcePoint {
                byte: 0,
                row: 0,
                column: 0,
            },
            end: SourcePoint {
                byte: 1,
                row: 0,
                column: 1,
            },
        };
        let mut index = RepositoryIndex {
            root: "/fixture".into(),
            git: None,
            scan_warnings: vec![],
            files: vec![],
            diagnostics: vec![],
            symbols: [
                ("b.rs", SymbolKind::Class, "z"),
                ("a.rs", SymbolKind::Struct, "a"),
                ("a.rs", SymbolKind::Function, "z"),
                ("a.rs", SymbolKind::Function, "a"),
            ]
            .into_iter()
            .map(|(path, kind, name)| Symbol {
                relative_path: path.into(),
                language: Language::Rust,
                kind,
                name: name.into(),
                qualified_name: name.into(),
                range,
                name_range: range,
            })
            .collect(),
        };
        index.sort();
        assert_eq!(
            index
                .symbols
                .iter()
                .map(|s| s.qualified_name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "z", "a", "z"]
        );
        assert_eq!(index.symbols[1].kind, SymbolKind::Function);
        assert_eq!(index.symbols[2].kind, SymbolKind::Struct);
    }
}
