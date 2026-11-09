//! # installer
//!
//! In-deck plugin installation, directory copying, and host system prerequisite diagnostics.

use std::{
    fmt::Write,
    path::{Path, PathBuf},
    process::Command,
};

use tkg_common::{DEFAULT_TOOL_TIMEOUT_SECS, FastHashSet, SubprocessSupervisor};

use crate::model::load_tools_from_file;

/// Prerequisite binary diagnostic report item.
#[derive(Debug, Clone)]
pub(crate) struct BinaryCheck {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) required: bool,
    pub(crate) installed: bool,
}

/// Full host system diagnostics report.
#[derive(Debug, Clone)]
pub(crate) struct DiagnosticsReport {
    pub(crate) plugin_installed: bool,
    pub(crate) plugin_path: PathBuf,
    pub(crate) is_symlink: bool,
    pub(crate) binary_checks: Vec<BinaryCheck>,
}

impl DiagnosticsReport {
    /// Formats the report into a human-readable multi-line summary string.
    #[must_use]
    pub(crate) fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str("=== TORQKAGE SYSTEM & ENVIRONMENT DIAGNOSTICS ===\n\n");

        // Plugin status
        if self.plugin_installed {
            let link_note = if self.is_symlink {
                " (symlinked dev bundle - press 'i' to copy standalone bundle)"
            } else {
                " (standalone copy)"
            };
            _ = write!(
                out,
                "✔ Plugin Status: Installed at {}{link_note}\n\n",
                self.plugin_path.display()
            );
        } else {
            _ = write!(
                out,
                "✖ Plugin Status: NOT INSTALLED at {}\n  Press 'i' to copy and install now.\n\n",
                self.plugin_path.display()
            );
        }

        // Host binaries
        out.push_str("--- Host Binary Prerequisites ---\n");
        for check in &self.binary_checks {
            let icon = if check.installed {
                "✔"
            } else if check.required {
                "✖"
            } else {
                "○"
            };
            let req_tag = if check.required {
                "[Required]"
            } else {
                "[Optional]"
            };
            _ = writeln!(
                out,
                "{icon} {req_tag:<10} {:<16} — {}",
                check.name, check.description
            );
        }

        out
    }
}

/// Checks whether a binary is available in system `PATH`.
pub(crate) fn is_binary_available(name: &str) -> bool {
    which::which(name).is_ok()
}

/// Behavioural contract for installing, registering, and checking configuration of an execution harness.
pub(crate) trait HarnessDriver: Send + Sync {
    /// Returns `(config_or_bundle_path, is_installed, is_symlink)` for diagnostics.
    fn config_status(
        &self,
        workspace_root: &Path,
        runner: &tkg_common::ProjectRunnerConfig,
    ) -> (PathBuf, bool, bool);

    /// Installs or registers the harness configuration for the given workspace.
    fn register_config(
        &self,
        workspace_root: &Path,
        runner: &tkg_common::ProjectRunnerConfig,
    ) -> Result<String, String>;

    /// Creates the session discovery provider for this harness.
    fn session_provider(&self) -> Box<dyn crate::session::SessionProvider>;
}

/// Driver for Google Antigravity (`agy`).
pub(crate) struct AntigravityDriver;

impl HarnessDriver for AntigravityDriver {
    fn config_status(
        &self,
        _workspace_root: &Path,
        runner: &tkg_common::ProjectRunnerConfig,
    ) -> (PathBuf, bool, bool) {
        let p = tkg_common::harness::plugin_install_dir(runner)
            .unwrap_or_else(|| PathBuf::from(".gemini/config/plugins/torqkage"));
        let exists = p.exists();
        let is_sym = p
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink());
        (p, exists, is_sym)
    }

    fn register_config(
        &self,
        workspace_root: &Path,
        runner: &tkg_common::ProjectRunnerConfig,
    ) -> Result<String, String> {
        install_plugin_interactive(workspace_root, runner)
    }

    fn session_provider(&self) -> Box<dyn crate::session::SessionProvider> {
        Box::new(crate::session::AntigravitySessionProvider)
    }
}

/// Driver for `OpenCode`.
pub(crate) struct OpenCodeDriver;

impl HarnessDriver for OpenCodeDriver {
    fn config_status(
        &self,
        workspace_root: &Path,
        _runner: &tkg_common::ProjectRunnerConfig,
    ) -> (PathBuf, bool, bool) {
        let p = workspace_root.join("opencode.json");
        let exists = p.is_file();
        (p, exists, false)
    }

    fn register_config(
        &self,
        workspace_root: &Path,
        _runner: &tkg_common::ProjectRunnerConfig,
    ) -> Result<String, String> {
        register_opencode_config(workspace_root)
    }

    fn session_provider(&self) -> Box<dyn crate::session::SessionProvider> {
        Box::new(crate::session::OpenCodeSessionProvider)
    }
}

/// Driver for Claude Code.
pub(crate) struct ClaudeDriver;

impl HarnessDriver for ClaudeDriver {
    fn config_status(
        &self,
        workspace_root: &Path,
        _runner: &tkg_common::ProjectRunnerConfig,
    ) -> (PathBuf, bool, bool) {
        let p = workspace_root.join("claude.json");
        let exists = p.is_file();
        (p, exists, false)
    }

    fn register_config(
        &self,
        workspace_root: &Path,
        _runner: &tkg_common::ProjectRunnerConfig,
    ) -> Result<String, String> {
        register_claude_config(workspace_root)
    }

    fn session_provider(&self) -> Box<dyn crate::session::SessionProvider> {
        // TODO(harness): Implement native ClaudeSessionProvider reading ~/.claude/projects/ JSONL transcripts.
        Box::new(crate::session::GenericSessionProvider)
    }
}

/// Driver for generic / custom CLI harnesses.
pub(crate) struct GenericDriver;

impl HarnessDriver for GenericDriver {
    fn config_status(
        &self,
        workspace_root: &Path,
        _runner: &tkg_common::ProjectRunnerConfig,
    ) -> (PathBuf, bool, bool) {
        let p = workspace_root.join(".tkg/state/mcp_servers.json");
        let exists = p.is_file();
        (p, exists, false)
    }

    fn register_config(
        &self,
        workspace_root: &Path,
        _runner: &tkg_common::ProjectRunnerConfig,
    ) -> Result<String, String> {
        register_custom_mcp_config(workspace_root)
    }

    fn session_provider(&self) -> Box<dyn crate::session::SessionProvider> {
        Box::new(crate::session::GenericSessionProvider)
    }
}

/// Resolves the [`HarnessDriver`] matching the configured runner.
#[must_use]
pub(crate) fn driver_for(runner: &tkg_common::ProjectRunnerConfig) -> Box<dyn HarnessDriver> {
    match runner.harness().id() {
        "agy" => Box::new(AntigravityDriver),
        "opencode" => Box::new(OpenCodeDriver),
        "claude" => Box::new(ClaudeDriver),
        _ => Box::new(GenericDriver),
    }
}

/// Runs a diagnostic scan of the host environment, plugin install path,
/// and dynamically validates all binaries declared across tools.toml and project.toml.
#[must_use]
pub(crate) fn run_diagnostics(workspace_root: &Path) -> DiagnosticsReport {
    let project_cfg = tkg_common::project::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    let harness_binary = runner.resolved_binary();

    let (plugin_path, plugin_installed, is_symlink) =
        driver_for(&runner).config_status(workspace_root, &runner);

    let mut binary_checks = vec![
        BinaryCheck {
            name: harness_binary.clone(),
            description: format!("{} CLI harness for agent execution", runner.harness),
            required: true,
            installed: is_binary_available(&harness_binary),
        },
        BinaryCheck {
            name: "jj".to_owned(),
            description: "Jujutsu VCS for isolated subagent workspaces".to_owned(),
            required: false,
            installed: is_binary_available("jj"),
        },
        BinaryCheck {
            name: "git".to_owned(),
            description: "Git version control system".to_owned(),
            required: false,
            installed: is_binary_available("git"),
        },
        BinaryCheck {
            name: "bwrap".to_owned(),
            description: "Bubblewrap unprivileged Linux container sandbox".to_owned(),
            required: false,
            installed: is_binary_available("bwrap"),
        },
    ];

    let mut checked_names = FastHashSet::default();
    checked_names.insert(harness_binary);
    checked_names.insert("jj".to_owned());
    checked_names.insert("git".to_owned());
    checked_names.insert("bwrap".to_owned());

    // Dynamically check tools declared in tools.toml
    if let Some(tools_path) = tkg_common::harness::find_tools_toml(workspace_root)
        && let Ok(tools) = load_tools_from_file(&tools_path)
    {
        for tool in &tools {
            if let Some(cmd) = &tool.command
                && let Some(bin) = cmd.first()
                && !bin.is_empty()
                && checked_names.insert(bin.clone())
            {
                let installed = is_binary_available(bin);
                binary_checks.push(BinaryCheck {
                    name: bin.clone(),
                    description: format!("Declared tool '{}' in tools.toml", tool.name),
                    required: false,
                    installed,
                });
            }
        }
    }

    // Dynamically check quality gate commands configured in project.toml
    let project = tkg_common::project::ProjectConfig::detect_or_load(workspace_root);
    for (gate_name, gate) in &project.gates {
        if let Some(bin) = gate.command.first()
            && !bin.is_empty()
            && checked_names.insert(bin.clone())
        {
            let installed = is_binary_available(bin);
            binary_checks.push(BinaryCheck {
                name: bin.clone(),
                description: format!("Project quality gate '[gates.{gate_name}]'"),
                required: true,
                installed,
            });
        }
    }

    DiagnosticsReport {
        plugin_installed,
        plugin_path,
        is_symlink,
        binary_checks,
    }
}

/// Recursively copies all directories and files from `src` to `dst`.
pub(crate) fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_child = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &dest_child)?;
        } else if path.is_file() {
            std::fs::copy(&path, &dest_child)?;
        }
    }
    Ok(())
}

/// Ensures binary executables in `dest_bin` have executable bits set on Unix.
#[cfg(unix)]
fn ensure_binaries_executable(dest_bin: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if dest_bin.is_dir()
        && let Ok(entries) = std::fs::read_dir(dest_bin)
    {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                drop(std::fs::set_permissions(entry.path(), perms));
            }
        }
    }
}

/// Discovers the local plugin source bundle either in `<workspace>/plugins/torqkage`
/// or directly in `<workspace>` if executed inside an extracted plugin release bundle.
fn resolve_source_plugin(workspace_root: &Path) -> Result<PathBuf, String> {
    if workspace_root
        .join("plugins/torqkage/plugin.json")
        .is_file()
    {
        Ok(workspace_root.join("plugins/torqkage"))
    } else if workspace_root.join("plugin.json").is_file() {
        Ok(workspace_root.to_path_buf())
    } else if workspace_root.join("plugins/torqkage").is_dir() {
        Ok(workspace_root.join("plugins/torqkage"))
    } else {
        Err(format!(
            "Source plugin bundle not found at '{}' or '{}'",
            workspace_root.join("plugins/torqkage").display(),
            workspace_root.display()
        ))
    }
}

/// Installs the plugin bundle by copying all declarative files, agents, and binaries
/// into the user-global plugin directory of the configured harness
/// (see [`tkg_common::harness::plugin_install_dir`]).
///
/// # Errors
/// Returns an error string if source resolution, directory creation, or file copying fails.
pub(crate) fn install_plugin_interactive(
    workspace_root: &Path,
    runner: &tkg_common::ProjectRunnerConfig,
) -> Result<String, String> {
    let local_plugin = resolve_source_plugin(workspace_root)?;

    let dest = tkg_common::harness::plugin_install_dir(runner)
        .ok_or_else(|| "HOME environment variable error: HOME is not set".to_owned())?;
    if let Some(target_dir) = dest.parent() {
        std::fs::create_dir_all(target_dir)
            .map_err(|e| format!("Failed to create {}: {e}", target_dir.display()))?;
    }

    // Remove existing target whether it's an old symlink or directory
    if let Ok(meta) = dest.symlink_metadata() {
        if meta.file_type().is_symlink() || !dest.is_dir() {
            std::fs::remove_file(&dest).map_err(|e| {
                format!(
                    "Failed to remove existing file or symlink {}: {e}",
                    dest.display()
                )
            })?;
        } else {
            std::fs::remove_dir_all(&dest).map_err(|e| {
                format!(
                    "Failed to remove old plugin directory {}: {e}",
                    dest.display()
                )
            })?;
        }
    }

    // Recursively copy all plugin files and subdirectories directly
    copy_dir_all(&local_plugin, &dest).map_err(|e| {
        format!(
            "Failed to copy plugin files from {} to {}: {e}",
            local_plugin.display(),
            dest.display()
        )
    })?;

    // Stage fresh compiled binaries if they exist in workspace target directories
    let dest_bin = dest.join("bin");
    drop(std::fs::create_dir_all(&dest_bin));
    for profile in ["release", "fast", "debug"] {
        let target_profile = workspace_root.join("target").join(profile);
        for bin_name in ["tkg-mcp", "tkg-hooks", "tkg"] {
            let candidate_bin = target_profile.join(bin_name);
            let target_file = dest_bin.join(bin_name);
            if candidate_bin.is_file() && !target_file.is_file() {
                drop(std::fs::copy(&candidate_bin, &target_file));
            }
        }
    }

    #[cfg(unix)]
    ensure_binaries_executable(&dest_bin);

    // Run harness plugin validation command if supported by the harness
    let val_status = if let Some(val_argv) = runner.harness().validate_plugin_command(&dest)
        && let Some((prog, args)) = val_argv.split_first()
    {
        let mut validate_cmd = Command::new(prog);
        validate_cmd.args(args);
        validate_cmd.current_dir(workspace_root);
        match SubprocessSupervisor::run_supervised(validate_cmd, DEFAULT_TOOL_TIMEOUT_SECS) {
            Ok(out) if out.status.success() => "✔ Validation passed",
            Ok(_) => "○ Installed (plugin validation returned warnings)",
            Err(_) => "○ Installed (harness CLI not found in PATH for pre-validation)",
        }
    } else {
        "✔ Installed"
    };

    Ok(format!(
        "✔ Plugin bundle successfully copied to: {}\n  Source: {}\n  Status: {val_status}",
        dest.display(),
        local_plugin.display()
    ))
}

/// Registers or installs the Torqkage configuration for the selected execution harness.
pub(crate) fn register_harness_config(
    workspace_root: &Path,
    runner: &tkg_common::ProjectRunnerConfig,
) -> Result<String, String> {
    driver_for(runner).register_config(workspace_root, runner)
}

/// Registers `tkg-mcp` in `opencode.json` for `OpenCode` integration.
pub(crate) fn register_opencode_config(workspace_root: &Path) -> Result<String, String> {
    let config_path = workspace_root.join("opencode.json");
    let mut config: serde_json::Value = if config_path.is_file() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read {}: {e}", config_path.display()))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({
            "$schema": "https://opencode.ai/schema.json"
        })
    };

    if !config.is_object() {
        config = serde_json::json!({});
    }

    if let Some(obj) = config.as_object_mut() {
        let servers = obj
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));
        if let Some(s_obj) = servers.as_object_mut() {
            s_obj.insert(
                "torqkage".to_owned(),
                serde_json::json!({
                    "command": "tkg-mcp",
                    "args": []
                }),
            );
        }
    }

    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize opencode.json: {e}"))?;
    std::fs::write(&config_path, serialized)
        .map_err(|e| format!("Failed to write {}: {e}", config_path.display()))?;

    Ok(format!(
        "✔ Registered 'tkg-mcp' in opencode.json at {}",
        config_path.display()
    ))
}

/// Registers `tkg-mcp` in `claude.json` for Claude Code integration.
pub(crate) fn register_claude_config(workspace_root: &Path) -> Result<String, String> {
    let config_path = workspace_root.join("claude.json");
    let mut config: serde_json::Value = if config_path.is_file() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read {}: {e}", config_path.display()))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !config.is_object() {
        config = serde_json::json!({});
    }

    if let Some(obj) = config.as_object_mut() {
        let servers = obj
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));
        if let Some(s_obj) = servers.as_object_mut() {
            s_obj.insert(
                "torqkage".to_owned(),
                serde_json::json!({
                    "command": "tkg-mcp",
                    "args": []
                }),
            );
        }
    }

    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize claude.json: {e}"))?;
    std::fs::write(&config_path, serialized)
        .map_err(|e| format!("Failed to write {}: {e}", config_path.display()))?;

    Ok(format!(
        "✔ Registered 'tkg-mcp' in claude.json at {}",
        config_path.display()
    ))
}

/// Generates a standalone MCP server configuration for custom harnesses.
pub(crate) fn register_custom_mcp_config(workspace_root: &Path) -> Result<String, String> {
    let state_dir = workspace_root.join(".tkg/state");
    std::fs::create_dir_all(&state_dir)
        .map_err(|e| format!("Failed to create {}: {e}", state_dir.display()))?;
    let config_path = state_dir.join("mcp_servers.json");
    let config = serde_json::json!({
        "mcpServers": {
            "torqkage": {
                "command": "tkg-mcp",
                "args": []
            }
        }
    });
    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize mcp_servers.json: {e}"))?;
    std::fs::write(&config_path, serialized)
        .map_err(|e| format!("Failed to write {}: {e}", config_path.display()))?;

    Ok(format!(
        "✔ Generated MCP server config at {}",
        config_path.display()
    ))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_dir_all_recursive() {
        let temp_base = std::env::temp_dir().join(format!("tkg_copy_test_{}", std::process::id()));
        let src = temp_base.join("src");
        let dst = temp_base.join("dst");
        drop(std::fs::remove_dir_all(&temp_base));

        drop(std::fs::create_dir_all(src.join("sub")));
        std::fs::write(src.join("a.txt"), "hello a").expect("write a");
        std::fs::write(src.join("sub/b.txt"), "hello b").expect("write b");

        copy_dir_all(&src, &dst).expect("copy_dir_all");

        assert!(dst.join("a.txt").is_file());
        assert!(dst.join("sub/b.txt").is_file());
        assert_eq!(
            std::fs::read_to_string(dst.join("sub/b.txt")).unwrap(),
            "hello b"
        );

        drop(std::fs::remove_dir_all(&temp_base));
    }

    #[test]
    fn test_resolve_source_plugin() {
        let temp = std::env::temp_dir().join(format!("tkg_src_test_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp));

        // Case 1: unzipped plugin root containing plugin.json directly
        drop(std::fs::create_dir_all(&temp));
        std::fs::write(temp.join("plugin.json"), "{}").expect("write plugin.json");
        let resolved = resolve_source_plugin(&temp).expect("resolve directly");
        assert_eq!(resolved, temp);

        // Case 2: repo workspace with plugins/torqkage/plugin.json
        let temp_ws = std::env::temp_dir().join(format!("tkg_ws_test_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp_ws));
        drop(std::fs::create_dir_all(temp_ws.join("plugins/torqkage")));
        std::fs::write(temp_ws.join("plugins/torqkage/plugin.json"), "{}").expect("write");
        let resolved_ws = resolve_source_plugin(&temp_ws).expect("resolve in workspace");
        assert_eq!(resolved_ws, temp_ws.join("plugins/torqkage"));

        drop(std::fs::remove_dir_all(&temp));
        drop(std::fs::remove_dir_all(&temp_ws));
    }

    #[test]
    fn test_register_harness_configs() {
        let temp_ws = std::env::temp_dir().join(format!("tkg_reg_test_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp_ws));
        drop(std::fs::create_dir_all(&temp_ws));

        // 1. OpenCode
        let opencode_res = register_opencode_config(&temp_ws);
        opencode_res.unwrap();
        let oc_file = temp_ws.join("opencode.json");
        assert!(oc_file.is_file());
        let oc_content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&oc_file).unwrap()).unwrap();
        assert_eq!(
            oc_content
                .pointer("/mcpServers/torqkage/command")
                .and_then(serde_json::Value::as_str),
            Some("tkg-mcp")
        );

        // 2. Claude
        let claude_res = register_claude_config(&temp_ws);
        claude_res.unwrap();
        let cl_file = temp_ws.join("claude.json");
        assert!(cl_file.is_file());
        let cl_content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cl_file).unwrap()).unwrap();
        assert_eq!(
            cl_content
                .pointer("/mcpServers/torqkage/command")
                .and_then(serde_json::Value::as_str),
            Some("tkg-mcp")
        );

        // 3. Custom MCP
        let custom_res = register_custom_mcp_config(&temp_ws);
        custom_res.unwrap();
        let custom_file = temp_ws.join(".tkg/state/mcp_servers.json");
        assert!(custom_file.is_file());
        let custom_content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&custom_file).unwrap()).unwrap();
        assert_eq!(
            custom_content
                .pointer("/mcpServers/torqkage/command")
                .and_then(serde_json::Value::as_str),
            Some("tkg-mcp")
        );

        drop(std::fs::remove_dir_all(&temp_ws));
    }
}
