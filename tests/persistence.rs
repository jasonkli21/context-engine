mod support;
use std::{fs, path::Path};
use support::ctx;
fn run(root: &Path, cache: &Path, args: &[&str]) -> Vec<u8> {
    ctx()
        .env("CONTEXT_ENGINE_CACHE_DIR", cache)
        .args(args)
        .arg(root)
        .args(["--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone()
}
fn database(cache: &Path) -> std::path::PathBuf {
    fs::read_dir(cache.join("indexes"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "sqlite3"))
        .unwrap()
}
fn git(root: &Path, args: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .unwrap()
            .success()
    );
}
#[test]
fn cold_warm_equivalence_staleness_corruption_and_external_cache() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("a.py"),
        "def one():\n    two()\ndef two(): pass\n",
    )
    .unwrap();
    run(root.path(), cache.path(), &["index"]);
    let db = database(cache.path());
    for args in [
        vec!["symbols"],
        vec!["symbol", "one"],
        vec!["graph"],
        vec!["deps", "one"],
        vec!["callers", "two"],
        vec!["related", "one"],
        vec!["search", "one"],
        vec!["neighborhood", "--symbol", "one"],
        vec!["hotspots"],
        vec!["context", "--task", "one", "--budget", "1000"],
    ] {
        run(root.path(), cache.path(), &["index"]);
        let before = fs::metadata(&db).unwrap().modified().unwrap();
        let first = run(root.path(), cache.path(), &args);
        let second = run(root.path(), cache.path(), &args);
        assert_eq!(first, second);
        assert_eq!(
            before,
            fs::metadata(&db).unwrap().modified().unwrap(),
            "warm command rewrote database: {args:?}"
        );
        // Missing-cache composition must yield the same public result.
        fs::remove_file(&db).unwrap();
        assert_eq!(first, run(root.path(), cache.path(), &args));
    }
    fs::write(
        root.path().join("a.py"),
        "def six():\n    two()\ndef two(): pass\n",
    )
    .unwrap();
    let output = run(root.path(), cache.path(), &["symbols"]);
    assert!(String::from_utf8_lossy(&output).contains("six"));
    assert!(!String::from_utf8_lossy(&output).contains("one"));
    fs::write(root.path().join("b.rs"), "fn fresh() {}").unwrap();
    assert!(
        String::from_utf8_lossy(&run(root.path(), cache.path(), &["symbols"])).contains("fresh")
    );
    fs::remove_file(root.path().join("b.rs")).unwrap();
    assert!(
        !String::from_utf8_lossy(&run(root.path(), cache.path(), &["symbols"])).contains("fresh")
    );
    fs::write(&db, b"not sqlite").unwrap();
    serde_json::from_slice::<serde_json::Value>(&run(root.path(), cache.path(), &["graph"]))
        .unwrap();
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn clean_git_context_change_and_live_metadata_survive_cache() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["config", "user.email", "test@example.com"]);
    git(root.path(), &["config", "user.name", "Test"]);
    fs::write(root.path().join("a.py"), "def old(): pass\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "base"]);
    fs::write(root.path().join("a.py"), "def new(): pass\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "head"]);
    run(root.path(), cache.path(), &["index"]);
    let status = || {
        std::process::Command::new("git")
            .current_dir(root.path())
            .args(["status", "--porcelain"])
            .output()
            .unwrap()
            .stdout
    };
    assert!(status().is_empty());
    let args = [
        "context",
        "--task",
        "new",
        "--budget",
        "1000",
        "--change",
        "HEAD~1..HEAD",
    ];
    assert_eq!(
        run(root.path(), cache.path(), &args),
        run(root.path(), cache.path(), &args)
    );
    assert!(status().is_empty());
    fs::write(root.path().join("README.md"), "dirty").unwrap();
    ctx()
        .env("CONTEXT_ENGINE_CACHE_DIR", cache.path())
        .args(args)
        .arg(root.path())
        .assert()
        .failure()
        .stdout("");
}
#[test]
fn storage_failure_and_override_validation_do_not_pollute_stdout_or_repository() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a.rs"), "fn one() {}").unwrap();
    let blocked = cache.path().join("file");
    fs::write(&blocked, "blocked").unwrap();
    ctx()
        .env("CONTEXT_ENGINE_CACHE_DIR", &blocked)
        .arg("index")
        .arg(root.path())
        .assert()
        .failure()
        .stdout("");
    let out = ctx()
        .env("CONTEXT_ENGINE_CACHE_DIR", &blocked)
        .arg("symbols")
        .arg(root.path())
        .args(["--format", "json"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(String::from_utf8_lossy(&out.stderr).contains("warning"));
    serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap();
    for path in [
        std::path::PathBuf::from("relative"),
        root.path().join("cache"),
    ] {
        ctx()
            .env("CONTEXT_ENGINE_CACHE_DIR", path)
            .arg("index")
            .arg(root.path())
            .assert()
            .failure()
            .stdout("");
    }
    assert!(!root.path().join("cache").exists());
}

#[test]
fn incremental_consumers_match_force_rebuild_and_old_schema_recovers() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::write(root.path().join("stable.rs"), "fn retained() {}\n").unwrap();
    fs::write(
        root.path().join("a.py"),
        "import os\ndef old():\n    old()\n\ndef broken(\n",
    )
    .unwrap();
    run(root.path(), cache.path(), &["index"]);
    let db = database(cache.path());
    fs::write(root.path().join("a.py"), "def current(): pass\n").unwrap();
    fs::write(root.path().join("b.py"), "def added(): pass\n").unwrap();
    let commands: &[&[&str]] = &[
        &["symbols"],
        &["graph"],
        &["search", "old"],
        &["neighborhood", "--symbol", "current"],
        &["hotspots"],
        &["context", "--task", "current", "--budget", "1000"],
    ];
    let incremented: Vec<_> = commands
        .iter()
        .map(|args| run(root.path(), cache.path(), args))
        .collect();
    let conn = rusqlite::Connection::open(&db).unwrap();
    for table in ["candidates", "diagnostics"] {
        let count: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "stale {table} survived");
    }
    drop(conn);
    run(root.path(), cache.path(), &["index"]);
    for (args, expected) in commands.iter().zip(incremented) {
        assert_eq!(expected, run(root.path(), cache.path(), args));
    }
    fs::rename(root.path().join("b.py"), root.path().join("renamed.py")).unwrap();
    fs::remove_file(root.path().join("a.py")).unwrap();
    let renamed = run(root.path(), cache.path(), &["graph"]);
    let text = String::from_utf8_lossy(&renamed);
    assert!(text.contains("renamed.py"));
    assert!(!text.contains("b.py") && !text.contains("a.py"));
    run(root.path(), cache.path(), &["index"]);
    assert_eq!(renamed, run(root.path(), cache.path(), &["graph"]));
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("PRAGMA user_version=2")
        .unwrap();
    assert_eq!(renamed, run(root.path(), cache.path(), &["graph"]));
    let schema: i64 = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(schema, 3);
}

#[test]
fn watch_help_rejects_machine_stream_and_missing_root() {
    ctx()
        .args(["watch", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--max-parse-file-size"));
    ctx()
        .args(["watch", "--format", "json"])
        .assert()
        .failure()
        .stdout("");
    let root = tempfile::tempdir().unwrap();
    ctx()
        .arg("watch")
        .arg(root.path().join("missing"))
        .assert()
        .failure()
        .stdout("");
}
