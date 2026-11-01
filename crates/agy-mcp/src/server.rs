//! The Model Context Protocol (MCP) server daemon handling JSON-RPC 2.0 requests over stdio.

use std::path::PathBuf;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use crate::actions;
use crate::models::ToolDefinition;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::schema::McpTool;

/// State machine managing the MCP server daemon and dispatching JSON-RPC requests.
#[derive(Debug, Clone)]
pub(crate) struct McpServer {
    tools: Vec<ToolDefinition>,
    mcp_tools: Vec<McpTool>,
    workspace_root: PathBuf,
}

impl McpServer {
    /// Constructs a new `McpServer` instance, pre-generating MCP tool schemas for fast responses.
    #[must_use]
    pub(crate) fn new(tools: Vec<ToolDefinition>, workspace_root: PathBuf) -> Self {
        let mcp_tools = tools.iter().map(McpTool::from_definition).collect();

        Self {
            tools,
            mcp_tools,
            workspace_root,
        }
    }

    /// Handles a single incoming JSON-RPC request and produces a response (if not a notification).
    #[must_use]
    pub(crate) fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let Some(id) = req.id else {
            handle_notification(&req);
            return None;
        };

        match req.method.as_str() {
            "initialize" => {
                let result = json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": "agy-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                });
                Some(JsonRpcResponse::success(id, result))
            }

            "tools/list" => {
                let result = json!({
                    "tools": self.mcp_tools
                });
                Some(JsonRpcResponse::success(id, result))
            }

            "ping" => Some(JsonRpcResponse::success(id, json!({}))),

            "tools/call" => {
                let tool_name = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("name"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");

                if tool_name.is_empty() {
                    return Some(JsonRpcResponse::invalid_params(
                        id,
                        "Missing required parameter 'name' in tools/call",
                    ));
                }

                let Some(tool) = self.tools.iter().find(|t| t.name == tool_name) else {
                    return Some(JsonRpcResponse::error(
                        id,
                        -32602,
                        format!("Tool '{tool_name}' is not recognized by agy-mcp"),
                    ));
                };

                let empty_args = json!({});
                let arguments = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("arguments"))
                    .unwrap_or(&empty_args);

                let (content_text, is_error) =
                    match actions::dispatch_action(tool, &self.workspace_root, arguments) {
                        Ok((text, err)) => (text, err),
                        Err(err_msg) => (err_msg, true),
                    };

                let result = json!({
                    "content": [
                        {
                            "type": "text",
                            "text": content_text
                        }
                    ],
                    "isError": is_error
                });

                Some(JsonRpcResponse::success(id, result))
            }

            unknown => Some(JsonRpcResponse::method_not_found(id, unknown)),
        }
    }

    /// Runs the long-running async stdio communication loop.
    ///
    /// # Errors
    /// Returns an I/O error if reading from `stdin` or writing to `stdout` fails.
    pub(crate) async fn run_stdio(&self) -> std::io::Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();

        eprintln!("[agy-mcp] Listening for JSON-RPC 2.0 messages on stdio...");

        while let Some(line) = reader.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parse_result = serde_json::from_str::<JsonRpcRequest>(trimmed);

            match parse_result {
                Ok(request) => {
                    if let Some(response) = self.handle_request(request) {
                        let mut response_bytes = serde_json::to_vec(&response)?;
                        response_bytes.push(b'\n');
                        stdout.write_all(&response_bytes).await?;
                        stdout.flush().await?;
                    }
                }
                Err(err) => {
                    eprintln!("[agy-mcp] JSON-RPC parse error: {err}");
                    let error_resp = JsonRpcResponse::error(
                        serde_json::Value::Null,
                        -32700,
                        format!("Parse error: {err}"),
                    );
                    let mut response_bytes = serde_json::to_vec(&error_resp)?;
                    response_bytes.push(b'\n');
                    stdout.write_all(&response_bytes).await?;
                    stdout.flush().await?;
                }
            }
        }

        eprintln!("[agy-mcp] Stdio stream closed, exiting daemon gracefully.");
        Ok(())
    }
}

fn handle_notification(req: &JsonRpcRequest) {
    if req.method == "notifications/initialized" {
        eprintln!("[agy-mcp] Handshake complete: Antigravity client successfully connected");
    } else if req.method == "notifications/cancelled" {
        eprintln!("[agy-mcp] Received operation cancellation notification from client");
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;
    use crate::models::ToolAction;

    fn sample_server() -> McpServer {
        let tools = vec![
            ToolDefinition {
                name: "read_project_file".to_owned(),
                description: "Read project file".to_owned(),
                action: ToolAction::FileRead,
                ..Default::default()
            },
            ToolDefinition {
                name: "run_check".to_owned(),
                description: "Run cargo check".to_owned(),
                action: ToolAction::StaticCommand,
                command: vec!["cargo".to_owned(), "check".to_owned()],
                ..Default::default()
            },
        ];

        McpServer::new(tools, std::env::temp_dir())
    }

    #[test]
    fn test_initialize_handshake() {
        let server = sample_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(json!(1)),
            method: "initialize".to_owned(),
            params: Some(json!({})),
        };

        let resp = server.handle_request(req).expect("response expected");
        assert_eq!(resp.id, json!(1));
        let res = resp.result.expect("result expected");
        assert_eq!(res["protocolVersion"], "2024-11-05");
        assert_eq!(res["serverInfo"]["name"], "agy-mcp");
    }

    #[test]
    fn test_tools_list() {
        let server = sample_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(json!(2)),
            method: "tools/list".to_owned(),
            params: None,
        };

        let resp = server.handle_request(req).expect("response expected");
        let res = resp.result.expect("result expected");
        let tools = res["tools"].as_array().expect("array expected");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "read_project_file");
        assert_eq!(tools[1]["name"], "run_check");
    }

    #[test]
    fn test_ping() {
        let server = sample_server();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(json!("ping-1")),
            method: "ping".to_owned(),
            params: None,
        };

        let resp = server.handle_request(req).expect("response expected");
        assert_eq!(resp.id, json!("ping-1"));
        assert_eq!(resp.result, Some(json!({})));
    }

    #[test]
    fn test_tools_call_file_read() {
        let temp_dir = std::env::temp_dir().join(format!("agy_test_read_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("create dir");
        let test_file = temp_dir.join("test.txt");
        std::fs::write(&test_file, "Hello from file_read action test!").expect("write test file");

        let tools = vec![ToolDefinition {
            name: "read_project_file".to_owned(),
            action: ToolAction::FileRead,
            ..Default::default()
        }];

        let server = McpServer::new(tools, temp_dir.clone());
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(json!(3)),
            method: "tools/call".to_owned(),
            params: Some(json!({ "name": "read_project_file", "arguments": { "path": "test.txt" } })),
        };

        let resp = server.handle_request(req).expect("response expected");
        let res = resp.result.expect("result expected");
        assert_eq!(res["isError"], false);
        let content = res["content"].as_array().expect("content array");
        assert!(content[0]["text"].as_str().unwrap_or("").contains("Hello from file_read"));

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
