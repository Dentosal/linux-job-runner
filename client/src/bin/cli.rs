use clap::{AppSettings, Clap};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use client::{Client, DResult, JobId, JobStartRequest, OutputStream, TlsConfig};

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

#[tokio::main]
async fn main() {
    env_logger::init();

    if let Err(err) = inner().await {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}

async fn inner() -> DResult<()> {
    let opts: Opts = Opts::parse();

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

            let error_msg = "writing out output stream failed";

            let mut out = std::io::stdout();
            let mut err = std::io::stderr();

            while let Some(event) = rx.recv().await {
                match event? {
                    (OutputStream::Stdout, data) => out.write_all(&data).expect(error_msg),
                    (OutputStream::Stderr, data) => err.write_all(&data).expect(error_msg),
                }
            }

            out.flush().expect(error_msg);
            err.flush().expect(error_msg);
        }
    }

    Ok(())
}
