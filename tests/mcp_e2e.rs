use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn send_and_receive(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    request: &serde_json::Value,
) -> serde_json::Value {
    let msg = serde_json::to_string(request).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim())
        .unwrap_or_else(|e| panic!("failed to parse response: {e}\nraw: {line}"))
}

fn initialize_msg() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "prx-test",
                "version": "0.1.0"
            }
        }
    })
}

fn initialized_notification() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    })
}

fn tools_list_msg(id: u64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/list"
    })
}

fn tools_call_msg(id: u64, name: &str, args: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": args
        }
    })
}

fn spawn_mcp() -> std::process::Child {
    let binary = assert_cmd::cargo::cargo_bin("prx");
    Command::new(binary)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("PRX_STATS_FILE", "/dev/null")
        .env("PRX_ERRORS_FILE", "/dev/null")
        .spawn()
        .expect("failed to spawn prx mcp")
}

fn make_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("lib.py"),
        "def greet(name):\n    print(f\"Hello {name}\")\n",
    )
    .unwrap();
    dir
}

#[test]
fn mcp_initialize_returns_server_info() {
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(resp["result"].is_object(), "expected result object: {resp}");
    assert!(
        resp["result"]["serverInfo"].is_object(),
        "expected serverInfo: {resp}"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_tools_list_returns_all_tools() {
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let _ = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());

    let notif = initialized_notification();
    let notif_str = serde_json::to_string(&notif).unwrap();
    writeln!(stdin, "{notif_str}").unwrap();
    stdin.flush().unwrap();

    let resp = send_and_receive(&mut stdin, &mut stdout, &tools_list_msg(2));
    assert_eq!(resp["id"], 2);

    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools should be array");
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(
        tool_names.contains(&"search"),
        "missing search tool: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"read"),
        "missing read tool: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"find"),
        "missing find tool: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"exists"),
        "missing exists tool: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"outline"),
        "missing outline tool: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"run"),
        "missing run tool: {tool_names:?}"
    );
    assert_eq!(
        tool_names.len(),
        6,
        "expected exactly 6 tools: {tool_names:?}"
    );

    for tool in tools {
        assert!(
            tool["inputSchema"].is_object(),
            "tool {} missing inputSchema",
            tool["name"]
        );
    }

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_tools_call_search() {
    let dir = make_test_dir();
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let _ = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());
    let notif = initialized_notification();
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    let resp = send_and_receive(
        &mut stdin,
        &mut stdout,
        &tools_call_msg(
            3,
            "search",
            serde_json::json!({
                "query": "fn main",
                "path": dir.path().to_string_lossy().to_string()
            }),
        ),
    );
    assert_eq!(resp["id"], 3);
    assert!(resp["result"].is_object(), "expected result: {resp}");

    let content = resp["result"]["content"]
        .as_array()
        .expect("expected content array");
    assert!(!content.is_empty(), "expected non-empty content");

    let text = content[0]["text"].as_str().expect("expected text content");
    assert!(
        text.contains("returned") || text.contains("matches"),
        "search result should contain matches info: {text}"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_tools_call_read() {
    let dir = make_test_dir();
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let _ = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());
    let notif = initialized_notification();
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    let file_path = dir.path().join("main.rs").to_string_lossy().to_string();
    let resp = send_and_receive(
        &mut stdin,
        &mut stdout,
        &tools_call_msg(
            3,
            "read",
            serde_json::json!({
                "file": file_path
            }),
        ),
    );
    assert_eq!(resp["id"], 3);
    assert!(resp["result"].is_object(), "expected result: {resp}");

    let content = resp["result"]["content"]
        .as_array()
        .expect("expected content array");
    assert!(!content.is_empty());

    let text = content[0]["text"].as_str().expect("expected text");
    assert!(
        text.contains("fn main") || text.contains("hello"),
        "read should return file content: {text}"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_tools_call_find() {
    let dir = make_test_dir();
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let _ = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());
    let notif = initialized_notification();
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    let resp = send_and_receive(
        &mut stdin,
        &mut stdout,
        &tools_call_msg(
            3,
            "find",
            serde_json::json!({
                "path": dir.path().to_string_lossy().to_string(),
                "pattern": "*.rs"
            }),
        ),
    );
    assert_eq!(resp["id"], 3);

    let content = resp["result"]["content"]
        .as_array()
        .expect("expected content array");
    assert!(!content.is_empty());

    let text = content[0]["text"].as_str().expect("expected text");
    assert!(text.contains("main.rs"), "find should list main.rs: {text}");

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_tools_call_exists() {
    let dir = make_test_dir();
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let _ = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());
    let notif = initialized_notification();
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    let resp = send_and_receive(
        &mut stdin,
        &mut stdout,
        &tools_call_msg(
            3,
            "exists",
            serde_json::json!({
                "pattern": "fn main",
                "path": dir.path().to_string_lossy().to_string()
            }),
        ),
    );
    assert_eq!(resp["id"], 3);

    let content = resp["result"]["content"]
        .as_array()
        .expect("expected content array");
    assert!(!content.is_empty());

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_tools_call_run() {
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let _ = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());
    let notif = initialized_notification();
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    let resp = send_and_receive(
        &mut stdin,
        &mut stdout,
        &tools_call_msg(
            3,
            "run",
            serde_json::json!({
                "command": ["echo", "hello"]
            }),
        ),
    );
    assert_eq!(resp["id"], 3);

    let content = resp["result"]["content"]
        .as_array()
        .expect("expected content array");
    assert!(!content.is_empty());

    let text = content[0]["text"].as_str().expect("expected text");
    assert!(
        text.contains("hello") || text.contains("exited 0"),
        "run should contain output: {text}"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn mcp_invalid_tool_returns_error() {
    let mut child = spawn_mcp();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let _ = send_and_receive(&mut stdin, &mut stdout, &initialize_msg());
    let notif = initialized_notification();
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    let resp = send_and_receive(
        &mut stdin,
        &mut stdout,
        &tools_call_msg(3, "nonexistent_tool", serde_json::json!({})),
    );
    assert_eq!(resp["id"], 3);
    assert!(
        resp["error"].is_object(),
        "expected error for invalid tool: {resp}"
    );

    drop(stdin);
    let _ = child.wait();
}
