# malice

`malice` is a lab-only adversary-emulation research project. It combines a Rust teamserver with a Windows COFF loader implant (`implant/zant`) to exercise registration, heartbeat tracking, integration-driven tasking, and result collection in a controlled environment.

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
  - a capability-aware task queue
  - a `ratatui`-based terminal UI
  - operator commands for server control, implant inspection, task queueing, and task result viewing
- A Windows implant submodule in [`implant/zant`](/implant/zant) that:
  - loads relocatable COFF object files
  - resolves symbols and imports at runtime
  - executes one entrypoint inside the current process
  - returns output through the teamserver protocol

Current implementation is intentionally narrow. The primary end-to-end task path is the `zant` plugin package, which advertises manifest task kinds such as `whoami` and serializes them into its own `execute_coff` transport envelope backed by payload artifacts such as `whoami.obj`.

## Repository Layout

- [`src/main.rs`](/src/main.rs): Rust application entrypoint
- [`src/ui`](/src/ui): terminal UI, controller, state, widgets, key handling
- [`src/core/app.rs`](/src/core/app.rs): shared `ServerContext` service container
- [`src/core/httpserver.rs`](/src/core/httpserver.rs): HTTP listener and `POST /packet` transport shim
- [`src/core/packet.rs`](/src/core/packet.rs): wire framing and packet encode/decode
- [`src/core/router`](/src/core/router): opcode router and packet handlers
- [`src/core/implants`](/src/core/implants): implant identity, family mapping, capabilities, registry
- [`src/core/tasks`](/src/core/tasks): generic queueing, repository, transport envelopes, and results
- [`src/core/payloads.rs`](/src/core/payloads.rs): payload artifact lookup for logical task names
- [`implant/zant/plugin`](/C:/Users/wammu/source/repos/malice/implant/zant/plugin): sample installable `zant` plugin package owned by the implant repo
- [`implant/zant/plugin-src`](/C:/Users/wammu/source/repos/malice/implant/zant/plugin-src): source for the `zant` plugin worker binary shipped in the package
- [`src/core/command`](/src/core/command): operator command parsing, dispatch, and output
- [`docs/wire-format.md`](/docs/wire-format.md): current packet format contract
- [`docs/modular-architecture.md`](/docs/modular-architecture.md): extension seams and server-side architecture
- [`implant/zant`](/implant/zant): Windows COFF loader implant submodule

## Architecture

At a high level, `malice` is organized around transport-independent domain modules. The transport layer accepts packets, the router dispatches by opcode, and domain services handle implant state, tasking, payload resolution, and results.

### Core services

[`ServerContext`](/src/core/app.rs) owns the main shared services:

- implant registry
- task service
- payload repository
- packet router
- activity log

This keeps business logic out of the HTTP shim and gives both the UI and protocol handlers a common application context.

### Transport

[`HttpServer`](/src/core/httpserver.rs) listens on `127.0.0.1:42069` and accepts `POST /packet`. Its responsibilities are intentionally narrow:

- receive the raw request body
- parse it into a packet
- hand the packet to the router
- return the router's response

The server does not own tasking policy, implant logic, or payload resolution.

### Packet routing

The router under [`src/core/router`](/src/core/router) dispatches parsed packets to per-opcode handlers. Current handler modules cover:

- registration
- heartbeat
- task fetch
- task result

This avoids a single monolithic packet-processing function and keeps protocol behavior additive.

### Implant model

The implant domain under [`src/core/implants`](/src/core/implants) separates:

- identity and metadata
- family mapping
- capability declaration
- runtime registry state

Right now the most important integration-declared capability is `execute_coff`, associated with the `coff_loader` implant family.

### Task model

The task system under [`src/core/tasks`](/src/core/tasks) stores generic queued task metadata:

- operator-facing `task_kind`
- required capability key
- opaque integration-owned task state

Capability checks still happen before queueing so unsupported implant families do not receive incompatible work, but the concrete execution model now belongs to the selected implant integration.

### Payload resolution

[`PayloadRepository`](/src/core/payloads.rs) maps a logical payload name such as `whoami` to an on-disk artifact. Search roots come from the active plugin package manifest, so artifact lookup stays package-relative instead of assuming repo-local build output paths.

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

In the current integration model, the selected integration resolves payload artifacts, packs arguments, serializes the implant-facing task envelope, and decodes results. For `zant`, that means converting manifest task kinds into its COFF-loader task envelope.

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

`malice` now loads implant integrations from the local plugin store under `plugins/`. This repo includes a sample install package for `zant` under [`implant/zant/plugin`](/C:/Users/wammu/source/repos/malice/implant/zant/plugin), but it is not auto-installed or auto-activated at startup.

To exercise the plugin flow explicitly:

```powershell
plugins install implant/zant/plugin
plugins activate zant 0.1.0
```

That copies the package into:

- `plugins/packages/zant/0.1.0`
- `plugins/active/zant`

If an active plugin's artifacts are missing, task queueing for payload-backed tasks will fail at payload resolution time.

The install package contains only the files required for installation and execution:

- `plugin.json`
- `manifest.json`
- `bin/plugin.exe`
- `artifacts/`

The source used to build `bin/plugin.exe` lives separately under [`implant/zant/plugin-src`](/C:/Users/wammu/source/repos/malice/implant/zant/plugin-src).

## Current Operator Commands

The command parser currently supports:

- `httpserver start`
- `httpserver stop`
- `implants list`
- `implants info <clientid>`
- `plugins list`
- `plugins install <path>`
- `plugins activate <id> [version]`
- `plugins deactivate <id>`
- `plugins remove <id> <version>`
- `plugins inspect <id> [version]`
- `task queue <clientid> <task-kind> [args..]`
- `task result <task_id>`
- `exit`

For the current `zant` integration, `task-kind` is an integration-defined command such as `whoami`, `ps`, `pwd`, or another manifest entry.

This command surface should be treated as a research interface rather than a stable user-facing CLI contract.

## Development Notes

Several design constraints are deliberate:

- transport code should stay transport-only
- packet handling should be split by opcode
- implant families should advertise capabilities explicitly
- tasking should remain capability-aware while execution semantics stay inside integrations
- payload lookup should stay centralized

If you plan to extend the project, start with these documents:

- [`docs/modular-architecture.md`](/docs/modular-architecture.md)
- [`docs/wire-format.md`](/docs/wire-format.md)

## Limitations

The current codebase is still intentionally small in scope:

- state is in-memory
- protocol framing is simple and text-based
- only one explicit implant capability is modeled
- only one integration-owned execution model is implemented
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
