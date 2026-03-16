# Modular Architecture Guide

This document describes the current teamserver architecture as implemented today.

It is for developers who want to:

- add a new implant family
- add a new task type
- add a new opcode handler
- extend the operator UI or CLI without pushing family logic back into the core

## Design Goals

The current architecture is organized around these goals:

- keep transport code transport-only
- keep implant-family specifics behind integrations
- keep task queueing typed and capability-aware
- keep operator commands modular
- keep packet handling split by opcode
- keep artifact lookup separate from integration logic

## High-Level Layout

Current top-level teamserver modules:

- [app.rs](/C:/Users/wammu/source/repos/malice/src/core/app.rs)
  - shared `ServerContext`
- [httpserver.rs](/C:/Users/wammu/source/repos/malice/src/core/httpserver.rs)
  - HTTP transport shim
- [admission.rs](/C:/Users/wammu/source/repos/malice/src/core/admission.rs)
  - request admission policy
- [packet.rs](/C:/Users/wammu/source/repos/malice/src/core/packet.rs)
  - packet framing and decode helpers
- [router](/C:/Users/wammu/source/repos/malice/src/core/router)
  - opcode router and handlers
- [implants](/C:/Users/wammu/source/repos/malice/src/core/implants)
  - implant identity, runtime state, registry, capabilities
- [tasks](/C:/Users/wammu/source/repos/malice/src/core/tasks)
  - generic task queueing, repository, transport envelopes, results
- [payloads.rs](/C:/Users/wammu/source/repos/malice/src/core/payloads.rs)
  - generic artifact lookup
- [integrations](/C:/Users/wammu/source/repos/malice/src/core/integrations)
  - manifest-backed implant integrations
- [command](/C:/Users/wammu/source/repos/malice/src/core/command)
  - CLI parsing, dispatch, and output
- [ui](/C:/Users/wammu/source/repos/malice/src/ui)
  - `ratatui` UI and integration-driven actions

## Core Request Flow

An implant request currently flows like this:

1. `HttpServer` accepts `POST /packet`
2. the body is parsed into `Packet`
3. admission policy validates the request
4. `PacketRouter` dispatches by `PacketOpcode`
5. the handler uses `ServerContext`
6. the handler returns `PacketReply`
7. HTTP converts `PacketReply` into the response

That boundary matters because implant-specific behavior should live in integrations and handlers, not in the HTTP shim.

## ServerContext

[app.rs](/C:/Users/wammu/source/repos/malice/src/core/app.rs) is the service container.

`ServerContext` currently owns:

- `ImplantRegistry`
- `TaskService`
- `PayloadRepository`
- `ImplantIntegrationRegistry`
- `PacketRouter`
- `ActivityLog`
- `AdmissionPolicy`

`ServerContext` is where generic server behavior coordinates:

- registration
- task queueing
- task fetch serialization
- task result decoding
- UI-facing integration metadata

## Transport Layer

[httpserver.rs](/C:/Users/wammu/source/repos/malice/src/core/httpserver.rs) is intentionally thin.

Its responsibilities are:

- bind the socket
- accept HTTP requests
- enforce `POST /packet`
- read the request body
- parse `Packet`
- build request context for admission
- hand the packet to the router
- return the router response

It should not:

- resolve artifacts
- build task specs
- decode task results
- assign implant capabilities

## Admission Layer

[admission.rs](/C:/Users/wammu/source/repos/malice/src/core/admission.rs) owns request-level policy checks.

Today it contains:

- `PacketRequestContext`
- `AdmissionPolicy`
- `RegisterHeaderPolicy`

That separation keeps registration policy out of the HTTP transport and makes future policy changes local.

## Packet Layer

[packet.rs](/C:/Users/wammu/source/repos/malice/src/core/packet.rs) owns:

- `Packet`
- `PacketOpcode`
- packet encode/decode
- inner payload deserialization

If you add a new implant message type:

1. add a new `PacketOpcode`
2. add a handler
3. register the handler in the router

## Packet Router

The router lives under [router](/C:/Users/wammu/source/repos/malice/src/core/router).

Important files:

- [registry.rs](/C:/Users/wammu/source/repos/malice/src/core/router/registry.rs)
- [reply.rs](/C:/Users/wammu/source/repos/malice/src/core/router/reply.rs)
- [register.rs](/C:/Users/wammu/source/repos/malice/src/core/router/handlers/register.rs)
- [heartbeat.rs](/C:/Users/wammu/source/repos/malice/src/core/router/handlers/heartbeat.rs)
- [fetch_task.rs](/C:/Users/wammu/source/repos/malice/src/core/router/handlers/fetch_task.rs)
- [task_result.rs](/C:/Users/wammu/source/repos/malice/src/core/router/handlers/task_result.rs)

Pattern:

- router maps opcode to handler
- each handler parses its own payload
- handlers talk to `ServerContext`
- handlers return `PacketReply`

## Implant Model

The implant model lives under [implants](/C:/Users/wammu/source/repos/malice/src/core/implants).

Important types:

- `ImplantRecord`
- `ImplantIdentity`
- `ImplantStaticMetadata`
- `ImplantRuntimeState`
- `ImplantCapability`
- `ImplantRegistry`

The split is:

- identity
  - who the implant is
- static metadata
  - host and process details
- capabilities
  - capability keys declared by the integration
- runtime state
  - liveness, status, and active task

### Implant families

`ImplantFamily` in [types.rs](/C:/Users/wammu/source/repos/malice/src/core/implants/types.rs) is now a coarse family classifier, not the source of truth for task catalogs or capabilities.

Current variants:

- `ImplantFamily::CoffLoader`
- `ImplantFamily::Unknown(String)`

The detailed capability set now comes from the selected integration, typically through its manifest.

## Integrations

The integration layer lives under [integrations](/C:/Users/wammu/source/repos/malice/src/core/integrations).

Important files:

- [types.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/types.rs)
- [manifest.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/manifest.rs)
- [package.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/package.rs)
- [plugin_api.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/plugin_api.rs)
- [worker.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/worker.rs)
- [loaded.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/loaded.rs)
- [registry.rs](/C:/Users/wammu/source/repos/malice/src/core/integrations/registry.rs)
- [manifest.json](/C:/Users/wammu/source/repos/malice/implant/zant/plugin/manifest.json)
- [plugin.json](/C:/Users/wammu/source/repos/malice/implant/zant/plugin/plugin.json)

An integration owns:

- accepted `implant_type`
- supported protocol versions
- capability keys
- task catalog
- UI action metadata
- task building
- task envelope serialization
- result decoding

This is the main boundary that keeps implant-family logic out of the core.

### Adding a new implant family

To add a new family safely:

1. create a plugin package under the implant repo, for example `implant/<name>/plugin/`
2. add `plugin.json` plus `manifest.json` to that package
3. package any artifacts under the plugin package, typically `artifacts/`
4. implement the runtime behavior behind the host integration boundary
5. install the package with `plugins install <path>`
6. activate it with `plugins activate <id> [version]`
7. make the implant register with the matching `implant_type`

You should not need to edit the HTTP transport layer.

Today there is still one transitional limitation:

- package installation and activation are dynamic
- runtime behavior is now provided by worker-backed plugins launched from the active package
- the first real worker-backed plugin is `zant`, so the public plugin authoring path is still early and lightly tooled

## Capability Model

Capabilities are now dynamic keys, not a closed enum of built-ins.

Important current behavior:

- manifests can declare `execute_coff` and arbitrary custom capability keys
- capability values are preserved on the implant record
- task validation compares required capability keys to the implant's declared capability keys

That means integrations can describe new capabilities without changing a core task enum.

## Task System

The task domain lives under [tasks](/C:/Users/wammu/source/repos/malice/src/core/tasks).

Important files:

- [types.rs](/C:/Users/wammu/source/repos/malice/src/core/tasks/types.rs)
- [queue.rs](/C:/Users/wammu/source/repos/malice/src/core/tasks/queue.rs)
- [repository.rs](/C:/Users/wammu/source/repos/malice/src/core/tasks/repository.rs)

Important types:

- `QueuedTask`
- `TaskRecord`
- `TaskEnvelope`
- `TaskStatus`
- `TaskResultPayload`
- `TaskResultData`

The core task layer now owns:

- queue-time capability validation
- generic persisted metadata such as `task_kind`
- opaque integration-owned task state
- transport-neutral task lifecycle state

That capability check is enforced before queueing in [queue.rs](/C:/Users/wammu/source/repos/malice/src/core/tasks/queue.rs), but the core no longer defines concrete execution variants.

### Important current boundary

The task repository/service no longer owns task-envelope serialization or result decoding.

Those responsibilities now live in the selected implant integration.

### Adding a new execution model

To add a new execution model:

1. define the operator-facing task catalog and capability keys in the integration manifest
2. add the integration-owned task-state struct in that integration module
3. build a `QueuedTask` with the required capability and opaque state
4. serialize the implant-facing envelope in that integration
5. decode results in that integration
6. add tests for capability validation and round-trip behavior

Avoid adding bespoke queue methods for each task kind. Keep queueing generic and let integrations own execution semantics.

## Artifact Repository

[payloads.rs](/C:/Users/wammu/source/repos/malice/src/core/payloads.rs) is now a generic artifact source.

Current behavior:

- caller provides a logical artifact name plus search roots
- repository resolves the file on disk
- repository returns the file name and bytes

It does not:

- define task catalogs
- define argument packing rules
- know about one implant family

Those behaviors belong in the integration layer.

## CLI Architecture

The CLI lives under [command](/C:/Users/wammu/source/repos/malice/src/core/command).

Important files:

- [parser.rs](/C:/Users/wammu/source/repos/malice/src/core/command/parser.rs)
- [dispatcher.rs](/C:/Users/wammu/source/repos/malice/src/core/command/dispatcher.rs)
- [output.rs](/C:/Users/wammu/source/repos/malice/src/core/command/output.rs)
- [commands/mod.rs](/C:/Users/wammu/source/repos/malice/src/core/command/commands/mod.rs)

The split is:

- parser
  - raw text to typed commands
- dispatcher
  - typed commands to handlers
- output
  - formatting
- command handlers
  - domain behavior

Current generic task command:

```text
task queue <clientid> <task-kind> [args...]
```

The key point is that operator-facing task kinds are resolved through the selected integration instead of being hard-coded in the command layer.

## UI Architecture

The `ratatui` UI lives under [ui](/C:/Users/wammu/source/repos/malice/src/ui).

Important current behavior:

- selected implant actions are driven by integration metadata
- task menu entries come from the integration manifest and module
- the UI no longer hard-codes family-specific commands like `whoami`

That keeps the operator surface aligned with the selected implant family without pushing those definitions into the core UI.

## How To Implement A New Implant

If you are writing a new implant, treat these as the contract:

- [wire-format.md](/C:/Users/wammu/source/repos/malice/docs/wire-format.md)
- [core-messages.md](/C:/Users/wammu/source/repos/malice/docs/protocol/v1/core-messages.md)
- [packet.rs](/C:/Users/wammu/source/repos/malice/src/core/packet.rs)
- [integration types](/C:/Users/wammu/source/repos/malice/src/core/integrations/types.rs)

At a minimum, an implant should be able to:

1. persist or generate a `clientid`
2. send `REGISTER`
3. send `HEARTBEAT`
4. poll with `FETCH_TASK`
5. parse returned tasks
6. execute or emulate supported tasks
7. return `TASK_RESULT`

## Recommended Extension Workflow

When extending the system, prefer this order:

1. define capability keys and task catalog in the integration manifest
2. define or update the Rust integration
3. add or update the integration-owned task state for that runtime
4. add or update packet handlers
5. add UI and command polish last

That order keeps the operator surface thin and keeps new family work localized.

## Guardrails

Preserve these invariants:

- do not put artifact resolution into `HttpServer`
- do not put registration policy checks into the transport shim
- do not queue tasks without capability validation
- do not move family-specific task logic back into the core UI or command parser
- do not hard-code one implant family's task catalog into the payload repository

## Current Limitations

The current architecture is still intentionally small in scope:

- only one integration-owned execution model is implemented today: `zant`'s COFF task envelope
- result typing is still simple and largely text-oriented
- plugin packages are worker-backed over a JSON stdio contract rather than a richer SDK or sandboxed runtime
- persistence is still in-memory
- admission policy is still simple

That is acceptable for the current stage. The important part is that new family work is now additive and mostly integration-scoped instead of transport-scoped.
