//! # launcher
//!
//! Interactive process launcher and pre-flight validation pipeline for dispatching `agy` sessions from `agy-ctrl`.

use std::io::{self, stdout};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use crate::model::AgentManifest;

/// Result of evaluating pre-launch checks prior to starting an `agy` session.
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
                msg.push_str(&format!("  • {err}\n"));
            }
        }
        if !self.warnings.is_empty() {
            msg.push_str("\n⚠ Warnings:\n");
            for warn in &self.warnings {
                msg.push_str(&format!("  • {warn}\n"));
            }
        }
        msg
    }
}

/// Executes all mandatory and advisory pre-launch checks before booting an `agy` session.
///
/// This provides a modular, extensible hook for gating agent execution.
pub(crate) fn run_pre_launch_checks(workspace_root: &Path, agent: &AgentManifest) -> PreLaunchReport {
    let mut report = PreLaunchReport::new(&agent.name);

    // Check 1: Verify agy executable is discoverable in system PATH
    if which::which("agy").is_err() {
        report.errors.push(
            "The 'agy' CLI executable could not be found in system PATH. Ensure Antigravity is installed.".to_owned(),
        );
    }

    // Check 2: Verify the agent's manifest file exists on disk
    if !agent.file_path.is_file() {
        report.errors.push(format!(
            "Agent manifest does not exist at '{}'. Please save or restore configurations.",
            agent.file_path.display()
        ));
    }

    // Check 3: Check for uncommitted/unsaved changes in the agy-ctrl editor buffer
    if agent.modified {
        report.errors.push(format!(
            "Agent '{}' has unsaved modifications in agy-ctrl. Press [F2] to Save & Validate before launching.",
            agent.name
        ));
    }

    // Check 4: Plugin and tools configuration integrity
    let tools_toml = workspace_root.join("tools.toml");
    let plugin_tools = workspace_root.join("plugins/agy-suite/tools.toml");
    let has_home_tools = std::env::var("HOME").is_ok_and(|home| {
        Path::new(&home).join(".gemini/config/plugins/agy-suite/tools.toml").is_file()
    });
    if !tools_toml.is_file() && !plugin_tools.is_file() && !has_home_tools {
        report.warnings.push("No tools.toml configuration found in workspace or plugin install.".to_owned());
    }

    // Check 5: Air-gapped / security notice
    if agent.mcp_tools.is_empty() && agent.native_tools.is_empty() {
        report.warnings.push(
            "Agent has no MCP or native tools assigned. It will run in completely air-gapped baseline mode.".to_owned(),
        );
    }

    report
}

/// Suspends Ratatui, runs pre-launch checks, and spawns an interactive `agy` CLI session.
///
/// Once the user exits the `agy` session, raw mode and alternate screen are re-established,
/// returning the user seamlessly back to the `agy-ctrl` deck.
///
/// # Errors
/// Returns an error string if pre-launch checks fail or if `agy` cannot be executed.
pub(crate) fn launch_agy_interactive(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    workspace_root: &Path,
    agent: &AgentManifest,
) -> Result<ExitStatus, String> {
    // 1. Run pre-launch checks
    let report = run_pre_launch_checks(workspace_root, agent);
    if !report.is_ok() {
        return Err(report.format_diagnostics());
    }

    // 2. Suspend Ratatui & restore terminal to normal canonical mode
    drop(disable_raw_mode());
    drop(execute!(stdout(), LeaveAlternateScreen));
    drop(terminal.show_cursor());

    // Print friendly launcher header
    println!("\n================================================================================");
    println!("🚀 [agy-ctrl] Launching interactive agy session with agent: {}", agent.name);
    println!("   Effort: {} | Permissions: {}", agent.effort, agent.permission_mode);
    println!("   Workspace: {}", workspace_root.display());
    println!("   (Exiting the agy session will return you directly to agy-ctrl)");
    println!("================================================================================\n");

    // 3. Spawn agy with inherited stdio
    let status_res = Command::new("agy")
        .arg("--agent")
        .arg(&agent.name)
        .current_dir(workspace_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    // 4. Resume Ratatui & crossterm alternate screen regardless of status
    drop(enable_raw_mode());
    let mut stdout_handle = stdout();
    drop(execute!(stdout_handle, EnterAlternateScreen));
    drop(terminal.clear());
    drop(terminal.hide_cursor());

    status_res.map_err(|e| format!("Failed to execute 'agy': {e}"))
}

/// Headless launcher: runs pre-launch checks and spawns `agy --agent <name>` directly from CLI.
///
/// # Errors
/// Returns an error string if pre-launch checks fail or if `agy` cannot be spawned.
pub(crate) fn launch_agy_headless(workspace_root: &Path, agent: &AgentManifest) -> Result<ExitStatus, String> {
    let report = run_pre_launch_checks(workspace_root, agent);
    if !report.is_ok() {
        return Err(report.format_diagnostics());
    }

    println!("🚀 [agy-ctrl] Launching agy session with agent: {}", agent.name);

    Command::new("agy")
        .arg("--agent")
        .arg(&agent.name)
        .current_dir(workspace_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to spawn 'agy': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
            main_agent: false,
            subagent: true,
            inherit_mcp: false,
            allowed_subagents: Vec::new(),
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            directives: String::new(),
            modified: true,
        };

        let report = run_pre_launch_checks(Path::new("."), &manifest);
        assert!(!report.is_ok());
        assert!(report.errors.iter().any(|e| e.contains("unsaved modifications")));
    }
}
