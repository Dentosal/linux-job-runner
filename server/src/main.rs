#![deny(unused_must_use)]

use std::net::SocketAddr;
use std::pin::Pin;

use tokio_stream::Stream;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};
use tonic::{Request, Response, Status};

use common::t_service_server::{TService, TServiceServer};
use common::*;

type BoxStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send + Sync + 'static>>;

pub struct TServiceImpl {}

impl TServiceImpl {
    pub fn new() -> Self {
        Self {}
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
        _request: Request<JobStartRequest>,
    ) -> Result<Response<TargetJobId>, Status> {
        todo!("Start job");
    }

    /// Starts killing the child process, but doesn't wait until it's actually stopped
    async fn stop(
        &self,
        _request: Request<TargetJobId>,
    ) -> Result<Response<StopSignalSent>, Status> {
        todo!("Stop job");
    }

    /// Get status of a job
    async fn status(&self, _request: Request<TargetJobId>) -> Result<Response<JobStatus>, Status> {
        todo!("Get status");
    }

    type OutputStream = BoxStream<OutputEvent>;

    /// Stream output of a job
    async fn output(
        &self,
        _request: Request<TargetJobId>,
    ) -> Result<Response<Self::OutputStream>, Status> {
        todo!("Stream output");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // TODO: do argument parsing properly, e.g. with clap
    let addr: SocketAddr = if let Some(arg) = std::env::args().nth(1) {
        arg.parse().expect("Invalid bind address")
    } else {
        "127.0.0.1:8000".parse().unwrap()
    };

    // TODO: read certificates from argument paths, and not hard coded locations
    let cert = tokio::fs::read("certs/server/server.crt").await?;
    let key = tokio::fs::read("certs/server/server.key").await?;
    let server_identity = Identity::from_pem(cert, key);
    let client_ca_cert = tokio::fs::read("certs/ca_client/root-ca.crt").await?;
    let client_ca_cert = Certificate::from_pem(client_ca_cert);

    let tls = ServerTlsConfig::new()
        .identity(server_identity)
        .client_ca_root(client_ca_cert);

    let service = TServiceImpl::new();
    let server = Server::builder()
        .tls_config(tls)?
        .add_service(TServiceServer::new(service));

    log::info!("Serving at {}", addr);
    server.serve(addr).await?;

    Ok(())
}
