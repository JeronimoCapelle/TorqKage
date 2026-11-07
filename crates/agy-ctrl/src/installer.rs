//! # installer
//!
//! In-deck plugin installation, directory copying, and host system prerequisite diagnostics.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use agy_common::{DEFAULT_TOOL_TIMEOUT_SECS, FastHashSet, SubprocessSupervisor};

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
        out.push_str("=== AGY-SUITE SYSTEM & ENVIRONMENT DIAGNOSTICS ===\n\n");

        // Plugin status
        if self.plugin_installed {
            let link_note = if self.is_symlink {
                " (symlinked dev bundle - press 'i' to copy standalone bundle)"
            } else {
                " (standalone copy)"
            };
            out.push_str(&format!(
                "✔ Plugin Status: Installed at {}{link_note}\n\n",
                self.plugin_path.display()
            ));
        } else {
            out.push_str(&format!(
                "✖ Plugin Status: NOT INSTALLED at {}\n  Press 'i' to copy and install now.\n\n",
                self.plugin_path.display()
            ));
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
            out.push_str(&format!(
                "{icon} {req_tag:<10} {:<16} — {}\n",
                check.name, check.description
            ));
        }

        out
    }
}

/// Checks whether a binary is available in system `PATH`.
pub(crate) fn is_binary_available(name: &str) -> bool {
    which::which(name).is_ok()
}

/// Runs a diagnostic scan of the host environment, plugin install path,
/// and dynamically validates all binaries declared across tools.toml and project.toml.
#[must_use]
pub(crate) fn run_diagnostics(workspace_root: &Path) -> DiagnosticsReport {
    let plugin_path = std::env::var("HOME").map_or_else(
        |_| PathBuf::from(".gemini/config/plugins/agy-suite"),
        |h| PathBuf::from(h).join(".gemini/config/plugins/agy-suite"),
    );

    let plugin_installed = plugin_path.exists();
    let is_symlink = plugin_path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink());

    let mut binary_checks = vec![
        BinaryCheck {
            name: "agy".to_owned(),
            description: "Antigravity CLI harness for agent execution".to_owned(),
            required: true,
            installed: is_binary_available("agy"),
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
    checked_names.insert("agy".to_owned());
    checked_names.insert("jj".to_owned());
    checked_names.insert("git".to_owned());
    checked_names.insert("bwrap".to_owned());

    // Dynamically check tools declared in tools.toml
    if let Ok(tools) = load_tools_from_file(workspace_root) {
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
    let project = agy_common::project::ProjectConfig::detect_or_load(workspace_root);
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

/// Discovers the local plugin source bundle either in `<workspace>/plugins/agy-suite`
/// or directly in `<workspace>` if executed inside an extracted plugin release bundle.
fn resolve_source_plugin(workspace_root: &Path) -> Result<PathBuf, String> {
    if workspace_root
        .join("plugins/agy-suite/plugin.json")
        .is_file()
    {
        Ok(workspace_root.join("plugins/agy-suite"))
    } else if workspace_root.join("plugin.json").is_file() {
        Ok(workspace_root.to_path_buf())
    } else if workspace_root.join("plugins/agy-suite").is_dir() {
        Ok(workspace_root.join("plugins/agy-suite"))
    } else {
        Err(format!(
            "Source plugin bundle not found at '{}' or '{}'",
            workspace_root.join("plugins/agy-suite").display(),
            workspace_root.display()
        ))
    }
}

/// Installs the plugin bundle by copying all declarative files, agents, and binaries
/// directly into `~/.gemini/config/plugins/agy-suite`.
///
/// # Errors
/// Returns an error string if source resolution, directory creation, or file copying fails.
pub(crate) fn install_plugin_interactive(workspace_root: &Path) -> Result<String, String> {
    let local_plugin = resolve_source_plugin(workspace_root)?;

    let home =
        std::env::var("HOME").map_err(|e| format!("HOME environment variable error: {e}"))?;
    let target_dir = PathBuf::from(home).join(".gemini/config/plugins");
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create {}: {e}", target_dir.display()))?;

    let dest = target_dir.join("agy-suite");

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
        for bin_name in ["agy-mcp", "agy-hooks", "agy-ctrl"] {
            let candidate_bin = target_profile.join(bin_name);
            let target_file = dest_bin.join(bin_name);
            if candidate_bin.is_file() && !target_file.is_file() {
                drop(std::fs::copy(&candidate_bin, &target_file));
            }
        }
    }

    #[cfg(unix)]
    ensure_binaries_executable(&dest_bin);

    // Run agy plugin validate if agy is installed
    let mut validate_cmd = Command::new("agy");
    validate_cmd.args(["plugin", "validate", &dest.to_string_lossy()]);
    validate_cmd.current_dir(workspace_root);
    let val_result = SubprocessSupervisor::run_supervised(validate_cmd, DEFAULT_TOOL_TIMEOUT_SECS);

    let val_status = match val_result {
        Ok(out) if out.status.success() => "✔ Validation passed",
        Ok(_) => "○ Installed (agy plugin validate returned warnings)",
        Err(_) => "○ Installed (agy CLI not found in PATH for pre-validation)",
    };

    Ok(format!(
        "✔ Plugin bundle successfully copied to: {}\n  Source: {}\n  Status: {val_status}",
        dest.display(),
        local_plugin.display()
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
        let temp_base = std::env::temp_dir().join(format!("agy_copy_test_{}", std::process::id()));
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
        let temp = std::env::temp_dir().join(format!("agy_src_test_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp));

        // Case 1: unzipped plugin root containing plugin.json directly
        drop(std::fs::create_dir_all(&temp));
        std::fs::write(temp.join("plugin.json"), "{}").expect("write plugin.json");
        let resolved = resolve_source_plugin(&temp).expect("resolve directly");
        assert_eq!(resolved, temp);

        // Case 2: repo workspace with plugins/agy-suite/plugin.json
        let temp_ws = std::env::temp_dir().join(format!("agy_ws_test_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp_ws));
        drop(std::fs::create_dir_all(temp_ws.join("plugins/agy-suite")));
        std::fs::write(temp_ws.join("plugins/agy-suite/plugin.json"), "{}").expect("write");
        let resolved_ws = resolve_source_plugin(&temp_ws).expect("resolve in workspace");
        assert_eq!(resolved_ws, temp_ws.join("plugins/agy-suite"));

        drop(std::fs::remove_dir_all(&temp));
        drop(std::fs::remove_dir_all(&temp_ws));
    }
}
