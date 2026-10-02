use crate::pathfinding::{self, IMPASSABLE};
use crate::protocol::{CommandResult, Event, GameError};
use crate::types::{Direction, PlayerId, TileCoord};

use super::{terrain_to_str, vegetation_to_str, Engine};

/// Computes the shortest-path delta between two tiles, accounting for wrapping.
/// Returns (dx, dy) where each component is in [-w/2, w/2] (or [-h/2, h/2]).
fn wrapped_delta(
    from: TileCoord,
    to: TileCoord,
    width: u32,
    height: u32,
    wrap_x: bool,
    wrap_y: bool,
) -> (i32, i32) {
    let w = width as i32;
    let h = height as i32;

    let mut dx = to.x as i32 - from.x as i32;
    if wrap_x {
        if dx > w / 2 {
            dx -= w;
        }
        if dx < -w / 2 {
            dx += w;
        }
    }

    let mut dy = to.y as i32 - from.y as i32;
    if wrap_y {
        if dy > h / 2 {
            dy -= h;
        }
        if dy < -h / 2 {
            dy += h;
        }
    }

    (dx, dy)
}

impl Engine {
    pub(super) fn handle_move_unit(
        &mut self,
        player: PlayerId,
        unit_id: crate::id::UnitId,
        destination: crate::types::TileCoord,
    ) -> CommandResult {
        use crate::scripting::api_unit::unit_id_to_lua;

        // Validate unit
        let (from, movement, unit_type_id) = {
            let world = self.world.borrow();
            let idx = match world.units.get(unit_id) {
                Some(i) => i,
                None => return CommandResult::err(GameError::InvalidUnit),
            };
            if world.units.owner[idx] != player {
                return CommandResult::err(GameError::InvalidUnit);
            }
            let movement = world.units.movement[idx];
            (
                world.units.position[idx],
                movement,
                world.units.unit_type[idx],
            )
        };

        // Validate destination is in bounds
        {
            let world = self.world.borrow();
            if !world.tiles.in_bounds(destination.x, destination.y) {
                return CommandResult::err(GameError::InvalidTarget);
            }
        }

        // Store destination on the unit
        {
            let mut world = self.world.borrow_mut();
            let idx = world.units.get(unit_id).unwrap();
            world.units.destination[idx] = Some(destination);
        }

        // If unit has no movement, just set destination for next turn
        if movement <= 0 {
            return CommandResult::ok(Event::DestinationSet {
                unit_id,
                destination,
            });
        }

        // Build path using A* pathfinder (fog_of_war=true: unseen tiles assumed passable,
        // step-by-step execution uses real terrain and stops if blocked)
        let unit_id_lua = unit_id_to_lua(unit_id);
        let path_result =
            self.find_unit_path(player, unit_id_lua, unit_type_id, from, destination, true);

        let path = match path_result {
            Some(p) => p,
            None => {
                // No path found: clear destination
                {
                    let mut world = self.world.borrow_mut();
                    let idx = world.units.get(unit_id).unwrap();
                    world.units.destination[idx] = None;
                }
                return CommandResult::ok(Event::MoveBlocked {
                    unit_id,
                    reason: "no path".to_string(),
                });
            }
        };

        // Execute the path tile-by-tile
        let mut events = Vec::new();
        let mut current_pos = from;
        let mut remaining_movement = movement;

        for step_idx in 1..path.tiles.len() {
            let step_tile = path.tiles[step_idx];

            // Calculate cost for this step
            let step_cost =
                self.compute_tile_cost(unit_id_lua, unit_type_id, current_pos, step_tile);

            if step_cost == IMPASSABLE {
                // Should not happen if pathfinder is correct, but safety check
                events.push(Event::MoveBlocked {
                    unit_id,
                    reason: "impassable terrain".to_string(),
                });
                break;
            }

            // Check if unit has enough movement
            // Civ3 rule: if movement > 0 (any remaining), the unit can always make one more move
            // regardless of the tile's cost. This prevents units from getting stuck with
            // fractional movement remaining.
            if remaining_movement <= 0 {
                break;
            }

            // ZoC check: if current tile is in enemy ZoC AND next tile is also in enemy ZoC,
            // the unit must stop (cannot move ZoC-to-ZoC).
            if self.is_zoc_tile(player, current_pos) && self.is_zoc_tile(player, step_tile) {
                // Set movement to 0 (pinned) and clear destination
                {
                    let mut world = self.world.borrow_mut();
                    let idx = world.units.get(unit_id).unwrap();
                    world.units.movement[idx] = 0;
                    world.units.destination[idx] = None;
                }
                events.push(Event::DestinationCleared { unit_id });
                events.push(Event::MoveInterrupted {
                    unit_id,
                    at: current_pos,
                    reason: crate::protocol::MoveInterruptReason::ZoneOfControl,
                    remaining_path: path.tiles[step_idx..].to_vec(),
                });
                break;
            }

            let step_from = current_pos;

            // Execute the move
            {
                let mut world = self.world.borrow_mut();
                let idx = world.units.get(unit_id).unwrap();
                world.units.position[idx] = step_tile;
                world.units.movement[idx] = (world.units.movement[idx] - step_cost).max(0);
                world.units.has_moved[idx] = true;
                world.units.fortified[idx] = false;
                // Update facing direction
                let (dx, dy) = wrapped_delta(
                    step_from,
                    step_tile,
                    world.tiles.width,
                    world.tiles.height,
                    world.tiles.wrap_x,
                    world.tiles.wrap_y,
                );
                if let Some(dir) = Direction::from_delta(dx, dy) {
                    world.units.direction[idx] = dir;
                }
            }

            remaining_movement = {
                let world = self.world.borrow();
                let idx = world.units.get(unit_id).unwrap();
                world.units.movement[idx]
            };

            current_pos = step_tile;

            // Capture visible tiles before fog update for reveal detection
            let visible_before: std::collections::HashSet<(u32, u32)> = {
                let world = self.world.borrow();
                let w = world.tiles.width;
                let h = world.tiles.height;
                let mut set = std::collections::HashSet::new();
                for y in 0..h {
                    for x in 0..w {
                        if world.tiles.get_visibility(player, x, y)
                            != crate::tile::Visibility::Unseen
                        {
                            set.insert((x, y));
                        }
                    }
                }
                set
            };

            // Fire on_unit_moved hook
            self.fire_hook_simple(
                "on_unit_moved",
                &[
                    ("unit_id", unit_id_lua),
                    ("from_x", step_from.x as i64),
                    ("from_y", step_from.y as i64),
                    ("to_x", step_tile.x as i64),
                    ("to_y", step_tile.y as i64),
                ],
            );

            // Update fog of war for the owner
            self.update_visibility(player);

            events.push(Event::UnitMoved {
                unit_id,
                from: step_from,
                to: step_tile,
                movement_left: remaining_movement,
            });

            // Detect newly revealed tiles
            let newly_revealed: Vec<crate::types::TileCoord> = {
                let world = self.world.borrow();
                let w = world.tiles.width;
                let h = world.tiles.height;
                let mut revealed = Vec::new();
                for y in 0..h {
                    for x in 0..w {
                        if !visible_before.contains(&(x, y))
                            && world.tiles.get_visibility(player, x, y)
                                != crate::tile::Visibility::Unseen
                        {
                            revealed.push(crate::types::TileCoord { x, y });
                        }
                    }
                }
                revealed
            };
            if !newly_revealed.is_empty() {
                events.push(Event::TilesRevealed {
                    player,
                    tiles: newly_revealed.clone(),
                });
            }

            // Check for enemy-spotted interrupt: if newly revealed tiles contain enemy units,
            // and there are more steps in the path, stop movement.
            if step_idx < path.tiles.len() - 1 {
                let enemy_spotted = {
                    let world = self.world.borrow();
                    newly_revealed.iter().any(|tile| {
                        world.units.iter_alive().any(|(_, uidx)| {
                            world.units.owner[uidx] != player && world.units.position[uidx] == *tile
                        })
                    })
                };
                if enemy_spotted {
                    // Clear destination on enemy spotted
                    {
                        let mut world = self.world.borrow_mut();
                        let idx = world.units.get(unit_id).unwrap();
                        world.units.destination[idx] = None;
                    }
                    events.push(Event::DestinationCleared { unit_id });
                    events.push(Event::MoveInterrupted {
                        unit_id,
                        at: current_pos,
                        reason: crate::protocol::MoveInterruptReason::EnemySpotted,
                        remaining_path: path.tiles[(step_idx + 1)..].to_vec(),
                    });
                    break;
                }
            }

            if remaining_movement <= 0 {
                break;
            }
        }

        if events.is_empty() {
            // No movement happened (shouldn't normally happen if movement > 0 and path exists)
            return CommandResult::err(GameError::NotEnoughMovement);
        }

        // If the unit reached its destination, clear it
        if current_pos == destination {
            let mut world = self.world.borrow_mut();
            if let Some(idx) = world.units.get(unit_id) {
                world.units.destination[idx] = None;
            }
            events.push(Event::DestinationCleared { unit_id });
        }

        CommandResult::with_events(events)
    }

    /// Calculates movement cost by firing the Lua hook. Returns the cost,
    /// or a negative value if movement is blocked.
    #[allow(clippy::too_many_arguments)]
    fn calculate_movement_cost(
        &self,
        unit_id_lua: i64,
        from: crate::types::TileCoord,
        to: crate::types::TileCoord,
        terrain: &str,
        vegetation: &str,
        road_level: u8,
        unit_category: &str,
    ) -> i32 {
        // Default costs if no hook modifies (integer-thirds: 3 = 1 movement point)
        let default_cost = match terrain {
            "mountain" | "ocean" => -1,
            "hill" => 6,
            _ if vegetation == "forest" || vegetation == "jungle" => 6,
            _ => 3,
        };

        let result = self
            .scripts
            .lua
            .load(format!(
                r#"
            local ctx = {{
                unit_id = {unit_id_lua},
                from_x = {from_x},
                from_y = {from_y},
                to_x = {to_x},
                to_y = {to_y},
                terrain = "{terrain}",
                vegetation = "{vegetation}",
                road_level = {road_level},
                unit_category = "{unit_category}",
                cost = {default_cost},
                blocked = {blocked}
            }}
            ctx = fire_hook("on_calculate_movement_cost", ctx)
            if ctx.blocked then return -1 end
            return ctx.cost
            "#,
                from_x = from.x,
                from_y = from.y,
                to_x = to.x,
                to_y = to.y,
                road_level = road_level,
                blocked = if default_cost < 0 { "true" } else { "false" },
            ))
            .eval::<i64>();

        match result {
            Ok(v) => v as i32,
            Err(_) => default_cost,
        }
    }

    /// Computes the movement cost for a unit type entering a tile.
    /// Uses the movement cache; on miss, fires the Lua hook.
    /// Returns IMPASSABLE for blocked tiles.
    fn compute_tile_cost(
        &mut self,
        unit_id_lua: i64,
        unit_type_id: crate::types::UnitTypeId,
        from: crate::types::TileCoord,
        to: crate::types::TileCoord,
    ) -> i32 {
        let (terrain, vegetation, road_level, category) = {
            let world = self.world.borrow();
            let tile_idx = world.tiles.idx(to.x, to.y);
            let cat = world
                .unit_types
                .get(unit_type_id)
                .map(|ut| ut.category.clone())
                .unwrap_or_default();
            (
                world.tiles.terrain[tile_idx],
                world.tiles.vegetation[tile_idx],
                world.tiles.road_level[tile_idx],
                cat,
            )
        };
        let terrain_str = terrain_to_str(terrain);
        let vegetation_str = vegetation_to_str(vegetation);

        // Check cache
        if let Some(&cost) = self.movement_cache.costs.get(&(unit_type_id, to)) {
            return cost;
        }

        // Cache miss: compute via Lua
        let raw_cost = self.calculate_movement_cost(
            unit_id_lua,
            from,
            to,
            terrain_str,
            vegetation_str,
            road_level,
            &category,
        );
        let cost = if raw_cost < 0 { IMPASSABLE } else { raw_cost };
        self.movement_cache.costs.insert((unit_type_id, to), cost);
        cost
    }

    /// Finds a path for a unit from `from` to `goal` using A*.
    /// Uses the movement cost cache and Lua hook for costs.
    /// When `fog_of_war` is true, unseen tiles are assumed to cost 1 movement point
    /// instead of revealing the actual terrain to the pathfinder.
    pub(super) fn find_unit_path(
        &mut self,
        player: PlayerId,
        unit_id_lua: i64,
        unit_type_id: crate::types::UnitTypeId,
        from: crate::types::TileCoord,
        goal: crate::types::TileCoord,
        fog_of_war: bool,
    ) -> Option<pathfinding::PathResult> {
        let (map_width, wrap_x, map_height, wrap_y, unit_category) = {
            let world = self.world.borrow();
            let cat = world
                .unit_types
                .get(unit_type_id)
                .map(|ut| ut.category.clone())
                .unwrap_or_default();
            (
                world.tiles.width,
                world.tiles.wrap_x,
                world.tiles.height,
                world.tiles.wrap_y,
                cat,
            )
        };

        // We'll use a RefCell-based approach: temporarily take ownership of the
        // cache, run pathfinding, then put it back.
        let mut cache = std::mem::take(&mut self.movement_cache);

        let cost_fn =
            |edge_from: crate::types::TileCoord, edge_to: crate::types::TileCoord| -> i32 {
                // Fog of war: unseen tiles assume default movement cost (1 move = 3 thirds)
                if fog_of_war {
                    let world = self.world.borrow();
                    let vis = world.tiles.get_visibility(player, edge_to.x, edge_to.y);
                    if vis == crate::tile::Visibility::Unseen {
                        return 3;
                    }
                }

                // Check cache for base terrain cost
                let base_cost = if let Some(&cost) = cache.costs.get(&(unit_type_id, edge_to)) {
                    cost
                } else {
                    let (terrain, vegetation, road_level) = {
                        let world = self.world.borrow();
                        let tile_idx = world.tiles.idx(edge_to.x, edge_to.y);
                        (
                            world.tiles.terrain[tile_idx],
                            world.tiles.vegetation[tile_idx],
                            world.tiles.road_level[tile_idx],
                        )
                    };
                    let terrain_str = terrain_to_str(terrain);
                    let vegetation_str = vegetation_to_str(vegetation);
                    let raw_cost = self.calculate_movement_cost(
                        unit_id_lua,
                        edge_from,
                        edge_to,
                        terrain_str,
                        vegetation_str,
                        road_level,
                        &unit_category,
                    );
                    let cost = if raw_cost < 0 { IMPASSABLE } else { raw_cost };
                    cache.costs.insert((unit_type_id, edge_to), cost);
                    cost
                };

                if base_cost == IMPASSABLE {
                    return IMPASSABLE;
                }

                // ZoC penalty: if both edge_from and edge_to are in enemy ZoC,
                // add a large penalty so the pathfinder avoids ZoC-to-ZoC routes
                // when possible (these stop movement at runtime).
                let from_zoc = self.is_zoc_tile(player, edge_from);
                let to_zoc = self.is_zoc_tile(player, edge_to);
                if from_zoc && to_zoc {
                    base_cost.saturating_add(1000)
                } else {
                    base_cost
                }
            };

        let neighbors_fn = |coord: crate::types::TileCoord| -> Vec<crate::types::TileCoord> {
            let world = self.world.borrow();
            world.tiles.neighbors(coord.x, coord.y)
        };

        let heuristic_fn = |a: crate::types::TileCoord, b: crate::types::TileCoord| -> i32 {
            pathfinding::chebyshev_heuristic(a, b, map_width, wrap_x, map_height, wrap_y)
        };

        let result = pathfinding::find_path(from, goal, cost_fn, neighbors_fn, heuristic_fn);

        // Restore cache
        self.movement_cache = cache;

        result
    }

    /// Auto-moves all units belonging to `player` that have a destination set.
    /// Called at turn start (during handle_end_turn) after movement is restored.
    /// Returns events from the auto-movement.
    pub(super) fn auto_move_units(&mut self, player: PlayerId) -> Vec<Event> {
        // Collect units with destinations and movement
        let units_to_move: Vec<(crate::id::UnitId, crate::types::TileCoord)> = {
            let world = self.world.borrow();
            world
                .units
                .iter_alive()
                .filter(|&(_, idx)| {
                    world.units.owner[idx] == player
                        && world.units.destination[idx].is_some()
                        && world.units.movement[idx] > 0
                })
                .map(|(uid, idx)| (uid, world.units.destination[idx].unwrap()))
                .collect()
        };

        let mut all_events = Vec::new();
        for (unit_id, destination) in units_to_move {
            let result = self.execute_auto_move(player, unit_id, destination);
            all_events.extend(result);
        }
        all_events
    }

    /// Executes movement toward a destination for auto-move (no turn validation).
    /// Reuses the same pathfinding and movement logic as handle_move_unit.
    fn execute_auto_move(
        &mut self,
        player: PlayerId,
        unit_id: crate::id::UnitId,
        destination: crate::types::TileCoord,
    ) -> Vec<Event> {
        use crate::scripting::api_unit::unit_id_to_lua;

        let (from, movement, unit_type_id) = {
            let world = self.world.borrow();
            let idx = match world.units.get(unit_id) {
                Some(i) => i,
                None => return Vec::new(),
            };
            (
                world.units.position[idx],
                world.units.movement[idx],
                world.units.unit_type[idx],
            )
        };

        if movement <= 0 || from == destination {
            if from == destination {
                let mut world = self.world.borrow_mut();
                if let Some(idx) = world.units.get(unit_id) {
                    world.units.destination[idx] = None;
                }
                return vec![Event::DestinationCleared { unit_id }];
            }
            return Vec::new();
        }

        let unit_id_lua = unit_id_to_lua(unit_id);
        let path_result =
            self.find_unit_path(player, unit_id_lua, unit_type_id, from, destination, true);

        let path = match path_result {
            Some(p) => p,
            None => {
                // No path: clear destination
                let mut world = self.world.borrow_mut();
                if let Some(idx) = world.units.get(unit_id) {
                    world.units.destination[idx] = None;
                }
                return vec![Event::DestinationCleared { unit_id }];
            }
        };

        // Execute path tile-by-tile (same logic as handle_move_unit)
        let mut events = Vec::new();
        let mut current_pos = from;
        let mut remaining_movement = movement;

        for step_idx in 1..path.tiles.len() {
            let step_tile = path.tiles[step_idx];

            let step_cost =
                self.compute_tile_cost(unit_id_lua, unit_type_id, current_pos, step_tile);

            if step_cost == IMPASSABLE {
                break;
            }

            if remaining_movement <= 0 {
                break;
            }

            // ZoC check
            if self.is_zoc_tile(player, current_pos) && self.is_zoc_tile(player, step_tile) {
                {
                    let mut world = self.world.borrow_mut();
                    let idx = world.units.get(unit_id).unwrap();
                    world.units.movement[idx] = 0;
                    world.units.destination[idx] = None;
                }
                events.push(Event::DestinationCleared { unit_id });
                events.push(Event::MoveInterrupted {
                    unit_id,
                    at: current_pos,
                    reason: crate::protocol::MoveInterruptReason::ZoneOfControl,
                    remaining_path: path.tiles[step_idx..].to_vec(),
                });
                break;
            }

            let step_from = current_pos;

            {
                let mut world = self.world.borrow_mut();
                let idx = world.units.get(unit_id).unwrap();
                world.units.position[idx] = step_tile;
                world.units.movement[idx] = (world.units.movement[idx] - step_cost).max(0);
                world.units.has_moved[idx] = true;
                world.units.fortified[idx] = false;
                // Update facing direction
                let (dx, dy) = wrapped_delta(
                    step_from,
                    step_tile,
                    world.tiles.width,
                    world.tiles.height,
                    world.tiles.wrap_x,
                    world.tiles.wrap_y,
                );
                if let Some(dir) = Direction::from_delta(dx, dy) {
                    world.units.direction[idx] = dir;
                }
            }

            remaining_movement = {
                let world = self.world.borrow();
                let idx = world.units.get(unit_id).unwrap();
                world.units.movement[idx]
            };

            current_pos = step_tile;

            // Capture visible tiles before fog update
            let visible_before: std::collections::HashSet<(u32, u32)> = {
                let world = self.world.borrow();
                let w = world.tiles.width;
                let h = world.tiles.height;
                let mut set = std::collections::HashSet::new();
                for y in 0..h {
                    for x in 0..w {
                        if world.tiles.get_visibility(player, x, y)
                            != crate::tile::Visibility::Unseen
                        {
                            set.insert((x, y));
                        }
                    }
                }
                set
            };

            self.fire_hook_simple(
                "on_unit_moved",
                &[
                    ("unit_id", unit_id_lua),
                    ("from_x", step_from.x as i64),
                    ("from_y", step_from.y as i64),
                    ("to_x", step_tile.x as i64),
                    ("to_y", step_tile.y as i64),
                ],
            );

            self.update_visibility(player);

            events.push(Event::UnitMoved {
                unit_id,
                from: step_from,
                to: step_tile,
                movement_left: remaining_movement,
            });

            let newly_revealed: Vec<crate::types::TileCoord> = {
                let world = self.world.borrow();
                let w = world.tiles.width;
                let h = world.tiles.height;
                let mut revealed = Vec::new();
                for y in 0..h {
                    for x in 0..w {
                        if !visible_before.contains(&(x, y))
                            && world.tiles.get_visibility(player, x, y)
                                != crate::tile::Visibility::Unseen
                        {
                            revealed.push(crate::types::TileCoord { x, y });
                        }
                    }
                }
                revealed
            };
            if !newly_revealed.is_empty() {
                events.push(Event::TilesRevealed {
                    player,
                    tiles: newly_revealed.clone(),
                });
            }

            // Enemy spotted interrupt
            if step_idx < path.tiles.len() - 1 {
                let enemy_spotted = {
                    let world = self.world.borrow();
                    newly_revealed.iter().any(|tile| {
                        world.units.iter_alive().any(|(_, uidx)| {
                            world.units.owner[uidx] != player && world.units.position[uidx] == *tile
                        })
                    })
                };
                if enemy_spotted {
                    {
                        let mut world = self.world.borrow_mut();
                        let idx = world.units.get(unit_id).unwrap();
                        world.units.destination[idx] = None;
                    }
                    events.push(Event::DestinationCleared { unit_id });
                    events.push(Event::MoveInterrupted {
                        unit_id,
                        at: current_pos,
                        reason: crate::protocol::MoveInterruptReason::EnemySpotted,
                        remaining_path: path.tiles[(step_idx + 1)..].to_vec(),
                    });
                    break;
                }
            }

            if remaining_movement <= 0 {
                break;
            }
        }

        // If arrived, clear destination
        if current_pos == destination {
            let mut world = self.world.borrow_mut();
            if let Some(idx) = world.units.get(unit_id) {
                world.units.destination[idx] = None;
            }
            events.push(Event::DestinationCleared { unit_id });
        }

        events
    }

    /// Returns true if the tile at `coord` is in the zone of control of an enemy
    /// unit for the given `player`. A tile is ZoC if any adjacent tile contains a
    /// living enemy military unit (category != "civilian").
    pub(super) fn is_zoc_tile(&self, player: PlayerId, coord: crate::types::TileCoord) -> bool {
        let world = self.world.borrow();
        let neighbors = world.tiles.neighbors(coord.x, coord.y);
        for neighbor in neighbors {
            for (_, uidx) in world.units.iter_alive() {
                if world.units.owner[uidx] != player && world.units.position[uidx] == neighbor {
                    // Check if this unit projects ZoC (non-civilian)
                    let ut_id = world.units.unit_type[uidx];
                    if let Some(ut) = world.unit_types.get(ut_id) {
                        if ut.category != "civilian" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}
