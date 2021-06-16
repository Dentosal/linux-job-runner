use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use tokio::process::{Child, Command};

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
    child: Child,
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

        Ok(Self {
            owner,
            child,
            stdout,
            stderr,
        })
    }

    /// Start an asynchronous kill operation
    pub fn start_kill(&mut self) {
        let _ = self.child.start_kill();
    }

    pub fn status(&mut self) -> JobStatus {
        match self
            .child
            .try_wait()
            .expect("Could not get status of a child process")
        {
            Some(status) => completed_status(status),
            None => JobStatus { completed: None },
        }
    }
}
