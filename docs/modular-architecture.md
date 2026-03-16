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

- [app.rs](/C:/Users/wammu/source/repos/malice/src/util/app.rs)
  - shared `ServerContext`
- [httpserver.rs](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs)
  - HTTP transport shim
- [admission.rs](/C:/Users/wammu/source/repos/malice/src/util/admission.rs)
  - request admission policy
- [packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs)
  - packet framing and decode helpers
- [router](/C:/Users/wammu/source/repos/malice/src/util/router)
  - opcode router and handlers
- [implants](/C:/Users/wammu/source/repos/malice/src/util/implants)
  - implant identity, runtime state, registry, capabilities
- [tasks](/C:/Users/wammu/source/repos/malice/src/util/tasks)
  - typed task specs, queueing, repository, results
- [payloads.rs](/C:/Users/wammu/source/repos/malice/src/util/payloads.rs)
  - generic artifact lookup
- [integrations](/C:/Users/wammu/source/repos/malice/src/util/integrations)
  - manifest-backed implant integrations
- [command](/C:/Users/wammu/source/repos/malice/src/util/command)
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

[app.rs](/C:/Users/wammu/source/repos/malice/src/util/app.rs) is the service container.

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

[httpserver.rs](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs) is intentionally thin.

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

[admission.rs](/C:/Users/wammu/source/repos/malice/src/util/admission.rs) owns request-level policy checks.

Today it contains:

- `PacketRequestContext`
- `AdmissionPolicy`
- `RegisterHeaderPolicy`

That separation keeps registration policy out of the HTTP transport and makes future policy changes local.

## Packet Layer

[packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs) owns:

- `Packet`
- `PacketOpcode`
- packet encode/decode
- inner payload deserialization

If you add a new implant message type:

1. add a new `PacketOpcode`
2. add a handler
3. register the handler in the router

## Packet Router

The router lives under [router](/C:/Users/wammu/source/repos/malice/src/util/router).

Important files:

- [registry.rs](/C:/Users/wammu/source/repos/malice/src/util/router/registry.rs)
- [reply.rs](/C:/Users/wammu/source/repos/malice/src/util/router/reply.rs)
- [register.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/register.rs)
- [heartbeat.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/heartbeat.rs)
- [fetch_task.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/fetch_task.rs)
- [task_result.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/task_result.rs)

Pattern:

- router maps opcode to handler
- each handler parses its own payload
- handlers talk to `ServerContext`
- handlers return `PacketReply`

## Implant Model

The implant model lives under [implants](/C:/Users/wammu/source/repos/malice/src/util/implants).

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

[family.rs](/C:/Users/wammu/source/repos/malice/src/util/implants/family.rs) is now a coarse family classifier, not the source of truth for task catalogs or capabilities.

Current variants:

- `ImplantFamily::CoffLoader`
- `ImplantFamily::Unknown(String)`

The detailed capability set now comes from the selected integration, typically through its manifest.

## Integrations

The integration layer lives under [integrations](/C:/Users/wammu/source/repos/malice/src/util/integrations).

Important files:

- [types.rs](/C:/Users/wammu/source/repos/malice/src/util/integrations/types.rs)
- [manifest.rs](/C:/Users/wammu/source/repos/malice/src/util/integrations/manifest.rs)
- [registry.rs](/C:/Users/wammu/source/repos/malice/src/util/integrations/registry.rs)
- [zant.rs](/C:/Users/wammu/source/repos/malice/src/util/integrations/zant.rs)
- [manifest.json](/C:/Users/wammu/source/repos/malice/integrations/zant/manifest.json)

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

1. create an integration manifest under `integrations/<name>/manifest.json`
2. create a Rust integration module under `src/util/integrations/`
3. implement `ImplantIntegration`
4. register it in `ImplantIntegrationRegistry`
5. make the implant register with the matching `implant_type`
6. add any new `TaskSpec` variants if the family needs new execution models

You should not need to edit the HTTP transport layer.

## Capability Model

Capabilities are now dynamic keys, not a closed enum of built-ins.

Important current behavior:

- manifests can declare `execute_coff` and arbitrary custom capability keys
- capability values are preserved on the implant record
- task validation compares required capability keys to the implant's declared capability keys

That means integrations can describe new capabilities without changing the core type definition.

## Task System

The task domain lives under [tasks](/C:/Users/wammu/source/repos/malice/src/util/tasks).

Important files:

- [types.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/types.rs)
- [queue.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/queue.rs)
- [repository.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/repository.rs)

Important types:

- `TaskSpec`
- `TaskRecord`
- `TaskStatus`
- `TaskResultPayload`
- `TaskResultData`

Current concrete task spec:

- `TaskSpec::ExecuteCoff`

`TaskSpec` currently defines:

- `task_type()`
- `required_capability()`

That capability check is enforced before queueing in [queue.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/queue.rs).

### Important current boundary

The task repository/service no longer owns task-envelope serialization or result decoding.

Those responsibilities now live in the selected implant integration.

### Adding a new task type

To add a new execution model:

1. add a new `TaskSpec` variant
2. add the typed payload struct it needs
3. update `task_type()` and `required_capability()`
4. update any integrations that should build or serialize that task
5. update result decoding for those integrations
6. add tests for capability validation and round-trip behavior

Avoid adding bespoke queue methods for each task kind. Keep queueing generic and let integrations map operator-facing task kinds into `TaskSpec`.

## Artifact Repository

[payloads.rs](/C:/Users/wammu/source/repos/malice/src/util/payloads.rs) is now a generic artifact source.

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

The CLI lives under [command](/C:/Users/wammu/source/repos/malice/src/util/command).

Important files:

- [parser.rs](/C:/Users/wammu/source/repos/malice/src/util/command/parser.rs)
- [dispatcher.rs](/C:/Users/wammu/source/repos/malice/src/util/command/dispatcher.rs)
- [output.rs](/C:/Users/wammu/source/repos/malice/src/util/command/output.rs)
- [server.rs](/C:/Users/wammu/source/repos/malice/src/util/command/commands/server.rs)
- [implants.rs](/C:/Users/wammu/source/repos/malice/src/util/command/commands/implants.rs)
- [tasks.rs](/C:/Users/wammu/source/repos/malice/src/util/command/commands/tasks.rs)

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
- [packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs)
- [types.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/types.rs)
- [integration types](/C:/Users/wammu/source/repos/malice/src/util/integrations/types.rs)

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
3. add or update `TaskSpec` only if a new execution model is needed
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

- only one concrete `TaskSpec` is implemented today: `ExecuteCoff`
- result typing is still simple and largely text-oriented
- integrations are statically linked, not runtime-loaded
- persistence is still in-memory
- admission policy is still simple

That is acceptable for the current stage. The important part is that new family work is now additive and mostly integration-scoped instead of transport-scoped.
