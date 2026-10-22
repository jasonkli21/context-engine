//! Invocation-local cross-repository evidence. Configuration is trusted local input;
//! logical IDs authorize selection, native canonical roots authorize file access.
use crate::{application::Result, git, source};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const MAX_MANIFEST: u64 = 1024 * 1024;
pub const MAX_REPOS: usize = 16;
pub const MAX_CONTRACTS: usize = 256;
pub const MAX_EDGES: usize = 1024;
pub const MAX_CARGO: u64 = 256 * 1024;
pub const MAX_GAPS: usize = 4096;
pub const MAX_SELECTOR: usize = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    workspace_id: String,
    repositories: Vec<Registration>,
    contracts: Vec<Contract>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    id: String,
    root: PathBuf,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    ApiSchema,
    EventSchema,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contract {
    id: String,
    kind: ContractKind,
    provider: Endpoint,
    consumers: Vec<Endpoint>,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub repo: String,
    pub path: String,
}

pub struct Workspace {
    pub id: String,
    pub fingerprint: String,
    pub repositories: Vec<Repository>,
    pub relations: Vec<Relation>,
    pub coverage: Vec<Gap>,
    pub(crate) unavailable_provider_relations: Vec<Relation>,
}
pub struct Repository {
    pub id: String,
    pub root: PathBuf,
    pub head: Option<String>,
    pub dirty: Option<bool>,
    pub git_known: bool,
}
#[derive(Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    CargoPathDependency,
    DeclaredContractConsumer,
}
#[derive(Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    ObservedManifestDependency,
    DeclaredWorkspaceMapping,
}
#[derive(Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Relation {
    pub id: String,
    pub provider: String,
    pub consumer: String,
    pub kind: RelationKind,
    pub basis: Basis,
    pub provider_path: Option<String>,
    pub consumer_path: String,
    pub contract_id: Option<String>,
    pub contract_kind: Option<ContractKind>,
    pub package: Option<String>,
    pub provider_package_fingerprint: Option<String>,
    pub locator: String,
    pub source_file: String,
    pub source_repo: Option<String>,
    pub source_fingerprint: String,
    pub validation: &'static str,
}
#[derive(Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gap {
    pub repo: String,
    pub locator: String,
    pub reason: &'static str,
}
impl Workspace {
    pub fn repo(&self, id: &str) -> Result<&Repository> {
        self.repositories
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| "unknown logical repository ID".into())
    }
    fn gap(&mut self, repo: &str, locator: &str, reason: &'static str) -> Result<()> {
        if self.coverage.len() >= MAX_GAPS {
            return Err("workspace coverage limit exceeded".into());
        }
        self.coverage.push(Gap {
            repo: repo.into(),
            locator: locator.into(),
            reason,
        });
        Ok(())
    }
    fn edge(&mut self, edge: Relation) -> Result<()> {
        if self.relations.len() >= MAX_EDGES {
            return Err("workspace relationship limit exceeded".into());
        }
        self.relations.push(edge);
        Ok(())
    }
}
fn id(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 64
        || !s.as_bytes()[0].is_ascii_lowercase()
        || !s
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return Err("workspace IDs must match [a-z][a-z0-9_-]{0,63}".into());
    }
    Ok(())
}
pub fn selector(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > MAX_SELECTOR
        || s.contains('\\')
        || s.contains(':')
        || s.split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.len() > 255)
    {
        return Err("invalid exact repository-relative file selector".into());
    }
    crate::application::validate_file(Path::new(s))
}
// Reject all endpoint symlinks, including in-root aliases, before evidence collection.
// Missing endpoints are usable coverage gaps, not malformed configuration.
fn preflight(root: &Path, path: &str) -> Result<()> {
    selector(path)?;
    let mut p = root.to_path_buf();
    for c in Path::new(path).components() {
        p.push(c);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err("workspace file selector contains a symlink".into());
            }
            Ok(_) => (),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        }
    }
    if let Ok(canonical) = p.canonicalize()
        && canonical != p
    {
        return Err("workspace selector normalizes to a different file identity".into());
    }
    Ok(())
}
fn available(root: &Path, path: &str) -> bool {
    source::open_checked(root, Path::new(path), u64::MAX).is_ok()
}

pub fn inspect(manifest: &Path) -> Result<Workspace> {
    let manifest = manifest.canonicalize()?;
    if !manifest.is_file() {
        return Err("workspace manifest must be a regular file".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&manifest)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("workspace manifest byte limit exceeded".into());
    }
    let m: Manifest = serde_json::from_slice(&bytes)?;
    if m.schema_version != 1 {
        return Err("unsupported workspace manifest schema_version".into());
    }
    id(&m.workspace_id)?;
    if m.repositories.is_empty()
        || m.repositories.len() > MAX_REPOS
        || m.contracts.len() > MAX_CONTRACTS
        || m.contracts.iter().map(|c| c.consumers.len()).sum::<usize>() > MAX_EDGES
    {
        return Err("workspace entry limit exceeded".into());
    }
    let mut w = Workspace {
        id: m.workspace_id,
        fingerprint: blake3::hash(&bytes).to_hex().to_string(),
        repositories: vec![],
        relations: vec![],
        coverage: vec![],
        unavailable_provider_relations: vec![],
    };
    let mut ids = BTreeSet::new();
    for r in m.repositories {
        id(&r.id)?;
        if !ids.insert(r.id.clone()) {
            return Err("duplicate repository ID".into());
        }
        if r.root.as_os_str().is_empty() || r.root.as_os_str().len() > MAX_SELECTOR {
            return Err("invalid root path".into());
        }
        let root = manifest.parent().unwrap().join(r.root).canonicalize()?;
        if !root.is_dir() {
            return Err("registered root must be a directory".into());
        }
        if w.repositories
            .iter()
            .any(|r| root.starts_with(&r.root) || r.root.starts_with(&root))
        {
            return Err("duplicate or overlapping canonical repository roots".into());
        }
        w.repositories.push(Repository {
            id: r.id,
            root,
            head: None,
            dirty: None,
            git_known: false,
        });
    }
    w.repositories.sort_by(|a, b| a.id.cmp(&b.id));
    let mut contracts = BTreeSet::new();
    let mut mappings = BTreeSet::new();
    for c in &m.contracts {
        id(&c.id)?;
        if !contracts.insert(&c.id) || c.consumers.is_empty() {
            return Err("duplicate contract ID or empty consumers".into());
        }
        for e in std::iter::once(&c.provider).chain(&c.consumers) {
            preflight(&w.repo(&e.repo)?.root, &e.path)?;
        }
        for e in &c.consumers {
            if e.repo == c.provider.repo
                || !mappings.insert((c.kind.clone(), c.provider.clone(), e.clone()))
            {
                return Err("duplicate or same-repository contract mapping".into());
            }
        }
    }
    // All manifest identity and selector checks precede Git/Cargo work.
    for i in 0..w.repositories.len() {
        let (metadata, warnings) = git::probe(&w.repositories[i].root);
        if let Some(meta) = metadata {
            w.repositories[i].git_known = true;
            w.repositories[i].head = meta.head_commit;
            w.repositories[i].dirty = meta.is_dirty;
        }
        if !warnings.is_empty() {
            let name = w.repositories[i].id.clone();
            w.gap(&name, "git", "git_metadata_incomplete")?;
        }
    }
    cargo_edges(&mut w)?;
    for c in m.contracts {
        let locator = format!("contracts/{}", c.id);
        let provider = available(&w.repo(&c.provider.repo)?.root, &c.provider.path);
        if !provider {
            w.gap(&c.provider.repo, &locator, "provider_file_unavailable")?;
        }
        for e in c.consumers {
            let consumer = available(&w.repo(&e.repo)?.root, &e.path);
            if !consumer {
                w.gap(&e.repo, &locator, "consumer_file_unavailable")?;
            }
            if consumer {
                let edge = Relation {
                    id: String::new(),
                    provider: c.provider.repo.clone(),
                    consumer: e.repo,
                    kind: RelationKind::DeclaredContractConsumer,
                    basis: Basis::DeclaredWorkspaceMapping,
                    provider_path: Some(c.provider.path.clone()),
                    consumer_path: e.path,
                    contract_id: Some(c.id.clone()),
                    contract_kind: Some(c.kind.clone()),
                    package: None,
                    provider_package_fingerprint: None,
                    locator: locator.clone(),
                    source_file: "workspace_manifest".into(),
                    source_repo: None,
                    source_fingerprint: w.fingerprint.clone(),
                    validation: "endpoints_readable_usage_unproven",
                };
                if provider {
                    w.edge(edge)?;
                } else {
                    w.unavailable_provider_relations.push(edge);
                }
            }
        }
    }
    for edge in w
        .relations
        .iter_mut()
        .chain(&mut w.unavailable_provider_relations)
    {
        let key = serde_json::to_vec(&(
            &edge.provider,
            &edge.consumer,
            &edge.kind,
            &edge.provider_path,
            &edge.consumer_path,
            &edge.contract_id,
            &edge.locator,
        ))?;
        edge.id = blake3::hash(&key).to_hex().to_string();
    }
    w.relations.sort_by(relation_order);
    w.coverage.sort();
    w.coverage.dedup();
    Ok(w)
}
struct Package {
    name: String,
    document: toml::Value,
    fingerprint: String,
}
fn cargo_edges(w: &mut Workspace) -> Result<()> {
    let mut packages = BTreeMap::new();
    for i in 0..w.repositories.len() {
        let repo = &w.repositories[i];
        let name = repo.id.clone();
        let data = match source::load(&repo.root, Path::new("Cargo.toml"), MAX_CARGO) {
            Ok(data) => data,
            Err(_) => {
                w.gap(
                    &name,
                    "Cargo.toml",
                    "cargo_manifest_unavailable_or_oversize",
                )?;
                continue;
            }
        };
        let Ok(document) = toml::from_str::<toml::Value>(&data) else {
            w.gap(&name, "Cargo.toml", "invalid_cargo_toml")?;
            continue;
        };
        if document.get("workspace").is_some()
            || document
                .get("package")
                .and_then(|p| p.get("workspace"))
                .is_some()
        {
            w.gap(&name, "Cargo.toml", "cargo_workspace_unsupported")?;
            continue;
        }
        let Some(package) = document
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
        else {
            w.gap(&name, "Cargo.toml", "root_package_identity_unavailable")?;
            continue;
        };
        if !package_name(package) {
            w.gap(&name, "Cargo.toml", "invalid_package_identity")?;
            continue;
        }
        packages.insert(
            name,
            Package {
                name: package.into(),
                document,
                fingerprint: blake3::hash(data.as_bytes()).to_hex().to_string(),
            },
        );
    }
    let mut examined = 0;
    for (consumer, package) in &packages {
        let doc = &package.document;
        for scope in [
            "target",
            "dev-dependencies",
            "dev_dependencies",
            "build-dependencies",
            "build_dependencies",
            "patch",
            "replace",
        ] {
            if doc.get(scope).is_some() {
                w.gap(consumer, scope, "cargo_scope_unsupported")?;
            }
        }
        let Some(deps) = doc.get("dependencies") else {
            continue;
        };
        let Some(deps) = deps.as_table() else {
            w.gap(consumer, "dependencies", "invalid_dependencies_table")?;
            continue;
        };
        for (key, dep) in deps {
            examined += 1;
            if examined > MAX_EDGES {
                return Err("workspace Cargo dependency check limit exceeded".into());
            }
            let locator = format!("dependencies/{key}");
            if key.len() > 256 {
                return Err("Cargo dependency key limit exceeded".into());
            }
            if !package_name(key) {
                w.gap(consumer, &locator, "invalid_dependency_key")?;
                continue;
            }
            let Some(t) = dep.as_table() else {
                w.gap(consumer, &locator, "registry_or_unsupported_dependency")?;
                continue;
            };
            let supported = [
                "path",
                "package",
                "version",
                "features",
                "optional",
                "default-features",
                "default_features",
            ];
            if t.keys().any(|k| !supported.contains(&k.as_str())) {
                w.gap(consumer, &locator, "cargo_dependency_shape_unsupported")?;
                continue;
            }
            if t.iter().any(|(k, v)| match k.as_str() {
                "optional" | "default-features" | "default_features" => !v.is_bool(),
                "features" => !v.as_array().is_some_and(|a| a.iter().all(|v| v.is_str())),
                _ => !v.is_str(),
            }) {
                w.gap(consumer, &locator, "invalid_dependency_value")?;
                continue;
            }
            let Some(path) = t.get("path").and_then(|v| v.as_str()) else {
                w.gap(consumer, &locator, "path_dependency_unavailable")?;
                continue;
            };
            if path.len() > MAX_SELECTOR {
                return Err("Cargo dependency path limit exceeded".into());
            }
            let root = &w.repo(consumer)?.root;
            let Ok(target) = root.join(path).canonicalize() else {
                w.gap(consumer, &locator, "dependency_path_unavailable")?;
                continue;
            };
            let Some(provider) = w
                .repositories
                .iter()
                .find(|r| r.root == target)
                .map(|r| r.id.clone())
            else {
                w.gap(consumer, &locator, "dependency_root_unregistered")?;
                continue;
            };
            let expected = match t.get("package") {
                Some(v) => v.as_str(),
                None => Some(key.as_str()),
            };
            if provider == *consumer
                || !packages
                    .get(&provider)
                    .is_some_and(|p| Some(p.name.as_str()) == expected)
            {
                w.gap(
                    consumer,
                    &locator,
                    "package_identity_mismatch_or_unavailable",
                )?;
                continue;
            }
            w.edge(Relation {
                id: String::new(),
                provider: provider.clone(),
                consumer: consumer.clone(),
                kind: RelationKind::CargoPathDependency,
                basis: Basis::ObservedManifestDependency,
                provider_path: None,
                consumer_path: "Cargo.toml".into(),
                contract_id: None,
                contract_kind: None,
                package: expected.map(str::to_owned),
                provider_package_fingerprint: packages
                    .get(&provider)
                    .map(|p| p.fingerprint.clone()),
                locator,
                source_file: "Cargo.toml".into(),
                source_repo: Some(consumer.clone()),
                source_fingerprint: package.fingerprint.clone(),
                validation: "exact_registered_root_and_package_name",
            })?;
        }
    }
    Ok(())
}
fn package_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub(crate) fn relation_order(a: &Relation, b: &Relation) -> std::cmp::Ordering {
    (
        &a.provider,
        &a.consumer,
        &a.kind,
        &a.provider_path,
        &a.consumer_path,
        &a.contract_id,
        &a.locator,
    )
        .cmp(&(
            &b.provider,
            &b.consumer,
            &b.kind,
            &b.provider_path,
            &b.consumer_path,
            &b.contract_id,
            &b.locator,
        ))
}
