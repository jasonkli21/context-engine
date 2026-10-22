//! Committed provider triggers composed with current local relationship evidence.
use crate::{
    application::Result,
    git::revision,
    paths,
    workspace::{self, RelationKind, Workspace},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
};
pub const MAX_VISITS: usize = 4096;
#[derive(Serialize)]
pub struct Impact {
    pub provider: String,
    pub base: String,
    pub head: String,
    pub depth: u8,
    pub changed_paths: Vec<ChangedPath>,
    pub non_utf8_paths_omitted: usize,
    pub candidates: Vec<Candidate>,
    pub traversal_truncated: bool,
    pub eligible_edges_examined: usize,
    pub alternative_witnesses_omitted: usize,
    pub depth_limited_edges: usize,
}
#[derive(Serialize)]
pub struct ChangedPath {
    pub path: String,
    pub side: &'static str,
    pub regular: bool,
}
#[derive(Serialize)]
pub struct Candidate {
    pub repo: String,
    pub distance: usize,
    pub reason: &'static str,
    pub witness_edges: Vec<usize>,
    pub trigger_paths: Vec<String>,
}
// Domain request, owned by launch-time caller; no Clap/adapter input leaks here.
pub fn analyze(
    manifest: &Path,
    provider: &str,
    range: &str,
    depth: u8,
) -> Result<(Workspace, Impact)> {
    if !(1..=3).contains(&depth) {
        return Err("workspace depth must be 1 through 3".into());
    }
    let mut w = workspace::inspect(manifest)?;
    let rev = revision::Revisions::open(&w.repo(provider)?.root, range, revision::LIMITS)?;
    let changes = rev.changes(revision::LIMITS)?;
    let mut paths = vec![];
    let mut eligible = BTreeSet::new();
    let mut package_paths = BTreeSet::new();
    let mut native_omissions = 0;
    for c in changes {
        for (side, entry) in [("base", c.before), ("head", c.after)] {
            if let Some(e) = entry {
                let Some(p) = textual_path(&e.path, &mut native_omissions) else {
                    continue;
                };
                if e.regular() {
                    eligible.insert(e.path.clone());
                    if package_trigger(&e.path) {
                        package_paths.insert(p.clone());
                    }
                }
                paths.push(ChangedPath {
                    path: p,
                    side,
                    regular: e.regular(),
                });
            }
        }
    }
    paths.sort_by(|a, b| (&a.path, a.side).cmp(&(&b.path, b.side)));
    // A deleted/renamed provider endpoint may be absent now. Its exact regular
    // committed diff side supplies availability; consumers must still be readable.
    for mut edge in std::mem::take(&mut w.unavailable_provider_relations) {
        if edge.provider == provider
            && edge
                .provider_path
                .as_ref()
                .is_some_and(|p| eligible.contains(Path::new(p)))
        {
            edge.validation = "committed_provider_current_consumer_usage_unproven";
            if w.relations.len() >= workspace::MAX_EDGES {
                return Err("workspace relationship limit exceeded".into());
            }
            w.relations.push(edge);
        }
    }
    w.relations.sort_by(workspace::relation_order);
    let mut result = Impact {
        provider: provider.into(),
        base: rev.base,
        head: rev.head,
        depth,
        changed_paths: paths,
        non_utf8_paths_omitted: native_omissions,
        candidates: vec![],
        traversal_truncated: false,
        eligible_edges_examined: 0,
        alternative_witnesses_omitted: 0,
        depth_limited_edges: 0,
    };
    traverse(&w, &eligible, &package_paths, &mut result, MAX_VISITS);
    Ok((w, result))
}
fn textual_path(path: &Path, omissions: &mut usize) -> Option<String> {
    let p = paths::utf8_relative(path);
    if p.is_none() {
        *omissions += 1;
    }
    p
}
fn package_trigger(path: &Path) -> bool {
    path == Path::new("Cargo.toml")
        || path == Path::new("build.rs")
        || matches!(
            crate::classify::classify_role(path, &crate::classify::classify_language(path)),
            crate::classify::FileRole::Source
        )
}
fn traverse(
    w: &Workspace,
    eligible: &BTreeSet<PathBuf>,
    package_paths: &BTreeSet<String>,
    result: &mut Impact,
    max: usize,
) {
    let mut seen = BTreeSet::from([result.provider.clone()]);
    let mut queue = VecDeque::from([(result.provider.clone(), vec![], vec![])]);
    let mut candidates = BTreeMap::new();
    while let Some((provider, witness, triggers)) = queue.pop_front() {
        for (i, edge) in w
            .relations
            .iter()
            .enumerate()
            .filter(|(_, e)| e.provider == provider)
        {
            let matched = if witness.is_empty() {
                match edge.kind {
                    RelationKind::CargoPathDependency => !package_paths.is_empty(),
                    RelationKind::DeclaredContractConsumer => edge
                        .provider_path
                        .as_ref()
                        .is_some_and(|p| eligible.contains(Path::new(p))),
                }
            } else {
                edge.kind == RelationKind::CargoPathDependency
            };
            if !matched {
                continue;
            }
            if result.eligible_edges_examined >= max {
                result.traversal_truncated = true;
                result.candidates = candidates.into_values().collect();
                return;
            }
            result.eligible_edges_examined += 1;
            if witness.len() >= result.depth as usize {
                result.depth_limited_edges += 1;
                continue;
            }
            if !seen.insert(edge.consumer.clone()) {
                result.alternative_witnesses_omitted += 1;
                continue;
            }
            let mut chain = witness.clone();
            chain.push(i);
            let triggers = if witness.is_empty() {
                if edge.kind == RelationKind::CargoPathDependency {
                    package_paths.iter().cloned().collect()
                } else {
                    vec![edge.provider_path.clone().unwrap()]
                }
            } else {
                triggers.clone()
            };
            let candidate = Candidate {
                repo: edge.consumer.clone(),
                distance: chain.len(),
                reason: if chain.len() > 1 {
                    "transitive_package_candidate"
                } else if edge.kind == RelationKind::CargoPathDependency {
                    "coarse_package_candidate"
                } else {
                    "declared_contract_candidate"
                },
                witness_edges: chain.clone(),
                trigger_paths: triggers.clone(),
            };
            candidates.insert(edge.consumer.clone(), candidate);
            queue.push_back((edge.consumer.clone(), chain, triggers));
        }
    }
    result.candidates = candidates.into_values().collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Basis, Relation};
    #[cfg(unix)]
    #[test]
    fn native_paths_never_match_a_lossy_selector() {
        use std::os::unix::ffi::OsStringExt;
        let native = PathBuf::from(std::ffi::OsString::from_vec(
            b"schemas/api\xff.yaml".to_vec(),
        ));
        let mut omitted = 0;
        assert!(textual_path(&native, &mut omitted).is_none());
        assert_eq!(
            textual_path(Path::new("schemas/api�.yaml"), &mut omitted).as_deref(),
            Some("schemas/api�.yaml")
        );
        assert_eq!(omitted, 1);
    }
    fn edge(provider: &str, consumer: &str) -> Relation {
        Relation {
            id: format!("{provider}/{consumer}"),
            provider: provider.into(),
            consumer: consumer.into(),
            kind: RelationKind::CargoPathDependency,
            basis: Basis::ObservedManifestDependency,
            provider_path: None,
            consumer_path: "Cargo.toml".into(),
            contract_id: None,
            contract_kind: None,
            package: Some(provider.into()),
            provider_package_fingerprint: None,
            locator: "dependencies/p".into(),
            source_file: "Cargo.toml".into(),
            source_repo: Some(consumer.into()),
            source_fingerprint: String::new(),
            validation: "test",
        }
    }
    #[test]
    fn traversal_exact_budget_first_witness_tie_and_partial_results() {
        let w = Workspace {
            id: "test".into(),
            fingerprint: String::new(),
            repositories: vec![],
            coverage: vec![],
            unavailable_provider_relations: vec![],
            relations: vec![
                edge("a", "b"),
                edge("a", "c"),
                edge("b", "d"),
                edge("c", "d"),
            ],
        };
        let result = || Impact {
            provider: "a".into(),
            base: String::new(),
            head: String::new(),
            depth: 3,
            changed_paths: vec![],
            non_utf8_paths_omitted: 0,
            candidates: vec![],
            traversal_truncated: false,
            eligible_edges_examined: 0,
            alternative_witnesses_omitted: 0,
            depth_limited_edges: 0,
        };
        let mut full = result();
        traverse(
            &w,
            &BTreeSet::new(),
            &BTreeSet::from(["lib.rs".into()]),
            &mut full,
            4,
        );
        assert!(!full.traversal_truncated);
        assert_eq!(full.candidates[2].witness_edges, vec![0, 2]);
        assert_eq!(full.alternative_witnesses_omitted, 1);
        let mut partial = result();
        traverse(
            &w,
            &BTreeSet::new(),
            &BTreeSet::from(["lib.rs".into()]),
            &mut partial,
            2,
        );
        assert!(partial.traversal_truncated);
        assert_eq!(partial.candidates.len(), 2);
        assert_eq!(partial.eligible_edges_examined, 2);
    }
}
