use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Value};

use crate::id::CityId;
use crate::world::World;

/// Encode a `CityId` as a single Lua integer: `(generation << 32) | index`.
pub fn city_id_to_lua(id: CityId) -> i64 {
    ((id.generation as i64) << 32) | (id.index as i64)
}

/// Decode a Lua integer back to a `CityId`.
pub fn lua_to_city_id(val: i64) -> CityId {
    CityId {
        index: (val & 0xFFFF_FFFF) as u32,
        generation: ((val >> 32) & 0xFFFF_FFFF) as u32,
    }
}

/// Register the `City` Lua global table with `get`, `set`, and `cities_for_player` functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let city_table = lua.create_table()?;

    // City.get(city_id_int, attr_name) -> value | nil
    city_table.set(
        "get",
        lua.create_function(|lua, (cid_int, attr): (i64, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let cid = lua_to_city_id(cid_int);
            let idx = match world.cities.get(cid) {
                Some(i) => i,
                None => return Ok(Value::Nil),
            };

            let val: Value = match attr.as_str() {
                "name" => Value::String(lua.create_string(&world.cities.name[idx])?),
                "owner" => Value::Integer(world.cities.owner[idx].0 as i64),
                "x" => Value::Integer(world.cities.position[idx].x as i64),
                "y" => Value::Integer(world.cities.position[idx].y as i64),
                "population" => Value::Integer(world.cities.population[idx] as i64),
                "food_stockpile" => Value::Integer(world.cities.food_stockpile[idx] as i64),
                "food_per_turn" => Value::Integer(world.cities.food_per_turn[idx] as i64),
                "shield_stockpile" => Value::Integer(world.cities.shield_stockpile[idx] as i64),
                "shields_per_turn" => Value::Integer(world.cities.shields_per_turn[idx] as i64),
                "commerce_per_turn" => Value::Integer(world.cities.commerce_per_turn[idx] as i64),
                "production_cost" => Value::Integer(world.cities.production_cost[idx] as i64),
                "producing" => match &world.cities.producing[idx] {
                    Some(crate::city::ProductionItem::Unit { unit_type_id }) => {
                        match world.unit_types.get(*unit_type_id) {
                            Some(ut) => Value::String(lua.create_string(&ut.name)?),
                            None => Value::Nil,
                        }
                    }
                    Some(crate::city::ProductionItem::Building { building_id }) => {
                        Value::String(lua.create_string(building_id)?)
                    }
                    Some(crate::city::ProductionItem::Wealth) => {
                        Value::String(lua.create_string("wealth")?)
                    }
                    None => Value::Nil,
                },
                "buildings" => {
                    let result = lua.create_table()?;
                    for (i, bid) in world.cities.buildings[idx].iter().enumerate() {
                        result.set((i + 1) as i64, bid.as_str())?;
                    }
                    Value::Table(result)
                }
                _ => Value::Nil,
            };
            Ok(val)
        })?,
    )?;

    // City.set(city_id_int, attr_name, value) -> bool
    city_table.set(
        "set",
        lua.create_function(|lua, (cid_int, attr, value): (i64, String, Value)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            let cid = lua_to_city_id(cid_int);
            let idx = match world.cities.get(cid) {
                Some(i) => i,
                None => return Ok(false),
            };

            match attr.as_str() {
                "population" => {
                    if let Value::Integer(v) = value {
                        world.cities.population[idx] = v as i32;
                        return Ok(true);
                    }
                }
                "food_stockpile" => {
                    if let Value::Integer(v) = value {
                        world.cities.food_stockpile[idx] = v as i32;
                        return Ok(true);
                    }
                }
                "shield_stockpile" => {
                    if let Value::Integer(v) = value {
                        world.cities.shield_stockpile[idx] = v as i32;
                        return Ok(true);
                    }
                }
                _ => {}
            }
            Ok(false)
        })?,
    )?;

    // City.cities_for_player(player_id) -> array of city_id integers
    city_table.set(
        "cities_for_player",
        lua.create_function(|lua, player_id: i64| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let pid = crate::types::PlayerId(player_id as u8);
            let result = lua.create_table()?;
            let mut i = 1;
            for (cid, idx) in world.cities.iter_alive() {
                if world.cities.owner[idx] == pid {
                    result.set(i, city_id_to_lua(cid))?;
                    i += 1;
                }
            }
            Ok(result)
        })?,
    )?;

    // City.get_worked_tiles(city_id_int) -> array of {x, y} tables
    city_table.set(
        "get_worked_tiles",
        lua.create_function(|lua, cid_int: i64| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let cid = lua_to_city_id(cid_int);
            let idx = match world.cities.get(cid) {
                Some(i) => i,
                None => return Ok(Value::Nil),
            };

            let result = lua.create_table()?;
            for (i, coord) in world.cities.worked_tiles[idx].iter().enumerate() {
                let tile = lua.create_table()?;
                tile.set("x", coord.x as i64)?;
                tile.set("y", coord.y as i64)?;
                result.set((i + 1) as i64, tile)?;
            }
            Ok(Value::Table(result))
        })?,
    )?;

    // City.create(name, owner, x, y) -> city_id integer
    // Creates a new city and pushes events/ids to pending queues.
    city_table.set(
        "create",
        lua.create_function(|lua, (name, owner, x, y): (String, i64, i64, i64)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            let player = crate::types::PlayerId(owner as u8);
            let pos = crate::types::TileCoord {
                x: x as u32,
                y: y as u32,
            };

            let city_id = world.cities.spawn(name.clone(), player, pos);

            // Set tile ownership for city radius tiles
            let width = world.tiles.width as i64;
            let height = world.tiles.height as i64;
            let wrap_x = world.tiles.wrap_x;
            let mut radius_tiles = Vec::new();
            for dy in -2i64..=2 {
                for dx in -2i64..=2 {
                    if dx.abs() == 2 && dy.abs() == 2 {
                        continue;
                    }
                    let ny = pos.y as i64 + dy;
                    if ny < 0 || ny >= height {
                        continue;
                    }
                    let nx = pos.x as i64 + dx;
                    let actual_x = if wrap_x {
                        nx.rem_euclid(width) as u32
                    } else {
                        if nx < 0 || nx >= width {
                            continue;
                        }
                        nx as u32
                    };
                    radius_tiles.push(crate::types::TileCoord {
                        x: actual_x,
                        y: ny as u32,
                    });
                }
            }
            for tile_pos in &radius_tiles {
                let tile_idx = world.tiles.idx(tile_pos.x, tile_pos.y);
                world.tiles.owner[tile_idx] = Some(player);
            }

            // Push pending event and city_id
            world
                .pending_events
                .push(crate::protocol::Event::CityFounded {
                    city_id,
                    at: pos,
                    name,
                    owner: player,
                });
            world.pending_city_ids.push(city_id);

            Ok(city_id_to_lua(city_id))
        })?,
    )?;

    // City.all_positions() -> array of {x, y} tables for all alive cities
    city_table.set(
        "all_positions",
        lua.create_function(|lua, ()| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let result = lua.create_table()?;
            let mut i = 1;
            for (_, idx) in world.cities.iter_alive() {
                let pos = world.cities.position[idx];
                let entry = lua.create_table()?;
                entry.set("x", pos.x as i64)?;
                entry.set("y", pos.y as i64)?;
                result.set(i, entry)?;
                i += 1;
            }
            Ok(result)
        })?,
    )?;

    // City.has_building(city_id_int, building_id) -> bool
    city_table.set(
        "has_building",
        lua.create_function(|lua, (cid_int, building_id): (i64, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();
            let cid = lua_to_city_id(cid_int);
            let idx = match world.cities.get(cid) {
                Some(i) => i,
                None => return Ok(false),
            };
            Ok(world.cities.buildings[idx].contains(&building_id))
        })?,
    )?;

    // City.add_building(city_id_int, building_id) -> bool
    city_table.set(
        "add_building",
        lua.create_function(|lua, (cid_int, building_id): (i64, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();
            let cid = lua_to_city_id(cid_int);
            let idx = match world.cities.get(cid) {
                Some(i) => i,
                None => return Ok(false),
            };
            if world.cities.buildings[idx].contains(&building_id) {
                return Ok(false);
            }
            world.cities.buildings[idx].push(building_id);
            Ok(true)
        })?,
    )?;

    lua.globals().set("City", city_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::ScriptEngine;
    use crate::types::{PlayerId, TileCoord};
    use crate::world::WorldConfig;

    fn setup() -> (ScriptEngine, Rc<RefCell<World>>) {
        let config = WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        };
        let world = World::new(&config);
        let world = Rc::new(RefCell::new(world));

        let scripts = ScriptEngine::new().unwrap();
        scripts.lua.set_app_data(Rc::clone(&world));
        register(&scripts.lua).unwrap();

        (scripts, world)
    }

    #[test]
    fn test_city_get_population() {
        let (scripts, world) = setup();
        let city_id = world.borrow_mut().cities.spawn(
            "Rome".to_string(),
            PlayerId(0),
            TileCoord { x: 5, y: 5 },
        );
        let cid_lua = city_id_to_lua(city_id);

        let result: i64 = scripts
            .lua
            .load(format!("return City.get({cid_lua}, 'population')"))
            .eval()
            .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_city_get_name() {
        let (scripts, world) = setup();
        let city_id = world.borrow_mut().cities.spawn(
            "Athens".to_string(),
            PlayerId(0),
            TileCoord { x: 3, y: 3 },
        );
        let cid_lua = city_id_to_lua(city_id);

        let result: String = scripts
            .lua
            .load(format!("return City.get({cid_lua}, 'name')"))
            .eval()
            .unwrap();
        assert_eq!(result, "Athens");
    }

    #[test]
    fn test_city_set_population() {
        let (scripts, world) = setup();
        let city_id = world.borrow_mut().cities.spawn(
            "Rome".to_string(),
            PlayerId(0),
            TileCoord { x: 5, y: 5 },
        );
        let cid_lua = city_id_to_lua(city_id);

        scripts
            .lua
            .load(format!("City.set({cid_lua}, 'population', 5)"))
            .exec()
            .unwrap();

        let w = world.borrow();
        let idx = w.cities.get(city_id).unwrap();
        assert_eq!(w.cities.population[idx], 5);
    }

    #[test]
    fn test_city_get_invalid_returns_nil() {
        let (scripts, _world) = setup();
        // Invalid city ID
        let result: Value = scripts
            .lua
            .load("return City.get(99999999, 'population')")
            .eval()
            .unwrap();
        assert!(matches!(result, Value::Nil));
    }

    #[test]
    fn test_cities_for_player() {
        let (scripts, world) = setup();
        world
            .borrow_mut()
            .cities
            .spawn("Rome".to_string(), PlayerId(0), TileCoord { x: 5, y: 5 });
        world.borrow_mut().cities.spawn(
            "Athens".to_string(),
            PlayerId(0),
            TileCoord { x: 3, y: 3 },
        );
        world.borrow_mut().cities.spawn(
            "Thebes".to_string(),
            PlayerId(1),
            TileCoord { x: 7, y: 7 },
        );

        let count: i64 = scripts
            .lua
            .load("local t = City.cities_for_player(0); return #t")
            .eval()
            .unwrap();
        assert_eq!(count, 2, "P0 should have 2 cities");

        let count: i64 = scripts
            .lua
            .load("local t = City.cities_for_player(1); return #t")
            .eval()
            .unwrap();
        assert_eq!(count, 1, "P1 should have 1 city");
    }
}
