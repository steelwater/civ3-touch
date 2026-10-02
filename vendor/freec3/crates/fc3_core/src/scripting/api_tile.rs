use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Value};

use crate::tile::{Terrain, Vegetation};
use crate::types::PlayerId;
use crate::world::World;

fn terrain_to_str(t: Terrain) -> &'static str {
    match t {
        Terrain::Grassland => "grassland",
        Terrain::Plains => "plains",
        Terrain::Desert => "desert",
        Terrain::Tundra => "tundra",
        Terrain::Ocean => "ocean",
        Terrain::Coast => "coast",
        Terrain::Mountain => "mountain",
        Terrain::Hill => "hill",
        Terrain::Ice => "ice",
    }
}

fn str_to_terrain(s: &str) -> Option<Terrain> {
    match s {
        "grassland" => Some(Terrain::Grassland),
        "plains" => Some(Terrain::Plains),
        "desert" => Some(Terrain::Desert),
        "tundra" => Some(Terrain::Tundra),
        "ocean" => Some(Terrain::Ocean),
        "coast" => Some(Terrain::Coast),
        "mountain" => Some(Terrain::Mountain),
        "hill" => Some(Terrain::Hill),
        "ice" => Some(Terrain::Ice),
        _ => None,
    }
}

fn vegetation_to_str(v: Vegetation) -> &'static str {
    match v {
        Vegetation::None => "none",
        Vegetation::Forest => "forest",
        Vegetation::Jungle => "jungle",
    }
}

fn str_to_vegetation(s: &str) -> Option<Vegetation> {
    match s {
        "none" => Some(Vegetation::None),
        "forest" => Some(Vegetation::Forest),
        "jungle" => Some(Vegetation::Jungle),
        _ => None,
    }
}

/// Register the `Tile` Lua global table with get/set/neighbors functions.
pub fn register(lua: &Lua) -> LuaResult<()> {
    let tile_table = lua.create_table()?;

    // Tile.get(x, y, attr_name) -> value | nil
    tile_table.set(
        "get",
        lua.create_function(|lua, (x, y, attr): (u32, u32, String)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();

            if !world.tiles.in_bounds(x, y) {
                return Ok(Value::Nil);
            }
            let idx = world.tiles.idx(x, y);

            let val: Value = match attr.as_str() {
                "terrain" => {
                    Value::String(lua.create_string(terrain_to_str(world.tiles.terrain[idx]))?)
                }
                "vegetation" => {
                    Value::String(lua.create_string(vegetation_to_str(world.tiles.vegetation[idx]))?)
                }
                "road_level" => Value::Integer(world.tiles.road_level[idx] as i64),
                "river_edges" => Value::Integer(world.tiles.river_edges[idx] as i64),
                "owner" => match world.tiles.owner[idx] {
                    Some(pid) => Value::Integer(pid.0 as i64),
                    None => Value::Nil,
                },
                "resource" => match world.tiles.resource[idx] {
                    Some(rid) => Value::Integer(rid.0 as i64),
                    None => Value::Nil,
                },
                "improvement" => match world.tiles.improvement[idx] {
                    Some(iid) => Value::Integer(iid.0 as i64),
                    None => Value::Nil,
                },
                _ => Value::Nil,
            };
            Ok(val)
        })?,
    )?;

    // Tile.set(x, y, attr_name, value) -> bool
    tile_table.set(
        "set",
        lua.create_function(|lua, (x, y, attr, value): (u32, u32, String, Value)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let mut world = world.borrow_mut();

            if !world.tiles.in_bounds(x, y) {
                return Ok(false);
            }
            let idx = world.tiles.idx(x, y);

            match attr.as_str() {
                "terrain" => {
                    if let Value::String(s) = value {
                        if let Ok(s) = s.to_str() {
                            if let Some(t) = str_to_terrain(&s) {
                                world.tiles.terrain[idx] = t;
                                return Ok(true);
                            }
                        }
                    }
                }
                "vegetation" => {
                    if let Value::String(s) = value {
                        if let Ok(s) = s.to_str() {
                            if let Some(v) = str_to_vegetation(&s) {
                                world.tiles.vegetation[idx] = v;
                                return Ok(true);
                            }
                        }
                    }
                }
                "road_level" => {
                    if let Value::Integer(v) = value {
                        world.tiles.road_level[idx] = v as u8;
                        return Ok(true);
                    }
                }
                "river_edges" => {
                    if let Value::Integer(v) = value {
                        world.tiles.river_edges[idx] = v as u8;
                        return Ok(true);
                    }
                }
                "owner" => match value {
                    Value::Integer(v) => {
                        world.tiles.owner[idx] = Some(PlayerId(v as u8));
                        return Ok(true);
                    }
                    Value::Nil => {
                        world.tiles.owner[idx] = None;
                        return Ok(true);
                    }
                    _ => {}
                },
                "improvement" => match value {
                    Value::Integer(v) => {
                        world.tiles.improvement[idx] = Some(crate::types::ImprovementId(v as u16));
                        return Ok(true);
                    }
                    Value::Nil => {
                        world.tiles.improvement[idx] = None;
                        return Ok(true);
                    }
                    _ => {}
                },
                _ => {}
            }
            Ok(false)
        })?,
    )?;

    // Tile.neighbors(x, y) -> array of {x, y} tables
    tile_table.set(
        "neighbors",
        lua.create_function(|lua, (x, y): (u32, u32)| {
            let world = lua
                .app_data_ref::<Rc<RefCell<World>>>()
                .ok_or_else(|| mlua::Error::external("world not initialized"))?
                .clone();
            let world = world.borrow();

            let neighbors = world.tiles.neighbors(x, y);
            let result = lua.create_table()?;
            for (i, coord) in neighbors.iter().enumerate() {
                let entry = lua.create_table()?;
                entry.set("x", coord.x)?;
                entry.set("y", coord.y)?;
                result.set(i + 1, entry)?;
            }
            Ok(result)
        })?,
    )?;

    lua.globals().set("Tile", tile_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::ScriptEngine;
    use crate::world::WorldConfig;

    fn setup() -> ScriptEngine {
        let engine = ScriptEngine::new().unwrap();
        let mut world = World::new(&WorldConfig {
            width: 10,
            height: 10,
            wrap_x: false,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        });
        // Set some known terrain
        let idx = world.tiles.idx(3, 3);
        world.tiles.terrain[idx] = Terrain::Mountain;

        let world = Rc::new(RefCell::new(world));
        engine.lua.set_app_data(world);
        register(&engine.lua).unwrap();
        engine
    }

    #[test]
    fn test_tile_get_terrain() {
        let engine = setup();
        let terrain: String = engine
            .lua
            .load("return Tile.get(0, 0, 'terrain')")
            .eval()
            .unwrap();
        assert_eq!(terrain, "grassland");

        let mountain: String = engine
            .lua
            .load("return Tile.get(3, 3, 'terrain')")
            .eval()
            .unwrap();
        assert_eq!(mountain, "mountain");
    }

    #[test]
    fn test_tile_set_road_level() {
        let engine = setup();
        engine
            .lua
            .load("Tile.set(5, 5, 'road_level', 2)")
            .exec()
            .unwrap();

        let world = engine
            .lua
            .app_data_ref::<Rc<RefCell<World>>>()
            .unwrap()
            .clone();
        let world = world.borrow();
        let idx = world.tiles.idx(5, 5);
        assert_eq!(world.tiles.road_level[idx], 2);
    }

    #[test]
    fn test_tile_set_road_level_read_from_lua() {
        let engine = setup();
        engine
            .lua
            .load("Tile.set(5, 5, 'road_level', 3)")
            .exec()
            .unwrap();
        let val: i64 = engine
            .lua
            .load("return Tile.get(5, 5, 'road_level')")
            .eval()
            .unwrap();
        assert_eq!(val, 3);
    }

    #[test]
    fn test_tile_neighbors_count() {
        let engine = setup();
        // Corner (0,0) on non-wrapping map -> 3 neighbors
        let count: i64 = engine
            .lua
            .load("return #Tile.neighbors(0, 0)")
            .eval()
            .unwrap();
        assert_eq!(count, 3);

        // Center (5,5) -> 8 neighbors
        let count: i64 = engine
            .lua
            .load("return #Tile.neighbors(5, 5)")
            .eval()
            .unwrap();
        assert_eq!(count, 8);
    }

    #[test]
    fn test_tile_neighbors_coords() {
        let engine = setup();
        let result: bool = engine
            .lua
            .load(
                r#"
                local n = Tile.neighbors(5, 5)
                for _, coord in ipairs(n) do
                    if coord.x == 4 and coord.y == 4 then
                        return true
                    end
                end
                return false
                "#,
            )
            .eval()
            .unwrap();
        assert!(result, "(4,4) should be a neighbor of (5,5)");
    }

    #[test]
    fn test_tile_get_out_of_bounds_returns_nil() {
        let engine = setup();
        let is_nil: bool = engine
            .lua
            .load("return Tile.get(100, 100, 'terrain') == nil")
            .eval()
            .unwrap();
        assert!(is_nil, "out of bounds should return nil");
    }
}
