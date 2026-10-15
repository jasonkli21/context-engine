//! Parser-independent syntax facts. Extraction and target resolution are separate.
use crate::{classify::Language, model::SourceRange};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationKind {
    Contains,
    Imports,
    References,
}
impl RelationKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Imports => "imports",
            Self::References => "references",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Derivation {
    RepositoryContainsFile,
    LexicalRangeContainment,
    RustUseSyntax,
    PythonImportSyntax,
    RustCallSyntax,
    PythonCallSyntax,
}
impl Derivation {
    pub fn name(self) -> &'static str {
        match self {
            Self::RepositoryContainsFile => "repository_contains_file",
            Self::LexicalRangeContainment => "lexical_range_containment",
            Self::RustUseSyntax => "rust_use_syntax",
            Self::PythonImportSyntax => "python_import_syntax",
            Self::RustCallSyntax => "rust_call_syntax",
            Self::PythonCallSyntax => "python_call_syntax",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Resolution {
    DirectStructural,
    ExactSameFileQualifiedName,
    UniqueSameFileLocalName,
    Unresolved,
}
impl Resolution {
    pub fn name(self) -> &'static str {
        match self {
            Self::DirectStructural => "direct_structural",
            Self::ExactSameFileQualifiedName => "exact_same_file_qualified_name",
            Self::UniqueSameFileLocalName => "unique_same_file_local_name",
            Self::Unresolved => "unresolved",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceContext {
    Call,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Metadata {
    None,
    Import {
        binding: Option<String>,
        wildcard: bool,
    },
    Reference {
        context: ReferenceContext,
    },
}
#[derive(Debug)]
pub struct Evidence {
    pub relative_path: PathBuf,
    pub range: Option<SourceRange>,
    pub language: Option<Language>,
    pub derivation: Derivation,
}
#[derive(Debug)]
pub struct Candidate {
    pub kind: RelationKind,
    pub target: String,
    pub range: SourceRange,
    pub metadata: Metadata,
    pub derivation: Derivation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationStatus {
    Analyzed,
    ExtractionFailed,
    AnalysisLimitExceeded,
}
impl RelationStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Analyzed => "analyzed",
            Self::ExtractionFailed => "extraction_failed",
            Self::AnalysisLimitExceeded => "analysis_limit_exceeded",
        }
    }
}
#[derive(Debug)]
pub struct Extraction {
    pub status: RelationStatus,
    pub candidates: Vec<Candidate>,
    pub unsupported: usize,
}
pub struct RelationFileAnalysis {
    pub relative_path: PathBuf,
    pub status: RelationStatus,
    pub import_count: usize,
    pub reference_count: usize,
    pub unresolved_count: usize,
    pub unsupported_candidate_count: usize,
}
pub struct FileRelations {
    pub relative_path: PathBuf,
    pub extraction: Extraction,
}
