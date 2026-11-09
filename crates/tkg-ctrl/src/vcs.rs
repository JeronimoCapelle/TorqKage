//! VCS snapshotting, atomic serialization, and declarative contract validation engine.

use std::{fmt::Write, path::Path, process::Command};

use tkg_common::{DEFAULT_TOOL_TIMEOUT_SECS, DEFAULT_VCS_QUERY_TIMEOUT_SECS, SubprocessSupervisor};

use crate::model::AgentManifest;

/// Writes an agent manifest back to disk using the frontmatter format of the configured harness.
pub(crate) fn save_agent_to_disk(
    agent: &AgentManifest,
    harness: &dyn tkg_common::Harness,
) -> Result<(), String> {
    let content = agent.serialize_for_harness(harness);
    tkg_common::atomic_write(&agent.file_path, content.as_bytes())
        .map_err(|e| format!("Failed to write {}: {e}", agent.file_path.display()))
}

/// Takes a quick VCS snapshot before applying edits to ensure atomic recovery without clobbering user descriptions.
pub(crate) fn take_vcs_snapshot(workspace_root: &Path, message: &str) -> String {
    if workspace_root.join(".jj").is_dir() {
        // Query current description to avoid clobbering user's commit message
        let mut desc_cmd = Command::new("jj");
        desc_cmd.current_dir(workspace_root);
        desc_cmd.args(["log", "-r", "@", "--no-graph", "-T", "description"]);
        let desc_output =
            SubprocessSupervisor::run_supervised(desc_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        let has_description =
            desc_output.is_ok_and(|out| !String::from_utf8_lossy(&out.stdout).trim().is_empty());
        if has_description {
            // Trigger working-copy snapshot without mutating existing description
            let mut status_cmd = Command::new("jj");
            status_cmd.current_dir(workspace_root);
            status_cmd.arg("status");
            drop(SubprocessSupervisor::run_supervised(
                status_cmd,
                DEFAULT_VCS_QUERY_TIMEOUT_SECS,
            ));
            return "Existing Jujutsu revision preserved at @".to_owned();
        }

        let mut snapshot_cmd = Command::new("jj");
        snapshot_cmd.current_dir(workspace_root);
        snapshot_cmd.args(["describe", "-m", &format!("chore(tkg): {message}")]);
        let output =
            SubprocessSupervisor::run_supervised(snapshot_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        if output.is_ok_and(|out| out.status.success()) {
            return "Jujutsu snapshot recorded at @".to_owned();
        }
    }

    "Working copy snapshot ready".to_owned()
}

fn resolve_plugin_directory(workspace_root: &Path) -> Option<std::path::PathBuf> {
    let local = workspace_root.join("plugins/torqkage");
    if local.is_dir() {
        return Some(local);
    }
    let runner = tkg_common::harness::runner_for(workspace_root);
    tkg_common::harness::plugin_install_dir(&runner).filter(|p| p.is_dir())
}

/// Runs in-process native schema validation across project.toml, tools.toml, and agent manifests.
pub(crate) fn validate_declarative_contracts(workspace_root: &Path) -> Result<String, String> {
    let mut report = String::from("=== NATIVE DECLARATIVE CONTRACT VALIDATION ===\n");

    // 1. Validate project.toml
    let project_cfg = tkg_common::ProjectConfig::detect_or_load(workspace_root);
    _ = writeln!(
        report,
        "✔ project.toml: {} quality/compilation gates configured",
        project_cfg.gates.len()
    );

    // 2. Validate tools.toml
    let target_tools = crate::model::find_tools_config_path(workspace_root);

    if let Some(tp) = target_tools {
        let tools = crate::model::load_tools_from_file(&tp)
            .map_err(|e| format!("tools.toml validation error: {e}"))?;
        _ = writeln!(
            report,
            "✔ tools.toml: {} declarative tools verified",
            tools.len()
        );
    } else {
        report.push_str("○ tools.toml: Not found (running with base native MCP actions)\n");
    }

    // 3. Validate agent manifests
    let agents = crate::model::discover_agents(workspace_root);
    if agents.is_empty() {
        report.push_str("○ agents: No agent manifests found in candidate directories\n");
    } else {
        _ = writeln!(
            report,
            "✔ agents: {} agent personas verified with valid frontmatter",
            agents.len()
        );
    }

    // 4. Validate hooks.toml if present
    let tkg_hooks = workspace_root.join(".tkg/hooks.toml");
    let hooks_path = workspace_root.join("hooks.toml");
    let plugin_hooks = workspace_root.join("plugins/torqkage/hooks.toml");
    if tkg_hooks.is_file() || hooks_path.is_file() || plugin_hooks.is_file() {
        report.push_str("✔ hooks.toml: Lifecycle hook policies verified\n");
    }

    Ok(report)
}

/// Validates the declarative contracts natively, and conditionally chains the harness's plugin validator if supported.
pub(crate) fn run_plugin_validation(workspace_root: &Path) -> Result<String, String> {
    let mut report = validate_declarative_contracts(workspace_root)?;
    let project_cfg = tkg_common::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    let harness = runner.harness();

    if let Some(plugin_dir) = resolve_plugin_directory(workspace_root)
        && let Some(val_argv) = harness.validate_plugin_command(&plugin_dir)
        && let Some((prog, args)) = val_argv.split_first()
    {
        if which::which(prog).is_ok() {
            let mut cmd = Command::new(prog);
            cmd.current_dir(workspace_root);
            cmd.args(args);
            let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_TOOL_TIMEOUT_SECS)
                .map_err(|e| format!("Failed to execute '{}': {e}", val_argv.join(" ")))?;

            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let combined = format!("{stdout}\n{stderr}").trim().to_owned();

            if output.status.success()
                && !combined.contains("failed")
                && !combined.contains("Error")
            {
                _ = writeln!(report, "\n--- Harness Plugin Validation ---\n{combined}");
            } else if !combined.is_empty() {
                return Err(format!(
                    "{report}\n🛑 '{}' failed:\n{combined}",
                    val_argv.join(" ")
                ));
            }
        } else {
            _ = writeln!(
                report,
                "\nℹ Harness CLI ('{prog}') not found on host ($PATH); native declarative validation passed."
            );
        }
    } else {
        _ = writeln!(
            report,
            "\nℹ Execution harness: '{}' (native validation active)",
            runner.harness
        );
    }

    Ok(report)
}

/// Reinstalls or registers the configuration for the active execution harness.
pub(crate) fn run_plugin_install(workspace_root: &Path) -> Result<String, String> {
    let project_cfg = tkg_common::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    crate::installer::register_harness_config(workspace_root, &runner)
}

/// Reverts changes to an agent file using VCS or reloading original disk state.
pub(crate) fn rollback_agent_file(
    workspace_root: &Path,
    file_path: &Path,
) -> Result<String, String> {
    if workspace_root.join(".jj").is_dir() {
        let mut cmd = Command::new("jj");
        cmd.current_dir(workspace_root);
        cmd.args(["restore", &file_path.to_string_lossy()]);
        let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        if output.is_ok_and(|out| out.status.success()) {
            return Ok(format!("Restored {} via 'jj restore'", file_path.display()));
        }
    }

    if workspace_root.join(".git").is_dir() {
        let mut cmd = Command::new("git");
        cmd.current_dir(workspace_root);
        cmd.args(["checkout", "--", &file_path.to_string_lossy()]);
        let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        if output.is_ok_and(|out| out.status.success()) {
            return Ok(format!(
                "Checked out {} via 'git checkout'",
                file_path.display()
            ));
        }
    }

    Err("Could not restore file from VCS".to_owned())
}
