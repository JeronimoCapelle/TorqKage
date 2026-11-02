//! Bounded tool action execution engines.

pub(crate) mod cargo_json;
pub(crate) mod cli_manual;
pub(crate) mod file_read;
pub(crate) mod file_search;
pub(crate) mod file_write;
pub(crate) mod mpatch_edit;
pub(crate) mod parameterized_cmd;
pub(crate) mod static_cmd;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use agy_common::{CircuitBreakerLedger, SubprocessSupervisor, extract_rustc_error_code};
use crate::models::{AgentGuard, ToolAction, ToolDefinition};

/// Executes a command either sandboxed via Bubblewrap or directly with supervisor hygiene.
///
/// # Errors
/// Returns an error message if sandbox setup fails or the supervised command fails to execute.
pub(crate) fn execute_supervised_command(
    program: &str,
    base_args: &[String],
    dynamic_args: &[String],
    workspace_root: &Path,
    timeout_seconds: u64,
    enforce_sandbox: bool,
    tool_name: &str,
) -> Result<std::process::Output, String> {
    if enforce_sandbox {
        if agy_common::sandbox::is_inside_outer_sandbox() {
            let mut cmd = Command::new(program);
            cmd.args(base_args);
            cmd.args(dynamic_args);
            cmd.current_dir(workspace_root);
            SubprocessSupervisor::run_supervised(cmd, timeout_seconds)
        } else {
            if which::which("bwrap").is_err() {
                return Err(format!(
                    "Security enforcement error: Tool '{tool_name}' requires sandbox enforcement ('enforce_sandbox = true'), but 'bwrap' was not found on host and no outer sandbox is active."
                ));
            }
            let cmd = agy_common::sandbox::build_bubblewrap_command(
                program,
                base_args,
                dynamic_args,
                workspace_root,
            );
            let out = SubprocessSupervisor::run_supervised(cmd, timeout_seconds)?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("Creating new namespace failed")
                    || stderr.contains("No permissions to create new namespace")
                    || stderr.contains("Setting up mount namespace failed")
                {
                    return Err(format!(
                        "Security enforcement error: Sandbox execution failed for tool '{tool_name}': {stderr}"
                    ));
                }
            }
            Ok(out)
        }
    } else {
        let mut cmd = Command::new(program);
        cmd.args(base_args);
        cmd.args(dynamic_args);
        cmd.current_dir(workspace_root);
        SubprocessSupervisor::run_supervised(cmd, timeout_seconds)
    }
}

/// Dispatches tool execution to the appropriate action handler.
///
/// Returns a tuple of `(output_text, is_error)`.
///
/// # Errors
/// Returns an error message if the tool parameters, pre-flight gates, or execution fails.
pub(crate) fn dispatch_action(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
    agent_name: Option<&str>,
    agent_guards: &BTreeMap<String, AgentGuard>,
) -> Result<(String, bool), String> {
    evaluate_pre_flight_guards(tool, workspace_root)?;

    let execution_result = match tool.action {
        ToolAction::FileRead => {
            let content = file_read::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::FileWrite => {
            let content = file_write::execute(tool, workspace_root, arguments, agent_name, agent_guards)?;
            Ok((content, false))
        }

        ToolAction::MpatchEdit => {
            let content = mpatch_edit::execute(tool, workspace_root, arguments, agent_name, agent_guards)?;
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
    };

    update_circuit_breaker(tool, workspace_root, agent_name, execution_result)
}

fn is_compiler_verification_tool(tool: &ToolDefinition) -> bool {
    tool.tracks_circuit_breaker() || tool.action == ToolAction::CargoJsonDiagnostics
}

fn update_circuit_breaker(
    tool: &ToolDefinition,
    workspace_root: &Path,
    agent_name: Option<&str>,
    result: Result<(String, bool), String>,
) -> Result<(String, bool), String> {
    if !is_compiler_verification_tool(tool) {
        return result;
    }

    let mut ledger = CircuitBreakerLedger::load(workspace_root);

    match result {
        Ok((mut output, is_error)) => {
            if is_error {
                let error_sig = extract_rustc_error_code(&output);
                drop(ledger.record_failure(
                    workspace_root,
                    agent_name,
                    Some(&tool.name),
                    error_sig.as_deref().or(Some("Compiler error")),
                    Some(&output),
                ));
                if ledger.tripped {
                    let banner = format!(
                        "\n\n🛑 [CIRCUIT BREAKER TRIPPED - {}/{} FAILURES]:\n\
                         Consecutive compilation failures reached threshold ({}).\n\
                         Last compiler error: {}\n\
                         Further autonomous code modifications are now GATED by policy hook.\n\
                         Operator intervention or manual code repair is required.",
                        ledger.consecutive_failures,
                        ledger.max_strikes,
                        ledger.max_strikes,
                        ledger.last_error_signature.as_deref().unwrap_or("unknown")
                    );
                    output.push_str(&banner);
                }
            } else {
                drop(ledger.record_success(workspace_root, Some(&tool.name)));
            }
            Ok((output, is_error))
        }
        Err(err) => {
            let error_sig = extract_rustc_error_code(&err);
            drop(ledger.record_failure(
                workspace_root,
                agent_name,
                Some(&tool.name),
                error_sig.as_deref().or(Some("Execution failure")),
                Some(&err),
            ));
            Err(err)
        }
    }
}

fn evaluate_pre_flight_guards(tool: &ToolDefinition, workspace_root: &Path) -> Result<(), String> {
    if tool.require_git_clean() {
        check_git_clean(workspace_root)?;
    }
    if tool.require_clippy_clean() {
        check_clippy_clean(workspace_root)?;
    }
    if tool.require_tests_passed() {
        check_tests_passed(workspace_root)?;
    }
    Ok(())
}

fn check_git_clean(workspace_root: &Path) -> Result<(), String> {
    if agy_common::JujutsuWorkspace::is_jj_repo(workspace_root) {
        if !agy_common::JujutsuWorkspace::is_workspace_clean(workspace_root) {
            return Err("Pre-flight gate failed: 'require_workspace_clean' is enabled, but workspace has uncommitted modifications.".to_owned());
        }
        return Ok(());
    }

    if workspace_root.join(".git").exists() {
        let git_out = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(workspace_root)
            .output();

        if let Ok(gout) = git_out
            && gout.status.success()
            && !gout.stdout.is_empty()
        {
            return Err("Pre-flight gate failed: 'require_workspace_clean' is enabled, but workspace has uncommitted modifications.".to_owned());
        }
    }

    Ok(())
}

fn check_clippy_clean(workspace_root: &Path) -> Result<(), String> {
    if !workspace_root.join("Cargo.toml").is_file() {
        return Ok(());
    }

    let output = Command::new("cargo")
        .args(["clippy", "--all-targets", "--", "-D", "warnings"])
        .current_dir(workspace_root)
        .output()
        .map_err(|err| format!("Failed to spawn cargo clippy gate: {err}"))?;

    if !output.status.success() {
        return Err("Pre-flight gate failed: 'require_clippy_clean' is enabled, but cargo clippy reported warnings or errors.".to_owned());
    }
    Ok(())
}

fn check_tests_passed(workspace_root: &Path) -> Result<(), String> {
    if !workspace_root.join("Cargo.toml").is_file() {
        return Ok(());
    }

    let output = Command::new("cargo")
        .args(["test", "--no-fail-fast"])
        .current_dir(workspace_root)
        .output()
        .map_err(|err| format!("Failed to spawn cargo test gate: {err}"))?;

    if !output.status.success() {
        return Err("Pre-flight gate failed: 'require_tests_passed' is enabled, but test suites reported failures.".to_owned());
    }
    Ok(())
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
    fn test_update_circuit_breaker_trips_on_consecutive_failures() {
        let temp_dir = std::env::temp_dir().join(format!("mcp_cb_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        drop(std::fs::create_dir_all(&temp_dir));

        let tool = ToolDefinition {
            name: "run_quick_check".to_owned(),
            safety: agy_common::tools::ToolSafetyPolicy {
                tracks_circuit_breaker: true,
                ..Default::default()
            },
            action: ToolAction::StaticCommand,
            ..Default::default()
        };

        // Failure 1
        let res1 = update_circuit_breaker(
            &tool,
            &temp_dir,
            Some("coder"),
            Ok(("error[E0502]: borrow error".to_owned(), true)),
        ).unwrap();
        assert!(res1.1);
        assert!(!res1.0.contains("CIRCUIT BREAKER TRIPPED"));

        // Failure 2 -> Tripped!
        let res2 = update_circuit_breaker(
            &tool,
            &temp_dir,
            Some("coder"),
            Ok(("error[E0502]: borrow error".to_owned(), true)),
        ).unwrap();
        assert!(res2.1);
        assert!(res2.0.contains("CIRCUIT BREAKER TRIPPED - 2/2 FAILURES"));

        // Success -> Cleared!
        let res3 = update_circuit_breaker(
            &tool,
            &temp_dir,
            Some("coder"),
            Ok(("Clean check".to_owned(), false)),
        ).unwrap();
        assert!(!res3.1);

        let ledger = CircuitBreakerLedger::load(&temp_dir);
        assert_eq!(ledger.consecutive_failures, 0);
        assert!(!ledger.tripped);

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_evaluate_pre_flight_guards_passes_clean() {
        let temp_dir = std::env::temp_dir().join(format!("mcp_guards_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        drop(std::fs::create_dir_all(&temp_dir));

        let tool = ToolDefinition {
            name: "test_guarded_tool".to_owned(),
            preconditions: agy_common::tools::ToolPreconditions {
                require_git_clean: true,
                require_clippy_clean: true,
                require_tests_passed: true,
            },
            ..Default::default()
        };

        // In a directory with no Cargo.toml and no git, pre-flight guards pass gracefully
        let res = evaluate_pre_flight_guards(&tool, &temp_dir);
        res.unwrap();

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
