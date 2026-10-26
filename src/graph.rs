//! Fresh, bounded repository graph. IDs address canonical vector entries, never names.
use crate::{index::IndexError, model::*, relations::*};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    path::Path,
};
pub const MAX_ENTITIES: usize = 50_000;
pub const MAX_RELATIONS: usize = 100_000;
pub const MAX_RELATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DEPTH: u8 = 8;
pub(crate) const MAX_QUERY_ROOTS: usize = 128;
pub(crate) const MAX_VISITED: usize = 10_000;
pub(crate) const MAX_EMITTED: usize = 25_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelationId(pub u32);
#[derive(Debug)]
pub enum Entity {
    Repository,
    File(usize),
    /// Canonical symbol record: structural key is path + kind + whole range.
    Symbol(usize),
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Resolved(EntityId),
    Unresolved(String),
}
#[derive(Debug)]
pub struct Relation {
    pub source: EntityId,
    pub target: Target,
    pub kind: RelationKind,
    pub resolution: Resolution,
    pub metadata: Metadata,
    pub evidence: Evidence,
}
pub struct RepositoryGraph {
    pub index: RepositoryIndex,
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub relation_files: Vec<RelationFileAnalysis>,
    forward: Vec<Vec<RelationId>>,
    reverse: Vec<Vec<RelationId>>,
}
#[derive(Clone, Copy)]
struct GraphLimits {
    text_bytes: usize,
    entities: usize,
    relations: usize,
}
#[cfg(test)]
pub fn build(options: &IndexOptions) -> Result<RepositoryGraph, IndexError> {
    let (index, files) = crate::index::graph_analysis(options)?;
    from_analysis(index, files)
}
pub(crate) fn from_analysis(
    index: RepositoryIndex,
    files: Vec<FileRelations>,
) -> Result<RepositoryGraph, IndexError> {
    construct(
        index,
        files,
        GraphLimits {
            text_bytes: MAX_RELATION_BYTES,
            entities: MAX_ENTITIES,
            relations: MAX_RELATIONS,
        },
    )
}

fn construct(
    index: RepositoryIndex,
    files: Vec<FileRelations>,
    limits: GraphLimits,
) -> Result<RepositoryGraph, IndexError> {
    if 1usize
        .saturating_add(index.files.len())
        .saturating_add(index.symbols.len())
        > limits.entities
    {
        return Err(IndexError::GraphLimit);
    }
    let text_bytes = index
        .files
        .iter()
        .map(|f| f.relative_path.as_os_str().len())
        .sum::<usize>()
        + index
            .symbols
            .iter()
            .map(|s| s.relative_path.as_os_str().len())
            .sum::<usize>()
        + files
            .iter()
            .flat_map(|f| {
                f.extraction
                    .candidates
                    .iter()
                    .map(|c| candidate_bytes(c, &f.relative_path))
            })
            .sum::<usize>();
    if text_bytes > limits.text_bytes {
        return Err(IndexError::GraphLimit);
    }
    let mut entities = vec![Entity::Repository];
    entities.extend((0..index.files.len()).map(Entity::File));
    let symbol_offset = entities.len();
    entities.extend((0..index.symbols.len()).map(Entity::Symbol));
    let mut g = RepositoryGraph {
        forward: vec![vec![]; entities.len()],
        reverse: vec![vec![]; entities.len()],
        index,
        entities,
        relations: vec![],
        relation_files: vec![],
    };
    let file_ids: BTreeMap<_, _> = g
        .index
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.relative_path.clone(), EntityId((i + 1) as u32)))
        .collect();
    let mut symbols: BTreeMap<_, Vec<usize>> = BTreeMap::new();
    for (i, s) in g.index.symbols.iter().enumerate() {
        symbols.entry(s.relative_path.clone()).or_default().push(i);
    }
    let mut extracted: BTreeMap<_, _> = files
        .into_iter()
        .map(|f| (f.relative_path, f.extraction))
        .collect();
    for (path, file_id) in file_ids {
        let file_index = file_id.0 as usize - 1;
        let language = g.index.files[file_index].language.clone();
        g.push(
            Relation {
                source: EntityId(0),
                target: Target::Resolved(file_id),
                kind: RelationKind::Contains,
                resolution: Resolution::DirectStructural,
                metadata: Metadata::None,
                evidence: Evidence {
                    relative_path: path.clone(),
                    range: None,
                    language: None,
                    derivation: Derivation::RepositoryContainsFile,
                },
            },
            limits,
        )?;
        let indices = symbols.remove(&path).unwrap_or_default();
        let lexical = LexicalIndex::new(
            indices
                .iter()
                .map(|&i| {
                    (
                        g.index.symbols[i].range,
                        EntityId((symbol_offset + i) as u32),
                    )
                })
                .collect(),
        );
        let mut qualified: HashMap<&str, Vec<EntityId>> = HashMap::new();
        // Build maps separately from mutation of graph relations to keep ownership simple.
        let mut containment = vec![];
        for &i in &indices {
            let s = &g.index.symbols[i];
            let id = EntityId((symbol_offset + i) as u32);
            qualified.entry(&s.qualified_name).or_default().push(id);
            containment.push(Relation {
                source: lexical.owner(s.range, true).unwrap_or(file_id),
                target: Target::Resolved(id),
                kind: RelationKind::Contains,
                resolution: Resolution::DirectStructural,
                metadata: Metadata::None,
                evidence: Evidence {
                    relative_path: path.clone(),
                    range: Some(s.range),
                    language: Some(language.clone()),
                    derivation: Derivation::LexicalRangeContainment,
                },
            });
        }
        let mut candidate_relations = vec![];
        if let Some(extraction) = extracted.remove(&path) {
            let mut accounting = RelationFileAnalysis {
                relative_path: path.clone(),
                status: extraction.status,
                import_count: 0,
                reference_count: 0,
                unresolved_count: 0,
                unsupported_candidate_count: extraction.unsupported,
            };
            for c in extraction.candidates {
                let (target, resolution) = if c.kind == RelationKind::References {
                    let exact = qualified.get(c.target.as_str());
                    if let Some(ids) = exact.filter(|ids| ids.len() == 1) {
                        (
                            Target::Resolved(ids[0]),
                            Resolution::ExactSameFileQualifiedName,
                        )
                    } else {
                        (Target::Unresolved(c.target), Resolution::Unresolved)
                    }
                } else {
                    (Target::Unresolved(c.target), Resolution::Unresolved)
                };
                if c.kind == RelationKind::Imports {
                    accounting.import_count += 1;
                } else {
                    accounting.reference_count += 1;
                }
                if resolution == Resolution::Unresolved {
                    accounting.unresolved_count += 1;
                }
                candidate_relations.push(Relation {
                    source: lexical.owner(c.range, false).unwrap_or(file_id),
                    target,
                    kind: c.kind,
                    resolution,
                    metadata: c.metadata,
                    evidence: Evidence {
                        relative_path: path.clone(),
                        range: Some(c.range),
                        language: Some(language.clone()),
                        derivation: c.derivation,
                    },
                });
            }
            g.relation_files.push(accounting);
        }
        for relation in containment.into_iter().chain(candidate_relations) {
            g.push(relation, limits)?;
        }
    }
    g.relations.sort_by(|a, b| {
        a.evidence
            .relative_path
            .cmp(&b.evidence.relative_path)
            .then(a.evidence.range.cmp(&b.evidence.range))
            .then(a.kind.cmp(&b.kind))
            .then(a.source.cmp(&b.source))
            .then(a.target.cmp(&b.target))
            .then(a.metadata.cmp(&b.metadata))
            .then(a.evidence.derivation.cmp(&b.evidence.derivation))
    });
    for (i, r) in g.relations.iter().enumerate() {
        let id = RelationId(i as u32);
        g.forward[r.source.0 as usize].push(id);
        if let Target::Resolved(target) = r.target {
            g.reverse[target.0 as usize].push(id);
        }
    }
    Ok(g)
}
impl RepositoryGraph {
    fn push(&mut self, relation: Relation, limits: GraphLimits) -> Result<(), IndexError> {
        if self.relations.len() >= limits.relations {
            return Err(IndexError::GraphLimit);
        }
        self.relations.push(relation);
        Ok(())
    }
    pub fn symbol(&self, id: EntityId) -> Option<&Symbol> {
        match self.entities.get(id.0 as usize)? {
            Entity::Symbol(i) => self.index.symbols.get(*i),
            _ => None,
        }
    }
    #[cfg(test)]
    fn lookup(&self, query: &str, file: Option<&Path>) -> (&'static str, Vec<EntityId>) {
        self.lookup_bounded(query, file, MAX_QUERY_ROOTS + 1)
    }
    fn lookup_bounded(
        &self,
        query: &str,
        file: Option<&Path>,
        limit: usize,
    ) -> (&'static str, Vec<EntityId>) {
        // Determine match mode without allocating all Phase 1 matches.
        let in_scope = |s: &&crate::model::Symbol| file.is_none_or(|p| p == s.relative_path);
        let mode = if self
            .index
            .symbols
            .iter()
            .filter(in_scope)
            .any(|s| s.qualified_name == query)
        {
            "qualified_name"
        } else if self
            .index
            .symbols
            .iter()
            .filter(in_scope)
            .any(|s| s.name == query)
        {
            "local_name"
        } else {
            "none"
        };
        let offset = 1 + self.index.files.len();
        let roots = self
            .index
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                (match mode {
                    "qualified_name" => s.qualified_name == query,
                    "local_name" => s.name == query,
                    _ => false,
                }) && file.is_none_or(|p| p == s.relative_path)
            })
            .map(|(i, _)| EntityId((offset + i) as u32))
            .take(limit)
            .collect();
        (mode, roots)
    }
}

/// Sorted intervals with a max-end segment tree support O(log S) owner queries.
/// Crossing/equal synthetic ranges invalidate their entire overlapping region;
/// no symbol touching an ambiguous region is used as a parent.
struct LexicalIndex {
    ranges: Vec<(SourceRange, EntityId)>,
    max_end: Vec<usize>,
    size: usize,
    ambiguous: Vec<(usize, usize)>,
}
impl LexicalIndex {
    fn new(mut ranges: Vec<(SourceRange, EntityId)>) -> Self {
        ranges.retain(|(r, _)| r.start.byte < r.end.byte);
        ranges.sort_by_key(|(r, id)| (r.start.byte, std::cmp::Reverse(r.end.byte), *id));
        let mut stack: Vec<SourceRange> = vec![];
        let mut bad = vec![];
        for &(r, _) in &ranges {
            while stack.last().is_some_and(|p| p.end.byte <= r.start.byte) {
                stack.pop();
            }
            while let Some(p) = stack.last() {
                if r.end.byte > p.end.byte
                    || (r.start.byte == p.start.byte && r.end.byte == p.end.byte)
                {
                    bad.push((p.start.byte, r.end.byte.max(p.end.byte)));
                    stack.pop();
                } else {
                    break;
                }
            }
            stack.push(r);
        }
        bad.sort();
        let mut ambiguous: Vec<(usize, usize)> = vec![];
        for (start, end) in bad {
            if let Some(last) = ambiguous.last_mut().filter(|p| p.1 >= start) {
                last.1 = last.1.max(end);
            } else {
                ambiguous.push((start, end));
            }
        }
        let intersects = |r: SourceRange| {
            let n = ambiguous.partition_point(|p| p.0 < r.end.byte);
            n > 0 && ambiguous[n - 1].1 > r.start.byte
        };
        ranges.retain(|(r, _)| !intersects(*r));
        let size = ranges.len().max(1).next_power_of_two();
        let mut max_end = vec![0; 2 * size];
        for (i, (r, _)) in ranges.iter().enumerate() {
            max_end[size + i] = r.end.byte;
        }
        for i in (1..size).rev() {
            max_end[i] = max_end[2 * i].max(max_end[2 * i + 1]);
        }
        Self {
            ranges,
            max_end,
            size,
            ambiguous,
        }
    }
    fn owner(&self, range: SourceRange, strict: bool) -> Option<EntityId> {
        let n = self.ambiguous.partition_point(|p| p.0 < range.end.byte);
        if n > 0 && self.ambiguous[n - 1].1 > range.start.byte {
            return None;
        }
        let upper = self.ranges.partition_point(|(r, _)| {
            if strict {
                r.start.byte < range.start.byte
                    || (r.start.byte == range.start.byte && r.end.byte > range.end.byte)
            } else {
                r.start.byte <= range.start.byte
            }
        });
        let (mut left, mut right) = (self.size, self.size + upper);
        let mut blocks = vec![];
        while left < right {
            if left % 2 == 1 {
                blocks.push(left);
                left += 1;
            }
            if right % 2 == 1 {
                right -= 1;
                blocks.push(right);
            }
            left /= 2;
            right /= 2;
        }
        // A prefix decomposes into blocks from right to left. Descend only
        // the first qualifying block, keeping lookup O(log S).
        for mut node in blocks {
            if self.max_end[node] < range.end.byte {
                continue;
            }
            while node < self.size {
                node = if self.max_end[node * 2 + 1] >= range.end.byte {
                    node * 2 + 1
                } else {
                    node * 2
                };
            }
            return Some(self.ranges[node - self.size].1);
        }
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    Deps,
    Callers,
    Related,
    Neighborhood,
}
pub struct RootResult {
    pub root: EntityId,
    pub entities: Vec<EntityId>,
    pub relations: Vec<RelationId>,
    pub truncated: bool,
    pub distances: BTreeMap<EntityId, u8>,
    pub predecessors: BTreeMap<EntityId, (EntityId, RelationId)>,
}
pub struct QueryResult {
    pub match_mode: &'static str,
    pub roots: Vec<EntityId>,
    pub results: Vec<RootResult>,
    pub depth: u8,
    pub truncated: bool,
    pub roots_truncated: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct TraversalLimits {
    pub roots: usize,
    pub visited: usize,
    pub emitted: usize,
}
impl RepositoryGraph {
    pub fn query(
        &self,
        kind: QueryKind,
        name: &str,
        file: Option<&Path>,
        depth: u8,
    ) -> QueryResult {
        self.query_with_limits(
            kind,
            name,
            file,
            depth,
            TraversalLimits {
                roots: MAX_QUERY_ROOTS,
                visited: MAX_VISITED,
                emitted: MAX_EMITTED,
            },
        )
    }
    fn query_with_limits(
        &self,
        kind: QueryKind,
        name: &str,
        file: Option<&Path>,
        depth: u8,
        limits: TraversalLimits,
    ) -> QueryResult {
        let (match_mode, roots) = if kind == QueryKind::Neighborhood && file.is_some() {
            (
                "file_path",
                self.index
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| Some(f.relative_path.as_path()) == file)
                    .map(|(i, _)| EntityId((i + 1) as u32))
                    .collect(),
            )
        } else {
            self.lookup_bounded(name, file, limits.roots.saturating_add(1))
        };
        self.traverse_roots(kind, roots, depth, limits, match_mode)
    }
    pub(crate) fn traverse_roots(
        &self,
        kind: QueryKind,
        roots: Vec<EntityId>,
        depth: u8,
        limits: TraversalLimits,
        match_mode: &'static str,
    ) -> QueryResult {
        let roots_truncated = roots.len() > limits.roots;
        let roots = roots.into_iter().take(limits.roots).collect();
        let mut result = QueryResult {
            match_mode,
            roots,
            results: vec![],
            depth,
            truncated: roots_truncated,
            roots_truncated,
        };
        let (mut total_entities, mut total_relations) = (0, 0);
        for &root in &result.roots {
            let mut visited = BTreeSet::new();
            let mut emitted = BTreeSet::new();
            let mut distances = BTreeMap::new();
            let mut predecessors = BTreeMap::new();
            let mut queue = VecDeque::new();
            let mut truncated = depth > MAX_DEPTH;
            if total_entities < limits.visited {
                visited.insert(root);
                distances.insert(root, 0);
                total_entities += 1;
                queue.push_back((root, 0));
            } else {
                truncated = true;
            }
            'walk: while let Some((entity, distance)) = queue.pop_front() {
                if distance >= depth.min(MAX_DEPTH) {
                    continue;
                }
                let outgoing = self.forward[entity.0 as usize].iter().map(|id| (*id, true));
                let incoming = self.reverse[entity.0 as usize]
                    .iter()
                    .map(|id| (*id, false));
                for (id, forward) in outgoing.chain(incoming) {
                    let r = &self.relations[id.0 as usize];
                    let eligible = match kind {
                        QueryKind::Deps => {
                            forward
                                && (r.kind == RelationKind::References
                                    || (r.kind == RelationKind::Imports
                                        && matches!(r.target, Target::Unresolved(_))))
                        }
                        QueryKind::Callers => {
                            !forward
                                && r.kind == RelationKind::References
                                && matches!(
                                    r.metadata,
                                    Metadata::Reference {
                                        context: ReferenceContext::Call
                                    }
                                )
                        }
                        QueryKind::Related => true,
                        QueryKind::Neighborhood => {
                            r.source != EntityId(0) && (r.kind != RelationKind::Imports || forward)
                        }
                    };
                    if !eligible || emitted.contains(&id) {
                        continue;
                    }
                    let next = if forward {
                        match r.target {
                            Target::Resolved(t) => Some(t),
                            Target::Unresolved(_) => None,
                        }
                    } else {
                        Some(r.source)
                    };
                    if total_relations >= limits.emitted
                        || next.is_some_and(|n| {
                            !visited.contains(&n) && total_entities >= limits.visited
                        })
                    {
                        truncated = true;
                        break 'walk;
                    }
                    emitted.insert(id);
                    total_relations += 1;
                    if let Some(next) = next.filter(|n| !visited.contains(n)) {
                        visited.insert(next);
                        distances.insert(next, distance + 1);
                        predecessors.insert(next, (entity, id));
                        total_entities += 1;
                        queue.push_back((next, distance + 1));
                    }
                }
            }
            result.truncated |= truncated;
            result.results.push(RootResult {
                root,
                entities: visited.into_iter().collect(),
                distances,
                predecessors,
                relations: emitted.into_iter().collect(),
                truncated,
            });
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn range(start: usize, end: usize) -> SourceRange {
        SourceRange {
            start: SourcePoint {
                byte: start,
                row: 0,
                column: start,
            },
            end: SourcePoint {
                byte: end,
                row: 0,
                column: end,
            },
        }
    }
    fn fixture(files: &[(&str, &str)]) -> (tempfile::TempDir, RepositoryGraph) {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in files {
            std::fs::write(dir.path().join(name), text).unwrap();
        }
        let g = build(&IndexOptions {
            root: dir.path().into(),
            max_parse_file_bytes: DEFAULT_MAX_PARSE_BYTES,
            probe_git: false,
        })
        .unwrap();
        (dir, g)
    }
    fn id(g: &RepositoryGraph, name: &str) -> EntityId {
        let (_, ids) = g.lookup(name, None);
        assert_eq!(ids.len(), 1);
        ids[0]
    }
    #[test]
    fn neighborhood_shares_exact_and_over_traversal_budgets() {
        let (_, g) = fixture(&[
            ("a.py", "def f(): f()\ndef f(): pass\n"),
            ("b.py", "def f(): pass\n"),
        ]);
        let run = |roots, visited, emitted, depth| {
            g.query_with_limits(
                QueryKind::Neighborhood,
                "f",
                None,
                depth,
                TraversalLimits {
                    roots,
                    visited,
                    emitted,
                },
            )
        };
        let full = run(3, 100, 100, 8);
        assert!(!full.truncated);
        let entities = full.results.iter().map(|r| r.entities.len()).sum();
        let relations = full.results.iter().map(|r| r.relations.len()).sum();
        assert!(!run(3, entities, relations, 8).truncated);
        assert!(run(3, entities - 1, relations, 8).truncated);
        assert!(run(3, entities, relations - 1, 8).truncated);
        assert!(!run(3, 3, 0, 0).truncated);
        let limited = run(2, 100, 100, 8);
        assert!(limited.roots_truncated && limited.truncated);
        assert_eq!(limited.roots, full.roots[..2]);
        assert!(
            full.results
                .iter()
                .all(|r| !r.entities.contains(&EntityId(0)))
        );
    }
    #[test]
    fn root_limits_are_explicit_deterministic_and_file_filtered() {
        let (_, g) = fixture(&[
            ("a.py", "def f(): pass\ndef f(): pass\n"),
            ("b.py", "def f(): pass\n"),
        ]);
        let query = |limit, file| {
            g.query_with_limits(
                QueryKind::Deps,
                "f",
                file,
                0,
                TraversalLimits {
                    roots: limit,
                    visited: 10,
                    emitted: 10,
                },
            )
        };
        let exact = query(3, None);
        assert!(!exact.truncated);
        let over = query(2, None);
        assert!(over.truncated && over.roots_truncated);
        assert_eq!(over.roots, exact.roots[..2]);
        assert_eq!(over.results.len(), 2);
        assert!(over.results.iter().all(|r| !r.truncated));
        let json = crate::output::graph_json("deps", &g, Some("f"), Some(&over)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(json["data"]["roots_truncated"], true);
        assert!(
            crate::output::graph_human("deps", &g, Some("f"), Some(&over))
                .contains("roots_truncated: true")
        );
        assert!(!query(2, Some(Path::new("a.py"))).truncated);
        assert!(!g.query(QueryKind::Deps, "absent", None, 0).roots_truncated);
    }
    #[test]
    fn rust_unqualified_method_and_nested_calls_remain_unresolved() {
        let (_, g) = fixture(&[(
            "a.rs",
            "struct C; impl C { fn unique() {} } fn outer() { fn nested() {} unique(); C::unique(); nested(); helper(); } fn helper() {}",
        )]);
        for name in ["unique", "nested"] {
            assert!(
                g.relations
                    .iter()
                    .any(|r| r.target == Target::Unresolved(name.into()))
            );
        }
        let callers = g.query(QueryKind::Callers, "C::unique", None, 1);
        assert_eq!(callers.results[0].relations.len(), 1);
    }
    #[test]
    fn aggregate_relation_text_counts_targets_aliases_and_repeated_paths() {
        let (dir, g) = fixture(&[
            ("a.py", "import outside as alias\n"),
            ("b.py", "import outside as alias\n"),
        ]);
        // Each file has one containment path plus one candidate path, target and alias.
        let bytes = 2 * (2 * "a.py".len() + "outside".len() + "alias".len());
        let opts = IndexOptions {
            root: dir.path().into(),
            max_parse_file_bytes: DEFAULT_MAX_PARSE_BYTES,
            probe_git: false,
        };
        let limits = GraphLimits {
            entities: g.entities.len(),
            relations: g.relations.len(),
            text_bytes: bytes,
        };
        let (idx, files) = crate::index::graph_analysis(&opts).unwrap();
        assert!(construct(idx, files, limits).is_ok());
        let (idx, files) = crate::index::graph_analysis(&opts).unwrap();
        assert!(matches!(
            construct(
                idx,
                files,
                GraphLimits {
                    text_bytes: bytes - 1,
                    ..limits
                }
            ),
            Err(IndexError::GraphLimit)
        ));
    }
    #[test]
    fn interval_owners_strict_shared_boundaries_and_overlap_defense() {
        let l = LexicalIndex::new(vec![
            (range(0, 100), EntityId(1)),
            (range(10, 50), EntityId(2)),
            (range(20, 50), EntityId(3)),
            (range(20, 40), EntityId(4)),
        ]);
        assert_eq!(l.owner(range(20, 40), true), Some(EntityId(3)));
        assert_eq!(l.owner(range(20, 50), true), Some(EntityId(2)));
        assert_eq!(l.owner(range(20, 30), false), Some(EntityId(4)));
        assert_eq!(l.owner(range(45, 55), false), Some(EntityId(1)));
        assert_eq!(l.owner(range(0, 100), true), None);
        let l = LexicalIndex::new(vec![
            (range(0, 10), EntityId(1)),
            (range(5, 15), EntityId(2)),
            (range(20, 40), EntityId(3)),
            (range(20, 40), EntityId(4)),
            (range(50, 60), EntityId(5)),
        ]);
        for r in [range(6, 7), range(12, 14), range(21, 22)] {
            assert_eq!(l.owner(r, false), None);
        }
        assert_eq!(l.owner(range(51, 52), false), Some(EntityId(5)));
    }
    #[test]
    fn lexical_index_matches_simple_oracle_for_nested_intervals() {
        let intervals = (0..200)
            .map(|i| (range(i * 2, 1000 - i * 2), EntityId(i as u32)))
            .collect::<Vec<_>>();
        let l = LexicalIndex::new(intervals.clone());
        for start in 0..500 {
            for end in [start + 1, 700, 1001] {
                if end <= start {
                    continue;
                }
                let expected = intervals
                    .iter()
                    .rev()
                    .find(|(r, _)| r.start.byte <= start && r.end.byte >= end)
                    .map(|(_, id)| *id);
                assert_eq!(l.owner(range(start, end), false), expected);
            }
        }
    }
    #[test]
    fn lexical_containment_and_occurrence_ownership() {
        let (_, g) = fixture(&[
            (
                "a.py",
                "import outside\nclass C:\n    def method(self):\n        import inside\n        def nested():\n            helper()\n        helper()\ndef helper(): pass\n",
            ),
            (
                "b.rs",
                "struct Foo {}\nimpl Foo { fn method() {} }\nmod inner { fn nested() { helper(); } }\nfn helper() {}",
            ),
        ]);
        for r in &g.relations {
            if r.evidence.derivation == Derivation::RepositoryContainsFile {
                assert_eq!(r.source, EntityId(0));
            }
        }
        let parent = |child| {
            g.relations
                .iter()
                .find(|r| {
                    r.kind == RelationKind::Contains && r.target == Target::Resolved(id(&g, child))
                })
                .unwrap()
                .source
        };
        assert_eq!(parent("C.method"), id(&g, "C"));
        assert_eq!(parent("C.method.nested"), id(&g, "C.method"));
        assert_eq!(parent("inner::nested"), id(&g, "inner"));
        assert!(matches!(
            g.entities[parent("Foo::method").0 as usize],
            Entity::File(_)
        ));
        assert_ne!(parent("Foo::method"), id(&g, "Foo"));
        let inside = g
            .relations
            .iter()
            .find(|r| r.target == Target::Unresolved("inside".into()))
            .unwrap();
        assert_eq!(inside.source, id(&g, "C.method"));
        let outside = g
            .relations
            .iter()
            .find(|r| r.target == Target::Unresolved("outside".into()))
            .unwrap();
        assert!(matches!(
            g.entities[outside.source.0 as usize],
            Entity::File(_)
        ));
        assert!(
            g.relations.iter().any(
                |r| r.kind == RelationKind::References && r.source == id(&g, "C.method.nested")
            )
        );
    }
    #[test]
    fn same_file_resolution_ambiguity_aliases_cycles_and_adjacency() {
        let (_, g) = fixture(&[
            (
                "a.py",
                "from pkg import Thing as B\ndef duplicate(): pass\ndef duplicate(): pass\nclass C:\n    def unique(self): pass\ndef a():\n    b()\n    b()\n    a()\n    C.unique()\n    unique()\n    duplicate()\n    elsewhere()\n    B()\n    self.unique()\ndef b():\n    a()\n",
            ),
            ("b.py", "def elsewhere(): pass\n"),
        ]);
        let a = id(&g, "a");
        let b = id(&g, "b");
        let refs: Vec<_> = g.forward[a.0 as usize]
            .iter()
            .map(|r| &g.relations[r.0 as usize])
            .filter(|r| r.kind == RelationKind::References)
            .collect();
        assert_eq!(
            refs.iter()
                .filter(|r| r.target == Target::Resolved(b))
                .count(),
            2
        );
        assert!(refs.iter().any(|r| r.target == Target::Resolved(a)));
        assert!(
            refs.iter()
                .any(|r| r.resolution == Resolution::ExactSameFileQualifiedName
                    && r.target == Target::Resolved(id(&g, "C.unique")))
        );
        for target in ["unique", "duplicate", "elsewhere", "B", "self.unique"] {
            assert!(
                refs.iter()
                    .any(|r| r.target == Target::Unresolved(target.into()))
            );
        }
        for (i, r) in g.relations.iter().enumerate() {
            let rid = RelationId(i as u32);
            assert!(g.forward[r.source.0 as usize].contains(&rid));
            let incoming: Vec<_> = g
                .reverse
                .iter()
                .enumerate()
                .filter(|(_, ids)| ids.contains(&rid))
                .map(|(i, _)| EntityId(i as u32))
                .collect();
            match r.target {
                Target::Resolved(id) => assert_eq!(incoming, [id]),
                Target::Unresolved(_) => assert!(incoming.is_empty()),
            }
        }
        let q = g.query(QueryKind::Deps, "a", None, 8);
        assert!(!q.truncated);
        assert_eq!(
            q.results[0].entities,
            [a, b, id(&g, "C.unique")]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        let zero = g.query(QueryKind::Deps, "a", None, 0);
        assert_eq!(zero.results[0].entities, [a]);
        assert!(zero.results[0].relations.is_empty());
        let callers = g.query(QueryKind::Callers, "b", None, 1);
        assert_eq!(callers.results[0].relations.len(), 2);
        assert!(callers.results[0].entities.contains(&a));
        let related = g.query(QueryKind::Related, "a", None, 2);
        assert!(related.results[0].entities.contains(&EntityId(0)));
        assert!(g.query(QueryKind::Deps, "absent", None, 1).roots.is_empty());
    }
    #[test]
    fn distinct_qualified_duplicates_local_ambiguity_depth_and_exact_traversal_limits() {
        let (_, g) = fixture(&[(
            "a.py",
            "class C:\n    def dup(self): pass\nclass D:\n    def dup(self): pass\ndef a():\n    b()\n    dup()\ndef b():\n    c()\ndef c(): pass\n",
        )]);
        assert!(
            g.relations
                .iter()
                .any(|r| r.target == Target::Unresolved("dup".into()))
        );
        let one = g.query(QueryKind::Deps, "a", None, 1);
        assert_eq!(one.results[0].entities.len(), 2);
        let two = g.query(QueryKind::Deps, "a", None, 2);
        assert_eq!(two.results[0].entities.len(), 3);
        let exact = g.query_with_limits(
            QueryKind::Deps,
            "a",
            None,
            2,
            TraversalLimits {
                roots: MAX_QUERY_ROOTS,
                visited: 3,
                emitted: 3,
            },
        );
        assert!(!exact.truncated);
        let over = g.query_with_limits(
            QueryKind::Deps,
            "a",
            None,
            2,
            TraversalLimits {
                roots: MAX_QUERY_ROOTS,
                visited: 2,
                emitted: 3,
            },
        );
        assert!(over.truncated);
        let over = g.query_with_limits(
            QueryKind::Deps,
            "a",
            None,
            2,
            TraversalLimits {
                roots: MAX_QUERY_ROOTS,
                visited: 3,
                emitted: 2,
            },
        );
        assert!(over.truncated);
        let callers = g.query(QueryKind::Callers, "c", None, 2);
        assert!(callers.results[0].entities.contains(&id(&g, "a")));
    }
    #[test]
    fn caps_identity_determinism_and_shared_query_budgets() {
        let (dir, g) = fixture(&[("a.py", "def f():\n    f()\ndef f():\n    f()\n")]);
        let (_, ids) = g.lookup("f", None);
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        assert!(
            g.relations
                .iter()
                .filter(|r| r.kind == RelationKind::References)
                .all(|r| matches!(r.target, Target::Unresolved(_)))
        );
        let opts = IndexOptions {
            root: dir.path().into(),
            max_parse_file_bytes: DEFAULT_MAX_PARSE_BYTES,
            probe_git: false,
        };
        let again = build(&opts).unwrap();
        assert_eq!(
            crate::output::graph_json("graph", &g, None, None).unwrap(),
            crate::output::graph_json("graph", &again, None, None).unwrap()
        );
        let exact = GraphLimits {
            text_bytes: MAX_RELATION_BYTES,
            entities: g.entities.len(),
            relations: g.relations.len(),
        };
        let analyze = || crate::index::graph_analysis(&opts).unwrap();
        let (idx, files) = analyze();
        assert!(construct(idx, files, exact).is_ok());
        for limits in [
            GraphLimits {
                entities: exact.entities - 1,
                ..exact
            },
            GraphLimits {
                relations: exact.relations - 1,
                ..exact
            },
        ] {
            let (idx, files) = analyze();
            assert!(matches!(
                construct(idx, files, limits),
                Err(IndexError::GraphLimit)
            ));
        }
        let q = g.query_with_limits(
            QueryKind::Deps,
            "f",
            None,
            8,
            TraversalLimits {
                roots: MAX_QUERY_ROOTS,
                visited: 1,
                emitted: 25,
            },
        );
        assert!(q.truncated);
        assert!(q.results[1].truncated);
        let q = g.query_with_limits(
            QueryKind::Deps,
            "f",
            None,
            8,
            TraversalLimits {
                roots: MAX_QUERY_ROOTS,
                visited: 10,
                emitted: 0,
            },
        );
        assert!(q.truncated);
        let q = g.query_with_limits(
            QueryKind::Related,
            "f",
            None,
            8,
            TraversalLimits {
                roots: MAX_QUERY_ROOTS,
                visited: 1,
                emitted: 100,
            },
        );
        assert!(q.truncated);
        assert!(g.lookup("f", Some(Path::new("elsewhere.py"))).1.is_empty());
    }
}
