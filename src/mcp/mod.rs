//! MCP (Model Context Protocol) client support for connecting to external tool servers.

pub mod client;
pub mod manager;
pub mod transport;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool definition from an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    /// Name of the server this tool belongs to
    pub server: String,
    /// Tool name as provided by the server (e.g., "add")
    pub name: String,
    /// Namespaced tool name (e.g., "mcp.echo.add")
    pub full_name: String,
    /// Tool description
    pub description: String,
    /// JSON Schema for tool input parameters
    pub input_schema: Value,
}

impl McpToolDef {
    /// Create an OpenAI-compatible tool schema for this MCP tool
    pub fn to_openai_schema(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.full_name,
                "description": format!("[MCP:{}] {}", self.server, self.description),
                "parameters": self.input_schema,
            }
        })
    }
}
