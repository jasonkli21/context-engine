mod support;
use predicates::prelude::*;
use serde_json::Value;
use std::{fs, path::Path, process::Command};
use support::ctx;
use tempfile::TempDir;

fn write(root: &Path, path: &str, data: &[u8]) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, data).unwrap();
}
fn json(root: &Path, command: &str, flags: &[&str]) -> (Value, Vec<u8>) {
    let output = ctx()
        .arg(command)
        .arg(root)
        .args(["--format", "json"])
        .args(flags)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    (serde_json::from_slice(&output).unwrap(), output)
}
fn fixture() -> TempDir {
    let temp = TempDir::new().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mixed-repo");
    for name in [
        ".gitignore",
        ".ignore",
        "Cargo.toml",
        "README.md",
        "src/main.rs",
        "src/helper.py",
        "tests/cli.rs",
        "docs/architecture.md",
        "migrations/001.sql",
        "schemas/api.yaml",
    ] {
        write(temp.path(), name, &fs::read(source.join(name)).unwrap());
    }
    // Empty directories and ignored content are created dynamically (Git cannot store empty dirs).
    fs::create_dir(temp.path().join("empty")).unwrap();
    write(temp.path(), "ignored.log", b"ignored\n");
    write(temp.path(), "omit.tmp", b"ignored\n");
    write(temp.path(), ".git/objects/fake", b"excluded\n");
    write(temp.path(), "unknown.zzz", b"unknown\n");
    temp
}
#[test]
fn help_version_and_default_commands() {
    ctx().arg("--help").assert().success().stdout(
        predicate::str::contains("scan")
            .and(predicate::str::contains("stats"))
            .and(predicate::str::contains("tree")),
    );
    ctx()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("ctx 0.1.0"));
    let temp = TempDir::new().unwrap();
    write(temp.path(), "a.rs", b"x\n");
    for command in ["scan", "stats", "tree"] {
        let help = ctx()
            .args([command, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let help = String::from_utf8(help).unwrap();
        assert!(help.contains("--format"));
        if command == "tree" {
            assert!(!help.contains("--no-lines"));
            assert!(!help.contains("--max-line-file-size"));
        }
        let output = ctx()
            .current_dir(temp.path())
            .args([command, "--format", "json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let v: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["command"], command);
        assert_eq!(
            v["data"]["root"],
            temp.path().canonicalize().unwrap().to_str().unwrap()
        );
    }
    ctx().args(["scan", "--format", "xml"]).assert().failure();
    ctx()
        .args(["scan", "--max-line-file-size", "bad"])
        .assert()
        .failure();
}
#[test]
fn invalid_roots() {
    let temp = TempDir::new().unwrap();
    for command in ["scan", "stats", "tree"] {
        ctx()
            .arg(command)
            .arg(temp.path().join("missing"))
            .assert()
            .failure()
            .stderr(predicate::str::contains("scan root"));
    }
    write(temp.path(), "file.rs", b"x");
    ctx()
        .arg("scan")
        .arg(temp.path().join("file.rs"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));
}
#[test]
fn mixed_inventory_counts_ignores_and_stability() {
    let temp = fixture();
    // Fake .git is excluded but not a valid Git repository.
    let (value, bytes) = json(temp.path(), "scan", &[]);
    let data = &value["data"];
    assert_eq!(data["directory_count"], 6);
    let files = data["files"].as_array().unwrap();
    assert_eq!(files.len(), 11);
    let paths: Vec<_> = files.iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(
        paths,
        vec![
            ".gitignore",
            ".ignore",
            "Cargo.toml",
            "README.md",
            "docs/architecture.md",
            "migrations/001.sql",
            "schemas/api.yaml",
            "src/helper.py",
            "src/main.rs",
            "tests/cli.rs",
            "unknown.zzz"
        ]
    );
    assert!(paths.iter().all(|p| !Path::new(p).is_absolute()));
    assert_eq!(data["totals"]["files"], 11);
    let total_bytes: u64 = paths
        .iter()
        .map(|p| fs::metadata(temp.path().join(p)).unwrap().len())
        .sum();
    assert_eq!(data["totals"]["bytes"], total_bytes);
    assert_eq!(data["totals"]["counted_lines"], 10);
    assert_eq!(data["totals"]["files_with_line_counts"], 8);
    let unknown = files.iter().find(|f| f["path"] == "unknown.zzz").unwrap();
    assert_eq!(unknown["language"], "Unknown");
    assert!(unknown["line_count"].is_null());
    assert!(data["git"].is_null());
    assert_eq!(json(temp.path(), "scan", &[]).1, bytes);
    let (stats, _) = json(temp.path(), "stats", &[]);
    assert_eq!(stats["data"]["totals"], data["totals"]);
    assert!(stats["data"].get("files").is_none());
    let (tree, tree_bytes) = json(temp.path(), "tree", &[]);
    let entries = tree["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 17);
    assert!(
        entries
            .iter()
            .any(|e| e["path"] == "empty" && e["kind"] == "directory")
    );
    assert_eq!(json(temp.path(), "tree", &[]).1, tree_bytes);
    let output = ctx()
        .arg("tree")
        .arg(temp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second = ctx()
        .arg("tree")
        .arg(temp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(output, second);
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("empty/\n"));
    assert!(!text.contains("ignored.log"));
    ctx()
        .arg("scan")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Repository:")
                .and(predicate::str::contains("Summary"))
                .and(predicate::str::contains("Languages"))
                .and(predicate::str::contains("Roles"))
                .and(predicate::str::contains("Largest files"))
                .and(predicate::str::contains("Git")),
        );
    ctx()
        .arg("stats")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Statistics").and(predicate::str::contains("counted lines")),
        );
}
#[test]
fn line_limits_and_recoverable_invalid_text() {
    let temp = TempDir::new().unwrap();
    for (name, data) in [
        ("empty.rs", &b""[..]),
        ("final.rs", &b"a\nb"[..]),
        ("newline.rs", &b"a\n"[..]),
        ("invalid.rs", &b"\xff"[..]),
    ] {
        write(temp.path(), name, data);
    }
    let (v, _) = json(temp.path(), "scan", &["--max-line-file-size", "3"]);
    let files = v["data"]["files"].as_array().unwrap();
    assert_eq!(files[0]["line_count"], 0);
    assert_eq!(files[1]["line_count"], 2); // exact threshold is included
    assert!(files[2]["line_count"].is_null());
    assert_eq!(files[3]["line_count"], 1);
    assert_eq!(v["data"]["warnings"].as_array().unwrap().len(), 1);
    assert_eq!(v["data"]["warnings"][0]["path"], "invalid.rs");
    assert_eq!(
        json(temp.path(), "scan", &["--max-line-file-size", "3"]).1,
        json(temp.path(), "scan", &["--max-line-file-size", "3"]).1
    );
    let (v, _) = json(temp.path(), "scan", &["--max-line-file-size", "2"]);
    assert!(v["data"]["files"][1]["line_count"].is_null());
    let (v, _) = json(temp.path(), "scan", &["--no-lines"]);
    assert!(
        v["data"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["line_count"].is_null())
    );
    assert_eq!(v["data"]["totals"]["files_with_line_counts"], 0);
    ctx()
        .arg("scan")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Warnings").and(predicate::str::contains("invalid.rs")));
}
fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn git_metadata_unborn_clean_dirty_detached_and_subdirectory() {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("Git unavailable; metadata test skipped");
        return;
    }
    let temp = TempDir::new().unwrap();
    git(temp.path(), &["init", "--initial-branch=main"]);
    let (v, _) = json(temp.path(), "scan", &[]);
    assert_eq!(v["data"]["git"]["branch"], "main");
    assert!(v["data"]["git"]["head_commit"].is_null());
    write(temp.path(), "src/a.py", b"a\n");
    git(temp.path(), &["add", "."]);
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-m",
            "fixture",
        ],
    );
    let (v, _) = json(temp.path(), "scan", &[]);
    assert_eq!(v["data"]["git"]["is_dirty"], false);
    assert_eq!(v["data"]["git"]["head_commit"].as_str().unwrap().len(), 40);
    write(temp.path(), "src/a.py", b"a\nb\n");
    let (v, _) = json(&temp.path().join("src"), "scan", &[]);
    assert_eq!(v["data"]["files"][0]["path"], "a.py");
    assert_eq!(
        v["data"]["git"]["repository_root"],
        temp.path().canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(v["data"]["git"]["is_dirty"], true);
    git(temp.path(), &["checkout", "--detach"]);
    let (v, _) = json(temp.path(), "scan", &[]);
    assert!(v["data"]["git"]["branch"].is_null());
    assert_eq!(v["data"]["warnings"], serde_json::json!([]));
}
#[test]
fn missing_git_is_optional() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "a.py", b"a\n");
    let output = ctx()
        .env("PATH", temp.path())
        .arg("scan")
        .arg(temp.path())
        .args(["--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(v["data"]["totals"]["files"], 1);
    assert!(v["data"]["git"].is_null());
    assert!(
        v["data"]["warnings"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Git metadata unavailable")
    );
    assert!(v["data"]["warnings"][0]["path"].is_null());
}
#[cfg(unix)]
#[test]
fn symlinks_and_failed_git() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(outside.path(), "secret.rs", b"not traversed");
    write(temp.path(), "normal.rs", b"a\n");
    symlink(outside.path(), temp.path().join("linked-dir")).unwrap();
    symlink(
        outside.path().join("secret.rs"),
        temp.path().join("linked-file.rs"),
    )
    .unwrap();
    symlink(temp.path(), temp.path().join("loop")).unwrap();
    let (v, _) = json(temp.path(), "scan", &[]);
    assert_eq!(v["data"]["totals"]["files"], 1);
    assert_eq!(v["data"]["directory_count"], 0);
    let bin = TempDir::new().unwrap();
    write(
        bin.path(),
        "git",
        b"#!/bin/sh\necho simulated-git-failure >&2\nexit 2\n",
    );
    fs::set_permissions(bin.path().join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let output = ctx()
        .env("PATH", bin.path())
        .arg("scan")
        .arg(temp.path())
        .args(["--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&output).unwrap();
    assert!(v["data"]["git"].is_null());
    assert!(v["data"]["warnings"].as_array().unwrap().iter().any(|w| {
        w["message"]
            .as_str()
            .unwrap()
            .contains("simulated-git-failure")
    }));
}

#[test]
fn nested_ignore_precedence_and_parent_rules() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), ".gitignore", b"*.log\n");
    write(temp.path(), "sub/.gitignore", b"!keep.log\n");
    write(temp.path(), "sub/.ignore", b"keep.log\n");
    write(temp.path(), "sub/keep.log", b"ignored by .ignore\n");
    write(temp.path(), "sub/drop.log", b"ignored by parent\n");
    write(temp.path(), "sub/visible.rs", b"a\n");
    let (v, _) = json(&temp.path().join("sub"), "scan", &[]);
    let paths: Vec<_> = v["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec![".gitignore", ".ignore", "visible.rs"]);
    fs::remove_file(temp.path().join("sub/.ignore")).unwrap();
    let (v, _) = json(&temp.path().join("sub"), "scan", &[]);
    let paths: Vec<_> = v["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec![".gitignore", "keep.log", "visible.rs"]);
}

#[cfg(unix)]
#[test]
fn symlinked_ignore_controls_fail_closed_at_root_and_in_parents() {
    use std::os::unix::fs::symlink;

    for name in [".gitignore", ".ignore"] {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        write(outside.path(), "patterns", b"hidden.rs\n");
        symlink(outside.path().join("patterns"), temp.path().join(name)).unwrap();
        write(
            temp.path(),
            "hidden.rs",
            b"must not be hidden by outside data\n",
        );

        let result = ctx()
            .arg("scan")
            .arg(temp.path())
            .args(["--format", "json"])
            .assert()
            .failure();
        let stderr = String::from_utf8_lossy(&result.get_output().stderr);
        assert!(stderr.contains("symlinked ignore file"), "{stderr}");
        assert!(stderr.contains(name), "{stderr}");
        assert!(stderr.contains("scan boundary"), "{stderr}");
    }

    let parent = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let scan_root = parent.path().join("sub");
    fs::create_dir(&scan_root).unwrap();
    write(outside.path(), "patterns", b"hidden.rs\n");
    symlink(
        outside.path().join("patterns"),
        parent.path().join(".gitignore"),
    )
    .unwrap();
    write(
        &scan_root,
        "hidden.rs",
        b"must not be hidden by outside data\n",
    );

    let result = ctx()
        .arg("scan")
        .arg(&scan_root)
        .args(["--format", "json"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&result.get_output().stderr);
    assert!(stderr.contains("symlinked ignore file"), "{stderr}");
    assert!(stderr.contains(".gitignore"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn unreadable_file_and_directory_degrade_to_warnings() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    write(temp.path(), "denied.rs", b"a\n");
    write(temp.path(), "closed/a.py", b"a\n");
    fs::set_permissions(
        temp.path().join("denied.rs"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    fs::set_permissions(
        temp.path().join("closed"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let permissions_enforced = fs::File::open(temp.path().join("denied.rs")).is_err();
    let (v, _) = json(temp.path(), "scan", &[]);
    let (tree, _) = json(temp.path(), "tree", &[]);
    assert!(
        tree["data"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == "denied.rs")
    );
    assert!(
        tree["data"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|warning| warning["path"] != "denied.rs")
    );
    // Restore permissions even when running as a privileged user, allowing cleanup.
    fs::set_permissions(
        temp.path().join("denied.rs"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::set_permissions(
        temp.path().join("closed"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    if permissions_enforced {
        assert_eq!(v["data"]["files"][0]["path"], "denied.rs");
        assert!(v["data"]["files"][0]["line_count"].is_null());
        let warnings = v["data"]["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0]["path"], "closed");
        assert_eq!(warnings[1]["path"], "denied.rs");
    } else {
        eprintln!(
            "privileged user bypasses filesystem permissions; I/O failure covered by unit test"
        );
    }
}

// macOS filesystems reject arbitrary byte names; output's unit test covers that boundary.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_utf8_filename_scan() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path()
            .join(OsString::from_vec(b"invalid-\xff.rs".to_vec())),
        b"a\n",
    )
    .unwrap();
    let (v, _) = json(temp.path(), "scan", &[]);
    assert_eq!(v["data"]["totals"]["files"], 1);
    assert!(
        v["data"]["warnings"][0]["message"]
            .as_str()
            .unwrap()
            .contains("non-UTF-8")
    );
}

#[test]
fn default_size_threshold_and_largest_file_ordering() {
    let temp = TempDir::new().unwrap();
    write(temp.path(), "large.rs", &vec![b'x'; 1024 * 1024 + 1]);
    write(temp.path(), "b.py", b"a\n");
    write(temp.path(), "a.rs", b"a\n");
    let (v, _) = json(temp.path(), "scan", &[]);
    assert_eq!(v["data"]["largest_files"][0]["path"], "large.rs");
    assert!(v["data"]["largest_files"][0]["line_count"].is_null());
    assert_eq!(v["data"]["largest_files"][1]["path"], "a.rs");
    assert_eq!(v["data"]["largest_files"][2]["path"], "b.py");
    assert_eq!(v["data"]["totals"]["counted_lines"], 2);
    assert!(v["data"]["warnings"].as_array().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn git_fsmonitor_in_target_is_not_executed() {
    use std::os::unix::fs::PermissionsExt;
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let temp = TempDir::new().unwrap();
    git(temp.path(), &["init", "--initial-branch=main"]);
    write(
        temp.path(),
        "monitor.sh",
        b"#!/bin/sh\ntouch executed-marker\n",
    );
    fs::set_permissions(
        temp.path().join("monitor.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    git(temp.path(), &["config", "core.fsmonitor", "./monitor.sh"]);
    let (v, _) = json(temp.path(), "scan", &[]);
    assert_eq!(v["data"]["git"]["is_dirty"], true);
    assert!(!temp.path().join("executed-marker").exists());
}

#[cfg(unix)]
#[test]
fn git_filter_configuration_skips_dirty_probe_without_running_filter() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let temp = TempDir::new().unwrap();
    git(temp.path(), &["init", "--initial-branch=main"]);
    write(temp.path(), ".gitattributes", b"*.txt filter=probe\n");
    write(temp.path(), "tracked.txt", b"before\n");
    git(temp.path(), &["add", "."]);
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-m",
            "fixture",
        ],
    );
    let marker = temp.path().join("filter-was-run");
    let clean = format!("touch '{}'; cat", marker.display());
    git(temp.path(), &["config", "filter.probe.clean", &clean]);
    write(temp.path(), "tracked.txt", b"after\n");

    let (value, first) = json(temp.path(), "scan", &[]);
    let (second_value, second) = json(temp.path(), "scan", &[]);
    assert_eq!(first, second);
    assert_eq!(value, second_value);
    assert!(!marker.exists());
    assert_eq!(
        value["data"]["git"]["repository_root"],
        temp.path().canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(value["data"]["git"]["branch"], "main");
    assert!(value["data"]["git"]["head_commit"].is_string());
    assert!(value["data"]["git"]["is_dirty"].is_null());
    let warning = value["data"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|warning| {
            warning["message"]
                == "Git dirty state unavailable because external Git filters are configured"
        })
        .expect("filter safety warning");
    assert!(warning["path"].is_null());
}

#[cfg(unix)]
#[test]
fn git_filter_safety_check_failure_skips_status() {
    use std::os::unix::fs::PermissionsExt;

    let real_git = Command::new("git").arg("--version").output();
    if real_git.is_err() {
        return;
    }
    let real_git = std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("git"))
                .find(|git| git.is_file())
        })
        .expect("the git executable used by the test");
    let temp = TempDir::new().unwrap();
    git(temp.path(), &["init", "--initial-branch=main"]);
    write(temp.path(), "tracked.rs", b"x\n");
    git(temp.path(), &["add", "."]);
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-m",
            "fixture",
        ],
    );

    let bin = TempDir::new().unwrap();
    write(
        bin.path(),
        "git",
        b"#!/bin/sh\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    config) echo config-check-failed >&2; exit 2 ;;\n    status) printf x > \"$CTX_GIT_MARKER\"; exit 0 ;;\n  esac\ndone\nexec \"$CTX_REAL_GIT\" \"$@\"\n",
    );
    fs::set_permissions(bin.path().join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let marker = temp.path().join("status-was-run");
    let output = ctx()
        .env("PATH", bin.path())
        .env("CTX_REAL_GIT", real_git)
        .env("CTX_GIT_MARKER", &marker)
        .arg("scan")
        .arg(temp.path())
        .args(["--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert!(!marker.exists());
    assert_eq!(value["data"]["git"]["branch"], "main");
    assert!(value["data"]["git"]["is_dirty"].is_null());
    assert!(value["data"]["warnings"].as_array().unwrap().iter().any(|warning| {
        warning["message"]
            == "Git dirty state unavailable because Git filter configuration could not be checked safely"
            && warning["path"].is_null()
    }));
}

#[cfg(unix)]
#[test]
fn tree_does_not_read_file_contents_or_invoke_git() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    write(temp.path(), "invalid.rs", b"\xff");
    let bin = TempDir::new().unwrap();
    write(
        bin.path(),
        "git",
        b"#!/bin/sh\nprintf x > \"$CTX_GIT_MARKER\"\nexit 1\n",
    );
    fs::set_permissions(bin.path().join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let marker = temp.path().join("tree-ran-git");
    let output = ctx()
        .env("PATH", bin.path())
        .env("CTX_GIT_MARKER", &marker)
        .arg("tree")
        .arg(temp.path())
        .args(["--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["data"]["entries"][0]["path"], "invalid.rs");
    assert_eq!(value["data"]["warnings"], serde_json::json!([]));
    assert!(!marker.exists());
}

#[cfg(unix)]
#[test]
fn human_tree_escapes_control_characters_in_repository_filenames() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let temp = TempDir::new().unwrap();
    let name = OsString::from_vec(b"bad\n\x1b.rs".to_vec());
    fs::write(temp.path().join(name), b"x\n").unwrap();
    let output = ctx()
        .arg("tree")
        .arg(temp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("bad\\n\\u{1b}.rs"));
    assert!(!text.contains('\u{1b}'));
    assert_eq!(text.lines().filter(|line| line.contains("bad")).count(), 1);
}

#[test]
fn aggregate_scan_failure_emits_no_partial_json_for_any_pipeline() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), ".ignore", &vec![b' '; 1024 * 1024 + 1]);
    write(dir.path(), "ok.py", b"def helper(): pass\n");
    for command in [
        "scan", "stats", "tree", "index", "symbols", "symbol", "graph", "deps", "callers",
        "related",
    ] {
        let mut cmd = ctx();
        cmd.arg(command);
        if ["symbol", "deps", "callers", "related"].contains(&command) {
            cmd.arg("helper");
        }
        cmd.arg(dir.path())
            .args(["--format", "json"])
            .assert()
            .failure()
            .stdout("")
            .stderr(predicate::str::contains(
                "repository scan safety limit exceeded",
            ));
    }
}
