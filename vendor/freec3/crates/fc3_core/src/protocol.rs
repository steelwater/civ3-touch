use serde::{Deserialize, Serialize};

use crate::id::{CityId, UnitId};
use crate::tile::{Terrain, Vegetation, Visibility};
use crate::types::{Direction, ImprovementId, PlayerId, TileCoord};

/// A command submitted by a player to the engine.
///
/// ## MoveUnit semantics
///
/// `MoveUnit { destination }` can target any tile on the map — the engine
/// pathfinds internally using A* with terrain costs from Lua hooks. The unit
/// moves tile-by-tile along the shortest path until one of:
///
/// - Movement points are exhausted (unit stops, remaining path lost).
/// - An enemy is spotted in newly revealed tiles → `MoveInterrupted { reason: EnemySpotted }`.
/// - The unit enters a ZoC-to-ZoC transition → `MoveInterrupted { reason: ZoneOfControl }`,
///   movement set to 0.
/// - The destination is reached → final `UnitMoved` event.
///
/// One `UnitMoved` event is emitted per tile step, with the current
/// `movement_left` after each step. Fog-of-war is updated after each step;
/// newly revealed tiles are emitted as `TilesRevealed` events.
///
/// If the destination is unreachable (no path exists), `MoveBlocked` is returned.
///
/// ## Movement cost convention (integer-thirds)
///
/// Movement uses integer-thirds representation: 1 full movement point = 3
/// internal units. Terrain costs from Lua: road = 1, grassland/plains = 3,
/// hill/forest/jungle = 6. A warrior with 1 movement point has 3 internal
/// units and can traverse one grassland tile (cost 3) or use the "any
/// movement" rule to enter a hill (cost 6) if it has any movement > 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Move a unit to any reachable tile. Engine pathfinds and executes
    /// tile-by-tile. See module-level docs for full semantics.
    MoveUnit {
        unit_id: UnitId,
        destination: TileCoord,
    },
    AttackUnit {
        attacker: UnitId,
        defender: UnitId,
    },
    FortifyUnit {
        unit_id: UnitId,
    },
    SetProduction {
        city_id: CityId,
        item: crate::city::ProductionItem,
    },
    SkipUnit {
        unit_id: UnitId,
    },
    PerformAction {
        unit_id: UnitId,
        action_id: String,
    },
    SetResearch {
        tech_id: String,
    },
    EndTurn,
}

/// An event produced by the engine in response to a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    UnitMoved {
        unit_id: UnitId,
        from: TileCoord,
        to: TileCoord,
        movement_left: i32,
    },
    MoveBlocked {
        unit_id: UnitId,
        reason: String,
    },
    UnitFortified {
        unit_id: UnitId,
    },
    UnitSkipped {
        unit_id: UnitId,
    },
    TurnStarted {
        player: PlayerId,
        turn: u32,
    },
    CityFounded {
        city_id: CityId,
        at: TileCoord,
        name: String,
        owner: PlayerId,
    },
    UnitConsumed {
        unit_id: UnitId,
        reason: String,
    },
    CombatStarted {
        attacker: UnitId,
        defender: UnitId,
        tile: TileCoord,
    },
    CombatRound {
        round: u32,
        attacker_hp: i32,
        defender_hp: i32,
    },
    CombatResolved {
        winner: UnitId,
        loser: UnitId,
        winner_hp: i32,
    },
    UnitDestroyed {
        unit_id: UnitId,
        at: TileCoord,
    },
    CityGrew {
        city_id: CityId,
        new_population: i32,
    },
    CityStarved {
        city_id: CityId,
        new_population: i32,
    },
    ProductionSet {
        city_id: CityId,
        item_name: String,
        cost: i32,
    },
    ProductionComplete {
        city_id: CityId,
        item_name: String,
    },
    UnitProduced {
        city_id: CityId,
        unit_id: UnitId,
        unit_type: String,
        at: TileCoord,
    },
    CityCaptured {
        city_id: CityId,
        old_owner: PlayerId,
        new_owner: PlayerId,
        new_population: i32,
    },
    TilesRevealed {
        player: PlayerId,
        tiles: Vec<TileCoord>,
    },
    /// Movement was interrupted before reaching the destination.
    /// The unit stopped at `at` and `remaining_path` contains the tiles
    /// it would have traversed. Clients can use `remaining_path` to display
    /// a ghosted "planned path" on the UI.
    MoveInterrupted {
        unit_id: UnitId,
        at: TileCoord,
        reason: MoveInterruptReason,
        remaining_path: Vec<TileCoord>,
    },
    PlayerEliminated {
        player: PlayerId,
    },
    ActionCompleted {
        unit_id: UnitId,
        action_id: String,
        at: TileCoord,
    },
    ActionStarted {
        unit_id: UnitId,
        action_id: String,
        turns_remaining: i32,
        at: TileCoord,
    },
    ResearchSet {
        player: PlayerId,
        tech_id: String,
    },
    TechResearched {
        player: PlayerId,
        tech_id: String,
    },
    BuildingCompleted {
        city_id: CityId,
        building_id: String,
    },
    DestinationSet {
        unit_id: UnitId,
        destination: TileCoord,
    },
    DestinationCleared {
        unit_id: UnitId,
    },
    GameOver {
        winner: PlayerId,
    },
}

/// Reason a multi-tile move was interrupted before reaching the destination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MoveInterruptReason {
    EnemySpotted,
    ZoneOfControl,
    UnitDestroyed,
    Custom(String),
}

/// Errors returned when a command cannot be executed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameError {
    NotYourTurn,
    InvalidUnit,
    NotEnoughMovement,
    InvalidTarget,
    Custom(String),
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::NotYourTurn => write!(f, "not your turn"),
            GameError::InvalidUnit => write!(f, "invalid unit"),
            GameError::NotEnoughMovement => write!(f, "not enough movement"),
            GameError::InvalidTarget => write!(f, "invalid target"),
            GameError::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

/// The result of submitting a command to the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub events: Vec<Event>,
    pub errors: Vec<GameError>,
}

impl CommandResult {
    /// Creates a result with a single event and no errors.
    pub fn ok(event: Event) -> Self {
        Self {
            events: vec![event],
            errors: Vec::new(),
        }
    }

    /// Creates a result with a single error and no events.
    pub fn err(error: GameError) -> Self {
        Self {
            events: Vec::new(),
            errors: vec![error],
        }
    }

    /// Creates a result with multiple events and no errors.
    pub fn with_events(events: Vec<Event>) -> Self {
        Self {
            events,
            errors: Vec::new(),
        }
    }

    /// Returns true if the command produced no errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// A command that is currently available for a player to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AvailableCommand {
    Move {
        unit_id: UnitId,
        valid_destinations: Vec<TileCoord>,
    },
    Attack {
        unit_id: UnitId,
        targets: Vec<UnitId>,
    },
    Fortify {
        unit_id: UnitId,
    },
    Skip {
        unit_id: UnitId,
    },
    UnitAction {
        unit_id: UnitId,
        action_id: String,
        name: String,
        hotkey: Option<String>,
        animation_name: Option<String>,
        icon_atlas_pos: Option<(i32, i32)>,
    },
    SetProduction {
        city_id: CityId,
        options: Vec<ProductionOption>,
    },
    SetResearch {
        options: Vec<TechOption>,
    },
    EndTurn,
}

/// A technology available for research, as presented in available commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechOption {
    pub id: String,
    pub name: String,
    pub cost: i32,
}

/// An item that can be produced by a city, as presented in available commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionOption {
    pub name: String,
    pub item: crate::city::ProductionItem,
    pub cost: i32,
}

/// Snapshot of a single tile as seen by a player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileSnapshot {
    pub coord: TileCoord,
    pub terrain: Terrain,
    pub vegetation: Vegetation,
    pub improvement: Option<ImprovementId>,
    pub road_level: u8,
    pub owner: Option<PlayerId>,
    pub visibility: Visibility,
}

/// Terrain-only snapshot for tiles adjacent to discovered territory.
/// These "fringe" tiles let the renderer show terrain slivers under fog transitions
/// without leaking gameplay-relevant information (units, improvements, ownership).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FringeTile {
    pub coord: TileCoord,
    pub terrain: Terrain,
    pub vegetation: Vegetation,
}

/// Snapshot of a single unit as seen by a player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitSnapshot {
    pub id: UnitId,
    pub unit_type_name: String,
    pub category: String,
    pub owner: PlayerId,
    pub position: TileCoord,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub movement: i32,
    pub max_movement: i32,
    pub fortified: bool,
    pub skipped: bool,
    pub current_action: Option<String>,
    pub current_action_animation: Option<String>,
    pub destination: Option<TileCoord>,
    pub direction: Direction,
    pub art_ini: Option<String>,
}

/// Snapshot of a city as seen by a player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitySnapshot {
    pub id: CityId,
    pub name: String,
    pub owner: PlayerId,
    pub position: TileCoord,
    pub population: i32,
    // Extended fields only present for the owning player
    pub food_stockpile: Option<i32>,
    pub food_per_turn: Option<i32>,
    pub shield_stockpile: Option<i32>,
    pub shields_per_turn: Option<i32>,
    pub commerce_per_turn: Option<i32>,
    pub producing: Option<String>,
    pub worked_tiles: Option<Vec<TileCoord>>,
    pub production_cost: Option<i32>,
    pub food_growth_threshold: Option<i32>,
    pub buildings: Option<Vec<String>>,
}

/// The portion of the game state visible to a single player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerView {
    pub player: PlayerId,
    pub turn: u32,
    pub gold: i32,
    pub science: i32,
    pub culture: i32,
    pub researching: Option<String>,
    pub researching_name: Option<String>,
    pub researched_techs: Vec<String>,
    pub science_per_turn: i32,
    pub gold_per_turn: i32,
    pub research_turns_left: Option<i32>,
    pub civilization: Option<String>,
    pub civ_adjective: Option<String>,
    pub civ_name: Option<String>,
    pub civ_noun: Option<String>,
    pub map_width: u32,
    pub map_height: u32,
    pub wrap_x: bool,
    pub wrap_y: bool,
    pub visible_tiles: Vec<TileSnapshot>,
    pub fringe_tiles: Vec<FringeTile>,
    pub known_units: Vec<UnitSnapshot>,
    pub own_cities: Vec<CitySnapshot>,
    pub known_cities: Vec<CitySnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::GenId;

    #[test]
    fn test_command_serialization_roundtrip() {
        let cmd = Command::MoveUnit {
            unit_id: GenId {
                index: 1,
                generation: 0,
            },
            destination: TileCoord { x: 3, y: 4 },
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        // Verify by re-serializing
        assert_eq!(json, serde_json::to_string(&back).unwrap());
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let event = Event::UnitMoved {
            unit_id: GenId {
                index: 0,
                generation: 0,
            },
            from: TileCoord { x: 1, y: 1 },
            to: TileCoord { x: 2, y: 1 },
            movement_left: 0,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(json, serde_json::to_string(&back).unwrap());
    }

    #[test]
    fn test_game_error_serialization_roundtrip() {
        let errors = vec![
            GameError::NotYourTurn,
            GameError::InvalidUnit,
            GameError::NotEnoughMovement,
            GameError::InvalidTarget,
            GameError::Custom("test error".to_string()),
        ];
        for err in errors {
            let json = serde_json::to_string(&err).unwrap();
            let back: GameError = serde_json::from_str(&json).unwrap();
            assert_eq!(err, back);
        }
    }

    #[test]
    fn test_command_result_serialization_roundtrip() {
        let result = CommandResult {
            events: vec![Event::TurnStarted {
                player: PlayerId(1),
                turn: 3,
            }],
            errors: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: CommandResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.events.len(), back.events.len());
        assert!(back.errors.is_empty());
    }

    #[test]
    fn test_command_result_helpers() {
        let ok = CommandResult::ok(Event::UnitFortified {
            unit_id: GenId {
                index: 0,
                generation: 0,
            },
        });
        assert!(ok.is_ok());
        assert_eq!(ok.events.len(), 1);

        let err = CommandResult::err(GameError::NotYourTurn);
        assert!(!err.is_ok());
        assert_eq!(err.errors.len(), 1);
    }

    #[test]
    fn test_all_command_variants_serialize() {
        let commands = vec![
            Command::MoveUnit {
                unit_id: GenId {
                    index: 0,
                    generation: 0,
                },
                destination: TileCoord { x: 1, y: 1 },
            },
            Command::AttackUnit {
                attacker: GenId {
                    index: 0,
                    generation: 0,
                },
                defender: GenId {
                    index: 1,
                    generation: 0,
                },
            },
            Command::FortifyUnit {
                unit_id: GenId {
                    index: 0,
                    generation: 0,
                },
            },
            Command::SkipUnit {
                unit_id: GenId {
                    index: 0,
                    generation: 0,
                },
            },
            Command::PerformAction {
                unit_id: GenId {
                    index: 0,
                    generation: 0,
                },
                action_id: "build_city".to_string(),
            },
            Command::SetResearch {
                tech_id: "bronze_working".to_string(),
            },
            Command::EndTurn,
        ];
        for cmd in commands {
            let json = serde_json::to_string(&cmd).unwrap();
            let _back: Command = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_all_event_variants_serialize() {
        let id0 = GenId {
            index: 0,
            generation: 0,
        };
        let id1 = GenId {
            index: 1,
            generation: 0,
        };
        let events = vec![
            Event::UnitMoved {
                unit_id: id0,
                from: TileCoord { x: 0, y: 0 },
                to: TileCoord { x: 1, y: 0 },
                movement_left: 0,
            },
            Event::MoveBlocked {
                unit_id: id0,
                reason: "mountain".to_string(),
            },
            Event::UnitFortified { unit_id: id0 },
            Event::UnitSkipped { unit_id: id0 },
            Event::TurnStarted {
                player: PlayerId(0),
                turn: 1,
            },
            Event::CityFounded {
                city_id: id0,
                at: TileCoord { x: 5, y: 5 },
                name: "Rome".to_string(),
                owner: PlayerId(0),
            },
            Event::UnitConsumed {
                unit_id: id0,
                reason: "founded city".to_string(),
            },
            Event::CombatStarted {
                attacker: id0,
                defender: id1,
                tile: TileCoord { x: 3, y: 3 },
            },
            Event::CombatRound {
                round: 1,
                attacker_hp: 2,
                defender_hp: 3,
            },
            Event::CombatResolved {
                winner: id0,
                loser: id1,
                winner_hp: 1,
            },
            Event::UnitDestroyed {
                unit_id: id1,
                at: TileCoord { x: 3, y: 3 },
            },
            Event::CityGrew {
                city_id: id0,
                new_population: 2,
            },
            Event::CityStarved {
                city_id: id0,
                new_population: 1,
            },
            Event::ProductionSet {
                city_id: id0,
                item_name: "warrior".to_string(),
                cost: 10,
            },
            Event::ProductionComplete {
                city_id: id0,
                item_name: "warrior".to_string(),
            },
            Event::UnitProduced {
                city_id: id0,
                unit_id: id1,
                unit_type: "warrior".to_string(),
                at: TileCoord { x: 5, y: 5 },
            },
            Event::CityCaptured {
                city_id: id0,
                old_owner: PlayerId(1),
                new_owner: PlayerId(0),
                new_population: 1,
            },
            Event::TilesRevealed {
                player: PlayerId(0),
                tiles: vec![TileCoord { x: 1, y: 1 }, TileCoord { x: 2, y: 2 }],
            },
            Event::MoveInterrupted {
                unit_id: id0,
                at: TileCoord { x: 3, y: 0 },
                reason: MoveInterruptReason::EnemySpotted,
                remaining_path: vec![TileCoord { x: 4, y: 0 }, TileCoord { x: 5, y: 0 }],
            },
            Event::ActionCompleted {
                unit_id: id0,
                action_id: "build_city".to_string(),
                at: TileCoord { x: 5, y: 5 },
            },
            Event::ActionStarted {
                unit_id: id0,
                action_id: "build_road".to_string(),
                turns_remaining: 3,
                at: TileCoord { x: 5, y: 5 },
            },
            Event::PlayerEliminated {
                player: PlayerId(1),
            },
            Event::ResearchSet {
                player: PlayerId(0),
                tech_id: "bronze_working".to_string(),
            },
            Event::TechResearched {
                player: PlayerId(0),
                tech_id: "bronze_working".to_string(),
            },
            Event::BuildingCompleted {
                city_id: id0,
                building_id: "granary".to_string(),
            },
            Event::DestinationSet {
                unit_id: id0,
                destination: TileCoord { x: 5, y: 5 },
            },
            Event::DestinationCleared { unit_id: id0 },
            Event::GameOver {
                winner: PlayerId(0),
            },
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let _back: Event = serde_json::from_str(&json).unwrap();
        }
    }
}
