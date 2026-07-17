//! Minecraft 1.8.9 protocol codec (protocol version 47).
//!
//! Packet format: VarInt length + VarInt packet ID + payload.
//! All multi-byte values are big-endian.

use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: i32 = 47;

// --- VarInt ---

pub fn encode_varint(mut value: i32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    loop {
        let mut byte = (value as u32 & 0x7F) as u8;
        value = ((value as u32) >> 7) as i32;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
    buf
}

pub fn decode_varint(read: &mut impl Read) -> io::Result<i32> {
    let mut result: i32 = 0;
    for shift in 0..5 {
        let mut byte = [0u8; 1];
        read.read_exact(&mut byte)?;
        result |= ((byte[0] & 0x7F) as i32) << (shift * 7);
        if byte[0] & 0x80 == 0 {
            return Ok(result);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "VarInt too long",
    ))
}

// --- Packet reading/writing ---

/// Read one packet from the stream. Returns (packet_id, payload_bytes).
pub fn read_packet(stream: &mut impl Read) -> io::Result<(i32, Vec<u8>)> {
    read_packet_with_compression(stream, None)
}

/// Read one packet, handling the post-login compression wrapper when enabled.
pub fn read_packet_with_compression(
    stream: &mut impl Read,
    compression_threshold: Option<i32>,
) -> io::Result<(i32, Vec<u8>)> {
    let length = decode_varint(stream)? as usize;
    let mut frame = vec![0u8; length];
    stream.read_exact(&mut frame)?;

    let payload = if compression_threshold.is_some() {
        let mut frame_buf = PacketBuffer::new(frame);
        let data_length = frame_buf.read_varint()?;
        let compressed = frame_buf.read_remaining_bytes();
        if data_length == 0 {
            compressed
        } else {
            let mut decoder = ZlibDecoder::new(&compressed[..]);
            let mut decompressed = Vec::with_capacity(data_length as usize);
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        }
    } else {
        frame
    };

    let mut payload_buf = PacketBuffer::new(payload);
    let packet_id = payload_buf.read_varint()?;
    Ok((packet_id, payload_buf.read_remaining_bytes()))
}

/// Read one uncompressed packet from the stream. Returns (packet_id, payload_bytes).
pub fn read_packet_legacy(stream: &mut impl Read) -> io::Result<(i32, Vec<u8>)> {
    let length = decode_varint(stream)?;
    let packet_id = decode_varint(stream)?;
    let id_bytes = encode_varint(packet_id);
    let remaining = length as usize - id_bytes.len();
    let mut payload = vec![0u8; remaining];
    stream.read_exact(&mut payload)?;
    Ok((packet_id, payload))
}

/// Write a packet to the stream.
pub fn write_packet(stream: &mut impl Write, packet_id: i32, payload: &[u8]) -> io::Result<()> {
    write_packet_with_compression(stream, packet_id, payload, None)
}

/// Write a packet, optionally using the post-login compression wrapper.
pub fn write_packet_with_compression(
    stream: &mut impl Write,
    packet_id: i32,
    payload: &[u8],
    compression_threshold: Option<i32>,
) -> io::Result<()> {
    let id_bytes = encode_varint(packet_id);
    let mut packet_data = Vec::with_capacity(id_bytes.len() + payload.len());
    packet_data.extend_from_slice(&id_bytes);
    packet_data.extend_from_slice(payload);

    let frame = if let Some(threshold) = compression_threshold {
        if packet_data.len() >= threshold as usize {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&packet_data)?;
            let compressed = encoder.finish()?;

            let mut out = encode_varint(packet_data.len() as i32);
            out.extend_from_slice(&compressed);
            out
        } else {
            let mut out = encode_varint(0);
            out.extend_from_slice(&packet_data);
            out
        }
    } else {
        packet_data
    };

    let length_bytes = encode_varint(frame.len() as i32);
    stream.write_all(&length_bytes)?;
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

// --- Packet buffer (for reading/writing structured data) ---

pub struct PacketBuffer {
    data: Vec<u8>,
    read_pos: usize,
}

impl PacketBuffer {
    pub fn new(data: Vec<u8>) -> Self {
        PacketBuffer { data, read_pos: 0 }
    }

    pub fn empty() -> Self {
        PacketBuffer {
            data: Vec::new(),
            read_pos: 0,
        }
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.read_pos
    }

    pub fn position(&self) -> usize {
        self.read_pos
    }

    pub fn slice(&self, start: usize, end: usize) -> io::Result<&[u8]> {
        if start > end || end > self.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "slice out of bounds",
            ));
        }
        Ok(&self.data[start..end])
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.data
    }

    // --- Read methods ---

    pub fn read_byte(&mut self) -> io::Result<i8> {
        if self.read_pos >= self.data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof"));
        }
        let v = self.data[self.read_pos] as i8;
        self.read_pos += 1;
        Ok(v)
    }

    pub fn read_unsigned_byte(&mut self) -> io::Result<u8> {
        Ok(self.read_byte()? as u8)
    }

    pub fn read_short(&mut self) -> io::Result<i16> {
        let b0 = self.read_unsigned_byte()? as i16;
        let b1 = self.read_unsigned_byte()? as i16;
        Ok((b0 << 8) | b1)
    }

    pub fn read_unsigned_short(&mut self) -> io::Result<u16> {
        Ok(self.read_short()? as u16)
    }

    pub fn read_int(&mut self) -> io::Result<i32> {
        let b0 = self.read_unsigned_byte()? as i32;
        let b1 = self.read_unsigned_byte()? as i32;
        let b2 = self.read_unsigned_byte()? as i32;
        let b3 = self.read_unsigned_byte()? as i32;
        Ok((b0 << 24) | (b1 << 16) | (b2 << 8) | b3)
    }

    pub fn read_long(&mut self) -> io::Result<i64> {
        let hi = self.read_int()? as i64;
        let lo = self.read_int()? as i64;
        Ok((hi << 32) | (lo & 0xFFFFFFFF))
    }

    pub fn read_float(&mut self) -> io::Result<f32> {
        Ok(f32::from_bits(self.read_int()? as u32))
    }

    pub fn read_double(&mut self) -> io::Result<f64> {
        Ok(f64::from_bits(self.read_long()? as u64))
    }

    pub fn read_varint(&mut self) -> io::Result<i32> {
        let mut result: i32 = 0;
        for shift in 0..5 {
            let byte = self.read_unsigned_byte()?;
            result |= ((byte & 0x7F) as i32) << (shift * 7);
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "VarInt too long",
        ))
    }

    pub fn read_varlong(&mut self) -> io::Result<i64> {
        let mut result: i64 = 0;
        for shift in 0..10 {
            let byte = self.read_unsigned_byte()?;
            result |= ((byte & 0x7F) as i64) << (shift * 7);
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "VarLong too long",
        ))
    }

    pub fn read_string(&mut self) -> io::Result<String> {
        let len = self.read_varint()? as usize;
        if self.read_pos + len > self.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "string too long",
            ));
        }
        let s = String::from_utf8(self.data[self.read_pos..self.read_pos + len].to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid utf8"))?;
        self.read_pos += len;
        Ok(s)
    }

    pub fn read_uuid(&mut self) -> io::Result<String> {
        let most = self.read_long()? as u64;
        let least = self.read_long()? as u64;
        let value = ((most as u128) << 64) | least as u128;
        Ok(format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            (value >> 96) as u32,
            ((value >> 80) & 0xffff) as u16,
            ((value >> 64) & 0xffff) as u16,
            ((value >> 48) & 0xffff) as u16,
            value & 0xffffffffffff
        ))
    }

    pub fn read_bool(&mut self) -> io::Result<bool> {
        Ok(self.read_byte()? != 0)
    }

    pub fn read_bytes(&mut self, len: usize) -> io::Result<Vec<u8>> {
        if self.read_pos + len > self.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "not enough bytes",
            ));
        }
        let bytes = self.data[self.read_pos..self.read_pos + len].to_vec();
        self.read_pos += len;
        Ok(bytes)
    }

    pub fn skip(&mut self, len: usize) -> io::Result<()> {
        if self.read_pos + len > self.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "skip past eof",
            ));
        }
        self.read_pos += len;
        Ok(())
    }

    pub fn read_remaining_bytes(&mut self) -> Vec<u8> {
        let bytes = self.data[self.read_pos..].to_vec();
        self.read_pos = self.data.len();
        bytes
    }

    // --- Write methods ---

    pub fn write_byte(&mut self, v: i8) {
        self.data.push(v as u8);
    }

    pub fn write_unsigned_byte(&mut self, v: u8) {
        self.data.push(v);
    }

    pub fn write_short(&mut self, v: i16) {
        self.data.extend_from_slice(&(v.to_be_bytes()));
    }

    pub fn write_int(&mut self, v: i32) {
        self.data.extend_from_slice(&(v.to_be_bytes()));
    }

    pub fn write_long(&mut self, v: i64) {
        self.data.extend_from_slice(&(v.to_be_bytes()));
    }

    pub fn write_float(&mut self, v: f32) {
        self.data.extend_from_slice(&v.to_bits().to_be_bytes());
    }

    pub fn write_double(&mut self, v: f64) {
        self.data.extend_from_slice(&v.to_bits().to_be_bytes());
    }

    pub fn write_varint(&mut self, mut v: i32) {
        loop {
            let mut byte = (v as u32 & 0x7F) as u8;
            v = ((v as u32) >> 7) as i32;
            if v != 0 {
                byte |= 0x80;
            }
            self.data.push(byte);
            if v == 0 {
                break;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        self.write_varint(s.len() as i32);
        self.data.extend_from_slice(s.as_bytes());
    }

    pub fn write_bool(&mut self, v: bool) {
        self.write_byte(if v { 1 } else { 0 });
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }
}
