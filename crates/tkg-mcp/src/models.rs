//! In-memory models and typed representation of declarative MCP tools.

#[cfg(test)]
pub(crate) use tkg_common::tools::ToolSandboxPolicy;
pub(crate) use tkg_common::{
    AgentGuard,
    tools::{OutputCapture, ToolAction, ToolArg, ToolDefinition},
};
