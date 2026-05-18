use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn ag() -> Command {
    Command::cargo_bin("prx").unwrap()
}

fn test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("main.rs"),
        "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n\nfn helper(n: i32) -> i32 {\n    n + 1\n}\n",
    ).unwrap();
    std::fs::write(
        dir.path().join("lib.py"),
        "def greet(name):\n    print(f\"Hello {name}\")\n\ndef add(a, b):\n    return a + b\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("data.json"), "{\"key\": \"value\"}\n").unwrap();
    dir
}

fn parse_json(output: &[u8]) -> serde_json::Value {
    serde_json::from_slice(output).expect("invalid JSON output")
}

// ==================== ag search ====================

#[test]
fn search_literal_finds_match() {
    let dir = test_dir();
    let out = ag()
        .args(["search", "fn main", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["status"], "ok");
    assert!(json["data"]["returned"].as_u64().unwrap() >= 1);
}

#[test]
fn search_no_match_returns_empty() {
    let dir = test_dir();
    let out = ag()
        .args([
            "search",
            "nonexistent_xyz_pattern",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["data"]["returned"], 0);
}

#[test]
fn search_bad_path_returns_error() {
    ag().args(["search", "test", "/nonexistent/path/zzz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("file_not_found"));
}

#[test]
fn search_semantic_returns_results() {
    let dir = test_dir();
    let out = ag()
        .args([
            "search",
            "--semantic",
            "greeting function",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["status"], "ok");
}

#[test]
fn search_with_budget() {
    let dir = test_dir();
    let out = ag()
        .args([
            "search",
            "fn",
            dir.path().to_str().unwrap(),
            "--budget",
            "10",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["budget_used"].as_u64().unwrap() <= 10);
}

// ==================== ag read ====================

#[test]
fn read_full_file() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");
    let out = ag()
        .args(["read", file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["status"], "ok");
    assert!(
        json["data"]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("fn main")
    );
    assert_eq!(json["data"]["meta"]["language"], "rust");
}

#[test]
fn read_skeleton() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");
    let out = ag()
        .args(["read", file.to_str().unwrap(), "--skeleton"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let text = json["data"]["content"]["text"].as_str().unwrap();
    assert!(text.contains("fn main"));
    assert!(!text.contains("println!"));
}

#[test]
fn read_hash_only() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");
    let out = ag()
        .args(["read", file.to_str().unwrap(), "--hash"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["content"].is_null());
    assert_eq!(json["data"]["meta"]["hash"].as_str().unwrap().len(), 32);
}

#[test]
fn read_nonexistent_file() {
    ag().args(["read", "/nonexistent/file.rs"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("file_not_found"));
}

#[test]
fn read_line_range() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");
    let out = ag()
        .args(["read", file.to_str().unwrap(), "--lines", "1-3"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let range = &json["data"]["content"]["range"];
    assert_eq!(range[0], 1);
    assert_eq!(range[1], 3);
}

// ==================== ag find ====================

#[test]
fn find_all_files() {
    let dir = test_dir();
    let out = ag()
        .args(["find", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["stats"]["total_files"].as_u64().unwrap() >= 3);
    assert!(json["data"]["flat"].is_array());
    assert!(json["data"]["tree"].is_object());
}

#[test]
fn find_with_pattern() {
    let dir = test_dir();
    let out = ag()
        .args(["find", dir.path().to_str().unwrap(), "--pattern", "*.rs"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let flat = json["data"]["flat"].as_array().unwrap();
    assert_eq!(flat.len(), 1);
    assert!(flat[0]["path"].as_str().unwrap().ends_with(".rs"));
}

#[test]
fn find_bad_path() {
    ag().args(["find", "/nonexistent/zzz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("file_not_found"));
}

// ==================== ag edit ====================

#[test]
fn edit_dry_run_no_change() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");
    let original = std::fs::read_to_string(&file).unwrap();

    let out = ag()
        .args([
            "edit",
            file.to_str().unwrap(),
            "--find",
            "let x = 1",
            "--replace",
            "let x = 99",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["dry_run"].as_bool().unwrap());
    assert_eq!(json["data"]["total_replacements"], 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn edit_apply_modifies() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");

    let out = ag()
        .args([
            "edit",
            file.to_str().unwrap(),
            "--find",
            "let x = 1",
            "--replace",
            "let x = 99",
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(!json["data"]["dry_run"].as_bool().unwrap());
    let modified = std::fs::read_to_string(&file).unwrap();
    assert!(modified.contains("let x = 99"));
}

#[test]
fn edit_no_match() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");
    let out = ag()
        .args([
            "edit",
            file.to_str().unwrap(),
            "--find",
            "nonexistent_xyz",
            "--replace",
            "x",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["data"]["total_replacements"], 0);
}

#[test]
fn edit_nonexistent_file() {
    ag().args(["edit", "/nonexistent.rs", "--find", "a", "--replace", "b"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("file_not_found"));
}

// ==================== ag outline ====================

#[test]
fn outline_file() {
    let dir = test_dir();
    let file = dir.path().join("main.rs");
    let out = ag()
        .args(["outline", file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let symbols = json["data"]["symbols"].as_array().unwrap();
    assert!(symbols.len() >= 2);
}

#[test]
fn outline_nonexistent() {
    ag().args(["outline", "/nonexistent.rs"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("file_not_found"));
}

// ==================== ag exists ====================

#[test]
fn exists_found() {
    let dir = test_dir();
    let out = ag()
        .args(["exists", "fn main", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["exists"].as_bool().unwrap());
}

#[test]
fn exists_not_found() {
    let dir = test_dir();
    let out = ag()
        .args([
            "exists",
            "nonexistent_xyz_symbol",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(!json["data"]["exists"].as_bool().unwrap());
}

#[test]
fn exists_bad_path() {
    ag().args(["exists", "test", "/nonexistent/zzz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("file_not_found"));
}

// ==================== ag run ====================

#[test]
fn run_echo() {
    let out = ag().args(["run", "echo", "hello"]).output().unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["data"]["exit_code"], 0);
    assert_eq!(json["data"]["tool"], "unknown");
}

#[test]
fn run_failing_command() {
    let out = ag().args(["run", "false"]).output().unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_ne!(json["data"]["exit_code"], 0);
}

// ==================== ag index ====================

#[test]
fn index_build() {
    let dir = test_dir();
    let out = ag()
        .args(["index", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["files_indexed"].as_u64().unwrap() >= 3);
    assert!(json["data"]["valid"].as_bool().unwrap());
}

#[test]
fn index_stats() {
    let dir = test_dir();
    ag().args(["index", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    let out = ag()
        .args(["index", dir.path().to_str().unwrap(), "--stats"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["valid"].as_bool().unwrap());
}

#[test]
fn index_bad_path() {
    ag().args(["index", "/nonexistent/zzz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("file_not_found"));
}

// ==================== ag init ====================

#[test]
fn init_agents_md() {
    let dir = TempDir::new().unwrap();
    let out = ag()
        .args(["init", "--agents-md"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let content = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(content.contains("prx search"));
}

// ==================== envelope format ====================

#[test]
fn envelope_has_version_and_command() {
    let dir = test_dir();
    let out = ag()
        .args(["search", "fn", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    let json = parse_json(&out.stdout);
    assert!(json["version"].as_str().is_some());
    assert_eq!(json["command"], "search");
    assert_eq!(json["status"], "ok");
    assert!(json["tokens"].as_u64().unwrap() > 0);
}

#[test]
fn error_envelope_on_failure() {
    let out = ag()
        .args(["read", "/nonexistent/file.rs"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["status"], "error");
    assert!(json["error"]["code"].as_str().is_some());
    assert!(json["error"]["message"].as_str().is_some());
}

#[test]
fn plain_mode_no_json() {
    let dir = test_dir();
    let out = ag()
        .args(["search", "fn", dir.path().to_str().unwrap(), "--plain"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.starts_with("{\"version\""));
}

// ==================== ag diff (git integration) ====================

fn git_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .unwrap();
    };
    run(&["init", "--quiet"]);
    std::fs::write(
        dir.path().join("main.rs"),
        "fn main() {\n    let x = 1;\n}\n",
    )
    .unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "initial", "--quiet"]);
    std::fs::write(
        dir.path().join("main.rs"),
        "fn main() {\n    let x = 42;\n}\n",
    )
    .unwrap();
    dir
}

#[test]
fn diff_shows_changes() {
    let dir = git_test_dir();
    let out = ag()
        .args(["diff", "--since", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["status"], "ok");
    assert!(json["data"]["stats"]["additions"].as_u64().unwrap() >= 1);
    assert!(json["data"]["stats"]["deletions"].as_u64().unwrap() >= 1);
    assert!(json["data"]["stats"]["files_changed"].as_u64().unwrap() >= 1);
}

#[test]
fn diff_stat_only() {
    let dir = git_test_dir();
    let out = ag()
        .args(["diff", "--since", "HEAD", "--stat-only"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["hunks"].is_null());
    assert!(!json["data"]["summary"].as_str().unwrap().is_empty());
}

#[test]
fn diff_no_changes() {
    let dir = TempDir::new().unwrap();
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init", "--quiet"])
        .current_dir(dir.path())
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();

    let out = ag()
        .args(["diff", "--since", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["data"]["summary"], "no changes");
}

#[test]
fn diff_semantic_notes_detect_new_function() {
    let dir = git_test_dir();
    std::fs::write(
        dir.path().join("main.rs"),
        "fn main() {\n    let x = 42;\n}\n\nfn new_func() {}\n",
    )
    .unwrap();
    let out = ag()
        .args(["diff", "--since", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let notes = json["data"]["semantic_notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n.as_str().unwrap().contains("new_func")),
        "should detect new function: {notes:?}"
    );
}

#[test]
fn find_changed_since() {
    let dir = git_test_dir();
    std::fs::write(dir.path().join("new_file.rs"), "fn new() {}").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "add", "--quiet"])
        .current_dir(dir.path())
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();

    let out = ag()
        .args([
            "find",
            dir.path().to_str().unwrap(),
            "--changed-since",
            "HEAD~1",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let flat = json["data"]["flat"].as_array().unwrap();
    assert!(
        flat.iter()
            .any(|f| f["path"].as_str().unwrap().contains("new_file")),
        "should find newly committed file"
    );
}

// ==================== ag stats ====================

#[test]
fn stats_with_env_override() {
    let dir = TempDir::new().unwrap();
    let stats_file = dir.path().join("test_stats.jsonl");
    let out = ag()
        .args(["stats"])
        .env("AG_STATS_FILE", stats_file.to_str().unwrap())
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert_eq!(json["status"], "ok");
}

// ==================== ag batch ====================

#[test]
fn batch_multiple_commands() {
    let dir = test_dir();
    let path = dir.path().to_str().unwrap().replace('\\', "/");
    let input = format!(
        "{{\"id\":\"1\",\"cmd\":\"find\",\"path\":\"{path}\"}}\n{{\"id\":\"2\",\"cmd\":\"exists\",\"pattern\":\"fn main\",\"path\":\"{path}\"}}\n",
    );
    let out = ag().args(["batch"]).write_stdin(input).output().unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let results = json["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["id"], "1");
    assert_eq!(results[1]["id"], "2");
}

#[test]
fn batch_unknown_command() {
    let out = ag()
        .args(["batch"])
        .write_stdin("{\"cmd\":\"unknown_xyz\"}\n")
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let results = json["data"]["results"].as_array().unwrap();
    assert_eq!(results[0]["status"], "error");
}

#[test]
fn batch_invalid_json() {
    let out = ag()
        .args(["batch"])
        .write_stdin("not json\n")
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    let results = json["data"]["results"].as_array().unwrap();
    assert_eq!(results[0]["status"], "error");
}

// ==================== semantic search E2E ====================

#[test]
fn search_semantic_finds_related_code() {
    let dir = test_dir();
    let out = ag()
        .args([
            "search",
            "--semantic",
            "greeting function print name",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["returned"].as_u64().unwrap() > 0);
}

#[test]
fn search_structural_finds_pattern() {
    let dir = test_dir();
    let out = ag()
        .args([
            "search",
            "--structural",
            "let $X = $Y",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = parse_json(&out.stdout);
    assert!(json["data"]["returned"].as_u64().unwrap() >= 1);
}

// ==================== version and help ====================

#[test]
fn version_flag() {
    ag().args(["--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("prx"));
}

#[test]
fn help_flag() {
    ag().args(["--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("read"))
        .stdout(predicate::str::contains("find"))
        .stdout(predicate::str::contains("edit"))
        .stdout(predicate::str::contains("diff"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("index"))
        .stdout(predicate::str::contains("outline"))
        .stdout(predicate::str::contains("exists"))
        .stdout(predicate::str::contains("batch"))
        .stdout(predicate::str::contains("stats"))
        .stdout(predicate::str::contains("init"));
}
