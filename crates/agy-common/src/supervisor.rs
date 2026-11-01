//! Process hygiene and Linux orphan prevention.

use std::process::{Child, Command, ExitStatus};
use crate::error::{AgyError, Result};

/// Drop guard ensuring child process groups are forcefully terminated if parent drops.
pub struct ChildGuard {
    child: Child,
    pgid: i32,
}

impl ChildGuard {
    /// Constructs a new `ChildGuard` wrapping the spawned process and its process group.
    #[must_use]
    pub const fn new(child: Child, pgid: i32) -> Self {
        Self { child, pgid }
    }

    /// Returns the process identifier.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Waits for the child process to complete.
    ///
    /// # Errors
    /// Returns an I/O error if the wait call fails.
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child.wait()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        // If child is still running, kill entire process group with SIGKILL
        if matches!(self.child.try_wait(), Ok(None)) {
            let target_pgid = self.pgid.saturating_neg();
            // SAFETY: Sending SIGKILL to negative process group id terminates the scoped process group
            unsafe {
                libc::kill(target_pgid, libc::SIGKILL);
            }
            if self.child.wait().is_ok() {}
        }
    }
}

#[cfg(target_os = "linux")]
fn configure_child_process_group() -> std::io::Result<()> {
    // SAFETY: Setting PR_SET_PDEATHSIG ensures SIGTERM on parent termination
    let prctl_status = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) };
    if prctl_status != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // SAFETY: Placing child in a new process group matching its PID
    let setpgid_status = unsafe { libc::setpgid(0, 0) };
    if setpgid_status != 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

/// Subprocess supervisor enforcing Linux process hygiene (`PR_SET_PDEATHSIG` + `setpgid`).
pub struct SubprocessSupervisor;

impl SubprocessSupervisor {
    /// Spawns a command configured with `PR_SET_PDEATHSIG` and its own process group.
    ///
    /// # Errors
    /// Returns `AgyError::Io` if spawning the process fails.
    pub fn spawn_scoped(mut command: Command) -> Result<ChildGuard> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: `pre_exec` registers the single post-fork initialization routine
            unsafe {
                command.pre_exec(configure_child_process_group);
            }
        }

        let child = command.spawn().map_err(AgyError::Io)?;
        let pgid = i32::try_from(child.id()).unwrap_or(0);

        Ok(ChildGuard::new(child, pgid))
    }

    /// Executes a command with process group supervision and an optional timeout deadline.
    ///
    /// # Errors
    /// Returns an error string if spawning fails or the command exceeds the timeout.
    pub fn run_supervised(
        mut command: Command,
        timeout_seconds: u64,
    ) -> std::result::Result<std::process::Output, String> {
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: `pre_exec` registers the single post-fork initialization routine
            unsafe {
                command.pre_exec(configure_child_process_group);
            }
        }

        let child = command
            .spawn()
            .map_err(|err| format!("Failed to spawn process: {err}"))?;

        let child_pid = child.id();
        let process_group_id = i32::try_from(child_pid).unwrap_or(0);

        if timeout_seconds > 0 {
            let timeout_dur = std::time::Duration::from_secs(timeout_seconds);
            let (tx, rx) = std::sync::mpsc::channel();

            let worker = std::thread::spawn(move || {
                let res = child.wait_with_output();
                drop(tx.send(res));
            });

            if let Ok(res) = rx.recv_timeout(timeout_dur) {
                drop(worker.join());
                res.map_err(|err| format!("Failed to read process output: {err}"))
            } else {
                // Timeout expired: kill process group with SIGKILL
                #[cfg(unix)]
                {
                    let target_pgid = process_group_id.saturating_neg();
                    // SAFETY: Sending SIGKILL to negative process group id terminates the scoped process group
                    unsafe {
                        libc::kill(target_pgid, libc::SIGKILL);
                    }
                    let raw_pid = i32::try_from(child_pid).unwrap_or(0);
                    // SAFETY: Sending SIGKILL directly to process id ensures termination
                    unsafe {
                        libc::kill(raw_pid, libc::SIGKILL);
                    }
                }
                drop(worker.join());
                Err(format!(
                    "Execution timed out after {timeout_seconds}s and was terminated."
                ))
            }
        } else {
            child
                .wait_with_output()
                .map_err(|err| format!("Failed to read process output: {err}"))
        }
    }
}
