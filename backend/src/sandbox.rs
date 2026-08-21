use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use tempfile::TempDir;
use thiserror::Error;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("workspace does not exist: {0}")]
    MissingWorkspace(PathBuf),
    #[error("workspace is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("workspace copy failed: {0}")]
    Copy(#[from] std::io::Error),
    #[error("command execution failed: {0}")]
    Execution(std::io::Error),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub shadow_path: PathBuf,
    pub synced: bool,
}

pub async fn execute_in_shadow(cmd: &str, cwd: &Path) -> Result<ExecutionResult, SandboxError> {
    let source = cwd
        .canonicalize()
        .map_err(|_| SandboxError::MissingWorkspace(cwd.to_path_buf()))?;
    if !source.is_dir() {
        return Err(SandboxError::NotDirectory(source));
    }
    let temp = TempDir::new().map_err(SandboxError::Copy)?;
    let shadow = temp.path().join("workspace");
    copy_directory(&source, &shadow)?;

    let mut command = shell_command(cmd);
    command
        .current_dir(&shadow)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.kill_on_drop(true);
    let output = match timeout(Duration::from_secs(60), command.output()).await {
        Ok(output) => output.map_err(SandboxError::Execution)?,
        Err(_) => {
            return Ok(ExecutionResult {
                success: false,
                exit_code: None,
                error: Some("Execution timed out".to_string()),
                stdout: String::new(),
                stderr: String::new(),
                shadow_path: shadow,
                synced: false,
            });
        }
    };
    let exit_code = output.status.code();
    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let synced = if success {
        sync_directory(&shadow, &source)?;
        true
    } else {
        false
    };
    let shadow_path = shadow.clone();
    Ok(ExecutionResult {
        success,
        exit_code,
        error: None,
        stdout,
        stderr,
        shadow_path,
        synced,
    })
}

fn shell_command(cmd: &str) -> Command {
    #[cfg(windows)]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", cmd]);
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command.args(["-c", cmd]);
        command
    }
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), SandboxError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path
            .file_name()
            .is_some_and(|name| name == ".git" || name == "node_modules" || name == "target")
        {
            continue;
        }
        if path.is_dir() {
            copy_directory(&path, &target)?;
        } else {
            fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn sync_directory(source: &Path, destination: &Path) -> Result<(), SandboxError> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            sync_directory(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}
