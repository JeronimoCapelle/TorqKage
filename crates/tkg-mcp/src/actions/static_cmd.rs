//! `static_command` action handler executing pre-configured argv vectors with zero shell injection.

use std::path::Path;

use crate::{
    actions::{execute_supervised_command, process_command_output},
    models::ToolDefinition,
};

/// Executes a static argv command vector.
///
/// Returns a tuple of `(output_text, is_error)`.
///
/// # Errors
/// Returns an error string if spawning the process fails or the command vector is empty.
pub(crate) fn execute(
    tool: &ToolDefinition,
    workspace_root: &Path,
) -> Result<(String, bool), String> {
    if tool.command.is_empty() {
        return Err(format!(
            "Tool '{}' has an empty command definition",
            tool.name
        ));
    }

    let program = tool.command.first().map_or("", String::as_str);
    let args = tool.command.get(1..).unwrap_or(&[]);

    let output = execute_supervised_command(
        program,
        args,
        &[],
        workspace_root,
        tool.timeout_seconds,
        tool.enforce_sandbox(),
        &tool.name,
    )?;

    Ok(process_command_output(&output, tool, program))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;
    use crate::models::OutputCapture;

    #[test]
    fn test_static_command_echo() {
        let root = std::env::temp_dir();
        let tool = ToolDefinition {
            name: "echo_test".to_owned(),
            command: vec!["echo".to_owned(), "hello_systems".to_owned()],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let (output, is_error) = execute(&tool, &root).expect("command failed");
        assert!(!is_error);
        assert!(output.contains("hello_systems"));
    }

    #[test]
    fn test_static_command_empty_fallback() {
        let root = std::env::temp_dir();
        let tool = ToolDefinition {
            name: "test_conflicts_fallback".to_owned(),
            command: vec!["true".to_owned()],
            output_capture: OutputCapture::Stdout,
            empty_fallback: Some("Clean (0 merge/rebase conflicts detected)".to_owned()),
            ..Default::default()
        };

        let (output, is_error) = execute(&tool, &root).expect("command failed");
        assert!(!is_error);
        assert_eq!(output, "Clean (0 merge/rebase conflicts detected)");
    }

    #[test]
    fn test_static_command_success_exit_codes() {
        let root = std::env::temp_dir();
        // sh -c "exit 101" simulates non-zero compiler check exit
        let tool = ToolDefinition {
            name: "test_exit_code".to_owned(),
            command: vec!["sh".to_owned(), "-c".to_owned(), "exit 101".to_owned()],
            output_capture: OutputCapture::ExitCodeOnly,
            success_exit_codes: vec![0, 101],
            ..Default::default()
        };

        let (output, is_error) = execute(&tool, &root).expect("command execution failed");
        assert!(!is_error);
        assert!(output.contains("exit code 101"));
    }
}
