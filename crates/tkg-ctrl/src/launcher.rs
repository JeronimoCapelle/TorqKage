//! # launcher
//!
//! Interactive process launcher and pre-flight validation pipeline for dispatching agent sessions from `tkg`.

use std::{
    fmt::Write,
    io::{self, stdout},
    path::Path,
    process::{Command, ExitStatus, Stdio},
};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::model::AgentManifest;

/// Result of evaluating pre-launch checks prior to starting an agent session.
#[derive(Debug, Clone)]
pub(crate) struct PreLaunchReport {
    pub(crate) agent_name: String,
    pub(crate) errors: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

impl PreLaunchReport {
    /// Constructs a new empty report for the given agent.
    #[must_use]
    pub(crate) fn new(agent_name: &str) -> Self {
        Self {
            agent_name: agent_name.to_owned(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Returns `true` if all mandatory pre-launch checks succeeded.
    #[must_use]
    pub(crate) const fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Formats errors and warnings into a readable diagnostic string.
    #[must_use]
    pub(crate) fn format_diagnostics(&self) -> String {
        let mut msg = format!("Pre-launch checks for agent '{}':\n", self.agent_name);
        if !self.errors.is_empty() {
            msg.push_str("\n❌ Blocking Errors:\n");
            for err in &self.errors {
                _ = writeln!(msg, "  • {err}");
            }
        }
        if !self.warnings.is_empty() {
            msg.push_str("\n⚠ Warnings:\n");
            for warn in &self.warnings {
                _ = writeln!(msg, "  • {warn}");
            }
        }
        msg
    }
}

/// Executes all mandatory and advisory pre-launch checks before booting an agent session.
///
/// This provides a modular, extensible hook for gating agent execution.
pub(crate) fn run_pre_launch_checks(
    workspace_root: &Path,
    agent: &AgentManifest,
) -> PreLaunchReport {
    let mut report = PreLaunchReport::new(&agent.name);
    let project_cfg = tkg_common::project::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    let binary = runner.resolved_binary();

    // Check 1: Verify harness executable is discoverable in system PATH
    if which::which(&binary).is_err() {
        report.errors.push(format!(
            "The '{}' CLI executable ('{binary}') could not be found in system PATH. Ensure it is installed.",
            runner.harness
        ));
    }

    // Check 2: Verify the agent's manifest file exists on disk
    if !agent.file_path.is_file() {
        report.errors.push(format!(
            "Agent manifest does not exist at '{}'. Please save or restore configurations.",
            agent.file_path.display()
        ));
    }

    // Check 3: Check for uncommitted/unsaved changes in the tkg editor buffer
    if agent.modified {
        report.errors.push(format!(
            "Agent '{}' has unsaved modifications in tkg. Press [F2] to Save & Validate before launching.",
            agent.name
        ));
    }

    // Check 4: Plugin and tools configuration integrity
    let tools_toml = workspace_root.join("tools.toml");
    let plugin_tools = workspace_root.join("plugins/torqkage/tools.toml");
    let has_home_tools = std::env::var("HOME").is_ok_and(|home| {
        Path::new(&home)
            .join(".gemini/config/plugins/torqkage/tools.toml")
            .is_file()
    });
    if !tools_toml.is_file() && !plugin_tools.is_file() && !has_home_tools {
        report
            .warnings
            .push("No tools.toml configuration found in workspace or plugin install.".to_owned());
    }

    // Check 5: Air-gapped / security notice
    if agent.mcp_tools.is_empty() && agent.native_tools.is_empty() {
        report.warnings.push(
            "Agent has no MCP or native tools assigned. It will run in completely air-gapped baseline mode.".to_owned(),
        );
    }

    // Check 6: Pre-flight admission checks declared in agent frontmatter
    if !agent.pre_launch_checks.is_empty() {
        let check_refs: Vec<&str> = agent.pre_launch_checks.iter().map(String::as_str).collect();
        match tkg_common::evaluate_pre_flight_checks(workspace_root, &check_refs) {
            Ok(warns) => {
                report.warnings.extend(warns);
            },
            Err(errs) => {
                report.errors.extend(errs);
            },
        }
    }

    report
}

/// Suspends Ratatui, runs pre-launch checks, and spawns an interactive session.
///
/// Builds a configured `Command` ready to execute for the configured execution harness.
#[must_use]
pub(crate) fn build_harness_command(
    workspace_root: &Path,
    runner: &tkg_common::ProjectRunnerConfig,
    agent_name: &str,
    sandbox: bool,
    resume_conversation: Option<&str>,
) -> Command {
    let project_cfg = tkg_common::project::ProjectConfig::detect_or_load(workspace_root);
    let binary = runner.resolved_binary();
    let mut cmd = Command::new(&binary);

    for (k, v) in project_cfg.resolved_environment(workspace_root) {
        cmd.env(k, v);
    }

    if let Some(custom_cmd) = &runner.command
        && let Some(sub_args) = custom_cmd.get(1..)
    {
        for arg in sub_args {
            if arg == "{agent}" {
                cmd.arg(agent_name);
            } else if (arg == "{conversation}" || arg == "{session}")
                && let Some(cid) = resume_conversation
            {
                cmd.arg(cid);
            } else {
                cmd.arg(arg);
            }
        }
    } else {
        if sandbox && let Some(flag) = runner.resolved_sandbox_flag() {
            cmd.arg(flag);
        }
        if let Some(conv_id) = resume_conversation {
            if let Some(flag) = runner.resolved_resume_flag() {
                cmd.arg(flag).arg(conv_id);
            }
        } else if let Some(flag) = runner.resolved_agent_flag() {
            cmd.arg(flag).arg(agent_name);
        }
    }

    for extra in &runner.extra_args {
        cmd.arg(extra);
    }

    cmd
}

/// Interactive launcher: suspends Ratatui, runs pre-flight checks, spawns the agent session,
/// and restores the terminal on exit.
///
/// # Errors
/// Returns an error string if pre-launch checks fail or if the harness cannot be executed.
pub(crate) fn launch_agent_interactive(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    workspace_root: &Path,
    agent: &AgentManifest,
    sandbox: bool,
    isolated_workspace: bool,
    resume_conversation: Option<&str>,
) -> Result<ExitStatus, String> {
    let project_cfg = tkg_common::project::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    let binary = runner.resolved_binary();

    // 1. Run pre-launch checks
    let report = run_pre_launch_checks(workspace_root, agent);
    if !report.is_ok() {
        return Err(report.format_diagnostics());
    }

    // 2. Suspend Ratatui & restore terminal to normal canonical mode
    drop(disable_raw_mode());
    drop(execute!(stdout(), LeaveAlternateScreen));
    drop(terminal.show_cursor());

    let (effective_workspace, was_isolated) = if isolated_workspace {
        match tkg_common::JujutsuWorkspace::ensure_agent_workspace(workspace_root, &agent.name) {
            Ok(ws_path) => (ws_path, true),
            Err(e) => {
                eprintln!(
                    "⚠ [tkg] Failed to initialize isolated workspace: {e}. Falling back to root."
                );
                (workspace_root.to_path_buf(), false)
            },
        }
    } else {
        (workspace_root.to_path_buf(), false)
    };

    // Print friendly launcher header
    println!("\n================================================================================");
    if let Some(conv_id) = resume_conversation {
        println!(
            "🚀 [tkg] RESUMING previous {} session for agent: {}",
            runner.harness, agent.name
        );
        println!("   Conversation ID: {conv_id}");
    } else {
        println!(
            "🚀 [tkg] Launching fresh {} session with agent: {}",
            runner.harness, agent.name
        );
    }
    println!(
        "   Effort: {} | Permissions: {} | Policy: {}",
        agent.effort, agent.permission_mode, agent.command_execution_policy
    );
    println!(
        "   Sandbox: {}",
        if sandbox {
            "ENABLED (Bubblewrap isolation)"
        } else {
            "DISABLED (direct host)"
        }
    );
    if was_isolated {
        println!(
            "   Workspace: {} (🛡️ Jujutsu Isolated Workspace)",
            effective_workspace.display()
        );
        println!("   (Main working copy @ and editor buffers are fully protected)");
    } else {
        println!("   Workspace: {}", workspace_root.display());
    }
    println!(
        "   (Exiting the {} session will return you directly to tkg)",
        runner.harness
    );
    println!("================================================================================\n");

    // 3. Spawn harness with inherited stdio
    let mut cmd = build_harness_command(
        workspace_root,
        &runner,
        &agent.name,
        sandbox,
        resume_conversation,
    );
    cmd.current_dir(&effective_workspace);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status_res = cmd.status();

    if was_isolated {
        println!(
            "\n--------------------------------------------------------------------------------"
        );
        println!(
            "🏁 [tkg] Session ended in isolated workspace: .tkg/workspaces/{}",
            agent.name
        );
        println!("   Changes are committed to workspace '{}@'.", agent.name);
        println!(
            "   Use 'jj workspace list' or 'jj diff -R .tkg/workspaces/{}' to inspect.",
            agent.name
        );
        println!(
            "--------------------------------------------------------------------------------"
        );
    }

    // 4. Resume Ratatui & crossterm alternate screen regardless of status
    drop(enable_raw_mode());
    let mut stdout_handle = stdout();
    drop(execute!(stdout_handle, EnterAlternateScreen));
    drop(terminal.clear());
    drop(terminal.hide_cursor());

    status_res.map_err(|e| format!("Failed to execute '{binary}': {e}"))
}

/// Headless launcher: runs pre-launch checks and spawns the configured agent harness directly from CLI.
///
/// # Errors
/// Returns an error string if pre-launch checks fail or if the harness cannot be spawned.
pub(crate) fn launch_agent_headless(
    workspace_root: &Path,
    agent: &AgentManifest,
    sandbox: bool,
    isolated_workspace: bool,
    resume_conversation: Option<&str>,
) -> Result<ExitStatus, String> {
    let project_cfg = tkg_common::project::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    let binary = runner.resolved_binary();

    let report = run_pre_launch_checks(workspace_root, agent);
    if !report.is_ok() {
        return Err(report.format_diagnostics());
    }

    let (effective_workspace, was_isolated) = if isolated_workspace {
        match tkg_common::JujutsuWorkspace::ensure_agent_workspace(workspace_root, &agent.name) {
            Ok(ws_path) => (ws_path, true),
            Err(e) => {
                eprintln!(
                    "⚠ [tkg] Failed to initialize isolated workspace: {e}. Falling back to root."
                );
                (workspace_root.to_path_buf(), false)
            },
        }
    } else {
        (workspace_root.to_path_buf(), false)
    };

    if let Some(conv_id) = resume_conversation {
        println!(
            "🚀 [tkg] Resuming {} session {conv_id} with agent: {} (Sandbox: {}, Isolated: {})",
            runner.harness,
            agent.name,
            if sandbox { "ENABLED" } else { "DISABLED" },
            if was_isolated {
                "YES (.tkg/workspaces)"
            } else {
                "NO"
            }
        );
    } else {
        println!(
            "🚀 [tkg] Launching {} session with agent: {} (Sandbox: {}, Isolated: {})",
            runner.harness,
            agent.name,
            if sandbox { "ENABLED" } else { "DISABLED" },
            if was_isolated {
                "YES (.tkg/workspaces)"
            } else {
                "NO"
            }
        );
    }

    let mut cmd = build_harness_command(
        workspace_root,
        &runner,
        &agent.name,
        sandbox,
        resume_conversation,
    );
    cmd.current_dir(&effective_workspace);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let res = cmd
        .status()
        .map_err(|e| format!("Failed to spawn '{binary}': {e}"));

    if was_isolated {
        println!(
            "\n🏁 [tkg] Session ended in isolated workspace: .tkg/workspaces/{}",
            agent.name
        );
        println!("   Changes are committed to workspace '{}@'.", agent.name);
    }

    res
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "Unit test assertions"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::model::AgentRoleFlags;

    #[test]
    fn test_pre_launch_report_diagnostics() {
        let mut report = PreLaunchReport::new("test-agent");
        assert!(report.is_ok());

        report.errors.push("Failed critical check".to_owned());
        assert!(!report.is_ok());

        let diag = report.format_diagnostics();
        assert!(diag.contains("test-agent"));
        assert!(diag.contains("Failed critical check"));
    }

    #[test]
    fn test_run_pre_launch_checks_unsaved_changes() {
        let manifest = AgentManifest {
            file_path: PathBuf::from("nonexistent.md"),
            name: "dirty-agent".to_owned(),
            description: "test".to_owned(),
            effort: "low".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            command_execution_policy: "off".to_owned(),
            role_flags: AgentRoleFlags {
                main_agent: false,
                subagent: true,
            },
            inherit_mcp: false,
            allowed_subagents: Vec::new(),
            pre_launch_checks: Vec::new(),
            allowed_dirs: Vec::new(),
            disallowed_dirs: Vec::new(),
            allowed_files: Vec::new(),
            disallowed_files: Vec::new(),
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            directives: String::new(),
            modified: true,
        };

        let report = run_pre_launch_checks(Path::new("."), &manifest);
        assert!(!report.is_ok());
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("unsaved modifications"))
        );
    }

    #[test]
    fn test_run_pre_launch_checks_declarative_gatekeeper() {
        let manifest = AgentManifest {
            file_path: PathBuf::from("nonexistent.md"),
            name: "declarative-sentinel".to_owned(),
            description: "test sentinel".to_owned(),
            effort: "high".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            command_execution_policy: "prompt".to_owned(),
            role_flags: AgentRoleFlags {
                main_agent: false,
                subagent: true,
            },
            inherit_mcp: false,
            allowed_subagents: Vec::new(),
            pre_launch_checks: vec![
                "compilation".to_owned(),
                "tests".to_owned(),
                "vcs_conflicts".to_owned(),
            ],
            allowed_dirs: Vec::new(),
            disallowed_dirs: Vec::new(),
            allowed_files: Vec::new(),
            disallowed_files: Vec::new(),
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            directives: String::new(),
            modified: false,
        };

        // When run against an empty temporary directory with no Cargo.toml, gatekeeper passes cleanly
        let temp_dir = std::env::temp_dir();
        let report = run_pre_launch_checks(&temp_dir, &manifest);
        // The only errors should be missing manifest file and possibly agy CLI if PATH is weird,
        // but no cargo check errors from gatekeeper
        assert!(
            !report
                .errors
                .iter()
                .any(|e| e.contains("requires 100% clean compilation"))
        );
    }

    #[test]
    fn test_launch_headless_pre_check_failure() {
        let manifest = AgentManifest {
            file_path: PathBuf::from("nonexistent.md"),
            name: "broken-agent".to_owned(),
            description: "test".to_owned(),
            effort: "low".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            command_execution_policy: "off".to_owned(),
            role_flags: AgentRoleFlags {
                main_agent: false,
                subagent: true,
            },
            inherit_mcp: false,
            allowed_subagents: Vec::new(),
            pre_launch_checks: Vec::new(),
            allowed_dirs: Vec::new(),
            disallowed_dirs: Vec::new(),
            allowed_files: Vec::new(),
            disallowed_files: Vec::new(),
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            directives: String::new(),
            modified: true, // triggers pre-check error
        };

        let res = launch_agent_headless(Path::new("."), &manifest, false, true, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("unsaved modifications"));
    }

    #[test]
    fn test_build_harness_command_opencode() {
        let runner = tkg_common::ProjectRunnerConfig {
            harness: "opencode".to_owned(),
            command: None,
            agent_flag: None,
            sandbox_flag: None,
            resume_flag: None,
            extra_args: vec!["--debug".to_owned()],
            agent_dirs: Vec::new(),
        };

        let cmd = build_harness_command(Path::new("."), &runner, "coder", true, None);
        let program = cmd.get_program().to_string_lossy().to_string();
        assert_eq!(program, "opencode");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(args, vec!["--agent", "coder", "--debug"]);
    }

    #[test]
    fn test_build_harness_command_custom_template() {
        let runner = tkg_common::ProjectRunnerConfig {
            harness: "custom".to_owned(),
            command: Some(vec![
                "custom-harness".to_owned(),
                "run".to_owned(),
                "--agent".to_owned(),
                "{agent}".to_owned(),
            ]),
            agent_flag: None,
            sandbox_flag: None,
            resume_flag: None,
            extra_args: Vec::new(),
            agent_dirs: Vec::new(),
        };

        let cmd = build_harness_command(Path::new("."), &runner, "reviewer", false, None);
        let program = cmd.get_program().to_string_lossy().to_string();
        assert_eq!(program, "custom-harness");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(args, vec!["run", "--agent", "reviewer"]);
    }
}
