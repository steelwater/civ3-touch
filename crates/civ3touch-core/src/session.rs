//! Platform-neutral playable session. FreeC3 owns rules; this adapter owns queues and replay.
use fc3_core::ai::{Agent, SimpleAgent};
use fc3_core::city::ProductionItem;
use fc3_core::engine::{Engine, GameConfig};
use fc3_core::id::{CityId, UnitId};
use fc3_core::protocol::{AvailableCommand, Command, CommandResult, Event, GameError};
use fc3_core::types::{PlayerId, TileCoord};
use fc3_core::world::WorldConfig;
use serde_json::{json, Value};
use std::path::Path;

const PLAYER: PlayerId = PlayerId(0);
const SAVE_VERSION: u64 = 1;
// Change this contract whenever the rules, session policy or engine version changes.
const RULESET: &str = "freec3-90fc7ee-civ3touch-m3-v2";
const MAX_ACTIONS: usize = 20_000;
pub const MAX_SAVE_BYTES: usize = 16 * 1024 * 1024;

pub struct Session {
    engine: Engine,
    queues: Vec<(CityId, Vec<ProductionItem>)>,
    journal: Vec<Value>,
    rules_identity: std::collections::BTreeMap<String, String>,
}

impl Session {
    pub fn new(rules: &Path) -> Result<Self, String> {
        let rules = rules.canonicalize().map_err(|e| e.to_string())?;
        let mut rules_identity = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(&rules).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "lua")
            {
                rules_identity.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read_to_string(entry.path()).map_err(|e| e.to_string())?,
                );
            }
        }
        let engine = Engine::new_game(&GameConfig {
            world: WorldConfig {
                width: 16,
                height: 16,
                wrap_x: false,
                wrap_y: false,
                num_players: 2,
                seed: 42,
            },
            mod_paths: vec![rules.to_str().ok_or("rules path is not UTF-8")?.into()],
            units_per_player: vec!["settler".into(), "worker".into(), "warrior".into()],
            max_turns: None,
        })
        .map_err(|e| e.to_string())?;
        Ok(Self {
            engine,
            queues: vec![],
            journal: vec![],
            rules_identity,
        })
    }

    fn moves(&mut self) -> Vec<(UnitId, Vec<TileCoord>)> {
        self.engine
            .available_commands(PLAYER)
            .into_iter()
            .filter_map(|command| match command {
                AvailableCommand::Move {
                    unit_id,
                    valid_destinations,
                } => {
                    let steps = valid_destinations
                        .into_iter()
                        .filter(|&destination| {
                            self.engine
                                .query_path(PLAYER, unit_id, destination)
                                .is_some_and(|path| path.tiles.len() == 2)
                        })
                        .collect();
                    Some((unit_id, steps))
                }
                _ => None,
            })
            .collect()
    }

    pub fn snapshot(&mut self, result: CommandResult) -> Value {
        let moves: Vec<_> = self
            .moves()
            .into_iter()
            .map(|(id, destinations)| json!({"unit_id": id, "destinations": destinations}))
            .collect();
        json!({"view": self.engine.player_view(PLAYER), "moves": moves,
            "available": self.engine.available_commands(PLAYER), "queues": self.queues,
            "game_over": self.engine.is_game_over(), "result": result})
    }
    pub fn initial_snapshot(&mut self) -> Value {
        self.snapshot(CommandResult::with_events(vec![]))
    }
    pub fn move_unit(&mut self, unit: UnitId, destination: TileCoord) -> Value {
        self.request(json!(Command::MoveUnit {
            unit_id: unit,
            destination
        }))
    }
    pub fn end_turn(&mut self) -> Value {
        self.request(json!(Command::EndTurn))
    }

    pub fn request(&mut self, request: Value) -> Value {
        if self.journal.len() >= MAX_ACTIONS {
            return self.snapshot(error(
                "This prototype save has reached its 20,000-action limit.",
            ));
        }
        let result = self.dispatch(&request);
        // Retain rejected commands too: replay checks their exact outcomes, and never
        // silently assumes an upstream error meant there was no state mutation.
        self.journal
            .push(json!({"request": request, "result": result}));
        self.snapshot(result)
    }

    fn dispatch(&mut self, request: &Value) -> CommandResult {
        if self.engine.is_game_over().is_some() || !self.engine.is_player_alive(PLAYER) {
            return error("The game has ended. Start a new game or load a save.");
        }
        if let Some(queue) = request.get("QueueProduction") {
            let parsed = (|| -> Result<_, serde_json::Error> {
                Ok((
                    serde_json::from_value::<CityId>(queue["city_id"].clone())?,
                    serde_json::from_value::<ProductionItem>(queue["item"].clone())?,
                ))
            })();
            let Ok((city, item)) = parsed else {
                return error("Invalid production request");
            };
            if !self.production_allowed(city, &item) {
                return error("Production is not available");
            }
            let pending = self
                .queues
                .iter()
                .find(|(id, _)| *id == city)
                .map_or(0, |(_, q)| q.len());
            if pending >= 8 {
                return error("A city can queue at most eight items");
            }
            if let Some((_, items)) = self.queues.iter_mut().find(|(id, _)| *id == city) {
                items.push(item);
            } else {
                self.queues.push((city, vec![item]));
            }
            return self.advance_queues(&[]);
        }
        if let Some(id) = request.get("ClearQueue") {
            let Ok(city) = serde_json::from_value::<CityId>(id.clone()) else {
                return error("Invalid city");
            };
            self.queues.retain(|(id, _)| *id != city);
            return CommandResult::with_events(vec![]);
        }
        let Ok(command) = serde_json::from_value::<Command>(request.clone()) else {
            return error("Invalid game command");
        };
        if matches!(command, Command::EndTurn) {
            return self.finish_round();
        }
        let allowed = match &command {
            Command::MoveUnit {
                unit_id,
                destination,
            } => self
                .moves()
                .iter()
                .any(|(id, destinations)| id == unit_id && destinations.contains(destination)),
            Command::SetProduction { city_id, item } => self.production_allowed(*city_id, item),
            _ => self
                .engine
                .available_commands(PLAYER)
                .iter()
                .any(|available| match (available, &command) {
                    (
                        AvailableCommand::UnitAction {
                            unit_id: a,
                            action_id: b,
                            ..
                        },
                        Command::PerformAction { unit_id, action_id },
                    ) => a == unit_id && b == action_id,
                    (
                        AvailableCommand::Attack { unit_id, targets },
                        Command::AttackUnit { attacker, defender },
                    ) => {
                        unit_id == attacker
                            && targets.contains(defender)
                            && self
                                .engine
                                .player_view(PLAYER)
                                .known_units
                                .iter()
                                .any(|u| u.id == *attacker && u.attack > 0)
                    }
                    (
                        AvailableCommand::Fortify { unit_id: a },
                        Command::FortifyUnit { unit_id },
                    )
                    | (AvailableCommand::Skip { unit_id: a }, Command::SkipUnit { unit_id }) => {
                        a == unit_id
                    }
                    (
                        AvailableCommand::SetResearch { options },
                        Command::SetResearch { tech_id },
                    ) => options.iter().any(|t| t.id == *tech_id),
                    _ => false,
                }),
        };
        if !allowed {
            return error("Choose an available action or a highlighted adjacent tile.");
        }
        self.engine.submit_command(PLAYER, command)
    }

    fn production_allowed(&self, city: CityId, item: &ProductionItem) -> bool {
        self.engine.available_commands(PLAYER).iter().any(|a| matches!(a,
            AvailableCommand::SetProduction { city_id, options } if *city_id == city && options.iter().any(|o| o.item == *item)))
    }

    fn advance_queues(&mut self, completed: &[CityId]) -> CommandResult {
        let cities = self.engine.player_view(PLAYER).own_cities;
        self.queues
            .retain(|(id, _)| cities.iter().any(|c| c.id == *id));
        let mut result = CommandResult::with_events(vec![]);
        for city in cities
            .iter()
            .filter(|c| c.producing.is_none() || completed.contains(&c.id))
        {
            let Some(index) = self
                .queues
                .iter()
                .position(|(id, items)| *id == city.id && !items.is_empty())
            else {
                continue;
            };
            let item = self.queues[index].1[0].clone();
            if !self.production_allowed(city.id, &item) {
                result.errors.push(GameError::Custom(format!(
                    "Queued item in {} is no longer available; queue cleared.",
                    city.name
                )));
                self.queues[index].1.clear();
                continue;
            }
            let applied = self.engine.submit_command(
                PLAYER,
                Command::SetProduction {
                    city_id: city.id,
                    item,
                },
            );
            if applied.is_ok() {
                if completed.contains(&city.id) {
                    // Queue advancement is continuation, not a manual change. Keep
                    // the overflow FreeC3 retained before auto-selecting its next item.
                    let world = self.engine.world();
                    let mut world = world.borrow_mut();
                    let index = world.cities.get(city.id).unwrap();
                    world.cities.shield_stockpile[index] = city.shield_stockpile.unwrap();
                }
                self.queues[index].1.remove(0);
            }
            result.events.extend(applied.events);
            result.errors.extend(applied.errors);
        }
        result
    }

    fn skip_and_end(&mut self, player: PlayerId) -> CommandResult {
        for command in self.engine.available_commands(player) {
            if let AvailableCommand::Skip { unit_id } = command {
                let result = self
                    .engine
                    .submit_command(player, Command::SkipUnit { unit_id });
                if !result.is_ok() {
                    return result;
                }
            }
        }
        self.engine.submit_command(player, Command::EndTurn)
    }

    fn finish_round(&mut self) -> CommandResult {
        let human = self.skip_and_end(PLAYER);
        if !human.is_ok() {
            return human;
        }
        // Research completes at human turn-end, before AI turn-start events.
        let research: Vec<_> = human
            .events
            .iter()
            .filter(
                |event| matches!(event, Event::TechResearched { player, .. } if *player == PLAYER),
            )
            .cloned()
            .collect();
        // Restart the existing AI from the current world RNG each round. Its future
        // decisions depend only on replayed state, not an unsaved private RNG stream.
        let rng = self.engine.world().borrow().rng.clone();
        let mut ai = SimpleAgent::new(rng);
        let mut last = human;
        for _ in 0..256 {
            let player = self.engine.current_player();
            if player == PLAYER || self.engine.is_game_over().is_some() {
                break;
            }
            let command = ai.decide(
                &self.engine.player_view(player),
                &self.engine.available_commands(player),
            );
            last = self.engine.submit_command(player, command);
            if !last.is_ok() {
                last = self.skip_and_end(player);
                break;
            }
        }
        if self.engine.current_player() != PLAYER && self.engine.is_game_over().is_none() {
            last = self.skip_and_end(self.engine.current_player());
        }
        // AI movement/combat events can expose hidden map information. Return only
        // the new human view and human turn-start economy events, never AI actions.
        if self.engine.current_player() == PLAYER {
            last.events.retain(|e| {
                !matches!(
                    e,
                    fc3_core::protocol::Event::UnitMoved { .. }
                        | fc3_core::protocol::Event::TilesRevealed {
                            player: PlayerId(1),
                            ..
                        }
                )
            });
        } else {
            last.events.clear();
        }
        // The final AI EndTurn may also contain AI research. Replace research
        // events with only the human completion retained above.
        last.events
            .retain(|event| !matches!(event, Event::TechResearched { .. }));
        last.events.splice(0..0, research);
        let completed: Vec<_> = last
            .events
            .iter()
            .filter_map(|event| match event {
                fc3_core::protocol::Event::ProductionComplete { city_id, .. } => Some(*city_id),
                _ => None,
            })
            .collect();
        let queues = self.advance_queues(&completed);
        last.events.extend(queues.events);
        last.errors.extend(queues.errors);
        last
    }

    pub fn save(&self) -> Result<String, String> {
        let world =
            serde_json::to_value(&*self.engine.world().borrow()).map_err(|e| e.to_string())?;
        let text = json!({"version": SAVE_VERSION, "ruleset": RULESET, "rules": self.rules_identity, "journal": self.journal,
            "world": world, "queues": self.queues, "turn": self.engine.current_turn(),
            "player": self.engine.current_player()})
        .to_string();
        if text.len() > MAX_SAVE_BYTES {
            return Err("Save exceeds the prototype's 16 MiB limit".into());
        }
        Ok(text)
    }

    pub fn load(rules: &Path, text: &str) -> Result<Self, String> {
        if text.len() > MAX_SAVE_BYTES {
            return Err("Save exceeds the prototype's 16 MiB limit".into());
        }
        let saved: Value = serde_json::from_str(text).map_err(|_| "Save is not valid JSON")?;
        if saved["version"] != SAVE_VERSION || saved["ruleset"] != RULESET {
            return Err("Unsupported save version or ruleset".into());
        }
        let journal = saved["journal"]
            .as_array()
            .ok_or("Save journal is missing")?;
        if journal.len() > MAX_ACTIONS {
            return Err("Save action limit exceeded".into());
        }
        let mut game = Self::new(rules)?;
        if json!(game.rules_identity) != saved["rules"] {
            return Err("Save rules differ from the installed rules".into());
        }
        for entry in journal {
            let result = game.dispatch(&entry["request"]);
            if json!(result) != entry["result"] {
                return Err("Save replay diverged; existing session was preserved".into());
            }
        }
        let world =
            serde_json::to_value(&*game.engine.world().borrow()).map_err(|e| e.to_string())?;
        if world != saved["world"]
            || json!(game.queues) != saved["queues"]
            || json!(game.engine.current_turn()) != saved["turn"]
            || json!(game.engine.current_player()) != saved["player"]
        {
            return Err("Save state verification failed; existing session was preserved".into());
        }
        game.journal = journal.clone();
        Ok(game)
    }
}
fn error(message: &str) -> CommandResult {
    CommandResult::err(GameError::Custom(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fc3_core::protocol::Event;
    fn session() -> Session {
        Session::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/freec3/mods/base"))
            .unwrap()
    }
    #[test]
    fn starting_settler_moves_one_tile_and_end_turn_refreshes_it() {
        let mut game = session();
        let before = game.engine.player_view(PLAYER);
        assert!(!before.visible_tiles.is_empty());
        assert_eq!(before.known_units.len(), 3);
        let unit = &before.known_units[0];
        assert_eq!(unit.unit_type_name, "settler");
        let (id, destinations) = game.moves().remove(0);
        assert!(!destinations.is_empty());
        let destination = destinations[0];
        let moved = game.move_unit(id, destination);
        assert_eq!(moved["result"]["errors"], json!([]));
        let events: Vec<Event> = serde_json::from_value(moved["result"]["events"].clone()).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::UnitMoved { .. }))
                .count(),
            1
        );
        let after = &game.engine.player_view(PLAYER).known_units[0];
        assert_eq!(after.position, destination);
        assert!(after.movement < unit.movement);
        assert_eq!(after.destination, None);
        let ended = game.end_turn();
        assert_eq!(ended["view"]["turn"], 2);
        assert_eq!(ended["result"]["errors"], json!([]));
        assert_eq!(
            game.engine.player_view(PLAYER).known_units[0].movement,
            unit.max_movement
        );
    }
    #[test]
    fn invalid_stale_distant_and_exhausted_moves_do_not_change_state_or_queue_paths() {
        let mut game = session();
        let (id, destinations) = game.moves().remove(0);
        let before = game.initial_snapshot()["view"].clone();
        for (unit, destination) in [
            (
                UnitId {
                    index: id.index,
                    generation: id.generation + 1,
                },
                destinations[0],
            ),
            (id, TileCoord { x: 999, y: 999 }),
            (id, game.engine.player_view(PLAYER).known_units[0].position),
        ] {
            let rejected = game.move_unit(unit, destination);
            assert_ne!(rejected["result"]["errors"], json!([]));
            assert_eq!(rejected["view"], before);
        }
        // Consume movement using engine-advertised steps, then reject any further move.
        while let Some((unit, steps)) = game.moves().into_iter().find(|(_, d)| !d.is_empty()) {
            game.move_unit(unit, steps[0]);
        }
        let before = game.initial_snapshot()["view"].clone();
        let rejected = game.move_unit(id, destinations[0]);
        assert_ne!(rejected["result"]["errors"], json!([]));
        assert_eq!(rejected["view"], before);
        assert_eq!(
            game.engine.player_view(PLAYER).known_units[0].destination,
            None
        );
    }
    #[test]
    fn new_game_restores_native_initial_facing_after_a_move() {
        let mut game = session();
        assert_eq!(
            game.initial_snapshot()["view"]["known_units"][0]["direction"],
            "SE"
        );
        let (id, destinations) = game.moves().remove(0);
        let moved = game.move_unit(id, destinations[0]);
        assert_eq!(moved["result"]["errors"], json!([]));
        let native_direction = game.engine.player_view(PLAYER).known_units[0].direction;
        assert_ne!(native_direction, fc3_core::types::Direction::SE);
        assert_eq!(
            moved["view"]["known_units"][0]["direction"],
            json!(native_direction)
        );
        assert_eq!(
            session().initial_snapshot()["view"]["known_units"][0]["direction"],
            "SE"
        );
    }

    #[test]
    fn end_turn_can_skip_unused_movement_and_new_game_resets_the_sandbox() {
        let mut game = session();
        assert_eq!(game.end_turn()["view"]["turn"], 2);
        assert_eq!(game.end_turn()["view"]["turn"], 3);
        assert_eq!(session().initial_snapshot()["view"]["turn"], 1);
        assert!(Session::new(Path::new("/nonexistent-civ3touch-rules")).is_err());
    }
}

#[cfg(test)]
mod playable_tests {
    use super::*;
    fn rules() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/freec3/mods/base")
    }
    fn game() -> Session {
        Session::new(&rules()).unwrap()
    }
    fn action(game: &mut Session, id: &str) {
        let command = game
            .engine
            .available_commands(PLAYER)
            .into_iter()
            .find_map(|a| match a {
                AvailableCommand::UnitAction {
                    unit_id, action_id, ..
                } if action_id == id => Some(Command::PerformAction { unit_id, action_id }),
                _ => None,
            })
            .expect("action must be legal");
        assert_eq!(game.request(json!(command))["result"]["errors"], json!([]));
    }
    fn round_trip(game: &mut Session) -> Session {
        let mut restored = Session::load(&rules(), &game.save().unwrap()).unwrap();
        assert_eq!(game.initial_snapshot(), restored.initial_snapshot());
        assert_eq!(game.save().unwrap(), restored.save().unwrap());
        restored
    }
    #[test]
    fn queued_production_keeps_completion_overflow_but_manual_changes_reset_shields() {
        let mut game = game();
        action(&mut game, "build_city");
        let city = game.engine.player_view(PLAYER).own_cities[0].id;
        let options = game
            .engine
            .available_commands(PLAYER)
            .into_iter()
            .find_map(|a| match a {
                AvailableCommand::SetProduction { options, .. } => Some(options),
                _ => None,
            })
            .unwrap();
        let warrior = options
            .iter()
            .find(|o| o.name == "warrior")
            .unwrap()
            .item
            .clone();
        let worker = options
            .iter()
            .find(|o| o.name == "worker")
            .unwrap()
            .item
            .clone();
        game.request(json!(Command::SetProduction {
            city_id: city,
            item: warrior.clone()
        }));
        game.request(json!({"QueueProduction": {"city_id": city, "item": worker}}));
        // Arrange an exact three-shield overflow at this turn's completion.
        let view = game.engine.player_view(PLAYER).own_cities.remove(0);
        {
            let world = game.engine.world();
            let mut world = world.borrow_mut();
            let index = world.cities.get(city).unwrap();
            world.cities.shield_stockpile[index] =
                view.production_cost.unwrap() - view.shields_per_turn.unwrap() + 3;
        }
        let result = game.end_turn();
        assert_eq!(result["result"]["errors"], json!([]));
        assert!(result["result"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.get("ProductionComplete").is_some()));
        let view = game.engine.player_view(PLAYER).own_cities.remove(0);
        assert_eq!(view.producing.as_deref(), Some("worker"));
        assert_eq!(view.shield_stockpile, Some(3));
        assert!(game.queues[0].1.is_empty());
        game.request(json!(Command::SetProduction {
            city_id: city,
            item: warrior
        }));
        assert_eq!(
            game.engine.player_view(PLAYER).own_cities[0].shield_stockpile,
            Some(0)
        );
    }

    #[test]
    fn simultaneous_human_and_ai_research_only_reports_the_human_completion() {
        let mut game = game();
        action(&mut game, "build_city");
        game.end_turn();
        {
            let world = game.engine.world();
            let mut world = world.borrow_mut();
            let cost = world.tech_registry.get("bronze_working").unwrap().cost;
            for player in &mut world.players {
                player.researching = Some("bronze_working".into());
                player.science = cost;
            }
        }
        let result = game.end_turn();
        assert_eq!(result["result"]["errors"], json!([]));
        for player in [PLAYER, PlayerId(1)] {
            assert!(game
                .engine
                .player_view(player)
                .researched_techs
                .contains(&"bronze_working".into()));
        }
        let research: Vec<_> = result["result"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|event| event.get("TechResearched"))
            .collect();
        assert_eq!(
            research,
            vec![&json!({"player": PLAYER, "tech_id": "bronze_working"})]
        );
    }

    #[test]
    fn economy_workers_research_and_queued_units_survive_reload_and_continue() {
        let mut game = game();
        action(&mut game, "build_city");
        let city = game.engine.player_view(PLAYER).own_cities[0].id;
        let warrior = game
            .engine
            .available_commands(PLAYER)
            .into_iter()
            .find_map(|a| match a {
                AvailableCommand::SetProduction { options, .. } => options
                    .into_iter()
                    .find(|o| o.name == "warrior")
                    .map(|o| o.item),
                _ => None,
            })
            .unwrap();
        for _ in 0..2 {
            assert_eq!(
                game.request(json!({"QueueProduction": {"city_id": city, "item": warrior}}))
                    ["result"]["errors"],
                json!([])
            );
        }
        assert_eq!(
            game.request(json!(Command::SetResearch {
                tech_id: "bronze_working".into()
            }))["result"]["errors"],
            json!([])
        );
        action(&mut game, "build_road");
        let mut restored = round_trip(&mut game);
        let before_units = game
            .engine
            .player_view(PLAYER)
            .known_units
            .iter()
            .filter(|u| u.owner == PLAYER)
            .count();
        let mut research_completed = false;
        let mut production_completed = false;
        let mut worker_completed = false;
        for _ in 0..45 {
            let result = game.end_turn();
            assert_eq!(result, restored.end_turn());
            for event in result["result"]["events"].as_array().unwrap() {
                if let Some(research) = event.get("TechResearched") {
                    assert_eq!(research["player"], json!(PLAYER));
                    assert_eq!(research["tech_id"], "bronze_working");
                    research_completed = true;
                }
                if let Some(turn) = event.get("TurnStarted") {
                    assert_eq!(turn["player"], json!(PLAYER));
                }
                production_completed |= event.get("ProductionComplete").is_some();
                worker_completed |= event.get("ActionCompleted").is_some();
            }
        }
        assert!(research_completed && production_completed && worker_completed);
        let view = game.engine.player_view(PLAYER);
        assert!(view.researched_techs.contains(&"bronze_working".into()));
        assert!(view.visible_tiles.iter().any(|t| t.road_level > 0));
        assert!(
            view.known_units
                .iter()
                .filter(|u| u.owner == PLAYER)
                .count()
                >= before_units + 2
        );
        assert!(view.own_cities[0].population > 1);
        assert!(game.queues.iter().all(|(_, items)| items.is_empty()));
        assert!(view.own_cities[0].commerce_per_turn.unwrap() > 0);
        assert!(!game.engine.player_view(PlayerId(1)).own_cities.is_empty());
        round_trip(&mut game);
    }
    #[test]
    fn workers_finish_mines_and_irrigation_on_legal_terrain_after_reload() {
        for (action_id, terrain, expected) in [
            ("build_mine", "Hill", 1),
            ("build_irrigation", "Grassland", 2),
        ] {
            let mut game = game();
            let worker = game
                .engine
                .player_view(PLAYER)
                .known_units
                .iter()
                .find(|u| u.unit_type_name == "worker")
                .unwrap()
                .id;
            let target = game
                .engine
                .player_view(PLAYER)
                .visible_tiles
                .iter()
                .filter(|t| {
                    format!("{:?}", t.terrain) == terrain
                        && (expected == 1 || format!("{:?}", t.vegetation) == "None")
                })
                .filter_map(|t| {
                    game.engine
                        .query_path(PLAYER, worker, t.coord)
                        .map(|path| (t.coord, path.tiles.len()))
                })
                .min_by_key(|(_, length)| *length)
                .unwrap()
                .0;
            for _ in 0..20 {
                let position = game
                    .engine
                    .player_view(PLAYER)
                    .known_units
                    .iter()
                    .find(|u| u.id == worker)
                    .unwrap()
                    .position;
                if position == target {
                    break;
                }
                let path = game.engine.query_path(PLAYER, worker, target).unwrap();
                let result = game.move_unit(worker, path.tiles[1]);
                assert_eq!(result["result"]["errors"], json!([]));
                game.end_turn();
            }
            action(&mut game, action_id);
            let mut restored = round_trip(&mut game);
            for _ in 0..3 {
                assert_eq!(game.end_turn(), restored.end_turn());
            }
            let view = game.engine.player_view(PLAYER);
            let tile = view
                .visible_tiles
                .iter()
                .find(|t| t.coord == target)
                .unwrap();
            assert_eq!(json!(tile.improvement), json!(expected));
        }
    }

    #[test]
    fn researched_buildings_can_be_queued_and_completed() {
        let mut game = game();
        action(&mut game, "build_city");
        let city = game.engine.player_view(PLAYER).own_cities[0].id;
        let granary = ProductionItem::Building {
            building_id: "granary".into(),
        };
        assert!(!game.production_allowed(city, &granary));
        game.request(json!(Command::SetResearch {
            tech_id: "pottery".into()
        }));
        for _ in 0..80 {
            if game.production_allowed(city, &granary) {
                break;
            }
            game.end_turn();
        }
        assert!(game.production_allowed(city, &granary));
        let queued = game.request(json!({"QueueProduction": {"city_id": city, "item": granary}}));
        assert_eq!(queued["result"]["errors"], json!([]));
        let mut loaded = round_trip(&mut game);
        for _ in 0..40 {
            assert_eq!(game.end_turn(), loaded.end_turn());
        }
        assert!(game.engine.player_view(PLAYER).own_cities[0]
            .buildings
            .as_ref()
            .unwrap()
            .contains(&"granary".into()));
        assert!(!game.production_allowed(city, &granary));
    }

    #[test]
    fn damaged_incompatible_or_tampered_saves_fail_without_replacing_the_live_game() {
        let mut game = game();
        game.end_turn();
        let original = game.save().unwrap();
        assert!(Session::load(&rules(), "{").is_err());
        for (key, value) in [
            ("version", json!(99)),
            ("ruleset", json!("future")),
            ("ruleset", json!("freec3-90fc7ee-civ3touch-m3-v1")),
            ("rules", json!({})),
            ("world", json!({})),
            ("journal", json!([])),
        ] {
            let mut saved: Value = serde_json::from_str(&original).unwrap();
            saved[key] = value;
            assert!(
                Session::load(&rules(), &saved.to_string()).is_err(),
                "{key}"
            );
        }
        assert_eq!(game.save().unwrap(), original);
    }
    #[test]
    fn exploration_and_combat_replay_preserve_future_random_outcomes() {
        let mut game = game();
        let mut agent = SimpleAgent::new(game.engine.world().borrow().rng.clone());
        for _ in 0..120 {
            if game.engine.is_game_over().is_some() {
                break;
            }
            for _ in 0..128 {
                let command = agent.decide(
                    &game.engine.player_view(PLAYER),
                    &game.engine.available_commands(PLAYER),
                );
                if matches!(command, Command::EndTurn) {
                    break;
                }
                let result = game.request(json!(command));
                if result["result"]["errors"] != json!([]) {
                    break;
                }
            }
            let mut loaded = round_trip(&mut game);
            assert_eq!(game.end_turn(), loaded.end_turn());
            assert_eq!(game.save().unwrap(), loaded.save().unwrap());
            if game
                .engine
                .event_log
                .iter()
                .any(|(_, _, c)| matches!(c, Command::AttackUnit { .. }))
            {
                return;
            }
        }
        panic!("Generated world must permit an encounter and combat through legal commands");
    }
}
