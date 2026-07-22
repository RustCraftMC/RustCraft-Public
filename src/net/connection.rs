//! TCP connection and packet routing for Minecraft 1.8.9.

use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
use aes::Aes128;
use rand::RngCore;
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use sha1::{Digest, Sha1};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// TCP connect timeout — without this, OS retries can hang for minutes and
/// look like the client is "spamming" connections after a failed join.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// Read/write timeout for the login handshake (encryption / compression).
const LOGIN_IO_TIMEOUT: Duration = Duration::from_secs(15);

use super::packet::{ClientboundPacket, ProtocolState};
use super::protocol;

pub struct Connection {
    outbound_tx: Sender<(i32, Vec<u8>)>,
    inbound_rx: Receiver<ClientboundPacket>,
    pub state: ProtocolState,
    pub connected: Arc<AtomicBool>,
    /// Packets deferred by the client-frame world-work budget. This normally
    /// contains world packets, plus ordering barriers and their immediate
    /// successors when a respawn must not overtake queued chunk data.
    pub deferred_packets: VecDeque<ClientboundPacket>,
    /// Number of join/respawn ordering barriers currently in the deferred
    /// stream. Tracking it avoids rescanning a potentially large backlog each
    /// frame merely to decide whether new packets may pass it.
    pub deferred_barrier_count: usize,
}

impl Connection {
    pub fn connect(
        addr: &str,
        username: &str,
        account: Option<&crate::auth::models::Account>,
    ) -> io::Result<Self> {
        let connect_started = Instant::now();
        let (host, port) = parse_addr(addr);
        let endpoint = format!("{host}:{port}");
        log::info!(
            "starting Minecraft connection: endpoint={}, protocol=47, account_mode={}",
            endpoint,
            if account.is_some() {
                "online"
            } else {
                "offline"
            }
        );
        let stream = connect_tcp(host, port)?;
        // NetworkManager enables ChannelOption.TCP_NODELAY for vanilla
        // connections. Movement packets are tiny and sent every 50 ms; Nagle
        // buffering can otherwise deliver several C03 packets as one burst,
        // which GrimAC reports as TimerLimit after latency or a stalled frame.
        stream.set_nodelay(true)?;
        log::info!(
            "TCP connection established in {:.2} ms: local={:?}, peer={:?}, nodelay=true",
            connect_started.elapsed().as_secs_f64() * 1000.0,
            stream.local_addr().ok(),
            stream.peer_addr().ok()
        );
        stream.set_nonblocking(false)?;
        let mut reader = Reader::Plain(stream.try_clone()?);
        let mut writer = Writer::Plain(stream);
        protocol::write_packet(
            &mut writer,
            0x00,
            &super::packet::write_handshake(host, port, 2),
        )?;
        log::debug!("sent handshake packet: endpoint={endpoint}, next_state=login");
        protocol::write_packet(
            &mut writer,
            0x00,
            &super::packet::write_login_start(username),
        )?;
        log::debug!("sent login start packet");

        let mut compression_threshold = None;
        loop {
            let (id, data) =
                protocol::read_packet_with_compression(&mut reader, compression_threshold)?;
            log::trace!(
                "login packet received: id=0x{id:02X}, bytes={}, compression={:?}",
                data.len(),
                compression_threshold
            );
            let packet = ClientboundPacket::parse(ProtocolState::Login, id, &data)?;
            match packet {
                ClientboundPacket::EncryptionRequest {
                    server_id,
                    public_key,
                    verify_token,
                } => {
                    log::info!(
                        "server requested encrypted login: public_key_bytes={}, verify_token_bytes={}",
                        public_key.len(),
                        verify_token.len()
                    );
                    let account = account.ok_or_else(|| {
                        io_other(
                            "This server requires a Microsoft account. Select one in Alt Manager.",
                        )
                    })?;
                    let access_token =
                        account.minecraft_access_token.as_deref().ok_or_else(|| {
                            io_other("Selected account has no Minecraft access token")
                        })?;
                    let uuid = account
                        .uuid
                        .as_deref()
                        .ok_or_else(|| io_other("Selected account has no Minecraft profile"))?;

                    let mut secret = [0u8; 16];
                    rand::rngs::OsRng.fill_bytes(&mut secret);
                    let hash = server_hash(&server_id, &secret, &public_key);
                    crate::auth::minecraft::join_server(access_token, uuid, &hash)
                        .map_err(|error| io_other(error.to_string()))?;
                    log::debug!("Minecraft session join accepted by authentication service");

                    let key = RsaPublicKey::from_public_key_der(&public_key)
                        .map_err(|error| io_other(format!("Invalid server RSA key: {error}")))?;
                    let encrypted_secret = key
                        .encrypt(&mut rand::rngs::OsRng, Pkcs1v15Encrypt, &secret)
                        .map_err(|error| io_other(format!("RSA encryption failed: {error}")))?;
                    let encrypted_token = key
                        .encrypt(&mut rand::rngs::OsRng, Pkcs1v15Encrypt, &verify_token)
                        .map_err(|error| io_other(format!("RSA encryption failed: {error}")))?;
                    let response = super::packet::write_encryption_response(
                        &encrypted_secret,
                        &encrypted_token,
                    );
                    protocol::write_packet(&mut writer, 0x01, &response)?;

                    reader = Reader::Encrypted(EncryptedReader::new(reader.into_stream(), secret));
                    writer = Writer::Encrypted(EncryptedWriter::new(writer.into_stream(), secret));
                    log::info!("AES/CFB8 transport encryption enabled");
                }
                ClientboundPacket::SetCompression { threshold } => {
                    compression_threshold = Some(threshold);
                    log::info!("packet compression enabled: threshold={threshold} bytes");
                }
                ClientboundPacket::LoginSuccess { .. } => {
                    log::info!(
                        "login completed in {:.2} ms",
                        connect_started.elapsed().as_secs_f64() * 1000.0
                    );
                    // Play-phase keepalives can be sparse under lag; clear the
                    // short login I/O timeouts so the reader is not killed.
                    let _ = reader.set_read_timeout(None);
                    let _ = writer.set_write_timeout(None);
                    break;
                }
                ClientboundPacket::Disconnect { reason } => {
                    log::warn!("server disconnected during login: {reason}");
                    return Err(io_other(reason));
                }
                _ => log::debug!("ignored unexpected packet while logging in"),
            }
        }

        let (outbound_tx, outbound_rx) = mpsc::channel::<(i32, Vec<u8>)>();
        let (inbound_tx, inbound_rx) = mpsc::channel::<ClientboundPacket>();
        let connected = Arc::new(AtomicBool::new(true));
        let reader_endpoint = endpoint.clone();
        let reader_connected = connected.clone();
        thread::Builder::new()
            .name("rustcraft-net-reader".to_string())
            .spawn(move || {
            log::debug!("network reader thread started: endpoint={reader_endpoint}");
            let mut state = ProtocolState::Play;
            let mut packet_count = 0u64;
            let mut byte_count = 0u64;
            loop {
                match protocol::read_packet_with_compression(&mut reader, compression_threshold) {
                    Ok((id, data)) => {
                        packet_count = packet_count.saturating_add(1);
                        byte_count = byte_count.saturating_add(data.len() as u64);
                        log::trace!(
                            "inbound packet: id=0x{id:02X}, bytes={}, state={state:?}",
                            data.len()
                        );
                        match ClientboundPacket::parse(state, id, &data) {
                        Ok(packet) => {
                            if inbound_tx.send(packet).is_err() {
                                log::debug!("network reader stopping because the client receiver closed");
                                break;
                            }
                        }
                        Err(error) => log::warn!(
                            "failed to parse inbound packet: id=0x{id:02X}, bytes={}, state={state:?}, error={error}",
                            data.len()
                        ),
                    }
                    }
                    Err(error) => {
                        log::warn!("network reader stopped: endpoint={reader_endpoint}, error={error}");
                        break;
                    }
                }
            }
            reader_connected.store(false, Ordering::SeqCst);
            let _ = reader.shutdown();
            log::info!(
                "network reader thread exited: endpoint={}, packets={}, payload_bytes={}",
                reader_endpoint,
                packet_count,
                byte_count
            );
        })?;
        let writer_endpoint = endpoint.clone();
        let writer_connected = connected.clone();
        thread::Builder::new()
            .name("rustcraft-net-writer".to_string())
            .spawn(move || {
            log::debug!("network writer thread started: endpoint={writer_endpoint}");
            let mut packet_count = 0u64;
            let mut byte_count = 0u64;
            while let Ok((packet_id, payload)) = outbound_rx.recv() {
                log::trace!(
                    "outbound packet: id=0x{packet_id:02X}, bytes={}",
                    payload.len()
                );
                if let Err(error) = protocol::write_packet_with_compression(
                    &mut writer,
                    packet_id,
                    &payload,
                    compression_threshold,
                ) {
                    log::error!(
                        "network writer stopped: endpoint={writer_endpoint}, packet_id=0x{packet_id:02X}, error={error}"
                    );
                    break;
                }
                packet_count = packet_count.saturating_add(1);
                byte_count = byte_count.saturating_add(payload.len() as u64);
            }
            writer_connected.store(false, Ordering::SeqCst);
            let _ = writer.shutdown();
            log::info!(
                "network writer thread exited: endpoint={}, packets={}, payload_bytes={}",
                writer_endpoint,
                packet_count,
                byte_count
            );
        })?;

        Ok(Self {
            outbound_tx,
            inbound_rx,
            state: ProtocolState::Play,
            connected,
            deferred_packets: VecDeque::new(),
            deferred_barrier_count: 0,
        })
    }

    pub fn poll(&mut self, max_packets: usize) -> Vec<ClientboundPacket> {
        let mut packets = Vec::with_capacity(max_packets);
        while packets.len() < max_packets {
            let Ok(packet) = self.inbound_rx.try_recv() else {
                break;
            };
            packets.push(packet);
        }
        packets
    }

    pub fn send_play_packet(&self, packet_id: i32, payload: &[u8]) {
        if self
            .outbound_tx
            .send((packet_id, payload.to_vec()))
            .is_err()
        {
            log::warn!(
                "dropping outbound packet because the writer thread is closed: id=0x{packet_id:02X}, bytes={}",
                payload.len()
            );
        }
    }

    /// Mark the connection dead so the reader/writer threads exit promptly.
    /// Dropping `Connection` also does this via `Drop`.
    pub fn close(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Signal threads first, then drop the outbound channel so the writer
        // unblocks and shuts down the TCP socket (which unblocks the reader).
        self.connected.store(false, Ordering::SeqCst);
        // Replace outbound with a dummy channel so Drop of the old Sender
        // closes the writer thread without requiring Option fields.
        let (dead_tx, _dead_rx) = mpsc::channel();
        let old = std::mem::replace(&mut self.outbound_tx, dead_tx);
        drop(old);
        log::debug!("connection dropped; reader/writer threads signalled to exit");
    }
}

struct Cfb8 {
    cipher: Aes128,
    shift: [u8; 16],
}
impl Cfb8 {
    fn new(secret: [u8; 16]) -> Self {
        Self {
            cipher: Aes128::new_from_slice(&secret).unwrap(),
            shift: secret,
        }
    }
    fn next(&self) -> u8 {
        let mut block = GenericArray::clone_from_slice(&self.shift);
        self.cipher.encrypt_block(&mut block);
        block[0]
    }
    fn encrypt(&mut self, data: &mut [u8]) {
        for byte in data {
            *byte ^= self.next();
            self.shift.rotate_left(1);
            self.shift[15] = *byte;
        }
    }
    fn decrypt(&mut self, data: &mut [u8]) {
        for byte in data {
            let cipher_byte = *byte;
            *byte ^= self.next();
            self.shift.rotate_left(1);
            self.shift[15] = cipher_byte;
        }
    }
}

struct EncryptedReader {
    stream: TcpStream,
    cipher: Cfb8,
}
impl EncryptedReader {
    fn new(stream: TcpStream, secret: [u8; 16]) -> Self {
        Self {
            stream,
            cipher: Cfb8::new(secret),
        }
    }
}
impl Read for EncryptedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.stream.read(buf)?;
        self.cipher.decrypt(&mut buf[..n]);
        Ok(n)
    }
}
struct EncryptedWriter {
    stream: TcpStream,
    cipher: Cfb8,
}
impl EncryptedWriter {
    fn new(stream: TcpStream, secret: [u8; 16]) -> Self {
        Self {
            stream,
            cipher: Cfb8::new(secret),
        }
    }
}
impl Write for EncryptedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut encrypted = buf.to_vec();
        self.cipher.encrypt(&mut encrypted);
        self.stream.write_all(&encrypted)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

enum Reader {
    Plain(TcpStream),
    Encrypted(EncryptedReader),
}
impl Reader {
    fn into_stream(self) -> TcpStream {
        match self {
            Self::Plain(s) => s,
            Self::Encrypted(s) => s.stream,
        }
    }
    fn shutdown(&self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.shutdown(std::net::Shutdown::Both),
            Self::Encrypted(s) => s.stream.shutdown(std::net::Shutdown::Both),
        }
    }
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(timeout),
            Self::Encrypted(s) => s.stream.set_read_timeout(timeout),
        }
    }
}
impl Read for Reader {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Encrypted(s) => s.read(b),
        }
    }
}
enum Writer {
    Plain(TcpStream),
    Encrypted(EncryptedWriter),
}
impl Writer {
    fn into_stream(self) -> TcpStream {
        match self {
            Self::Plain(s) => s,
            Self::Encrypted(s) => s.stream,
        }
    }
    fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.set_write_timeout(timeout),
            Self::Encrypted(s) => s.stream.set_write_timeout(timeout),
        }
    }
    fn shutdown(&self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.shutdown(std::net::Shutdown::Both),
            Self::Encrypted(s) => s.stream.shutdown(std::net::Shutdown::Both),
        }
    }
}
impl Write for Writer {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(b),
            Self::Encrypted(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Encrypted(s) => s.flush(),
        }
    }
}

fn server_hash(server_id: &str, secret: &[u8], public_key: &[u8]) -> String {
    let mut digest = Sha1::new();
    digest.update(server_id.as_bytes());
    digest.update(secret);
    digest.update(public_key);
    signed_hex(&digest.finalize())
}

fn signed_hex(bytes: &[u8]) -> String {
    let negative = bytes.first().is_some_and(|byte| byte & 0x80 != 0);
    let mut magnitude = bytes.to_vec();
    if negative {
        for byte in &mut magnitude {
            *byte = !*byte;
        }
        for byte in magnitude.iter_mut().rev() {
            let (value, carry) = byte.overflowing_add(1);
            *byte = value;
            if !carry {
                break;
            }
        }
    }
    let hex = magnitude
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let hex = hex.trim_start_matches('0');
    format!(
        "{}{}",
        if negative { "-" } else { "" },
        if hex.is_empty() { "0" } else { hex }
    )
}

fn io_other(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::Other, message.into())
}
fn parse_addr(addr: &str) -> (&str, u16) {
    if let Some(index) = addr.rfind(':') {
        (&addr[..index], addr[index + 1..].parse().unwrap_or(25565))
    } else {
        (addr, 25565)
    }
}

/// Resolve `host:port` and open a TCP socket with a hard connect timeout.
fn connect_tcp(host: &str, port: u16) -> io::Result<TcpStream> {
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| io::Error::new(e.kind(), format!("DNS resolve failed for {host}:{port}: {e}")))?
        .collect();
    if addrs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no addresses for {host}:{port}"),
        ));
    }
    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(LOGIN_IO_TIMEOUT));
                let _ = stream.set_write_timeout(Some(LOGIN_IO_TIMEOUT));
                return Ok(stream);
            }
            Err(e) => {
                log::debug!("TCP connect_timeout failed: addr={addr}, error={e}");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("could not connect to {host}:{port}"),
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::{signed_hex, Cfb8};
    #[test]
    fn formats_minecraft_signed_sha1() {
        assert_eq!(signed_hex(&[0x00, 0x01]), "1");
        assert_eq!(signed_hex(&[0xff]), "-1");
        assert_eq!(signed_hex(&[0x80, 0x00]), "-8000");
    }

    #[test]
    fn cfb8_keeps_state_across_packet_boundaries() {
        let secret = *b"0123456789abcdef";
        let mut encryptor = Cfb8::new(secret);
        let mut first = b"first packet".to_vec();
        let mut second = b"second packet".to_vec();
        encryptor.encrypt(&mut first);
        encryptor.encrypt(&mut second);
        let mut decryptor = Cfb8::new(secret);
        decryptor.decrypt(&mut first);
        decryptor.decrypt(&mut second);
        assert_eq!(first, b"first packet");
        assert_eq!(second, b"second packet");
    }
}
