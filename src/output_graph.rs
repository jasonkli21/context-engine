//! Explicit schema-v1 adapters; no domain serialization or source access.
use super::{
    Envelope, GitDto, WarningDto, escape_controls,
    index::{RangeDto, SymbolDto},
    relative,
};
use crate::{graph::*, model::AnalysisStatus, relations::*};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct EntityDto<'a> {
    id: u32,
    kind: &'static str,
    path: Option<String>,
    symbol: Option<SymbolDto<'a>>,
}
fn entity(g: &RepositoryGraph, id: EntityId) -> EntityDto<'_> {
    let (kind, path, symbol) = match g.entities[id.0 as usize] {
        Entity::Repository => ("repository", None, None),
        Entity::File(i) => (
            "file",
            Some(relative(&g.index.files[i].relative_path)),
            None,
        ),
        Entity::Symbol(i) => (
            "symbol",
            Some(relative(&g.index.symbols[i].relative_path)),
            Some(SymbolDto::from(&g.index.symbols[i])),
        ),
    };
    EntityDto {
        id: id.0,
        kind,
        path,
        symbol,
    }
}
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum TargetDto<'a> {
    Resolved { entity_id: u32 },
    Unresolved { name: &'a str },
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MetadataDto<'a> {
    None,
    Import {
        binding: Option<&'a str>,
        wildcard: bool,
    },
    Reference {
        reference_context: &'static str,
    },
}
#[derive(Serialize)]
struct EvidenceDto {
    path: String,
    range: Option<RangeDto>,
    language: Option<String>,
    derivation: &'static str,
}
#[derive(Serialize)]
struct RelationDto<'a> {
    id: u32,
    kind: &'static str,
    source_id: u32,
    target: TargetDto<'a>,
    resolution: &'static str,
    metadata: MetadataDto<'a>,
    evidence: EvidenceDto,
}
fn relation(g: &RepositoryGraph, id: RelationId) -> RelationDto<'_> {
    let r = &g.relations[id.0 as usize];
    RelationDto {
        id: id.0,
        kind: r.kind.name(),
        source_id: r.source.0,
        target: match &r.target {
            Target::Resolved(t) => TargetDto::Resolved { entity_id: t.0 },
            Target::Unresolved(name) => TargetDto::Unresolved { name },
        },
        resolution: r.resolution.name(),
        metadata: match &r.metadata {
            Metadata::None => MetadataDto::None,
            Metadata::Import { binding, wildcard } => MetadataDto::Import {
                binding: binding.as_deref(),
                wildcard: *wildcard,
            },
            Metadata::Reference {
                context: ReferenceContext::Call,
            } => MetadataDto::Reference {
                reference_context: "call",
            },
        },
        evidence: EvidenceDto {
            path: relative(&r.evidence.relative_path),
            range: r.evidence.range.map(RangeDto::from),
            language: r
                .evidence
                .language
                .as_ref()
                .map(|l| l.name().to_lowercase()),
            derivation: r.evidence.derivation.name(),
        },
    }
}
#[derive(Serialize)]
struct FileDto {
    path: String,
    language: String,
    symbol_status: &'static str,
    symbol_count: usize,
    syntax_error_count: usize,
}
#[derive(Serialize)]
struct RelationFileDto {
    path: String,
    status: &'static str,
    import_count: usize,
    reference_count: usize,
    unresolved_count: usize,
    unsupported_candidate_count: usize,
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
    entities: usize,
    relations: usize,
    resolved: usize,
    unresolved: usize,
    relation_statuses: BTreeMap<&'static str, usize>,
    symbol_analysis_unavailable: usize,
    unsupported_candidates: usize,
}
fn summary(g: &RepositoryGraph) -> SummaryDto {
    let mut statuses = BTreeMap::new();
    for f in &g.relation_files {
        *statuses.entry(f.status.name()).or_default() += 1;
    }
    let unresolved = g
        .relations
        .iter()
        .filter(|r| matches!(r.target, Target::Unresolved(_)))
        .count();
    SummaryDto {
        entities: g.entities.len(),
        relations: g.relations.len(),
        resolved: g.relations.len() - unresolved,
        unresolved,
        relation_statuses: statuses,
        symbol_analysis_unavailable: g
            .index
            .files
            .iter()
            .filter(|f| f.status != AnalysisStatus::Analyzed)
            .count(),
        unsupported_candidates: g
            .relation_files
            .iter()
            .map(|f| f.unsupported_candidate_count)
            .sum(),
    }
}
#[derive(Serialize)]
struct RootDto<'a> {
    root_id: u32,
    entities: Vec<EntityDto<'a>>,
    relations: Vec<RelationDto<'a>>,
    truncated: bool,
}
#[derive(Serialize)]
struct ReportDto<'a> {
    root: String,
    git: Option<GitDto>,
    id_semantics: &'static str,
    summary: SummaryDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    entities: Option<Vec<EntityDto<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relations: Option<Vec<RelationDto<'a>>>,
    files: Vec<FileDto>,
    relation_files: Vec<RelationFileDto>,
    diagnostics: Vec<DiagnosticDto<'a>>,
    scan_warnings: Vec<WarningDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    match_mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roots: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    results: Option<Vec<RootDto<'a>>>,
}
pub fn json<'a>(
    command: &str,
    g: &'a RepositoryGraph,
    query: Option<&'a str>,
    result: Option<&QueryResult>,
) -> Result<String, serde_json::Error> {
    let data = ReportDto {
        root: g.index.root.to_string_lossy().into_owned(),
        git: g.index.git.as_ref().map(GitDto::from),
        id_semantics: "index-local; deterministic for stable input; not stable across edits or lineage IDs",
        summary: summary(g),
        entities: result.is_none().then(|| {
            (0..g.entities.len())
                .map(|i| entity(g, EntityId(i as u32)))
                .collect()
        }),
        relations: result.is_none().then(|| {
            (0..g.relations.len())
                .map(|i| relation(g, RelationId(i as u32)))
                .collect()
        }),
        files: g
            .index
            .files
            .iter()
            .map(|f| FileDto {
                path: relative(&f.relative_path),
                language: f.language.name().to_lowercase(),
                symbol_status: f.status.name(),
                symbol_count: f.symbol_count,
                syntax_error_count: f.syntax_error_count,
            })
            .collect(),
        relation_files: g
            .relation_files
            .iter()
            .map(|f| RelationFileDto {
                path: relative(&f.relative_path),
                status: f.status.name(),
                import_count: f.import_count,
                reference_count: f.reference_count,
                unresolved_count: f.unresolved_count,
                unsupported_candidate_count: f.unsupported_candidate_count,
            })
            .collect(),
        diagnostics: g
            .index
            .diagnostics
            .iter()
            .map(|d| DiagnosticDto {
                path: relative(&d.relative_path),
                kind: d.kind.name(),
                message: &d.message,
                range: d.range.map(RangeDto::from),
            })
            .collect(),
        scan_warnings: g
            .index
            .scan_warnings
            .iter()
            .map(|w| WarningDto {
                path: w.path.as_deref().map(relative),
                message: w.message.clone(),
            })
            .collect(),
        query,
        match_mode: result.map(|r| r.match_mode),
        roots: result.map(|r| r.roots.iter().map(|id| id.0).collect()),
        depth: result.map(|r| r.depth),
        truncated: result.map(|r| r.truncated),
        results: result.map(|r| {
            r.results
                .iter()
                .map(|r| RootDto {
                    root_id: r.root.0,
                    entities: r.entities.iter().map(|id| entity(g, *id)).collect(),
                    relations: r.relations.iter().map(|id| relation(g, *id)).collect(),
                    truncated: r.truncated,
                })
                .collect()
        }),
    };
    serde_json::to_string_pretty(&Envelope {
        schema_version: 1,
        command: command.into(),
        data,
    })
}
fn label(g: &RepositoryGraph, id: EntityId) -> String {
    if let Some(s) = g.symbol(id) {
        format!(
            "{} ({}:{})",
            s.qualified_name,
            relative(&s.relative_path),
            s.name_range.start.row + 1
        )
    } else {
        match g.entities[id.0 as usize] {
            Entity::Repository => "repository".into(),
            Entity::File(i) => relative(&g.index.files[i].relative_path),
            _ => unreachable!(),
        }
    }
}
pub fn human(
    command: &str,
    g: &RepositoryGraph,
    query: Option<&str>,
    result: Option<&QueryResult>,
) -> String {
    let s = summary(g);
    let mut text = format!(
        "Repository graph: {}\nEntities: {}; relations: {} ({} resolved, {} unresolved)\nRelation files: {:?}; symbol analysis unavailable: {}; unsupported candidates: {}\nIDs are index-local; evidence is syntactic and best effort.\n",
        escape_controls(&g.index.root.to_string_lossy()),
        s.entities,
        s.relations,
        s.resolved,
        s.unresolved,
        s.relation_statuses,
        s.symbol_analysis_unavailable,
        s.unsupported_candidates
    );
    if let Some(result) = result {
        text.push_str(&format!(
            "{}: {} roots for {} ({}), depth {}\n",
            if command == "callers" {
                "Syntactic callers"
            } else {
                command
            },
            result.roots.len(),
            escape_controls(query.unwrap_or_default()),
            result.match_mode,
            result.depth
        ));
        for root in &result.results {
            text.push_str(&format!(
                "\nRoot #{} {}\n",
                root.root.0,
                escape_controls(&label(g, root.root))
            ));
            for id in &root.relations {
                let r = &g.relations[id.0 as usize];
                let target = match &r.target {
                    Target::Resolved(id) => label(g, *id),
                    Target::Unresolved(name) => format!("{name} [unresolved]"),
                };
                text.push_str(&format!(
                    "  #{} {} --{}--> {} [{}]\n",
                    id.0,
                    escape_controls(&label(g, r.source)),
                    r.kind.name(),
                    escape_controls(&target),
                    r.resolution.name()
                ));
            }
            if root.truncated {
                text.push_str("  truncated: true\n");
            }
        }
        text.push_str(&format!("\ntruncated: {}\n", result.truncated));
    }
    text.trim_end().into()
}
