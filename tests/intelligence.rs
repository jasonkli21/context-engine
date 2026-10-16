use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::{fs, path::Path};
use tempfile::TempDir;
fn fixture() -> TempDir {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("a.py"), "import missing\ndef helper(): pass\ndef caller():\n    helper()\n    helper()\n    caller()\n    unknown()\ndef another():\n    helper()\nclass C:\n    def helper(self): pass\n").unwrap();
    fs::write(
        d.path().join("test_a.py"),
        "def helper(): pass\ndef test_run(): helper()\n",
    )
    .unwrap();
    fs::write(d.path().join("z.rs"), "fn unused() {}\n").unwrap();
    d
}
fn run(root: &Path, args: &[&str]) -> (Value, Vec<u8>) {
    let bytes = cargo_bin_cmd!("ctx")
        .args(args)
        .arg(root)
        .args(["--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 1);
    (v["data"].clone(), bytes)
}
#[test]
fn search_tiers_filters_bounds_and_determinism() {
    let d = fixture();
    let root = d.path();
    let (v, b) = run(root, &["search", "helper"]);
    assert_eq!(b, run(root, &["search", "helper"]).1);
    let m = v["matches"].as_array().unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(m[0]["match_reason"], "exact_qualified_symbol");
    assert_eq!(m[1]["match_reason"], "exact_qualified_symbol");
    assert_eq!(m[2]["match_reason"], "exact_local_symbol");
    assert!(!v["truncated"].as_bool().unwrap());
    assert_eq!(
        run(root, &["search", "helper", "--limit", "3"]).0["truncated"],
        false
    );
    assert_eq!(
        run(root, &["search", "helper", "--limit", "2"]).0["truncated"],
        true
    );
    assert!(
        run(root, &["search", "helper", "--kind", "file"]).0["matches"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for (q, reason) in [
        ("a.py", "exact_file_path"),
        ("a.", "file_path_prefix"),
        (".py", "file_path_substring"),
        ("C.", "qualified_symbol_prefix"),
        ("hel", "local_symbol_prefix"),
        ("elper", "qualified_symbol_substring"),
    ] {
        let v = run(root, &["search", q]).0;
        assert!(
            v["matches"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["match_reason"] == reason),
            "{q}: {v}"
        );
    }
    assert!(
        run(root, &["search", "absent"]).0["matches"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    fs::write(
        root.join("z.rs"),
        (0..201)
            .map(|i| format!("fn hit_{i}() {{}}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let v = run(
        root,
        &["search", "hit_", "--kind", "symbol", "--limit", "200"],
    )
    .0;
    assert_eq!(v["matches"].as_array().unwrap().len(), 200);
    assert_eq!(v["truncated"], true);
    assert!(
        run(root, &["search", &"é".repeat(2048)]).0["matches"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn neighborhood_locality_distances_cycles_and_test_evidence() {
    let d = fixture();
    let root = d.path();
    let (v, b) = run(
        root,
        &["neighborhood", "--symbol", "helper", "--depth", "8"],
    );
    assert_eq!(
        b,
        run(
            root,
            &["neighborhood", "--symbol", "helper", "--depth", "8"]
        )
        .1
    );
    assert_eq!(v["roots"].as_array().unwrap().len(), 2);
    let groups = v["results"].as_array().unwrap();
    for group in groups {
        let visits = group["entities"].as_array().unwrap();
        let root_visit = visits
            .iter()
            .find(|e| e["entity"]["id"] == group["root_id"])
            .unwrap();
        assert_eq!(root_visit["distance"], 0);
        let path = root_visit["entity"]["path"].as_str().unwrap();
        for e in visits {
            assert_eq!(e["entity"]["path"], path);
            if e["distance"] != 0 {
                assert!(
                    group["relations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|r| r["id"] == e["via_relation_id"])
                );
                let parent = visits
                    .iter()
                    .find(|p| p["entity"]["id"] == e["predecessor_id"])
                    .unwrap();
                assert_eq!(
                    parent["distance"].as_u64().unwrap() + 1,
                    e["distance"].as_u64().unwrap()
                );
            }
        }
        assert_eq!(
            group["candidate_test_associations"]
                .as_array()
                .unwrap()
                .is_empty(),
            path != "test_a.py"
        );
    }
    let v = run(root, &["neighborhood", "--file", "a.py", "--depth", "0"]).0;
    assert_eq!(v["results"][0]["entities"].as_array().unwrap().len(), 1);
    assert!(v["results"][0]["relations"].as_array().unwrap().is_empty());
    let v = run(root, &["neighborhood", "--file", "a.py", "--depth", "1"]).0;
    assert!(
        v["results"][0]["entities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["distance"].as_u64().unwrap() <= 1)
    );
    assert!(
        v["results"][0]["relations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["target"]["state"] == "unresolved")
    );
    assert!(
        run(root, &["neighborhood", "--file", "absent.py"]).0["roots"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn metrics_distinct_counts_unresolved_activity_and_result_limits() {
    let d = fixture();
    let root = d.path();
    let (v, b) = run(root, &["hotspots"]);
    assert_eq!(b, run(root, &["hotspots"]).1);
    assert_eq!(v["basis"], "structural/current_graph");
    let helper = &v["central_symbols"][0];
    assert_eq!(helper["entity"]["symbol"]["qualified_name"], "helper");
    assert_eq!(helper["fan_in"], 2);
    assert_eq!(helper["incoming_resolved_references"], 3);
    let caller = v["central_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["entity"]["symbol"]["name"] == "caller")
        .unwrap();
    assert_eq!(caller["fan_out"], 2);
    assert_eq!(caller["outgoing_resolved_references"], 3);
    assert_eq!(caller["unresolved_references"], 1);
    let file = &v["files"][0];
    assert_eq!(file["entity"]["path"], "a.py");
    assert_eq!(file["structural_activity"], 6);
    assert_eq!(file["incoming_resolved_references"], 4);
    assert_eq!(file["outgoing_resolved_references"], 4);
    assert_eq!(file["unresolved_imports"], 1);
    assert_eq!(file["unresolved_references"], 1);
    assert_eq!(
        run(root, &["hotspots", "--limit", "1"]).0["truncated"],
        true
    );
    let empty = TempDir::new().unwrap();
    fs::write(empty.path().join("a.rs"), "fn unused() {}\n").unwrap();
    let v = run(empty.path(), &["hotspots"]).0;
    assert!(v["central_symbols"].as_array().unwrap().is_empty());
    assert!(v["files"].as_array().unwrap().is_empty());
    fs::write(
        empty.path().join("a.rs"),
        (0..201)
            .map(|i| format!("fn f{i}() {{ f{i}(); }}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let v = run(empty.path(), &["hotspots", "--limit", "200"]).0;
    assert_eq!(v["central_symbols"].as_array().unwrap().len(), 200);
    assert_eq!(v["symbols_truncated"], true);
}
#[test]
fn validation_and_control_escaping() {
    for args in [
        vec!["search", ""],
        vec!["search", "x", "--limit", "0"],
        vec!["hotspots", "--limit", "201"],
        vec!["neighborhood"],
        vec!["neighborhood", "--file", "a", "--symbol", "b"],
        vec!["neighborhood", "--file", "../a"],
        vec!["neighborhood", "--file", "/a"],
        vec!["neighborhood", "--symbol", "x", "--depth", "9"],
    ] {
        cargo_bin_cmd!("ctx")
            .args(args)
            .assert()
            .failure()
            .stdout("");
    }
    cargo_bin_cmd!("ctx")
        .args(["search", &"a".repeat(4097)])
        .assert()
        .failure()
        .stdout("");
    let d = fixture();
    for args in [
        vec!["search", "x\u{1b}"],
        vec!["neighborhood", "--symbol", "x\u{1b}"],
    ] {
        let bytes = cargo_bin_cmd!("ctx")
            .args(args)
            .arg(d.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let s = String::from_utf8(bytes).unwrap();
        assert!(!s.contains('\u{1b}'));
        assert!(s.contains("\\u{1b}"));
    }
}
