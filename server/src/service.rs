use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Mutex;

use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use common::t_service_server::TService;
use common::*;

use crate::client_cert::ClientName;
use crate::job::Job;
use crate::output_stream;

type BoxStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send + Sync + 'static>>;

/// Enforce authentication, return client CN from the certificate
fn authenticate<T>(request: &Request<T>) -> Result<ClientName, Status> {
    match ClientName::from_request(&request) {
        Some(name) => {
            log::info!("Authenticated as {:?}", name);
            Ok(name)
        }
        None => {
            log::warn!("Client certificate missing");
            Err(Status::unauthenticated("Client certificate missing"))
        }
    }
}

/// Enforce authorization
fn verify_authorized(client_name: &ClientName, job: &Job) -> Result<(), Status> {
    if &job.owner != client_name {
        log::warn!(
            "Client {:?} tried to access a job without permission",
            client_name
        );
        return Err(Status::permission_denied("Job is owned by another user"));
    }
    Ok(())
}

pub struct TServiceImpl {
    state: Mutex<HashMap<JobId, Job>>,
}

impl TServiceImpl {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Access job by id.
    fn with_job<F, R>(&self, jobid: JobId, mut f: F) -> Result<R, Status>
    where
        F: FnMut(&mut Job) -> Result<R, Status>,
    {
        let mut jobs = self.state.lock().unwrap();
        if let Some(job) = jobs.get_mut(&jobid) {
            f(job)
        } else {
            Err(Status::not_found("No such job"))
        }
    }

    /// Access job by target id.
    /// Handles job id parsing and verifies authorization automatically.
    fn target_job<F, R>(
        &self,
        target_jobid: TargetJobId,
        client_name: &ClientName,
        mut f: F,
    ) -> Result<R, Status>
    where
        F: FnMut(&mut Job) -> Result<R, Status>,
    {
        if let Ok(jobid) = JobId::from_bytes(&target_jobid.jobid) {
            self.with_job(jobid, |job| {
                verify_authorized(&client_name, job)?;
                f(job)
            })
        } else {
            Err(Status::invalid_argument("JobId"))
        }
    }
}

impl Default for TServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl TService for TServiceImpl {
    /// Spawn a new job
    async fn start(
        &self,
        request: Request<JobStartRequest>,
    ) -> Result<Response<TargetJobId>, Status> {
        let client_name = authenticate(&request)?;

        match Job::spawn(client_name, request.into_inner()) {
            Ok(job) => {
                let jobid = JobId::new();
                let mut jobs = self.state.lock().unwrap();
                jobs.insert(jobid, job);
                Ok(Response::new(jobid.into()))
            }
            Err(msg) => Err(Status::failed_precondition(msg)),
        }
    }

    /// Starts killing the child process, but doesn't wait until it's actually stopped
    async fn stop(
        &self,
        request: Request<TargetJobId>,
    ) -> Result<Response<StopSignalSent>, Status> {
        let client_name = authenticate(&request)?;
        self.target_job(request.into_inner(), &client_name, |job| {
            job.start_kill();
            Ok(Response::new(StopSignalSent {}))
        })
    }

    /// Get status of a job
    async fn status(&self, request: Request<TargetJobId>) -> Result<Response<JobStatus>, Status> {
        let client_name = authenticate(&request)?;
        self.target_job(request.into_inner(), &client_name, |job| {
            Ok(Response::new(job.status()))
        })
    }

    type OutputStream = BoxStream<OutputEvent>;

    /// Stream output of a job
    async fn output(
        &self,
        request: Request<TargetJobId>,
    ) -> Result<Response<Self::OutputStream>, Status> {
        let client_name = authenticate(&request)?;

        let (tx, rx) = tokio::sync::mpsc::channel(2);

        self.target_job(request.into_inner(), &client_name, |job| {
            verify_authorized(&client_name, job)?;
            output_stream::stream_to(job.stdout.clone(), tx.clone());
            output_stream::stream_to(job.stderr.clone(), tx.clone());
            Ok(())
        })?;

        let s = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(s)))
    }
}
