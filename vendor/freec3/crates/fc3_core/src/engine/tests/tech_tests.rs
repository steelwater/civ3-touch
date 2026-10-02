use super::*;

#[test]
fn test_set_research_command() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Clear auto-research set by found_city helper
    {
        let mut world = engine.world.borrow_mut();
        world.players[0].researching = None;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetResearch {
            tech_id: "bronze_working".to_string(),
        },
    );
    assert!(
        result.is_ok(),
        "SetResearch should succeed: {:?}",
        result.errors
    );

    // Verify event
    let has_event = result.events.iter().any(|e| {
        matches!(e, Event::ResearchSet { player, tech_id }
            if *player == PlayerId(0) && tech_id == "bronze_working")
    });
    assert!(has_event, "should have ResearchSet event");

    // Verify player state
    let world = engine.world.borrow();
    assert_eq!(
        world.players[0].researching,
        Some("bronze_working".to_string())
    );
}

#[test]
fn test_set_research_invalid_tech() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let _city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetResearch {
            tech_id: "nonexistent".to_string(),
        },
    );
    assert!(!result.is_ok(), "SetResearch with invalid tech should fail");
}

#[test]
fn test_set_research_already_researched() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let _city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Manually mark a tech as researched
    {
        let mut world = engine.world.borrow_mut();
        world.players[0]
            .researched_techs
            .push("alphabet".to_string());
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetResearch {
            tech_id: "alphabet".to_string(),
        },
    );
    assert!(
        !result.is_ok(),
        "SetResearch for already-researched tech should fail"
    );
}

#[test]
fn test_science_accumulation_at_end_turn() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Set commerce per turn manually (city yields not computed until city processing)
    {
        let mut world = engine.world.borrow_mut();
        let idx = world.cities.get(city_id).unwrap();
        world.cities.commerce_per_turn[idx] = 3;
    }

    // Verify research is set (done by found_city helper)
    {
        let world = engine.world.borrow();
        assert!(
            world.players[0].researching.is_some(),
            "research should be set"
        );
    }

    // Skip all units for P0
    let p0_units: Vec<crate::id::UnitId> = {
        let world = engine.world.borrow();
        world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(0))
            .map(|(uid, _)| uid)
            .collect()
    };
    for uid in p0_units {
        engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    }

    // End P0's turn
    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn should succeed: {:?}",
        result.errors
    );

    // Check science accumulated
    let world = engine.world.borrow();
    assert_eq!(
        world.players[0].science, 3,
        "science should equal commerce per turn (3)"
    );
}

#[test]
fn test_future_tech_offered_when_all_techs_researched() {
    let engine = Engine::new_game(&test_config()).unwrap();

    // Mark all registry techs as researched
    {
        let mut world = engine.world.borrow_mut();
        let tech_ids: Vec<String> = world
            .tech_registry
            .all()
            .iter()
            .map(|t| t.id.clone())
            .collect();
        world.players[0].researched_techs = tech_ids;
    }

    let techs = engine.available_techs(PlayerId(0));
    assert_eq!(techs.len(), 1, "should offer exactly one future tech");
    assert_eq!(techs[0].id, "future_tech_1");
    assert_eq!(techs[0].name, "Future Tech 1");
    assert_eq!(techs[0].cost, 300);
}

#[test]
fn test_tech_completion_with_overflow() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Set research to a tech with cost 40, set commerce to 5, science to 38
    {
        let mut world = engine.world.borrow_mut();
        world.players[0].researching = Some("bronze_working".to_string());
        world.players[0].science = 38;
        let idx = world.cities.get(city_id).unwrap();
        world.cities.commerce_per_turn[idx] = 5;
    }

    // Skip all units and end turn
    let p0_units: Vec<crate::id::UnitId> = {
        let world = engine.world.borrow();
        world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(0))
            .map(|(uid, _)| uid)
            .collect()
    };
    for uid in p0_units {
        engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    }

    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn should succeed: {:?}",
        result.errors
    );

    // Check tech completed (38 + 5 = 43 >= 40)
    let has_tech_researched = result.events.iter().any(|e| {
        matches!(e, Event::TechResearched { player, tech_id }
            if *player == PlayerId(0) && tech_id == "bronze_working")
    });
    assert!(has_tech_researched, "TechResearched event should fire");

    // Verify overflow preserved: 38 + 5 - 40 = 3
    let world = engine.world.borrow();
    assert_eq!(
        world.players[0].science, 3,
        "science overflow should be preserved (38+5-40=3)"
    );
    assert!(
        world.players[0]
            .researched_techs
            .contains(&"bronze_working".to_string()),
        "bronze_working should be in researched_techs"
    );
    assert!(
        world.players[0].researching.is_none(),
        "researching should be None after completion"
    );
}

#[test]
fn test_end_turn_allowed_without_research() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Clear research
    {
        let mut world = engine.world.borrow_mut();
        world.players[0].researching = None;
    }

    // Skip all units
    let p0_units: Vec<crate::id::UnitId> = {
        let world = engine.world.borrow();
        world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(0))
            .map(|(uid, _)| uid)
            .collect()
    };
    for uid in p0_units {
        engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    }

    // EndTurn should succeed even without research set (tech popup shown at turn start)
    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn should succeed without research set: {:?}",
        result.errors
    );
}

#[test]
fn test_end_turn_allowed_when_researching_future_tech() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Mark all registry techs as researched, then set research to future tech
    {
        let mut world = engine.world.borrow_mut();
        let tech_ids: Vec<String> = world
            .tech_registry
            .all()
            .iter()
            .map(|t| t.id.clone())
            .collect();
        world.players[0].researched_techs = tech_ids;
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetResearch {
            tech_id: "future_tech_1".to_string(),
        },
    );
    assert!(result.is_ok(), "SetResearch for future tech should succeed");

    // Skip all units
    let p0_units: Vec<crate::id::UnitId> = {
        let world = engine.world.borrow();
        world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(0))
            .map(|(uid, _)| uid)
            .collect()
    };
    for uid in p0_units {
        engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: uid });
    }

    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn should succeed when researching future tech: {:?}",
        result.errors
    );
}

#[test]
fn test_available_techs_prerequisite_filtering() {
    let engine = Engine::new_game(&test_config()).unwrap();

    // Register a tech with a prerequisite
    {
        let mut world = engine.world.borrow_mut();
        world.tech_registry.register(crate::tech::TechDef {
            id: "writing".to_string(),
            name: "Writing".to_string(),
            cost: 60,
            requires: vec!["alphabet".to_string()],
        });
    }

    // Without alphabet researched, writing should not be available
    let techs = engine.available_techs(PlayerId(0));
    let has_writing = techs.iter().any(|t| t.id == "writing");
    assert!(
        !has_writing,
        "writing should not be available without alphabet"
    );

    // Research alphabet
    {
        let mut world = engine.world.borrow_mut();
        world.players[0]
            .researched_techs
            .push("alphabet".to_string());
    }

    // Now writing should be available
    let techs = engine.available_techs(PlayerId(0));
    let has_writing = techs.iter().any(|t| t.id == "writing");
    assert!(has_writing, "writing should be available after alphabet");

    // Alphabet should NOT be available (already researched)
    let has_alphabet = techs.iter().any(|t| t.id == "alphabet");
    assert!(
        !has_alphabet,
        "alphabet should not be available (already researched)"
    );
}

#[test]
fn test_available_commands_includes_set_research() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Clear research
    {
        let mut world = engine.world.borrow_mut();
        world.players[0].researching = None;
    }

    let commands = engine.available_commands(PlayerId(0));
    let has_set_research = commands.iter().any(|c| {
        matches!(c, crate::protocol::AvailableCommand::SetResearch { options } if !options.is_empty())
    });
    assert!(
        has_set_research,
        "available_commands should include SetResearch when research not set"
    );

    // EndTurn should still be available (research no longer blocks EndTurn)
    let has_end_turn = commands
        .iter()
        .any(|c| matches!(c, crate::protocol::AvailableCommand::EndTurn));
    assert!(
        has_end_turn,
        "EndTurn should be available even when research not set"
    );
}

#[test]
fn test_player_view_includes_tech_info() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");

    // Set specific research
    engine.submit_command(
        PlayerId(0),
        Command::SetResearch {
            tech_id: "pottery".to_string(),
        },
    );

    // Add a completed tech and set commerce
    {
        let mut world = engine.world.borrow_mut();
        world.players[0]
            .researched_techs
            .push("masonry".to_string());
        let idx = world.cities.get(city_id).unwrap();
        world.cities.commerce_per_turn[idx] = 3;
    }

    let view = engine.player_view(PlayerId(0));
    assert_eq!(view.researching, Some("pottery".to_string()));
    assert!(view.researched_techs.contains(&"masonry".to_string()));
    assert_eq!(
        view.science_per_turn, 3,
        "science_per_turn should reflect city commerce"
    );
}

#[test]
fn test_set_research_prerequisites_not_met() {
    let mut engine = Engine::new_game(&test_config()).unwrap();

    // Register a tech with a prerequisite
    {
        let mut world = engine.world.borrow_mut();
        world.tech_registry.register(crate::tech::TechDef {
            id: "writing".to_string(),
            name: "Writing".to_string(),
            cost: 60,
            requires: vec!["alphabet".to_string()],
        });
    }

    let result = engine.submit_command(
        PlayerId(0),
        Command::SetResearch {
            tech_id: "writing".to_string(),
        },
    );
    assert!(
        !result.is_ok(),
        "SetResearch should fail when prerequisites not met"
    );
}

#[test]
fn test_future_tech_completion_increments_number() {
    let mut engine = Engine::new_game(&test_config()).unwrap();
    let city_id = found_city(&mut engine, PlayerId(0), TileCoord { x: 5, y: 5 }, "Rome");
    set_production_warrior(&mut engine, PlayerId(0), city_id);

    // Mark all registry techs as researched
    {
        let mut world = engine.world.borrow_mut();
        let tech_ids: Vec<String> = world
            .tech_registry
            .all()
            .iter()
            .map(|t| t.id.clone())
            .collect();
        world.players[0].researched_techs = tech_ids;
    }

    // Set research to Future Tech 1
    let result = engine.submit_command(
        PlayerId(0),
        Command::SetResearch {
            tech_id: "future_tech_1".to_string(),
        },
    );
    assert!(result.is_ok(), "should be able to research future_tech_1");

    // Give enough science to complete it and end turn
    {
        let mut world = engine.world.borrow_mut();
        world.players[0].science = 300;
        let idx = world.cities.get(city_id).unwrap();
        world.cities.commerce_per_turn[idx] = 1;
    }

    let p0_units: Vec<crate::id::UnitId> = {
        let world = engine.world.borrow();
        world
            .units
            .iter_alive()
            .filter(|(_, idx)| world.units.owner[*idx] == PlayerId(0))
            .map(|(uid, _)| uid)
            .collect()
    };
    for uid in &p0_units {
        engine.submit_command(PlayerId(0), Command::SkipUnit { unit_id: *uid });
    }

    let result = engine.submit_command(PlayerId(0), Command::EndTurn);
    assert!(
        result.is_ok(),
        "EndTurn should succeed: {:?}",
        result.errors
    );

    // Verify future_tech_1 completed
    let has_event = result
        .events
        .iter()
        .any(|e| matches!(e, Event::TechResearched { tech_id, .. } if tech_id == "future_tech_1"));
    assert!(has_event, "TechResearched event for future_tech_1");

    {
        let world = engine.world.borrow();
        assert!(
            world.players[0]
                .researched_techs
                .contains(&"future_tech_1".to_string()),
            "future_tech_1 should be in researched_techs"
        );
        assert!(
            world.players[0].researching.is_none(),
            "researching should be None after completion"
        );
    }

    // Now available_techs should offer Future Tech 2
    let techs = engine.available_techs(PlayerId(0));
    assert_eq!(techs.len(), 1);
    assert_eq!(techs[0].id, "future_tech_2");
    assert_eq!(techs[0].name, "Future Tech 2");
}
