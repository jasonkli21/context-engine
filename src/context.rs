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
    path::Path,
};

/// The public scoring, explanation, and ordering constants for file-ranker-v1.
/// Update the version and its contract test together when these semantics change.
pub mod ranker_v1 {
    pub const VERSION: &str = "file-ranker-v1";

    pub const REASON_PATH_COMPONENT_EXACT: &str = "path_component_exact";
    pub const REASON_PATH_PREFIX: &str = "path_prefix";
    pub const REASON_PATH_SUBSTRING: &str = "path_substring";
    pub const REASON_PATH_PHRASE: &str = "path_phrase";
    pub const REASON_SYMBOL_QUALIFIED_EXACT: &str = "symbol_qualified_exact";
    pub const REASON_SYMBOL_LOCAL_EXACT: &str = "symbol_local_exact";
    pub const REASON_SYMBOL_PREFIX: &str = "symbol_prefix";
    pub const REASON_SYMBOL_SUBSTRING: &str = "symbol_substring";
    pub const REASON_GRAPH_DISTANCE_1: &str = "graph_distance_1";
    pub const REASON_GRAPH_DISTANCE_2: &str = "graph_distance_2";
    pub const REASON_RELATED_TEST_CANDIDATE: &str = "related_test_candidate";
    pub const REASON_DIRECT_TEST_MATCH: &str = "direct_test_match";
    pub const REASON_DIRECT_TASK_ROLE_MATCH: &str = "direct_task_role_match";
    pub const REASON_CHANGED_SYMBOL_OWNER: &str = "changed_symbol_owner";
    pub const REASON_CHANGED_FILE: &str = "changed_file";
    pub const REASON_CHANGE_DISTANCE_1: &str = "change_distance_1";
    pub const REASON_CHANGE_DISTANCE_2: &str = "change_distance_2";
    pub const REASON_CENTRALITY_ACTIVITY: &str = "centrality_activity";
    pub const REASON_CENTRALITY_FALLBACK: &str = "centrality_fallback";

    pub const PATH_COMPONENT_EXACT: u16 = 30;
    pub const PATH_PREFIX: u16 = 18;
    pub const PATH_SUBSTRING: u16 = 10;
    pub const PATH_PHRASE: u16 = 12;
    pub const PATH_CAP: u16 = 60;

    pub const SYMBOL_QUALIFIED_EXACT: u16 = 45;
    pub const SYMBOL_LOCAL_EXACT: u16 = 40;
    pub const SYMBOL_PREFIX: u16 = 26;
    pub const SYMBOL_SUBSTRING: u16 = 15;
    pub const SYMBOL_CAP: u16 = 90;
    pub const MAX_EXPLAINED_SYMBOLS: usize = 8;

    // Distance zero is seed ownership, not graph evidence for another file.
    pub const GRAPH_DISTANCE: [u16; 3] = [0, 20, 10];
    pub const GRAPH_REASONS: [&str; 3] = ["", REASON_GRAPH_DISTANCE_1, REASON_GRAPH_DISTANCE_2];

    pub const RELATED_TEST_ROLE: u16 = 12;
    pub const DIRECT_TEST_ROLE: u16 = 8;
    pub const DIRECT_SUPPORTING_ROLE: u16 = 6;

    pub const CHANGED_SYMBOL_OWNER: u16 = 40;
    pub const CHANGED_FILE: u16 = 32;
    pub const CHANGE_DISTANCE: [u16; 3] = [0, 20, 10];
    pub const CHANGE_REASONS: [&str; 3] = ["", REASON_CHANGE_DISTANCE_1, REASON_CHANGE_DISTANCE_2];
    pub const TASK_SYMBOL_ROOT_PRIORITY: u16 = 100;
    pub const CHANGE_FILE_ROOT_PRIORITY: u16 = 32;
    pub const CHANGE_SYMBOL_ROOT_PRIORITY: u16 = 140;

    pub const CENTRALITY_ACTIVITY_CAP: usize = 4;
    pub const CENTRALITY_FAN_IN_CAP: usize = 3;
    pub const CENTRALITY_FAN_OUT_CAP: usize = 3;
    pub const FALLBACK_FILES: usize = 20;

    pub const FILE_ORDER: &str =
        "total_desc,symbol_desc,path_desc,change_desc,graph_desc,centrality_desc,native_path_asc";

    pub const REASON_CODES: &[&str] = &[
        REASON_PATH_COMPONENT_EXACT,
        REASON_PATH_PREFIX,
        REASON_PATH_SUBSTRING,
        REASON_PATH_PHRASE,
        REASON_SYMBOL_QUALIFIED_EXACT,
        REASON_SYMBOL_LOCAL_EXACT,
        REASON_SYMBOL_PREFIX,
        REASON_SYMBOL_SUBSTRING,
        REASON_GRAPH_DISTANCE_1,
        REASON_GRAPH_DISTANCE_2,
        REASON_RELATED_TEST_CANDIDATE,
        REASON_DIRECT_TEST_MATCH,
        REASON_DIRECT_TASK_ROLE_MATCH,
        REASON_CHANGED_SYMBOL_OWNER,
        REASON_CHANGED_FILE,
        REASON_CHANGE_DISTANCE_1,
        REASON_CHANGE_DISTANCE_2,
        REASON_CENTRALITY_ACTIVITY,
        REASON_CENTRALITY_FALLBACK,
    ];
}
pub const RANKER_VERSION: &str = ranker_v1::VERSION;
pub const MAX_TASK_BYTES: usize = 4096;
pub const MAX_TASK_TERMS: usize = 64;
pub const MAX_BUDGET: u64 = 16 * 1024 * 1024;
pub const MAX_SELECTED: usize = 200;
pub const MAX_SKIPS: usize = 50;
const MAX_SYMBOLS: usize = ranker_v1::MAX_EXPLAINED_SYMBOLS;
const FALLBACK_FILES: usize = ranker_v1::FALLBACK_FILES;
pub const MAX_LOAD_ATTEMPTS: usize = 256;
pub const MAX_LOAD_BYTES: u64 = 64 * 1024 * 1024;

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
    pub sensitive_files_omitted: usize,
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
    pub load_attempts: usize,
    pub load_bytes_reserved: u64,
    pub load_safety_limited: bool,
    pub load_candidates_omitted: usize,
    pub skips_omitted: usize,
    pub selection_omitted: usize,
}

pub fn eligible(role: FileRole) -> bool {
    !matches!(role, FileRole::Generated | FileRole::Vendor)
}

/// Full-file context must not disclose conventional credential files by default.
/// This deliberately affects context selection only; inventory and exact path
/// queries continue to describe every scanned file.
pub fn sensitive_context_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || name == ".npmrc"
        || name == ".pypirc"
        || name == ".netrc"
        || name.starts_with("credentials")
        || name == "secrets.json"
        || name == "id_rsa"
        || name == "id_ed25519"
        || name.ends_with(".pem")
        || name.ends_with(".key")
}

fn context_eligible(file: &crate::model::FileAnalysis) -> bool {
    eligible(file.role) && !sensitive_context_path(&file.relative_path)
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
            (
                ranker_v1::PATH_COMPONENT_EXACT,
                ranker_v1::REASON_PATH_COMPONENT_EXACT,
            )
        } else if text.starts_with(term) || components.iter().any(|c| c.starts_with(term)) {
            (ranker_v1::PATH_PREFIX, ranker_v1::REASON_PATH_PREFIX)
        } else if text.contains(term) {
            (ranker_v1::PATH_SUBSTRING, ranker_v1::REASON_PATH_SUBSTRING)
        } else {
            continue;
        };
        score = score.saturating_add(value).min(ranker_v1::PATH_CAP);
        reasons.insert(reason);
    }
    if !task.phrase.is_empty() && text.contains(&task.phrase) {
        score = score
            .saturating_add(ranker_v1::PATH_PHRASE)
            .min(ranker_v1::PATH_CAP);
        reasons.insert(ranker_v1::REASON_PATH_PHRASE);
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
                (
                    ranker_v1::SYMBOL_QUALIFIED_EXACT,
                    ranker_v1::REASON_SYMBOL_QUALIFIED_EXACT,
                )
            } else if name == *term {
                (
                    ranker_v1::SYMBOL_LOCAL_EXACT,
                    ranker_v1::REASON_SYMBOL_LOCAL_EXACT,
                )
            } else if qualified.starts_with(term) || name.starts_with(term) {
                (ranker_v1::SYMBOL_PREFIX, ranker_v1::REASON_SYMBOL_PREFIX)
            } else if qualified.contains(term) || name.contains(term) {
                (
                    ranker_v1::SYMBOL_SUBSTRING,
                    ranker_v1::REASON_SYMBOL_SUBSTRING,
                )
            } else {
                (0, "")
            }
        })
        .max_by_key(|v| v.0)
        .unwrap_or((0, ""))
}
fn sort_candidates(g: &RepositoryGraph, candidates: &mut [Candidate]) {
    debug_assert_eq!(
        ranker_v1::FILE_ORDER,
        "total_desc,symbol_desc,path_desc,change_desc,graph_desc,centrality_desc,native_path_asc"
    );
    debug_assert!(candidates.iter().all(|candidate| {
        candidate
            .reasons
            .iter()
            .all(|reason| ranker_v1::REASON_CODES.contains(reason))
    }));
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
fn record_traversal_evidence(
    candidates: &mut [Candidate],
    owners: &[Option<usize>],
    root: EntityId,
    distances: &BTreeMap<EntityId, u8>,
    from_change: bool,
) {
    let root_owner = owners[root.0 as usize];
    for (&id, &distance) in distances {
        let Some(file) = owners[id.0 as usize] else {
            continue;
        };
        // A root's own file is already represented by direct path, symbol, or
        // change evidence. Graph evidence must reach a distinct file.
        if distance == 0 || root_owner == Some(file) {
            continue;
        }
        let candidate = &mut candidates[file];
        candidate.graph_distance = Some(
            candidate
                .graph_distance
                .map_or(distance, |existing| existing.min(distance)),
        );
        if from_change {
            candidate.change_distance = Some(
                candidate
                    .change_distance
                    .map_or(distance, |existing| existing.min(distance)),
            );
        }
    }
}
// One traversal call shares all operation-wide ceilings across lexical and change roots.
// Root origin is retained so change distances do not accidentally become task evidence.
pub fn rank(g: &RepositoryGraph, task: &Task, changes: Option<&ChangeSignal>) -> Ranking {
    let owners = query::file_owners(g);
    let metrics = query::structural_metrics(g, &owners);
    let mut candidates: Vec<_> = (0..g.index.files.len()).map(Candidate::new).collect();
    let sensitive_files_omitted = g
        .index
        .files
        .iter()
        .filter(|file| eligible(file.role) && sensitive_context_path(&file.relative_path))
        .count();
    let mut seeds = BTreeMap::<EntityId, (u16, bool)>::new();
    let mut matches = vec![vec![]; candidates.len()];
    for c in &mut candidates {
        if !context_eligible(&g.index.files[c.file]) {
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
            (m.structural_activity
                .min(ranker_v1::CENTRALITY_ACTIVITY_CAP)
                + m.incoming.min(ranker_v1::CENTRALITY_FAN_IN_CAP)
                + m.outgoing.min(ranker_v1::CENTRALITY_FAN_OUT_CAP)) as u16;
    }
    for (i, s) in g.index.symbols.iter().enumerate() {
        let id = EntityId((1 + g.index.files.len() + i) as u32);
        let Some(file) = owners[id.0 as usize] else {
            continue;
        };
        if !context_eligible(&g.index.files[file]) {
            continue;
        }
        let (value, reason) = symbol_score(&s.name, &s.qualified_name, task);
        if value > 0 {
            matches[file].push((Reverse(value), i, reason));
            seeds.insert(id, (ranker_v1::TASK_SYMBOL_ROOT_PRIORITY + value, false));
        }
    }
    for (c, matching) in candidates.iter_mut().zip(&mut matches) {
        matching.sort();
        c.symbols_omitted = matching.len().saturating_sub(MAX_SYMBOLS);
        for &(Reverse(score), index, reason) in matching.iter().take(MAX_SYMBOLS) {
            c.score.lexical_symbol = c
                .score
                .lexical_symbol
                .saturating_add(score)
                .min(ranker_v1::SYMBOL_CAP);
            c.symbols.push(index);
            c.reasons.insert(reason);
        }
    }
    if let Some(change) = changes {
        for (&file, &value) in &change.files {
            if !context_eligible(&g.index.files[file]) {
                continue;
            }
            candidates[file].score.change = value;
            candidates[file]
                .reasons
                .insert(if value == ranker_v1::CHANGED_SYMBOL_OWNER {
                    ranker_v1::REASON_CHANGED_SYMBOL_OWNER
                } else {
                    ranker_v1::REASON_CHANGED_FILE
                });
        }
        for &id in &change.seeds {
            if owners[id.0 as usize].is_some_and(|f| context_eligible(&g.index.files[f])) {
                let priority = if g.symbol(id).is_some() {
                    ranker_v1::CHANGE_SYMBOL_ROOT_PRIORITY
                } else {
                    ranker_v1::CHANGE_FILE_ROOT_PRIORITY
                };
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
        record_traversal_evidence(
            &mut candidates,
            &owners,
            root.root,
            &root.distances,
            from_change,
        );
    }
    let eligible_count = candidates
        .iter()
        .filter(|c| context_eligible(&g.index.files[c.file]))
        .count();
    candidates.retain_mut(|c| {
        if !context_eligible(&g.index.files[c.file]) {
            return false;
        }
        if let Some(d) = c.graph_distance {
            c.score.graph = ranker_v1::GRAPH_DISTANCE[d as usize];
            c.reasons.insert(ranker_v1::GRAPH_REASONS[d as usize]);
        }
        if let Some(d) = c.change_distance {
            let boost = ranker_v1::CHANGE_DISTANCE[d as usize];
            if boost > c.score.change {
                c.score.change = boost;
                c.reasons.insert(ranker_v1::CHANGE_REASONS[d as usize]);
            }
        }
        let direct = c.score.lexical_path > 0 || c.score.lexical_symbol > 0;
        match g.index.files[c.file].role {
            FileRole::Test if direct => {
                c.score.role = ranker_v1::DIRECT_TEST_ROLE;
                c.reasons.insert(ranker_v1::REASON_DIRECT_TEST_MATCH);
            }
            FileRole::Test if c.graph_distance.is_some() => {
                c.score.role = ranker_v1::RELATED_TEST_ROLE;
                c.reasons.insert(ranker_v1::REASON_RELATED_TEST_CANDIDATE);
            }
            FileRole::Documentation | FileRole::Configuration | FileRole::Schema if direct => {
                c.score.role = ranker_v1::DIRECT_SUPPORTING_ROLE;
                c.reasons.insert(ranker_v1::REASON_DIRECT_TASK_ROLE_MATCH);
            }
            _ => (),
        }
        if c.score.centrality > 0 {
            c.reasons.insert(ranker_v1::REASON_CENTRALITY_ACTIVITY);
        }
        if fallback {
            c.reasons.insert(ranker_v1::REASON_CENTRALITY_FALLBACK);
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
        sensitive_files_omitted,
    }
}

pub fn select(g: &RepositoryGraph, ranking: Ranking, budget: u64) -> Bundle {
    select_with_work_limits(
        g,
        ranking,
        budget,
        SelectionLimits {
            max_selected: MAX_SELECTED,
            max_skips: MAX_SKIPS,
            max_file_bytes: MAX_BUDGET,
        },
        LoadWorkLimits {
            max_attempts: MAX_LOAD_ATTEMPTS,
            max_reserved_bytes: MAX_LOAD_BYTES,
        },
    )
}
#[derive(Clone, Copy)]
struct SelectionLimits {
    max_selected: usize,
    max_skips: usize,
    max_file_bytes: u64,
}
#[derive(Clone, Copy)]
struct LoadWorkLimits {
    max_attempts: usize,
    max_reserved_bytes: u64,
}
#[cfg(test)]
fn select_with_limits(
    g: &RepositoryGraph,
    ranking: Ranking,
    budget: u64,
    max_selected: usize,
    max_skips: usize,
    max_file: u64,
) -> Bundle {
    select_with_work_limits(
        g,
        ranking,
        budget,
        SelectionLimits {
            max_selected,
            max_skips,
            max_file_bytes: max_file,
        },
        LoadWorkLimits {
            max_attempts: MAX_LOAD_ATTEMPTS,
            max_reserved_bytes: MAX_LOAD_BYTES,
        },
    )
}
fn select_with_work_limits(
    g: &RepositoryGraph,
    mut ranking: Ranking,
    budget: u64,
    selection_limits: SelectionLimits,
    load_limits: LoadWorkLimits,
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
        load_attempts: 0,
        load_bytes_reserved: 0,
        load_safety_limited: false,
        load_candidates_omitted: 0,
        skips_omitted: 0,
        selection_omitted: 0,
    };
    for (i, candidate) in candidates.into_iter().enumerate() {
        if bundle.selected.len() >= selection_limits.max_selected {
            bundle.selection_omitted = count - i;
            break;
        }
        let remaining = budget - bundle.used;
        let read_allowance = remaining.min(selection_limits.max_file_bytes);
        // source::load can read at most allowance+1 bytes to reject a file that
        // grows past its metadata size. Reserve that maximum before the call so
        // failed candidates and metadata checks remain inside an operation cap.
        let reservation = read_allowance.saturating_add(1);
        if bundle.load_attempts >= load_limits.max_attempts
            || bundle.load_bytes_reserved.saturating_add(reservation)
                > load_limits.max_reserved_bytes
        {
            bundle.load_safety_limited = true;
            bundle.load_candidates_omitted = count - i;
            break;
        }
        bundle.load_attempts += 1;
        bundle.load_bytes_reserved += reservation;
        // The loader reads at most remaining+1 bytes; rejected large files never
        // materialize their full contents. This same packer is used by baselines.
        let result = source::load(
            &g.index.root,
            &g.index.files[candidate.file].relative_path,
            read_allowance,
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
            Err(AnalysisStatus::TooLarge) if remaining < selection_limits.max_file_bytes => {
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
        if bundle.skipped.len() < selection_limits.max_skips {
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
                signal.files.insert(i, ranker_v1::CHANGED_FILE);
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
                    signal.files.insert(file, ranker_v1::CHANGED_SYMBOL_OWNER);
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
        assert_eq!(c("src/a.rs").graph_distance, None);
        assert_eq!(c("src/a.rs").score.graph, 0);
        assert_eq!(c("tests/spec.py").graph_distance, None);
        assert_eq!(c("tests/spec.py").score.role, ranker_v1::DIRECT_TEST_ROLE);
        assert!(c("tests/spec.py").reasons.contains("direct_test_match"));
        assert!(
            !c("tests/spec.py")
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
        // Seed owners remain direct evidence; they do not receive graph evidence.
        assert!(
            r.candidates
                .iter()
                .filter(|c| g.index.files[c.file]
                    .relative_path
                    .extension()
                    .is_some_and(|e| e == "rs"))
                .all(|c| c.graph_distance.is_none() && c.score.graph == 0)
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

    #[test]
    fn direct_matches_and_change_seed_owners_are_not_graph_evidence() {
        let d = TempDir::new().unwrap();
        write(d.path(), "src/changed.py", "def target(): pass\n");
        write(
            d.path(),
            "tests/test_target.py",
            "def target(): pass\ndef test_case(): target()\n",
        );
        let g = build(d.path());
        let task = Task::parse("target").unwrap();
        let ranked = rank(&g, &task, None);
        let test = ranked
            .candidates
            .iter()
            .find(|candidate| {
                g.index.files[candidate.file].relative_path == Path::new("tests/test_target.py")
            })
            .unwrap();
        assert_eq!(test.score.role, ranker_v1::DIRECT_TEST_ROLE);
        assert_eq!(test.graph_distance, None);
        assert!(test.reasons.contains("direct_test_match"));
        assert!(!test.reasons.contains("related_test_candidate"));
        assert!(ranked.candidates.iter().all(|candidate| {
            candidate.graph_distance.is_none()
                && candidate.score.graph == 0
                && !candidate
                    .reasons
                    .iter()
                    .any(|reason| reason.starts_with("graph_distance_"))
        }));

        let file = g
            .index
            .files
            .iter()
            .position(|file| file.relative_path == Path::new("src/changed.py"))
            .unwrap();
        let change = ChangeSignal {
            range: "base..head".into(),
            base: "base".into(),
            head: "head".into(),
            files_observed: 1,
            symbols_observed: 0,
            symbols_mapped: 0,
            symbols_unmapped: 0,
            ambiguous_keys: 0,
            comparisons_unavailable: 0,
            deleted_only: 0,
            files: BTreeMap::from([(file, ranker_v1::CHANGED_FILE)]),
            seeds: BTreeSet::from([EntityId((1 + file) as u32)]),
        };
        let changed = rank(&g, &Task::parse("absent").unwrap(), Some(&change));
        let owner = changed
            .candidates
            .iter()
            .find(|candidate| candidate.file == file)
            .unwrap();
        assert_eq!(owner.score.change, ranker_v1::CHANGED_FILE);
        assert_eq!(owner.graph_distance, None);
        assert_eq!(owner.change_distance, None);
        assert!(!owner.reasons.iter().any(|reason| {
            reason.starts_with("graph_distance_") || reason.starts_with("change_distance_")
        }));
    }

    #[test]
    fn traversal_evidence_requires_a_distinct_file_and_relation_distance() {
        let mut candidates = vec![Candidate::new(0), Candidate::new(1)];
        let owners = [None, Some(0), Some(0), Some(1)];
        let root = EntityId(2); // symbol in file 0
        let distances = BTreeMap::from([(root, 0), (EntityId(3), 1)]);
        record_traversal_evidence(&mut candidates, &owners, root, &distances, true);
        assert_eq!(candidates[0].graph_distance, None);
        assert_eq!(candidates[0].change_distance, None);
        assert_eq!(candidates[1].graph_distance, Some(1));
        assert_eq!(candidates[1].change_distance, Some(1));
    }

    #[test]
    fn direct_test_classification_survives_root_truncation() {
        let d = TempDir::new().unwrap();
        write(
            d.path(),
            "tests/test_hit.py",
            (0..150)
                .map(|i| format!("def hit_{i}(): pass\n"))
                .collect::<String>(),
        );
        let g = build(d.path());
        let r = rank(&g, &Task::parse("hit").unwrap(), None);
        assert!(r.roots_omitted > 0);
        let test = r.candidates.first().unwrap();
        assert_eq!(g.index.files[test.file].role, FileRole::Test);
        assert_eq!(test.score.role, ranker_v1::DIRECT_TEST_ROLE);
        assert!(test.reasons.contains("direct_test_match"));
        assert!(!test.reasons.contains("related_test_candidate"));
        assert_eq!(test.graph_distance, None);
        assert_eq!(test.score.graph, 0);
    }

    #[test]
    fn candidate_load_work_stops_at_attempt_and_byte_limits() {
        let d = TempDir::new().unwrap();
        for path in [
            "a_candidate.py",
            "b_candidate.py",
            "c_candidate.py",
            "d_candidate.py",
            "e_candidate.py",
        ] {
            write(d.path(), path, "def candidate(): pass\n");
        }
        fs::write(d.path().join("a_candidate.py"), [255]).unwrap();
        fs::write(d.path().join("b_candidate.py"), [255]).unwrap();
        fs::write(d.path().join("d_candidate.py"), "").unwrap();
        let g = build(d.path());
        fs::remove_file(d.path().join("c_candidate.py")).unwrap();
        let ranking = || rank(&g, &Task::parse("candidate").unwrap(), None);

        let attempts = select_with_work_limits(
            &g,
            ranking(),
            10,
            SelectionLimits {
                max_selected: 200,
                max_skips: 50,
                max_file_bytes: 10,
            },
            LoadWorkLimits {
                max_attempts: 2,
                max_reserved_bytes: 100,
            },
        );
        assert_eq!(attempts.load_attempts, 2);
        assert_eq!(attempts.load_failures, 2);
        assert_eq!(attempts.load_bytes_reserved, 22);
        assert!(attempts.load_safety_limited);
        assert_eq!(attempts.load_candidates_omitted, 3);
        assert!(attempts.selected.is_empty());

        let bytes = select_with_work_limits(
            &g,
            ranking(),
            10,
            SelectionLimits {
                max_selected: 200,
                max_skips: 50,
                max_file_bytes: 10,
            },
            LoadWorkLimits {
                max_attempts: 10,
                max_reserved_bytes: 22,
            },
        );
        assert_eq!(bytes.load_attempts, 2);
        assert_eq!(bytes.load_bytes_reserved, 22);
        assert_eq!(bytes.load_failures, 2);
        assert!(bytes.selected.is_empty());
        assert!(bytes.load_safety_limited);
        assert_eq!(bytes.load_candidates_omitted, 3);

        let mut zero_first = ranking();
        zero_first.candidates.retain(|candidate| {
            g.index.files[candidate.file].relative_path == Path::new("d_candidate.py")
                || g.index.files[candidate.file].relative_path == Path::new("e_candidate.py")
        });
        zero_first
            .candidates
            .sort_by_key(|candidate| g.index.files[candidate.file].relative_path.clone());
        let zeros = select_with_work_limits(
            &g,
            zero_first,
            10,
            SelectionLimits {
                max_selected: 200,
                max_skips: 50,
                max_file_bytes: 10,
            },
            LoadWorkLimits {
                max_attempts: 1,
                max_reserved_bytes: 11,
            },
        );
        assert_eq!(zeros.load_attempts, 1);
        assert_eq!(zeros.used, 0);
        assert_eq!(zeros.selected.len(), 1);
        assert!(zeros.selected[0].content.is_empty());
        assert!(zeros.load_safety_limited);
        assert_eq!(zeros.load_candidates_omitted, 1);
    }

    #[test]
    fn file_ranker_v1_contract_pins_scores_reasons_order_and_fallback() {
        let d = TempDir::new().unwrap();
        write(d.path(), "src/cache.py", "def cache(): pass\n");
        write(
            d.path(),
            "tests/cache_contract.py",
            "def test_contract(): pass\n",
        );
        write(d.path(), "a_notes/cache.md", "cache notes\n");
        write(
            d.path(),
            "hub.py",
            (0..80)
                .map(|i| format!("def f{i}(): f0()\n"))
                .collect::<String>(),
        );
        let g = build(d.path());
        assert_eq!(RANKER_VERSION, "file-ranker-v1");
        assert_eq!(
            ranker_v1::FILE_ORDER,
            "total_desc,symbol_desc,path_desc,change_desc,graph_desc,centrality_desc,native_path_asc"
        );

        let ranked = rank(&g, &Task::parse("cache").unwrap(), None);
        let actual: Vec<_> = ranked
            .candidates
            .iter()
            .map(|candidate| {
                (
                    g.index.files[candidate.file]
                        .relative_path
                        .to_string_lossy()
                        .into_owned(),
                    candidate.score.lexical_path,
                    candidate.score.lexical_symbol,
                    candidate.score.graph,
                    candidate.score.role,
                    candidate.score.change,
                    candidate.score.centrality,
                    candidate.reasons.iter().copied().collect::<Vec<_>>(),
                )
            })
            .collect();
        assert!(
            actual
                .iter()
                .flat_map(|entry| &entry.7)
                .all(|reason| { ranker_v1::REASON_CODES.contains(reason) })
        );
        assert_eq!(
            actual,
            vec![
                (
                    "src/cache.py".into(),
                    42,
                    45,
                    0,
                    0,
                    0,
                    0,
                    vec![
                        "path_component_exact",
                        "path_phrase",
                        "symbol_qualified_exact"
                    ]
                ),
                (
                    "tests/cache_contract.py".into(),
                    42,
                    0,
                    0,
                    8,
                    0,
                    0,
                    vec!["direct_test_match", "path_component_exact", "path_phrase"]
                ),
                (
                    "a_notes/cache.md".into(),
                    42,
                    0,
                    0,
                    6,
                    0,
                    0,
                    vec![
                        "direct_task_role_match",
                        "path_component_exact",
                        "path_phrase"
                    ]
                ),
            ]
        );
        let fallback = rank(&g, &Task::parse("absent").unwrap(), None);
        assert!(fallback.fallback);
        assert_eq!(
            g.index.files[fallback.candidates[0].file].relative_path,
            Path::new("hub.py")
        );
        assert!(
            fallback.candidates[0]
                .reasons
                .contains("centrality_fallback")
        );
    }

    /// Labels are fixture-authored before ranking. Test files are expected because
    /// their paths directly match the task and their test role is useful; none is
    /// presented as a cross-file graph association or coverage fact.
    #[test]
    fn expected_file_recall_benchmark() {
        let d = TempDir::new().unwrap();
        let fixtures = [
            ("a_notes/cache.md", "cache migration notes\n"),
            ("a_cache_helper.rs", "pub fn helper() {}\n"),
            ("src/cache.py", "def cache(): return True\n"),
            (
                "tests/test_cache_contract.py",
                "from cache import cache\ndef test_contract(): assert cache()\n",
            ),
            ("a_notes/router.md", "router migration notes\n"),
            ("a_router_helper.rs", "pub fn helper() {}\n"),
            (
                "src/router.rs",
                "pub fn router(value: &str) -> bool { !value.is_empty() }\n",
            ),
            (
                "tests/router_contract.rs",
                "use crate::router::router;\n#[test] fn rejects_unknown() { assert!(!router(\"\")); }\n",
            ),
            ("a_notes/retry.md", "retry migration notes\n"),
            ("a_retry_helper.rs", "pub fn helper() {}\n"),
            ("src/retry.py", "def retry(attempts): return attempts > 0\n"),
            (
                "tests/test_retry_policy.py",
                "from retry import retry\ndef honors_delay(): assert retry(1)\n",
            ),
            ("a_notes/codec.md", "codec migration notes\n"),
            ("a_codec_helper.rs", "pub fn helper() {}\n"),
            (
                "src/codec.rs",
                "pub fn codec(input: &[u8]) -> Vec<u8> { input.to_vec() }\n",
            ),
            (
                "tests/codec_roundtrip.rs",
                "use crate::codec::codec;\n#[test] fn preserves_roundtrip() { assert_eq!(codec(b\"x\"), b\"x\"); }\n",
            ),
            ("a_setup_helper.rs", "pub fn helper() {}\n"),
            ("docs/setup.md", "setup instructions\n"),
            ("config/setup.toml", "setup = true\n"),
            ("a_protocol_helper.rs", "pub fn helper() {}\n"),
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
            ("cache", ["src/cache.py", "tests/test_cache_contract.py"]),
            ("router", ["src/router.rs", "tests/router_contract.rs"]),
            ("retry", ["src/retry.py", "tests/test_retry_policy.py"]),
            ("codec", ["src/codec.rs", "tests/codec_roundtrip.rs"]),
            ("setup", ["docs/setup.md", "config/setup.toml"]),
            ("protocol", ["docs/protocol.md", "schema/protocol.json"]),
        ];
        let strategies = [
            "production",
            "prephase_path_symbol_search",
            "path_order",
            "path_lexical",
            "production_without_role",
        ];
        let mut total_hits = [0usize; 5];
        for (text, expected) in scenarios {
            let budget = 256;
            let task = Task::parse(text).unwrap();
            let expected_test = expected
                .iter()
                .find(|path| path.starts_with("tests/"))
                .copied();
            if let Some(test_path) = expected_test {
                let file = g
                    .index
                    .files
                    .iter()
                    .position(|f| f.relative_path == Path::new(test_path))
                    .unwrap();
                let owners = query::file_owners(&g);
                let symbol_search = query::search(
                    &g.index,
                    text,
                    query::SearchKind::Symbol,
                    query::MAX_RESULTS,
                )
                .unwrap();
                assert!(
                    !symbol_search
                        .matches
                        .iter()
                        .any(|m| owners[m.entity.0 as usize] == Some(file))
                );
                let candidate = rank(&g, &task, None)
                    .candidates
                    .into_iter()
                    .find(|candidate| candidate.file == file)
                    .unwrap();
                assert!(candidate.reasons.contains("direct_test_match"));
                assert!(!candidate.reasons.contains("related_test_candidate"));
                assert_eq!(candidate.graph_distance, None);
            }
            let mut scenario_hits = [0; 5];
            let mut scenario_selected = Vec::with_capacity(strategies.len());
            for strategy in 0..strategies.len() {
                let mut ranking = match strategy {
                    0 | 4 => rank(&g, &task, None),
                    1 => {
                        let found = query::search(
                            &g.index,
                            text,
                            query::SearchKind::All,
                            query::MAX_RESULTS,
                        )
                        .unwrap();
                        let owners = query::file_owners(&g);
                        let mut seen = BTreeSet::new();
                        let candidates = found
                            .matches
                            .into_iter()
                            .filter_map(|matched| owners[matched.entity.0 as usize])
                            .filter(|&file| eligible(g.index.files[file].role) && seen.insert(file))
                            .map(Candidate::new)
                            .collect();
                        Ranking {
                            candidates,
                            eligible: g.index.files.iter().filter(|f| eligible(f.role)).count(),
                            fallback: false,
                            fallback_omitted: 0,
                            roots: 0,
                            roots_omitted: 0,
                            graph_truncated: false,
                            sensitive_files_omitted: 0,
                        }
                    }
                    2 | 3 => Ranking {
                        candidates: g
                            .index
                            .files
                            .iter()
                            .enumerate()
                            .filter(|(_, f)| eligible(f.role))
                            .map(|(i, _)| Candidate::new(i))
                            .collect(),
                        eligible: g.index.files.iter().filter(|f| eligible(f.role)).count(),
                        fallback: false,
                        fallback_omitted: 0,
                        roots: 0,
                        roots_omitted: 0,
                        graph_truncated: false,
                        sensitive_files_omitted: 0,
                    },
                    _ => unreachable!(),
                };
                if strategy == 2 {
                    ranking.candidates.sort_by_key(|candidate| {
                        g.index.files[candidate.file].relative_path.clone()
                    });
                } else if strategy == 3 {
                    ranking.candidates.sort_by_key(|candidate| {
                        (
                            Reverse(
                                path_score(
                                    &utf8_relative(&g.index.files[candidate.file].relative_path)
                                        .unwrap(),
                                    &task,
                                )
                                .0,
                            ),
                            g.index.files[candidate.file].relative_path.clone(),
                        )
                    });
                } else if strategy == 4 {
                    for candidate in &mut ranking.candidates {
                        candidate.score.role = 0;
                        candidate.reasons.retain(|reason| {
                            !matches!(
                                *reason,
                                "direct_test_match"
                                    | "related_test_candidate"
                                    | "direct_task_role_match"
                            )
                        });
                    }
                    sort_candidates(&g, &mut ranking.candidates);
                }
                let b = select(&g, ranking, budget);
                let selected_paths: Vec<_> = b
                    .selected
                    .iter()
                    .map(|selected| {
                        g.index.files[selected.candidate.file]
                            .relative_path
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect();
                scenario_selected.push(selected_paths);
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
                "benchmark task={text:?} budget={budget} expected={expected:?} strategy_hits={strategies:?}:{scenario_hits:?}/2 selected={scenario_selected:?} bytes=[256,256,256,256,256]"
            );
            assert_eq!(scenario_hits[0], 2);
            assert_eq!(scenario_hits[1], 1);
            assert_eq!(scenario_hits[2], 0);
            assert_eq!(
                scenario_hits[4],
                if expected_test.is_some() { 2 } else { 1 }
            );
        }
        eprintln!(
            "benchmark aggregate strategies={strategies:?} hits={total_hits:?}/12 macro_recall={:?}",
            total_hits.map(|h| h as f64 / 12.0)
        );
        assert_eq!(total_hits[0], 12);
        assert!(total_hits[0] > total_hits[1]);
        assert!(total_hits[0] > total_hits[2]);
        assert!(total_hits[0] > total_hits[3]);
        assert!(total_hits[0] > total_hits[4]);
    }
}
