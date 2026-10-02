use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Value};
use rand::Rng;

use crate::dynamic::{AttrValue, ColType};
use crate::world::World;

/// Register the `Engine` Lua global table with utility functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let engine_table = lua.create_table()?;

    // Engine.log(msg) — prints a message for mod debugging
    engine_table.set(
        "log",
        lua.create_function(|_, msg: String| {
            println!("[mod] {msg}");
            Ok(())
        })?,
    )?;

    // Engine.random(min, max) — returns a random integer in [min, max] inclusive
    engine_table.set(
        "random",
        lua.create_function(|lua, (min, max): (i64, i64)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            let value = world.rng.gen_range(min..=max);
            world.rng_calls += 1;
            Ok(value)
        })?,
    )?;

    // Engine.register_attribute(store_name, attr_name, type_name, default)
    // store_name: "unit" (future: "city", "tile")
    // type_name: "int", "float", "bool"
    engine_table.set(
        "register_attribute",
        lua.create_function(
            |lua, (store_name, attr_name, type_name, default): (String, String, String, Value)| {
                let world = lua
                    .app_data_ref::<Rc<RefCell<World>>>()
                    .ok_or_else(|| mlua::Error::external("world not initialized"))?
                    .clone();
                let mut world = world.borrow_mut();

                let (col_type, default_val) = match type_name.as_str() {
                    "int" => {
                        let v = match default {
                            Value::Integer(i) => i as i32,
                            _ => 0,
                        };
                        (ColType::Int, AttrValue::Int(v))
                    }
                    "float" => {
                        let v = match default {
                            Value::Number(f) => f as f32,
                            Value::Integer(i) => i as f32,
                            _ => 0.0,
                        };
                        (ColType::Float, AttrValue::Float(v))
                    }
                    "bool" => {
                        let v = match default {
                            Value::Boolean(b) => b,
                            _ => false,
                        };
                        (ColType::Bool, AttrValue::Bool(v))
                    }
                    _ => {
                        return Err(mlua::Error::external(format!(
                            "unknown attribute type: {type_name}"
                        )));
                    }
                };

                let dynamic = match store_name.as_str() {
                    "unit" => &mut world.units.dynamic,
                    _ => {
                        return Err(mlua::Error::external(format!(
                            "unknown store: {store_name}"
                        )));
                    }
                };

                dynamic
                    .register(&attr_name, col_type, default_val)
                    .map_err(|e| {
                        mlua::Error::external(format!("register_attribute failed: {e}"))
                    })?;

                Ok(true)
            },
        )?,
    )?;

    lua.globals().set("Engine", engine_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::api_unit;
    use crate::scripting::ScriptEngine;
    use crate::types::{PlayerId, TileCoord, UnitTypeId};
    use crate::unit_type::UnitType;
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
    fn test_engine_log_does_not_panic() {
        let engine = setup();
        engine
            .lua
            .load(r#"Engine.log("hello from Lua")"#)
            .exec()
            .unwrap();
    }

    #[test]
    fn test_engine_random_in_range() {
        let engine = setup();
        let all_in_range: bool = engine
            .lua
            .load(
                r#"
                for i = 1, 100 do
                    local v = Engine.random(1, 6)
                    if v < 1 or v > 6 then
                        return false
                    end
                end
                return true
                "#,
            )
            .eval()
            .unwrap();
        assert!(all_in_range, "all random values should be in [1, 6]");
    }

    #[test]
    fn test_engine_random_increments_rng_calls() {
        let engine = setup();
        engine
            .lua
            .load("Engine.random(1, 10); Engine.random(1, 10); Engine.random(1, 10)")
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        assert_eq!(world.rng_calls, 3);
    }

    #[test]
    fn test_engine_register_attribute_then_use_via_unit_api() {
        let engine = setup();
        // Also register the Unit API so we can test get/set
        api_unit::register(&engine.lua).unwrap();

        // Spawn a unit first
        {
            let world = engine
                .lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .unwrap()
                .clone();
            let mut world = world.borrow_mut();
            let warrior = UnitType {
                id: UnitTypeId(0),
                name: "warrior".to_string(),
                attack: 1,
                defense: 1,
                movement: 1,
                max_hp: 3,
                cost: 10,
                category: "melee".to_string(),
                traits: vec![],
                actions: vec![],
                replaces: None,
                requires_civ: None,
                art_ini: None,
            };
            let wid = world.unit_types.register(warrior.clone());
            let uid = world
                .units
                .spawn(wid, PlayerId(0), TileCoord { x: 5, y: 5 }, &warrior);
            let uid_lua = api_unit::unit_id_to_lua(uid);
            engine.lua.globals().set("test_uid", uid_lua).unwrap();
        }

        // Register attribute and use it
        engine
            .lua
            .load(
                r#"
                Engine.register_attribute("unit", "cargo", "int", 0)
                "#,
            )
            .exec()
            .unwrap();

        // Read default
        let val: i64 = engine
            .lua
            .load("return Unit.get(test_uid, 'cargo')")
            .eval()
            .unwrap();
        assert_eq!(val, 0);

        // Set and read back
        engine
            .lua
            .load("Unit.set(test_uid, 'cargo', 5)")
            .exec()
            .unwrap();
        let val: i64 = engine
            .lua
            .load("return Unit.get(test_uid, 'cargo')")
            .eval()
            .unwrap();
        assert_eq!(val, 5);
    }
}
