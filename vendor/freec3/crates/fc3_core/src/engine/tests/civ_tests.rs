use super::*;

#[test]
fn test_civs_assigned_to_players_after_mod_loading() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let world = engine.world.borrow();
    // Both players should have a civ assigned
    for player in &world.players {
        assert!(
            player.civilization.is_some(),
            "Player {} should have a civilization assigned",
            player.id.0
        );
    }
}

#[test]
fn test_no_duplicate_civ_assignments() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let world = engine.world.borrow();
    let civs: Vec<&str> = world
        .players
        .iter()
        .filter_map(|p| p.civilization.as_deref())
        .collect();
    // All assigned civs should be unique
    let mut unique = civs.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(civs.len(), unique.len(), "civ assignments should be unique");
}

#[test]
fn test_player_name_set_to_ruler_name() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let world = engine.world.borrow();
    for player in &world.players {
        if let Some(civ_id) = &player.civilization {
            let def = world.civilization_registry.get(civ_id).unwrap();
            assert_eq!(
                player.name, def.ruler_name,
                "player name should match ruler name for civ {}",
                civ_id
            );
        }
    }
}

#[test]
fn test_civ_assignment_deterministic_with_seed() {
    let config = test_config();
    let engine1 = Engine::new_game(&config).unwrap();
    let engine2 = Engine::new_game(&config).unwrap();

    let world1 = engine1.world.borrow();
    let world2 = engine2.world.borrow();

    for i in 0..world1.players.len() {
        assert_eq!(
            world1.players[i].civilization, world2.players[i].civilization,
            "civ assignment should be deterministic for player {}",
            i
        );
        assert_eq!(
            world1.players[i].name, world2.players[i].name,
            "player name should be deterministic for player {}",
            i
        );
    }
}

#[test]
fn test_player_view_includes_civ_fields() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let view = engine.player_view(PlayerId(0));
    assert!(view.civilization.is_some(), "civilization should be set");
    assert!(view.civ_adjective.is_some(), "civ_adjective should be set");

    // Verify the adjective matches the registry
    let world = engine.world.borrow();
    let civ_id = view.civilization.as_ref().unwrap();
    let def = world.civilization_registry.get(civ_id).unwrap();
    assert_eq!(
        view.civ_adjective.as_ref().unwrap(),
        &def.adjective,
        "civ_adjective should match registry"
    );
}

#[test]
fn test_civ_assignment_with_more_players_than_civs() {
    // Create a config with more players than civs (6 civs defined in base mod)
    let config = GameConfig {
        world: WorldConfig {
            width: 20,
            height: 20,
            wrap_x: true,
            wrap_y: false,
            num_players: 8,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec![],
        max_turns: None,
    };
    let engine = Engine::new_game(&config).unwrap();
    let world = engine.world.borrow();

    // First 6 players should have civs
    let assigned: Vec<&str> = world
        .players
        .iter()
        .filter_map(|p| p.civilization.as_deref())
        .collect();
    assert_eq!(assigned.len(), 6, "only 6 civs available for 8 players");

    // Players without civs should keep default names
    for player in &world.players {
        if player.civilization.is_none() {
            assert!(
                player.name.starts_with("Player"),
                "unassigned players should keep default name, got: {}",
                player.name
            );
        }
    }
}

#[test]
fn test_base_mod_loads_civilizations() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let world = engine.world.borrow();
    assert!(
        world.civilization_registry.all().len() >= 6,
        "base mod should register at least 6 civilizations"
    );
    assert!(
        world.civilization_registry.get("rome").is_some(),
        "rome should be registered"
    );
    assert!(
        world.civilization_registry.get("greece").is_some(),
        "greece should be registered"
    );
}
