# Ratatui TUI Design

This document proposes a `ratatui`-based terminal UI for the teamserver.

The goal is to replace the current UUID-heavy command flow with an operator interface that is:

- navigable
- glanceable
- keyboard-driven
- compatible with the current teamserver architecture
- extensible without collapsing back into one large UI module

This design assumes the current Rust teamserver structure described in:

- [docs/modular-architecture.md](/C:/Users/wammu/source/repos/malice/docs/modular-architecture.md)
- [docs/interop-architecture.md](/C:/Users/wammu/source/repos/malice/docs/interop-architecture.md)

## Problem Statement

The current CLI works, but the operator experience is poor:

- implants are identified by UUID, which forces copy/paste
- there is no persistent active implant selection
- host metadata is only visible after a separate info command
- the tasking workflow is detached from implant navigation
- command history and command context are not visible together

This increases friction for routine workflows such as:

- checking which implants are alive
- selecting one implant
- queueing a task against it
- reviewing the result

## Goals

- present active implants in a navigable list
- allow one implant to be selected as the active context
- show key host and runtime information without requiring extra commands
- keep a command interpreter pane for freeform or advanced operator actions
- support keyboard-only operation
- preserve modularity between UI, application state, and command/task services

## Non-Goals

- mouse-first interaction
- embedded graphical widgets beyond terminal UI
- full-screen text editor behavior
- replacing the underlying command/task architecture
- implementing persistence or multi-user coordination in this phase

## High-Level UI Layout

Recommended initial layout:

```text
+-----------------------------------------------------------+
| Header                                                    |
| server status | active implant | filters | mode           |
+-----------------------------+-----------------------------+
| Implants                     | Implant Details            |
|                             |                             |
| [list / table]              | host, user, pid, process   |
|                             | os, arch, first seen       |
|                             | last heartbeat, status     |
|                             | capabilities, active task  |
|                             |                             |
+-----------------------------+-----------------------------+
| Task / Activity Panel                                     |
| recent task queueing | results | events                   |
+-----------------------------------------------------------+
| Command Interpreter                                       |
| > task queue active execute_coff whoami main              |
+-----------------------------------------------------------+
| Status / Key Hints                                        |
| j/k move | enter select | : command | q quit | ? help     |
+-----------------------------------------------------------+
```

This gives the operator:

- a stable implant inventory view
- a stable detail panel for the selected implant
- a command line for advanced actions
- a visible place for recent system events and task results

## Primary Panes

### 1. Implant List Pane

Purpose:

- show the current implant inventory
- allow fast navigation
- support filtering and sorting
- establish the active implant context

Recommended columns:

- marker for selected row
- short client identifier
- hostname
- username
- process name
- status
- heartbeat age
- active task

Recommended behavior:

- arrow keys or `j`/`k` move selection
- `Enter` sets active implant
- `/` starts local filtering
- `s` cycles sort mode
- `r` refreshes immediately

Important design choice:

- show a shortened client ID in the table by default
- keep the full UUID visible in the details pane

That eliminates most copy/paste without hiding exact identity.

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

- `i` inspect full implant info
- `t` open task actions
- `c` focus command line with active implant inserted

### 3. Task / Activity Pane

Purpose:

- show operator-relevant recent activity without leaving the main screen

Recommended content:

- recent task queue actions
- recent task completions
- recent error events
- result preview for the selected implant's latest task

This should be event-oriented, not a raw log dump.

Suggested event line format:

```text
[19:01:51] completed execute_coff whoami -> WAMMU-PC\wammu
```

### 4. Command Interpreter Pane

Purpose:

- preserve power-user and future advanced workflows
- allow freeform commands without forcing every action into menus

Recommended behavior:

- `:` focuses command mode
- current active implant can be referenced symbolically
- command history available via up/down arrows
- tab completion for command verbs and selected contextual values

Recommended symbolic aliases:

- `active`
- `selected`
- later: named tags or groups

Example:

```text
task queue active execute_coff whoami main
```

The TUI should resolve `active` to the selected implant UUID before dispatch.

This preserves the existing command architecture while removing the worst ergonomics.

## Interaction Model

The TUI should be modal only where necessary.

Recommended modes:

- `Browse`
  - default
  - implant list navigation
- `Command`
  - command input focused
- `Filter`
  - implant list filtering
- `Help`
  - keybinding reference

Avoid Vim-style deep modal complexity. The UI should remain predictable.

## Ergonomic Flow

### Common Flow: Queue A Task

Recommended operator path:

1. open TUI
2. use implant list to select an implant
3. press `t` for task actions or `:` for command mode
4. choose `execute_coff -> whoami` or type `task queue active execute_coff whoami main`
5. activity pane shows queued and completed state
6. result preview updates automatically

This is substantially better than:

- `implants list`
- copy UUID
- `task queue <uuid> ...`
- copy task ID
- `task result <taskid>`

### Common Flow: Review Implant State

1. navigate implant list
2. details pane updates immediately
3. recent activity pane shows recent tasks/errors for that implant

### Common Flow: Recover From Failure

1. activity pane highlights recent error
2. selected implant remains visible
3. operator can re-task from the same context

## Recommended UI Features

### Active Implant Context

The TUI should maintain an active implant separate from transient cursor movement if needed.

This allows:

- browsing one implant while keeping another as the command target
- explicit operator control over command targeting

Recommended first version:

- selection and active implant are the same

Future extension:

- `Space` marks active implant
- cursor can move independently

### Short ID Display

Display a short ID in the list, for example:

```text
a1f4acb4
```

but show the full UUID in details and copy/export actions.

### Result Preview

For the selected implant, show:

- latest task kind
- current task state
- most recent result snippet

This avoids forcing a separate result query for common cases.

### Inline Actions

Recommended keybindings for first version:

- `q` quit
- `j` / `k` or arrow keys: move
- `Enter`: select implant
- `:`: command mode
- `/`: filter implants
- `r`: refresh
- `t`: task actions
- `i`: implant details focus
- `?`: help

### Command Completion

Recommended completion targets:

- top-level command verbs
- subcommands
- task kinds
- payload names
- `active`

This is one of the highest-value ergonomic wins.

## Task Action UX

A task action overlay or popup is preferable to forcing every task through raw command input.

Recommended initial task menu:

```text
Task Actions
- execute_coff / whoami
- view latest result
- copy full client ID
```

Future task kinds can be added here without changing the command pane.

This should be driven by implant capabilities when possible.

If the selected implant does not support a task kind:

- disable it visually
- explain why

That aligns with the teamserver capability-aware design.

## Data Flow And Architecture

The TUI should not bypass the current application/service layer.

Recommended architecture:

```text
ui/
- app.rs
- state.rs
- events.rs
- layout.rs
- widgets/
  - implants.rs
  - details.rs
  - activity.rs
  - command.rs
  - help.rs
- actions.rs
- controller.rs
```

### Responsibilities

`ui::state`

- local UI state only:
  - selected row
  - focused pane
  - filter text
  - command buffer
  - command history cursor
  - activity scroll offset

`ui::controller`

- translates key events into actions
- requests data from application services
- dispatches commands

`ui::widgets`

- rendering only
- no business logic

`ui::actions`

- typed UI actions such as:
  - `SelectNextImplant`
  - `SelectPreviousImplant`
  - `QueueWhoami`
  - `FocusCommand`
  - `SubmitCommand`

## Integration With Existing Command System

The current command system is already modular.

That is useful because the TUI can treat the command interpreter as a frontend over the same command dispatcher.

Recommended integration:

- reuse current parser and command handlers where possible
- add a thin adapter that resolves `active` into a concrete UUID before command parsing
- expose typed service calls for common UI actions rather than shelling everything through raw strings

Important design rule:

- common actions should call typed services directly
- command pane should remain available for advanced and fallback workflows

That keeps the UI ergonomic without making it rigid.

## Refresh Strategy

The UI needs periodic data refresh without flicker or blocking input.

Recommended model:

- event loop polls input frequently
- background tick every 250ms to 1000ms
- implant list refresh on tick
- activity updates on task or heartbeat changes

Recommended first refresh intervals:

- UI render tick: 100ms to 250ms
- data refresh tick: 1s

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

The activity pane can render this event stream with severity-aware styling.

## Styling And Visual Language

For a terminal UI, visual clarity matters more than novelty.

Recommended style guidance:

- strong pane borders and titles
- selected implant row visibly highlighted
- status colors:
  - idle/healthy: green
  - busy: yellow
  - error/offline: red
- recent event severities similarly color-coded
- command pane should feel distinct but not dominant

Avoid:

- excessive color noise
- dense borders everywhere
- scrolling regions without focus indicators

## Extensibility Recommendations

### 1. Separate Screen State From Domain State

Do not make widgets fetch application state directly.

Instead:

- controller gathers data
- controller updates a UI view model
- widgets render the view model

This keeps the UI testable and avoids hidden coupling.

### 2. Use Typed UI Actions

Avoid giant key-handling match blocks that directly mutate everything.

Prefer:

```text
KeyEvent -> UiAction -> reducer/controller -> state update + service call
```

This makes future panes and commands additive.

### 3. Capability-Aware Actions

Task menus and quick actions should derive from implant capabilities instead of a hard-coded universal list.

That keeps the TUI aligned with the teamserver’s implant-family extensibility goals.

### 4. Keep The Command Pane

Do not remove the command interpreter just because the TUI exists.

It remains useful for:

- advanced workflows
- debugging
- newly added features before dedicated widgets exist

### 5. Design For More Screens

Likely future screens:

- task history screen
- payload browser
- implant details full-screen view
- audit/log view
- grouped implants or tags view

The first TUI version should keep those as future tabs or routes rather than trying to fit everything into one screen.

## Suggested First Milestone

The first TUI milestone should be intentionally narrow.

Implement:

- full-screen app shell
- implant table
- implant details pane
- activity pane
- command input pane
- active implant resolution in commands
- key hints/help overlay

That alone removes the biggest workflow pain.

Do not try to add:

- multi-screen navigation
- popup forms for every task kind
- extensive log viewers

until the basic operator loop feels good.

## Suggested Second Milestone

After the first milestone works:

- add quick task actions for common workflows
- add command completion
- add result preview improvements
- add filters and sorts
- add capability-aware action availability

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

## Acceptance Criteria

The first TUI phase is successful when:

- the operator can navigate active implants without copying UUIDs
- the selected implant’s key host information is visible at a glance
- the operator can queue a task against the selected implant without manually pasting the UUID
- the operator can review recent task results without leaving the main screen
- the command interpreter remains available for advanced operations
- the UI structure is modular enough to add new panes and actions without centralizing everything in one file
