//! Schema-v1 adapters; rendering never reads repository content.
use super::{
    Envelope, MAX_JSON_BYTES, bounded_json, escape_controls,
    index::SymbolDto,
    query::{QueryContext, human_context, query_context},
    relative,
};
use crate::{context::*, graph::RepositoryGraph};
use serde::Serialize;
use std::fmt::Write;

fn error() -> serde_json::Error {
    serde_json::Error::io(std::io::Error::other(
        "context output safety limit exceeded",
    ))
}
fn check_materialization(
    g: &RepositoryGraph,
    task: &Task,
    bundle: &Bundle,
    change: Option<&ChangeSignal>,
    limit: usize,
) -> Result<usize, serde_json::Error> {
    let mut estimate = 16384usize;
    let mut add = |n: usize| -> Result<(), serde_json::Error> {
        estimate = estimate
            .checked_add(n)
            .filter(|v| *v <= limit)
            .ok_or_else(error)?;
        Ok(())
    };
    add(task.original.len() * 6 + g.index.root.as_os_str().len() * 6)?;
    if let Some(git) = &g.index.git {
        add(git.head_commit.as_ref().map_or(0, String::len) * 6)?;
    }
    if let Some(change) = change {
        add((change.range.len() + change.base.len() + change.head.len()) * 6 + 2048)?;
    }
    for selected in &bundle.selected {
        add(8192
            + relative(&g.index.files[selected.candidate.file].relative_path).len() * 6
            + selected.content.len() * 6)?;
        for &i in &selected.candidate.symbols {
            let s = &g.index.symbols[i];
            add(4096
                + (s.name.len() + s.qualified_name.len() + relative(&s.relative_path).len()) * 6)?;
        }
    }
    for skip in &bundle.skipped {
        add(1024 + relative(&g.index.files[skip.file].relative_path).len() * 6)?;
    }
    Ok(estimate)
}
#[derive(Serialize)]
struct ScoreDto {
    total: u16,
    lexical_path: u16,
    lexical_symbol: u16,
    graph: u16,
    role: u16,
    change: u16,
    centrality: u16,
}
impl From<&Score> for ScoreDto {
    fn from(s: &Score) -> Self {
        Self {
            total: s.total(),
            lexical_path: s.lexical_path,
            lexical_symbol: s.lexical_symbol,
            graph: s.graph,
            role: s.role,
            change: s.change,
            centrality: s.centrality,
        }
    }
}
#[derive(Serialize)]
struct BudgetDto {
    unit: &'static str,
    max: u64,
    used: u64,
    remaining: u64,
}
#[derive(Serialize)]
struct ChangeDto<'a> {
    range: &'a str,
    base: &'a str,
    head: &'a str,
    changed_files_observed: usize,
    changed_symbols_observed: usize,
    current_file_mappings: usize,
    current_symbol_mappings: usize,
    unmapped_symbols: usize,
    ambiguous_symbol_keys: usize,
    unavailable_symbol_comparisons: usize,
    deleted_only_entities: usize,
}
fn change_dto(c: &ChangeSignal) -> ChangeDto<'_> {
    ChangeDto {
        range: &c.range,
        base: &c.base,
        head: &c.head,
        changed_files_observed: c.files_observed,
        changed_symbols_observed: c.symbols_observed,
        current_file_mappings: c.files.len(),
        current_symbol_mappings: c.symbols_mapped,
        unmapped_symbols: c.symbols_unmapped,
        ambiguous_symbol_keys: c.ambiguous_keys,
        unavailable_symbol_comparisons: c.comparisons_unavailable,
        deleted_only_entities: c.deleted_only,
    }
}
#[derive(Serialize)]
struct CoverageDto {
    files_considered: usize,
    files_eligible: usize,
    excluded_roles: usize,
    task_terms: usize,
    task_terms_truncated: bool,
    lexical_non_utf8_path_omissions: usize,
    ranking_roots: usize,
    ranking_roots_omitted: usize,
    graph_truncated: bool,
    fallback_candidates_omitted: usize,
    content_load_failures: usize,
    content_load_attempts: usize,
    content_load_bytes_reserved: u64,
    content_load_safety_limited: bool,
    content_load_candidates_omitted: usize,
    budget_skips: usize,
    skips_omitted: usize,
    reported_skips_truncated: bool,
    selection_candidates_omitted: usize,
}
fn coverage(g: &RepositoryGraph, t: &Task, b: &Bundle) -> CoverageDto {
    CoverageDto {
        files_considered: g.index.files.len(),
        files_eligible: b.ranking.eligible,
        excluded_roles: g.index.files.len() - b.ranking.eligible,
        task_terms: t.terms.len(),
        task_terms_truncated: t.terms_truncated,
        lexical_non_utf8_path_omissions: g
            .index
            .files
            .iter()
            .filter(|f| f.relative_path.to_str().is_none())
            .count(),
        ranking_roots: b.ranking.roots,
        ranking_roots_omitted: b.ranking.roots_omitted,
        graph_truncated: b.ranking.graph_truncated,
        fallback_candidates_omitted: b.ranking.fallback_omitted,
        content_load_failures: b.load_failures,
        content_load_attempts: b.load_attempts,
        content_load_bytes_reserved: b.load_bytes_reserved,
        content_load_safety_limited: b.load_safety_limited,
        content_load_candidates_omitted: b.load_candidates_omitted,
        budget_skips: b.budget_skips,
        skips_omitted: b.skips_omitted,
        reported_skips_truncated: b.skips_omitted > 0,
        selection_candidates_omitted: b.selection_omitted,
    }
}
#[derive(Serialize)]
struct SelectedDto<'a> {
    rank: usize,
    file_id: u32,
    path: String,
    path_lossy: bool,
    language: String,
    role: String,
    content_bytes: usize,
    score: ScoreDto,
    reasons: Vec<&'static str>,
    matched_symbols: Vec<SymbolDto<'a>>,
    matched_symbols_omitted: usize,
    graph_distance: Option<u8>,
    change_distance: Option<u8>,
    content: &'a str,
}
#[derive(Serialize)]
struct SkipDto {
    rank: usize,
    path: String,
    path_lossy: bool,
    reason: &'static str,
}
#[derive(Serialize)]
struct ContextDto<'a> {
    repository: QueryContext,
    git_head: Option<&'a str>,
    git_dirty: Option<bool>,
    task: &'a str,
    ranker_version: &'static str,
    fallback_mode: Option<&'static str>,
    budget: BudgetDto,
    change: Option<ChangeDto<'a>>,
    coverage: CoverageDto,
    selected: Vec<SelectedDto<'a>>,
    skipped: Vec<SkipDto>,
}
pub fn json(
    g: &RepositoryGraph,
    t: &Task,
    b: &Bundle,
    c: Option<&ChangeSignal>,
) -> Result<String, serde_json::Error> {
    bounded_json(&dto(g, t, b, c)?, MAX_JSON_BYTES)
}
pub fn dto<'a>(
    g: &'a RepositoryGraph,
    t: &'a Task,
    b: &'a Bundle,
    c: Option<&'a ChangeSignal>,
) -> Result<impl Serialize + 'a, serde_json::Error> {
    check_materialization(g, t, b, c, MAX_JSON_BYTES)?;
    let selected = b
        .selected
        .iter()
        .map(|s| {
            let f = &g.index.files[s.candidate.file];
            let candidate = &s.candidate;
            SelectedDto {
                rank: s.rank,
                file_id: (1 + candidate.file) as u32,
                path: relative(&f.relative_path),
                path_lossy: f.relative_path.to_str().is_none(),
                language: f.language.name().to_lowercase(),
                role: f.role.name().to_lowercase(),
                content_bytes: s.content.len(),
                score: (&candidate.score).into(),
                reasons: candidate.reasons.iter().copied().collect(),
                matched_symbols: candidate
                    .symbols
                    .iter()
                    .map(|&i| (&g.index.symbols[i]).into())
                    .collect(),
                matched_symbols_omitted: candidate.symbols_omitted,
                graph_distance: candidate.graph_distance,
                change_distance: candidate.change_distance,
                content: &s.content,
            }
        })
        .collect();
    let skipped = b
        .skipped
        .iter()
        .map(|s| SkipDto {
            rank: s.rank,
            path: relative(&g.index.files[s.file].relative_path),
            path_lossy: g.index.files[s.file].relative_path.to_str().is_none(),
            reason: s.reason,
        })
        .collect();
    Ok(Envelope {
        schema_version: 1,
        command: "context".into(),
        data: ContextDto {
            repository: query_context(&g.index, Some(g)),
            git_head: g.index.git.as_ref().and_then(|m| m.head_commit.as_deref()),
            git_dirty: g.index.git.as_ref().and_then(|m| m.is_dirty),
            task: &t.original,
            ranker_version: RANKER_VERSION,
            fallback_mode: b.ranking.fallback.then_some("centrality"),
            budget: BudgetDto {
                unit: "utf8_content_bytes",
                max: b.budget,
                used: b.used,
                remaining: b.budget - b.used,
            },
            change: c.map(change_dto),
            coverage: coverage(g, t, b),
            selected,
            skipped,
        },
    })
}
pub fn human(
    g: &RepositoryGraph,
    t: &Task,
    b: &Bundle,
    c: Option<&ChangeSignal>,
) -> Result<String, serde_json::Error> {
    let estimate = check_materialization(g, t, b, c, MAX_JSON_BYTES)?;
    let mut out = String::with_capacity(estimate);
    out.push_str(&human_context(&g.index, Some(g)));
    writeln!(out, "Task: {}\nRanker: {}\nFallback: {}\nBudget: {} / {} utf8_content_bytes\nGit HEAD: {} (dirty: {:?})", escape_controls(&t.original), RANKER_VERSION, if b.ranking.fallback { "centrality" } else { "none" }, b.used, b.budget, escape_controls(g.index.git.as_ref().and_then(|m|m.head_commit.as_deref()).unwrap_or("unavailable")), g.index.git.as_ref().and_then(|m| m.is_dirty)).unwrap();
    if let Some(c) = c {
        writeln!(out,"Change: {} ({}..{}); files observed/mapped: {}/{}; symbols observed/mapped/unmapped: {}/{}/{}; deleted-only entities: {}", escape_controls(&c.range), c.base, c.head, c.files_observed,c.files.len(),c.symbols_observed,c.symbols_mapped,c.symbols_unmapped,c.deleted_only).unwrap();
    }
    let cv = coverage(g, t, b);
    writeln!(out,"Coverage: files={} eligible={} excluded_roles={} terms={} terms_truncated={} non_utf8_path_omissions={} roots={} omitted_roots={} graph_truncated={} fallback_omitted={} load_failures={} load_attempts={} load_bytes_reserved={} load_safety_limited={} load_candidates_omitted={} budget_skips={} unreported_skips={} selection_omitted={}",cv.files_considered,cv.files_eligible,cv.excluded_roles,cv.task_terms,cv.task_terms_truncated,cv.lexical_non_utf8_path_omissions,cv.ranking_roots,cv.ranking_roots_omitted,cv.graph_truncated,cv.fallback_candidates_omitted,cv.content_load_failures,cv.content_load_attempts,cv.content_load_bytes_reserved,cv.content_load_safety_limited,cv.content_load_candidates_omitted,cv.budget_skips,cv.skips_omitted,cv.selection_candidates_omitted).unwrap();
    for s in &b.selected {
        let f = &g.index.files[s.candidate.file];
        writeln!(
            out,
            "\n{}. {} ({} bytes; path_lossy={})\n   score: {:?} total={}\n   reasons: {}",
            s.rank,
            escape_controls(&relative(&f.relative_path)),
            s.content.len(),
            f.relative_path.to_str().is_none(),
            s.candidate.score,
            s.candidate.score.total(),
            s.candidate
                .reasons
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
        if s.candidate.reasons.contains("related_test_candidate") {
            out.push_str("   related test candidate\n");
        }
        for &i in &s.candidate.symbols {
            writeln!(
                out,
                "   matched symbol: {}",
                escape_controls(&g.index.symbols[i].qualified_name)
            )
            .unwrap();
        }
        writeln!(
            out,
            "   matched symbols omitted: {}\n   --- content (each line prefixed |) ---",
            s.candidate.symbols_omitted
        )
        .unwrap();
        // Prefix every line; source cannot impersonate top-level metadata/boundaries.
        for line in s.content.split('\n') {
            out.push_str("   | ");
            for c in line.chars() {
                match c {
                    '\t' => out.push(c),
                    '\r' => out.push_str("\\r"),
                    c if c.is_control() => write!(out, "\\u{{{:x}}}", c as u32).unwrap(),
                    c => out.push(c),
                }
            }
            out.push('\n');
        }
        out.push_str("   --- end content ---\n");
    }
    for s in &b.skipped {
        writeln!(
            out,
            "Skipped rank {}: {} ({})",
            s.rank,
            escape_controls(&relative(&g.index.files[s.file].relative_path)),
            s.reason
        )
        .unwrap();
    }
    if out.len() > MAX_JSON_BYTES {
        return Err(error());
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn materialization_rejects_before_rendering() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("task.py"),
            "def task():\n    pass # é \u{001b}\n",
        )
        .unwrap();
        let g = crate::graph::build(&crate::model::IndexOptions {
            root: dir.path().into(),
            max_parse_file_bytes: 100,
            probe_git: false,
        })
        .unwrap();
        let t = Task::parse("task").unwrap();
        let b = select(&g, rank(&g, &t, None), 100);
        assert_eq!(b.selected.len(), 1);
        let n = check_materialization(&g, &t, &b, None, MAX_JSON_BYTES).unwrap();
        assert!(json(&g, &t, &b, None).unwrap().len() <= n);
        assert!(human(&g, &t, &b, None).unwrap().len() <= n);
        assert_eq!(check_materialization(&g, &t, &b, None, n).unwrap(), n);
        assert!(check_materialization(&g, &t, &b, None, n - 1).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn non_utf8_path_coverage_uses_native_identity() {
        use crate::{
            classify::{FileRole, Language},
            model::*,
        };
        use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};
        // The desktop sandbox cannot create invalid-UTF-8 names. Inject the
        // scanner's native-path model, as the existing search regression does.
        let d = tempfile::tempdir().unwrap();
        let index = RepositoryIndex {
            root: d.path().into(),
            git: None,
            scan_warnings: vec![],
            diagnostics: vec![],
            symbols: vec![],
            files: vec![FileAnalysis {
                relative_path: PathBuf::from(OsString::from_vec(b"odd\xff.txt".to_vec())),
                language: Language::Unknown,
                role: FileRole::Unknown,
                status: AnalysisStatus::UnsupportedLanguage,
                symbol_count: 0,
                syntax_error_count: 0,
            }],
        };
        let g = crate::graph::from_analysis(index, vec![]).unwrap();
        let t = Task::parse("odd").unwrap();
        let b = select(&g, rank(&g, &t, None), 100);
        let v: serde_json::Value = serde_json::from_str(&json(&g, &t, &b, None).unwrap()).unwrap();
        assert_eq!(v["data"]["coverage"]["lexical_non_utf8_path_omissions"], 1);
        assert_eq!(v["data"]["fallback_mode"], "centrality");
        assert_eq!(v["data"]["skipped"][0]["path_lossy"], true);
    }

    #[test]
    fn load_safety_budget_is_reported_in_both_formats() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("task.py"), "def task(): pass\n").unwrap();
        let g = crate::graph::build(&crate::model::IndexOptions {
            root: dir.path().into(),
            max_parse_file_bytes: 100,
            probe_git: false,
        })
        .unwrap();
        let task = Task::parse("task").unwrap();
        let mut bundle = select(&g, rank(&g, &task, None), 100);
        bundle.load_attempts = 1;
        bundle.load_bytes_reserved = 101;
        bundle.load_safety_limited = true;
        bundle.load_candidates_omitted = 3;

        let json: serde_json::Value =
            serde_json::from_str(&self::json(&g, &task, &bundle, None).unwrap()).unwrap();
        assert_eq!(json["data"]["ranker_version"], RANKER_VERSION);
        assert_eq!(json["data"]["coverage"]["content_load_attempts"], 1);
        assert_eq!(json["data"]["coverage"]["content_load_bytes_reserved"], 101);
        assert_eq!(
            json["data"]["coverage"]["content_load_safety_limited"],
            true
        );
        assert_eq!(
            json["data"]["coverage"]["content_load_candidates_omitted"],
            3
        );

        let human = self::human(&g, &task, &bundle, None).unwrap();
        assert!(human.contains(&format!("Ranker: {RANKER_VERSION}")));
        assert!(human.contains("load_attempts=1 load_bytes_reserved=101 load_safety_limited=true load_candidates_omitted=3"));
    }
}
