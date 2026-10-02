use crate::id::UnitId;
use crate::protocol::{CommandResult, Event, GameError};
use crate::types::PlayerId;

use super::Engine;

impl Engine {
    pub(super) fn handle_perform_action(
        &mut self,
        player: PlayerId,
        unit_id: UnitId,
        action_id: String,
    ) -> CommandResult {
        use crate::scripting::api_unit::unit_id_to_lua;

        // Validate unit exists, alive, owned by player
        let (pos, unit_type_id) = {
            let world = self.world.borrow();
            let idx = match world.units.get(unit_id) {
                Some(i) => i,
                None => return CommandResult::err(GameError::InvalidUnit),
            };
            if world.units.owner[idx] != player {
                return CommandResult::err(GameError::InvalidUnit);
            }
            let movement = world.units.movement[idx];
            if movement <= 0 {
                return CommandResult::err(GameError::NotEnoughMovement);
            }
            if world.units.skipped[idx] || world.units.fortified[idx] {
                return CommandResult::err(GameError::Custom(
                    "unit is skipped or fortified".to_string(),
                ));
            }
            if world.units.current_action[idx].is_some() {
                return CommandResult::err(GameError::Custom(
                    "unit already has an active action".to_string(),
                ));
            }
            (world.units.position[idx], world.units.unit_type[idx])
        };

        // Check action_id is in unit type's actions list
        {
            let world = self.world.borrow();
            let has_action = world
                .unit_types
                .get(unit_type_id)
                .is_some_and(|ut| ut.actions.contains(&action_id));
            if !has_action {
                return CommandResult::err(GameError::Custom(format!(
                    "unit type does not have action '{action_id}'"
                )));
            }
        }

        // Check action exists in ActionRegistry and get its properties
        let (turns, consumes_unit) = {
            let world = self.world.borrow();
            match world.action_registry.get(&action_id) {
                Some(def) => (def.turns, def.consumes_unit),
                None => {
                    return CommandResult::err(GameError::Custom(format!(
                        "action '{action_id}' not defined"
                    )))
                }
            }
        };

        // Get terrain string and map info for context
        let (terrain_str, vegetation_str, map_width, wrap_x) = {
            let world = self.world.borrow();
            let tile_idx = world.tiles.idx(pos.x, pos.y);
            let terrain = super::terrain_to_str(world.tiles.terrain[tile_idx]);
            let vegetation = super::vegetation_to_str(world.tiles.vegetation[tile_idx]);
            (
                terrain.to_string(),
                vegetation.to_string(),
                world.tiles.width,
                world.tiles.wrap_x,
            )
        };

        // Build context and call valid_conditions
        let unit_id_lua = unit_id_to_lua(unit_id);
        let valid_code = format!(
            r#"
            local handlers = __action_handlers["{action_id}"]
            if not handlers or not handlers.valid_conditions then
                return {{ blocked = false }}
            end
            local ctx = {{
                unit_id = {unit_id_lua},
                player_id = {player_id},
                x = {x},
                y = {y},
                terrain = "{terrain}",
                vegetation = "{vegetation}",
                action_id = "{action_id}",
                map_width = {map_width},
                wrap_x = {wrap_x},
                blocked = false,
                reason = ""
            }}
            return handlers.valid_conditions(ctx)
            "#,
            player_id = player.0 as i64,
            x = pos.x,
            y = pos.y,
            terrain = terrain_str,
            vegetation = vegetation_str,
            wrap_x = if wrap_x { "true" } else { "false" },
        );

        if let Ok(table) = self.scripts.lua.load(&valid_code).eval::<mlua::Table>() {
            if table.get::<bool>("blocked").unwrap_or(false) {
                let reason = table
                    .get::<String>("reason")
                    .unwrap_or_else(|_| "action blocked".to_string());
                return CommandResult::err(GameError::Custom(reason));
            }
        }

        // Clear destination when performing any action
        {
            let mut world = self.world.borrow_mut();
            if let Some(idx) = world.units.get(unit_id) {
                world.units.destination[idx] = None;
            }
        }

        if turns == 0 {
            // Instant action: call on_complete
            let complete_code = format!(
                r#"
                local handlers = __action_handlers["{action_id}"]
                if handlers and handlers.on_complete then
                    local ctx = {{
                        unit_id = {unit_id_lua},
                        player_id = {player_id},
                        x = {x},
                        y = {y},
                        terrain = "{terrain}",
                        vegetation = "{vegetation}",
                        action_id = "{action_id}",
                        map_width = {map_width},
                        wrap_x = {wrap_x}
                    }}
                    handlers.on_complete(ctx)
                end
                "#,
                player_id = player.0 as i64,
                x = pos.x,
                y = pos.y,
                terrain = terrain_str,
                vegetation = vegetation_str,
                wrap_x = if wrap_x { "true" } else { "false" },
            );
            let _ = self.scripts.lua.load(&complete_code).exec();

            // Post-process
            self.post_process_action(player, unit_id, &action_id, pos, consumes_unit)
        } else {
            // Multi-turn action: set tracking state
            {
                let mut world = self.world.borrow_mut();
                let idx = match world.units.get(unit_id) {
                    Some(i) => i,
                    None => return CommandResult::err(GameError::InvalidUnit),
                };
                world.units.current_action[idx] = Some(action_id.clone());
                world.units.action_turns_left[idx] = turns;
                world.units.movement[idx] = 0;
            }

            CommandResult::ok(Event::ActionStarted {
                unit_id,
                action_id,
                turns_remaining: turns,
                at: pos,
            })
        }
    }

    /// Post-processes an action after on_complete has run.
    /// Drains pending events/cities/consumed units and handles consumes_unit flag.
    pub(super) fn post_process_action(
        &mut self,
        player: PlayerId,
        unit_id: UnitId,
        action_id: &str,
        pos: crate::types::TileCoord,
        consumes_unit: bool,
    ) -> CommandResult {
        let mut events = Vec::new();

        // Drain pending events from Lua
        {
            let mut world = self.world.borrow_mut();
            events.append(&mut world.pending_events);
        }

        // If consumes_unit and Lua didn't already consume it
        if consumes_unit {
            let mut world = self.world.borrow_mut();
            if world.units.is_alive(unit_id) {
                world.units.destroy(unit_id);
                events.push(Event::UnitConsumed {
                    unit_id,
                    reason: format!("performed action '{action_id}'"),
                });
            }
        }

        // Process pending city IDs: reassign tiles and fire hooks
        let pending_cities: Vec<crate::id::CityId> = {
            let mut world = self.world.borrow_mut();
            std::mem::take(&mut world.pending_city_ids)
        };
        for city_id in &pending_cities {
            let (city_id_lua, city_pos) = {
                let world = self.world.borrow();
                if let Some(idx) = world.cities.get(*city_id) {
                    let pos = world.cities.position[idx];
                    let lua_id = (city_id.generation as i64) << 32 | city_id.index as i64;
                    (lua_id, pos)
                } else {
                    continue;
                }
            };
            self.fire_hook_simple(
                "on_city_founded",
                &[
                    ("city_id", city_id_lua),
                    ("player_id", player.0 as i64),
                    ("x", city_pos.x as i64),
                    ("y", city_pos.y as i64),
                ],
            );
            self.reassign_city_tiles(*city_id);
            // Auto-set cheapest production (after hooks, which may add Palace)
            self.auto_set_cheapest_production(*city_id);
        }

        // Update visibility
        self.update_visibility(player);

        // Clear consumed units list (already handled)
        {
            let mut world = self.world.borrow_mut();
            world.pending_consumed_units.clear();
        }

        // Emit ActionCompleted
        events.push(Event::ActionCompleted {
            unit_id,
            action_id: action_id.to_string(),
            at: pos,
        });

        CommandResult::with_events(events)
    }

    /// Processes multi-turn actions at turn start for a player's units.
    /// Called during handle_end_turn when processing the new active player.
    /// Returns events for any completed actions.
    pub(super) fn process_unit_actions(&mut self, player: PlayerId) -> Vec<Event> {
        use crate::scripting::api_unit::unit_id_to_lua;

        let mut events = Vec::new();

        // Collect units with active actions
        let units_with_actions: Vec<(UnitId, usize)> = {
            let world = self.world.borrow();
            world
                .units
                .iter_alive()
                .filter(|&(_, idx)| {
                    world.units.owner[idx] == player && world.units.current_action[idx].is_some()
                })
                .collect()
        };

        for (uid, _) in units_with_actions {
            // Decrement turns left
            let (action_id, turns_left, pos, consumes_unit) = {
                let mut world = self.world.borrow_mut();
                let idx = match world.units.get(uid) {
                    Some(i) => i,
                    None => continue,
                };
                world.units.action_turns_left[idx] -= 1;
                let turns_left = world.units.action_turns_left[idx];
                let action_id = world.units.current_action[idx].clone().unwrap_or_default();
                let pos = world.units.position[idx];

                let consumes = world
                    .action_registry
                    .get(&action_id)
                    .map(|d| d.consumes_unit)
                    .unwrap_or(false);

                // Clear action state
                if turns_left <= 0 {
                    world.units.current_action[idx] = None;
                    world.units.action_turns_left[idx] = 0;
                }

                (action_id, turns_left, pos, consumes)
            };

            if turns_left <= 0 {
                // Action complete: call on_complete
                let (terrain_str, vegetation_str, map_width, wrap_x) = {
                    let world = self.world.borrow();
                    let tile_idx = world.tiles.idx(pos.x, pos.y);
                    let terrain = super::terrain_to_str(world.tiles.terrain[tile_idx]);
                    let vegetation = super::vegetation_to_str(world.tiles.vegetation[tile_idx]);
                    (
                        terrain.to_string(),
                        vegetation.to_string(),
                        world.tiles.width,
                        world.tiles.wrap_x,
                    )
                };

                let uid_lua = unit_id_to_lua(uid);
                let complete_code = format!(
                    r#"
                    local handlers = __action_handlers["{action_id}"]
                    if handlers and handlers.on_complete then
                        local ctx = {{
                            unit_id = {uid_lua},
                            player_id = {player_id},
                            x = {x},
                            y = {y},
                            terrain = "{terrain}",
                            vegetation = "{vegetation}",
                            action_id = "{action_id}",
                            map_width = {map_width},
                            wrap_x = {wrap_x}
                        }}
                        handlers.on_complete(ctx)
                    end
                    "#,
                    player_id = player.0 as i64,
                    x = pos.x,
                    y = pos.y,
                    terrain = terrain_str,
                    vegetation = vegetation_str,
                    wrap_x = if wrap_x { "true" } else { "false" },
                );
                let _ = self.scripts.lua.load(&complete_code).exec();

                let result = self.post_process_action(player, uid, &action_id, pos, consumes_unit);
                events.extend(result.events);
            }
        }

        events
    }

    /// Checks action valid_conditions for a specific unit and action.
    /// Returns None if the action is valid, Some(reason) if blocked.
    pub(super) fn check_action_valid(&self, unit_id: UnitId, action_id: &str) -> Option<String> {
        use crate::scripting::api_unit::unit_id_to_lua;

        let (pos, player, map_width, wrap_x, terrain_str, vegetation_str) = {
            let world = self.world.borrow();
            let idx = match world.units.get(unit_id) {
                Some(i) => i,
                None => return Some("invalid unit".to_string()),
            };
            let pos = world.units.position[idx];
            let player = world.units.owner[idx];
            let tile_idx = world.tiles.idx(pos.x, pos.y);
            let terrain = super::terrain_to_str(world.tiles.terrain[tile_idx]);
            let vegetation = super::vegetation_to_str(world.tiles.vegetation[tile_idx]);
            (
                pos,
                player,
                world.tiles.width,
                world.tiles.wrap_x,
                terrain.to_string(),
                vegetation.to_string(),
            )
        };

        let uid_lua = unit_id_to_lua(unit_id);
        let code = format!(
            r#"
            local handlers = __action_handlers["{action_id}"]
            if not handlers or not handlers.valid_conditions then
                return {{ blocked = false }}
            end
            local ctx = {{
                unit_id = {uid_lua},
                player_id = {player_id},
                x = {x},
                y = {y},
                terrain = "{terrain}",
                vegetation = "{vegetation}",
                action_id = "{action_id}",
                map_width = {map_width},
                wrap_x = {wrap_x},
                blocked = false,
                reason = ""
            }}
            return handlers.valid_conditions(ctx)
            "#,
            player_id = player.0 as i64,
            x = pos.x,
            y = pos.y,
            terrain = terrain_str,
            vegetation = vegetation_str,
            wrap_x = if wrap_x { "true" } else { "false" },
        );

        match self.scripts.lua.load(&code).eval::<mlua::Table>() {
            Ok(table) => {
                if table.get::<bool>("blocked").unwrap_or(false) {
                    Some(
                        table
                            .get::<String>("reason")
                            .unwrap_or_else(|_| "action blocked".to_string()),
                    )
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}
