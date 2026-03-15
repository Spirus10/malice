# Teamserver And `coff_loader` Interop Architecture

This document describes the target architecture for the next implementation phase: implant registration, heartbeat tracking, implant listing, and delivery of a single COFF object payload that runs `whoami` on the implant host and returns the result to the teamserver.

This builds on the current wire contract defined in [docs/wire-format.md](/C:/Users/wammu/source/repos/malice/docs/wire-format.md).

## Goals

- Allow a new implant instance to register with the teamserver.
- Allow the implant to send periodic heartbeat packets.
- Allow the operator to list known implants and their last heartbeat timestamps.
- Allow the operator to queue one object file payload for an implant.
- Allow the implant to fetch that payload, execute it through `coff_loader`, and return stdout back to the teamserver.
- Keep the current opcode + UTF-8 JSON framing for the first interop milestone.

## Non-Goals For This Phase

- Multi-object jobing or concurrent task pipelines.
- Full binary protocol redesign.
- Reliable delivery guarantees beyond simple queue semantics.
- Encrypted transport or message signing.
- Generic BOF/COFF argument marshaling.

## High-Level Design

The teamserver remains the system of record. The implant is a polling client.

Core components:

- `packet` layer
  - owns wire encode/decode
  - remains the protocol boundary
- `httpserver` layer
  - exposes one implant-facing endpoint
  - reads implant packets and forwards them to a packet router
- packet router
  - dispatches by opcode to the correct handler
  - keeps route knowledge out of the implant
- implant registry
  - new state store for known implants and liveness metadata
- task queue
  - existing queue concept, extended to carry one object payload task
- operator command layer
  - new CLI commands for listing implants and queueing the initial `whoami` payload
- `coff_loader` implant
  - registers
  - heartbeats
  - polls for tasking
  - executes the delivered COFF object
  - posts result data back

## Proposed Message Types

The current code already defines these opcodes in [`src/util/packet.rs`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs):

- `0x00` `REGISTER`
- `0x01` `FETCH_TASK`
- `0x02` `TASK_RESULT`

To support heartbeat cleanly without overloading registration, add:

- `0x03` `HEARTBEAT`

This is the least disruptive extension because it preserves the current framing and keeps behavior explicit.

## Logical Schemas

The outer packet remains:

```json
{
  "clientid": "<uuid string>",
  "data": "<json string>"
}
```

The inner payload schemas below describe the content of `data`.

### 1. Registration

Opcode: `REGISTER` (`0x00`)

Implant to teamserver:

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

Responsibilities:

- implant generates or persists a stable `clientid`
- implant sends a registration packet on startup
- teamserver creates or updates implant state
- teamserver sets:
  - `first_seen`
  - `last_seen`
  - `last_heartbeat`
  - current metadata snapshot

### 2. Heartbeat

Opcode: `HEARTBEAT` (`0x03`)

Implant to teamserver:

```json
{
  "sequence": 7,
  "status": "idle"
}
```

Responsibilities:

- implant sends this on a fixed interval
- teamserver updates `last_seen` and `last_heartbeat`
- teamserver does not create a new task as a side effect

Notes:

- `status` can remain simple for now: `idle`, `busy`, or `error`
- `sequence` is optional operational metadata but helps detect missed packets during debugging

### 3. Fetch Task

Opcode: `FETCH_TASK` (`0x01`)

Implant to teamserver:

```json
{
  "want": 1
}
```

Teamserver to implant:

```json
{
  "tasks": [
    {
      "task_id": "d4fcb0c2-3414-4e11-9a2f-b9f0d86c4377",
      "task_type": "execute_coff",
      "object_name": "whoami.obj",
      "entrypoint": "go",
      "object_encoding": "base64",
      "object_data": "<base64 object bytes>",
      "args_encoding": "utf8",
      "args_data": ""
    }
  ]
}
```

Design choice:

- even though only one task is supported initially, return a `tasks` array now
- this fixes the current invalid multi-task concatenation problem in [`src/util/httpserver.rs`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs)
- the first implementation can enforce `tasks.len() <= 1`

### 4. Task Result

Opcode: `TASK_RESULT` (`0x02`)

Implant to teamserver:

```json
{
  "task_id": "d4fcb0c2-3414-4e11-9a2f-b9f0d86c4377",
  "status": "success",
  "result_encoding": "utf8",
  "result_data": "host01\\alice\r\n"
}
```

Failure result:

```json
{
  "task_id": "d4fcb0c2-3414-4e11-9a2f-b9f0d86c4377",
  "status": "error",
  "result_encoding": "utf8",
  "result_data": "entrypoint go not found"
}
```

Responsibilities:

- implant posts exactly one result record for each delivered task
- teamserver stores the latest result and makes it visible to the operator

## Teamserver State Model

Add an implant registry separate from the task queue.

Suggested logical model:

```text
ImplantRecord
- clientid: Uuid
- implant_type: String
- protocol_version: u32
- hostname: String
- username: String
- pid: u32
- process_name: String
- os: String
- arch: String
- first_seen: SystemTime
- last_seen: SystemTime
- last_heartbeat: SystemTime
- status: String
```

Suggested task model for this phase:

```text
TaskRecord
- task_id: Uuid
- clientid: Uuid
- task_type: ExecuteCoff
- object_name: String
- object_bytes: Vec<u8>
- entrypoint: String
- args: Vec<u8>
- queued_at: SystemTime
- delivered_at: Option<SystemTime>
- completed_at: Option<SystemTime>
- status: Queued | Delivered | Completed | Failed
- result: Option<String>
```

Separation matters:

- implant registry answers liveness and inventory queries
- task queue answers pending work and result tracking
- heartbeat updates should not mutate queued work

## HTTP/API Shape

The current server already uses `/tasks` in [`src/util/httpserver.rs`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs). For interop, change that model: the implant should only know one URL, and the teamserver should dispatch by opcode.

Recommended implant-facing route:

- `POST /packet`
  - request body is the raw opcode-prefixed packet
  - the teamserver parses the outer packet once
  - the teamserver dispatches to the correct handler based on `opcode`

Reasoning:

- the implant only has to know one path
- all packet classification lives on the server side
- the protocol boundary remains the packet format rather than the HTTP route map
- adding a new opcode later does not require implant route changes

Recommended operator-facing routes:

- none required for the first milestone

The operator interacts through the CLI, not over HTTP.

### Packet Router Contract

Add a central dispatch function that takes a parsed packet and returns the handler response:

```text
route_packet(packet) -> response
```

Dispatch table:

- `REGISTER` -> `handle_register`
- `HEARTBEAT` -> `handle_heartbeat`
- `FETCH_TASK` -> `handle_fetch_task`
- `TASK_RESULT` -> `handle_task_result`
- unknown opcode -> reject and log

This should become the authoritative teamserver control flow for implant traffic.

## Operator Commands

The CLI in [`src/util/command.rs`](/C:/Users/wammu/source/repos/malice/src/util/command.rs) currently only supports `tcpserver start|stop`.

Add these commands:

- `implants list`
  - prints `clientid`, `hostname`, `username`, `process_name`, `last_heartbeat`, `status`
- `task whoami <clientid>`
  - reads the local `whoami.obj`
  - queues one `execute_coff` task for that implant
- `task result <task_id>`
  - prints stored result text for a completed task

Optional but useful:

- `implants info <clientid>`
  - prints the full registration metadata for one implant

## End-To-End Execution Flow

### 1. Implant startup

1. implant generates or loads `clientid`
2. implant sends a `REGISTER` packet to `POST /packet`
3. teamserver creates or refreshes `ImplantRecord`
4. operator can now see the implant in `implants list`

### 2. Liveness maintenance

1. implant sends `HEARTBEAT` every fixed interval, for example every 30 seconds
2. teamserver updates `last_heartbeat`
3. `implants list` shows current heartbeat age

### 3. Queue `whoami.obj`

1. operator runs `task whoami <clientid>`
2. teamserver loads local `whoami.obj`
3. teamserver creates a queued `TaskRecord`

### 4. Implant task poll

1. implant sends `FETCH_TASK`
2. teamserver finds the oldest queued task for that `clientid`
3. teamserver returns one `execute_coff` task with base64 object bytes
4. teamserver marks the task as `Delivered`

### 5. Execution

1. implant decodes base64 object bytes
2. implant passes the object into `coff_loader`
3. the object executes `whoami`
4. implant captures stdout or the loader-returned output buffer

### 6. Result upload

1. implant sends `TASK_RESULT`
2. teamserver marks the task `Completed` or `Failed`
3. operator retrieves the result with `task result <task_id>`

## Initial Payload Design: `whoami.obj`

For the first milestone, use one fixed payload:

- file name: `whoami.obj`
- purpose: execute `whoami` on the implant host
- arguments: none
- expected output: current security context, typically `hostname\\username`

Why this is the right first task:

- simple operator-visible success condition
- exercises object delivery, execution, output capture, and result return
- minimal argument-marshaling complexity

## Encoding Choice For The Object File

Use base64 inside `object_data` for the first implementation.

Reasoning:

- the current packet format is JSON-string-based and not binary-safe
- base64 keeps the transport compatible with the existing framing
- it is easy to decode in both Rust and the Windows implant

Deferred improvements:

- explicit payload length
- compression
- chunking for large objects
- binary protocol version for large or frequent transfers

## Error Handling Rules

Registration:

- if `clientid` is invalid, reject and log
- if metadata fields are missing, reject with a parse error for now rather than silently defaulting

Heartbeat:

- if the implant is unknown, either:
  - reject and require re-registration, or
  - auto-create a minimal record and mark it incomplete

Recommended first behavior: reject unknown heartbeats and require re-registration. It is easier to reason about.

Task delivery:

- if no task is queued, return `{"tasks":[]}`
- do not return concatenated JSON objects

Task result:

- if `task_id` is unknown, log and reject
- if a duplicate result is posted, prefer idempotent update with a warning rather than panicking

## Persistence Strategy

For the first milestone, in-memory state is acceptable:

- implant registry in memory
- task queue in memory
- results in memory

Constraint:

- restarting the teamserver loses implant inventory and task/result history

That is acceptable for a first interop milestone, but the data model should be written as if persistence will be added later.

## Required Code Changes

### Teamserver

- extend [`src/util/packet.rs`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs)
  - add `HEARTBEAT`
  - add typed inner payload parsing helpers
  - add helpers for parsing the inner payload by opcode
- add an implant registry module
  - likely `src/util/implants.rs`
- add a packet router module
  - likely `src/util/router.rs`
  - centralize opcode-to-handler dispatch
- extend [`src/util/tasks.rs`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs)
  - replace plain string tasks with typed task records
  - return structured task arrays
- extend [`src/util/httpserver.rs`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs)
  - replace per-operation implant routes with `POST /packet`
  - parse the raw packet body once
  - call the packet router
- extend [`src/util/command.rs`](/C:/Users/wammu/source/repos/malice/src/util/command.rs)
  - add implant listing
  - add whoami task queueing
  - add result retrieval
- update [`src/util.rs`](/C:/Users/wammu/source/repos/malice/src/util.rs)
  - export the implant registry module
  - export the packet router module

### Implant

- add persistent or startup-generated `clientid`
- add a single packet sender targeting `POST /packet`
- add registration sender
- add heartbeat loop
- add task polling loop
- add base64 decode for object bytes
- add `coff_loader` execution path
- add result upload path

### Object payload

- create or add `whoami.obj` build input
- define the expected entrypoint name
- define how stdout is captured and surfaced back through the implant

## Suggested Implementation Order

1. Implement the implant registry and `implants list`.
2. Add `REGISTER` handling end to end.
3. Add `HEARTBEAT` handling and last-seen reporting.
4. Replace raw string task concatenation with structured task responses.
5. Add `task whoami <clientid>` queueing on the teamserver.
6. Add task polling and result upload in the implant.
7. Add result inspection command on the teamserver.

This order gives operator visibility first, then liveness, then tasking, then execution.

## Acceptance Criteria

- starting the implant creates a visible record in `implants list`
- `implants list` shows a changing `last_heartbeat`
- queueing `task whoami <clientid>` creates exactly one pending task
- the implant receives `whoami.obj` and executes it
- the teamserver stores the returned output
- the operator can read the returned `whoami` output from the CLI
