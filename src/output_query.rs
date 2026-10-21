//! Command-specific schema-v1 adapters over query results.
use super::{
    Envelope, MAX_JSON_BYTES, bounded_json, escape_controls,
    graph::{
        EntityDto, RelationDto, SummaryDto as GraphSummaryDto, entity, index_entity, label,
        relation, summary as graph_summary,
    },
    index::{SummaryDto as AnalysisSummaryDto, summary as analysis_summary},
    relative,
};
use crate::{classify::FileRole, graph::*, model::RepositoryIndex, query::*};
use serde::Serialize;

const ID_SEMANTICS: &str = "index-local; canonical repository/file/symbol ordering; not stable across edits or lineage IDs";

#[derive(Serialize)]
struct CoverageDto {
    file_analysis: AnalysisSummaryDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    graph_analysis: Option<GraphSummaryDto>,
    analysis_incomplete: bool,
    diagnostics_count: usize,
    diagnostics_included: bool,
    scan_warning_count: usize,
    scan_warning_details_included: bool,
    paths_may_be_lossy: bool,
}

#[derive(Serialize)]
pub(super) struct QueryContext {
    root: String,
    id_semantics: &'static str,
    coverage: CoverageDto,
}

fn paths_may_be_lossy(index: &RepositoryIndex) -> bool {
    index.root.to_str().is_none()
        || index
            .files
            .iter()
            .any(|file| file.relative_path.to_str().is_none())
        || index
            .git
            .as_ref()
            .is_some_and(|git| git.repository_root.to_str().is_none())
        || index.scan_warnings.iter().any(|warning| {
            warning
                .path
                .as_deref()
                .is_some_and(|path| path.to_str().is_none())
        })
}

pub(super) fn query_context(
    index: &RepositoryIndex,
    graph: Option<&RepositoryGraph>,
) -> QueryContext {
    let file_analysis = analysis_summary(index);
    let graph_analysis = graph.map(graph_summary);
    let paths_may_be_lossy = paths_may_be_lossy(index);
    let relation_analysis_incomplete = graph_analysis.as_ref().is_some_and(|summary| {
        summary
            .relation_statuses
            .iter()
            .any(|(status, count)| *status != "analyzed" && *count > 0)
    });
    let analysis_incomplete = file_analysis.skipped_files > 0
        || file_analysis.failed_files > 0
        || !index.diagnostics.is_empty()
        || !index.scan_warnings.is_empty()
        || paths_may_be_lossy
        || relation_analysis_incomplete;

    QueryContext {
        root: index.root.to_string_lossy().into_owned(),
        id_semantics: ID_SEMANTICS,
        coverage: CoverageDto {
            file_analysis,
            graph_analysis,
            analysis_incomplete,
            diagnostics_count: index.diagnostics.len(),
            diagnostics_included: false,
            scan_warning_count: index.scan_warnings.len(),
            scan_warning_details_included: false,
            paths_may_be_lossy,
        },
    }
}

pub(super) fn human_context(index: &RepositoryIndex, graph: Option<&RepositoryGraph>) -> String {
    let context = query_context(index, graph);
    let coverage = context.coverage;
    let summary = coverage.file_analysis;
    let mut out = format!(
        "Repository: {}\nIDs: {}\nAnalysis incomplete: {}\nFile analysis: discovered={} supported={} analyzable={} analyzed={} skipped={} failed={} syntax_error_files={} syntax_error_count={} symbols={} statuses={:?}\nDiagnostics details included: {} ({}); scan warning details included: {} ({}); paths may be lossy: {}\n",
        escape_controls(&context.root),
        context.id_semantics,
        coverage.analysis_incomplete,
        summary.discovered_files,
        summary.supported_files,
        summary.analyzable_files,
        summary.analyzed_files,
        summary.skipped_files,
        summary.failed_files,
        summary.syntax_error_files,
        summary.syntax_error_count,
        summary.symbols,
        summary.statuses,
        coverage.diagnostics_included,
        coverage.diagnostics_count,
        coverage.scan_warning_details_included,
        coverage.scan_warning_count,
        coverage.paths_may_be_lossy,
    );
    if let Some(summary) = coverage.graph_analysis {
        out.push_str(&format!(
            "Graph: entities={} relations={} resolved={} unresolved={} relation_statuses={:?} symbol_analysis_unavailable={} unsupported_candidates={}\n",
            summary.entities,
            summary.relations,
            summary.resolved,
            summary.unresolved,
            summary.relation_statuses,
            summary.symbol_analysis_unavailable,
            summary.unsupported_candidates,
        ));
    }
    out
}

fn json(command: &str, data: impl Serialize) -> Result<String, serde_json::Error> {
    bounded_json(
        &Envelope {
            schema_version: 1,
            command: command.into(),
            data,
        },
        MAX_JSON_BYTES,
    )
}
#[derive(Serialize)]
struct MatchDto<'a> {
    entity: EntityDto<'a>,
    match_reason: &'static str,
}
#[derive(Serialize)]
struct SearchDto<'a> {
    #[serde(flatten)]
    context: QueryContext,
    query: &'a str,
    kind: &'static str,
    matches: Vec<MatchDto<'a>>,
    truncated: bool,
    non_utf8_file_paths_omitted: usize,
}
pub fn search_json(
    index: &RepositoryIndex,
    query: &str,
    kind: SearchKind,
    result: &SearchResult,
) -> Result<String, serde_json::Error> {
    bounded_json(&search_dto(index, query, kind, result), MAX_JSON_BYTES)
}
pub fn search_dto<'a>(
    index: &'a RepositoryIndex,
    query: &'a str,
    kind: SearchKind,
    result: &'a SearchResult,
) -> impl Serialize + 'a {
    Envelope {
        schema_version: 1,
        command: "search".into(),
        data: SearchDto {
            context: query_context(index, None),
            query,
            kind: kind.name(),
            matches: result
                .matches
                .iter()
                .map(|m| MatchDto {
                    entity: index_entity(index, m.entity),
                    match_reason: MATCH_REASONS[m.tier],
                })
                .collect(),
            truncated: result.truncated,
            non_utf8_file_paths_omitted: if matches!(kind, SearchKind::Symbol) {
                0
            } else {
                index
                    .files
                    .iter()
                    .filter(|file| file.relative_path.to_str().is_none())
                    .count()
            },
        },
    }
}
pub fn search_human(
    index: &RepositoryIndex,
    query: &str,
    kind: SearchKind,
    result: &SearchResult,
) -> String {
    let mut out = human_context(index, None);
    out.push_str(&format!(
        "Search: {}\ntruncated: {}\n",
        escape_controls(query),
        result.truncated
    ));
    let omitted = if matches!(kind, SearchKind::Symbol) {
        0
    } else {
        index
            .files
            .iter()
            .filter(|file| file.relative_path.to_str().is_none())
            .count()
    };
    if omitted > 0 {
        out.push_str(&format!(
            "Non-UTF-8 file paths omitted from textual path search: {omitted}\n"
        ));
    }
    for m in &result.matches {
        let i = m.entity.0 as usize;
        let text = if i <= index.files.len() {
            relative(&index.files[i - 1].relative_path)
        } else {
            let s = &index.symbols[i - 1 - index.files.len()];
            format!(
                "{} ({}:{})",
                s.qualified_name,
                relative(&s.relative_path),
                s.name_range.start.row + 1
            )
        };
        out.push_str(&format!(
            "  {} [{}]\n",
            escape_controls(&text),
            MATCH_REASONS[m.tier]
        ));
    }
    out
}
#[derive(Serialize)]
struct VisitDto<'a> {
    entity: EntityDto<'a>,
    distance: u8,
    predecessor_id: Option<u32>,
    via_relation_id: Option<u32>,
}
#[derive(Serialize)]
struct NeighborhoodRootDto<'a> {
    root_id: u32,
    entities: Vec<VisitDto<'a>>,
    relations: Vec<RelationDto<'a>>,
    candidate_test_associations: Vec<u32>,
    truncated: bool,
}
#[derive(Serialize)]
struct NeighborhoodDto<'a> {
    #[serde(flatten)]
    context: QueryContext,
    selector: &'a str,
    selector_kind: &'static str,
    match_mode: &'static str,
    depth: u8,
    roots: Vec<u32>,
    roots_truncated: bool,
    truncated: bool,
    evidence_note: &'static str,
    results: Vec<NeighborhoodRootDto<'a>>,
}
const TEST_NOTE: &str = "Candidate test associations are reached test-role entities; predecessor/relation chains provide structural evidence, not coverage. Cross-file resolution is unavailable; empty associations are expected.";
fn test_entities(g: &RepositoryGraph, r: &RootResult, owners: &[Option<usize>]) -> Vec<u32> {
    r.entities
        .iter()
        .filter(|id| owners[id.0 as usize].is_some_and(|f| g.index.files[f].role == FileRole::Test))
        .map(|id| id.0)
        .collect()
}
pub fn neighborhood_json(
    g: &RepositoryGraph,
    selector: &str,
    file: bool,
    result: &QueryResult,
) -> Result<String, serde_json::Error> {
    bounded_json(&neighborhood_dto(g, selector, file, result), MAX_JSON_BYTES)
}
pub fn neighborhood_dto<'a>(
    g: &'a RepositoryGraph,
    selector: &'a str,
    file: bool,
    result: &'a QueryResult,
) -> impl Serialize + 'a {
    let owners = file_owners(g);
    Envelope {
        schema_version: 1,
        command: "neighborhood".into(),
        data: NeighborhoodDto {
            context: query_context(&g.index, Some(g)),
            selector,
            selector_kind: if file { "file" } else { "symbol" },
            match_mode: result.match_mode,
            depth: result.depth,
            roots: result.roots.iter().map(|r| r.0).collect(),
            roots_truncated: result.roots_truncated,
            truncated: result.truncated,
            evidence_note: TEST_NOTE,
            results: result
                .results
                .iter()
                .map(|r| NeighborhoodRootDto {
                    root_id: r.root.0,
                    entities: r
                        .entities
                        .iter()
                        .map(|id| VisitDto {
                            entity: entity(g, *id),
                            distance: r.distances[id],
                            predecessor_id: r.predecessors.get(id).map(|p| p.0.0),
                            via_relation_id: r.predecessors.get(id).map(|p| p.1.0),
                        })
                        .collect(),
                    relations: r.relations.iter().map(|id| relation(g, *id)).collect(),
                    candidate_test_associations: test_entities(g, r, &owners),
                    truncated: r.truncated,
                })
                .collect(),
        },
    }
}
pub fn neighborhood_human(g: &RepositoryGraph, selector: &str, result: &QueryResult) -> String {
    let owners = file_owners(g);
    let mut out = human_context(&g.index, Some(g));
    out.push_str(&format!(
        "Neighborhood: {}\nmatch_mode: {} depth: {} roots_truncated: {} truncated: {}\n{}\n",
        escape_controls(selector),
        result.match_mode,
        result.depth,
        result.roots_truncated,
        result.truncated,
        TEST_NOTE
    ));
    for r in &result.results {
        out.push_str(&format!(
            "Root: {} truncated: {}\n",
            escape_controls(&label(g, r.root)),
            r.truncated
        ));
        for id in &r.entities {
            let evidence = r.predecessors.get(id).map_or_else(
                || "root".to_owned(),
                |(predecessor, relation)| {
                    format!(
                        "predecessor #{} via relation #{}",
                        predecessor.0, relation.0
                    )
                },
            );
            out.push_str(&format!(
                "  visit #{} distance {}: {} ({evidence})\n",
                id.0,
                r.distances[id],
                escape_controls(&label(g, *id))
            ));
        }
        for id in &r.relations {
            let rel = &g.relations[id.0 as usize];
            let target = match &rel.target {
                Target::Resolved(id) => label(g, *id),
                Target::Unresolved(name) => format!("unresolved: {name}"),
            };
            out.push_str(&format!(
                "  relation {}: {} {} -> {} [{}]\n",
                id.0,
                escape_controls(&label(g, rel.source)),
                rel.kind.name(),
                escape_controls(&target),
                rel.resolution.name()
            ));
        }
        let associations = test_entities(g, r, &owners);
        if associations.is_empty() {
            out.push_str("  candidate test associations: none\n");
        } else {
            for id in associations {
                let evidence = r.predecessors.get(&EntityId(id)).map_or_else(
                    || "root".to_owned(),
                    |(predecessor, relation)| {
                        format!(
                            "predecessor #{} via relation #{}",
                            predecessor.0, relation.0
                        )
                    },
                );
                out.push_str(&format!(
                    "  candidate test association: #{} {} distance {} ({evidence})\n",
                    id,
                    escape_controls(&label(g, EntityId(id))),
                    r.distances[&EntityId(id)]
                ));
            }
        }
    }
    out
}
#[derive(Serialize)]
struct MetricsDto<'a> {
    entity: EntityDto<'a>,
    incoming_resolved_references: usize,
    outgoing_resolved_references: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    fan_in: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fan_out: Option<usize>,
    unresolved_references: usize,
    unresolved_imports: usize,
    direct_child_symbols: usize,
    symbol_count: usize,
    structural_activity: usize,
}
fn metric<'a>(g: &'a RepositoryGraph, h: &Hotspots, id: EntityId) -> MetricsDto<'a> {
    let m = &h.metrics[id.0 as usize];
    MetricsDto {
        entity: entity(g, id),
        incoming_resolved_references: m.incoming,
        outgoing_resolved_references: m.outgoing,
        fan_in: g.symbol(id).map(|_| m.fan_in),
        fan_out: g.symbol(id).map(|_| m.fan_out),
        unresolved_references: m.unresolved_references,
        unresolved_imports: m.unresolved_imports,
        direct_child_symbols: m.child_symbols,
        symbol_count: m.symbol_count,
        structural_activity: m.structural_activity,
    }
}
const CENTRAL_RANK: &str = "fan_in, incoming_resolved_references, fan_out, outgoing_resolved_references descending; canonical symbol order ascending";
const FILE_RANK: &str = "structural_activity, incoming+outgoing resolved references, unresolved references+imports, symbol_count descending; canonical path ascending";
const METRIC_NOTE: &str = "Current syntactic graph only. Unresolved references do not contribute resolved centrality. Activity counts each non-containment occurrence once per associated file; same-file references contribute to both incoming and outgoing counts.";
#[derive(Serialize)]
struct HotspotDto<'a> {
    #[serde(flatten)]
    context: QueryContext,
    basis: &'static str,
    central_symbol_ranking: &'static str,
    file_ranking: &'static str,
    evidence_note: &'static str,
    central_symbols: Vec<MetricsDto<'a>>,
    files: Vec<MetricsDto<'a>>,
    symbols_truncated: bool,
    files_truncated: bool,
    truncated: bool,
}
pub fn hotspots_json(g: &RepositoryGraph, h: &Hotspots) -> Result<String, serde_json::Error> {
    json(
        "hotspots",
        HotspotDto {
            context: query_context(&g.index, Some(g)),
            basis: "structural/current_graph",
            central_symbol_ranking: CENTRAL_RANK,
            file_ranking: FILE_RANK,
            evidence_note: METRIC_NOTE,
            central_symbols: h.symbols.iter().map(|id| metric(g, h, *id)).collect(),
            files: h.files.iter().map(|id| metric(g, h, *id)).collect(),
            symbols_truncated: h.symbols_truncated,
            files_truncated: h.files_truncated,
            truncated: h.symbols_truncated || h.files_truncated,
        },
    )
}
pub fn hotspots_human(g: &RepositoryGraph, h: &Hotspots) -> String {
    let mut out = human_context(&g.index, Some(g));
    out.push_str(&format!(
        "Structural hotspots (current graph)\n{METRIC_NOTE}\nCentral ranking: {CENTRAL_RANK}\nFile ranking: {FILE_RANK}\nsymbols_truncated: {} files_truncated: {}\n",
        h.symbols_truncated, h.files_truncated
    ));
    for (heading, ids) in [
        ("Central symbols", &h.symbols),
        ("Structural files", &h.files),
    ] {
        out.push_str(&format!("{heading}\n"));
        for id in ids {
            let m = &h.metrics[id.0 as usize];
            let distinct = if g.symbol(*id).is_some() {
                format!("fan_in={} fan_out={} ", m.fan_in, m.fan_out)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "  {}: {distinct}incoming={} outgoing={} unresolved_references={} unresolved_imports={} children={} symbols={} activity={}\n",
                escape_controls(&label(g,*id)), m.incoming, m.outgoing,
                m.unresolved_references, m.unresolved_imports, m.child_symbols,
                m.symbol_count, m.structural_activity
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_dto_uses_shared_exact_and_over_escaped_byte_limit() {
        let dto = SearchDto {
            context: query_context(
                &RepositoryIndex {
                    root: "/fixture".into(),
                    git: None,
                    scan_warnings: vec![],
                    files: vec![],
                    symbols: vec![],
                    diagnostics: vec![],
                },
                None,
            ),
            query: "\u{1b}é",
            kind: "all",
            matches: vec![],
            truncated: false,
            non_utf8_file_paths_omitted: 0,
        };
        let rendered = json("search", &dto).unwrap();
        let envelope = Envelope {
            schema_version: 1,
            command: "search".into(),
            data: dto,
        };
        assert_eq!(bounded_json(&envelope, rendered.len()).unwrap(), rendered);
        assert!(bounded_json(&envelope, rendered.len() - 1).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn search_output_reports_non_utf8_file_path_omissions() {
        use crate::{
            classify::{FileRole, Language},
            model::{AnalysisStatus, FileAnalysis},
        };
        use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

        let index = RepositoryIndex {
            root: "/fixture".into(),
            git: None,
            scan_warnings: vec![],
            files: [b"odd\xff.rs", b"odd\xfe.rs"]
                .into_iter()
                .map(|path| FileAnalysis {
                    relative_path: PathBuf::from(OsString::from_vec(path.to_vec())),
                    language: Language::Rust,
                    role: FileRole::Source,
                    status: AnalysisStatus::Analyzed,
                    symbol_count: 0,
                    syntax_error_count: 0,
                })
                .collect(),
            symbols: vec![],
            diagnostics: vec![],
        };
        let result = crate::query::search(&index, "odd\u{fffd}.rs", SearchKind::File, 20).unwrap();
        let output = search_json(&index, "odd\u{fffd}.rs", SearchKind::File, &result).unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(value["data"]["matches"].as_array().unwrap().is_empty());
        assert_eq!(value["data"]["non_utf8_file_paths_omitted"], 2);
        assert_eq!(value["data"]["coverage"]["paths_may_be_lossy"], true);
        assert_eq!(value["data"]["coverage"]["analysis_incomplete"], true);
    }
}
