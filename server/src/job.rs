use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{Notify, OnceCell};

use common::job_status::Completed;
use common::output_event::Stream as OutputStream;
use common::*;

use crate::client_cert::ClientName;
use crate::output_stream::OutputHandler;

/// Map status of a completed process to `JobStatus`
fn completed_status(status: ExitStatus) -> JobStatus {
    if let Some(value) = status.code() {
        JobStatus {
            completed: Some(Completed::StatusCode(value)),
        }
    } else if let Some(value) = status.signal() {
        JobStatus {
            completed: Some(Completed::Signal(value)),
        }
    } else {
        panic!("Unknown process exit state")
    }
}

/// A single running job, i.e. a process
pub struct Job {
    pub owner: ClientName,
    status: Arc<OnceCell<ExitStatus>>,
    kill_request: Arc<Notify>,
    pub stdout: Arc<OutputHandler>,
    pub stderr: Arc<OutputHandler>,
}
impl Job {
    pub fn spawn(owner: ClientName, req: JobStartRequest) -> Result<Self, String> {
        let mut cmd = Command::new(req.path);

        cmd.args(req.args);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // TODO: use pre_exec to configure cgroups and namespaces

        let mut child = cmd.spawn().map_err(|e| format!("{:?}", e))?;

        let stdout = OutputHandler::setup(OutputStream::Stdout, child.stdout.take().unwrap());
        let stderr = OutputHandler::setup(OutputStream::Stderr, child.stderr.take().unwrap());

        let status = Arc::new(OnceCell::new());
        let kill_request = Arc::new(Notify::new());

        // State management task
        let status_handle = status.clone();
        let kill_requested = kill_request.clone();
        tokio::spawn(async move {
            tokio::select! {
                wait_result = child.wait() => {
                    // Process completed
                    log::debug!("Process completed {:?}", wait_result);
                    let _ = status_handle.set(wait_result.expect("Unknown process exit state"));
                },
                _ = kill_requested.notified() => {
                    // Kill the process
                    log::debug!("Killing job");
                    child.kill().await.expect("kill failed");
                    let wait_result = child.wait().await.expect("wait failed");
                    // Process completed
                    log::debug!("Job killed {:?}", wait_result);
                    let _ = status_handle.set(wait_result);
                }
            }
        });

        Ok(Self {
            owner,
            status,
            kill_request,
            stdout,
            stderr,
        })
    }

    /// Start an asynchronous kill operation
    pub fn start_kill(&mut self) {
        self.kill_request.notify_waiters();
    }

    pub fn status(&mut self) -> JobStatus {
        match self.status.get() {
            Some(status) => completed_status(*status),
            None => JobStatus { completed: None },
        }
    }
}
