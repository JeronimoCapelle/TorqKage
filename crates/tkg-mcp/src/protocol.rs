//! JSON-RPC 2.0 protocol models for Model Context Protocol (MCP) communication.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 Request or Notification received from an MCP client over stdio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JsonRpcRequest {
    #[serde(default = "default_jsonrpc_version")]
    pub(crate) jsonrpc: String,
    #[serde(default)]
    pub(crate) id: Option<serde_json::Value>,
    pub(crate) method: String,
    #[serde(default)]
    pub(crate) params: Option<serde_json::Value>,
}

fn default_jsonrpc_version() -> String {
    "2.0".to_owned()
}

/// JSON-RPC 2.0 Error descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 Response emitted to stdout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JsonRpcResponse {
    pub(crate) jsonrpc: String,
    pub(crate) id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Constructs a successful JSON-RPC response with a payload result.
    #[must_use]
    pub(crate) fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Constructs an error JSON-RPC response.
    #[must_use]
    pub(crate) fn error(id: serde_json::Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Standard -32601 Method Not Found error response.
    #[must_use]
    pub(crate) fn method_not_found(id: serde_json::Value, method: &str) -> Self {
        Self::error(id, -32601, format!("Method '{method}' not found"))
    }

    /// Standard -32602 Invalid Params error response.
    #[must_use]
    pub(crate) fn invalid_params(id: serde_json::Value, message: impl Into<String>) -> Self {
        Self::error(id, -32602, message)
    }
}
