//! Integration test: full Lua round-trip.
//!
//! Creates a World, initializes a ScriptEngine with all APIs, loads the base
//! mod, spawns units, and verifies Lua can read/write attributes and fire hooks.

use std::cell::RefCell;
use std::rc::Rc;

use fc3_core::scripting::api_action;
use fc3_core::scripting::api_building;
use fc3_core::scripting::api_city;
use fc3_core::scripting::api_civilization;
use fc3_core::scripting::api_engine;
use fc3_core::scripting::api_tech;
use fc3_core::scripting::api_tile;
use fc3_core::scripting::api_unit;
use fc3_core::scripting::api_unit_type;
use fc3_core::scripting::ScriptEngine;
use fc3_core::types::{PlayerId, TileCoord};
use fc3_core::world::{World, WorldConfig};

/// Set up a full engine: world + script engine + all APIs + base mod loaded.
fn full_setup() -> (ScriptEngine, Rc<RefCell<World>>) {
    let engine = ScriptEngine::new().unwrap();
    let world = World::new(&WorldConfig {
        width: 10,
        height: 10,
        wrap_x: true,
        wrap_y: false,
        num_players: 2,
        seed: 12345,
    });
    let world = Rc::new(RefCell::new(world));

    engine.lua.set_app_data(Rc::clone(&world));

    // Register all Lua APIs
    api_unit_type::register(&engine.lua).unwrap();
    api_unit::register(&engine.lua).unwrap();
    api_tile::register(&engine.lua).unwrap();
    api_engine::register(&engine.lua).unwrap();
    api_action::register(&engine.lua).unwrap();
    api_tech::register(&engine.lua).unwrap();
    api_building::register(&engine.lua).unwrap();
    api_civilization::register(&engine.lua).unwrap();
    api_city::register(&engine.lua).unwrap();

    // Load the base mod
    let mod_dir = ScriptEngine::find_mod_dir("base");
    engine.load_mod(&mod_dir).unwrap();

    (engine, world)
}

#[test]
fn test_full_lua_roundtrip() {
    let (engine, world) = full_setup();

    // ── Assert: UnitTypeRegistry has "warrior" and "settler" ──
    {
        let w = world.borrow();
        assert!(
            w.unit_types.get_by_name("warrior").is_some(),
            "warrior should be registered"
        );
        assert!(
            w.unit_types.get_by_name("settler").is_some(),
            "settler should be registered"
        );
    }

    // ── Spawn a warrior via Rust API ──
    let warrior_uid_lua;
    {
        let mut w = world.borrow_mut();
        let wid = w.unit_types.get_by_name("warrior").unwrap().id;
        let template = w.unit_types.get(wid).unwrap().clone();
        let uid = w
            .units
            .spawn(wid, PlayerId(0), TileCoord { x: 3, y: 4 }, &template);
        warrior_uid_lua = api_unit::unit_id_to_lua(uid);
    }

    engine
        .lua
        .globals()
        .set("warrior_id", warrior_uid_lua)
        .unwrap();

    // ── From Lua: read warrior's attack (should be 1) ──
    let attack: i64 = engine
        .lua
        .load("return Unit.get(warrior_id, 'hp')")
        .eval()
        .unwrap();
    assert_eq!(attack, 3, "warrior hp should be 3 (max_hp from template)");

    let movement: i64 = engine
        .lua
        .load("return Unit.get(warrior_id, 'movement')")
        .eval()
        .unwrap();
    // Movement is scaled: template.movement=1 → runtime = 1 * MOVEMENT_SCALE = 3
    assert_eq!(
        movement, 3,
        "warrior movement should be 3 (1 * MOVEMENT_SCALE)"
    );

    // ── From Lua: modify movement to 5 ──
    engine
        .lua
        .load("Unit.set(warrior_id, 'movement', 5)")
        .exec()
        .unwrap();

    // ── From Rust: verify movement is now 5 ──
    {
        let w = world.borrow();
        let uid = api_unit::lua_to_unit_id(warrior_uid_lua);
        let idx = w.units.get(uid).unwrap();
        assert_eq!(
            w.units.movement[idx], 5,
            "movement should be 5 after Lua set"
        );
    }

    // ── Register a hook that doubles a value ──
    engine
        .lua
        .load(
            r#"
            register_hook("on_test_event", function(ctx)
                ctx.value = ctx.value * 2
            end)
            "#,
        )
        .exec()
        .unwrap();

    // ── Fire the hook from Lua with {value = 10} ──
    let result: i64 = engine
        .lua
        .load(
            r#"
            local result = fire_hook("on_test_event", { value = 10 })
            return result.value
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(result, 20, "hook should double the value: 10 -> 20");

    // ── Verify tile data is readable from Lua ──
    let terrain: String = engine
        .lua
        .load("return Tile.get(3, 4, 'terrain')")
        .eval()
        .unwrap();
    assert_eq!(terrain, "grassland");

    // ── Verify UnitType.get works ──
    let warrior_cost: i64 = engine
        .lua
        .load(r#"return UnitType.get("warrior", "cost")"#)
        .eval()
        .unwrap();
    assert_eq!(warrior_cost, 10);

    // ── Verify Engine.random works ──
    let rand_val: i64 = engine
        .lua
        .load("return Engine.random(1, 100)")
        .eval()
        .unwrap();
    assert!(
        (1..=100).contains(&rand_val),
        "random value {rand_val} should be in [1, 100]"
    );

    // ── Verify Engine.register_attribute + dynamic column round-trip ──
    engine
        .lua
        .load(r#"Engine.register_attribute("unit", "xp", "int", 0)"#)
        .exec()
        .unwrap();

    let xp: i64 = engine
        .lua
        .load("return Unit.get(warrior_id, 'xp')")
        .eval()
        .unwrap();
    assert_eq!(xp, 0, "default xp should be 0");

    engine
        .lua
        .load("Unit.set(warrior_id, 'xp', 42)")
        .exec()
        .unwrap();

    let xp: i64 = engine
        .lua
        .load("return Unit.get(warrior_id, 'xp')")
        .eval()
        .unwrap();
    assert_eq!(xp, 42, "xp should be 42 after set");
}
