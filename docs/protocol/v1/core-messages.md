# Protocol V1 Core Messages

This directory is the versioned home for the shared message shapes used by the Rust teamserver and implant runtimes.

For protocol version `1`, the core message set is:

- `RegisterPayload`
  - `implant_type`
  - `protocol_version`
  - `hostname`
  - `username`
  - `pid`
  - `process_name`
  - `os`
  - `arch`
- `HeartbeatPayload`
  - `sequence`
  - `status`
- `FetchTaskRequest`
  - `want`
- `FetchTaskResponse`
  - `tasks`
- `TaskResultPayload`
  - `task_id`
  - `status`
  - `result_encoding`
  - `result_data`

Core-owned fields:

- packet opcode
- outer `clientid`
- registration identity fields
- heartbeat liveness fields
- task lifecycle identifiers
- result status and encoding

Integration-owned fields:

- supported `implant_type` values
- supported protocol versions for a family
- concrete task catalog exposed to operators
- concrete task envelope semantics for a runtime
- artifact lookup conventions and argument packing

The normative framing rules still live in [docs/wire-format.md](/C:/Users/wammu/source/repos/malice/docs/wire-format.md). This folder exists so the core packet shapes have a versioned home separate from any single implant family.
