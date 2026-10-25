use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
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
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn write(root: &Path, p: &str, s: &str) {
    let p = root.join(p);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}
fn commit(root: &Path) -> String {
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "fixture"]);
    git(root, &["rev-parse", "HEAD"])
}
struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    manifest: PathBuf,
    value: Value,
    relocated: Option<PathBuf>,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root_dir = dir.path().to_path_buf();
        for name in ["provider", "consumer", "downstream", "client"] {
            let root = dir.path().join(name);
            fs::create_dir(&root).unwrap();
            git(&root, &["init", "-q"]);
            git(&root, &["config", "user.email", "fixture@example.test"]);
            git(&root, &["config", "user.name", "Fixture"]);
            write(&root, "src/lib.rs", "pub fn run() {}\n");
            write(
                &root,
                "Cargo.toml",
                &format!("[package]\nname = '{name}'\nversion = '0.1.0'\n"),
            );
        }
        write(
            &dir.path().join("consumer"),
            "Cargo.toml",
            "[package]\nname='consumer'\nversion='0.1.0'\n[dependencies]\naliased={path='../provider',package='provider'}\n",
        );
        write(
            &dir.path().join("downstream"),
            "Cargo.toml",
            "[package]\nname='downstream'\nversion='0.1.0'\n[dependencies]\nconsumer={path='../consumer'}\n",
        );
        fs::remove_file(dir.path().join("client/Cargo.toml")).unwrap();
        write(
            &dir.path().join("client"),
            "src/client.py",
            "# schema consumer\n",
        );
        write(
            &dir.path().join("provider"),
            "schemas/api.yaml",
            "api: v1\n",
        );
        for name in ["provider", "consumer", "downstream", "client"] {
            commit(&dir.path().join(name));
        }
        fs::create_dir(dir.path().join("nongit")).unwrap();
        let manifest = dir.path().join("workspace.json");
        let value = json!({"schema_version":1,"workspace_id":"sample","repositories": [
            {"id":"provider","root":"provider"},{"id":"consumer","root":"consumer"},
            {"id":"downstream","root":"downstream"},{"id":"client","root":"client"},{"id":"nongit","root":"nongit"}],
            "contracts":[{"id":"api","kind":"api_schema","provider":{"repo":"provider","path":"schemas/api.yaml"},
                "consumers":[{"repo":"client","path":"src/client.py"}]}]});
        let f = Self {
            _dir: dir,
            root: root_dir,
            manifest,
            value,
            relocated: None,
        };
        f.save();
        f
    }
    fn save(&self) {
        fs::write(&self.manifest, serde_json::to_vec(&self.value).unwrap()).unwrap();
    }
    fn root(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
    fn run(&self, args: &[&str]) -> Output {
        self.run_format(args, "json")
    }
    fn run_format(&self, args: &[&str], format: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ctx"))
            .args(["workspace"])
            .args(args)
            .arg("--manifest")
            .arg(&self.manifest)
            .args(["--format", format])
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn fail(&self, args: &[&str]) {
        let out = self.run(args);
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
    }
    fn relocate(&mut self) {
        let old = self.root.clone();
        let moved = old.with_file_name(format!(
            "{}-moved",
            old.file_name().unwrap().to_string_lossy()
        ));
        assert!(!moved.exists());
        fs::rename(&old, &moved).unwrap();
        self.root = moved.clone();
        self.manifest = moved.join("workspace.json");
        self.relocated = Some(moved);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(path) = &self.relocated {
            let _ = fs::remove_dir_all(path);
        }
    }
}
#[test]
fn registry_observed_alias_declared_consumers_and_determinism() {
    let mut f = Fixture::new();
    let a = f.ok(&["inspect"]);
    assert_eq!(a["workspace_schema_version"], 2);
    assert_eq!(a["relations"].as_array().unwrap().len(), 3, "{a:#}");
    let observed = a["relations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["provider"] == "provider" && e["consumer"] == "consumer")
        .unwrap();
    assert_eq!(observed["basis"], "observed_manifest_dependency");
    assert_eq!(observed["locator"], "dependencies/aliased");
    assert_eq!(observed["package"], "provider");
    assert!(
        !String::from_utf8(f.run(&["inspect"]).stdout)
            .unwrap()
            .contains(f._dir.path().to_str().unwrap())
    );
    let c = f.ok(&[
        "consumers",
        "--repo",
        "provider",
        "--path",
        "schemas/api.yaml",
    ]);
    assert_eq!(c["relations"].as_array().unwrap().len(), 1);
    assert_eq!(c["relations"][0]["basis"], "declared_workspace_mapping");
    assert_eq!(
        f.ok(&[
            "consumers",
            "--repo",
            "provider",
            "--path",
            "unrelated.yaml"
        ])["relations"],
        json!([])
    );
    assert_eq!(a, f.ok(&["inspect"]));
    f.value["repositories"].as_array_mut().unwrap().reverse();
    f.save();
    let mut b = f.ok(&["inspect"]);
    let mut a = a;
    // Fingerprints identify actual manifest bytes; declaration evidence retains it.
    for x in [&mut a, &mut b] {
        x.as_object_mut().unwrap().remove("manifest_fingerprint");
        for e in x["relations"].as_array_mut().unwrap() {
            if e["basis"] == "declared_workspace_mapping" {
                e.as_object_mut().unwrap().remove("source_fingerprint");
            }
        }
    }
    assert_eq!(a, b);
    let ng = b["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "nongit")
        .unwrap();
    assert_eq!(ng["git_known"], false);
}

#[test]
fn relation_ids_include_contract_kind_and_survive_reorder_and_relocation() {
    let mut f = Fixture::new();
    let relation_id = |value: &Value| {
        value["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["contract_id"] == "api")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let api_id = relation_id(&f.ok(&["inspect"]));

    f.value["contracts"][0]["kind"] = json!("event_schema");
    f.save();
    let event_id = relation_id(&f.ok(&["inspect"]));
    assert_ne!(api_id, event_id);

    f.value["repositories"].as_array_mut().unwrap().reverse();
    f.save();
    assert_eq!(event_id, relation_id(&f.ok(&["inspect"])));

    f.relocate();
    assert_eq!(event_id, relation_id(&f.ok(&["inspect"])));
}

#[test]
fn impact_retains_direct_contract_triggers_and_human_relationship_provenance() {
    let mut f = Fixture::new();
    f.value["contracts"][0]["consumers"] = json!([
        {"repo":"consumer","path":"src/api.rs"}
    ]);
    f.value["contracts"].as_array_mut().unwrap().push(json!({
        "id":"events",
        "kind":"event_schema",
        "provider":{"repo":"provider","path":"schemas/events.proto"},
        "consumers":[{"repo":"consumer","path":"src/events.rs"}]
    }));
    write(
        &f.root("provider"),
        "schemas/events.proto",
        "syntax = 'proto3';\n",
    );
    write(
        &f.root("consumer"),
        "src/api.rs",
        "// API declaration endpoint\n",
    );
    write(
        &f.root("consumer"),
        "src/events.rs",
        "// Event declaration endpoint\n",
    );
    commit(&f.root("provider"));
    commit(&f.root("consumer"));
    f.save();

    let provider = f.root("provider");
    let base = git(&provider, &["rev-parse", "HEAD"]);
    write(&provider, "schemas/api.yaml", "api: v2\n");
    write(
        &provider,
        "schemas/events.proto",
        "syntax = 'proto3';\nmessage E {}\n",
    );
    write(
        &provider,
        "schemas/generated.proto",
        "message Generated {}\n",
    );
    write(&provider, "src/lib.rs", "pub fn changed() {}\n");
    let head = commit(&provider);
    let range = format!("{base}..{head}");
    let response = f.ok(&["impact", "--repo", "provider", "--change", &range]);
    let impact = &response["impact"];
    let candidates = impact["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(candidate["repo"], "consumer");
    assert_eq!(candidate["reason"], "declared_contract_candidate");
    assert_eq!(candidate["direct_triggers"].as_array().unwrap().len(), 3);
    assert_eq!(candidate["direct_triggers_omitted"], 0);
    let direct = candidate["direct_triggers"].as_array().unwrap();
    assert_eq!(direct[0]["reason"], "declared_contract_candidate");
    assert_eq!(direct[1]["reason"], "declared_contract_candidate");
    assert_eq!(direct[2]["reason"], "coarse_package_candidate");
    let direct_ids = direct
        .iter()
        .map(|t| t["relation_id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    let relations = response["relations"].as_array().unwrap();
    for contract_id in ["api", "events"] {
        let relation = relations
            .iter()
            .find(|r| r["contract_id"] == contract_id)
            .unwrap();
        assert!(direct_ids.contains(relation["id"].as_str().unwrap()));
    }
    assert!(
        candidate["trigger_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| { p == "schemas/api.yaml" })
    );
    assert!(
        candidate["trigger_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| { p == "schemas/events.proto" })
    );
    assert!(
        impact["trigger_omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| {
                o["path"] == "schemas/generated.proto" && o["reason"] == "role_not_package_trigger"
            })
    );

    let human = String::from_utf8(
        f.run_format(&["consumers", "--repo", "provider"], "human")
            .stdout,
    )
    .unwrap();
    assert!(human.contains("consumer_path=src/api.rs"));
    assert!(human.contains("contract_kind=api_schema"));
    assert!(human.contains("contract_id=api"));
    assert!(human.contains("consumer_path=Cargo.toml"));
    assert!(human.contains("source=consumer/Cargo.toml"));
    assert!(human.contains("source_fingerprint="));
    assert!(human.contains("provider_fingerprint="));

    // Manifest ordering affects its byte fingerprint, not relationship identity
    // or the sorted direct-trigger evidence.
    f.value["repositories"].as_array_mut().unwrap().reverse();
    f.value["contracts"].as_array_mut().unwrap().reverse();
    f.save();
    let reordered = f.ok(&["impact", "--repo", "provider", "--change", &range]);
    assert_eq!(
        candidate["direct_triggers"],
        reordered["impact"]["candidates"][0]["direct_triggers"]
    );
    assert_eq!(
        candidate["trigger_paths"],
        reordered["impact"]["candidates"][0]["trigger_paths"]
    );
}
#[test]
fn committed_schema_rename_delete_dirty_consumer_and_unrelated_changes() {
    let f = Fixture::new();
    let p = f.root("provider");
    let base = git(&p, &["rev-parse", "HEAD"]);
    write(&p, "schemas/api.yaml", "api: v2\n");
    let head = commit(&p);
    write(&f.root("client"), "src/client.py", "# dirty consumer\n");
    let range = format!("{base}..{head}");
    let a = f.ok(&[
        "impact", "--repo", "provider", "--change", &range, "--depth", "3",
    ]);
    assert_eq!(a["impact"]["candidates"][0]["repo"], "client");
    assert_eq!(
        a["impact"]["candidates"][0]["reason"],
        "declared_contract_candidate"
    );
    assert_eq!(a["impact"]["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(
        a["repositories"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "client")
            .unwrap()["dirty"],
        true
    );
    // Dirty provider edits do not become committed triggers.
    write(&p, "src/lib.rs", "pub fn dirty() {}\n");
    assert_eq!(
        a["impact"],
        f.ok(&[
            "impact", "--repo", "provider", "--change", &range, "--depth", "3"
        ])["impact"]
    );
    git(&p, &["mv", "schemas/api.yaml", "schemas/renamed.yaml"]);
    let renamed = commit(&p);
    let a = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{head}..{renamed}"),
        "--depth",
        "2",
    ]);
    assert!(
        a["impact"]["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["repo"] == "client")
    );
    assert!(
        a["relations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["validation"] == "committed_provider_current_consumer_usage_unproven")
    );
    // Deleted exact endpoint is also eligible from its base side.
    git(&p, &["mv", "schemas/renamed.yaml", "schemas/api.yaml"]);
    let restored = commit(&p);
    fs::remove_file(p.join("schemas/api.yaml")).unwrap();
    let deleted = commit(&p);
    let a = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{restored}..{deleted}"),
    ]);
    assert_eq!(a["impact"]["candidates"][0]["repo"], "client");
    write(&p, "README.md", "documentation only\n");
    let unrelated = commit(&p);
    let a = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{deleted}..{unrelated}"),
    ]);
    assert_eq!(a["impact"]["candidates"], json!([]));
    f.fail(&["impact", "--repo", "nongit", "--change", "HEAD~1..HEAD"]);
    f.fail(&["impact", "--repo", "provider", "--change", "missing..HEAD"]);
}

#[test]
fn unmodeled_changes_are_reported_and_supported_triggers_still_match() {
    let mut f = Fixture::new();
    let provider = f.root("provider");
    let base = git(&provider, &["rev-parse", "HEAD"]);
    write(
        &provider,
        "schemas/generated.proto",
        "message Generated {}\n",
    );
    let head = commit(&provider);
    let result = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{base}..{head}"),
    ]);
    assert_eq!(result["impact"]["candidates"], json!([]));
    assert!(
        result["impact"]["trigger_omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["path"] == "schemas/generated.proto"
                && o["reason"] == "role_not_package_trigger")
    );

    let base = head;
    write(&provider, "src/lib.rs", "pub fn rust_change() {}\n");
    let head = commit(&provider);
    let rust_change = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{base}..{head}"),
    ]);
    assert_eq!(
        rust_change["impact"]["candidates"][0]["reason"],
        "coarse_package_candidate"
    );

    let base = head;
    write(&provider, "build.rs", "fn main() {}\n");
    let head = commit(&provider);
    let build_change = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{base}..{head}"),
    ]);
    assert_eq!(
        build_change["impact"]["candidates"][0]["reason"],
        "coarse_package_candidate"
    );

    // A declared .proto endpoint can still produce exact evidence even though
    // the same file class is outside coarse package-trigger coverage.
    f.value["contracts"].as_array_mut().unwrap().push(json!({
        "id":"generated-api",
        "kind":"api_schema",
        "provider":{"repo":"provider","path":"schemas/events.proto"},
        "consumers":[{"repo":"client","path":"src/events.py"}]
    }));
    write(&provider, "schemas/events.proto", "message Event {}\n");
    write(
        &f.root("client"),
        "src/events.py",
        "# generated contract consumer\n",
    );
    commit(&provider);
    commit(&f.root("client"));
    f.save();
    let base = git(&provider, &["rev-parse", "HEAD"]);
    write(
        &provider,
        "schemas/events.proto",
        "message Event { string id = 1; }\n",
    );
    let head = commit(&provider);
    let declared = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{base}..{head}"),
    ]);
    assert_eq!(declared["impact"]["candidates"][0]["repo"], "client");
    assert_eq!(
        declared["impact"]["candidates"][0]["reason"],
        "declared_contract_candidate"
    );
    assert!(
        declared["impact"]["trigger_omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["path"] == "schemas/events.proto"
                && o["reason"] == "role_not_package_trigger")
    );

    #[cfg(unix)]
    {
        let base = head;
        std::os::unix::fs::symlink("api.yaml", provider.join("schemas/link.proto")).unwrap();
        let head = commit(&provider);
        let non_regular = f.ok(&[
            "impact",
            "--repo",
            "provider",
            "--change",
            &format!("{base}..{head}"),
        ]);
        assert!(
            non_regular["impact"]["trigger_omissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["path"] == "schemas/link.proto" && o["reason"] == "non_regular")
        );
    }
}
#[test]
fn package_transitivity_cycles_depth_and_witnesses() {
    let f = Fixture::new();
    let p = f.root("provider");
    // Create cycle provider <- consumer <- downstream <- provider.
    write(
        &p,
        "Cargo.toml",
        "[package]\nname='provider'\nversion='0.1.0'\n[dependencies]\ndownstream={path='../downstream'}\n",
    );
    commit(&p);
    let base = git(&p, &["rev-parse", "HEAD"]);
    write(&p, "src/lib.rs", "pub fn changed() {}\n");
    let head = commit(&p);
    let range = format!("{base}..{head}");
    let a = f.ok(&[
        "impact", "--repo", "provider", "--change", &range, "--depth", "3",
    ]);
    let c = a["impact"]["candidates"].as_array().unwrap();
    assert_eq!(c.len(), 2);
    assert_eq!(c[0]["repo"], "consumer");
    assert_eq!(c[1]["distance"], 2);
    assert_eq!(c[1]["witness_edges"].as_array().unwrap().len(), 2);
    assert!(
        a["impact"]["alternative_witnesses_omitted"]
            .as_u64()
            .unwrap()
            > 0
    );
    let b = f.ok(&["impact", "--repo", "provider", "--change", &range]);
    assert_eq!(b["impact"]["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(b["impact"]["depth_limited_edges"], 1);
    f.fail(&[
        "impact", "--repo", "provider", "--change", &range, "--depth", "4",
    ]);
}
#[test]
fn strict_manifest_identity_selectors_and_limits() {
    let mut f = Fixture::new();
    let original = f.value.clone();
    for mutate in [
        json!({"schema_version":2}),
        json!({"extra":true}),
        json!({"workspace_id":"Bad"}),
        json!({"repositories":[{"id":"same","root":"provider"},{"id":"same","root":"consumer"}]}),
        json!({"repositories":[{"id":"a","root":"provider"},{"id":"b","root":"provider/../provider"}]}),
        json!({"repositories":[{"id":"a","root":"provider"},{"id":"b","root":"provider/src"}]}),
    ] {
        f.value = original.clone();
        for (k, v) in mutate.as_object().unwrap() {
            f.value[k] = v.clone();
        }
        f.save();
        f.fail(&["inspect"]);
    }
    for path in [
        "../api.yaml",
        "/etc/passwd",
        "schemas//api.yaml",
        "schemas/./api.yaml",
        "schemas/../schemas/api.yaml",
        "",
        "C:\\file",
    ] {
        f.value = original.clone();
        f.value["contracts"][0]["provider"]["path"] = json!(path);
        f.save();
        f.fail(&["inspect"]);
    }
    f.value = original.clone();
    f.value["contracts"][0]["consumers"][0]["repo"] = json!("unknown");
    f.save();
    f.fail(&["inspect"]);
    f.value = original.clone();
    f.value["contracts"][0]["consumers"]
        .as_array_mut()
        .unwrap()
        .push(original["contracts"][0]["consumers"][0].clone());
    f.save();
    f.fail(&["inspect"]);
    f.value = original;
    f.save();
    f.fail(&["consumers", "--repo", "unknown"]);
    let mut bytes = serde_json::to_vec(&f.value).unwrap();
    bytes.resize(1024 * 1024, b' ');
    fs::write(&f.manifest, &bytes).unwrap();
    f.ok(&["inspect"]);
    bytes.push(b' ');
    fs::write(&f.manifest, bytes).unwrap();
    f.fail(&["inspect"]);
}
#[test]
fn unsupported_cargo_shapes_missing_endpoints_and_escaping() {
    let mut f = Fixture::new();
    write(
        &f.root("consumer"),
        "Cargo.toml",
        "[package]\nname='consumer'\nversion='0.1.0'\n[dependencies]\nwrong={path='../provider'}\nexternal='1'\n'bad alias'={path='../provider',package='provider'}\ninherited={workspace=true}\n[dev-dependencies]\nprovider={path='../provider'}\n",
    );
    let a = f.ok(&["inspect"]);
    assert_eq!(a["relations"].as_array().unwrap().len(), 2);
    for reason in [
        "package_identity_mismatch_or_unavailable",
        "registry_or_unsupported_dependency",
        "invalid_dependency_key",
        "cargo_dependency_shape_unsupported",
        "cargo_scope_unsupported",
    ] {
        assert!(
            a["coverage"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["reason"] == reason),
            "{reason}"
        );
    }
    fs::remove_file(f.root("client").join("src/client.py")).unwrap();
    let a = f.ok(&["inspect"]);
    assert!(
        !a["relations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["consumer"] == "client")
    );
    write(&f.root("consumer"), "Cargo.toml", "not [ valid");
    let a = f.ok(&["inspect"]);
    assert!(
        a["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["reason"] == "invalid_cargo_toml")
    );
    write(&f.root("client"), "src/hostile\n\u{1b}.py", "# harmless\n");
    f.value["contracts"][0]["consumers"][0]["path"] = json!("src/hostile\n\u{1b}.py");
    f.save();
    let out = Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["workspace", "inspect", "--manifest"])
        .arg(&f.manifest)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!out.stdout.contains(&27));
}
#[cfg(unix)]
#[test]
fn symlink_files_parent_escapes_and_root_alias_policy() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new();
    let original = f.value.clone();
    symlink(
        f.root("provider").join("schemas/api.yaml"),
        f.root("provider").join("alias.yaml"),
    )
    .unwrap();
    symlink(
        f.root("client").join("src"),
        f.root("provider").join("escape"),
    )
    .unwrap();
    for path in ["alias.yaml", "escape/client.py"] {
        f.value = original.clone();
        f.value["contracts"][0]["provider"]["path"] = json!(path);
        f.save();
        f.fail(&["inspect"]);
    }
    symlink(f.root("provider"), f._dir.path().join("alias-root")).unwrap();
    f.value = original.clone();
    f.value["repositories"][0]["root"] = json!("alias-root");
    f.save();
    f.ok(&["inspect"]);
    f.value["repositories"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"duplicate","root":"provider"}));
    f.save();
    f.fail(&["inspect"]);
}
#[test]
fn logical_identity_survives_physical_relocation() {
    let mut f = Fixture::new();
    let a = f.ok(&["inspect"]);
    fs::rename(f.root("nongit"), f.root("relocated")).unwrap();
    f.value["repositories"][4]["root"] = json!("relocated");
    f.save();
    let b = f.ok(&["inspect"]);
    assert_eq!(a["repositories"], b["repositories"]);
    assert_eq!(a["workspace_id"], b["workspace_id"]);
}

#[test]
fn workspace_resource_ceilings_and_nested_git_scope() {
    let mut f = Fixture::new();
    let original = f.value.clone();
    f.value["repositories"] = json!(
        (0..17)
            .map(|i| json!({"id":format!("r{i}"),"root":"provider"}))
            .collect::<Vec<_>>()
    );
    f.save();
    f.fail(&["inspect"]);
    f.value = original.clone();
    f.value["contracts"] = json!(
        (0..257)
            .map(|i| {
                let mut c = original["contracts"][0].clone();
                c["id"] = json!(format!("c{i}"));
                c
            })
            .collect::<Vec<_>>()
    );
    f.save();
    f.fail(&["inspect"]);
    f.value = original.clone();
    f.value["contracts"][0]["consumers"] =
        json!(vec![original["contracts"][0]["consumers"][0].clone(); 1025]);
    f.save();
    f.fail(&["inspect"]);
    f.value = original.clone();
    f.save();
    let mut cargo = String::from("[package]\nname='consumer'\nversion='0.1.0'\n[dependencies]\n");
    for i in 0..1025 {
        cargo.push_str(&format!("dep{i}='1'\n"));
    }
    write(&f.root("consumer"), "Cargo.toml", &cargo);
    f.fail(&["inspect"]);
    // The selected registered root is nested inside its Git worktree.
    let p = f.root("provider");
    fs::create_dir(p.join("nested")).unwrap();
    for name in ["src", "schemas", "Cargo.toml"] {
        fs::rename(p.join(name), p.join("nested").join(name)).unwrap();
    }
    let base = commit(&p);
    f.value["repositories"][0]["root"] = json!("provider/nested");
    f.save();
    write(
        &f.root("consumer"),
        "Cargo.toml",
        "[package]\nname='consumer'\nversion='0.1.0'\n[dependencies]\nprovider={path='../provider/nested'}\n",
    );
    write(&p, "nested/schemas/api.yaml", "api: nested\n");
    let head = commit(&p);
    let a = f.ok(&[
        "impact",
        "--repo",
        "provider",
        "--change",
        &format!("{base}..{head}"),
    ]);
    assert_eq!(a["impact"]["candidates"][0]["repo"], "client");
    assert_eq!(a["impact"]["changed_paths"][0]["path"], "schemas/api.yaml");
}

#[test]
fn virtual_workspaces_oversize_cargo_and_json_schema_shape() {
    let f = Fixture::new();
    write(
        &f.root("provider"),
        "Cargo.toml",
        "[workspace]\nmembers=['nested']\n",
    );
    let a = f.ok(&["inspect"]);
    assert!(
        a["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["reason"] == "cargo_virtual_workspace_unsupported")
    );
    assert!(
        !a["relations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["provider"] == "provider" && r["basis"] == "observed_manifest_dependency")
    );
    fs::write(
        f.root("provider").join("Cargo.toml"),
        vec![b' '; 256 * 1024 + 1],
    )
    .unwrap();
    let a = f.ok(&["inspect"]);
    assert!(a["coverage"].as_array().unwrap().iter().any(
        |g| g["repo"] == "provider" && g["reason"] == "cargo_manifest_unavailable_or_oversize"
    ));
    let mut keys = a
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "command",
            "conclusion",
            "coverage",
            "impact",
            "manifest_fingerprint",
            "manifest_schema_version",
            "relations",
            "repositories",
            "selector_path",
            "selector_repo",
            "sort_order",
            "time_basis",
            "workspace_id",
            "workspace_schema_version"
        ]
    );
}

#[test]
fn explicit_root_package_with_workspace_is_observed_and_members_are_omitted() {
    let f = Fixture::new();
    write(
        &f.root("consumer"),
        "Cargo.toml",
        "[workspace]\nmembers=[]\n[package]\nname='consumer'\nversion='0.1.0'\n[dependencies]\nprovider={path='../provider'}\n",
    );
    let a = f.ok(&["inspect"]);
    assert!(a["relations"].as_array().unwrap().iter().any(|r| {
        r["provider"] == "provider"
            && r["consumer"] == "consumer"
            && r["basis"] == "observed_manifest_dependency"
    }));
    assert!(a["coverage"].as_array().unwrap().iter().any(|g| {
        g["repo"] == "consumer" && g["reason"] == "cargo_workspace_members_not_discovered"
    }));

    write(
        &f.root("consumer"),
        "Cargo.toml",
        "[package]\nname='consumer'\nversion='0.1.0'\n[dependencies]\nprovider={workspace=true}\n",
    );
    let a = f.ok(&["inspect"]);
    assert!(a["coverage"].as_array().unwrap().iter().any(|g| {
        g["repo"] == "consumer" && g["reason"] == "cargo_dependency_shape_unsupported"
    }));

    write(
        &f.root("consumer"),
        "Cargo.toml",
        "[workspace]\nmembers=[]\n[package]\nworkspace='..'\nversion.workspace=true\n",
    );
    let a = f.ok(&["inspect"]);
    assert!(
        a["coverage"].as_array().unwrap().iter().any(|g| {
            g["repo"] == "consumer" && g["reason"] == "cargo_package_identity_inherited"
        })
    );
}

#[test]
fn event_mapping_and_provider_cargo_symlink_are_honest_evidence() {
    let mut f = Fixture::new();
    f.value["contracts"][0]["kind"] = json!("event_schema");
    f.save();
    let a = f.ok(&["consumers", "--repo", "provider"]);
    assert!(
        a["relations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["contract_kind"] == "event_schema")
    );
    #[cfg(unix)]
    {
        fs::remove_file(f.root("provider").join("Cargo.toml")).unwrap();
        std::os::unix::fs::symlink(
            f.root("consumer").join("Cargo.toml"),
            f.root("provider").join("Cargo.toml"),
        )
        .unwrap();
        let a = f.ok(&["inspect"]);
        assert!(
            !a["relations"].as_array().unwrap().iter().any(|r| r["basis"]
                == "observed_manifest_dependency"
                && r["provider"] == "provider")
        );
        assert!(
            a["coverage"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["repo"] == "provider"
                    && r["reason"] == "cargo_manifest_unavailable_or_oversize")
        );
    }
}
