# malice

`malice` is a lab-only adversary-emulation research project. It combines a Rust teamserver with a Windows COFF loader implant (`implant/zant`) to exercise registration, heartbeat tracking, typed tasking, and result collection in a controlled environment.

The project is intended for personal research, blue-team validation, red-team lab work, and detection-content development. It is not designed, documented, or supported for real-world deployment against third-party systems.

## Safety Notice

This repository is for isolated lab use only.

- Do not deploy it on networks or hosts you do not own or explicitly control.
- Do not treat it as production-ready command-and-control infrastructure.
- Do not use it as a stealth, evasion, or persistence framework.
- Expect the protocol, data model, and implementation details to change as the project evolves.

The long-term intent is to pair the project with defensive content such as YARA, Sigma, and related detection guidance so that the same codebase can support both emulation and detection engineering.

## Current Scope

Today the repository contains two main components:

- A Rust teamserver with:
  - an HTTP packet endpoint
  - an in-memory implant registry
  - a typed task queue
  - a `ratatui`-based terminal UI
  - operator commands for server control, implant inspection, task queueing, and task result viewing
- A Windows implant submodule in [`implant/zant`](/implant/zant) that:
  - loads relocatable COFF object files
  - resolves symbols and imports at runtime
  - executes one entrypoint inside the current process
  - returns output through the teamserver protocol

Current implementation is intentionally narrow. The primary end-to-end task path is a typed `execute_coff` task backed by payload artifacts such as `whoami.obj`.

## Repository Layout

- [`src/main.rs`](/src/main.rs): Rust application entrypoint
- [`src/ui`](/src/ui): terminal UI, controller, state, widgets, key handling
- [`src/util/app.rs`](/src/util/app.rs): shared `ServerContext` service container
- [`src/util/httpserver.rs`](/src/util/httpserver.rs): HTTP listener and `POST /packet` transport shim
- [`src/util/packet.rs`](/src/util/packet.rs): wire framing and packet encode/decode
- [`src/util/router`](/src/util/router): opcode router and packet handlers
- [`src/util/implants`](/src/util/implants): implant identity, family mapping, capabilities, registry
- [`src/util/tasks`](/src/util/tasks): typed task definitions, queueing, serialization, and results
- [`src/util/payloads.rs`](/src/util/payloads.rs): payload artifact lookup for logical task names
- [`src/util/command`](/src/util/command): operator command parsing, dispatch, and output
- [`docs/wire-format.md`](/docs/wire-format.md): current packet format contract
- [`docs/modular-architecture.md`](/docs/modular-architecture.md): extension seams and server-side architecture
- [`docs/interop-architecture.md`](/docs/interop-architecture.md): intended teamserver and `zant` integration shape
- [`implant/zant`](/implant/zant): Windows COFF loader implant submodule

## Architecture

At a high level, `malice` is organized around transport-independent domain modules. The transport layer accepts packets, the router dispatches by opcode, and domain services handle implant state, tasking, payload resolution, and results.

### Core services

[`ServerContext`](/src/util/app.rs) owns the main shared services:

- implant registry
- task service
- payload repository
- packet router
- activity log

This keeps business logic out of the HTTP shim and gives both the UI and protocol handlers a common application context.

### Transport

[`HttpServer`](/src/util/httpserver.rs) listens on `127.0.0.1:42069` and accepts `POST /packet`. Its responsibilities are intentionally narrow:

- receive the raw request body
- parse it into a packet
- hand the packet to the router
- return the router's response

The server does not own tasking policy, implant logic, or payload resolution.

### Packet routing

The router under [`src/util/router`](/src/util/router) dispatches parsed packets to per-opcode handlers. Current handler modules cover:

- registration
- heartbeat
- task fetch
- task result

This avoids a single monolithic packet-processing function and keeps protocol behavior additive.

### Implant model

The implant domain under [`src/util/implants`](/src/util/implants) separates:

- identity and metadata
- family mapping
- capability declaration
- runtime registry state

Right now the only explicit family-specific capability is `ExecuteCoff`, associated with the `coff_loader` implant family.

### Task model

The task system under [`src/util/tasks`](/src/util/tasks) uses typed task specifications instead of free-form strings. The current concrete task is:

- `execute_coff`

That task carries:

- the payload object name
- raw object bytes
- the entrypoint name
- optional argument bytes

Capability checks happen before queueing so unsupported implant families do not receive incompatible work.

### Payload resolution

[`PayloadRepository`](/src/util/payloads.rs) maps a logical payload name such as `whoami` to an on-disk artifact. By default it searches:

- `implant/zant/out/build/msvc-debug/payloads`
- `implant/zant/out/build/msvc-release/payloads`
- `implant/zant/out/build/default/payloads`

This keeps artifact lookup out of transport and command code.

### Operator interface

The Rust binary launches a `ratatui` terminal UI from [`src/ui/app.rs`](/src/ui/app.rs). The interface renders:

- implant inventory
- selected implant details
- recent activity
- command input
- task results

The UI is backed by typed operator commands rather than embedding protocol behavior directly in widgets.

## Protocol Model

The current packet format is documented in [`docs/wire-format.md`](/docs/wire-format.md).

In short:

- each packet starts with a 1-byte opcode
- the remaining bytes are UTF-8 JSON
- the outer JSON contains:
  - `clientid`
  - `data`
- `data` is itself JSON encoded as a string, not nested as an object

Current opcode flow covers:

- register
- heartbeat
- fetch task
- task result

The current protocol is intentionally simple and text-oriented. It is suitable for interoperability work inside a lab, not for hardened or production use.

## `zant` Integration

[`implant/zant`](/implant/zant) is tracked as a git submodule:

- repository: `https://github.com/spirus10/zant.git`

`zant` is a Windows COFF object loader that:

- maps relocatable `.obj` files into memory
- resolves symbols and imports
- applies relocations
- invokes a selected entrypoint

In the current integration model, the teamserver resolves a payload artifact, serializes it into a task envelope, and the implant executes it through the loader runtime. The default artifact path already assumes `zant` build outputs.

See [`implant/zant/README.md`](/implant/zant/README.md) for loader-specific details.

## Building

### Teamserver

Prerequisites:

- Rust toolchain
- Cargo

Build the Rust application:

```powershell
cargo build
```

Run the terminal UI:

```powershell
cargo run
```

Run tests:

```powershell
cargo test
```

### `zant` submodule

`malice` expects payload artifacts produced by the `zant` build. Build that component separately from [`implant/zant`](/implant/zant) using its own documented workflow.

The project currently expects payload objects under one of:

- `implant/zant/out/build/msvc-debug/payloads`
- `implant/zant/out/build/msvc-release/payloads`
- `implant/zant/out/build/default/payloads`

If those artifacts are missing, task queueing for payload-backed tasks will fail at payload resolution time.

## Current Operator Commands

The command parser currently supports:

- `tcpserver start`
- `tcpserver stop`
- `implants list`
- `implants info <clientid>`
- `task queue <clientid> <task-kind> [args..]`
- `task result <task_id>`
- `exit`

For the current task model, `task-kind` is effectively `execute_coff` or `coff`.

This command surface should be treated as a research interface rather than a stable user-facing CLI contract.

## Development Notes

Several design constraints are deliberate:

- transport code should stay transport-only
- packet handling should be split by opcode
- implant families should advertise capabilities explicitly
- tasking should remain typed and capability-aware
- payload lookup should stay centralized

If you plan to extend the project, start with these documents:

- [`docs/modular-architecture.md`](/docs/modular-architecture.md)
- [`docs/interop-architecture.md`](/docs/interop-architecture.md)
- [`docs/wire-format.md`](/docs/wire-format.md)

## Limitations

The current codebase is still intentionally small in scope:

- state is in-memory
- protocol framing is simple and text-based
- only one explicit implant capability is modeled
- only one concrete task family is implemented
- artifact lookup is local and convention-based
- the UI and command surface are still evolving

Those constraints are acceptable for the current phase because the focus is on architecture, interoperability, and repeatable lab experimentation.

## Responsible Use

If this project is published, it should be accompanied by:

- explicit lab-only usage guidance
- defensive detections for the generated traffic and payload flow
- documentation for safe testing boundaries
- versioned protocol notes so detections remain aligned with implementation

Until then, treat the repository as a local research environment, not a finished platform.
