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
pub const MAX_DIRECT_TRIGGERS_PER_CANDIDATE: usize = 64;
#[derive(Serialize)]
pub struct Impact {
    pub provider: String,
    pub base: String,
    pub head: String,
    pub depth: u8,
    pub changed_paths: Vec<ChangedPath>,
    pub trigger_omissions: Vec<TriggerOmission>,
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
pub struct TriggerOmission {
    pub path: Option<String>,
    pub side: &'static str,
    pub reason: &'static str,
}
#[derive(Serialize)]
pub struct DirectTrigger {
    pub relation_id: String,
    pub reason: &'static str,
}
#[derive(Serialize)]
pub struct Candidate {
    pub repo: String,
    pub distance: usize,
    pub reason: &'static str,
    pub witness_edges: Vec<usize>,
    pub direct_triggers: Vec<DirectTrigger>,
    pub direct_triggers_omitted: usize,
    pub trigger_paths: Vec<String>,
    #[serde(skip)]
    pub(crate) seen_direct_trigger_ids: BTreeSet<String>,
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
    let mut trigger_omissions = vec![];
    let mut native_omissions = 0;
    for c in changes {
        for (side, entry) in [("base", c.before), ("head", c.after)] {
            if let Some(e) = entry {
                let Some(p) = textual_path(&e.path, &mut native_omissions) else {
                    trigger_omissions.push(TriggerOmission {
                        path: None,
                        side,
                        reason: "non_utf8_omitted",
                    });
                    continue;
                };
                if e.regular() {
                    eligible.insert(e.path.clone());
                    if package_trigger(&e.path) {
                        package_paths.insert(p.clone());
                    } else {
                        trigger_omissions.push(TriggerOmission {
                            path: Some(p.clone()),
                            side,
                            reason: "role_not_package_trigger",
                        });
                    }
                } else {
                    trigger_omissions.push(TriggerOmission {
                        path: Some(p.clone()),
                        side,
                        reason: "non_regular",
                    });
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
    trigger_omissions.sort_by(|a, b| (&a.path, a.side, a.reason).cmp(&(&b.path, b.side, b.reason)));
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
        trigger_omissions,
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
        let mut edges = w
            .relations
            .iter()
            .enumerate()
            .filter(|(_, e)| e.provider == provider)
            .collect::<Vec<_>>();
        if witness.is_empty() {
            // Visit exact declared triggers before coarse package triggers so
            // the representative first-hop explanation keeps the strongest fact.
            edges.sort_by(|(left_index, left), (right_index, right)| {
                let priority = |e: &workspace::Relation| match e.kind {
                    RelationKind::DeclaredContractConsumer => 0,
                    RelationKind::CargoPathDependency => 1,
                };
                (priority(left), left_index).cmp(&(priority(right), right_index))
            });
        }
        for (i, edge) in edges {
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
            let mut chain = witness.clone();
            chain.push(i);
            if witness.is_empty() {
                let reason = if edge.kind == RelationKind::DeclaredContractConsumer {
                    "declared_contract_candidate"
                } else {
                    "coarse_package_candidate"
                };
                let candidate =
                    candidates
                        .entry(edge.consumer.clone())
                        .or_insert_with(|| Candidate {
                            repo: edge.consumer.clone(),
                            distance: chain.len(),
                            reason,
                            witness_edges: chain.clone(),
                            direct_triggers: vec![],
                            direct_triggers_omitted: 0,
                            trigger_paths: vec![],
                            seen_direct_trigger_ids: BTreeSet::new(),
                        });
                if candidate.seen_direct_trigger_ids.insert(edge.id.clone()) {
                    candidate.direct_triggers.push(DirectTrigger {
                        relation_id: edge.id.clone(),
                        reason,
                    });
                    candidate.direct_triggers.sort_by(|a, b| {
                        (reason_priority(a.reason), &a.relation_id)
                            .cmp(&(reason_priority(b.reason), &b.relation_id))
                    });
                    if candidate.direct_triggers.len() > MAX_DIRECT_TRIGGERS_PER_CANDIDATE {
                        candidate.direct_triggers.pop();
                        candidate.direct_triggers_omitted += 1;
                    }
                    if edge.kind == RelationKind::CargoPathDependency {
                        for path in package_paths {
                            add_trigger_path(candidate, path.clone());
                        }
                    } else if let Some(path) = &edge.provider_path {
                        add_trigger_path(candidate, path.clone());
                    }
                }
                if summary_priority(reason) < summary_priority(candidate.reason)
                    || (summary_priority(reason) == summary_priority(candidate.reason)
                        && chain < candidate.witness_edges)
                {
                    candidate.distance = chain.len();
                    candidate.reason = reason;
                    candidate.witness_edges = chain.clone();
                }
                if seen.insert(edge.consumer.clone()) {
                    let representative_triggers = if edge.kind == RelationKind::CargoPathDependency
                    {
                        package_paths.iter().cloned().collect()
                    } else {
                        edge.provider_path.iter().cloned().collect()
                    };
                    queue.push_back((edge.consumer.clone(), chain, representative_triggers));
                }
                continue;
            }
            if !seen.insert(edge.consumer.clone()) {
                result.alternative_witnesses_omitted += 1;
                continue;
            }
            let candidate = Candidate {
                repo: edge.consumer.clone(),
                distance: chain.len(),
                reason: "transitive_package_candidate",
                witness_edges: chain.clone(),
                direct_triggers: vec![],
                direct_triggers_omitted: 0,
                trigger_paths: triggers.clone(),
                seen_direct_trigger_ids: BTreeSet::new(),
            };
            candidates.insert(edge.consumer.clone(), candidate);
            queue.push_back((edge.consumer.clone(), chain, triggers.clone()));
        }
    }
    result.candidates = candidates.into_values().collect();
}
fn reason_priority(reason: &str) -> u8 {
    match reason {
        "declared_contract_candidate" => 0,
        "coarse_package_candidate" => 1,
        "transitive_package_candidate" => 2,
        _ => 3,
    }
}
fn summary_priority(reason: &str) -> u8 {
    reason_priority(reason)
}
fn add_trigger_path(candidate: &mut Candidate, path: String) {
    match candidate.trigger_paths.binary_search(&path) {
        Ok(_) => (),
        Err(index) => candidate.trigger_paths.insert(index, path),
    }
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
            trigger_omissions: vec![],
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

    #[test]
    fn direct_trigger_cap_prioritizes_exact_contracts_and_counts_omissions() {
        let mut relations = (0..70)
            .map(|i| {
                let mut e = edge("a", "b");
                e.id = format!("coarse-{i:03}");
                e
            })
            .collect::<Vec<_>>();
        for (id, path) in [
            ("contract-a", "schemas/a.proto"),
            ("contract-b", "schemas/b.proto"),
        ] {
            let mut e = edge("a", "b");
            e.id = id.into();
            e.kind = RelationKind::DeclaredContractConsumer;
            e.basis = Basis::DeclaredWorkspaceMapping;
            e.provider_path = Some(path.into());
            e.contract_id = Some(id.into());
            e.contract_kind = Some(crate::workspace::ContractKind::EventSchema);
            relations.push(e);
        }
        let w = Workspace {
            id: "test".into(),
            fingerprint: String::new(),
            repositories: vec![],
            relations,
            coverage: vec![],
            unavailable_provider_relations: vec![],
        };
        let mut result = Impact {
            provider: "a".into(),
            base: String::new(),
            head: String::new(),
            depth: 1,
            changed_paths: vec![],
            trigger_omissions: vec![],
            non_utf8_paths_omitted: 0,
            candidates: vec![],
            traversal_truncated: false,
            eligible_edges_examined: 0,
            alternative_witnesses_omitted: 0,
            depth_limited_edges: 0,
        };
        traverse(
            &w,
            &BTreeSet::from([
                PathBuf::from("schemas/a.proto"),
                PathBuf::from("schemas/b.proto"),
            ]),
            &BTreeSet::from(["src/lib.rs".into()]),
            &mut result,
            MAX_VISITS,
        );
        let candidate = &result.candidates[0];
        assert_eq!(candidate.reason, "declared_contract_candidate");
        assert_eq!(
            candidate.direct_triggers.len(),
            MAX_DIRECT_TRIGGERS_PER_CANDIDATE
        );
        assert_eq!(candidate.direct_triggers_omitted, 8);
        assert!(
            candidate
                .direct_triggers
                .iter()
                .any(|t| t.relation_id == "contract-a")
        );
        assert!(
            candidate
                .direct_triggers
                .iter()
                .any(|t| t.relation_id == "contract-b")
        );
        assert!(candidate.trigger_paths.contains(&"schemas/a.proto".into()));
        assert!(candidate.trigger_paths.contains(&"schemas/b.proto".into()));
        assert_eq!(result.alternative_witnesses_omitted, 0);
    }
}
