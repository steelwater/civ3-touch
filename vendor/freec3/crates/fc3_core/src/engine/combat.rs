use crate::protocol::{CommandResult, Event, GameError};
use crate::types::PlayerId;

use super::Engine;

impl Engine {
    pub(super) fn handle_attack_unit(
        &mut self,
        player: PlayerId,
        attacker_id: crate::id::UnitId,
        defender_id: crate::id::UnitId,
    ) -> CommandResult {
        use crate::scripting::api_unit::unit_id_to_lua;

        // Validate attacker
        let (
            attacker_pos,
            _attacker_movement,
            attacker_owner,
            _attacker_type_name,
            attacker_attack,
        ) = {
            let world = self.world.borrow();
            let idx = match world.units.get(attacker_id) {
                Some(i) => i,
                None => return CommandResult::err(GameError::InvalidUnit),
            };
            if world.units.owner[idx] != player {
                return CommandResult::err(GameError::InvalidUnit);
            }
            if world.units.movement[idx] <= 0 {
                return CommandResult::err(GameError::NotEnoughMovement);
            }
            let type_id = world.units.unit_type[idx];
            let type_name = world
                .unit_types
                .get(type_id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let attack = world.unit_types.get(type_id).map(|t| t.attack).unwrap_or(0);
            (
                world.units.position[idx],
                world.units.movement[idx],
                world.units.owner[idx],
                type_name,
                attack,
            )
        };

        // Validate defender
        let (
            defender_pos,
            defender_owner,
            _defender_type_name,
            defender_defense,
            defender_fortified,
        ) = {
            let world = self.world.borrow();
            let idx = match world.units.get(defender_id) {
                Some(i) => i,
                None => return CommandResult::err(GameError::InvalidUnit),
            };
            let type_id = world.units.unit_type[idx];
            let type_name = world
                .unit_types
                .get(type_id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let defense = world
                .unit_types
                .get(type_id)
                .map(|t| t.defense)
                .unwrap_or(0);
            (
                world.units.position[idx],
                world.units.owner[idx],
                type_name,
                defense,
                world.units.fortified[idx],
            )
        };

        // Cannot attack own unit
        if attacker_owner == defender_owner {
            return CommandResult::err(GameError::InvalidTarget);
        }

        // Defender must be adjacent
        {
            let world = self.world.borrow();
            let neighbors = world.tiles.neighbors(attacker_pos.x, attacker_pos.y);
            if !neighbors.contains(&defender_pos) {
                return CommandResult::err(GameError::InvalidTarget);
            }
        }

        // Read combat stats from world
        let (
            attacker_hp,
            attacker_max_hp,
            defender_hp,
            defender_max_hp,
            defender_terrain_str,
            defender_vegetation_str,
            defender_in_city,
        ) = {
            let world = self.world.borrow();
            let a_idx = world.units.get(attacker_id).unwrap();
            let d_idx = world.units.get(defender_id).unwrap();
            let d_tile = world.tiles.idx(defender_pos.x, defender_pos.y);
            let terrain = world.tiles.terrain[d_tile];
            let vegetation = world.tiles.vegetation[d_tile];
            let terrain_str = super::terrain_to_str(terrain);
            let vegetation_str = super::vegetation_to_str(vegetation);
            let in_city = world
                .cities
                .iter_alive()
                .any(|(_, idx)| world.cities.position[idx] == defender_pos);
            (
                world.units.hp[a_idx],
                world.units.max_hp[a_idx],
                world.units.hp[d_idx],
                world.units.max_hp[d_idx],
                terrain_str.to_string(),
                vegetation_str.to_string(),
                in_city,
            )
        };

        let mut events = Vec::new();
        events.push(Event::CombatStarted {
            attacker: attacker_id,
            defender: defender_id,
            tile: defender_pos,
        });

        // Fire on_pre_combat hook to modify strengths (terrain/fortification bonuses)
        // Then fire on_resolve_combat hook to run combat rounds
        // Then read back results
        let lua_attacker = unit_id_to_lua(attacker_id);
        let lua_defender = unit_id_to_lua(defender_id);

        let combat_code = format!(
            r#"
            local ctx = {{
                attacker_id = {attacker_id},
                defender_id = {defender_id},
                attacker_strength = {attacker_strength},
                defender_strength = {defender_strength},
                attacker_hp = {attacker_hp},
                attacker_max_hp = {attacker_max_hp},
                defender_hp = {defender_hp},
                defender_max_hp = {defender_max_hp},
                defender_terrain = "{defender_terrain}",
                defender_vegetation = "{defender_vegetation}",
                defender_fortified = {defender_fortified},
                defender_in_city = {defender_in_city},
                rounds = {{}}
            }}
            ctx = fire_hook("on_pre_combat", ctx)
            if ctx.__cancelled then
                return {{ cancelled = true, reason = ctx.__cancel_reason or "cancelled" }}
            end
            ctx = fire_hook("on_resolve_combat", ctx)
            ctx = fire_hook("on_post_combat", ctx)
            return ctx
            "#,
            attacker_id = lua_attacker,
            defender_id = lua_defender,
            attacker_strength = attacker_attack,
            defender_strength = defender_defense,
            attacker_hp = attacker_hp,
            attacker_max_hp = attacker_max_hp,
            defender_hp = defender_hp,
            defender_max_hp = defender_max_hp,
            defender_terrain = defender_terrain_str,
            defender_vegetation = defender_vegetation_str,
            defender_fortified = if defender_fortified { "true" } else { "false" },
            defender_in_city = if defender_in_city { "true" } else { "false" },
        );

        let result = self.scripts.lua.load(&combat_code).eval::<mlua::Table>();

        let (final_attacker_hp, final_defender_hp, combat_rounds) = match result {
            Ok(table) => {
                // Check if combat was cancelled
                if table.get::<bool>("cancelled").unwrap_or(false) {
                    let reason: String = table
                        .get::<String>("reason")
                        .unwrap_or_else(|_| "cancelled".to_string());
                    return CommandResult::ok(Event::MoveBlocked {
                        unit_id: attacker_id,
                        reason,
                    });
                }

                let a_hp = table.get::<i64>("attacker_hp").unwrap_or(0) as i32;
                let d_hp = table.get::<i64>("defender_hp").unwrap_or(0) as i32;

                // Extract combat rounds if available
                let rounds: Vec<(i32, i32)> =
                    if let Ok(rounds_table) = table.get::<mlua::Table>("rounds") {
                        let mut r = Vec::new();
                        let len = rounds_table.len().unwrap_or(0);
                        for i in 1..=len {
                            if let Ok(round) = rounds_table.get::<mlua::Table>(i) {
                                let ahp = round.get::<i64>("attacker_hp").unwrap_or(0) as i32;
                                let dhp = round.get::<i64>("defender_hp").unwrap_or(0) as i32;
                                r.push((ahp, dhp));
                            }
                        }
                        r
                    } else {
                        Vec::new()
                    };

                (a_hp, d_hp, rounds)
            }
            Err(_) => {
                // Fallback: no combat hooks, attacker loses
                (0, defender_hp, Vec::new())
            }
        };

        // Emit combat round events
        for (i, (ahp, dhp)) in combat_rounds.iter().enumerate() {
            events.push(Event::CombatRound {
                round: (i + 1) as u32,
                attacker_hp: *ahp,
                defender_hp: *dhp,
            });
        }

        // Determine winner/loser and apply results
        let attacker_won = final_defender_hp <= 0;
        let (winner_id, loser_id, winner_hp) = if attacker_won {
            (attacker_id, defender_id, final_attacker_hp)
        } else {
            (defender_id, attacker_id, final_defender_hp)
        };

        events.push(Event::CombatResolved {
            winner: winner_id,
            loser: loser_id,
            winner_hp,
        });

        // Apply HP changes and clear attacker destination
        {
            let mut world = self.world.borrow_mut();
            if let Some(idx) = world.units.get(attacker_id) {
                world.units.hp[idx] = final_attacker_hp;
                // Attacker movement set to 0 after combat
                world.units.movement[idx] = 0;
                world.units.destination[idx] = None;
            }
            if let Some(idx) = world.units.get(defender_id) {
                world.units.hp[idx] = final_defender_hp;
            }
        }

        // Destroy the loser
        let loser_pos = if attacker_won {
            defender_pos
        } else {
            attacker_pos
        };
        {
            let mut world = self.world.borrow_mut();
            world.units.destroy(loser_id);
        }
        events.push(Event::UnitDestroyed {
            unit_id: loser_id,
            at: loser_pos,
        });

        // If attacker won, move attacker to defender's tile (only if no other enemies remain)
        if attacker_won {
            let can_advance = {
                let world = self.world.borrow();
                let has_enemies = world.units.iter_alive().any(|(_, uidx)| {
                    world.units.position[uidx] == defender_pos
                        && world.units.owner[uidx] != attacker_owner
                });
                !has_enemies
            };

            if can_advance {
                {
                    let mut world = self.world.borrow_mut();
                    if let Some(idx) = world.units.get(attacker_id) {
                        world.units.position[idx] = defender_pos;
                    }
                }

                // Check for city capture: is there a city at defender_pos owned by the defender's owner?
                let capture_info = {
                    let world = self.world.borrow();
                    let mut found = None;
                    for (cid, idx) in world.cities.iter_alive() {
                        if world.cities.position[idx] == defender_pos
                            && world.cities.owner[idx] == defender_owner
                        {
                            // Check if there are any remaining defenders
                            let has_defenders = world.units.iter_alive().any(|(_, uidx)| {
                                world.units.owner[uidx] == defender_owner
                                    && world.units.position[uidx] == defender_pos
                            });
                            if !has_defenders {
                                found = Some((cid, world.cities.population[idx]));
                            }
                            break;
                        }
                    }
                    found
                };

                if let Some((city_id, old_pop)) = capture_info {
                    // Capture the city: transfer ownership, reduce population by 1 (min 1)
                    let new_pop = (old_pop - 1).max(1);
                    {
                        let mut world = self.world.borrow_mut();
                        let idx = world.cities.get(city_id).unwrap();
                        world.cities.owner[idx] = attacker_owner;
                        world.cities.population[idx] = new_pop;
                        world.cities.food_stockpile[idx] = 0;
                        world.cities.shield_stockpile[idx] = 0;
                        world.cities.producing[idx] = None;
                        world.cities.production_cost[idx] = 0;

                        // Update tile ownership
                        let city_pos = world.cities.position[idx];
                        let radius = Self::city_radius_static(&world.tiles, city_pos);
                        for tile_pos in &radius {
                            let tile_idx = world.tiles.idx(tile_pos.x, tile_pos.y);
                            if world.tiles.owner[tile_idx] == Some(defender_owner) {
                                world.tiles.owner[tile_idx] = Some(attacker_owner);
                            }
                        }
                    }

                    events.push(Event::CityCaptured {
                        city_id,
                        old_owner: defender_owner,
                        new_owner: attacker_owner,
                        new_population: new_pop,
                    });

                    // Reassign tiles for the captured city
                    self.reassign_city_tiles(city_id);
                }
            }
        }

        // Update fog of war for both players
        self.update_visibility(attacker_owner);
        self.update_visibility(defender_owner);

        // Check if the loser's owner has been eliminated
        let loser_owner = if attacker_won {
            defender_owner
        } else {
            attacker_owner
        };
        self.check_elimination(loser_owner, &mut events);

        CommandResult::with_events(events)
    }

    /// Checks if a player has been eliminated (0 units AND 0 cities).
    /// If so, marks them as dead and emits events.
    fn check_elimination(&self, player: PlayerId, events: &mut Vec<Event>) {
        let mut world = self.world.borrow_mut();
        let has_units = world
            .units
            .iter_alive()
            .any(|(_, idx)| world.units.owner[idx] == player);
        let has_cities = world
            .cities
            .iter_alive()
            .any(|(_, idx)| world.cities.owner[idx] == player);
        if !has_units && !has_cities {
            if let Some(p) = world.players.get_mut(player.0 as usize) {
                if p.alive {
                    p.alive = false;
                    drop(world);
                    events.push(Event::PlayerEliminated { player });

                    // Check if only one player remains
                    let world = self.world.borrow();
                    let alive: Vec<PlayerId> = world
                        .players
                        .iter()
                        .filter(|p| p.alive)
                        .map(|p| p.id)
                        .collect();
                    if alive.len() == 1 {
                        events.push(Event::GameOver { winner: alive[0] });
                    }
                }
            }
        }
    }
}
