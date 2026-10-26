//! Production subprocess conformance through a separate Python JSON/schema consumer.
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
struct Repo {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    cache: PathBuf,
}
impl Repo {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        fs::create_dir(&root).unwrap();
        let cache = tmp.path().join("cache");
        Self {
            _tmp: tmp,
            root,
            cache,
        }
    }
    fn write(&self, name: &str, text: &[u8]) {
        let p = self.root.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn init(&self) {
        self.git(&["init", "-q"]);
        self.git(&["config", "user.name", "Fixture"]);
        self.git(&["config", "user.email", "fixture@example.invalid"]);
    }
    fn commit(&self) -> String {
        self.git(&["add", "."]);
        self.git(&["commit", "-qm", "fixture"]);
        self.git(&["rev-parse", "HEAD"])
    }
    fn run(&self, feed: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ctx"))
            .args(["evidence", feed])
            .args(args)
            .arg(&self.root)
            .args(["--repository-id", "fixture", "--format", "json"])
            .env("CONTEXT_ENGINE_CACHE_DIR", &self.cache)
            .output()
            .unwrap()
    }
    fn ok(&self, feed: &str, args: &[&str]) -> Value {
        let o = self.run(feed, args);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert!(
            valid(&o.stdout),
            "consumer rejected {}",
            String::from_utf8_lossy(&o.stdout)
        );
        let text = String::from_utf8(o.stdout.clone()).unwrap();
        assert!(!text.contains(self.root.to_str().unwrap()));
        serde_json::from_slice(&o.stdout).unwrap()
    }
}
fn valid(bytes: &[u8]) -> bool {
    let mut p = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/evidence/validate.py"
        ))
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    // Invalid oversized documents may close stdin early.
    let _ = p.stdin.take().unwrap().write_all(bytes);
    p.wait().unwrap().success()
}
fn bad(o: Output) {
    assert!(!o.status.success());
    assert!(o.stdout.is_empty());
}
#[test]
fn rust_python_inventory_uncertainty_and_body_privacy() {
    let r = Repo::new();
    r.write("a.rs", b"use other::Thing;\nfn target() {}\nfn caller() { target(); target(); }\n// PRIVATE_BODY_SENTINEL\n");
    r.write(
        "b.py",
        b"import external\ndef target():\n    pass\ndef caller():\n    target()\n",
    );
    r.write("bad.py", b"def damaged(:\n");
    r.write("invalid.rs", &[0xff, 0xfe]);
    r.write("notes.md", b"PRIVATE_BODY_SENTINEL");
    r.write("generated/g.rs", b"fn generated() {}\n");
    let d = r.ok("structure", &[]);
    assert_eq!(d["observation"]["git_head"], Value::Null);
    assert_eq!(d["coverage"]["counts"]["files_observed"], 6);
    assert_eq!(d["coverage"]["counts"]["analysis_invalid_utf8"], 1);
    assert!(d["coverage"]["counts"]["syntax_errors"].as_u64().unwrap() > 0);
    let rs = d["data"]["relations"].as_array().unwrap();
    assert!(
        rs.iter()
            .any(|r| r["resolution"] == "exact_same_file_qualified_name")
    );
    assert!(
        rs.iter()
            .filter(|r| r["unresolved_name"].is_string())
            .count()
            >= 2
    );
    assert_eq!(
        rs.iter()
            .filter(|r| r["unresolved_name"] == "other::Thing")
            .count(),
        1
    );
    assert!(!d.to_string().contains("PRIVATE_BODY_SENTINEL"));
    let cached = r.ok("structure", &[]);
    assert_ne!(d["observation"]["scope"], cached["observation"]["scope"]);
    assert_eq!(d["coverage"], cached["coverage"]);
    assert_eq!(
        d["data"]["files"].as_array().unwrap().len(),
        cached["data"]["files"].as_array().unwrap().len()
    );
}
#[test]
fn candidates_share_ranker_scores_order_and_exclusions_without_bodies() {
    let r = Repo::new();
    r.write(
        "target.py",
        b"def target():\n    return 'PRIVATE_BODY_SENTINEL'\n",
    );
    r.write("other.py", b"def target_extra():\n    pass\n");
    r.write("credentials.py", b"def target():\n    pass\n");
    let d = r.ok("candidates", &["--task", "target", "--limit", "50"]);
    let o = Command::new(env!("CARGO_BIN_EXE_ctx"))
        .arg("context")
        .arg(&r.root)
        .args(["--task", "target", "--budget", "100000", "--format", "json"])
        .env("CONTEXT_ENGINE_CACHE_DIR", &r.cache)
        .output()
        .unwrap();
    assert!(o.status.success());
    let old: Value = serde_json::from_slice(&o.stdout).unwrap();
    let selected = old["data"]["selected"].as_array().unwrap();
    let candidates = d["data"]["candidates"].as_array().unwrap();
    assert_eq!(selected.len(), candidates.len());
    for (a, b) in candidates.iter().zip(selected) {
        assert_eq!(a["file"]["path"], b["path"]);
        assert_eq!(a["score"], b["score"]);
        assert_eq!(a["reasons"], b["reasons"]);
    }
    assert_eq!(d["coverage"]["counts"]["sensitive_files_omitted"], 1);
    assert!(!d.to_string().contains("PRIVATE_BODY_SENTINEL"));
    assert!(!d.to_string().contains("\"task\":"));
    let limited = r.ok("candidates", &["--task", "target", "--limit", "1"]);
    assert_eq!(
        limited["coverage"]["counts"]["candidate_limit_omitted"],
        candidates.len() - 1
    );
    let fallback = r.ok("candidates", &["--task", "zzzzzz"]);
    assert_eq!(fallback["data"]["fallback"], true);
}
#[test]
fn committed_changes_rename_delete_modes_and_dirty_checkout() {
    let r = Repo::new();
    r.init();
    r.write("old.py", b"def target():\n    return 1\n");
    r.write("deleted.rs", b"fn gone() {}\n");
    r.write("mode.py", b"def mode():\n    pass\n");
    let base = r.commit();
    r.git(&["mv", "old.py", "renamed.py"]);
    r.write("renamed.py", b"def target():\n    return 2\n");
    fs::remove_file(r.root.join("deleted.rs")).unwrap();
    r.git(&["update-index", "--chmod=+x", "mode.py"]);
    // Preserve executable mode through git add on platforms supporting it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(r.root.join("mode.py"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    r.write("added.py", b"def added():\n    pass\n");
    let head = r.commit();
    r.write("renamed.py", b"PRIVATE_DIRTY_SENTINEL");
    let range = format!("{base}..{head}");
    let d = r.ok("changes", &[&range]);
    assert_eq!(d["observation"]["base"], base);
    assert_eq!(d["observation"]["head"], head);
    let files = d["data"]["files"].as_array().unwrap();
    assert!(
        files
            .iter()
            .any(|f| f["before"]["path"] == "old.py" && f["after"]["path"] == "renamed.py")
    );
    assert!(
        files
            .iter()
            .any(|f| f["before"]["path"] == "deleted.rs" && f["after"].is_null())
    );
    assert!(files.iter().any(|f| f["after"]["mode"] == "100755"));
    let delta_kinds = files
        .iter()
        .flat_map(|f| f["symbols"].as_array().unwrap())
        .map(|s| s["kind"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(delta_kinds.contains("added"));
    assert!(delta_kinds.contains("removed"));
    assert!(delta_kinds.contains("modified"));

    let modified_file_index = files
        .iter()
        .position(|f| {
            f["symbols"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["kind"] == "modified")
        })
        .expect("rename fixture should retain a modified symbol pair");
    let modified_delta_index = files[modified_file_index]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .position(|s| s["kind"] == "modified")
        .unwrap();
    let modified = &files[modified_file_index];
    let before_path = modified["before"]["path"].as_str().unwrap();
    let base_other_symbol = d["data"]["base"]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["path"] != before_path)
        .expect("fixture has a second changed file with a symbol");
    let base_file = d["data"]["base"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == before_path)
        .unwrap();

    // Mutate a real immutable-Git rename response. Each document keeps the
    // producer output otherwise intact so the rejection isolates one invariant.
    let mut wrong_file_type = d.clone();
    wrong_file_type["data"]["files"][modified_file_index]["symbols"][modified_delta_index]["before_ref"] =
        base_file["id"].clone();
    assert!(!valid(&serde_json::to_vec(&wrong_file_type).unwrap()));

    let mut wrong_repo_type = d.clone();
    wrong_repo_type["data"]["files"][modified_file_index]["symbols"][modified_delta_index]["after_ref"] =
        d["data"]["head"]["repository_ref"].clone();
    assert!(!valid(&serde_json::to_vec(&wrong_repo_type).unwrap()));

    let mut wrong_side_path = d.clone();
    wrong_side_path["data"]["files"][modified_file_index]["before"]["path"] = json!("unrelated.py");
    assert!(!valid(&serde_json::to_vec(&wrong_side_path).unwrap()));

    let mut wrong_same_revision_file = d.clone();
    wrong_same_revision_file["data"]["files"][modified_file_index]["symbols"]
        [modified_delta_index]["before_ref"] = base_other_symbol["id"].clone();
    assert!(!valid(
        &serde_json::to_vec(&wrong_same_revision_file).unwrap()
    ));

    let mut wrong_revision = d.clone();
    wrong_revision["data"]["files"][modified_file_index]["symbols"][modified_delta_index]["before_ref"] =
        d["data"]["files"][modified_file_index]["symbols"][modified_delta_index]["after_ref"]
            .clone();
    assert!(!valid(&serde_json::to_vec(&wrong_revision).unwrap()));

    let mut invalid_added_shape = d.clone();
    invalid_added_shape["data"]["files"][modified_file_index]["symbols"][modified_delta_index]["kind"] =
        json!("added");
    assert!(!valid(&serde_json::to_vec(&invalid_added_shape).unwrap()));

    let mut dirty_change_observation = d.clone();
    dirty_change_observation["observation"]["git_head"] = json!(head);
    dirty_change_observation["observation"]["git_dirty"] = json!(true);
    assert!(!valid(
        &serde_json::to_vec(&dirty_change_observation).unwrap()
    ));

    let candidate_doc = r.ok("candidates", &["--task", "target", "--limit", "20"]);
    let mut wrong_candidate_sort = candidate_doc.clone();
    wrong_candidate_sort["data"]["sort_order"] = json!("random");
    assert!(!valid(&serde_json::to_vec(&wrong_candidate_sort).unwrap()));

    assert!(!d.to_string().contains("PRIVATE_DIRTY_SENTINEL"));
    assert_eq!(d, r.ok("changes", &[&range]));
    bad(r.run("changes", &["missing..HEAD"]));
    bad(r.run("candidates", &["--task", "target", "--change", &range]));
    r.git(&["restore", "renamed.py"]);
    let c = r.ok("candidates", &["--task", "added", "--change", &range]);
    assert_eq!(c["data"]["change"]["head"], head);
    bad(r.run(
        "candidates",
        &["--task", "added", "--change", &format!("{base}..{base}")],
    ));
}
#[test]
fn empty_non_git_and_rejected_selectors_leave_stdout_empty() {
    let r = Repo::new();
    let d = r.ok("structure", &[]);
    assert_eq!(d["data"]["files"], json!([]));
    bad(r.run("candidates", &["--task", ""]));
    bad(r.run("candidates", &["--task", "x", "--limit", "0"]));
    let o = Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["evidence", "structure"])
        .arg(&r.root)
        .args(["--repository-id", "../bad"])
        .output()
        .unwrap();
    bad(o);
}
#[cfg(unix)]
#[test]
fn native_control_paths_are_counted_and_essential_changes_fail_closed() {
    use std::os::unix::ffi::OsStrExt;
    let r = Repo::new();
    r.init();
    r.write("good.py", b"def good():\n    pass\n");
    let base = r.commit();
    let native_created = fs::write(
        r.root.join(std::ffi::OsStr::from_bytes(b"bad\xff.py")),
        b"def native():\n    pass\n",
    )
    .is_ok();
    if !native_created {
        eprintln!("APFS rejected native byte filename; injected exporter unit test covers it");
    }
    r.write("control\n.py", b"def control():\n    pass\n");
    r.write("literal\\path.py", b"def backslash():\n    pass\n");
    let d = r.ok("structure", &[]);
    assert_eq!(
        d["coverage"]["counts"]["unrepresentable_files_omitted"],
        2 + usize::from(native_created)
    );
    assert_eq!(d["data"]["files"].as_array().unwrap().len(), 1);
    let head = r.commit();
    bad(r.run("changes", &[&format!("{base}..{head}")]));
}
#[test]
fn independent_decoder_golden_valid_and_invalid_documents() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/evidence");
    for name in ["structure", "changes", "candidates", "workspace"] {
        assert!(valid(
            &fs::read(p.join(format!("valid-{name}.json"))).unwrap()
        ));
    }
    for name in [
        "version",
        "duplicate-key",
        "dangling",
        "unsafe-path",
        "duplicate-id",
        "count",
        "nullability",
        "relation-source-type",
        "relation-path-mismatch",
        "empty-scope",
        "containment-endpoint",
        "candidate-rank",
        "change-side-reference",
        "change-file-reference",
        "change-repository-reference",
        "change-side-path-mismatch",
        "change-added-with-before-reference",
        "changes-current-dirty-observation",
        "change-same-revision-wrong-file-symbol",
        "candidate-sort-order",
        "task-digest",
    ] {
        assert!(
            !valid(&fs::read(p.join(format!("invalid-{name}.json"))).unwrap()),
            "{name}"
        );
    }
}
#[cfg(windows)]
#[test]
fn native_nested_windows_paths_survive_structure_candidates_and_changes() {
    let r = Repo::new();
    r.init();
    r.write("src/lib.rs", b"pub fn nested_symbol() {}\n");
    let base = r.commit();
    r.write("src/lib.rs", b"pub fn nested_symbol() { let _ = 1; }\n");
    let head = r.commit();

    let structure = r.ok("structure", &[]);
    assert!(
        structure["data"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "src/lib.rs")
    );
    assert!(
        structure["data"]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["path"] == "src/lib.rs")
    );
    assert_eq!(
        structure["coverage"]["counts"]["unrepresentable_files_omitted"],
        0
    );

    let candidates = r.ok("candidates", &["--task", "nested_symbol", "--limit", "50"]);
    assert!(
        candidates["data"]["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["file"]["path"] == "src/lib.rs")
    );

    let changes = r.ok("changes", &[&format!("{base}..{head}")]);
    assert!(
        changes["data"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["after"]["path"] == "src/lib.rs")
    );
}
#[test]
fn independent_decoder_rejects_mutated_live_references_and_counts() {
    let r = Repo::new();
    r.write("a.py", b"def a():\n    a()\n");
    let d = r.ok("structure", &[]);
    for mutate in [0, 1, 2, 3, 4] {
        let mut x = d.clone();
        match mutate {
            0 => x["data"]["relations"][0]["source_ref"] = json!("missing"),
            1 => x["data"]["files"][0]["path"] = json!("a/../secret"),
            2 => x["coverage"]["counts"]["files_exported"] = json!(99),
            3 => x["observation"]["atomic"] = json!(true),
            _ => x["data"]["symbols"][0]["id"] = x["data"]["files"][0]["id"].clone(),
        }
        assert!(!valid(&serde_json::to_vec(&x).unwrap()));
    }
}
#[test]
fn workspace_preserves_v2_observed_declared_and_impact_semantics() {
    let tmp = tempfile::tempdir().unwrap();
    for name in ["provider", "consumer", "downstream", "alternate"] {
        let root = tmp.path().join(name);
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname='{name}'\nversion='0.1.0'\n[workspace]\n"),
        )
        .unwrap();
        fs::write(root.join("api.json"), "{}").unwrap();
    }
    fs::write(tmp.path().join("consumer/Cargo.toml"), "[package]\nname='consumer'\nversion='0.1.0'\n[dependencies]\nprovider={path='../provider'}\n").unwrap();
    fs::write(tmp.path().join("downstream/Cargo.toml"), "[package]\nname='downstream'\nversion='0.1.0'\n[dependencies]\nconsumer={path='../consumer'}\n").unwrap();
    let provider = tmp.path().join("provider");
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .arg("-C")
            .arg(&provider)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success());
        String::from_utf8(o.stdout).unwrap().trim().to_owned()
    };
    git(&["init", "-q"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["add", "."]);
    git(&["commit", "-qm", "base"]);
    let base = git(&["rev-parse", "HEAD"]);
    fs::write(provider.join("api.json"), "{\"changed\":true}").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "head"]);
    let mut manifest = json!({"schema_version":1,"workspace_id":"sample","repositories":[{"id":"provider","root":"provider"},{"id":"consumer","root":"consumer"},{"id":"downstream","root":"downstream"},{"id":"alternate","root":"alternate"}],"contracts":[{"id":"api","kind":"api_schema","provider":{"repo":"provider","path":"api.json"},"consumers":[{"repo":"consumer","path":"api.json"}]},{"id":"alternate-api","kind":"api_schema","provider":{"repo":"alternate","path":"api.json"},"consumers":[{"repo":"consumer","path":"api.json"}]}]});
    let file = tmp.path().join("workspace.json");
    fs::write(&file, manifest.to_string()).unwrap();
    let run = |extra: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_ctx"))
            .args(["evidence", "workspace", "--manifest"])
            .arg(&file)
            .args(extra)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert!(valid(&o.stdout));
        serde_json::from_slice::<Value>(&o.stdout).unwrap()
    };
    let d = run(&[]);
    assert_eq!(d["data"]["relations"].as_array().unwrap().len(), 4);
    manifest["repositories"].as_array_mut().unwrap().reverse();
    fs::write(&file, manifest.to_string()).unwrap();
    let reordered = run(&[]);
    let ids = |v: &Value| {
        v["data"]["relations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&d), ids(&reordered));
    let impact = run(&[
        "--repo",
        "provider",
        "--change",
        &format!("{base}..HEAD"),
        "--depth",
        "3",
    ]);
    assert_eq!(
        impact["data"]["impact"]["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["repo"] == "consumer")
            .unwrap()["reason"],
        "declared_contract_candidate"
    );
    let candidates = impact["data"]["impact"]["candidates"].as_array().unwrap();
    let consumer_index = candidates
        .iter()
        .position(|c| c["repo"] == "consumer")
        .unwrap();
    let downstream_index = candidates
        .iter()
        .position(|c| c["repo"] == "downstream")
        .unwrap();
    assert_eq!(
        candidates[consumer_index]["direct_triggers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut wrong_trigger_owner = impact.clone();
    wrong_trigger_owner["data"]["impact"]["candidates"][consumer_index]["repo"] = json!("provider");
    assert!(!valid(&serde_json::to_vec(&wrong_trigger_owner).unwrap()));

    let relations = impact["data"]["relations"].as_array().unwrap();
    let edge_index = |provider: &str, consumer: &str| {
        relations
            .iter()
            .position(|r| r["provider"] == provider && r["consumer"] == consumer)
            .unwrap()
    };
    let provider_consumer = edge_index("provider", "consumer");
    let alternate_consumer = edge_index("alternate", "consumer");

    let mut broken_chain = impact.clone();
    broken_chain["data"]["impact"]["candidates"][downstream_index]["witness_edges"] =
        json!([provider_consumer, alternate_consumer]);
    assert!(!valid(&serde_json::to_vec(&broken_chain).unwrap()));

    let mut missing_witness = impact.clone();
    missing_witness["data"]["impact"]["candidates"][downstream_index]["witness_edges"] = json!([]);
    missing_witness["data"]["impact"]["candidates"][downstream_index]["distance"] = json!(0);
    assert!(!valid(&serde_json::to_vec(&missing_witness).unwrap()));

    let mut wrong_final_consumer = impact.clone();
    wrong_final_consumer["data"]["impact"]["candidates"][downstream_index]["repo"] =
        json!("consumer");
    assert!(!valid(&serde_json::to_vec(&wrong_final_consumer).unwrap()));

    let mut wrong_trigger_provider = impact.clone();
    wrong_trigger_provider["data"]["impact"]["candidates"][consumer_index]["direct_triggers"][0]
        ["relation_id"] = relations[alternate_consumer]["id"].clone();
    assert!(!valid(
        &serde_json::to_vec(&wrong_trigger_provider).unwrap()
    ));

    let mut wrong_impact_revision = impact.clone();
    wrong_impact_revision["observation"]["head"] = base.clone().into();
    assert!(!valid(&serde_json::to_vec(&wrong_impact_revision).unwrap()));
    // Physical relocation keeps structural workspace relationships, not manifest bytes.
    let moved = tmp.path().with_extension("relocated");
    fs::rename(tmp.path(), &moved).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["evidence", "workspace", "--manifest"])
        .arg(moved.join("workspace.json"))
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(valid(&o.stdout));
    let relocated: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(
        reordered["data"]["relations"],
        relocated["data"]["relations"]
    );
    fs::remove_dir_all(moved).unwrap();
}

#[test]
fn oversized_structure_is_rejected_before_stdout() {
    let r = Repo::new();
    let source = (0..1000)
        .map(|i| format!("def function_{i}():\n    pass\n"))
        .collect::<String>();
    for i in 0..10 {
        r.write(&format!("large{i}.py"), source.as_bytes());
    }
    bad(r.run("structure", &[]));
}
#[test]
fn incomplete_ambiguous_changes_and_nested_scope_remain_explicit() {
    let r = Repo::new();
    r.init();
    r.write(
        "nested/a.py",
        b"def same():\n    pass\ndef same():\n    pass\n",
    );
    r.write("nested/b.py", b"def okay():\n    pass\n");
    let base = r.commit();
    r.write(
        "nested/a.py",
        b"def same():\n    return 1\ndef same():\n    return 2\n",
    );
    r.write("nested/b.py", b"def damaged(:\n");
    let head = r.commit();
    let o = Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["evidence", "changes", &format!("{base}..{head}")])
        .arg(r.root.join("nested"))
        .args(["--repository-id", "fixture"])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(valid(&o.stdout));
    let d: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(d["data"]["scope_prefix"], "nested");
    assert!(
        d["coverage"]["counts"]["ambiguous_symbol_keys"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        d["coverage"]["counts"]["unavailable_symbol_comparisons"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn independent_digest_shape_rejections_and_real_producer_acceptance() {
    let r = Repo::new();
    r.write("target.py", b"def target(): pass\n");
    let original = r.ok("candidates", &["--task", "target"]);
    assert_eq!(original["producer"]["analysis_compat_version"], 2);
    let digest = original["data"]["task_digest"].as_str().unwrap();
    assert_eq!(digest.len(), 64);
    for wrong in [
        "".into(),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        format!("{}\n", "a".repeat(64)),
    ] {
        let mut altered = original.clone();
        altered["data"]["task_digest"] = json!(wrong);
        assert!(!valid(&serde_json::to_vec(&altered).unwrap()));
    }
    // The independent stdlib decoder checks shape, not cryptographic derivation.
    // A different well-formed digest is intentionally not claimed as verified.
    let mut opaque = original;
    opaque["data"]["task_digest"] = json!("0".repeat(64));
    assert!(valid(&serde_json::to_vec(&opaque).unwrap()));
}
