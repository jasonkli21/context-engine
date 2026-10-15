use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use serde_json::Value;
use std::{fs, path::Path};
use tempfile::TempDir;
fn write(root: &Path, path: &str, text: &str) {
    fs::write(root.join(path), text).unwrap();
}
fn run(root: &Path, command: &str, name: Option<&str>, flags: &[&str]) -> (Value, Vec<u8>) {
    let mut cmd = cargo_bin_cmd!("ctx");
    cmd.arg(command);
    if let Some(name) = name {
        cmd.arg(name);
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
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["command"], command);
    (v, bytes)
}
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "a.py",
        include_str!("graph_fixtures/relations.py"),
    );
    write(
        dir.path(),
        "b.rs",
        include_str!("graph_fixtures/relations.rs"),
    );
    write(dir.path(), "README.md", "fixture docs\n");
    dir
}
#[test]
fn full_graph_contract_evidence_accounting_and_determinism() {
    let dir = fixture();
    let root = dir.path();
    let (v, bytes) = run(root, "graph", None, &[]);
    assert_eq!(bytes, run(root, "graph", None, &[]).1);
    let d = &v["data"];
    assert!(d["id_semantics"].as_str().unwrap().contains("index-local"));
    let entities = d["entities"].as_array().unwrap();
    let relations = d["relations"].as_array().unwrap();
    assert_eq!(entities[0]["kind"], "repository");
    assert_eq!(entities[1]["path"], "README.md");
    assert_eq!(entities[2]["path"], "a.py");
    assert_eq!(entities[3]["path"], "b.rs");
    for (i, e) in entities.iter().enumerate() {
        assert_eq!(e["id"], i);
    }
    for (i, r) in relations.iter().enumerate() {
        assert_eq!(r["id"], i);
        assert!(r["source_id"].as_u64().unwrap() < entities.len() as u64);
        assert!(r["evidence"]["path"].is_string());
        assert!(r["evidence"]["derivation"].is_string());
        assert!(r["resolution"].is_string());
        if r["kind"] == "imports" {
            assert_eq!(r["target"]["state"], "unresolved");
            assert_eq!(r["resolution"], "unresolved");
        }
        if r["kind"] == "references" {
            assert_eq!(r["metadata"]["reference_context"], "call");
            assert!(r["evidence"]["range"]["start"]["byte"].is_number());
        }
    }
    assert_eq!(d["files"].as_array().unwrap().len(), 3);
    let files = d["relation_files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f["status"] == "analyzed"));
    assert!(
        files
            .iter()
            .all(|f| f["unsupported_candidate_count"].as_u64().unwrap() >= 2)
    );
    assert_eq!(
        files
            .iter()
            .map(|f| f["unresolved_count"].as_u64().unwrap())
            .sum::<u64>(),
        d["summary"]["unresolved"]
    );
    assert!(
        relations
            .iter()
            .any(|r| r["metadata"]["binding"] == "Alias")
    );
    assert!(relations.iter().any(|r| r["metadata"]["wildcard"] == true));
    cargo_bin_cmd!("ctx")
        .arg("graph")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Entities:").and(predicate::str::contains("index-local")));
}
#[test]
fn queries_group_roots_preserve_depth_occurrences_and_file_filter() {
    let dir = fixture();
    let root = dir.path();
    for command in ["deps", "callers", "related"] {
        let (v, bytes) = run(root, command, Some("helper"), &[]);
        assert_eq!(bytes, run(root, command, Some("helper"), &[]).1);
        assert_eq!(v["data"]["roots"].as_array().unwrap().len(), 2);
        assert_eq!(v["data"]["results"].as_array().unwrap().len(), 2);
        assert_eq!(v["data"]["depth"], 1);
        assert_eq!(v["data"]["truncated"], false);
        let (filtered, _) = run(
            root,
            command,
            Some("helper"),
            &["--file", "a.py", "--depth", "2"],
        );
        assert_eq!(filtered["data"]["roots"].as_array().unwrap().len(), 1);
        assert_eq!(filtered["data"]["depth"], 2);
        let (zero, _) = run(root, command, Some("missing"), &[]);
        assert!(zero["data"]["roots"].as_array().unwrap().is_empty());
        let (zero, _) = run(root, command, Some("helper"), &["--depth", "0"]);
        assert!(
            zero["data"]["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["relations"].as_array().unwrap().is_empty())
        );
        let (zero, _) = run(root, command, Some("helper"), &["--file", "missing.py"]);
        assert!(zero["data"]["roots"].as_array().unwrap().is_empty());
    }
    let (deps, _) = run(root, "deps", Some("caller"), &["--file", "a.py"]);
    let refs = deps["data"]["results"][0]["relations"].as_array().unwrap();
    assert_eq!(refs.len(), 3);
    assert_ne!(refs[0]["evidence"]["range"], refs[1]["evidence"]["range"]);
    assert!(refs.iter().all(|r| r["kind"] == "references"));
    let (deps, _) = run(root, "deps", Some("Outer.method"), &[]);
    assert!(
        deps["data"]["results"][0]["relations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "imports" && r["target"]["name"] == "local")
    );
    cargo_bin_cmd!("ctx")
        .args(["callers", "helper"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Syntactic callers"));
}
#[test]
fn limits_fail_closed_preserve_symbols_and_account_for_unavailable_files() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "dense.rs",
        &format!("fn survivor() {{ {} }}", "x();".repeat(16_385)),
    );
    write(root, "ok.py", "def ok(): pass\n");
    write(root, "flat.rs", &"fn f() {}\n".repeat(4097));
    fs::write(root.join("invalid.py"), [255]).unwrap();
    let (v, bytes) = run(root, "graph", None, &[]);
    assert_eq!(bytes, run(root, "graph", None, &[]).1);
    let d = &v["data"];
    let files = d["relation_files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["status"], "analysis_limit_exceeded");
    assert_eq!(files[0]["reference_count"], 0);
    assert!(
        d["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["symbol"]["name"] == "survivor")
    );
    assert!(
        d["relations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["kind"] == "contains")
    );
    assert_eq!(d["summary"]["symbol_analysis_unavailable"], 2);
    let (v, _) = run(root, "graph", None, &["--max-parse-file-size", "0"]);
    assert!(v["data"]["relation_files"].as_array().unwrap().is_empty());
}
#[test]
fn production_normalization_limit_preserves_symbols_without_partial_facts() {
    let dir = TempDir::new().unwrap();
    // 12,501 four-segment targets exceed 50,000 normalization visits,
    // while staying below the candidate and target-byte ceilings.
    write(
        dir.path(),
        "visits.rs",
        &format!("fn survivor() {{ {} }}", "a::b::c::d();".repeat(12_501)),
    );
    let (v, _) = run(dir.path(), "graph", None, &[]);
    assert_eq!(
        v["data"]["relation_files"][0]["status"],
        "analysis_limit_exceeded"
    );
    assert_eq!(v["data"]["relation_files"][0]["reference_count"], 0);
    assert_eq!(v["data"]["files"][0]["symbol_count"], 1);
    assert!(
        v["data"]["relations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["kind"] == "contains")
    );
}
#[test]
fn exact_candidate_boundary_is_supported() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "exact.rs",
        &format!("fn f() {{ {} }}", "x();".repeat(16_384)),
    );
    let (v, _) = run(dir.path(), "graph", None, &[]);
    assert_eq!(v["data"]["relation_files"][0]["status"], "analyzed");
    assert_eq!(v["data"]["relation_files"][0]["reference_count"], 16_384);
}
#[test]
fn help_defaults_and_depth_validation() {
    let dir = TempDir::new().unwrap();
    for command in ["graph", "deps", "callers", "related"] {
        let help = cargo_bin_cmd!("ctx")
            .args([command, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let help = String::from_utf8(help).unwrap();
        assert!(help.contains("--format"));
        assert!(help.contains("--max-parse-file-size"));
        let mut cmd = cargo_bin_cmd!("ctx");
        cmd.current_dir(dir.path()).arg(command);
        if command != "graph" {
            cmd.arg("missing");
            assert!(help.contains("--depth"));
            assert!(help.contains("--file"));
        }
        cmd.assert().success();
        if command != "graph" {
            for depth in ["9", "-1", "256"] {
                cargo_bin_cmd!("ctx")
                    .args([command, "missing", "--depth", depth])
                    .assert()
                    .failure();
            }
        }
    }
}
#[test]
fn no_execution_body_leakage_or_alias_resolution() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let marker = root.join("EXECUTED");
    write(
        root,
        "danger.py",
        &format!(
            "from pathlib import Path as P\nP({marker:?}).write_text('executed')\ndef safe():\n    P('BODY_SECRET_123')\n    return 'BODY_SECRET_123'\n"
        ),
    );
    write(
        root,
        "build.rs",
        "fn main() { panic!(\"BODY_SECRET_123\"); }",
    );
    for command in ["graph", "deps", "callers", "related"] {
        let (v, bytes) = run(root, command, (command != "graph").then_some("safe"), &[]);
        assert!(
            !String::from_utf8(bytes)
                .unwrap()
                .contains("BODY_SECRET_123")
        );
        if command == "deps" {
            assert_eq!(
                v["data"]["results"][0]["relations"][0]["target"]["name"],
                "P"
            );
            assert_eq!(
                v["data"]["results"][0]["relations"][0]["resolution"],
                "unresolved"
            );
        }
        let mut cmd = cargo_bin_cmd!("ctx");
        cmd.arg(command);
        if command != "graph" {
            cmd.arg("safe");
        }
        cmd.arg(root)
            .assert()
            .success()
            .stdout(predicate::str::contains("BODY_SECRET_123").not());
    }
    assert!(!marker.exists());
}
#[cfg(unix)]
#[test]
fn graph_symlinks_and_human_control_escaping() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(outside.path(), "other.py", "def other(): pass");
    symlink(outside.path().join("other.py"), dir.path().join("link.py")).unwrap();
    write(dir.path(), "bad\n\u{1b}.py", "def safe():\n    unknown()\n");
    let (v, _) = run(dir.path(), "graph", None, &[]);
    assert_eq!(v["data"]["files"].as_array().unwrap().len(), 1);
    for command in ["deps", "callers", "related"] {
        let out = cargo_bin_cmd!("ctx")
            .args([command, "safe"])
            .arg(dir.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let s = String::from_utf8(out).unwrap();
        assert!(!s.contains('\u{1b}'));
        assert!(s.contains("\\n\\u{1b}"));
        let out = cargo_bin_cmd!("ctx")
            .args([command, "safe\n\u{1b}"])
            .arg(dir.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(!String::from_utf8(out).unwrap().contains('\u{1b}'));
    }
}
