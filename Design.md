# Design doc for job-runner

Job-runner provides a gRCP API to execute arbitrary commands on a Linux host.

## Runner

The jobs themselves are normal child processes of the runner. The runner configures CGroups and Linux namespaces for children to limit resource usage and isolate the them from the other processes. It also handles output recording and forwarding. All data is stored in-memory. In case of the service process termination, all jobs and their status are lost.

## Resource limits and isolation

The job-runner uses [cgroups V2](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html) for resource limits. There are many tweakable options for limiting resource use, but the following minimal set has been selected to demonstrate how they work. In particular, the following cgroup parameters will be set:
* CPU:
    * `cpu.max` quota and period
* Memory:
    * `memory.max` (max amount of memory to be used, hard limit)
    * `memory.high` (set to 75% of `memory.max`)
* Disk IO: (Device list readable from `/proc/partitions`)
    * `io.max.{rbps,wbps,riops,wiops}`

In addition to limiting resource use with cgroups, job-runner also isolates jobs from each other using namespaces. A PID namespace is set up to make sure the job cannot kill processes not spawned by it, and to make sure all child processes are terminated together with the actual job. A mount namespace is used to limit process to a subset of the file system, together with [`pivot_root(2)`](https://linux.die.net/man/2/pivot_root) (see [Understanding Containerization By Recreating Docker](https://itnext.io/linux-container-from-scratch-339c3ba0411d), search for pivot_root). Finally, a network namespace is created to limit network access of the jobs. It must be used together with [Virtual ethernet (VETH)](https://developers.redhat.com/blog/2018/10/22/introduction-to-linux-interfaces-for-virtual-networking#veth) and bridge interfaces if jobs should be allowed to communicate between each other. This also means that internet access must be granted separately.

## Communication and auth

Clients communicate via gRPC with a simple protocol: One service with four endpoints. The communication is secured [RusTLS](https://github.com/ctz/rustls) which properly [audited](https://github.com/ctz/rustls/blob/master/audit/TLS-01-report.pdf) and by design supports only modern, secure cipher suites. Only TLS 1.2/1.3 are used, and authentication is done using ECDSA, Ed25519 or RSA. RusTLS will remove support for cipher suites deemed insecure, and simply keeping the version up to date should be sufficient in the future as well. [Tonic](https://github.com/hyperium/tonic) is used to serve gRPC and almost automatically handles protocol buffers, encryption and related concerns.

Authentication is implemented with mTLS. Server and client have different CA roots, which they are expected to exchange in a secure way. (Scenario-wise: the client CA is operated by the same organization that hosts job-runner). The server identifies each client with it's CommonName (CN) field of the certificate. The client CA only issues certificates with CN values to developers that are allowed to access the API. Any client with a such certificate is allowed to start new jobs. All running jobs are bound to the CN of the client calling `Start`, and only a client with that certificate is allowed to execute operations for that job.

A more complete system should also support certificate revocation lists (CRL), so that individual certificates can be revoked in case they are compromised. Other useful feature would be having more fine-grained access control per certificate, for instance restricting clients to only specific endpoints, executables, and resource limits.

## Endpoints

### Protobuf

```proto3
syntax = "proto3";

package common;

service TService {
    rpc Start (JobStartRequest) returns (TargetJobId);
    rpc Stop (TargetJobId) returns (StopSignalSent);
    rpc Status (TargetJobId) returns (JobStatus);
    rpc Output (TargetJobId) returns (stream OutputEvent);
}

message JobStartRequest {
    string path = 1;
    repeated string args = 2;
}

message StopSignalSent {}

message TargetJobId {
    bytes jobid = 1;
}

message JobStatus {
    oneof completed {               // Empty if still running
        int32   status_code = 2;    // Completed normally
        int32   signal = 3;         // Terminated by a signal
    }
}

message OutputEvent {
    enum Stream {
        stdout = 0;
        stderr = 1;
    }
    Stream stream = 1;
    bytes output = 2;
}

```

### Start

Starts a new job by spawning a process from given executable path and arguments. Returns a unique job id (UUID v4), that is used to specify the target job for other endpoints. If the executable is not found or cannot be executed, immediately returns an error.

No security checks are applied to the program and arguments. However, as the job is placed inside an isolated container, it shouldn't be able to do much damage. It can still consume resources in the limits that cgroup-limits allow, and it can flood it's output with thrash data.

### Stop

Cancels a job by sending `SIGKILL`. This is done asynchronously, and stop can return before the process has terminated. If the client must wait until the job has stopped, it can do so by polling `Status`.

Could be improved by sending `SIGTERM` shortly before `SIGKILL`, but I'm aiming for simplicity here. Also separating soft and hard kills in the API level might be useful in some situations.

### Status

Returns job status, i.e. is it running, and the status code if the job has completed. If the job has been terminated with a signal, that is reported instead.

### Output

Streams output of a job in binary blobs. Each blob is tagged to be either from stdout or stderr. Stream is automatically closed when the process completes and all output has been streamed. All calls to output stream the whole output history from the moment the process was started.

#### Internals

There will be two async-tasks reading the output of a process, one for stdout and one for stderr. For both tasks, a separate buffer of output history is maintained, along with a boolean marking process completion. This state is protected by a [`tokio::sync::RwLock`](https://docs.rs/tokio/1.6.1/tokio/sync/struct.RwLock.html). In addition, there will be a [`tokio::sync::Notify`](https://docs.rs/tokio/1.6.1/tokio/sync/struct.Notify.html). Every time the process writes more output, the listerners are woken up through the `Notify`.

Each call of `Output` spawns async-tasks for stdio and stdout. They read the output buffer until the end. Then it checks if the process is completed (from the field). If yes, then the connection to client is closed to mark process completion. Otherwise, it waits until the output reader task notifies it that new data is available, and then repeats the above process.

## CLI

The CLI can be used to operate the job runner. All CLI commands have the job server URL as the first argument and the actual command after that.

```
cli grpcs://job-service.example.org:8000 subcommand arguments
```

It has the following subcommands:

* `start <executable> [args]...` -- Starts a new job by spawning a process, prints the job id to stdout.
* `stop jobid` -- Stops job with given id.
* `status jobid` -- Prints job status to stdout.
* `output jobid` -- Streams job stdout and stderr to respective output streams. Starts from the beginning of the job.

TLS setup can be passed in through environment variables: `TLS_SERVER_ROOT_CA_CRT`, `TLS_CLIENT_CRT` and `TLS_CLIENT_KEY`. These should point to the PEM-encoded files: `_CRT`s to  certificates and `_KEY` to the private key. (A real program should probably prefix these with a semi-unique name, but that would require naming the project first.)

### Example usage

#### List directory

```
$ export TLS_SERVER_ROOT_CA_CRT=certs/ca_server/root-ca.crt
$ export TLS_CLIENT_CRT=certs/client1/client.crt
$ export TLS_CLIENT_KEY=certs/client1/client.key
$ cli grpcs://localhost:8000 start ls /usr
bba87bff-3719-4f76-98d9-c8e86f03f7aa
$ cli grpcs://localhost:8000 output bba87bff-3719-4f76-98d9-c8e86f03f7aa
bin
games
include
lib
lib32
libexec
local
sbin
share
src
$ cli grpcs://localhost:8000 status bba87bff-3719-4f76-98d9-c8e86f03f7aa
Completed 0
```

#### Stopping a process

```
$ cli grpcs://localhost:8000 start yes
a354142a-c59f-44dd-ac53-c0110943df2b
$ cli grpcs://localhost:8000 output a354142a-c59f-44dd-ac53-c0110943df2b
y
y
y
y
y
...     # Omitted
y
y
^C      # Keyboard interrupt
$ cli grpcs://localhost:8000 stop a354142a-c59f-44dd-ac53-c0110943df2b
Signal 9
$ cli grpcs://localhost:8000 status a354142a-c59f-44dd-ac53-c0110943df2b
Signal 9
```


## Scalability and high availability

The system is reasonably perfomant, it should be able to handle quite a lot of processes on a single machine. However, there is no way to run this on multiple machines without some coordinator in front. The limits the horizontal scalability to a single machine. Of course it's possible for the client to handle multiple machines, but this is not built into the system. Ideally there would be a way to move processes from machine to another, but whether that's viable depends on many factors. This requires some way to synchronize shared state, usually using a database

As for high availablity, similar concerns apply. Again, coordination with multiple controller nodes might be the best way to solve this. Many things depend on what properties are required. For instance, usually the platform itself cannot handle all high-availability concerns, as those must be solved by the application. However, things like "there should always be at least one copy of this software running" can usually be solved by duplication.

### Other tradeoffs and simplifications

Full output history of all jobs is stored in memory, and is only removed on server restart. In a real system, the output would usually be streamed to a log database, or just into a file, to reduce memory pressure. After process termination logs should be either removed or moved to an archive (e.g. Amazon S3).

To further simplify the implementation, process state change notifications. To wait a process terminates, a client must poll the status endpoint or read output of the process. When the output is not required, this is rather inefficient, but it reduces the complexity of the server. If better performance is required, an endpoint that only returns after a state change occurs (similar to long polling) could be implemented.

Many details of the system that should usually be configured either in the application config or in the API calls are simply hardcoded. This includes access control, resource limits and the location and configuration of TLS certificates.
