use std::collections::HashMap;
use std::fmt::Write;

use fc3_core::protocol::{CitySnapshot, Event, PlayerView, UnitSnapshot};
use fc3_core::tile::{Terrain, Vegetation, Visibility};
use fc3_core::types::PlayerId;

/// ANSI color codes for player units (normal intensity).
const PLAYER_COLORS: [&str; 6] = [
    "\x1b[31m", // Red
    "\x1b[34m", // Blue
    "\x1b[32m", // Green
    "\x1b[33m", // Yellow
    "\x1b[35m", // Magenta
    "\x1b[36m", // Cyan
];

/// ANSI bright background colors for city territory.
const PLAYER_BG_COLORS: [&str; 6] = [
    "\x1b[101m", // Bright Red background
    "\x1b[104m", // Bright Blue background
    "\x1b[102m", // Bright Green background
    "\x1b[103m", // Bright Yellow background
    "\x1b[105m", // Bright Magenta background
    "\x1b[106m", // Bright Cyan background
];

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";

/// Map terrain to a single ASCII character.
fn terrain_char(terrain: Terrain, vegetation: Vegetation) -> char {
    // Vegetation overlay takes priority
    match vegetation {
        Vegetation::Forest => return 'F',
        Vegetation::Jungle => return 'J',
        Vegetation::None => {}
    }
    match terrain {
        Terrain::Grassland => '.',
        Terrain::Plains => 'p',
        Terrain::Desert => 'd',
        Terrain::Tundra => 't',
        Terrain::Ocean => '~',
        Terrain::Coast => ',',
        Terrain::Mountain => 'M',
        Terrain::Hill => 'H',
        Terrain::Ice => '#',
    }
}

/// Render a PlayerView as an ASCII map with ANSI color codes.
///
/// Shows terrain, units with player-colored numbers, cities as `^`,
/// city territory with background colors, and legends for units and cities.
pub fn render_ascii(view: &PlayerView, map_width: u32, map_height: u32) -> String {
    let mut out = String::new();

    // Header
    writeln!(
        out,
        "Turn {} — Player {} — Gold: {}",
        view.turn, view.player.0, view.gold
    )
    .unwrap();
    writeln!(out).unwrap();

    // Build lookup tables
    let tile_map: HashMap<(u32, u32), _> = view
        .visible_tiles
        .iter()
        .map(|t| ((t.coord.x, t.coord.y), t))
        .collect();

    // Group units by position: (x,y) -> list of unit snapshots
    let mut units_by_pos: HashMap<(u32, u32), Vec<&UnitSnapshot>> = HashMap::new();
    for u in &view.known_units {
        units_by_pos
            .entry((u.position.x, u.position.y))
            .or_default()
            .push(u);
    }

    // Build city position lookup (own + known)
    let mut city_at_pos: HashMap<(u32, u32), &CitySnapshot> = HashMap::new();
    for c in &view.own_cities {
        city_at_pos.insert((c.position.x, c.position.y), c);
    }
    for c in &view.known_cities {
        city_at_pos.insert((c.position.x, c.position.y), c);
    }

    // Build tile ownership lookup from visible tiles
    let mut tile_owner: HashMap<(u32, u32), PlayerId> = HashMap::new();
    for tile in &view.visible_tiles {
        if let Some(owner) = tile.owner {
            tile_owner.insert((tile.coord.x, tile.coord.y), owner);
        }
    }

    // Render grid
    for y in 0..map_height {
        for x in 0..map_width {
            if let Some(tile) = tile_map.get(&(x, y)) {
                // Determine background color from tile ownership
                let bg = tile_owner
                    .get(&(x, y))
                    .map(|owner| PLAYER_BG_COLORS[owner.0 as usize % PLAYER_BG_COLORS.len()]);

                // Check for city on this tile first (highest priority)
                if let Some(city) = city_at_pos.get(&(x, y)) {
                    let color_idx = city.owner.0 as usize % PLAYER_COLORS.len();
                    if let Some(bg_color) = bg {
                        write!(out, "{}{}^{}", bg_color, PLAYER_COLORS[color_idx], RESET).unwrap();
                    } else {
                        write!(out, "{}^{}", PLAYER_COLORS[color_idx], RESET).unwrap();
                    }
                }
                // Then check for units
                else if let Some(units) = units_by_pos.get(&(x, y)) {
                    let first_unit = units[0];
                    let color_idx = first_unit.owner.0 as usize % PLAYER_COLORS.len();
                    let count = units.len();
                    let ch = if count <= 9 {
                        (b'0' + count as u8) as char
                    } else {
                        '+'
                    };
                    if let Some(bg_color) = bg {
                        write!(
                            out,
                            "{}{}{}{}",
                            bg_color, PLAYER_COLORS[color_idx], ch, RESET
                        )
                        .unwrap();
                    } else {
                        write!(out, "{}{}{}", PLAYER_COLORS[color_idx], ch, RESET).unwrap();
                    }
                } else if tile.visibility == Visibility::Revealed {
                    write!(out, "{}{}{}", DIM, terrain_char(tile.terrain, tile.vegetation), RESET).unwrap();
                } else {
                    // Visible terrain with optional ownership background
                    if let Some(bg_color) = bg {
                        write!(out, "{}{}{}", bg_color, terrain_char(tile.terrain, tile.vegetation), RESET).unwrap();
                    } else {
                        out.push(terrain_char(tile.terrain, tile.vegetation));
                    }
                }
            } else {
                // Unseen
                out.push(' ');
            }
        }
        writeln!(out).unwrap();
    }

    // Unit legend
    if !view.known_units.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, "Units:").unwrap();
        for u in &view.known_units {
            let color_idx = u.owner.0 as usize % PLAYER_COLORS.len();
            let fortified = if u.fortified { " [fortified]" } else { "" };
            writeln!(
                out,
                "  {}{} (P{}) at ({},{}) — {}/{} HP, {} mv{}{}",
                PLAYER_COLORS[color_idx],
                u.unit_type_name,
                u.owner.0,
                u.position.x,
                u.position.y,
                u.hp,
                u.max_hp,
                u.movement,
                fortified,
                RESET,
            )
            .unwrap();
        }
    }

    // City info section
    let all_cities: Vec<&CitySnapshot> = view
        .own_cities
        .iter()
        .chain(view.known_cities.iter())
        .collect();
    if !all_cities.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, "Cities:").unwrap();
        for c in &all_cities {
            let color_idx = c.owner.0 as usize % PLAYER_COLORS.len();
            let prod_str = match (&c.producing, c.shield_stockpile) {
                (Some(name), Some(shields)) => {
                    format!(", building {} ({} shields)", name, shields)
                }
                (Some(name), None) => format!(", building {}", name),
                _ => String::new(),
            };
            writeln!(
                out,
                "  {}{} (P{}, pop {}{}){}",
                PLAYER_COLORS[color_idx], c.name, c.owner.0, c.population, prod_str, RESET,
            )
            .unwrap();
        }
    }

    out
}

/// Format an event as a human-readable one-line description.
pub fn format_event(event: &Event) -> String {
    match event {
        Event::UnitMoved {
            unit_id: _,
            from,
            to,
            movement_left,
        } => {
            format!(
                "Unit moved from ({},{}) to ({},{}) [{} mv left]",
                from.x, from.y, to.x, to.y, movement_left
            )
        }
        Event::MoveBlocked { unit_id: _, reason } => {
            format!("Move blocked: {reason}")
        }
        Event::UnitFortified { unit_id: _ } => "Unit fortified".to_string(),
        Event::TurnStarted { player, turn } => {
            format!("=== Turn {turn} — Player {} ===", player.0)
        }
        Event::CityFounded {
            city_id: _,
            at,
            name,
            owner,
        } => {
            format!(
                "City \"{name}\" founded at ({},{}) by Player {}",
                at.x, at.y, owner.0
            )
        }
        Event::UnitConsumed { unit_id: _, reason } => {
            format!("Unit consumed: {reason}")
        }
        Event::CombatStarted {
            attacker: _,
            defender: _,
            tile,
        } => {
            format!("Combat at ({},{})", tile.x, tile.y)
        }
        Event::CombatRound {
            round,
            attacker_hp,
            defender_hp,
        } => {
            format!("  Round {round}: attacker {attacker_hp} HP, defender {defender_hp} HP")
        }
        Event::CombatResolved {
            winner: _,
            loser: _,
            winner_hp,
        } => {
            format!("  Winner with {winner_hp} HP remaining")
        }
        Event::UnitDestroyed { unit_id: _, at } => {
            format!("Unit destroyed at ({},{})", at.x, at.y)
        }
        Event::CityGrew {
            city_id: _,
            new_population,
        } => {
            format!("City grew to population {new_population}")
        }
        Event::CityStarved {
            city_id: _,
            new_population,
        } => {
            format!("City starved to population {new_population}")
        }
        Event::ProductionSet {
            city_id: _,
            item_name,
            cost,
        } => {
            format!("Production set to {item_name} (cost: {cost})")
        }
        Event::ProductionComplete {
            city_id: _,
            item_name,
        } => {
            format!("Production complete: {item_name}")
        }
        Event::UnitProduced {
            city_id: _,
            unit_id: _,
            unit_type,
            at,
        } => {
            format!("Unit produced: {unit_type} at ({},{})", at.x, at.y)
        }
        Event::CityCaptured {
            city_id: _,
            old_owner,
            new_owner,
            new_population,
        } => {
            format!(
                "City captured by Player {} from Player {} (pop {})",
                new_owner.0, old_owner.0, new_population
            )
        }
        Event::TilesRevealed { player, tiles } => {
            format!("Player {} revealed {} new tiles", player.0, tiles.len())
        }
        Event::MoveInterrupted {
            unit_id,
            at,
            reason,
            ..
        } => {
            format!(
                "Unit {}/{} interrupted at ({},{}) — {:?}",
                unit_id.index, unit_id.generation, at.x, at.y, reason
            )
        }
        Event::UnitSkipped { unit_id } => {
            format!("Unit {}/{} skipped", unit_id.index, unit_id.generation)
        }
        Event::PlayerEliminated { player } => {
            format!("*** Player {} eliminated ***", player.0)
        }
        Event::ActionCompleted {
            unit_id: _,
            action_id,
            at,
        } => {
            format!("Action '{action_id}' completed at ({},{})", at.x, at.y)
        }
        Event::ActionStarted {
            unit_id: _,
            action_id,
            turns_remaining,
            at,
        } => {
            format!(
                "Action '{action_id}' started at ({},{}) — {turns_remaining} turns remaining",
                at.x, at.y
            )
        }
        Event::ResearchSet { player, tech_id } => {
            format!("Player {} set research to {tech_id}", player.0)
        }
        Event::TechResearched { player, tech_id } => {
            format!("Player {} researched {tech_id}", player.0)
        }
        Event::BuildingCompleted {
            city_id: _,
            building_id,
        } => {
            format!("Building completed: {building_id}")
        }
        Event::DestinationSet {
            unit_id: _,
            destination,
        } => {
            format!("Destination set: ({},{})", destination.x, destination.y)
        }
        Event::DestinationCleared { unit_id: _ } => "Destination cleared".to_string(),
        Event::GameOver { winner } => {
            format!("*** GAME OVER — Player {} wins! ***", winner.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fc3_core::id::GenId;
    use fc3_core::protocol::TileSnapshot;
    use fc3_core::types::{Direction, TileCoord};

    fn make_view() -> PlayerView {
        // 5x3 map with a mix of terrain
        let mut tiles = Vec::new();
        let terrains = [
            Terrain::Grassland,
            Terrain::Plains,
            Terrain::Grassland,
            Terrain::Hill,
            Terrain::Ocean,
        ];
        let vegetations = [
            Vegetation::None,
            Vegetation::None,
            Vegetation::Forest,
            Vegetation::None,
            Vegetation::None,
        ];
        for y in 0..3u32 {
            for x in 0..5u32 {
                let idx = (x + y * 5) as usize % terrains.len();
                tiles.push(TileSnapshot {
                    coord: TileCoord { x, y },
                    terrain: terrains[idx],
                    vegetation: vegetations[idx],
                    improvement: None,
                    road_level: 0,
                    owner: None,
                    visibility: if y == 2 && x >= 3 {
                        Visibility::Revealed
                    } else {
                        Visibility::Visible
                    },
                });
            }
        }

        let units = vec![
            UnitSnapshot {
                id: GenId {
                    index: 0,
                    generation: 0,
                },
                unit_type_name: "Warrior".to_string(),
                category: "melee".to_string(),
                owner: PlayerId(0),
                position: TileCoord { x: 1, y: 1 },
                hp: 3,
                max_hp: 3,
                attack: 1,
                defense: 1,
                movement: 1,
                max_movement: 1,
                fortified: false,
                skipped: false,
                current_action: None,
                current_action_animation: None,
                destination: None,
                direction: Direction::default(),
                art_ini: None,
            },
            UnitSnapshot {
                id: GenId {
                    index: 1,
                    generation: 0,
                },
                unit_type_name: "Warrior".to_string(),
                category: "melee".to_string(),
                owner: PlayerId(1),
                position: TileCoord { x: 3, y: 0 },
                hp: 2,
                max_hp: 3,
                attack: 1,
                defense: 1,
                movement: 0,
                max_movement: 1,
                fortified: true,
                skipped: false,
                current_action: None,
                current_action_animation: None,
                destination: None,
                direction: Direction::default(),
                art_ini: None,
            },
        ];

        PlayerView {
            player: PlayerId(0),
            turn: 5,
            gold: 0,
            science: 0,
            culture: 0,
            researching: None,
            researching_name: None,
            researched_techs: Vec::new(),
            science_per_turn: 0,
            gold_per_turn: 0,
            research_turns_left: None,
            civilization: None,
            civ_adjective: None,
            civ_name: None,
            civ_noun: None,
            map_width: 5,
            map_height: 5,
            wrap_x: false,
            wrap_y: false,
            visible_tiles: tiles,
            fringe_tiles: Vec::new(),
            known_units: units,
            own_cities: Vec::new(),
            known_cities: Vec::new(),
        }
    }

    #[test]
    fn test_render_ascii_contains_header() {
        let view = make_view();
        let rendered = render_ascii(&view, 5, 3);
        assert!(rendered.contains("Turn 5 — Player 0 — Gold: 0"));
    }

    #[test]
    fn test_render_ascii_contains_terrain() {
        let view = make_view();
        let rendered = render_ascii(&view, 5, 3);
        assert!(rendered.contains('~') || rendered.contains('.') || rendered.contains('F'));
    }

    #[test]
    fn test_render_ascii_contains_unit_legend() {
        let view = make_view();
        let rendered = render_ascii(&view, 5, 3);
        assert!(rendered.contains("Units:"));
        assert!(rendered.contains("Warrior"));
        assert!(rendered.contains("3/3 HP"));
        assert!(rendered.contains("2/3 HP"));
        assert!(rendered.contains("[fortified]"));
    }

    #[test]
    fn test_render_ascii_correct_dimensions() {
        let view = make_view();
        let rendered = render_ascii(&view, 5, 3);
        let lines: Vec<&str> = rendered.lines().collect();
        assert!(lines.len() >= 5);
    }

    #[test]
    fn test_render_ascii_shows_cities() {
        let mut view = make_view();
        view.own_cities.push(CitySnapshot {
            id: GenId {
                index: 0,
                generation: 0,
            },
            name: "Rome".to_string(),
            owner: PlayerId(0),
            position: TileCoord { x: 2, y: 1 },
            population: 3,
            food_stockpile: Some(5),
            food_per_turn: Some(2),
            shield_stockpile: Some(7),
            shields_per_turn: Some(3),
            commerce_per_turn: Some(1),
            producing: Some("Warrior".to_string()),
            worked_tiles: None,
            production_cost: None,
            food_growth_threshold: None,
            buildings: None,
        });
        let rendered = render_ascii(&view, 5, 3);
        assert!(rendered.contains('^'), "Should show city as ^");
        assert!(rendered.contains("Cities:"), "Should have city legend");
        assert!(rendered.contains("Rome"), "Should show city name");
        assert!(rendered.contains("pop 3"), "Should show population");
    }

    #[test]
    fn test_render_ascii_city_territory_has_background() {
        let mut view = make_view();
        // Mark a tile as owned
        view.visible_tiles[6].owner = Some(PlayerId(0)); // tile at (1,1)
        view.own_cities.push(CitySnapshot {
            id: GenId {
                index: 0,
                generation: 0,
            },
            name: "Rome".to_string(),
            owner: PlayerId(0),
            position: TileCoord { x: 2, y: 1 },
            population: 1,
            food_stockpile: Some(0),
            food_per_turn: Some(0),
            shield_stockpile: Some(0),
            shields_per_turn: Some(0),
            commerce_per_turn: Some(0),
            producing: None,
            worked_tiles: None,
            production_cost: None,
            food_growth_threshold: None,
            buildings: None,
        });
        let rendered = render_ascii(&view, 5, 3);
        // Should contain ANSI background escape codes for owned tiles
        assert!(
            rendered.contains("\x1b[101m") || rendered.contains("\x1b[104m"),
            "Owned tiles should have background color"
        );
    }

    #[test]
    fn test_format_event_unit_moved() {
        let event = Event::UnitMoved {
            unit_id: GenId {
                index: 0,
                generation: 0,
            },
            from: TileCoord { x: 3, y: 4 },
            to: TileCoord { x: 3, y: 5 },
            movement_left: 0,
        };
        let s = format_event(&event);
        assert!(s.contains("(3,4)"));
        assert!(s.contains("(3,5)"));
        assert!(s.contains("0 mv left"));
    }

    #[test]
    fn test_format_event_combat_started() {
        let event = Event::CombatStarted {
            attacker: GenId {
                index: 0,
                generation: 0,
            },
            defender: GenId {
                index: 1,
                generation: 0,
            },
            tile: TileCoord { x: 5, y: 6 },
        };
        let s = format_event(&event);
        assert!(s.contains("Combat"));
        assert!(s.contains("(5,6)"));
    }

    #[test]
    fn test_format_event_combat_round() {
        let event = Event::CombatRound {
            round: 3,
            attacker_hp: 2,
            defender_hp: 1,
        };
        let s = format_event(&event);
        assert!(s.contains("Round 3"));
        assert!(s.contains("attacker 2 HP"));
        assert!(s.contains("defender 1 HP"));
    }

    #[test]
    fn test_format_event_combat_resolved() {
        let event = Event::CombatResolved {
            winner: GenId {
                index: 0,
                generation: 0,
            },
            loser: GenId {
                index: 1,
                generation: 0,
            },
            winner_hp: 2,
        };
        let s = format_event(&event);
        assert!(s.contains("Winner"));
        assert!(s.contains("2 HP"));
    }

    #[test]
    fn test_format_event_unit_destroyed() {
        let event = Event::UnitDestroyed {
            unit_id: GenId {
                index: 1,
                generation: 0,
            },
            at: TileCoord { x: 5, y: 6 },
        };
        let s = format_event(&event);
        assert!(s.contains("destroyed"));
        assert!(s.contains("(5,6)"));
    }

    #[test]
    fn test_format_event_turn_started() {
        let event = Event::TurnStarted {
            player: PlayerId(0),
            turn: 5,
        };
        let s = format_event(&event);
        assert!(s.contains("Turn 5"));
        assert!(s.contains("Player 0"));
    }

    #[test]
    fn test_format_event_player_eliminated() {
        let event = Event::PlayerEliminated {
            player: PlayerId(1),
        };
        let s = format_event(&event);
        assert!(s.contains("Player 1"));
        assert!(s.contains("eliminated"));
    }

    #[test]
    fn test_format_event_game_over() {
        let event = Event::GameOver {
            winner: PlayerId(0),
        };
        let s = format_event(&event);
        assert!(s.contains("GAME OVER"));
        assert!(s.contains("Player 0 wins"));
    }

    #[test]
    fn test_format_event_move_blocked() {
        let event = Event::MoveBlocked {
            unit_id: GenId {
                index: 0,
                generation: 0,
            },
            reason: "not enough movement".to_string(),
        };
        let s = format_event(&event);
        assert!(s.contains("blocked"));
        assert!(s.contains("not enough movement"));
    }

    #[test]
    fn test_format_event_unit_fortified() {
        let event = Event::UnitFortified {
            unit_id: GenId {
                index: 0,
                generation: 0,
            },
        };
        let s = format_event(&event);
        assert!(s.contains("fortified"));
    }

    #[test]
    fn test_format_event_city_founded() {
        let event = Event::CityFounded {
            city_id: GenId {
                index: 0,
                generation: 0,
            },
            at: TileCoord { x: 5, y: 5 },
            name: "Rome".to_string(),
            owner: PlayerId(0),
        };
        let s = format_event(&event);
        assert!(s.contains("Rome"));
        assert!(s.contains("(5,5)"));
    }

    #[test]
    fn test_format_all_event_variants_covered() {
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
                reason: "test".to_string(),
            },
            Event::UnitFortified { unit_id: id0 },
            Event::TurnStarted {
                player: PlayerId(0),
                turn: 1,
            },
            Event::CityFounded {
                city_id: id0,
                at: TileCoord { x: 0, y: 0 },
                name: "Test".to_string(),
                owner: PlayerId(0),
            },
            Event::UnitConsumed {
                unit_id: id0,
                reason: "founded city".to_string(),
            },
            Event::CombatStarted {
                attacker: id0,
                defender: id1,
                tile: TileCoord { x: 0, y: 0 },
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
                at: TileCoord { x: 0, y: 0 },
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
                tiles: vec![TileCoord { x: 1, y: 1 }],
            },
            Event::MoveInterrupted {
                unit_id: id0,
                at: TileCoord { x: 3, y: 0 },
                reason: fc3_core::protocol::MoveInterruptReason::EnemySpotted,
                remaining_path: vec![TileCoord { x: 4, y: 0 }],
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
            Event::GameOver {
                winner: PlayerId(0),
            },
        ];
        for event in &events {
            let s = format_event(event);
            assert!(
                !s.is_empty(),
                "format_event should produce non-empty string for {:?}",
                event
            );
        }
    }
}
