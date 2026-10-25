//! Pure workspace output: no filesystem or Git access, no absolute root disclosure.
use crate::{
    application::Result,
    workspace::{Relation, Workspace},
    workspace_impact::Impact,
};
use serde::Serialize;
#[derive(Serialize)]
pub struct Repository<'a> {
    id: &'a str,
    head: &'a Option<String>,
    dirty: Option<bool>,
    git_known: bool,
    observation: &'static str,
}
#[derive(Serialize)]
pub struct Document<'a> {
    workspace_schema_version: u32,
    manifest_schema_version: u32,
    command: &'a str,
    workspace_id: &'a str,
    manifest_fingerprint: &'a str,
    time_basis: &'static str,
    conclusion: &'static str,
    sort_order: &'static str,
    repositories: Vec<Repository<'a>>,
    relations: Vec<&'a Relation>,
    coverage: &'a [crate::workspace::Gap],
    impact: Option<&'a Impact>,
    selector_repo: Option<&'a str>,
    selector_path: Option<&'a str>,
}
pub fn document<'a>(
    w: &'a Workspace,
    command: &'a str,
    repo: Option<&'a str>,
    path: Option<&'a str>,
    impact: Option<&'a Impact>,
) -> Result<Document<'a>> {
    if let Some(repo) = repo {
        w.repo(repo)?;
    }
    if let Some(path) = path {
        crate::workspace::selector(path)?;
    }
    Ok(Document {
        workspace_schema_version: 2,
        manifest_schema_version: 1,
        command,
        workspace_id: &w.id,
        manifest_fingerprint: &w.fingerprint,
        time_basis: if impact.is_some() {
            "committed_provider_range_with_current_working_tree_relationships"
        } else {
            "current_working_tree_best_effort"
        },
        conclusion: "candidate_consumers_only_no_proven_breakage_or_completeness",
        sort_order: "repositories_by_id_relations_by_provider_consumer_kind_paths_breadth_first_representative_witness_direct_triggers_by_contract_priority_and_relation_id",
        repositories: w
            .repositories
            .iter()
            .map(|r| Repository {
                id: &r.id,
                head: &r.head,
                dirty: r.dirty,
                git_known: r.git_known,
                observation: "best_effort_non_atomic",
            })
            .collect(),
        relations: w
            .relations
            .iter()
            .filter(|e| {
                command != "consumers"
                    || (repo == Some(e.provider.as_str())
                        && path.is_none_or(|p| e.provider_path.as_deref() == Some(p)))
            })
            .collect(),
        coverage: &w.coverage,
        impact,
        selector_repo: repo,
        selector_path: path,
    })
}
pub fn json(d: &Document<'_>) -> Result<String> {
    Ok(crate::output::bounded_json(d, 8 * 1024 * 1024)?)
}
pub fn human(d: &Document<'_>) -> Result<String> {
    use std::fmt::Write;
    // Count serialized escaping before materializing either output representation.
    let _ = json(d)?;
    let esc = crate::output::escape_controls;
    let mut out = format!(
        "Workspace {} — {}\nCandidate evidence only; usage and breakage unproven.\nTime basis: {}\n",
        esc(d.workspace_id),
        d.command,
        d.time_basis
    );
    for r in &d.repositories {
        writeln!(
            out,
            "repo {} head={} dirty={:?} (best effort)",
            esc(r.id),
            r.head.as_deref().unwrap_or("unknown"),
            r.dirty
        )?;
    }
    for (i, e) in d.relations.iter().enumerate() {
        let basis = match e.basis {
            crate::workspace::Basis::ObservedManifestDependency => "observed Cargo path/package",
            crate::workspace::Basis::DeclaredWorkspaceMapping => {
                "declared mapping; runtime use unproven"
            }
        };
        let kind = match e.kind {
            crate::workspace::RelationKind::CargoPathDependency => "cargo_path_dependency",
            crate::workspace::RelationKind::DeclaredContractConsumer => {
                "declared_contract_consumer"
            }
        };
        let contract_kind = match e.contract_kind.as_ref() {
            Some(crate::workspace::ContractKind::ApiSchema) => "api_schema",
            Some(crate::workspace::ContractKind::EventSchema) => "event_schema",
            None => "<none>",
        };
        let source_repo = e.source_repo.as_deref().unwrap_or("<workspace>");
        let provider_fingerprint = e
            .provider_package_fingerprint
            .as_deref()
            .unwrap_or("<none>");
        writeln!(
            out,
            "edge {i}: id={} {} -> {} kind={} [{}] provider_path={} consumer_path={} contract_kind={} contract_id={} locator={} source={}/{} source_fingerprint={} provider_fingerprint={} validation={}",
            esc(&e.id),
            esc(&e.provider),
            esc(&e.consumer),
            kind,
            basis,
            esc(e.provider_path.as_deref().unwrap_or("<package>")),
            esc(&e.consumer_path),
            contract_kind,
            esc(e.contract_id.as_deref().unwrap_or("<none>")),
            esc(&e.locator),
            esc(source_repo),
            esc(&e.source_file),
            esc(&e.source_fingerprint),
            esc(provider_fingerprint),
            e.validation
        )?;
    }
    if let Some(i) = d.impact {
        writeln!(out, "committed {}..{} depth={}", i.base, i.head, i.depth)?;
        for c in &i.candidates {
            writeln!(
                out,
                "candidate {}: {} distance={} witness={:?} direct_triggers={:?} direct_triggers_omitted={} trigger_paths={:?}",
                esc(&c.repo),
                c.reason,
                c.distance,
                c.witness_edges,
                c.direct_triggers
                    .iter()
                    .map(|t| format!("{}@{}", t.reason, t.relation_id))
                    .collect::<Vec<_>>(),
                c.direct_triggers_omitted,
                c.trigger_paths.iter().map(|p| esc(p)).collect::<Vec<_>>()
            )?;
        }
        for omission in &i.trigger_omissions {
            writeln!(
                out,
                "trigger coverage {} side={}: {}",
                omission
                    .path
                    .as_deref()
                    .map(esc)
                    .unwrap_or_else(|| "<non-utf8-path>".into()),
                omission.side,
                omission.reason
            )?;
        }
        writeln!(
            out,
            "limits: traversal_truncated={} depth_limited_edges={} alternate_witnesses_omitted={} non_utf8_paths_omitted={}",
            i.traversal_truncated,
            i.depth_limited_edges,
            i.alternative_witnesses_omitted,
            i.non_utf8_paths_omitted
        )?;
    }
    for g in d.coverage {
        writeln!(
            out,
            "coverage {} {}: {}",
            esc(&g.repo),
            esc(&g.locator),
            g.reason
        )?;
    }
    out.push_str("Zero candidates means no supported match within coverage.\n");
    if out.len() > 8 * 1024 * 1024 {
        return Err("workspace human output limit exceeded".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escaped_large_trigger_evidence_fails_before_output_materialization() {
        let w = Workspace {
            id: "fixture".into(),
            fingerprint: String::new(),
            repositories: vec![],
            relations: vec![],
            coverage: vec![],
            unavailable_provider_relations: vec![],
        };
        let mut impact = Impact {
            provider: "p".into(),
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
        // Trigger path bytes can repeat across candidates even though the Git path
        // input itself is bounded. Count complete escaped output independently.
        for name in ["a", "b"] {
            impact.candidates.push(crate::workspace_impact::Candidate {
                repo: name.into(),
                distance: 1,
                reason: "coarse_package_candidate",
                witness_edges: vec![0],
                direct_triggers: vec![],
                direct_triggers_omitted: 0,
                trigger_paths: vec!["x".repeat(1024); 4096],
                seen_direct_trigger_ids: std::collections::BTreeSet::new(),
            });
        }
        let d = document(&w, "impact", None, None, Some(&impact)).unwrap();
        assert!(json(&d).is_err());
        assert!(human(&d).is_err());
    }
}
