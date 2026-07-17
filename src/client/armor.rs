use super::inventory::Inventory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmorMaterial {
    Leather,
    Chain,
    Iron,
    Diamond,
    Gold,
}

/// Protection values per slot: [helmet, chestplate, leggings, boots]
const LEATHER_PROTECTION: [u8; 4] = [1, 3, 2, 1];
const CHAIN_PROTECTION: [u8; 4] = [2, 5, 4, 1];
const IRON_PROTECTION: [u8; 4] = [2, 6, 5, 2];
const DIAMOND_PROTECTION: [u8; 4] = [3, 8, 6, 3];
const GOLD_PROTECTION: [u8; 4] = [2, 5, 3, 1];

/// Base durability per slot: [helmet, chestplate, leggings, boots]
const LEATHER_DURABILITY: [u16; 4] = [55, 80, 75, 65];
const CHAIN_DURABILITY: [u16; 4] = [165, 240, 225, 195];
const IRON_DURABILITY: [u16; 4] = [165, 240, 225, 195];
const DIAMOND_DURABILITY: [u16; 4] = [363, 528, 495, 429];
const GOLD_DURABILITY: [u16; 4] = [77, 112, 105, 91];

/// MC 1.8.9 armor item IDs: 298-317
/// Slot order for return: 0=helmet, 1=chestplate, 2=leggings, 3=boots
pub fn armor_material_and_slot(item_id: u16) -> Option<(ArmorMaterial, usize)> {
    match item_id {
        // Leather
        298 => Some((ArmorMaterial::Leather, 0)), // helmet
        299 => Some((ArmorMaterial::Leather, 1)), // chestplate
        300 => Some((ArmorMaterial::Leather, 2)), // leggings
        301 => Some((ArmorMaterial::Leather, 3)), // boots
        // Chainmail
        302 => Some((ArmorMaterial::Chain, 0)),
        303 => Some((ArmorMaterial::Chain, 1)),
        304 => Some((ArmorMaterial::Chain, 2)),
        305 => Some((ArmorMaterial::Chain, 3)),
        // Iron
        306 => Some((ArmorMaterial::Iron, 0)),
        307 => Some((ArmorMaterial::Iron, 1)),
        308 => Some((ArmorMaterial::Iron, 2)),
        309 => Some((ArmorMaterial::Iron, 3)),
        // Diamond
        310 => Some((ArmorMaterial::Diamond, 0)),
        311 => Some((ArmorMaterial::Diamond, 1)),
        312 => Some((ArmorMaterial::Diamond, 2)),
        313 => Some((ArmorMaterial::Diamond, 3)),
        // Gold
        314 => Some((ArmorMaterial::Gold, 0)),
        315 => Some((ArmorMaterial::Gold, 1)),
        316 => Some((ArmorMaterial::Gold, 2)),
        317 => Some((ArmorMaterial::Gold, 3)),
        _ => None,
    }
}

pub fn protection_for(material: ArmorMaterial, slot: usize) -> u8 {
    match material {
        ArmorMaterial::Leather => LEATHER_PROTECTION[slot.min(3)],
        ArmorMaterial::Chain => CHAIN_PROTECTION[slot.min(3)],
        ArmorMaterial::Iron => IRON_PROTECTION[slot.min(3)],
        ArmorMaterial::Diamond => DIAMOND_PROTECTION[slot.min(3)],
        ArmorMaterial::Gold => GOLD_PROTECTION[slot.min(3)],
    }
}

/// Compute total armor points from inventory armor slots.
/// MC 1.8.9 armor points formula: sum of each piece's protection value.
pub fn total_armor_points(inventory: &Inventory) -> u8 {
    let mut points = 0u8;
    for armor_item in &inventory.armor {
        if armor_item.is_empty() {
            continue;
        }
        if let Some((material, slot)) = armor_material_and_slot(armor_item.item_id) {
            points += protection_for(material, slot);
        }
    }
    points.min(25)
}

/// MC 1.8.9 damage reduction: armor_points * 4%, capped at 80%.
pub fn damage_reduction(armor_points: u8) -> f32 {
    ((armor_points as f32) * 0.04).min(0.80)
}

/// Enchantment-based armor toughness is not in 1.8.9 — that's 1.9+.
pub fn durability_for(material: ArmorMaterial, slot: usize) -> u16 {
    match material {
        ArmorMaterial::Leather => LEATHER_DURABILITY[slot.min(3)],
        ArmorMaterial::Chain => CHAIN_DURABILITY[slot.min(3)],
        ArmorMaterial::Iron => IRON_DURABILITY[slot.min(3)],
        ArmorMaterial::Diamond => DIAMOND_DURABILITY[slot.min(3)],
        ArmorMaterial::Gold => GOLD_DURABILITY[slot.min(3)],
    }
}

/// Get armor texture name for a given material and layer.
/// Layer 1 = helmet + chestplate + boots, Layer 2 = leggings.
pub fn armor_texture_name(material: ArmorMaterial, layer: u8) -> &'static str {
    match (material, layer) {
        (ArmorMaterial::Leather, 1) => "leather_layer_1",
        (ArmorMaterial::Leather, _) => "leather_layer_2",
        (ArmorMaterial::Chain, 1) => "chainmail_layer_1",
        (ArmorMaterial::Chain, _) => "chainmail_layer_2",
        (ArmorMaterial::Iron, 1) => "iron_layer_1",
        (ArmorMaterial::Iron, _) => "iron_layer_2",
        (ArmorMaterial::Diamond, 1) => "diamond_layer_1",
        (ArmorMaterial::Diamond, _) => "diamond_layer_2",
        (ArmorMaterial::Gold, 1) => "gold_layer_1",
        (ArmorMaterial::Gold, _) => "gold_layer_2",
    }
}

/// Map an armor item ID to its texture names (layer_1, layer_2).
pub fn armor_texture_names(item_id: u16) -> Option<(&'static str, &'static str)> {
    let (material, _slot) = armor_material_and_slot(item_id)?;
    Some((
        armor_texture_name(material, 1),
        armor_texture_name(material, 2),
    ))
}

/// Check if an item ID is any armor piece.
pub fn is_armor(item_id: u16) -> bool {
    matches!(item_id, 298..=317)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_uses_its_real_layer_texture_name() {
        assert_eq!(
            armor_texture_name(ArmorMaterial::Chain, 1),
            "chainmail_layer_1"
        );
        assert_eq!(armor_texture_name(ArmorMaterial::Iron, 2), "iron_layer_2");
        assert_eq!(
            armor_texture_name(ArmorMaterial::Diamond, 1),
            "diamond_layer_1"
        );
        assert_eq!(armor_texture_name(ArmorMaterial::Gold, 2), "gold_layer_2");
    }
}
