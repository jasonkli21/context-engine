//! Revision-scoped change evidence. Numeric graph IDs never establish lineage.
use crate::{
    classify::{classify_language, classify_role},
    git::revision::{self, Entry, FileChange, Limits, Revisions},
    graph::{self, Entity, EntityId, QueryResult, RepositoryGraph, Target, TraversalLimits},
    index,
    model::*,
    relations::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub struct Snapshot {
    pub graph: RepositoryGraph,
    texts: BTreeMap<PathBuf, String>,
    symbols_by_file: BTreeMap<PathBuf, Vec<usize>>,
    symbol_names: Vec<Rc<str>>,
    relations_by_file: BTreeMap<PathBuf, Vec<usize>>,
}
pub struct SymbolChange {
    pub kind: &'static str,
    pub before: Option<usize>,
    pub after: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Endpoint {
    pub state: &'static str,
    pub kind: &'static str,
    pub name: Rc<str>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Signature {
    pub source: Endpoint,
    pub target: Endpoint,
    pub kind: RelationKind,
    pub resolution: Resolution,
    pub metadata: Metadata,
    pub derivation: Derivation,
}
pub struct Occurrences {
    pub count: usize,
    pub examples: Vec<SourceRange>,
}
pub struct RelationDelta {
    pub signature: Signature,
    pub before: Occurrences,
    pub after: Occurrences,
}
pub struct ChangedFile {
    pub git: FileChange,
    pub symbols: Vec<SymbolChange>,
    pub relations: Vec<RelationDelta>,
    pub ambiguous_keys: usize,
    pub comparison_available: bool,
    pub relation_comparison_available: bool,
}
pub struct Impact {
    pub base: QueryResult,
    pub head: QueryResult,
    pub base_seed_count: usize,
    pub head_seed_count: usize,
}
pub struct ChangeSet {
    pub revisions: Revisions,
    pub range: String,
    pub files: Vec<ChangedFile>,
    pub base: Snapshot,
    pub head: Snapshot,
    pub impact: Option<Impact>,
}

pub fn analyze(path: &Path, range: &str, max: u64, depth: Option<u8>) -> Result<ChangeSet> {
    analyze_with_limits(path, range, max, depth, revision::LIMITS)
}
fn analyze_with_limits(
    path: &Path,
    range: &str,
    max: u64,
    depth: Option<u8>,
    limits: Limits,
) -> Result<ChangeSet> {
    let revisions = Revisions::open(path, range, limits)?;
    let changes = revisions.changes(limits)?;
    let entries = |before: bool| -> Result<Vec<Entry>> {
        if depth.is_some() {
            return Ok(revisions.inventory(
                if before {
                    &revisions.base
                } else {
                    &revisions.head
                },
                limits,
            )?);
        }
        let mut entries = vec![];
        for c in &changes {
            if let Some(e) = if before { &c.before } else { &c.after } {
                let mut e = e.clone();
                if e.regular() {
                    e.size = revisions.size(&e)?;
                }
                entries.push(e);
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    };
    let wanted = |before: bool| {
        changes
            .iter()
            .filter_map(|c| {
                if before {
                    c.before.as_ref()
                } else {
                    c.after.as_ref()
                }
            })
            .map(|e| e.path.clone())
            .collect()
    };
    let mut base = snapshot(
        &revisions,
        &revisions.base,
        entries(true)?,
        &wanted(true),
        max,
        limits,
    )?;
    let mut head = snapshot(
        &revisions,
        &revisions.head,
        entries(false)?,
        &wanted(false),
        max,
        limits,
    )?;
    let mut files = vec![];
    for git in changes {
        files.push(compare_file(git, &base, &head));
    }
    // Source bodies are needed only for slice matching, never for public results.
    base.texts.clear();
    head.texts.clear();
    let mut set = ChangeSet {
        revisions,
        range: range.into(),
        files,
        base,
        head,
        impact: None,
    };
    if let Some(depth) = depth {
        set.impact = Some(impact(
            &set,
            depth,
            TraversalLimits {
                roots: graph::MAX_QUERY_ROOTS,
                visited: graph::MAX_VISITED,
                emitted: graph::MAX_EMITTED,
            },
        ));
    }
    Ok(set)
}
fn impact(set: &ChangeSet, depth: u8, mut limits: TraversalLimits) -> Impact {
    let base_seeds = seeds(set, true);
    let head_seeds = seeds(set, false);
    let base_seed_count = base_seeds.len();
    let head_seed_count = head_seeds.len();

    let base = set.base.graph.traverse_roots(
        graph::QueryKind::Neighborhood,
        base_seeds,
        depth,
        limits,
        "exact_revision_descriptors",
    );
    limits.roots = limits.roots.saturating_sub(base.roots.len());
    limits.visited = limits
        .visited
        .saturating_sub(base.results.iter().map(|r| r.entities.len()).sum());
    limits.emitted = limits
        .emitted
        .saturating_sub(base.results.iter().map(|r| r.relations.len()).sum());
    let head = set.head.graph.traverse_roots(
        graph::QueryKind::Neighborhood,
        head_seeds,
        depth,
        limits,
        "exact_revision_descriptors",
    );
    Impact {
        base,
        head,
        base_seed_count,
        head_seed_count,
    }
}

fn snapshot(
    r: &Revisions,
    commit: &str,
    entries: Vec<Entry>,
    wanted: &BTreeSet<PathBuf>,
    max: u64,
    limits: Limits,
) -> Result<Snapshot> {
    let mut map = BTreeMap::new();
    let mut records = vec![];
    for e in entries {
        let language = classify_language(&e.path);
        let role = classify_role(&e.path, &language);
        records.push(FileRecord {
            relative_path: e.path.clone(),
            language,
            role,
            size_bytes: e.size,
            line_count: None,
        });
        map.insert(e.path.clone(), e);
    }
    let scan = RepositoryScan {
        root: r.scope.clone(),
        files: records,
        directories: vec![],
        git: Some(GitMetadata {
            repository_root: r.root.clone(),
            head_commit: Some(commit.into()),
            branch: None,
            is_dirty: None,
        }),
        warnings: vec![],
    };
    let mut texts = BTreeMap::new();
    let mut bytes = 0u64;
    let mut fatal = None;
    let result = index::analyze_loaded(scan, max, |_, file, _| {
        if fatal.is_some() {
            return Err(AnalysisStatus::ReadFailed);
        }
        let e = &map[&file.relative_path];
        if !e.regular() || e.path.to_str().is_none() {
            return Err(AnalysisStatus::ReadFailed);
        }
        bytes = bytes.saturating_add(e.size);
        if bytes > limits.source_bytes {
            fatal = Some(revision::Error::Limit("aggregate source bytes"));
            return Err(AnalysisStatus::ReadFailed);
        }
        match r.blob(e, e.size) {
            Ok(data) => match String::from_utf8(data) {
                Ok(text) => {
                    if wanted.contains(&file.relative_path) {
                        texts.insert(file.relative_path.clone(), text.clone());
                    }
                    Ok(text)
                }
                Err(_) => Err(AnalysisStatus::InvalidUtf8),
            },
            Err(e) => {
                fatal = Some(e);
                Err(AnalysisStatus::ReadFailed)
            }
        }
    });
    if let Some(error) = fatal {
        return Err(error.into());
    }
    let (index, relations) = result?;
    let graph = graph::from_analysis(index, relations)?;
    let mut symbols_by_file: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    let mut relations_by_file: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (i, s) in graph.index.symbols.iter().enumerate() {
        symbols_by_file
            .entry(s.relative_path.clone())
            .or_default()
            .push(i);
    }
    for (i, r) in graph.relations.iter().enumerate() {
        if r.source != EntityId(0) {
            relations_by_file
                .entry(r.evidence.relative_path.clone())
                .or_default()
                .push(i);
        }
    }
    let symbol_names = graph
        .index
        .symbols
        .iter()
        .map(|s| Rc::from(s.qualified_name.as_str()))
        .collect();
    Ok(Snapshot {
        symbol_names,
        graph,
        texts,
        symbols_by_file,
        relations_by_file,
    })
}

fn symbols<'a>(
    s: &'a Snapshot,
    e: Option<&Entry>,
) -> BTreeMap<(&'static str, &'a str), Vec<usize>> {
    let mut map = BTreeMap::new();
    if let Some(e) = e {
        for &i in s.symbols_by_file.get(&e.path).into_iter().flatten() {
            let symbol = &s.graph.index.symbols[i];
            map.entry((symbol.kind.name(), symbol.qualified_name.as_str()))
                .or_insert_with(Vec::new)
                .push(i);
        }
    }
    map
}
pub fn file_analysis<'a>(s: &'a Snapshot, e: Option<&Entry>) -> Option<&'a FileAnalysis> {
    e.and_then(|e| {
        s.graph
            .index
            .files
            .binary_search_by(|f| f.relative_path.cmp(&e.path))
            .ok()
            .map(|i| &s.graph.index.files[i])
    })
}
pub fn relation_analysis<'a>(s: &'a Snapshot, e: &Entry) -> Option<&'a RelationFileAnalysis> {
    s.graph
        .relation_files
        .binary_search_by(|f| f.relative_path.cmp(&e.path))
        .ok()
        .map(|i| &s.graph.relation_files[i])
}
fn usable(s: &Snapshot, e: Option<&Entry>) -> bool {
    e.is_none()
        || file_analysis(s, e)
            .is_some_and(|f| f.status == AnalysisStatus::Analyzed && f.syntax_error_count == 0)
}
fn relation_usable(s: &Snapshot, e: Option<&Entry>) -> bool {
    e.is_none()
        || e.and_then(|e| relation_analysis(s, e)).is_some_and(|f| {
            f.status == RelationStatus::Analyzed && f.unsupported_candidate_count == 0
        })
}

fn compare_file(git: FileChange, base: &Snapshot, head: &Snapshot) -> ChangedFile {
    let comparison_available =
        usable(base, git.before.as_ref()) && usable(head, git.after.as_ref());
    let mut result = ChangedFile {
        git,
        symbols: vec![],
        relations: vec![],
        ambiguous_keys: 0,
        comparison_available,
        relation_comparison_available: false,
    };
    if !comparison_available {
        return result;
    }
    let a = symbols(base, result.git.before.as_ref());
    let b = symbols(head, result.git.after.as_ref());
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).copied().collect();
    for key in keys {
        let old = a.get(&key).map(Vec::as_slice).unwrap_or_default();
        let new = b.get(&key).map(Vec::as_slice).unwrap_or_default();
        if old.len() > 1 || new.len() > 1 {
            result.ambiguous_keys += 1;
            continue;
        }
        let before = old.first().copied();
        let after = new.first().copied();
        let kind = match (before, after) {
            (None, Some(_)) => "added",
            (Some(_), None) => "removed",
            (Some(i), Some(j)) => {
                let x = &base.graph.index.symbols[i];
                let y = &head.graph.index.symbols[j];
                let x = base
                    .texts
                    .get(&x.relative_path)
                    .and_then(|t| t.get(x.range.start.byte..x.range.end.byte));
                let y = head
                    .texts
                    .get(&y.relative_path)
                    .and_then(|t| t.get(y.range.start.byte..y.range.end.byte));
                if x.is_none() || y.is_none() {
                    result.comparison_available = false;
                    result.symbols.clear();
                    return result;
                }
                if x == y {
                    continue;
                }
                "modified"
            }
            _ => continue,
        };
        result.symbols.push(SymbolChange {
            kind,
            before,
            after,
        });
    }
    result.relation_comparison_available = result.ambiguous_keys == 0
        && relation_usable(base, result.git.before.as_ref())
        && relation_usable(head, result.git.after.as_ref());
    if result.relation_comparison_available {
        let mut a = occurrences(base, result.git.before.as_ref());
        let mut b = occurrences(head, result.git.after.as_ref());
        let keys: BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
        for signature in keys {
            let before = a.remove(&signature).unwrap_or(Occurrences {
                count: 0,
                examples: vec![],
            });
            let after = b.remove(&signature).unwrap_or(Occurrences {
                count: 0,
                examples: vec![],
            });
            if before.count != after.count {
                result.relations.push(RelationDelta {
                    signature,
                    before,
                    after,
                });
            }
        }
    }
    result
}
fn endpoint(s: &Snapshot, id: EntityId) -> Endpoint {
    let g = &s.graph;
    match g.entities[id.0 as usize] {
        Entity::Repository => Endpoint {
            state: "resolved",
            kind: "repository",
            name: Rc::from(""),
        },
        Entity::File(_) => Endpoint {
            state: "resolved",
            kind: "file",
            name: Rc::from(""),
        },
        Entity::Symbol(i) => {
            let symbol = &g.index.symbols[i];
            Endpoint {
                state: "resolved",
                kind: symbol.kind.name(),
                name: s.symbol_names[i].clone(),
            }
        }
    }
}
fn occurrences(s: &Snapshot, e: Option<&Entry>) -> BTreeMap<Signature, Occurrences> {
    let mut map = BTreeMap::new();
    if let Some(e) = e {
        for &i in s.relations_by_file.get(&e.path).into_iter().flatten() {
            let r = &s.graph.relations[i];
            let signature = Signature {
                source: endpoint(s, r.source),
                target: match &r.target {
                    Target::Resolved(t) => endpoint(s, *t),
                    Target::Unresolved(name) => Endpoint {
                        state: "unresolved",
                        kind: "text",
                        name: Rc::from(name.as_str()),
                    },
                },
                kind: r.kind,
                resolution: r.resolution,
                metadata: r.metadata.clone(),
                derivation: r.evidence.derivation,
            };
            let o = map.entry(signature).or_insert(Occurrences {
                count: 0,
                examples: vec![],
            });
            o.count += 1;
            if o.examples.len() < 3
                && let Some(range) = r.evidence.range
            {
                o.examples.push(range);
            }
        }
    }
    map
}
fn seeds(set: &ChangeSet, before: bool) -> Vec<EntityId> {
    let g = if before {
        &set.base.graph
    } else {
        &set.head.graph
    };
    let mut symbols = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for f in &set.files {
        if let Some(e) = if before { &f.git.before } else { &f.git.after } {
            paths.insert(e.path.as_path());
        }
        for s in &f.symbols {
            if let Some(i) = if before { s.before } else { s.after } {
                symbols.insert(i);
            }
        }
    }
    g.entities
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Entity::File(f) if paths.contains(g.index.files[*f].relative_path.as_path()) => {
                Some(EntityId(i as u32))
            }
            Entity::Symbol(s) if symbols.contains(s) => Some(EntityId(i as u32)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn git(root: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().into()
    }
    fn fixture() -> (tempfile::TempDir, String) {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        git(r, &["init", "-q"]);
        git(r, &["config", "user.name", "Fixture"]);
        git(r, &["config", "user.email", "fixture@example.test"]);
        std::fs::write(r.join("a.py"), "def a(): return 1\n").unwrap();
        git(r, &["add", "."]);
        git(r, &["commit", "-qm", "base"]);
        let a = git(r, &["rev-parse", "HEAD"]);
        std::fs::write(r.join("a.py"), "def a(): return 2\n").unwrap();
        git(r, &["add", "."]);
        git(r, &["commit", "-qm", "head"]);
        let b = git(r, &["rev-parse", "HEAD"]);
        (d, format!("{a}..{b}"))
    }
    #[test]
    fn production_composition_honors_exact_and_over_revision_budgets() {
        let (d, input) = fixture();
        let r = d.path();
        let rev = Revisions::open(r, &input, revision::LIMITS).unwrap();
        let raw = std::process::Command::new("git")
            .arg("-C")
            .arg(r)
            .args([
                "diff-tree",
                "-r",
                "--raw",
                "-z",
                "--no-commit-id",
                "--no-abbrev",
                "--find-renames=50%",
                &rev.base,
                &rev.head,
            ])
            .output()
            .unwrap()
            .stdout
            .len();
        let tree = std::process::Command::new("git")
            .arg("-C")
            .arg(r)
            .args(["ls-tree", "-r", "-z", "-l", &rev.head])
            .output()
            .unwrap()
            .stdout
            .len();
        let exact = Limits {
            revision: input.len(),
            changes: 1,
            change_paths: 8,
            diff_bytes: raw,
            entries: 1,
            tree_paths: 4,
            tree_bytes: tree,
            source_bytes: 18,
        };
        for depth in [None, Some(1)] {
            let set = analyze_with_limits(r, &input, 100, depth, exact).unwrap();
            assert_eq!(set.files[0].symbols[0].kind, "modified");
            for l in [
                Limits {
                    revision: exact.revision - 1,
                    ..exact
                },
                Limits {
                    changes: 0,
                    ..exact
                },
                Limits {
                    change_paths: 7,
                    ..exact
                },
                Limits {
                    diff_bytes: raw - 1,
                    ..exact
                },
                Limits {
                    source_bytes: 17,
                    ..exact
                },
            ] {
                assert!(analyze_with_limits(r, &input, 100, depth, l).is_err());
            }
        }
        for l in [
            Limits {
                entries: 0,
                ..exact
            },
            Limits {
                tree_paths: 3,
                ..exact
            },
            Limits {
                tree_bytes: tree - 1,
                ..exact
            },
        ] {
            assert!(analyze_with_limits(r, &input, 100, Some(1), l).is_err());
        }
    }
    #[test]
    fn explicit_roots_share_traversal_budget_with_predecessor_and_truncation() {
        let (d, input) = fixture();
        let set = analyze(d.path(), &input, 100, Some(2)).unwrap();
        let g = &set.base.graph;
        let roots = seeds(&set, true);
        assert_eq!(roots.len(), 2);
        let full = g.traverse_roots(
            graph::QueryKind::Neighborhood,
            roots.clone(),
            2,
            TraversalLimits {
                roots: 2,
                visited: 4,
                emitted: 2,
            },
            "test",
        );
        assert!(!full.truncated);
        assert_eq!(
            full.results.iter().map(|r| r.entities.len()).sum::<usize>(),
            4
        );
        let limited = g.traverse_roots(
            graph::QueryKind::Neighborhood,
            roots.clone(),
            2,
            TraversalLimits {
                roots: 2,
                visited: 3,
                emitted: 2,
            },
            "test",
        );
        assert!(limited.truncated);
        let omitted = g.traverse_roots(
            graph::QueryKind::Neighborhood,
            roots,
            2,
            TraversalLimits {
                roots: 1,
                visited: 4,
                emitted: 2,
            },
            "test",
        );
        assert!(omitted.roots_truncated);
        assert_eq!(full.results[0].predecessors.len(), 1);
    }
    #[test]
    fn impact_budgets_are_shared_across_both_revision_graphs() {
        let (d, input) = fixture();
        let set = analyze(d.path(), &input, 100, None).unwrap();
        let full = TraversalLimits {
            roots: 4,
            visited: 8,
            emitted: 4,
        };
        let result = impact(&set, 2, full);
        assert!(!result.base.truncated && !result.head.truncated);
        assert_eq!(result.base_seed_count + result.head_seed_count, 4);
        for l in [
            TraversalLimits { roots: 3, ..full },
            TraversalLimits { visited: 7, ..full },
            TraversalLimits { emitted: 3, ..full },
        ] {
            let r = impact(&set, 2, l);
            assert!(r.head.truncated);
            assert!(r.base.roots.len() + r.head.roots.len() <= l.roots);
            assert!(
                r.base
                    .results
                    .iter()
                    .chain(&r.head.results)
                    .map(|r| r.entities.len())
                    .sum::<usize>()
                    <= l.visited
            );
            assert!(
                r.base
                    .results
                    .iter()
                    .chain(&r.head.results)
                    .map(|r| r.relations.len())
                    .sum::<usize>()
                    <= l.emitted
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn non_utf8_blob_paths_remain_file_changes_with_explicit_omission() {
        let (d, input) = fixture();
        let r = Revisions::open(d.path(), &input, revision::LIMITS).unwrap();
        use std::os::unix::ffi::OsStringExt;
        let e = Entry {
            path: PathBuf::from(std::ffi::OsString::from_vec(b"a\xff.py".to_vec())),
            mode: "100644".into(),
            oid: "1".repeat(40),
            size: 17,
        };
        // Synthetic native path avoids filesystems that reject invalid UTF-8 names.
        let old = snapshot(
            &r,
            &r.base,
            vec![e.clone()],
            &BTreeSet::new(),
            100,
            revision::LIMITS,
        )
        .unwrap();
        let new = snapshot(&r, &r.head, vec![], &BTreeSet::new(), 100, revision::LIMITS).unwrap();
        let f = compare_file(
            FileChange {
                status: "deleted".into(),
                similarity: None,
                before: Some(e),
                after: None,
            },
            &old,
            &new,
        );
        assert!(!f.comparison_available);
        assert!(f.symbols.is_empty());
        let set = ChangeSet {
            revisions: r,
            range: input,
            files: vec![f],
            base: old,
            head: new,
            impact: None,
        };
        let v: serde_json::Value =
            serde_json::from_str(&crate::output::change::json("diff", &set).unwrap()).unwrap();
        assert_eq!(v["data"]["total_file_changes"], 1);
        assert_eq!(v["data"]["base_coverage"]["non_utf8_paths"], 1);
        assert_eq!(
            v["data"]["files"][0]["before"]["analysis_status"],
            "non_utf8_path"
        );
    }
}
