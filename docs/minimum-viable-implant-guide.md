# Minimum Viable Implant Guide

This guide walks through the smallest useful implant you can build for the current `malice` platform.

It is written against the current codebase, where:

- the teamserver owns core orchestration
- implant-family specifics are delivered as installable plugin packages
- integration metadata is declared in a package-local manifest
- runtime execution logic is delivered by a package-local plugin worker declared in `plugin.json`

Use this document when you want to:

- create a stub implant that can talk to the teamserver
- understand the packet protocol
- understand what the teamserver expects from a new implant family
- see how the current `zant` integration is attached

## What "Minimum Viable Implant" Means Here

A minimum viable implant for `malice` does not need to be feature-rich.

It only needs to do these things correctly:

1. persist or generate a `clientid`
2. send `REGISTER`
3. send periodic `HEARTBEAT`
4. send `FETCH_TASK`
5. parse the task response
6. optionally fake or execute the task
7. send `TASK_RESULT`

That is enough to validate:

- protocol compatibility
- server-side integration attachment
- implant lifecycle handling
- task delivery and result flow

For a first stub, it is acceptable to return canned output instead of actually executing payloads.

## Current Architecture Boundary

The current platform has three parts:

1. Teamserver core
2. Server-side implant integration
3. Implant runtime

### Teamserver core

The teamserver core owns:

- implant registry
- capability-aware task queue
- packet routing
- result storage
- operator UI and commands
- admission policy

Relevant files:

- [src/core/app.rs](/C:/Users/wammu/source/repos/malice/src/core/app.rs)
- [src/core/router](/C:/Users/wammu/source/repos/malice/src/core/router)
- [src/core/tasks](/C:/Users/wammu/source/repos/malice/src/core/tasks)
- [src/core/httpserver.rs](/C:/Users/wammu/source/repos/malice/src/core/httpserver.rs)

### Server-side implant integration

Each implant family should have a server-side integration that knows:

- which `implant_type` string it accepts
- which protocol versions it supports
- which capabilities it exposes
- which task kinds it offers
- how to build queued task metadata plus integration-owned task state
- how to serialize implant-facing task envelopes
- how to decode task results

Relevant files:

- [src/core/integrations/types.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/types.rs)
- [src/core/integrations/registry.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/registry.rs)
- [src/core/integrations/loaded.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/loaded.rs)
- [src/core/integrations/worker.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/worker.rs)
- [src/core/integrations/plugin_api.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/plugin_api.rs)

### Integration manifest

Each integration can now declare its metadata in a manifest.

Current example:

- [implant/zant/plugin/manifest.json](/C:/Users/wammu/source/repos/malice/implant/zant/plugin/manifest.json)

That manifest currently holds:

- `id`
- `implant_type`
- `family`
- `protocol_versions`
- `capabilities`
- `artifact_roots`
- `tasks`
- `ui_actions`

Capability values are now extensible. Manifest entries can declare `execute_coff` and any custom capability keys such as:

- `run_process`
- `download_file`
- `screenshot`

All of those values are preserved in implant records without requiring a core code change.

### Implant runtime

The implant runtime owns:

- HTTP transport client code
- packet encode/decode
- task polling
- task execution or emulation
- result upload

For the current `zant` runtime, see:

- [implant/zant/runtime/protocol.h](/C:/Users/wammu/source/repos/malice/implant/zant/runtime/protocol.h)
- [implant/zant/runtime/protocol.cpp](/C:/Users/wammu/source/repos/malice/implant/zant/runtime/protocol.cpp)
- [implant/zant/runtime/runtime.cpp](/C:/Users/wammu/source/repos/malice/implant/zant/runtime/runtime.cpp)

## The Smallest End-To-End Path

If you are creating a new implant family from scratch, the smallest useful path is:

1. create a runtime that can register, heartbeat, fetch, and post results
2. create a plugin package for that runtime
3. create a manifest for that integration
4. install and activate the package through the teamserver
5. test with one fake or real task

If you want the fastest first success, make the first task a no-op or echo-style task:

- receive one task
- do not execute anything
- return `"stub implant executed task"`

That proves the platform boundary before you add real execution logic.

## Packet Protocol

The current wire format is defined in:

- [docs/wire-format.md](/C:/Users/wammu/source/repos/malice/docs/wire-format.md)
- [docs/protocol/v1/core-messages.md](/C:/Users/wammu/source/repos/malice/docs/protocol/v1/core-messages.md)
- [src/core/packet.rs](/C:/Users/wammu/source/repos/malice/src/core/packet.rs)

### Outer packet shape

Each packet is:

- 1 byte of opcode
- followed by UTF-8 JSON

The JSON body is the outer envelope:

```json
{
  "clientid": "513a666c-3349-40dd-9462-95c4449b0d0d",
  "data": "{\"want\":1}"
}
```

Important detail:

- `data` is a JSON string, not a nested JSON object

### Current opcodes

From [src/core/packet.rs](/C:/Users/wammu/source/repos/malice/src/core/packet.rs):

- `0x00` `Register`
- `0x01` `FetchTask`
- `0x02` `TaskResult`
- `0x03` `Heartbeat`

### Core payloads

#### Register

```json
{
  "implant_type": "coff_loader",
  "protocol_version": 1,
  "hostname": "HOST01",
  "username": "alice",
  "pid": 1234,
  "process_name": "stub.exe",
  "os": "windows",
  "arch": "x64"
}
```

#### Heartbeat

```json
{
  "sequence": 1,
  "status": "idle"
}
```

#### Fetch task request

```json
{
  "want": 1
}
```

#### Task result

```json
{
  "task_id": "d4fcb0c2-3414-4e11-9a2f-b9f0d86c4377",
  "status": "success",
  "result_encoding": "utf8",
  "result_data": "stub implant executed task"
}
```

## HTTP Behavior

The implant-facing route is:

- `POST /packet`

The teamserver listens by default on:

- `127.0.0.1:42069`

For registration, the current admission policy also expects this header:

- `x-malice-register: coff-loader-v1`

That policy currently lives in:

- [src/core/admission.rs](/C:/Users/wammu/source/repos/malice/src/core/admission.rs)

If you create a second implant family, decide whether:

- it should use the same admission policy, or
- you should generalize that header/policy further

## Building A Stub Implant

This section describes the smallest implementation shape.

### Step 1: Pick your implant identity

Choose:

- `implant_type`
- protocol version
- whether you are reusing an existing family or creating a new one

If you want a brand-new family, pick a new `implant_type` string such as:

- `stub_loader`

That string becomes the attachment key between runtime and integration.

### Step 2: Create the runtime project

A minimum runtime needs these modules:

```text
runtime/
- config
- protocol
- transport
- runtime_loop
- tasks
```

Responsibilities:

- `config`
  - server URL
  - persisted `clientid` path
- `protocol`
  - opcode definitions
  - packet builders
  - packet parsers
- `transport`
  - `POST /packet`
- `runtime_loop`
  - register
  - heartbeat loop
  - fetch-task loop
- `tasks`
  - dispatch received tasks

### Step 3: Persist or generate a client ID

The implant should either:

- load a prior UUID from disk, or
- generate one once and persist it

If you do not persist it, every restart will appear as a new implant to the teamserver.

### Step 4: Implement packet builders

You need helpers equivalent to:

```text
build_register_packet(clientid, payload)
build_heartbeat_packet(clientid, payload)
build_fetch_task_packet(clientid, payload)
build_task_result_packet(clientid, payload)
```

The current `zant` implementation is a good reference:

- [implant/zant/runtime/protocol.h](/C:/Users/wammu/source/repos/malice/implant/zant/runtime/protocol.h)

### Step 5: Send `REGISTER`

On startup:

1. gather host/process metadata
2. build a register packet
3. `POST` it to `/packet`
4. read the response

For the current server behavior:

- if the outer `clientid` is empty, the server issues a new one
- if the outer `clientid` is populated, the server expects it to already exist

So for a brand-new runtime, the usual first registration path is:

- send an empty outer `clientid`
- store the returned `clientid`

### Step 6: Send heartbeat on an interval

Heartbeat should:

- run on a timer
- carry a monotonically increasing `sequence`
- report a simple status such as `idle`, `busy`, or `error`

### Step 7: Poll for tasks

On an interval:

1. send `FETCH_TASK`
2. parse `FetchTaskResponse`
3. if `tasks` is empty, sleep and continue
4. if a task is present, dispatch it

### Step 8: Implement a fake executor first

A stub implant does not need real payload execution.

You can start with:

```text
if task.task_type is supported:
    return success("stub implant executed " + task.task_type)
else:
    return error("unsupported task type")
```

That lets you validate:

- task queueing
- task fetch
- result upload
- operator visibility

### Step 9: Upload `TASK_RESULT`

After dispatch:

1. build a result payload
2. preserve the `task_id`
3. post the result packet

This is enough for the teamserver to mark the task completed or failed.

## Creating The Server-Side Integration

If your implant is brand new, you need a server-side integration in addition to the runtime.

### Step 1: Add a manifest

Create a plugin package directory such as:

```text
implant/stub/plugin/
  plugin.json
  manifest.json
  bin/
  artifacts/
```

A minimal example:

`plugin.json`

```json
{
  "package_id": "malice.integration.stub",
  "plugin_id": "stub",
  "version": "0.1.0",
  "plugin_api_version": 1,
  "runtime": {
    "type": "stdio",
    "command": ["bin/plugin.exe"]
  },
  "manifest_path": "manifest.json",
  "min_core_version": "0.1.0",
  "artifact_roots": ["artifacts"]
}
```

`manifest.json`

```json
{
  "schema_version": 2,
  "plugin_api_version": 1,
  "id": "stub",
  "implant_type": "stub_loader",
  "family": "stub_loader",
  "protocol_versions": [1],
  "capabilities": ["run_process", "screenshot"],
  "artifact_roots": ["payloads"],
  "tasks": [
    {
      "kind": "whoami",
      "usage": "whoami",
      "artifact": "whoami",
      "entrypoint": "main",
      "arg_mode": { "type": "none" }
    }
  ],
  "ui_actions": [
    {
      "id": "queue_whoami",
      "label": "queue whoami now",
      "task_kind": "whoami",
      "args_template": [],
      "command_template": null,
      "queue_immediately": true
    }
  ]
}
```

Notes:

- `capabilities` can contain `execute_coff` and arbitrary custom values.
- custom capabilities are useful for describing what the implant can do without adding a new core task enum variant.
- queue validation compares the queued task's required capability key to the implant record; execution semantics stay inside the integration.

### Step 2: Add the plugin worker project

Create a sibling source tree for the worker runtime, separate from the install package:

```text
implant/stub/
  plugin/
    plugin.json
    manifest.json
    bin/
      plugin.exe
    artifacts/
  plugin-src/
    Cargo.toml
    src/
      main.rs
```

The worker should implement the JSON operations described in [internal-docs/plugin-api-v1-design.md](/C:/Users/wammu/source/repos/malice/internal-docs/plugin-api-v1-design.md):

- `get_manifest`
- `validate_registration`
- `build_task`
- `serialize_task`
- `decode_result`

The teamserver handles package install, activation, process management, artifact lookup, and task persistence. The worker only implements implant-family-specific behavior.

Capability note:

- `manifest.capabilities()` now accepts any capability string
- `execute_coff` is declared and stored the same way as every other capability key
- task validation compares capabilities by key rather than by a special enum variant or task enum

### Step 3: Build the worker into the package

Build the worker under `plugin-src/` and place the runnable binary in `plugin/bin/`.

The install package should only contain files required at install and runtime:

- `plugin.json`
- `manifest.json`
- `bin/plugin.exe`
- `artifacts/`

Do not ship Cargo sources or `target/` inside the install package.

### Step 4: Install and activate the package

With the package laid out under the implant repo, load it through the operator interface:

```text
plugins install implant/stub/plugin
plugins activate stub 0.1.0
```

The teamserver copies the package into its local plugin store and reloads the active integration registry from there.

## A Very Small Runtime Loop

Pseudo-code:

```text
load_or_create_clientid()

if clientid unknown to server:
    register()
else:
    try register with existing id

spawn heartbeat loop

loop:
    response = fetch_task(want=1)
    if response.tasks empty:
        sleep
        continue

    for task in response.tasks:
        result = dispatch(task)
        post_result(task.task_id, result)
```

That is enough for a minimal first implant.

## Recommended First Task Strategy

There are two good first strategies.

### Strategy A: Fake execution

Return canned text for every supported task.

Pros:

- fastest first success
- no payload loader required
- great for protocol debugging

Cons:

- does not prove artifact execution

### Strategy B: One real task

Support one real task only, such as:

- `whoami`

Pros:

- validates the full execution path

Cons:

- more runtime code

For a new family, Strategy A is the better first milestone.

## Testing Checklist

A minimum viable implant is ready when all of these work:

1. `implants list` shows the implant
2. heartbeat updates change liveness
3. `task queue <clientid> <task-kind>` succeeds
4. the implant fetches that task
5. the implant posts a result
6. `task result <task_id>` shows the result

## Common Failure Modes

### Wrong `implant_type`

Symptom:

- registration fails with "no integration registered"

Cause:

- runtime `implant_type` does not match the integration manifest

### Wrong protocol version

Symptom:

- registration rejected by integration validation

Cause:

- runtime `protocol_version` not listed in the manifest

### Missing registration header

Symptom:

- registration gets `403`

Cause:

- missing `x-malice-register` header

### Artifact lookup failure

Symptom:

- task queueing fails

Cause:

- integration manifest `artifact_roots` do not contain the expected artifact

### Task kind mismatch

Symptom:

- operator queues a task but integration returns "unknown task kind"

Cause:

- task kind not present in manifest or not handled by the integration module

## How The Protocol Could Be Extended

These are illustrative directions, not implemented requirements.

### 1. Add an explicit protocol message layer

Today packet payload parsing is per-opcode and JSON-string-based.

A future protocol layer could define:

```text
ProtocolMessage
- Register
- Heartbeat
- FetchTask
- TaskResult
- UploadChunk
- DownloadChunk
```

That would make versioning and schema evolution cleaner.

### 2. Split task kinds from transport task types

Today a task often maps directly to the current execution model.

A future model could distinguish:

- operator task kind: `identity.whoami`
- transport task type: `execute_coff`
- runtime executor: `coff_loader`

That would let multiple implant families implement the same logical task differently.

### 3. Add binary-safe payload transport

Today object bytes are base64 inside JSON.

Future options:

- chunked binary packets
- compression
- message authentication
- task leasing acknowledgements

### 4. Add richer result types

Current results are mostly text-oriented.

Future result shapes could include:

- structured process lists
- file download metadata
- screenshot blobs
- typed error codes

## Suggested Developer Workflow For New Implants

1. Build a runtime that only registers and heartbeats.
2. Add fetch-task polling.
3. Add one fake task executor.
4. Add the plugin package under the implant repo.
5. Add the plugin manifest and package metadata.
6. Add the plugin worker source under `plugin-src/`.
7. Build the worker into `plugin/bin/`.
8. Install and activate the package.
9. Validate task round-trip.
10. Replace fake execution with one real executor.
11. Expand task catalog only after the lifecycle is stable.

## Reference Files

Useful starting points in this repo:

- [docs/modular-architecture.md](/C:/Users/wammu/source/repos/malice/docs/modular-architecture.md)
- [docs/wire-format.md](/C:/Users/wammu/source/repos/malice/docs/wire-format.md)
- [docs/protocol/v1/core-messages.md](/C:/Users/wammu/source/repos/malice/docs/protocol/v1/core-messages.md)
- [src/core/integrations/types.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/types.rs)
- [src/core/integrations/registry.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/registry.rs)
- [implant/zant/plugin/manifest.json](/C:/Users/wammu/source/repos/malice/implant/zant/plugin/manifest.json)
- [implant/zant/runtime/protocol.h](/C:/Users/wammu/source/repos/malice/implant/zant/runtime/protocol.h)

## Summary

For the current `malice` platform, a basic implant does not need a complex loader or a large task set.

The minimum useful shape is:

- one runtime loop
- one matching server-side integration
- one manifest
- one successful task round-trip

Build the protocol and lifecycle first. Add execution complexity second.




