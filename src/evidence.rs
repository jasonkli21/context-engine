//! Evidence v1 projection. No source loading, policy, transport types or JSON reparsing.
use crate::{
    application::{self, Result},
    change, context,
    graph::{self, Entity, Target},
    model, relations,
};
use schemars::JsonSchema;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
#[derive(Serialize, JsonSchema)]
pub struct Document<'a> {
    evidence_schema_version: u32,
    producer: Producer,
    subject: Subject,
    observation: Observation,
    coverage: Coverage,
    #[serde(flatten)]
    payload: Feed<'a>,
}
#[derive(Serialize, JsonSchema)]
struct Producer {
    name: &'static str,
    version: &'static str,
    analysis_compat_version: i64,
    identity_version: &'static str,
    parser_version: Option<String>,
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Subject {
    Repository { repository_id: String },
    Workspace { workspace_id: String },
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Basis {
    BestEffortWorktree,
    CommittedChangedPairs,
    MixedTimeWorkspace,
}
#[derive(Serialize, JsonSchema)]
struct Observation {
    basis: Basis,
    scope: String,
    atomic: bool,
    git_head: Option<String>,
    git_dirty: Option<bool>,
    base: Option<String>,
    head: Option<String>,
}
#[derive(Default, Serialize, JsonSchema)]
struct Coverage {
    /// Always false: scanner exclusions, syntax heuristics and races preclude completeness.
    complete: bool,
    counts: BTreeMap<String, usize>,
    limitations: Vec<&'static str>,
}
impl Coverage {
    fn add(&mut self, name: &str, n: usize) {
        *self.counts.entry(name.into()).or_default() += n;
    }
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "feed", content = "data", rename_all = "snake_case")]
enum Feed<'a> {
    Structure(Structure),
    Changes(Changes),
    Candidates(Candidates),
    Workspace(crate::output_workspace::Document<'a>),
}
#[derive(Serialize, JsonSchema)]
struct Structure {
    repository_ref: String,
    files: Vec<File>,
    symbols: Vec<Symbol>,
    relations: Vec<Relation>,
}
#[derive(Serialize, JsonSchema)]
struct File {
    id: String,
    path: String,
    language: String,
    role: String,
    analysis_status: &'static str,
    syntax_errors: usize,
    relation_status: Option<&'static str>,
    unsupported_relation_candidates: usize,
}
#[derive(Serialize, JsonSchema)]
struct Point {
    byte: usize,
    row: usize,
    column: usize,
}
#[derive(Serialize, JsonSchema)]
struct Range {
    start: Point,
    end: Point,
}
impl From<model::SourceRange> for Range {
    fn from(r: model::SourceRange) -> Self {
        let point = |p: model::SourcePoint| Point {
            byte: p.byte,
            row: p.row,
            column: p.column,
        };
        Self {
            start: point(r.start),
            end: point(r.end),
        }
    }
}
#[derive(Serialize, JsonSchema)]
struct Symbol {
    id: String,
    file_ref: String,
    path: String,
    language: String,
    kind: &'static str,
    name: String,
    qualified_name: String,
    range: Range,
    name_range: Range,
    derivation: &'static str,
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Metadata {
    None,
    Import {
        binding: Option<String>,
        wildcard: bool,
    },
    Reference {
        context: &'static str,
    },
}
fn metadata(m: &relations::Metadata) -> Metadata {
    match m {
        relations::Metadata::None => Metadata::None,
        relations::Metadata::Import { binding, wildcard } => Metadata::Import {
            binding: binding.clone(),
            wildcard: *wildcard,
        },
        relations::Metadata::Reference { .. } => Metadata::Reference { context: "call" },
    }
}
#[derive(Serialize, JsonSchema)]
struct Relation {
    id: String,
    source_ref: String,
    target_ref: Option<String>,
    unresolved_name: Option<String>,
    kind: &'static str,
    resolution: &'static str,
    metadata: Metadata,
    path: String,
    range: Option<Range>,
    language: Option<String>,
    derivation: &'static str,
}
/// Domain separation + u64 little-endian byte lengths; no native root or numeric graph ID.
fn identity(domain: &str, fields: &[&str]) -> String {
    let mut h = blake3::Hasher::new();
    for f in std::iter::once("context-engine-evidence-id-v1")
        .chain(std::iter::once(domain))
        .chain(fields.iter().copied())
    {
        h.update(&(f.len() as u64).to_le_bytes());
        h.update(f.as_bytes());
    }
    h.finalize().to_hex().to_string()
}
fn nonce() -> String {
    // Invocation namespace only, never a fingerprint or assertion about source bytes.
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    identity(
        "observation",
        &[
            &format!("{:?}", std::time::SystemTime::now()),
            &std::process::id().to_string(),
            &SEQUENCE
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                .to_string(),
        ],
    )
}
pub fn safe_path(p: &Path) -> Option<String> {
    // Validate native input before component formatting (Path::components normalizes dots).
    let raw = p.to_str()?;
    if raw.contains(['\\', ':', '\u{fffd}']) || raw.chars().any(char::is_control) {
        return None;
    }
    if raw
        .split('/')
        .any(|c| c.is_empty() || c == "." || c == ".." || c.len() > 255)
        || raw.len() > 4096
    {
        return None;
    }
    crate::application::validate_file(p).ok()?;
    crate::paths::utf8_relative(p)
}
fn logical_id(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 64
        || !s.as_bytes()[0].is_ascii_lowercase()
        || !s
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
    {
        return Err("repository ID must match [a-z][a-z0-9_-]{0,63}".into());
    }
    Ok(())
}
fn worktree(g: &graph::RepositoryGraph) -> Observation {
    Observation {
        basis: Basis::BestEffortWorktree,
        scope: nonce(),
        atomic: false,
        git_head: g.index.git.as_ref().and_then(|m| m.head_commit.clone()),
        git_dirty: g.index.git.as_ref().and_then(|m| m.is_dirty),
        base: None,
        head: None,
    }
}
fn document<'a>(
    id: &str,
    observation: Observation,
    coverage: Coverage,
    payload: Feed<'a>,
) -> Document<'a> {
    Document {
        evidence_schema_version: 1,
        producer: Producer {
            name: "context-engine",
            version: env!("CARGO_PKG_VERSION"),
            analysis_compat_version: crate::persistence::INDEX_COMPAT_VERSION,
            identity_version: "evidence-id-v1",
            parser_version: None,
        },
        subject: Subject::Repository {
            repository_id: id.into(),
        },
        observation,
        coverage,
        payload,
    }
}
fn coverage(g: &graph::RepositoryGraph) -> Coverage {
    let mut c = Coverage {
        limitations: vec![
            "scanner_exclusions_apply",
            "syntax_not_compiler_binding",
            "raw_diagnostics_withheld",
        ],
        ..Coverage::default()
    };
    c.add("files_observed", g.index.files.len());
    c.add("diagnostics_withheld", g.index.diagnostics.len());
    c.add("scan_warnings_withheld", g.index.scan_warnings.len());
    for f in &g.index.files {
        c.add(&format!("analysis_{}", f.status.name()), 1);
        c.add("syntax_errors", f.syntax_error_count);
    }
    for f in &g.relation_files {
        c.add(&format!("relations_{}", f.status.name()), 1);
        c.add(
            "unsupported_relation_candidates",
            f.unsupported_candidate_count,
        );
        c.add("unresolved_names", f.unresolved_count);
    }
    c
}
fn project(
    g: &graph::RepositoryGraph,
    repo: &str,
    scope: &str,
    c: &mut Coverage,
    include_relations: bool,
) -> Result<(Structure, Vec<Option<String>>)> {
    let repository_ref = identity("repository", &[repo, scope]);
    let mut files = vec![];
    let mut symbols = vec![];
    let mut refs = Vec::with_capacity(g.entities.len());
    let mut ids = BTreeSet::new();
    ids.insert(repository_ref.clone());
    let relation_files: BTreeMap<_, _> = g
        .relation_files
        .iter()
        .map(|f| (&f.relative_path, f))
        .collect();
    let mut file_refs = BTreeMap::new();
    for e in &g.entities {
        let id = match e {
            Entity::Repository => Some(repository_ref.clone()),
            Entity::File(i) => {
                let f = &g.index.files[*i];
                if let Some(path) = safe_path(&f.relative_path) {
                    let id = identity("file", &[repo, scope, &path]);
                    let r = relation_files.get(&f.relative_path);
                    files.push(File {
                        id: id.clone(),
                        path,
                        language: f.language.name().into(),
                        role: f.role.name().into(),
                        analysis_status: f.status.name(),
                        syntax_errors: f.syntax_error_count,
                        relation_status: r.map(|r| r.status.name()),
                        unsupported_relation_candidates: r
                            .map_or(0, |r| r.unsupported_candidate_count),
                    });
                    file_refs.insert(&f.relative_path, id.clone());
                    Some(id)
                } else {
                    c.add("unrepresentable_files_omitted", 1);
                    None
                }
            }
            Entity::Symbol(i) => {
                let s = &g.index.symbols[*i];
                if let Some(file_ref) = file_refs.get(&s.relative_path) {
                    let path = safe_path(&s.relative_path).ok_or("invalid symbol path")?;
                    let range = serde_json::to_string(&Range::from(s.range))?;
                    let id = identity(
                        "symbol",
                        &[
                            repo,
                            scope,
                            &path,
                            s.kind.name(),
                            &s.name,
                            &s.qualified_name,
                            &range,
                        ],
                    );
                    symbols.push(Symbol {
                        id: id.clone(),
                        file_ref: file_ref.clone(),
                        path,
                        language: s.language.name().into(),
                        kind: s.kind.name(),
                        name: s.name.clone(),
                        qualified_name: s.qualified_name.clone(),
                        range: s.range.into(),
                        name_range: s.name_range.into(),
                        derivation: "parser_definition",
                    });
                    Some(id)
                } else {
                    c.add("unrepresentable_symbols_omitted", 1);
                    None
                }
            }
        };
        if !matches!(e, Entity::Repository)
            && let Some(id) = &id
            && !ids.insert(id.clone())
        {
            return Err("duplicate structural identity".into());
        }
        refs.push(id);
    }
    let mut occurrences = BTreeMap::<String, usize>::new();
    let mut relations = vec![];
    for r in g.relations.iter().filter(|_| include_relations) {
        let Some(path) = safe_path(&r.evidence.relative_path) else {
            c.add("unrepresentable_relations_omitted", 1);
            continue;
        };
        let Some(source_ref) = &refs[r.source.0 as usize] else {
            c.add("unrepresentable_relations_omitted", 1);
            continue;
        };
        let (target_ref, unresolved_name) = match &r.target {
            Target::Resolved(t) => match &refs[t.0 as usize] {
                Some(id) => (Some(id.clone()), None),
                None => {
                    c.add("unrepresentable_relations_omitted", 1);
                    continue;
                }
            },
            Target::Unresolved(n) => (None, Some(n.clone())),
        };
        let mut dto = Relation {
            id: String::new(),
            source_ref: source_ref.clone(),
            target_ref,
            unresolved_name,
            kind: r.kind.name(),
            resolution: r.resolution.name(),
            metadata: metadata(&r.metadata),
            path,
            range: r.evidence.range.map(Into::into),
            language: r.evidence.language.as_ref().map(|l| l.name().into()),
            derivation: r.evidence.derivation.name(),
        };
        let descriptor = identity(
            "relation_descriptor",
            &[repo, scope, &serde_json::to_string(&dto)?],
        );
        let occurrence = occurrences.entry(descriptor.clone()).or_default();
        dto.id = identity("relation", &[&descriptor, &occurrence.to_string()]);
        *occurrence += 1;
        if !ids.insert(dto.id.clone())
            || !ids.contains(&dto.source_ref)
            || dto.target_ref.as_ref().is_some_and(|t| !ids.contains(t))
        {
            return Err("invalid relation reference".into());
        }
        relations.push(dto);
    }
    c.add("files_exported", files.len());
    c.add("symbols_exported", symbols.len());
    c.add("relations_exported", relations.len());
    Ok((
        Structure {
            repository_ref,
            files,
            symbols,
            relations,
        },
        refs,
    ))
}

#[derive(Serialize, JsonSchema)]
struct Changes {
    base: Structure,
    head: Structure,
    base_coverage: Coverage,
    head_coverage: Coverage,
    files: Vec<ChangedFile>,
    scope_prefix: Option<String>,
}
#[derive(Serialize, JsonSchema)]
struct Side {
    path: String,
    mode: String,
    object_id: String,
}
#[derive(Serialize, JsonSchema)]
struct SymbolDelta {
    kind: &'static str,
    before_ref: Option<String>,
    after_ref: Option<String>,
}
#[derive(Serialize, JsonSchema)]
struct ChangedFile {
    status: String,
    similarity: Option<u8>,
    before: Option<Side>,
    after: Option<Side>,
    symbols: Vec<SymbolDelta>,
    relations: Vec<RelationDelta>,
    ambiguous_keys: usize,
    comparison_available: bool,
    relation_comparison_available: bool,
}
#[derive(Serialize, JsonSchema)]
struct Endpoint {
    state: &'static str,
    kind: &'static str,
    name: String,
}
#[derive(Serialize, JsonSchema)]
struct Occurrences {
    count: usize,
    examples: Vec<Range>,
    examples_omitted: usize,
}
#[derive(Serialize, JsonSchema)]
struct RelationDelta {
    source: Endpoint,
    target: Endpoint,
    kind: &'static str,
    resolution: &'static str,
    metadata: Metadata,
    derivation: &'static str,
    before: Occurrences,
    after: Occurrences,
}
fn changes<'a>(repo: &str, set: &change::ChangeSet) -> Result<Document<'a>> {
    let prefix = if set.revisions.prefix.as_os_str().is_empty() {
        None
    } else {
        Some(safe_path(&set.revisions.prefix).ok_or("unrepresentable Git scope")?)
    };
    let scope = |revision: &str| {
        identity(
            "revision_scope",
            &[revision, prefix.as_deref().unwrap_or("")],
        )
    };
    let mut base_coverage = coverage(&set.base.graph);
    let mut head_coverage = coverage(&set.head.graph);
    let (base, br) = project(
        &set.base.graph,
        repo,
        &scope(&set.revisions.base),
        &mut base_coverage,
        true,
    )?;
    let (head, hr) = project(
        &set.head.graph,
        repo,
        &scope(&set.revisions.head),
        &mut head_coverage,
        true,
    )?;
    let mut c = Coverage {
        limitations: vec![
            "changed_pairs_only",
            "git_rename_heuristic",
            "no_cross_refactor_lineage",
        ],
        ..Coverage::default()
    };
    let side = |s: &crate::git::revision::Entry| -> Result<Side> {
        Ok(Side {
            path: safe_path(&s.path).ok_or("unrepresentable essential Git side")?,
            mode: s.mode.clone(),
            object_id: s.oid.clone(),
        })
    };
    let symbol_refs = |g: &graph::RepositoryGraph, refs: &[Option<String>]| {
        g.entities
            .iter()
            .enumerate()
            .filter_map(|(n, e)| match e {
                Entity::Symbol(i) => Some((*i, refs[n].clone())),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>()
    };
    let bs = symbol_refs(&set.base.graph, &br);
    let hs = symbol_refs(&set.head.graph, &hr);
    let symbol_ref = |refs: &BTreeMap<usize, Option<String>>, i: usize| -> Result<String> {
        refs.get(&i)
            .and_then(Clone::clone)
            .ok_or_else(|| "unrepresentable essential symbol".into())
    };
    let mut files = vec![];
    for f in &set.files {
        let mut symbols = vec![];
        for s in &f.symbols {
            symbols.push(SymbolDelta {
                kind: s.kind,
                before_ref: s.before.map(|i| symbol_ref(&bs, i)).transpose()?,
                after_ref: s.after.map(|i| symbol_ref(&hs, i)).transpose()?,
            });
        }
        let endpoint = |e: &change::Endpoint| Endpoint {
            state: e.state,
            kind: e.kind,
            name: e.name.to_string(),
        };
        let occ = |o: &change::Occurrences| Occurrences {
            count: o.count,
            examples: o.examples.iter().copied().map(Into::into).collect(),
            examples_omitted: o.count.saturating_sub(o.examples.len()),
        };
        let relations = f
            .relations
            .iter()
            .map(|r| RelationDelta {
                source: endpoint(&r.signature.source),
                target: endpoint(&r.signature.target),
                kind: r.signature.kind.name(),
                resolution: r.signature.resolution.name(),
                metadata: metadata(&r.signature.metadata),
                derivation: r.signature.derivation.name(),
                before: occ(&r.before),
                after: occ(&r.after),
            })
            .collect();
        c.add("ambiguous_symbol_keys", f.ambiguous_keys);
        c.add(
            "unavailable_symbol_comparisons",
            usize::from(!f.comparison_available),
        );
        c.add(
            "unavailable_relation_comparisons",
            usize::from(!f.relation_comparison_available),
        );
        files.push(ChangedFile {
            status: f.git.status.clone(),
            similarity: f.git.similarity,
            before: f.git.before.as_ref().map(side).transpose()?,
            after: f.git.after.as_ref().map(side).transpose()?,
            symbols,
            relations,
            ambiguous_keys: f.ambiguous_keys,
            comparison_available: f.comparison_available,
            relation_comparison_available: f.relation_comparison_available,
        });
    }
    c.add("changed_files", files.len());
    Ok(document(
        repo,
        Observation {
            basis: Basis::CommittedChangedPairs,
            scope: identity(
                "comparison",
                &[&scope(&set.revisions.base), &scope(&set.revisions.head)],
            ),
            atomic: false,
            git_head: None,
            git_dirty: None,
            base: Some(set.revisions.base.clone()),
            head: Some(set.revisions.head.clone()),
        },
        c,
        Feed::Changes(Changes {
            base,
            head,
            base_coverage,
            head_coverage,
            files,
            scope_prefix: prefix,
        }),
    ))
}
#[derive(Serialize, JsonSchema)]
struct Score {
    total: u16,
    lexical_path: u16,
    lexical_symbol: u16,
    graph: u16,
    role: u16,
    change: u16,
    centrality: u16,
}
#[derive(Serialize, JsonSchema)]
struct Candidate {
    rank: usize,
    file: File,
    score: Score,
    reasons: Vec<&'static str>,
    matched_symbols: Vec<Symbol>,
    matched_symbols_omitted: usize,
    graph_distance: Option<u8>,
    change_distance: Option<u8>,
}
#[derive(Serialize, JsonSchema)]
struct CandidateChange {
    base: String,
    head: String,
    changed_files_observed: usize,
    changed_symbols_observed: usize,
    current_file_mappings: usize,
    current_symbol_mappings: usize,
    unmapped_symbols: usize,
    ambiguous_symbol_keys: usize,
    unavailable_symbol_comparisons: usize,
    deleted_only_entities: usize,
}
#[derive(Serialize, JsonSchema)]
struct Candidates {
    ranker_version: &'static str,
    sort_order: &'static str,
    semantics: &'static str,
    task_digest: String,
    fallback: bool,
    change: Option<CandidateChange>,
    candidates: Vec<Candidate>,
}
fn candidates<'a>(repo: &str, r: &application::Candidates, limit: usize) -> Result<Document<'a>> {
    if !(1..=200).contains(&limit) {
        return Err("candidate limit must be 1 through 200".into());
    }
    let g = &r.graph;
    let observation = worktree(g);
    let mut c = coverage(g);
    c.limitations.extend([
        "best_effort_non_atomic",
        "ranked_metadata_not_content_selection",
    ]);
    let ranking = &r.ranking;
    for (key, value) in [
        ("eligible_files", ranking.eligible),
        ("sensitive_files_omitted", ranking.sensitive_files_omitted),
        (
            "excluded_role_files",
            g.index
                .files
                .iter()
                .filter(|f| !context::eligible(f.role))
                .count(),
        ),
        ("task_terms", r.task.terms.len()),
        ("task_terms_truncated", usize::from(r.task.terms_truncated)),
        ("ranking_roots", ranking.roots),
        ("ranking_roots_omitted", ranking.roots_omitted),
        ("graph_truncated", usize::from(ranking.graph_truncated)),
        ("fallback_candidates_omitted", ranking.fallback_omitted),
    ] {
        c.add(key, value);
    }
    // The shared projection owns all reference derivation. No body loader is called.
    let (structure, _) = project(g, repo, &observation.scope, &mut c, false)?;
    let mut files: BTreeMap<_, _> = structure
        .files
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();
    let mut symbols: BTreeMap<_, _> = structure
        .symbols
        .into_iter()
        .map(|s| (s.id.clone(), s))
        .collect();
    let mut symbol_ids = BTreeMap::new();
    for s in &g.index.symbols {
        if let Some(path) = safe_path(&s.relative_path) {
            let range = serde_json::to_string(&Range::from(s.range))?;
            symbol_ids.insert(
                (
                    s.relative_path.clone(),
                    s.range,
                    s.kind.name(),
                    s.qualified_name.clone(),
                ),
                identity(
                    "symbol",
                    &[
                        repo,
                        &observation.scope,
                        &path,
                        s.kind.name(),
                        &s.name,
                        &s.qualified_name,
                        &range,
                    ],
                ),
            );
        }
    }
    let mut candidates = vec![];
    let mut representable = 0;
    for (i, candidate) in ranking.candidates.iter().enumerate() {
        let f = &g.index.files[candidate.file];
        let Some(path) = safe_path(&f.relative_path) else {
            c.add("unrepresentable_candidates_omitted", 1);
            continue;
        };
        representable += 1;
        if candidates.len() >= limit {
            continue;
        }
        let mut matched_symbols = vec![];
        for &i in &candidate.symbols {
            let s = &g.index.symbols[i];
            let key = (
                s.relative_path.clone(),
                s.range,
                s.kind.name(),
                s.qualified_name.clone(),
            );
            let id = symbol_ids
                .get(&key)
                .ok_or("missing candidate symbol identity")?;
            matched_symbols.push(symbols.remove(id).ok_or("duplicate candidate symbol")?);
        }
        let s = &candidate.score;
        candidates.push(Candidate {
            rank: i + 1,
            file: files.remove(&path).ok_or("duplicate candidate file")?,
            score: Score {
                total: s.total(),
                lexical_path: s.lexical_path,
                lexical_symbol: s.lexical_symbol,
                graph: s.graph,
                role: s.role,
                change: s.change,
                centrality: s.centrality,
            },
            reasons: candidate.reasons.iter().copied().collect(),
            matched_symbols,
            matched_symbols_omitted: candidate.symbols_omitted,
            graph_distance: candidate.graph_distance,
            change_distance: candidate.change_distance,
        });
    }
    c.add("ranking_candidates", ranking.candidates.len());
    c.add("candidate_limit_omitted", representable - candidates.len());
    c.add("candidates_exported", candidates.len());
    // Projection counts describe analysis; keep exported candidate facts separately named.
    for key in ["files_exported", "symbols_exported", "relations_exported"] {
        c.counts.remove(key);
    }
    let change = r.changes.as_ref().map(|s| CandidateChange {
        base: s.base.clone(),
        head: s.head.clone(),
        changed_files_observed: s.files_observed,
        changed_symbols_observed: s.symbols_observed,
        current_file_mappings: s.files.len(),
        current_symbol_mappings: s.symbols_mapped,
        unmapped_symbols: s.symbols_unmapped,
        ambiguous_symbol_keys: s.ambiguous_keys,
        unavailable_symbol_comparisons: s.comparisons_unavailable,
        deleted_only_entities: s.deleted_only,
    });
    Ok(document(
        repo,
        observation,
        c,
        Feed::Candidates(Candidates {
            ranker_version: context::RANKER_VERSION,
            sort_order: context::ranker_v1::FILE_ORDER,
            semantics: "ranked_metadata_not_content_selection",
            task_digest: identity("normalized_task", &[&r.task.phrase]),
            fallback: ranking.fallback,
            change,
            candidates,
        }),
    ))
}
fn render(d: &Document<'_>) -> Result<String> {
    // Include main's trailing LF in the wire cap. The bounded writer retains no partial stdout.
    Ok(crate::output::bounded_json(d, MAX_BYTES - 1)?)
}
fn validate_workspace(
    w: &crate::workspace::Workspace,
    impact: Option<&crate::workspace_impact::Impact>,
) -> Result<()> {
    let paths = |s: &str| -> Result<()> {
        safe_path(Path::new(s)).ok_or("unrepresentable workspace path")?;
        Ok(())
    };
    let mut ids = BTreeSet::new();
    for r in &w.relations {
        if !ids.insert(&r.id) {
            return Err("duplicate workspace relationship ID".into());
        }
        w.repo(&r.provider)?;
        w.repo(&r.consumer)?;
        for p in [
            r.provider_path.as_deref(),
            Some(&r.consumer_path),
            Some(&r.source_file),
        ]
        .into_iter()
        .flatten()
        {
            paths(p)?;
        }
    }
    if let Some(i) = impact {
        for p in &i.changed_paths {
            paths(&p.path)?;
        }
        for p in &i.trigger_omissions {
            if let Some(p) = &p.path {
                paths(p)?;
            }
        }
        for c in &i.candidates {
            w.repo(&c.repo)?;
            for p in &c.trigger_paths {
                paths(p)?;
            }
            if c.witness_edges.iter().any(|&e| e >= w.relations.len())
                || c.direct_triggers
                    .iter()
                    .any(|t| !ids.contains(&t.relation_id))
            {
                return Err("invalid workspace impact reference".into());
            }
        }
    }
    Ok(())
}
pub fn command(args: crate::cli::EvidenceArgs) -> Result<String> {
    use crate::cli::EvidenceCommand;
    match args.command {
        EvidenceCommand::Structure(root) => {
            logical_id(&root.repository_id)?;
            let app = application::Application::pinned(&root.path)?;
            let r = app.graph_with_probe(true)?;
            let observation = worktree(&r.data);
            let mut c = coverage(&r.data);
            c.add("reconciliation_warnings_withheld", r.warnings.len());
            c.limitations.push("best_effort_non_atomic");
            let (data, _) = project(
                &r.data,
                &root.repository_id,
                &observation.scope,
                &mut c,
                true,
            )?;
            render(&document(
                &root.repository_id,
                observation,
                c,
                Feed::Structure(data),
            ))
        }
        EvidenceCommand::Changes { range, root } => {
            logical_id(&root.repository_id)?;
            let app = application::Application::pinned(&root.path)?;
            let set = app.changes(&range)?;
            render(&changes(&root.repository_id, &set)?)
        }
        EvidenceCommand::Candidates {
            root,
            task,
            limit,
            change,
        } => {
            logical_id(&root.repository_id)?;
            let app = application::Application::pinned(&root.path)?;
            let r = app.candidates(&task, change.as_deref())?;
            let mut d = candidates(&root.repository_id, &r.data, limit as usize)?;
            d.coverage
                .add("reconciliation_warnings_withheld", r.warnings.len());
            // Recheck after DTO preparation as the content command does after selection.
            if let Some(c) = &r.data.changes {
                context::recheck_snapshot(&r.data.graph, &c.head)?;
            }
            render(&d)
        }
        EvidenceCommand::Workspace {
            manifest,
            repo,
            change,
            depth,
            ..
        } => {
            let (w, impact) = if let (Some(repo), Some(range)) = (&repo, &change) {
                let (w, i) = application::workspace_impact(&manifest, repo, range, depth)?;
                (w, Some(i))
            } else {
                (application::workspace_inspect(&manifest)?, None)
            };
            validate_workspace(&w, impact.as_ref())?;
            let data = crate::output_workspace::document(
                &w,
                if impact.is_some() {
                    "impact"
                } else {
                    "inspect"
                },
                repo.as_deref(),
                None,
                impact.as_ref(),
            )?;
            let mut c = Coverage {
                limitations: vec![
                    "best_effort_non_atomic",
                    "mixed_time_not_synchronized",
                    "candidate_consumers_only",
                ],
                ..Coverage::default()
            };
            c.add("workspace_gaps", w.coverage.len());
            let mut d = document(
                &w.id,
                Observation {
                    basis: Basis::MixedTimeWorkspace,
                    scope: nonce(),
                    atomic: false,
                    git_head: None,
                    git_dirty: None,
                    base: impact.as_ref().map(|i| i.base.clone()),
                    head: impact.as_ref().map(|i| i.head.clone()),
                },
                c,
                Feed::Workspace(data),
            );
            d.subject = Subject::Workspace {
                workspace_id: w.id.clone(),
            };
            render(&d)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_is_frozen() {
        let mut schema = serde_json::to_value(
            schemars::generate::SchemaSettings::draft2020_12()
                .for_serialize()
                .into_generator()
                .into_root_schema_for::<Document>(),
        )
        .unwrap();
        schema["properties"]["evidence_schema_version"] = serde_json::json!({"const": 1});
        for (def, field, value) in [
            ("Producer", "name", serde_json::json!("context-engine")),
            (
                "Producer",
                "identity_version",
                serde_json::json!("evidence-id-v1"),
            ),
            ("Producer", "analysis_compat_version", serde_json::json!(1)),
            ("Producer", "parser_version", serde_json::Value::Null),
            ("Observation", "atomic", serde_json::json!(false)),
            ("Coverage", "complete", serde_json::json!(false)),
            ("Document", "workspace_schema_version", serde_json::json!(2)),
            ("Document", "manifest_schema_version", serde_json::json!(1)),
        ] {
            schema["$defs"][def]["properties"][field] = serde_json::json!({"const": value});
        }
        for (def, field, variants, nullable) in [
            (
                "File",
                "analysis_status",
                vec![
                    "analyzed",
                    "unsupported_language",
                    "skipped_role",
                    "too_large",
                    "invalid_utf8",
                    "read_failed",
                    "parse_failed",
                    "analysis_limit_exceeded",
                ],
                false,
            ),
            (
                "File",
                "relation_status",
                vec!["analyzed", "extraction_failed", "analysis_limit_exceeded"],
                true,
            ),
            (
                "Symbol",
                "kind",
                vec![
                    "module",
                    "class",
                    "struct",
                    "enum",
                    "union",
                    "trait",
                    "function",
                    "method",
                    "type_alias",
                    "constant",
                    "static",
                ],
                false,
            ),
            ("Symbol", "derivation", vec!["parser_definition"], false),
            (
                "Relation",
                "kind",
                vec!["contains", "imports", "references"],
                false,
            ),
            (
                "Relation",
                "resolution",
                vec![
                    "direct_structural",
                    "exact_same_file_qualified_name",
                    "unresolved",
                ],
                false,
            ),
            (
                "RelationDelta",
                "resolution",
                vec![
                    "direct_structural",
                    "exact_same_file_qualified_name",
                    "unresolved",
                ],
                false,
            ),
            (
                "RelationDelta",
                "kind",
                vec!["contains", "imports", "references"],
                false,
            ),
            (
                "Relation",
                "derivation",
                vec![
                    "repository_contains_file",
                    "lexical_range_containment",
                    "rust_use_syntax",
                    "python_import_syntax",
                    "rust_call_syntax",
                    "python_call_syntax",
                ],
                false,
            ),
            (
                "RelationDelta",
                "derivation",
                vec![
                    "repository_contains_file",
                    "lexical_range_containment",
                    "rust_use_syntax",
                    "python_import_syntax",
                    "rust_call_syntax",
                    "python_call_syntax",
                ],
                false,
            ),
            (
                "Candidates",
                "semantics",
                vec!["ranked_metadata_not_content_selection"],
                false,
            ),
            (
                "Candidates",
                "ranker_version",
                vec!["file-ranker-v1"],
                false,
            ),
            (
                "SymbolDelta",
                "kind",
                vec!["added", "removed", "modified"],
                false,
            ),
        ] {
            let mut values: Vec<serde_json::Value> = variants.into_iter().map(Into::into).collect();
            if nullable {
                values.push(serde_json::Value::Null);
            }
            schema["$defs"][def]["properties"][field] = serde_json::json!({"enum": values});
        }
        let schema = serde_json::to_string_pretty(&schema).unwrap() + "\n";
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/contracts/context-integrity-evidence-v1.schema.json"
        );
        if std::env::var_os("UPDATE_EVIDENCE_SCHEMA").is_some() {
            std::fs::write(path, &schema).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), schema);
    }
    #[test]
    fn strict_paths_and_domain_separation() {
        for path in [
            "",
            "/a",
            "a/../b",
            "a/./b",
            "a//b",
            "a\\b",
            "C:a",
            "x\n",
            "x\u{fffd}",
            "a/",
        ] {
            assert!(safe_path(Path::new(path)).is_none(), "{path}");
        }
        assert_eq!(safe_path(Path::new("a/b.rs")), Some("a/b.rs".into()));
        assert_ne!(
            identity("file", &["ab", "c"]),
            identity("file", &["a", "bc"])
        );
        assert_ne!(identity("file", &["a"]), identity("symbol", &["a"]));
    }
    #[cfg(unix)]
    #[test]
    fn native_identity_omission_does_not_need_filesystem_support() {
        use std::os::unix::ffi::OsStrExt;
        let path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"bad\xff.py"));
        let index = model::RepositoryIndex {
            root: "/unused".into(),
            git: None,
            scan_warnings: vec![],
            diagnostics: vec![],
            symbols: vec![],
            files: vec![model::FileAnalysis {
                relative_path: path,
                language: crate::classify::Language::Python,
                role: crate::classify::FileRole::Source,
                status: model::AnalysisStatus::Analyzed,
                symbol_count: 0,
                syntax_error_count: 0,
            }],
        };
        let g = graph::from_analysis(index, vec![]).unwrap();
        let mut c = coverage(&g);
        let (s, _) = project(&g, "fixture", "scope", &mut c, true).unwrap();
        assert!(s.files.is_empty() && s.relations.is_empty());
        assert_eq!(c.counts["unrepresentable_files_omitted"], 1);
        assert_eq!(c.counts["unrepresentable_relations_omitted"], 1);
    }
    #[test]
    fn application_ranking_returns_metadata_and_enforces_change_gate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("target.py"), "def target():\n    pass\n").unwrap();
        std::fs::write(
            dir.path().join("credentials.py"),
            "def target():\n    pass\n",
        )
        .unwrap();
        let app = application::Application::pinned(dir.path()).unwrap();
        let r = app.candidates("target", None).unwrap();
        assert_eq!(r.data.ranking.sensitive_files_omitted, 1);
        assert_eq!(r.data.ranking.candidates.len(), 1);
        assert_eq!(
            r.data.graph.index.files[r.data.ranking.candidates[0].file].relative_path,
            Path::new("target.py")
        );
        assert!(app.candidates("target", Some("HEAD~1..HEAD")).is_err());
    }
    #[test]
    fn exact_wire_limit_includes_escaping_and_lf() {
        let text = "x".repeat(MAX_BYTES - 3); // JSON quotes + LF
        let encoded = crate::output::bounded_json(&text, MAX_BYTES - 1).unwrap();
        assert_eq!(encoded.len() + 1, MAX_BYTES);
        assert!(crate::output::bounded_json(&(text + "x"), MAX_BYTES - 1).is_err());
        assert!(crate::output::bounded_json(&"\n".repeat(MAX_BYTES / 2), MAX_BYTES - 1).is_err());
    }
}
