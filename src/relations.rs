//! Parser-independent syntax facts. Extraction and target resolution are separate.
use crate::{classify::Language, model::SourceRange};
use std::path::PathBuf;

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum RelationKind {
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "imports")]
    Imports,
    #[serde(rename = "references")]
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
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum Derivation {
    #[serde(rename = "repository_contains_file")]
    RepositoryContainsFile,
    #[serde(rename = "lexical_range_containment")]
    LexicalRangeContainment,
    #[serde(rename = "rust_use_syntax")]
    RustUseSyntax,
    #[serde(rename = "python_import_syntax")]
    PythonImportSyntax,
    #[serde(rename = "rust_call_syntax")]
    RustCallSyntax,
    #[serde(rename = "python_call_syntax")]
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
    Unresolved,
}
impl Resolution {
    pub fn name(self) -> &'static str {
        match self {
            Self::DirectStructural => "direct_structural",
            Self::ExactSameFileQualifiedName => "exact_same_file_qualified_name",
            Self::Unresolved => "unresolved",
        }
    }
}
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum ReferenceContext {
    #[serde(rename = "call")]
    Call,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Metadata {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "import")]
    Import {
        binding: Option<String>,
        wildcard: bool,
    },
    #[serde(rename = "reference")]
    Reference { context: ReferenceContext },
}
#[derive(Debug)]
pub struct Evidence {
    pub relative_path: PathBuf,
    pub range: Option<SourceRange>,
    pub language: Option<Language>,
    pub derivation: Derivation,
}
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Candidate {
    pub kind: RelationKind,
    pub target: String,
    pub range: SourceRange,
    pub metadata: Metadata,
    pub derivation: Derivation,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationStatus {
    #[serde(rename = "analyzed")]
    Analyzed,
    #[serde(rename = "extraction_failed")]
    ExtractionFailed,
    #[serde(rename = "analysis_limit_exceeded")]
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
#[derive(serde::Serialize, serde::Deserialize, Debug)]
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

/// Owned candidate text and the evidence path duplicated by graph construction.
pub fn candidate_bytes(c: &Candidate, path: &std::path::Path) -> usize {
    c.target
        .len()
        .saturating_add(path.as_os_str().len())
        .saturating_add(match &c.metadata {
            Metadata::Import { binding, .. } => binding.as_ref().map_or(0, String::len),
            _ => 0,
        })
}
