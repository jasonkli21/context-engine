mod support;
use predicates::prelude::*;
use serde_json::Value;
use std::{fs, path::Path};
use support::ctx;
use tempfile::TempDir;
fn write(root: &Path, path: &str, data: &[u8]) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}
fn run(root: &Path, command: &str, query: Option<&str>, flags: &[&str]) -> (Value, Vec<u8>) {
    let mut cmd = ctx();
    cmd.arg(command);
    if let Some(query) = query {
        cmd.arg(query);
    }
    let bytes = cmd
        .arg(root)
        .args(["--format", "json"])
        .args(flags)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["command"], command);
    (value, bytes)
}
#[test]
fn mixed_index_statuses_counts_and_determinism() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for (path, text) in [
        ("main.rs", "fn one() {}"),
        ("a.py", "def two():\n    pass\n"),
        ("a.ts", "function ignored() {}"),
        ("README.md", "# docs"),
        ("generated/g.rs", "fn generated() {}"),
        ("vendor/v.py", "def vendor(): pass"),
        ("bad.rs", "fn recovered() {}\nfn broken( {"),
        ("large.py", "# oversized\n"),
        (".ignore", "ignored.py\n"),
        ("ignored.py", "def invisible(): pass"),
    ] {
        write(root, path, text.as_bytes());
    }
    write(root, "large.py", &vec![b'#'; 201]);
    write(root, "invalid.py", &[255]);
    let flags = ["--max-parse-file-size", "200"];
    let (value, bytes) = run(root, "index", None, &flags);
    assert_eq!(bytes, run(root, "index", None, &flags).1);
    let data = &value["data"];
    assert_eq!(data["summary"]["discovered_files"], 10);
    assert_eq!(data["summary"]["supported_files"], 7);
    assert_eq!(data["summary"]["analyzable_files"], 5);
    assert_eq!(data["summary"]["analyzed_files"], 3);
    assert_eq!(data["summary"]["skipped_files"], 6);
    assert_eq!(data["summary"]["failed_files"], 1);
    assert_eq!(data["summary"]["syntax_error_files"], 1);
    assert_eq!(data["summary"]["statuses"]["skipped_role"], 2);
    assert_eq!(data["summary"]["statuses"]["too_large"], 1);
    let files = data["files"].as_array().unwrap();
    let paths: Vec<_> = files.iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert!(paths.windows(2).all(|w| w[0] < w[1]));
    assert!(
        files
            .iter()
            .all(|f| !Path::new(f["path"].as_str().unwrap()).is_absolute())
    );
    assert_eq!(
        files
            .iter()
            .map(|f| f["symbol_count"].as_u64().unwrap())
            .sum::<u64>(),
        data["summary"]["symbols"].as_u64().unwrap()
    );
    assert_eq!(data["languages"]["python"], 1);
    assert!(
        data["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["path"].is_string())
    );
    ctx()
        .args(["index"])
        .arg(root)
        .args(flags)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Discovered: 10").and(predicate::str::contains("By kind")),
        );
}

#[test]
fn per_file_analysis_budgets_fail_closed_and_are_deterministic() {
    let dir = TempDir::new().unwrap();
    // The production per-file symbol ceiling is 4,096.
    write(
        dir.path(),
        "flat.rs",
        "fn f() {}\n".repeat(4_097).as_bytes(),
    );

    let mut deep = String::new();
    for _ in 0..160 {
        deep.push_str("mod m {\n");
    }
    deep.push_str("fn leaf() {}\n");
    for _ in 0..160 {
        deep.push_str("}\n");
    }
    write(dir.path(), "deep.rs", deep.as_bytes());

    let (value, bytes) = run(dir.path(), "index", None, &[]);
    assert_eq!(bytes, run(dir.path(), "index", None, &[]).1);
    let data = &value["data"];
    assert_eq!(data["summary"]["symbols"], 0);
    assert_eq!(data["summary"]["failed_files"], 2);
    assert_eq!(data["summary"]["statuses"]["analysis_limit_exceeded"], 2);
    assert_eq!(
        data["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| (
                file["status"].as_str().unwrap(),
                file["symbol_count"].as_u64().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            ("analysis_limit_exceeded", 0),
            ("analysis_limit_exceeded", 0)
        ]
    );
    let diagnostics = data["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic["kind"] == "analysis_limit_exceeded"
            && diagnostic["message"] == "per-file symbol analysis limit exceeded"
            && diagnostic["range"].is_null()
    }));
}
#[test]
fn lookup_modes_multiple_matches_ranges_and_no_fuzzy_search() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "b.py",
        b"class B:\n    def method(self):\n        pass\n",
    );
    write(
        root,
        "a.py",
        b"class A:\n    def method(self):\n        pass\n",
    );
    let (value, bytes) = run(root, "symbols", None, &[]);
    assert_eq!(bytes, run(root, "symbols", None, &[]).1);
    let symbols = value["data"]["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 4);
    for symbol in symbols {
        let source = fs::read(root.join(symbol["path"].as_str().unwrap())).unwrap();
        let start = symbol["name_range"]["start"]["byte"].as_u64().unwrap() as usize;
        let end = symbol["name_range"]["end"]["byte"].as_u64().unwrap() as usize;
        assert_eq!(
            &source[start..end],
            symbol["name"].as_str().unwrap().as_bytes()
        );
        assert_eq!(symbol["language"], "python");
        assert!(symbol["range"]["start"]["byte"].as_u64().unwrap() <= start as u64);
        assert!(symbol["range"]["end"]["byte"].as_u64().unwrap() >= end as u64);
    }
    let (v, _) = run(root, "symbol", Some("A.method"), &[]);
    assert_eq!(v["data"]["match_mode"], "qualified_name");
    assert_eq!(v["data"]["matches"].as_array().unwrap().len(), 1);
    let (v, bytes) = run(root, "symbol", Some("method"), &[]);
    assert_eq!(bytes, run(root, "symbol", Some("method"), &[]).1);
    assert_eq!(v["data"]["match_mode"], "local_name");
    assert_eq!(v["data"]["matches"][0]["path"], "a.py");
    assert_eq!(v["data"]["matches"][1]["path"], "b.py");
    for query in ["meth", "Method", "missing"] {
        let (v, _) = run(root, "symbol", Some(query), &[]);
        assert_eq!(v["data"]["match_mode"], "none");
        assert!(v["data"]["matches"].as_array().unwrap().is_empty());
    }
    // Qualified matching wins even when other symbols have the same local name.
    write(root, "c.py", b"def method(): pass\n");
    let (v, _) = run(root, "symbol", Some("method"), &[]);
    assert_eq!(v["data"]["match_mode"], "qualified_name");
    assert_eq!(v["data"]["matches"].as_array().unwrap().len(), 1);
    ctx()
        .args(["symbol", "missing"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("0 matches"));
    ctx()
        .args(["symbol", "A.method"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("1 match").and(predicate::str::contains("a.py:2:9")));
}
#[test]
fn help_defaults_and_size_flags() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "empty.rs", b"");
    for command in ["index", "symbols", "symbol"] {
        let help = ctx()
            .args([command, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let help = String::from_utf8(help).unwrap();
        assert!(help.contains("--max-parse-file-size"));
        assert!(help.contains("--format"));
        if command == "index" {
            assert!(help.contains("persistent index"));
        }
        let mut cmd = ctx();
        cmd.current_dir(dir.path()).arg(command);
        if command == "symbol" {
            cmd.arg("missing");
        }
        cmd.assert().success();
    }
    let (value, _) = run(dir.path(), "index", None, &["--max-parse-file-size", "0"]);
    assert_eq!(value["data"]["summary"]["analyzed_files"], 1);
    write(dir.path(), "large.rs", &vec![b' '; 1_048_577]);
    let (value, _) = run(dir.path(), "index", None, &[]);
    assert_eq!(value["data"]["summary"]["statuses"]["too_large"], 1);
    ctx()
        .args(["index", "--max-parse-file-size", "-1"])
        .assert()
        .failure();
}
#[test]
fn no_source_execution_or_body_output_and_duplicates_preserved() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("EXECUTED");
    let text = format!(
        "from pathlib import Path\nPath({:?}).write_text('EXECUTED')\ndef safe():\n    return 'BODY_SECRET_123'\ndef safe():\n    pass\n",
        marker
    );
    write(dir.path(), "danger.py", text.as_bytes());
    write(
        dir.path(),
        "build.rs",
        b"fn main() { panic!(\"BODY_SECRET_123\"); }",
    );
    for command in ["index", "symbols", "symbol"] {
        let query = (command == "symbol").then_some("safe");
        let (value, bytes) = run(dir.path(), command, query, &[]);
        assert!(
            !String::from_utf8(bytes)
                .unwrap()
                .contains("BODY_SECRET_123")
        );
        if command == "symbol" {
            assert_eq!(value["data"]["matches"].as_array().unwrap().len(), 2);
        }
        let mut cmd = ctx();
        cmd.arg(command);
        if let Some(query) = query {
            cmd.arg(query);
        }
        cmd.arg(dir.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("BODY_SECRET_123").not());
    }
    assert!(!marker.exists());
}
#[cfg(unix)]
#[test]
fn symlinks_and_human_controls() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(outside.path(), "outside.py", b"def outside(): pass");
    symlink(
        outside.path().join("outside.py"),
        dir.path().join("link.py"),
    )
    .unwrap();
    symlink(outside.path(), dir.path().join("linked")).unwrap();
    write(dir.path(), "bad\n\u{1b}.py", b"def safe(): pass");
    let (value, _) = run(dir.path(), "index", None, &[]);
    assert_eq!(value["data"]["summary"]["discovered_files"], 1);
    for command in ["index", "symbols", "symbol"] {
        let mut cmd = ctx();
        cmd.arg(command);
        if command == "symbol" {
            cmd.arg("safe\n\u{1b}");
        }
        let out = cmd
            .arg(dir.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains('\u{1b}'));
        if command != "index" {
            assert!(text.contains("\\n\\u{1b}"));
        }
    }
}

#[test]
fn supported_non_source_roles_and_size_boundary_are_analyzed() {
    let dir = TempDir::new().unwrap();
    let text = b"fn found() {}";
    for path in ["tests/a.rs", "migrations/a.rs", "config/a.rs"] {
        write(dir.path(), path, text);
    }
    write(dir.path(), "config/a.py", b"def found(): pass");
    let (v, _) = run(dir.path(), "index", None, &["--max-parse-file-size", "17"]);
    assert_eq!(v["data"]["summary"]["analyzed_files"], 4);
    assert_eq!(v["data"]["summary"]["symbols"], 4);
    let (v, _) = run(dir.path(), "index", None, &["--max-parse-file-size", "13"]);
    assert_eq!(v["data"]["summary"]["analyzed_files"], 3);
    assert_eq!(v["data"]["summary"]["statuses"]["too_large"], 1);
}
