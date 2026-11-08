//! Bounded tool action execution engines.

pub(crate) mod cli_manual;
pub(crate) mod file_read;
pub(crate) mod file_search;
pub(crate) mod file_write;
pub(crate) mod mpatch_edit;
pub(crate) mod parameterized_cmd;
pub(crate) mod project_state;
pub(crate) mod static_cmd;
pub(crate) mod workspace_ensure;
pub(crate) mod workspace_inspect;
pub(crate) mod workspace_merge;

use std::{collections::BTreeMap, path::Path, process::Command};

use agy_common::{CircuitBreakerLedger, SubprocessSupervisor, extract_compiler_error_signature};

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
    execute_supervised_command_with_detector(
        program,
        (base_args, dynamic_args),
        workspace_root,
        timeout_seconds,
        enforce_sandbox,
        tool_name,
        agy_common::sandbox::is_inside_container,
    )
}

/// Executes a command with configurable sandbox detection (useful for dependency injection in tests).
pub(crate) fn execute_supervised_command_with_detector<F>(
    program: &str,
    args: (&[String], &[String]),
    workspace_root: &Path,
    timeout_seconds: u64,
    enforce_sandbox: bool,
    tool_name: &str,
    is_outer_sandbox: F,
) -> Result<std::process::Output, String>
where
    F: Fn() -> bool,
{
    if !program.contains('/') && which::which(program).is_err() {
        return Err(format!(
            "Tool '{tool_name}' failed to execute: binary '{program}' is not installed on host ($PATH)"
        ));
    }

    let (base_args, dynamic_args) = args;
    let project_cfg = agy_common::ProjectConfig::detect_or_load(workspace_root);
    let resolved_env = project_cfg.resolved_environment(workspace_root);

    if enforce_sandbox {
        if which::which("bwrap").is_ok() {
            let resolved_program = if program.contains('/') {
                program.to_owned()
            } else {
                which::which(program)
                    .map_or_else(|_| program.to_owned(), |p| p.to_string_lossy().into_owned())
            };
            let mut cmd = agy_common::sandbox::build_bubblewrap_command(
                &resolved_program,
                base_args,
                dynamic_args,
                workspace_root,
            );
            cmd.envs(&resolved_env);
            let out = SubprocessSupervisor::run_supervised(cmd, timeout_seconds)?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let is_bwrap_setup_failure = stderr.contains("bwrap:")
                    || stderr.contains("Creating new namespace failed")
                    || stderr.contains("No permissions to create new namespace")
                    || stderr.contains("Setting up mount namespace failed")
                    || stderr.contains("user namespace")
                    || stderr.contains("Permission denied")
                    || stderr.contains("Operation not permitted");

                if is_bwrap_setup_failure {
                    if is_outer_sandbox() {
                        let mut fallback_cmd = Command::new(program);
                        fallback_cmd.args(base_args);
                        fallback_cmd.args(dynamic_args);
                        fallback_cmd.envs(&resolved_env);
                        fallback_cmd.current_dir(workspace_root);
                        return SubprocessSupervisor::run_supervised(fallback_cmd, timeout_seconds);
                    }
                    return Err(format!(
                        "Security enforcement error: Sandbox execution failed for tool '{tool_name}': {stderr}"
                    ));
                }
            }
            Ok(out)
        } else if is_outer_sandbox() {
            let mut cmd = Command::new(program);
            cmd.args(base_args);
            cmd.args(dynamic_args);
            cmd.envs(&resolved_env);
            cmd.current_dir(workspace_root);
            SubprocessSupervisor::run_supervised(cmd, timeout_seconds)
        } else {
            Err(format!(
                "Security enforcement error: Tool '{tool_name}' requires sandbox enforcement ('enforce_sandbox = true'), but 'bwrap' was not found on host."
            ))
        }
    } else {
        let mut cmd = Command::new(program);
        cmd.args(base_args);
        cmd.args(dynamic_args);
        cmd.envs(&resolved_env);
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
    available_tools: &[ToolDefinition],
) -> Result<(String, bool), String> {
    evaluate_pre_flight_guards(tool, workspace_root)?;

    let execution_result = match tool.action {
        ToolAction::FileRead => {
            let content =
                file_read::execute(tool, workspace_root, arguments, agent_name, agent_guards)?;
            Ok((content, false))
        },

        ToolAction::FileWrite => {
            let content =
                file_write::execute(tool, workspace_root, arguments, agent_name, agent_guards)?;
            Ok((content, false))
        },

        ToolAction::MpatchEdit => {
            let content =
                mpatch_edit::execute(tool, workspace_root, arguments, agent_name, agent_guards)?;
            Ok((content, false))
        },

        ToolAction::FileSearch => {
            let content = file_search::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        },

        ToolAction::StaticCommand => static_cmd::execute(tool, workspace_root),

        ToolAction::ParameterizedCommand => {
            parameterized_cmd::execute(tool, workspace_root, arguments)
        },

        ToolAction::CliManual => Ok(cli_manual::execute(tool, arguments, available_tools)),

        ToolAction::ProjectState => Ok(project_state::execute(workspace_root)),

        ToolAction::WorkspaceEnsure => workspace_ensure::execute(workspace_root, arguments),

        ToolAction::WorkspaceInspect => workspace_inspect::execute(workspace_root, arguments),

        ToolAction::WorkspaceMerge => workspace_merge::execute(workspace_root, arguments),

        ToolAction::Unknown(_) => Err(format!(
            "Unknown or unsupported tool action for tool '{}'",
            tool.name
        )),
    };

    update_circuit_breaker(tool, workspace_root, agent_name, execution_result)
}

const fn is_compiler_verification_tool(tool: &ToolDefinition) -> bool {
    tool.tracks_circuit_breaker()
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
    let project = agy_common::ProjectConfig::detect_or_load(workspace_root);
    let custom_pattern = project
        .gates
        .values()
        .find_map(|g| g.error_regex.as_deref());

    match result {
        Ok((mut output, is_error)) => {
            if is_error {
                let error_sig = extract_compiler_error_signature(&output, custom_pattern);
                drop(ledger.record_failure(
                    workspace_root,
                    agent_name,
                    Some(&tool.name),
                    error_sig.as_deref().or(Some("Quality gate failure")),
                    Some(&output),
                ));
                if ledger.tripped {
                    let banner = format!(
                        "\n\n🛑 [CIRCUIT BREAKER TRIPPED - {}/{} FAILURES]:\n\
                         Consecutive verification/gate failures reached threshold ({}).\n\
                         Last failure signature: {}\n\
                         Further autonomous code modifications are now GATED by policy hook.\n\
                         Operator intervention or manual repair is required.",
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
        },
        Err(err) => {
            let error_sig = extract_compiler_error_signature(&err, custom_pattern);
            drop(ledger.record_failure(
                workspace_root,
                agent_name,
                Some(&tool.name),
                error_sig.as_deref().or(Some("Execution failure")),
                Some(&err),
            ));
            Err(err)
        },
    }
}

fn evaluate_pre_flight_guards(tool: &ToolDefinition, workspace_root: &Path) -> Result<(), String> {
    let required_gates = tool.required_gates();
    if !required_gates.is_empty() {
        let gate_names: Vec<&str> = required_gates.iter().map(String::as_str).collect();
        match agy_common::gatekeeper::evaluate_pre_flight_checks(workspace_root, &gate_names) {
            Ok(_warnings) => {},
            Err(errors) => {
                return Err(format!(
                    "Pre-flight gate check failed for tool '{}':\n{}",
                    tool.name,
                    errors.join("\n\n")
                ));
            },
        }
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
        let temp_dir = std::env::temp_dir().join(format!(
            "mcp_cb_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));

        let tool = ToolDefinition {
            name: "quick_check".to_owned(),
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
            Some("worker_agent"),
            Ok(("error[E0502]: borrow error".to_owned(), true)),
        )
        .unwrap();
        assert!(res1.1);
        assert!(!res1.0.contains("CIRCUIT BREAKER TRIPPED"));

        // Failure 2 -> Tripped!
        let res2 = update_circuit_breaker(
            &tool,
            &temp_dir,
            Some("worker_agent"),
            Ok(("error[E0502]: borrow error".to_owned(), true)),
        )
        .unwrap();
        assert!(res2.1);
        assert!(res2.0.contains("CIRCUIT BREAKER TRIPPED - 2/2 FAILURES"));

        // Success -> Cleared!
        let res3 = update_circuit_breaker(
            &tool,
            &temp_dir,
            Some("worker_agent"),
            Ok(("Clean check".to_owned(), false)),
        )
        .unwrap();
        assert!(!res3.1);

        let ledger = CircuitBreakerLedger::load(&temp_dir);
        assert_eq!(ledger.consecutive_failures, 0);
        assert!(!ledger.tripped);

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_evaluate_pre_flight_guards_passes_clean() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mcp_guards_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));

        let tool = ToolDefinition {
            name: "test_guarded_tool".to_owned(),
            preconditions: agy_common::tools::ToolPreconditions {
                require_gates: vec![
                    "workspace_clean".to_owned(),
                    "linter".to_owned(),
                    "tests".to_owned(),
                ],
            },
            ..Default::default()
        };

        // In a directory with no Cargo.toml and no git, pre-flight guards pass gracefully
        let res = evaluate_pre_flight_guards(&tool, &temp_dir);
        res.unwrap();

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
