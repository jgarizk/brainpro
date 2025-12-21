//! MCP tool dispatch - handles execution of MCP tools.

use crate::mcp::manager::{McpManager, McpToolResult};
use anyhow::Result;
use serde_json::{json, Value};

/// Execute an MCP tool call
pub fn execute(manager: &mut McpManager, full_name: &str, args: Value) -> Result<Value> {
    let result = manager.call_tool(full_name, args)?;
    format_result(&result)
}

/// Format an MCP tool result into the standard tool response format
fn format_result(result: &McpToolResult) -> Result<Value> {
    if result.ok {
        Ok(json!({
            "server": result.server,
            "tool": result.tool,
            "ok": true,
            "data": result.data,
            "duration_ms": result.duration_ms,
            "truncated": result.truncated
        }))
    } else {
        Ok(json!({
            "server": result.server,
            "tool": result.tool,
            "ok": false,
            "error": result.data.get("error").cloned().unwrap_or(json!({
                "code": "mcp_error",
                "message": "Unknown error"
            }))
        }))
    }
}
