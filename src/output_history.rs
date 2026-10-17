//! Explicit schema-v1 history DTOs. Rendering never reads repository state.
use super::{Envelope, bounded_json, escape_controls, relative};
use crate::history::{Descriptor, HistoryResult};
use serde::Serialize;

const MAX_OUTPUT: usize = 16 * 1024 * 1024;
#[derive(Serialize)]
struct SymbolDto<'a> {
    path: String,
    language: &'a str,
    kind: &'a str,
    qualified_name: &'a str,
}
fn symbol(d: &Descriptor) -> SymbolDto<'_> {
    SymbolDto {
        path: relative(&d.path),
        language: &d.language,
        kind: d.kind,
        qualified_name: &d.qualified_name,
    }
}
#[derive(Serialize)]
struct EntryDto<'a> {
    path: String,
    path_is_utf8: bool,
    mode: &'a str,
    blob_oid: &'a str,
}
fn entry(e: &crate::git::revision::Entry) -> EntryDto<'_> {
    EntryDto {
        path: relative(&e.path),
        path_is_utf8: e.path.to_str().is_some(),
        mode: &e.mode,
        blob_oid: &e.oid,
    }
}
#[derive(Serialize)]
struct StructuralDto {
    comparison_available: bool,
    relation_comparison_available: bool,
    symbols_added: usize,
    symbols_removed: usize,
    symbols_modified: usize,
    relations_added: usize,
    relations_removed: usize,
    ambiguous_keys: usize,
}
#[derive(Serialize)]
struct EventDto<'a> {
    commit: &'a str,
    first_parent: Option<&'a str>,
    committer_timestamp: i64,
    subject: &'a str,
    kind: &'a str,
    git_status: &'a str,
    rename_similarity: Option<u8>,
    before: Option<EntryDto<'a>>,
    after: Option<EntryDto<'a>>,
    structural: StructuralDto,
    coverage_gap: Option<&'a str>,
    lineage_symbol_modified: Option<bool>,
}
#[derive(Serialize)]
struct CochangeDto {
    path: String,
    path_is_utf8: bool,
    cochange_commit_count: usize,
}
#[derive(Serialize)]
struct CoverageDto {
    commits_inspected: usize,
    target_change_commits: usize,
    structural_comparisons_attempted: usize,
    structural_comparisons_available: usize,
    relation_comparisons_available: usize,
    coverage_gaps: usize,
    head_analysis_gaps: usize,
    history_truncated: bool,
    lineage_complete_to_introduction: bool,
    cochanges_omitted: usize,
}
#[derive(Default, Serialize)]
struct TotalsDto {
    file_change_counts: std::collections::BTreeMap<String, usize>,
    symbols_added: usize,
    symbols_removed: usize,
    symbols_modified: usize,
    relations_added: usize,
    relations_removed: usize,
}
fn totals(r: &HistoryResult) -> TotalsDto {
    let mut t = TotalsDto::default();
    for e in &r.events {
        *t.file_change_counts
            .entry(e.git.status.clone())
            .or_default() += 1;
        t.symbols_added += e.structural.added;
        t.symbols_removed += e.structural.removed;
        t.symbols_modified += e.structural.modified;
        t.relations_added += e.structural.relations_added;
        t.relations_removed += e.structural.relations_removed;
    }
    t
}
#[derive(Serialize)]
struct DataDto<'a> {
    repository_root: String,
    scan_root: String,
    start_revision: &'a str,
    history_policy: &'static str,
    event_order: &'static str,
    lineage_rule: &'static str,
    structural_derivation: &'static str,
    analysis_compat_version: i64,
    target: String,
    symbol: Option<SymbolDto<'a>>,
    events: Vec<EventDto<'a>>,
    reported_event_file_totals: TotalsDto,
    cochanges: Vec<CochangeDto>,
    cochange_meaning: &'static str,
    cochange_denominator: usize,
    coverage: CoverageDto,
}
fn error() -> serde_json::Error {
    serde_json::Error::io(std::io::Error::other(
        "history output safety limit exceeded",
    ))
}
fn preflight(r: &HistoryResult, max: usize) -> Result<(), serde_json::Error> {
    let mut n = 8192usize;
    let mut add = |bytes: usize| -> Result<(), serde_json::Error> {
        n = n
            .checked_add(bytes.saturating_mul(6))
            .filter(|n| *n <= max)
            .ok_or_else(error)?;
        Ok(())
    };
    add(r.root.as_os_str().len() + r.scope.as_os_str().len() + r.target.as_os_str().len())?;
    if let Some(s) = &r.symbol {
        add(s.path.as_os_str().len() + s.qualified_name.len())?;
    }
    for e in &r.events {
        add(2048 + e.commit.subject.len() + e.gap.as_deref().map_or(0, str::len))?;
        for entry in [&e.git.before, &e.git.after].into_iter().flatten() {
            add(entry.path.as_os_str().len())?;
        }
    }
    for (p, _) in &r.cochanges {
        add(128 + p.as_os_str().len())?;
    }
    Ok(())
}
pub fn json(command: &str, r: &HistoryResult) -> Result<String, serde_json::Error> {
    preflight(r, MAX_OUTPUT)?;
    let c = &r.coverage;
    let data = DataDto {
        repository_root: r.root.to_string_lossy().into_owned(),
        scan_root: r.scope.to_string_lossy().into_owned(),
        start_revision: &r.start_revision,
        history_policy: "first_parent",
        event_order: "newest_first_git_walk",
        lineage_rule: "unique_exact_language_kind_qualified_name_in_file_pair; follow_git_file_renames_only",
        structural_derivation: "shared_revision_syntax_analysis_and_source_slice_comparison",
        analysis_compat_version: crate::persistence::INDEX_COMPAT_VERSION,
        target: if r.target.as_os_str().is_empty() {
            ".".into()
        } else {
            relative(&r.target)
        },
        symbol: r.symbol.as_ref().map(symbol),
        events: r
            .events
            .iter()
            .map(|e| EventDto {
                commit: &e.commit.oid,
                first_parent: e.commit.first_parent.as_deref(),
                committer_timestamp: e.commit.committer_timestamp,
                subject: &e.commit.subject,
                kind: e.kind,
                git_status: &e.git.status,
                rename_similarity: e.git.similarity,
                before: e.git.before.as_ref().map(entry),
                after: e.git.after.as_ref().map(entry),
                structural: StructuralDto {
                    comparison_available: e.structural.available,
                    relation_comparison_available: e.structural.relations_available,
                    symbols_added: e.structural.added,
                    symbols_removed: e.structural.removed,
                    symbols_modified: e.structural.modified,
                    relations_added: e.structural.relations_added,
                    relations_removed: e.structural.relations_removed,
                    ambiguous_keys: e.structural.ambiguous_keys,
                },
                coverage_gap: e.gap.as_deref(),
                lineage_symbol_modified: e.lineage_symbol_modified,
            })
            .collect(),
        reported_event_file_totals: totals(r),
        cochanges: r
            .cochanges
            .iter()
            .map(|(p, n)| CochangeDto {
                path: relative(p),
                path_is_utf8: p.to_str().is_some(),
                cochange_commit_count: *n,
            })
            .collect(),
        cochange_meaning: "same scoped commit occurrence; not dependency, causality or test coverage",
        cochange_denominator: c.target_change_commits,
        coverage: CoverageDto {
            commits_inspected: c.commits_inspected,
            target_change_commits: c.target_change_commits,
            structural_comparisons_attempted: c.structural_comparisons,
            structural_comparisons_available: c.structural_available,
            relation_comparisons_available: c.relation_comparisons_available,
            coverage_gaps: c.gaps,
            head_analysis_gaps: c.head_analysis_gaps,
            history_truncated: c.history_truncated,
            lineage_complete_to_introduction: c.lineage_complete_to_introduction,
            cochanges_omitted: c.cochanges_omitted,
        },
    };
    bounded_json(
        &Envelope {
            schema_version: 1,
            command: command.into(),
            data,
        },
        MAX_OUTPUT,
    )
}
pub fn human(command: &str, r: &HistoryResult) -> Result<String, serde_json::Error> {
    preflight(r, MAX_OUTPUT)?;
    use std::fmt::Write;
    let c = &r.coverage;
    let mut out = format!(
        "{command}: {}\nstart revision: {}\nhistory policy: first_parent (merge result vs first parent; newest first)\nlineage: unique exact descriptor; Git file renames only\n",
        escape_controls(&if r.target.as_os_str().is_empty() {
            ".".into()
        } else {
            relative(&r.target)
        }),
        r.start_revision
    );
    if let Some(s) = &r.symbol {
        let _ = writeln!(
            out,
            "symbol: {} ({}, {})",
            escape_controls(&s.qualified_name),
            s.language,
            s.kind
        );
    }
    for e in &r.events {
        let path = |entry: Option<&crate::git::revision::Entry>| {
            entry
                .map(|e| escape_controls(&relative(&e.path)))
                .unwrap_or_else(|| "(absent)".into())
        };
        let _ = writeln!(
            out,
            "{} {} [{}] {} -> {}: {}",
            e.commit.oid,
            e.kind,
            e.git.status,
            path(e.git.before.as_ref()),
            path(e.git.after.as_ref()),
            escape_controls(&e.commit.subject)
        );
        if let Some(modified) = e.lineage_symbol_modified {
            let _ = writeln!(out, "  lineage symbol source slice modified={modified}");
        }
        let s = &e.structural;
        let _ = writeln!(
            out,
            "  syntax evidence: available={} symbols +{} -{} modified={} ambiguous={}; relations available={} +{} -{}",
            s.available,
            s.added,
            s.removed,
            s.modified,
            s.ambiguous_keys,
            s.relations_available,
            s.relations_added,
            s.relations_removed
        );
        if let Some(gap) = &e.gap {
            let _ = writeln!(out, "  coverage gap: {}", escape_controls(gap));
        }
    }
    let t = totals(r);
    let _ = writeln!(
        out,
        "reported event file totals: {:?}; syntax symbols +{} -{} modified={}, relations +{} -{} (available comparisons only)",
        t.file_change_counts,
        t.symbols_added,
        t.symbols_removed,
        t.symbols_modified,
        t.relations_added,
        t.relations_removed
    );
    let _ = writeln!(
        out,
        "co-change: same scoped commit occurrence (not dependency, causality or coverage); denominator={}",
        c.target_change_commits
    );
    for (p, n) in &r.cochanges {
        let _ = writeln!(out, "  {} {n}", escape_controls(&relative(p)));
    }
    let _ = writeln!(
        out,
        "coverage: inspected={} target commits={} structural={}/{} relation comparisons={} gaps={} HEAD analysis gaps={}\nhistory truncated={} lineage complete to introduction={} co-change omitted={}",
        c.commits_inspected,
        c.target_change_commits,
        c.structural_available,
        c.structural_comparisons,
        c.relation_comparisons_available,
        c.gaps,
        c.head_analysis_gaps,
        c.history_truncated,
        c.lineage_complete_to_introduction,
        c.cochanges_omitted
    );
    if out.len() > MAX_OUTPUT {
        return Err(error());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn materialization_is_checked_before_dto_allocation() {
        let r = HistoryResult {
            root: "/repo".into(),
            scope: "/repo".into(),
            start_revision: "a".repeat(40),
            target: "a.py".into(),
            symbol: None,
            events: vec![],
            cochanges: vec![],
            coverage: Default::default(),
        };
        let exact = 8192 + 6 * (5 + 5 + 4);
        assert!(preflight(&r, exact).is_ok());
        assert!(preflight(&r, exact - 1).is_err());
        let v: serde_json::Value = serde_json::from_str(&json("evolution", &r).unwrap()).unwrap();
        assert_eq!(v["schema_version"], 1);
    }
}
