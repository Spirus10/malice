# `util` Modularity And Extensibility Review

This document captures a code review of the current `src/util` modules with a focus on scaling the teamserver to support multiple implant families with different capabilities, task types, and orchestration requirements.

The review is organized by module. Each section includes:

- current concern
- why it will become a problem as implants diversify
- recommended refactor direction

## Review Goals

- prevent the CLI command layer from becoming a monolithic switch statement
- prevent the task system from becoming a stringly typed pile of special cases
- keep implant-specific behavior out of the generic transport layer
- make it possible to add new implant families without rewriting the teamserver core

## `src/util/command.rs`

### Current concerns

- [`src/util/command.rs:33`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L33) through [`src/util/command.rs:42`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L42) encode every operator action into one enum.
- [`src/util/command.rs:56`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L56) through [`src/util/command.rs:78`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L78) parse the entire CLI with one hard-coded match tree.
- [`src/util/command.rs:81`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L81) through [`src/util/command.rs:185`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L185) execute commands in the same module that parses them.
- [`src/util/command.rs:144`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L144) through [`src/util/command.rs:151`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L151) hard-code a specific task verb, `whoami`, into the CLI.
- [`src/util/command.rs:45`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L45) through [`src/util/command.rs:47`](/C:/Users/wammu/source/repos/malice/src/util/command.rs#L47) make the command layer depend directly on `HttpServer`, which now also owns state and task orchestration.

### Why this will not scale

- every new implant capability will require editing the parser enum, the parser logic, and the handler match
- operator-facing verbs and backend orchestration are tightly coupled
- implant-specific commands will accumulate in a global namespace, even when only some implants support them

### Recommendations

- split the command system into:
  - `command/parser.rs`
  - `command/dispatcher.rs`
  - `command/output.rs`
- replace the single `Command` enum with a hierarchical model:
  - `ServerCommand`
  - `ImplantCommand`
  - `TaskCommand`
- introduce a `CommandContext` that exposes service traits instead of a concrete `HttpServer`
- define command handlers as trait objects or registered functions:

```text
trait CliCommandHandler {
    fn name(&self) -> &'static str;
    async fn execute(&self, ctx: &CommandContext, args: &[String]) -> Result<()>;
}
```

- move implant-family-specific commands behind capability-aware registration
- represent payload queueing generically:
  - `task queue <clientid> <task-kind> [args...]`
  - do not keep adding one-off top-level verbs like `task whoami`

### Suggested future structure

```text
src/util/command/
- mod.rs
- parser.rs
- dispatcher.rs
- output.rs
- commands/
  - server.rs
  - implants.rs
  - tasks.rs
```

## `src/util/httpserver.rs`

### Current concerns

- [`src/util/httpserver.rs:32`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L32) through [`src/util/httpserver.rs:37`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L37) combine transport concerns with application state ownership.
- [`src/util/httpserver.rs:59`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L59) through [`src/util/httpserver.rs:88`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L88) parse transport errors, HTTP route checks, and packet ingress in one method.
- [`src/util/httpserver.rs:155`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L155) through [`src/util/httpserver.rs:175`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L175) make the HTTP server directly responsible for task construction and payload file loading.
- [`src/util/httpserver.rs:182`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L182) through [`src/util/httpserver.rs:199`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L199) bake build-output paths for one payload into the server layer.
- [`src/util/httpserver.rs:101`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L101) through [`src/util/httpserver.rs:124`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs#L124) wrap the entire server state inside a `Mutex<HttpServer>`, which will become a contention point as traffic increases.

### Why this will not scale

- transport code should not know how to locate payload artifacts or build task records
- future non-HTTP transports would have to reimplement logic currently trapped in `HttpServer`
- one coarse-grained mutex around the whole server object constrains concurrency and complicates composition

### Recommendations

- reduce this module to transport-only responsibilities:
  - bind socket
  - accept connections
  - read request body
  - hand raw packet bytes to an application service
  - write response bytes
- move stateful business behavior into an `AppState` or `ServerContext`
- inject dependencies into the HTTP layer via a thin application trait:

```text
trait PacketService {
    async fn handle_packet(&self, peer: SocketAddr, body: Bytes) -> Response<...>;
}
```

- move payload loading into a separate payload repository module
- move task queueing helpers out of the HTTP server
- replace `Mutex<HttpServer>` with immutable shared state composed from independently synchronized subservices

### Suggested future structure

```text
src/util/http/
- mod.rs
- server.rs
- request.rs
- response.rs
```

## `src/util/router.rs`

### Current concerns

- [`src/util/router.rs:23`](/C:/Users/wammu/source/repos/malice/src/util/router.rs#L23) through [`src/util/router.rs:91`](/C:/Users/wammu/source/repos/malice/src/util/router.rs#L91) are one large opcode match with inline parsing, validation, service calls, and response formatting.
- the router depends directly on concrete request payload structs from multiple modules.
- the router returns transport-shaped `Response<Full<Bytes>>` values, which mixes protocol dispatch with HTTP rendering.

### Why this will not scale

- every new opcode will make the central match larger and more fragile
- opcode handling is not yet decomposed by concern
- non-HTTP transports would still need to depend on HTTP response types because the router currently emits them directly

### Recommendations

- split routing from handling:
  - router decides which handler to invoke
  - handler returns a protocol-level result
  - transport adapter turns that result into HTTP
- introduce per-opcode handler functions or handler objects:

```text
trait PacketHandler {
    fn opcode(&self) -> PacketOpcode;
    async fn handle(&self, ctx: &ServerContext, packet: Packet) -> Result<PacketReply>;
}
```

- create a `PacketReply` type that is transport-agnostic
- move request parsing into handler-local typed decoders
- keep the router registration-based rather than hard-coded if the opcode set is expected to grow

### Suggested future structure

```text
src/util/router/
- mod.rs
- registry.rs
- reply.rs
- handlers/
  - register.rs
  - heartbeat.rs
  - fetch_task.rs
  - task_result.rs
```

## `src/util/tasks.rs`

### Current concerns

- [`src/util/tasks.rs:24`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L24) through [`src/util/tasks.rs:32`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L32) use a single generic envelope shape for one task type.
- [`src/util/tasks.rs:55`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L55) stores `task_type` as a free-form string.
- [`src/util/tasks.rs:57`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L57) and [`src/util/tasks.rs:59`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L59) bake COFF-specific fields directly into the core task record.
- [`src/util/tasks.rs:81`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L81) through [`src/util/tasks.rs:113`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L113) expose only `queue_execute_coff`, which means every new task kind will add another bespoke queue function.
- [`src/util/tasks.rs:115`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L115) through [`src/util/tasks.rs:152`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L152) encode task data for wire delivery inside the queue manager itself.
- [`src/util/tasks.rs:155`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L155) through [`src/util/tasks.rs:169`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs#L169) reduce results to a single string and a success/error label.

### Why this will not scale

- different implant families will need different task payloads, argument models, and result schemas
- COFF execution is a task type, not the definition of the whole task subsystem
- task storage, queue policy, and wire serialization are currently collapsed into one module

### Recommendations

- introduce a typed task spec model:

```text
enum TaskSpec {
    ExecuteCoff(ExecuteCoffTask),
    RunProcess(RunProcessTask),
    Download(DownloadTask),
}
```

- make `TaskRecord` store `TaskSpec` instead of a string plus COFF-specific fields
- introduce a capability check before queueing:
  - task kind must be supported by the target implant family
- split responsibilities into:
  - task repository
  - queue scheduler
  - task serializer
  - result repository
- treat result data as a typed payload, not just `Option<String>`
- add delivery semantics explicitly:
  - queued
  - leased
  - acknowledged
  - timed out
  - completed

### Suggested future structure

```text
src/util/tasks/
- mod.rs
- types.rs
- queue.rs
- repository.rs
- serializer.rs
- results.rs
```

## `src/util/implants.rs`

### Current concerns

- [`src/util/implants.rs:32`](/C:/Users/wammu/source/repos/malice/src/util/implants.rs#L32) through [`src/util/implants.rs:45`](/C:/Users/wammu/source/repos/malice/src/util/implants.rs#L45) store registration and liveness data in one flat record.
- there is no explicit concept of implant family, capabilities, or supported task kinds beyond the string `implant_type`.
- [`src/util/implants.rs:60`](/C:/Users/wammu/source/repos/malice/src/util/implants.rs#L60) through [`src/util/implants.rs:96`](/C:/Users/wammu/source/repos/malice/src/util/implants.rs#L96) hard-code how registration mutates state.
- [`src/util/implants.rs:99`](/C:/Users/wammu/source/repos/malice/src/util/implants.rs#L99) through [`src/util/implants.rs:115`](/C:/Users/wammu/source/repos/malice/src/util/implants.rs#L99) treat heartbeat as only a liveness update; there is no room for richer runtime state.

### Why this will not scale

- the teamserver needs to know what each implant can do before queueing work
- different implants may expose different metadata, heartbeat contents, and orchestration needs
- a flat record pushes the codebase toward optional fields and string comparisons

### Recommendations

- split implant identity from runtime state:

```text
ImplantRecord
- identity
- static_metadata
- capabilities
- runtime_state
```

- introduce an explicit capability model:

```text
enum ImplantCapability {
    ExecuteCoff,
    RunProcess,
    UploadResult,
    DownloadFile,
}
```

- parse registration into an implant-family-specific descriptor, then normalize into a generic internal model
- track runtime fields separately from registration:
  - last_seen
  - last_heartbeat
  - current_status
  - active_task_id
  - last_error
- add lookup paths by:
  - client id
  - implant family
  - capability
  - online/offline state

### Suggested future structure

```text
src/util/implants/
- mod.rs
- registry.rs
- types.rs
- capabilities.rs
- family.rs
```

## `src/util/packet.rs`

### Current concerns

- [`src/util/packet.rs:8`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs#L8) through [`src/util/packet.rs:12`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs#L12) expose only one generic base packet shape.
- [`src/util/packet.rs:71`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs#L71) through [`src/util/packet.rs:85`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs#L85) accept a raw opcode byte instead of a richer typed protocol abstraction.
- [`src/util/packet.rs:88`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs#L88) through [`src/util/packet.rs:90`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs#L88) make packet consumers parse inner payloads ad hoc.
- the packet layer knows nothing about versioning or capability negotiation.

### Why this will not scale

- as soon as multiple implant families exist, packet payload validation needs to be more explicit
- packet parsing should produce typed protocol messages, not just a bag of JSON plus an opcode
- future binary payloads will be awkward if packet responsibility stays this generic

### Recommendations

- introduce a protocol message enum above raw packet bytes:

```text
enum ProtocolMessage {
    Register(RegisterPayload),
    Heartbeat(HeartbeatPayload),
    FetchTask(FetchTaskRequest),
    TaskResult(TaskResultPayload),
}
```

- move from `Packet::build(opcode: u8, ...)` to typed builders keyed by `PacketOpcode`
- add explicit protocol version handling in this module or in a sibling `protocol` module
- make decoding responsible for:
  - opcode validation
  - version validation
  - body decoding
  - typed message construction
- keep raw framing and typed protocol decoding separate

### Suggested future structure

```text
src/util/packet/
- mod.rs
- framing.rs
- opcode.rs
- protocol.rs
- codec.rs
```

## `src/util/logger.rs`

### Current concerns

- [`src/util/logger.rs:4`](/C:/Users/wammu/source/repos/malice/src/util/logger.rs#L4) through [`src/util/logger.rs:58`](/C:/Users/wammu/source/repos/malice/src/util/logger.rs#L58) repeat timestamp formatting logic in every function.
- logging is print-based and has no levels, no structured fields, and no routing to sinks.
- if operator UI, debug tracing, and packet audits are added later, this module will not support them cleanly.

### Why this matters for extensibility

- richer orchestration means richer observability requirements
- packet routing, task dispatch, and implant state transitions should be traceable without hand-formatting strings everywhere

### Recommendations

- replace this with a small facade over a real logging/tracing stack
- separate:
  - human CLI output
  - internal tracing
  - audit events
- avoid direct `println!` in core service modules
- standardize event fields:
  - clientid
  - opcode
  - task_id
  - implant_type
  - status

## `src/util.rs`

### Current concerns

- [`src/util.rs:1`](/C:/Users/wammu/source/repos/malice/src/util.rs#L1) through [`src/util.rs:7`](/C:/Users/wammu/source/repos/malice/src/util.rs#L7) expose a flat module namespace.
- as more services are added, `util` becomes a catch-all rather than a meaningful boundary.

### Recommendations

- use `util` only if it remains a true shared-infrastructure namespace
- otherwise rename and reorganize around explicit domains:

```text
src/
- cli/
- transport/
- protocol/
- implants/
- tasks/
- payloads/
- observability/
```

- if the current crate is kept flat for now, still start grouping related modules into folders before the codebase grows further

## Cross-Cutting Recommendations

### 1. Introduce a service layer

Add a central service container with explicit dependencies:

```text
ServerContext
- implant_registry
- task_service
- payload_repository
- packet_router
```

This reduces direct module-to-module coupling.

### 2. Introduce capability-aware tasking

Before queueing any task, the server should verify:

- implant family
- advertised capabilities
- protocol version compatibility

This is the main guardrail against task and command bloat.

### 3. Add a payload repository abstraction

Do not let the HTTP server or command layer find payload files directly.

Add a payload repository that resolves artifacts by logical name:

```text
payloads.resolve("whoami")
```

That repository can later support:

- different payload families
- architecture-specific builds
- prebuilt metadata
- signing

### 4. Separate transport models from domain models

Request payload structs should not automatically become long-lived domain objects.

Prefer:

- packet/request DTOs
- internal service/domain types
- response DTOs

### 5. Define extension seams early

The first explicit extension seams should be:

- implant family descriptor
- capability registry
- task spec enum
- packet handler registry
- payload repository

Those seams will make future implant types additive instead of invasive.

## Recommended Refactor Order

1. Extract payload lookup from [`src/util/httpserver.rs`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs).
2. Introduce a typed `TaskSpec` in place of `task_type: String`.
3. Introduce implant capabilities in [`src/util/implants.rs`](/C:/Users/wammu/source/repos/malice/src/util/implants.rs).
4. Split routing into handler modules under a router folder.
5. Split CLI parsing from command execution.
6. Replace the flat `util` layout with domain-oriented folders.

## Acceptance Criteria For The Refactor

- adding a new implant family does not require editing the HTTP transport layer
- adding a new task type does not require inventing new hard-coded queue methods in the server
- task queueing validates implant capabilities before enqueue
- operator commands are registered modularly rather than added to one global match block
- payload lookup is abstracted behind a repository rather than hard-coded file paths
