//! Phase 4 DTOs; serialization is bounded before any stdout is written.
use super::{
    Envelope, MAX_JSON_BYTES, bounded_json, escape_controls,
    graph::{EntityDto, RelationDto, entity, label, relation},
    index::{RangeDto, SymbolDto},
    relative,
};
use crate::{
    change::*,
    classify::{FileRole, classify_language, classify_role},
    git::revision::Entry,
    graph::{Entity, EntityId, QueryResult, RepositoryGraph},
    model::AnalysisStatus,
    relations::Metadata,
};
use serde::Serialize;
use std::collections::BTreeMap;

fn budget_error() -> serde_json::Error {
    serde_json::Error::io(std::io::Error::other("change output safety limit exceeded"))
}

// A conservative upper bound is checked before building owned DTO vectors or
// the human String. Account for worst-case JSON escaping and per-record syntax,
// including repeated entities/relations in each root group.
pub(crate) fn check_materialization(
    set: &ChangeSet,
    limit: usize,
) -> Result<usize, serde_json::Error> {
    let mut estimate = 4096usize;
    let mut add = |n: usize| -> Result<(), serde_json::Error> {
        estimate = estimate
            .checked_add(n)
            .filter(|n| *n <= limit)
            .ok_or_else(budget_error)?;
        Ok(())
    };
    add(set.range.len().saturating_mul(6))?;
    add(set.revisions.root.as_os_str().len().saturating_mul(6))?;
    add(set.revisions.scope.as_os_str().len().saturating_mul(6))?;
    for f in &set.files {
        add(2048)?;
        for e in [&f.git.before, &f.git.after].into_iter().flatten() {
            add(e.path.as_os_str().len().saturating_mul(6) + e.mode.len() + e.oid.len())?;
        }
        for s in &f.symbols {
            add(2048)?;
            for (snap, index) in [(&set.base, s.before), (&set.head, s.after)] {
                if let Some(i) = index {
                    let v = &snap.graph.index.symbols[i];
                    add(v.qualified_name.len().saturating_mul(6)
                        + v.relative_path.as_os_str().len().saturating_mul(6))?;
                }
            }
        }
        for r in &f.relations {
            let metadata = match &r.signature.metadata {
                Metadata::Import { binding, .. } => binding.as_deref().map_or(0, str::len),
                _ => 0,
            };
            add(4096
                + r.signature.source.name.len().saturating_mul(6)
                + r.signature.target.name.len().saturating_mul(6)
                + metadata.saturating_mul(6))?;
        }
    }
    for (graph, query) in set
        .impact
        .as_ref()
        .map(|i| [(&set.base.graph, &i.base), (&set.head.graph, &i.head)])
        .into_iter()
        .flatten()
    {
        for root in &query.results {
            add(4096 + entity_text_bytes(graph, root.root).saturating_mul(6))?;
            for &id in &root.entities {
                add(4096 + entity_text_bytes(graph, id).saturating_mul(12))?;
            }
            for &id in &root.relations {
                let relation = &graph.relations[id.0 as usize];
                let metadata = match &relation.metadata {
                    Metadata::Import { binding, .. } => binding.as_deref().map_or(0, str::len),
                    _ => 0,
                };
                let endpoints = entity_text_bytes(graph, relation.source).saturating_add(
                    match &relation.target {
                        crate::graph::Target::Resolved(target) => entity_text_bytes(graph, *target),
                        crate::graph::Target::Unresolved(text) => text.len(),
                    },
                );
                add(4096
                    + relation
                        .evidence
                        .relative_path
                        .as_os_str()
                        .len()
                        .saturating_mul(6)
                    + endpoints.saturating_mul(6)
                    + metadata.saturating_mul(6))?;
            }
        }
    }
    Ok(estimate)
}
fn entity_text_bytes(g: &RepositoryGraph, id: EntityId) -> usize {
    match g.entities[id.0 as usize] {
        Entity::File(i) => g.index.files[i].relative_path.as_os_str().len(),
        Entity::Symbol(i) => {
            let s = &g.index.symbols[i];
            s.relative_path
                .as_os_str()
                .len()
                .saturating_add(s.qualified_name.len())
        }
        Entity::Repository => 16,
    }
}

#[derive(Serialize)]
struct Coverage {
    files: usize,
    statuses: BTreeMap<&'static str, usize>,
    syntax_errors: usize,
    relation_statuses: BTreeMap<&'static str, usize>,
    unsupported_relation_candidates: usize,
    non_utf8_paths: usize,
    diagnostics_omitted: usize,
    scan_warnings_omitted: usize,
    analysis_incomplete: bool,
}
fn coverage(s: &Snapshot) -> Coverage {
    let mut statuses = BTreeMap::new();
    let mut relation_statuses = BTreeMap::new();
    for f in &s.graph.index.files {
        *statuses.entry(f.status.name()).or_default() += 1;
    }
    for f in &s.graph.relation_files {
        *relation_statuses.entry(f.status.name()).or_default() += 1;
    }
    let syntax_errors = s
        .graph
        .index
        .files
        .iter()
        .map(|f| f.syntax_error_count)
        .sum();
    let unsupported_relation_candidates = s
        .graph
        .relation_files
        .iter()
        .map(|f| f.unsupported_candidate_count)
        .sum();
    Coverage {
        files: s.graph.index.files.len(),
        statuses,
        relation_statuses,
        syntax_errors,
        unsupported_relation_candidates,
        non_utf8_paths: s
            .graph
            .index
            .files
            .iter()
            .filter(|f| f.relative_path.to_str().is_none())
            .count(),
        diagnostics_omitted: s.graph.index.diagnostics.len(),
        scan_warnings_omitted: s.graph.index.scan_warnings.len(),
        analysis_incomplete: s
            .graph
            .index
            .files
            .iter()
            .any(|f| f.status != AnalysisStatus::Analyzed)
            || syntax_errors > 0
            || unsupported_relation_candidates > 0
            || s.graph
                .relation_files
                .iter()
                .any(|f| f.status != crate::relations::RelationStatus::Analyzed),
    }
}
#[derive(Serialize)]
struct SideDto {
    path: String,
    path_is_lossy: bool,
    mode: String,
    object_id: String,
    role: &'static str,
    analysis_status: &'static str,
    syntax_error_count: usize,
    relation_status: Option<&'static str>,
    unsupported_relation_candidates: usize,
}
fn side(s: &Snapshot, e: &Entry) -> SideDto {
    let analysis = file_analysis(s, Some(e));
    let relation = relation_analysis(s, e);
    SideDto {
        path: relative(&e.path),
        path_is_lossy: e.path.to_str().is_none(),
        mode: e.mode.clone(),
        object_id: e.oid.clone(),
        role: classify_role(&e.path, &classify_language(&e.path)).name(),
        analysis_status: if !e.regular() {
            "non_regular_mode"
        } else if e.path.to_str().is_none() {
            "non_utf8_path"
        } else {
            analysis.map_or("unavailable", |f| f.status.name())
        },
        syntax_error_count: analysis.map_or(0, |f| f.syntax_error_count),
        relation_status: relation.map(|f| f.status.name()),
        unsupported_relation_candidates: relation.map_or(0, |f| f.unsupported_candidate_count),
    }
}
#[derive(Serialize)]
struct SymbolChangeDto<'a> {
    kind: &'static str,
    before: Option<SymbolDto<'a>>,
    after: Option<SymbolDto<'a>>,
}
#[derive(Serialize)]
struct EndpointDto<'a> {
    state: &'static str,
    kind: &'static str,
    name: &'a str,
}
impl<'a> From<&'a Endpoint> for EndpointDto<'a> {
    fn from(e: &'a Endpoint) -> Self {
        Self {
            state: e.state,
            kind: e.kind,
            name: &e.name,
        }
    }
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
        context: &'static str,
    },
}
#[derive(Serialize)]
struct OccurrencesDto {
    count: usize,
    examples: Vec<RangeDto>,
    examples_omitted: usize,
}
fn occurrences(o: &Occurrences) -> OccurrencesDto {
    OccurrencesDto {
        count: o.count,
        examples: o.examples.iter().copied().map(Into::into).collect(),
        examples_omitted: o.count.saturating_sub(o.examples.len()),
    }
}
#[derive(Serialize)]
struct RelationDeltaDto<'a> {
    source: EndpointDto<'a>,
    target: EndpointDto<'a>,
    kind: &'static str,
    resolution: &'static str,
    metadata: MetadataDto<'a>,
    derivation: &'static str,
    before: OccurrencesDto,
    after: OccurrencesDto,
}
#[derive(Serialize)]
struct FileDto<'a> {
    kind: &'a str,
    rename_similarity: Option<u8>,
    before: Option<SideDto>,
    after: Option<SideDto>,
    comparison_available: bool,
    relation_comparison_available: bool,
    ambiguous_symbol_keys: usize,
    symbols: Vec<SymbolChangeDto<'a>>,
    relations: Vec<RelationDeltaDto<'a>>,
}
#[derive(Serialize)]
struct VisitDto<'a> {
    entity: EntityDto<'a>,
    distance: u8,
    predecessor: Option<PredecessorDto>,
    candidate_test: bool,
}
#[derive(Serialize)]
struct PredecessorDto {
    entity_id: u32,
    relation_id: u32,
}
#[derive(Serialize)]
struct RootDto<'a> {
    seed: EntityDto<'a>,
    seed_file_change: usize,
    seed_symbol_change: Option<usize>,
    seed_kind: &'static str,
    traversal_policy: &'static str,
    visits: Vec<VisitDto<'a>>,
    relations: Vec<RelationDto<'a>>,
    truncated: bool,
}
#[derive(Serialize)]
struct ImpactSideDto<'a> {
    revision_side: &'static str,
    seeds_requested: usize,
    seeds_omitted: usize,
    depth: u8,
    seed_kind: &'static str,
    traversal_policy: &'static str,
    roots_truncated: bool,
    truncated: bool,
    results: Vec<RootDto<'a>>,
}
fn candidate_test(g: &RepositoryGraph, id: EntityId) -> bool {
    let path = match g.entities[id.0 as usize] {
        Entity::File(i) => Some(&g.index.files[i].relative_path),
        Entity::Symbol(i) => Some(&g.index.symbols[i].relative_path),
        _ => None,
    };
    path.is_some_and(|p| {
        g.index
            .files
            .binary_search_by(|f| f.relative_path.cmp(p))
            .ok()
            .is_some_and(|i| g.index.files[i].role == FileRole::Test)
    })
}
fn seed_change(
    set: &ChangeSet,
    g: &RepositoryGraph,
    id: EntityId,
    before: bool,
) -> (usize, Option<usize>) {
    for (i, f) in set.files.iter().enumerate() {
        match g.entities[id.0 as usize] {
            Entity::File(j) => {
                if (if before { &f.git.before } else { &f.git.after })
                    .as_ref()
                    .is_some_and(|e| e.path == g.index.files[j].relative_path)
                {
                    return (i, None);
                }
            }
            Entity::Symbol(j) => {
                for (k, s) in f.symbols.iter().enumerate() {
                    if (if before { s.before } else { s.after }) == Some(j) {
                        return (i, Some(k));
                    }
                }
            }
            _ => {}
        }
    }
    unreachable!("roots are selected only from changed files/symbols")
}
fn impact_side<'a>(
    set: &'a ChangeSet,
    g: &'a RepositoryGraph,
    q: &QueryResult,
    before: bool,
) -> ImpactSideDto<'a> {
    let count = set.impact.as_ref().map_or(0, |i| {
        if before {
            i.base_seed_count
        } else {
            i.head_seed_count
        }
    });
    ImpactSideDto {
        seeds_requested: count,
        seeds_omitted: count - q.roots.len(),
        revision_side: if before { "base" } else { "head" },
        seed_kind: if before {
            set.impact.as_ref().unwrap().base_seed_kind
        } else {
            set.impact.as_ref().unwrap().head_seed_kind
        },
        traversal_policy: if before {
            set.impact.as_ref().unwrap().base_policy
        } else {
            set.impact.as_ref().unwrap().head_policy
        },
        depth: q.depth,
        roots_truncated: q.roots_truncated,
        truncated: q.truncated,
        results: q
            .results
            .iter()
            .map(|r| {
                let (seed_file_change, seed_symbol_change) = seed_change(set, g, r.root, before);
                RootDto {
                    seed: entity(g, r.root),
                    seed_file_change,
                    seed_symbol_change,
                    seed_kind: if before {
                        set.impact.as_ref().unwrap().base_seed_kind
                    } else {
                        set.impact.as_ref().unwrap().head_seed_kind
                    },
                    traversal_policy: if before {
                        set.impact.as_ref().unwrap().base_policy
                    } else {
                        set.impact.as_ref().unwrap().head_policy
                    },
                    visits: r
                        .entities
                        .iter()
                        .map(|id| VisitDto {
                            entity: entity(g, *id),
                            distance: r.distances[id],
                            predecessor: r.predecessors.get(id).map(|(e, r)| PredecessorDto {
                                entity_id: e.0,
                                relation_id: r.0,
                            }),
                            candidate_test: candidate_test(g, *id),
                        })
                        .collect(),
                    relations: r.relations.iter().map(|id| relation(g, *id)).collect(),
                    truncated: r.truncated,
                }
            })
            .collect(),
    }
}
#[derive(Serialize)]
struct DirectCandidate {
    file_change: usize,
    revision_side: &'static str,
    path: String,
    role: &'static str,
}
#[derive(Serialize)]
struct ReportDto<'a> {
    repository_root: String,
    scope_root: String,
    root_paths_lossy: bool,
    input_range: &'a str,
    base_commit: &'a str,
    head_commit: &'a str,
    semantics: &'static str,
    rename_detection: &'static str,
    id_semantics: &'static str,
    analysis_scope: &'static str,
    total_file_changes: usize,
    ambiguous_symbol_keys: usize,
    unavailable_symbol_comparisons: usize,
    unavailable_relation_comparisons: usize,
    base_coverage: Coverage,
    head_coverage: Coverage,
    files: Vec<FileDto<'a>>,
    impact: Option<Vec<ImpactSideDto<'a>>>,
    directly_changed_test_doc_candidates: Vec<DirectCandidate>,
}
fn report(set: &ChangeSet) -> ReportDto<'_> {
    let mut candidates = vec![];
    for (i, f) in set.files.iter().enumerate() {
        for (side, e) in [("base", &f.git.before), ("head", &f.git.after)] {
            if let Some(e) = e {
                let role = classify_role(&e.path, &classify_language(&e.path));
                if matches!(role, FileRole::Test | FileRole::Documentation) {
                    candidates.push(DirectCandidate {
                        file_change: i,
                        revision_side: side,
                        path: relative(&e.path),
                        role: role.name(),
                    });
                }
            }
        }
    }
    ReportDto {
        repository_root: set.revisions.root.to_string_lossy().into(),
        scope_root: set.revisions.scope.to_string_lossy().into(),
        root_paths_lossy: set.revisions.root.to_str().is_none()
            || set.revisions.scope.to_str().is_none(),
        input_range: &set.range,
        base_commit: &set.revisions.base,
        head_commit: &set.revisions.head,
        semantics: "explicit two-dot committed snapshots; syntactic evidence, not runtime prediction",
        rename_detection: "Git --find-renames=50%; renameLimit=10000",
        id_semantics: "IDs are local to the indicated revision graph; never cross-revision lineage",
        analysis_scope: if set.impact.is_some() {
            "full scoped revision inventories"
        } else {
            "changed file sides only"
        },
        total_file_changes: set.files.len(),
        ambiguous_symbol_keys: set.files.iter().map(|f| f.ambiguous_keys).sum(),
        unavailable_symbol_comparisons: set
            .files
            .iter()
            .filter(|f| !f.comparison_available)
            .count(),
        unavailable_relation_comparisons: set
            .files
            .iter()
            .filter(|f| !f.relation_comparison_available)
            .count(),
        base_coverage: coverage(&set.base),
        head_coverage: coverage(&set.head),
        files: set
            .files
            .iter()
            .map(|f| FileDto {
                kind: &f.git.status,
                rename_similarity: f.git.similarity,
                before: f.git.before.as_ref().map(|e| side(&set.base, e)),
                after: f.git.after.as_ref().map(|e| side(&set.head, e)),
                comparison_available: f.comparison_available,
                relation_comparison_available: f.relation_comparison_available,
                ambiguous_symbol_keys: f.ambiguous_keys,
                symbols: f
                    .symbols
                    .iter()
                    .map(|s| SymbolChangeDto {
                        kind: s.kind,
                        before: s
                            .before
                            .map(|i| SymbolDto::from(&set.base.graph.index.symbols[i])),
                        after: s
                            .after
                            .map(|i| SymbolDto::from(&set.head.graph.index.symbols[i])),
                    })
                    .collect(),
                relations: f
                    .relations
                    .iter()
                    .map(|r| RelationDeltaDto {
                        source: (&r.signature.source).into(),
                        target: (&r.signature.target).into(),
                        kind: r.signature.kind.name(),
                        resolution: r.signature.resolution.name(),
                        metadata: match &r.signature.metadata {
                            Metadata::None => MetadataDto::None,
                            Metadata::Import { binding, wildcard } => MetadataDto::Import {
                                binding: binding.as_deref(),
                                wildcard: *wildcard,
                            },
                            Metadata::Reference { .. } => {
                                MetadataDto::Reference { context: "call" }
                            }
                        },
                        derivation: r.signature.derivation.name(),
                        before: occurrences(&r.before),
                        after: occurrences(&r.after),
                    })
                    .collect(),
            })
            .collect(),
        impact: set.impact.as_ref().map(|i| {
            vec![
                impact_side(set, &set.base.graph, &i.base, true),
                impact_side(set, &set.head.graph, &i.head, false),
            ]
        }),
        directly_changed_test_doc_candidates: candidates,
    }
}
pub fn json(command: &str, set: &ChangeSet) -> Result<String, serde_json::Error> {
    let _estimate = check_materialization(set, MAX_JSON_BYTES)?;
    bounded_json(
        &Envelope {
            schema_version: 1,
            command: command.into(),
            data: report(set),
        },
        MAX_JSON_BYTES,
    )
}
pub fn human(command: &str, set: &ChangeSet) -> Result<String, serde_json::Error> {
    use std::fmt::Write;
    let estimate = check_materialization(set, MAX_JSON_BYTES)?;
    let d = report(set);
    let mut out = String::with_capacity(estimate);
    let _ = write!(
        out,
        "{command}: {}\nRepository: {}\nScope: {}\nBase: {}\nHead: {}\n{}\n{}\n{}\nAnalysis scope: {}\nFile changes: {}; ambiguous symbol keys: {}; unavailable symbol/relation comparisons: {}/{}\n",
        escape_controls(d.input_range),
        escape_controls(&d.repository_root),
        escape_controls(&d.scope_root),
        d.base_commit,
        d.head_commit,
        d.semantics,
        d.rename_detection,
        d.id_semantics,
        d.analysis_scope,
        d.total_file_changes,
        d.ambiguous_symbol_keys,
        d.unavailable_symbol_comparisons,
        d.unavailable_relation_comparisons
    );
    for (side, c) in [("base", &d.base_coverage), ("head", &d.head_coverage)] {
        let _ = writeln!(
            out,
            "{side} coverage: {:?}; relation statuses {:?}; syntax errors {}; unsupported relations {}; non-UTF-8 paths {}; diagnostics/warnings omitted {}/{}; analysis_incomplete: {}",
            c.statuses,
            c.relation_statuses,
            c.syntax_errors,
            c.unsupported_relation_candidates,
            c.non_utf8_paths,
            c.diagnostics_omitted,
            c.scan_warnings_omitted,
            c.analysis_incomplete
        );
    }
    for f in &set.files {
        let p = |e: &Option<Entry>| {
            e.as_ref()
                .map(|e| escape_controls(&relative(&e.path)))
                .unwrap_or_else(|| "(absent)".into())
        };
        let _ = writeln!(
            out,
            "\n{}: {} -> {} (rename similarity {:?})",
            f.git.status,
            p(&f.git.before),
            p(&f.git.after),
            f.git.similarity
        );
        for (name, s, e) in [
            ("base", &set.base, &f.git.before),
            ("head", &set.head, &f.git.after),
        ] {
            if let Some(e) = e {
                let a = side(s, e);
                let _ = writeln!(
                    out,
                    "  {name}: mode {} object {} status {} syntax_errors {} relation_status {:?}",
                    e.mode, e.oid, a.analysis_status, a.syntax_error_count, a.relation_status
                );
            }
        }
        let _ = writeln!(
            out,
            "  comparison_available: {}; relation_comparison_available: {}; ambiguous_keys: {}",
            f.comparison_available, f.relation_comparison_available, f.ambiguous_keys
        );
        for s in &f.symbols {
            let symbol = s
                .after
                .map(|i| &set.head.graph.index.symbols[i])
                .or_else(|| s.before.map(|i| &set.base.graph.index.symbols[i]))
                .unwrap();
            let _ = writeln!(
                out,
                "  {} {} {}",
                s.kind,
                symbol.kind.name(),
                escape_controls(&symbol.qualified_name)
            );
        }
        for change in &f.symbols {
            for (name, snapshot, index) in [
                ("base", &set.base, change.before),
                ("head", &set.head, change.after),
            ] {
                if let Some(index) = index {
                    let symbol = &snapshot.graph.index.symbols[index];
                    let _ = writeln!(
                        out,
                        "    {name}: {} bytes {}..{}",
                        escape_controls(&relative(&symbol.relative_path)),
                        symbol.range.start.byte,
                        symbol.range.end.byte
                    );
                }
            }
        }
        for r in &f.relations {
            let _ = writeln!(
                out,
                "  {} {} -> {} [{}; {}]: {} -> {} occurrences",
                r.signature.kind.name(),
                escape_controls(&r.signature.source.name),
                escape_controls(&r.signature.target.name),
                r.signature.resolution.name(),
                r.signature.derivation.name(),
                r.before.count,
                r.after.count
            );
            let _ = writeln!(
                out,
                "    metadata: {}; base examples {:?}; head examples {:?} (up to 3 each)",
                escape_controls(&format!("{:?}", r.signature.metadata)),
                r.before.examples,
                r.after.examples
            );
        }
    }
    if let Some(i) = &set.impact {
        for (side, g, q) in [
            ("base", &set.base.graph, &i.base),
            ("head", &set.head.graph, &i.head),
        ] {
            let _ = writeln!(
                out,
                "\n{side} impact depth {} roots_truncated: {} truncated: {}",
                q.depth, q.roots_truncated, q.truncated
            );
            let count = if side == "base" {
                i.base_seed_count
            } else {
                i.head_seed_count
            };
            let seed_kind = if side == "base" {
                i.base_seed_kind
            } else {
                i.head_seed_kind
            };
            let policy = if side == "base" {
                i.base_policy
            } else {
                i.head_policy
            };
            let _ = writeln!(
                out,
                "  Seed kind: {seed_kind}; traversal policy: {policy}; requested: {}; omitted: {}",
                count,
                count - q.roots.len()
            );
            for r in &q.results {
                let _ = writeln!(
                    out,
                    "  Seed #{} {} truncated: {}",
                    r.root.0,
                    escape_controls(&label(g, r.root)),
                    r.truncated
                );
                for id in &r.entities {
                    let _ = writeln!(
                        out,
                        "    #{} {} distance {} predecessor {:?} candidate_test: {}",
                        id.0,
                        escape_controls(&label(g, *id)),
                        r.distances[id],
                        r.predecessors.get(id).map(|(e, r)| (e.0, r.0)),
                        candidate_test(g, *id)
                    );
                }
                for id in &r.relations {
                    let rel = &g.relations[id.0 as usize];
                    let target = match &rel.target {
                        crate::graph::Target::Resolved(id) => {
                            format!("#{} {}", id.0, label(g, *id))
                        }
                        crate::graph::Target::Unresolved(text) => format!("unresolved {text}"),
                    };
                    let _ = writeln!(
                        out,
                        "    relation #{} {} source #{} target {} resolution {} derivation {} path {} range {:?}",
                        id.0,
                        rel.kind.name(),
                        rel.source.0,
                        escape_controls(&target),
                        rel.resolution.name(),
                        rel.evidence.derivation.name(),
                        escape_controls(&relative(&rel.evidence.relative_path)),
                        rel.evidence.range
                    );
                }
            }
        }
    }
    for c in d.directly_changed_test_doc_candidates {
        let _ = writeln!(
            out,
            "Directly changed {} candidate ({}): {}",
            c.role,
            c.revision_side,
            escape_controls(&c.path)
        );
    }
    out.truncate(out.trim_end().len());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn change_dto_escaped_bytes_use_shared_exact_output_limit() {
        let dto = EndpointDto {
            state: "unresolved",
            kind: "text",
            name: "a\n\u{0001}",
        };
        let bytes = serde_json::to_string_pretty(&dto).unwrap();
        assert_eq!(bounded_json(&dto, bytes.len()).unwrap(), bytes);
        assert!(bounded_json(&dto, bytes.len() - 1).is_err());
    }
}
