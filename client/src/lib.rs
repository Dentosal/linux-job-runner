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
    pub server_root_ca_crt: Vec<u8>,
    pub client_crt: Vec<u8>,
    pub client_key: Vec<u8>,
}

type DResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct Client {
    client: TServiceClient<Channel>,
}
impl Client {
    /// Connect to a job server
    pub async fn connect(target: String, tls: TlsConfig) -> DResult<Self> {
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

    /// Start a new job
    pub async fn start(&mut self, req: JobStartRequest) -> DResult<JobId> {
        let command = tonic::Request::new(req);
        let response = self.client.start(command).await?;
        Ok(response
            .into_inner()
            .try_into()
            .expect("Server returned invalid JobId"))
    }

    /// Stop a job (non-blocking, use `wait` afterwards if needed)
    pub async fn stop(&mut self, jobid: JobId) -> DResult<()> {
        self.client
            .stop(tonic::Request::new(TargetJobId {
                jobid: jobid.to_bytes(),
            }))
            .await?;

        Ok(())
    }

    /// Get status of a job
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

    /// Stream output of a job to an mpsc queue
    pub async fn output(
        &mut self,
        jobid: JobId,
        tx: Sender<(OutputStream, Vec<u8>)>,
    ) -> DResult<()> {
        let response = self
            .client
            .output(tonic::Request::new(TargetJobId {
                jobid: jobid.to_bytes(),
            }))
            .await?;

        let mut inner = response.into_inner();
        tokio::spawn(async move {
            while let Some(res) = inner.message().await.unwrap() {
                let event = OutputStream::from_i32(res.stream).unwrap();
                let r = tx.send((event, res.output)).await;
                // TODO: error handling
                if r.is_err() {
                    break;
                }
            }
        });
        Ok(())
    }
}
