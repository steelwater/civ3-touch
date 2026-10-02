use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table, Value};

use crate::civilization::CivDef;
use crate::world::World;

/// Register the `Civilization` Lua global table with `define`, `get`, and `all` functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let civ_table = lua.create_table()?;

    // Civilization.define({ id, name, ruler_name, adjective, noun })
    civ_table.set(
        "define",
        lua.create_function(|lua, table: Table| {
            let id: String = table
                .get("id")
                .map_err(|_| mlua::Error::external("Civilization.define: 'id' field required"))?;
            let name: String = table.get("name").unwrap_or_else(|_| id.clone());
            let ruler_name: String = table
                .get("ruler_name")
                .unwrap_or_else(|_| "Leader".to_string());
            let adjective: String = table.get("adjective").unwrap_or_else(|_| name.clone());
            let noun: String = table
                .get("noun")
                .unwrap_or_else(|_| format!("{adjective}s"));

            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            world.civilization_registry.register(CivDef {
                id,
                name,
                ruler_name,
                adjective,
                noun,
            });

            Ok(())
        })?,
    )?;

    // Civilization.get(civ_id) -> metadata table or nil
    civ_table.set(
        "get",
        lua.create_function(|lua, civ_id: String| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            match world.civilization_registry.get(&civ_id) {
                Some(def) => {
                    let result = lua.create_table()?;
                    result.set("id", def.id.as_str())?;
                    result.set("name", def.name.as_str())?;
                    result.set("ruler_name", def.ruler_name.as_str())?;
                    result.set("adjective", def.adjective.as_str())?;
                    result.set("noun", def.noun.as_str())?;
                    Ok(Value::Table(result))
                }
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // Civilization.all() -> array of civ IDs
    civ_table.set(
        "all",
        lua.create_function(|lua, ()| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let result = lua.create_table()?;
            for (i, civ) in world.civilization_registry.all().iter().enumerate() {
                result.set(i + 1, civ.id.as_str())?;
            }
            Ok(result)
        })?,
    )?;

    lua.globals().set("Civilization", civ_table)?;
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
    fn test_civilization_define_registers_in_registry() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Civilization.define({
                    id = "rome",
                    name = "Roman Empire",
                    ruler_name = "Caesar",
                    adjective = "Roman",
                    noun = "Romans",
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
        let civ = world.civilization_registry.get("rome").unwrap();
        assert_eq!(civ.name, "Roman Empire");
        assert_eq!(civ.ruler_name, "Caesar");
        assert_eq!(civ.adjective, "Roman");
        assert_eq!(civ.noun, "Romans");
    }

    #[test]
    fn test_civilization_get_returns_metadata() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Civilization.define({
                    id = "greece",
                    name = "Greek Empire",
                    ruler_name = "Alexander",
                    adjective = "Greek",
                    noun = "Greeks",
                })
                "#,
            )
            .exec()
            .unwrap();

        let name: String = engine
            .lua
            .load(r#"return Civilization.get("greece").name"#)
            .eval()
            .unwrap();
        assert_eq!(name, "Greek Empire");

        let ruler: String = engine
            .lua
            .load(r#"return Civilization.get("greece").ruler_name"#)
            .eval()
            .unwrap();
        assert_eq!(ruler, "Alexander");
    }

    #[test]
    fn test_civilization_get_nonexistent_returns_nil() {
        let engine = setup();
        let is_nil: bool = engine
            .lua
            .load(r#"return Civilization.get("nonexistent") == nil"#)
            .eval()
            .unwrap();
        assert!(is_nil);
    }

    #[test]
    fn test_civilization_all_returns_ids() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Civilization.define({ id = "rome", name = "Roman Empire", ruler_name = "Caesar", adjective = "Roman", noun = "Romans" })
                Civilization.define({ id = "greece", name = "Greek Empire", ruler_name = "Alexander", adjective = "Greek", noun = "Greeks" })
                "#,
            )
            .exec()
            .unwrap();

        let count: i64 = engine
            .lua
            .load(r#"return #Civilization.all()"#)
            .eval()
            .unwrap();
        assert_eq!(count, 2);

        let first: String = engine
            .lua
            .load(r#"return Civilization.all()[1]"#)
            .eval()
            .unwrap();
        assert_eq!(first, "rome");

        let second: String = engine
            .lua
            .load(r#"return Civilization.all()[2]"#)
            .eval()
            .unwrap();
        assert_eq!(second, "greece");
    }

    #[test]
    fn test_civilization_define_defaults() {
        let engine = setup();
        engine
            .lua
            .load(r#"Civilization.define({ id = "minimal" })"#)
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let civ = world.civilization_registry.get("minimal").unwrap();
        assert_eq!(civ.name, "minimal");
        assert_eq!(civ.ruler_name, "Leader");
        assert_eq!(civ.adjective, "minimal");
        assert_eq!(civ.noun, "minimals");
    }
}
