use std::io::{Error, ErrorKind, Result};

use super::packet::{Packet, PacketOpcode};

#[derive(Debug, Clone, Default)]
pub struct PacketRequestContext {
    pub registration_header: Option<String>,
}

pub trait AdmissionPolicy: Send + Sync {
    /// Validates whether a packet is allowed to proceed for the current request.
    ///
    /// @param request Request-scoped metadata extracted from the transport layer.
    /// @param packet Parsed packet envelope being evaluated.
    /// @return `Ok(())` when the request is authorized, otherwise an I/O error.
    fn validate(&self, request: &PacketRequestContext, packet: &Packet) -> Result<()>;
}

#[derive(Clone, Default)]
pub struct RegisterHeaderPolicy;

impl RegisterHeaderPolicy {
    const EXPECTED_VALUE: &'static str = "coff-loader-v1";
}

impl AdmissionPolicy for RegisterHeaderPolicy {
    /// Validates the registration header for incoming register packets.
    ///
    /// @param request Request-scoped metadata extracted from the transport layer.
    /// @param packet Parsed packet envelope being evaluated.
    /// @return `Ok(())` when the register packet contains the expected header value.
    fn validate(&self, request: &PacketRequestContext, packet: &Packet) -> Result<()> {
        if packet.opcode_kind() != PacketOpcode::Register {
            return Ok(());
        }

        let authorized = request
            .registration_header
            .as_deref()
            .map(|value| value == Self::EXPECTED_VALUE)
            .unwrap_or(false);

        if authorized {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::PermissionDenied,
                "missing or invalid registration header value",
            ))
        }
    }
}
