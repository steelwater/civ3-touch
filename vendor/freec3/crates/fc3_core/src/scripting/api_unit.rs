use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Value};

use crate::dynamic::AttrValue;
use crate::id::UnitId;
use crate::world::World;

/// Encode a `UnitId` as a single Lua integer: `(generation << 32) | index`.
pub fn unit_id_to_lua(id: UnitId) -> i64 {
    ((id.generation as i64) << 32) | (id.index as i64)
}

/// Decode a Lua integer back to a `UnitId`.
pub fn lua_to_unit_id(val: i64) -> UnitId {
    UnitId {
        index: (val & 0xFFFF_FFFF) as u32,
        generation: ((val >> 32) & 0xFFFF_FFFF) as u32,
    }
}

/// Register the `Unit` Lua global table with `get` and `set` functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let unit_table = lua.create_table()?;

    // Unit.get(unit_id_int, attr_name) -> value | nil
    unit_table.set(
        "get",
        lua.create_function(|lua, (uid_int, attr): (i64, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let uid = lua_to_unit_id(uid_int);
            let idx = match world.units.get(uid) {
                Some(i) => i,
                None => return Ok(Value::Nil),
            };

            let val: Value = match attr.as_str() {
                "type" => Value::Integer(world.units.unit_type[idx].0 as i64),
                "owner" => Value::Integer(world.units.owner[idx].0 as i64),
                "x" => Value::Integer(world.units.position[idx].x as i64),
                "y" => Value::Integer(world.units.position[idx].y as i64),
                "movement" => Value::Integer(world.units.movement[idx] as i64),
                "max_movement" => Value::Integer(world.units.max_movement[idx] as i64),
                "hp" => Value::Integer(world.units.hp[idx] as i64),
                "max_hp" => Value::Integer(world.units.max_hp[idx] as i64),
                "fortified" => Value::Boolean(world.units.fortified[idx]),
                "has_moved" => Value::Boolean(world.units.has_moved[idx]),
                "skipped" => Value::Boolean(world.units.skipped[idx]),
                "destination_x" => match world.units.destination[idx] {
                    Some(dest) => Value::Integer(dest.x as i64),
                    None => Value::Nil,
                },
                "destination_y" => match world.units.destination[idx] {
                    Some(dest) => Value::Integer(dest.y as i64),
                    None => Value::Nil,
                },
                _ => {
                    // Fallback to dynamic columns
                    match world.units.dynamic.get(&attr, idx) {
                        Some(AttrValue::Int(v)) => Value::Integer(v as i64),
                        Some(AttrValue::Float(v)) => Value::Number(v as f64),
                        Some(AttrValue::Bool(v)) => Value::Boolean(v),
                        None => Value::Nil,
                    }
                }
            };
            Ok(val)
        })?,
    )?;

    // Unit.set(unit_id_int, attr_name, value) -> bool
    unit_table.set(
        "set",
        lua.create_function(|lua, (uid_int, attr, value): (i64, String, Value)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            let uid = lua_to_unit_id(uid_int);
            let idx = match world.units.get(uid) {
                Some(i) => i,
                None => return Ok(false),
            };

            match attr.as_str() {
                "movement" => {
                    if let Value::Integer(v) = value {
                        world.units.movement[idx] = v as i32;
                        return Ok(true);
                    }
                }
                "max_movement" => {
                    if let Value::Integer(v) = value {
                        world.units.max_movement[idx] = v as i32;
                        return Ok(true);
                    }
                }
                "hp" => {
                    if let Value::Integer(v) = value {
                        world.units.hp[idx] = v as i32;
                        return Ok(true);
                    }
                }
                "max_hp" => {
                    if let Value::Integer(v) = value {
                        world.units.max_hp[idx] = v as i32;
                        return Ok(true);
                    }
                }
                "fortified" => {
                    if let Value::Boolean(v) = value {
                        world.units.fortified[idx] = v;
                        return Ok(true);
                    }
                }
                "has_moved" => {
                    if let Value::Boolean(v) = value {
                        world.units.has_moved[idx] = v;
                        return Ok(true);
                    }
                }
                "skipped" => {
                    if let Value::Boolean(v) = value {
                        world.units.skipped[idx] = v;
                        return Ok(true);
                    }
                }
                "destination_x" => {
                    if let Value::Nil = value {
                        world.units.destination[idx] = None;
                        return Ok(true);
                    }
                    if let Value::Integer(v) = value {
                        let dest = world.units.destination[idx]
                            .unwrap_or(crate::types::TileCoord { x: 0, y: 0 });
                        world.units.destination[idx] = Some(crate::types::TileCoord {
                            x: v as u32,
                            y: dest.y,
                        });
                        return Ok(true);
                    }
                }
                "destination_y" => {
                    if let Value::Nil = value {
                        world.units.destination[idx] = None;
                        return Ok(true);
                    }
                    if let Value::Integer(v) = value {
                        let dest = world.units.destination[idx]
                            .unwrap_or(crate::types::TileCoord { x: 0, y: 0 });
                        world.units.destination[idx] = Some(crate::types::TileCoord {
                            x: dest.x,
                            y: v as u32,
                        });
                        return Ok(true);
                    }
                }
                _ => {
                    // Fallback to dynamic columns
                    let attr_val = match value {
                        Value::Integer(v) => AttrValue::Int(v as i32),
                        Value::Number(v) => AttrValue::Float(v as f32),
                        Value::Boolean(v) => AttrValue::Bool(v),
                        _ => return Ok(false),
                    };
                    return Ok(world.units.dynamic.set(&attr, idx, attr_val).is_ok());
                }
            }
            Ok(false)
        })?,
    )?;

    // Unit.consume(unit_id_int, reason) -> bool
    // Destroys a unit and pushes events to pending queues.
    unit_table.set(
        "consume",
        lua.create_function(|lua, (uid_int, reason): (i64, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            let uid = lua_to_unit_id(uid_int);
            if !world.units.destroy(uid) {
                return Ok(false);
            }
            world
                .pending_events
                .push(crate::protocol::Event::UnitConsumed {
                    unit_id: uid,
                    reason,
                });
            world.pending_consumed_units.push(uid);
            Ok(true)
        })?,
    )?;

    lua.globals().set("Unit", unit_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::ScriptEngine;
    use crate::types::{PlayerId, TileCoord, UnitTypeId};
    use crate::unit_type::UnitType;
    use crate::world::WorldConfig;

    fn make_warrior() -> UnitType {
        UnitType {
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
        }
    }

    fn setup_with_unit() -> (ScriptEngine, i64) {
        let engine = ScriptEngine::new().unwrap();
        let mut world = World::new(&WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        });

        let warrior = make_warrior();
        let wid = world.unit_types.register(warrior.clone());
        let uid = world
            .units
            .spawn(wid, PlayerId(0), TileCoord { x: 5, y: 5 }, &warrior);

        let uid_lua = unit_id_to_lua(uid);
        let world = Rc::new(RefCell::new(world));
        engine.lua.set_app_data(world);
        register(&engine.lua).unwrap();

        (engine, uid_lua)
    }

    #[test]
    fn test_unit_get_movement() {
        let (engine, uid) = setup_with_unit();
        engine.lua.globals().set("test_uid", uid).unwrap();
        let movement: i64 = engine
            .lua
            .load("return Unit.get(test_uid, 'movement')")
            .eval()
            .unwrap();
        // Movement is scaled: template.movement=1 → runtime = 1 * MOVEMENT_SCALE = 3
        assert_eq!(movement, 3);
    }

    #[test]
    fn test_unit_set_movement_then_read_from_rust() {
        let (engine, uid_lua) = setup_with_unit();
        engine.lua.globals().set("test_uid", uid_lua).unwrap();
        engine
            .lua
            .load("Unit.set(test_uid, 'movement', 5)")
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let uid = lua_to_unit_id(uid_lua);
        let idx = world.units.get(uid).unwrap();
        assert_eq!(world.units.movement[idx], 5);
    }

    #[test]
    fn test_unit_get_invalid_id_returns_nil() {
        let (engine, _) = setup_with_unit();
        let is_nil: bool = engine
            .lua
            .load("return Unit.get(9999999, 'movement') == nil")
            .eval()
            .unwrap();
        assert!(is_nil, "invalid unit id should return nil");
    }

    #[test]
    fn test_unit_get_position() {
        let (engine, uid) = setup_with_unit();
        engine.lua.globals().set("test_uid", uid).unwrap();
        let x: i64 = engine
            .lua
            .load("return Unit.get(test_uid, 'x')")
            .eval()
            .unwrap();
        let y: i64 = engine
            .lua
            .load("return Unit.get(test_uid, 'y')")
            .eval()
            .unwrap();
        assert_eq!(x, 5);
        assert_eq!(y, 5);
    }

    #[test]
    fn test_unit_dynamic_attribute() {
        let (engine, uid_lua) = setup_with_unit();
        engine.lua.globals().set("test_uid", uid_lua).unwrap();

        // Register a dynamic column from Rust
        {
            let world = engine
                .lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .unwrap()
                .clone();
            let mut world = world.borrow_mut();
            use crate::dynamic::{AttrValue, ColType};
            world
                .units
                .dynamic
                .register("morale", ColType::Int, AttrValue::Int(50))
                .unwrap();
        }

        // Read default from Lua
        let morale: i64 = engine
            .lua
            .load("return Unit.get(test_uid, 'morale')")
            .eval()
            .unwrap();
        assert_eq!(morale, 50);

        // Set from Lua, read back
        engine
            .lua
            .load("Unit.set(test_uid, 'morale', 80)")
            .exec()
            .unwrap();
        let morale: i64 = engine
            .lua
            .load("return Unit.get(test_uid, 'morale')")
            .eval()
            .unwrap();
        assert_eq!(morale, 80);
    }

    #[test]
    fn test_unit_id_encoding_roundtrip() {
        let id = UnitId {
            index: 42,
            generation: 7,
        };
        let encoded = unit_id_to_lua(id);
        let decoded = lua_to_unit_id(encoded);
        assert_eq!(id, decoded);
    }
}
