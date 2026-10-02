use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table, Value};

use crate::tech::TechDef;
use crate::world::World;

/// Register the `Tech` Lua global table with `define` and `get` functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let tech_table = lua.create_table()?;

    // Tech.define({ id, name, cost, requires })
    tech_table.set(
        "define",
        lua.create_function(|lua, table: Table| {
            let id: String = table
                .get("id")
                .map_err(|_| mlua::Error::external("Tech.define: 'id' field required"))?;
            let name: String = table.get("name").unwrap_or_else(|_| id.clone());
            let cost: i32 = table.get("cost").unwrap_or(40);
            let requires: Vec<String> = table.get("requires").unwrap_or_default();

            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            world.tech_registry.register(TechDef {
                id,
                name,
                cost,
                requires,
            });

            Ok(())
        })?,
    )?;

    // Tech.get(tech_id) -> metadata table or nil
    tech_table.set(
        "get",
        lua.create_function(|lua, tech_id: String| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            match world.tech_registry.get(&tech_id) {
                Some(def) => {
                    let result = lua.create_table()?;
                    result.set("id", def.id.as_str())?;
                    result.set("name", def.name.as_str())?;
                    result.set("cost", def.cost)?;
                    let requires = lua.create_table()?;
                    for (i, req) in def.requires.iter().enumerate() {
                        requires.set(i + 1, req.as_str())?;
                    }
                    result.set("requires", requires)?;
                    Ok(Value::Table(result))
                }
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // Tech.has_researched(player_id, tech_id) -> bool
    tech_table.set(
        "has_researched",
        lua.create_function(|lua, (player_id, tech_id): (i64, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let has = world
                .players
                .get(player_id as usize)
                .is_some_and(|p| p.researched_techs.contains(&tech_id));
            Ok(has)
        })?,
    )?;

    lua.globals().set("Tech", tech_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::ScriptEngine;
    use crate::world::WorldConfig;

    fn setup() -> ScriptEngine {
        let engine = ScriptEngine::new().unwrap();
        let world = World::new(&WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        });
        let world = Rc::new(RefCell::new(world));
        engine.lua.set_app_data(world);
        register(&engine.lua).unwrap();
        engine
    }

    #[test]
    fn test_tech_define_registers_in_registry() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Tech.define({
                    id = "bronze_working",
                    name = "Bronze Working",
                    cost = 40,
                    requires = {},
                })
                "#,
            )
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let tech = world.tech_registry.get("bronze_working").unwrap();
        assert_eq!(tech.name, "Bronze Working");
        assert_eq!(tech.cost, 40);
        assert!(tech.requires.is_empty());
    }

    #[test]
    fn test_tech_define_with_prerequisites() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Tech.define({ id = "alphabet", name = "Alphabet", cost = 40, requires = {} })
                Tech.define({ id = "writing", name = "Writing", cost = 60, requires = {"alphabet"} })
                "#,
            )
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let writing = world.tech_registry.get("writing").unwrap();
        assert_eq!(writing.requires, vec!["alphabet".to_string()]);
    }

    #[test]
    fn test_tech_get_returns_metadata() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Tech.define({
                    id = "pottery",
                    name = "Pottery",
                    cost = 40,
                    requires = {},
                })
                "#,
            )
            .exec()
            .unwrap();

        let name: String = engine
            .lua
            .load(r#"return Tech.get("pottery").name"#)
            .eval()
            .unwrap();
        assert_eq!(name, "Pottery");

        let cost: i64 = engine
            .lua
            .load(r#"return Tech.get("pottery").cost"#)
            .eval()
            .unwrap();
        assert_eq!(cost, 40);
    }

    #[test]
    fn test_tech_get_nonexistent_returns_nil() {
        let engine = setup();
        let is_nil: bool = engine
            .lua
            .load(r#"return Tech.get("nonexistent") == nil"#)
            .eval()
            .unwrap();
        assert!(is_nil);
    }

    #[test]
    fn test_tech_define_defaults() {
        let engine = setup();
        engine
            .lua
            .load(r#"Tech.define({ id = "minimal" })"#)
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let tech = world.tech_registry.get("minimal").unwrap();
        assert_eq!(tech.name, "minimal");
        assert_eq!(tech.cost, 40);
        assert!(tech.requires.is_empty());
    }

    #[test]
    fn test_base_mod_loads_techs() {
        let engine = ScriptEngine::new().unwrap();
        let world = World::new(&WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        });
        let world = Rc::new(RefCell::new(world));
        engine.lua.set_app_data(world.clone());
        crate::scripting::api_unit_type::register(&engine.lua).unwrap();
        crate::scripting::api_action::register(&engine.lua).unwrap();
        register(&engine.lua).unwrap();
        crate::scripting::api_building::register(&engine.lua).unwrap();
        crate::scripting::api_civilization::register(&engine.lua).unwrap();

        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).expect("base mod should load");

        let world = world.borrow();
        assert!(
            world.tech_registry.get("bronze_working").is_some(),
            "bronze_working should be registered"
        );
        assert!(
            world.tech_registry.get("alphabet").is_some(),
            "alphabet should be registered"
        );
        assert!(
            world.tech_registry.all().len() >= 7,
            "at least 7 techs should be registered"
        );
    }
}
