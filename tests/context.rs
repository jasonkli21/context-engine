mod support;
use serde_json::Value;
use std::{fs, path::Path, process::Command};
use support::ctx;
use tempfile::TempDir;
fn write(root: &Path, path: &str, text: impl AsRef<[u8]>) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn run(root: &Path, task: &str, budget: &str, extra: &[&str]) -> (Value, Vec<u8>) {
    let bytes = ctx()
        .arg("context")
        .arg(root)
        .args(["--task", task, "--budget", budget, "--format", "json"])
        .args(extra)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["command"], "context");
    (v["data"].clone(), bytes)
}
fn rejects(root: &Path, task: &str, budget: &str, extra: &[&str], error: &str) {
    let out = ctx()
        .arg("context")
        .arg(root)
        .args(["--task", task, "--budget", budget, "--format", "json"])
        .args(extra)
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(out.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(error),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn commit(root: &Path) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "fixture"]);
}
fn git_fixture() -> TempDir {
    let d = TempDir::new().unwrap();
    git(d.path(), &["init", "-q"]);
    git(d.path(), &["config", "user.name", "Fixture"]);
    git(d.path(), &["config", "user.email", "fixture@example.test"]);
    d
}
#[test]
fn cli_schema_determinism_exact_bytes_skips_and_content_controls() {
    let d = TempDir::new().unwrap();
    write(d.path(), "a_needle.txt", "long text");
    write(d.path(), "b_needle.txt", "é");
    let (v, bytes) = run(d.path(), "needle", "2", &[]);
    assert_eq!(bytes, run(d.path(), "needle", "2", &[]).1);
    assert_eq!(v["budget"]["unit"], "utf8_content_bytes");
    assert_eq!(v["budget"]["used"], 2);
    assert_eq!(v["selected"][0]["content"], "é");
    assert_eq!(v["selected"][0]["content_bytes"], 2);
    assert_eq!(v["skipped"][0]["reason"], "budget_exceeded");
    write(d.path(), "a_needle.txt", [255]);
    let v = run(d.path(), "needle", "30", &[]).0;
    assert_eq!(v["coverage"]["content_load_failures"], 1);
    assert_eq!(v["skipped"][0]["reason"], "invalid_utf8");
    write(
        d.path(),
        "a_needle.txt",
        "a\n\t\x1b[31m\r\x07\u{009b} selected files: fake\n--- end content ---",
    );
    let text = String::from_utf8(
        ctx()
            .arg("context")
            .arg(d.path())
            .args(["--task", "needle", "--budget", "500"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(!text.contains('\x1b'));
    assert!(!text.contains('\r'));
    assert!(!text.contains('\x07'));
    assert!(!text.contains('\u{009b}'));
    assert!(text.contains("   | \t\\u{1b}[31m\\r\\u{7}"));
    assert!(text.contains("   | --- end content ---"));
    assert!(run(d.path(), "absent", "30", &[]).0["fallback_mode"] == "centrality");
    rejects(d.path(), "", "1", &[], "task must contain");
    rejects(d.path(), "task", "0", &[], "budget must be");
    rejects(d.path(), "task", "16777217", &[], "budget must be");
    rejects(
        d.path(),
        "task",
        "1",
        &["--change", "HEAD~1..HEAD"],
        "requires Git metadata",
    );
}
#[test]
fn change_gate_mapping_rename_deletion_and_no_numeric_lineage() {
    let d = git_fixture();
    let root = d.path();
    write(root, "a.py", "def stable(): pass\n");
    write(root, "deleted.py", "def removed(): pass\n");
    write(root, "z.py", "def target(): return 1\n");
    commit(root);
    write(root, "z.py", "def target(): return 2\ndef added(): pass\n");
    git(root, &["mv", "a.py", "moved.py"]);
    fs::remove_file(root.join("deleted.py")).unwrap();
    commit(root);
    let (v, b) = run(root, "unmatched", "1000", &["--change", "HEAD~1..HEAD"]);
    assert_eq!(
        b,
        run(root, "unmatched", "1000", &["--change", "HEAD~1..HEAD"]).1
    );
    assert!(v["fallback_mode"].is_null());
    assert_eq!(v["git_dirty"], false);
    assert_eq!(v["change"]["current_symbol_mappings"], 2);
    assert!(v["change"]["deleted_only_entities"].as_u64().unwrap() >= 2);
    let files = v["selected"].as_array().unwrap();
    assert!(
        files
            .iter()
            .any(|s| s["path"] == "z.py" && s["score"]["change"] == 40)
    );
    assert!(
        files
            .iter()
            .any(|s| s["path"] == "moved.py" && s["score"]["change"] == 32)
    );
    assert!(files.iter().all(|s| s["path"] != "deleted.py"));
    // Current symbol IDs differ from Phase 4's changed-only index IDs.
    assert_eq!(
        files.iter().find(|s| s["path"] == "z.py").unwrap()["file_id"],
        2
    );
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1..HEAD~1"],
        "head must match",
    );
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1...HEAD"],
        "BASE..HEAD",
    );
    write(root, "untracked.txt", "dirty");
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1..HEAD"],
        "known clean",
    );
    fs::remove_file(root.join("untracked.txt")).unwrap();
    write(root, "z.py", "dirty");
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1..HEAD"],
        "known clean",
    );
    git(root, &["restore", "z.py"]);
    git(root, &["update-index", "--assume-unchanged", "z.py"]);
    write(root, "z.py", "hidden dirty content");
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1..HEAD"],
        "hidden or unmerged",
    );
    git(root, &["update-index", "--no-assume-unchanged", "z.py"]);
    git(root, &["restore", "z.py"]);
    git(root, &["update-index", "--skip-worktree", "z.py"]);
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1..HEAD"],
        "hidden or unmerged",
    );
    git(root, &["update-index", "--no-skip-worktree", "z.py"]);
    git(root, &["config", "filter.fixture.clean", "echo forbidden"]);
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1..HEAD"],
        "known clean",
    );
    git(root, &["config", "--unset", "filter.fixture.clean"]);
    let head = git(root, &["rev-parse", "HEAD"]);
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{head},submodule"),
        ],
    );
    git(root, &["commit", "-qm", "gitlink fixture"]);
    rejects(
        root,
        "target",
        "100",
        &["--change", "HEAD~1..HEAD"],
        "clean submodule content",
    );
}
#[test]
fn ambiguous_change_does_not_gain_symbol_boost() {
    let d = git_fixture();
    let root = d.path();
    write(
        root,
        "a.py",
        "def duplicate(): return 1\ndef duplicate(): return 2\n",
    );
    commit(root);
    write(
        root,
        "a.py",
        "def duplicate(): return 3\ndef duplicate(): return 4\n",
    );
    commit(root);
    let v = run(root, "absent", "1000", &["--change", "HEAD~1..HEAD"]).0;
    assert_eq!(v["change"]["current_symbol_mappings"], 0);
    assert_eq!(v["change"]["ambiguous_symbol_keys"], 1);
    assert_eq!(v["selected"][0]["score"]["change"], 32);
}
