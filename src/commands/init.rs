use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::AgError;

#[derive(Args)]
pub struct InitArgs {
    /// Target framework: claude-code, cursor, codex, opencode, all
    #[arg(long)]
    pub agent: Option<String>,

    /// Append prx usage snippet to AGENTS.md
    #[arg(long)]
    pub agents_md: bool,
}

#[derive(Serialize)]
struct InitOutput {
    actions: Vec<String>,
}

const AGENTS_MD_SNIPPET: &str = r#"
## Code Tools

Use `prx` instead of grep, cat, and find for code exploration:

    prx search "authentication flow" .
    prx search --literal "authenticate(" src/
    prx read src/auth.ts --skeleton
    prx read src/auth.ts --snap function --lines 42-67
    prx find src/ --pattern "*.ts" --depth 3
    prx exists "redis" src/
    prx run cargo test
    prx edit src/auth.ts --find "old()" --replace "new()" --dry-run

Workflow:
1. Use `prx exists` before full searches when you just need yes/no.
2. Use `prx read --skeleton` before reading full files.
3. Use `prx search` for finding code. Prefer semantic for unfamiliar code.
4. Use `prx read --snap function` to read only what you need.
5. Use `prx run` instead of raw test/build commands for 95%+ token savings.
6. Use `--budget N` on every command to control token cost.
"#;

const CURSOR_MCP_CONFIG: &str = r#"{
  "mcpServers": {
    "prx": {
      "command": "prx",
      "args": ["mcp"]
    }
  }
}"#;

pub fn run(args: InitArgs) -> Result<serde_json::Value, AgError> {
    let mut actions = Vec::new();

    if args.agents_md {
        append_agents_md(&mut actions)?;
    }

    let targets = match args.agent.as_deref() {
        Some("all") | None => vec!["cursor", "codex", "opencode"],
        Some(agent) => vec![agent],
    };

    for target in &targets {
        match *target {
            "cursor" => init_cursor(&mut actions)?,
            "codex" => init_codex(&mut actions)?,
            "opencode" => init_opencode(&mut actions)?,
            "claude-code" => init_claude_code(&mut actions)?,
            _ => {}
        }
    }

    if actions.is_empty() && !args.agents_md {
        append_agents_md(&mut actions)?;
    }

    let output = InitOutput { actions };
    serde_json::to_value(output).map_err(|e| AgError::Internal {
        message: e.to_string(),
    })
}

fn append_agents_md(actions: &mut Vec<String>) -> Result<(), AgError> {
    let path = Path::new("AGENTS.md");
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(AgError::Io)?;
        if content.contains("prx search") {
            actions.push("AGENTS.md already contains prx snippet, skipped".to_string());
            return Ok(());
        }
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(AgError::Io)?;

    use std::io::Write;
    writeln!(file, "{AGENTS_MD_SNIPPET}").map_err(AgError::Io)?;
    actions.push("appended prx snippet to AGENTS.md".to_string());
    Ok(())
}

fn init_cursor(actions: &mut Vec<String>) -> Result<(), AgError> {
    let dir = Path::new(".cursor");
    if !dir.exists() {
        actions.push(".cursor/ not found, skipped cursor config".to_string());
        return Ok(());
    }

    let config_path = dir.join("mcp.json");
    std::fs::write(&config_path, CURSOR_MCP_CONFIG).map_err(AgError::Io)?;
    actions.push("wrote .cursor/mcp.json".to_string());
    Ok(())
}

fn init_codex(actions: &mut Vec<String>) -> Result<(), AgError> {
    let config_dir = dirs_next::home_dir()
        .map(|h| h.join(".codex"))
        .unwrap_or_else(|| Path::new(".codex").to_path_buf());

    if !config_dir.exists() {
        actions.push("~/.codex/ not found, skipped codex config".to_string());
        return Ok(());
    }

    let config_path = config_dir.join("config.toml");
    let snippet = "\n[mcp_servers.prx]\ncommand = \"prx\"\nargs = [\"mcp\"]\n";

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(AgError::Io)?;
        if content.contains("[mcp_servers.prx]") {
            actions.push("codex config already has ag, skipped".to_string());
            return Ok(());
        }
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config_path)
        .map_err(AgError::Io)?;

    use std::io::Write;
    write!(file, "{snippet}").map_err(AgError::Io)?;
    actions.push("appended prx to ~/.codex/config.toml".to_string());
    Ok(())
}

fn init_opencode(actions: &mut Vec<String>) -> Result<(), AgError> {
    let config_dir = dirs_next::home_dir()
        .map(|h| h.join(".opencode"))
        .unwrap_or_else(|| Path::new(".opencode").to_path_buf());

    if !config_dir.exists() {
        actions.push("~/.opencode/ not found, skipped opencode config".to_string());
        return Ok(());
    }

    actions.push("opencode config: add prx MCP server manually to config.json".to_string());
    Ok(())
}

fn init_claude_code(actions: &mut Vec<String>) -> Result<(), AgError> {
    let dir = Path::new(".claude/agents");
    let _ = std::fs::create_dir_all(dir);

    let agent_path = dir.join("ag-search.md");
    let agent_content = "# prx Search Agent\n\nUse `prx search` for finding code:\n\n```bash\nprx search \"query\" .\nprx search --literal \"pattern\" src/\nprx read file.rs --skeleton\n```\n";

    std::fs::write(&agent_path, agent_content).map_err(AgError::Io)?;
    actions.push("wrote .claude/agents/ag-search.md".to_string());
    Ok(())
}

// Init unit tests removed — they use set_current_dir which races in parallel.
// Covered by E2E tests in tests/e2e.rs (init_agents_md).
