#![deny(unused_must_use)]

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{AppSettings, Clap};

use common::t_service_server::TServiceServer;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

mod client_cert;
mod job;
mod output_stream;
mod service;

use self::service::TServiceImpl;

#[derive(Clap)]
#[clap(version, author)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(
        long = "client-root-ca-crt",
        env = "TLS_CLIENT_ROOT_CA_CRT",
        default_value = "certs/ca_client/root-ca.crt"
    )]
    client_root_ca_crt: PathBuf,
    #[clap(
        short = 'c',
        long = "crt",
        env = "TLS_SERVER_CRT",
        default_value = "certs/server/server.crt"
    )]
    server_crt: PathBuf,
    #[clap(
        short = 'k',
        long = "key",
        env = "TLS_SERVER_KEY",
        default_value = "certs/server/server.key"
    )]
    server_key: PathBuf,
    /// The address to serve at
    #[clap(default_value = "127.0.0.1:8000")]
    bind: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let opts: Opts = Opts::parse();

    // TODO: better error messages
    let crt = tokio::fs::read(opts.server_crt).await?;
    let key = tokio::fs::read(opts.server_key).await?;
    let server_identity = Identity::from_pem(crt, key);
    let client_ca_crt = tokio::fs::read(opts.client_root_ca_crt).await?;
    let client_ca_crt = Certificate::from_pem(client_ca_crt);

    let tls = ServerTlsConfig::new()
        .identity(server_identity)
        .client_ca_root(client_ca_crt);

    let service = TServiceImpl::new();
    let server = Server::builder()
        .tls_config(tls)?
        .add_service(TServiceServer::new(service));

    log::info!("Serving at {}", opts.bind);
    server.serve(opts.bind).await?;

    Ok(())
}
