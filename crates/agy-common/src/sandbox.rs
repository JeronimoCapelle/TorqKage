//! # sandbox
//!
//! Declarative sandboxing detection, Bubblewrap jail creation, and security isolation primitives.

use std::path::Path;
use std::process::Command;

/// Checks if the current process is already executing inside an outer sandbox or container.
#[must_use]
#[allow(
    clippy::module_name_repetitions,
    reason = "Descriptive function name clarifying outer sandbox detection"
)]
pub fn is_inside_outer_sandbox() -> bool {
    // 1. Explicit environment variable set by agy-ctrl or outer orchestrator
    if std::env::var("AGY_OUTER_SANDBOX").is_ok()
        || std::env::var("SANDBOX").is_ok()
        || std::env::var("CONTAINER").is_ok()
    {
        return true;
    }

    // 2. Container filesystem markers
    if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() {
        return true;
    }

    // 3. Isolated PID namespace: inside bwrap (--unshare-pid), PID 1 is not systemd or init
    if let Ok(comm) = std::fs::read_to_string("/proc/1/comm") {
        let comm_trimmed = comm.trim();
        if !comm_trimmed.is_empty() && comm_trimmed != "systemd" && comm_trimmed != "init" {
            return true;
        }
    }

    false
}

/// Builds a Bubblewrap command wrapping the specified binary and arguments in an isolated jail.
#[must_use]
pub fn build_bubblewrap_command(
    binary: &str,
    base_args: &[String],
    dynamic_args: &[String],
    workspace_root: &Path,
) -> Command {
    let mut cmd = Command::new("bwrap");
    cmd.args([
        "--unshare-all",
        "--tmpfs", "/",
        "--dir", "/tmp",
        "--ro-bind", "/usr", "/usr",
    ]);

    if Path::new("/bin").is_symlink() {
        cmd.args(["--symlink", "usr/bin", "/bin"]);
    } else if Path::new("/bin").exists() {
        cmd.args(["--ro-bind", "/bin", "/bin"]);
    }

    if Path::new("/lib").is_symlink() {
        cmd.args(["--symlink", "usr/lib", "/lib"]);
    } else if Path::new("/lib").exists() {
        cmd.args(["--ro-bind", "/lib", "/lib"]);
    }

    if Path::new("/lib64").is_symlink() {
        cmd.args(["--symlink", "usr/lib", "/lib64"]);
    } else if Path::new("/lib64").exists() {
        cmd.args(["--ro-bind", "/lib64", "/lib64"]);
    }

    if Path::new("/sbin").is_symlink() {
        cmd.args(["--symlink", "usr/bin", "/sbin"]);
    } else if Path::new("/sbin").exists() {
        cmd.args(["--ro-bind", "/sbin", "/sbin"]);
    }

    if Path::new("/etc").exists() {
        cmd.args(["--ro-bind-try", "/etc", "/etc"]);
    }

    cmd.args(["--proc", "/proc", "--dev", "/dev"]);

    cmd.arg("--bind");
    cmd.arg(workspace_root);
    cmd.arg(workspace_root);

    cmd.arg("--chdir");
    cmd.arg(workspace_root);

    cmd.arg(binary);
    cmd.args(base_args);
    cmd.args(dynamic_args);

    cmd
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
    fn test_is_inside_outer_sandbox_env() {
        // SAFETY: Modifying test environment variable during isolated unit test.
        unsafe {
            std::env::set_var("AGY_OUTER_SANDBOX", "1");
        }
        assert!(is_inside_outer_sandbox());
        // SAFETY: Cleaning up test environment variable during isolated unit test.
        unsafe {
            std::env::remove_var("AGY_OUTER_SANDBOX");
        }
    }

    #[test]
    fn test_build_bubblewrap_command_structure() {
        let root = Path::new("/tmp");
        let cmd = build_bubblewrap_command("python3", &[String::from("-c")], &[String::from("print(1)")], root);
        let program = cmd.get_program().to_string_lossy();
        assert_eq!(program, "bwrap");
    }
}
