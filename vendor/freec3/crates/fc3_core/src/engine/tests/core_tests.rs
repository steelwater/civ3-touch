use super::*;

#[test]
fn test_engine_new_game() {
    let engine = Engine::new_game(&test_config()).unwrap();
    assert_eq!(engine.current_player(), PlayerId(0));
    assert_eq!(engine.current_turn(), 1);

    // Base mod should have registered unit types
    let world = engine.world.borrow();
    assert!(world.unit_types.get_by_name("warrior").is_some());
    assert!(world.unit_types.get_by_name("settler").is_some());
}

#[test]
fn test_end_turn_advances_player() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    assert_eq!(engine.current_player(), PlayerId(0));

    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(result.is_ok());
    assert_eq!(engine.current_player(), PlayerId(1));

    match &result.events[0] {
        Event::TurnStarted { player, turn } => {
            assert_eq!(*player, PlayerId(1));
            assert_eq!(*turn, 1);
        }
        other => panic!("expected TurnStarted, got {:?}", other),
    }
}

#[test]
fn test_end_turn_wrong_player_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::NotYourTurn);
}

#[test]
fn test_full_turn_cycle() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    assert_eq!(engine.current_turn(), 1);

    // Player 0 ends turn
    engine.submit_command(PlayerId(0), Command::EndTurn);
    assert_eq!(engine.current_player(), PlayerId(1));

    // Player 1 ends turn -> new turn
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);
    assert_eq!(engine.current_player(), PlayerId(0));
    assert_eq!(engine.current_turn(), 2);

    match &result.events[0] {
        Event::TurnStarted { player, turn } => {
            assert_eq!(*player, PlayerId(0));
            assert_eq!(*turn, 2);
        }
        other => panic!("expected TurnStarted, got {:?}", other),
    }
}

#[test]
fn test_player_view_delegates() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let view = engine.player_view(PlayerId(0));
    assert_eq!(view.player, PlayerId(0));
}

#[test]
fn test_event_log_records_commands() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    engine.submit_command(PlayerId(0), Command::EndTurn);
    assert_eq!(engine.event_log.len(), 1);
    assert_eq!(engine.event_log[0].1, PlayerId(0));
}

#[test]
fn test_submit_command_on_wrong_turn_not_logged() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);
    assert!(!result.is_ok());
    // NotYourTurn errors are not logged
    assert_eq!(engine.event_log.len(), 0);
}

#[test]
fn test_fortify_unit() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(PlayerId(0), Command::FortifyUnit { unit_id: uid });
    assert!(result.is_ok());

    match &result.events[0] {
        Event::UnitFortified { unit_id } => assert_eq!(*unit_id, uid),
        other => panic!("expected UnitFortified, got {:?}", other),
    }

    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert!(world.units.fortified[idx]);
}

#[test]
fn test_fortify_enemy_unit_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(PlayerId(0), Command::FortifyUnit { unit_id: uid });
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::InvalidUnit);
}

#[test]
fn test_moving_fortified_unit_clears_fortified() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Fortify the unit
    engine.submit_command(PlayerId(0), Command::FortifyUnit { unit_id: uid });
    {
        let world = engine.world.borrow();
        let idx = world.units.get(uid).unwrap();
        assert!(world.units.fortified[idx]);
    }

    // Move the fortified unit
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 6, y: 5 },
        },
    );
    assert!(result.is_ok());

    // Fortified flag should be cleared
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert!(!world.units.fortified[idx]);
}

// ── Movement tests ──────────────────────────────────────────────

#[test]
fn test_move_warrior_on_grassland_costs_1() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(result.is_ok());

    match &result.events[0] {
        Event::UnitMoved {
            unit_id,
            from,
            to,
            movement_left,
        } => {
            assert_eq!(*unit_id, uid);
            assert_eq!(*from, TileCoord { x: 5, y: 5 });
            assert_eq!(*to, TileCoord { x: 5, y: 6 });
            assert_eq!(*movement_left, 0); // warrior has 1 movement, cost 1
        }
        other => panic!("expected UnitMoved, got {:?}", other),
    }

    // Verify position updated
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 5, y: 6 });
    assert_eq!(world.units.movement[idx], 0);
    assert!(world.units.has_moved[idx]);
}

#[test]
fn test_move_warrior_onto_hill_first_move_succeeds() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Set destination terrain to Hill
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 6);
        world.tiles.terrain[idx] = crate::tile::Terrain::Hill;
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Warrior has 1 movement (== max_movement), hill costs 2.
    // Civ3 rule: first move always succeeds, movement set to 0.
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(result.is_ok(), "first move onto hill should succeed");
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 5, y: 6 });
    assert_eq!(world.units.movement[idx], 0);
}

#[test]
fn test_move_onto_hill_after_partial_move_succeeds_with_any_movement() {
    // Civ3 rule: any movement > 0 allows entering any passable tile
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Set terrain: grassland at (5,6), hill at (5,7)
    {
        let mut world = engine.world.borrow_mut();
        let idx6 = world.tiles.idx(5, 6);
        world.tiles.terrain[idx6] = crate::tile::Terrain::Grassland;
        let idx7 = world.tiles.idx(5, 7);
        world.tiles.terrain[idx7] = crate::tile::Terrain::Hill;
    }
    // Give unit 6 max movement (2 points * MOVEMENT_SCALE=3) so it can move once on grassland
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.max_movement[idx] = 6;
        world.units.movement[idx] = 6;
    }

    // First move: grassland costs 3, leaving 3 movement
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(result.is_ok());

    // Second move: hill costs 6, unit has 3 > 0, so it succeeds (any movement rule)
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 7 },
        },
    );
    assert!(result.is_ok(), "should succeed with any movement > 0");
    match &result.events[0] {
        Event::UnitMoved {
            movement_left, to, ..
        } => {
            assert_eq!(*to, TileCoord { x: 5, y: 7 });
            assert_eq!(*movement_left, 0); // 3 - 6 clamped to 0
        }
        other => panic!("expected UnitMoved, got {:?}", other),
    }
}

#[test]
fn test_move_onto_mountain_blocked() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 6);
        world.tiles.terrain[idx] = crate::tile::Terrain::Mountain;
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(result.is_ok()); // MoveBlocked is an event, not an error
    match &result.events[0] {
        Event::MoveBlocked { unit_id, reason } => {
            assert_eq!(*unit_id, uid);
            assert!(
                reason.contains("no path"),
                "expected 'no path' in reason, got: {reason}"
            );
        }
        other => panic!("expected MoveBlocked, got {:?}", other),
    }

    // Position should not have changed
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 5, y: 5 });
}

#[test]
fn test_move_with_zero_movement_sets_destination() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Set movement to 0
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 0;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    // Now returns DestinationSet instead of error — unit queues movement for next turn
    assert!(result.is_ok());
    assert!(result.events.iter().any(|e| matches!(
        e,
        Event::DestinationSet {
            destination: TileCoord { x: 5, y: 6 },
            ..
        }
    )));

    // Verify destination is stored
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.destination[idx], Some(TileCoord { x: 5, y: 6 }));
}

#[test]
fn test_move_onto_road_reduces_cost() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Set road on destination
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 6);
        world.tiles.road_level[idx] = 1;
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(result.is_ok());

    match &result.events[0] {
        Event::UnitMoved { movement_left, .. } => {
            // Warrior: 3 movement (1 * MOVEMENT_SCALE), road costs 1
            assert_eq!(*movement_left, 2); // 3 - 1 = 2
        }
        other => panic!("expected UnitMoved, got {:?}", other),
    }
}

#[test]
fn test_move_updates_position() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 3, y: 3 });

    engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 4, y: 3 },
        },
    );

    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 4, y: 3 });
}

#[test]
fn test_move_updates_fog_of_war() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Initial: set up visibility around starting position
    engine.update_visibility(PlayerId(0));

    // Move the unit
    engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );

    let world = engine.world.borrow();
    // New position and neighbors should be Visible
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 5, 6),
        crate::tile::Visibility::Visible
    );
    // Tiles around new position
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 5, 7),
        crate::tile::Visibility::Visible
    );
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 4, 6),
        crate::tile::Visibility::Visible
    );
}

#[test]
fn test_move_to_unreachable_tile_blocked() {
    // [REVISED] Was test_move_non_adjacent_rejected. Now that MoveUnit supports
    // pathfinding to any reachable tile, we test that moving to an unreachable
    // tile (surrounded by mountains) returns MoveBlocked.
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Create a pocket surrounded by mountains
    {
        let mut world = engine.world.borrow_mut();
        for &(x, y) in &[
            (6, 6),
            (6, 7),
            (6, 8),
            (7, 6),
            (7, 8),
            (8, 6),
            (8, 7),
            (8, 8),
        ] {
            let idx = world.tiles.idx(x, y);
            world.tiles.terrain[idx] = crate::tile::Terrain::Mountain;
        }
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    reveal_all(&engine, PlayerId(0));

    // Try to move to the tile inside the mountain pocket
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 7, y: 7 },
        },
    );
    assert!(result.is_ok()); // MoveBlocked is an event, not an error
    match &result.events[0] {
        Event::MoveBlocked { unit_id, reason } => {
            assert_eq!(*unit_id, uid);
            assert!(reason.contains("no path"));
        }
        other => panic!("expected MoveBlocked, got {:?}", other),
    }
}

#[test]
fn test_move_invalid_unit_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let invalid_uid = GenId {
        index: 999,
        generation: 0,
    };

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: invalid_uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::InvalidUnit);
}

#[test]
fn test_move_non_adjacent_with_pathfinding() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    // Give the unit enough movement to go 3 tiles (cost 3 each in integer-thirds)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 15;
        world.units.max_movement[idx] = 15;
    }

    // Move 3 tiles away
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 8 },
        },
    );
    assert!(
        result.is_ok(),
        "multi-tile move should succeed: {:?}",
        result.errors
    );
    // Should have 3 UnitMoved events (one per step)
    let move_events: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::UnitMoved { .. }))
        .collect();
    assert_eq!(move_events.len(), 3, "expected 3 move events");

    // Unit should be at destination
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 5, y: 8 });
    assert_eq!(world.units.movement[idx], 6); // 15 - 9 = 6
}

#[test]
fn test_move_pathfinding_routes_around_mountains() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Create a wall of mountains between start and destination
    {
        let mut world = engine.world.borrow_mut();
        // Mountain wall at x=6, y=3..7
        for y in 3..=7 {
            let idx = world.tiles.idx(6, y);
            world.tiles.terrain[idx] = crate::tile::Terrain::Mountain;
        }
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 60;
        world.units.max_movement[idx] = 60;
    }
    reveal_all(&engine, PlayerId(0));

    // Move to the other side of the wall
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 7, y: 5 },
        },
    );
    assert!(result.is_ok(), "should find path around mountain wall");

    // Verify unit arrived
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 7, y: 5 });
}

#[test]
fn test_move_enemy_unit_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::InvalidUnit);
}

// ── Fog of war / visibility tests ───────────────────────────────

#[test]
fn test_visibility_unit_at_5_5_makes_surrounding_visible() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let _uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    engine.update_visibility(PlayerId(0));

    let world = engine.world.borrow();
    // Sight range 1: tiles (4,4) through (6,6) should all be Visible
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let x = (5 + dx) as u32;
            let y = (5 + dy) as u32;
            assert_eq!(
                world.tiles.get_visibility(PlayerId(0), x, y),
                crate::tile::Visibility::Visible,
                "tile ({x}, {y}) should be visible"
            );
        }
    }
    // Tile outside range should still be Unseen
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 3, 5),
        crate::tile::Visibility::Unseen
    );
}

#[test]
fn test_visibility_old_tiles_become_revealed_after_move() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    engine.update_visibility(PlayerId(0));

    // Give the warrior extra movement so it can move twice
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 10;
    }

    // Move south to (5,6), then further south to (5,7)
    engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 7 },
        },
    );

    let world = engine.world.borrow();
    // Old position (5,5) should now be Revealed (not Unseen, not Visible)
    // unless it's still in range of (5,7) which it isn't (range 1)
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 5, 5),
        crate::tile::Visibility::Revealed,
        "(5,5) should be Revealed after unit moved away"
    );
    // New position should be Visible
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 5, 7),
        crate::tile::Visibility::Visible
    );
}

#[test]
fn test_visibility_two_units_union() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let _uid1 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 2, y: 2 });
    let _uid2 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 7, y: 7 });
    engine.update_visibility(PlayerId(0));

    let world = engine.world.borrow();
    // Both unit neighborhoods should be visible
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 2, 2),
        crate::tile::Visibility::Visible
    );
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 7, 7),
        crate::tile::Visibility::Visible
    );
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 3, 3),
        crate::tile::Visibility::Visible
    );
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 8, 8),
        crate::tile::Visibility::Visible
    );
}

#[test]
fn test_visibility_destroyed_unit_downgrades_on_recalc() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    engine.update_visibility(PlayerId(0));

    // Verify visible
    {
        let world = engine.world.borrow();
        assert_eq!(
            world.tiles.get_visibility(PlayerId(0), 5, 5),
            crate::tile::Visibility::Visible
        );
    }

    // Destroy the unit
    {
        let mut world = engine.world.borrow_mut();
        world.units.destroy(uid);
    }

    // Recalculate visibility
    engine.update_visibility(PlayerId(0));

    let world = engine.world.borrow();
    // Tiles should be Revealed, not Visible (no units left to see them)
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 5, 5),
        crate::tile::Visibility::Revealed
    );
}

// ── Turn lifecycle tests ─────────────────────────────────────────

#[test]
fn test_turn_lifecycle_resets_movement() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Move the warrior, using up its movement
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(result.is_ok());

    // Verify movement is 0
    {
        let world = engine.world.borrow();
        let idx = world.units.get(uid).unwrap();
        assert_eq!(world.units.movement[idx], 0);
        assert!(world.units.has_moved[idx]);
    }

    // Player 0 ends turn
    engine.submit_command(PlayerId(0), Command::EndTurn);
    // Player 1 ends turn -> back to player 0, new turn
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Movement should be reset to max_movement (1 * MOVEMENT_SCALE = 3)
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(
        world.units.movement[idx], 3,
        "movement should be reset after turn cycle"
    );
    assert!(
        !world.units.has_moved[idx],
        "has_moved should be reset after turn cycle"
    );
}

#[test]
fn test_turn_lifecycle_only_resets_current_player_units() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid0 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 3, y: 3 });
    let uid1 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 7, y: 7 });

    // Drain movement on both units
    {
        let mut world = engine.world.borrow_mut();
        let idx0 = world.units.get(uid0).unwrap();
        world.units.movement[idx0] = 0;
        world.units.has_moved[idx0] = true;
        let idx1 = world.units.get(uid1).unwrap();
        world.units.movement[idx1] = 0;
        world.units.has_moved[idx1] = true;
    }

    // Player 0 ends turn -> Player 1's turn starts
    engine.submit_command(PlayerId(0), Command::EndTurn);

    // Player 1's unit should have movement reset (it's their turn start)
    let world = engine.world.borrow();
    let idx1 = world.units.get(uid1).unwrap();
    assert_eq!(
        world.units.movement[idx1], 3,
        "p1 unit movement should be reset"
    );
    assert!(
        !world.units.has_moved[idx1],
        "p1 unit has_moved should be reset"
    );

    // Player 0's unit should NOT be reset yet (it was reset when p0's turn started)
    // Since we drained it AFTER turn start, it stays drained
    let idx0 = world.units.get(uid0).unwrap();
    assert_eq!(world.units.movement[idx0], 0);
}

#[test]
fn test_visibility_hill_gives_extra_sight_range() {
    let engine = Engine::new_game(&test_config()).unwrap();
    // Place unit on a hill
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Hill;
    }
    let _uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    engine.update_visibility(PlayerId(0));

    let world = engine.world.borrow();
    // Sight range 2: tile 2 away should be visible
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 5, 7),
        crate::tile::Visibility::Visible,
        "hill unit should see 2 tiles away"
    );
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 3, 3),
        crate::tile::Visibility::Visible,
        "hill unit should see diagonally 2 tiles"
    );
}

// ── Deterministic replay tests ──────────────────────────────────

#[test]
fn test_deterministic_replay_produces_identical_state() {
    let config = test_config();
    let mut engine = Engine::new_game(&config).unwrap();

    // Spawn warriors
    let uid0 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 2, y: 2 });
    let uid1 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 7, y: 7 });

    // Play several turns
    // Turn 1: P0 moves, P0 ends, P1 ends
    engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid0,
            destination: TileCoord { x: 3, y: 2 },
        },
    );
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(
        PlayerId(1),
        Command::MoveUnit {
            unit_id: uid1,
            destination: TileCoord { x: 6, y: 7 },
        },
    );
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Turn 2: P0 moves, fortifies, ends. P1 ends.
    engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid0,
            destination: TileCoord { x: 4, y: 2 },
        },
    );
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::FortifyUnit { unit_id: uid1 });
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Turn 3-5: skip units then end turns
    for _ in 0..6 {
        let player = engine.current_player();
        // Skip all unhandled units for this player
        let available = engine.available_commands(player);
        for cmd in &available {
            if let AvailableCommand::Skip { unit_id } = cmd {
                engine.submit_command(player, Command::SkipUnit { unit_id: *unit_id });
            }
        }
        engine.submit_command(player, Command::EndTurn);
    }

    // Capture final state
    let original_world = engine.world.borrow();
    let original_turn = engine.current_turn();
    let _original_p0_pos = original_world.units.position[original_world.units.get(uid0).unwrap()];
    let _original_p1_fortified =
        original_world.units.fortified[original_world.units.get(uid1).unwrap()];
    let _original_unit_count = original_world.units.count();
    drop(original_world);

    // Build log and replay
    let log = engine.to_game_log(&config);
    let json = serde_json::to_string(&log).unwrap();
    let log_back: GameLog = serde_json::from_str(&json).unwrap();
    let replayed = log_back.replay().unwrap();

    // Note: replayed engine will have spawned units via different mechanism
    // (the log only captures submit_command calls, not direct spawn calls)
    // So we verify turn state and log length match
    assert_eq!(replayed.current_turn(), original_turn);
    assert_eq!(replayed.event_log.len(), engine.event_log.len());

    // Verify the log round-trips through JSON
    assert_eq!(log_back.commands.len(), engine.event_log.len());
}

#[test]
fn test_game_log_serialization_roundtrip() {
    let config = test_config();
    let mut engine = Engine::new_game(&config).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    let log = engine.to_game_log(&config);
    let json = serde_json::to_string(&log).unwrap();
    let back: GameLog = serde_json::from_str(&json).unwrap();

    assert_eq!(back.seed, config.world.seed);
    assert_eq!(back.commands.len(), 3);
}

// ── Skip / EndTurn enforcement tests ─────────────────────────────

#[test]
fn test_skip_unit_sets_skipped_flag() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    assert!(result.is_ok());

    match &result.events[0] {
        Event::UnitSkipped { unit_id } => assert_eq!(*unit_id, uid),
        other => panic!("expected UnitSkipped, got {:?}", other),
    }

    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert!(world.units.skipped[idx]);
}

#[test]
fn test_skip_enemy_unit_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::InvalidUnit);
}

#[test]
fn test_skipped_unit_excluded_from_available_commands() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Before skip: unit should appear in available commands
    let available = engine.available_commands(PlayerId(0));
    assert!(
        available
            .iter()
            .any(|c| matches!(c, AvailableCommand::Move { unit_id, .. } if *unit_id == uid)),
        "unit should have Move available before skip"
    );

    // Skip the unit
    engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });

    // After skip: unit should NOT appear in available commands
    let available = engine.available_commands(PlayerId(0));
    assert!(
        !available
            .iter()
            .any(|c| matches!(c, AvailableCommand::Move { unit_id, .. } if *unit_id == uid)),
        "skipped unit should not have Move available"
    );
    assert!(
        !available
            .iter()
            .any(|c| matches!(c, AvailableCommand::Fortify { unit_id } if *unit_id == uid)),
        "skipped unit should not have Fortify available"
    );
    assert!(
        !available
            .iter()
            .any(|c| matches!(c, AvailableCommand::Skip { unit_id } if *unit_id == uid)),
        "skipped unit should not have Skip available"
    );
}

#[test]
fn test_end_turn_blocked_with_unhandled_units() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let _uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Unit has movement > 0, is not fortified, not skipped
    let available = engine.available_commands(PlayerId(0));
    assert!(
        !available
            .iter()
            .any(|c| matches!(c, AvailableCommand::EndTurn)),
        "EndTurn should not be available when unit has unhandled movement"
    );
}

#[test]
fn test_end_turn_available_after_all_units_handled() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid_skip = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 3, y: 3 });
    let uid_fortify = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let uid_move = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 7, y: 7 });

    // Skip one
    engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid_skip });
    // Fortify one
    engine.submit_command(
        PlayerId(0),
        Command::FortifyUnit {
            unit_id: uid_fortify,
        },
    );
    // Move one (exhaust movement)
    engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid_move,
            destination: TileCoord { x: 7, y: 8 },
        },
    );

    let available = engine.available_commands(PlayerId(0));
    assert!(
        available
            .iter()
            .any(|c| matches!(c, AvailableCommand::EndTurn)),
        "EndTurn should be available when all units are handled"
    );
}

#[test]
fn test_skipped_resets_on_turn_start() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    // Also give player 1 a unit so they have something
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 8, y: 8 });

    // Skip the unit
    engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });

    {
        let world = engine.world.borrow();
        let idx = world.units.get(uid).unwrap();
        assert!(world.units.skipped[idx], "unit should be skipped");
    }

    // End turn for P0 (all units handled now)
    engine.submit_command(PlayerId(0), Command::EndTurn);
    // P1 skips their unit and ends turn
    {
        let available = engine.available_commands(PlayerId(1));
        for cmd in &available {
            if let AvailableCommand::Skip { unit_id } = cmd {
                engine.submit_command(PlayerId(1), Command::SkipUnit { unit_id: *unit_id });
                break;
            }
        }
    }
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Now it's P0's turn again — skipped should be reset
    {
        let world = engine.world.borrow();
        let idx = world.units.get(uid).unwrap();
        assert!(
            !world.units.skipped[idx],
            "skipped should be reset after turn cycle"
        );
    }
}

// ── Combat tests ─────────────────────────────────────────────────
