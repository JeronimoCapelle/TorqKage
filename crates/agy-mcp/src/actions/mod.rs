//! Bounded tool action execution engines.

pub(crate) mod cargo_json;
pub(crate) mod cli_manual;
pub(crate) mod file_read;
pub(crate) mod file_search;
pub(crate) mod file_write;
pub(crate) mod mpatch_edit;
pub(crate) mod parameterized_cmd;
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

        ToolAction::FileWrite => {
            let content = file_write::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::MpatchEdit => {
            let content = mpatch_edit::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::FileSearch => {
            let content = file_search::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::StaticCommand => static_cmd::execute(tool, workspace_root),

        ToolAction::ParameterizedCommand => {
            parameterized_cmd::execute(tool, workspace_root, arguments)
        }

        ToolAction::CargoJsonDiagnostics => cargo_json::execute(tool, workspace_root),

        ToolAction::CliManual => Ok(cli_manual::execute(tool, arguments)),

        ToolAction::Unknown(_) => Err(format!(
            "Unknown or unsupported tool action for tool '{}'",
            tool.name
        )),
    }
}
