//! Command-specific schema-v1 adapters over query results.
use super::{
    Envelope, MAX_JSON_BYTES, bounded_json, escape_controls,
    graph::{EntityDto, RelationDto, entity, index_entity, label, relation},
    relative,
};
use crate::{classify::FileRole, graph::*, model::RepositoryIndex, query::*};
use serde::Serialize;

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
    query: &'a str,
    kind: &'static str,
    id_semantics: &'static str,
    matches: Vec<MatchDto<'a>>,
    truncated: bool,
}
pub fn search_json(
    index: &RepositoryIndex,
    query: &str,
    kind: SearchKind,
    result: &SearchResult,
) -> Result<String, serde_json::Error> {
    json(
        "search",
        SearchDto {
            query,
            kind: kind.name(),
            id_semantics: "index-local; canonical repository/file/symbol ordering; not stable across edits",
            matches: result
                .matches
                .iter()
                .map(|m| MatchDto {
                    entity: index_entity(index, m.entity),
                    match_reason: MATCH_REASONS[m.tier],
                })
                .collect(),
            truncated: result.truncated,
        },
    )
}
pub fn search_human(index: &RepositoryIndex, query: &str, result: &SearchResult) -> String {
    let mut out = format!(
        "Search: {}\ntruncated: {}\n",
        escape_controls(query),
        result.truncated
    );
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
    let owners = file_owners(g);
    json(
        "neighborhood",
        NeighborhoodDto {
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
    )
}
pub fn neighborhood_human(g: &RepositoryGraph, selector: &str, result: &QueryResult) -> String {
    let owners = file_owners(g);
    let mut out = format!(
        "Neighborhood: {}\nmatch_mode: {} depth: {} roots_truncated: {} truncated: {}\n{}\n",
        escape_controls(selector),
        result.match_mode,
        result.depth,
        result.roots_truncated,
        result.truncated,
        TEST_NOTE
    );
    for r in &result.results {
        out.push_str(&format!(
            "Root: {} truncated: {}\n",
            escape_controls(&label(g, r.root)),
            r.truncated
        ));
        for id in &r.entities {
            out.push_str(&format!(
                "  distance {}: {}\n",
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
        out.push_str(&format!(
            "  candidate test associations: {:?}\n",
            test_entities(g, r, &owners)
        ));
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
    let mut out = format!(
        "Structural hotspots (current graph)\n{METRIC_NOTE}\nCentral ranking: {CENTRAL_RANK}\nFile ranking: {FILE_RANK}\nsymbols_truncated: {} files_truncated: {}\n",
        h.symbols_truncated, h.files_truncated
    );
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
            query: "\u{1b}é",
            kind: "all",
            id_semantics: "index-local",
            matches: vec![],
            truncated: false,
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
}
