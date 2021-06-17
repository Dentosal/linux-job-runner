//! A suite of full end to end tests

mod helpers;

use helpers::TestServer;

use common::JobId;

#[test]
fn test_simple_ls() {
    let s = TestServer::new();

    let job_id = cli!(s, "start", "ls", "/");

    let status = cli!(s, "status", &job_id);
    assert!(status.contains("Complete"));

    let output = cli!(s, "output", &job_id);
    assert!(output.contains("root"));
    assert!(output.contains("etc"));
}

#[test]
fn test_simple_stop() {
    let s = TestServer::new();

    let job_id = cli!(s, "start", "sleep", "infinity");

    let status = cli!(s, "status", &job_id);
    println!("Status {}", status);
    assert!(status.contains("Running"));

    cli!(s, "stop", &job_id);

    let status = cli!(s, "status", &job_id);
    assert!(status.contains("Signal"));
}

#[test]
fn test_concurrent() {
    let s = TestServer::new();

    let job_a = cli!(s, "start", "sleep", "infinity");
    let job_b = cli!(s, "start", "sleep", "infinity");

    let status = cli!(s, "status", &job_a);
    assert!(status.contains("Running"));

    let status = cli!(s, "status", &job_b);
    assert!(status.contains("Running"));

    cli!(s, "stop", &job_a);
    cli!(s, "stop", &job_b);

    let status = cli!(s, "status", &job_a);
    assert!(status.contains("Signal"));

    let status = cli!(s, "status", &job_b);
    assert!(status.contains("Signal"));
}

#[test]
fn test_exit_code() {
    let s = TestServer::new();

    let job_id = cli!(s, "start", "ls", "/NONEXISTENT");

    let status = cli!(s, "status", &job_id);
    assert!(status.contains("Complete"));
    assert!(status.contains('2')); // Exit code ls returns for nonexistent files
}

#[test]
#[should_panic(expected = "FailedPrecondition")]
fn test_nonexistent_binary() {
    let s = TestServer::new();

    cli!(s, "start", "NONEXISTENT");
}

#[test]
#[should_panic(expected = "No such job")]
fn test_nonexistent_job() {
    let s = TestServer::new();
    cli!(s, "status", &JobId::new().to_string());
}
