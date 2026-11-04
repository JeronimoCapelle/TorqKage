//! In-memory models and typed representation of declarative MCP tools.

use serde::{Deserialize, Serialize};

pub(crate) use agy_common::tools::{OutputCapture, ToolAction, ToolArg, ToolDefinition};

#[cfg(test)]
pub(crate) use agy_common::tools::ToolSandboxPolicy;

/// Declarative path and directory guards scoped to a specific agent role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct AgentGuard {
    #[serde(default)]
    pub(crate) allowed_dirs: Vec<String>,
    #[serde(default)]
    pub(crate) disallowed_dirs: Vec<String>,
    #[serde(default)]
    pub(crate) allowed_files: Vec<String>,
    #[serde(default)]
    pub(crate) disallowed_files: Vec<String>,
}
