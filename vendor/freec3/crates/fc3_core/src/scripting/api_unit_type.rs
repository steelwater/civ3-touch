use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table, Value};

use crate::scripting::api_unit;
use crate::types::UnitTypeId;
use crate::unit_type::UnitType;
use crate::world::World;

/// Register the `UnitType` Lua global table with its API functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let ut_table = lua.create_table()?;

    // Initialize the __unit_type_handlers global table
    lua.load("__unit_type_handlers = __unit_type_handlers or {}")
        .exec()?;

    ut_table.set(
        "define",
        lua.create_function(|lua, table: Table| {
            let name: String = table
                .get("name")
                .or_else(|_| table.get::<String>("id"))
                .map_err(|_| {
                    mlua::Error::external("UnitType.define: 'name' or 'id' field required")
                })?;
            let attack: i32 = table.get("attack").unwrap_or(0);
            let defense: i32 = table.get("defense").unwrap_or(0);
            let movement: i32 = table.get("movement").unwrap_or(1);
            let max_hp: i32 = table.get("max_hp").unwrap_or(1);
            let cost: i32 = table.get("cost").unwrap_or(0);
            let category: String = table.get("category").unwrap_or_default();

            // Read traits array
            let traits: Vec<String> = match table.get::<Value>("traits") {
                Ok(Value::Table(t)) => {
                    let mut v = Vec::new();
                    for pair in t.sequence_values::<String>() {
                        v.push(pair?);
                    }
                    v
                }
                _ => Vec::new(),
            };

            // Read actions array
            let actions: Vec<String> = match table.get::<Value>("actions") {
                Ok(Value::Table(t)) => {
                    let mut v = Vec::new();
                    for pair in t.sequence_values::<String>() {
                        v.push(pair?);
                    }
                    v
                }
                _ => Vec::new(),
            };

            // Optional fields
            let replaces: Option<UnitTypeId> = table
                .get::<Option<u16>>("replaces")
                .ok()
                .flatten()
                .map(UnitTypeId);
            let requires_civ: Option<String> = table.get("requires_civ").unwrap_or(None);
            let art_ini: Option<String> = table.get("art_ini").unwrap_or(None);

            // Store can_produce callback in __unit_type_handlers[name]
            {
                let handlers: Table = lua.globals().get("__unit_type_handlers")?;
                let handler_entry = lua.create_table()?;
                if let Ok(can_produce) = table.get::<mlua::Function>("can_produce") {
                    handler_entry.set("can_produce", can_produce)?;
                }
                handlers.set(name.as_str(), handler_entry)?;
            }

            // Register the unit type in the world
            let assigned_id = {
                let world = lua
                    .app_data_ref::<Rc<RefCell<World>>>()
                    .ok_or_else(|| mlua::Error::external("world not initialized"))?
                    .clone();
                let mut world = world.borrow_mut();

                let ut = UnitType {
                    id: UnitTypeId(0), // will be overwritten by register()
                    name,
                    attack,
                    defense,
                    movement,
                    max_hp,
                    cost,
                    category,
                    traits,
                    actions,
                    replaces,
                    requires_civ,
                    art_ini,
                };

                world.unit_types.register(ut)
            };
            Ok(assigned_id.0 as i64)
        })?,
    )?;

    // UnitType.get_traits(unit_id_int) -> array of trait strings
    // Takes a *unit* ID (not a unit type ID), looks up the unit's type
    // and returns its traits.
    ut_table.set(
        "get_traits",
        lua.create_function(|lua, uid_int: i64| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let uid = api_unit::lua_to_unit_id(uid_int);
            let idx = match world.units.get(uid) {
                Some(i) => i,
                None => return Ok(Value::Nil),
            };
            let type_id = world.units.unit_type[idx];
            let ut = match world.unit_types.get(type_id) {
                Some(t) => t,
                None => return Ok(Value::Nil),
            };
            let result = lua.create_table()?;
            for (i, trait_name) in ut.traits.iter().enumerate() {
                result.set(i + 1, trait_name.as_str())?;
            }
            Ok(Value::Table(result))
        })?,
    )?;

    // UnitType.get(unit_type_name, field) -> value | nil
    // Looks up a unit type by name and returns the requested field.
    ut_table.set(
        "get",
        lua.create_function(|lua, (name, field): (String, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let ut = match world.unit_types.get_by_name(&name) {
                Some(t) => t,
                None => return Ok(Value::Nil),
            };
            let val: Value = match field.as_str() {
                "name" => Value::String(lua.create_string(&ut.name)?),
                "attack" => Value::Integer(ut.attack as i64),
                "defense" => Value::Integer(ut.defense as i64),
                "movement" => Value::Integer(ut.movement as i64),
                "max_hp" => Value::Integer(ut.max_hp as i64),
                "cost" => Value::Integer(ut.cost as i64),
                "category" => Value::String(lua.create_string(&ut.category)?),
                "art_ini" => match &ut.art_ini {
                    Some(s) => Value::String(lua.create_string(s)?),
                    None => Value::Nil,
                },
                _ => Value::Nil,
            };
            Ok(val)
        })?,
    )?;

    lua.globals().set("UnitType", ut_table)?;
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
        crate::scripting::api_action::register(&engine.lua).unwrap();
        crate::scripting::api_tech::register(&engine.lua).unwrap();
        crate::scripting::api_building::register(&engine.lua).unwrap();
        crate::scripting::api_civilization::register(&engine.lua).unwrap();
        engine
    }

    #[test]
    fn test_define_warrior_from_lua() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                UnitType.define({
                    name = "warrior",
                    attack = 1,
                    defense = 1,
                    movement = 1,
                    max_hp = 3,
                    cost = 10,
                    category = "melee",
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
        let ut = world.unit_types.get_by_name("warrior").unwrap();
        assert_eq!(ut.attack, 1);
        assert_eq!(ut.defense, 1);
        assert_eq!(ut.movement, 1);
        assert_eq!(ut.max_hp, 3);
        assert_eq!(ut.cost, 10);
        assert_eq!(ut.category, "melee");
    }

    #[test]
    fn test_define_two_types() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                UnitType.define({ name = "warrior", attack = 1, defense = 1, movement = 1, max_hp = 3, cost = 10, category = "melee" })
                UnitType.define({ name = "settler", attack = 0, defense = 0, movement = 1, max_hp = 1, cost = 30, category = "civilian" })
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
        assert!(world.unit_types.get_by_name("warrior").is_some());
        assert!(world.unit_types.get_by_name("settler").is_some());
    }

    #[test]
    fn test_define_type_with_traits() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                UnitType.define({
                    name = "settler",
                    attack = 0,
                    defense = 0,
                    movement = 1,
                    max_hp = 1,
                    cost = 30,
                    category = "civilian",
                    traits = {"found_city", "no_combat"},
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
        let ut = world.unit_types.get_by_name("settler").unwrap();
        assert_eq!(ut.traits, vec!["found_city", "no_combat"]);
    }

    #[test]
    fn test_define_returns_id() {
        let engine = setup();
        let id: i64 = engine
            .lua
            .load(
                r#"
                return UnitType.define({ name = "warrior", category = "melee" })
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(id, 0); // first type registered gets id 0
    }

    #[test]
    fn test_get_traits_warrior_empty() {
        let engine = setup();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).unwrap();

        // Spawn a warrior
        let uid_lua = {
            let world = engine
                .lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .unwrap()
                .clone();
            let mut world = world.borrow_mut();
            let wid = world.unit_types.get_by_name("warrior").unwrap().id;
            let template = world.unit_types.get(wid).unwrap().clone();
            let uid = world.units.spawn(
                wid,
                crate::types::PlayerId(0),
                crate::types::TileCoord { x: 0, y: 0 },
                &template,
            );
            api_unit::unit_id_to_lua(uid)
        };

        engine.lua.globals().set("test_uid", uid_lua).unwrap();
        let count: i64 = engine
            .lua
            .load("return #UnitType.get_traits(test_uid)")
            .eval()
            .unwrap();
        assert_eq!(count, 0, "warrior has no traits");
    }

    #[test]
    fn test_settler_has_build_city_action() {
        let engine = setup();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).unwrap();

        // Verify settler has build_city in its actions list (not traits)
        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let settler = world.unit_types.get_by_name("settler").unwrap();
        assert!(
            settler.actions.contains(&"build_city".to_string()),
            "settler should have build_city in actions"
        );
        assert!(
            settler.traits.is_empty(),
            "settler should have no traits (build_city moved to actions)"
        );
    }

    #[test]
    fn test_unit_type_get_by_name() {
        let engine = setup();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).unwrap();

        let attack: i64 = engine
            .lua
            .load(r#"return UnitType.get("warrior", "attack")"#)
            .eval()
            .unwrap();
        assert_eq!(attack, 1);

        let cost: i64 = engine
            .lua
            .load(r#"return UnitType.get("settler", "cost")"#)
            .eval()
            .unwrap();
        assert_eq!(cost, 30);
    }

    #[test]
    fn test_unit_type_get_nonexistent_returns_nil() {
        let engine = setup();
        let is_nil: bool = engine
            .lua
            .load(r#"return UnitType.get("nonexistent", "attack") == nil"#)
            .eval()
            .unwrap();
        assert!(is_nil);
    }

    #[test]
    fn test_base_mod_registers_warrior_and_settler() {
        let engine = setup();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();

        let warrior = world.unit_types.get_by_name("warrior").unwrap();
        assert_eq!(warrior.attack, 1);
        assert_eq!(warrior.defense, 1);
        assert_eq!(warrior.max_hp, 3);

        let settler = world.unit_types.get_by_name("settler").unwrap();
        assert_eq!(settler.attack, 0);
        assert_eq!(settler.cost, 30);
        assert!(
            settler.traits.is_empty(),
            "settler traits should be empty (build_city moved to actions)"
        );
        assert_eq!(settler.actions, vec!["build_city"]);
    }

    #[test]
    fn test_can_produce_callback_stored_in_handlers() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                UnitType.define({
                    name = "galley",
                    category = "naval",
                    can_produce = function(ctx)
                        if not ctx.is_coastal then
                            ctx.blocked = true
                            ctx.reason = "requires a coastal city"
                        end
                        return ctx
                    end,
                })
                "#,
            )
            .exec()
            .unwrap();

        let has_callback: bool = engine
            .lua
            .load(r#"return type(__unit_type_handlers["galley"].can_produce) == "function""#)
            .eval()
            .unwrap();
        assert!(has_callback, "can_produce callback should be stored");
    }

    #[test]
    fn test_can_produce_callback_blocks_when_expected() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                UnitType.define({
                    name = "test_unit",
                    can_produce = function(ctx)
                        ctx.blocked = true
                        ctx.reason = "always blocked"
                        return ctx
                    end,
                })
                "#,
            )
            .exec()
            .unwrap();

        let blocked: bool = engine
            .lua
            .load(
                r#"
                local ctx = { is_coastal = false, blocked = false, reason = "" }
                local result = __unit_type_handlers["test_unit"].can_produce(ctx)
                return result.blocked
                "#,
            )
            .eval()
            .unwrap();
        assert!(blocked);
    }

    #[test]
    fn test_no_can_produce_means_no_handler_entry() {
        let engine = setup();
        engine
            .lua
            .load(
                r#"
                UnitType.define({
                    name = "warrior",
                    category = "melee",
                })
                "#,
            )
            .exec()
            .unwrap();

        // Handler entry exists but has no can_produce
        let has_callback: bool = engine
            .lua
            .load(r#"return __unit_type_handlers["warrior"].can_produce ~= nil"#)
            .eval()
            .unwrap();
        assert!(
            !has_callback,
            "warrior should not have a can_produce callback"
        );
    }

    #[test]
    fn test_base_mod_settler_has_can_produce() {
        let engine = setup();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).unwrap();

        let has_callback: bool = engine
            .lua
            .load(r#"return type(__unit_type_handlers["settler"].can_produce) == "function""#)
            .eval()
            .unwrap();
        assert!(has_callback, "base mod settler should have can_produce");

        let has_galley: bool = engine
            .lua
            .load(r#"return type(__unit_type_handlers["galley"].can_produce) == "function""#)
            .eval()
            .unwrap();
        assert!(has_galley, "base mod galley should have can_produce");
    }
}
