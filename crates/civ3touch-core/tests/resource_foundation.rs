use civ3touch_core::session::Session;
use fc3_core::engine::{Engine, GameConfig, GameLog};
use fc3_core::protocol::{Command, Event};
use fc3_core::tile::{Terrain, Vegetation, Visibility};
use fc3_core::types::{PlayerId, ResourceId, TileCoord};
use fc3_core::world::WorldConfig;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn rules() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/freec3/mods/base")
}

fn engine(resources: bool) -> Engine {
    Engine::new_game_with_resources(
        &GameConfig {
            world: WorldConfig {
                width: 8,
                height: 8,
                wrap_x: false,
                wrap_y: false,
                num_players: 2,
                seed: 42,
            },
            mod_paths: vec![rules().to_str().unwrap().into()],
            units_per_player: vec![],
            max_turns: None,
        },
        resources,
    )
    .unwrap()
}

#[test]
fn shared_engine_logs_replay_their_resource_mode_without_an_android_session() {
    let config = GameConfig {
        world: WorldConfig {
            width: 8,
            height: 8,
            wrap_x: false,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec![rules().to_str().unwrap().into()],
        units_per_player: vec![],
        max_turns: None,
    };
    for resources in [false, true] {
        let mut game = Engine::new_game_with_resources(&config, resources).unwrap();
        assert!(game.submit_command(PlayerId(0), Command::EndTurn).is_ok());
        let encoded = json!(game.to_game_log(&config));
        assert_eq!(encoded.get("resources_enabled").is_some(), resources);
        let replay = serde_json::from_value::<GameLog>(encoded)
            .unwrap()
            .replay()
            .unwrap();
        assert_eq!(
            json!(*game.world().borrow()),
            json!(*replay.world().borrow())
        );
    }
}

#[test]
fn old_save_replays_and_continues_exactly_as_the_unpatched_engine() {
    // Captured from 350d15c before engine edits, through legal session commands.
    let before = include_str!("fixtures/m3-v2-turn-6.json");
    let after: Value = serde_json::from_str(include_str!("fixtures/m3-v2-turn-7.json")).unwrap();
    let mut old = Session::load(&rules(), before).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&old.save().unwrap()).unwrap(),
        serde_json::from_str::<Value>(before).unwrap()
    );
    old.end_turn();
    assert_eq!(
        serde_json::from_str::<Value>(&old.save().unwrap()).unwrap(),
        after
    );
    let mut restored = Session::load(&rules(), &old.save().unwrap()).unwrap();
    for _ in 0..3 {
        assert_eq!(old.end_turn(), restored.end_turn());
    }
}

#[test]
fn new_games_place_resources_and_keep_their_ruleset_through_reload() {
    let mut game = Session::new(&rules()).unwrap();
    let start: Value = serde_json::from_str(&game.save().unwrap()).unwrap();
    assert_eq!(start["ruleset"], "freec3-90fc7ee-civ3touch-m5-resources-v1");
    assert_eq!(start["world"]["resources_enabled"], true);
    assert!(start["world"]["tiles"]["resource"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| !r.is_null()));
    let mut loaded = Session::load(&rules(), &game.save().unwrap()).unwrap();
    for _ in 0..5 {
        assert_eq!(game.end_turn(), loaded.end_turn());
    }
    assert_eq!(game.save().unwrap(), loaded.save().unwrap());
    for contract in ["unknown", "freec3-90fc7ee-civ3touch-m3-v2"] {
        let mut wrong = start.clone();
        wrong["ruleset"] = json!(contract);
        assert!(Session::load(&rules(), &wrong.to_string()).is_err());
    }
    let mut corrupted = start;
    corrupted["world"]["tiles"]["resource"][0] = json!(65535);
    assert!(Session::load(&rules(), &corrupted.to_string()).is_err());
}

#[test]
fn placement_is_repeatable_uses_valid_terrain_and_does_not_advance_game_rng() {
    let old = engine(false);
    let new = engine(true);
    let same = engine(true);
    let old_world = serde_json::to_value(&*old.world().borrow()).unwrap();
    let new_world = serde_json::to_value(&*new.world().borrow()).unwrap();
    assert_eq!(old_world["players"], new_world["players"]);
    assert_eq!(old_world["rng_calls"], new_world["rng_calls"]);
    // Compare actual RNG state, which upstream omits from serialization.
    assert_eq!(old.world().borrow().rng, new.world().borrow().rng);
    assert_eq!(
        new.world().borrow().tiles.resource,
        same.world().borrow().tiles.resource
    );
    let mut tiles = new.world().borrow().tiles.clone();
    for i in 0..tiles.terrain.len() {
        tiles.terrain[i] = if i % 2 == 0 {
            Terrain::Ocean
        } else {
            Terrain::Hill
        };
        tiles.vegetation[i] = Vegetation::None;
        tiles.resource[i] = None;
    }
    fc3_core::resource::place(&mut tiles, 7);
    for (i, r) in tiles.resource.iter().enumerate() {
        if i % 2 == 0 {
            assert_eq!(*r, None);
        } else if let Some(r) = r {
            assert!([ResourceId(2), ResourceId(3)].contains(r));
        }
    }
}

#[test]
fn new_game_research_and_resource_economy_replay_across_discovery() {
    let mut game = Session::new(&rules()).unwrap();
    let snapshot = game.initial_snapshot();
    let settler = snapshot["available"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|entry| {
            let action = &entry["UnitAction"];
            (action["action_id"] == "build_city").then(|| action["unit_id"].clone())
        })
        .unwrap();
    assert_eq!(
        game.request(json!({"PerformAction": {"unit_id": settler, "action_id": "build_city"}}))
            ["result"]["errors"],
        json!([])
    );
    assert_eq!(
        game.request(json!({"SetResearch": {"tech_id": "the_wheel"}}))["result"]["errors"],
        json!([])
    );
    let mut loaded = Session::load(&rules(), &game.save().unwrap()).unwrap();
    let mut discovered = false;
    for _ in 0..80 {
        let state = game.end_turn();
        assert_eq!(state, loaded.end_turn());
        if state["view"]["researched_techs"]
            .as_array()
            .unwrap()
            .contains(&json!("the_wheel"))
        {
            discovered = true;
            break;
        }
    }
    assert!(discovered);
    let mut after_discovery = Session::load(&rules(), &game.save().unwrap()).unwrap();
    assert_eq!(game.end_turn(), after_discovery.end_turn());
    assert_eq!(game.save().unwrap(), after_discovery.save().unwrap());
}

#[test]
fn worked_bonus_resources_add_yields_without_a_road_and_unworked_tiles_do_not() {
    let game = engine(true);
    let city;
    {
        let w = game.world();
        let mut w = w.borrow_mut();
        w.tiles.resource.fill(None);
        city = w
            .cities
            .spawn("Fixture".into(), PlayerId(0), TileCoord { x: 3, y: 3 });
        w.tiles.resource[3 * 8 + 4] = Some(ResourceId(1)); // Cattle: +2 food, +1 shield.
        w.tiles.resource[0] = Some(ResourceId(0)); // Outside city radius.
    }
    game.reassign_city_tiles(city);
    let view = game.player_view(PlayerId(0));
    assert_eq!(view.own_cities[0].food_per_turn, Some(6)); // center 2 + worked grass 2 + cattle 2
    assert_eq!(view.own_cities[0].shields_per_turn, Some(3));
    assert!(view.own_cities[0]
        .worked_tiles
        .as_ref()
        .unwrap()
        .contains(&TileCoord { x: 4, y: 3 }));
    assert_eq!(
        game.calculate_tile_yield_for_player(PlayerId(0), 0, 0),
        (4, 1, 0)
    );
    {
        let w = game.world();
        let mut w = w.borrow_mut();
        w.tiles.resource[3 * 8 + 4] = Some(ResourceId(2));
    }
    assert_eq!(
        game.calculate_tile_yield_for_player(PlayerId(0), 4, 3),
        (2, 1, 4)
    );
    let old = engine(false);
    old.world().borrow_mut().tiles.resource[0] = Some(ResourceId(1));
    assert_eq!(
        old.calculate_tile_yield_for_player(PlayerId(0), 0, 0),
        (2, 1, 0)
    );
}

#[test]
fn research_reveals_horses_only_to_its_owner_and_refreshes_city_yields() {
    let mut game = engine(true);
    let city;
    {
        let w = game.world();
        let mut w = w.borrow_mut();
        w.tiles.resource.fill(None);
        city = w
            .cities
            .spawn("Fixture".into(), PlayerId(0), TileCoord { x: 3, y: 3 });
        w.tiles.resource[3 * 8 + 4] = Some(ResourceId(3));
        for p in [PlayerId(0), PlayerId(1)] {
            w.tiles.set_visibility(p, 4, 3, Visibility::Revealed);
        }
        w.players[0].science = 40;
    }
    game.reassign_city_tiles(city);
    assert!(game
        .player_view(PlayerId(0))
        .visible_tiles
        .iter()
        .all(|t| t.resource.is_none()));
    assert_eq!(
        game.calculate_tile_yield_for_player(PlayerId(0), 4, 3),
        (2, 1, 0)
    );
    assert!(game
        .submit_command(
            PlayerId(0),
            Command::SetResearch {
                tech_id: "the_wheel".into()
            }
        )
        .is_ok());
    let result = game.submit_command(PlayerId(0), Command::EndTurn);
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, Event::TechResearched { tech_id, .. } if tech_id == "the_wheel")));
    assert_eq!(
        game.calculate_tile_yield_for_player(PlayerId(0), 4, 3),
        (2, 1, 1)
    );
    assert_eq!(
        game.player_view(PlayerId(0)).own_cities[0].commerce_per_turn,
        Some(2)
    );
    assert_eq!(
        game.player_view(PlayerId(0))
            .visible_tiles
            .iter()
            .find(|t| t.coord == TileCoord { x: 4, y: 3 })
            .unwrap()
            .resource
            .as_deref(),
        Some("Horses")
    );
    assert!(game
        .player_view(PlayerId(1))
        .visible_tiles
        .iter()
        .all(|t| t.resource.is_none()));
    game.world()
        .borrow_mut()
        .tiles
        .set_visibility(PlayerId(0), 4, 3, Visibility::Unseen);
    assert!(game
        .player_view(PlayerId(0))
        .visible_tiles
        .iter()
        .all(|t| t.coord != TileCoord { x: 4, y: 3 }));
}
