//! Read-only intelligence over the canonical bounded index and graph.
use crate::{graph::*, model::RepositoryIndex, paths::utf8_relative, relations::RelationKind};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_QUERY_BYTES: usize = 4096;
pub const MAX_RESULTS: usize = 200;
#[derive(Debug, Clone, Copy)]
pub enum SearchKind {
    All,
    Symbol,
    File,
}
impl SearchKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Symbol => "symbol",
            Self::File => "file",
        }
    }
}
pub const MATCH_REASONS: [&str; 9] = [
    "exact_qualified_symbol",
    "exact_local_symbol",
    "exact_file_path",
    "qualified_symbol_prefix",
    "local_symbol_prefix",
    "file_path_prefix",
    "qualified_symbol_substring",
    "local_symbol_substring",
    "file_path_substring",
];
pub struct SearchMatch {
    pub entity: EntityId,
    pub tier: usize,
}
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
}
pub fn validate_limit(limit: usize) -> Result<(), &'static str> {
    if (1..=MAX_RESULTS).contains(&limit) {
        Ok(())
    } else {
        Err("limit must be between 1 and 200")
    }
}
pub fn validate_query(query: &str) -> Result<(), &'static str> {
    if query.is_empty() || query.len() > MAX_QUERY_BYTES {
        Err("search query must contain 1 through 4096 UTF-8 bytes")
    } else {
        Ok(())
    }
}
fn tier(text: &str, query: &str, base: usize) -> Option<usize> {
    if text == query {
        Some(base)
    } else if text.starts_with(query) {
        Some(base + 3)
    } else if text.contains(query) {
        Some(base + 6)
    } else {
        None
    }
}
pub fn search(
    index: &RepositoryIndex,
    query: &str,
    kind: SearchKind,
    limit: usize,
) -> Result<SearchResult, &'static str> {
    validate_query(query)?;
    validate_limit(limit)?;
    let mut matches = vec![];
    if !matches!(kind, SearchKind::Symbol) {
        for (i, file) in index.files.iter().enumerate() {
            let Some(path) = utf8_relative(&file.relative_path) else {
                continue;
            };
            if let Some(tier) = tier(&path, query, 2) {
                matches.push(SearchMatch {
                    entity: EntityId((1 + i) as u32),
                    tier,
                });
            }
        }
    }
    if !matches!(kind, SearchKind::File) {
        for (i, symbol) in index.symbols.iter().enumerate() {
            if let Some(tier) = [
                tier(&symbol.qualified_name, query, 0),
                tier(&symbol.name, query, 1),
            ]
            .into_iter()
            .flatten()
            .min()
            {
                matches.push(SearchMatch {
                    entity: EntityId((1 + index.files.len() + i) as u32),
                    tier,
                });
            }
        }
    }
    matches.sort_by_key(|m| (m.tier, m.entity));
    let truncated = matches.len() > limit;
    matches.truncate(limit);
    Ok(SearchResult { matches, truncated })
}

/// Maps graph entities to their owning file once; no source access or per-entity graph scans.
pub fn file_owners(g: &RepositoryGraph) -> Vec<Option<usize>> {
    let files: BTreeMap<_, _> = g
        .index
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.relative_path.as_path(), i))
        .collect();
    g.entities
        .iter()
        .map(|e| match e {
            Entity::Repository => None,
            Entity::File(i) => Some(*i),
            Entity::Symbol(i) => files
                .get(g.index.symbols[*i].relative_path.as_path())
                .copied(),
        })
        .collect()
}
#[derive(Default, Clone)]
pub struct Metrics {
    pub incoming: usize,
    pub outgoing: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub unresolved_references: usize,
    pub unresolved_imports: usize,
    pub child_symbols: usize,
    pub symbol_count: usize,
    pub structural_activity: usize,
}
pub struct Hotspots {
    pub metrics: Vec<Metrics>,
    pub symbols: Vec<EntityId>,
    pub files: Vec<EntityId>,
    pub symbols_truncated: bool,
    pub files_truncated: bool,
}
pub fn structural_metrics(g: &RepositoryGraph, owners: &[Option<usize>]) -> Vec<Metrics> {
    let mut metrics = vec![Metrics::default(); g.entities.len()];
    let mut incoming = vec![BTreeSet::new(); g.entities.len()];
    let mut outgoing = vec![BTreeSet::new(); g.entities.len()];
    for (i, e) in g.entities.iter().enumerate() {
        if matches!(e, Entity::Symbol(_))
            && let Some(f) = owners[i]
        {
            metrics[1 + f].symbol_count += 1;
        }
    }
    for r in &g.relations {
        let s = r.source.0 as usize;
        if r.kind == RelationKind::Contains {
            if let Target::Resolved(t) = r.target
                && matches!(g.entities[t.0 as usize], Entity::Symbol(_))
            {
                metrics[s].child_symbols += 1;
            }
            continue;
        }
        let sf = owners[s].map(|f| 1 + f);
        let tf = match r.target {
            Target::Resolved(t) => owners[t.0 as usize].map(|f| 1 + f),
            _ => None,
        };
        // Activity counts an occurrence once per associated file, including same-file edges.
        if let Some(f) = sf {
            metrics[f].structural_activity += 1;
        }
        if let Some(f) = tf.filter(|f| Some(*f) != sf) {
            metrics[f].structural_activity += 1;
        }
        match r.target {
            Target::Resolved(t) if r.kind == RelationKind::References => {
                let t = t.0 as usize;
                metrics[s].outgoing += 1;
                metrics[t].incoming += 1;
                incoming[t].insert(r.source);
                outgoing[s].insert(EntityId(t as u32));
                if let Some(f) = sf.filter(|f| *f != s) {
                    metrics[f].outgoing += 1;
                }
                if let Some(f) = tf.filter(|f| *f != t) {
                    metrics[f].incoming += 1;
                }
            }
            Target::Unresolved(_) => {
                for id in std::iter::once(s).chain(sf.filter(|f| *f != s)) {
                    if r.kind == RelationKind::Imports {
                        metrics[id].unresolved_imports += 1;
                    } else {
                        metrics[id].unresolved_references += 1;
                    }
                }
            }
            _ => {}
        }
    }
    for i in 0..metrics.len() {
        metrics[i].fan_in = incoming[i].len();
        metrics[i].fan_out = outgoing[i].len();
    }
    metrics
}

pub fn hotspots(g: &RepositoryGraph, limit: usize) -> Result<Hotspots, &'static str> {
    validate_limit(limit)?;
    let metrics = structural_metrics(g, &file_owners(g));
    let mut symbols: Vec<_> = g
        .entities
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            matches!(e, Entity::Symbol(_)) && metrics[*i].incoming + metrics[*i].outgoing > 0
        })
        .map(|(i, _)| EntityId(i as u32))
        .collect();
    let mut files: Vec<_> = (0..g.index.files.len())
        .map(|i| EntityId((1 + i) as u32))
        .filter(|i| metrics[i.0 as usize].structural_activity > 0)
        .collect();
    symbols.sort_by_key(|id| {
        let m = &metrics[id.0 as usize];
        (
            std::cmp::Reverse((m.fan_in, m.incoming, m.fan_out, m.outgoing)),
            *id,
        )
    });
    files.sort_by_key(|id| {
        let m = &metrics[id.0 as usize];
        (
            std::cmp::Reverse((
                m.structural_activity,
                m.incoming + m.outgoing,
                m.unresolved_references + m.unresolved_imports,
                m.symbol_count,
            )),
            *id,
        )
    });
    let symbols_truncated = symbols.len() > limit;
    let files_truncated = files.len() > limit;
    symbols.truncate(limit);
    files.truncate(limit);
    Ok(Hotspots {
        metrics,
        symbols,
        files,
        symbols_truncated,
        files_truncated,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{
        classify::{FileRole, Language},
        model::{AnalysisStatus, FileAnalysis},
    };
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

    #[test]
    fn distinct_non_utf8_paths_that_display_identically_are_not_searched_as_text() {
        let first = PathBuf::from(OsString::from_vec(b"odd\xff.rs".to_vec()));
        let second = PathBuf::from(OsString::from_vec(b"odd\xfe.rs".to_vec()));
        assert_ne!(first, second);
        assert_eq!(first.to_string_lossy(), second.to_string_lossy());

        let index = RepositoryIndex {
            root: "/fixture".into(),
            git: None,
            scan_warnings: vec![],
            files: [first, second]
                .into_iter()
                .map(|relative_path| FileAnalysis {
                    relative_path,
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

        let result = search(&index, "odd\u{fffd}.rs", SearchKind::File, 20).unwrap();
        assert!(result.matches.is_empty());
    }
}
