//! Bounded tool action execution engines.

pub(crate) mod cli_manual;
pub(crate) mod file_read;
pub(crate) mod static_cmd;

use std::path::Path;
use crate::models::{ToolAction, ToolDefinition};

/// Dispatches tool execution to the appropriate action handler.
///
/// Returns a tuple of `(output_text, is_error)`.
///
/// # Errors
/// Returns an error message if the tool parameters or execution fails.
pub(crate) fn dispatch_action(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
) -> Result<(String, bool), String> {
    match tool.action {
        ToolAction::FileRead => {
            let content = file_read::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::StaticCommand | ToolAction::CargoJsonDiagnostics => {
            static_cmd::execute(tool, workspace_root)
        }

        ToolAction::CliManual => Ok(cli_manual::execute(tool, arguments)),

        ToolAction::FileWrite
        | ToolAction::MpatchEdit
        | ToolAction::FileSearch
        | ToolAction::ParameterizedCommand
        | ToolAction::Unknown(_) => Err(format!(
            "Tool '{}' action '{:?}' is queued for the next implementation phase.",
            tool.name, tool.action
        )),
    }
}
