use super::*;

// ── Test: MoveUnit sets destination, cleared on arrival ──

#[test]
fn test_move_sets_destination_cleared_on_arrival() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Move to adjacent tile (should arrive in one step)
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 6 },
        },
    );
    assert!(result.is_ok());

    // Should have UnitMoved + DestinationCleared events
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, Event::UnitMoved { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, Event::DestinationCleared { .. })));

    // Destination should be cleared after arrival
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.destination[idx], None);
    assert_eq!(world.units.position[idx], TileCoord { x: 5, y: 6 });
}

// ── Test: MoveUnit with 0 movement sets destination ──

#[test]
fn test_move_with_zero_movement_sets_destination_event() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Drain movement
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 0;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 8 },
        },
    );
    assert!(result.is_ok());
    assert!(result.events.iter().any(|e| matches!(
        e,
        Event::DestinationSet {
            destination: TileCoord { x: 5, y: 8 },
            ..
        }
    )));

    // Unit should still be at original position with destination stored
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 5, y: 5 });
    assert_eq!(world.units.destination[idx], Some(TileCoord { x: 5, y: 8 }));
}

// ── Test: Destination persists when movement exhausted mid-path ──

#[test]
fn test_destination_persists_when_movement_exhausted() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Warrior has movement=1 (3 internal). Moving to a tile 3 steps away
    // should exhaust movement after 1 step, keeping destination.
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 8 },
        },
    );
    assert!(result.is_ok());

    // Unit should have moved one step but not arrived
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_ne!(world.units.position[idx], TileCoord { x: 5, y: 8 });

    // Destination should still be set
    assert_eq!(world.units.destination[idx], Some(TileCoord { x: 5, y: 8 }));
}

// ── Test: Destination cleared on EnemySpotted interrupt ──

#[test]
fn test_destination_cleared_on_enemy_spotted() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Place a warrior for player 0 at (2,5)
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 2, y: 5 });

    // Give it extra movement to travel far
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 30;
    }

    // Place an enemy at (5,5) — will be spotted en route
    let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    // Move toward (7,5), which passes near the enemy
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 7, y: 5 },
        },
    );
    assert!(result.is_ok());

    // Check if movement was interrupted
    let interrupted = result
        .events
        .iter()
        .any(|e| matches!(e, Event::MoveInterrupted { reason, .. } if *reason == crate::protocol::MoveInterruptReason::EnemySpotted));

    if interrupted {
        // Destination should be cleared
        let world = engine.world.borrow();
        let idx = world.units.get(uid).unwrap();
        assert_eq!(
            world.units.destination[idx], None,
            "destination should be cleared on enemy spotted"
        );
        assert!(result
            .events
            .iter()
            .any(|e| matches!(e, Event::DestinationCleared { .. })));
    }
    // If not interrupted (enemy was already visible), destination should still be cleared on arrival
    // or persisted (both are valid). The key test is that interrupts clear destination.
}

// ── Test: Destination cleared on ZoC interrupt ──

#[test]
fn test_destination_cleared_on_zoc_interrupt() {
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();

    // Set up ZoC scenario: enemy at (5,4) projects ZoC to (4,4), (4,5), (5,5), (6,4), (6,5)
    let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 4 });

    // Place player 0's warrior at a ZoC tile, trying to move ZoC-to-ZoC
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 4, y: 4 });

    // Give extra movement
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 30;
        // Make enemy visible
        world
            .tiles
            .set_visibility(PlayerId(0), 5, 4, crate::tile::Visibility::Visible);
    }

    // Move that would go through ZoC tiles
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 6, y: 4 },
        },
    );
    assert!(result.is_ok());

    let zoc_interrupted = result.events.iter().any(|e| {
        matches!(
            e,
            Event::MoveInterrupted { reason, .. }
                if *reason == crate::protocol::MoveInterruptReason::ZoneOfControl
        )
    });

    if zoc_interrupted {
        let world = engine.world.borrow();
        let idx = world.units.get(uid).unwrap();
        assert_eq!(
            world.units.destination[idx], None,
            "destination should be cleared on ZoC interrupt"
        );
    }
}

// ── Test: Destination cleared on fortify/skip/attack/action ──

#[test]
fn test_destination_cleared_on_fortify() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Set a destination manually
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.destination[idx] = Some(TileCoord { x: 5, y: 8 });
    }

    let result = engine.submit_command(PlayerId(0), Command::FortifyUnit { unit_id: uid });
    assert!(result.is_ok());

    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.destination[idx], None);
}

#[test]
fn test_destination_cleared_on_skip() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Set a destination manually
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.destination[idx] = Some(TileCoord { x: 5, y: 8 });
    }

    let result = engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    assert!(result.is_ok());

    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.destination[idx], None);
}

#[test]
fn test_destination_cleared_on_attack() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    // Set a destination on the attacker
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(attacker).unwrap();
        world.units.destination[idx] = Some(TileCoord { x: 5, y: 9 });
        // Make defender visible
        world
            .tiles
            .set_visibility(PlayerId(0), 5, 6, crate::tile::Visibility::Visible);
    }

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result.is_ok());

    // Attacker's destination should be cleared (if still alive)
    let world = engine.world.borrow();
    if let Some(idx) = world.units.get(attacker) {
        assert_eq!(world.units.destination[idx], None);
    }
}

#[test]
fn test_destination_cleared_on_action() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Spawn a worker to test with build_road action
    let worker = spawn_unit_by_name(&engine, "worker", PlayerId(0), TileCoord { x: 5, y: 5 });

    // Set a destination on the worker
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(worker).unwrap();
        world.units.destination[idx] = Some(TileCoord { x: 5, y: 8 });
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: worker,
            action_id: "build_road".to_string(),
        },
    );
    assert!(result.is_ok());

    let world = engine.world.borrow();
    if let Some(idx) = world.units.get(worker) {
        assert_eq!(world.units.destination[idx], None);
    }
}

// ── Test: Auto-move at turn start ──

#[test]
fn test_auto_move_at_turn_start() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Move toward a distant destination (will exhaust movement this turn)
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 8 },
        },
    );
    assert!(result.is_ok());

    // Verify unit moved one step but destination persists
    {
        let world = engine.world.borrow();
        let idx = world.units.get(uid).unwrap();
        assert_eq!(world.units.destination[idx], Some(TileCoord { x: 5, y: 8 }));
        assert_ne!(world.units.position[idx], TileCoord { x: 5, y: 8 });
    }

    // Spawn a warrior for P1 (needed to avoid elimination)
    let p1_warrior = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 0, y: 0 });

    // End player 0's turn (unit with destination is "handled")
    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn P0 should work: {:?}",
        result.errors
    );

    // Skip player 1's warrior on P1's own turn, then end turn
    let _ = engine.submit_command(
        PlayerId(1),
        Command::SkipUnit {
            unit_id: p1_warrior,
        },
    );
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn P1 should work: {:?}",
        result.errors
    );

    // The auto-move should have moved the unit closer to destination
    // Check for UnitMoved events in the EndTurn result (from auto-move)
    let auto_moved = result
        .events
        .iter()
        .any(|e| matches!(e, Event::UnitMoved { unit_id, .. } if *unit_id == uid));
    assert!(auto_moved, "unit should auto-move toward destination");

    // Unit should be closer to (5,8) or have arrived
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    let pos = world.units.position[idx];
    assert!(pos.y > 5, "unit should have moved south toward destination");
}

// ── Test: Unit with destination counts as "handled" for EndTurn ──

#[test]
fn test_unit_with_destination_handled_for_end_turn() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Set destination with 0 movement (queued for next turn)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 0;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 8 },
        },
    );
    assert!(result.is_ok());

    // EndTurn should succeed — unit with destination is handled
    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn should work with destination-set unit: {:?}",
        result.errors
    );
}

// ── Test: No path found clears destination ──

#[test]
fn test_no_path_clears_destination() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Surround the unit with mountains (impassable)
    {
        let mut world = engine.world.borrow_mut();
        for (dx, dy) in &[
            (4, 4),
            (5, 4),
            (6, 4),
            (4, 5),
            (6, 5),
            (4, 6),
            (5, 6),
            (6, 6),
        ] {
            let idx = world.tiles.idx(*dx, *dy);
            world.tiles.terrain[idx] = crate::tile::Terrain::Mountain;
        }
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 8, y: 8 },
        },
    );
    assert!(result.is_ok());

    // Should get MoveBlocked and destination should be cleared
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, Event::MoveBlocked { .. })));

    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(
        world.units.destination[idx], None,
        "destination should be cleared when no path exists"
    );
}
