use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table, Value};

use crate::building::BuildingDef;
use crate::world::World;

/// Register the `Building` Lua global table with `define` and `get` functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let building_table = lua.create_table()?;

    // Building.define({ id, name, cost, maintenance, requires, can_produce, on_complete })
    building_table.set(
        "define",
        lua.create_function(|lua, table: Table| {
            let id: String = table
                .get("id")
                .map_err(|_| mlua::Error::external("Building.define: 'id' field required"))?;
            let name: String = table.get("name").unwrap_or_else(|_| id.clone());
            let cost: i32 = table.get("cost").unwrap_or(60);
            let maintenance: i32 = table.get("maintenance").unwrap_or(0);
            let requires: Vec<String> = table.get("requires").unwrap_or_default();

            // Store callbacks in __building_handlers[id]
            let handlers: Table = lua
                .globals()
                .get::<Table>("__building_handlers")
                .unwrap_or_else(|_| {
                    let t = lua.create_table().unwrap();
                    lua.globals().set("__building_handlers", t.clone()).unwrap();
                    t
                });

            let handler_entry = lua.create_table()?;
            if let Ok(can_produce) = table.get::<mlua::Function>("can_produce") {
                handler_entry.set("can_produce", can_produce)?;
            }
            if let Ok(on_complete) = table.get::<mlua::Function>("on_complete") {
                handler_entry.set("on_complete", on_complete)?;
            }
            handlers.set(id.as_str(), handler_entry)?;

            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            world.building_registry.register(BuildingDef {
                id,
                name,
                cost,
                maintenance,
                requires,
            });

            Ok(())
        })?,
    )?;

    // Building.get(building_id) -> metadata table or nil
    building_table.set(
        "get",
        lua.create_function(|lua, building_id: String| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            match world.building_registry.get(&building_id) {
                Some(def) => {
                    let result = lua.create_table()?;
                    result.set("id", def.id.as_str())?;
                    result.set("name", def.name.as_str())?;
                    result.set("cost", def.cost)?;
                    result.set("maintenance", def.maintenance)?;
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

    // Initialize __building_handlers if not present
    if lua.globals().get::<Value>("__building_handlers")? == Value::Nil {
        lua.globals()
            .set("__building_handlers", lua.create_table()?)?;
    }

    lua.globals().set("Building", building_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::ScriptEngine;
    use crate::world::WorldConfig;

    fn setup() -> ScriptEngine {
        let engine = ScriptEngine::new().unwrap();
        let world = crate::world::World::new(&WorldConfig {
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
    fn test_building_define_registers_in_registry() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Building.define({
                    id = "granary",
                    name = "Granary",
                    cost = 60,
                    maintenance = 1,
                    requires = {"pottery"},
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
        let building = world.building_registry.get("granary").unwrap();
        assert_eq!(building.name, "Granary");
        assert_eq!(building.cost, 60);
        assert_eq!(building.maintenance, 1);
        assert_eq!(building.requires, vec!["pottery".to_string()]);
    }

    #[test]
    fn test_building_define_with_callbacks() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Building.define({
                    id = "palace",
                    name = "Palace",
                    cost = 200,
                    maintenance = 0,
                    requires = {},
                    can_produce = function(ctx) ctx.blocked = true; return ctx end,
                    on_complete = function(ctx) return ctx end,
                })
                "#,
            )
            .exec()
            .unwrap();

        // Verify callbacks stored
        let has_can_produce: bool = engine
            .lua
            .load(r#"return __building_handlers["palace"].can_produce ~= nil"#)
            .eval()
            .unwrap();
        assert!(has_can_produce);

        let has_on_complete: bool = engine
            .lua
            .load(r#"return __building_handlers["palace"].on_complete ~= nil"#)
            .eval()
            .unwrap();
        assert!(has_on_complete);
    }

    #[test]
    fn test_building_get_returns_metadata() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Building.define({
                    id = "granary",
                    name = "Granary",
                    cost = 60,
                    maintenance = 1,
                    requires = {"pottery"},
                })
                "#,
            )
            .exec()
            .unwrap();

        let name: String = engine
            .lua
            .load(r#"return Building.get("granary").name"#)
            .eval()
            .unwrap();
        assert_eq!(name, "Granary");

        let cost: i64 = engine
            .lua
            .load(r#"return Building.get("granary").cost"#)
            .eval()
            .unwrap();
        assert_eq!(cost, 60);

        let maintenance: i64 = engine
            .lua
            .load(r#"return Building.get("granary").maintenance"#)
            .eval()
            .unwrap();
        assert_eq!(maintenance, 1);
    }

    #[test]
    fn test_building_get_nonexistent_returns_nil() {
        let engine = setup();
        let is_nil: bool = engine
            .lua
            .load(r#"return Building.get("nonexistent") == nil"#)
            .eval()
            .unwrap();
        assert!(is_nil);
    }

    #[test]
    fn test_building_define_defaults() {
        let engine = setup();
        engine
            .lua
            .load(r#"Building.define({ id = "minimal" })"#)
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let building = world.building_registry.get("minimal").unwrap();
        assert_eq!(building.name, "minimal");
        assert_eq!(building.cost, 60);
        assert_eq!(building.maintenance, 0);
        assert!(building.requires.is_empty());
    }
}
