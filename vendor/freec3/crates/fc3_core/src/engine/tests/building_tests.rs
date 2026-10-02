use super::*;
use crate::city::ProductionItem;
use crate::protocol::Event;

// ── Palace auto-add on city founding ─────────────────────────────

#[test]
fn test_palace_added_on_first_city_founding() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert!(
        world.cities.buildings[idx].contains(&"palace".to_string()),
        "First city should get a Palace"
    );
}

#[test]
fn test_palace_not_added_on_second_city() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let _city1 = found_city(&mut engine, PlayerId(0), TileCoord { x: 3, y: 3 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), _city1);

    let city2 = found_city(&mut engine, PlayerId(0), TileCoord { x: 8, y: 8 }, "Athens");

    let world = engine.world.borrow();
    let idx = world.cities.get(city2).unwrap();
    assert!(
        !world.cities.buildings[idx].contains(&"palace".to_string()),
        "Second city should NOT get a Palace"
    );
}

// ── Palace not buildable ─────────────────────────────────────────

#[test]
fn test_palace_not_in_buildable_options() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let options = engine.buildable_options(city_id);
    let has_palace = options.iter().any(|o| o.name == "Palace");
    assert!(!has_palace, "Palace should not appear in buildable options");
}

#[test]
fn test_palace_set_production_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: ProductionItem::Building {
                building_id: "palace".to_string(),
            },
        },
    );
    assert!(!result.is_ok(), "Setting production to Palace should fail");
}

// ── Building appears in buildable_options ─────────────────────────

#[test]
fn test_granary_requires_pottery_tech() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Without Pottery researched, Granary should not be available
    let options = engine.buildable_options(city_id);
    let has_granary = options.iter().any(|o| o.name == "Granary");
    assert!(
        !has_granary,
        "Granary should not appear without Pottery tech"
    );
}

#[test]
fn test_granary_available_after_pottery_researched() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Give the player the Pottery tech
    {
        let mut world = engine.world.borrow_mut();
        if let Some(p) = world.players.get_mut(0) {
            p.researched_techs.push("pottery".to_string());
        }
    }

    let options = engine.buildable_options(city_id);
    let has_granary = options.iter().any(|o| o.name == "Granary");
    assert!(
        has_granary,
        "Granary should appear after Pottery is researched"
    );
}

// ── Building production completes and adds to city ──────────────

#[test]
fn test_building_production_completes_and_adds_to_city() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 0, y: 0 });

    // Give the player Pottery so Granary is buildable
    {
        let mut world = engine.world.borrow_mut();
        if let Some(p) = world.players.get_mut(0) {
            p.researched_techs.push("pottery".to_string());
        }
    }

    // Set production to Granary
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: ProductionItem::Building {
                building_id: "granary".to_string(),
            },
        },
    );
    assert!(
        result.is_ok(),
        "Setting production to Granary should succeed"
    );

    // Give enough shields to complete the building
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shield_stockpile[idx] = 100; // Granary costs 60
    }

    // End turn for player 0 (skip warrior for player 1)
    engine.submit_command(PlayerId(0), Command::EndTurn);

    // Player 1 ends turn (skip their units)
    {
        let world = engine.world.borrow();
        let p1_units: Vec<_> = world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(1))
            .map(|(uid, _)| uid)
            .collect();
        drop(world);
        for uid in p1_units {
            engine.submit_command(PlayerId(1), Command::SkipUnit { unit_id: uid });
        }
    }
    set_research(&mut engine, PlayerId(1));
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);

    // Check for BuildingCompleted event
    let has_building_complete = result.events.iter().any(
        |e| matches!(e, Event::BuildingCompleted { building_id, .. } if building_id == "granary"),
    );
    assert!(
        has_building_complete,
        "Should have BuildingCompleted event for granary, events: {:?}",
        result.events
    );

    // Verify the building is in the city's buildings list
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert!(
        world.cities.buildings[idx].contains(&"granary".to_string()),
        "City should have granary after production completes"
    );

    // Production should be auto-set to next cheapest option
    assert!(
        world.cities.producing[idx].is_some(),
        "Production should be auto-set after building completes"
    );
}

// ── Can't build same building twice ──────────────────────────────

#[test]
fn test_cannot_build_same_building_twice() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Give the player Pottery and add Granary to the city
    {
        let mut world = engine.world.borrow_mut();
        if let Some(p) = world.players.get_mut(0) {
            p.researched_techs.push("pottery".to_string());
        }
        let idx = world.cities.get(city_id).unwrap();
        world.cities.buildings[idx].push("granary".to_string());
    }

    // Granary should not be in buildable options
    let options = engine.buildable_options(city_id);
    let has_granary = options.iter().any(|o| o.name == "Granary");
    assert!(
        !has_granary,
        "Granary should not appear if city already has it"
    );

    // Trying to set production should also fail
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: ProductionItem::Building {
                building_id: "granary".to_string(),
            },
        },
    );
    assert!(
        !result.is_ok(),
        "Building same building twice should be rejected"
    );
}

// ── Building maintenance deducts gold ────────────────────────────

#[test]
fn test_building_maintenance_deducts_gold() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 0, y: 0 });

    // Give the city a Granary (maintenance = 1)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.buildings[idx].push("granary".to_string());
        // Set player's gold to a known value
        if let Some(p) = world.players.get_mut(0) {
            p.gold = 10;
        }
    }

    // Get commerce per turn
    let commerce = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.commerce_per_turn[idx]
    };

    // End turn
    engine.submit_command(PlayerId(0), Command::EndTurn);

    // Gold should be: 10 + commerce - 1 (granary maintenance)
    let world = engine.world.borrow();
    let expected_gold = 10 + commerce - 1;
    assert_eq!(
        world.players[0].gold, expected_gold,
        "Gold should be 10 + commerce({}) - 1 maintenance = {}, got {}",
        commerce, expected_gold, world.players[0].gold
    );
}

// ── Buildings appear in PlayerView ──────────────────────────────

#[test]
fn test_buildings_in_player_view() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Add Palace is auto-added, verify it shows in view
    let view = engine.player_view(PlayerId(0));
    let city_snap = view.own_cities.iter().find(|c| c.id == city_id).unwrap();
    assert!(
        city_snap
            .buildings
            .as_ref()
            .is_some_and(|b| b.contains(&"palace".to_string())),
        "PlayerView should include palace in city buildings"
    );
}

// ── City.has_building and City.add_building Lua API ─────────────

#[test]
fn test_city_has_building_lua_api() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let cid_lua = crate::scripting::api_city::city_id_to_lua(city_id);

    // Palace was auto-added
    let has_palace: bool = engine
        .scripts()
        .lua
        .load(format!(r#"return City.has_building({cid_lua}, "palace")"#))
        .eval()
        .unwrap();
    assert!(has_palace, "City should have palace");

    // Granary was NOT added
    let has_granary: bool = engine
        .scripts()
        .lua
        .load(format!(r#"return City.has_building({cid_lua}, "granary")"#))
        .eval()
        .unwrap();
    assert!(!has_granary, "City should not have granary");
}

#[test]
fn test_city_add_building_lua_api() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let cid_lua = crate::scripting::api_city::city_id_to_lua(city_id);

    // Add granary via Lua
    let added: bool = engine
        .scripts()
        .lua
        .load(format!(r#"return City.add_building({cid_lua}, "granary")"#))
        .eval()
        .unwrap();
    assert!(added, "First add should return true");

    // Second add should return false (already has it)
    let added_again: bool = engine
        .scripts()
        .lua
        .load(format!(r#"return City.add_building({cid_lua}, "granary")"#))
        .eval()
        .unwrap();
    assert!(!added_again, "Second add should return false");

    // Verify via has_building
    let has: bool = engine
        .scripts()
        .lua
        .load(format!(r#"return City.has_building({cid_lua}, "granary")"#))
        .eval()
        .unwrap();
    assert!(has, "City should have granary after add");
}

// ── City.get("buildings") returns table ─────────────────────────

#[test]
fn test_city_get_buildings_attr() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let cid_lua = crate::scripting::api_city::city_id_to_lua(city_id);

    let count: i64 = engine
        .scripts()
        .lua
        .load(format!(
            r#"local b = City.get({cid_lua}, "buildings"); return #b"#
        ))
        .eval()
        .unwrap();
    assert_eq!(count, 1, "City should have 1 building (palace)");

    let first: String = engine
        .scripts()
        .lua
        .load(format!(r#"return City.get({cid_lua}, "buildings")[1]"#))
        .eval()
        .unwrap();
    assert_eq!(first, "palace");
}

// ── Building.define and Building.get from base mod ──────────────

#[test]
fn test_base_mod_defines_palace_and_granary() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let world = engine.world.borrow();

    assert!(
        world.building_registry.get("palace").is_some(),
        "palace should be defined"
    );
    assert!(
        world.building_registry.get("granary").is_some(),
        "granary should be defined"
    );

    let palace = world.building_registry.get("palace").unwrap();
    assert_eq!(palace.name, "Palace");
    assert_eq!(palace.cost, 200);
    assert_eq!(palace.maintenance, 0);

    let granary = world.building_registry.get("granary").unwrap();
    assert_eq!(granary.name, "Granary");
    assert_eq!(granary.cost, 60);
    assert_eq!(granary.maintenance, 1);
    assert_eq!(granary.requires, vec!["pottery".to_string()]);
}

// ── Granary preserves 50% food on growth ─────────────────────────

#[test]
fn test_granary_preserves_food_on_growth() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 0, y: 0 });

    // Add granary to the city
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.buildings[idx].push("granary".to_string());
        // Set food to just enough to grow: threshold for pop 1 is 10 + 2*1 = 12
        world.cities.food_stockpile[idx] = 12;
        world.cities.food_per_turn[idx] = 4; // net = 4 - 2 = 2, stockpile becomes 14 >= 12
    }

    // End turn and let production cycle
    engine.submit_command(PlayerId(0), Command::EndTurn);

    // Skip player 1
    {
        let world = engine.world.borrow();
        let p1_units: Vec<_> = world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(1))
            .map(|(uid, _)| uid)
            .collect();
        drop(world);
        for uid in p1_units {
            engine.submit_command(PlayerId(1), Command::SkipUnit { unit_id: uid });
        }
    }
    set_research(&mut engine, PlayerId(1));
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // After growth with granary: food_stockpile should be floor(12/2) = 6
    // (50% of the growth threshold for pop 1)
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.population[idx], 2,
        "City should have grown to pop 2"
    );
    assert_eq!(
        world.cities.food_stockpile[idx], 6,
        "Granary should preserve 50% of growth threshold (12/2 = 6)"
    );
}

#[test]
fn test_no_granary_food_stockpile_resets_on_growth() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);
    spawn_warrior(&engine, PlayerId(1), TileCoord { x: 0, y: 0 });

    // No granary — set food to grow
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        // threshold for pop 1 = 12, set to exact threshold
        world.cities.food_stockpile[idx] = 12;
        world.cities.food_per_turn[idx] = 4; // net = 4-2 = 2, stockpile = 14 >= 12
    }

    engine.submit_command(PlayerId(0), Command::EndTurn);

    {
        let world = engine.world.borrow();
        let p1_units: Vec<_> = world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(1))
            .map(|(uid, _)| uid)
            .collect();
        drop(world);
        for uid in p1_units {
            engine.submit_command(PlayerId(1), Command::SkipUnit { unit_id: uid });
        }
    }
    set_research(&mut engine, PlayerId(1));
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Without granary: food_stockpile should be overflow (14 - 12 = 2)
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(world.cities.population[idx], 2, "Should grow to pop 2");
    assert_eq!(
        world.cities.food_stockpile[idx], 2,
        "Without granary, overflow should be 14 - 12 = 2"
    );
}
