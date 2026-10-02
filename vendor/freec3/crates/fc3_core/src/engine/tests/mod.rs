mod building_tests;
mod city_tests;
mod civ_tests;
mod combat_tests;
mod core_tests;
mod destination_tests;
mod movement_zoc_tests;
mod tech_tests;
mod worker_tests;

use super::*;
use crate::id::GenId;
use crate::protocol::{Command, Event};
use crate::types::TileCoord;

fn test_config() -> GameConfig {
    GameConfig {
        world: WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec![],
        max_turns: None,
    }
}

fn _test_config_no_mods() -> GameConfig {
    GameConfig {
        world: WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec![],
        units_per_player: vec![],
        max_turns: None,
    }
}

fn spawn_warrior(engine: &Engine, owner: PlayerId, pos: TileCoord) -> GenId {
    let id = {
        let mut world = engine.world.borrow_mut();
        let wid = world.unit_types.get_by_name("warrior").unwrap().id;
        let template = world.unit_types.get(wid).unwrap().clone();
        world.units.spawn(wid, owner, pos, &template)
    };
    engine.update_visibility(owner);
    id
}

fn spawn_settler(engine: &Engine, owner: PlayerId, pos: TileCoord) -> GenId {
    let id = {
        let mut world = engine.world.borrow_mut();
        let sid = world.unit_types.get_by_name("settler").unwrap().id;
        let template = world.unit_types.get(sid).unwrap().clone();
        world.units.spawn(sid, owner, pos, &template)
    };
    engine.update_visibility(owner);
    id
}

/// Helper: makes all tiles Visible for a player (so pathfinding sees real terrain).
fn reveal_all(engine: &Engine, player: PlayerId) {
    let mut world = engine.world.borrow_mut();
    let w = world.tiles.width;
    let h = world.tiles.height;
    for y in 0..h {
        for x in 0..w {
            world
                .tiles
                .set_visibility(player, x, y, crate::tile::Visibility::Visible);
        }
    }
}

/// Helper: founds a city and returns the city_id.
/// Also sets research for the player if not already set (required for EndTurn).
fn found_city(
    engine: &mut Engine,
    player: PlayerId,
    pos: TileCoord,
    _name: &str,
) -> crate::id::CityId {
    let settler_id = spawn_settler(engine, player, pos);
    let result = engine.submit_command(
        player,
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );
    assert!(
        result.is_ok(),
        "founding should succeed: {:?}",
        result.errors
    );
    // Extract city_id from event
    let city_id = result
        .events
        .iter()
        .find_map(|e| match e {
            Event::CityFounded { city_id, .. } => Some(*city_id),
            _ => None,
        })
        .expect("CityFounded event missing");

    // Auto-set research if player doesn't have research set yet
    // (ensures science accumulates during tests)
    {
        let world = engine.world.borrow();
        let needs_research = world
            .players
            .get(player.0 as usize)
            .is_some_and(|p| p.researching.is_none());
        drop(world);
        if needs_research {
            set_research(engine, player);
        }
    }

    city_id
}

/// Sets production to warrior for a city. Used by tests that need
/// EndTurn to succeed (which now requires all cities to have production set).
fn set_production_warrior(engine: &mut Engine, player: PlayerId, city_id: crate::id::CityId) {
    let warrior_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("warrior").unwrap().id
    };
    let result = engine.submit_command(
        player,
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );
    assert!(
        result.is_ok(),
        "set production should succeed: {:?}",
        result.errors
    );
}

/// Sets research to the first available tech. Used by tests that need
/// science to accumulate (or that previously required research for EndTurn).
fn set_research(engine: &mut Engine, player: PlayerId) {
    let techs = engine.available_techs(player);
    if let Some(tech) = techs.first() {
        let result = engine.submit_command(
            player,
            Command::SetResearch {
                tech_id: tech.id.clone(),
            },
        );
        assert!(
            result.is_ok(),
            "set research should succeed: {:?}",
            result.errors
        );
    }
}

fn test_config_no_wrap() -> GameConfig {
    GameConfig {
        world: WorldConfig {
            width: 10,
            height: 10,
            wrap_x: false,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec![],
        max_turns: None,
    }
}

// ── Helpers for unit category / trait tests ─────────────────────

fn spawn_unit_by_name(engine: &Engine, name: &str, owner: PlayerId, pos: TileCoord) -> GenId {
    let mut world = engine.world.borrow_mut();
    let ut_id = world
        .unit_types
        .get_by_name(name)
        .unwrap_or_else(|| panic!("unit type '{}' not found", name))
        .id;
    let template = world.unit_types.get(ut_id).unwrap().clone();
    world.units.spawn(ut_id, owner, pos, &template)
}
