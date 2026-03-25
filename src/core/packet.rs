//! Packet wire-format helpers shared by the HTTP ingress and packet handlers.
//!
//! Wire format:
//!
//!   +--------+---------------------------------------------+
//!   | byte 0 | opcode                                      |
//!   +--------+---------------------------------------------+
//!   | bytes  | UTF-8 JSON for `BasePacket`                 |
//!   +--------+---------------------------------------------+
//!
//! `BasePacket` JSON:
//!
//!   {
//!     "clientid": "<uuid or empty string>",
//!     "data": "<JSON string for opcode-specific payload>"
//!   }
//!
//! Decode path:
//!
//!   raw bytes
//!      -> Packet::new(...)
//!      -> opcode + BasePacket
//!      -> Packet::parse_data::<T>()
//!      -> typed payload for the selected handler

use std::{
    io::{Error, ErrorKind, Result},
    net::SocketAddr,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct BasePacket {
    clientid: String,
    data: String,
}

#[derive(Debug, Clone)]
pub struct Packet {
    opcode: u8,
    clientid: String,
    data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacketOpcode {
    Register,
    FetchTask,
    TaskResult,
    Heartbeat,
    Unknown,
}

impl PacketOpcode {
    /// Converts a packet opcode into its wire-format byte value.
    ///
    /// @return Opcode byte used on the wire.
    pub fn to_u8(&self) -> u8 {
        match self {
            PacketOpcode::Register => 0x00,
            PacketOpcode::FetchTask => 0x01,
            PacketOpcode::TaskResult => 0x02,
            PacketOpcode::Heartbeat => 0x03,
            PacketOpcode::Unknown => 0xff,
        }
    }

    /// Parses a wire-format opcode byte.
    ///
    /// @param v Opcode byte read from the wire.
    /// @return Packet opcode variant matching the byte.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0x00 => PacketOpcode::Register,
            0x01 => PacketOpcode::FetchTask,
            0x02 => PacketOpcode::TaskResult,
            0x03 => PacketOpcode::Heartbeat,
            _ => PacketOpcode::Unknown,
        }
    }
}

impl Packet {
    /// Parses the opcode-prefixed packet bytes received over HTTP.
    ///
    /// @param _peer_addr Remote socket address associated with the packet.
    /// @param buf Raw opcode-prefixed packet bytes.
    /// @return Decoded packet envelope or an I/O error if parsing fails.
    pub fn new(_peer_addr: SocketAddr, buf: &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, "Packet buffer is empty"));
        }

        let opcode = buf[0];
        let base_str = String::from_utf8(buf[1..].to_vec())
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Error parsing base packet data"))?;
        let base: BasePacket = serde_json::from_str(&base_str)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        Ok(Self {
            opcode,
            clientid: base.clientid,
            data: base.data,
        })
    }

    /// Builds the opcode-prefixed wire format used by implants and the teamserver.
    ///
    /// @param opcode Packet opcode to encode in the first byte.
    /// @param clientid Implant identifier stored in the outer envelope.
    /// @param data Serializable payload stored in the outer envelope.
    /// @return Encoded packet bytes or an I/O error if serialization fails.
    pub fn build<T: Serialize>(opcode: PacketOpcode, clientid: &str, data: &T) -> Result<Vec<u8>> {
        let mut ret = vec![opcode.to_u8()];
        let data = serde_json::to_string(data)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        let base = BasePacket {
            clientid: clientid.to_string(),
            data,
        };

        let data = serde_json::to_string(&base)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
        ret.extend_from_slice(data.as_bytes());

        Ok(ret)
    }

    /// Deserializes the inner packet payload into the requested type.
    ///
    /// @return Parsed payload or an I/O error if JSON deserialization fails.
    pub fn parse_data<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.data)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))
    }

    /// Returns the typed packet opcode for the current envelope.
    ///
    /// @return Packet opcode derived from the stored opcode byte.
    pub fn opcode_kind(&self) -> PacketOpcode {
        PacketOpcode::from_u8(self.opcode)
    }

    /// Returns the outer packet client identifier.
    ///
    /// @return Client identifier stored in the packet envelope.
    pub fn clientid(&self) -> &str {
        &self.clientid
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use serde_json::json;

    use super::{Packet, PacketOpcode};

    #[test]
    /// Verifies that packet serialization preserves opcode and payload fields.
    fn packet_round_trip_preserves_opcode_and_inner_json() {
        let clientid = "513a666c-3349-40dd-9462-95c4449b0d0d";
        let payload = json!({ "want": 1 });
        let bytes = Packet::build(PacketOpcode::FetchTask, clientid, &payload).unwrap();

        let packet = Packet::new((IpAddr::V4(Ipv4Addr::LOCALHOST), 42069).into(), &bytes).unwrap();
        let parsed_payload: serde_json::Value = packet.parse_data().unwrap();

        assert_eq!(packet.opcode_kind(), PacketOpcode::FetchTask);
        assert_eq!(packet.clientid(), clientid);
        assert_eq!(parsed_payload, payload);
    }
}
