use std::{
    collections::HashMap, convert::Infallible, net::{Incoming, SocketAddr}, 
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    clone::Clone,
    io::{
        Write,
        Result,
    },
};

use serde::{Serialize, Deserialize};
use serde_json::{Value, json};

use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct BasePacket {
    clientid: String,
    data: String,
}


#[derive(Debug)]
pub struct Packet {
    opcode: u8,
    peer_addr: SocketAddr,
    clientid: String,
    data: String,
    data_length: usize,
}

pub enum PacketOpcode {
    REGISTER,
    FETCH_TASK,
    TASK_RESULT,
    UNKNOWN,
}

impl PacketOpcode {
    pub fn to_u8(&self) -> u8 {
        match self {
            PacketOpcode::REGISTER => 0x00,
            PacketOpcode::FETCH_TASK => 0x01,
            PacketOpcode::TASK_RESULT => 0x02,
            PacketOpcode::UNKNOWN => 0xff,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0x00 => PacketOpcode::REGISTER,
            0x01 => PacketOpcode::FETCH_TASK,
            0x02 => PacketOpcode::TASK_RESULT,
            _ => PacketOpcode::UNKNOWN,
        }
    }
}

impl Packet {
    pub fn new(peer_addr: SocketAddr, buf: &[u8]) -> Result<Self> {


        let test_packet = BasePacket { 
            clientid: "513a666c-3349-40dd-9462-95c4449b0d0d".to_string(),
            data: "junk data".to_string()
        };
        println!("{:#?}", serde_json::to_string_pretty(&test_packet));


        let opcode = buf[0];

        println!("opcode: {}", opcode);

        let base_str = match String::from_utf8(buf[1..].to_vec()) {
            Ok(p) => p,
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Error parsing base packet data",
                ))
            }
        };

        println!("{:#?}", base_str);
        let base: BasePacket = serde_json::from_str(&base_str)?;

        Ok(Self { 
            opcode, 
            peer_addr, 
            clientid: base.clientid, 
            data: base.data, 
            data_length: buf.len(),
         })
    }

    pub fn build<T: Serialize>(opcode: u8, clientid: &String, data: T)  -> Result<Vec<u8>> {
        let mut ret = vec![];
        ret.push(opcode);

        let data = serde_json::to_string(&data)?;

        let base = BasePacket {
            clientid: clientid.clone(),
            data,
        };

        let data = serde_json::to_string(&base)?;
        ret.append(&mut data.as_bytes().to_vec());

        Ok(ret)
    }

    pub fn opcode(&self) -> u8 {
        self.opcode
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    pub fn clientid(&self) -> String {
        self.clientid.clone()
    }
}

pub fn packet_cb(p: Packet) {
    match PacketOpcode::from_u8(p.opcode()) {
        PacketOpcode::REGISTER => todo!(),
        PacketOpcode::FETCH_TASK => println!("cb triggered for fetch task packet"),
        PacketOpcode::TASK_RESULT => todo!(),
        PacketOpcode::UNKNOWN => println!("cb triggered for packet unknown"),

    }
}
// let packet = r#"{"clientid": "513a666c-3349-40dd-9462-95c4449b0d0d","data": "junk data"}"#;