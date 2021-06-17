use std::fmt;

mod job_id;

pub use job_id::{InvalidJobIdBytes, InvalidJobIdString, JobId};

tonic::include_proto!("common");

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use job_status::Completed;
        if let Some(result) = &self.completed {
            match result {
                Completed::StatusCode(code) => write!(f, "Completed({})", code),
                Completed::Signal(signal) => write!(f, "Signal({})", signal),
            }
        } else {
            write!(f, "Running")
        }
    }
}
