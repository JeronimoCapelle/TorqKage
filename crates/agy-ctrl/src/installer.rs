//! # installer
//!
//! In-deck plugin installation, symlink management, and host system prerequisite diagnostics.

use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::{DEFAULT_TOOL_TIMEOUT_SECS, SubprocessSupervisor};

/// Prerequisite binary diagnostic report item.
#[derive(Debug, Clone)]
pub(crate) struct BinaryCheck {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
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
            let link_note = if self.is_symlink { " (live symlink)" } else { "" };
            out.push_str(&format!("✔ Plugin Status: Installed at {}{link_note}\n\n", self.plugin_path.display()));
        } else {
            out.push_str(&format!("✖ Plugin Status: NOT INSTALLED at {}\n  Press 'i' to install/link now.\n\n", self.plugin_path.display()));
        }

        // Host binaries
        out.push_str("--- Host Binary Prerequisites ---\n");
        for check in &self.binary_checks {
            let icon = if check.installed { "✔" } else if check.required { "✖" } else { "○" };
            let req_tag = if check.required { "[Required]" } else { "[Optional]" };
            out.push_str(&format!("{icon} {req_tag:<10} {:<16} — {}\n", check.name, check.description));
        }

        out
    }
}

/// Checks whether a binary is available in system `PATH`.
fn is_binary_available(name: &str) -> bool {
    let mut cmd = Command::new("which");
    cmd.arg(name);
    SubprocessSupervisor::run_supervised(cmd, 3).is_ok_and(|out| out.status.success())
}

/// Runs a diagnostic scan of the host environment and plugin install path.
#[must_use]
pub(crate) fn run_diagnostics(_workspace_root: &Path) -> DiagnosticsReport {
    let plugin_path = std::env::var("HOME")
        .map_or_else(|_| PathBuf::from(".gemini/config/plugins/agy-suite"), |h| {
            PathBuf::from(h).join(".gemini/config/plugins/agy-suite")
        });

    let plugin_installed = plugin_path.exists();
    let is_symlink = plugin_path.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink());

    let binary_checks = vec![
        BinaryCheck {
            name: "agy",
            description: "Antigravity CLI harness for agent execution",
            required: true,
            installed: is_binary_available("agy"),
        },
        BinaryCheck {
            name: "jj",
            description: "Jujutsu VCS for isolated subagent workspaces",
            required: true,
            installed: is_binary_available("jj"),
        },
        BinaryCheck {
            name: "bwrap",
            description: "Bubblewrap unprivileged Linux container sandbox",
            required: false,
            installed: is_binary_available("bwrap"),
        },
        BinaryCheck {
            name: "cargo",
            description: "Rust package manager & build system",
            required: true,
            installed: is_binary_available("cargo"),
        },
        BinaryCheck {
            name: "rustc",
            description: "Rust compiler",
            required: true,
            installed: is_binary_available("rustc"),
        },
        BinaryCheck {
            name: "paru",
            description: "Arch Linux AUR & package helper",
            required: false,
            installed: is_binary_available("paru"),
        },
        BinaryCheck {
            name: "cargo-binstall",
            description: "Fast binary crate installer",
            required: false,
            installed: is_binary_available("cargo-binstall"),
        },
        BinaryCheck {
            name: "cargo-nextest",
            description: "Parallel test harness runner",
            required: false,
            installed: is_binary_available("cargo-nextest"),
        },
        BinaryCheck {
            name: "cargo-llvm-cov",
            description: "Source-based code coverage generator",
            required: false,
            installed: is_binary_available("cargo-llvm-cov"),
        },
        BinaryCheck {
            name: "cargo-mutants",
            description: "Mutation testing framework",
            required: false,
            installed: is_binary_available("cargo-mutants"),
        },
    ];

    DiagnosticsReport {
        plugin_installed,
        plugin_path,
        is_symlink,
        binary_checks,
    }
}

/// Installs or symlinks the `plugins/agy-suite` directory into `~/.gemini/config/plugins/agy-suite`.
///
/// # Errors
/// Returns an error string if directory creation or linking fails.
pub(crate) fn install_plugin_interactive(workspace_root: &Path) -> Result<String, String> {
    let local_plugin = workspace_root.join("plugins/agy-suite");
    if !local_plugin.is_dir() {
        return Err(format!(
            "Source plugin directory not found at: {}",
            local_plugin.display()
        ));
    }

    let home = std::env::var("HOME").map_err(|e| format!("HOME environment variable error: {e}"))?;
    let target_dir = PathBuf::from(home).join(".gemini/config/plugins");
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create {}: {e}", target_dir.display()))?;

    let dest = target_dir.join("agy-suite");

    // Remove existing target if it's already a symlink or directory
    if dest.symlink_metadata().is_ok() {
        if dest.is_dir() && !dest.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
            std::fs::remove_dir_all(&dest)
                .map_err(|e| format!("Failed to remove old plugin dir: {e}"))?;
        } else {
            std::fs::remove_file(&dest)
                .map_err(|e| format!("Failed to remove existing symlink: {e}"))?;
        }
    }

    // Create relative or absolute symlink
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&local_plugin, &dest)
            .map_err(|e| format!("Failed to symlink plugin: {e}"))?;
    }

    // Run agy plugin validate
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
        "✔ Plugin successfully linked to: {}\n  Target: {}\n  Status: {val_status}",
        dest.display(),
        local_plugin.display()
    ))
}
