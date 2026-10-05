//! Small, single-player sandbox adapter. All legality stays in FreeC3.
use fc3_core::engine::{Engine, GameConfig};
use fc3_core::id::UnitId;
use fc3_core::protocol::{AvailableCommand, Command, CommandResult};
use fc3_core::types::{PlayerId, TileCoord};
use fc3_core::world::WorldConfig;
use serde_json::{json, Value};
use std::path::Path;

const PLAYER: PlayerId = PlayerId(0);

pub struct Session {
    engine: Engine,
}

impl Session {
    pub fn new(rules: &Path) -> Result<Self, String> {
        let rules = rules.canonicalize().map_err(|e| e.to_string())?;
        let engine = Engine::new_game(&GameConfig {
            world: WorldConfig {
                width: 16,
                height: 16,
                wrap_x: false,
                wrap_y: false,
                num_players: 1,
                seed: 42,
            },
            mod_paths: vec![rules.to_str().ok_or("rules path is not UTF-8")?.into()],
            units_per_player: vec!["settler".into()],
            max_turns: None,
        })
        .map_err(|e| e.to_string())?;
        Ok(Self { engine })
    }

    /// Only expose immediate steps, never a queued multi-tile destination.
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
        json!({
            "view": self.engine.player_view(PLAYER),
            "moves": moves,
            "result": result,
        })
    }

    pub fn initial_snapshot(&mut self) -> Value {
        self.snapshot(CommandResult::with_events(vec![]))
    }

    pub fn move_unit(&mut self, unit: UnitId, destination: TileCoord) -> Value {
        let allowed = self
            .moves()
            .iter()
            .any(|(id, destinations)| *id == unit && destinations.contains(&destination));
        let result = if allowed {
            self.engine.submit_command(
                PLAYER,
                Command::MoveUnit {
                    unit_id: unit,
                    destination,
                },
            )
        } else {
            CommandResult::err(fc3_core::protocol::GameError::Custom(
                "Choose a highlighted adjacent tile; otherwise end the turn to refresh movement."
                    .into(),
            ))
        };
        self.snapshot(result)
    }

    /// End Turn explicitly skips unused movement in this one-player sandbox.
    pub fn end_turn(&mut self) -> Value {
        let mut events = vec![];
        for command in self.engine.available_commands(PLAYER) {
            if let AvailableCommand::Skip { unit_id } = command {
                let result = self
                    .engine
                    .submit_command(PLAYER, Command::SkipUnit { unit_id });
                if !result.is_ok() {
                    return self.snapshot(result);
                }
                events.extend(result.events);
            }
        }
        let result = self.engine.submit_command(PLAYER, Command::EndTurn);
        events.extend(result.events);
        self.snapshot(CommandResult {
            events,
            errors: result.errors,
        })
    }
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
        assert_eq!(before.known_units.len(), 1);
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
    fn end_turn_can_skip_unused_movement_and_new_game_resets_the_sandbox() {
        let mut game = session();
        assert_eq!(game.end_turn()["view"]["turn"], 2);
        assert_eq!(game.end_turn()["view"]["turn"], 3);
        assert_eq!(session().initial_snapshot()["view"]["turn"], 1);
        assert!(Session::new(Path::new("/nonexistent-civ3touch-rules")).is_err());
    }
}
