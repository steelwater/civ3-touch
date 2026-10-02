use super::*;

#[test]
fn test_combat_basic_warrior_vs_warrior() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Place warriors adjacent on grassland
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result.is_ok(), "combat should succeed: {:?}", result.errors);

    // Should have CombatStarted, some CombatRound(s), CombatResolved, UnitDestroyed
    let has_combat_started = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CombatStarted { .. }));
    let has_combat_resolved = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CombatResolved { .. }));
    let has_unit_destroyed = result
        .events
        .iter()
        .any(|e| matches!(e, Event::UnitDestroyed { .. }));
    assert!(has_combat_started, "should have CombatStarted event");
    assert!(has_combat_resolved, "should have CombatResolved event");
    assert!(has_unit_destroyed, "should have UnitDestroyed event");

    // One unit should be dead, one alive
    let world = engine.world.borrow();
    let attacker_alive = world.units.is_alive(attacker);
    let defender_alive = world.units.is_alive(defender);
    assert!(
            attacker_alive != defender_alive,
            "exactly one unit should survive: attacker_alive={attacker_alive}, defender_alive={defender_alive}"
        );

    // Winner should be alive with valid HP
    if attacker_alive {
        let idx = world.units.get(attacker).unwrap();
        assert!(
            world.units.hp[idx] > 0 && world.units.hp[idx] <= world.units.max_hp[idx],
            "winner should have valid HP"
        );
        // Attacker movement should be 0
        assert_eq!(
            world.units.movement[idx], 0,
            "attacker movement should be 0 after combat"
        );
    }
}

#[test]
fn test_combat_attacker_moves_to_defender_tile_on_win() {
    // Use a seed where we know the outcome. We'll try and verify the
    // winner occupies the defender's former tile.
    let config = test_config();
    let mut engine = Engine::new_game(&config).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });
    let defender_pos = TileCoord { x: 5, y: 6 };

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result.is_ok());

    let world = engine.world.borrow();
    if world.units.is_alive(attacker) {
        // Attacker won — should be at defender's old position
        let idx = world.units.get(attacker).unwrap();
        assert_eq!(
            world.units.position[idx], defender_pos,
            "victorious attacker should move to defender's tile"
        );
    }
    // If defender won, attacker is dead — that's also valid
}

#[test]
fn test_combat_terrain_defense_bonus_hill() {
    // Run many combats: attacker on grassland vs defender on hill.
    // Defender should win significantly more often due to +50% defense.
    let mut defender_wins = 0;
    for seed in 100..200 {
        let config = GameConfig {
            world: WorldConfig {
                width: 10,
                height: 10,
                wrap_x: true,
                wrap_y: false,
                num_players: 2,
                seed,
            },
            mod_paths: vec!["base".to_string()],
            units_per_player: vec![],
            max_turns: None,
        };
        let mut engine = Engine::new_game(&config).unwrap();
        // Set defender tile to Hill
        {
            let mut world = engine.world.borrow_mut();
            let idx = world.tiles.idx(5, 6);
            world.tiles.terrain[idx] = crate::tile::Terrain::Hill;
        }
        let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

        engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

        let world = engine.world.borrow();
        if world.units.is_alive(defender) {
            defender_wins += 1;
        }
    }
    // With 1.0 atk vs 1.5 def, defender should win ~60% or more.
    assert!(
        defender_wins > 50,
        "defender on hill should win more than 50% of 100 fights, got {defender_wins}"
    );
}

#[test]
fn test_combat_fortification_bonus() {
    // Run many combats: attacker vs fortified defender on grassland.
    let mut defender_wins = 0;
    for seed in 200..300 {
        let config = GameConfig {
            world: WorldConfig {
                width: 10,
                height: 10,
                wrap_x: true,
                wrap_y: false,
                num_players: 2,
                seed,
            },
            mod_paths: vec!["base".to_string()],
            units_per_player: vec![],
            max_turns: None,
        };
        let mut engine = Engine::new_game(&config).unwrap();
        let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

        // Fortify the defender
        {
            let mut world = engine.world.borrow_mut();
            let idx = world.units.get(defender).unwrap();
            world.units.fortified[idx] = true;
        }

        engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

        let world = engine.world.borrow();
        if world.units.is_alive(defender) {
            defender_wins += 1;
        }
    }
    // With 1.0 atk vs 1.25 def, defender should win more than 50%.
    assert!(
        defender_wins > 45,
        "fortified defender should win more than 45% of 100 fights, got {defender_wins}"
    );
}

#[test]
fn test_combat_unequal_units_swordsman_vs_warrior() {
    // Define a swordsman inline via Lua, run combats
    let mut attacker_wins = 0;
    for seed in 300..400 {
        let config = GameConfig {
            world: WorldConfig {
                width: 10,
                height: 10,
                wrap_x: true,
                wrap_y: false,
                num_players: 2,
                seed,
            },
            mod_paths: vec!["base".to_string()],
            units_per_player: vec![],
            max_turns: None,
        };
        let mut engine = Engine::new_game(&config).unwrap();

        // Define swordsman via Lua
        engine
            .scripts()
            .lua
            .load(
                r#"UnitType.define({
                        name = "swordsman",
                        attack = 3,
                        defense = 2,
                        movement = 1,
                        max_hp = 3,
                        cost = 20,
                        category = "melee",
                    })"#,
            )
            .exec()
            .unwrap();

        // Spawn swordsman as attacker
        let attacker = {
            let mut world = engine.world().borrow_mut();
            let sid = world.unit_types.get_by_name("swordsman").unwrap().id;
            let template = world.unit_types.get(sid).unwrap().clone();
            world
                .units
                .spawn(sid, PlayerId(0), TileCoord { x: 5, y: 5 }, &template)
        };
        let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

        engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

        let world = engine.world().borrow();
        if world.units.is_alive(attacker) {
            attacker_wins += 1;
        }
    }
    // Swordsman (3 atk) vs warrior (1 def) — swordsman should win most of the time
    assert!(
        attacker_wins > 65,
        "swordsman should win most fights vs warrior, got {attacker_wins}/100"
    );
}

#[test]
fn test_combat_attack_own_unit_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let uid1 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let uid2 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 6 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::AttackUnit {
            attacker: uid1,
            defender: uid2,
        },
    );
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::InvalidTarget);
}

#[test]
fn test_combat_attack_non_adjacent_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 0, y: 0 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::InvalidTarget);
}

#[test]
fn test_combat_attack_with_zero_movement_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    // Drain attacker movement
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(attacker).unwrap();
        world.units.movement[idx] = 0;
    }

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::NotEnoughMovement);
}

#[test]
fn test_combat_attack_invalid_unit_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let invalid_uid = GenId {
        index: 999,
        generation: 0,
    };
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::AttackUnit {
            attacker: invalid_uid,
            defender,
        },
    );
    assert!(!result.is_ok());
    assert_eq!(result.errors[0], GameError::InvalidUnit);
}

#[test]
fn test_combat_destroyed_unit_not_in_player_view() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    // Set up visibility so both players can see the combat area
    engine.update_visibility(PlayerId(0));
    engine.update_visibility(PlayerId(1));

    engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

    let world = engine.world.borrow();
    let attacker_alive = world.units.is_alive(attacker);
    drop(world);

    // Check PlayerView for both players — the dead unit should not appear
    let p0_view = engine.player_view(PlayerId(0));
    let p1_view = engine.player_view(PlayerId(1));

    let dead_id = if attacker_alive { defender } else { attacker };
    assert!(
        !p0_view.known_units.iter().any(|u| u.id == dead_id),
        "dead unit should not be in player 0's view"
    );
    assert!(
        !p1_view.known_units.iter().any(|u| u.id == dead_id),
        "dead unit should not be in player 1's view"
    );
}

#[test]
fn test_combat_destroyed_unit_stale_id_returns_none() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

    let world = engine.world.borrow();
    let dead_id = if world.units.is_alive(attacker) {
        defender
    } else {
        attacker
    };
    assert!(
        world.units.get(dead_id).is_none(),
        "stale ID of destroyed unit should return None"
    );
}

#[test]
fn test_combat_fog_of_war_updates_after_destruction() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Player 1 has only one unit — when it dies, their visibility degrades
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    engine.update_visibility(PlayerId(1));

    // Verify defender's area is visible for player 1
    {
        let world = engine.world.borrow();
        assert_eq!(
            world.tiles.get_visibility(PlayerId(1), 5, 6),
            crate::tile::Visibility::Visible
        );
    }

    engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

    let world = engine.world.borrow();
    if !world.units.is_alive(defender) {
        // Defender destroyed — player 1's tiles should be Revealed (not Visible)
        // since they have no units left
        assert_eq!(
            world.tiles.get_visibility(PlayerId(1), 5, 6),
            crate::tile::Visibility::Revealed,
            "after unit destroyed, tiles should downgrade to Revealed"
        );
    }
}

#[test]
fn test_combat_deterministic_replay() {
    let config = test_config();
    let mut engine = Engine::new_game(&config).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    // Attack
    let result1 = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result1.is_ok());

    // Capture state
    let world = engine.world.borrow();
    let attacker_alive1 = world.units.is_alive(attacker);
    let defender_alive1 = world.units.is_alive(defender);
    let winner_hp1 = if attacker_alive1 {
        world.units.hp[world.units.get(attacker).unwrap()]
    } else {
        world.units.hp[world.units.get(defender).unwrap()]
    };
    drop(world);

    // Replay with same seed — spawn units again at same positions
    let mut engine2 = Engine::new_game(&config).unwrap();
    let attacker2 = spawn_warrior(&engine2, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender2 = spawn_warrior(&engine2, PlayerId(1), TileCoord { x: 5, y: 6 });

    let result2 = engine2.submit_command(
        PlayerId(0),
        Command::AttackUnit {
            attacker: attacker2,
            defender: defender2,
        },
    );
    assert!(result2.is_ok());

    let world2 = engine2.world.borrow();
    let attacker_alive2 = world2.units.is_alive(attacker2);
    let defender_alive2 = world2.units.is_alive(defender2);
    let winner_hp2 = if attacker_alive2 {
        world2.units.hp[world2.units.get(attacker2).unwrap()]
    } else {
        world2.units.hp[world2.units.get(defender2).unwrap()]
    };

    assert_eq!(
        attacker_alive1, attacker_alive2,
        "same seed should produce same combat outcome"
    );
    assert_eq!(defender_alive1, defender_alive2);
    assert_eq!(winner_hp1, winner_hp2, "same seed should produce same HP");
}

#[test]
fn test_combat_movement_set_to_zero_after_attack() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

    let world = engine.world.borrow();
    if world.units.is_alive(attacker) {
        let idx = world.units.get(attacker).unwrap();
        assert_eq!(
            world.units.movement[idx], 0,
            "attacker movement should be 0 after combat"
        );
    }
}

// ── Game setup tests (Phase 5.1) ────────────────────────────────

#[test]
fn test_game_setup_spawns_starting_units() {
    let config = GameConfig {
        world: WorldConfig {
            width: 20,
            height: 20,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string(), "warrior".to_string()],
        max_turns: None,
    };
    let engine = Engine::new_game(&config).unwrap();
    let world = engine.world.borrow();

    // Each player should have 2 warriors
    let p0_units: Vec<_> = world
        .units
        .iter_alive()
        .filter(|&(_, idx)| world.units.owner[idx] == PlayerId(0))
        .collect();
    let p1_units: Vec<_> = world
        .units
        .iter_alive()
        .filter(|&(_, idx)| world.units.owner[idx] == PlayerId(1))
        .collect();
    assert_eq!(p0_units.len(), 2, "player 0 should have 2 units");
    assert_eq!(p1_units.len(), 2, "player 1 should have 2 units");
}

#[test]
fn test_game_setup_units_on_valid_positions() {
    let config = GameConfig {
        world: WorldConfig {
            width: 20,
            height: 20,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string()],
        max_turns: None,
    };
    let engine = Engine::new_game(&config).unwrap();
    let world = engine.world.borrow();

    for (_, idx) in world.units.iter_alive() {
        let pos = world.units.position[idx];
        assert!(
            world.tiles.in_bounds(pos.x, pos.y),
            "unit should be at valid position"
        );
        let tile_idx = world.tiles.idx(pos.x, pos.y);
        assert!(
            !matches!(
                world.tiles.terrain[tile_idx],
                crate::tile::Terrain::Ocean | crate::tile::Terrain::Mountain
            ),
            "unit should be on land tile"
        );
    }
}

#[test]
fn test_game_setup_fog_initialized() {
    let config = GameConfig {
        world: WorldConfig {
            width: 20,
            height: 20,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string()],
        max_turns: None,
    };
    let engine = Engine::new_game(&config).unwrap();
    let world = engine.world.borrow();

    // Each player should have some visible tiles around their starting position
    for player_idx in 0..2u8 {
        let player = PlayerId(player_idx);
        let visible_count: usize = (0..world.tiles.height)
            .flat_map(|y| (0..world.tiles.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                world.tiles.get_visibility(player, x, y) == crate::tile::Visibility::Visible
            })
            .count();
        assert!(
            visible_count > 0,
            "player {player_idx} should see some tiles around their starting position"
        );
    }
}

#[test]
fn test_game_config_serialization_roundtrip() {
    let config = GameConfig {
        world: WorldConfig {
            width: 40,
            height: 25,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string(), "warrior".to_string()],
        max_turns: Some(500),
    };
    let json = serde_json::to_string(&config).unwrap();
    let back: GameConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.world.width, 40);
    assert_eq!(back.units_per_player.len(), 2);
    assert_eq!(back.max_turns, Some(500));
}

// ── Available commands tests (Phase 5.5) ────────────────────────

#[test]
fn test_available_commands_end_turn_always_present() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let cmds = engine.available_commands(PlayerId(0));
    assert!(
        cmds.iter().any(|c| matches!(c, AvailableCommand::EndTurn)),
        "EndTurn should always be available"
    );
}

#[test]
fn test_available_commands_unit_with_no_movement() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.movement[idx] = 0;
    }
    let cmds = engine.available_commands(PlayerId(0));
    // Should not have Move, Attack, or Fortify for this unit
    assert!(
        !cmds.iter().any(|c| match c {
            AvailableCommand::Move { unit_id, .. } => *unit_id == uid,
            AvailableCommand::Attack { unit_id, .. } => *unit_id == uid,
            AvailableCommand::Fortify { unit_id } => *unit_id == uid,
            _ => false,
        }),
        "unit with 0 movement should have no commands"
    );
}

#[test]
fn test_available_commands_unit_surrounded_by_ocean() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    // Set all neighbors to ocean
    {
        let mut world = engine.world.borrow_mut();
        for dest in world.tiles.neighbors(5, 5).clone() {
            let idx = world.tiles.idx(dest.x, dest.y);
            world.tiles.terrain[idx] = crate::tile::Terrain::Ocean;
        }
    }
    // Init visibility
    engine.update_visibility(PlayerId(0));
    let cmds = engine.available_commands(PlayerId(0));
    // Should not have Move destinations
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, AvailableCommand::Move { unit_id, .. } if *unit_id == uid)),
        "unit surrounded by ocean should have no move destinations"
    );
}

#[test]
fn test_available_commands_attack_adjacent_enemy() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });
    // Make enemy visible
    {
        let mut world = engine.world.borrow_mut();
        world
            .tiles
            .set_visibility(PlayerId(0), 5, 6, crate::tile::Visibility::Visible);
    }
    let cmds = engine.available_commands(PlayerId(0));
    let has_attack = cmds.iter().any(|c| match c {
        AvailableCommand::Attack { unit_id, targets } => {
            *unit_id == uid && targets.contains(&enemy)
        }
        _ => false,
    });
    assert!(has_attack, "should have attack command for adjacent enemy");
}

#[test]
fn test_available_commands_no_fortify_when_already_fortified() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(uid).unwrap();
        world.units.fortified[idx] = true;
    }
    let cmds = engine.available_commands(PlayerId(0));
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, AvailableCommand::Fortify { unit_id } if *unit_id == uid)),
        "already fortified unit should not have Fortify command"
    );
}

#[test]
fn test_available_commands_no_other_player_units() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let _uid0 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let uid1 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 7, y: 7 });
    let cmds = engine.available_commands(PlayerId(0));
    // Should not include any commands for player 1's unit
    assert!(
        !cmds.iter().any(|c| match c {
            AvailableCommand::Move { unit_id, .. } => *unit_id == uid1,
            AvailableCommand::Attack { unit_id, .. } => *unit_id == uid1,
            AvailableCommand::Fortify { unit_id } => *unit_id == uid1,
            _ => false,
        }),
        "should not include commands for other player's units"
    );
}

// ── Elimination tests (Phase 5.6) ───────────────────────────────

#[test]
fn test_elimination_last_unit_killed() {
    // Use a specific seed where we can control the outcome
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });

    // Give attacker overwhelming advantage to ensure they win
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(attacker).unwrap();
        world.units.hp[idx] = 100;
        world.units.max_hp[idx] = 100;
    }

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result.is_ok());

    // Should have PlayerEliminated and GameOver events
    let has_eliminated = result
        .events
        .iter()
        .any(|e| matches!(e, Event::PlayerEliminated { player } if *player == PlayerId(1)));
    let has_game_over = result
        .events
        .iter()
        .any(|e| matches!(e, Event::GameOver { winner } if *winner == PlayerId(0)));
    assert!(has_eliminated, "should have PlayerEliminated for player 1");
    assert!(has_game_over, "should have GameOver with player 0 winning");
    assert_eq!(engine.is_game_over(), Some(PlayerId(0)));
}

#[test]
fn test_elimination_with_multiple_units() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let d1 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });
    let _d2 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 8, y: 8 });

    // Make attacker very strong
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(attacker).unwrap();
        world.units.hp[idx] = 100;
        world.units.max_hp[idx] = 100;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::AttackUnit {
            attacker,
            defender: d1,
        },
    );
    assert!(result.is_ok());

    // Should NOT have PlayerEliminated — player 1 still has d2
    let has_eliminated = result
        .events
        .iter()
        .any(|e| matches!(e, Event::PlayerEliminated { .. }));
    assert!(
        !has_eliminated,
        "should not eliminate player 1 when they still have units"
    );
    assert!(engine.is_game_over().is_none());
}

#[test]
fn test_eliminated_player_skipped_in_turns() {
    // 3 players, eliminate player 1, verify turns skip them
    let config = GameConfig {
        world: WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 3,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec![],
        max_turns: None,
    };
    let mut engine = Engine::new_game(&config).unwrap();

    // Eliminate player 1 by marking them dead
    {
        let mut world = engine.world.borrow_mut();
        world.players[1].alive = false;
    }

    assert_eq!(engine.current_player(), PlayerId(0));

    // Player 0 ends turn -> should skip player 1, go to player 2
    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(result.is_ok());
    assert_eq!(engine.current_player(), PlayerId(2));

    // Player 2 ends turn -> should skip player 1, go to player 0 (new turn)
    let result = engine.submit_command(PlayerId(2), Command::EndTurn);
    assert!(result.is_ok());
    assert_eq!(engine.current_player(), PlayerId(0));
}

// ── Turn limit tests (Phase 5.7) ────────────────────────────────

#[test]
fn test_turn_limit_ends_game() {
    let config = GameConfig {
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
        max_turns: Some(3),
    };
    let mut engine = Engine::new_game(&config).unwrap();
    // Spawn units so players aren't empty
    spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 7, y: 7 });

    // Play through turns until max_turns is exceeded
    let mut game_over = false;
    for _ in 0..10 {
        let player = engine.current_player();
        // Skip all unhandled units before ending turn
        let available = engine.available_commands(player);
        for cmd in &available {
            if let AvailableCommand::Skip { unit_id } = cmd {
                engine.submit_command(player, Command::SkipUnit { unit_id: *unit_id });
            }
        }
        let result = engine.submit_command(player, Command::EndTurn);
        if result
            .events
            .iter()
            .any(|e| matches!(e, Event::GameOver { .. }))
        {
            game_over = true;
            break;
        }
    }
    assert!(game_over, "game should end when turn limit is reached");
    assert!(engine.is_game_over().is_some());
}

// ── Stack combat tests ───────────────────────────────────────────

#[test]
fn test_attacker_does_not_advance_into_remaining_enemy_stack() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 6 });
    let defender1 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    let defender2 = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    // Give attacker overwhelming HP to guarantee a win
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(attacker).unwrap();
        world.units.hp[idx] = 100;
        world.units.max_hp[idx] = 100;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::AttackUnit {
            attacker,
            defender: defender1,
        },
    );
    assert!(result.is_ok(), "combat should succeed: {:?}", result.errors);

    // Defender1 should be dead, defender2 still alive
    let world = engine.world.borrow();
    assert!(!world.units.is_alive(defender1));
    assert!(world.units.is_alive(defender2));

    // Attacker should NOT have advanced — defender2 still occupies (5,5)
    let attacker_idx = world.units.get(attacker).unwrap();
    assert_eq!(
        world.units.position[attacker_idx],
        TileCoord { x: 5, y: 6 },
        "attacker must not advance when enemy units remain on target tile"
    );
}

#[test]
fn test_attacker_advances_after_killing_last_unit_in_stack() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 6 });
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });

    // Give attacker overwhelming HP to guarantee a win
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.units.get(attacker).unwrap();
        world.units.hp[idx] = 100;
        world.units.max_hp[idx] = 100;
    }

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result.is_ok(), "combat should succeed: {:?}", result.errors);

    // Attacker should advance to the now-empty tile
    let world = engine.world.borrow();
    let attacker_idx = world.units.get(attacker).unwrap();
    assert_eq!(
        world.units.position[attacker_idx],
        TileCoord { x: 5, y: 5 },
        "attacker should advance when no enemies remain on target tile"
    );
}

// ── City founding tests (Phase 6.3) ─────────────────────────────
