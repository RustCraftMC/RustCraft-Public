//! Declarative, manager-editable mod configuration API.

use mlua::{Lua, LuaSerdeExt, Table, Value};

use crate::scripting::config::{
    ConfigChoice, ConfigDefinition, ConfigEntryKind, ConfigValue, SharedModConfig,
};

pub(crate) fn install(lua: &Lua, game: &Table, config: SharedModConfig) -> mlua::Result<()> {
    let api = lua.create_table()?;

    let define_config = config.clone();
    api.set(
        "define",
        lua.create_function(move |lua, schema: Table| {
            let definitions = schema
                .sequence_values::<Table>()
                .map(|entry| parse_definition(entry?))
                .collect::<mlua::Result<Vec<_>>>()?;
            define_config
                .borrow_mut()
                .define_all(definitions)
                .map_err(mlua::Error::external)?;
            create_reader(lua, define_config.clone())
        })?,
    )?;
    api.set("get", create_getter(lua, config)?)?;
    game.set("config", api)
}

fn create_reader(lua: &Lua, config: SharedModConfig) -> mlua::Result<Table> {
    let reader = lua.create_table()?;
    reader.set("get", create_getter(lua, config)?)?;
    Ok(reader)
}

fn create_getter(lua: &Lua, config: SharedModConfig) -> mlua::Result<mlua::Function> {
    lua.create_function(move |lua, key: String| {
        let value = config.borrow().value(&key).map_err(mlua::Error::external)?;
        match value {
            ConfigValue::Boolean(v) => lua.to_value(&v),
            ConfigValue::Number(v) => lua.to_value(&v),
            ConfigValue::Choice(v) => lua.to_value(&v),
        }
    })
}

fn parse_definition(entry: Table) -> mlua::Result<ConfigDefinition> {
    let key = entry.get::<String>("key")?;
    let label = entry
        .get::<Option<String>>("label")?
        .unwrap_or_else(|| key.clone());
    let description = entry
        .get::<Option<String>>("description")?
        .unwrap_or_default();
    let entry_type = entry.get::<String>("type")?;

    let (kind, default_value) = match entry_type.as_str() {
        "boolean" => (
            ConfigEntryKind::Boolean,
            ConfigValue::Boolean(entry.get::<bool>("default")?),
        ),
        "number" => (
            ConfigEntryKind::Number {
                min: entry.get::<f64>("min")?,
                max: entry.get::<f64>("max")?,
                step: entry.get::<Option<f64>>("step")?.unwrap_or(1.0),
            },
            ConfigValue::Number(entry.get::<f64>("default")?),
        ),
        "choice" => {
            let options = entry
                .get::<Table>("options")?
                .sequence_values::<Value>()
                .map(|option| parse_choice(option?))
                .collect::<mlua::Result<Vec<_>>>()?;
            (
                ConfigEntryKind::Choice { options },
                ConfigValue::Choice(entry.get::<String>("default")?),
            )
        }
        _ => {
            return Err(mlua::Error::RuntimeError(format!(
                "config entry '{key}' has unsupported type '{entry_type}'"
            )))
        }
    };

    Ok(ConfigDefinition {
        key,
        label,
        description,
        kind,
        default_value,
    })
}

fn parse_choice(value: Value) -> mlua::Result<ConfigChoice> {
    match value {
        Value::String(value) => {
            let value = value.to_str()?.to_owned();
            Ok(ConfigChoice {
                label: value.clone(),
                value,
            })
        }
        Value::Table(option) => {
            let value = option.get::<String>("value")?;
            let label = option
                .get::<Option<String>>("label")?
                .unwrap_or_else(|| value.clone());
            Ok(ConfigChoice { value, label })
        }
        _ => Err(mlua::Error::RuntimeError(
            "choice options must be strings or { value, label } tables".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::scripting::config::{ConfigValue, ModConfig};

    use super::*;

    #[test]
    fn lua_reads_live_values_after_native_update() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustcraft-config-api-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();

        let config = Rc::new(RefCell::new(ModConfig::load(&root).unwrap()));
        let lua = Lua::new();
        let game = lua.create_table().unwrap();
        install(&lua, &game, config.clone()).unwrap();
        lua.globals().set("game", game).unwrap();
        lua.load(
            r#"
                settings = game.config.define({
                    { key = "enabled", type = "boolean", default = true },
                    { key = "strength", type = "number", default = 1, min = 0, max = 2, step = 0.1 },
                    { key = "indicator", type = "choice", default = "always", options = {
                        { value = "always", label = "Always" }, "never"
                    } }
                })
                assert(settings.get("enabled") == true)
                assert(settings.get("strength") == 1)
                assert(settings.get("indicator") == "always")
            "#,
        )
        .exec()
        .unwrap();

        config
            .borrow_mut()
            .set_value("enabled", ConfigValue::Boolean(false))
            .unwrap();
        lua.load("assert(settings.get('enabled') == false)")
            .exec()
            .unwrap();

        drop(lua);
        let _ = fs::remove_dir_all(root);
    }
}
