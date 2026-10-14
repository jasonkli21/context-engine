use super::{Envelope, GitDto, WarningDto, escape_controls, relative};
use crate::{index::is_supported, model::*};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct PointDto {
    byte: usize,
    row: usize,
    column: usize,
}
impl From<SourcePoint> for PointDto {
    fn from(p: SourcePoint) -> Self {
        Self {
            byte: p.byte,
            row: p.row,
            column: p.column,
        }
    }
}
#[derive(Serialize)]
struct RangeDto {
    start: PointDto,
    end: PointDto,
}
impl From<SourceRange> for RangeDto {
    fn from(r: SourceRange) -> Self {
        Self {
            start: r.start.into(),
            end: r.end.into(),
        }
    }
}
#[derive(Serialize)]
struct SymbolDto<'a> {
    name: &'a str,
    qualified_name: &'a str,
    kind: &'static str,
    language: String,
    path: String,
    range: RangeDto,
    name_range: RangeDto,
}
impl<'a> From<&'a Symbol> for SymbolDto<'a> {
    fn from(s: &'a Symbol) -> Self {
        Self {
            name: &s.name,
            qualified_name: &s.qualified_name,
            kind: s.kind.name(),
            language: s.language.name().to_lowercase(),
            path: relative(&s.relative_path),
            range: s.range.into(),
            name_range: s.name_range.into(),
        }
    }
}
#[derive(Serialize)]
struct AnalysisDto {
    path: String,
    language: String,
    role: String,
    status: &'static str,
    symbol_count: usize,
    syntax_error_count: usize,
}
#[derive(Serialize)]
struct DiagnosticDto<'a> {
    path: String,
    kind: &'static str,
    message: &'a str,
    range: Option<RangeDto>,
}
#[derive(Serialize)]
struct SummaryDto {
    discovered_files: usize,
    supported_files: usize,
    analyzable_files: usize,
    analyzed_files: usize,
    skipped_files: usize,
    failed_files: usize,
    syntax_error_files: usize,
    syntax_error_count: usize,
    symbols: usize,
    statuses: BTreeMap<&'static str, usize>,
}
#[derive(Serialize)]
struct ReportDto<'a> {
    root: String,
    git: Option<GitDto>,
    summary: SummaryDto,
    languages: BTreeMap<String, usize>,
    kinds: BTreeMap<&'static str, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<AnalysisDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbols: Option<Vec<SymbolDto<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    match_mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    matches: Option<Vec<SymbolDto<'a>>>,
    diagnostics: Vec<DiagnosticDto<'a>>,
    scan_warnings: Vec<WarningDto>,
}
fn summary(index: &RepositoryIndex) -> SummaryDto {
    let mut result = SummaryDto {
        discovered_files: index.files.len(),
        supported_files: 0,
        analyzable_files: 0,
        analyzed_files: 0,
        skipped_files: 0,
        failed_files: 0,
        syntax_error_files: 0,
        syntax_error_count: 0,
        symbols: index.symbols.len(),
        statuses: BTreeMap::new(),
    };
    for f in &index.files {
        *result.statuses.entry(f.status.name()).or_default() += 1;
        if is_supported(&f.language) {
            result.supported_files += 1;
            if !matches!(
                f.role,
                crate::classify::FileRole::Generated | crate::classify::FileRole::Vendor
            ) {
                result.analyzable_files += 1;
            }
        }
        match f.status {
            AnalysisStatus::Analyzed => result.analyzed_files += 1,
            AnalysisStatus::UnsupportedLanguage
            | AnalysisStatus::SkippedRole
            | AnalysisStatus::TooLarge => result.skipped_files += 1,
            _ => result.failed_files += 1,
        }
        result.syntax_error_count += f.syntax_error_count;
        if f.syntax_error_count > 0 {
            result.syntax_error_files += 1;
        }
    }
    result
}
fn distributions(
    index: &RepositoryIndex,
) -> (BTreeMap<String, usize>, BTreeMap<&'static str, usize>) {
    let mut languages = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for s in &index.symbols {
        *languages
            .entry(s.language.name().to_lowercase())
            .or_default() += 1;
        *kinds.entry(s.kind.name()).or_default() += 1;
    }
    (languages, kinds)
}
pub fn json(
    command: &str,
    index: &RepositoryIndex,
    query: Option<&str>,
) -> Result<String, serde_json::Error> {
    let (languages, kinds) = distributions(index);
    let lookup = query.map(|q| index.lookup(q));
    let data = ReportDto {
        root: index.root.to_string_lossy().into_owned(),
        git: index.git.as_ref().map(GitDto::from),
        summary: summary(index),
        languages,
        kinds,
        files: (command == "index").then(|| {
            index
                .files
                .iter()
                .map(|f| AnalysisDto {
                    path: relative(&f.relative_path),
                    language: f.language.name().to_lowercase(),
                    role: f.role.name().to_lowercase(),
                    status: f.status.name(),
                    symbol_count: f.symbol_count,
                    syntax_error_count: f.syntax_error_count,
                })
                .collect()
        }),
        symbols: (command == "symbols")
            .then(|| index.symbols.iter().map(SymbolDto::from).collect()),
        query,
        match_mode: lookup.as_ref().map(|(mode, _)| *mode),
        matches: lookup.map(|(_, matches)| matches.into_iter().map(SymbolDto::from).collect()),
        diagnostics: index
            .diagnostics
            .iter()
            .map(|d| DiagnosticDto {
                path: relative(&d.relative_path),
                kind: d.kind.name(),
                message: &d.message,
                range: d.range.map(RangeDto::from),
            })
            .collect(),
        scan_warnings: index
            .scan_warnings
            .iter()
            .map(|w| WarningDto {
                path: w.path.as_deref().map(relative),
                message: w.message.clone(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&Envelope {
        schema_version: 1,
        command: command.into(),
        data,
    })
}
pub fn human(command: &str, index: &RepositoryIndex, query: Option<&str>) -> String {
    let mut text = format!(
        "Repository index: {}\n",
        escape_controls(&index.root.to_string_lossy())
    );
    let s = summary(index);
    if command == "index" {
        text.push_str(&format!("\nFiles\n  Discovered: {}\n  Supported language: {}\n  Analyzable (before size/read checks): {}\n  Analyzed: {}\n  Skipped: {}\n  Failed: {}\n  Syntax-error files: {}\n  Syntax issues: {}\n", s.discovered_files, s.supported_files, s.analyzable_files, s.analyzed_files, s.skipped_files, s.failed_files, s.syntax_error_files, s.syntax_error_count));
        for (status, count) in &s.statuses {
            text.push_str(&format!("  {status}: {count}\n"));
        }
        text.push_str(&format!("\nSymbols\n  Total: {}\n", s.symbols));
        let (languages, kinds) = distributions(index);
        text.push_str("\nBy language\n");
        for (name, count) in languages {
            text.push_str(&format!("  {name}: {count}\n"));
        }
        text.push_str("\nBy kind\n");
        for (name, count) in kinds {
            text.push_str(&format!("  {name}: {count}\n"));
        }
        if let Some(git) = &index.git {
            text.push_str(&format!(
                "\nGit\n  Root: {}\n  Head: {}\n  Branch: {}\n  Dirty: {}\n",
                escape_controls(&git.repository_root.to_string_lossy()),
                escape_controls(git.head_commit.as_deref().unwrap_or("unavailable")),
                escape_controls(git.branch.as_deref().unwrap_or("unavailable")),
                match git.is_dirty {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unavailable",
                }
            ));
        }
    } else {
        let symbols = if let Some(query) = query {
            let (mode, matches) = index.lookup(query);
            text.push_str(&format!(
                "\n{} {} ({mode}) for {}\n",
                matches.len(),
                if matches.len() == 1 {
                    "match"
                } else {
                    "matches"
                },
                escape_controls(query)
            ));
            matches
        } else {
            index.symbols.iter().collect()
        };
        for symbol in symbols {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}:{}:{}\n",
                symbol.language.name().to_lowercase(),
                symbol.kind.name(),
                escape_controls(&symbol.qualified_name),
                escape_controls(&relative(&symbol.relative_path)),
                symbol.name_range.start.row + 1,
                symbol.name_range.start.column + 1
            ));
        }
    }
    text.push_str(&format!(
        "\nDiagnostics: {} syntax issues, {} failed files; {} scan warnings\n",
        s.syntax_error_count,
        s.failed_files,
        index.scan_warnings.len()
    ));
    for warning in index.scan_warnings.iter().take(20) {
        let path = warning.path.as_deref().map(relative).unwrap_or_default();
        text.push_str(&format!(
            "  {}: {}\n",
            escape_controls(&path),
            escape_controls(&warning.message)
        ));
    }
    text.trim_end().into()
}
