mod action;
mod city;
mod combat;
mod movement;
mod visibility;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::pathfinding::MovementCostCache;
use crate::protocol::{
    AvailableCommand, Command, CommandResult, Event, GameError, PlayerView, TechOption,
};
use crate::scripting::ScriptEngine;
use crate::tile::{Terrain, Vegetation};
use crate::turn::{TurnStateMachine, TurnTransition};
use crate::types::PlayerId;
use crate::world::{World, WorldConfig};

/// Configuration for starting a new game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub world: WorldConfig,
    pub mod_paths: Vec<String>,
    /// Unit type names each player starts with. Default: empty (no auto-spawn).
    #[serde(default)]
    pub units_per_player: Vec<String>,
    /// Maximum turns before game ends. None = no limit.
    #[serde(default)]
    pub max_turns: Option<u32>,
}

/// Converts a Terrain enum to the lowercase string used in Lua hooks.
pub fn terrain_to_str(terrain: Terrain) -> &'static str {
    match terrain {
        Terrain::Grassland => "grassland",
        Terrain::Plains => "plains",
        Terrain::Desert => "desert",
        Terrain::Tundra => "tundra",
        Terrain::Ocean => "ocean",
        Terrain::Coast => "coast",
        Terrain::Mountain => "mountain",
        Terrain::Hill => "hill",
        Terrain::Ice => "ice",
    }
}

/// Converts a Vegetation enum to the lowercase string used in Lua hooks.
pub fn vegetation_to_str(v: Vegetation) -> &'static str {
    match v {
        Vegetation::None => "none",
        Vegetation::Forest => "forest",
        Vegetation::Jungle => "jungle",
    }
}

/// The main game engine. Owns the world, script engine, and turn state.
pub struct Engine {
    world: Rc<RefCell<World>>,
    scripts: ScriptEngine,
    turn_state: TurnStateMachine,
    pub event_log: Vec<(u32, PlayerId, Command)>,
    max_turns: Option<u32>,
    movement_cache: MovementCostCache,
}

impl Engine {
    /// Creates a new game from the given configuration.
    /// Initializes the world, script engine, all Lua APIs, loads mods,
    /// spawns starting units, initializes fog of war, and fires on_game_start.
    pub fn new_game(config: &GameConfig) -> Result<Self, mlua::Error> {
        let world = if config.units_per_player.is_empty() {
            // Legacy/test mode: flat grassland
            World::new(&config.world)
        } else {
            // Full game mode: generate terrain
            World::new_with_terrain(&config.world)
        };
        let player_order: Vec<PlayerId> = world.players.iter().map(|p| p.id).collect();
        let turn_state = TurnStateMachine::new(player_order.clone());

        let world = Rc::new(RefCell::new(world));

        let scripts = ScriptEngine::new()?;
        scripts.lua.set_app_data(Rc::clone(&world));

        // Register all Lua APIs
        crate::scripting::api_unit_type::register(&scripts.lua)?;
        crate::scripting::api_unit::register(&scripts.lua)?;
        crate::scripting::api_tile::register(&scripts.lua)?;
        crate::scripting::api_engine::register(&scripts.lua)?;
        crate::scripting::api_city::register(&scripts.lua)?;
        crate::scripting::api_action::register(&scripts.lua)?;
        crate::scripting::api_tech::register(&scripts.lua)?;
        crate::scripting::api_building::register(&scripts.lua)?;
        crate::scripting::api_civilization::register(&scripts.lua)?;

        // Load mods
        for mod_path in &config.mod_paths {
            let path = Path::new(mod_path);
            if path.is_absolute() {
                scripts.load_mod(path)?;
            } else {
                let resolved = ScriptEngine::find_mod_dir(mod_path);
                scripts.load_mod(&resolved)?;
            }
        }

        // Assign civilizations to players (after mods define civs)
        {
            use rand::seq::SliceRandom;
            let mut w = world.borrow_mut();
            let mut civ_ids: Vec<String> = w
                .civilization_registry
                .all()
                .iter()
                .map(|c| c.id.clone())
                .collect();
            civ_ids.shuffle(&mut w.rng);
            // Pre-lookup ruler names to avoid borrow conflict
            let assignments: Vec<(String, String)> = civ_ids
                .iter()
                .filter_map(|cid| {
                    w.civilization_registry
                        .get(cid)
                        .map(|def| (cid.clone(), def.ruler_name.clone()))
                })
                .collect();
            for (i, player) in w.players.iter_mut().enumerate() {
                if let Some((civ_id, ruler_name)) = assignments.get(i) {
                    player.civilization = Some(civ_id.clone());
                    player.name = ruler_name.clone();
                }
            }
        }

        let engine = Engine {
            world,
            scripts,
            turn_state,
            event_log: Vec::new(),
            max_turns: config.max_turns,
            movement_cache: MovementCostCache::new(),
        };

        // Spawn starting units if configured
        if !config.units_per_player.is_empty() {
            let starting_positions = {
                let w = engine.world.borrow();
                let mut rng = w.rng.clone();
                let positions = crate::mapgen::compute_starting_positions(
                    &w.tiles,
                    config.world.num_players,
                    &mut rng,
                );
                drop(w);
                // Update the rng state in the world
                engine.world.borrow_mut().rng = rng;
                positions
            };

            for (i, player_id) in player_order.iter().enumerate() {
                let pos = starting_positions[i];
                let mut w = engine.world.borrow_mut();
                for unit_name in &config.units_per_player {
                    if let Some(ut) = w.unit_types.get_by_name(unit_name) {
                        let type_id = ut.id;
                        let template = ut.clone();
                        w.units.spawn(type_id, *player_id, pos, &template);
                    }
                }
            }

            // Initialize fog of war for each player
            for player_id in &player_order {
                engine.update_visibility(*player_id);
            }
        }

        // Fire on_game_start hook
        engine.fire_hook_simple("on_game_start", &[]);

        Ok(engine)
    }

    /// Returns the current player whose turn it is.
    pub fn current_player(&self) -> PlayerId {
        self.turn_state.current_player()
    }

    /// Returns the current turn number.
    pub fn current_turn(&self) -> u32 {
        self.turn_state.turn_number
    }

    /// Returns whether a player is still alive.
    pub fn is_player_alive(&self, player: PlayerId) -> bool {
        let world = self.world.borrow();
        world
            .players
            .get(player.0 as usize)
            .is_some_and(|p| p.alive)
    }

    /// Returns the winner if the game is over, or None if still in progress.
    pub fn is_game_over(&self) -> Option<PlayerId> {
        let world = self.world.borrow();
        let alive: Vec<PlayerId> = world
            .players
            .iter()
            .filter(|p| p.alive)
            .map(|p| p.id)
            .collect();
        if alive.len() == 1 {
            return Some(alive[0]);
        }
        // Check turn limit
        if let Some(max) = self.max_turns {
            if world.turn > max {
                // Winner is the alive player with the most units (tiebreak: lower ID)
                return self.player_with_most_units(&alive, &world);
            }
        }
        None
    }

    /// Returns the player with the most alive units among the given candidates.
    fn player_with_most_units(&self, candidates: &[PlayerId], world: &World) -> Option<PlayerId> {
        let mut best = None;
        let mut best_count = 0;
        for &pid in candidates {
            let count = world
                .units
                .iter_alive()
                .filter(|&(_, idx)| world.units.owner[idx] == pid)
                .count();
            if count > best_count || best.is_none() {
                best_count = count;
                best = Some(pid);
            }
        }
        best
    }

    /// Builds the player view for the given player (fog-of-war filtered).
    pub fn player_view(&self, player: PlayerId) -> PlayerView {
        self.world.borrow().build_player_view(player)
    }

    /// Builds a debug view with all tiles visible and all units shown.
    /// Uses the given player for economy/research data but ignores fog of war.
    pub fn debug_view(&self, player: PlayerId) -> PlayerView {
        self.world.borrow().build_debug_view(player)
    }

    /// Returns (unit_type_name, art_ini_path) for all unit types that have art defined.
    pub fn unit_art_paths(&self) -> Vec<(String, String)> {
        let world = self.world.borrow();
        world
            .unit_types
            .iter()
            .filter_map(|ut| {
                ut.art_ini
                    .as_ref()
                    .map(|ini| (ut.name.clone(), ini.clone()))
            })
            .collect()
    }

    /// Returns a reference to the shared world.
    pub fn world(&self) -> &Rc<RefCell<World>> {
        &self.world
    }

    /// Returns a reference to the script engine.
    pub fn scripts(&self) -> &ScriptEngine {
        &self.scripts
    }

    /// Queries the pathfinder for a path from a unit to a destination without
    /// modifying game state. Returns the path result including per-tile costs.
    pub fn query_path(
        &mut self,
        player: PlayerId,
        unit_id: crate::id::UnitId,
        destination: crate::types::TileCoord,
    ) -> Option<crate::pathfinding::PathResult> {
        use crate::scripting::api_unit::unit_id_to_lua;

        let (from, unit_type_id) = {
            let world = self.world.borrow();
            let idx = world.units.get(unit_id)?;
            if world.units.owner[idx] != player {
                return None;
            }
            (world.units.position[idx], world.units.unit_type[idx])
        };

        let unit_id_lua = unit_id_to_lua(unit_id);
        self.find_unit_path(player, unit_id_lua, unit_type_id, from, destination, true)
    }

    /// Returns the list of available commands for the given player.
    #[allow(clippy::type_complexity)]
    pub fn available_commands(&self, player: PlayerId) -> Vec<AvailableCommand> {
        let mut commands = Vec::new();
        let mut action_candidates: Vec<(
            crate::id::UnitId,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<(i32, i32)>,
        )> = Vec::new();
        let mut city_ids_for_player: Vec<crate::id::CityId> = Vec::new();
        let has_unhandled_unit;

        {
            let world = self.world.borrow();

            for (uid, idx) in world.units.iter_alive() {
                if world.units.owner[idx] != player {
                    continue;
                }

                let movement = world.units.movement[idx];
                if movement <= 0 {
                    continue;
                }

                if world.units.skipped[idx] {
                    continue;
                }

                // Skip units with active multi-turn actions
                if world.units.current_action[idx].is_some() {
                    continue;
                }

                let pos = world.units.position[idx];

                // Move destinations: adjacent passable tiles (category-aware)
                let neighbors = world.tiles.neighbors(pos.x, pos.y);
                let type_id = world.units.unit_type[idx];
                let is_naval = world
                    .unit_types
                    .get(type_id)
                    .is_some_and(|ut| ut.category == "naval");
                let mut valid_dests = Vec::new();
                for dest in &neighbors {
                    let tile_idx = world.tiles.idx(dest.x, dest.y);
                    let terrain = world.tiles.terrain[tile_idx];
                    let passable = if is_naval {
                        matches!(
                            terrain,
                            crate::tile::Terrain::Ocean | crate::tile::Terrain::Coast
                        )
                    } else {
                        !matches!(
                            terrain,
                            crate::tile::Terrain::Ocean
                                | crate::tile::Terrain::Coast
                                | crate::tile::Terrain::Mountain
                                | crate::tile::Terrain::Ice
                        )
                    };
                    if passable {
                        valid_dests.push(*dest);
                    }
                }
                if !valid_dests.is_empty() {
                    commands.push(AvailableCommand::Move {
                        unit_id: uid,
                        valid_destinations: valid_dests,
                    });
                }

                // Attack targets: adjacent enemy units on visible tiles
                let mut targets = Vec::new();
                for (other_uid, other_idx) in world.units.iter_alive() {
                    if world.units.owner[other_idx] == player {
                        continue;
                    }
                    let other_pos = world.units.position[other_idx];
                    if neighbors.contains(&other_pos) {
                        // Check visibility
                        let vis = world.tiles.get_visibility(player, other_pos.x, other_pos.y);
                        if vis == crate::tile::Visibility::Visible {
                            targets.push(other_uid);
                        }
                    }
                }
                if !targets.is_empty() {
                    commands.push(AvailableCommand::Attack {
                        unit_id: uid,
                        targets,
                    });
                }

                // Fortify: if not already fortified
                if !world.units.fortified[idx] {
                    commands.push(AvailableCommand::Fortify { unit_id: uid });
                }

                // Skip: always available for units with movement
                commands.push(AvailableCommand::Skip { unit_id: uid });

                // UnitAction candidates: collect actions from unit type
                let type_id = world.units.unit_type[idx];
                if let Some(ut) = world.unit_types.get(type_id) {
                    for action_id in &ut.actions {
                        if let Some(def) = world.action_registry.get(action_id) {
                            action_candidates.push((
                                uid,
                                action_id.clone(),
                                def.name.clone(),
                                def.hotkey.clone(),
                                def.animation_name.clone(),
                                def.icon_atlas_pos,
                            ));
                        }
                    }
                }
            }

            // Collect city IDs for production options (built outside borrow)
            for (cid, idx) in world.cities.iter_alive() {
                if world.cities.owner[idx] == player {
                    city_ids_for_player.push(cid);
                }
            }

            // Check if any unit has movement remaining and hasn't been dealt with.
            // Units with a destination set are considered "handled" (they'll auto-move).
            has_unhandled_unit = world.units.iter_alive().any(|(_, idx)| {
                world.units.owner[idx] == player
                    && world.units.movement[idx] > 0
                    && !world.units.fortified[idx]
                    && !world.units.skipped[idx]
                    && world.units.current_action[idx].is_none()
                    && world.units.destination[idx].is_none()
            });
        }

        // SetProduction: build options per city using can_produce callbacks
        for city_id in &city_ids_for_player {
            let options = self.buildable_options(*city_id);
            commands.push(AvailableCommand::SetProduction {
                city_id: *city_id,
                options,
            });
        }

        // SetResearch: offer tech options when player has cities and no research set
        let has_cities = !city_ids_for_player.is_empty();
        let needs_research = has_cities && {
            let world = self.world.borrow();
            world
                .players
                .get(player.0 as usize)
                .is_some_and(|p| p.researching.is_none())
        };
        if needs_research {
            let techs = self.available_techs(player);
            if !techs.is_empty() {
                commands.push(AvailableCommand::SetResearch { options: techs });
            }
        }

        // EndTurn is only available when all units have been handled
        if !has_unhandled_unit {
            commands.push(AvailableCommand::EndTurn);
        }

        // UnitAction: check valid_conditions for each action candidate
        for (uid, action_id, name, hotkey, animation_name, icon_atlas_pos) in action_candidates {
            if self.check_action_valid(uid, &action_id).is_none() {
                commands.push(AvailableCommand::UnitAction {
                    unit_id: uid,
                    action_id,
                    name,
                    hotkey,
                    animation_name,
                    icon_atlas_pos,
                });
            }
        }

        commands
    }

    /// Submits a command on behalf of a player.
    /// Validates turn order, dispatches to the appropriate handler,
    /// and logs the command.
    pub fn submit_command(&mut self, player: PlayerId, command: Command) -> CommandResult {
        // Validate turn
        if !self.turn_state.is_player_turn(player) {
            return CommandResult::err(GameError::NotYourTurn);
        }

        // Log command before executing
        let turn = self.turn_state.turn_number;
        self.event_log.push((turn, player, command.clone()));

        match command {
            Command::EndTurn => self.handle_end_turn(player),
            Command::MoveUnit {
                unit_id,
                destination,
            } => self.handle_move_unit(player, unit_id, destination),
            Command::AttackUnit { attacker, defender } => {
                self.handle_attack_unit(player, attacker, defender)
            }
            Command::FortifyUnit { unit_id } => self.handle_fortify_unit(player, unit_id),
            Command::SkipUnit { unit_id } => self.handle_skip_unit(player, unit_id),
            Command::SetProduction { city_id, item } => {
                self.handle_set_production(player, city_id, item)
            }
            Command::PerformAction { unit_id, action_id } => {
                self.handle_perform_action(player, unit_id, action_id)
            }
            Command::SetResearch { tech_id } => self.handle_set_research(player, tech_id),
        }
    }

    fn handle_end_turn(&mut self, player: PlayerId) -> CommandResult {
        {
            let world = self.world.borrow();

            // Validate all units have been handled (moved, fortified, skipped, performing an action,
            // or have a destination set for auto-move)
            let has_unhandled = world.units.iter_alive().any(|(_, idx)| {
                world.units.owner[idx] == player
                    && world.units.movement[idx] > 0
                    && !world.units.fortified[idx]
                    && !world.units.skipped[idx]
                    && world.units.current_action[idx].is_none()
                    && world.units.destination[idx].is_none()
            });
            if has_unhandled {
                return CommandResult::err(GameError::Custom(
                    "all units must be moved, fortified, or skipped".to_string(),
                ));
            }
        }

        let mut events = Vec::new();

        // Accumulate commerce → gold and science for the ending player,
        // then deduct building maintenance
        {
            let mut world = self.world.borrow_mut();
            let mut total_commerce = 0i32;
            let mut total_maintenance = 0i32;
            for (_, idx) in world.cities.iter_alive() {
                if world.cities.owner[idx] == player {
                    total_commerce += world.cities.commerce_per_turn[idx];
                    for bid in &world.cities.buildings[idx] {
                        if let Some(def) = world.building_registry.get(bid) {
                            total_maintenance += def.maintenance;
                        }
                    }
                }
            }
            if let Some(p) = world.players.get_mut(player.0 as usize) {
                p.gold += total_commerce - total_maintenance;
                if p.researching.is_some() {
                    p.science += total_commerce;
                }
            }
        }

        // Check tech completion
        {
            let mut world = self.world.borrow_mut();
            // Read the tech cost first to avoid borrow conflict
            let tech_info = world
                .players
                .get(player.0 as usize)
                .and_then(|p| p.researching.clone())
                .and_then(|tech_id| {
                    if tech_id.starts_with("future_tech_") {
                        Some((tech_id, Self::FUTURE_TECH_COST))
                    } else {
                        world.tech_registry.get(&tech_id).map(|t| (tech_id, t.cost))
                    }
                });
            if let Some((tech_id, tech_cost)) = tech_info {
                if let Some(p) = world.players.get_mut(player.0 as usize) {
                    if p.science >= tech_cost {
                        p.science -= tech_cost;
                        p.researched_techs.push(tech_id.clone());
                        p.researching = None;
                        events.push(Event::TechResearched { player, tech_id });
                    }
                }
            }
        }

        // Fire on_turn_end hook
        self.fire_hook_simple("on_turn_end", &[("player_id", player.0 as i64)]);

        // Advance turn state, skipping eliminated players
        let num_players = self.turn_state.player_count();
        let mut skipped = 0;
        loop {
            let transition = self.turn_state.advance_player();
            let (next_player, turn) = match transition {
                TurnTransition::NextPlayer(p) => (p, self.turn_state.turn_number),
                TurnTransition::NewTurn(t) => (self.turn_state.current_player(), t),
            };

            // If this player is alive, proceed normally
            if self.is_player_alive(next_player) {
                // Update world turn tracking
                {
                    let mut world = self.world.borrow_mut();
                    world.current_player = next_player;
                    world.turn = turn;
                }

                // Invalidate movement cost cache at turn start
                self.movement_cache.invalidate();

                // Fire on_turn_start hook
                self.fire_hook_simple(
                    "on_turn_start",
                    &[("player_id", next_player.0 as i64), ("turn", turn as i64)],
                );

                // Fire on_turn_start_unit for each unit belonging to the new player
                let unit_ids: Vec<i64> = {
                    let world = self.world.borrow();
                    world
                        .units
                        .iter_alive()
                        .filter(|&(_, idx)| world.units.owner[idx] == next_player)
                        .map(|(uid, _)| crate::scripting::api_unit::unit_id_to_lua(uid))
                        .collect()
                };
                for uid_lua in unit_ids {
                    self.fire_hook_simple(
                        "on_turn_start_unit",
                        &[("unit_id", uid_lua), ("player_id", next_player.0 as i64)],
                    );
                }

                // Process city food/growth and production
                let food_events = self.process_city_food(next_player);
                events.extend(food_events);
                let prod_events = self.process_city_production(next_player);
                events.extend(prod_events);

                // Process multi-turn actions
                let action_events = self.process_unit_actions(next_player);
                events.extend(action_events);

                // Auto-move units with destinations
                let auto_events = self.auto_move_units(next_player);
                events.extend(auto_events);

                events.push(Event::TurnStarted {
                    player: next_player,
                    turn,
                });

                // Check turn limit
                if let Some(max) = self.max_turns {
                    if turn > max {
                        let world = self.world.borrow();
                        let alive: Vec<PlayerId> = world
                            .players
                            .iter()
                            .filter(|p| p.alive)
                            .map(|p| p.id)
                            .collect();
                        if let Some(winner) = self.player_with_most_units(&alive, &world) {
                            drop(world);
                            events.push(Event::GameOver { winner });
                        }
                    }
                }
                break;
            }

            // Skip eliminated player
            skipped += 1;
            if skipped >= num_players {
                // All players eliminated — shouldn't happen normally
                break;
            }
        }

        CommandResult::with_events(events)
    }

    fn handle_fortify_unit(
        &mut self,
        player: PlayerId,
        unit_id: crate::id::UnitId,
    ) -> CommandResult {
        use crate::scripting::api_unit::unit_id_to_lua;

        {
            let mut world = self.world.borrow_mut();
            let idx = match world.units.get(unit_id) {
                Some(i) => i,
                None => return CommandResult::err(GameError::InvalidUnit),
            };
            if world.units.owner[idx] != player {
                return CommandResult::err(GameError::InvalidUnit);
            }
            world.units.fortified[idx] = true;
            world.units.destination[idx] = None;
        }

        // Fire on_unit_fortified hook
        self.fire_hook_simple("on_unit_fortified", &[("unit_id", unit_id_to_lua(unit_id))]);

        CommandResult::ok(Event::UnitFortified { unit_id })
    }

    fn handle_skip_unit(&mut self, player: PlayerId, unit_id: crate::id::UnitId) -> CommandResult {
        use crate::scripting::api_unit::unit_id_to_lua;

        {
            let mut world = self.world.borrow_mut();
            let idx = match world.units.get(unit_id) {
                Some(i) => i,
                None => return CommandResult::err(GameError::InvalidUnit),
            };
            if world.units.owner[idx] != player {
                return CommandResult::err(GameError::InvalidUnit);
            }
            world.units.skipped[idx] = true;
            world.units.destination[idx] = None;
        }

        // Fire on_unit_skipped hook
        self.fire_hook_simple("on_unit_skipped", &[("unit_id", unit_id_to_lua(unit_id))]);

        CommandResult::ok(Event::UnitSkipped { unit_id })
    }

    /// Cost of each Future Tech level.
    const FUTURE_TECH_COST: i32 = 300;

    /// Returns the list of technologies available for a player to research.
    pub(super) fn available_techs(&self, player: PlayerId) -> Vec<TechOption> {
        let world = self.world.borrow();
        let p = match world.players.get(player.0 as usize) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let researched = &p.researched_techs;
        let techs: Vec<TechOption> = world
            .tech_registry
            .all()
            .iter()
            .filter(|tech| {
                // Not already researched
                !researched.contains(&tech.id)
                    // All prerequisites met
                    && tech.requires.iter().all(|req| researched.contains(req))
            })
            .map(|tech| TechOption {
                id: tech.id.clone(),
                name: tech.name.clone(),
                cost: tech.cost,
            })
            .collect();

        // If no regular techs available but player has researched at least one,
        // inject a synthetic Future Tech.
        if techs.is_empty() && !researched.is_empty() {
            let n = researched
                .iter()
                .filter(|t| t.starts_with("future_tech_"))
                .count()
                + 1;
            vec![TechOption {
                id: format!("future_tech_{n}"),
                name: format!("Future Tech {n}"),
                cost: Self::FUTURE_TECH_COST,
            }]
        } else {
            techs
        }
    }

    fn handle_set_research(&mut self, player: PlayerId, tech_id: String) -> CommandResult {
        // Future techs are synthetic — validate via available_techs instead of registry
        if tech_id.starts_with("future_tech_") {
            let available = self.available_techs(player);
            if !available.iter().any(|t| t.id == tech_id) {
                return CommandResult::err(GameError::Custom(format!(
                    "future tech not available: {tech_id}"
                )));
            }
        } else {
            // Validate tech exists and is available
            let world = self.world.borrow();
            let p = match world.players.get(player.0 as usize) {
                Some(p) => p,
                None => return CommandResult::err(GameError::Custom("invalid player".to_string())),
            };
            let tech = match world.tech_registry.get(&tech_id) {
                Some(t) => t,
                None => {
                    return CommandResult::err(GameError::Custom(format!(
                        "unknown tech: {tech_id}"
                    )))
                }
            };
            if p.researched_techs.contains(&tech_id) {
                return CommandResult::err(GameError::Custom(format!(
                    "already researched: {tech_id}"
                )));
            }
            if !tech
                .requires
                .iter()
                .all(|req| p.researched_techs.contains(req))
            {
                return CommandResult::err(GameError::Custom(format!(
                    "prerequisites not met for: {tech_id}"
                )));
            }
        }

        // Set researching
        {
            let mut world = self.world.borrow_mut();
            if let Some(p) = world.players.get_mut(player.0 as usize) {
                p.researching = Some(tech_id.clone());
            }
        }

        CommandResult::ok(Event::ResearchSet { player, tech_id })
    }

    /// Fire a simple hook with key-value pairs as the context table.
    fn fire_hook_simple(&self, hook_name: &str, pairs: &[(&str, i64)]) {
        let fields: String = pairs
            .iter()
            .map(|(k, v)| format!("{k} = {v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let code = format!(r#"fire_hook("{hook_name}", {{ {fields} }})"#,);
        // Ignore errors from hooks — they shouldn't crash the engine
        let _ = self.scripts.lua.load(code).exec();
    }

    /// Extracts a GameLog from the current engine state.
    pub fn to_game_log(&self, config: &GameConfig) -> GameLog {
        GameLog {
            seed: config.world.seed,
            game_config: config.clone(),
            commands: self.event_log.clone(),
        }
    }
}

/// A complete record of a game for deterministic replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLog {
    pub seed: u64,
    pub game_config: GameConfig,
    pub commands: Vec<(u32, PlayerId, Command)>,
}

impl GameLog {
    /// Replays the game log, producing an Engine in the final state.
    pub fn replay(&self) -> Result<Engine, mlua::Error> {
        let mut engine = Engine::new_game(&self.game_config)?;
        for (_turn, player, command) in &self.commands {
            engine.submit_command(*player, command.clone());
        }
        Ok(engine)
    }
}
