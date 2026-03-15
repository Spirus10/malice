# Ratatui TUI Design

This document describes the `ratatui`-based terminal UI for the teamserver.

The TUI exists to replace the UUID-heavy CLI workflow with an operator interface that is:

- navigable
- glanceable
- keyboard-driven
- compatible with the current teamserver architecture
- extensible without collapsing into one large UI module

This design assumes the current Rust teamserver structure described in:

- [docs/modular-architecture.md](/C:/Users/wammu/source/repos/malice/docs/modular-architecture.md)
- [docs/interop-architecture.md](/C:/Users/wammu/source/repos/malice/docs/interop-architecture.md)

## Problem Statement

The original CLI works, but the operator experience is weak:

- implants are identified by UUID, which forces copy/paste
- there is no persistent active implant selection
- host metadata is only visible after a separate info command
- tasking is detached from implant navigation
- command history and command context are not visible together
- there is no strong distinction between global teamserver commands and implant-bound actions

Once an operator selects an implant, their working mode changes. They are no longer just browsing. They are tasking one target.

The TUI should reflect that shift.

## Goals

- present implants in a navigable list
- allow one implant to be selected as the active context
- show key host and runtime information without extra commands
- preserve a teamserver command interpreter for global commands
- provide a larger implant-bound command workspace once one agent is selected for tasking
- expose only commands the bound implant can actually run
- support keyboard-only operation
- preserve modularity between UI, application state, and command/task services

## Non-Goals

- mouse-first interaction
- graphical widgets beyond terminal UI
- a full-screen text editor experience
- replacing the underlying command/task architecture
- implementing persistence or multi-user coordination in this phase
- inventing a freeform remote shell language for implants

## High-Level Layout

Recommended browse or teamserver layout:

```text
+-----------------------------------------------------------+
| Header                                                    |
| server | active implant | filters | context | focus       |
+-----------------------------+-----------------------------+
| Implants                     | Implant Details            |
| [list / table]              | host, user, pid, process   |
|                             | os, arch, seen, status     |
|                             | capabilities, active task  |
+-----------------------------------------------------------+
| Activity / Result Preview                                  |
+-----------------------------------------------------------+
| Teamserver Commands                                        |
| teamserver> tcpserver start                                |
+--------------------+--------------------------------------+
| Status             | Key Hints                            |
+--------------------+--------------------------------------+
```

Recommended implant tasking layout:

```text
+-----------------------------------------------------------+
| Header                                                    |
| server | active implant | filters | context=agent         |
+-----------------------------+-----------------------------+
| Implants                     | Implant Details            |
| [list / table]              | host, user, pid, process   |
|                             | os, arch, seen, status     |
|                             | capabilities, active task  |
+-----------------------------------------------------------+
| Activity / Result Preview                                  |
+-----------------------------------------------------------+
| Agent Commands [bound client]                              |
| bound to <clientid>                                        |
| supported: whoami, help, back                              |
|                                                            |
| agent> whoami                                              |
|                                                            |
| queued task / latest result / recent errors                |
+--------------------+--------------------------------------+
| Status             | Key Hints                            |
+--------------------+--------------------------------------+
```

Important layout rule:

- the command pane is short in teamserver context
- the command pane becomes larger in agent context
- the larger pane is bound to one explicit implant, not to the transient cursor row

## Primary Panes

### 1. Implant List Pane

Purpose:

- show the implant inventory
- allow fast navigation
- establish the selected implant
- provide the source row for agent binding

Recommended columns:

- short client identifier
- hostname
- username
- process name
- status
- heartbeat age
- active task

Recommended behavior:

- arrow keys or `j`/`k` move selection
- `Enter` binds the selected implant and enters agent command mode
- `/` starts local filtering
- `r` refreshes immediately

Important design choice:

- show a short client ID in the table
- show the full UUID in the details pane and agent command pane

### 2. Implant Details Pane

Purpose:

- show the full record for the selected implant
- keep important state visible while the operator navigates

Recommended fields:

- full client ID
- implant type
- protocol version
- hostname
- username
- pid
- process name
- os
- arch
- first seen
- last seen
- last heartbeat
- current status
- active task ID
- capabilities

Recommended actions shown as hints:

- `Enter` bind selected implant
- `a` open agent commands
- `:` open teamserver commands

### 3. Activity / Result Pane

Purpose:

- show operator-relevant recent activity without leaving the main screen
- show the latest task/result preview for the currently relevant context

Recommended content:

- recent task queue actions
- recent task completions
- recent error events
- result preview for the selected or bound implant

Suggested event line format:

```text
[19:01:51] completed execute_coff whoami -> WAMMU-PC\wammu
```

### 4. Command Pane

The TUI should have two distinct command contexts:

1. `Teamserver`
2. `Agent`

These are not the same thing and should not be presented as the same thing.

#### Teamserver Command Context

Purpose:

- manage the server
- inspect global state
- perform administrative or diagnostic actions not tied to one implant

Recommended behavior:

- `:` enters teamserver command mode
- use the existing parser and command handlers
- keep teamserver command history separate from agent command history
- keep the pane relatively short in this context

Example commands:

- `tcpserver start`
- `tcpserver stop`
- `implants list`
- `implants info active`
- `task result <taskid>`

#### Agent Command Context

Purpose:

- bind the command workspace to one specific implant
- expose only supported implant actions
- keep command and result flow for one implant in one place

Recommended behavior:

- `Enter` or `a` binds the selected implant and enters agent command mode
- the command pane grows vertically when an implant is bound
- browsing another row does not silently retarget the bound implant
- `Tab`, `:`, or `back` returns to teamserver command mode

Important design rule:

- agent mode should expose operator-facing verbs, not raw teamserver syntax

First agent command set:

```text
whoami
help
back
```

`whoami` is the only real implant task currently implemented.

The UI may internally translate:

```text
whoami
```

into:

```text
task queue <clientid> execute_coff whoami main
```

That translation belongs in the controller/service layer, not in the widget.

## Interaction Model

Recommended modes:

- `Browse`
  - default
  - implant list navigation
- `TeamserverCommand`
  - teamserver command input focused
- `AgentCommand`
  - implant-bound command input focused
- `Filter`
  - implant list filtering
- `Help`
  - keybinding reference

The UI should be modal only where needed. Avoid deep Vim-style modal behavior.

## Context Model

The TUI should explicitly track command scope.

Recommended state:

```text
enum CommandContextMode {
    Teamserver,
    Agent { clientid: Uuid },
}
```

This binding must live in `ui::state`.

Do not infer command context indirectly from cursor position alone.

Recommended semantics:

- moving the cursor changes selection
- binding an agent is explicit
- the bound agent remains the command target until the operator switches away
- cursor movement must not silently retarget an already bound agent context

## Capability-Gated Agent Commands

The agent command set must derive from implant capabilities.

The current backend already models capabilities on `ImplantRecord`.

That should drive:

- which commands appear in the agent workspace
- which quick actions are enabled
- which completions are available
- which manual submissions are accepted

### First Version

First version command map:

- if implant supports `ExecuteCoff`, expose `whoami`

That means the UI-visible command list is:

```text
whoami
```

If the implant does not support that capability:

- the agent workspace should show no runnable implant commands
- the UI should still allow `help` and `back`
- manual submission of unsupported implant commands should be rejected with a clear status message

Important rule:

- widgets present
- controller validates
- service layer remains authoritative

## Ergonomic Flows

### Queue A Task

1. open TUI
2. navigate the implant list
3. press `Enter` or `a` to bind the selected implant
4. type `whoami`
5. activity pane shows queued and completed state
6. result preview updates automatically

This is substantially better than:

- `implants list`
- copy UUID
- `task queue <uuid> ...`
- copy task ID
- `task result <taskid>`

### Return To Teamserver Commands

1. bind an implant and task it in agent mode
2. press `Tab` or `:`
3. teamserver command pane becomes active again
4. run global commands without losing implant situational awareness

### Review Implant State

1. navigate the implant list
2. details pane updates immediately
3. activity pane shows recent tasks and errors for the selected or bound implant

## Keybindings

Recommended keybindings:

- `q` quit
- `j` / `k` or arrow keys move selection
- `Enter` bind selected implant and enter agent command mode
- `a` enter agent command mode for selected implant
- `:` enter teamserver command mode
- `Tab` switch between teamserver and agent command contexts
- `/` filter implants
- `r` refresh
- `t` open task actions
- `?` help
- `Esc` return to browse

## Task Action UX

A task action overlay or popup is still useful for common actions.

Recommended initial task menu:

```text
Task Actions
- execute_coff / whoami
- view latest result
```

Task actions should also be driven by implant capabilities.

If the selected or bound implant does not support a task kind:

- disable it visually or reject it with a clear explanation
- keep the service layer as the final authority

## Data Flow And Architecture

The TUI should not bypass the existing application or service layer.

Recommended architecture:

```text
ui/
- app.rs
- state.rs
- events.rs
- layout.rs
- actions.rs
- controller.rs
- widgets/
  - implants.rs
  - details.rs
  - activity.rs
  - command.rs
  - help.rs
```

### Responsibilities

`ui::state`

- local UI state only:
  - selected row
  - focused mode
  - filter text
  - command context binding
  - teamserver command buffer
  - teamserver command history cursor
  - agent command buffer
  - agent command history cursor

`ui::controller`

- translates key events into actions
- requests data from application services
- dispatches commands
- validates capability-gated agent commands
- maps agent verbs to typed service calls

`ui::widgets`

- rendering only
- no business logic

`ui::actions`

- typed UI actions such as:
  - `SelectNextImplant`
  - `SelectPreviousImplant`
  - `EnterAgentContext`
  - `EnterTeamserverContext`
  - `ToggleCommandContext`
  - `QueueWhoami`
  - `SubmitTeamserverCommand`
  - `SubmitAgentCommand`

## Integration With Existing Command System

The current command system is already modular.

Recommended integration:

- reuse the existing parser and command handlers for teamserver commands
- resolve `active` into a concrete UUID before teamserver command parsing
- call typed services directly for common UI actions
- do not parse agent commands through the raw teamserver parser
- add a thin agent command adapter that maps:
  - `whoami` -> typed `execute_coff whoami` queue call

Important design rule:

- teamserver commands keep the existing syntax
- agent commands keep a narrower operator-facing vocabulary

## Refresh Strategy

The UI needs periodic refresh without blocking input.

Recommended model:

- event loop polls input frequently
- background UI tick every 250ms
- data refresh tick every 1s
- implant list refresh on tick
- activity updates on task or heartbeat changes

Do not tie rendering directly to network or database access.

## Status And Event Model

The TUI benefits from a small internal event stream.

Recommended event categories:

- implant registered
- heartbeat updated
- task queued
- task completed
- task failed
- command error
- transport/server error

The activity pane should render this with severity-aware styling.

## Styling And Visual Language

For a terminal UI, clarity matters more than novelty.

Recommended style guidance:

- strong pane borders and titles
- selected implant row visibly highlighted
- clear distinction between teamserver and agent command panes
- status colors:
  - healthy/success: green
  - informational: cyan or blue
  - error/offline: red

Avoid:

- excessive color noise
- dense borders everywhere
- context switching without visible labels

## Extensibility Recommendations

### 1. Separate Screen State From Domain State

Do not make widgets fetch application state directly.

Instead:

- controller gathers data
- controller updates UI state or view-model data
- widgets render the view model

### 2. Use Typed UI Actions

Avoid giant key-handling match blocks that directly mutate everything.

Prefer:

```text
KeyEvent -> UiAction -> controller -> state update + service call
```

### 3. Capability-Aware Actions

Task menus, quick actions, and the agent command set should derive from implant capabilities instead of a hard-coded universal list.

### 4. Keep The Teamserver Command Pane

Do not remove the teamserver interpreter just because the TUI exists.

It remains useful for:

- advanced workflows
- debugging
- newly added features before dedicated widgets exist

### 5. Avoid Hidden Retargeting

If agent context silently follows the highlighted row, the operator can task the wrong implant.

Mitigation:

- bind agent context to an explicit client ID
- show that ID in the header and command pane title

## Suggested First Milestone

Implement:

- full-screen app shell
- implant table
- implant details pane
- activity pane
- explicit teamserver and agent command contexts
- active implant resolution in teamserver commands
- agent binding to one implant
- capability-gated `whoami` in agent mode
- key hints/help overlay

Do not try to add:

- multiple full-screen routes
- popup forms for every task kind
- a full log browser

until the basic operator loop feels good.

## Suggested Second Milestone

After the first milestone works:

- add more capability-aware agent commands
- add command completion
- improve result preview
- add filters and sorts
- add richer disabled-command explanations

## Risks

### 1. Recreating A UI Monolith

If rendering, key handling, service access, and state mutation all live in one file, the TUI will become difficult to extend.

Mitigation:

- split widgets, state, and controller logic early

### 2. Blocking UI On Service Calls

If implant refresh or task queueing blocks the render loop, the UI will feel unstable.

Mitigation:

- separate render ticks from refresh ticks
- use non-blocking update flow where possible

### 3. Too Much Modal Complexity

Deep modal behavior often makes TUIs harder to operate than simple CLIs.

Mitigation:

- keep mode count low
- make focus and key hints always visible

### 4. Two Sources Of Truth

If the TUI invents its own business logic instead of using the service layer, behavior will diverge from the CLI.

Mitigation:

- route actions through shared services and command dispatch

### 5. Context Confusion

If the operator cannot tell whether they are typing to the teamserver or to an implant, the UI will feel unsafe.

Mitigation:

- use different pane titles
- use different prompt labels
- use different pane heights
- show clear context indicators in the header

## Acceptance Criteria

The first phase is successful when:

- the operator can navigate implants without copying UUIDs
- the selected implant's key host information is visible at a glance
- selecting an implant binds a larger agent command workspace to that implant
- only implant-supported agent commands are shown and accepted
- `whoami` is available only for implants that support the current task path
- the operator can review recent task results without leaving the main screen
- the operator can switch back to teamserver commands without losing situational awareness
- the UI remains modular enough to add new panes and commands without centralizing everything in one file
