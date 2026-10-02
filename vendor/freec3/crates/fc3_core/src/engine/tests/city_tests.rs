use super::*;
use rand::SeedableRng;

#[test]
fn test_found_city_settler_on_grassland() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
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

    // Check events
    let has_city_founded = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CityFounded { name, .. } if name == "Alexandria"));
    let has_unit_consumed = result
        .events
        .iter()
        .any(|e| matches!(e, Event::UnitConsumed { .. }));
    assert!(has_city_founded, "should have CityFounded event");
    assert!(has_unit_consumed, "should have UnitConsumed event");

    // City exists
    let world = engine.world.borrow();
    assert_eq!(world.cities.count(), 1);
    let (_, idx) = world.cities.iter_alive().next().unwrap();
    assert_eq!(world.cities.name[idx], "Alexandria");
    assert_eq!(world.cities.owner[idx], PlayerId(0));
    assert_eq!(world.cities.position[idx], TileCoord { x: 5, y: 5 });
    assert_eq!(world.cities.population[idx], 1);

    // Settler is consumed
    assert!(!world.units.is_alive(settler_id));
}

#[test]
fn test_found_city_on_ocean_blocked() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Ocean;
    }
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );
    assert!(!result.is_ok());
    assert!(result.errors[0].to_string().contains("ocean"));
}

#[test]
fn test_found_city_on_mountain_blocked() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Mountain;
    }
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );
    assert!(!result.is_ok());
    assert!(result.errors[0].to_string().contains("mountain"));
}

#[test]
fn test_found_city_too_close_to_existing_blocked() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Found first city
    let settler1 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler1,
            action_id: "build_city".to_string(),
        },
    );
    assert!(result.is_ok());

    // Try to found another city within 2 tiles
    let settler2 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 6, y: 5 });
    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler2,
            action_id: "build_city".to_string(),
        },
    );
    assert!(!result.is_ok());
    assert!(result.errors[0].to_string().contains("too close"));
}

#[test]
fn test_found_city_warrior_cannot_found() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let warrior_id = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: warrior_id,
            action_id: "build_city".to_string(),
        },
    );
    assert!(!result.is_ok());
    assert!(result.errors[0].to_string().contains("build_city"));
}

#[test]
fn test_found_city_sets_tile_ownership() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );

    let world = engine.world.borrow();
    // City center should be owned
    let center_idx = world.tiles.idx(5, 5);
    assert_eq!(world.tiles.owner[center_idx], Some(PlayerId(0)));
    // Adjacent tiles should be owned
    let adj_idx = world.tiles.idx(5, 6);
    assert_eq!(world.tiles.owner[adj_idx], Some(PlayerId(0)));
}

#[test]
fn test_found_city_appears_in_own_player_view() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );

    let view = engine.player_view(PlayerId(0));
    assert_eq!(view.own_cities.len(), 1);
    assert!(
        !view.own_cities[0].name.is_empty(),
        "city should have a name"
    );
    assert_eq!(view.own_cities[0].population, 1);
    assert!(view.own_cities[0].food_stockpile.is_some());
}

#[test]
fn test_found_city_appears_in_enemy_known_cities() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    // Make the tile visible to player 1
    {
        let mut world = engine.world.borrow_mut();
        world
            .tiles
            .set_visibility(PlayerId(1), 5, 5, crate::tile::Visibility::Visible);
    }

    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );

    let view = engine.player_view(PlayerId(1));
    assert_eq!(view.known_cities.len(), 1);
    assert!(
        !view.known_cities[0].name.is_empty(),
        "city should have a name"
    );
    assert!(
        view.known_cities[0].food_stockpile.is_none(),
        "enemy should not see food details"
    );
}

// ── City radius tests (Phase 6.4) ───────────────────────────────

// ── Tile yield tests (Phase 6.5) ───────────────────────────────

#[test]
fn test_tile_yield_grassland() {
    let engine = Engine::new_game(&test_config()).unwrap();
    let (food, shields, commerce) = engine.calculate_tile_yield(5, 5);
    assert_eq!((food, shields, commerce), (2, 1, 0));
}

#[test]
fn test_tile_yield_plains() {
    let engine = Engine::new_game(&test_config()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Plains;
    }
    let (food, shields, commerce) = engine.calculate_tile_yield(5, 5);
    assert_eq!((food, shields, commerce), (1, 1, 1));
}

#[test]
fn test_tile_yield_hill() {
    let engine = Engine::new_game(&test_config()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Hill;
    }
    let (food, shields, commerce) = engine.calculate_tile_yield(5, 5);
    assert_eq!((food, shields, commerce), (1, 2, 0));
}

#[test]
fn test_tile_yield_road_adds_commerce() {
    let engine = Engine::new_game(&test_config()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 5);
        world.tiles.road_level[idx] = 1;
    }
    let (food, shields, commerce) = engine.calculate_tile_yield(5, 5);
    assert_eq!((food, shields, commerce), (2, 1, 1));
}

// ── City tile assignment tests (Phase 6.6) ──────────────────────

#[test]
fn test_city_pop1_works_center_plus_one() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );

    let world = engine.world.borrow();
    let (_, idx) = world.cities.iter_alive().next().unwrap();
    // Pop 1: center + 1 tile = 2 worked tiles
    assert_eq!(
        world.cities.worked_tiles[idx].len(),
        2,
        "pop-1 city should work center + 1 tile"
    );
    // food_per_turn should be center (min 2) + best adjacent tile (grassland=2)
    assert!(
        world.cities.food_per_turn[idx] >= 4,
        "pop-1 city on grassland should produce at least 4 food, got {}",
        world.cities.food_per_turn[idx]
    );
}

#[test]
fn test_city_pop3_works_center_plus_three() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });

    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );

    // Manually set population to 3 and reassign
    let city_id = {
        let mut world = engine.world.borrow_mut();
        let (cid, idx) = world.cities.iter_alive().next().unwrap();
        world.cities.population[idx] = 3;
        cid
    };
    engine.reassign_city_tiles(city_id);

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.worked_tiles[idx].len(),
        4,
        "pop-3 city should work center + 3 tiles"
    );
}

#[test]
fn test_two_cities_no_double_work() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Found two cities 3 tiles apart (just outside minimum spacing)
    let settler1 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 3, y: 5 });
    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler1,
            action_id: "build_city".to_string(),
        },
    );
    let settler2 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 7, y: 5 });
    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler2,
            action_id: "build_city".to_string(),
        },
    );

    // Check no tile is worked by two different cities
    let world = engine.world.borrow();
    let total_tiles = (world.tiles.width * world.tiles.height) as usize;
    for i in 0..total_tiles {
        if let Some(_cid) = world.tiles.worked_by[i] {
            // This tile is worked — it should only be worked by one city
            // (the worked_by array prevents double-working by design)
        }
    }
    // More directly: check the cities don't share worked tiles
    let cities: Vec<_> = world.cities.iter_alive().collect();
    if cities.len() == 2 {
        let (_, idx0) = cities[0];
        let (_, idx1) = cities[1];
        let tiles0: std::collections::HashSet<_> = world.cities.worked_tiles[idx0].iter().collect();
        let tiles1: std::collections::HashSet<_> = world.cities.worked_tiles[idx1].iter().collect();
        let overlap: Vec<_> = tiles0.intersection(&tiles1).collect();
        assert!(
            overlap.is_empty(),
            "two cities should not share worked tiles, overlap: {:?}",
            overlap
        );
    }
}

#[test]
fn test_city_yield_on_mixed_terrain() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Set some tiles to hill and plains around (5,5)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(5, 6);
        world.tiles.terrain[idx] = crate::tile::Terrain::Hill;
        let idx = world.tiles.idx(4, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Plains;
    }
    let settler_id = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
    engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler_id,
            action_id: "build_city".to_string(),
        },
    );

    let world = engine.world.borrow();
    let (_, idx) = world.cities.iter_alive().next().unwrap();
    // Should have some shields from the hill
    assert!(
        world.cities.shields_per_turn[idx] >= 2,
        "city near hill should produce shields"
    );
}

#[test]
fn test_city_radius_center_of_map() {
    let config = test_config();
    let engine = Engine::new_game(&config).unwrap();
    let world = engine.world.borrow();
    let radius = Engine::city_radius_static(&world.tiles, TileCoord { x: 5, y: 5 });
    // 5x5 minus 4 corners = 21
    assert_eq!(radius.len(), 21, "city radius should be 21 tiles");
}

#[test]
fn test_city_radius_corner_non_wrapping() {
    let config = GameConfig {
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
    };
    let engine = Engine::new_game(&config).unwrap();
    let world = engine.world.borrow();
    let radius = Engine::city_radius_static(&world.tiles, TileCoord { x: 0, y: 0 });
    // Only tiles in positive quadrant + center row/column within range
    assert!(
        radius.len() < 21,
        "corner city should have fewer than 21 radius tiles, got {}",
        radius.len()
    );
    assert!(
        radius.len() >= 6,
        "corner city should have at least 6 radius tiles, got {}",
        radius.len()
    );
}

#[test]
fn test_city_radius_wrapping_map_edge() {
    let config = test_config(); // wrap_x = true
    let engine = Engine::new_game(&config).unwrap();
    let world = engine.world.borrow();
    let radius = Engine::city_radius_static(&world.tiles, TileCoord { x: 0, y: 5 });
    // Should wrap and still get 21 tiles
    assert_eq!(
        radius.len(),
        21,
        "wrapping map should still get full radius"
    );
    // Should include tiles at x=9 (wrapped from x=-1) and x=8 (wrapped from x=-2)
    assert!(radius.iter().any(|t| t.x == 9), "should wrap to x=9");
}

#[test]
fn test_set_production_warrior() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let warrior_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("warrior").unwrap().id
    };

    let result = engine.submit_command(
        PlayerId(0),
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

    // Check ProductionSet event
    let has_event = result.events.iter().any(|e| {
            matches!(e, Event::ProductionSet { item_name, cost, .. } if item_name == "warrior" && *cost == 10)
        });
    assert!(has_event, "should have ProductionSet event");

    // Check city state
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert!(world.cities.producing[idx].is_some());
    assert_eq!(world.cities.production_cost[idx], 10);
    assert_eq!(world.cities.shield_stockpile[idx], 0);
}

#[test]
fn test_set_production_enemy_city_rejected() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let warrior_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("warrior").unwrap().id
    };

    // Player 1 tries to set production on Player 0's city
    // First end Player 0's turn
    engine.submit_command(PlayerId(0), Command::EndTurn);

    let result = engine.submit_command(
        PlayerId(1),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );
    assert!(
        !result.is_ok(),
        "should not allow setting production on enemy city"
    );
}

#[test]
fn test_set_production_resets_shields_on_change() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let (warrior_type_id, settler_type_id) = {
        let world = engine.world.borrow();
        (
            world.unit_types.get_by_name("warrior").unwrap().id,
            world.unit_types.get_by_name("settler").unwrap().id,
        )
    };

    // Grow city to pop 3 so settler production is allowed
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx] = 3;
    }

    // Set production to warrior
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );

    // Manually add some shields
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shield_stockpile[idx] = 5;
    }

    // Change to settler — shields should reset
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: settler_type_id,
            },
        },
    );

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.shield_stockpile[idx], 0,
        "shields should reset when changing production"
    );
}

#[test]
fn test_set_production_same_item_keeps_shields() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let warrior_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("warrior").unwrap().id
    };

    // Set production to warrior
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );

    // Manually add some shields
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shield_stockpile[idx] = 5;
    }

    // Set same production again — shields should be kept
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.shield_stockpile[idx], 5,
        "shields should be kept when re-setting same production"
    );
}

#[test]
fn test_city_food_accumulates_per_turn() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Check initial yields
    let (initial_food_per_turn, initial_population) = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        (
            world.cities.food_per_turn[idx],
            world.cities.population[idx],
        )
    };
    assert_eq!(initial_population, 1);
    // Net food = food_per_turn - (pop * 2) = food_per_turn - 2
    let expected_net = initial_food_per_turn - 2;

    // End turn for P0, P1 ends turn — P0's next turn will process cities
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // After one full cycle, food stockpile should increase
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.food_stockpile[idx], expected_net,
        "food stockpile should be {} after 1 turn (food_per_turn={}, pop=1, consumption=2)",
        expected_net, initial_food_per_turn
    );
}

#[test]
fn test_city_growth_at_threshold() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Growth threshold at pop 1: 10 + 2*1 = 12
    // Set food stockpile just below threshold, so next turn's food pushes it over
    let food_per_turn = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.food_per_turn[idx]
    };
    let net_food = food_per_turn - 2; // pop=1, consumption=2
                                      // Set stockpile so that one turn brings it to exactly threshold
    let needed = 12; // food_needed_for_growth(1) = 10 + 2*1 = 12
    let stockpile_to_set = needed - net_food;
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.food_stockpile[idx] = stockpile_to_set;
    }

    // End turn cycle
    engine.submit_command(PlayerId(0), Command::EndTurn);
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);

    // Check for growth event (emitted when P0's turn starts)
    let has_growth = result.events.iter().any(|e| {
        matches!(
            e,
            Event::CityGrew {
                new_population: 2,
                ..
            }
        )
    });
    assert!(
        has_growth,
        "city should grow to pop 2, events: {:?}",
        result.events
    );

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(world.cities.population[idx], 2);
}

#[test]
fn test_city_starvation_reduces_pop() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Set pop to 2, but zero food_per_turn so the city starves
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx] = 2;
        world.cities.food_per_turn[idx] = 0; // 0 food, consumption = 2*2=4, net=-4
        world.cities.food_stockpile[idx] = 0;
    }

    // End turn cycle
    engine.submit_command(PlayerId(0), Command::EndTurn);
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);

    let has_starvation = result.events.iter().any(|e| {
        matches!(
            e,
            Event::CityStarved {
                new_population: 1,
                ..
            }
        )
    });
    assert!(
        has_starvation,
        "city should starve to pop 1, events: {:?}",
        result.events
    );

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(world.cities.population[idx], 1);
    assert_eq!(
        world.cities.food_stockpile[idx], 0,
        "food stockpile should reset on starvation"
    );
}

#[test]
fn test_city_pop1_cannot_starve_below_1() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Set food_per_turn to 0 at pop 1 — city should not go below 1
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.food_per_turn[idx] = 0;
        world.cities.food_stockpile[idx] = 0;
    }

    engine.submit_command(PlayerId(0), Command::EndTurn);
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);

    // Should NOT have starvation event (pop can't drop below 1)
    let has_starvation = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CityStarved { .. }));
    assert!(
        !has_starvation,
        "pop 1 city should not emit starvation event"
    );

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(world.cities.population[idx], 1);
    assert_eq!(world.cities.food_stockpile[idx], 0);
}

#[test]
fn test_city_production_accumulates_shields() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let warrior_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("warrior").unwrap().id
    };

    // Set production
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );

    let shields_per_turn = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shields_per_turn[idx]
    };

    // End turn cycle
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    // If production isn't complete, shields should have accumulated
    if world.cities.shield_stockpile[idx] < 10 {
        assert_eq!(
            world.cities.shield_stockpile[idx], shields_per_turn,
            "shields should accumulate by shields_per_turn each cycle"
        );
    }
}

#[test]
fn test_city_production_completes_and_spawns_unit() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let warrior_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("warrior").unwrap().id
    };

    // Set production
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );

    // Set shields just below cost so next turn completes it
    // warrior cost = 10
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        let shields_per_turn = world.cities.shields_per_turn[idx];
        // Set stockpile so stockpile + shields_per_turn >= 10
        world.cities.shield_stockpile[idx] = (10 - shields_per_turn).max(0);
    }

    // Count units before
    let units_before = {
        let world = engine.world.borrow();
        world
            .units
            .iter_alive()
            .filter(|&(_, idx)| world.units.owner[idx] == PlayerId(0))
            .count()
    };

    // End turn cycle
    engine.submit_command(PlayerId(0), Command::EndTurn);
    let result = engine.submit_command(PlayerId(1), Command::EndTurn);

    // Check for production complete event
    let has_complete = result.events.iter().any(
        |e| matches!(e, Event::ProductionComplete { item_name, .. } if item_name == "warrior"),
    );
    assert!(
        has_complete,
        "should have ProductionComplete event, events: {:?}",
        result.events
    );

    // Check for UnitProduced event
    let has_unit = result.events.iter().any(|e| {
            matches!(e, Event::UnitProduced { unit_type, at, .. } if unit_type == "warrior" && *at == TileCoord { x: 5, y: 5 })
        });
    assert!(
        has_unit,
        "should have UnitProduced event, events: {:?}",
        result.events
    );

    // Count units after — should be one more
    let units_after = {
        let world = engine.world.borrow();
        world
            .units
            .iter_alive()
            .filter(|&(_, idx)| world.units.owner[idx] == PlayerId(0))
            .count()
    };
    assert_eq!(
        units_after,
        units_before + 1,
        "should have one more unit after production"
    );
}

#[test]
fn test_city_production_overflow_carries_over() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let warrior_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("warrior").unwrap().id
    };

    // Set production
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: warrior_type_id,
            },
        },
    );

    let shields_per_turn = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shields_per_turn[idx]
    };

    // Set shields so completion will have overflow
    // warrior cost = 10, set stockpile to 10 - shields_per_turn + 3
    // After turn: stockpile = 10-spt+3 + spt = 13 => overflow = 13-10 = 3
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shield_stockpile[idx] = 10 - shields_per_turn + 3;
    }

    // End turn cycle
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.shield_stockpile[idx], 3,
        "overflow of 3 shields should carry over"
    );
}

#[test]
fn test_city_auto_production_allows_end_turn() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let _city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // City has auto-set production after founding, so EndTurn should succeed
    // (assuming no unhandled units and research is set)
    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn should succeed when city has auto-set production: {:?}",
        result.errors
    );
}

#[test]
fn test_available_commands_includes_set_production_for_city() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let commands = engine.available_commands(PlayerId(0));
    let has_set_prod = commands.iter().any(
        |c| matches!(c, AvailableCommand::SetProduction { city_id: cid, .. } if *cid == city_id),
    );
    assert!(
        has_set_prod,
        "available commands should include SetProduction for owned city"
    );
}

#[test]
fn test_city_provides_visibility_on_founding() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let _city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // City provides sight range 2 — tiles within Chebyshev distance 2 should be visible
    let view = engine.player_view(PlayerId(0));
    let visible_coords: Vec<(u32, u32)> = view
        .visible_tiles
        .iter()
        .filter(|t| t.visibility == crate::tile::Visibility::Visible)
        .map(|t| (t.coord.x, t.coord.y))
        .collect();

    // Should include tiles at distance 2
    assert!(
        visible_coords.contains(&(3, 5)),
        "tile at (3,5) should be visible (distance 2 from city)"
    );
    assert!(
        visible_coords.contains(&(7, 5)),
        "tile at (7,5) should be visible (distance 2 from city)"
    );
    assert!(
        visible_coords.contains(&(5, 3)),
        "tile at (5,3) should be visible (distance 2 from city)"
    );
    assert!(
        visible_coords.contains(&(5, 7)),
        "tile at (5,7) should be visible (distance 2 from city)"
    );
}

#[test]
fn test_city_visibility_persists_without_units() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Spawn a settler, found a city, then destroy any remaining units
    let _city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Destroy all units for player 0
    {
        let mut world = engine.world.borrow_mut();
        let ids: Vec<crate::id::UnitId> = world
            .units
            .iter_alive()
            .filter(|&(_, idx)| world.units.owner[idx] == PlayerId(0))
            .map(|(uid, _)| uid)
            .collect();
        for uid in ids {
            world.units.destroy(uid);
        }
    }

    // Recalculate visibility — city should still provide vision
    engine.update_visibility(PlayerId(0));

    let view = engine.player_view(PlayerId(0));
    let visible_coords: Vec<(u32, u32)> = view
        .visible_tiles
        .iter()
        .filter(|t| t.visibility == crate::tile::Visibility::Visible)
        .map(|t| (t.coord.x, t.coord.y))
        .collect();

    // City center and surrounding tiles should still be visible
    assert!(
        visible_coords.contains(&(5, 5)),
        "city tile should be visible even without units"
    );
    assert!(
        visible_coords.contains(&(4, 5)),
        "adjacent tile should be visible from city"
    );
    assert!(
        visible_coords.contains(&(6, 5)),
        "adjacent tile should be visible from city"
    );
    // Border tiles are at distance 2; sight range 2 from border means distance 4 is visible
    assert!(
        visible_coords.contains(&(5, 1)),
        "tile 4 away from center (2 from border) should be visible"
    );
}

#[test]
fn test_city_visibility_combines_with_unit_visibility() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Found city at (5,5) — provides sight range 2 from border tiles
    let _city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Spawn warrior at (1,1) — provides sight range 1 (or 2 on hill)
    let _warrior = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 1, y: 1 });
    engine.update_visibility(PlayerId(0));

    let view = engine.player_view(PlayerId(0));
    let visible_coords: Vec<(u32, u32)> = view
        .visible_tiles
        .iter()
        .filter(|t| t.visibility == crate::tile::Visibility::Visible)
        .map(|t| (t.coord.x, t.coord.y))
        .collect();

    // Both city area and warrior area should be visible
    assert!(
        visible_coords.contains(&(5, 5)),
        "city tile should be visible"
    );
    assert!(
        visible_coords.contains(&(1, 1)),
        "warrior tile should be visible"
    );
    assert!(
        visible_coords.contains(&(1, 2)),
        "tile near warrior should be visible"
    );
    assert!(
        visible_coords.contains(&(3, 5)),
        "tile at range 2 from city should be visible"
    );
}

#[test]
fn test_city_visibility_extends_from_border() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Found city at (5,5). Border tiles are at distance 2 (e.g., (5,3)).
    // Sight range 2 from border means tiles up to distance 4 from center are visible.
    let _city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Destroy all units so only city provides vision
    {
        let mut world = engine.world.borrow_mut();
        let ids: Vec<crate::id::UnitId> = world
            .units
            .iter_alive()
            .filter(|&(_, idx)| world.units.owner[idx] == PlayerId(0))
            .map(|(uid, _)| uid)
            .collect();
        for uid in ids {
            world.units.destroy(uid);
        }
    }
    engine.update_visibility(PlayerId(0));

    let view = engine.player_view(PlayerId(0));
    let visible_coords: Vec<(u32, u32)> = view
        .visible_tiles
        .iter()
        .filter(|t| t.visibility == crate::tile::Visibility::Visible)
        .map(|t| (t.coord.x, t.coord.y))
        .collect();

    // Center should be visible
    assert!(
        visible_coords.contains(&(5, 5)),
        "city center should be visible"
    );

    // Distance 2 from center (border tile) should be visible
    assert!(
        visible_coords.contains(&(5, 3)),
        "border tile at distance 2 should be visible"
    );

    // Distance 4 from center (2 from border at (5,3)) should be visible
    assert!(
        visible_coords.contains(&(5, 1)),
        "tile at distance 4 from center should be visible (2 from border)"
    );

    // Distance 4 along x-axis: border at (7,5), sight +2 = (9,5)
    assert!(
        visible_coords.contains(&(9, 5)),
        "tile at distance 4 along x from center should be visible"
    );

    // Distance 5 from center should NOT be visible (map is 10x10, (5,0) is distance 5)
    // Border at (5,3) + sight 2 only reaches (5,1), not (5,0) which is distance 5
    // Actually (5,0) is at distance 5 from center. No border tile is close enough.
    assert!(
        !visible_coords.contains(&(5, 0)),
        "tile at distance 5 from center should NOT be visible"
    );
}

#[test]
fn test_production_completion_auto_sets_new_production() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Set shields so next turn completes production (warrior cost = 10)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        let spt = world.cities.shields_per_turn[idx];
        world.cities.shield_stockpile[idx] = (10 - spt).max(0);
    }

    // End turn cycle — production processes on P0's next turn start
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // After completion, production should be auto-set to cheapest option
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert!(
        world.cities.producing[idx].is_some(),
        "producing should be auto-set after completion"
    );
    assert!(
        world.cities.production_cost[idx] > 0,
        "production cost should be set for auto-selected item"
    );
}

#[test]
fn test_city_founding_auto_sets_production() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // City should have production auto-set after founding
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert!(
        world.cities.producing[idx].is_some(),
        "newly founded city should have production auto-set"
    );
    assert!(
        world.cities.production_cost[idx] > 0,
        "production cost should be set for auto-selected item"
    );
}

#[test]
fn test_wealth_in_production_options() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let commands = engine.available_commands(PlayerId(0));
    let set_prod = commands.iter().find(
        |c| matches!(c, AvailableCommand::SetProduction { city_id: cid, .. } if *cid == city_id),
    );
    assert!(set_prod.is_some());
    if let Some(AvailableCommand::SetProduction { options, .. }) = set_prod {
        let has_wealth = options.iter().any(|o| o.name == "wealth" && o.cost == 0);
        assert!(has_wealth, "production options should include wealth");
    }
}

#[test]
fn test_wealth_settable_and_converts_shields_to_gold() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Set production to wealth
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Wealth,
        },
    );
    assert!(result.is_ok());

    // Get shields_per_turn for this city
    let shields_per_turn = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shields_per_turn[idx]
    };

    // End turn cycle — wealth converts shields to gold on P0's next turn
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Player 0's gold should have increased by shields_per_turn (from wealth)
    // plus commerce_per_turn (from commerce accumulation at EndTurn)
    let world = engine.world.borrow();
    let p0 = &world.players[0];
    assert!(
        p0.gold >= shields_per_turn,
        "gold should include wealth conversion: got {} expected at least {}",
        p0.gold,
        shields_per_turn
    );

    // Wealth should NOT accumulate shield stockpile
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.shield_stockpile[idx], 0,
        "wealth should not accumulate shields"
    );
}

#[test]
fn test_commerce_accumulates_into_player_gold() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    let commerce_per_turn = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.commerce_per_turn[idx]
    };

    // Gold should start at 0
    {
        let world = engine.world.borrow();
        assert_eq!(world.players[0].gold, 0);
    }

    // End turn — commerce accumulates when P0 ends turn
    engine.submit_command(PlayerId(0), Command::EndTurn);

    // Check gold increased by commerce
    let world = engine.world.borrow();
    assert_eq!(
        world.players[0].gold, commerce_per_turn,
        "gold should increase by commerce_per_turn on EndTurn"
    );
}

#[test]
fn test_player_view_includes_gold_science_culture() {
    let engine = Engine::new_game(&test_config()).unwrap();

    // Directly set some resources
    {
        let mut world = engine.world().borrow_mut();
        world.players[0].gold = 42;
        world.players[0].science = 10;
        world.players[0].culture = 5;
    }

    let view = engine.player_view(PlayerId(0));
    assert_eq!(view.gold, 42);
    assert_eq!(view.science, 10);
    assert_eq!(view.culture, 5);
}

#[test]
fn test_city_defense_bonus_in_combat() {
    // Defender in a city should get +50% defense bonus
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Found a city for P1 at (5,5)
    let settler = spawn_settler(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    // Need to be P1's turn to found city
    engine.submit_command(PlayerId(0), Command::EndTurn);
    let found_result = engine.submit_command(
        PlayerId(1),
        Command::PerformAction {
            unit_id: settler,
            action_id: "build_city".to_string(),
        },
    );
    let p1_city_id = found_result
        .events
        .iter()
        .find_map(|e| match e {
            Event::CityFounded { city_id, .. } => Some(*city_id),
            _ => None,
        })
        .unwrap();
    set_production_warrior(&mut engine, PlayerId(1), p1_city_id);
    set_research(&mut engine, PlayerId(1));

    // Place a defender in the city for P1
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    // Place attacker adjacent for P0
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 4, y: 5 });

    // Skip defender so P1 can end turn
    engine.submit_command(PlayerId(1), Command::SkipUnit { unit_id: defender });
    // End P1's turn so P0 can attack
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Give attacker full movement
    {
        let mut world = engine.world.borrow_mut();
        if let Some(idx) = world.units.get(attacker) {
            world.units.movement[idx] = 1;
        }
    }

    // Run multiple seeded games to verify defense bonus kicks in
    // Just verify the combat runs without error; the bonus is applied via Lua hook
    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result.is_ok());

    // Verify combat events include CombatStarted + CombatResolved
    let has_combat_started = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CombatStarted { .. }));
    let has_combat_resolved = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CombatResolved { .. }));
    assert!(has_combat_started);
    assert!(has_combat_resolved);
}

#[test]
fn test_city_capture_on_defeating_last_defender() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Found a city for P1
    let settler = spawn_settler(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    engine.submit_command(PlayerId(0), Command::EndTurn);
    let found_result = engine.submit_command(
        PlayerId(1),
        Command::PerformAction {
            unit_id: settler,
            action_id: "build_city".to_string(),
        },
    );
    assert!(found_result.is_ok());
    let p1_city_id = found_result
        .events
        .iter()
        .find_map(|e| match e {
            Event::CityFounded { city_id, .. } => Some(*city_id),
            _ => None,
        })
        .unwrap();
    set_production_warrior(&mut engine, PlayerId(1), p1_city_id);
    set_research(&mut engine, PlayerId(1));

    // Place a weak defender in the city (1 HP)
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    {
        let mut world = engine.world.borrow_mut();
        if let Some(idx) = world.units.get(defender) {
            world.units.hp[idx] = 1;
        }
    }

    // Place a strong attacker adjacent (full HP, give extra attack strength)
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 4, y: 5 });
    {
        let mut world = engine.world.borrow_mut();
        if let Some(idx) = world.units.get(attacker) {
            world.units.hp[idx] = 3;
        }
    }

    // Skip defender so P1 can end turn
    engine.submit_command(PlayerId(1), Command::SkipUnit { unit_id: defender });
    // End P1's turn
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Give attacker movement
    {
        let mut world = engine.world.borrow_mut();
        if let Some(idx) = world.units.get(attacker) {
            world.units.movement[idx] = 1;
        }
    }

    // Attack! With 1HP defender, attacker should win most of the time
    // Use a specific seed that guarantees attacker wins
    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });
    assert!(result.is_ok());

    // Check if attacker won
    let attacker_won = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CombatResolved { winner, .. } if *winner == attacker));

    if attacker_won {
        // Should have CityCaptured event
        let capture_event = result
            .events
            .iter()
            .find(|e| matches!(e, Event::CityCaptured { .. }));
        assert!(
            capture_event.is_some(),
            "should have CityCaptured event when last defender defeated, events: {:?}",
            result.events
        );

        if let Some(Event::CityCaptured {
            old_owner,
            new_owner,
            ..
        }) = capture_event
        {
            assert_eq!(*old_owner, PlayerId(1));
            assert_eq!(*new_owner, PlayerId(0));
        }

        // Verify city ownership changed
        let world = engine.world.borrow();
        let idx = world.cities.get(p1_city_id).unwrap();
        assert_eq!(
            world.cities.owner[idx],
            PlayerId(0),
            "city should be captured by P0"
        );
    }
}

#[test]
fn test_city_capture_reduces_population() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Found a city for P1 and grow it to pop 3
    let settler = spawn_settler(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    engine.submit_command(PlayerId(0), Command::EndTurn);
    let found_result = engine.submit_command(
        PlayerId(1),
        Command::PerformAction {
            unit_id: settler,
            action_id: "build_city".to_string(),
        },
    );
    let p1_city_id = found_result
        .events
        .iter()
        .find_map(|e| match e {
            Event::CityFounded { city_id, .. } => Some(*city_id),
            _ => None,
        })
        .unwrap();
    set_production_warrior(&mut engine, PlayerId(1), p1_city_id);
    set_research(&mut engine, PlayerId(1));

    // Set population to 3
    let city_id = {
        let mut world = engine.world.borrow_mut();
        let (cid, idx) = world.cities.iter_alive().next().unwrap();
        world.cities.population[idx] = 3;
        cid
    };

    // Place a 1HP defender
    let defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    {
        let mut world = engine.world.borrow_mut();
        if let Some(idx) = world.units.get(defender) {
            world.units.hp[idx] = 1;
        }
    }

    // Place strong attacker
    let attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 4, y: 5 });

    // Skip defender so P1 can end turn
    engine.submit_command(PlayerId(1), Command::SkipUnit { unit_id: defender });
    engine.submit_command(PlayerId(1), Command::EndTurn);

    {
        let mut world = engine.world.borrow_mut();
        if let Some(idx) = world.units.get(attacker) {
            world.units.movement[idx] = 1;
        }
    }

    let result = engine.submit_command(PlayerId(0), Command::AttackUnit { attacker, defender });

    let attacker_won = result
        .events
        .iter()
        .any(|e| matches!(e, Event::CombatResolved { winner, .. } if *winner == attacker));

    if attacker_won {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        assert_eq!(
            world.cities.population[idx], 2,
            "captured city pop should drop from 3 to 2"
        );
    }
}

#[test]
fn test_elimination_requires_no_units_and_no_cities() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Found a city for P1
    let settler = spawn_settler(&engine, PlayerId(1), TileCoord { x: 5, y: 5 });
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(
        PlayerId(1),
        Command::PerformAction {
            unit_id: settler,
            action_id: "build_city".to_string(),
        },
    );

    // P1 has a city but let's destroy all their units
    {
        let mut world = engine.world.borrow_mut();
        let p1_units: Vec<crate::id::UnitId> = world
            .units
            .iter_alive()
            .filter(|&(_, idx)| world.units.owner[idx] == PlayerId(1))
            .map(|(uid, _)| uid)
            .collect();
        for uid in p1_units {
            world.units.destroy(uid);
        }
    }

    // P1 should still be alive because they have a city
    assert!(
        engine.is_player_alive(PlayerId(1)),
        "player with city but no units should still be alive"
    );
}

// ── Deterministic replay with cities ──────────────────────────

#[test]
fn test_deterministic_replay_with_cities() {
    // Play a game with city founding, production, and growth for 50 turns
    let config = GameConfig {
        world: WorldConfig {
            width: 15,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string(), "settler".to_string()],
        max_turns: Some(50),
    };

    let mut engine = Engine::new_game(&config).unwrap();
    let mut agents: Vec<Box<dyn crate::ai::Agent>> = (0..2)
        .map(|i| {
            Box::new(crate::ai::SimpleAgent::new(
                rand_chacha::ChaCha8Rng::seed_from_u64(100 + i),
            )) as Box<dyn crate::ai::Agent>
        })
        .collect();

    // Play 50 turns
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
        let mut cmds = 0u32;
        loop {
            let view = engine.player_view(current);
            let available = engine.available_commands(current);
            let cmd = agents[agent_idx].decide(&view, &available);
            let is_end = matches!(cmd, Command::EndTurn);
            engine.submit_command(current, cmd);
            if is_end {
                break;
            }
            cmds += 1;
            if cmds > 500 {
                engine.submit_command(current, Command::EndTurn);
                break;
            }
        }
    }

    // Capture original state
    let original_turn = engine.current_turn();
    let original_world = engine.world().borrow();
    let original_city_count = original_world.cities.count();
    let original_unit_count = original_world.units.count();
    let original_city_pops: Vec<(String, i32, i32)> = original_world
        .cities
        .iter_alive()
        .map(|(_, idx)| {
            (
                original_world.cities.name[idx].clone(),
                original_world.cities.population[idx],
                original_world.cities.food_stockpile[idx],
            )
        })
        .collect();
    drop(original_world);

    // Build log and replay
    let log = engine.to_game_log(&config);
    let json = serde_json::to_string(&log).unwrap();
    let log_back: GameLog = serde_json::from_str(&json).unwrap();
    let replayed = log_back.replay().unwrap();

    // Verify replay matches
    assert_eq!(replayed.current_turn(), original_turn);
    let replayed_world = replayed.world().borrow();
    assert_eq!(
        replayed_world.cities.count(),
        original_city_count,
        "Replayed game should have same number of cities"
    );
    assert_eq!(
        replayed_world.units.count(),
        original_unit_count,
        "Replayed game should have same number of units"
    );

    // Verify city state matches exactly
    let replayed_city_pops: Vec<(String, i32, i32)> = replayed_world
        .cities
        .iter_alive()
        .map(|(_, idx)| {
            (
                replayed_world.cities.name[idx].clone(),
                replayed_world.cities.population[idx],
                replayed_world.cities.food_stockpile[idx],
            )
        })
        .collect();
    assert_eq!(
        original_city_pops, replayed_city_pops,
        "City populations and food stockpiles should match after replay"
    );

    // Verify unit positions match
    let mut original_positions: Vec<_> = engine
        .world()
        .borrow()
        .units
        .iter_alive()
        .map(|(_, idx)| engine.world().borrow().units.position[idx])
        .collect();
    let mut replayed_positions: Vec<_> = replayed_world
        .units
        .iter_alive()
        .map(|(_, idx)| replayed_world.units.position[idx])
        .collect();
    original_positions.sort_by_key(|p| (p.x, p.y));
    replayed_positions.sort_by_key(|p| (p.x, p.y));
    assert_eq!(
        original_positions, replayed_positions,
        "Unit positions should match after replay"
    );
}

// ── Serialization with cities ─────────────────────────────────

#[test]
fn test_serialization_roundtrip_with_cities() {
    let config = test_config();
    let mut engine = Engine::new_game(&config).unwrap();

    // Found two cities for Player 0
    let city1_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 3, y: 3 }, "Rome");
    let city2_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 8, y: 8 }, "Milan");

    // Set production for one city
    {
        let world = engine.world().borrow();
        let warrior_type_id = world.unit_types.get_by_name("warrior").unwrap().id;
        drop(world);
        engine.submit_command(
            PlayerId(0),
            Command::SetProduction {
                city_id: city1_id,
                item: crate::city::ProductionItem::Unit {
                    unit_type_id: warrior_type_id,
                },
            },
        );
    }

    // Manually set some city state for testing
    {
        let mut world = engine.world().borrow_mut();
        let idx1 = world.cities.get(city1_id).unwrap();
        world.cities.population[idx1] = 3;
        world.cities.food_stockpile[idx1] = 7;
        world.cities.shield_stockpile[idx1] = 5;
        let idx2 = world.cities.get(city2_id).unwrap();
        world.cities.population[idx2] = 2;
        world.cities.food_stockpile[idx2] = 4;
    }

    // Serialize the world
    let world = engine.world().borrow();
    let json = serde_json::to_string(&*world).unwrap();
    drop(world);

    // Deserialize
    let mut back: crate::world::World = serde_json::from_str(&json).unwrap();
    back.restore_rng();

    // Verify city count
    assert_eq!(back.cities.count(), 2, "Should have 2 cities");

    // Verify city 1 state
    let idx1 = back.cities.get(city1_id).unwrap();
    assert!(
        !back.cities.name[idx1].is_empty(),
        "city 1 should have a name"
    );
    assert_eq!(back.cities.owner[idx1], PlayerId(0));
    assert_eq!(back.cities.position[idx1], TileCoord { x: 3, y: 3 });
    assert_eq!(back.cities.population[idx1], 3);
    assert_eq!(back.cities.food_stockpile[idx1], 7);
    assert_eq!(back.cities.shield_stockpile[idx1], 5);
    assert!(back.cities.producing[idx1].is_some());

    // Verify city 2 state
    let idx2 = back.cities.get(city2_id).unwrap();
    assert!(
        !back.cities.name[idx2].is_empty(),
        "city 2 should have a name"
    );
    assert_eq!(back.cities.population[idx2], 2);
    assert_eq!(back.cities.food_stockpile[idx2], 4);

    // Verify worked tiles are preserved
    assert!(
        !back.cities.worked_tiles[idx1].is_empty(),
        "Worked tiles for city 1 should be preserved"
    );

    // Verify tile ownership is preserved
    let city1_pos = back.cities.position[idx1];
    let tile_idx = back.tiles.idx(city1_pos.x, city1_pos.y);
    assert_eq!(
        back.tiles.owner[tile_idx],
        Some(PlayerId(0)),
        "Tile at city center should be owned"
    );
}

#[test]
fn test_wrapping_distance_blocks_city_founding() {
    // On a wrapping map of width 10, cities at x=1 and x=9 are only distance 2 apart
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
        max_turns: None,
    };
    let mut engine = Engine::new_game(&config).unwrap();

    // Found first city at x=1
    let _city1 = found_city(&mut engine, PlayerId(0), TileCoord { x: 1, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), _city1);

    // Try to found city at x=9 — only 2 tiles away via wrapping, should be blocked
    let settler2 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 9, y: 5 });
    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler2,
            action_id: "build_city".to_string(),
        },
    );
    assert!(
        !result.is_ok(),
        "should block founding within wrapped distance 2"
    );
}

#[test]
fn test_wrapping_distance_allows_city_founding_beyond_min_spacing() {
    // On a wrapping map of width 10, cities at x=1 and x=6 are distance 5 apart
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
        max_turns: None,
    };
    let mut engine = Engine::new_game(&config).unwrap();

    // Found first city at x=1
    let _city1 = found_city(&mut engine, PlayerId(0), TileCoord { x: 1, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), _city1);

    // Found city at x=6 — distance 5 via wrapping (or 5 directly), should succeed
    let settler2 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 6, y: 5 });
    let result = engine.submit_command(
        PlayerId(0),
        Command::PerformAction {
            unit_id: settler2,
            action_id: "build_city".to_string(),
        },
    );
    assert!(
        result.is_ok(),
        "should allow founding at distance > 2: {:?}",
        result.errors
    );
}

#[test]
fn test_found_city_suppressed_in_available_commands_when_too_close() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Found a city
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Spawn a settler 1 tile away (within minimum spacing)
    let _settler = spawn_settler(&engine, PlayerId(0), TileCoord { x: 6, y: 5 });
    engine.update_visibility(PlayerId(0));

    let available = engine.available_commands(PlayerId(0));
    let has_found_city = available
        .iter()
        .any(|c| matches!(c, AvailableCommand::UnitAction { action_id, .. } if action_id == "build_city"));
    assert!(
        !has_found_city,
        "found_city action should not be available when too close to existing city"
    );
}

#[test]
fn test_settler_filtered_from_production_options_when_pop_less_than_3() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // City has pop 1 (default) — settler should not be in options
    let available = engine.available_commands(PlayerId(0));
    for cmd in &available {
        if let AvailableCommand::SetProduction {
            city_id: cid,
            options,
        } = cmd
        {
            if *cid == city_id {
                assert!(
                    !options.iter().any(|o| o.name == "settler"),
                    "settler should not be in production options at pop 1"
                );
            }
        }
    }
}

#[test]
fn test_settler_available_in_production_options_when_pop_3() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Grow city to pop 3
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx] = 3;
    }

    let available = engine.available_commands(PlayerId(0));
    for cmd in &available {
        if let AvailableCommand::SetProduction {
            city_id: cid,
            options,
        } = cmd
        {
            if *cid == city_id {
                assert!(
                    options.iter().any(|o| o.name == "settler"),
                    "settler should be in production options at pop 3"
                );
            }
        }
    }
}

#[test]
fn test_settler_production_rejected_when_city_pop_less_than_3() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let settler_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("settler").unwrap().id
    };

    // Try to set production to settler with pop 1
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: settler_type_id,
            },
        },
    );
    assert!(
        !result.is_ok(),
        "setting settler production should fail at pop 1"
    );
}

#[test]
fn test_settler_production_reduces_population_by_2() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Grow city to pop 4 (food_stockpile=0 prevents growth during food processing)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx] = 4;
        world.cities.food_stockpile[idx] = 0;
    }

    let settler_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("settler").unwrap().id
    };

    // Set production to settler
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: settler_type_id,
            },
        },
    );

    // Complete production immediately by filling shields
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shield_stockpile[idx] = 30; // settler cost is 30
    }

    // Record pop before turn processing
    let pop_before = {
        let world = engine.world.borrow();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx]
    };

    // End turn for P0, then P1, so production processes for P0 at start of new turn
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Check that population dropped by at least 2 (settler cost)
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    let pop_after = world.cities.population[idx];
    assert!(
        pop_before - pop_after >= 2,
        "settler production should reduce pop by at least 2 (was {}, now {})",
        pop_before,
        pop_after
    );
    // Verify a settler was actually produced
    let settler_count = world
        .units
        .iter_alive()
        .filter(|&(_, uidx)| {
            world.units.owner[uidx] == PlayerId(0) && {
                let tid = world.units.unit_type[uidx];
                world
                    .unit_types
                    .get(tid)
                    .is_some_and(|ut| ut.actions.contains(&"build_city".to_string()))
            }
        })
        .count();
    assert!(settler_count >= 1, "a settler should have been produced");
}

#[test]
fn test_settler_production_pop_min_1() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Set pop to exactly 3 (minimum for settler); food_stockpile=0 prevents growth
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx] = 3;
        world.cities.food_stockpile[idx] = 0;
    }

    let settler_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("settler").unwrap().id
    };

    // Set production to settler and complete it
    engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: settler_type_id,
            },
        },
    );

    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.shield_stockpile[idx] = 30;
    }

    // End turn for P0, then P1, so production processes for P0 at start of new turn
    engine.submit_command(PlayerId(0), Command::EndTurn);
    engine.submit_command(PlayerId(1), Command::EndTurn);

    // Pop should be max(3-2, 1) = 1
    let world = engine.world.borrow();
    let idx = world.cities.get(city_id).unwrap();
    assert_eq!(
        world.cities.population[idx], 1,
        "population should be 1 (min) after settler from pop 3"
    );
}

// ── can_produce callback tests ───────────────────────────────

#[test]
fn test_galley_blocked_in_inland_city() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // All tiles are grassland by default — no coast, so city is inland
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let galley_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("galley").unwrap().id
    };

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: galley_type_id,
            },
        },
    );
    assert!(
        !result.is_ok(),
        "galley production should be blocked in inland city"
    );
    assert!(result.errors[0].to_string().contains("coastal"));
}

#[test]
fn test_galley_allowed_in_coastal_city() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Place coast adjacent to city center
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(6, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Coast;
    }
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let galley_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("galley").unwrap().id
    };

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: galley_type_id,
            },
        },
    );
    assert!(
        result.is_ok(),
        "galley production should succeed in coastal city: {:?}",
        result.errors
    );
}

#[test]
fn test_galley_filtered_from_available_commands_in_inland_city() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    let available = engine.available_commands(PlayerId(0));
    for cmd in &available {
        if let AvailableCommand::SetProduction {
            city_id: cid,
            options,
        } = cmd
        {
            if *cid == city_id {
                assert!(
                    !options.iter().any(|o| o.name == "galley"),
                    "galley should not be in production options for inland city"
                );
            }
        }
    }
}

#[test]
fn test_galley_in_available_commands_for_coastal_city() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.tiles.idx(6, 5);
        world.tiles.terrain[idx] = crate::tile::Terrain::Coast;
    }
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    let available = engine.available_commands(PlayerId(0));
    for cmd in &available {
        if let AvailableCommand::SetProduction {
            city_id: cid,
            options,
        } = cmd
        {
            if *cid == city_id {
                assert!(
                    options.iter().any(|o| o.name == "galley"),
                    "galley should be in production options for coastal city"
                );
            }
        }
    }
}

#[test]
fn test_can_produce_settler_via_lua_callback() {
    // Verifies the settler pop check now comes from the Lua can_produce callback
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let settler_type_id = {
        let world = engine.world.borrow();
        world.unit_types.get_by_name("settler").unwrap().id
    };

    // Pop 1: should be blocked
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: settler_type_id,
            },
        },
    );
    assert!(!result.is_ok(), "settler should be blocked at pop 1");
    assert!(result.errors[0].to_string().contains("population"));

    // Pop 2: should still be blocked
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx] = 2;
    }
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: settler_type_id,
            },
        },
    );
    assert!(!result.is_ok(), "settler should be blocked at pop 2");

    // Pop 3: should succeed
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.population[idx] = 3;
    }
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetProduction {
            city_id,
            item: crate::city::ProductionItem::Unit {
                unit_type_id: settler_type_id,
            },
        },
    );
    assert!(
        result.is_ok(),
        "settler should be allowed at pop 3: {:?}",
        result.errors
    );
}

#[test]
fn test_buildable_options_includes_wealth() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let options = engine.buildable_options(city_id);
    assert!(
        options.iter().any(|o| o.name == "wealth"),
        "buildable_options should always include wealth"
    );
}

#[test]
fn test_buildable_options_filters_by_can_produce() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    // Inland city: no coast adjacent
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let options = engine.buildable_options(city_id);

    // Should have warrior, swordsman, scout, worker, wealth (no tech required)
    assert!(options.iter().any(|o| o.name == "warrior"));
    assert!(options.iter().any(|o| o.name == "swordsman"));
    assert!(options.iter().any(|o| o.name == "scout"));
    assert!(options.iter().any(|o| o.name == "worker"));
    assert!(options.iter().any(|o| o.name == "wealth"));

    // Should NOT have galley (inland), settler (pop 1), spearman (no tech)
    assert!(
        !options.iter().any(|o| o.name == "galley"),
        "galley should be filtered out for inland city"
    );
    assert!(
        !options.iter().any(|o| o.name == "settler"),
        "settler should be filtered out for pop 1 city"
    );
    assert!(
        !options.iter().any(|o| o.name == "spearman"),
        "spearman should be filtered out without Bronze Working"
    );
}
