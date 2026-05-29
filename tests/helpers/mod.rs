use assert_cmd::Command;
use tempfile::TempDir;

pub fn ag() -> Command {
    let mut cmd = Command::cargo_bin("prx").unwrap();
    cmd.env("PRX_STATS_FILE", "/dev/null");
    cmd.env("PRX_ERRORS_FILE", "/dev/null");
    cmd
}

pub fn test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("main.rs"),
        "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n\nfn helper(n: i32) -> i32 {\n    n + 1\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("lib.py"),
        "def greet(name):\n    print(f\"Hello {name}\")\n\ndef add(a, b):\n    return a + b\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("data.json"), "{\"key\": \"value\"}\n").unwrap();
    dir
}

pub fn parse_json(output: &[u8]) -> serde_json::Value {
    serde_json::from_slice(output).expect("invalid JSON output")
}
