# Modular Architecture Guide

This document explains the current teamserver architecture after the modularity refactor. It is written for developers who want to:

- implement a new implant family
- add a new task type
- add new packet handlers
- extend the operator CLI without turning it back into a monolith

This guide describes the current code, not just the target design.

## Design Goals

The refactor introduced explicit extension seams so that new functionality is additive instead of invasive.

The main goals are:

- transport code should not know implant-specific behavior
- task queueing should be typed and capability-aware
- operator commands should be registered by domain
- payload resolution should be abstracted behind a repository
- packet routing should be split into per-opcode handlers

## High-Level Layout

Current top-level modules:

- [src/util/app.rs](/C:/Users/wammu/source/repos/malice/src/util/app.rs)
  - shared application context
- [src/util/httpserver.rs](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs)
  - HTTP transport only
- [src/util/packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs)
  - wire framing and packet encode/decode
- [src/util/router](/C:/Users/wammu/source/repos/malice/src/util/router/mod.rs)
  - opcode routing and per-opcode handlers
- [src/util/implants](/C:/Users/wammu/source/repos/malice/src/util/implants/mod.rs)
  - implant identity, capabilities, runtime state, registry
- [src/util/tasks](/C:/Users/wammu/source/repos/malice/src/util/tasks/mod.rs)
  - typed task specs, queueing, serialization, results
- [src/util/payloads.rs](/C:/Users/wammu/source/repos/malice/src/util/payloads.rs)
  - payload lookup by logical name
- [src/util/command](/C:/Users/wammu/source/repos/malice/src/util/command/mod.rs)
  - CLI parsing, dispatch, output, and domain handlers

## Core Request Flow

An implant request currently flows like this:

1. `HttpServer` accepts `POST /packet`.
2. The raw request body is parsed into `Packet`.
3. `PacketRouter` looks up a handler by `PacketOpcode`.
4. The handler uses `ServerContext` services such as:
   - implant registry
   - task service
   - payload repository
5. The handler returns a transport-agnostic `PacketReply`.
6. The HTTP layer converts `PacketReply` into an HTTP response.

That separation matters because a new implant family should require changes in implant parsing, capability mapping, task handling, or routing, but not in the HTTP transport shim.

## ServerContext

[src/util/app.rs](/C:/Users/wammu/source/repos/malice/src/util/app.rs) is the main service container.

`ServerContext` currently owns:

- `ImplantRegistry`
- `TaskService`
- `PayloadRepository`
- `PacketRouter`

Use `ServerContext` when new behavior needs shared access to application services. Do not push new business logic down into `HttpServer`.

## Transport Layer

[src/util/httpserver.rs](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs) is intentionally thin.

Its responsibilities are:

- bind and listen on the configured socket
- accept HTTP requests
- enforce `POST /packet`
- read the request body
- parse a `Packet`
- forward the packet to the router
- return the resulting response

It should not:

- locate payload files
- decide whether an implant supports a task
- build implant records
- implement task queueing policy

If a feature proposal requires editing `HttpServer` for anything other than transport mechanics, that is usually a smell.

## Packet Layer

[src/util/packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs) owns the wire framing.

Current important types:

- `Packet`
- `PacketOpcode`

Current responsibilities:

- decode the opcode-prefixed wire format
- expose the client ID from the outer packet
- deserialize the inner payload with `parse_data`
- build response packets with `Packet::build`

If you need a new implant message type, start by adding a new `PacketOpcode`, then register a handler for it in the router.

## Packet Router

The router lives under [src/util/router](/C:/Users/wammu/source/repos/malice/src/util/router/mod.rs).

Important files:

- [src/util/router/registry.rs](/C:/Users/wammu/source/repos/malice/src/util/router/registry.rs)
- [src/util/router/reply.rs](/C:/Users/wammu/source/repos/malice/src/util/router/reply.rs)
- [src/util/router/handlers/register.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/register.rs)
- [src/util/router/handlers/heartbeat.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/heartbeat.rs)
- [src/util/router/handlers/fetch_task.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/fetch_task.rs)
- [src/util/router/handlers/task_result.rs](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/task_result.rs)

The pattern is:

- `PacketRouter` stores a registry of opcode -> handler
- each handler parses its own payload type
- each handler talks to `ServerContext`
- each handler returns `PacketReply`

To add a new opcode:

1. Add the opcode to `PacketOpcode` in [src/util/packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs).
2. Create a new handler file under [src/util/router/handlers](/C:/Users/wammu/source/repos/malice/src/util/router/handlers/mod.rs).
3. Register it in [src/util/router/registry.rs](/C:/Users/wammu/source/repos/malice/src/util/router/registry.rs).
4. Add or reuse the payload DTO the handler needs.

This keeps new protocol behavior local instead of growing one global match block.

## Implant Model

The implant domain lives under [src/util/implants](/C:/Users/wammu/source/repos/malice/src/util/implants/mod.rs).

Important types:

- `ImplantRecord`
- `ImplantIdentity`
- `ImplantStaticMetadata`
- `ImplantRuntimeState`
- `ImplantCapability`
- `ImplantRegistry`

This split is important:

- identity describes what the implant is
- static metadata describes host and process details
- capabilities describe what the implant can do
- runtime state describes liveness and active execution state

### Implant Families

[src/util/implants/family.rs](/C:/Users/wammu/source/repos/malice/src/util/implants/family.rs) maps `implant_type` strings to internal family descriptors.

That file currently defines:

- `ImplantFamily::CoffLoader`
- `ImplantFamily::Unknown`

Each family provides a capability list. Right now the `coff_loader` family advertises:

- `ImplantCapability::ExecuteCoff`

### Adding A New Implant Family

To add a new implant family safely:

1. Add a new `ImplantFamily` variant in [src/util/implants/family.rs](/C:/Users/wammu/source/repos/malice/src/util/implants/family.rs).
2. Extend `ImplantFamily::from_type` to recognize the new `implant_type` string.
3. Define the capabilities returned by `ImplantFamily::capabilities`.
4. Ensure the implant registers with the expected `implant_type`.
5. If the implant needs new packet types, add router handlers for those opcodes.
6. If the implant supports new tasks, add new `TaskSpec` variants and serializers.

The key point is that a new implant family should primarily affect:

- family mapping
- capabilities
- task support
- message handlers

It should not require changes to the HTTP transport layer.

## Task System

The task domain lives under [src/util/tasks](/C:/Users/wammu/source/repos/malice/src/util/tasks/mod.rs).

Important files:

- [src/util/tasks/types.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/types.rs)
- [src/util/tasks/queue.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/queue.rs)
- [src/util/tasks/repository.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/repository.rs)
- [src/util/tasks/serializer.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/serializer.rs)
- [src/util/tasks/results.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/results.rs)

Important types:

- `TaskSpec`
- `TaskRecord`
- `TaskStatus`
- `TaskResultPayload`
- `TaskResultData`

### Why `TaskSpec` Exists

`TaskSpec` replaces stringly typed task definitions and COFF-specific queue methods.

Current example:

- `TaskSpec::ExecuteCoff`

Each task spec defines:

- its logical task type string via `task_type()`
- its required implant capability via `required_capability()`

That capability check is enforced in [src/util/tasks/queue.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/queue.rs) before a task is queued.

### Adding A New Task Type

To add a new task kind:

1. Add a new `TaskSpec` variant in [src/util/tasks/types.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/types.rs).
2. Add any typed task payload struct it needs.
3. Extend `task_type()` and `required_capability()`.
4. Extend the serializer in [src/util/tasks/serializer.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/serializer.rs).
5. Update result handling if the task returns a new result shape.
6. Add command-side support if operators need to queue it from the CLI.
7. Add tests that verify unsupported implants cannot receive the task.

Do not add bespoke queue methods like `queue_run_process`, `queue_download`, and so on unless there is a strong reason. The preferred pattern is generic queueing through `TaskSpec`.

## Payload Repository

[src/util/payloads.rs](/C:/Users/wammu/source/repos/malice/src/util/payloads.rs) isolates payload lookup from the transport and command layers.

Current behavior:

- the caller asks for a logical payload name such as `whoami`
- the repository resolves it to an artifact file on disk
- the repository returns bytes plus the resolved file name

If you add more payload families or output locations, extend the repository. Do not hard-code file paths in:

- `HttpServer`
- packet handlers
- CLI command handlers

## CLI Architecture

The CLI lives under [src/util/command](/C:/Users/wammu/source/repos/malice/src/util/command/mod.rs).

Important files:

- [src/util/command/parser.rs](/C:/Users/wammu/source/repos/malice/src/util/command/parser.rs)
- [src/util/command/dispatcher.rs](/C:/Users/wammu/source/repos/malice/src/util/command/dispatcher.rs)
- [src/util/command/output.rs](/C:/Users/wammu/source/repos/malice/src/util/command/output.rs)
- [src/util/command/commands/server.rs](/C:/Users/wammu/source/repos/malice/src/util/command/commands/server.rs)
- [src/util/command/commands/implants.rs](/C:/Users/wammu/source/repos/malice/src/util/command/commands/implants.rs)
- [src/util/command/commands/tasks.rs](/C:/Users/wammu/source/repos/malice/src/util/command/commands/tasks.rs)

The split is:

- parser
  - turns input text into typed commands
- dispatcher
  - routes parsed commands to registered domain handlers
- output
  - prints domain objects cleanly
- command handlers
  - implement server, implant, and task command behavior

### Adding A New Operator Command

There are two cases:

If the new behavior belongs to an existing domain:

1. extend the parsed command type in [src/util/command/parser.rs](/C:/Users/wammu/source/repos/malice/src/util/command/parser.rs)
2. update that domain handler under [src/util/command/commands](/C:/Users/wammu/source/repos/malice/src/util/command/commands/mod.rs)

If the new behavior is a new domain:

1. add a new parsed command branch
2. add a new command handler file
3. register it in [src/util/command/commands/mod.rs](/C:/Users/wammu/source/repos/malice/src/util/command/commands/mod.rs)

Avoid reintroducing a single global executor function.

### Current Generic Task Command

The current operator-facing queue flow is:

```text
task queue <clientid> <task-kind> [args...]
```

For the current COFF path, that means:

```text
task queue <clientid> execute_coff [payload-name] [entrypoint] [args...]
```

Examples:

```text
task queue 513a666c-3349-40dd-9462-95c4449b0d0d execute_coff
task queue 513a666c-3349-40dd-9462-95c4449b0d0d execute_coff whoami
task queue 513a666c-3349-40dd-9462-95c4449b0d0d execute_coff whoami main
```

## How To Implement A New Implant

If you are writing a new implant, treat these parts as the contract:

- the wire format in [docs/wire-format.md](/C:/Users/wammu/source/repos/malice/docs/wire-format.md)
- the packet opcodes in [src/util/packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs)
- the expected registration fields in [src/util/implants/types.rs](/C:/Users/wammu/source/repos/malice/src/util/implants/types.rs)
- the task response format in [src/util/tasks/types.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/types.rs)

At a minimum, an implant should be able to:

1. generate or persist a `clientid`
2. send `REGISTER`
3. send `HEARTBEAT`
4. poll with `FETCH_TASK`
5. parse returned tasks
6. execute supported tasks
7. return `TASK_RESULT`

For a new implant family, also decide:

- what `implant_type` string it will register with
- what capabilities it should advertise through the server-side family map
- whether it reuses existing task types or needs new ones
- whether it needs any family-specific opcodes

## Recommended Extension Workflow

When extending the system, prefer this order:

1. Define the capability model first.
2. Define or update the task spec second.
3. Update implant family mapping.
4. Add or update packet handlers.
5. Add command support last.

That order keeps the operator interface as a thin layer over already-correct domain behavior.

## Guardrails

When making changes, preserve these invariants:

- do not put payload path resolution into the HTTP server
- do not queue tasks without checking implant capabilities
- do not add implant-specific behavior directly to transport code
- do not collapse task specs back into free-form strings
- do not add new top-level CLI behavior through one giant match block

If a proposed change violates one of those rules, it probably belongs in a domain module instead.

## Current Limitations

This refactor created the extension seams, but some areas are still intentionally minimal:

- only one explicit capability is implemented today: `ExecuteCoff`
- only one task spec is implemented today: `ExecuteCoff`
- task result typing is still simple and currently text-oriented
- family mapping is code-based, not plugin-based
- persistence is still in-memory

That is acceptable for the current stage. The important part is that future work can now be added without reworking the transport or CLI foundations.
