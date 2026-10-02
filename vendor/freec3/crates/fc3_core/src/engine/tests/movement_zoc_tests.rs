use super::*;

#[test]
fn test_multitile_two_grassland_stops_after_movement_exhausted() {
    // Unit with 2 movement (6 internal) on grassland (cost 3 each).
    // Move 4 tiles away. Should move 2 tiles then stop.
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Create a corridor: grassland at x=5, mountains on either side
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        // Grassland corridor at x=5
        for y in 0..10 {
            let idx = world.tiles.idx(5, y);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        }
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 1 });
    // Give it 6 (2 game points)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 6;
        world.units.max_movement[idx] = 6;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 5 },
        },
    );
    assert!(result.is_ok());

    // Should have 2 UnitMoved events (moved 2 tiles, ran out of movement)
    let move_events: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::UnitMoved { .. }))
        .collect();
    assert_eq!(move_events.len(), 2, "should move 2 tiles before stopping");

    // Unit should be at (5,3), 2 tiles from start
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 5, y: 3 });
    assert_eq!(world.units.movement[idx], 0);
}

#[test]
fn test_multitile_road_chain_three_roads_for_one_movement() {
    // Road cost = 1 internal unit. A warrior with 3 movement (1 game point = 3 internal)
    // should traverse 3 road tiles.
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Set road tiles at (4,3), (5,3), (6,3)
    {
        let mut world = engine.world.borrow_mut();
        for x in 4..=6 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.road_level[idx] = 1;
        }
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 3, y: 3 });
    reveal_all(&engine, PlayerId(0));
    // Default warrior: 3 internal movement (1 game point)

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 6, y: 3 },
        },
    );
    assert!(result.is_ok());

    let move_events: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::UnitMoved { .. }))
        .collect();
    assert_eq!(move_events.len(), 3, "should traverse 3 road tiles");

    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 6, y: 3 });
    assert_eq!(world.units.movement[idx], 0); // 3 - 1 - 1 - 1 = 0
}

#[test]
fn test_multitile_events_per_step_with_correct_movement_left() {
    // Verify each UnitMoved event has correct movement_left
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Create a corridor at x=5 to force straight-line path
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for y in 0..10 {
            let idx = world.tiles.idx(5, y);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        }
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 1 });
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 9; // 3 game points
        world.units.max_movement[idx] = 9;
    }

    // Move 3 tiles along y-axis corridor
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 5, y: 4 },
        },
    );
    assert!(result.is_ok());

    let move_events: Vec<_> = result
        .events
        .iter()
        .filter_map(|e| match e {
            Event::UnitMoved {
                movement_left, to, ..
            } => Some((*movement_left, *to)),
            _ => None,
        })
        .collect();

    assert_eq!(move_events.len(), 3);
    // Step 1: (5,1) -> (5,2), cost 3, remaining 6
    assert_eq!(move_events[0], (6, TileCoord { x: 5, y: 2 }));
    // Step 2: (5,2) -> (5,3), cost 3, remaining 3
    assert_eq!(move_events[1], (3, TileCoord { x: 5, y: 3 }));
    // Step 3: (5,3) -> (5,4), cost 3, remaining 0
    assert_eq!(move_events[2], (0, TileCoord { x: 5, y: 4 }));
}

#[test]
fn test_multitile_any_movement_remaining_enters_expensive_tile() {
    // Civ3 rule: if unit has any movement > 0, it can enter any passable tile
    // regardless of cost. Movement goes to 0 (clamped), unit stops.
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Set a hill at (4,3) — costs 6 internal units
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(4, 3);
        world.tiles.terrain[idx] = crate::tile::Terrain::Hill;
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 3, y: 3 });
    // Give unit 1 internal movement (1/3 of a game point) — less than hill cost (6)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 1;
        world.units.max_movement[idx] = 3;
    }

    // Try to move onto the hill
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 4, y: 3 },
        },
    );
    assert!(
        result.is_ok(),
        "should succeed with any movement > 0: {:?}",
        result.errors
    );

    match &result.events[0] {
        Event::UnitMoved {
            movement_left, to, ..
        } => {
            assert_eq!(*to, TileCoord { x: 4, y: 3 });
            assert_eq!(*movement_left, 0); // clamped to 0 (1 - 6 < 0)
        }
        other => panic!("expected UnitMoved, got {:?}", other),
    }

    // Unit should be on the hill
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 4, y: 3 });
    assert_eq!(world.units.movement[idx], 0);
}

#[test]
fn test_multitile_fog_reveals_during_movement() {
    // Unit moves across map. Should emit TilesRevealed events as new tiles come into view.
    let config = GameConfig {
        world: crate::world::WorldConfig {
            width: 20,
            height: 20,
            wrap_x: false,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec![],
        max_turns: None,
    };
    let mut engine = Engine::new_game(&config).unwrap();

    // Spawn unit at (0,0) and set all terrain to grassland for predictability
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Grassland;
        }
    }
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 0 });
    // Initialize visibility from starting position
    engine.update_visibility(PlayerId(0));

    // Give plenty of movement
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 30; // 10 game points
        world.units.max_movement[idx] = 30;
    }

    // Move 5 tiles along y-axis
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: uid,
            destination: TileCoord { x: 0, y: 5 },
        },
    );
    assert!(result.is_ok());

    // Should have TilesRevealed events (as unit moves into previously unseen areas)
    let reveal_events: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::TilesRevealed { .. }))
        .collect();
    assert!(
        !reveal_events.is_empty(),
        "should have at least one TilesRevealed event when moving into unseen territory"
    );

    // Unit should be at destination
    let world = engine.world.borrow();
    let idx = world.units.get(uid).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 0, y: 5 });

    // Tiles around final position should be Visible
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 0, 5),
        crate::tile::Visibility::Visible
    );
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 1, 5),
        crate::tile::Visibility::Visible
    );

    // Tiles around starting position should be Revealed (not Visible) since unit left
    assert_eq!(
        world.tiles.get_visibility(PlayerId(0), 0, 0),
        crate::tile::Visibility::Revealed
    );
}

// ── Movement interrupt tests (Task 7.6) ──────────────────────────

#[test]
fn test_enemy_spotted_interrupt_stops_movement() {
    // Corridor: grassland at y=5, mountains everywhere else. No wrapping.
    // P0 unit at (0,5) with lots of movement.
    // P1 unit at (7,5), not visible to P0 (sight range 1).
    // P0 moves to (9,5). Should stop when enemy comes into sight range.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for x in 0..10 {
            let idx = world.tiles.idx(x, 5);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        }
    }

    let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 5 });
    let _p1_unit = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 7, y: 5 });

    // Reveal the mountain rows adjacent to the corridor (y=4, y=6) for
    // all x values so the pathfinder knows only the corridor at y=5 is
    // passable. Reveal corridor tiles only up to x=4 so the enemy at
    // (7,5) remains undiscovered and can trigger an enemy-spotted interrupt.
    {
        let mut world = engine.world.borrow_mut();
        // Reveal mountain rows y=4 and y=6 across full width
        for x in 0..10u32 {
            world
                .tiles
                .set_visibility(PlayerId(0), x, 4, crate::tile::Visibility::Visible);
            world
                .tiles
                .set_visibility(PlayerId(0), x, 6, crate::tile::Visibility::Visible);
        }
        // Reveal corridor tiles (y=5) only near the start
        for x in 0..5u32 {
            world
                .tiles
                .set_visibility(PlayerId(0), x, 5, crate::tile::Visibility::Visible);
        }
    }

    // Give P0 unit lots of movement
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(p0_unit).unwrap();
        world.units.movement[idx] = 30;
        world.units.max_movement[idx] = 30;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_unit,
            destination: TileCoord { x: 9, y: 5 },
        },
    );
    assert!(
        result.is_ok(),
        "move should succeed; errors: {:?}",
        result.errors
    );

    // Should have some UnitMoved events, then a MoveInterrupted
    let interrupt_events: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::MoveInterrupted { .. }))
        .collect();
    assert_eq!(
        interrupt_events.len(),
        1,
        "should have exactly one MoveInterrupted event; events: {:?}",
        result.events
    );

    match &interrupt_events[0] {
        Event::MoveInterrupted { reason, .. } => {
            assert_eq!(*reason, crate::protocol::MoveInterruptReason::EnemySpotted);
        }
        _ => unreachable!(),
    }

    // Unit should NOT have reached (9,5) — stopped before enemy
    let world = engine.world.borrow();
    let idx = world.units.get(p0_unit).unwrap();
    let unit_x = world.units.position[idx].x;
    assert!(
        unit_x < 7,
        "unit should stop before enemy at x=7, but is at x={unit_x}"
    );
    // Unit should still have movement remaining
    assert!(world.units.movement[idx] > 0, "unit should keep movement");
}

// ── Zone of Control tests (Task 7.7) ─────────────────────────────

#[test]
fn test_zoc_stops_unit_moving_through_enemy_zone() {
    // 1-row corridor at y=3, mountains everywhere else.
    // Enemy at (4,2) — NOT on the corridor, but adjacent to it.
    // ZoC of enemy at (4,2) covers: (3,1),(4,1),(5,1),(3,2),(5,2),(3,3),(4,3),(5,3)
    // So corridor tiles (3,3),(4,3),(5,3) are all in enemy ZoC.
    // Unit moving from (0,3) to (8,3) must pass through consecutive ZoC tiles.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        // 1-row corridor at y=3; enemy on mountain at (4,2) — impassable,
        // so the pathfinder can't route through it to escape ZoC.
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        }
    }

    let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 3 });
    // Enemy on mountain tile — units can exist on any terrain
    let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 4, y: 2 });

    // Reveal all tiles so pathfinder sees real terrain costs
    reveal_all(&engine, PlayerId(0));

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(p0_unit).unwrap();
        world.units.movement[idx] = 60;
        world.units.max_movement[idx] = 60;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_unit,
            destination: TileCoord { x: 8, y: 3 },
        },
    );
    assert!(result.is_ok());

    // Should have MoveInterrupted with ZoneOfControl reason.
    // The only path is through the corridor, and tiles (3,3)→(4,3) are both ZoC.
    let interrupt_events: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::MoveInterrupted { .. }))
        .collect();
    assert_eq!(
        interrupt_events.len(),
        1,
        "should be interrupted by ZoC; events: {:?}",
        result.events
    );

    match &interrupt_events[0] {
        Event::MoveInterrupted { reason, .. } => {
            assert_eq!(*reason, crate::protocol::MoveInterruptReason::ZoneOfControl);
        }
        _ => unreachable!(),
    }

    // Unit should have 0 movement (pinned by ZoC)
    let world = engine.world.borrow();
    let idx = world.units.get(p0_unit).unwrap();
    assert_eq!(world.units.movement[idx], 0, "ZoC should set movement to 0");
}

#[test]
fn test_zoc_allows_entering_zoc_from_non_zoc() {
    // Unit can enter a ZoC tile from a non-ZoC tile without stopping.
    // It only stops if the NEXT step would also be ZoC.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        }
    }

    // Enemy at (5,3). ZoC tiles at (4,3) and (6,3).
    // P0 unit at (0,3) moves to just (4,3) — entering ZoC from non-ZoC should work.
    let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 3 });
    let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 3 });

    reveal_all(&engine, PlayerId(0));

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(p0_unit).unwrap();
        world.units.movement[idx] = 30;
        world.units.max_movement[idx] = 30;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_unit,
            destination: TileCoord { x: 4, y: 3 },
        },
    );
    assert!(result.is_ok());

    // Should NOT be interrupted — we're only entering ZoC, not moving ZoC-to-ZoC
    let interrupts: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::MoveInterrupted { .. }))
        .collect();
    assert!(
        interrupts.is_empty(),
        "entering ZoC from non-ZoC should not interrupt"
    );

    // Unit should be at (4,3)
    let world = engine.world.borrow();
    let idx = world.units.get(p0_unit).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 4, y: 3 });
}

#[test]
fn test_zoc_allows_leaving_zoc_to_non_zoc() {
    // Unit in a ZoC tile can move to a non-ZoC tile (retreating).
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        }
    }

    // Enemy at (3,3). ZoC tile at (2,3). P0 unit starts at (2,3) (in ZoC).
    // Move to (0,3) — leaving ZoC to non-ZoC should be fine.
    let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 2, y: 3 });
    let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 3, y: 3 });

    engine.update_visibility(PlayerId(0));

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(p0_unit).unwrap();
        world.units.movement[idx] = 30;
        world.units.max_movement[idx] = 30;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_unit,
            destination: TileCoord { x: 0, y: 3 },
        },
    );
    assert!(result.is_ok());

    // Should NOT be interrupted — leaving ZoC to non-ZoC is fine
    let interrupts: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::MoveInterrupted { .. }))
        .collect();
    assert!(
        interrupts.is_empty(),
        "leaving ZoC to non-ZoC should not interrupt"
    );

    let world = engine.world.borrow();
    let idx = world.units.get(p0_unit).unwrap();
    assert_eq!(world.units.position[idx], TileCoord { x: 0, y: 3 });
}

#[test]
fn test_zoc_civilians_dont_project() {
    // Enemy settler adjacent — should NOT create ZoC.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        }
    }

    let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 3 });
    // Spawn an enemy settler at (3,3)
    {
        let mut world = engine.world.borrow_mut();
        let settler_id = world.unit_types.get_by_name("settler").unwrap().id;
        let template = world.unit_types.get(settler_id).unwrap().clone();
        world
            .units
            .spawn(settler_id, PlayerId(1), TileCoord { x: 3, y: 3 }, &template);
    }

    engine.update_visibility(PlayerId(0));

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(p0_unit).unwrap();
        world.units.movement[idx] = 30;
        world.units.max_movement[idx] = 30;
    }

    // Move through tiles adjacent to settler — no ZoC should apply
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_unit,
            destination: TileCoord { x: 6, y: 3 },
        },
    );
    assert!(result.is_ok());

    // Should not be interrupted by ZoC (settlers don't project ZoC)
    let zoc_interrupts: Vec<_> = result
        .events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::MoveInterrupted {
                    reason: crate::protocol::MoveInterruptReason::ZoneOfControl,
                    ..
                }
            )
        })
        .collect();
    assert!(zoc_interrupts.is_empty(), "settler should not project ZoC");
}

#[test]
fn test_zoc_multiple_enemies_overlapping() {
    // Two enemy warriors creating overlapping ZoC. 3-row corridor.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for y in 2..=4 {
            for x in 0..10 {
                let idx = world.tiles.idx(x, y);
                world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
            }
        }
    }

    // Two enemies at (4,3) and (6,3)
    let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 3 });
    let _enemy1 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 4, y: 3 });
    let _enemy2 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 6, y: 3 });

    engine.update_visibility(PlayerId(0));

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(p0_unit).unwrap();
        world.units.movement[idx] = 30;
        world.units.max_movement[idx] = 30;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_unit,
            destination: TileCoord { x: 9, y: 3 },
        },
    );
    assert!(result.is_ok());

    // Should be interrupted by ZoC
    let interrupts: Vec<_> = result
        .events
        .iter()
        .filter(|e| matches!(e, Event::MoveInterrupted { .. }))
        .collect();
    assert!(
        !interrupts.is_empty(),
        "should be interrupted by overlapping ZoC; events: {:?}",
        result.events
    );
}

#[test]
fn test_pathfinder_avoids_zoc_when_alternate_route_exists() {
    // 5-row corridor (y=1..=5), enemy at (5,2) on mountain (off-corridor at y=0 row).
    // ZoC tiles on corridor: (4,1),(5,1),(6,1),(4,2),(6,2),(4,3),(5,3),(6,3)
    // With no ZoC awareness, shortest path (0,3)→(9,3) goes straight through ZoC.
    // With ZoC-aware pathfinder, it should route around via y=4 or y=5.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        // Wide corridor: y=1 to y=5
        for y in 1..=5 {
            for x in 0..10 {
                let idx = world.tiles.idx(x, y);
                world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
            }
        }
    }

    let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 3 });
    // Enemy warrior on mountain at (5,0) — adjacent to corridor tile (5,1)
    // ZoC: (4,0),(6,0),(4,1),(5,1),(6,1) — only row y=1 on corridor is affected
    let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 0 });

    // Make everything visible so no EnemySpotted interrupts
    {
        let mut world = engine.world.borrow_mut();
        for y in 0..8 {
            for x in 0..10 {
                world
                    .tiles
                    .set_visibility(PlayerId(0), x, y, crate::tile::Visibility::Visible);
            }
        }
    }

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(p0_unit).unwrap();
        world.units.movement[idx] = 60;
        world.units.max_movement[idx] = 60;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_unit,
            destination: TileCoord { x: 9, y: 3 },
        },
    );
    assert!(result.is_ok());

    // The pathfinder should avoid ZoC — no ZoC interrupts
    let zoc_interrupts: Vec<_> = result
        .events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::MoveInterrupted {
                    reason: crate::protocol::MoveInterruptReason::ZoneOfControl,
                    ..
                }
            )
        })
        .collect();
    assert!(
        zoc_interrupts.is_empty(),
        "pathfinder should route around ZoC; events: {:?}",
        result.events
    );

    // Unit should reach destination (or stop due to movement exhaustion, not ZoC)
    let world = engine.world.borrow();
    let idx = world.units.get(p0_unit).unwrap();
    assert_eq!(
        world.units.position[idx],
        TileCoord { x: 9, y: 3 },
        "unit should reach destination via ZoC-free route"
    );
}

// ── is_zoc_tile direct tests ─────────────────────────────────────

#[test]
fn test_is_zoc_tile_adjacent_to_enemy_warrior() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    // Tile (4,5) is adjacent to enemy at (5,5) — should be ZoC for P0
    assert!(engine.is_zoc_tile(PlayerId(0), TileCoord { x: 4, y: 5 }));
    assert!(engine.is_zoc_tile(PlayerId(0), TileCoord { x: 6, y: 5 }));
    assert!(engine.is_zoc_tile(PlayerId(0), TileCoord { x: 5, y: 4 }));
    // Tile (3,5) is 2 tiles away — NOT ZoC
    assert!(!engine.is_zoc_tile(PlayerId(0), TileCoord { x: 3, y: 5 }));
}

#[test]
fn test_is_zoc_tile_own_unit_does_not_create_zoc() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let _own = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Own unit should NOT create ZoC for self
    assert!(!engine.is_zoc_tile(PlayerId(0), TileCoord { x: 4, y: 5 }));
}

// ── Task 7.9: Naval movement tests ──────────────────────────────

#[test]
fn test_naval_unit_moves_on_ocean() {
    // Galley on ocean should be able to move on ocean tiles.
    // Galley has movement=3, ×3 scale = 9 internal. Ocean costs 3 each.
    // 9 / 3 = 3 tiles moved: (0,3)→(1,3)→(2,3)→(3,3).
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        // Make row 3 ocean
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Ocean;
        }
    }

    let galley = spawn_unit_by_name(&engine, "galley", PlayerId(0), TileCoord { x: 0, y: 3 });
    reveal_all(&engine, PlayerId(0));

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: galley,
            destination: TileCoord { x: 5, y: 3 },
        },
    );
    assert!(result.is_ok(), "galley should move on ocean: {:?}", result);

    let world = engine.world.borrow();
    let idx = world.units.get(galley).unwrap();
    assert_eq!(
        world.units.position[idx],
        TileCoord { x: 3, y: 3 },
        "galley should move 3 tiles (9 movement / 3 cost per ocean tile)"
    );
}

#[test]
fn test_naval_unit_cannot_enter_land() {
    // Galley on ocean cannot move to adjacent grassland tile.
    // Start adjacent so the galley can see the grassland through fog of war.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        // Row 3: ocean, except (5,3) is grassland
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Ocean;
        }
        let idx = world.tiles.idx(5, 3);
        world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
    }

    let galley = spawn_unit_by_name(&engine, "galley", PlayerId(0), TileCoord { x: 4, y: 3 });
    engine.update_visibility(PlayerId(0));

    // Try to move to the grassland tile (5,3) — should be blocked
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: galley,
            destination: TileCoord { x: 5, y: 3 },
        },
    );
    // The pathfinder should fail to find a path since (5,3) is visible and impassable for naval
    let blocked = result
        .events
        .iter()
        .any(|e| matches!(e, Event::MoveBlocked { .. }));
    assert!(
        blocked,
        "galley should not be able to enter land; events: {:?}",
        result.events
    );
}

#[test]
fn test_land_unit_cannot_enter_ocean() {
    // Warrior cannot enter ocean tile.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        // (5,3) is ocean, rest is grassland
        let idx = world.tiles.idx(5, 3);
        world.tiles.terrain[idx] = crate::tile::Terrain::Ocean;
    }

    let warrior = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 3 });
    engine.update_visibility(PlayerId(0));

    // Move to ocean tile — pathfinder routes around it
    let _result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: warrior,
            destination: TileCoord { x: 5, y: 3 },
        },
    );
    // With a single ocean tile on an open map, the pathfinder routes around
    let world = engine.world.borrow();
    let idx = world.units.get(warrior).unwrap();
    // The warrior should NOT be on the ocean tile
    assert_ne!(
        world.units.position[idx],
        TileCoord { x: 5, y: 3 },
        "land unit should not be on ocean"
    );
}

#[test]
fn test_naval_unit_moves_on_coast() {
    // Galley on coast can move to adjacent coast tile.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Coast;
        }
    }

    let galley = spawn_unit_by_name(&engine, "galley", PlayerId(0), TileCoord { x: 0, y: 3 });
    reveal_all(&engine, PlayerId(0));

    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: galley,
            destination: TileCoord { x: 5, y: 3 },
        },
    );
    assert!(result.is_ok(), "galley should move on coast: {:?}", result);

    let world = engine.world.borrow();
    let idx = world.units.get(galley).unwrap();
    // Galley movement=3 ×3 = 9; coast costs 3 each; moves 3 tiles
    assert_eq!(
        world.units.position[idx],
        TileCoord { x: 3, y: 3 },
        "galley should move 3 tiles on coast (9 movement / 3 cost)"
    );
}

// ── Task 7.10: ignore_terrain_cost trait tests ──────────────────

#[test]
fn test_scout_ignore_terrain_cost_forest() {
    // Scout with ignore_terrain_cost treats forest as cost 3 (1 movement point)
    // instead of the normal 6 (2 movement points).
    // 1-row corridor (mountains elsewhere) forces path through forests.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
            world.tiles.vegetation[idx] = crate::tile::Vegetation::Forest;
        }
        // Start tile is grassland (no vegetation) so we can spawn
        let idx = world.tiles.idx(0, 3);
        world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
        world.tiles.vegetation[idx] = crate::tile::Vegetation::None;
    }

    let scout = spawn_unit_by_name(&engine, "scout", PlayerId(0), TileCoord { x: 0, y: 3 });
    reveal_all(&engine, PlayerId(0));

    // Scout has movement=2, so after ×3 scale = 6 internal movement points.
    // With ignore_terrain_cost, forests cost 3 each (not 6).
    // Path: (0,3)→(1,3)→(2,3) costs 3+3=6, movement exhausted at (2,3).
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: scout,
            destination: TileCoord { x: 5, y: 3 },
        },
    );
    assert!(result.is_ok());

    let world = engine.world.borrow();
    let idx = world.units.get(scout).unwrap();
    let pos = world.units.position[idx];
    // Scout moved 2 tiles through forest at cost 3 each = 6 total movement used.
    assert_eq!(
        pos,
        TileCoord { x: 2, y: 3 },
        "scout should move 2 tiles through forest (ignore_terrain_cost: 3 each)"
    );
}

#[test]
fn test_warrior_slowed_by_forest_compared_to_scout() {
    // Warrior (no trait) treats forest as cost 6.
    // 1-row corridor of forest, mountains elsewhere.
    let mut engine = Engine::new_game(&test_config_no_wrap()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        for i in 0..world.tiles.terrain.len() {
            world.tiles.terrain[i] = crate::tile::Terrain::Mountain;
        }
        for x in 0..10 {
            let idx = world.tiles.idx(x, 3);
            world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
            world.tiles.vegetation[idx] = crate::tile::Vegetation::Forest;
        }
    }

    let warrior = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 3 });
    engine.update_visibility(PlayerId(0));

    // Warrior has movement=1, so ×3 = 3 internal movement.
    // Forest costs 6. With "any movement > 0" rule, warrior can enter the first forest
    // at cost 6, movement goes to max(3-6, 0) = 0. So warrior moves just 1 tile.
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: warrior,
            destination: TileCoord { x: 5, y: 3 },
        },
    );
    assert!(result.is_ok());

    let world = engine.world.borrow();
    let idx = world.units.get(warrior).unwrap();
    let warrior_pos = world.units.position[idx];
    assert_eq!(
            warrior_pos,
            TileCoord { x: 1, y: 3 },
            "warrior should only move 1 tile into forest (cost 6 > movement 3, but any-movement rule allows it)"
        );
}
