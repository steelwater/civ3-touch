use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::id::UnitId;
use crate::protocol::{AvailableCommand, Command, PlayerView, UnitSnapshot};
use crate::types::TileCoord;

/// Trait for AI agents that can decide on commands.
pub trait Agent {
    fn decide(&mut self, view: &PlayerView, available: &[AvailableCommand]) -> Command;
}

/// A simple AI agent that explores, fights, founds cities, and produces units.
pub struct SimpleAgent {
    rng: ChaCha8Rng,
}

impl SimpleAgent {
    pub fn new(rng: ChaCha8Rng) -> Self {
        Self { rng }
    }

    /// Find the unit snapshot for a given ID.
    fn find_unit<'a>(&self, view: &'a PlayerView, uid: UnitId) -> Option<&'a UnitSnapshot> {
        view.known_units.iter().find(|u| u.id == uid)
    }

    /// Chebyshev distance between two positions, optionally accounting for map wrapping.
    fn distance(a: TileCoord, b: TileCoord, wrap_width: Option<u32>) -> u32 {
        let mut dx = (a.x as i64 - b.x as i64).unsigned_abs() as u32;
        if let Some(w) = wrap_width {
            dx = dx.min(w - dx);
        }
        let dy = (a.y as i64 - b.y as i64).unsigned_abs() as u32;
        dx.max(dy)
    }

    /// Get the wrap_width parameter from a view (Some(width) if wrapping, None otherwise).
    fn wrap_width(view: &PlayerView) -> Option<u32> {
        if view.wrap_x {
            Some(view.map_width)
        } else {
            None
        }
    }

    /// Pick the best adjacent destination toward a target (for retreat/fallback).
    fn move_toward_adjacent(
        &self,
        destinations: &[TileCoord],
        target: TileCoord,
        wrap: Option<u32>,
    ) -> Option<TileCoord> {
        destinations
            .iter()
            .min_by_key(|d| Self::distance(**d, target, wrap))
            .copied()
    }

    /// Pick the best move destination away from a threat.
    fn move_away(
        &self,
        destinations: &[TileCoord],
        threat: TileCoord,
        wrap: Option<u32>,
    ) -> Option<TileCoord> {
        destinations
            .iter()
            .max_by_key(|d| Self::distance(**d, threat, wrap))
            .copied()
    }

    /// Check if the player already has a settler unit.
    fn has_settler_unit(view: &PlayerView) -> bool {
        view.known_units
            .iter()
            .any(|u| u.owner == view.player && u.unit_type_name == "settler")
    }

    /// Check if the player already has a worker unit.
    fn has_worker_unit(view: &PlayerView) -> bool {
        view.known_units
            .iter()
            .any(|u| u.owner == view.player && u.unit_type_name == "worker")
    }

    /// Find a distant exploration goal — the nearest tile that is revealed but not
    /// visible (or any tile the AI hasn't seen near visible boundaries).
    fn pick_exploration_goal(&mut self, view: &PlayerView, unit_pos: TileCoord) -> TileCoord {
        let wrap = Self::wrap_width(view);

        // Find revealed-but-not-visible tiles on passable terrain (fog boundary)
        let revealed_tiles: Vec<TileCoord> = view
            .visible_tiles
            .iter()
            .filter(|t| t.visibility == crate::tile::Visibility::Revealed)
            .filter(|t| {
                !matches!(
                    t.terrain,
                    crate::tile::Terrain::Ocean
                        | crate::tile::Terrain::Mountain
                        | crate::tile::Terrain::Coast
                )
            })
            .map(|t| t.coord)
            .collect();

        if !revealed_tiles.is_empty() {
            // Pick the nearest revealed passable tile as our exploration target
            if let Some(nearest) = revealed_tiles
                .iter()
                .min_by_key(|t| Self::distance(unit_pos, **t, wrap))
            {
                return *nearest;
            }
        }

        // No passable revealed tiles — pick a random passable visible tile
        let passable_tiles: Vec<TileCoord> = view
            .visible_tiles
            .iter()
            .filter(|t| {
                !matches!(
                    t.terrain,
                    crate::tile::Terrain::Ocean
                        | crate::tile::Terrain::Mountain
                        | crate::tile::Terrain::Coast
                )
            })
            .map(|t| t.coord)
            .collect();

        if !passable_tiles.is_empty() {
            let idx = self.rng.gen_range(0..passable_tiles.len());
            return passable_tiles[idx];
        }

        // Absolute fallback: random tile on the map
        let w = view.map_width;
        let h = view.map_height;
        TileCoord {
            x: self.rng.gen_range(0..w),
            y: self.rng.gen_range(0..h),
        }
    }

    /// Find the best city founding location for a settler.
    fn pick_city_site(&self, view: &PlayerView, settler_pos: TileCoord) -> Option<TileCoord> {
        let wrap = Self::wrap_width(view);

        // Score visible tiles: sum of food yields in approximate city radius
        let mut best_tile = None;
        let mut best_score = i32::MIN;

        for tile_snap in &view.visible_tiles {
            let coord = tile_snap.coord;

            // Must be passable land
            if matches!(
                tile_snap.terrain,
                crate::tile::Terrain::Ocean
                    | crate::tile::Terrain::Mountain
                    | crate::tile::Terrain::Coast
            ) {
                continue;
            }

            // Must be > 4 tiles from any existing city
            let too_close = view
                .own_cities
                .iter()
                .any(|c| Self::distance(coord, c.position, wrap) <= 4);
            if too_close {
                continue;
            }

            // Score: food potential of adjacent tiles (simple heuristic)
            let score = Self::estimate_food_score(view, coord, wrap);

            // Tiebreak by distance from settler (prefer closer)
            let dist = Self::distance(settler_pos, coord, wrap) as i32;
            let adjusted = score * 100 - dist;

            if adjusted > best_score {
                best_score = adjusted;
                best_tile = Some(coord);
            }
        }

        best_tile
    }

    /// Rough food score for a potential city site.
    fn estimate_food_score(view: &PlayerView, center: TileCoord, wrap: Option<u32>) -> i32 {
        let mut score = 0;
        for tile in &view.visible_tiles {
            if Self::distance(center, tile.coord, wrap) <= 2 {
                score += match tile.terrain {
                    crate::tile::Terrain::Grassland => 2,
                    crate::tile::Terrain::Plains => 1,
                    crate::tile::Terrain::Coast => 1,
                    _ => 0,
                };
                // Forest vegetation provides 1 food (like forest terrain used to)
                if tile.vegetation == crate::tile::Vegetation::Forest {
                    // Forest on grassland: terrain gives 2, but forest overrides to 1 food
                    // So reduce by 1 compared to base terrain
                    score -= 1;
                }
            }
        }
        score
    }

    /// Check if there's a friendly city within a given distance of a position.
    fn has_nearby_city(&self, view: &PlayerView, pos: TileCoord, max_dist: u32) -> bool {
        let wrap = Self::wrap_width(view);
        view.own_cities
            .iter()
            .any(|c| Self::distance(pos, c.position, wrap) <= max_dist)
    }

    /// Count the player's military (non-civilian) units visible in the view.
    fn count_military_units(&self, view: &PlayerView) -> usize {
        view.known_units
            .iter()
            .filter(|u| {
                u.owner == view.player
                    && u.unit_type_name != "settler"
                    && u.unit_type_name != "worker"
            })
            .count()
    }
}

impl Agent for SimpleAgent {
    fn decide(&mut self, view: &PlayerView, available: &[AvailableCommand]) -> Command {
        let my_player = view.player;
        let wrap = Self::wrap_width(view);

        // 0. Set research if needed: pick cheapest available tech
        for cmd in available {
            if let AvailableCommand::SetResearch { options } = cmd {
                if !options.is_empty() {
                    let cheapest = options.iter().min_by_key(|o| o.cost).unwrap();
                    return Command::SetResearch {
                        tech_id: cheapest.id.clone(),
                    };
                }
            }
        }

        // 1. Found cities: if we have a settler and no city within 4 tiles, found one
        for cmd in available {
            if let AvailableCommand::UnitAction {
                unit_id, action_id, ..
            } = cmd
            {
                if action_id == "build_city" {
                    if let Some(unit) = self.find_unit(view, *unit_id) {
                        if !self.has_nearby_city(view, unit.position, 4) {
                            return Command::PerformAction {
                                unit_id: *unit_id,
                                action_id: "build_city".to_string(),
                            };
                        }
                    }
                }
            }
        }

        // 2. Worker actions: build improvements when available
        for cmd in available {
            if let AvailableCommand::UnitAction {
                unit_id, action_id, ..
            } = cmd
            {
                if action_id == "build_mine"
                    || action_id == "build_irrigation"
                    || action_id == "build_road"
                {
                    return Command::PerformAction {
                        unit_id: *unit_id,
                        action_id: action_id.clone(),
                    };
                }
            }
        }

        // 3. Set production for idle cities
        let has_settler = Self::has_settler_unit(view);
        let has_worker = Self::has_worker_unit(view);
        for cmd in available {
            if let AvailableCommand::SetProduction { city_id, options } = cmd {
                // Check if this city already has production set
                let city = view.own_cities.iter().find(|c| c.id == *city_id);
                let has_production = city.is_some_and(|c| c.producing.is_some());
                if !has_production && !options.is_empty() {
                    let military_count = self.count_military_units(view);
                    let city_count = view.own_cities.len();

                    // Heuristic: warriors if < 3 military, worker if don't have one,
                    // settlers if < 3 cities (but only if we don't already have a settler),
                    // else warriors
                    let target_name = if military_count < 3 {
                        "warrior"
                    } else if !has_worker {
                        "worker"
                    } else if city_count < 3 && !has_settler {
                        "settler"
                    } else {
                        "warrior"
                    };

                    // Find the matching option, fall back to cheapest building,
                    // then first non-wealth option
                    let chosen = options
                        .iter()
                        .find(|o| o.name == target_name)
                        .or_else(|| {
                            // Pick cheapest available building if no preferred unit
                            options
                                .iter()
                                .filter(|o| {
                                    matches!(o.item, crate::city::ProductionItem::Building { .. })
                                })
                                .min_by_key(|o| o.cost)
                        })
                        .or_else(|| options.iter().find(|o| o.name != "wealth"));

                    if let Some(option) = chosen {
                        return Command::SetProduction {
                            city_id: *city_id,
                            item: option.item.clone(),
                        };
                    }
                }
            }
        }

        // 3. Collect all enemy units visible to us
        let enemies: Vec<&UnitSnapshot> = view
            .known_units
            .iter()
            .filter(|u| u.owner != my_player)
            .collect();

        // 4. Attack: prioritize attacking (with self-preservation for wounded)
        for cmd in available {
            match cmd {
                AvailableCommand::Attack { unit_id, targets } => {
                    let unit = match self.find_unit(view, *unit_id) {
                        Some(u) => u,
                        None => continue,
                    };

                    // Don't attack with civilians
                    if unit.unit_type_name == "settler" || unit.unit_type_name == "worker" {
                        continue;
                    }

                    // Self-preservation: if HP <= 1 and there's a retreat path, flee instead
                    if unit.hp <= 1 {
                        if let Some(AvailableCommand::Move {
                            unit_id: _,
                            valid_destinations,
                        }) = available.iter().find(|c| {
                            matches!(c, AvailableCommand::Move { unit_id: m, .. } if *m == *unit_id)
                        }) {
                            if let Some(target_snap) = targets
                                .iter()
                                .filter_map(|tid| self.find_unit(view, *tid))
                                .min_by_key(|e| {
                                    Self::distance(unit.position, e.position, wrap)
                                })
                            {
                                if let Some(retreat) = self.move_away(
                                    valid_destinations,
                                    target_snap.position,
                                    wrap,
                                ) {
                                    return Command::MoveUnit {
                                        unit_id: *unit_id,
                                        destination: retreat,
                                    };
                                }
                            }
                        }
                    }

                    // Attack the weakest enemy (lowest HP)
                    let weakest = targets
                        .iter()
                        .filter_map(|tid| self.find_unit(view, *tid).map(|s| (*tid, s)))
                        .min_by_key(|(_, snap)| snap.hp);

                    if let Some((target_id, _)) = weakest {
                        return Command::AttackUnit {
                            attacker: *unit_id,
                            defender: target_id,
                        };
                    }
                }
                _ => continue,
            }
        }

        // 5. Move: goal-based movement with pathfinding
        for cmd in available {
            match cmd {
                AvailableCommand::Move {
                    unit_id,
                    valid_destinations,
                } => {
                    let unit = match self.find_unit(view, *unit_id) {
                        Some(u) => u,
                        None => continue,
                    };

                    // Workers: explore (no special goal-based movement)
                    if unit.unit_type_name == "worker" {
                        let goal = self.pick_exploration_goal(view, unit.position);
                        if valid_destinations.contains(&goal) {
                            return Command::MoveUnit {
                                unit_id: *unit_id,
                                destination: goal,
                            };
                        }
                        if let Some(dest) =
                            self.move_toward_adjacent(valid_destinations, goal, wrap)
                        {
                            return Command::MoveUnit {
                                unit_id: *unit_id,
                                destination: dest,
                            };
                        }
                        continue;
                    }

                    // Settlers: move toward the best city site
                    if unit.unit_type_name == "settler" {
                        if let Some(site) = self.pick_city_site(view, unit.position) {
                            if valid_destinations.contains(&site) {
                                return Command::MoveUnit {
                                    unit_id: *unit_id,
                                    destination: site,
                                };
                            }
                            if let Some(dest) =
                                self.move_toward_adjacent(valid_destinations, site, wrap)
                            {
                                return Command::MoveUnit {
                                    unit_id: *unit_id,
                                    destination: dest,
                                };
                            }
                        }
                        // No good site found — pick any adjacent destination to keep moving
                        if !valid_destinations.is_empty() {
                            let goal = self.pick_exploration_goal(view, unit.position);
                            if let Some(dest) =
                                self.move_toward_adjacent(valid_destinations, goal, wrap)
                            {
                                return Command::MoveUnit {
                                    unit_id: *unit_id,
                                    destination: dest,
                                };
                            }
                        }
                        continue;
                    }

                    // Self-preservation: low HP and no enemies adjacent -> fortify
                    if unit.hp <= 1
                        && !unit.fortified
                        && available.iter().any(|c| {
                            matches!(c, AvailableCommand::Fortify { unit_id: fid } if *fid == *unit_id)
                        })
                    {
                        return Command::FortifyUnit { unit_id: *unit_id };
                    }

                    // Chase visible enemies: move toward nearest enemy via adjacent step
                    if let Some(nearest_enemy) = enemies
                        .iter()
                        .filter(|e| Self::distance(unit.position, e.position, wrap) <= 10)
                        .min_by_key(|e| Self::distance(unit.position, e.position, wrap))
                    {
                        if valid_destinations.contains(&nearest_enemy.position) {
                            return Command::MoveUnit {
                                unit_id: *unit_id,
                                destination: nearest_enemy.position,
                            };
                        }
                        if let Some(dest) = self.move_toward_adjacent(
                            valid_destinations,
                            nearest_enemy.position,
                            wrap,
                        ) {
                            return Command::MoveUnit {
                                unit_id: *unit_id,
                                destination: dest,
                            };
                        }
                    }

                    // Explore: move toward distant unseen/revealed tiles
                    let goal = self.pick_exploration_goal(view, unit.position);
                    if valid_destinations.contains(&goal) {
                        return Command::MoveUnit {
                            unit_id: *unit_id,
                            destination: goal,
                        };
                    }
                    if let Some(dest) = self.move_toward_adjacent(valid_destinations, goal, wrap) {
                        return Command::MoveUnit {
                            unit_id: *unit_id,
                            destination: dest,
                        };
                    }
                }
                _ => continue,
            }
        }

        // Fallback: set production for any idle city (may have been missed above)
        for cmd in available {
            if let AvailableCommand::SetProduction { city_id, options } = cmd {
                let city = view.own_cities.iter().find(|c| c.id == *city_id);
                if city.is_some_and(|c| c.producing.is_none()) {
                    if let Some(option) = options.iter().find(|o| o.name != "wealth") {
                        return Command::SetProduction {
                            city_id: *city_id,
                            item: option.item.clone(),
                        };
                    }
                }
            }
        }

        // Skip any remaining units that haven't been handled yet
        for cmd in available {
            if let AvailableCommand::Skip { unit_id } = cmd {
                return Command::SkipUnit { unit_id: *unit_id };
            }
        }

        // Nothing else to do
        Command::EndTurn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::ProductionItem;
    use crate::engine::{Engine, GameConfig};
    use crate::types::PlayerId;
    use crate::world::WorldConfig;
    use rand::SeedableRng;

    fn test_config() -> GameConfig {
        GameConfig {
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
        }
    }

    fn spawn_warrior(engine: &Engine, owner: PlayerId, pos: TileCoord) -> crate::id::GenId {
        let mut world = engine.world().borrow_mut();
        let wid = world.unit_types.get_by_name("warrior").unwrap().id;
        let template = world.unit_types.get(wid).unwrap().clone();
        world.units.spawn(wid, owner, pos, &template)
    }

    fn spawn_settler(engine: &Engine, owner: PlayerId, pos: TileCoord) -> crate::id::GenId {
        let mut world = engine.world().borrow_mut();
        let sid = world.unit_types.get_by_name("settler").unwrap().id;
        let template = world.unit_types.get(sid).unwrap().clone();
        world.units.spawn(sid, owner, pos, &template)
    }

    #[test]
    fn test_ai_produces_valid_commands() {
        let mut engine = Engine::new_game(&test_config()).unwrap();
        spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        engine.update_visibility(PlayerId(0));

        let mut agent = SimpleAgent::new(ChaCha8Rng::seed_from_u64(42));

        let mut iterations = 0;
        loop {
            let view = engine.player_view(PlayerId(0));
            let available = engine.available_commands(PlayerId(0));
            let cmd = agent.decide(&view, &available);

            // Verify the command is valid
            let is_valid = match &cmd {
                Command::EndTurn => true,
                // MoveUnit: engine supports pathfinding to any reachable tile,
                // so the AI can issue moves to non-adjacent destinations.
                // We just verify the unit has a Move available command.
                Command::MoveUnit { unit_id, .. } => available
                    .iter()
                    .any(|c| matches!(c, AvailableCommand::Move { unit_id: uid, .. } if *uid == *unit_id)),
                Command::AttackUnit { attacker, defender } => available.iter().any(|c| match c {
                    AvailableCommand::Attack {
                        unit_id: uid,
                        targets,
                    } => *uid == *attacker && targets.contains(defender),
                    _ => false,
                }),
                Command::FortifyUnit { unit_id } => available.iter().any(
                    |c| matches!(c, AvailableCommand::Fortify { unit_id: uid } if *uid == *unit_id),
                ),
                Command::PerformAction { unit_id, action_id } => available.iter().any(
                    |c| matches!(c, AvailableCommand::UnitAction { unit_id: uid, action_id: aid, .. } if *uid == *unit_id && aid == action_id),
                ),
                Command::SetProduction { city_id, .. } => available.iter().any(
                    |c| matches!(c, AvailableCommand::SetProduction { city_id: cid, .. } if *cid == *city_id),
                ),
                Command::SkipUnit { unit_id } => available.iter().any(
                    |c| matches!(c, AvailableCommand::Skip { unit_id: uid } if *uid == *unit_id),
                ),
                Command::SetResearch { .. } => available
                    .iter()
                    .any(|c| matches!(c, AvailableCommand::SetResearch { .. })),
            };
            assert!(
                is_valid,
                "AI should only produce valid commands, got: {:?}",
                cmd
            );

            engine.submit_command(PlayerId(0), cmd.clone());
            if matches!(cmd, Command::EndTurn) {
                break;
            }
            iterations += 1;
            assert!(iterations < 1000, "AI should not loop forever");
        }
    }

    #[test]
    fn test_ai_explores() {
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
        let mut engine = Engine::new_game(&config).unwrap();
        let uid = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        spawn_warrior(&engine, PlayerId(1), TileCoord { x: 9, y: 9 });
        engine.update_visibility(PlayerId(0));
        engine.update_visibility(PlayerId(1));

        let initial_pos = {
            engine.world().borrow().units.position[engine.world().borrow().units.get(uid).unwrap()]
        };

        let mut agent0 = SimpleAgent::new(ChaCha8Rng::seed_from_u64(100));
        let mut agent1 = SimpleAgent::new(ChaCha8Rng::seed_from_u64(200));

        // Run 20 turns
        for _ in 0..20 {
            for player_idx in 0..2u8 {
                let player = PlayerId(player_idx);
                if engine.current_player() != player {
                    continue;
                }
                let agent: &mut dyn Agent = if player_idx == 0 {
                    &mut agent0
                } else {
                    &mut agent1
                };
                let mut iterations = 0;
                loop {
                    let view = engine.player_view(player);
                    let available = engine.available_commands(player);
                    let cmd = agent.decide(&view, &available);
                    engine.submit_command(player, cmd.clone());
                    if matches!(cmd, Command::EndTurn) {
                        break;
                    }
                    iterations += 1;
                    if iterations > 100 {
                        engine.submit_command(player, Command::EndTurn);
                        break;
                    }
                }
            }
        }

        // Verify the unit moved
        let world = engine.world().borrow();
        if let Some(idx) = world.units.get(uid) {
            let final_pos = world.units.position[idx];
            assert_ne!(
                final_pos, initial_pos,
                "AI unit should have moved from starting position"
            );
        }
    }

    #[test]
    fn test_ai_attacks_adjacent_enemy() {
        let engine = Engine::new_game(&test_config()).unwrap();
        let _attacker = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        let _defender = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 5, y: 6 });
        engine.update_visibility(PlayerId(0));

        let mut agent = SimpleAgent::new(ChaCha8Rng::seed_from_u64(42));
        let view = engine.player_view(PlayerId(0));
        let available = engine.available_commands(PlayerId(0));
        let cmd = agent.decide(&view, &available);

        assert!(
            matches!(cmd, Command::AttackUnit { .. }),
            "AI should attack adjacent enemy, got: {:?}",
            cmd
        );
    }

    #[test]
    fn test_ai_founds_city_with_settler() {
        let engine = Engine::new_game(&test_config()).unwrap();
        let _settler = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        engine.update_visibility(PlayerId(0));

        let mut agent = SimpleAgent::new(ChaCha8Rng::seed_from_u64(42));
        let view = engine.player_view(PlayerId(0));
        let available = engine.available_commands(PlayerId(0));
        let cmd = agent.decide(&view, &available);

        assert!(
            matches!(cmd, Command::PerformAction { ref action_id, .. } if action_id == "build_city"),
            "AI should found a city with settler, got: {:?}",
            cmd
        );
    }

    #[test]
    fn test_ai_settler_moves_if_city_nearby() {
        let mut engine = Engine::new_game(&test_config()).unwrap();
        // Found a city first
        let settler1 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        engine.update_visibility(PlayerId(0));
        engine.submit_command(
            PlayerId(0),
            Command::PerformAction {
                unit_id: settler1,
                action_id: "build_city".to_string(),
            },
        );

        // Spawn another settler near the city
        let _settler2 = spawn_settler(&engine, PlayerId(0), TileCoord { x: 6, y: 5 });
        engine.update_visibility(PlayerId(0));

        let mut agent = SimpleAgent::new(ChaCha8Rng::seed_from_u64(42));
        let view = engine.player_view(PlayerId(0));
        let available = engine.available_commands(PlayerId(0));
        let cmd = agent.decide(&view, &available);

        // Should set research, move the settler, or set production (but not found a city — too close)
        assert!(
            matches!(
                cmd,
                Command::MoveUnit { .. } | Command::SetProduction { .. } | Command::SetResearch { .. }
            ),
            "AI should set research, move settler, or set production when city is nearby, got: {:?}",
            cmd
        );
    }

    #[test]
    fn test_ai_sets_production() {
        let mut engine = Engine::new_game(&test_config()).unwrap();
        // Found a city — production is auto-set by the engine
        let settler = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        engine.update_visibility(PlayerId(0));
        engine.submit_command(
            PlayerId(0),
            Command::PerformAction {
                unit_id: settler,
                action_id: "build_city".to_string(),
            },
        );

        // Verify city has auto-set production
        {
            let world = engine.world().borrow();
            let city = world
                .cities
                .iter_alive()
                .find(|(_, idx)| world.cities.owner[*idx] == PlayerId(0));
            assert!(city.is_some(), "City should exist after founding");
            let (_, idx) = city.unwrap();
            assert!(
                world.cities.producing[idx].is_some(),
                "City should have auto-set production after founding"
            );
        }

        let mut agent = SimpleAgent::new(ChaCha8Rng::seed_from_u64(42));

        // AI will set research (its step 0 priority, before other actions)
        let view = engine.player_view(PlayerId(0));
        let available = engine.available_commands(PlayerId(0));
        let cmd = agent.decide(&view, &available);
        assert!(
            matches!(cmd, Command::SetResearch { .. }),
            "AI should set research first, got: {:?}",
            cmd
        );
        engine.submit_command(PlayerId(0), cmd);

        // With auto-production set and research set, AI should be able to EndTurn
        let view = engine.player_view(PlayerId(0));
        let available = engine.available_commands(PlayerId(0));
        let cmd = agent.decide(&view, &available);
        assert!(
            matches!(cmd, Command::EndTurn),
            "AI should end turn when production is auto-set, got: {:?}",
            cmd
        );
    }

    #[test]
    fn test_ai_full_game_with_cities() {
        let config = GameConfig {
            world: WorldConfig {
                width: 15,
                height: 10,
                wrap_x: true,
                wrap_y: false,
                num_players: 2,
                seed: 777,
            },
            mod_paths: vec!["base".to_string()],
            units_per_player: vec!["warrior".to_string(), "settler".to_string()],
            max_turns: Some(100),
        };
        let mut engine = Engine::new_game(&config).unwrap();
        let num_players = config.world.num_players;

        let mut agents: Vec<Box<dyn Agent>> = (0..num_players)
            .map(|i| {
                Box::new(SimpleAgent::new(ChaCha8Rng::seed_from_u64(100 + i as u64)))
                    as Box<dyn Agent>
            })
            .collect();

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
            let mut commands_this_turn = 0u32;

            loop {
                let view = engine.player_view(current);
                let available = engine.available_commands(current);
                let cmd = agents[agent_idx].decide(&view, &available);
                let is_end = matches!(cmd, Command::EndTurn);
                engine.submit_command(current, cmd);
                if is_end {
                    break;
                }
                commands_this_turn += 1;
                if commands_this_turn > 500 {
                    engine.submit_command(current, Command::EndTurn);
                    break;
                }
            }
        }

        // Verify at least one player founded a city and the game progressed
        let world = engine.world().borrow();
        let total_cities = world.cities.iter_alive().count();
        let total_units = world.units.iter_alive().count();

        assert!(
            total_cities >= 1,
            "At least one city should have been founded"
        );
        assert!(
            total_units >= 2 || engine.current_turn() > 10,
            "Game should have progressed: {} units, {} turns",
            total_units,
            engine.current_turn()
        );
    }

    #[test]
    fn test_ai_does_not_build_settler_when_one_exists() {
        use crate::protocol::AvailableCommand;

        let engine = Engine::new_game(&test_config()).unwrap();
        // Player has a settler and 3 warriors (triggers settler heuristic normally)
        let _settler = spawn_settler(&engine, PlayerId(0), TileCoord { x: 5, y: 5 });
        let _w1 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 3, y: 3 });
        let _w2 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 4, y: 3 });
        let _w3 = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 3, y: 4 });
        engine.update_visibility(PlayerId(0));

        let mut agent = SimpleAgent::new(ChaCha8Rng::seed_from_u64(42));

        // Create a view that simulates: 1 city, 3 military, 1 settler, city idle
        let view = engine.player_view(PlayerId(0));

        // Build fake available commands with SetProduction including settler
        let settler_type_id = {
            let world = engine.world().borrow();
            world.unit_types.get_by_name("settler").unwrap().id
        };
        let warrior_type_id = {
            let world = engine.world().borrow();
            world.unit_types.get_by_name("warrior").unwrap().id
        };

        let available = vec![AvailableCommand::SetProduction {
            city_id: crate::id::GenId {
                index: 0,
                generation: 0,
            },
            options: vec![
                crate::protocol::ProductionOption {
                    name: "warrior".to_string(),
                    item: crate::city::ProductionItem::Unit {
                        unit_type_id: warrior_type_id,
                    },
                    cost: 10,
                },
                crate::protocol::ProductionOption {
                    name: "settler".to_string(),
                    item: crate::city::ProductionItem::Unit {
                        unit_type_id: settler_type_id,
                    },
                    cost: 30,
                },
            ],
        }];

        let cmd = agent.decide(&view, &available);

        // AI should NOT pick settler since it already has one
        if let Command::SetProduction { item, .. } = &cmd {
            match item {
                ProductionItem::Unit { unit_type_id } => {
                    let world = engine.world().borrow();
                    let ut = world.unit_types.get(*unit_type_id).unwrap();
                    assert_ne!(
                        ut.name, "settler",
                        "AI should not build settler when one already exists"
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_ai_wrapping_distance() {
        // Verify the distance function accounts for wrapping
        let a = TileCoord { x: 1, y: 5 };
        let b = TileCoord { x: 9, y: 5 };

        // Without wrapping: distance is 8
        assert_eq!(SimpleAgent::distance(a, b, None), 8);

        // With wrapping on map width 10: distance is 2
        assert_eq!(SimpleAgent::distance(a, b, Some(10)), 2);

        // With wrapping on map width 20: distance is still 8 (non-wrapped is shorter)
        assert_eq!(SimpleAgent::distance(a, b, Some(20)), 8);
    }

    #[test]
    fn test_ai_chases_visible_enemy() {
        // AI should move toward a visible enemy within 10 tiles
        let engine = Engine::new_game(&test_config()).unwrap();
        let p0_unit = spawn_warrior(&engine, PlayerId(0), TileCoord { x: 2, y: 5 });
        let _enemy = spawn_warrior(&engine, PlayerId(1), TileCoord { x: 7, y: 5 });

        // Set all tiles to grassland and visible so the path is clear
        {
            let mut world = engine.world().borrow_mut();
            for y in 0..world.tiles.height {
                for x in 0..world.tiles.width {
                    let idx = world.tiles.idx(x, y);
                    world.tiles.terrain[idx] = crate::tile::Terrain::Grassland;
                    world
                        .tiles
                        .set_visibility(PlayerId(0), x, y, crate::tile::Visibility::Visible);
                }
            }
        }

        let mut agent = SimpleAgent::new(ChaCha8Rng::seed_from_u64(42));
        let view = engine.player_view(PlayerId(0));
        let available = engine.available_commands(PlayerId(0));
        let cmd = agent.decide(&view, &available);

        // Should issue a move toward the enemy (adjacent step, closer than start)
        match &cmd {
            Command::MoveUnit {
                unit_id,
                destination,
            } => {
                assert_eq!(*unit_id, p0_unit);
                let initial_dist = SimpleAgent::distance(
                    TileCoord { x: 2, y: 5 },
                    TileCoord { x: 7, y: 5 },
                    Some(10),
                );
                let new_dist =
                    SimpleAgent::distance(*destination, TileCoord { x: 7, y: 5 }, Some(10));
                assert!(
                    new_dist < initial_dist,
                    "AI should move closer to enemy: initial_dist={}, new_dist={}, dest={:?}",
                    initial_dist,
                    new_dist,
                    destination
                );
            }
            _ => panic!("AI should move toward visible enemy, got: {:?}", cmd),
        }
    }
}
