use clap::{AppSettings, Clap};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use client::{Client, JobId, JobStartRequest, OutputStream, TlsConfig};

#[derive(Clap)]
#[clap(version, author)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    server: String,
    #[clap(
        short = 's',
        long = "server-root-ca-crt",
        env = "TLS_SERVER_ROOT_CA_CRT"
    )]
    server_root_ca_crt: PathBuf,
    #[clap(short = 'c', long = "client-crt", env = "TLS_CLIENT_CRT")]
    client_crt: PathBuf,
    #[clap(short = 'k', long = "client-key", env = "TLS_CLIENT_KEY")]
    client_key: PathBuf,
    #[clap(subcommand)]
    subcmd: Operation,
}

#[derive(Clap)]
enum Operation {
    Start { path: String, args: Vec<String> },
    Stop { jobid: String },
    Status { jobid: String },
    Wait { jobid: String },
    Output { jobid: String },
}

// TODO: better error formatting
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let opts: Opts = Opts::parse();

    // TODO: handle errors nicer, maybe with "anyhow"?
    let tls = TlsConfig {
        server_root_ca_crt: fs::read(opts.server_root_ca_crt)?,
        client_crt: fs::read(opts.client_crt)?,
        client_key: fs::read(opts.client_key)?,
    };

    let mut client = Client::connect(opts.server, tls).await?;

    match opts.subcmd {
        Operation::Start { path, args } => {
            let jobid = client.start(JobStartRequest { path, args }).await?;
            println!("{}", jobid);
        }
        Operation::Stop { jobid } => client.stop(JobId::parse(&jobid)?).await?,
        Operation::Status { jobid } => {
            println!("{}", client.status(JobId::parse(&jobid)?).await?);
        }
        Operation::Wait { jobid } => {
            println!("{}", client.wait(JobId::parse(&jobid)?).await?);
        }
        Operation::Output { jobid } => {
            let (tx, mut rx) = tokio::sync::mpsc::channel(2);
            client.output(JobId::parse(&jobid)?, tx).await?;

            // If writing to stdout or stderr fails, there is no reason to continue => panic

            let mut out = std::io::stdout();
            let mut err = std::io::stderr();

            while let Some(event) = rx.recv().await {
                match event? {
                    (OutputStream::Stdout, data) => out.write_all(&data).unwrap(),
                    (OutputStream::Stderr, data) => err.write_all(&data).unwrap(),
                }
            }

            out.flush().unwrap();
            err.flush().unwrap();
        }
    }

    Ok(())
}
