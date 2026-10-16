//! Deterministic file ranking over the current graph; content stays behind source::load.
use crate::{
    change,
    classify::FileRole,
    git,
    graph::{self, EntityId, QueryKind, RepositoryGraph, TraversalLimits},
    model::AnalysisStatus,
    paths::utf8_relative,
    query, source,
};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};

pub const RANKER_VERSION: &str = "file-ranker-v1";
pub const MAX_TASK_BYTES: usize = 4096;
pub const MAX_TASK_TERMS: usize = 64;
pub const MAX_BUDGET: u64 = 16 * 1024 * 1024;
pub const MAX_SELECTED: usize = 200;
pub const MAX_SKIPS: usize = 50;
const MAX_SYMBOLS: usize = 8;
const FALLBACK_FILES: usize = 20;

pub struct Task {
    pub original: String,
    pub phrase: String,
    pub terms: Vec<String>,
    pub terms_truncated: bool,
}
impl Task {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        if text.is_empty() || text.len() > MAX_TASK_BYTES {
            return Err("task must contain 1 through 4096 UTF-8 bytes");
        }
        let phrase = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let mut terms = BTreeSet::new();
        for token in phrase.split(|c: char| !c.is_alphanumeric() && !"_-./:".contains(c)) {
            for term in std::iter::once(token).chain(token.split(['_', '-', '.', '/', ':'])) {
                if !term.is_empty() && (term.chars().count() > 1 || phrase.chars().count() == 1) {
                    terms.insert(term.to_owned());
                }
            }
        }
        let terms_truncated = terms.len() > MAX_TASK_TERMS;
        Ok(Self {
            original: text.into(),
            phrase,
            terms: terms.into_iter().take(MAX_TASK_TERMS).collect(),
            terms_truncated,
        })
    }
}
pub fn validate_budget(budget: u64) -> Result<(), &'static str> {
    if (1..=MAX_BUDGET).contains(&budget) {
        Ok(())
    } else {
        Err("budget must be between 1 and 16777216 UTF-8 content bytes")
    }
}
#[derive(Default, Clone, Debug)]
pub struct Score {
    pub lexical_path: u16,
    pub lexical_symbol: u16,
    pub graph: u16,
    pub role: u16,
    pub change: u16,
    pub centrality: u16,
}
impl Score {
    pub fn total(&self) -> u16 {
        self.lexical_path
            + self.lexical_symbol
            + self.graph
            + self.role
            + self.change
            + self.centrality
    }
}
pub struct Candidate {
    pub file: usize,
    pub score: Score,
    pub reasons: BTreeSet<&'static str>,
    pub symbols: Vec<usize>,
    pub symbols_omitted: usize,
    pub graph_distance: Option<u8>,
    pub change_distance: Option<u8>,
}
impl Candidate {
    fn new(file: usize) -> Self {
        Self {
            file,
            score: Score::default(),
            reasons: BTreeSet::new(),
            symbols: vec![],
            symbols_omitted: 0,
            graph_distance: None,
            change_distance: None,
        }
    }
}
pub struct Ranking {
    pub candidates: Vec<Candidate>,
    pub eligible: usize,
    pub fallback: bool,
    pub fallback_omitted: usize,
    pub roots: usize,
    pub roots_omitted: usize,
    pub graph_truncated: bool,
}
pub struct Selected {
    pub candidate: Candidate,
    pub rank: usize,
    pub content: String,
}
pub struct Skipped {
    pub file: usize,
    pub rank: usize,
    pub reason: &'static str,
}
pub struct Bundle {
    pub ranking: Ranking,
    pub selected: Vec<Selected>,
    pub skipped: Vec<Skipped>,
    pub budget: u64,
    pub used: u64,
    pub budget_skips: usize,
    pub load_failures: usize,
    pub skips_omitted: usize,
    pub selection_omitted: usize,
}

pub fn eligible(role: FileRole) -> bool {
    !matches!(role, FileRole::Generated | FileRole::Vendor)
}

pub fn path_score(path: &str, task: &Task) -> (u16, BTreeSet<&'static str>) {
    let text = path.to_lowercase();
    let stem = text
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.').map(|(stem, _)| stem));
    let components: Vec<_> = text
        .split('/')
        .chain(stem)
        .chain(text.split(['/', '.', '_', '-', ':']))
        .filter(|s| !s.is_empty())
        .collect();
    let mut score = 0u16;
    let mut reasons = BTreeSet::new();
    for term in &task.terms {
        let (value, reason) = if components.contains(&term.as_str()) || text == *term {
            (30, "path_component_exact")
        } else if text.starts_with(term) || components.iter().any(|c| c.starts_with(term)) {
            (18, "path_prefix")
        } else if text.contains(term) {
            (10, "path_substring")
        } else {
            continue;
        };
        score = score.saturating_add(value).min(60);
        reasons.insert(reason);
    }
    if !task.phrase.is_empty() && text.contains(&task.phrase) {
        score = (score + 12).min(60);
        reasons.insert("path_phrase");
    }
    (score, reasons)
}
fn symbol_score(name: &str, qualified: &str, task: &Task) -> (u16, &'static str) {
    let name = name.to_lowercase();
    let qualified = qualified.to_lowercase();
    task.terms
        .iter()
        .map(|term| {
            if qualified == *term {
                (45, "symbol_qualified_exact")
            } else if name == *term {
                (40, "symbol_local_exact")
            } else if qualified.starts_with(term) || name.starts_with(term) {
                (26, "symbol_prefix")
            } else if qualified.contains(term) || name.contains(term) {
                (15, "symbol_substring")
            } else {
                (0, "")
            }
        })
        .max_by_key(|v| v.0)
        .unwrap_or((0, ""))
}
fn sort_candidates(g: &RepositoryGraph, candidates: &mut [Candidate]) {
    candidates.sort_by(|a, b| {
        let key = |c: &Candidate| {
            Reverse((
                c.score.total(),
                c.score.lexical_symbol,
                c.score.lexical_path,
                c.score.change,
                c.score.graph,
                c.score.centrality,
            ))
        };
        key(a).cmp(&key(b)).then_with(|| {
            g.index.files[a.file]
                .relative_path
                .cmp(&g.index.files[b.file].relative_path)
        })
    });
}
// One traversal call shares all operation-wide ceilings across lexical and change roots.
// Root origin is retained so change distances do not accidentally become task evidence.
pub fn rank(g: &RepositoryGraph, task: &Task, changes: Option<&ChangeSignal>) -> Ranking {
    let owners = query::file_owners(g);
    let metrics = query::structural_metrics(g, &owners);
    let mut candidates: Vec<_> = (0..g.index.files.len()).map(Candidate::new).collect();
    let mut seeds = BTreeMap::<EntityId, (u16, bool)>::new();
    let mut matches = vec![vec![]; candidates.len()];
    for c in &mut candidates {
        if !eligible(g.index.files[c.file].role) {
            continue;
        }
        if let Some(path) = utf8_relative(&g.index.files[c.file].relative_path) {
            (c.score.lexical_path, c.reasons) = path_score(&path, task);
            if c.score.lexical_path > 0 {
                seeds.insert(EntityId((c.file + 1) as u32), (c.score.lexical_path, false));
            }
        }
        let m = &metrics[1 + c.file];
        c.score.centrality =
            (m.structural_activity.min(4) + m.incoming.min(3) + m.outgoing.min(3)) as u16;
    }
    for (i, s) in g.index.symbols.iter().enumerate() {
        let id = EntityId((1 + g.index.files.len() + i) as u32);
        let Some(file) = owners[id.0 as usize] else {
            continue;
        };
        if !eligible(g.index.files[file].role) {
            continue;
        }
        let (value, reason) = symbol_score(&s.name, &s.qualified_name, task);
        if value > 0 {
            matches[file].push((Reverse(value), i, reason));
            seeds.insert(id, (100 + value, false));
        }
    }
    for (c, matching) in candidates.iter_mut().zip(&mut matches) {
        matching.sort();
        c.symbols_omitted = matching.len().saturating_sub(MAX_SYMBOLS);
        for &(Reverse(score), index, reason) in matching.iter().take(MAX_SYMBOLS) {
            c.score.lexical_symbol = (c.score.lexical_symbol + score).min(90);
            c.symbols.push(index);
            c.reasons.insert(reason);
        }
    }
    if let Some(change) = changes {
        for (&file, &value) in &change.files {
            if !eligible(g.index.files[file].role) {
                continue;
            }
            candidates[file].score.change = value;
            candidates[file].reasons.insert(if value == 40 {
                "changed_symbol_owner"
            } else {
                "changed_file"
            });
        }
        for &id in &change.seeds {
            if owners[id.0 as usize].is_some_and(|f| eligible(g.index.files[f].role)) {
                let priority = if g.symbol(id).is_some() { 140 } else { 32 };
                seeds
                    .entry(id)
                    .and_modify(|s| {
                        s.0 = s.0.max(priority);
                        s.1 = true;
                    })
                    .or_insert((priority, true));
            }
        }
    }
    let fallback = seeds.is_empty();
    let mut ordered: Vec<_> = seeds
        .iter()
        .map(|(&id, &(strength, _))| (Reverse(strength), id))
        .collect();
    ordered.sort();
    let roots_count = ordered.len();
    let result = g.traverse_roots(
        QueryKind::Neighborhood,
        ordered.into_iter().map(|(_, id)| id).collect(),
        2,
        TraversalLimits {
            roots: graph::MAX_QUERY_ROOTS,
            visited: graph::MAX_VISITED,
            emitted: graph::MAX_EMITTED,
        },
        "task_and_current_change_descriptors",
    );
    for root in &result.results {
        let from_change = seeds[&root.root].1;
        for (&id, &distance) in &root.distances {
            let Some(file) = owners[id.0 as usize] else {
                continue;
            };
            let c = &mut candidates[file];
            c.graph_distance = Some(c.graph_distance.map_or(distance, |d| d.min(distance)));
            if from_change {
                c.change_distance = Some(c.change_distance.map_or(distance, |d| d.min(distance)));
            }
        }
    }
    let eligible_count = candidates
        .iter()
        .filter(|c| eligible(g.index.files[c.file].role))
        .count();
    candidates.retain_mut(|c| {
        if !eligible(g.index.files[c.file].role) {
            return false;
        }
        if let Some(d) = c.graph_distance {
            c.score.graph = [30, 20, 10][d as usize];
            c.reasons
                .insert(["graph_distance_0", "graph_distance_1", "graph_distance_2"][d as usize]);
        }
        if let Some(d) = c.change_distance {
            let boost = [32, 20, 10][d as usize];
            if boost > c.score.change {
                c.score.change = boost;
                c.reasons.insert(
                    [
                        "change_distance_0",
                        "change_distance_1",
                        "change_distance_2",
                    ][d as usize],
                );
            }
        }
        let direct = c.score.lexical_path > 0 || c.score.lexical_symbol > 0;
        match g.index.files[c.file].role {
            FileRole::Test if c.graph_distance.is_some() => {
                c.score.role = 12;
                c.reasons.insert("related_test_candidate");
            }
            FileRole::Test if direct => {
                c.score.role = 8;
                c.reasons.insert("direct_test_match");
            }
            FileRole::Documentation | FileRole::Configuration | FileRole::Schema if direct => {
                c.score.role = 6;
                c.reasons.insert("direct_task_role_match");
            }
            _ => (),
        }
        if c.score.centrality > 0 {
            c.reasons.insert("centrality_activity");
        }
        if fallback {
            c.reasons.insert("centrality_fallback");
        }
        fallback || direct || c.score.graph > 0 || c.score.change > 0
    });
    sort_candidates(g, &mut candidates);
    let fallback_omitted = if fallback {
        candidates.len().saturating_sub(FALLBACK_FILES)
    } else {
        0
    };
    if fallback {
        candidates.truncate(FALLBACK_FILES);
    }
    Ranking {
        candidates,
        eligible: eligible_count,
        fallback,
        fallback_omitted,
        roots: result.roots.len(),
        roots_omitted: roots_count.saturating_sub(result.roots.len()),
        graph_truncated: result.truncated,
    }
}

pub fn select(g: &RepositoryGraph, ranking: Ranking, budget: u64) -> Bundle {
    select_with_limits(g, ranking, budget, MAX_SELECTED, MAX_SKIPS, MAX_BUDGET)
}
fn select_with_limits(
    g: &RepositoryGraph,
    mut ranking: Ranking,
    budget: u64,
    max_selected: usize,
    max_skips: usize,
    max_file: u64,
) -> Bundle {
    let candidates = std::mem::take(&mut ranking.candidates);
    let count = candidates.len();
    let mut bundle = Bundle {
        ranking,
        selected: vec![],
        skipped: vec![],
        budget,
        used: 0,
        budget_skips: 0,
        load_failures: 0,
        skips_omitted: 0,
        selection_omitted: 0,
    };
    for (i, candidate) in candidates.into_iter().enumerate() {
        if bundle.selected.len() >= max_selected {
            bundle.selection_omitted = count - i;
            break;
        }
        let remaining = budget - bundle.used;
        // The loader reads at most remaining+1 bytes; rejected large files never
        // materialize their full contents. This same packer is used by baselines.
        let result = source::load(
            &g.index.root,
            &g.index.files[candidate.file].relative_path,
            remaining.min(max_file),
        );
        let reason = match result {
            Ok(content) => {
                bundle.used += content.len() as u64;
                bundle.selected.push(Selected {
                    candidate,
                    rank: i + 1,
                    content,
                });
                continue;
            }
            Err(AnalysisStatus::TooLarge) if remaining < max_file => {
                bundle.budget_skips += 1;
                "budget_exceeded"
            }
            Err(AnalysisStatus::TooLarge) => {
                bundle.load_failures += 1;
                "too_large"
            }
            Err(status) => {
                bundle.load_failures += 1;
                status.name()
            }
        };
        if bundle.skipped.len() < max_skips {
            bundle.skipped.push(Skipped {
                file: candidate.file,
                rank: i + 1,
                reason,
            });
        } else {
            bundle.skips_omitted += 1;
        }
    }
    bundle
}

pub struct ChangeSignal {
    pub range: String,
    pub base: String,
    pub head: String,
    pub files_observed: usize,
    pub symbols_observed: usize,
    pub symbols_mapped: usize,
    pub symbols_unmapped: usize,
    pub ambiguous_keys: usize,
    pub comparisons_unavailable: usize,
    pub deleted_only: usize,
    pub files: BTreeMap<usize, u16>,
    pub seeds: BTreeSet<EntityId>,
}
type ContextResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn compatible_change(
    g: &RepositoryGraph,
    range: &str,
    max_parse: u64,
) -> ContextResult<ChangeSignal> {
    let meta = g
        .index
        .git
        .as_ref()
        .ok_or("--change requires Git metadata")?;
    let head = meta
        .head_commit
        .as_ref()
        .ok_or("--change requires a current HEAD")?;
    if meta.is_dirty != Some(false) {
        return Err("--change requires a known clean worktree".into());
    }
    git::revision::validate_context_index(&g.index.root)?;
    let set = change::analyze(&g.index.root, range, max_parse, None)?;
    if &set.revisions.head != head {
        return Err("--change range head must match current HEAD".into());
    }
    Ok(map_change(g, &set))
}
pub fn recheck_snapshot(g: &RepositoryGraph, head: &str) -> ContextResult<()> {
    git::revision::validate_context_index(&g.index.root)?;
    let (meta, _) = git::probe(&g.index.root);
    if !meta.is_some_and(|m| m.is_dirty == Some(false) && m.head_commit.as_deref() == Some(head)) {
        return Err("--change snapshot changed or became unknown during context assembly".into());
    }
    Ok(())
}
fn map_change(g: &RepositoryGraph, set: &change::ChangeSet) -> ChangeSignal {
    let paths: BTreeMap<_, _> = g
        .index
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.relative_path.as_path(), i))
        .collect();
    let mut keys = BTreeMap::<_, Vec<usize>>::new();
    for (i, s) in g.index.symbols.iter().enumerate() {
        keys.entry((
            s.relative_path.as_path(),
            s.kind.name(),
            s.qualified_name.as_str(),
        ))
        .or_default()
        .push(i);
    }
    let mut signal = ChangeSignal {
        range: set.range.clone(),
        base: set.revisions.base.clone(),
        head: set.revisions.head.clone(),
        files_observed: set.files.len(),
        symbols_observed: 0,
        symbols_mapped: 0,
        symbols_unmapped: 0,
        ambiguous_keys: 0,
        comparisons_unavailable: 0,
        deleted_only: 0,
        files: BTreeMap::new(),
        seeds: BTreeSet::new(),
    };
    for f in &set.files {
        if let Some(after) = &f.git.after {
            if let Some(&i) = paths.get(after.path.as_path()) {
                signal.files.insert(i, 32);
                signal.seeds.insert(EntityId((1 + i) as u32));
            }
        } else {
            signal.deleted_only += 1;
        }
        for changed in &f.symbols {
            // Phase 4 emits only changed descriptors; head indices are used solely
            // to read structural keys, never as current numeric identity.
            signal.symbols_observed += 1;
            let Some(i) = changed.after else {
                signal.deleted_only += 1;
                continue;
            };
            let s = &set.head.graph.index.symbols[i];
            match keys.get(&(
                s.relative_path.as_path(),
                s.kind.name(),
                s.qualified_name.as_str(),
            )) {
                Some(indices) if indices.len() == 1 => {
                    let current = indices[0];
                    let file = paths[s.relative_path.as_path()];
                    signal.files.insert(file, 40);
                    signal
                        .seeds
                        .insert(EntityId((1 + g.index.files.len() + current) as u32));
                    signal.symbols_mapped += 1;
                }
                _ => signal.symbols_unmapped += 1,
            }
        }
        signal.ambiguous_keys += f.ambiguous_keys;
        signal.comparisons_unavailable += usize::from(!f.comparison_available);
    }
    signal
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    use tempfile::TempDir;
    fn write(root: &Path, path: &str, text: impl AsRef<[u8]>) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn build(root: &Path) -> RepositoryGraph {
        graph::build(&crate::model::IndexOptions {
            root: root.into(),
            max_parse_file_bytes: 4096,
            probe_git: false,
        })
        .unwrap()
    }
    #[test]
    fn normalization_bounds_and_lexical_tiers() {
        assert!(Task::parse("").is_err());
        assert!(Task::parse(&"é".repeat(2049)).is_err());
        assert!(Task::parse(&"é".repeat(2048)).is_ok());
        let task = Task::parse("Src/Model_Store.rs src MODEL").unwrap();
        assert_eq!(
            task.terms,
            vec!["model", "rs", "src", "src/model_store.rs", "store"]
        );
        let terms = (0..65)
            .map(|n| format!("term{n:02}"))
            .collect::<Vec<_>>()
            .join(" ");
        let task = Task::parse(&terms).unwrap();
        assert_eq!(task.terms.len(), 64);
        assert!(task.terms_truncated);
        assert!(Task::parse("?!").unwrap().terms.is_empty());
        assert_eq!(Task::parse("x").unwrap().terms, vec!["x"]);
        let task = Task::parse("store").unwrap();
        assert_eq!(path_score("src/store.rs", &task).0, 42);
        assert_eq!(path_score("src/stores.rs", &task).0, 30); // prefix + phrase
        assert_eq!(path_score("src/restore.rs", &task).0, 22); // substring + phrase
        let (_, reasons) = path_score("src/model_store.rs", &Task::parse("model_store").unwrap());
        assert!(reasons.contains("path_component_exact"));
        assert!(!reasons.contains("path_substring"));
        assert_eq!(symbol_score("store", "Box::store", &task).0, 40);
        assert_eq!(symbol_score("store", "store", &task).0, 45);
        assert_eq!(symbol_score("stores", "stores", &task).0, 26);
        assert_eq!(symbol_score("restore", "restore", &task).0, 15);
        for n in [0, MAX_BUDGET + 1] {
            assert!(validate_budget(n).is_err());
        }
        assert!(validate_budget(MAX_BUDGET).is_ok());
    }
    #[test]
    fn caps_order_graph_test_role_and_fallback() {
        let d = TempDir::new().unwrap();
        write(
            d.path(),
            "src/z.rs",
            "fn needle() {}\nfn needle_two() { needle(); }\nfn needle_three() {}\n",
        );
        write(
            d.path(),
            "src/a.rs",
            "fn needle() {}\nfn needle_two() { needle(); }\nfn needle_three() {}\n",
        );
        write(
            d.path(),
            "tests/spec.py",
            "def needle(): pass\ndef test_it(): needle()\n",
        );
        write(d.path(), "docs/needle.md", "needle docs");
        write(
            d.path(),
            "hub.py",
            (0..80)
                .map(|i| format!("def f{i}(): f0()\n"))
                .collect::<String>(),
        );
        let g = build(d.path());
        let t = Task::parse("needle").unwrap();
        let r = rank(&g, &t, None);
        let c = |path| {
            r.candidates
                .iter()
                .find(|c| g.index.files[c.file].relative_path == Path::new(path))
                .unwrap()
        };
        assert_eq!(c("src/a.rs").score.lexical_symbol, 90);
        assert_eq!(c("src/a.rs").graph_distance, Some(0));
        assert_eq!(c("tests/spec.py").score.role, 12);
        assert!(
            c("tests/spec.py")
                .reasons
                .contains("related_test_candidate")
        );
        assert_eq!(c("docs/needle.md").score.role, 6);
        assert!(r.candidates.iter().all(|c| c.score.centrality <= 10));
        assert!(
            r.candidates
                .iter()
                .all(|c| g.index.files[c.file].relative_path != Path::new("hub.py"))
        );
        let paths: Vec<_> = r
            .candidates
            .iter()
            .map(|c| g.index.files[c.file].relative_path.as_path())
            .collect();
        assert!(
            paths
                .iter()
                .position(|p| **p == *Path::new("src/a.rs"))
                .unwrap()
                < paths
                    .iter()
                    .position(|p| **p == *Path::new("src/z.rs"))
                    .unwrap()
        );
        let mut limited = Task::parse("needle").unwrap();
        limited.terms = vec!["needle_two".into()];
        let r = rank(&g, &limited, None);
        // Matching symbol roots contribute distance zero to their owning files;
        // unrelated files remain excluded despite the repository containment hub.
        assert!(
            r.candidates
                .iter()
                .filter(|c| g.index.files[c.file]
                    .relative_path
                    .extension()
                    .is_some_and(|e| e == "rs"))
                .all(|c| c.graph_distance == Some(0))
        );
        let r = rank(&g, &Task::parse("absent").unwrap(), None);
        assert!(r.fallback);
        assert_eq!(
            g.index.files[r.candidates[0].file].relative_path,
            Path::new("hub.py")
        );
        assert!(
            r.candidates
                .iter()
                .all(|c| c.reasons.contains("centrality_fallback"))
        );
    }
    #[test]
    fn byte_packing_failures_and_injected_count_limits() {
        let d = TempDir::new().unwrap();
        write(d.path(), "a.txt", "too large");
        write(d.path(), "b.txt", "é");
        write(d.path(), "c.txt", [255]);
        write(d.path(), "d.txt", "x");
        write(d.path(), "e.txt", "x");
        let g = build(d.path());
        fs::remove_file(d.path().join("d.txt")).unwrap();
        let ranking = || rank(&g, &Task::parse("txt").unwrap(), None);
        let b = select_with_limits(&g, ranking(), 2, 200, 50, MAX_BUDGET);
        assert_eq!(b.used, 2);
        assert_eq!(b.selected.len(), 1);
        assert_eq!(b.selected[0].content, "é");
        assert_eq!(b.budget_skips, 3);
        assert_eq!(b.load_failures, 1);
        let all = select(&g, ranking(), 100);
        assert!(all.skipped.iter().any(|s| s.reason == "invalid_utf8"));
        assert!(b.skipped.iter().any(|s| s.reason == "read_failed"));
        let b = select_with_limits(&g, ranking(), 100, 1, 0, 2);
        assert_eq!(b.selected.len(), 1);
        assert_eq!(b.skips_omitted, 1);
        assert_eq!(b.selection_omitted, 3);
        let b = select_with_limits(&g, ranking(), 100, 200, 50, 2);
        assert!(b.skipped.iter().any(|s| s.reason == "too_large"));
    }

    #[test]
    fn explanation_root_and_fallback_bounds() {
        let d = TempDir::new().unwrap();
        write(
            d.path(),
            "symbols.py",
            (0..150)
                .map(|i| format!("def hit_{i}(): pass\n"))
                .collect::<String>(),
        );
        let g = build(d.path());
        let r = rank(&g, &Task::parse("hit").unwrap(), None);
        assert_eq!(r.roots, 128);
        assert_eq!(r.roots_omitted, 22);
        assert!(r.graph_truncated);
        assert_eq!(r.candidates[0].symbols.len(), 8);
        assert_eq!(r.candidates[0].symbols_omitted, 142);
        for n in 0..21 {
            write(d.path(), &format!("file{n}.txt"), "x");
        }
        let g = build(d.path());
        let r = rank(&g, &Task::parse("unmatched").unwrap(), None);
        assert!(r.fallback);
        assert_eq!(r.candidates.len(), 20);
        assert_eq!(r.fallback_omitted, 2);
    }

    /// Fixture-authored expected files: symbol implementation and its local test
    /// harness, or direct docs/config. Every file is 128 raw bytes; all three
    /// strategies call the exact production packer with the same role eligibility.
    #[test]
    fn expected_file_recall_benchmark() {
        let d = TempDir::new().unwrap();
        let fixtures = [
            ("a_notes/dispatch.md", "dispatch migration notes\n"),
            ("a_notes/hydrate.md", "hydrate migration notes\n"),
            ("a_notes/verify.md", "verify migration notes\n"),
            ("a_notes/encode.md", "encode migration notes\n"),
            (
                "src/router.rs",
                "fn dispatch() {}\nfn run() { dispatch(); }\n",
            ),
            (
                "tests/router.rs",
                "fn dispatch() {}\n#[test]\nfn test_route() { dispatch(); }\n",
            ),
            (
                "src/storage.py",
                "def hydrate(): pass\ndef load(): hydrate()\n",
            ),
            (
                "tests/test_storage.py",
                "def hydrate(): pass\ndef test_load(): hydrate()\n",
            ),
            ("src/check.rs", "fn verify() {}\nfn check() { verify(); }\n"),
            (
                "tests/check.rs",
                "fn verify() {}\n#[test]\nfn test_check() { verify(); }\n",
            ),
            (
                "src/codec.py",
                "def encode(): pass\ndef write(): encode()\n",
            ),
            (
                "tests/test_codec.py",
                "def encode(): pass\ndef test_write(): encode()\n",
            ),
            ("docs/setup.md", "setup instructions\n"),
            ("config/setup.toml", "setup = true\n"),
            ("docs/protocol.md", "protocol documentation\n"),
            ("schema/protocol.json", "{}\n"),
        ];
        for (path, text) in fixtures {
            write(
                d.path(),
                path,
                format!("{text}{}", " ".repeat(128 - text.len())),
            );
        }
        let g = build(d.path());
        let scenarios = [
            ("dispatch", 256, ["src/router.rs", "tests/router.rs"]),
            ("hydrate", 256, ["src/storage.py", "tests/test_storage.py"]),
            ("verify", 256, ["src/check.rs", "tests/check.rs"]),
            ("encode", 256, ["src/codec.py", "tests/test_codec.py"]),
            ("setup", 256, ["docs/setup.md", "config/setup.toml"]),
            (
                "protocol",
                256,
                ["docs/protocol.md", "schema/protocol.json"],
            ),
        ];
        let mut total_hits = [0usize; 3];
        for (text, budget, expected) in scenarios {
            let task = Task::parse(text).unwrap();
            let mut scenario_hits = [0; 3];
            for strategy in 0..3 {
                let mut ranking = rank(&g, &task, None);
                if strategy > 0 {
                    ranking.candidates = g
                        .index
                        .files
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| eligible(f.role))
                        .map(|(i, _)| Candidate::new(i))
                        .collect();
                    if strategy == 2 {
                        ranking.candidates.sort_by_key(|c| {
                            (
                                Reverse(
                                    path_score(
                                        &utf8_relative(&g.index.files[c.file].relative_path)
                                            .unwrap(),
                                        &task,
                                    )
                                    .0,
                                ),
                                g.index.files[c.file].relative_path.clone(),
                            )
                        });
                    }
                }
                let b = select(&g, ranking, budget);
                let hits = b
                    .selected
                    .iter()
                    .filter(|s| {
                        expected
                            .iter()
                            .any(|p| g.index.files[s.candidate.file].relative_path == Path::new(p))
                    })
                    .count();
                assert_eq!(b.used, 256);
                scenario_hits[strategy] = hits;
                total_hits[strategy] += hits;
            }
            eprintln!(
                "benchmark task={text:?} budget={budget} expected={expected:?} hits(rank,path,lex)={scenario_hits:?}/2 used=[256,256,256]"
            );
        }
        eprintln!(
            "benchmark total_hits(rank,path,lex)={total_hits:?}/12 macro={:?}",
            total_hits.map(|h| h as f64 / 12.0)
        );
        assert!(total_hits[0] > total_hits[1] && total_hits[0] > total_hits[2]);
    }
}
