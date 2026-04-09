//! Child process management for hot reload.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Manages a child process lifecycle (start, kill, restart).
pub struct ProcessManager {
    child: Option<Child>,
    binary_path: PathBuf,
    env_vars: HashMap<String, String>,
}

impl ProcessManager {
    /// Creates a new process manager for the given binary.
    pub fn new(binary_path: PathBuf, env_vars: HashMap<String, String>) -> Self {
        Self {
            child: None,
            binary_path,
            env_vars,
        }
    }

    /// Starts the child process, inheriting stdin/stdout/stderr.
    pub fn start(&mut self) -> anyhow::Result<()> {
        let child = Command::new(&self.binary_path)
            .envs(&self.env_vars)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!("Failed to start {}: {}", self.binary_path.display(), e)
            })?;

        self.child = Some(child);
        Ok(())
    }

    /// Kills the child process. Waits briefly for exit, then sends SIGKILL.
    pub fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
            self.child = None;
        }
        Ok(())
    }

    /// Kills then restarts the child process.
    pub fn restart(&mut self) -> anyhow::Result<()> {
        self.kill()?;
        self.start()
    }

    /// Returns `true` if the child process is still running.
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    /// Waits for the child to exit and returns its exit code.
    pub fn wait(&mut self) -> anyhow::Result<i32> {
        if let Some(ref mut child) = self.child {
            let status = child.wait()?;
            let code = status.code().unwrap_or(1);
            self.child = None;
            Ok(code)
        } else {
            Ok(0)
        }
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}
