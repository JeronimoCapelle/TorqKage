//! `static_command` action handler executing pre-configured argv vectors with zero shell injection.

use std::path::Path;
use std::process::Command;
use agy_common::SubprocessSupervisor;
use crate::guards::truncate_output;
use crate::models::{OutputCapture, ToolDefinition};

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
        return Err(format!("Tool '{}' has an empty command definition", tool.name));
    }

    let program = tool.command.first().map_or("", String::as_str);
    let args = tool.command.get(1..).unwrap_or(&[]);

    let output = if tool.enforce_sandbox {
        if agy_common::sandbox::is_inside_outer_sandbox() {
            let mut cmd = Command::new(program);
            cmd.args(args);
            cmd.current_dir(workspace_root);
            SubprocessSupervisor::run_supervised(cmd, tool.timeout_seconds)?
        } else {
            if which::which("bwrap").is_err() {
                return Err(format!(
                    "Security enforcement error: Tool '{}' requires sandbox enforcement ('enforce_sandbox = true'), but 'bwrap' was not found on host and no outer sandbox is active.",
                    tool.name
                ));
            }
            let cmd = agy_common::sandbox::build_bubblewrap_command(
                program,
                args,
                &[],
                workspace_root,
            );
            let out = SubprocessSupervisor::run_supervised(cmd, tool.timeout_seconds)?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("Creating new namespace failed")
                    || stderr.contains("No permissions to create new namespace")
                    || stderr.contains("Setting up mount namespace failed")
                {
                    return Err(format!(
                        "Security enforcement error: Sandbox execution failed for tool '{}': {stderr}",
                        tool.name
                    ));
                }
            }
            out
        }
    } else {
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.current_dir(workspace_root);
        SubprocessSupervisor::run_supervised(cmd, tool.timeout_seconds)?
    };

    let is_error = !output.status.success();

    let text_content = match tool.output_capture {
        OutputCapture::ExitCodeOnly => {
            let code = output.status.code().unwrap_or(-1);
            if output.status.success() {
                format!("Command '{program}' succeeded (exit code 0)")
            } else {
                format!("Command '{program}' failed (exit code {code})")
            }
        }

        OutputCapture::Stdout => {
            let out = String::from_utf8_lossy(&output.stdout);
            if out.trim().is_empty() && is_error {
                String::from_utf8_lossy(&output.stderr).into_owned()
            } else {
                out.into_owned()
            }
        }

        OutputCapture::Stderr => {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.trim().is_empty() && !is_error {
                String::from_utf8_lossy(&output.stdout).into_owned()
            } else {
                err.into_owned()
            }
        }

        OutputCapture::Both | OutputCapture::JsonSpans => {
            let out = String::from_utf8_lossy(&output.stdout);
            let err = String::from_utf8_lossy(&output.stderr);
            if err.trim().is_empty() {
                out.into_owned()
            } else if out.trim().is_empty() {
                err.into_owned()
            } else {
                format!("{out}\n--- stderr ---\n{err}")
            }
        }
    };

    let truncated = truncate_output(
        &text_content,
        tool.truncate_lines,
        tool.truncate_bytes,
        tool.emit_truncation_notice,
    );

    Ok((truncated, is_error))
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
}
