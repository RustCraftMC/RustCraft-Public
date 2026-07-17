//! Player inventory — hotbar + main inventory.
//!
//! MC 1.8.9 inventory layout:
//!   Hotbar: 9 slots (bottom of screen)
//!   Main:   27 slots (3 rows of 9)
//!   Total:  36 slots
//!
//! Each slot holds an ItemStack (item_id + count + damage).

use crate::world::block::Block;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemStackMeta {
    pub nbt: Option<Vec<u8>>,
}

/// Slot counts for GUIs whose 1.8.9 Open Window packet deliberately advertises
/// zero slots because the vanilla client constructs their containers locally.
pub fn effective_container_slot_count(window_type: &str, advertised: usize) -> usize {
    if advertised != 0 {
        return advertised;
    }

    match window_type
        .strip_prefix("minecraft:")
        .unwrap_or(window_type)
    {
        "crafting_table" | "workbench" => 10,
        "enchanting_table" | "enchanting" => 2,
        "anvil" => 3,
        _ => 0,
    }
}

#[cfg(test)]
mod special_container_tests {
    use super::effective_container_slot_count;

    #[test]
    fn local_special_containers_restore_their_vanilla_slot_counts() {
        assert_eq!(
            effective_container_slot_count("minecraft:crafting_table", 0),
            10
        );
        assert_eq!(
            effective_container_slot_count("minecraft:enchanting_table", 0),
            2
        );
        assert_eq!(effective_container_slot_count("minecraft:anvil", 0), 3);
        assert_eq!(effective_container_slot_count("minecraft:hopper", 5), 5);
        assert_eq!(effective_container_slot_count("custom:menu", 0), 0);
    }
}

/// Read an enchantment level from an item's protocol NBT.
///
/// 1.8.9 stores enchantments as `tag.ench`, a list of `{id: short, lvl:
/// short}` compounds (`EnchantmentHelper.getEnchantmentLevel`). Returns 0
/// when the item has no NBT or the enchantment is absent.
pub fn nbt_enchantment_level(nbt: Option<&[u8]>, enchantment_id: i16) -> i16 {
    let Some(bytes) = nbt else {
        return 0;
    };
    let Ok(root) = crate::net::nbt::parse_root(bytes) else {
        return 0;
    };
    let Some(compound) = root.as_compound() else {
        return 0;
    };
    let Some(list) = compound.get("ench").and_then(|tag| tag.as_list()) else {
        return 0;
    };
    for entry in list {
        let Some(entry) = entry.as_compound() else {
            continue;
        };
        if entry.get("id").and_then(|tag| tag.as_i16()) == Some(enchantment_id) {
            return entry
                .get("lvl")
                .and_then(|tag| tag.as_i16())
                .unwrap_or(0)
                .max(0);
        }
    }
    0
}

pub fn has_glint(view: &ItemStackView) -> bool {
    let Some(bytes) = view.nbt.as_deref() else {
        return false;
    };
    let Ok(root) = crate::net::nbt::parse_root(bytes) else {
        return false;
    };
    let Some(compound) = root.as_compound() else {
        return false;
    };
    if compound
        .get("ench")
        .and_then(|tag| tag.as_list())
        .is_some_and(|list| !list.is_empty())
    {
        return true;
    }
    if compound
        .get("StoredEnchantments")
        .and_then(|tag| tag.as_list())
        .is_some_and(|list| !list.is_empty())
    {
        return true;
    }
    false
}

/// Returns a purple-tinted color for enchanted item glint effect.
/// `elapsed` is wall-clock seconds; the color oscillates over time.
pub fn glint_tint(elapsed: f32) -> [f32; 4] {
    let wave = (elapsed * 5.0).sin() * 0.5 + 0.5;
    let r = 0.6 + wave * 0.4;
    let g = 0.2 + wave * 0.3;
    let b = 0.7 + wave * 0.3;
    [r, g, b, 0.95]
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemStackView {
    pub item_id: u16,
    pub count: u8,
    pub damage: u16,
    pub nbt: Option<Vec<u8>>,
}

impl ItemStackView {
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.item_id == 0
    }

    pub fn to_protocol_slot(&self) -> crate::net::slot::Slot {
        if self.is_empty() {
            crate::net::slot::Slot::EMPTY
        } else {
            crate::net::slot::Slot {
                item_id: self.item_id as i16,
                count: self.count,
                damage: self.damage as i16,
                nbt: self.nbt.clone(),
            }
        }
    }
}

/// Vanilla 1.8.9 `Item.getMaxDamage()` for every damageable item.
/// Returns 0 for items without durability (`isItemStackDamageable` false).
pub fn max_damage(item_id: u16) -> u16 {
    match item_id {
        // Tools and swords by ToolMaterial: WOOD 59, STONE 131, IRON 250,
        // EMERALD (diamond) 1561, GOLD 32. Hoes (290-294) share materials.
        268..=271 | 290 => 59,
        272..=275 | 291 => 131,
        256..=258 | 267 | 292 => 250,
        276..=279 | 293 => 1561,
        283..=286 | 294 => 32,
        // ItemArmor: durability factor × base [helmet 11, chest 16, legs 15,
        // boots 13]; LEATHER ×5, CHAIN ×15, IRON ×15, GOLD ×7, DIAMOND ×33.
        298 => 55,
        299 => 80,
        300 => 75,
        301 => 65,
        302 | 306 => 165,
        303 | 307 => 240,
        304 | 308 => 225,
        305 | 309 => 195,
        310 => 363,
        311 => 528,
        312 => 495,
        313 => 429,
        314 => 77,
        315 => 112,
        316 => 105,
        317 => 91,
        259 => 64,  // flint and steel
        261 => 384, // bow
        346 => 64,  // fishing rod
        359 => 238, // shears
        398 => 25,  // carrot on a stick
        _ => 0,
    }
}

/// An item stack in inventory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemStack {
    /// Block/Item ID (uses Block enum for blocks, item IDs for items)
    pub item_id: u16,
    /// Stack count (1-64)
    pub count: u8,
    /// Damage value (for tools) or 0
    pub damage: u16,
}

impl ItemStack {
    pub const EMPTY: ItemStack = ItemStack {
        item_id: 0,
        count: 0,
        damage: 0,
    };

    pub fn new(item_id: u16, count: u8) -> Self {
        if item_id == 0 || count == 0 {
            return Self::EMPTY;
        }

        let mut stack = ItemStack {
            item_id,
            count,
            damage: 0,
        };
        stack.count = stack.count.min(stack.max_stack());
        stack
    }

    pub fn from_block(block: Block, count: u8) -> Self {
        Self::new(block.to_id(), count)
    }

    pub fn from_protocol_slot(slot: &crate::net::slot::Slot) -> Self {
        if slot.is_empty() {
            return Self::EMPTY;
        }
        ItemStack {
            item_id: slot.item_id_u16(),
            count: slot.count,
            damage: slot.damage.max(0) as u16,
        }
    }

    pub fn to_protocol_slot(&self) -> crate::net::slot::Slot {
        self.to_protocol_slot_with_meta(None)
    }

    pub fn to_protocol_slot_with_meta(
        &self,
        meta: Option<&ItemStackMeta>,
    ) -> crate::net::slot::Slot {
        if self.is_empty() {
            crate::net::slot::Slot::EMPTY
        } else {
            crate::net::slot::Slot {
                item_id: self.item_id as i16,
                count: self.count,
                damage: self.damage as i16,
                nbt: meta.and_then(|meta| meta.nbt.clone()),
            }
        }
    }

    pub fn view_with_meta(&self, meta: Option<&ItemStackMeta>) -> ItemStackView {
        if self.is_empty() {
            ItemStackView::default()
        } else {
            ItemStackView {
                item_id: self.item_id,
                count: self.count,
                damage: self.damage,
                nbt: meta.and_then(|meta| meta.nbt.clone()),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.item_id == 0
    }

    pub fn max_stack(&self) -> u8 {
        // Most blocks stack to 64, tools to 1, etc.
        // Simplified: everything stacks to 64
        64
    }

    pub fn can_stack_with(&self, other: &ItemStack) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.item_id == other.item_id
            && self.damage == other.damage
    }

    fn normalized(mut self) -> Self {
        if self.is_empty() {
            Self::EMPTY
        } else {
            self.count = self.count.min(self.max_stack());
            self
        }
    }
}

/// Player inventory with hotbar.
pub struct Inventory {
    /// All 36 slots. slots[0..9] = hotbar, slots[9..36] = main inventory.
    pub slots: [ItemStack; 36],
    pub slot_meta: [ItemStackMeta; 36],
    /// Armor slots from the player inventory window.
    pub armor: [ItemStack; 4],
    pub armor_meta: [ItemStackMeta; 4],
    /// Crafting result + 2x2 crafting input from the player inventory window.
    pub crafting: [ItemStack; 5],
    pub crafting_meta: [ItemStackMeta; 5],
    /// Item currently carried by the cursor.
    pub cursor: ItemStack,
    pub cursor_meta: ItemStackMeta,
    /// Last opened server window. Window 0 is the player inventory.
    pub open_window_id: u8,
    /// Set to true when a server container (window_id != 0) was open.
    /// Used to detect server-initiated window closures.
    pub had_server_window: bool,
    /// Chest targeted by the most recent block-use packet. It becomes the
    /// active chest only if the server answers with a chest window.
    pub pending_chest_position: Option<(i32, i32, i32)>,
    pub open_chest_position: Option<(i32, i32, i32)>,
    pub open_window_type: String,
    pub open_window_title: String,
    pub open_window_slot_count: usize,
    pub open_window_slots: Vec<ItemStack>,
    pub open_window_slot_meta: Vec<ItemStackMeta>,
    pub open_window_properties: Vec<(i16, i16)>,
    /// Set to true when server opens a container; read+clear by handle_redraw.
    pub window_just_opened: bool,
    /// Currently selected hotbar slot (0-8).
    pub selected: usize,
}

impl Inventory {
    pub fn new() -> Self {
        Inventory {
            slots: [ItemStack::EMPTY; 36],
            slot_meta: std::array::from_fn(|_| ItemStackMeta::default()),
            armor: [ItemStack::EMPTY; 4],
            armor_meta: std::array::from_fn(|_| ItemStackMeta::default()),
            crafting: [ItemStack::EMPTY; 5],
            crafting_meta: std::array::from_fn(|_| ItemStackMeta::default()),
            cursor: ItemStack::EMPTY,
            cursor_meta: ItemStackMeta::default(),
            open_window_id: 0,
            had_server_window: false,
            pending_chest_position: None,
            open_chest_position: None,
            open_window_type: "minecraft:container".to_string(),
            open_window_title: "Inventory".to_string(),
            open_window_slot_count: 0,
            open_window_slots: Vec::new(),
            open_window_slot_meta: Vec::new(),
            open_window_properties: Vec::new(),
            window_just_opened: false,
            selected: 0,
        }
    }

    /// Create a creative-mode inventory with all blocks and items pre-filled.
    pub fn creative() -> Self {
        let mut inv = Self::new();
        // Fill hotbar with common tools
        inv.slots[0] = ItemStack::new(276, 1); // Diamond Sword
        inv.slots[1] = ItemStack::new(278, 1); // Diamond Pickaxe
        inv.slots[2] = ItemStack::new(277, 1); // Diamond Shovel
        inv.slots[3] = ItemStack::new(261, 1); // Bow
        inv.slots[4] = ItemStack::new(297, 64); // Bread
        inv.slots[5] = ItemStack::new(331, 64); // Redstone
        inv.slots[6] = ItemStack::new(325, 1); // Bucket
        inv.slots[7] = ItemStack::new(326, 1); // Water Bucket
        inv.slots[8] = ItemStack::new(259, 1); // Flint and Steel

        // Fill main inventory with blocks
        let all_blocks = Self::all_creative_blocks();
        for (i, block) in all_blocks.iter().enumerate() {
            if i >= 27 {
                break;
            }
            inv.slots[9 + i] = ItemStack::from_block(*block, 64);
        }
        inv
    }

    /// Get all items available in creative mode (tools, food, materials).
    pub fn all_creative_items() -> Vec<u16> {
        vec![
            // Tools
            268, 269, 270, 271, // Wood tools
            272, 273, 274, 275, // Stone tools
            256, 257, 258, 259, // Iron tools
            276, 277, 278, 279, // Diamond tools
            283, 284, 285, 286, // Gold tools
            261, // Bow
            346, // Fishing Rod
            287, // Shears
            // Armor
            298, 299, 300, 301, // Leather
            302, 303, 304, 305, // Chainmail
            306, 307, 308, 309, // Iron
            310, 311, 312, 313, // Diamond
            314, 315, 316, 317, // Gold
            // Food
            260, 322, 297, 357, 360, 400,
            354, // Apple, Golden Apple, Bread, Cookie, Melon, Pumpkin Pie, Cake
            319, 320, 363, 364, // Porkchop, Beef
            365, 366, 428, 429, // Chicken, Mutton
            411, 412, 349, 350, // Rabbit, Fish
            396, 391, 392, 393, 394, // Golden Carrot, Carrot, Potato
            // Materials
            280, 287, 288, 289, 334, 339, 340,
            341, // Stick, String, Feather, Gunpowder, Leather, Paper, Book, Slimeball
            369, 377, 370, 378, // Blaze Rod/Powder, Ghast Tear, Magma Cream
            264, 265, 266, 263, 352, 351,
            331, // Diamond, Iron, Gold, Coal, Bone, Bone Meal, Redstone
            388, 406, 398, // Emerald, Quartz, Nether Star
            // Misc
            332, 344, 368, 381, // Snowball, Egg, Ender Pearl, Eye of Ender
            401, 358, 345, 347, // Firework, Map, Compass, Clock
            421, 420, // Name Tag, Lead
            373, 438, // Potion, Splash Potion
            384, // Experience Bottle
            383, // Spawn Egg
        ]
    }

    /// Get all blocks available in creative mode.
    pub fn all_creative_blocks() -> Vec<Block> {
        vec![
            // Building blocks
            Block::Stone,
            Block::Cobblestone,
            Block::Planks,
            Block::Bricks,
            Block::StoneBricks,
            Block::Sandstone,
            Block::NetherBrick,
            Block::MossyCobblestone,
            Block::Obsidian,
            Block::Glass,
            // Natural
            Block::Grass,
            Block::Dirt,
            Block::Sand,
            Block::Gravel,
            Block::Log,
            Block::Leaves,
            Block::Clay,
            Block::SnowBlock,
            Block::Ice,
            Block::Mycelium,
            Block::Netherrack,
            Block::EndStone,
            // Ores & minerals
            Block::CoalOre,
            Block::IronOre,
            Block::GoldOre,
            Block::DiamondOre,
            Block::EmeraldOre,
            Block::RedstoneOre,
            Block::LapisOre,
            // Metal blocks
            Block::IronBlock,
            Block::GoldBlock,
            Block::DiamondBlock,
            Block::EmeraldBlock,
            Block::LapisBlock,
            // Decoration
            Block::Wool,
            Block::Bookshelf,
            Block::Tnt,
            Block::Torch,
            Block::Sponge,
            Block::Glowstone,
            Block::Pumpkin,
            Block::MelonBlock,
            Block::Cactus,
            Block::SugarCane,
            Block::Vine,
            // Fences & walls
            Block::OakFence,
            Block::CobblestoneWall,
            Block::IronBars,
            Block::GlassPane,
            // Doors & gates
            Block::OakDoor,
            Block::IronDoor,
            Block::OakFenceGate,
            Block::Trapdoor,
            // Stairs & slabs
            Block::OakStairs,
            Block::CobblestoneStairs,
            Block::BrickStairs,
            Block::StoneBrickStairs,
            Block::NetherBrickStairs,
            Block::StoneSlab,
            Block::DoubleStoneSlab,
            // Redstone
            Block::RedstoneWire,
            Block::UnlitRedstoneTorch,
            Block::RedstoneTorch,
            Block::UnpoweredRepeater,
            Block::PoweredRepeater,
            Block::StonePressurePlate,
            Block::WoodenPressurePlate,
            Block::StoneButton,
            Block::Lever,
            // Nature
            Block::Dandelion,
            Block::Flower,
            Block::BrownMushroom,
            Block::RedMushroom,
            Block::DeadBush,
            Block::TallGrass,
            Block::LilyPad,
            Block::Cobweb,
            Block::Bed,
            Block::Cake,
            // Functional
            Block::Chest,
            Block::Furnace,
            Block::LitFurnace,
            Block::CraftingTable,
            Block::EnchantingTable,
            Block::BrewingStand,
            Block::Cauldron,
            Block::Jukebox,
            Block::NoteBlock,
            Block::Dispenser,
            Block::Piston,
            Block::StickyPiston,
            Block::MobSpawner,
            // Rails
            Block::Rail,
            Block::PoweredRail,
            Block::DetectorRail,
            // Special
            Block::HayBlock,
            Block::HardenedClay,
            Block::CoalBlock,
            Block::PackedIce,
            Block::FlowerPot,
            Block::Anvil,
            Block::TrappedChest,
            Block::DaylightDetector,
            Block::Hopper,
            Block::QuartzBlock,
            Block::QuartzStairs,
            Block::Dropper,
            Block::StainedClay,
            // Plants & crops
            Block::Sapling,
            Block::Wheat,
            Block::Farmland,
            Block::NetherWart,
            Block::MelonStem,
            Block::PumpkinStem,
            Block::StandingSign,
            Block::WallSign,
            // Misc
            Block::GrassSnowy,
            Block::Ladder,
            Block::Fire,
            Block::SnowLayer,
            Block::Bedrock,
        ]
    }

    /// Get the currently selected hotbar item.
    pub fn selected_item(&self) -> &ItemStack {
        &self.slots[self.selected]
    }

    /// Enchantment level on the held item, from its server-provided NBT.
    /// Mirrors `EnchantmentHelper.getEnchantmentLevel` on the current item.
    pub fn selected_enchantment_level(&self, enchantment_id: i16) -> i16 {
        if self.slots[self.selected].is_empty() {
            return 0;
        }
        nbt_enchantment_level(self.slot_meta[self.selected].nbt.as_deref(), enchantment_id)
    }

    /// Highest enchantment level across the four armor slots. Mirrors
    /// `EnchantmentHelper.getMaxEnchantmentLevel` over `player.getInventory()`
    /// armor (used by Aqua Affinity, Respiration, ...).
    pub fn max_armor_enchantment_level(&self, enchantment_id: i16) -> i16 {
        self.armor
            .iter()
            .zip(self.armor_meta.iter())
            .filter(|(stack, _)| !stack.is_empty())
            .map(|(_, meta)| nbt_enchantment_level(meta.nbt.as_deref(), enchantment_id))
            .max()
            .unwrap_or(0)
    }

    /// Get the currently selected hotbar item (mutable).
    pub fn selected_item_mut(&mut self) -> &mut ItemStack {
        &mut self.slots[self.selected]
    }

    /// Set the selected hotbar slot (0-8).
    pub fn set_selected(&mut self, slot: usize) {
        self.selected = slot.min(8);
    }

    /// Scroll hotbar selection. positive = next, negative = prev.
    pub fn scroll(&mut self, delta: i32) {
        let s = self.selected as i32 + delta;
        self.selected = ((s % 9 + 9) % 9) as usize;
    }

    /// Get the block type of the selected hotbar item (if it's a block).
    pub fn selected_block(&self) -> Option<Block> {
        let item = self.selected_item();
        if item.is_empty() {
            return None;
        }
        let block = Block::from_id(item.item_id);
        if block == Block::Air || block.to_id() != item.item_id {
            None
        } else {
            Some(block)
        }
    }

    /// Try to add an item stack to inventory. Returns leftover count.
    pub fn add_item(&mut self, mut stack: ItemStack) -> u8 {
        stack = stack.normalized();
        if stack.is_empty() {
            return 0;
        }

        // First try to stack with existing slots
        for slot in self.slots.iter_mut() {
            if slot.can_stack_with(&stack) && slot.count < slot.max_stack() {
                let space = slot.max_stack() - slot.count;
                let add = space.min(stack.count);
                slot.count += add;
                stack.count -= add;
                if stack.count == 0 {
                    return 0;
                }
            }
        }
        // Then try empty slots
        for slot in self.slots.iter_mut() {
            if slot.is_empty() {
                let add = stack.max_stack().min(stack.count);
                *slot = ItemStack {
                    count: add,
                    ..stack
                };
                stack.count -= add;
                if stack.count == 0 {
                    return 0;
                }
            }
        }
        stack.count // leftover
    }

    /// Remove one item from the selected slot.
    pub fn remove_selected_one(&mut self) {
        let item = &mut self.slots[self.selected];
        if item.count > 0 {
            item.count -= 1;
            if item.count == 0 {
                *item = ItemStack::EMPTY;
            }
        }
    }

    pub fn drop_selected(&mut self, drop_stack: bool) {
        if drop_stack {
            self.slots[self.selected] = ItemStack::EMPTY;
        } else {
            self.remove_selected_one();
        }
    }

    pub fn click_local_slot(&mut self, slot: usize, right_click: bool) {
        if slot >= self.slots.len() {
            return;
        }

        Self::click_stack_pair(
            &mut self.cursor,
            &mut self.cursor_meta,
            &mut self.slots[slot],
            &mut self.slot_meta[slot],
            right_click,
        );
    }

    pub fn click_player_window_slot(&mut self, protocol_slot: i16, right_click: bool) {
        match protocol_slot {
            0..=4 => {
                let idx = protocol_slot as usize;
                Self::click_stack_pair(
                    &mut self.cursor,
                    &mut self.cursor_meta,
                    &mut self.crafting[idx],
                    &mut self.crafting_meta[idx],
                    right_click,
                );
            }
            5..=8 => {
                let idx = (protocol_slot - 5) as usize;
                Self::click_stack_pair(
                    &mut self.cursor,
                    &mut self.cursor_meta,
                    &mut self.armor[idx],
                    &mut self.armor_meta[idx],
                    right_click,
                );
            }
            9..=35 => {
                self.click_local_slot(protocol_slot as usize, right_click);
            }
            36..=44 => {
                self.click_local_slot((protocol_slot - 36) as usize, right_click);
            }
            _ => {}
        }
    }

    /// Click a slot that belongs to an open window (e.g. chest) when running client-side
    /// (no server connection). protocol_slot is the index within the open window's slot list.
    pub fn click_open_window_slot(&mut self, protocol_slot: i16, right_click: bool) {
        if protocol_slot < 0 {
            return;
        }
        let idx = protocol_slot as usize;
        if idx >= self.open_window_slot_count {
            return;
        }
        if right_click {
            self.right_click_open_window_slot(idx);
        } else {
            self.left_click_open_window_slot(idx);
        }
    }

    fn left_click_open_window_slot(&mut self, idx: usize) {
        if idx >= self.open_window_slots.len() {
            return;
        }
        Self::click_stack_pair(
            &mut self.cursor,
            &mut self.cursor_meta,
            &mut self.open_window_slots[idx],
            &mut self.open_window_slot_meta[idx],
            false,
        );
    }

    fn right_click_open_window_slot(&mut self, idx: usize) {
        if idx >= self.open_window_slots.len() {
            return;
        }
        Self::click_stack_pair(
            &mut self.cursor,
            &mut self.cursor_meta,
            &mut self.open_window_slots[idx],
            &mut self.open_window_slot_meta[idx],
            true,
        );
    }

    pub fn shift_click_protocol_slot(&mut self, protocol_slot: i16) {
        if self.open_window_id == 0 {
            self.shift_click_player_window_slot(protocol_slot);
        } else {
            self.shift_click_open_window_slot(protocol_slot);
        }
    }

    pub fn drop_protocol_slot(&mut self, protocol_slot: i16, drop_stack: bool) {
        if protocol_slot == -999 {
            self.drop_cursor(drop_stack);
            return;
        }

        if self.open_window_id != 0
            && protocol_slot >= 0
            && (protocol_slot as usize) < self.open_window_slot_count
        {
            if let Some(slot) = self.open_window_slots.get_mut(protocol_slot as usize) {
                drop_from_slot(slot, drop_stack);
                if slot.is_empty() {
                    if let Some(meta) = self.open_window_slot_meta.get_mut(protocol_slot as usize) {
                        *meta = ItemStackMeta::default();
                    }
                }
            }
            return;
        }

        if self.open_window_id != 0 {
            if let Some(local) = self.local_index_for_open_window_slot(protocol_slot) {
                drop_from_slot(&mut self.slots[local], drop_stack);
                if self.slots[local].is_empty() {
                    self.slot_meta[local] = ItemStackMeta::default();
                }
            }
            return;
        }

        match protocol_slot {
            0..=4 => {
                let idx = protocol_slot as usize;
                drop_from_slot(&mut self.crafting[idx], drop_stack);
                if self.crafting[idx].is_empty() {
                    self.crafting_meta[idx] = ItemStackMeta::default();
                }
            }
            5..=8 => {
                let idx = (protocol_slot - 5) as usize;
                drop_from_slot(&mut self.armor[idx], drop_stack);
                if self.armor[idx].is_empty() {
                    self.armor_meta[idx] = ItemStackMeta::default();
                }
            }
            _ => {
                if let Some(local) = Self::local_index_for_player_window_slot(protocol_slot) {
                    drop_from_slot(&mut self.slots[local], drop_stack);
                    if self.slots[local].is_empty() {
                        self.slot_meta[local] = ItemStackMeta::default();
                    }
                }
            }
        }
    }

    pub fn click_outside(&mut self, right_click: bool) {
        self.drop_cursor(!right_click);
    }

    fn drop_cursor(&mut self, drop_stack: bool) {
        drop_from_slot(&mut self.cursor, drop_stack);
        if self.cursor.is_empty() {
            self.cursor_meta = ItemStackMeta::default();
        }
    }

    fn left_click_local_slot(&mut self, slot: usize) {
        Self::click_stack_pair(
            &mut self.cursor,
            &mut self.cursor_meta,
            &mut self.slots[slot],
            &mut self.slot_meta[slot],
            false,
        );
    }

    fn right_click_local_slot(&mut self, slot: usize) {
        Self::click_stack_pair(
            &mut self.cursor,
            &mut self.cursor_meta,
            &mut self.slots[slot],
            &mut self.slot_meta[slot],
            true,
        );
    }

    fn click_stack_pair(
        cursor: &mut ItemStack,
        cursor_meta: &mut ItemStackMeta,
        target: &mut ItemStack,
        target_meta: &mut ItemStackMeta,
        right_click: bool,
    ) {
        if right_click {
            if cursor.is_empty() {
                if target.is_empty() {
                    return;
                }
                let take = (target.count + 1) / 2;
                *cursor = ItemStack {
                    count: take,
                    ..*target
                };
                *cursor_meta = target_meta.clone();
                target.count -= take;
                if target.count == 0 {
                    *target = ItemStack::EMPTY;
                    *target_meta = ItemStackMeta::default();
                }
            } else if target.is_empty() {
                *target = ItemStack {
                    count: 1,
                    ..*cursor
                };
                *target_meta = cursor_meta.clone();
                cursor.count -= 1;
                if cursor.count == 0 {
                    *cursor = ItemStack::EMPTY;
                    *cursor_meta = ItemStackMeta::default();
                }
            } else if target.can_stack_with(cursor) && target.count < target.max_stack() {
                target.count += 1;
                cursor.count -= 1;
                if cursor.count == 0 {
                    *cursor = ItemStack::EMPTY;
                    *cursor_meta = ItemStackMeta::default();
                }
            }
        } else if cursor.is_empty() {
            *cursor = *target;
            *cursor_meta = target_meta.clone();
            *target = ItemStack::EMPTY;
            *target_meta = ItemStackMeta::default();
        } else if target.is_empty() {
            *target = *cursor;
            *target_meta = cursor_meta.clone();
            *cursor = ItemStack::EMPTY;
            *cursor_meta = ItemStackMeta::default();
        } else if target.can_stack_with(cursor) {
            let space = target.max_stack().saturating_sub(target.count);
            let moved = space.min(cursor.count);
            target.count += moved;
            cursor.count -= moved;
            if cursor.count == 0 {
                *cursor = ItemStack::EMPTY;
                *cursor_meta = ItemStackMeta::default();
            }
        } else {
            std::mem::swap(target, cursor);
            std::mem::swap(target_meta, cursor_meta);
        }
    }

    fn shift_click_player_window_slot(&mut self, protocol_slot: i16) {
        match protocol_slot {
            36..=44 => {
                let local = (protocol_slot - 36) as usize;
                self.move_player_slot_to_ranges(local, &[(9, 36)]);
            }
            9..=35 => {
                let local = protocol_slot as usize;
                self.move_player_slot_to_ranges(local, &[(0, 9)]);
            }
            5..=8 => {
                let idx = (protocol_slot - 5) as usize;
                let stack = self.armor[idx];
                self.armor[idx] = ItemStack::EMPTY;
                self.armor_meta[idx] = ItemStackMeta::default();
                self.armor[idx] = self.merge_into_player_ranges(stack, &[(9, 36), (0, 9)]);
            }
            0..=4 => {
                let idx = protocol_slot as usize;
                let stack = self.crafting[idx];
                self.crafting[idx] = ItemStack::EMPTY;
                self.crafting_meta[idx] = ItemStackMeta::default();
                self.crafting[idx] = self.merge_into_player_ranges(stack, &[(9, 36), (0, 9)]);
            }
            _ => {}
        }
    }

    fn shift_click_open_window_slot(&mut self, protocol_slot: i16) {
        if protocol_slot >= 0 && (protocol_slot as usize) < self.open_window_slot_count {
            let idx = protocol_slot as usize;
            let stack = self.open_window_slots[idx];
            self.open_window_slots[idx] = ItemStack::EMPTY;
            if let Some(meta) = self.open_window_slot_meta.get_mut(idx) {
                *meta = ItemStackMeta::default();
            }
            self.open_window_slots[idx] = self.merge_into_player_ranges(stack, &[(9, 36), (0, 9)]);
            return;
        }

        if let Some(local) = self.local_index_for_open_window_slot(protocol_slot) {
            let stack = self.slots[local];
            self.slots[local] = ItemStack::EMPTY;
            self.slot_meta[local] = ItemStackMeta::default();
            self.slots[local] = self.merge_into_open_window(stack);
        }
    }

    fn move_player_slot_to_ranges(&mut self, local: usize, ranges: &[(usize, usize)]) {
        if local >= self.slots.len() {
            return;
        }
        let stack = self.slots[local];
        self.slots[local] = ItemStack::EMPTY;
        self.slot_meta[local] = ItemStackMeta::default();
        self.slots[local] = self.merge_into_player_ranges(stack, ranges);
    }

    fn merge_into_player_ranges(
        &mut self,
        mut stack: ItemStack,
        ranges: &[(usize, usize)],
    ) -> ItemStack {
        stack = stack.normalized();
        if stack.is_empty() {
            return ItemStack::EMPTY;
        }

        for &(start, end) in ranges {
            for idx in start.min(36)..end.min(36) {
                let target = &mut self.slots[idx];
                if target.can_stack_with(&stack) && target.count < target.max_stack() {
                    let moved = (target.max_stack() - target.count).min(stack.count);
                    target.count += moved;
                    stack.count -= moved;
                    if stack.count == 0 {
                        return ItemStack::EMPTY;
                    }
                }
            }
        }

        for &(start, end) in ranges {
            for idx in start.min(36)..end.min(36) {
                if self.slots[idx].is_empty() {
                    let moved = stack.max_stack().min(stack.count);
                    self.slots[idx] = ItemStack {
                        count: moved,
                        ..stack
                    };
                    stack.count -= moved;
                    if stack.count == 0 {
                        return ItemStack::EMPTY;
                    }
                }
            }
        }

        stack.normalized()
    }

    fn merge_into_open_window(&mut self, mut stack: ItemStack) -> ItemStack {
        stack = stack.normalized();
        if stack.is_empty() || self.open_window_slots.is_empty() {
            return stack;
        }

        for target in &mut self.open_window_slots {
            if target.can_stack_with(&stack) && target.count < target.max_stack() {
                let moved = (target.max_stack() - target.count).min(stack.count);
                target.count += moved;
                stack.count -= moved;
                if stack.count == 0 {
                    return ItemStack::EMPTY;
                }
            }
        }

        for target in &mut self.open_window_slots {
            if target.is_empty() {
                let moved = stack.max_stack().min(stack.count);
                *target = ItemStack {
                    count: moved,
                    ..stack
                };
                stack.count -= moved;
                if stack.count == 0 {
                    return ItemStack::EMPTY;
                }
            }
        }

        stack.normalized()
    }

    pub fn set_open_window(
        &mut self,
        window_id: u8,
        window_type: String,
        title: String,
        slot_count: usize,
    ) {
        let slot_count = effective_container_slot_count(&window_type, slot_count);
        self.open_chest_position = if window_type == "minecraft:chest" {
            self.pending_chest_position.take()
        } else {
            self.pending_chest_position = None;
            None
        };
        self.open_window_id = window_id;
        self.open_window_type = window_type;
        self.open_window_title = title;
        self.open_window_slot_count = slot_count;
        self.open_window_slots = vec![ItemStack::EMPTY; slot_count];
        self.open_window_slot_meta = vec![ItemStackMeta::default(); slot_count];
        self.open_window_properties.clear();
        self.window_just_opened = window_id != 0;
    }

    pub fn close_window(&mut self, window_id: u8) {
        if self.open_window_id == window_id {
            self.open_window_id = 0;
            self.open_window_type = "minecraft:container".to_string();
            self.open_window_title = "Inventory".to_string();
            self.open_window_slot_count = 0;
            self.open_window_slots.clear();
            self.open_window_slot_meta.clear();
            self.open_window_properties.clear();
            self.window_just_opened = false;
            self.pending_chest_position = None;
            // Do NOT clear open_chest_position here — the frame loop needs it
            // to call close_chest_for_local_viewer for the lid animation.
            // It is cleared by close_inventory_screen or overwritten by
            // set_open_window on the next chest open.
        }
    }

    pub fn apply_window_property(&mut self, window_id: u8, property: i16, value: i16) {
        if window_id != self.open_window_id {
            return;
        }
        if let Some(existing) = self
            .open_window_properties
            .iter_mut()
            .find(|(key, _)| *key == property)
        {
            existing.1 = value;
        } else {
            self.open_window_properties.push((property, value));
            self.open_window_properties.sort_by_key(|(key, _)| *key);
        }
    }

    pub fn apply_window_items(&mut self, window_id: u8, slots: &[crate::net::slot::Slot]) {
        if window_id == 0 {
            self.apply_player_window_items(slots);
        } else if window_id == self.open_window_id {
            for (idx, slot) in slots.iter().enumerate() {
                self.apply_window_slot(window_id as i8, idx as i16, slot);
            }
        }
    }

    pub fn apply_window_slot(
        &mut self,
        window_id: i8,
        slot_index: i16,
        slot: &crate::net::slot::Slot,
    ) {
        if slot_index == -1 {
            self.cursor = ItemStack::from_protocol_slot(slot);
            self.cursor_meta = ItemStackMeta {
                nbt: slot.nbt.clone(),
            };
            return;
        }

        if window_id == 0 {
            self.apply_player_window_slot(slot_index, slot);
        } else if window_id >= 0 && window_id as u8 == self.open_window_id {
            if slot_index >= 0 && (slot_index as usize) < self.open_window_slot_count {
                if let Some(target) = self.open_window_slots.get_mut(slot_index as usize) {
                    *target = ItemStack::from_protocol_slot(slot);
                }
                if let Some(meta) = self.open_window_slot_meta.get_mut(slot_index as usize) {
                    *meta = ItemStackMeta {
                        nbt: slot.nbt.clone(),
                    };
                }
            } else {
                let player_slot = slot_index - self.open_window_slot_count as i16 + 9;
                self.apply_player_window_slot(player_slot, slot);
            }
        }
    }

    fn apply_player_window_items(&mut self, slots: &[crate::net::slot::Slot]) {
        for (idx, slot) in slots.iter().enumerate() {
            self.apply_player_window_slot(idx as i16, slot);
        }
    }

    fn apply_player_window_slot(&mut self, slot_index: i16, slot: &crate::net::slot::Slot) {
        let stack = ItemStack::from_protocol_slot(slot);
        let meta = ItemStackMeta {
            nbt: slot.nbt.clone(),
        };
        match slot_index {
            0..=4 => {
                self.crafting[slot_index as usize] = stack;
                self.crafting_meta[slot_index as usize] = meta;
            }
            5..=8 => {
                let idx = (slot_index - 5) as usize;
                self.armor[idx] = stack;
                self.armor_meta[idx] = meta;
            }
            9..=35 => {
                self.slots[slot_index as usize] = stack;
                self.slot_meta[slot_index as usize] = meta;
            }
            36..=44 => {
                let idx = (slot_index - 36) as usize;
                self.slots[idx] = stack;
                self.slot_meta[idx] = meta;
            }
            _ => {}
        }
    }

    pub fn local_index_for_player_window_slot(slot_index: i16) -> Option<usize> {
        match slot_index {
            9..=35 => Some(slot_index as usize),
            36..=44 => Some((slot_index - 36) as usize),
            _ => None,
        }
    }

    pub fn local_index_for_open_window_slot(&self, protocol_slot: i16) -> Option<usize> {
        if self.open_window_id == 0 {
            return Self::local_index_for_player_window_slot(protocol_slot);
        }
        if protocol_slot < self.open_window_slot_count as i16 {
            return None;
        }
        let player_slot = protocol_slot - self.open_window_slot_count as i16 + 9;
        Self::local_index_for_player_window_slot(player_slot)
    }

    pub fn item_for_protocol_slot(&self, protocol_slot: i16) -> ItemStack {
        if protocol_slot == -999 {
            return self.cursor;
        }

        if self.open_window_id != 0
            && protocol_slot >= 0
            && (protocol_slot as usize) < self.open_window_slot_count
        {
            return self.open_window_slots[protocol_slot as usize];
        }

        if self.open_window_id != 0 {
            if let Some(local) = self.local_index_for_open_window_slot(protocol_slot) {
                return self.slots[local];
            }
        }

        if let Some(local) = Self::local_index_for_player_window_slot(protocol_slot) {
            return self.slots[local];
        }
        match protocol_slot {
            0..=4 => self.crafting[protocol_slot as usize],
            5..=8 => self.armor[(protocol_slot - 5) as usize],
            _ => ItemStack::EMPTY,
        }
    }

    pub fn item_view_for_protocol_slot(&self, protocol_slot: i16) -> ItemStackView {
        if protocol_slot == -999 {
            return self.cursor.view_with_meta(Some(&self.cursor_meta));
        }

        if self.open_window_id != 0
            && protocol_slot >= 0
            && (protocol_slot as usize) < self.open_window_slot_count
        {
            let idx = protocol_slot as usize;
            return self.open_window_slots[idx].view_with_meta(self.open_window_slot_meta.get(idx));
        }

        if self.open_window_id != 0 {
            if let Some(local) = self.local_index_for_open_window_slot(protocol_slot) {
                return self.slots[local].view_with_meta(Some(&self.slot_meta[local]));
            }
        }

        if let Some(local) = Self::local_index_for_player_window_slot(protocol_slot) {
            return self.slots[local].view_with_meta(Some(&self.slot_meta[local]));
        }
        match protocol_slot {
            0..=4 => self.crafting[protocol_slot as usize]
                .view_with_meta(Some(&self.crafting_meta[protocol_slot as usize])),
            5..=8 => {
                let idx = (protocol_slot - 5) as usize;
                self.armor[idx].view_with_meta(Some(&self.armor_meta[idx]))
            }
            _ => ItemStackView::default(),
        }
    }

    pub fn protocol_slot_for_selected_item(&self) -> crate::net::slot::Slot {
        self.slots[self.selected].to_protocol_slot_with_meta(Some(&self.slot_meta[self.selected]))
    }
}

fn drop_from_slot(slot: &mut ItemStack, drop_stack: bool) {
    if slot.is_empty() {
        return;
    }
    if drop_stack || slot.count <= 1 {
        *slot = ItemStack::EMPTY;
    } else {
        slot.count -= 1;
    }
}
