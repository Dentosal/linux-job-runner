#![allow(dead_code)]

use std::process::{Child, Command};
use std::sync::Once;

/// Makes sure the binary dependencies are only built once
static BUILD_DONE: Once = Once::new();

pub struct TestServer {
    child: Child,
    pub addr: String,
}

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
            .env("RUST_LOG", "server=debug")
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

pub fn client_tls_config(client_number: usize) -> client::TlsConfig {
    client::TlsConfig {
        server_root_ca_crt: std::fs::read("../certs/ca_server/root-ca.crt").unwrap(),
        client_crt: std::fs::read(format!("../certs/client{}/client.crt", client_number)).unwrap(),
        client_key: std::fs::read(format!("../certs/client{}/client.key", client_number)).unwrap(),
    }
}

#[macro_export]
macro_rules! cli {
    ($server:expr, $($a:expr),*) => {{
        let output = std::process::Command::new("target/debug/cli")
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
