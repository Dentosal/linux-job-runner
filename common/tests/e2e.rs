//! A suite of full end to end tests

use std::process::{Child, Command};
use std::sync::Once;

use common::JobId;

macro_rules! cli {
    ($server:expr, $($a:expr),*) => {{
        let output = Command::new("target/debug/cli")
                .arg(&format!("grpcs://{}", $server.addr))
                $(.arg($a))*
                .current_dir("..")
                .env("TLS_SERVER_ROOT_CA_CRT", "certs/ca_server/root-ca.crt")
                .env("TLS_CLIENT_CRT", "certs/client1/client.crt")
                .env("TLS_CLIENT_KEY", "certs/client1/client.key")
                .output()
                .expect("CLI failed");

        if !output.status.success() {
            panic!("Command {:?} failed: {}", ($($a),*), String::from_utf8_lossy(&output.stderr));
        }

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF8 in command output");
        stdout.trim().to_owned()
    }}
}

struct TestServer {
    child: Child,
    addr: String,
}

/// Makes sure the binary dependencies are only built once
static BUILD_DONE: Once = Once::new();

impl TestServer {
    pub fn new() -> Self {
        BUILD_DONE.call_once(|| {
            let build_status = Command::new("cargo")
                .arg("build")
                .current_dir("..")
                .status()
                .expect("Build failed");
            assert!(build_status.success());
        });

        let port = portpicker::pick_unused_port().expect("No ports free");
        let addr = format!("127.0.0.1:{}", port);

        let child = Command::new("target/debug/server")
            .arg(&addr)
            .current_dir("..")
            .spawn()
            .expect("Server failed to start");

        // Give the server a moment to wake up
        std::thread::sleep(std::time::Duration::from_millis(50));
        Self { child, addr }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.child.kill().expect("Dropping TestServer");
        self.child.wait().expect("Dropping TestServer");
    }
}

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
