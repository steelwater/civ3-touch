//! Integration test: programmatic game session.
//!
//! Tests the full game loop through the Engine API: spawn units, move,
//! block on insufficient movement, end turns, verify resets, move again.

use fc3_core::engine::{Engine, GameConfig};
use fc3_core::id::GenId;
use fc3_core::protocol::{Command, Event};
use fc3_core::types::{PlayerId, TileCoord};
use fc3_core::world::WorldConfig;

fn game_config() -> GameConfig {
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

/// Spawn a warrior for a player at the given position.
fn spawn_warrior(engine: &Engine, owner: PlayerId, pos: TileCoord) -> GenId {
    let mut world = engine.world().borrow_mut();
    let wid = world.unit_types.get_by_name("warrior").unwrap().id;
    let template = world.unit_types.get(wid).unwrap().clone();
    world.units.spawn(wid, owner, pos, &template)
}

#[test]
fn test_programmatic_game() {
    let mut engine = Engine::new_game(&game_config()).unwrap();

    // ── Spawn warriors ──
    let p0_warrior = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 0 });
    let p1_warrior = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 9, y: 9 });

    // ── Player 0: move to (1,0) ──
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_warrior,
            destination: TileCoord { x: 1, y: 0 },
        },
    );
    assert!(result.is_ok(), "move should succeed");
    match &result.events[0] {
        Event::UnitMoved {
            unit_id,
            from,
            to,
            movement_left,
        } => {
            assert_eq!(*unit_id, p0_warrior);
            assert_eq!(*from, TileCoord { x: 0, y: 0 });
            assert_eq!(*to, TileCoord { x: 1, y: 0 });
            assert_eq!(*movement_left, 0);
        }
        other => panic!("expected UnitMoved, got {:?}", other),
    }

    // ── Player 0: move to (2,0) — now sets destination (queued for next turn) ──
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_warrior,
            destination: TileCoord { x: 2, y: 0 },
        },
    );
    assert!(result.is_ok(), "0-movement move now sets destination");
    // Clear destination so it doesn't affect the rest of the test
    {
        let mut world = engine.world().borrow_mut();
        let idx = world.units.get(p0_warrior).unwrap();
        world.units.destination[idx] = None;
    }

    // ── Player 0: EndTurn ──
    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(result.is_ok());
    match &result.events[0] {
        Event::TurnStarted { player, turn } => {
            assert_eq!(*player, PlayerId(1));
            assert_eq!(*turn, 1);
        }
        other => panic!("expected TurnStarted, got {:?}", other),
    }

    // ── Player 1: skip warrior, then EndTurn — wraps to new turn ──
    engine.submit_command(
        PlayerId(1),
        Command::SkipUnit {
            unit_id: p1_warrior,
        },
    );
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);
    assert!(result.is_ok());
    match &result.events[0] {
        Event::TurnStarted { player, turn } => {
            assert_eq!(*player, PlayerId(0));
            assert_eq!(*turn, 2);
        }
        other => panic!("expected TurnStarted, got {:?}", other),
    }

    // ── Player 0: verify movement is reset ──
    {
        let world = engine.world().borrow();
        let idx = world.units.get(p0_warrior).unwrap();
        // Movement uses integer-thirds: 1 movement point = 3 internal units
        assert_eq!(
            world.units.movement[idx], 3,
            "movement should be reset to max_movement (1 * MOVEMENT_SCALE = 3)"
        );
        assert!(!world.units.has_moved[idx], "has_moved should be reset");
    }

    // ── Player 0: move to (2,0) — now works ──
    let result = engine.submit_command(
        PlayerId(0),
        Command::MoveUnit {
            unit_id: p0_warrior,
            destination: TileCoord { x: 2, y: 0 },
        },
    );
    assert!(result.is_ok(), "move should succeed after turn reset");
    match &result.events[0] {
        Event::UnitMoved { to, .. } => {
            assert_eq!(*to, TileCoord { x: 2, y: 0 });
        }
        other => panic!("expected UnitMoved, got {:?}", other),
    }

    // ── Verify final positions ──
    {
        let world = engine.world().borrow();
        let idx0 = world.units.get(p0_warrior).unwrap();
        assert_eq!(world.units.position[idx0], TileCoord { x: 2, y: 0 });

        let idx1 = world.units.get(p1_warrior).unwrap();
        assert_eq!(world.units.position[idx1], TileCoord { x: 9, y: 9 });
    }

    // ── Verify fog of war was updated ──
    {
        let world = engine.world().borrow();
        // Player 0 should see tiles around (2,0)
        assert_eq!(
            world.tiles.get_visibility(PlayerId(0), 2, 0),
            fc3_core::tile::Visibility::Visible
        );
        assert_eq!(
            world.tiles.get_visibility(PlayerId(0), 3, 0),
            fc3_core::tile::Visibility::Visible
        );
    }

    // ── Verify event log ──
    // Commands logged: move(1,0), move_fail(2,0), endturn, skip, endturn, move(2,0)
    assert_eq!(engine.event_log.len(), 6);
}

/// Integration test: full AI game with pathfinding (Task 7.13).
///
/// Runs a 50-turn headless game with 2 AI players on a 30x20 map.
/// AI uses pathfinding to navigate and attack.
/// Asserts: no panics, game completes without infinite loops.
#[test]
fn test_full_game_with_pathfinding() {
    use fc3_core::ai::{Agent, SimpleAgent};
    use fc3_core::protocol::Command;
    use fc3_core::world::WorldConfig;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    // Run 5 games with different seeds to verify stability
    for seed in [42u64, 100, 200, 300, 400] {
        let config = GameConfig {
            world: WorldConfig {
                width: 30,
                height: 20,
                wrap_x: true,
                wrap_y: false,
                num_players: 2,
                seed,
            },
            mod_paths: vec!["base".to_string()],
            units_per_player: vec!["warrior".to_string(), "warrior".to_string()],
            max_turns: Some(50),
        };

        let mut engine = Engine::new_game(&config).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = (0..2u8)
            .map(|i| {
                let agent_seed = seed.wrapping_add(i as u64);
                Box::new(SimpleAgent::new(ChaCha8Rng::seed_from_u64(agent_seed))) as Box<dyn Agent>
            })
            .collect();

        let mut total_commands = 0u64;

        loop {
            if engine.is_game_over().is_some() {
                break;
            }

            let current = engine.current_player();
            if !engine.is_player_alive(current) {
                engine.submit_command(current, Command::EndTurn);
                continue;
            }

            let agent_idx = current.0 as usize;
            let mut commands_this_turn = 0u32;

            loop {
                let view = engine.player_view(current);
                let available = engine.available_commands(current);
                let cmd = agents[agent_idx].decide(&view, &available);

                let is_end_turn = matches!(cmd, Command::EndTurn);
                engine.submit_command(current, cmd);
                total_commands += 1;

                if is_end_turn {
                    break;
                }

                commands_this_turn += 1;
                if commands_this_turn > 500 {
                    engine.submit_command(current, Command::EndTurn);
                    break;
                }
            }
        }

        println!(
            "Game seed={seed}: {} turns, {} total commands",
            engine.current_turn(),
            total_commands
        );
    }
}

/// Integration test: deterministic replay with pathfinding (Task 7.13).
///
/// Plays a game with pathfinding-heavy movement, replays from command log,
/// and verifies all unit positions match.
#[test]
fn test_deterministic_replay_with_pathfinding() {
    use fc3_core::ai::{Agent, SimpleAgent};
    use fc3_core::protocol::Command;
    use fc3_core::world::WorldConfig;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    let config = GameConfig {
        world: WorldConfig {
            width: 30,
            height: 20,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 54321,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string(), "warrior".to_string()],
        max_turns: Some(50),
    };

    let mut engine = Engine::new_game(&config).unwrap();
    let mut agents: Vec<Box<dyn Agent>> = (0..2u8)
        .map(|i| {
            let agent_seed = 54321u64.wrapping_add(i as u64);
            Box::new(SimpleAgent::new(ChaCha8Rng::seed_from_u64(agent_seed))) as Box<dyn Agent>
        })
        .collect();

    loop {
        if engine.is_game_over().is_some() {
            break;
        }

        let current = engine.current_player();
        if !engine.is_player_alive(current) {
            engine.submit_command(current, Command::EndTurn);
            continue;
        }

        let agent_idx = current.0 as usize;
        let mut commands_this_turn = 0u32;

        loop {
            let view = engine.player_view(current);
            let available = engine.available_commands(current);
            let cmd = agents[agent_idx].decide(&view, &available);
            let is_end_turn = matches!(cmd, Command::EndTurn);
            engine.submit_command(current, cmd);

            if is_end_turn {
                break;
            }
            commands_this_turn += 1;
            if commands_this_turn > 500 {
                engine.submit_command(current, Command::EndTurn);
                break;
            }
        }
    }

    // Capture original state
    let original_turn = engine.current_turn();
    let original_winner = engine.is_game_over();
    let original_unit_count = engine.world().borrow().units.count();

    // Capture all unit positions
    let original_positions: Vec<_> = {
        let world = engine.world().borrow();
        world
            .units
            .iter_alive()
            .map(|(_uid, idx)| (world.units.position[idx], world.units.owner[idx]))
            .collect()
    };

    // Replay from command log
    let log = engine.to_game_log(&config);
    let replayed = log.replay().unwrap();

    assert_eq!(
        replayed.current_turn(),
        original_turn,
        "replay turn mismatch"
    );
    assert_eq!(
        replayed.is_game_over(),
        original_winner,
        "replay winner mismatch"
    );
    assert_eq!(
        replayed.world().borrow().units.count(),
        original_unit_count,
        "replay unit count mismatch"
    );

    // Compare all unit positions
    let replayed_positions: Vec<_> = {
        let world = replayed.world().borrow();
        world
            .units
            .iter_alive()
            .map(|(_uid, idx)| (world.units.position[idx], world.units.owner[idx]))
            .collect()
    };

    assert_eq!(
        original_positions, replayed_positions,
        "unit positions differ after replay"
    );
}
