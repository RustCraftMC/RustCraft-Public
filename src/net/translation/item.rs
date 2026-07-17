//! Canonical item representation used by translators and Lua packet views.

use super::ProtocolError;

#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalItemStack {
    pub item_id: String,
    pub count: i8,
    pub damage: i16,
    pub nbt: Option<serde_json::Value>,
}

pub fn v47_to_canonical(
    numeric_id: i16,
    count: i8,
    damage: i16,
    nbt: Option<serde_json::Value>,
) -> Result<Option<CanonicalItemStack>, ProtocolError> {
    if numeric_id < 0 {
        return Ok(None);
    }
    let item_id = match numeric_id as u16 {
        0..=255 => crate::world::block_models::block_id_to_name(numeric_id as u16)
            .map(|name| format!("minecraft:{name}")),
        id => crate::render::item_icons::item_icon_path(id, damage.max(0) as u16)
            .map(|name| format!("minecraft:{}", name.rsplit('/').next().unwrap_or(name))),
    }
    .ok_or_else(|| ProtocolError(format!("unknown v47 item id {numeric_id}")))?;
    Ok(Some(CanonicalItemStack {
        item_id,
        count,
        damage,
        nbt,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_real_legacy_diamond_sword_id() {
        let item = v47_to_canonical(276, 1, 3, None).unwrap().unwrap();
        assert_eq!(item.item_id, "minecraft:diamond_sword");
        assert_eq!(item.damage, 3);
    }
}
