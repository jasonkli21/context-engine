mod support;
use serde_json::Value;
use std::{fs, path::Path, process::Command};
use support::ctx;
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
fn write(r: &Path, p: &str, text: impl AsRef<[u8]>) {
    let p = r.join(p);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}
fn commit(r: &Path, subject: &str) -> String {
    git(r, &["add", "-A"]);
    git(r, &["commit", "-qm", subject]);
    git(r, &["rev-parse", "HEAD"])
}
struct Fixture {
    dir: TempDir,
    introduction: String,
    modification: String,
    rename: String,
    rename_modified: String,
    merge: String,
    branch: String,
}
fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.email", "test@example.test"]);
    git(r, &["config", "user.name", "Fixture"]);
    write(
        r,
        "scope/old.py",
        "class Box:\n    def run(self):\n        return 1\n\ndef stable(): return 0\n",
    );
    write(
        r,
        "scope/other.py",
        "class Other:\n    def run(self): pass\n",
    );
    let introduction = commit(r, "introduce");
    write(r, "outside.py", "def outside(): pass\n");
    commit(r, "unrelated");
    write(
        r,
        "scope/old.py",
        "class Box:\n    def run(self):\n        return 2\n\ndef stable(): return 0\n",
    );
    write(r, "scope/tests/test_box.py", "def test_box(): pass\n");
    let modification = commit(r, "modify\x1b[31m hostile");
    git(r, &["mv", "scope/old.py", "scope/middle.py"]);
    let rename = commit(r, "rename");
    git(r, &["mv", "scope/middle.py", "scope/new.py"]);
    write(
        r,
        "scope/new.py",
        "class Box:\n    def run(self):\n        return 3\n\ndef stable(): return 0\n",
    );
    let rename_modified = commit(r, "rename and modify");
    git(r, &["checkout", "-qb", "feature"]);
    write(
        r,
        "scope/new.py",
        "class Box:\n    def run(self):\n        return 4\n\ndef stable(): return 0\n",
    );
    write(r, "scope/branch.py", "def feature(): pass\n");
    let branch = commit(r, "branch internals");
    git(r, &["checkout", "-q", "main"]);
    write(r, "scope/main.txt", "main\n");
    commit(r, "mainline");
    git(r, &["merge", "--no-ff", "-qm", "merge result", "feature"]);
    let merge = git(r, &["rev-parse", "HEAD"]);
    Fixture {
        dir,
        introduction,
        modification,
        rename,
        rename_modified,
        merge,
        branch,
    }
}
fn run(r: &Path, command: &str, target: &str, extra: &[&str]) -> (Value, Vec<u8>) {
    let mut c = ctx();
    c.args([command, target]);
    if command == "evolution" {
        c.arg("--repo");
    }
    c.arg(r).args(["--format", "json"]).args(extra);
    let bytes = c.assert().success().get_output().stdout.clone();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["command"], command);
    (v["data"].clone(), bytes)
}
#[test]
fn lineage_renames_merge_cochange_dirty_determinism_and_cache_isolation() {
    let f = fixture();
    let r = f.dir.path().join("scope");
    let cache = TempDir::new().unwrap();
    ctx()
        .args(["index"])
        .arg(&r)
        .env("CONTEXT_ENGINE_CACHE_DIR", cache.path())
        .assert()
        .success();
    fn files(r: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        let mut result = vec![];
        for e in fs::read_dir(r).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                result.extend(files(&p));
            } else {
                result.push((p.clone(), fs::read(p).unwrap()));
            }
        }
        result.sort();
        result
    }
    let before = files(cache.path());
    let (v, bytes) = run(&r, "history", "Box.run", &[]);
    assert_eq!(v["history_policy"], "first_parent");
    assert_eq!(v["symbol"]["path"], "new.py");
    let events = v["events"].as_array().unwrap();
    assert_eq!(events.len(), 5);
    for (event, (oid, kind)) in events.iter().zip([
        (&f.merge, "modified"),
        (&f.rename_modified, "file_renamed"),
        (&f.rename, "file_renamed"),
        (&f.modification, "modified"),
        (&f.introduction, "introduced"),
    ]) {
        assert_eq!(event["commit"], *oid);
        assert_eq!(event["kind"], kind);
    }
    assert_eq!(events[1]["lineage_symbol_modified"], true);
    assert_eq!(events[2]["lineage_symbol_modified"], false);
    assert!(events.iter().all(|e| e["commit"] != f.branch));
    assert_eq!(v["coverage"]["lineage_complete_to_introduction"], true);
    assert_eq!(v["coverage"]["commits_inspected"], 7);
    assert!(
        v["cochanges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["path"] == "tests/test_box.py")
    );
    write(&r, "new.py", "def contradictory_dirty(): pass\n");
    assert_eq!(bytes, run(&r, "history", "Box.run", &[]).1);
    assert_eq!(bytes, run(&r, "history", "run", &["--file", "new.py"]).1);
    ctx()
        .args(["history", "run"])
        .arg(&r)
        .assert()
        .failure()
        .stderr(predicates::str::contains("2 matches"));
    // Queries run with an existing populated cache, which must remain byte-identical.
    ctx()
        .args(["history", "Box.run"])
        .arg(&r)
        .env("CONTEXT_ENGINE_CACHE_DIR", cache.path())
        .assert()
        .success();
    ctx()
        .args(["evolution", ".", "--repo"])
        .arg(&r)
        .env("CONTEXT_ENGINE_CACHE_DIR", cache.path())
        .assert()
        .success();
    assert_eq!(before, files(cache.path()));
    let human = ctx()
        .args(["history", "Box.run"])
        .arg(&r)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(!human.contains(&0x1b));
    assert!(String::from_utf8(human).unwrap().contains("\\u{1b}"));
    let (short, _) = run(&r, "history", "Box.run", &["--max-commits", "1"]);
    assert_eq!(short["coverage"]["history_truncated"], true);
    assert_eq!(short["coverage"]["lineage_complete_to_introduction"], false);
    // A different symbol's edit in the lineage file is not a symbol co-change.
    write(
        &r,
        "new.py",
        "class Box:\n    def run(self):\n        return 4\n\ndef stable(): return 1\n",
    );
    write(&r, "unrelated_companion.txt", "unrelated\n");
    commit(f.dir.path(), "only another symbol changed");
    let (v, _) = run(&r, "history", "Box.run", &[]);
    assert_eq!(v["coverage"]["target_change_commits"], 5);
    assert!(
        v["cochanges"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["path"] != "unrelated_companion.txt")
    );
}
#[test]
fn evolution_deleted_paths_directory_scope_structural_counts_and_validation() {
    let f = fixture();
    let r = f.dir.path().join("scope");
    let (old, _) = run(&r, "evolution", "old.py", &[]);
    let events = old["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["git_status"], "renamed");
    assert_eq!(events[1]["structural"]["symbols_modified"], 2); // class slice includes method
    assert_eq!(events[2]["git_status"], "added");
    let (all, bytes) = run(&r, "evolution", ".", &[]);
    assert_eq!(bytes, run(&r, "evolution", ".", &[]).1);
    assert_eq!(all["coverage"]["commits_inspected"], 7);
    assert_eq!(all["coverage"]["history_truncated"], false);
    assert!(
        all["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| !e.to_string().contains("outside.py"))
    );
    for invalid in ["../outside", "/absolute"] {
        ctx()
            .args(["evolution", invalid, "--repo"])
            .arg(&r)
            .assert()
            .failure();
    }
    for n in ["0", "513"] {
        ctx()
            .args(["evolution", ".", "--repo"])
            .arg(&r)
            .args(["--max-commits", n])
            .assert()
            .failure();
    }
    ctx()
        .args(["history", "missing"])
        .arg(&r)
        .assert()
        .failure()
        .stderr(predicates::str::contains("0 matches"));
    // Deletion and mode/type evidence remains useful even for unsupported blobs.
    fs::remove_file(r.join("other.py")).unwrap();
    write(&r, "notes.md", "text\n");
    commit(f.dir.path(), "delete and docs");
    let (deleted, _) = run(&r, "evolution", "other.py", &[]);
    assert_eq!(deleted["events"][0]["git_status"], "deleted");
    let (docs, _) = run(&r, "evolution", "notes.md", &[]);
    assert_eq!(
        docs["events"][0]["structural"]["comparison_available"],
        false
    );
    assert!(
        docs["events"][0]["coverage_gap"]
            .as_str()
            .unwrap()
            .contains("unsupported_language")
    );
    write(&r, "relations.py", "def a(): pass\ndef caller(): a()\n");
    commit(f.dir.path(), "relations");
    write(
        &r,
        "relations.py",
        "def a(): pass\ndef caller():\n    a()\n    a()\n",
    );
    commit(f.dir.path(), "more relations");
    let (v, _) = run(&r, "evolution", "relations.py", &[]);
    assert_eq!(
        v["events"][0]["structural"]["relation_comparison_available"],
        true
    );
    assert_eq!(v["events"][0]["structural"]["relations_added"], 1);
    assert_eq!(
        v["reported_event_file_totals"]["file_change_counts"]["added"],
        1
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(r.join("relations.py"), fs::Permissions::from_mode(0o755)).unwrap();
        commit(f.dir.path(), "mode");
        let (v, _) = run(&r, "evolution", "relations.py", &[]);
        assert_eq!(v["events"][0]["git_status"], "mode_changed");
        fs::remove_file(r.join("relations.py")).unwrap();
        std::os::unix::fs::symlink("new.py", r.join("relations.py")).unwrap();
        commit(f.dir.path(), "type");
        let (v, _) = run(&r, "evolution", "relations.py", &[]);
        assert_eq!(v["events"][0]["git_status"], "type_changed");
        assert!(
            v["events"][0]["coverage_gap"]
                .as_str()
                .unwrap()
                .contains("non_regular_git_mode")
        );
    }
}
#[test]
fn historical_ambiguity_syntax_utf8_and_scope_entry_stop_false_introduction() {
    let f = fixture();
    let r = f.dir.path().join("scope");
    // Duplicate historical descriptor, subsequently made unique.
    write(
        &r,
        "new.py",
        "class Box:\n    def run(self): pass\n    def run(self): pass\n",
    );
    commit(f.dir.path(), "duplicate");
    write(&r, "new.py", "class Box:\n    def run(self): pass\n");
    commit(f.dir.path(), "unique");
    let (v, _) = run(&r, "history", "Box.run", &[]);
    assert_eq!(v["events"][0]["kind"], "ambiguous");
    assert_eq!(v["coverage"]["lineage_complete_to_introduction"], false);
    write(&r, "new.py", b"def bad():\n    \xff\n");
    commit(f.dir.path(), "invalid utf8");
    write(&r, "new.py", "class Box:\n    def run(self): pass\n");
    commit(f.dir.path(), "valid");
    let (v, _) = run(&r, "history", "Box.run", &[]);
    assert_eq!(v["events"][0]["kind"], "comparison_unavailable");
    assert!(
        v["events"][0]["coverage_gap"]
            .as_str()
            .unwrap()
            .contains("invalid_utf8")
    );
    write(
        &r,
        "new.py",
        "class Box:\n    def run(self):\n        return (\n",
    );
    commit(f.dir.path(), "syntax damage");
    write(&r, "new.py", "class Box:\n    def run(self): pass\n");
    commit(f.dir.path(), "syntax repaired");
    let (v, _) = run(&r, "history", "Box.run", &[]);
    assert_eq!(v["events"][0]["kind"], "comparison_unavailable");
    assert!(
        v["events"][0]["coverage_gap"]
            .as_str()
            .unwrap()
            .contains("syntax_errors")
    );
    // Move a file from outside the selected nested root. This is not introduction.
    write(f.dir.path(), "external.py", "def entered(): pass\n");
    commit(f.dir.path(), "external");
    git(f.dir.path(), &["mv", "external.py", "scope/entered.py"]);
    commit(f.dir.path(), "enters");
    let (v, _) = run(&r, "history", "entered", &[]);
    assert_eq!(v["events"][0]["git_status"], "entered_scope");
    assert_eq!(v["events"][0]["kind"], "comparison_unavailable");
    assert_eq!(v["coverage"]["lineage_complete_to_introduction"], false);
    // A bounded per-file size omission is a result coverage gap for evolution.
    let (v, _) = run(
        &r,
        "evolution",
        "entered.py",
        &["--max-parse-file-size", "1"],
    );
    assert!(
        v["events"][0]["coverage_gap"]
            .as_str()
            .unwrap()
            .contains("too_large")
    );
}
