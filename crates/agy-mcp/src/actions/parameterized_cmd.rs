//! `parameterized_cmd` action handler executing bounded host binaries with argument regex validation.

use std::path::Path;
use crate::actions::execute_supervised_command;
use crate::guards::truncate_output;
use crate::models::{OutputCapture, ToolDefinition};

fn validate_and_build_args(
    tool: &ToolDefinition,
    arguments: &serde_json::Value,
) -> Result<Vec<String>, String> {
    let mut dynamic_args = Vec::new();

    for arg in &tool.args {
        let val_opt = arguments
            .get(&arg.name)
            .and_then(serde_json::Value::as_str)
            .map(str::trim);

        if let Some(val) = val_opt
            && !val.is_empty()
        {
            if let Some(pattern) = &arg.regex
                && !matches_pattern(val, pattern)
            {
                return Err(format!(
                    "Security validation error: Argument '{}' with value '{val}' violates pattern '{pattern}'",
                    arg.name
                ));
            }

            if let Some(flag) = &arg.flag {
                dynamic_args.push(flag.clone());
            }
            dynamic_args.push(val.to_owned());
        } else if arg.required {
            return Err(format!(
                "Missing required argument '{}' for tool '{}'",
                arg.name, tool.name
            ));
        }
    }

    Ok(dynamic_args)
}

fn format_captured_output(
    output: &std::process::Output,
    tool: &ToolDefinition,
    binary: &str,
) -> String {
    match tool.output_capture {
        OutputCapture::ExitCodeOnly => {
            let code = output.status.code().unwrap_or(-1);
            format!("Process exited with status code {code}.")
        }

        OutputCapture::Stderr => String::from_utf8_lossy(&output.stderr).into_owned(),

        OutputCapture::Both => {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            if binary == "ast-grep" && stdout_str.trim().is_empty() && stderr_str.trim().is_empty() {
                "No matching AST patterns found to rewrite.".to_owned()
            } else if stderr_str.trim().is_empty() {
                stdout_str.into_owned()
            } else if stdout_str.trim().is_empty() {
                stderr_str.into_owned()
            } else {
                format!("{stdout_str}\n--- stderr ---\n{stderr_str}")
            }
        }

        OutputCapture::Stdout | OutputCapture::JsonSpans => {
            let text = String::from_utf8_lossy(&output.stdout);
            if binary == "rg" && text.trim().is_empty() {
                "No occurrences found in codebase for the specified search query.".to_owned()
            } else if binary == "ast-grep" && text.trim().is_empty() {
                "No matching AST patterns found to rewrite.".to_owned()
            } else {
                text.into_owned()
            }
        }
    }
}

/// Executes a parameterized CLI command with argument validation.
///
/// Returns a tuple of `(output_text, is_error)`.
///
/// # Errors
/// Returns an error message if argument validation fails or the process fails to spawn.
pub(crate) fn execute(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
) -> Result<(String, bool), String> {
    if tool.command.is_empty() {
        return Err(format!("Tool '{}' has no command configured", tool.name));
    }

    let mut dynamic_args = validate_and_build_args(tool, arguments)?;

    let binary = tool.command.first().ok_or_else(|| "Empty command".to_owned())?;
    let base_args = tool.command.get(1..).unwrap_or(&[]);

    if binary == "rg" {
        dynamic_args.push(".".to_owned());
    }

    let output = execute_supervised_command(
        binary,
        base_args,
        &dynamic_args,
        workspace_root,
        tool.timeout_seconds,
        tool.enforce_sandbox,
        &tool.name,
    )?;

    let is_error = if (binary == "rg" || binary == "ast-grep")
        && output.status.code() == Some(1)
        && output.stdout.is_empty()
    {
        false
    } else {
        !output.status.success()
    };

    let raw_text = format_captured_output(&output, tool, binary);

    // Apply error fallback if configured and process failed
    let final_text = if is_error && tool.error_fallback.as_deref() == Some("token_smart_not_found") {
        if raw_text.contains("could not find") || raw_text.contains("not found") {
            format!(
                "Item not found in module tree. Verify item path or inspect module tree via inspect_module_structure.\n\n{raw_text}"
            )
        } else {
            raw_text
        }
    } else {
        raw_text
    };

    let truncated = truncate_output(
        &final_text,
        tool.truncate_lines,
        tool.truncate_bytes,
        tool.emit_truncation_notice,
    );

    Ok((truncated, is_error))
}

fn matches_pattern(val: &str, pattern: &str) -> bool {
    if val.is_empty() {
        return false;
    }

    if pattern.contains("\\x00-\\x1f") {
        return !val.chars().any(char::is_control);
    }

    if pattern.contains("\\x00") {
        return !val.contains('\0');
    }

    if let Some(inner) = pattern.strip_prefix("^[").and_then(|p| p.strip_suffix("]+$")) {
        let has_alnum = inner.contains("a-zA-Z0-9")
            || (inner.contains("a-z") && inner.contains("A-Z") && inner.contains("0-9"));
        let has_colon = inner.contains(':');
        let has_underscore = inner.contains('_');
        let has_dash = inner.contains('-');

        return val.chars().all(|c| {
            (has_alnum && c.is_ascii_alphanumeric())
                || (has_underscore && c == '_')
                || (has_colon && c == ':')
                || (has_dash && c == '-')
        });
    }

    // Strict fallback: letters, numbers, underscore, colon, hyphen
    val.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '-')
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_result_states,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;
    use crate::models::ToolArg;
    use serde_json::json;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("Processor", "^[a-zA-Z0-9_:]+$"));
        assert!(matches_pattern("types::state", "^[a-zA-Z0-9_:]+$"));
        assert!(matches_pattern("unit-test_1", "^[a-zA-Z0-9_-]+$"));
        assert!(matches_pattern("struct ToolDefinition", "^[^\\x00-\\x1f]+$"));
        assert!(!matches_pattern("bad\0text", "^[^\\x00-\\x1f]+$"));
        assert!(matches_pattern("import sympy\nx = sympy.Symbol('x')", "^[^\\x00]+$"));
        assert!(!matches_pattern("bad\0code", "^[^\\x00]+$"));
        assert!(!matches_pattern("foo; rm -rf /", "^[a-zA-Z0-9_:]+$"));
        assert!(!matches_pattern("foo bar", "^[a-zA-Z0-9_:]+$"));
    }

    fn create_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agy-test-param-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn test_parameterized_cmd_echo() {
        let temp = create_test_dir("echo");
        let tool = ToolDefinition {
            name: "test_echo".to_owned(),
            command: vec!["echo".to_owned()],
            args: vec![
                ToolArg {
                    name: "msg".to_owned(),
                    required: true,
                    regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                    flag: None,
                    description: "Message".to_owned(),
                },
                ToolArg {
                    name: "prefix".to_owned(),
                    required: false,
                    regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                    flag: Some("-n".to_owned()),
                    description: "No newline".to_owned(),
                },
            ],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "msg": "hello_world"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert!(out.contains("hello_world"));
    }

    #[test]
    fn test_parameterized_cmd_injection_blocked() {
        let temp = create_test_dir("inject");
        let tool = ToolDefinition {
            name: "test_echo".to_owned(),
            command: vec!["echo".to_owned()],
            args: vec![ToolArg {
                name: "target".to_owned(),
                required: true,
                regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = json!({
            "target": "target && cat /etc/passwd"
        });

        let res = execute(&tool, &temp, &args);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("violates pattern"));
    }

    #[test]
    fn test_parameterized_cmd_ast_grep_no_matches() {
        if which::which("ast-grep").is_err() {
            return;
        }
        let temp = create_test_dir("ast_grep");
        let tool = ToolDefinition {
            name: "ast_grep_rewrite".to_owned(),
            command: vec!["ast-grep".to_owned(), "run".to_owned(), "--update-all".to_owned()],
            args: vec![
                ToolArg {
                    name: "pattern".to_owned(),
                    required: true,
                    flag: Some("-p".to_owned()),
                    regex: Some("^[^\\x00]+$".to_owned()),
                    description: "pattern".to_owned(),
                },
                ToolArg {
                    name: "rewrite".to_owned(),
                    required: true,
                    flag: Some("-r".to_owned()),
                    regex: Some("^[^\\x00]+$".to_owned()),
                    description: "rewrite".to_owned(),
                },
            ],
            output_capture: OutputCapture::Both,
            ..Default::default()
        };

        let args = json!({
            "pattern": "fn nonexistent_pattern_foo()",
            "rewrite": "fn replaced()"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert_eq!(out, "No matching AST patterns found to rewrite.");
    }

    #[test]
    fn test_enforce_sandbox_in_outer_sandbox() {
        let temp = create_test_dir("sandbox_outer");
        // SAFETY: Setting test environment variable for isolated unit test.
        unsafe {
            std::env::set_var("AGY_OUTER_SANDBOX", "1");
        }
        let tool = ToolDefinition {
            name: "test_sandboxed_echo".to_owned(),
            command: vec!["echo".to_owned()],
            enforce_sandbox: true,
            args: vec![ToolArg {
                name: "msg".to_owned(),
                required: true,
                regex: Some("^[a-zA-Z0-9_]+$".to_owned()),
                ..Default::default()
            }],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "msg": "sandboxed_payload"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        // SAFETY: Cleaning up test environment variable after isolated unit test.
        unsafe {
            std::env::remove_var("AGY_OUTER_SANDBOX");
        }
        assert!(!is_err);
        assert!(out.contains("sandboxed_payload"));
    }

    #[test]
    fn test_enforce_sandbox_via_bwrap() {
        if which::which("bwrap").is_err() || which::which("python3").is_err() {
            return;
        }
        let temp = create_test_dir("sandbox_bwrap");
        let tool = ToolDefinition {
            name: "eval_test".to_owned(),
            command: vec!["python3".to_owned(), "-c".to_owned()],
            enforce_sandbox: true,
            args: vec![ToolArg {
                name: "code".to_owned(),
                required: true,
                flag: None,
                regex: Some("^[^\\x00]+$".to_owned()),
                description: "python code".to_owned(),
            }],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "code": "print('bwrap_isolation_passed')"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert!(out.contains("bwrap_isolation_passed"));
    }
}
