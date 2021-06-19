#![deny(unused_must_use)]

use std::time::Duration;
use tokio::time::Instant;

mod helpers;

use helpers::{client_tls_config, TestServer};

use client::{Client, JobStartRequest};

#[tokio::test]
async fn test_concurrent_output() -> Result<(), Box<dyn std::error::Error>> {
    let s = TestServer::new();

    let mut client = Client::connect(format!("grpcs://{}", s.addr), client_tls_config(1)).await?;

    let jobid = client
        .start(JobStartRequest {
            path: "./common/tests/scripts/slow-output.sh".to_owned(),
            args: Vec::new(),
        })
        .await?;

    let (a_tx, mut a_rx) = tokio::sync::mpsc::channel(1);
    let (b_tx, mut b_rx) = tokio::sync::mpsc::channel(1);

    client.output(jobid, a_tx).await.expect("Output failed");
    client.output(jobid, b_tx).await.expect("Output failed");

    // Test that messages are streamed immediately when they are ready

    let timer = Instant::now();
    a_rx.recv().await.expect("No event").expect("Error");
    a_rx.recv().await.expect("No event").expect("Error");
    a_rx.recv().await.expect("No event").expect("Error");
    assert!(timer.elapsed() > Duration::new(2, 0));

    let timer = Instant::now();
    b_rx.recv().await.expect("No event").expect("Error");
    b_rx.recv().await.expect("No event").expect("Error");
    b_rx.recv().await.expect("No event").expect("Error");
    assert!(timer.elapsed() < Duration::from_millis(10));

    // Test that the output is really concurrent

    for _ in 0..2 {
        let timer = Instant::now();
        b_rx.recv().await.expect("No event").expect("Error");
        assert!(timer.elapsed() > Duration::new(1, 0));

        let timer = Instant::now();
        a_rx.recv().await.expect("No event").expect("Error");
        assert!(timer.elapsed() < Duration::from_millis(10));
    }

    client.stop(jobid).await.expect("Stopping failed");

    Ok(())
}
