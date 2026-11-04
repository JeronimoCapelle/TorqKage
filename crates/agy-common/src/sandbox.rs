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
    is_inside_outer_sandbox_with(|var| std::env::var(var).is_ok())
}

/// Checks if the current process is already executing inside an outer sandbox or container
/// using a custom environment variable lookup closure.
#[must_use]
#[allow(
    clippy::module_name_repetitions,
    reason = "Descriptive function name clarifying outer sandbox detection"
)]
pub fn is_inside_outer_sandbox_with<F>(env_lookup: F) -> bool
where
    F: Fn(&str) -> bool,
{
    // 1. Explicit environment variable set by agy-ctrl or outer orchestrator
    if env_lookup("AGY_OUTER_SANDBOX")
        || env_lookup("SANDBOX")
        || env_lookup("CONTAINER")
    {
        return true;
    }

    // 2. Container filesystem markers
    if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() {
        return true;
    }

    // 3. Container signatures in /proc/1/cgroup or /proc/self/cgroup
    for cgroup_path in &["/proc/1/cgroup", "/proc/self/cgroup"] {
        if let Ok(cgroup) = std::fs::read_to_string(cgroup_path) {
            let lower = cgroup.to_ascii_lowercase();
            if lower.contains("docker")
                || lower.contains("containerd")
                || lower.contains("kubepods")
                || lower.contains("podman")
                || lower.contains("lxc")
                || lower.contains("libpod")
                || lower.contains("sandbox")
            {
                return true;
            }
        }
    }

    // 4. Nested PID namespace: if NSpid has > 1 entries, process is in an unshared PID namespace (e.g. bwrap --unshare-pid)
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("NSpid:")
                && rest.split_whitespace().count() > 1
            {
                return true;
            }
        }
    }

    // 5. Container-specific init processes (e.g. tini, dumb-init) or non-standard init in isolated environments
    if let Ok(comm) = std::fs::read_to_string("/proc/1/comm") {
        let comm_trimmed = comm.trim();
        let known_host_inits = [
            "systemd",
            "init",
            "openrc-init",
            "runit",
            "dinit",
            "s6-svscan",
            "shepherd",
            "busybox",
        ];
        if !comm_trimmed.is_empty()
            && !known_host_inits.contains(&comm_trimmed)
            && (comm_trimmed == "tini" || comm_trimmed == "dumb-init" || comm_trimmed.contains("bwrap"))
        {
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

    // If workspace_root is an isolated subagent workspace (e.g. .agy/workspaces/<agent>),
    // mount the repo root and shared target cache so symlinks resolve correctly in bwrap
    let repo_root = crate::resolver::WorkspaceResolver::find_root(Some(workspace_root))
        .unwrap_or_else(|_| workspace_root.to_path_buf());

    if repo_root != workspace_root && repo_root.exists() {
        cmd.args(["--ro-bind"]);
        cmd.arg(&repo_root);
        cmd.arg(&repo_root);

        let shared_target = repo_root.join("target");
        if shared_target.is_dir() {
            cmd.args(["--bind"]);
            cmd.arg(&shared_target);
            cmd.arg(&shared_target);
        }
    }

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
        assert!(is_inside_outer_sandbox_with(|var| var == "AGY_OUTER_SANDBOX"));
        assert!(is_inside_outer_sandbox_with(|var| var == "SANDBOX"));
        assert!(is_inside_outer_sandbox_with(|var| var == "CONTAINER"));
    }

    #[test]
    fn test_build_bubblewrap_command_structure() {
        let root = Path::new("/tmp");
        let cmd = build_bubblewrap_command("python3", &[String::from("-c")], &[String::from("print(1)")], root);
        let program = cmd.get_program().to_string_lossy();
        assert_eq!(program, "bwrap");
    }
}
