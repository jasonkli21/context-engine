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
