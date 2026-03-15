# `coff_loader` Runtime Design

This document defines the runtime design for the remaining `coff_loader` implant work needed to fully interoperate with the current teamserver.

It is intentionally narrower than a general implant architecture document. The goal here is to turn the existing standalone loader into a polling implant process that:

- identifies itself to the teamserver
- maintains liveness with heartbeats
- polls for tasking
- executes `execute_coff` tasks
- returns results through the current packet protocol

This design builds on:

- [docs/wire-format.md](/C:/Users/wammu/source/repos/malice/docs/wire-format.md)
- [docs/interop-architecture.md](/C:/Users/wammu/source/repos/malice/docs/interop-architecture.md)
- [docs/modular-architecture.md](/C:/Users/wammu/source/repos/malice/docs/modular-architecture.md)

## Scope

This phase covers the first usable implant runtime for the `coff_loader` family.

Included:

- implant configuration
- client ID persistence
- packet encode/decode
- HTTP transport to `POST /packet`
- registration
- heartbeat loop
- task polling loop
- `execute_coff` task execution
- task result upload
- minimal runtime status tracking

Not included:

- encrypted transport
- proxy support
- reconnect backoff tuning beyond a basic implementation
- concurrent task execution
- chunked binary transfer
- process injection
- generic BOF argument marshalling
- persistence of queued tasks on the implant side

## Goals

- keep the implant runtime small and explicit
- match the current teamserver protocol exactly
- separate transport, protocol, runtime, and loader concerns inside the implant
- make it straightforward to add more task kinds later without rewriting the polling runtime

## Runtime Model

The implant is a single-process polling client.

At a high level:

1. startup loads configuration and a stable `clientid`
2. implant collects host metadata
3. implant sends `REGISTER`
4. implant enters a main loop
5. main loop sends `HEARTBEAT` on a fixed interval
6. main loop polls `FETCH_TASK`
7. implant executes zero or one fetched tasks
8. implant posts `TASK_RESULT`

The first version should remain single-threaded in terms of task execution. Heartbeat and task polling can be coordinated by a simple scheduler in one main loop rather than a complex multi-threaded worker model.

## Process Architecture

Recommended internal module layout for the `coff_loader` project:

```text
implant/coff_loader/
- runtime/
  - config.h / config.cpp
  - identity.h / identity.cpp
  - metadata.h / metadata.cpp
  - protocol.h / protocol.cpp
  - transport.h / transport.cpp
  - scheduler.h / scheduler.cpp
  - tasks.h / tasks.cpp
  - status.h / status.cpp
- loader/
  - coff_loader.h / coff_loader.cpp
- main.cpp
```

The exact filenames can vary, but the separation should remain.

### Module Responsibilities

`config`

- teamserver base URL
- poll interval
- heartbeat interval
- optional implant sleep jitter later
- path for `clientid` persistence

`identity`

- generate or load the implant `clientid`
- persist it locally

`metadata`

- collect registration fields:
  - `implant_type`
  - `protocol_version`
  - `hostname`
  - `username`
  - `pid`
  - `process_name`
  - `os`
  - `arch`

`protocol`

- define request and response DTOs
- encode packets using the current opcode + JSON-string payload format
- decode server replies

`transport`

- send raw packet bytes to `POST /packet`
- return raw response bytes
- own timeout and retry policy for one request

`scheduler`

- drive registration, heartbeat timing, and task polling cadence
- maintain simple runtime state:
  - unregistered
  - idle
  - busy
  - error

`tasks`

- interpret fetched `TaskEnvelope` values
- dispatch by `task_type`
- currently only `execute_coff`

`loader`

- execute a COFF object from memory
- return a structured execution result to the runtime

## Main Loop Design

The implant should use a cooperative loop with two independent timers:

- heartbeat timer
- task poll timer

Pseudo-flow:

```text
load config
load or create clientid
collect metadata
register

last_heartbeat = now
last_poll = now - poll_interval
status = idle

loop forever:
    now = current time

    if now - last_heartbeat >= heartbeat_interval:
        send heartbeat(status)
        last_heartbeat = now

    if status != busy and now - last_poll >= poll_interval:
        tasks = fetch_task(want=1)
        last_poll = now

        if tasks not empty:
            status = busy
            for task in tasks:
                result = execute task
                upload result
            status = idle

    sleep small interval
```

This is enough for the first milestone and avoids premature concurrency.

## Configuration

The runtime needs a minimal explicit configuration surface.

Recommended first configuration fields:

```text
teamserver_url = http://127.0.0.1:42069/packet
heartbeat_interval_seconds = 30
poll_interval_seconds = 5
request_timeout_seconds = 10
clientid_path = %ProgramData%\malice\coff_loader\clientid.txt
```

For local development, these can initially be:

- compile-time constants
- a small config file next to the binary
- command-line arguments

Recommended first implementation: command-line overrides plus sensible defaults.

## Client ID Persistence

The teamserver expects a stable UUID string per implant instance.

The runtime should:

1. attempt to read `clientid` from disk
2. validate that it is a UUID string
3. if missing or invalid, generate a new UUID
4. persist it atomically

Recommended behavior:

- store plain UUID text
- create parent directories if missing
- fail closed only if the filesystem is unusable and no in-memory fallback is acceptable

For the first milestone, an in-memory fallback is acceptable during development, but persisted identity is preferred.

## Registration

Registration uses opcode `REGISTER` (`0x00`).

The implant sends:

```json
{
  "implant_type": "coff_loader",
  "protocol_version": 1,
  "hostname": "HOST01",
  "username": "alice",
  "pid": 1234,
  "process_name": "coff_loader.exe",
  "os": "windows",
  "arch": "x64"
}
```

Requirements:

- `implant_type` must stay `coff_loader`
- `protocol_version` must match the current teamserver expectation
- the implant must treat registration failure as blocking startup

If registration fails:

- retry with bounded delay
- do not begin task polling before registration succeeds

## Heartbeat

Heartbeat uses opcode `HEARTBEAT` (`0x03`).

The implant sends:

```json
{
  "sequence": 7,
  "status": "idle"
}
```

Recommended statuses for the first phase:

- `idle`
- `busy`
- `error`

Rules:

- increment `sequence` monotonically per process lifetime
- send `busy` only while executing a task
- send `error` only if the implant enters a repeated transport or execution failure state

If heartbeat fails:

- log locally
- continue retrying on the next interval
- do not terminate the implant solely because one heartbeat failed

## Task Polling

Task polling uses opcode `FETCH_TASK` (`0x01`).

Request:

```json
{
  "want": 1
}
```

Response:

```json
{
  "tasks": [
    {
      "task_id": "<uuid>",
      "task_type": "execute_coff",
      "object_name": "whoami.obj",
      "entrypoint": "main",
      "object_encoding": "base64",
      "object_data": "...",
      "args_encoding": "base64",
      "args_data": ""
    }
  ]
}
```

Rules:

- the implant should currently request `want = 1`
- it should accept an empty array
- it should reject unknown task types with an error result only if a `task_id` exists
- it should reject unsupported encodings explicitly

## Task Dispatch

The implant runtime should decode fetched tasks into internal task specs rather than handling raw JSON objects throughout the code.

Recommended runtime type:

```text
TaskSpec
- ExecuteCoff
```

Execution dispatcher:

```text
handle_task(task):
    switch task.task_type
        execute_coff -> execute_coff_task(task)
        default -> produce error result
```

This mirrors the teamserver’s typed `TaskSpec` direction and keeps future task additions additive.

## `execute_coff` Task Design

This is the only required task type for the first milestone.

Input fields:

- `task_id`
- `object_name`
- `entrypoint`
- `object_encoding`
- `object_data`
- `args_encoding`
- `args_data`

Runtime steps:

1. validate `object_encoding == "base64"`
2. base64-decode `object_data`
3. decode `args_data`
4. pass object bytes, entrypoint name, and args into the COFF loader
5. capture execution result
6. convert it into a `TASK_RESULT` payload

### Argument Handling

The current teamserver encodes args as bytes and then base64 for `execute_coff`.

For the first runtime version:

- accept empty args
- accept opaque byte arguments
- pass raw bytes through to the loader interface

Do not invent a new argument schema inside the implant. The implant should treat args as transport-provided input and let the loader or payload contract interpret them.

## Loader Interface

The current project has a standalone loader executable. For implant runtime integration, the important change is that the loader should become a callable library-like component rather than only a CLI.

Recommended interface:

```text
ExecuteCoffResult execute_coff_object(
    const std::vector<std::uint8_t>& object_bytes,
    std::string_view entrypoint,
    const std::vector<std::uint8_t>& args);
```

Recommended result type:

```text
ExecuteCoffResult
- success: bool
- exit_code: optional integer
- stdout_text: optional string
- stderr_text: optional string
- error_message: optional string
```

For the first milestone, it is acceptable if the loader only returns:

- success/failure
- exit code
- optional text output

The runtime should not depend on internal loader implementation details.

## Output Capture

The interop plan expects the implant to send back human-readable result text.

There are two plausible execution models:

1. the loader returns an integer exit code only
2. the payload writes to an output buffer or stdout and the loader returns that text

Recommended first milestone behavior:

- preserve the current exit-code-based loader behavior for test fixtures
- extend the runtime path so that real implant task execution can capture output text from task payloads

That means the loader interface should be designed to support text output even if some current fixtures only validate exit codes.

For `whoami`, the intended steady-state result sent to the teamserver is:

```text
HOSTNAME\username
```

or an error string if execution fails.

## Task Result Upload

Task results use opcode `TASK_RESULT` (`0x02`).

Success result:

```json
{
  "task_id": "<uuid>",
  "status": "success",
  "result_encoding": "utf8",
  "result_data": "HOST01\\alice"
}
```

Failure result:

```json
{
  "task_id": "<uuid>",
  "status": "error",
  "result_encoding": "utf8",
  "result_data": "entrypoint main not found"
}
```

Rules:

- exactly one result upload per executed task
- unknown or malformed tasks should still generate an error result if `task_id` is known
- if result upload fails, retry with bounded attempts before moving on

For the first milestone, it is acceptable for a failed result upload to be retried only in-memory during the current process lifetime.

## Protocol Layer Design

The implant protocol layer should mirror the teamserver packet rules exactly.

Recommended DTOs:

```text
RegisterPayload
HeartbeatPayload
FetchTaskRequest
FetchTaskResponse
TaskEnvelope
TaskResultPayload
BasePacket
```

Recommended packet helpers:

```text
build_packet(opcode, clientid, payload_json) -> bytes
parse_packet(bytes) -> opcode + clientid + inner_json
```

Important wire rule:

- the inner payload is serialized JSON text stored as a JSON string in `BasePacket.data`

That means double-serialization is required and should be centralized in one place. Do not duplicate this behavior ad hoc in request code.

## HTTP Transport Design

The transport layer should know only:

- destination URL
- request timeout
- raw request bytes
- raw response bytes

Request contract:

- `POST /packet`
- `Content-Type: application/octet-stream`
- request body is the full opcode-prefixed packet

Response contract:

- response body is the full opcode-prefixed packet

The transport layer should not:

- inspect opcodes
- know task types
- parse registration metadata

## Error Handling Strategy

Use three error classes:

`transport errors`

- cannot connect
- timeout
- bad HTTP status

`protocol errors`

- invalid packet framing
- JSON parse failure
- unsupported opcode
- unsupported encoding

`execution errors`

- invalid task payload
- base64 decode failure
- loader failure
- missing entrypoint

Recommended policy:

- registration transport failure: retry
- heartbeat transport failure: log and continue
- fetch transport failure: log and continue
- task execution failure: upload error result if possible
- result upload failure: retry a few times, then log and continue

## Logging And Diagnostics

The implant should log locally to stderr or a small logging sink.

Recommended events:

- startup
- loaded/generated clientid
- registration success/failure
- heartbeat send/failure
- task poll success/failure
- task receipt
- task execution start/finish
- task result upload success/failure

Do not dump raw object bytes or other large payloads to logs.

## Security And Operational Constraints

For this milestone:

- no encryption
- no mutual authentication
- no artifact signing
- no persistence hardening

That is acceptable only because this is still a development-stage runtime.

Guardrails:

- validate server-provided encodings before decoding
- validate task IDs before use
- never treat arbitrary payload bytes as strings without explicit decoding
- keep the loader interface bounded and typed

## Testing Strategy

Testing should be split into layers.

### 1. Packet Tests

Verify:

- packet encode matches teamserver format
- packet decode handles valid replies
- malformed packets fail cleanly

### 2. Transport Tests

Verify:

- request body is sent to `POST /packet`
- non-200 responses are handled

### 3. Runtime Loop Tests

Verify:

- startup registers once
- heartbeat cadence works
- empty task responses do not execute work
- fetched `execute_coff` tasks are dispatched
- success and error results are uploaded correctly

### 4. Loader Integration Tests

Reuse the current `coff_loader` end-to-end fixture coverage for:

- basic execution
- relocation handling
- import resolution
- `whoami`-oriented API usage

### 5. End-To-End Interop Test

Target final validation:

1. start teamserver
2. start implant runtime
3. observe implant registration
4. queue `task queue <clientid> execute_coff whoami main`
5. observe task fetch
6. observe result upload
7. verify `task result <taskid>`

## Implementation Order

Recommended order for the runtime work:

1. extract the loader into a callable interface while preserving current tests
2. add protocol DTOs and packet helpers
3. add HTTP transport wrapper for `POST /packet`
4. add registration request path
5. add clientid persistence
6. add heartbeat loop
7. add task polling and `TaskEnvelope` parsing
8. add `execute_coff` task dispatch
9. add result upload
10. run full end-to-end interop against the teamserver

This order keeps the loader stable while the runtime layers are added around it.

## Acceptance Criteria

The runtime work for this phase is complete when:

- the implant generates or loads a stable `clientid`
- the implant successfully registers with the teamserver
- `implants list` shows the implant as `coff_loader`
- the implant sends periodic heartbeats and status updates
- the implant polls `FETCH_TASK` and handles empty responses cleanly
- the implant executes a delivered `execute_coff` task
- the implant uploads a success result for `whoami`
- `task result <taskid>` shows the returned output
- the current loader fixture tests remain green

## Open Questions

These should be resolved before implementation starts if possible:

- should the runtime be a new executable that embeds the loader, or should the current `coff_loader` binary evolve directly into the implant process?
- what is the preferred HTTP client dependency for Windows in this project?
- how should payload stdout be surfaced from the loader: callback, shared buffer, or wrapper API?
- should `clientid` live in `%ProgramData%`, the current working directory, or a repo-local dev path during development?
- do we want jitter on polling now, or only after first interop works?

My recommendation:

- keep the existing loader core, but introduce a new implant runtime executable around it
- keep packet and runtime logic separate from loader internals
- defer jitter and advanced transport concerns until after successful first interop
