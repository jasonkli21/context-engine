use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_DATE", "2026-10-15T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-10-15T00:00:00Z")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn init() -> TempDir {
    let d = TempDir::new().unwrap();
    git(d.path(), &["init", "-q"]);
    git(d.path(), &["config", "user.email", "test@example.test"]);
    git(d.path(), &["config", "user.name", "Fixture"]);
    d
}
fn write(root: &Path, path: &str, text: impl AsRef<[u8]>) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}
fn commit(root: &Path) -> String {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "fixture"]);
    git(root, &["rev-parse", "HEAD"])
}
fn run(root: &Path, command: &str, range: &str, flags: &[&str]) -> (Value, Vec<u8>) {
    let out = cargo_bin_cmd!("ctx")
        .args([command, range])
        .arg(root)
        .args(["--format", "json"])
        .args(flags)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["command"], command);
    (v["data"].clone(), out)
}
fn human(root: &Path, command: &str, range: &str) -> String {
    String::from_utf8(
        cargo_bin_cmd!("ctx")
            .args([command, range])
            .arg(root)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap()
}
fn file<'a>(v: &'a Value, path: &str) -> &'a Value {
    v["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["after"]["path"] == path || f["before"]["path"] == path)
        .unwrap()
}
fn fixture() -> (TempDir, String) {
    let d = init();
    let r = d.path();
    write(
        r,
        "backend/a.py",
        "import outside\ndef modified():\n    return 1\ndef removed(): pass\ndef caller():\n    removed()\n    modified()\n",
    );
    write(
        r,
        "backend/rename.py",
        "import somewhere\ndef unchanged(): pass\n",
    );
    write(
        r,
        "backend/tests/test_a.py",
        "def helper(): return 1\ndef test_helper(): helper()\n",
    );
    write(r, "backend/docs/guide.md", "before\n");
    write(r, "other/sibling.py", "def before(): pass\n");
    let base = commit(r);
    write(
        r,
        "backend/a.py",
        "\nimport outside\ndef modified():\n    return 2\ndef added(): pass\ndef caller():\n    added()\n    modified()\n    modified()\n",
    );
    git(r, &["mv", "backend/rename.py", "backend/renamed.py"]);
    write(
        r,
        "backend/tests/test_a.py",
        "def helper(): return 2\ndef test_helper(): helper()\n",
    );
    write(r, "backend/docs/guide.md", "after\n");
    write(r, "other/sibling.py", "def after(): pass\n");
    let head = commit(r);
    (d, format!("{base}..{head}"))
}
#[test]
fn committed_diff_symbols_multisets_rename_scope_and_dirty_isolation() {
    let (d, range) = fixture();
    let r = d.path().join("backend");
    let (v, bytes) = run(&r, "diff", &range, &[]);
    assert_eq!(bytes, run(&r, "diff", &range, &[]).1);
    assert_eq!(v["total_file_changes"], 4);
    assert_eq!(v["analysis_scope"], "changed file sides only");
    assert!(v["base_commit"].as_str().unwrap().len() >= 40);
    let f = file(&v, "a.py");
    let syms = f["symbols"].as_array().unwrap();
    for (kind, name) in [
        ("modified", "modified"),
        ("removed", "removed"),
        ("added", "added"),
        ("modified", "caller"),
    ] {
        assert!(
            syms.iter().any(|s| s["kind"] == kind
                && (s["after"]["qualified_name"] == name || s["before"]["qualified_name"] == name)),
            "{kind} {name}"
        );
    }
    let relations = f["relations"].as_array().unwrap();
    assert!(relations.iter().any(|r| r["kind"] == "references"
        && r["target"]["name"] == "modified"
        && r["before"]["count"] == 1
        && r["after"]["count"] == 2));
    assert!(!relations.iter().any(|r| r["kind"] == "imports"));
    let renamed = file(&v, "renamed.py");
    assert_eq!(renamed["kind"], "renamed");
    assert_eq!(renamed["rename_similarity"], 100);
    assert_eq!(renamed["symbols"], serde_json::json!([]));
    assert_eq!(renamed["relations"], serde_json::json!([]));
    assert!(
        v["directly_changed_test_doc_candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["role"] == "Documentation")
    );
    let git_index = fs::read(d.path().join(".git/index")).unwrap();
    let git_head = fs::read(d.path().join(".git/HEAD")).unwrap();
    let impact = run(&r, "impact", &range, &[]).1;
    let h = human(&r, "diff", &range);
    assert_eq!(h, human(&r, "diff", &range));
    write(d.path(), "backend/a.py", "THIS IS DIRTY NOT SOURCE\n");
    write(d.path(), "backend/tests/test_a.py", "dirty\n");
    assert_eq!(bytes, run(&r, "diff", &range, &[]).1);
    assert_eq!(impact, run(&r, "impact", &range, &[]).1);
    assert_eq!(h, human(&r, "diff", &range));
    assert_eq!(git_index, fs::read(d.path().join(".git/index")).unwrap());
    assert_eq!(git_head, fs::read(d.path().join(".git/HEAD")).unwrap());
}
#[test]
fn impact_has_correct_sides_shortest_predecessors_and_candidate_evidence() {
    let (d, range) = fixture();
    let r = d.path().join("backend");
    let (v, bytes) = run(&r, "impact", &range, &["--depth", "2"]);
    assert_eq!(bytes, run(&r, "impact", &range, &["--depth", "2"]).1);
    let sides = v["impact"].as_array().unwrap();
    for (side, name) in [(0, "removed"), (1, "added")] {
        let results = sides[side]["results"].as_array().unwrap();
        let root = results
            .iter()
            .find(|r| r["seed"]["symbol"]["qualified_name"] == name)
            .unwrap();
        let visits = root["visits"].as_array().unwrap();
        let caller = visits
            .iter()
            .find(|v| v["entity"]["symbol"]["qualified_name"] == "caller")
            .unwrap();
        assert_eq!(caller["distance"], 1);
        let rid = &caller["predecessor"]["relation_id"];
        assert!(
            root["relations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| &r["id"] == rid
                    && r["kind"] == "references"
                    && r["resolution"] == "exact_same_file_qualified_name")
        );
        assert!(root["seed_symbol_change"].is_number());
        assert!(
            !results.iter().any(|r| r["seed"]["symbol"]["qualified_name"]
                == if side == 0 { "added" } else { "removed" })
        );
    }
    for side in sides {
        let roots = side["results"].as_array().unwrap();
        assert!(
            roots
                .iter()
                .any(|r| r["seed"]["symbol"]["qualified_name"] == "modified")
        );
        assert!(
            roots
                .iter()
                .flat_map(|r| r["visits"].as_array().unwrap())
                .any(|v| v["candidate_test"] == true && v["predecessor"].is_object())
        );
        for r in roots {
            assert!(
                !r["visits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["entity"]["kind"] == "repository")
            );
        }
    }
    let zero = run(&r, "impact", &range, &["--depth", "0"]).0;
    for side in zero["impact"].as_array().unwrap() {
        for root in side["results"].as_array().unwrap() {
            assert_eq!(root["visits"].as_array().unwrap().len(), 1);
            assert!(root["relations"].as_array().unwrap().is_empty());
        }
    }
    let h = human(&r, "impact", &range);
    assert_eq!(h, human(&r, "impact", &range));
    assert!(h.contains("predecessor Some("));
    assert!(h.contains("candidate_test: true"));
}
#[test]
fn file_accounting_survives_incomplete_ambiguous_and_mode_changes() {
    let d = init();
    let r = d.path();
    write(
        r,
        "a.py",
        "def duplicate(): return 1\ndef duplicate(): return 2\n",
    );
    write(r, "bad.py", "def before(): pass\n");
    write(r, "deleted.rs", "fn removed() {}\n");
    write(r, "mode.py", "def mode(): pass\n");
    let a = commit(r);
    write(
        r,
        "a.py",
        "def duplicate(): return 3\ndef duplicate(): return 2\n",
    );
    write(r, "bad.py", [255]);
    fs::remove_file(r.join("deleted.rs")).unwrap();
    write(r, "new.rs", "fn added() {}\n");
    git(r, &["update-index", "--chmod=+x", "mode.py"]);
    // Commit index mode separately from the fixture helper's add -A.
    git(r, &["add", "a.py", "bad.py", "deleted.rs", "new.rs"]);
    git(r, &["commit", "-qm", "changes"]);
    let b = git(r, &["rev-parse", "HEAD"]);
    let range = format!("{a}..{b}");
    let v = run(r, "diff", &range, &[]).0;
    assert_eq!(file(&v, "a.py")["ambiguous_symbol_keys"], 1);
    assert_eq!(file(&v, "a.py")["symbols"], serde_json::json!([]));
    assert_eq!(file(&v, "a.py")["relation_comparison_available"], false);
    assert_eq!(
        file(&v, "bad.py")["after"]["analysis_status"],
        "invalid_utf8"
    );
    assert_eq!(file(&v, "bad.py")["symbols"], serde_json::json!([]));
    assert_eq!(v["head_coverage"]["analysis_incomplete"], true);
    assert_eq!(file(&v, "deleted.rs")["kind"], "deleted");
    assert_eq!(file(&v, "deleted.rs")["symbols"][0]["kind"], "removed");
    assert_eq!(file(&v, "new.rs")["symbols"][0]["kind"], "added");
    let mode = file(&v, "mode.py");
    assert_eq!(mode["before"]["mode"], "100644");
    assert_eq!(mode["after"]["mode"], "100755");
    assert_eq!(mode["symbols"], serde_json::json!([]));
    let limited = run(r, "diff", &range, &["--max-parse-file-size", "1"]).0;
    assert_eq!(limited["total_file_changes"], v["total_file_changes"]);
    assert_eq!(
        file(&limited, "new.rs")["after"]["analysis_status"],
        "too_large"
    );
    assert_eq!(file(&limited, "new.rs")["symbols"], serde_json::json!([]));
}
#[test]
fn range_cli_validation_zero_delta_and_no_git_errors_have_empty_stdout() {
    let (d, range) = fixture();
    for invalid in [
        "HEAD",
        "..HEAD",
        "HEAD..",
        "HEAD...HEAD",
        "HEAD..HEAD..HEAD",
        "missing..HEAD",
    ] {
        let a = cargo_bin_cmd!("ctx")
            .args(["diff", invalid])
            .arg(d.path())
            .args(["--format", "json"])
            .assert()
            .failure();
        assert!(a.get_output().stdout.is_empty());
    }
    let same = range.split_once("..").unwrap().1;
    let zero = format!("{same}..{same}");
    for c in ["diff", "impact"] {
        let v = run(d.path(), c, &zero, &[]).0;
        assert_eq!(v["total_file_changes"], 0);
    }
    cargo_bin_cmd!("ctx").arg("diff").assert().failure();
    cargo_bin_cmd!("ctx")
        .args(["impact", &range, "--depth", "9"])
        .assert()
        .failure();
    let empty = TempDir::new().unwrap();
    let a = cargo_bin_cmd!("ctx")
        .args(["diff", "HEAD..HEAD"])
        .arg(empty.path())
        .assert()
        .failure();
    assert!(a.get_output().stdout.is_empty());
}
#[test]
fn moving_symbols_relations_without_body_edits_does_not_churn() {
    let d = init();
    let r = d.path();
    let text = "import missing\ndef a(): b()\ndef b(): pass\n";
    write(r, "x.py", text);
    let a = commit(r);
    write(r, "x.py", format!("\n\n{text}"));
    let b = commit(r);
    let v = run(r, "diff", &format!("{a}..{b}"), &[]).0;
    assert_eq!(v["files"][0]["symbols"], serde_json::json!([]));
    assert_eq!(v["files"][0]["relations"], serde_json::json!([]));
}
#[test]
fn raw_blobs_ignore_attributes_filters_and_git_command_overrides() {
    let (d, range) = fixture();
    let r = d.path();
    let expected = run(r, "diff", &range, &[]).1;
    write(r, ".gitattributes", "*.py filter=evil diff=evil\n");
    let sentinel = r.join("EXECUTED");
    let malicious = format!("touch {}", sentinel.display());
    for key in [
        "filter.evil.clean",
        "filter.evil.smudge",
        "filter.evil.process",
        "diff.evil.textconv",
        "diff.external",
        "core.fsmonitor",
    ] {
        git(r, &["config", key, &malicious]);
    }
    let out = cargo_bin_cmd!("ctx")
        .args(["diff", &range])
        .arg(r)
        .args(["--format", "json"])
        .env("GIT_DIR", r.join("wrong"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.bare")
        .env("GIT_CONFIG_VALUE_0", "true")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(expected, out);
    assert!(!sentinel.exists());
}
#[cfg(unix)]
#[test]
fn human_escapes_control_paths_and_non_regular_modes_are_not_followed() {
    use std::os::unix::fs::symlink;
    let d = init();
    let r = d.path();
    write(r, "base.py", "def old(): pass\n");
    let a = commit(r);
    write(r, "control\nname.py", "def new(): pass\n");
    symlink("base.py", r.join("link.py")).unwrap();
    let b = commit(r);
    let range = format!("{a}..{b}");
    let v = run(r, "diff", &range, &[]).0;
    assert_eq!(
        file(&v, "link.py")["after"]["analysis_status"],
        "non_regular_mode"
    );
    assert_eq!(file(&v, "link.py")["symbols"], serde_json::json!([]));
    let h = human(r, "diff", &range);
    assert!(h.contains("control\\nname.py"));
    assert!(!h.contains("control\nname.py"));
}
#[test]
fn crossing_scope_rename_is_add_or_delete_without_sibling_path_leakage() {
    let d = init();
    let r = d.path();
    write(r, "inside/old.py", "def f(): pass\n");
    write(r, "outside/other.py", "def g(): pass\n");
    let a = commit(r);
    git(r, &["mv", "inside/old.py", "outside/new.py"]);
    git(r, &["mv", "outside/other.py", "inside/new.py"]);
    let b = commit(r);
    let range = format!("{a}..{b}");
    for c in ["diff", "impact"] {
        let v = run(&r.join("inside"), c, &range, &[]).0;
        assert_eq!(v["total_file_changes"], 2);
        assert_eq!(file(&v, "old.py")["kind"], "deleted");
        assert_eq!(file(&v, "new.py")["kind"], "added");
        for f in v["files"].as_array().unwrap() {
            for side in ["before", "after"] {
                assert!(!f[side]["path"].as_str().unwrap_or("").contains("outside"));
            }
        }
    }
}

#[test]
fn production_impact_reports_shared_root_omissions_across_sides() {
    let d = init();
    let r = d.path();
    let source = |n| {
        (0..70)
            .map(|i| format!("def f{i}(): return {n}\n"))
            .collect::<String>()
    };
    write(r, "a.py", source(1));
    let a = commit(r);
    write(r, "a.py", source(2));
    let b = commit(r);
    let range = format!("{a}..{b}");
    let v = run(r, "impact", &range, &["--depth", "0"]).0;
    let sides = v["impact"].as_array().unwrap();
    assert_eq!(
        sides
            .iter()
            .map(|s| s["results"].as_array().unwrap().len())
            .sum::<usize>(),
        128
    );
    assert_eq!(sides[0]["seeds_requested"], 71);
    assert_eq!(sides[1]["seeds_requested"], 71);
    assert_eq!(sides[0]["seeds_omitted"], 0);
    assert_eq!(sides[1]["seeds_omitted"], 14);
    assert_eq!(sides[1]["roots_truncated"], true);
    assert!(human(r, "impact", &range).contains("omitted: 14"));
}

#[test]
fn generated_unsupported_recovered_syntax_and_relation_gaps_remain_visible() {
    let d = init();
    let r = d.path();
    write(r, "README.md", "fixture\n");
    let a = commit(r);
    write(r, "vendor/a.py", "def vendor(): pass\n");
    write(r, "generated/a.rs", "fn generated() {}\n");
    write(r, "unknown.ts", "function outside() {}\n");
    write(r, "syntax.py", "def recovered(): pass\nbroken = (\n");
    write(r, "relations.py", "def f():\n    get()()\n");
    let b = commit(r);
    for command in ["diff", "impact"] {
        let v = run(r, command, &format!("{a}..{b}"), &[]).0;
        assert_eq!(v["total_file_changes"], 5);
        for path in ["vendor/a.py", "generated/a.rs"] {
            let f = file(&v, path);
            assert_eq!(f["after"]["analysis_status"], "skipped_role");
            assert_eq!(f["symbols"], serde_json::json!([]));
        }
        assert_eq!(
            file(&v, "unknown.ts")["after"]["analysis_status"],
            "unsupported_language"
        );
        assert_eq!(file(&v, "syntax.py")["comparison_available"], false);
        assert_eq!(file(&v, "syntax.py")["symbols"], serde_json::json!([]));
        assert_eq!(
            file(&v, "relations.py")["relation_comparison_available"],
            false
        );
        assert!(
            file(&v, "relations.py")["after"]["unsupported_relation_candidates"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(v["head_coverage"]["analysis_incomplete"], true);
    }
}
