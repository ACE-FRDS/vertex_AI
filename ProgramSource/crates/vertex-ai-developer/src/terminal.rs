use crate::{
    CommandExecution, CommandExecutionId, CommandStatus, DeveloperError, DeveloperMode,
    FileToolkit, RiskLevel,
};
use chrono::Utc;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    sync::{Mutex, watch},
    time::{Instant, timeout_at},
};
use uuid::Uuid;

const MAX_OUTPUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TIMEOUT_MS: u64 = 30 * 60 * 1000;

#[derive(Clone, Default)]
pub struct TerminalRunner {
    active: Arc<Mutex<BTreeMap<CommandExecutionId, watch::Sender<bool>>>>,
}

pub struct TerminalRequest<'a> {
    pub executable: &'a str,
    pub args: &'a [String],
    pub working_directory: &'a str,
    pub timeout_ms: u64,
    pub approved_high_risk: bool,
}

impl TerminalRunner {
    pub async fn execute(
        &self,
        toolkit: &FileToolkit,
        mode: DeveloperMode,
        request: TerminalRequest<'_>,
    ) -> Result<CommandExecution, DeveloperError> {
        validate_command(
            mode,
            request.executable,
            request.args,
            request.approved_high_risk,
        )?;
        let working_directory = toolkit.resolve_working_directory(request.working_directory)?;
        let timeout_ms = request.timeout_ms.clamp(1_000, MAX_TIMEOUT_MS);
        let id = Uuid::new_v4();
        let started_at = Utc::now();
        let mut command = Command::new(request.executable);
        command
            .args(request.args)
            .current_dir(&working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| DeveloperError::Terminal(error.to_string()))?;
        let process_id = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DeveloperError::Terminal("stdout pipe is unavailable".to_owned()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| DeveloperError::Terminal("stderr pipe is unavailable".to_owned()))?;
        let stdout_task = tokio::spawn(read_limited(stdout));
        let stderr_task = tokio::spawn(read_limited(stderr));
        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        self.active.lock().await.insert(id, cancel_tx);

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let (status, exit_code) = tokio::select! {
            status = timeout_at(deadline, child.wait()) => match status {
                Ok(Ok(exit)) if exit.success() => (CommandStatus::Completed, exit.code()),
                Ok(Ok(exit)) => (CommandStatus::Failed, exit.code()),
                Ok(Err(error)) => {
                    self.active.lock().await.remove(&id);
                    return Err(DeveloperError::Terminal(error.to_string()));
                }
                Err(_) => {
                    terminate_process_tree(&mut child, process_id).await;
                    (CommandStatus::Timeout, None)
                }
            },
            changed = cancel_rx.changed() => {
                if changed.is_ok() && *cancel_rx.borrow() {
                    terminate_process_tree(&mut child, process_id).await;
                    (CommandStatus::Cancelled, None)
                } else {
                    let exit = child.wait().await.map_err(|error| DeveloperError::Terminal(error.to_string()))?;
                    (if exit.success() { CommandStatus::Completed } else { CommandStatus::Failed }, exit.code())
                }
            }
        };
        self.active.lock().await.remove(&id);
        let stdout = join_reader(stdout_task).await;
        let stderr = join_reader(stderr_task).await;
        Ok(CommandExecution {
            id,
            executable: request.executable.to_owned(),
            args: request.args.to_vec(),
            working_directory: working_directory.to_string_lossy().into_owned(),
            process_id,
            started_at,
            finished_at: Some(Utc::now()),
            timeout_ms,
            exit_code,
            stdout,
            stderr,
            status,
        })
    }

    pub async fn cancel(&self, id: CommandExecutionId) -> Result<bool, DeveloperError> {
        let sender = self.active.lock().await.get(&id).cloned();
        match sender {
            Some(sender) => sender
                .send(true)
                .map(|_| true)
                .map_err(|_| DeveloperError::Terminal("command worker is unavailable".to_owned())),
            None => Ok(false),
        }
    }

    pub async fn cancel_all(&self) -> usize {
        let senders = self
            .active
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut cancelled = 0;
        for sender in senders {
            if sender.send(true).is_ok() {
                cancelled += 1;
            }
        }
        cancelled
    }
}

fn validate_command(
    mode: DeveloperMode,
    executable: &str,
    args: &[String],
    approved_high_risk: bool,
) -> Result<(), DeveloperError> {
    if !matches!(mode, DeveloperMode::Execute | DeveloperMode::Auto) {
        return Err(DeveloperError::Permission(
            "terminal execution requires EXECUTE or AUTO mode".to_owned(),
        ));
    }
    if executable.trim().is_empty()
        || Path::new(executable).components().count() != 1
        || executable.contains(['/', '\\'])
    {
        return Err(DeveloperError::Permission(
            "terminal executable must be an allowlisted basename".to_owned(),
        ));
    }
    let executable = executable.trim_end_matches(".exe").to_ascii_lowercase();
    let allowed = BTreeSet::from([
        "cargo",
        "rustc",
        "pnpm",
        "npm",
        "node",
        "git",
        "dotnet",
        "python",
        "pytest",
        "powershell",
        "pwsh",
    ]);
    if !allowed.contains(executable.as_str()) {
        return Err(DeveloperError::Permission(format!(
            "command is not allowlisted: {executable}"
        )));
    }
    if args.iter().any(|value| value.contains('\0')) {
        return Err(DeveloperError::Invalid(
            "command argument contains NUL".to_owned(),
        ));
    }
    let lower_args = args
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let joined = lower_args.join(" ");
    let forbidden = [
        "--force",
        "-rf",
        "remove-item",
        "format",
        "diskpart",
        "shutdown",
        "reg delete",
        "reset --hard",
        "clean -f",
        "checkout --",
        "push --force",
    ];
    if forbidden.iter().any(|pattern| joined.contains(pattern)) {
        return Err(DeveloperError::Permission(
            "destructive command arguments are forbidden".to_owned(),
        ));
    }
    if executable == "git" {
        let operation = lower_args.first().map(String::as_str).unwrap_or_default();
        if !matches!(operation, "status" | "diff" | "branch" | "log" | "show") {
            return Err(DeveloperError::Permission(
                "Phase 1 Git access is read-only".to_owned(),
            ));
        }
    }
    if (executable == "python" && lower_args.first().is_some_and(|value| value == "-c"))
        || (executable == "node" && lower_args.first().is_some_and(|value| value == "-e"))
        || matches!(lower_args.first().map(String::as_str), Some("exec" | "dlx"))
    {
        return Err(DeveloperError::Permission(
            "inline or package-execution commands are forbidden".to_owned(),
        ));
    }
    let risk = if matches!(executable.as_str(), "powershell" | "pwsh") {
        RiskLevel::High
    } else {
        RiskLevel::Low
    };
    if risk >= RiskLevel::High && !approved_high_risk {
        return Err(DeveloperError::Permission(
            "high-risk command requires explicit human approval".to_owned(),
        ));
    }
    if matches!(executable.as_str(), "powershell" | "pwsh") {
        let file_index = lower_args.iter().position(|value| value == "-file");
        if file_index.is_none() || lower_args.iter().any(|value| value == "-command") {
            return Err(DeveloperError::Permission(
                "PowerShell is limited to approved -File execution".to_owned(),
            ));
        }
    }
    Ok(())
}

async fn read_limited<R: tokio::io::AsyncRead + Unpin>(reader: R) -> String {
    let mut bytes = Vec::new();
    let _ = reader.take(MAX_OUTPUT_BYTES).read_to_end(&mut bytes).await;
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn join_reader(task: tokio::task::JoinHandle<String>) -> String {
    task.await
        .unwrap_or_else(|_| "[output reader failed]".to_owned())
}

async fn terminate_process_tree(child: &mut Child, process_id: Option<u32>) {
    #[cfg(target_os = "windows")]
    if let Some(process_id) = process_id {
        let _ = Command::new("taskkill.exe")
            .args(["/PID", &process_id.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;
    use std::fs;

    fn toolkit() -> (tempfile::TempDir, FileToolkit) {
        let temp = tempfile::tempdir().unwrap();
        let toolkit = FileToolkit::new(Workspace {
            id: Uuid::new_v4(),
            name: "terminal-test".to_owned(),
            root: fs::canonicalize(temp.path())
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            git_enabled: false,
            branch: None,
            registered_at: Utc::now(),
            last_opened_at: Utc::now(),
        })
        .unwrap();
        (temp, toolkit)
    }

    #[test]
    fn dangerous_and_inline_commands_are_rejected() {
        assert!(
            validate_command(
                DeveloperMode::Auto,
                "git",
                &["reset".to_owned(), "--hard".to_owned()],
                false
            )
            .is_err()
        );
        assert!(
            validate_command(
                DeveloperMode::Auto,
                "python",
                &["-c".to_owned(), "print('x')".to_owned()],
                false
            )
            .is_err()
        );
        assert!(
            validate_command(DeveloperMode::Auto, "cargo", &["test".to_owned()], false).is_ok()
        );
    }

    #[tokio::test]
    async fn captures_exit_code_and_output_without_shell_interpolation() {
        let (_temp, toolkit) = toolkit();
        let result = TerminalRunner::default()
            .execute(
                &toolkit,
                DeveloperMode::Execute,
                TerminalRequest {
                    executable: "rustc",
                    args: &["--version".to_owned()],
                    working_directory: ".",
                    timeout_ms: 30_000,
                    approved_high_risk: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.status, CommandStatus::Completed);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("rustc"));
    }
}
