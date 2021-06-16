#![deny(unused_must_use)]

use std::net::SocketAddr;

use common::t_service_server::TServiceServer;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

mod client_cert;
mod job;
mod output_stream;
mod service;

use self::service::TServiceImpl;

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
