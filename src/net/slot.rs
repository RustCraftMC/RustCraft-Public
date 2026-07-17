//! Minecraft 1.8.9 slot codec.
//!
//! Slots appear in inventory/window packets and item entity metadata. The NBT
//! payload is kept as raw compressed bytes for now so protocol sync works before
//! a full NBT parser lands.

use super::protocol::PacketBuffer;
use std::io;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub item_id: i16,
    pub count: u8,
    pub damage: i16,
    pub nbt: Option<Vec<u8>>,
}

impl Slot {
    pub const EMPTY: Slot = Slot {
        item_id: -1,
        count: 0,
        damage: 0,
        nbt: None,
    };

    pub fn is_empty(&self) -> bool {
        self.item_id < 0 || self.count == 0
    }

    pub fn item_id_u16(&self) -> u16 {
        self.item_id.max(0) as u16
    }
}

pub fn read_slot(buf: &mut PacketBuffer) -> io::Result<Slot> {
    let item_id = buf.read_short()?;
    if item_id < 0 {
        return Ok(Slot::EMPTY);
    }

    let count = buf.read_unsigned_byte()?;
    let damage = buf.read_short()?;
    let nbt = read_nbt_payload(buf)?;
    Ok(Slot {
        item_id,
        count,
        damage,
        nbt,
    })
}

pub fn write_slot(buf: &mut PacketBuffer, slot: &Slot) {
    if slot.is_empty() {
        buf.write_short(-1);
        return;
    }

    buf.write_short(slot.item_id);
    buf.write_unsigned_byte(slot.count);
    buf.write_short(slot.damage);
    if let Some(nbt) = &slot.nbt {
        buf.write_bytes(nbt);
    } else {
        buf.write_byte(0);
    }
}

fn read_nbt_payload(buf: &mut PacketBuffer) -> io::Result<Option<Vec<u8>>> {
    if buf.remaining() == 0 {
        return Ok(None);
    }

    let start = buf.position();
    let tag_id = buf.read_unsigned_byte()?;
    if tag_id == 0 {
        return Ok(None);
    }

    let name_len = buf.read_unsigned_short()? as usize;
    buf.skip(name_len)?;
    skip_nbt_payload(buf, tag_id)?;
    let end = buf.position();
    Ok(Some(buf.slice(start, end)?.to_vec()))
}

fn skip_nbt_payload(buf: &mut PacketBuffer, tag_id: u8) -> io::Result<()> {
    match tag_id {
        0 => Ok(()),
        1 => buf.skip(1),
        2 => buf.skip(2),
        3 => buf.skip(4),
        4 => buf.skip(8),
        5 => buf.skip(4),
        6 => buf.skip(8),
        7 => {
            let len = buf.read_int()?.max(0) as usize;
            buf.skip(len)
        }
        8 => {
            let len = buf.read_unsigned_short()? as usize;
            buf.skip(len)
        }
        9 => {
            let child_id = buf.read_unsigned_byte()?;
            let len = buf.read_int()?.max(0) as usize;
            for _ in 0..len {
                skip_nbt_payload(buf, child_id)?;
            }
            Ok(())
        }
        10 => loop {
            let child_id = buf.read_unsigned_byte()?;
            if child_id == 0 {
                break Ok(());
            }
            let name_len = buf.read_unsigned_short()? as usize;
            buf.skip(name_len)?;
            skip_nbt_payload(buf, child_id)?;
        },
        11 => {
            let len = buf.read_int()?.max(0) as usize;
            buf.skip(len * 4)
        }
        12 => {
            let len = buf.read_int()?.max(0) as usize;
            buf.skip(len * 8)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown nbt tag id {}", tag_id),
        )),
    }
}
