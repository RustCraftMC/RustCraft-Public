use crate::client::inventory::ItemStackView;
use crate::net::nbt::NbtTag;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;
use crate::ui::format::format_text;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct TooltipLine {
    pub text: String,
    pub color: [f32; 4],
}

impl Renderer {
    pub(super) fn tooltip_lines_for_stack(&self, stack: &ItemStackView) -> Vec<TooltipLine> {
        if stack.is_empty() {
            return Vec::new();
        }

        let root = stack
            .nbt
            .as_deref()
            .and_then(|bytes| crate::net::nbt::parse_root(bytes).ok());
        let compound = root.as_ref().and_then(NbtTag::as_compound);
        let display = compound
            .and_then(|root| root.get("display"))
            .and_then(NbtTag::as_compound);
        let hide_flags = compound
            .and_then(|root| root.get("HideFlags"))
            .and_then(NbtTag::as_i32)
            .unwrap_or(0);

        let custom_name = display
            .and_then(|display| display.get("Name"))
            .and_then(NbtTag::as_str)
            .filter(|name| !name.is_empty());
        let mut lines = vec![TooltipLine {
            text: custom_name
                .map(strip_mc_formatting)
                .unwrap_or_else(|| self.item_display_name(stack.item_id, stack.damage)),
            color: if custom_name.is_some() {
                [0.68, 0.33, 1.0, 1.0]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            },
        }];

        if hide_flags & HIDE_ENCHANTMENTS == 0 {
            for enchantment in enchantments_from(compound, "ench")
                .into_iter()
                .chain(enchantments_from(compound, "StoredEnchantments"))
            {
                lines.push(TooltipLine {
                    text: self.enchantment_display_name(enchantment.id, enchantment.level),
                    color: TOOLTIP_BLUE,
                });
            }
        }

        if hide_flags & HIDE_ADDITIONAL == 0 {
            self.append_extra_stack_details(stack, compound, &mut lines);
        }

        if hide_flags & HIDE_UNBREAKABLE == 0
            && compound
                .and_then(|root| root.get("Unbreakable"))
                .and_then(NbtTag::as_i32)
                .unwrap_or(0)
                != 0
        {
            lines.push(TooltipLine {
                text: self.t_dynamic("item.unbreakable"),
                color: TOOLTIP_BLUE,
            });
        }

        if let Some(lore) = display
            .and_then(|display| display.get("Lore"))
            .and_then(NbtTag::as_list)
        {
            for line in lore.iter().filter_map(NbtTag::as_str) {
                lines.push(TooltipLine {
                    text: strip_mc_formatting(line),
                    color: [0.68, 0.33, 1.0, 1.0],
                });
            }
        }

        if hide_flags & HIDE_CAN_DESTROY == 0 {
            self.append_block_list(compound, "CanDestroy", "item.canBreak", &mut lines);
        }
        if hide_flags & HIDE_CAN_PLACE == 0 {
            self.append_block_list(compound, "CanPlaceOn", "item.canPlace", &mut lines);
        }
        if hide_flags & HIDE_ATTRIBUTES == 0 {
            self.append_attribute_modifiers(compound, &mut lines);
        }

        if self.state.advanced_tooltips {
            lines.push(TooltipLine {
                text: format!("#{:04} / {}", stack.item_id, stack.damage),
                color: TOOLTIP_DARK_GRAY,
            });
            if let Some(nbt) = &stack.nbt {
                let tag_count = compound.map(|root| root.len()).unwrap_or(0);
                lines.push(TooltipLine {
                    text: format_text(
                        &self.t_dynamic("rustcraft.tooltip.nbt"),
                        &[&tag_count.to_string(), &nbt.len().to_string()],
                    ),
                    color: TOOLTIP_DARK_GRAY,
                });
            }
        }

        lines
    }

    pub(super) fn draw_item_tooltip(
        &mut self,
        font_gui: &mut GuiVertexBuilder,
        mouse_pos: [f32; 2],
        lines: &[TooltipLine],
        font_sz: f32,
        gs: f32,
    ) {
        if lines.is_empty() {
            return;
        }

        let sw = self.swapchain_extent.width as f32;
        let sh = self.swapchain_extent.height as f32;
        let text_size = font_sz * 0.62;
        let line_h = 10.0 * gs;
        let content_w = lines
            .iter()
            .map(|line| {
                self.font
                    .text_width(&strip_mc_formatting(&line.text), text_size)
            })
            .fold(0.0_f32, f32::max);
        let w = content_w + 8.0 * gs;
        // Vanilla leaves a two-pixel gap between the item name and its details.
        let detail_gap = (lines.len() > 1) as u8 as f32 * 2.0 * gs;
        let h = 8.0 * gs + lines.len() as f32 * line_h + detail_gap;
        let mut x = mouse_pos[0] + 12.0 * gs;
        let mut y = mouse_pos[1] - 12.0 * gs;
        if x + w > sw - 4.0 * gs {
            x = mouse_pos[0] - w - 12.0 * gs;
        }
        if y + h > sh - 4.0 * gs {
            y = sh - h - 4.0 * gs;
        }
        x = x.max(4.0 * gs);
        y = y.max(4.0 * gs);

        font_gui.fill_rect(
            x - 3.0 * gs,
            y - 4.0 * gs,
            w + 6.0 * gs,
            h + 6.0 * gs,
            [0.05, 0.00, 0.08, 0.94],
        );
        font_gui.fill_rect(
            x - 2.0 * gs,
            y - 3.0 * gs,
            w + 4.0 * gs,
            1.0 * gs,
            [0.22, 0.06, 0.34, 1.0],
        );
        font_gui.fill_rect(
            x - 2.0 * gs,
            y + h + 2.0 * gs,
            w + 4.0 * gs,
            1.0 * gs,
            [0.22, 0.06, 0.34, 1.0],
        );
        font_gui.fill_rect(
            x - 2.0 * gs,
            y - 3.0 * gs,
            1.0 * gs,
            h + 6.0 * gs,
            [0.22, 0.06, 0.34, 1.0],
        );
        font_gui.fill_rect(
            x + w + 1.0 * gs,
            y - 3.0 * gs,
            1.0 * gs,
            h + 6.0 * gs,
            [0.22, 0.06, 0.34, 1.0],
        );

        for (idx, line) in lines.iter().enumerate() {
            let text = strip_mc_formatting(&line.text);
            font_gui.draw_text(
                &mut self.font,
                x + 3.0 * gs,
                y + 3.0 * gs + idx as f32 * line_h + (idx > 0) as u8 as f32 * 2.0 * gs,
                &text,
                text_size,
                line.color,
            );
        }
    }

    pub(crate) fn item_display_name(&self, item_id: u16, damage: u16) -> String {
        if item_id < 256 {
            for key in block_translation_keys(item_id, damage) {
                let value = self.t_dynamic(key);
                if value != *key {
                    return value;
                }
            }
        }

        for key in item_translation_keys(item_id, damage) {
            let value = self.t_dynamic(key);
            if value != *key {
                return value;
            }
        }

        self.state.ui_text.get("rustcraft.item.unknown").to_string()
    }

    fn append_extra_stack_details(
        &self,
        stack: &ItemStackView,
        compound: Option<&HashMap<String, NbtTag>>,
        lines: &mut Vec<TooltipLine>,
    ) {
        if stack.item_id == 373 {
            let mut potion_lines = self.potion_tooltip_lines(stack.damage, compound);
            if lines
                .first()
                .map(|line| line.text == self.t_dynamic("item.potion.name"))
                .unwrap_or(false)
            {
                if let Some(first) = potion_lines.first() {
                    lines[0].text = first.text.clone();
                    potion_lines.remove(0);
                }
            }
            lines.extend(potion_lines);
        }

        if stack.item_id == 401 {
            if let Some(flight) = compound
                .and_then(|root| root.get("Fireworks"))
                .and_then(NbtTag::as_compound)
                .and_then(|fireworks| fireworks.get("Flight"))
                .and_then(NbtTag::as_i32)
            {
                lines.push(TooltipLine {
                    text: format!("{} {}", self.t_dynamic("item.fireworks.flight"), flight),
                    color: TOOLTIP_GRAY,
                });
            }
        }

        if self.state.advanced_tooltips {
            if let Some(max_damage) = max_damage_for_item(stack.item_id) {
                let remaining = max_damage.saturating_sub(stack.damage as u32);
                lines.push(TooltipLine {
                    text: format_text(
                        &self.t_dynamic("rustcraft.tooltip.durability"),
                        &[&remaining.to_string(), &max_damage.to_string()],
                    ),
                    color: TOOLTIP_GRAY,
                });
            }
        }
    }

    fn append_block_list(
        &self,
        compound: Option<&HashMap<String, NbtTag>>,
        tag_key: &str,
        title_key: &str,
        lines: &mut Vec<TooltipLine>,
    ) {
        let Some(blocks) = compound
            .and_then(|root| root.get(tag_key))
            .and_then(NbtTag::as_list)
        else {
            return;
        };
        let mut names = blocks
            .iter()
            .filter_map(NbtTag::as_str)
            .map(|name| self.block_name_from_nbt_id(name))
            .collect::<Vec<_>>();
        names.dedup();
        if names.is_empty() {
            return;
        }

        lines.push(TooltipLine {
            text: String::new(),
            color: TOOLTIP_GRAY,
        });
        lines.push(TooltipLine {
            text: self.t_dynamic(title_key),
            color: TOOLTIP_GRAY,
        });
        for name in names {
            lines.push(TooltipLine {
                text: name,
                color: TOOLTIP_DARK_GRAY,
            });
        }
    }

    fn append_attribute_modifiers(
        &self,
        compound: Option<&HashMap<String, NbtTag>>,
        lines: &mut Vec<TooltipLine>,
    ) {
        let Some(modifiers) = compound
            .and_then(|root| root.get("AttributeModifiers"))
            .and_then(NbtTag::as_list)
        else {
            return;
        };
        let mut added_header = false;
        for modifier in modifiers {
            let Some(modifier) = modifier.as_compound() else {
                continue;
            };
            let attr = modifier
                .get("AttributeName")
                .or_else(|| modifier.get("Name"))
                .and_then(NbtTag::as_str)
                .unwrap_or("");
            let amount = modifier
                .get("Amount")
                .and_then(NbtTag::as_f64)
                .unwrap_or(0.0);
            let operation = modifier
                .get("Operation")
                .and_then(NbtTag::as_i32)
                .unwrap_or(0)
                .clamp(0, 2);
            if attr.is_empty() || amount == 0.0 {
                continue;
            }
            if !added_header {
                lines.push(TooltipLine {
                    text: String::new(),
                    color: TOOLTIP_GRAY,
                });
                lines.push(TooltipLine {
                    text: self.t_dynamic("rustcraft.tooltip.whenEquipped"),
                    color: TOOLTIP_GRAY,
                });
                added_header = true;
            }
            let attr_name = self.attribute_name(attr);
            lines.push(TooltipLine {
                text: self.attribute_modifier_text(amount, operation, &attr_name),
                color: if amount > 0.0 {
                    [0.33, 0.78, 0.33, 1.0]
                } else {
                    [0.78, 0.33, 0.33, 1.0]
                },
            });
        }
    }

    fn potion_tooltip_lines(
        &self,
        damage: u16,
        compound: Option<&HashMap<String, NbtTag>>,
    ) -> Vec<TooltipLine> {
        let mut lines = Vec::new();
        let potion = potion_effect_from_damage(damage);
        let mut potion_name = self.t_dynamic(
            potion
                .map(|effect| effect.postfix_key)
                .unwrap_or("item.emptyPotion.name"),
        );
        if damage & 0x4000 != 0 && potion.is_some() {
            let splash = self.t_dynamic("potion.prefix.grenade");
            potion_name = format!("{} {}", splash, potion_name);
        }
        lines.push(TooltipLine {
            text: potion_name,
            color: [1.0, 1.0, 1.0, 1.0],
        });

        let custom_effects = compound
            .and_then(|root| root.get("CustomPotionEffects"))
            .and_then(NbtTag::as_list);
        if let Some(custom_effects) = custom_effects {
            for effect in custom_effects {
                let Some(effect) = effect.as_compound() else {
                    continue;
                };
                let id = effect.get("Id").and_then(NbtTag::as_i32).unwrap_or(0);
                let amplifier = effect
                    .get("Amplifier")
                    .and_then(NbtTag::as_i32)
                    .unwrap_or(0);
                let duration = effect.get("Duration").and_then(NbtTag::as_i32).unwrap_or(0);
                if let Some(effect) = potion_effect_by_id(id) {
                    lines.push(TooltipLine {
                        text: self.potion_effect_line(effect.name_key, amplifier, duration),
                        color: TOOLTIP_BLUE,
                    });
                }
            }
        } else if let Some(effect) = potion {
            lines.push(TooltipLine {
                text: self.potion_effect_line(effect.name_key, effect.amplifier, effect.duration),
                color: TOOLTIP_BLUE,
            });
        } else {
            lines.push(TooltipLine {
                text: self.t_dynamic("potion.empty"),
                color: TOOLTIP_GRAY,
            });
        }

        lines
    }

    fn potion_effect_line(&self, name_key: &str, amplifier: i32, duration_ticks: i32) -> String {
        let potency_key = format!("potion.potency.{}", amplifier.clamp(0, 3));
        let potency = self.t_dynamic(&potency_key);
        let duration = format_duration(duration_ticks);
        let name = self.t_dynamic(name_key);
        if potency.is_empty() {
            format!("{} ({})", name, duration)
        } else {
            format!("{} {} ({})", name, potency, duration)
        }
    }

    fn block_name_from_nbt_id(&self, id: &str) -> String {
        let normalized = id.strip_prefix("minecraft:").unwrap_or(id);
        for key in block_id_translation_keys(normalized) {
            let value = self.t_dynamic(key);
            if value != *key {
                return value;
            }
        }
        normalized.to_string()
    }

    fn attribute_name(&self, key: &str) -> String {
        let lang_key = format!("attribute.name.{}", key);
        let value = self.t_dynamic(&lang_key);
        if value == lang_key {
            key.rsplit('.').next().unwrap_or(key).to_string()
        } else {
            value
        }
    }

    fn attribute_modifier_text(&self, amount: f64, operation: i32, attr_name: &str) -> String {
        let percent = operation == 1 || operation == 2;
        let value = if percent { amount * 100.0 } else { amount };
        let key = if value >= 0.0 {
            format!("attribute.modifier.plus.{}", operation)
        } else {
            format!("attribute.modifier.take.{}", operation)
        };
        format_text(
            &self.t_dynamic(&key),
            &[&format_number(value.abs()), attr_name],
        )
    }

    fn enchantment_display_name(&self, id: i16, level: i16) -> String {
        let key = enchantment_key(id);
        let name = key
            .map(|key| self.t_dynamic(key))
            .filter(|value| !value.starts_with("enchantment."))
            .unwrap_or_else(|| {
                self.state
                    .ui_text
                    .get("rustcraft.enchant.unknown")
                    .to_string()
            });
        let level_key = format!("enchantment.level.{}", level.max(1));
        let level_text = self.t_dynamic(&level_key);
        if level_text == level_key {
            format!("{} {}", name, level)
        } else {
            format!("{} {}", name, level_text)
        }
    }

    pub(super) fn t_dynamic(&self, key: &str) -> String {
        self.state.ui_text.dynamic(key)
    }
}

const HIDE_ENCHANTMENTS: i32 = 0x01;
const HIDE_ATTRIBUTES: i32 = 0x02;
const HIDE_UNBREAKABLE: i32 = 0x04;
const HIDE_CAN_DESTROY: i32 = 0x08;
const HIDE_CAN_PLACE: i32 = 0x10;
const HIDE_ADDITIONAL: i32 = 0x20;
const TOOLTIP_BLUE: [f32; 4] = [0.56, 0.56, 1.0, 1.0];
const TOOLTIP_GRAY: [f32; 4] = [0.62, 0.62, 0.62, 1.0];
const TOOLTIP_DARK_GRAY: [f32; 4] = [0.45, 0.45, 0.45, 1.0];

#[derive(Clone, Copy, Debug)]
struct Enchantment {
    id: i16,
    level: i16,
}

fn enchantments_from(compound: Option<&HashMap<String, NbtTag>>, key: &str) -> Vec<Enchantment> {
    compound
        .and_then(|root| root.get(key))
        .and_then(NbtTag::as_list)
        .map(|list| {
            list.iter()
                .filter_map(|tag| {
                    let compound = tag.as_compound()?;
                    Some(Enchantment {
                        id: compound.get("id").and_then(NbtTag::as_i16).unwrap_or(0),
                        level: compound.get("lvl").and_then(NbtTag::as_i16).unwrap_or(1),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn enchantment_key(id: i16) -> Option<&'static str> {
    Some(match id {
        0 => "enchantment.protect.all",
        1 => "enchantment.protect.fire",
        2 => "enchantment.protect.fall",
        3 => "enchantment.protect.explosion",
        4 => "enchantment.protect.projectile",
        5 => "enchantment.oxygen",
        6 => "enchantment.waterWorker",
        7 => "enchantment.thorns",
        8 => "enchantment.waterWalker",
        16 => "enchantment.damage.all",
        17 => "enchantment.damage.undead",
        18 => "enchantment.damage.arthropods",
        19 => "enchantment.knockback",
        20 => "enchantment.fire",
        21 => "enchantment.lootBonus",
        32 => "enchantment.digging",
        33 => "enchantment.untouching",
        34 => "enchantment.durability",
        35 => "enchantment.lootBonusDigger",
        48 => "enchantment.arrowDamage",
        49 => "enchantment.arrowKnockback",
        50 => "enchantment.arrowFire",
        51 => "enchantment.arrowInfinite",
        61 => "enchantment.lootBonusFishing",
        62 => "enchantment.fishingSpeed",
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug)]
struct PotionEffect {
    name_key: &'static str,
    postfix_key: &'static str,
    amplifier: i32,
    duration: i32,
}

fn potion_effect_from_damage(damage: u16) -> Option<PotionEffect> {
    let base = damage & 0x3f;
    let amplified = damage & 0x20 != 0;
    let extended = damage & 0x40 != 0;
    let (name_key, postfix_key, default_duration) = match base {
        1 => ("potion.regeneration", "potion.regeneration.postfix", 900),
        2 => ("potion.moveSpeed", "potion.moveSpeed.postfix", 3600),
        3 => (
            "potion.fireResistance",
            "potion.fireResistance.postfix",
            3600,
        ),
        4 => ("potion.poison", "potion.poison.postfix", 900),
        5 => ("potion.heal", "potion.heal.postfix", 1),
        6 => ("potion.nightVision", "potion.nightVision.postfix", 3600),
        8 => ("potion.weakness", "potion.weakness.postfix", 1800),
        9 => ("potion.damageBoost", "potion.damageBoost.postfix", 3600),
        10 => ("potion.moveSlowdown", "potion.moveSlowdown.postfix", 1800),
        11 => ("potion.jump", "potion.jump.postfix", 3600),
        12 => ("potion.harm", "potion.harm.postfix", 1),
        13 => (
            "potion.waterBreathing",
            "potion.waterBreathing.postfix",
            3600,
        ),
        14 => ("potion.invisibility", "potion.invisibility.postfix", 3600),
        _ => return None,
    };
    let mut duration = default_duration;
    if extended && duration > 1 {
        duration = (duration as f32 * 8.0 / 3.0).round() as i32;
    }
    if amplified && duration > 1 {
        duration /= 2;
    }
    Some(PotionEffect {
        name_key,
        postfix_key,
        amplifier: if amplified { 1 } else { 0 },
        duration,
    })
}

fn potion_effect_by_id(id: i32) -> Option<PotionEffect> {
    let (name_key, postfix_key) = match id {
        1 => ("potion.moveSpeed", "potion.moveSpeed.postfix"),
        2 => ("potion.moveSlowdown", "potion.moveSlowdown.postfix"),
        3 => ("potion.digSpeed", "potion.digSpeed.postfix"),
        4 => ("potion.digSlowDown", "potion.digSlowDown.postfix"),
        5 => ("potion.damageBoost", "potion.damageBoost.postfix"),
        6 => ("potion.heal", "potion.heal.postfix"),
        7 => ("potion.harm", "potion.harm.postfix"),
        8 => ("potion.jump", "potion.jump.postfix"),
        9 => ("potion.confusion", "potion.confusion.postfix"),
        10 => ("potion.regeneration", "potion.regeneration.postfix"),
        11 => ("potion.resistance", "potion.resistance.postfix"),
        12 => ("potion.fireResistance", "potion.fireResistance.postfix"),
        13 => ("potion.waterBreathing", "potion.waterBreathing.postfix"),
        14 => ("potion.invisibility", "potion.invisibility.postfix"),
        15 => ("potion.blindness", "potion.blindness.postfix"),
        16 => ("potion.nightVision", "potion.nightVision.postfix"),
        17 => ("potion.hunger", "potion.hunger.postfix"),
        18 => ("potion.weakness", "potion.weakness.postfix"),
        19 => ("potion.poison", "potion.poison.postfix"),
        20 => ("potion.wither", "potion.wither.postfix"),
        21 => ("potion.healthBoost", "potion.healthBoost.postfix"),
        22 => ("potion.absorption", "potion.absorption.postfix"),
        23 => ("potion.saturation", "potion.saturation.postfix"),
        _ => return None,
    };
    Some(PotionEffect {
        name_key,
        postfix_key,
        amplifier: 0,
        duration: 0,
    })
}

fn block_translation_keys(item_id: u16, damage: u16) -> &'static [&'static str] {
    match item_id {
        1 => match damage {
            1 => &["tile.stone.granite.name"],
            2 => &["tile.stone.graniteSmooth.name"],
            3 => &["tile.stone.diorite.name"],
            4 => &["tile.stone.dioriteSmooth.name"],
            5 => &["tile.stone.andesite.name"],
            6 => &["tile.stone.andesiteSmooth.name"],
            _ => &["tile.stone.stone.name", "tile.stone.name"],
        },
        2 => &["tile.grass.name"],
        3 => match damage {
            1 => &["tile.dirt.coarse.name"],
            2 => &["tile.dirt.podzol.name"],
            _ => &["tile.dirt.default.name", "tile.dirt.name"],
        },
        4 => &["tile.stonebrick.name", "tile.cobblestone.name"],
        5 => wood_variant_keys(damage, "tile.wood"),
        6 => wood_variant_keys(damage, "tile.sapling"),
        7 => &["tile.bedrock.name"],
        8 | 9 => &["tile.water.name"],
        10 | 11 => &["tile.lava.name"],
        12 => match damage {
            1 => &["tile.sand.red.name"],
            _ => &["tile.sand.default.name", "tile.sand.name"],
        },
        13 => &["tile.gravel.name"],
        14 => &["tile.oreGold.name"],
        15 => &["tile.oreIron.name"],
        16 => &["tile.oreCoal.name"],
        17 => wood_variant_keys(damage, "tile.log"),
        18 => wood_variant_keys(damage, "tile.leaves"),
        19 => match damage {
            1 => &["tile.sponge.wet.name"],
            _ => &["tile.sponge.dry.name", "tile.sponge.name"],
        },
        20 => &["tile.glass.name"],
        21 => &["tile.oreLapis.name"],
        22 => &["tile.blockLapis.name"],
        23 => &["tile.dispenser.name"],
        24 => match damage {
            1 => &["tile.sandStone.chiseled.name"],
            2 => &["tile.sandStone.smooth.name"],
            _ => &["tile.sandStone.default.name", "tile.sandStone.name"],
        },
        25 => &["tile.musicBlock.name"],
        26 => &["tile.bed.name"],
        27 => &["tile.goldenRail.name"],
        28 => &["tile.detectorRail.name"],
        29 => &["tile.pistonStickyBase.name"],
        30 => &["tile.web.name"],
        31 => match damage {
            0 => &["tile.tallgrass.shrub.name"],
            2 => &["tile.tallgrass.fern.name"],
            _ => &["tile.tallgrass.grass.name", "tile.tallgrass.name"],
        },
        32 => &["tile.deadbush.name"],
        33 => &["tile.pistonBase.name"],
        35 => dye_variant_keys(damage, "tile.cloth"),
        37 => &["tile.flower1.dandelion.name"],
        38 => flower_variant_keys(damage),
        39 | 40 => &["tile.mushroom.name"],
        41 => &["tile.blockGold.name", "tile.goldBlock.name"],
        42 => &["tile.blockIron.name", "tile.ironBlock.name"],
        43 | 44 => stone_slab_keys(damage),
        45 => &["tile.brick.name"],
        46 => &["tile.tnt.name"],
        47 => &["tile.bookshelf.name"],
        48 => &["tile.stoneMoss.name", "tile.mossyCobblestone.name"],
        49 => &["tile.obsidian.name"],
        50 => &["tile.torch.name"],
        52 => &["tile.mobSpawner.name"],
        53 => &["tile.stairsWood.name"],
        54 => &["tile.chest.name"],
        55 => &["tile.redstoneDust.name"],
        56 => &["tile.oreDiamond.name"],
        57 => &["tile.blockDiamond.name"],
        58 => &["tile.workbench.name", "tile.craftingTable.name"],
        61 | 62 => &["tile.furnace.name"],
        64 => &["tile.doorWood.name", "item.doorOak.name"],
        65 => &["tile.ladder.name"],
        66 => &["tile.rail.name"],
        67 => &["tile.stairsStone.name"],
        68 => &["tile.sign.name"],
        69 => &["tile.lever.name"],
        70 => &["tile.pressurePlateStone.name"],
        71 => &["tile.doorIron.name", "item.doorIron.name"],
        72 => &["tile.pressurePlateWood.name"],
        73 | 74 => &["tile.oreRedstone.name"],
        75 | 76 => &["tile.notGate.name"],
        77 => &["tile.button.name"],
        78 => &["tile.snow.name"],
        79 => &["tile.ice.name"],
        80 => &["tile.snow.name"],
        81 => &["tile.cactus.name"],
        82 => &["tile.clay.name"],
        83 => &["tile.reeds.name", "item.reeds.name"],
        84 => &["tile.jukebox.name"],
        85 => wood_fence_keys(damage, "tile.fence"),
        86 => &["tile.pumpkin.name"],
        87 => &["tile.hellrock.name"],
        88 => &["tile.hellsand.name"],
        89 => &["tile.lightgem.name", "tile.glowstone.name"],
        91 => &["tile.litpumpkin.name"],
        92 => &["tile.cake.name"],
        93 | 94 => &["item.diode.name"],
        96 => &["tile.trapdoor.name"],
        97 => monster_egg_keys(damage),
        98 => stone_brick_keys(damage),
        101 => &["tile.fenceIron.name"],
        102 => &["tile.thinGlass.name"],
        103 => &["tile.melon.name"],
        106 => &["tile.vine.name"],
        107 => wood_fence_keys(damage, "tile.fenceGate"),
        108 => &["tile.stairsBrick.name"],
        109 => &["tile.stairsStoneBrickSmooth.name"],
        110 => &["tile.mycel.name"],
        111 => &["tile.waterlily.name"],
        112 => &["tile.netherBrick.name"],
        113 => &["tile.netherFence.name"],
        114 => &["tile.stairsNetherBrick.name"],
        115 => &["tile.netherStalk.name"],
        116 => &["tile.enchantmentTable.name"],
        117 => &["item.brewingStand.name", "tile.brewingStand.name"],
        118 => &["item.cauldron.name", "tile.cauldron.name"],
        120 => &["tile.endPortalFrame.name"],
        121 => &["tile.whiteStone.name"],
        122 => &["tile.dragonEgg.name"],
        123 | 124 => &["tile.redstoneLight.name"],
        126 => wood_variant_keys(damage, "tile.woodSlab"),
        128 => &["tile.stairsSandStone.name"],
        129 => &["tile.oreEmerald.name"],
        130 => &["tile.enderChest.name"],
        131 => &["tile.tripWireSource.name"],
        132 => &["tile.tripWire.name"],
        133 => &["tile.blockEmerald.name"],
        134 => &["tile.stairsWoodSpruce.name"],
        135 => &["tile.stairsWoodBirch.name"],
        136 => &["tile.stairsWoodJungle.name"],
        137 => &["tile.commandBlock.name"],
        138 => &["tile.beacon.name"],
        139 => match damage {
            1 => &["tile.cobbleWall.mossy.name"],
            _ => &["tile.cobbleWall.normal.name"],
        },
        140 => &["tile.flowerPot.name", "item.flowerPot.name"],
        143 => &["tile.button.name"],
        144 => &["item.skull.skeleton.name"],
        145 => match damage {
            1 => &["tile.anvil.slightlyDamaged.name"],
            2 => &["tile.anvil.veryDamaged.name"],
            _ => &["tile.anvil.intact.name", "tile.anvil.name"],
        },
        146 => &["tile.chestTrap.name"],
        147 => &["tile.weightedPlate_light.name"],
        148 => &["tile.weightedPlate_heavy.name"],
        149 | 150 => &["item.comparator.name"],
        151 => &["tile.daylightDetector.name"],
        152 => &["tile.blockRedstone.name"],
        153 => &["tile.netherquartz.name"],
        154 => &["tile.hopper.name"],
        155 => match damage {
            1 => &["tile.quartzBlock.chiseled.name"],
            2 => &["tile.quartzBlock.lines.name"],
            _ => &["tile.quartzBlock.default.name", "tile.quartzBlock.name"],
        },
        156 => &["tile.stairsQuartz.name"],
        157 => &["tile.activatorRail.name"],
        158 => &["tile.dropper.name"],
        159 => dye_variant_keys(damage, "tile.clayHardenedStained"),
        160 => dye_variant_keys(damage, "tile.thinStainedGlass"),
        161 => wood_variant_keys(damage + 4, "tile.leaves"),
        162 => wood_variant_keys(damage + 4, "tile.log"),
        170 => &["tile.hayBlock.name"],
        171 => dye_variant_keys(damage, "tile.woolCarpet"),
        172 => &["tile.clayHardened.name"],
        173 => &["tile.blockCoal.name", "tile.coalBlock.name"],
        174 => &["tile.icePacked.name", "tile.packedIce.name"],
        175 => double_plant_keys(damage),
        95 => dye_variant_keys(damage, "tile.stainedGlass"),
        163 => &["tile.stairsWoodAcacia.name"],
        164 => &["tile.stairsWoodDarkOak.name"],
        165 => &["tile.slime.name"],
        166 => &["tile.barrier.name"],
        167 => &["tile.ironTrapdoor.name"],
        168 => match damage {
            1 => &["tile.prismarine.bricks.name"],
            2 => &["tile.prismarine.dark.name"],
            _ => &["tile.prismarine.rough.name"],
        },
        169 => &["tile.seaLantern.name"],
        178 => &["tile.daylightDetector.name"],
        179 => match damage {
            1 => &["tile.redSandStone.chiseled.name"],
            2 => &["tile.redSandStone.smooth.name"],
            _ => &["tile.redSandStone.default.name", "tile.redSandStone.name"],
        },
        180 => &["tile.stairsRedSandStone.name"],
        181 | 182 => &["tile.stoneSlab2.red_sandstone.name"],
        183 => &["tile.spruceFenceGate.name"],
        184 => &["tile.birchFenceGate.name"],
        185 => &["tile.jungleFenceGate.name"],
        186 => &["tile.darkOakFenceGate.name"],
        187 => &["tile.acaciaFenceGate.name"],
        188 => &["tile.spruceFence.name"],
        189 => &["tile.birchFence.name"],
        190 => &["tile.jungleFence.name"],
        191 => &["tile.darkOakFence.name"],
        192 => &["tile.acaciaFence.name"],
        193 => &["tile.doorWood.name", "item.doorSpruce.name"],
        194 => &["tile.doorWood.name", "item.doorBirch.name"],
        195 => &["tile.doorWood.name", "item.doorJungle.name"],
        196 => &["tile.doorWood.name", "item.doorAcacia.name"],
        197 => &["tile.doorWood.name", "item.doorDarkOak.name"],
        _ => &[],
    }
}

fn item_translation_keys(item_id: u16, damage: u16) -> &'static [&'static str] {
    match item_id {
        256 => &["item.shovelIron.name"],
        257 => &["item.pickaxeIron.name"],
        258 => &["item.hatchetIron.name", "item.axeIron.name"],
        259 => &["item.flintAndSteel.name"],
        260 => &["item.apple.name"],
        261 => &["item.bow.name"],
        262 => &["item.arrow.name"],
        263 => match damage {
            1 => &["item.charcoal.name"],
            _ => &["item.coal.name"],
        },
        264 => &["item.diamond.name"],
        265 => &["item.ingotIron.name"],
        266 => &["item.ingotGold.name"],
        267 => &["item.swordIron.name"],
        268 => &["item.swordWood.name"],
        269 => &["item.shovelWood.name"],
        270 => &["item.pickaxeWood.name"],
        271 => &["item.hatchetWood.name"],
        272 => &["item.swordStone.name"],
        273 => &["item.shovelStone.name"],
        274 => &["item.pickaxeStone.name"],
        275 => &["item.hatchetStone.name"],
        276 => &["item.swordDiamond.name"],
        277 => &["item.shovelDiamond.name"],
        278 => &["item.pickaxeDiamond.name"],
        279 => &["item.hatchetDiamond.name"],
        280 => &["item.stick.name"],
        281 => &["item.bowl.name"],
        282 => &["item.mushroomStew.name"],
        283 => &["item.swordGold.name"],
        284 => &["item.shovelGold.name"],
        285 => &["item.pickaxeGold.name"],
        286 => &["item.hatchetGold.name"],
        287 => &["item.string.name"],
        288 => &["item.feather.name"],
        289 => &["item.sulphur.name"],
        290 => &["item.hoeWood.name"],
        291 => &["item.hoeStone.name"],
        292 => &["item.hoeIron.name"],
        293 => &["item.hoeDiamond.name"],
        294 => &["item.hoeGold.name"],
        295 => &["item.seeds.name"],
        296 => &["item.wheat.name"],
        297 => &["item.bread.name"],
        298 => &["item.helmetCloth.name"],
        299 => &["item.chestplateCloth.name"],
        300 => &["item.leggingsCloth.name"],
        301 => &["item.bootsCloth.name"],
        302 => &["item.helmetChain.name"],
        303 => &["item.chestplateChain.name"],
        304 => &["item.leggingsChain.name"],
        305 => &["item.bootsChain.name"],
        306 => &["item.helmetIron.name"],
        307 => &["item.chestplateIron.name"],
        308 => &["item.leggingsIron.name"],
        309 => &["item.bootsIron.name"],
        310 => &["item.helmetDiamond.name"],
        311 => &["item.chestplateDiamond.name"],
        312 => &["item.leggingsDiamond.name"],
        313 => &["item.bootsDiamond.name"],
        314 => &["item.helmetGold.name"],
        315 => &["item.chestplateGold.name"],
        316 => &["item.leggingsGold.name"],
        317 => &["item.bootsGold.name"],
        318 => &["item.flint.name"],
        319 => &["item.porkchopRaw.name"],
        320 => &["item.porkchopCooked.name"],
        321 => &["item.painting.name"],
        322 => &["item.appleGold.name"],
        323 => &["item.sign.name"],
        324 => &["item.doorOak.name", "item.doorWood.name"],
        325 => &["item.bucket.name"],
        326 => &["item.bucketWater.name"],
        327 => &["item.bucketLava.name"],
        328 => &["item.minecart.name"],
        329 => &["item.saddle.name"],
        330 => &["item.doorIron.name"],
        331 => &["item.redstone.name"],
        332 => &["item.snowball.name"],
        333 => &["item.boat.name"],
        334 => &["item.leather.name"],
        335 => &["item.milk.name"],
        336 => &["item.brick.name"],
        337 => &["item.clay.name"],
        338 => &["item.reeds.name"],
        339 => &["item.paper.name"],
        340 => &["item.book.name"],
        341 => &["item.slimeball.name"],
        342 => &["item.minecartChest.name"],
        343 => &["item.minecartFurnace.name"],
        344 => &["item.egg.name"],
        345 => &["item.compass.name"],
        346 => &["item.fishingRod.name"],
        347 => &["item.clock.name"],
        348 => &["item.yellowDust.name"],
        349 => match damage {
            1 => &["item.fish.salmon.raw.name"],
            2 => &["item.fish.clownfish.raw.name"],
            3 => &["item.fish.pufferfish.raw.name"],
            _ => &["item.fish.cod.raw.name", "item.fish.name"],
        },
        350 => match damage {
            1 => &["item.fish.salmon.cooked.name"],
            _ => &["item.fish.cod.cooked.name", "item.fishCooked.name"],
        },
        351 => dye_variant_keys(damage, "item.dyePowder"),
        352 => &["item.bone.name"],
        353 => &["item.sugar.name"],
        354 => &["item.cake.name"],
        355 => &["item.bed.name"],
        356 => &["item.diode.name"],
        357 => &["item.cookie.name"],
        358 => &["item.map.name"],
        359 => &["item.shears.name"],
        360 => &["item.melon.name"],
        361 => &["item.seeds_pumpkin.name"],
        362 => &["item.seeds_melon.name"],
        363 => &["item.beefRaw.name"],
        364 => &["item.beefCooked.name"],
        365 => &["item.chickenRaw.name"],
        366 => &["item.chickenCooked.name"],
        367 => &["item.rottenFlesh.name"],
        368 => &["item.enderPearl.name"],
        369 => &["item.blazeRod.name"],
        370 => &["item.ghastTear.name"],
        371 => &["item.goldNugget.name"],
        372 => &["item.netherStalkSeeds.name"],
        373 => match damage {
            0 => &["item.emptyPotion.name"],
            _ => &["item.potion.name"],
        },
        374 => &["item.glassBottle.name"],
        375 => &["item.spiderEye.name"],
        376 => &["item.fermentedSpiderEye.name"],
        377 => &["item.blazePowder.name"],
        378 => &["item.magmaCream.name"],
        379 => &["item.brewingStand.name"],
        380 => &["item.cauldron.name"],
        381 => &["item.eyeOfEnder.name"],
        382 => &["item.speckledMelon.name"],
        383 => &["item.monsterPlacer.name"],
        384 => &["item.expBottle.name"],
        385 => &["item.fireball.name"],
        386 => &["item.writingBook.name"],
        387 => &["item.writtenBook.name"],
        388 => &["item.emerald.name"],
        389 => &["item.frame.name"],
        390 => &["item.flowerPot.name"],
        391 => &["item.carrots.name"],
        392 => &["item.potato.name"],
        393 => &["item.potatoBaked.name"],
        394 => &["item.potatoPoisonous.name"],
        395 => &["item.emptyMap.name"],
        396 => &["item.carrotGolden.name"],
        397 => skull_keys(damage),
        398 => &["item.carrotOnAStick.name"],
        399 => &["item.netherStar.name"],
        400 => &["item.pumpkinPie.name"],
        401 => &["item.fireworks.name"],
        402 => &["item.fireworksCharge.name"],
        403 => &["item.enchantedBook.name"],
        404 => &["item.comparator.name"],
        405 => &["item.netherbrick.name"],
        406 => &["item.netherquartz.name"],
        407 => &["item.minecartTnt.name"],
        408 => &["item.minecartHopper.name"],
        409 => &["item.prismarineShard.name"],
        410 => &["item.prismarineCrystals.name"],
        411 => &["item.rabbitRaw.name"],
        412 => &["item.rabbitCooked.name"],
        413 => &["item.rabbitStew.name"],
        414 => &["item.rabbitFoot.name"],
        415 => &["item.rabbitHide.name"],
        416 => &["item.armorStand.name"],
        417 => &["item.horsearmormetal.name"],
        418 => &["item.horsearmorgold.name"],
        419 => &["item.horsearmordiamond.name"],
        420 => &["item.leash.name"],
        421 => &["item.nameTag.name"],
        422 => &["item.minecartCommandBlock.name"],
        423 => &["item.muttonRaw.name"],
        424 => &["item.muttonCooked.name"],
        425 => banner_variant_keys(damage),
        427 => &["item.doorSpruce.name"],
        428 => &["item.doorBirch.name"],
        429 => &["item.doorJungle.name"],
        430 => &["item.doorAcacia.name"],
        431 => &["item.doorDarkOak.name"],
        2256 => &["item.record.name", "item.record.13.desc"],
        2257 => &["item.record.name", "item.record.cat.desc"],
        2258 => &["item.record.name", "item.record.blocks.desc"],
        2259 => &["item.record.name", "item.record.chirp.desc"],
        2260 => &["item.record.name", "item.record.far.desc"],
        2261 => &["item.record.name", "item.record.mall.desc"],
        2262 => &["item.record.name", "item.record.mellohi.desc"],
        2263 => &["item.record.name", "item.record.stal.desc"],
        2264 => &["item.record.name", "item.record.strad.desc"],
        2265 => &["item.record.name", "item.record.ward.desc"],
        2266 => &["item.record.name", "item.record.11.desc"],
        2267 => &["item.record.name", "item.record.wait.desc"],
        _ => &[],
    }
}

fn wood_variant_keys(damage: u16, prefix: &str) -> &'static [&'static str] {
    match (prefix, damage & 7) {
        ("tile.wood", 1) => &["tile.wood.spruce.name"],
        ("tile.wood", 2) => &["tile.wood.birch.name"],
        ("tile.wood", 3) => &["tile.wood.jungle.name"],
        ("tile.wood", 4) => &["tile.wood.acacia.name"],
        ("tile.wood", 5) => &["tile.wood.big_oak.name"],
        ("tile.wood", _) => &["tile.wood.oak.name", "tile.wood.name"],
        ("tile.sapling", 1) => &["tile.sapling.spruce.name"],
        ("tile.sapling", 2) => &["tile.sapling.birch.name"],
        ("tile.sapling", 3) => &["tile.sapling.jungle.name"],
        ("tile.sapling", 4) => &["tile.sapling.acacia.name"],
        ("tile.sapling", 5) => &["tile.sapling.big_oak.name"],
        ("tile.sapling", _) => &["tile.sapling.oak.name"],
        ("tile.log", 1) => &["tile.log.spruce.name"],
        ("tile.log", 2) => &["tile.log.birch.name"],
        ("tile.log", 3) => &["tile.log.jungle.name"],
        ("tile.log", 4) => &["tile.log.acacia.name"],
        ("tile.log", 5) => &["tile.log.big_oak.name"],
        ("tile.log", _) => &["tile.log.oak.name", "tile.log.name"],
        ("tile.leaves", 1) => &["tile.leaves.spruce.name"],
        ("tile.leaves", 2) => &["tile.leaves.birch.name"],
        ("tile.leaves", 3) => &["tile.leaves.jungle.name"],
        ("tile.leaves", 4) => &["tile.leaves.acacia.name"],
        ("tile.leaves", 5) => &["tile.leaves.big_oak.name"],
        ("tile.leaves", _) => &["tile.leaves.oak.name", "tile.leaves.name"],
        ("tile.woodSlab", 1) => &["tile.woodSlab.spruce.name"],
        ("tile.woodSlab", 2) => &["tile.woodSlab.birch.name"],
        ("tile.woodSlab", 3) => &["tile.woodSlab.jungle.name"],
        ("tile.woodSlab", 4) => &["tile.woodSlab.acacia.name"],
        ("tile.woodSlab", 5) => &["tile.woodSlab.big_oak.name"],
        ("tile.woodSlab", _) => &["tile.woodSlab.oak.name", "tile.woodSlab.name"],
        _ => &[],
    }
}

fn wood_fence_keys(damage: u16, prefix: &str) -> &'static [&'static str] {
    match (prefix, damage & 7) {
        ("tile.fence", 1) => &["tile.spruceFence.name"],
        ("tile.fence", 2) => &["tile.birchFence.name"],
        ("tile.fence", 3) => &["tile.jungleFence.name"],
        ("tile.fence", 4) => &["tile.acaciaFence.name"],
        ("tile.fence", 5) => &["tile.darkOakFence.name"],
        ("tile.fence", _) => &["tile.fence.name"],
        ("tile.fenceGate", 1) => &["tile.spruceFenceGate.name"],
        ("tile.fenceGate", 2) => &["tile.birchFenceGate.name"],
        ("tile.fenceGate", 3) => &["tile.jungleFenceGate.name"],
        ("tile.fenceGate", 4) => &["tile.acaciaFenceGate.name"],
        ("tile.fenceGate", 5) => &["tile.darkOakFenceGate.name"],
        ("tile.fenceGate", _) => &["tile.fenceGate.name"],
        _ => &[],
    }
}

fn dye_variant_keys(damage: u16, prefix: &str) -> &'static [&'static str] {
    match (prefix, damage & 15) {
        ("tile.cloth", 0) => &["tile.cloth.white.name", "tile.cloth.name"],
        ("tile.cloth", 1) => &["tile.cloth.orange.name"],
        ("tile.cloth", 2) => &["tile.cloth.magenta.name"],
        ("tile.cloth", 3) => &["tile.cloth.lightBlue.name"],
        ("tile.cloth", 4) => &["tile.cloth.yellow.name"],
        ("tile.cloth", 5) => &["tile.cloth.lime.name"],
        ("tile.cloth", 6) => &["tile.cloth.pink.name"],
        ("tile.cloth", 7) => &["tile.cloth.gray.name"],
        ("tile.cloth", 8) => &["tile.cloth.silver.name"],
        ("tile.cloth", 9) => &["tile.cloth.cyan.name"],
        ("tile.cloth", 10) => &["tile.cloth.purple.name"],
        ("tile.cloth", 11) => &["tile.cloth.blue.name"],
        ("tile.cloth", 12) => &["tile.cloth.brown.name"],
        ("tile.cloth", 13) => &["tile.cloth.green.name"],
        ("tile.cloth", 14) => &["tile.cloth.red.name"],
        ("tile.cloth", _) => &["tile.cloth.black.name"],
        ("item.dyePowder", 0) => &["item.dyePowder.black.name"],
        ("item.dyePowder", 1) => &["item.dyePowder.red.name"],
        ("item.dyePowder", 2) => &["item.dyePowder.green.name"],
        ("item.dyePowder", 3) => &["item.dyePowder.brown.name"],
        ("item.dyePowder", 4) => &["item.dyePowder.blue.name"],
        ("item.dyePowder", 5) => &["item.dyePowder.purple.name"],
        ("item.dyePowder", 6) => &["item.dyePowder.cyan.name"],
        ("item.dyePowder", 7) => &["item.dyePowder.silver.name"],
        ("item.dyePowder", 8) => &["item.dyePowder.gray.name"],
        ("item.dyePowder", 9) => &["item.dyePowder.pink.name"],
        ("item.dyePowder", 10) => &["item.dyePowder.lime.name"],
        ("item.dyePowder", 11) => &["item.dyePowder.yellow.name"],
        ("item.dyePowder", 12) => &["item.dyePowder.lightBlue.name"],
        ("item.dyePowder", 13) => &["item.dyePowder.magenta.name"],
        ("item.dyePowder", 14) => &["item.dyePowder.orange.name"],
        ("item.dyePowder", _) => &["item.dyePowder.white.name"],
        ("tile.clayHardenedStained", 0) => &["tile.clayHardenedStained.white.name"],
        ("tile.clayHardenedStained", 1) => &["tile.clayHardenedStained.orange.name"],
        ("tile.clayHardenedStained", 2) => &["tile.clayHardenedStained.magenta.name"],
        ("tile.clayHardenedStained", 3) => &["tile.clayHardenedStained.lightBlue.name"],
        ("tile.clayHardenedStained", 4) => &["tile.clayHardenedStained.yellow.name"],
        ("tile.clayHardenedStained", 5) => &["tile.clayHardenedStained.lime.name"],
        ("tile.clayHardenedStained", 6) => &["tile.clayHardenedStained.pink.name"],
        ("tile.clayHardenedStained", 7) => &["tile.clayHardenedStained.gray.name"],
        ("tile.clayHardenedStained", 8) => &["tile.clayHardenedStained.silver.name"],
        ("tile.clayHardenedStained", 9) => &["tile.clayHardenedStained.cyan.name"],
        ("tile.clayHardenedStained", 10) => &["tile.clayHardenedStained.purple.name"],
        ("tile.clayHardenedStained", 11) => &["tile.clayHardenedStained.blue.name"],
        ("tile.clayHardenedStained", 12) => &["tile.clayHardenedStained.brown.name"],
        ("tile.clayHardenedStained", 13) => &["tile.clayHardenedStained.green.name"],
        ("tile.clayHardenedStained", 14) => &["tile.clayHardenedStained.red.name"],
        ("tile.clayHardenedStained", _) => &["tile.clayHardenedStained.black.name"],
        ("tile.stainedGlass", 0) => &["tile.stainedGlass.white.name"],
        ("tile.stainedGlass", 1) => &["tile.stainedGlass.orange.name"],
        ("tile.stainedGlass", 2) => &["tile.stainedGlass.magenta.name"],
        ("tile.stainedGlass", 3) => &["tile.stainedGlass.lightBlue.name"],
        ("tile.stainedGlass", 4) => &["tile.stainedGlass.yellow.name"],
        ("tile.stainedGlass", 5) => &["tile.stainedGlass.lime.name"],
        ("tile.stainedGlass", 6) => &["tile.stainedGlass.pink.name"],
        ("tile.stainedGlass", 7) => &["tile.stainedGlass.gray.name"],
        ("tile.stainedGlass", 8) => &["tile.stainedGlass.silver.name"],
        ("tile.stainedGlass", 9) => &["tile.stainedGlass.cyan.name"],
        ("tile.stainedGlass", 10) => &["tile.stainedGlass.purple.name"],
        ("tile.stainedGlass", 11) => &["tile.stainedGlass.blue.name"],
        ("tile.stainedGlass", 12) => &["tile.stainedGlass.brown.name"],
        ("tile.stainedGlass", 13) => &["tile.stainedGlass.green.name"],
        ("tile.stainedGlass", 14) => &["tile.stainedGlass.red.name"],
        ("tile.stainedGlass", _) => &["tile.stainedGlass.black.name"],
        ("tile.thinStainedGlass", 0) => &["tile.thinStainedGlass.white.name"],
        ("tile.thinStainedGlass", 1) => &["tile.thinStainedGlass.orange.name"],
        ("tile.thinStainedGlass", 2) => &["tile.thinStainedGlass.magenta.name"],
        ("tile.thinStainedGlass", 3) => &["tile.thinStainedGlass.lightBlue.name"],
        ("tile.thinStainedGlass", 4) => &["tile.thinStainedGlass.yellow.name"],
        ("tile.thinStainedGlass", 5) => &["tile.thinStainedGlass.lime.name"],
        ("tile.thinStainedGlass", 6) => &["tile.thinStainedGlass.pink.name"],
        ("tile.thinStainedGlass", 7) => &["tile.thinStainedGlass.gray.name"],
        ("tile.thinStainedGlass", 8) => &["tile.thinStainedGlass.silver.name"],
        ("tile.thinStainedGlass", 9) => &["tile.thinStainedGlass.cyan.name"],
        ("tile.thinStainedGlass", 10) => &["tile.thinStainedGlass.purple.name"],
        ("tile.thinStainedGlass", 11) => &["tile.thinStainedGlass.blue.name"],
        ("tile.thinStainedGlass", 12) => &["tile.thinStainedGlass.brown.name"],
        ("tile.thinStainedGlass", 13) => &["tile.thinStainedGlass.green.name"],
        ("tile.thinStainedGlass", 14) => &["tile.thinStainedGlass.red.name"],
        ("tile.thinStainedGlass", _) => &["tile.thinStainedGlass.black.name"],
        ("tile.woolCarpet", 0) => &["tile.woolCarpet.white.name", "tile.woolCarpet.name"],
        ("tile.woolCarpet", 1) => &["tile.woolCarpet.orange.name"],
        ("tile.woolCarpet", 2) => &["tile.woolCarpet.magenta.name"],
        ("tile.woolCarpet", 3) => &["tile.woolCarpet.lightBlue.name"],
        ("tile.woolCarpet", 4) => &["tile.woolCarpet.yellow.name"],
        ("tile.woolCarpet", 5) => &["tile.woolCarpet.lime.name"],
        ("tile.woolCarpet", 6) => &["tile.woolCarpet.pink.name"],
        ("tile.woolCarpet", 7) => &["tile.woolCarpet.gray.name"],
        ("tile.woolCarpet", 8) => &["tile.woolCarpet.silver.name"],
        ("tile.woolCarpet", 9) => &["tile.woolCarpet.cyan.name"],
        ("tile.woolCarpet", 10) => &["tile.woolCarpet.purple.name"],
        ("tile.woolCarpet", 11) => &["tile.woolCarpet.blue.name"],
        ("tile.woolCarpet", 12) => &["tile.woolCarpet.brown.name"],
        ("tile.woolCarpet", 13) => &["tile.woolCarpet.green.name"],
        ("tile.woolCarpet", 14) => &["tile.woolCarpet.red.name"],
        ("tile.woolCarpet", _) => &["tile.woolCarpet.black.name"],
        _ => &[],
    }
}

fn banner_variant_keys(damage: u16) -> &'static [&'static str] {
    match damage & 15 {
        0 => &["item.banner.black.name"],
        1 => &["item.banner.red.name"],
        2 => &["item.banner.green.name"],
        3 => &["item.banner.brown.name"],
        4 => &["item.banner.blue.name"],
        5 => &["item.banner.purple.name"],
        6 => &["item.banner.cyan.name"],
        7 => &["item.banner.silver.name"],
        8 => &["item.banner.gray.name"],
        9 => &["item.banner.pink.name"],
        10 => &["item.banner.lime.name"],
        11 => &["item.banner.yellow.name"],
        12 => &["item.banner.lightBlue.name"],
        13 => &["item.banner.magenta.name"],
        14 => &["item.banner.orange.name"],
        _ => &["item.banner.white.name"],
    }
}

fn skull_keys(damage: u16) -> &'static [&'static str] {
    match damage {
        0 => &["item.skull.skeleton.name"],
        1 => &["item.skull.wither.name"],
        2 => &["item.skull.zombie.name"],
        3 => &["item.skull.player.name", "item.skull.char.name"],
        4 => &["item.skull.creeper.name"],
        _ => &["item.skull.char.name"],
    }
}

fn flower_variant_keys(damage: u16) -> &'static [&'static str] {
    match damage {
        1 => &["tile.flower2.blueOrchid.name"],
        2 => &["tile.flower2.allium.name"],
        3 => &["tile.flower2.houstonia.name"],
        4 => &["tile.flower2.tulipRed.name"],
        5 => &["tile.flower2.tulipOrange.name"],
        6 => &["tile.flower2.tulipWhite.name"],
        7 => &["tile.flower2.tulipPink.name"],
        8 => &["tile.flower2.oxeyeDaisy.name"],
        _ => &["tile.flower2.poppy.name", "tile.flower2.name"],
    }
}

fn stone_slab_keys(damage: u16) -> &'static [&'static str] {
    match damage & 7 {
        1 => &["tile.stoneSlab.sand.name"],
        2 => &["tile.stoneSlab.wood.name"],
        3 => &["tile.stoneSlab.cobble.name"],
        4 => &["tile.stoneSlab.brick.name"],
        5 => &["tile.stoneSlab.smoothStoneBrick.name"],
        6 => &["tile.stoneSlab.netherBrick.name"],
        7 => &["tile.stoneSlab.quartz.name"],
        _ => &["tile.stoneSlab.stone.name", "tile.stoneSlab.name"],
    }
}

fn monster_egg_keys(damage: u16) -> &'static [&'static str] {
    match damage {
        1 => &["tile.monsterStoneEgg.cobble.name"],
        2 => &["tile.monsterStoneEgg.brick.name"],
        3 => &["tile.monsterStoneEgg.mossybrick.name"],
        4 => &["tile.monsterStoneEgg.crackedbrick.name"],
        5 => &["tile.monsterStoneEgg.chiseledbrick.name"],
        _ => &[
            "tile.monsterStoneEgg.stone.name",
            "tile.monsterStoneEgg.name",
        ],
    }
}

fn stone_brick_keys(damage: u16) -> &'static [&'static str] {
    match damage {
        1 => &["tile.stonebricksmooth.mossy.name"],
        2 => &["tile.stonebricksmooth.cracked.name"],
        3 => &["tile.stonebricksmooth.chiseled.name"],
        _ => &[
            "tile.stonebricksmooth.default.name",
            "tile.stonebricksmooth.name",
        ],
    }
}

fn double_plant_keys(damage: u16) -> &'static [&'static str] {
    match damage {
        1 => &["tile.doublePlant.syringa.name"],
        2 => &["tile.doublePlant.grass.name"],
        3 => &["tile.doublePlant.fern.name"],
        4 => &["tile.doublePlant.rose.name"],
        5 => &["tile.doublePlant.paeonia.name"],
        _ => &["tile.doublePlant.sunflower.name", "tile.doublePlant.name"],
    }
}

fn block_id_translation_keys(id: &str) -> &'static [&'static str] {
    match id {
        "stone" => &["tile.stone.stone.name", "tile.stone.name"],
        "grass" => &["tile.grass.name"],
        "dirt" => &["tile.dirt.name"],
        "cobblestone" => &["tile.stonebrick.name"],
        "planks" => &["tile.wood.name"],
        "sand" => &["tile.sand.name"],
        "log" => &["tile.log.name"],
        "leaves" => &["tile.leaves.name"],
        "glass" => &["tile.glass.name"],
        "wool" => &["tile.cloth.name"],
        "gold_block" => &["tile.blockGold.name"],
        "iron_block" => &["tile.blockIron.name"],
        "brick_block" => &["tile.brick.name"],
        "bookshelf" => &["tile.bookshelf.name"],
        "mossy_cobblestone" => &["tile.stoneMoss.name"],
        "obsidian" => &["tile.obsidian.name"],
        "torch" => &["tile.torch.name"],
        "chest" => &["tile.chest.name"],
        "crafting_table" => &["tile.workbench.name"],
        "furnace" | "lit_furnace" => &["tile.furnace.name"],
        "ladder" => &["tile.ladder.name"],
        "rail" => &["tile.rail.name"],
        "lever" => &["tile.lever.name"],
        "snow" => &["tile.snow.name"],
        "ice" => &["tile.ice.name"],
        "cactus" => &["tile.cactus.name"],
        "clay" => &["tile.clay.name"],
        "fence" => &["tile.fence.name"],
        "pumpkin" => &["tile.pumpkin.name"],
        "netherrack" => &["tile.hellrock.name"],
        "soul_sand" => &["tile.hellsand.name"],
        "glowstone" => &["tile.lightgem.name"],
        "stonebrick" => &["tile.stonebricksmooth.name"],
        "glass_pane" => &["tile.thinGlass.name"],
        "melon_block" => &["tile.melon.name"],
        "vine" => &["tile.vine.name"],
        "nether_brick" => &["tile.netherBrick.name"],
        "enchanting_table" => &["tile.enchantmentTable.name"],
        "brewing_stand" => &["item.brewingStand.name"],
        "cauldron" => &["item.cauldron.name"],
        "end_stone" => &["tile.whiteStone.name"],
        "dragon_egg" => &["tile.dragonEgg.name"],
        "emerald_block" => &["tile.blockEmerald.name"],
        "command_block" => &["tile.commandBlock.name"],
        "beacon" => &["tile.beacon.name"],
        "hopper" => &["tile.hopper.name"],
        "quartz_block" => &["tile.quartzBlock.name"],
        "coal_block" => &["tile.blockCoal.name"],
        "packed_ice" => &["tile.icePacked.name"],
        _ => &[],
    }
}

fn max_damage_for_item(item_id: u16) -> Option<u32> {
    match crate::client::inventory::max_damage(item_id) {
        0 => None,
        max => Some(u32::from(max)),
    }
}

fn format_duration(ticks: i32) -> String {
    if ticks <= 0 {
        return "0:00".to_string();
    }
    let total_seconds = ticks / 20;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{}:{:02}", minutes, seconds)
}

fn format_number(value: f64) -> String {
    if (value - value.round()).abs() < 0.005 {
        format!("{}", value.round() as i64)
    } else {
        format!("{:.2}", value)
    }
}

fn strip_mc_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{00a7}' {
            chars.next();
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_creative_entry_has_translation_candidates() {
        let mut missing = Vec::new();
        let en_us = crate::ui::i18n::I18n::load("assets/minecraft/lang/en_US.lang", None);
        for entry in crate::render::hud::inventory::creative_tab_entries(
            crate::render::hud::inventory::CREATIVE_TAB_SEARCH,
        ) {
            let keys = if entry.item_id < 256 {
                block_translation_keys(entry.item_id, entry.damage)
            } else {
                item_translation_keys(entry.item_id, entry.damage)
            };
            if keys.is_empty() || keys.iter().all(|key| en_us.t(key) == *key) {
                missing.push(format!("{}:{}", entry.item_id, entry.damage));
            }
        }
        assert!(
            missing.is_empty(),
            "missing tooltip mappings: {}",
            missing.join(", ")
        );
    }
}
