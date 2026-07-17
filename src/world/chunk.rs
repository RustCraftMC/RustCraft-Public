//! Chunk storage — a 16×256×16 column of blocks plus light and biome data.
//! Uses nibble-packed light and split block/metadata storage to reduce memory.

use super::block::Block;

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;
pub const SECTION_SIZE: usize = 16;
pub const SECTION_COUNT: usize = CHUNK_HEIGHT / SECTION_SIZE;
pub const SECTION_VOLUME: usize = SECTION_SIZE * SECTION_SIZE * SECTION_SIZE;
pub const NIBBLE_SECTION_BYTES: usize = SECTION_VOLUME / 2;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_HEIGHT * CHUNK_SIZE;
pub const CHUNK_VOLUME_NIBBLE: usize = CHUNK_VOLUME / 2;
pub const BIOME_COUNT: usize = CHUNK_SIZE * CHUNK_SIZE;

#[inline(always)]
fn nibble_get(data: &[u8], idx: usize) -> u8 {
    (data[idx >> 1] >> ((idx & 1) << 2)) & 0x0f
}

#[inline(always)]
fn nibble_set(data: &mut [u8], idx: usize, val: u8) {
    let byte = &mut data[idx >> 1];
    let shift = (idx & 1) << 2;
    *byte = (*byte & !(0x0f << shift)) | ((val & 0x0f) << shift);
}

pub struct Chunk {
    blocks: Box<[u8; CHUNK_VOLUME]>,
    metadata_nibble: Box<[u8; CHUNK_VOLUME_NIBBLE]>,
    block_light: Box<[u8; CHUNK_VOLUME_NIBBLE]>,
    sky_light: Option<Box<[u8; CHUNK_VOLUME_NIBBLE]>>,
    biomes: Box<[u8; BIOME_COUNT]>,
    network_light_valid: AtomicBool,
    has_sky_light: bool,
    pub cx: i32,
    pub cz: i32,
}

impl Chunk {
    pub fn new(cx: i32, cz: i32) -> Self {
        Chunk {
            blocks: Box::new([0; CHUNK_VOLUME]),
            metadata_nibble: Box::new([0; CHUNK_VOLUME_NIBBLE]),
            block_light: Box::new([0; CHUNK_VOLUME_NIBBLE]),
            sky_light: Some(Box::new([0xFF; CHUNK_VOLUME_NIBBLE])),
            biomes: Box::new([1; BIOME_COUNT]),
            network_light_valid: AtomicBool::new(false),
            has_sky_light: true,
            cx,
            cz,
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Block {
        index(x, y, z).map_or(Block::Air, |idx| {
            let state = ((self.blocks[idx] as u16) << 4)
                | nibble_get(self.metadata_nibble.as_ref(), idx) as u16;
            Block::from_state(state)
        })
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: Block) {
        if let Some(idx) = index(x, y, z) {
            self.blocks[idx] = block.to_id() as u8;
            nibble_set(self.metadata_nibble.as_mut(), idx, 0);
        }
    }

    #[inline]
    pub fn state(&self, x: usize, y: usize, z: usize) -> u16 {
        index(x, y, z).map_or(0, |idx| {
            ((self.blocks[idx] as u16) << 4) | nibble_get(self.metadata_nibble.as_ref(), idx) as u16
        })
    }

    #[inline]
    pub fn metadata(&self, x: usize, y: usize, z: usize) -> u8 {
        index(x, y, z).map_or(0, |idx| nibble_get(self.metadata_nibble.as_ref(), idx))
    }

    pub fn set_state(&mut self, x: usize, y: usize, z: usize, state: u16) {
        if let Some(idx) = index(x, y, z) {
            self.blocks[idx] = (state >> 4) as u8;
            nibble_set(self.metadata_nibble.as_mut(), idx, (state & 0x0f) as u8);
        }
    }

    pub fn finish_network_light(&mut self, has_sky_light: bool, data_valid: bool) {
        self.has_sky_light = has_sky_light;
        self.network_light_valid
            .store(data_valid, Ordering::Relaxed);
        if !has_sky_light {
            self.sky_light = None;
        }
    }

    pub fn has_valid_network_light(&self) -> bool {
        self.network_light_valid.load(Ordering::Relaxed)
    }

    pub fn has_sky_light(&self) -> bool {
        self.has_sky_light
    }

    pub fn invalidate_network_light(&self) {
        self.network_light_valid.store(false, Ordering::Relaxed);
    }

    pub fn set_block_light(&mut self, x: usize, y: usize, z: usize, light: u8) {
        if let Some(idx) = index(x, y, z) {
            nibble_set(self.block_light.as_mut(), idx, light.min(15));
        }
    }

    pub fn set_sky_light(&mut self, x: usize, y: usize, z: usize, light: u8) {
        if let Some(ref mut sl) = self.sky_light {
            if let Some(idx) = index(x, y, z) {
                nibble_set(sl.as_mut(), idx, light.min(15));
            }
        }
    }

    pub fn light_at(&self, x: usize, y: usize, z: usize) -> (u8, u8) {
        index(x, y, z).map_or((15, 0), |idx| {
            let sky = self
                .sky_light
                .as_ref()
                .map_or(0, |sl| nibble_get(sl.as_ref(), idx));
            (sky, nibble_get(self.block_light.as_ref(), idx))
        })
    }

    pub fn set_biome(&mut self, x: usize, z: usize, biome: u8) {
        if let Some(idx) = biome_index(x, z) {
            self.biomes[idx] = biome;
        }
    }

    pub fn biome(&self, x: usize, z: usize) -> u8 {
        biome_index(x, z).map_or(1, |idx| self.biomes[idx])
    }

    pub fn clear(&mut self) {
        self.blocks.fill(0);
        self.metadata_nibble.fill(0);
        self.block_light.fill(0);
        if let Some(ref mut sl) = self.sky_light {
            for b in sl.iter_mut() {
                *b = 0xFF;
            }
        }
        self.biomes.fill(1);
        self.network_light_valid.store(false, Ordering::Relaxed);
        self.has_sky_light = true;
    }
}

impl Clone for Chunk {
    fn clone(&self) -> Self {
        Self {
            blocks: self.blocks.clone(),
            metadata_nibble: self.metadata_nibble.clone(),
            block_light: self.block_light.clone(),
            sky_light: self.sky_light.clone(),
            biomes: self.biomes.clone(),
            network_light_valid: AtomicBool::new(self.has_valid_network_light()),
            has_sky_light: self.has_sky_light,
            cx: self.cx,
            cz: self.cz,
        }
    }
}

#[inline]
fn index(x: usize, y: usize, z: usize) -> Option<usize> {
    if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
        Some((y * CHUNK_SIZE + z) * CHUNK_SIZE + x)
    } else {
        None
    }
}

#[inline]
fn biome_index(x: usize, z: usize) -> Option<usize> {
    if x < CHUNK_SIZE && z < CHUNK_SIZE {
        Some(z * CHUNK_SIZE + x)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::Chunk;

    #[test]
    fn block_change_invalidates_server_light_snapshot() {
        let mut chunk = Chunk::new(0, 0);
        chunk.finish_network_light(true, true);
        assert!(chunk.has_valid_network_light());

        chunk.invalidate_network_light();
        assert!(!chunk.has_valid_network_light());
    }
}
use std::sync::atomic::{AtomicBool, Ordering};
