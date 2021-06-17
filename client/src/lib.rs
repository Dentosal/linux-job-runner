//! Client libarary for the job runner

use std::convert::TryInto;

use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};

use common::t_service_client::TServiceClient;
use common::*;

// Re-exports
pub use common::output_event::Stream as OutputStream;
pub use common::{JobId, JobStartRequest};

#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Contents of the PEM-formatted server root CA certificate file
    pub server_root_ca_crt: Vec<u8>,
    /// Contents of the PEM-formatted client certificate file
    pub client_crt: Vec<u8>,
    /// Contents of the PEM-formatted client private key file
    pub client_key: Vec<u8>,
}

// TODO: create a proper error type, and add conversions to it

type DResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct Client {
    client: TServiceClient<Channel>,
}
impl Client {
    /// Connect to a job server
    pub async fn connect<T: Into<String>>(target: T, tls: TlsConfig) -> DResult<Self> {
        let target: String = target.into();

        let server_root_ca_crt = Certificate::from_pem(tls.server_root_ca_crt);
        let client_identity = Identity::from_pem(tls.client_crt, tls.client_key);

        let tls = ClientTlsConfig::new()
            .domain_name("localhost")
            .ca_certificate(server_root_ca_crt)
            .identity(client_identity);

        log::debug!("Connecting to {}", target);
        let channel = Channel::from_shared(target)?
            .tls_config(tls)?
            .connect()
            .await?;
        log::debug!("Connected");

        Ok(Self {
            client: TServiceClient::new(channel),
        })
    }

    /// Starts a new job by spawning a process from given executable path and arguments.
    /// Returns a unique job id (UUID v4), that is used to specify the target job for other endpoints.
    // If the executable is not found or cannot be executed, immediately returns an error.
    pub async fn start(&mut self, req: JobStartRequest) -> DResult<JobId> {
        let command = tonic::Request::new(req);
        let response = self.client.start(command).await?;
        Ok(response
            .into_inner()
            .try_into()
            .expect("Server returned invalid JobId"))
    }

    /// Cancels a job.
    /// This is done by sending a `SIGKILL` to the underlying process.
    /// If the job has already terminated or stop has been called before, then this is a no-op.
    /// This is done asynchronously, and can return before the process has terminated.
    /// If you must wait until the job has stopped, do so by calling `wait`.
    pub async fn stop(&mut self, jobid: JobId) -> DResult<()> {
        self.client
            .stop(tonic::Request::new(TargetJobId {
                jobid: jobid.to_bytes(),
            }))
            .await?;

        Ok(())
    }

    /// Get job status, i.e. is it running, and the status code if the job has completed.
    /// If the job has been terminated with a signal, that is reported instead.
    pub async fn status(&mut self, jobid: JobId) -> DResult<JobStatus> {
        let response = self
            .client
            .status(tonic::Request::new(TargetJobId {
                jobid: jobid.to_bytes(),
            }))
            .await?;

        Ok(response.into_inner())
    }

    /// Wait until a job completes, the return it's status
    pub async fn wait(&mut self, jobid: JobId) -> DResult<JobStatus> {
        loop {
            let status = self.status(jobid).await?;
            if status.completed.is_some() {
                return Ok(status);
            }

            // TODO: make polling interval configurable, or even better,
            // implement status change notifications
            sleep(Duration::from_millis(500)).await;
        }
    }

    /// Stream output of a job to an mpsc queue.
    /// Stream is automatically closed when the process completes and all output has been streamed.
    /// All calls stream the whole output history from the moment the process was started.
    pub async fn output(
        &mut self,
        jobid: JobId,
        tx: Sender<Result<(OutputStream, Vec<u8>), tonic::Status>>,
    ) -> DResult<()> {
        let response = self
            .client
            .output(tonic::Request::new(TargetJobId {
                jobid: jobid.to_bytes(),
            }))
            .await?;

        let mut inner = response.into_inner();
        tokio::spawn(async move {
            loop {
                let r = match inner.message().await {
                    Ok(Some(msg)) => {
                        let event = OutputStream::from_i32(msg.stream).unwrap();
                        Ok((event, msg.output))
                    }
                    Ok(None) => break,
                    Err(err) => Err(err),
                };
                let is_err = r.is_err();
                tx.send(r).await.expect("Receiver has hung up");
                if is_err {
                    break;
                }
            }
        });
        Ok(())
    }
}
