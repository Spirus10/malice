# Teamserver Wire Format

This document defines the current packet format used by the Rust teamserver and establishes the compatibility rules that new implants, including `coff_loader`, must follow.

## Status

- Current implementation owner:
  - decode: [`src/util/packet.rs`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs)
  - HTTP endpoint shim: [`src/util/httpserver.rs`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs)
  - task payload producer: [`src/util/tasks.rs`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs)
- Transport status:
  - implemented today: custom 1-byte opcode prefix plus UTF-8 JSON body
  - not implemented today: explicit schema version field, fixed-width length prefixes, checksum, binary framing

## Packed Layout

Each packet is encoded as:

```text
+----------+-------------------------------+
| Offset   | Field                         |
+----------+-------------------------------+
| 0        | opcode                        |
| 1..N-1   | base packet JSON, UTF-8 bytes |
+----------+-------------------------------+
```

`base packet JSON` is a serialized object with this logical shape:

```json
{
  "clientid": "<uuid string>",
  "data": "<json string>"
}
```

Important detail: `data` is not embedded as a nested JSON object. It is serialized once on its own, then embedded as a JSON string inside the outer JSON object.

That means the wire format is effectively:

1. Emit `opcode` as a single byte.
2. Serialize the logical payload object to JSON.
3. Serialize that JSON text again as the `data` string inside the outer object.
4. Append the outer JSON UTF-8 bytes directly after the opcode byte.

## Field Definitions

### Header

- `opcode`
  - size: 1 byte
  - type: unsigned 8-bit integer
  - alignment: none; byte-packed
  - endianness: not applicable for a single byte
  - values currently defined in [`src/util/packet.rs`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs):
    - `0x00`: `REGISTER`
    - `0x01`: `FETCH_TASK`
    - `0x02`: `TASK_RESULT`
    - any other value maps to `UNKNOWN`

### Body

- `clientid`
  - size: variable
  - type: JSON string
  - current content: UUID text, for example `513a666c-3349-40dd-9462-95c4449b0d0d`
  - encoding: UTF-8 bytes as part of the outer JSON document

- `data`
  - size: variable
  - type: JSON string
  - content: serialized JSON payload, not raw bytes
  - encoding: UTF-8 bytes as part of the outer JSON document

## Alignment And Endianness

- The packet is byte-packed.
- There is no compiler-struct layout dependency on the wire because the body is JSON text.
- There are no multi-byte integer fields in the current wire format, so endianness only matters if a future version adds binary numeric fields.
- Any future binary extension must define endianness explicitly. Unless there is a strong reason otherwise, use little-endian for new multi-byte numeric fields because both the teamserver target and the `coff_loader` implant are currently Windows-centric.

## Serialization Ownership

### Teamserver responsibilities

- The teamserver owns authoritative encode/decode behavior for the current protocol in [`src/util/packet.rs`](/C:/Users/wammu/source/repos/malice/src/util/packet.rs).
- `Packet::new(...)` is the current deserializer:
  - byte `0` is interpreted as the opcode
  - bytes `1..` are interpreted as UTF-8 JSON
  - the JSON must deserialize into `BasePacket { clientid: String, data: String }`
- `Packet::build(...)` is the current serializer:
  - serialize the payload argument to JSON text
  - place that JSON text in `BasePacket.data`
  - serialize `BasePacket`
  - prepend the opcode byte

### HTTP shim responsibilities

- `/tasks` currently maps HTTP requests into packet parsing by prepending the `FETCH_TASK` opcode before calling `Packet::new(...)` in [`src/util/httpserver.rs`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs).
- This means the HTTP route currently supplies transport context that is not present in the incoming request body itself.

### Implant responsibilities

- Implants must treat the Rust teamserver serializer/deserializer behavior as the source of truth until both sides are updated together.
- An implant sender must:
  - emit the 1-byte opcode
  - send outer JSON encoded in UTF-8
  - ensure `data` contains serialized JSON text, not an object inserted directly
- An implant receiver must:
  - read byte `0` as opcode
  - parse bytes `1..` as UTF-8 JSON
  - parse `data` as a JSON string containing the logical payload

## Versioning Rules

There is no explicit version field in the current packet format. For interop work, use these rules:

1. Treat the current layout as protocol version `v1` by convention, even though the bytes do not carry that marker.
2. Do not change the meaning of existing opcodes `0x00`, `0x01`, or `0x02` in place.
3. Do not change `clientid` from a JSON string to a binary UUID without introducing a new versioned format.
4. Do not change `data` from JSON-string-wrapped payload to nested JSON object in place. That would break existing parsers.
5. If `coff_loader` needs binary payloads, introduce either:
   - a new opcode whose `data` string carries an encoded representation such as base64, or
   - a new packet version with an explicit version byte/field and binary-safe framing
6. Any future versioned format should include:
   - explicit version identifier
   - explicit payload length
   - explicit encoding rules for binary data
   - backward-compatibility handling for unknown versions

## Sample Payloads

### Example 1: `FETCH_TASK`

Logical fields:

- opcode: `0x01`
- clientid: `513a666c-3349-40dd-9462-95c4449b0d0d`
- inner payload object:

```json
{
  "request": "tasks"
}
```

Inner payload JSON text:

```json
"{\"request\":\"tasks\"}"
```

Outer JSON body:

```json
{
  "clientid": "513a666c-3349-40dd-9462-95c4449b0d0d",
  "data": "{\"request\":\"tasks\"}"
}
```

Wire bytes:

```text
01 7B 22 63 6C 69 65 6E 74 69 64 22 3A 22 35 31 ...
```

Interpretation:

- `01` is the opcode byte
- the remaining bytes are the UTF-8 encoding of the outer JSON object

### Example 2: task result payload

Logical inner payload object:

```json
{
  "task_type": "ping",
  "task_options": ["verbose", "etc"]
}
```

Outer JSON body sent after opcode `0x02`:

```json
{
  "clientid": "513a666c-3349-40dd-9462-95c4449b0d0d",
  "data": "{\"task_type\":\"ping\",\"task_options\":[\"verbose\",\"etc\"]}"
}
```

### Example 3: current task response behavior

`TaskManager::get_tasks(...)` in [`src/util/tasks.rs`](/C:/Users/wammu/source/repos/malice/src/util/tasks.rs) currently returns a vector of serialized task JSON strings, and [`src/util/httpserver.rs`](/C:/Users/wammu/source/repos/malice/src/util/httpserver.rs) concatenates them with `join(\"\")`.

With one task queued, the response body looks like:

```json
{"task_type":"ping","task_options":["verbose","etc"]}
```

With multiple tasks queued, the current implementation would concatenate objects directly:

```text
{"task_type":"ping","task_options":["verbose","etc"]}{"task_type":"sleep","task_options":["30"]}
```

That output is not valid JSON as a whole. Any implant work should avoid depending on multi-task responses until that framing is revised.

## Interop Notes For `coff_loader`

- The current protocol is text-oriented and not binary-safe by design.
- Raw COFF object bytes should not be inserted directly into `data` without an agreed encoding layer.
- If the implant needs to receive object code, symbol data, relocation metadata, or execution results, define those payloads as versioned logical schemas first, then choose one of:
  - JSON-only metadata plus separately encoded binary blob
  - base64 inside `data`
  - a new framed binary protocol version

For the immediate integration work, preserve the current opcode + UTF-8 JSON framing unless both the teamserver and the implant are changed together under a new explicit version.
