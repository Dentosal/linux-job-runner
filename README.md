# Linux job runner

Server and API for running encapsulated Linux commands remotely.

## Dependencies

Requires latest stable Rust release, which can be installed using [rustup](https://rustup.rs/). The project is only tested on Linux systems, and uses platform-specific features (although making them optional might be possible in the future).

A recent version of `openssl` command is required as well.


## Setup

First generate and export certs with:

```bash
./gen-certs.sh
export TLS_SERVER_ROOT_CA_CRT=certs/ca_server/root-ca.crt
export TLS_CLIENT_CRT=certs/client1/client.crt
export TLS_CLIENT_KEY=certs/client1/client.key
```

## Run

Use `cargo run --bin server` to run start the server. By default it starts on `127.0.0.1:8000`, but you can change that by passing in the `host:port`, for instance: `cargo run --bin server 0.0.0.0:8123`.

Now that the server is running, run the client (in another terminal) with `cargo run --bin cli`. For for instance, try:

```bash
export SERVER='grpcs://localhost:8000'

# Show the path where out process runs
job_id=$(cargo run --bin cli $SERVER start pwd)
echo "Job id: $job_id"
cargo run --bin cli $SERVER status $job_id
cargo run --bin cli $SERVER output $job_id

# Stop a long-running process
job_id=$(cargo run --bin cli $SERVER start sleep infinity)
echo "Job id: $job_id"
cargo run --bin cli $SERVER status $job_id
cargo run --bin cli $SERVER stop $job_id
cargo run --bin cli $SERVER status $job_id
```

## Development

Code is formatted with `rustfmt`, use `cargo fmt` to apply.

For linting `cargo clippy` is used. New changes should not introduce any style regressions.

The automated test suite can be ran with `cargo test`. This is rather limited at the moment, so manual testing is required.

To automatically enforce all these in the version control, run `./git-hooks.sh setup`.

