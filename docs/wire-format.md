# Teamserver Wire Format

This document describes the current implant-facing packet format used by the `malice` teamserver.

It reflects the current implementation in:

- [packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs)
- [httpserver.rs](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs)
- [router](/C:/Users/wammu/source/repos/malice/src/util/router)
- [types.rs](/C:/Users/wammu/source/repos/malice/src/util/tasks/types.rs)
- [protocol.h](/C:/Users/wammu/source/repos/malice/implant/zant/runtime/protocol.h)

## Status

- implemented today: 1-byte opcode prefix plus UTF-8 JSON body
- implemented today: `REGISTER`, `FETCH_TASK`, `TASK_RESULT`, and `HEARTBEAT`
- implemented today: one implant-facing route, `POST /packet`
- not implemented today: explicit packet version field, explicit packet length field, binary framing, checksums, encryption, or signing

## Packet Layout

Each packet is encoded as:

```text
+----------+--------------------------+
| Offset   | Field                    |
+----------+--------------------------+
| 0        | opcode                   |
| 1..N-1   | outer packet JSON, UTF-8 |
+----------+--------------------------+
```

The outer packet JSON has this shape:

```json
{
  "clientid": "<uuid string or empty string>",
  "data": "<json string>"
}
```

Important detail:

- `data` is not a nested JSON object
- `data` is itself serialized JSON text stored as a string inside the outer JSON object

Encoding works like this:

1. write the opcode as a single byte
2. serialize the logical payload to JSON text
3. store that JSON text in the outer `data` field
4. serialize the outer object to UTF-8 JSON
5. append the outer JSON bytes after the opcode byte

## Packet Fields

### `opcode`

- size: 1 byte
- type: unsigned 8-bit integer
- current values:
  - `0x00` `Register`
  - `0x01` `FetchTask`
  - `0x02` `TaskResult`
  - `0x03` `Heartbeat`
  - any other value maps to `Unknown`

### `clientid`

- type: JSON string
- content: UUID text or empty string during first registration
- example: `513a666c-3349-40dd-9462-95c4449b0d0d`

### `data`

- type: JSON string
- content: serialized inner payload JSON
- encoding: UTF-8 as part of the outer JSON document

## Alignment And Endianness

- the packet is byte-packed
- there is no struct-layout dependency because the body is JSON text
- there are no multi-byte binary fields in the current packet framing

If a future protocol version adds binary framing, it should define:

- endianness
- packet length
- binary encoding rules
- version negotiation behavior

## Serialization Ownership

### Teamserver

The teamserver is authoritative for the current framing rules.

In [packet.rs](/C:/Users/wammu/source/repos/malice/src/util/packet.rs):

- `Packet::new(...)` parses the opcode-prefixed packet
- `Packet::build(...)` builds the opcode-prefixed packet
- `Packet::parse_data<T>(...)` deserializes the inner `data` JSON string

### Implant

An implant sender must:

- emit the opcode byte first
- serialize the inner payload to JSON text
- embed that text in the outer `data` string
- serialize the outer envelope as UTF-8 JSON

An implant receiver must:

- read byte `0` as opcode
- parse bytes `1..` as the outer JSON envelope
- parse the inner `data` string as JSON

## HTTP Transport

The implant-facing endpoint is:

- `POST /packet`

The HTTP transport is only a shim around the packet format:

1. accept the raw request body
2. parse it into `Packet`
3. apply admission policy
4. route by opcode
5. return the handler response

Registration currently also requires this header:

- `x-malice-register: coff-loader-v1`

That check is enforced by the admission layer in [admission.rs](/C:/Users/wammu/source/repos/malice/src/util/admission.rs), not by the packet framing itself.

## Inner Payload Schemas

The current packet framing is generic. These are the current logical payloads transported inside `data`.

### Register

Opcode: `0x00`

Implant to teamserver:

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

Teamserver to implant:

```json
{
  "status": "ok",
  "clientid": "513a666c-3349-40dd-9462-95c4449b0d0d"
}
```

### Heartbeat

Opcode: `0x03`

Implant to teamserver:

```json
{
  "sequence": 1,
  "status": "idle"
}
```

Typical `status` values today:

- `idle`
- `busy`
- `error`

### Fetch Task

Opcode: `0x01`

Implant to teamserver:

```json
{
  "want": 1
}
```

Teamserver to implant:

```json
{
  "tasks": [
    {
      "task_id": "d4fcb0c2-3414-4e11-9a2f-b9f0d86c4377",
      "task_type": "execute_coff",
      "object_name": "whoami.obj",
      "entrypoint": "main",
      "object_encoding": "base64",
      "object_data": "<base64 object bytes>",
      "args_encoding": "base64",
      "args_data": ""
    }
  ]
}
```

Important current behavior:

- task responses are always a JSON object with a `tasks` array
- empty work is returned as `{"tasks":[]}`
- task envelopes are serialized by the selected server-side implant integration

### Task Result

Opcode: `0x02`

Implant to teamserver:

```json
{
  "task_id": "d4fcb0c2-3414-4e11-9a2f-b9f0d86c4377",
  "status": "success",
  "result_encoding": "utf8",
  "result_data": "stub implant executed task"
}
```

Typical `status` values today:

- `success`
- `error`

## Example Packet

Example `FETCH_TASK` request with `want = 1`.

Inner payload:

```json
{
  "want": 1
}
```

Outer packet:

```json
{
  "clientid": "513a666c-3349-40dd-9462-95c4449b0d0d",
  "data": "{\"want\":1}"
}
```

Wire bytes begin with:

```text
01 7B 22 63 6C 69 65 6E 74 69 64 22 3A 22 ...
```

Where:

- `01` is the opcode byte
- the remaining bytes are the UTF-8 encoding of the outer JSON document

## Versioning Rules

There is no explicit packet version field today. Use these rules:

1. treat the current layout as protocol v1 by convention
2. do not change existing opcode meanings in place
3. do not change `clientid` from JSON string to binary UUID in place
4. do not change `data` from JSON-string-wrapped payload to nested object in place
5. if binary-safe framing is needed, add a new explicit packet version

Any future versioned framing should include:

- explicit version identifier
- explicit payload length
- binary encoding rules
- unknown-version handling

## Current Limitations

- framing is text-oriented
- object bytes are base64 in JSON, not raw binary
- there is no chunking for large payloads
- there is no packet authentication or encryption
- there is no explicit packet version field

Those limitations are acceptable for the current lab-oriented interop stage, but new implants should treat them as current constraints rather than permanent design.
