//! Real sockets, real subprocesses, and the pinned official MCP client.
use rmcp::{
    ClientLifecycleMode, ClientServiceExt,
    model::{CallToolRequestParams, ProtocolVersion},
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ctx")
}
struct Repo {
    _temp: tempfile::TempDir,
    root: std::path::PathBuf,
    cache: std::path::PathBuf,
}
impl Repo {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("a.py"),
            "def target():\n    return 1\n\ndef caller():\n    return target()\n",
        )
        .unwrap();
        std::fs::write(root.join("b.py"), "def target():\n    return 2\n").unwrap();
        let git = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&root)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        };
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "initial",
        ]);
        let cache = temp.path().join("cache");
        Self {
            _temp: temp,
            root,
            cache,
        }
    }
    fn cli(&self, args: &[&str]) -> Value {
        let output = Command::new(bin())
            .args(args)
            .arg(&self.root)
            .args(["--format", "json"])
            .env("CONTEXT_ENGINE_CACHE_DIR", &self.cache)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}
struct Http {
    child: Child,
    address: String,
}
impl Http {
    fn start(root: &Path, cache: &Path) -> Self {
        let mut child = Command::new(bin())
            .arg("serve")
            .arg(root)
            .env("CONTEXT_ENGINE_CACHE_DIR", cache)
            .env("CONTEXT_ENGINE_SERVE_TOKEN", TOKEN)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let err = child.stderr.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(err).read_line(&mut line).unwrap();
            tx.send(line).unwrap();
        });
        let line = rx.recv_timeout(Duration::from_secs(15)).unwrap();
        let address = line
            .trim()
            .strip_prefix("ctx: listening http://")
            .unwrap()
            .to_owned();
        Self { child, address }
    }
    fn request(
        &self,
        method: &str,
        route: &str,
        headers: &str,
        body: &[u8],
    ) -> (u16, String, Vec<u8>) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(40)))
            .unwrap();
        write!(stream,"{method} {route} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n",self.address,body.len()).unwrap();
        stream.write_all(body).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8(bytes[..split].to_vec()).unwrap();
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, head, bytes[split + 4..].to_vec())
    }
    fn call(&self, route: &str, args: Value) -> Value {
        let (status, head, bytes) = self.request(
            "POST",
            route,
            &format!("Authorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\n"),
            &serde_json::to_vec(&args).unwrap(),
        );
        assert_eq!(status, 200, "{head}: {}", String::from_utf8_lossy(&bytes));
        assert!(
            head.to_ascii_lowercase()
                .contains("cache-control: no-store")
        );
        serde_json::from_slice(&bytes).unwrap()
    }
}
impl Drop for Http {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_http_and_real_mcp_client_parity_and_freshness() {
    let repo = Repo::new();
    let http = Http::start(&repo.root, &repo.cache);
    let mut cmd = tokio::process::Command::new(bin());
    cmd.arg("mcp")
        .arg(&repo.root)
        .env("CONTEXT_ENGINE_CACHE_DIR", &repo.cache);
    let transport = TokioChildProcess::new(cmd).unwrap();
    let client = ()
        .serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .unwrap();
    let list = client.list_tools(None).await.unwrap();
    assert_eq!(list.tools.len(), 5);
    for t in &list.tools {
        assert_eq!(t.input_schema["additionalProperties"], false);
    }
    let cases = [
        (
            "/v1/search",
            "search_codebase",
            json!({"query":"target","limit":1}),
            vec!["search", "target", "--limit", "1"],
        ),
        (
            "/v1/symbol",
            "find_symbol",
            json!({"name":"target"}),
            vec!["symbol", "target"],
        ),
        (
            "/v1/neighborhood",
            "get_dependency_neighborhood",
            json!({"symbol":"target","depth":2}),
            vec!["neighborhood", "--symbol", "target", "--depth", "2"],
        ),
        (
            "/v1/impact",
            "get_change_impact",
            json!({"range":"HEAD..HEAD","depth":0}),
            vec!["impact", "HEAD..HEAD", "--depth", "0"],
        ),
        (
            "/v1/context",
            "build_task_context",
            json!({"task":"target","budget":4000}),
            vec!["context", "--task", "target", "--budget", "4000"],
        ),
    ];
    for (route, name, args, cli) in cases {
        let expected = repo.cli(&cli);
        let h = http.call(route, args.clone());
        let m = client
            .call_tool(
                CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        assert_eq!(m.is_error, Some(false));
        let m = m.structured_content.unwrap();
        assert_eq!(h["result"], expected, "HTTP {name}");
        assert_eq!(m["result"], expected, "MCP {name}");
        if name == "find_symbol" {
            assert_eq!(expected["data"]["matches"].as_array().unwrap().len(), 2);
        }
        if name == "build_task_context" {
            let data = &expected["data"];
            let used: usize = data["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["content"].as_str().unwrap().len())
                .sum();
            assert_eq!(data["budget"]["used"], used);
        }
    }
    // A long-lived process must observe new source and dirty content, while
    // committed impact continues to ignore working-tree bytes.
    std::fs::write(
        repo.root.join("a.py"),
        "def changed():\n    return 'dirty'\n",
    )
    .unwrap();
    let h = http.call("/v1/search", json!({"query":"changed"}));
    assert!(
        !h["result"]["data"]["matches"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let m = client
        .call_tool(
            CallToolRequestParams::new("search_codebase")
                .with_arguments(json!({"query":"changed"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(m.structured_content.unwrap()["result"], h["result"]);
    assert_eq!(
        http.call("/v1/impact", json!({"range":"HEAD..HEAD"}))["result"],
        repo.cli(&["impact", "HEAD..HEAD"])
    );
    assert!(
        client
            .call_tool(CallToolRequestParams::new("unknown"))
            .await
            .is_err()
    );
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("find_symbol")
                    .with_arguments(json!({"name":"x","root":"/"}).as_object().unwrap().clone())
            )
            .await
            .is_err()
    );
    let fail = client
        .call_tool(
            CallToolRequestParams::new("get_change_impact").with_arguments(
                json!({"range":"missing..HEAD"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(fail.is_error, Some(true));
    client.cancel().await.unwrap();
}
#[test]
fn http_socket_security_validation_and_non_git_errors() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let h = Http::start(&root, &temp.path().join("cache"));
    let auth = format!("Authorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\n");
    let req = |m, r, headers: &str, b: &[u8]| h.request(m, r, headers, b).0;
    assert_eq!(req("GET", "/health", "", b""), 200);
    assert_eq!(req("POST", "/v1/search", "", b"{}"), 401);
    assert_eq!(
        req(
            "POST",
            "/v1/search",
            "Authorization: Bearer wrong\r\n",
            b"{}"
        ),
        401
    );
    assert_eq!(
        req(
            "POST",
            "/v1/search",
            &(auth.clone() + "Host: evil.invalid\r\n"),
            b"{}"
        ),
        403
    );
    assert_eq!(
        req(
            "POST",
            "/v1/search",
            &(auth.clone() + "Origin: http://evil.invalid\r\n"),
            b"{}"
        ),
        403
    );
    assert_eq!(req("GET", "/v1/search", &auth, b"{}"), 405);
    assert_eq!(
        req(
            "POST",
            "/v1/search",
            &format!("Authorization: Bearer {TOKEN}\r\n"),
            b"{}"
        ),
        415
    );
    for b in [
        b"{".as_slice(),
        br#"{"query":"x","root":"/"}"#,
        br#"{"query":3}"#,
        br#"{"query":"x","query":"y"}"#,
        br#"{"query":"x","limit":201}"#,
    ] {
        assert_eq!(req("POST", "/v1/search", &auth, b), 400);
    }
    assert_eq!(req("POST", "/v1/search", &auth, &vec![b' '; 16385]), 413);
    for file in ["../secret.py", "/etc/passwd", "a/../b", "."] {
        assert_eq!(
            req(
                "POST",
                "/v1/neighborhood",
                &auth,
                &serde_json::to_vec(&json!({"file":file})).unwrap()
            ),
            400
        );
    }
    assert_eq!(
        req("POST", "/v1/impact", &auth, br#"{"range":"HEAD..HEAD"}"#),
        422
    );
    let (_, _, body) = h.request("POST", "/v1/impact", &auth, br#"{"range":"HEAD..HEAD"}"#);
    assert!(!String::from_utf8_lossy(&body).contains(&root.to_string_lossy().to_string()));
    assert_eq!(
        req(
            "POST",
            "/v1/context",
            &auth,
            br#"{"task":"secret","budget":1048577}"#
        ),
        400
    );
}
#[test]
fn startup_rejects_public_bind_and_missing_or_weak_token() {
    let t = tempfile::tempdir().unwrap();
    for (listen, token) in [
        ("0.0.0.0:0", Some(TOKEN)),
        ("127.0.0.1:0", Some("weak")),
        ("127.0.0.1:0", None),
    ] {
        let mut c = Command::new(bin());
        c.arg("serve")
            .arg(t.path())
            .args(["--listen", listen])
            .env_remove("CONTEXT_ENGINE_SERVE_TOKEN");
        if let Some(t) = token {
            c.env("CONTEXT_ENGINE_SERVE_TOKEN", t);
        }
        let out = c.output().unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&out.stderr).contains(TOKEN));
    }
}

#[test]
fn mcp_wire_metadata_errors_notifications_and_eof() {
    let repo = Repo::new();
    let mut child = Command::new(bin())
        .arg("mcp")
        .arg(&repo.root)
        .env("CONTEXT_ENGINE_CACHE_DIR", &repo.cache)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if tx.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let receive = || -> Value {
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(10)).unwrap()).unwrap()
    };
    let meta = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{},"io.modelcontextprotocol/clientInfo":{"name":"wire-test","version":"1"}});
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":"discover","method":"server/discover","params":{"_meta":meta}})
    )
    .unwrap();
    let discovery = receive();
    assert_eq!(discovery["id"], "discover");
    assert!(
        discovery["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!("2026-07-28"))
    );
    writeln!(input, "{{").unwrap();
    // The SDK drops malformed JSON syntax and continues with the next frame.
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
    )
    .unwrap();
    assert_eq!(receive()["error"]["code"], -32602);
    let mut old = meta.clone();
    old["io.modelcontextprotocol/protocolVersion"] = json!("2025-11-25");
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{"_meta":old}})
    )
    .unwrap();
    assert_eq!(receive()["error"]["code"], -32022);
    writeln!(input, "{}", " ".repeat(16385)).unwrap();
    // Oversized frames are dropped; notifications must not generate responses; the next line must belong to 4.
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"absent"}})
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":4,"method":"tools/list","params":{"_meta":meta}})
    )
    .unwrap();
    assert_eq!(receive()["id"], 4);
    drop(input);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            panic!("MCP failed to terminate on EOF");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    assert!(err.is_empty(), "{err}");
}

#[cfg(unix)]
#[test]
fn source_boundaries_warnings_and_controlled_shutdown() {
    use std::os::unix::fs::symlink;
    let repo = Repo::new();
    let secret = repo._temp.path().join("outside.py");
    std::fs::write(
        &secret,
        "def confidential():\n    return 'OUTSIDE_SECRET'\n",
    )
    .unwrap();
    symlink(&secret, repo.root.join("link.py")).unwrap();
    std::fs::create_dir(repo.root.join("vendor")).unwrap();
    std::fs::write(
        repo.root.join("vendor/secret.py"),
        "def vendor_secret():\n    return 'VENDOR_SECRET'\n",
    )
    .unwrap();
    std::fs::write(repo.root.join(".env"), "PRIVATE_SECRET").unwrap();
    // Invalid automatic cache configuration yields structured warnings, not stdout noise.
    let h = Http::start(&repo.root, &repo.root.join("cache"));
    let reply = h.call(
        "/v1/context",
        json!({"task":"secret confidential target","budget":4000}),
    );
    let rendered = reply.to_string();
    for secret in ["OUTSIDE_SECRET", "VENDOR_SECRET", "PRIVATE_SECRET"] {
        assert!(!rendered.contains(secret));
    }
    assert!(!reply["warnings"].as_array().unwrap().is_empty());
    let _ = Command::new("kill")
        .args(["-TERM", &h.child.id().to_string()])
        .status()
        .unwrap();
    let mut h = h;
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(status) = h.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut output = Vec::new();
    h.child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    assert!(output.is_empty());
}

#[test]
fn http_response_bound_on_real_large_graph() {
    let repo = Repo::new();
    let mut path = repo.root.clone();
    for i in 0..4 {
        path.push(format!("{i}{}", "x".repeat(180)));
    }
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("large.py"),
        format!("def many():\n{}", "    external()\n".repeat(11000)),
    )
    .unwrap();
    let h = Http::start(&repo.root, &repo.cache);
    let auth = format!("Authorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\n");
    let (status, _, bytes) = h.request(
        "POST",
        "/v1/neighborhood",
        &auth,
        br#"{"symbol":"many","depth":1}"#,
    );
    assert_eq!(
        status,
        413,
        "{}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(500)])
    );
    assert!(bytes.len() < 1000);
    assert!(String::from_utf8_lossy(&bytes).contains("response_limit"));
}

#[test]
fn http_overload_survives_disconnected_cache_writer() {
    let repo = Repo::new();
    let h = Http::start(&repo.root, &repo.cache);
    h.call("/v1/search", json!({"query":"target"}));
    let lock = std::fs::read_dir(repo.cache.join("indexes"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().ends_with(".writer-lock"))
        .unwrap();
    let conn = rusqlite::Connection::open(lock).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE;").unwrap();
    std::fs::write(repo.root.join("a.py"), "def updated():\n    pass\n").unwrap();
    let auth = format!("Authorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\n");
    let body = br#"{"query":"updated"}"#;
    let mut pending = TcpStream::connect(&h.address).unwrap();
    write!(
        pending,
        "POST /v1/search HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\n{auth}\r\n",
        h.address,
        body.len()
    )
    .unwrap();
    pending.write_all(body).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    // A pipelined competing query must fail fast while the first worker waits
    // for the real SQLite writer reservation. It cannot create a second writer.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let (status, _, _) = h.request("POST", "/v1/search", &auth, body);
        if status == 503 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "no overload observed");
    }
    drop(pending);
    assert_eq!(h.request("POST", "/v1/search", &auth, body).0, 503);
    conn.execute_batch("ROLLBACK;").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let (status, _, bytes) = h.request("POST", "/v1/search", &auth, body);
        if status == 200 {
            let r: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(
                !r["result"]["data"]["matches"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            break;
        }
        assert_eq!(status, 503);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
