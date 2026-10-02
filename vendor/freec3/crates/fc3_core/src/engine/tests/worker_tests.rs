use super::*;
use crate::protocol::Command;
use crate::tile::Terrain;
use crate::types::{ImprovementId, TileCoord};

fn spawn_worker(engine: &Engine, owner: PlayerId, pos: TileCoord) -> GenId {
    let mut world = engine.world.borrow_mut();
    let wid = world.unit_types.get_by_name("worker").unwrap().id;
    let template = world.unit_types.get(wid).unwrap().clone();
    world.units.spawn(wid, owner, pos, &template)
}

/// Helper: end turns for both players to advance one full round.
/// Skips all units for both players before ending their turns.
fn advance_one_round(engine: &mut Engine) {
    // Skip all P0 units that need handling
    skip_all_units(engine, PlayerId(0));
    let r = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(r.is_ok(), "P0 EndTurn should succeed: {:?}", r.errors);
    // Skip all P1 units that need handling
    skip_all_units(engine, PlayerId(1));
    let r = engine.submit_command(PlayerId(1), Command::EndTurn);
    assert!(r.is_ok(), "P1 EndTurn should succeed: {:?}", r.errors);
}

/// Helper: skip all units with remaining movement for a player.
fn skip_all_units(engine: &mut Engine, player: PlayerId) {
    loop {
        let uids: Vec<GenId> = {
            let world = engine.world.borrow();
            world
                .units
                .iter_alive()
                .filter(|&(_, idx)| {
                    world.units.owner[idx] == player
                        && world.units.movement[idx] > 0
                        && !world.units.fortified[idx]
                        && !world.units.skipped[idx]
                        && world.units.current_action[idx].is_none()
                })
                .map(|(uid, _)| uid)
                .collect()
        };
        if uids.is_empty() {
            break;
        }
        for uid in uids {
            engine.submit_command(player, Command::SkipUnit { unit_id: uid });
        }
    }
}

// ── Build Road ─────────────────────────────────────────────────

#[test]
fn test_worker_builds_road() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    // Ensure the tile is grassland with no road
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Grassland;
        world.tiles.road_level[idx] = 0;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    // Need a unit for P1 so the game doesn't end
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 9, y: 9 });
    engine.update_visibility(PlayerId(0));
    engine.update_visibility(PlayerId(1));

    // Start building road (2-turn action)
    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_road".to_string(),
        },
    );
    assert!(
        result.is_ok(),
        "build_road should start: {:?}",
        result.errors
    );

    // After 1 round: action in progress, not yet complete
    advance_one_round(&mut engine);
    {
        let world = engine.world.borrow();
        let idx = world.tiles.idx(pos.x, pos.y);
        assert_eq!(
            world.tiles.road_level[idx], 0,
            "road should not be built yet after 1 round"
        );
    }

    // After 2nd round: action completes
    advance_one_round(&mut engine);
    {
        let world = engine.world.borrow();
        let idx = world.tiles.idx(pos.x, pos.y);
        assert_eq!(
            world.tiles.road_level[idx], 1,
            "road should be built after 2 rounds"
        );
    }

    // Worker should still be alive (not consumed)
    {
        let world = engine.world.borrow();
        assert!(
            world.units.get(worker_id).is_some(),
            "worker should still be alive after building road"
        );
    }
}

// ── Build Mine ─────────────────────────────────────────────────

#[test]
fn test_worker_builds_mine_on_hill() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Hill;
        world.tiles.improvement[idx] = None;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 9, y: 9 });
    engine.update_visibility(PlayerId(0));
    engine.update_visibility(PlayerId(1));

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_mine".to_string(),
        },
    );
    assert!(
        result.is_ok(),
        "build_mine should start: {:?}",
        result.errors
    );

    // 3-turn action: need 3 rounds
    advance_one_round(&mut engine);
    advance_one_round(&mut engine);
    {
        let world = engine.world.borrow();
        let idx = world.tiles.idx(pos.x, pos.y);
        assert_eq!(
            world.tiles.improvement[idx], None,
            "mine should not be built yet after 2 rounds"
        );
    }

    advance_one_round(&mut engine);
    {
        let world = engine.world.borrow();
        let idx = world.tiles.idx(pos.x, pos.y);
        assert_eq!(
            world.tiles.improvement[idx],
            Some(ImprovementId(1)),
            "mine should be built after 3 rounds"
        );
    }

    // Worker should still be alive
    {
        let world = engine.world.borrow();
        assert!(
            world.units.get(worker_id).is_some(),
            "worker survives mine building"
        );
    }
}

// ── Build Irrigation ───────────────────────────────────────────

#[test]
fn test_worker_builds_irrigation_on_grassland() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Grassland;
        world.tiles.improvement[idx] = None;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 9, y: 9 });
    engine.update_visibility(PlayerId(0));
    engine.update_visibility(PlayerId(1));

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_irrigation".to_string(),
        },
    );
    assert!(
        result.is_ok(),
        "build_irrigation should start: {:?}",
        result.errors
    );

    // 3-turn action
    advance_one_round(&mut engine);
    advance_one_round(&mut engine);
    advance_one_round(&mut engine);

    {
        let world = engine.world.borrow();
        let idx = world.tiles.idx(pos.x, pos.y);
        assert_eq!(
            world.tiles.improvement[idx],
            Some(ImprovementId(2)),
            "irrigation should be built after 3 rounds"
        );
    }

    // Worker should still be alive
    {
        let world = engine.world.borrow();
        assert!(
            world.units.get(worker_id).is_some(),
            "worker survives irrigation building"
        );
    }
}

// ── Validation: blocked conditions ─────────────────────────────

#[test]
fn test_build_road_blocked_on_ocean() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Ocean;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    engine.update_visibility(PlayerId(0));

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_road".to_string(),
        },
    );
    assert!(!result.is_ok(), "build_road should be blocked on ocean");
}

#[test]
fn test_build_road_blocked_on_existing_road() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Grassland;
        world.tiles.road_level[idx] = 1;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    engine.update_visibility(PlayerId(0));

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_road".to_string(),
        },
    );
    assert!(
        !result.is_ok(),
        "build_road should be blocked when road already exists"
    );
}

#[test]
fn test_build_mine_blocked_on_non_hill() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Grassland;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    engine.update_visibility(PlayerId(0));

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_mine".to_string(),
        },
    );
    assert!(
        !result.is_ok(),
        "build_mine should be blocked on non-hill terrain"
    );
}

#[test]
fn test_build_irrigation_blocked_on_hill() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Hill;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    engine.update_visibility(PlayerId(0));

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_irrigation".to_string(),
        },
    );
    assert!(
        !result.is_ok(),
        "build_irrigation should be blocked on hill"
    );
}

#[test]
fn test_build_irrigation_blocked_on_mountain() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let pos = TileCoord { x: 5, y: 5 };

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(pos.x, pos.y);
        world.tiles.terrain[idx] = Terrain::Mountain;
    }

    let worker_id = spawn_worker(&engine, PlayerId(0), pos);
    engine.update_visibility(PlayerId(0));

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker_id,
            action_id: "build_irrigation".to_string(),
        },
    );
    assert!(
        !result.is_ok(),
        "build_irrigation should be blocked on mountain"
    );
}

// ── Yield bonuses ──────────────────────────────────────────────

#[test]
fn test_mine_yields_plus_one_shield() {
    let engine = Engine::new_game(&test_config()).unwrap();

    // Set a hill tile without improvement
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(3, 3);
        world.tiles.terrain[idx] = Terrain::Hill;
        world.tiles.improvement[idx] = None;
    }
    let base_yield = engine.calculate_tile_yield(3, 3);

    // Add a mine
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(3, 3);
        world.tiles.improvement[idx] = Some(ImprovementId(1));
    }
    let mine_yield = engine.calculate_tile_yield(3, 3);

    assert_eq!(
        mine_yield.1,
        base_yield.1 + 1,
        "mine should add +1 shield: base={}, mine={}",
        base_yield.1,
        mine_yield.1
    );
    assert_eq!(mine_yield.0, base_yield.0, "mine should not change food");
}

#[test]
fn test_irrigation_yields_plus_one_food() {
    let engine = Engine::new_game(&test_config()).unwrap();

    // Set a grassland tile without improvement
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(3, 3);
        world.tiles.terrain[idx] = Terrain::Grassland;
        world.tiles.improvement[idx] = None;
    }
    let base_yield = engine.calculate_tile_yield(3, 3);

    // Add irrigation
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(3, 3);
        world.tiles.improvement[idx] = Some(ImprovementId(2));
    }
    let irr_yield = engine.calculate_tile_yield(3, 3);

    assert_eq!(
        irr_yield.0,
        base_yield.0 + 1,
        "irrigation should add +1 food: base={}, irrigation={}",
        base_yield.0,
        irr_yield.0
    );
    assert_eq!(
        irr_yield.1, base_yield.1,
        "irrigation should not change shields"
    );
}

// ── Tile.set improvement from Lua ──────────────────────────────

#[test]
fn test_tile_set_improvement_from_lua() {
    let engine = Engine::new_game(&test_config()).unwrap();

    // Verify Tile.set works for improvement
    let result: bool = engine
        .scripts
        .lua
        .load("return Tile.set(4, 4, 'improvement', 1)")
        .eval()
        .unwrap();
    assert!(result, "Tile.set improvement should return true");

    {
        let world = engine.world.borrow();
        let idx = world.tiles.idx(4, 4);
        assert_eq!(world.tiles.improvement[idx], Some(ImprovementId(1)));
    }

    // Set to nil to clear
    let result: bool = engine
        .scripts
        .lua
        .load("return Tile.set(4, 4, 'improvement', nil)")
        .eval()
        .unwrap();
    assert!(result, "Tile.set improvement nil should return true");

    {
        let world = engine.world.borrow();
        let idx = world.tiles.idx(4, 4);
        assert_eq!(world.tiles.improvement[idx], None);
    }
}
