# Design doc for job-runner

Job-runner provides a gRCP API to execute arbitrary commands on a Linux host.

## Runner

The jobs themselves are normal child processes of the runner. The runner configures CGroups and Linux namespaces for children to limit resource usage and isolate the them from the other processes. It also handles output recording and forwarding. All data is stored in-memory. In case of the service process termination, all jobs and their status are lost.

## Communication and auth

Clients communicate via gRPC with a simple protocol: One service with four endpoints. The communication is secured [RusTLS](https://github.com/ctz/rustls) which by design supports only modern, secure cipher suites. [Tonic](https://github.com/hyperium/tonic) is used to serve gRPC and almost automatically handles protocol buffers, encryption and related concerns.

Authentication is implemented with mTLS. Server and client have different CA roots, which they are expected to exchange in a secure way. (Scenario-wise: the client CA is operated by the same organization that hosts job-runner). The server identifies each client with it's CommonName (CN) field of the certificate. The client CA only issues certificates with CN values to developers that are allowed to access the API. All running jobs are bound to the CN of the client calling `Start`, and only a client with that certificate is allowed to execute operations for that job.

A more complete system should also support certificate revocation lists (CRL), so that individual certificates can be revoked in case they are compromised. Other useful feature would be having more fine-grained access control per certificate, for instance restricting clients to only specific endpoints, executables, and resource limits.

## Endpoints

### Start

Starts a new job by spawning a process from given executable path and arguments. Returns a unique job id, that is used to specify the target job for other endpoints. If the executable is not found or cannot be executed, immediately returns an error.

No security checks are applied to the program and arguments. If the client wishes to remove some files or start a fork bomb, it's allowed to do so.

### Stop

Cancels a job by sending `SIGKILL`. Stop only returns after the process has been terminated, and returns the status of the job. If job was already terminated before calling stop, the old state is returned without attempting termination.

Could be improved by sending `SIGTERM` shortly before `SIGKILL`, but I'm aiming for simplicity here. Also separating soft and hard kills in the API level might be useful in some situations.

### Status

Returns job status, i.e. is it running, and the status code if the job has completed. If the job has been terminated with a signal, that is reported instead.

### Output

Streams output of a job in binary blobs. Each blob is tagged to be either from stdout or stderr. Stream is automatically closed when the process completes and all output has been streamed. All calls to output stream the whole output history from the moment the process was started.

All output of each process is stored in a separate memory buffer. For each call of output, a separate offset value for both IO streams is maintained.

## Scalability and high availability

The system is reasonably perfomant, it should be able to handle quite a lot of processes on a single machine. However, there is no way to run this on multiple machines without some coordinator in front. The limits the horizontal scalability to a single machine. Of course it's possible for the client to handle multiple machines, but this is not built into the system. Ideally there would be a way to move processes from machine to another, but whether that's viable depends on many factors. This requires some way to synchronize shared state, usually using a database

As for high availablity, similar concerns apply. Again, coordination with multiple controller nodes might be the best way to solve this. Many things depend on what properties are required. For instance, usually the platform itself cannot handle all high-availability concerns, as those must be solved by the application. However, things like "there should always be at least one copy of this software running" can usually be solved by duplication.

### Other tradeoffs and simplifications

Full output history of all jobs is stored in memory, and is only removed on server restart. In a real system, the output would usually be streamed to a log database, or just into a file, to reduce memory pressure. After process termination logs should be either removed or moved to an archive (e.g. Amazon S3).

Other simplification is about how jobs are identified. A proper implementation would use either UUIDs or user-given names, maybe even both. This implementation, however, uses sequential unsigned 64-bit integers to identify jobs. The CLI tool uses them as well, making it particularly error-prone when jobs are manually referenced. The job ids also reset to zero every time the server is restarted, so that no persitent storage is required. This may cause conflicts if the server is restarted. Using UUIDs would solve this issue automatically.

To further simplify the implementation, process state change notifications. To wait a process terminates, a client must poll the status endpoint or read output of the process. When the output is not required, this is rather inefficient, but it reduces the complexity of the server. If better performance is required, an endpoint that only returns after a state change occurs (similar to long polling) could be implemented.

Many details of the system that should usually be configured either in the application config or in the API calls are simply hardcoded. This includes access control, resource limits and the location and configuration of TLS certificates.
