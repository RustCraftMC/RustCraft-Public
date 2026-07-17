use super::ClientboundPacket;
use crate::net::protocol::PacketBuffer;
use std::io;

impl ClientboundPacket {
    pub fn parse_login(id: i32, data: &[u8]) -> io::Result<Self> {
        let mut buf = PacketBuffer::new(data.to_vec());
        match id {
            0x01 => {
                let server_id = buf.read_string()?;
                let public_key_len = buf.read_varint()?.max(0) as usize;
                let public_key = buf.read_bytes(public_key_len)?;
                let verify_token_len = buf.read_varint()?.max(0) as usize;
                let verify_token = buf.read_bytes(verify_token_len)?;
                Ok(ClientboundPacket::EncryptionRequest {
                    server_id,
                    public_key,
                    verify_token,
                })
            }
            0x02 => Ok(ClientboundPacket::LoginSuccess {
                uuid: buf.read_string()?,
                username: buf.read_string()?,
            }),
            0x03 => Ok(ClientboundPacket::SetCompression {
                threshold: buf.read_varint()?,
            }),
            0x00 => Ok(ClientboundPacket::Disconnect {
                reason: buf.read_string()?,
            }),
            _ => Ok(ClientboundPacket::Unknown { id }),
        }
    }
}
