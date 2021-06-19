#![deny(unused_must_use)]

mod helpers;

use helpers::{client_tls_config, TestServer};

use client::{Client, JobStartRequest};

#[tokio::test]
async fn test_permission_denied() -> Result<(), Box<dyn std::error::Error>> {
    let s = TestServer::new();

    let mut client1 = Client::connect(format!("grpcs://{}", s.addr), client_tls_config(1)).await?;
    let mut client2 = Client::connect(format!("grpcs://{}", s.addr), client_tls_config(2)).await?;

    let jobid = client1
        .start(JobStartRequest {
            path: "sleep".to_owned(),
            args: vec!["infinity".to_owned()],
        })
        .await?;

    let result = client2.stop(jobid).await;
    assert!(
        result.is_err(),
        "Expected permission denied error, instead succeeded"
    );

    Ok(())
}
