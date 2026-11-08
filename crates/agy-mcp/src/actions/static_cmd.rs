//! `static_command` action handler executing pre-configured argv vectors with zero shell injection.

use std::path::Path;

use crate::{
    actions::execute_supervised_command,
    guards::truncate_output,
    models::{OutputCapture, ToolDefinition},
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

    let is_error = if let Some(code) = output.status.code() {
        if tool.success_exit_codes.contains(&code) {
            false
        } else {
            !output.status.success()
        }
    } else {
        !output.status.success()
    };

    let text_content = match tool.output_capture {
        OutputCapture::ExitCodeOnly => {
            let code = output.status.code().unwrap_or(-1);
            if is_error {
                format!("Command '{program}' failed (exit code {code})")
            } else {
                format!("Command '{program}' completed successfully (exit code {code})")
            }
        },

        OutputCapture::Stdout => {
            let out = String::from_utf8_lossy(&output.stdout);
            if out.trim().is_empty() && is_error {
                String::from_utf8_lossy(&output.stderr).into_owned()
            } else {
                out.into_owned()
            }
        },

        OutputCapture::Stderr => {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.trim().is_empty() && !is_error {
                String::from_utf8_lossy(&output.stdout).into_owned()
            } else {
                err.into_owned()
            }
        },

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
        },
    };

    let effective_content = if text_content.trim().is_empty()
        && let Some(fallback) = &tool.empty_fallback
    {
        fallback.clone()
    } else {
        text_content
    };

    let clean_content = crate::guards::strip_ansi(&effective_content);

    let truncated = truncate_output(
        &clean_content,
        tool.truncate_lines,
        tool.truncate_bytes,
        tool.emit_truncation_notice(),
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
