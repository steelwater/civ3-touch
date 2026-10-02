use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table, Value};

use crate::action::ActionDef;
use crate::world::World;

/// Register the `Action` Lua global table with `define` and `get` functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let action_table = lua.create_table()?;

    // Initialize the __action_handlers global table
    lua.load("__action_handlers = __action_handlers or {}")
        .exec()?;

    // Action.define({ id, name, hotkey, turns, consumes_unit, valid_conditions, on_complete })
    action_table.set(
        "define",
        lua.create_function(|lua, table: Table| {
            let id: String = table
                .get("id")
                .map_err(|_| mlua::Error::external("Action.define: 'id' field required"))?;
            let name: String = table.get("name").unwrap_or_else(|_| id.clone());
            let hotkey: Option<String> = table.get("hotkey").unwrap_or(None);
            let turns: i32 = table.get("turns").unwrap_or(0);
            let consumes_unit: bool = table.get("consumes_unit").unwrap_or(false);
            let animation_name: Option<String> = table.get("animation_name").unwrap_or(None);
            let icon_atlas_pos: Option<(i32, i32)> =
                if let Ok(icon_table) = table.get::<Table>("icon") {
                    let col: i32 = icon_table.get(1).map_err(|_| {
                        mlua::Error::external("Action.define: 'icon' must be a table {col, row}")
                    })?;
                    let row: i32 = icon_table.get(2).map_err(|_| {
                        mlua::Error::external("Action.define: 'icon' must be a table {col, row}")
                    })?;
                    Some((col, row))
                } else {
                    None
                };

            // Store metadata in ActionRegistry via World
            {
                let world = lua
                    .app_data_ref::<Rc<RefCell<World>>>()
                    .ok_or_else(|| mlua::Error::external("world not initialized"))?
                    .clone();
                let mut world = world.borrow_mut();
                world.action_registry.register(ActionDef {
                    id: id.clone(),
                    name,
                    hotkey,
                    turns,
                    consumes_unit,
                    animation_name,
                    icon_atlas_pos,
                });
            }

            // Store Lua callback functions in __action_handlers[id]
            let handlers: Table = lua.globals().get("__action_handlers")?;
            let handler_entry = lua.create_table()?;

            if let Ok(valid_conditions) = table.get::<mlua::Function>("valid_conditions") {
                handler_entry.set("valid_conditions", valid_conditions)?;
            }
            if let Ok(on_complete) = table.get::<mlua::Function>("on_complete") {
                handler_entry.set("on_complete", on_complete)?;
            }

            handlers.set(id, handler_entry)?;

            Ok(())
        })?,
    )?;

    // Action.get(action_id) -> metadata table or nil
    action_table.set(
        "get",
        lua.create_function(|lua, action_id: String| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            match world.action_registry.get(&action_id) {
                Some(def) => {
                    let result = lua.create_table()?;
                    result.set("id", def.id.as_str())?;
                    result.set("name", def.name.as_str())?;
                    if let Some(ref hk) = def.hotkey {
                        result.set("hotkey", hk.as_str())?;
                    }
                    result.set("turns", def.turns)?;
                    result.set("consumes_unit", def.consumes_unit)?;
                    if let Some(ref anim) = def.animation_name {
                        result.set("animation_name", anim.as_str())?;
                    }
                    if let Some((col, row)) = def.icon_atlas_pos {
                        let icon = lua.create_table()?;
                        icon.set(1, col)?;
                        icon.set(2, row)?;
                        result.set("icon", icon)?;
                    }
                    Ok(Value::Table(result))
                }
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    lua.globals().set("Action", action_table)?;
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
    fn test_action_define_registers_in_registry() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Action.define({
                    id = "build_city",
                    name = "Build City",
                    hotkey = "B",
                    turns = 0,
                    consumes_unit = true,
                    valid_conditions = function(ctx) return ctx end,
                    on_complete = function(ctx) end,
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
        let action = world.action_registry.get("build_city").unwrap();
        assert_eq!(action.name, "Build City");
        assert_eq!(action.hotkey.as_deref(), Some("B"));
        assert_eq!(action.turns, 0);
        assert!(action.consumes_unit);
    }

    #[test]
    fn test_action_define_stores_handlers() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Action.define({
                    id = "test_action",
                    name = "Test",
                    valid_conditions = function(ctx) ctx.blocked = true; return ctx end,
                    on_complete = function(ctx) end,
                })
                "#,
            )
            .exec()
            .unwrap();

        let has_valid: bool = engine
            .lua
            .load(r#"return type(__action_handlers["test_action"].valid_conditions) == "function""#)
            .eval()
            .unwrap();
        assert!(has_valid);

        let has_complete: bool = engine
            .lua
            .load(r#"return type(__action_handlers["test_action"].on_complete) == "function""#)
            .eval()
            .unwrap();
        assert!(has_complete);
    }

    #[test]
    fn test_action_get_returns_metadata() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Action.define({
                    id = "build_road",
                    name = "Build Road",
                    turns = 3,
                    consumes_unit = false,
                })
                "#,
            )
            .exec()
            .unwrap();

        let name: String = engine
            .lua
            .load(r#"return Action.get("build_road").name"#)
            .eval()
            .unwrap();
        assert_eq!(name, "Build Road");

        let turns: i64 = engine
            .lua
            .load(r#"return Action.get("build_road").turns"#)
            .eval()
            .unwrap();
        assert_eq!(turns, 3);
    }

    #[test]
    fn test_action_get_nonexistent_returns_nil() {
        let engine = setup();
        let is_nil: bool = engine
            .lua
            .load(r#"return Action.get("nonexistent") == nil"#)
            .eval()
            .unwrap();
        assert!(is_nil);
    }

    #[test]
    fn test_action_valid_conditions_can_block() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                Action.define({
                    id = "test_block",
                    name = "Test Block",
                    valid_conditions = function(ctx)
                        ctx.blocked = true
                        ctx.reason = "test reason"
                        return ctx
                    end,
                    on_complete = function(ctx) end,
                })
                "#,
            )
            .exec()
            .unwrap();

        let blocked: bool = engine
            .lua
            .load(
                r#"
                local ctx = { blocked = false, reason = "" }
                local result = __action_handlers["test_block"].valid_conditions(ctx)
                return result.blocked
                "#,
            )
            .eval()
            .unwrap();
        assert!(blocked);
    }
}
