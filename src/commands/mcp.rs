use clap::Args;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::output::AgError;

#[derive(Args)]
pub struct McpArgs {}

pub fn run(_args: McpArgs) -> Result<serde_json::Value, AgError> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| AgError::Internal {
        message: format!("failed to create tokio runtime: {e}"),
    })?;

    rt.block_on(async {
        let server = AgMcpServer::new();
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let service = server
            .serve((stdin, stdout))
            .await
            .map_err(|e| AgError::Internal {
                message: format!("MCP server error: {e}"),
            })?;
        let _ = service.waiting().await;
        Ok(serde_json::json!({"status": "stopped"}))
    })
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Natural language or code query
    pub query: String,
    /// Root path to search
    #[serde(default = "default_path")]
    pub path: String,
    /// Number of results
    #[serde(default = "default_top_k")]
    pub top_k: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ReadParams {
    /// File path to read
    pub file: String,
    /// Return signatures only
    #[serde(default)]
    pub skeleton: bool,
    /// Return symbol table only
    #[serde(default)]
    pub outline: bool,
    /// Return hash only
    #[serde(default)]
    pub hash: bool,
    /// Return cached stub if file hash matches
    #[serde(default)]
    pub if_changed: Option<String>,
    /// Read mode: aggressive (strip comments) or entropy (filter repetitive)
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FindParams {
    /// Root path
    #[serde(default = "default_path")]
    pub path: String,
    /// Glob pattern filter
    pub pattern: Option<String>,
    /// Max directory depth
    pub depth: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ExistsParams {
    /// Pattern to check
    pub pattern: String,
    /// Root path
    #[serde(default = "default_path")]
    pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OutlineParams {
    /// File path
    pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RunParams {
    /// Command and arguments
    pub command: Vec<String>,
}

fn default_path() -> String {
    ".".to_string()
}
fn default_top_k() -> Option<usize> {
    Some(5)
}

#[derive(Debug, Clone)]
pub struct AgMcpServer {
    tool_router: ToolRouter<Self>,
}

impl AgMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for AgMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AgMcpServer {}

#[tool_router(router = tool_router)]
impl AgMcpServer {
    #[tool(
        name = "search",
        description = "Search codebase with natural language or code query"
    )]
    async fn search(&self, params: Parameters<SearchParams>) -> String {
        let p = params.0;
        let args = super::search::SearchArgs {
            query: p.query,
            path: p.path,
            literal: false,
            semantic: false,
            structural: false,
            top_k: p.top_k.unwrap_or(5),
            budget: None,
            context: None,
            exists: false,
            continue_token: None,
            alpha: None,
        };
        match super::search::run(args) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("error: {e}")),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "read",
        description = "Read file with optional skeleton/outline/hash mode"
    )]
    async fn read(&self, params: Parameters<ReadParams>) -> String {
        let p = params.0;
        let args = super::read::ReadArgs {
            file: p.file,
            lines: None,
            snap: None,
            skeleton: p.skeleton,
            outline: p.outline,
            hash: p.hash,
            budget: None,
            meta: false,
            if_changed: p.if_changed,
            mode: p.mode,
        };
        match super::read::run(args) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("error: {e}")),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "find", description = "List and filter files in workspace")]
    async fn find(&self, params: Parameters<FindParams>) -> String {
        let p = params.0;
        let args = super::find::FindArgs {
            path: p.path,
            pattern: p.pattern,
            depth: p.depth,
            related_to: None,
            changed_since: None,
            outline: false,
            tree: false,
            flat: false,
            budget: None,
        };
        match super::find::run(args) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("error: {e}")),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "exists",
        description = "Check if a pattern exists in the codebase (O(1) bloom filter)"
    )]
    async fn exists(&self, params: Parameters<ExistsParams>) -> String {
        let p = params.0;
        let args = super::exists::ExistsArgs {
            pattern: p.pattern,
            path: p.path,
        };
        match super::exists::run(args) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("error: {e}")),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "outline",
        description = "Get symbol table for a file or directory"
    )]
    async fn outline(&self, params: Parameters<OutlineParams>) -> String {
        let p = params.0;
        let args = super::outline::OutlineArgs {
            path: p.path,
            depth: None,
            kind: None,
        };
        match super::outline::run(args) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("error: {e}")),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "run",
        description = "Run a command and return structured output (test/build/lint)"
    )]
    async fn run_cmd(&self, params: Parameters<RunParams>) -> String {
        let p = params.0;
        let args = super::run::RunArgs {
            command: p.command,
            raw: false,
            full: false,
            timeout: 300,
        };
        match super::run::run(args) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("error: {e}")),
            Err(e) => format!("error: {e}"),
        }
    }
}
