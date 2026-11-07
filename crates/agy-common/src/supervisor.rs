//! Process hygiene and Linux orphan prevention.

use std::process::{Command, ExitStatus};

use command_group::{CommandGroup, GroupChild};

use crate::{
    error::{AgyError, Result},
    limits::PROCESS_KILL_GRACE_WAIT,
};

/// Drop guard ensuring child process groups are forcefully terminated if parent drops.
pub struct ChildGuard {
    child: GroupChild,
}

impl ChildGuard {
    /// Constructs a new `ChildGuard` wrapping the spawned process group.
    #[must_use]
    pub const fn new(child: GroupChild) -> Self {
        Self { child }
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
        if matches!(self.child.try_wait(), Ok(None)) {
            drop(self.child.kill());
            drop(self.child.wait());
        }
    }
}

/// Subprocess supervisor enforcing process hygiene and process group termination.
pub struct SubprocessSupervisor;

impl SubprocessSupervisor {
    /// Spawns a command configured in its own isolated process group.
    ///
    /// # Errors
    /// Returns `AgyError::Io` if spawning the process fails.
    pub fn spawn_scoped(mut command: Command) -> Result<ChildGuard> {
        let child = command.group_spawn().map_err(AgyError::Io)?;
        Ok(ChildGuard::new(child))
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

        let child = command
            .group_spawn()
            .map_err(|err| format!("Failed to spawn process: {err}"))?;

        let child_pid = child.id();

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
                // Timeout expired: kill process group with SIGKILL via safe nix wrapper
                #[cfg(unix)]
                {
                    if let Ok(pgid) = i32::try_from(child_pid)
                        && pgid > 0
                    {
                        let pid = nix::unistd::Pid::from_raw(pgid);
                        let _killpg_res =
                            nix::sys::signal::killpg(pid, nix::sys::signal::Signal::SIGKILL);
                        let _kill_res =
                            nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL);
                    }
                }
                #[cfg(not(unix))]
                {
                    // Fallback for non-unix targets
                    let _ = child_pid;
                }

                // Allow grace duration for worker to unblock post-kill; avoid indefinite deadlock
                if rx.recv_timeout(PROCESS_KILL_GRACE_WAIT).is_ok() {
                    drop(worker.join());
                }
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
