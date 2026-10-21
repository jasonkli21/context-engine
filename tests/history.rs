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
    assert_eq!(v["schema_version"], 2);
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
        (&f.introduction, "path_lineage_start"),
    ]) {
        assert_eq!(event["commit"], *oid);
        assert_eq!(event["kind"], kind);
    }
    assert_eq!(events[1]["lineage_symbol_modified"], true);
    assert_eq!(events[2]["lineage_symbol_modified"], false);
    assert!(events.iter().all(|e| e["commit"] != f.branch));
    assert_eq!(v["coverage"]["history_stop_reason"], "tracked_path_start");
    assert_eq!(events[4]["origin_scope"], "tracked_file_first_parent_path");
    assert_eq!(events[4]["cross_file_origin_unknown"], true);
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
    let (selected, _) = run(&r, "history", "run", &["--file", "new.py"]);
    // Phase 8 review fixes distinguish scoped HEAD lookup coverage; the lineage
    // evidence stays equivalent, while the full envelopes intentionally differ.
    assert_eq!(v["symbol"], selected["symbol"]);
    assert_eq!(v["events"], selected["events"]);
    assert_eq!(v["cochanges"], selected["cochanges"]);
    assert_eq!(
        v["coverage"]["head_lookup_scope"],
        "supported_rust_python_inventory"
    );
    assert_eq!(selected["coverage"]["head_lookup_scope"], "selected_file");
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
    assert_eq!(short["coverage"]["history_stop_reason"], "window_limit");
    let (continuous, _) = run(&r, "history", "Box.run", &["--max-commits", "2"]);
    assert_eq!(continuous["coverage"]["history_truncated"], true);
    assert_eq!(
        continuous["coverage"]["history_stop_reason"],
        "window_limit"
    );
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
    assert_eq!(events[1]["structural"]["observed_symbols_modified"], 2); // class slice includes method
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
fn historical_ambiguity_syntax_utf8_and_scope_entry_stop_lineage() {
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
    assert_eq!(v["coverage"]["history_stop_reason"], "coverage_gap");
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
    assert_eq!(v["coverage"]["history_stop_reason"], "coverage_gap");
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

#[test]
fn path_lineage_start_and_exact_window_stop_do_not_claim_global_introduction() {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.email", "test@example.test"]);
    git(r, &["config", "user.name", "Fixture"]);
    write(r, "notes.md", "unrelated root content\n");
    commit(r, "unrelated root");
    write(
        r,
        "old.py",
        format!(
            "def target(): return 1\n{}",
            (0..80)
                .map(|i| format!("def old_{i}(): return {i}\n"))
                .collect::<String>()
        ),
    );
    commit(r, "source path");
    // The target survives while enough unrelated content changes to make this an A/D pair.
    fs::remove_file(r.join("old.py")).unwrap();
    write(
        r,
        "new.py",
        format!(
            "def target(): return 1\n{}",
            (0..80)
                .map(|i| format!("def new_{i}(): return {}\n", i + 1000))
                .collect::<String>()
        ),
    );
    commit(r, "move with rewritten file");
    write(r, "new.py", "def target(): return 2\n");
    commit(r, "edit moved target");

    let (full, _) = run(r, "history", "target", &["--file", "new.py"]);
    assert_eq!(full["events"][0]["kind"], "modified");
    assert_eq!(full["events"][1]["git_status"], "added");
    assert_eq!(full["events"][1]["kind"], "path_lineage_start");
    assert_eq!(full["events"][1]["before"], Value::Null);
    assert_eq!(
        full["events"][1]["origin_scope"],
        "tracked_file_first_parent_path"
    );
    assert_eq!(full["events"][1]["cross_file_origin_unknown"], true);
    assert_eq!(
        full["coverage"]["history_stop_reason"],
        "tracked_path_start"
    );
    assert_eq!(full["coverage"]["history_truncated"], false);

    // The path-lineage start lands on the final allowed commit while older first-parent
    // history exists. Reaching that boundary is not window truncation.
    let (boundary, _) = run(
        r,
        "history",
        "target",
        &["--file", "new.py", "--max-commits", "2"],
    );
    assert_eq!(boundary["coverage"]["commits_inspected"], 2);
    assert_eq!(
        boundary["coverage"]["history_stop_reason"],
        "tracked_path_start"
    );
    assert_eq!(boundary["coverage"]["history_truncated"], false);

    // A copied symbol in another file begins a separate tracked path lineage.
    write(r, "copy.py", "def target(): return 2\n");
    commit(r, "copy target to another path");
    let (copy, _) = run(r, "history", "target", &["--file", "copy.py"]);
    assert_eq!(copy["events"][0]["kind"], "path_lineage_start");
    assert_eq!(copy["events"][0]["cross_file_origin_unknown"], true);
    write(r, "destination.py", "def placeholder(): pass\n");
    commit(r, "create destination path");
    write(r, "destination.py", "def target(): return 2\n");
    commit(r, "move symbol into existing path");
    let (moved, _) = run(r, "history", "target", &["--file", "destination.py"]);
    assert_eq!(moved["events"][0]["git_status"], "modified");
    assert_eq!(moved["events"][0]["kind"], "path_lineage_start");
    assert_eq!(moved["events"][0]["cross_file_origin_unknown"], true);

    let root_only = TempDir::new().unwrap();
    let root = root_only.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "test@example.test"]);
    git(root, &["config", "user.name", "Fixture"]);
    write(root, "root.py", "def target(): pass\n");
    let root_commit = commit(root, "first-parent root");
    let (root_history, _) = run(root, "history", "target", &[]);
    assert_eq!(root_history["events"][0]["commit"], root_commit);
    assert_eq!(root_history["events"][0]["first_parent"], Value::Null);
    assert_eq!(root_history["events"][0]["kind"], "path_lineage_start");
    assert_eq!(root_history["coverage"]["history_truncated"], false);
}

#[test]
fn a_coverage_gap_at_the_last_window_commit_is_not_window_truncation() {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.email", "test@example.test"]);
    git(r, &["config", "user.name", "Fixture"]);
    write(r, "notes.md", "root\n");
    commit(r, "root");
    write(r, "target.py", "def target(): return 0\n");
    commit(r, "introduce target");
    write(r, "target.py", "def target(): return (\n");
    commit(r, "damage syntax");
    write(r, "target.py", "def target(): return 1\n");
    commit(r, "repair syntax");
    write(r, "target.py", "def target(): return 2\n");
    commit(r, "edit after repair");

    let (v, _) = run(r, "history", "target", &["--max-commits", "2"]);
    assert_eq!(v["events"].as_array().unwrap().len(), 2);
    assert_eq!(v["events"][1]["kind"], "comparison_unavailable");
    assert_eq!(v["coverage"]["history_stop_reason"], "coverage_gap");
    assert_eq!(v["coverage"]["history_truncated"], false);
}

#[test]
fn tracked_descriptor_ambiguity_at_window_boundary_is_a_coverage_stop() {
    let f = fixture();
    let r = f.dir.path().join("scope");
    write(
        &r,
        "new.py",
        "class Box:\n    def run(self): pass\n    def run(self): return 1\n",
    );
    commit(f.dir.path(), "duplicate tracked descriptor");
    write(&r, "new.py", "class Box:\n    def run(self): return 2\n");
    commit(f.dir.path(), "restore unique descriptor");
    write(&r, "new.py", "class Box:\n    def run(self): return 3\n");
    commit(f.dir.path(), "continue after repair");

    let (v, _) = run(r.as_path(), "history", "Box.run", &["--max-commits", "2"]);
    assert_eq!(v["events"].as_array().unwrap().len(), 2);
    assert_eq!(v["events"][0]["kind"], "modified");
    assert_eq!(v["events"][1]["kind"], "ambiguous");
    assert_eq!(v["coverage"]["history_stop_reason"], "coverage_gap");
    assert_eq!(v["coverage"]["history_truncated"], false);
}

#[test]
fn partial_symbol_delta_totals_are_explicit_without_stopping_unique_lineage() {
    let f = fixture();
    let r = f.dir.path().join("scope");
    write(
        &r,
        "new.py",
        "class Box:\n    def run(self):\n        return 3\n\ndef helper(): pass\ndef helper(): pass\ndef recent(): pass\n",
    );
    commit(
        f.dir.path(),
        "ambiguous unrelated group and unique addition",
    );
    let (v, _) = run(&r, "history", "Box.run", &[]);
    let structural = &v["events"][0]["structural"];
    assert_eq!(structural["comparison_available"], true);
    assert_eq!(structural["symbol_delta_complete"], false);
    assert_eq!(structural["ambiguous_keys"], 1);
    assert_eq!(structural["observed_symbols_added"], 1);
    assert_eq!(
        v["reported_event_file_totals"]["symbol_deltas_complete"],
        false
    );
    assert_eq!(
        v["reported_event_file_totals"]["symbol_delta_incomplete_files"],
        1
    );
    assert_eq!(
        v["reported_event_file_totals"]["ambiguous_symbol_keys_omitted"],
        1
    );
    // The partial whole-file delta did not make the unique Box.run descriptor ambiguous.
    assert_eq!(v["coverage"]["history_stop_reason"], "tracked_path_start");
    assert!(
        v["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "modified")
    );
}

#[test]
fn head_lookup_fails_closed_on_supported_source_gaps_but_honors_file_filter() {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.email", "test@example.test"]);
    git(r, &["config", "user.name", "Fixture"]);
    write(r, "a.py", "def target(): pass\n");
    write(
        r,
        "large.py",
        format!("def other(): pass\n{}", "# padding\n".repeat(200)),
    );
    write(r, "README.md", "notes\n");
    commit(r, "source inventory");

    ctx()
        .args(["history", "target"])
        .arg(r)
        .args(["--max-parse-file-size", "1024"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "supported Rust/Python source file(s)",
        ));
    let (v, _) = run(
        r,
        "history",
        "target",
        &["--file", "a.py", "--max-parse-file-size", "1024"],
    );
    assert_eq!(v["coverage"]["head_supported_source_gaps"], 1);
    assert_eq!(v["coverage"]["head_expected_exclusions"], 1);
    assert_eq!(v["coverage"]["head_lookup_scope"], "selected_file");
    assert_eq!(v["coverage"]["head_resolution_complete"], true);
}

#[cfg(unix)]
#[test]
fn non_utf8_history_paths_have_distinct_native_identities() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.email", "test@example.test"]);
    git(r, &["config", "user.name", "Fixture"]);
    write(r, "target.py", "def target(): return 1\n");
    let first = PathBuf::from(OsString::from_vec(b"a\xff.py".to_vec()));
    let second = PathBuf::from(OsString::from_vec(b"a\xfe.py".to_vec()));
    // Write native names directly; the string helper intentionally accepts UTF-8 only.
    if let Err(error) = fs::write(r.join(&first), "def old(): return 1\n") {
        if cfg!(target_os = "macos") && error.raw_os_error() == Some(92) {
            // APFS cannot represent this native fixture. Injected native identity
            // tests in history/output/store still cover codec distinctions.
            return;
        }
        panic!("failed to create native history fixture: {error}");
    }
    fs::write(r.join(&second), "def other(): return 1\n").unwrap();
    commit(r, "add colliding display names");
    fs::write(r.join("target.py"), "def target(): return 2\n").unwrap();
    fs::write(r.join(&first), "def old(): return 2\n").unwrap();
    fs::write(r.join(&second), "def other(): return 2\n").unwrap();
    commit(r, "change target and companions");

    let (history, _) = run(r, "history", "target", &[]);
    let cochanges = history["cochanges"].as_array().unwrap();
    let colliding: Vec<_> = cochanges.iter().filter(|c| c["path"] == "a�.py").collect();
    assert_eq!(colliding.len(), 2);
    assert_eq!(
        colliding[0]["native_path_identity"]["encoding"],
        "unix_bytes"
    );
    assert_ne!(
        colliding[0]["native_path_identity"]["hex"],
        colliding[1]["native_path_identity"]["hex"]
    );

    let (evolution, _) = run(r, "evolution", ".", &[]);
    let events = evolution["events"].as_array().unwrap();
    let colliding_before: Vec<_> = events
        .iter()
        .filter_map(|e| e["before"].as_object())
        .filter(|e| e["path"] == "a�.py")
        .collect();
    assert!(colliding_before.len() >= 2);
    assert_ne!(
        colliding_before[0]["native_path_identity"]["hex"],
        colliding_before[1]["native_path_identity"]["hex"]
    );
    let colliding_entries: Vec<_> = events
        .iter()
        .filter_map(|e| e["after"].as_object())
        .filter(|e| e["path"] == "a�.py")
        .collect();
    assert!(colliding_entries.len() >= 2);
    assert_ne!(
        colliding_entries[0]["native_path_identity"]["hex"],
        colliding_entries[1]["native_path_identity"]["hex"]
    );
}

#[cfg(unix)]
#[test]
fn head_inventory_reuses_tree_sizes_instead_of_spawning_size_queries_per_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let r = dir.path().join("repo");
    fs::create_dir_all(&r).unwrap();
    git(&r, &["init", "-q", "-b", "main"]);
    git(&r, &["config", "user.email", "test@example.test"]);
    git(&r, &["config", "user.name", "Fixture"]);
    for i in 0..24 {
        write(
            &r,
            &format!("inventory_{i}.py"),
            format!("def f_{i}(): pass\n"),
        );
    }
    commit(&r, "inventory");
    write(&r, "target.py", "def target(): pass\n");
    commit(&r, "add target");

    let real_git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|path| path.join("git"))
        .find(|path| path.is_file())
        .unwrap();
    let wrapper_dir = dir.path().join("bin");
    fs::create_dir_all(&wrapper_dir).unwrap();
    let wrapper = wrapper_dir.join("git");
    let log = dir.path().join("size-queries.log");
    fs::write(
        &wrapper,
        "#!/bin/sh\ncase \"$*\" in *\"cat-file -s\"*) printf 'size\\n' >> \"$CTX_TEST_SIZE_LOG\";; esac\nexec \"$CTX_TEST_REAL_GIT\" \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let original_path = std::env::var_os("PATH").unwrap();
    let wrapped_path = std::env::join_paths(
        std::iter::once(wrapper_dir).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();

    ctx()
        .args(["history", "target"])
        .arg(&r)
        .args(["--max-commits", "1"])
        .env("PATH", wrapped_path)
        .env("CTX_TEST_REAL_GIT", real_git)
        .env("CTX_TEST_SIZE_LOG", &log)
        .assert()
        .success();
    let count = fs::read_to_string(log).unwrap().lines().count();
    // Only the changed raw-diff entry needs a targeted size query; the 24-file HEAD
    // inventory already carries trustworthy sizes from `ls-tree -l`.
    assert_eq!(count, 1);
}
