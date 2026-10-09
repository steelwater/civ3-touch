use crate::id::CityId;
use crate::protocol::{CommandResult, Event, GameError};
use crate::types::{PlayerId, TileCoord};

use super::Engine;

impl Engine {
    /// Automatically sets the cheapest available production for a city.
    /// Prefers the cheapest non-Wealth option; falls back to Wealth if nothing else.
    /// Used after city founding (once hooks have run) and after production completion.
    pub(super) fn auto_set_cheapest_production(&self, city_id: CityId) {
        let options = self.buildable_options(city_id);
        let chosen = options
            .iter()
            .filter(|o| !matches!(o.item, crate::city::ProductionItem::Wealth))
            .min_by_key(|o| o.cost)
            .or(options.first());
        if let Some(opt) = chosen {
            let cost = opt.cost;
            let item = opt.item.clone();
            let mut world = self.world.borrow_mut();
            if let Some(idx) = world.cities.get(city_id) {
                world.cities.producing[idx] = Some(item);
                world.cities.production_cost[idx] = cost;
            }
        }
    }

    /// Returns the tiles within a city's workable radius (Chebyshev distance 2, 21-tile "fat cross").
    /// Static method that takes a TileStore reference.
    // TODO: Refine to match actual Civ3 fat cross shape (exclude the 4 outermost corners)
    pub fn city_radius_static(
        tiles: &crate::tile::TileStore,
        center: crate::types::TileCoord,
    ) -> Vec<crate::types::TileCoord> {
        let mut result = Vec::new();
        let width = tiles.width as i64;
        let height = tiles.height as i64;
        let wrap_x = tiles.wrap_x;
        let wrap_y = tiles.wrap_y;

        for dy in -2i64..=2 {
            for dx in -2i64..=2 {
                // Skip the 4 outermost corners to get the 21-tile fat cross
                if dx.abs() == 2 && dy.abs() == 2 {
                    continue;
                }
                let ny = center.y as i64 + dy;
                let actual_y = if wrap_y {
                    ny.rem_euclid(height) as u32
                } else {
                    if ny < 0 || ny >= height {
                        continue;
                    }
                    ny as u32
                };
                let nx = center.x as i64 + dx;
                let actual_x = if wrap_x {
                    nx.rem_euclid(width) as u32
                } else {
                    if nx < 0 || nx >= width {
                        continue;
                    }
                    nx as u32
                };
                result.push(crate::types::TileCoord {
                    x: actual_x,
                    y: actual_y,
                });
            }
        }
        result
    }

    /// Returns only the outermost (border) tiles of the fat cross city radius.
    /// These are the 12 tiles at Chebyshev distance 2 from center, excluding the
    /// 4 corner tiles (where both |dx|==2 and |dy|==2).
    pub(super) fn city_border_tiles_static(
        tiles: &crate::tile::TileStore,
        center: crate::types::TileCoord,
    ) -> Vec<crate::types::TileCoord> {
        let mut result = Vec::new();
        let width = tiles.width as i64;
        let height = tiles.height as i64;
        let wrap_x = tiles.wrap_x;
        let wrap_y = tiles.wrap_y;

        // Border tiles are those at max(|dx|,|dy|) == 2 within the fat cross
        let offsets: [(i64, i64); 12] = [
            (-1, -2),
            (0, -2),
            (1, -2),
            (-2, -1),
            (2, -1),
            (-2, 0),
            (2, 0),
            (-2, 1),
            (2, 1),
            (-1, 2),
            (0, 2),
            (1, 2),
        ];

        for (dx, dy) in offsets {
            let ny = center.y as i64 + dy;
            let actual_y = if wrap_y {
                ny.rem_euclid(height) as u32
            } else {
                if ny < 0 || ny >= height {
                    continue;
                }
                ny as u32
            };
            let nx = center.x as i64 + dx;
            let actual_x = if wrap_x {
                nx.rem_euclid(width) as u32
            } else {
                if nx < 0 || nx >= width {
                    continue;
                }
                nx as u32
            };
            result.push(crate::types::TileCoord {
                x: actual_x,
                y: actual_y,
            });
        }
        result
    }

    /// Calculates the yield (food, shields, commerce) for a tile by firing the Lua hook.
    pub fn calculate_tile_yield(&self, x: u32, y: u32) -> (i32, i32, i32) {
        let (terrain_str, vegetation_str, road_level, improvement) = {
            let world = self.world.borrow();
            let tile_idx = world.tiles.idx(x, y);
            let terrain = super::terrain_to_str(world.tiles.terrain[tile_idx]);
            let vegetation = super::vegetation_to_str(world.tiles.vegetation[tile_idx]);
            (
                terrain.to_string(),
                vegetation.to_string(),
                world.tiles.road_level[tile_idx],
                world.tiles.improvement[tile_idx],
            )
        };

        let improvement_str = match improvement {
            Some(id) => format!("{}", id.0),
            None => "nil".to_string(),
        };

        let code = format!(
            r#"
            local ctx = {{
                x = {x},
                y = {y},
                terrain = "{terrain}",
                vegetation = "{vegetation}",
                road_level = {road_level},
                improvement = {improvement},
                food = 0,
                shields = 0,
                commerce = 0
            }}
            ctx = fire_hook("on_calculate_tile_yield", ctx)
            return ctx
            "#,
            terrain = terrain_str,
            vegetation = vegetation_str,
            road_level = road_level,
            improvement = improvement_str,
        );

        match self.scripts.lua.load(&code).eval::<mlua::Table>() {
            Ok(table) => {
                let food = table.get::<i64>("food").unwrap_or(0) as i32;
                let shields = table.get::<i64>("shields").unwrap_or(0) as i32;
                let commerce = table.get::<i64>("commerce").unwrap_or(0) as i32;
                (food, shields, commerce)
            }
            Err(_) => (0, 0, 0),
        }
    }

    /// Worked-tile yield, including resources revealed to this city's owner.
    pub fn calculate_tile_yield_for_player(
        &self, player: PlayerId, x: u32, y: u32,
    ) -> (i32, i32, i32) {
        let base = self.calculate_tile_yield(x, y);
        let world = self.world.borrow();
        match crate::resource::revealed(&world, player, world.tiles.idx(x, y)) {
            Some(r) => (base.0 + r.food, base.1 + r.shields, base.2 + r.commerce),
            None => base,
        }
    }

    /// Reassigns tiles for a city based on its population.
    /// City of population N works the city center + N best tiles.
    /// The city center always produces at minimum {2, 1, 0} or the terrain yield, whichever is better.
    pub fn reassign_city_tiles(&self, city_id: crate::id::CityId) {
        let (city_pos, city_pop, city_owner) = {
            let world = self.world.borrow();
            let idx = match world.cities.get(city_id) {
                Some(i) => i,
                None => return,
            };
            (
                world.cities.position[idx],
                world.cities.population[idx],
                world.cities.owner[idx],
            )
        };

        // Get the city radius tiles
        let radius_tiles = {
            let world = self.world.borrow();
            Self::city_radius_static(&world.tiles, city_pos)
        };

        // Calculate yield for each tile, excluding tiles worked by other cities
        let mut tile_yields: Vec<(crate::types::TileCoord, i32, i32, i32)> = Vec::new();
        for tile_pos in &radius_tiles {
            if *tile_pos == city_pos {
                continue; // City center handled separately
            }
            let is_available = {
                let world = self.world.borrow();
                let tile_idx = world.tiles.idx(tile_pos.x, tile_pos.y);
                match world.tiles.worked_by[tile_idx] {
                    None => true,
                    Some(other_city) => other_city == city_id,
                }
            };
            if !is_available {
                continue;
            }
            let (food, shields, commerce) =
                self.calculate_tile_yield_for_player(city_owner, tile_pos.x, tile_pos.y);
            tile_yields.push((*tile_pos, food, shields, commerce));
        }

        // Sort by food (desc), then shields (desc), then commerce (desc)
        tile_yields.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(b.3.cmp(&a.3)));

        // Pick the top N tiles (population count)
        let num_tiles = city_pop.max(0) as usize;
        let selected: Vec<crate::types::TileCoord> =
            tile_yields.iter().take(num_tiles).map(|t| t.0).collect();

        // Clear old worked_by for this city, set new ones
        {
            let mut world = self.world.borrow_mut();
            // Clear all tiles previously worked by this city
            let total_tiles = (world.tiles.width * world.tiles.height) as usize;
            for i in 0..total_tiles {
                if world.tiles.worked_by[i] == Some(city_id) {
                    world.tiles.worked_by[i] = None;
                }
            }

            // Mark center as worked by this city
            let center_idx = world.tiles.idx(city_pos.x, city_pos.y);
            world.tiles.worked_by[center_idx] = Some(city_id);

            // Mark selected tiles as worked
            for tile_pos in &selected {
                let tile_idx = world.tiles.idx(tile_pos.x, tile_pos.y);
                world.tiles.worked_by[tile_idx] = Some(city_id);
            }

            // Store worked tiles in city
            let city_idx = world.cities.get(city_id).unwrap();
            let mut all_worked = vec![city_pos];
            all_worked.extend_from_slice(&selected);
            world.cities.worked_tiles[city_idx] = all_worked;
        }

        // Calculate total yields from worked tiles
        let (center_food, center_shields, center_commerce) =
            self.calculate_tile_yield(city_pos.x, city_pos.y);
        // City center minimum: {2, 1, 1} or terrain yield, whichever is better
        let center_food = center_food.max(2);
        let center_shields = center_shields.max(1);
        let center_commerce = center_commerce.max(1);

        let mut total_food = center_food;
        let mut total_shields = center_shields;
        let mut total_commerce = center_commerce;

        for &(_, food, shields, commerce) in tile_yields.iter().take(num_tiles) {
            total_food += food;
            total_shields += shields;
            total_commerce += commerce;
        }

        // Fire on_calculate_city_yields hook for mod bonuses
        let code = format!(
            r#"
            local ctx = {{
                city_id = {city_id},
                food = {food},
                shields = {shields},
                commerce = {commerce}
            }}
            ctx = fire_hook("on_calculate_city_yields", ctx)
            return ctx
            "#,
            city_id = (city_id.generation as i64) << 32 | city_id.index as i64,
            food = total_food,
            shields = total_shields,
            commerce = total_commerce,
        );

        if let Ok(table) = self.scripts.lua.load(&code).eval::<mlua::Table>() {
            total_food = table.get::<i64>("food").unwrap_or(total_food as i64) as i32;
            total_shields = table.get::<i64>("shields").unwrap_or(total_shields as i64) as i32;
            total_commerce = table
                .get::<i64>("commerce")
                .unwrap_or(total_commerce as i64) as i32;
        }

        // Update cached yields
        {
            let mut world = self.world.borrow_mut();
            let city_idx = world.cities.get(city_id).unwrap();
            world.cities.food_per_turn[city_idx] = total_food;
            world.cities.shields_per_turn[city_idx] = total_shields;
            world.cities.commerce_per_turn[city_idx] = total_commerce;
        }
    }

    /// Checks whether a city can produce a given unit type.
    /// Returns None if production is allowed, Some(reason) if blocked.
    /// Calls per-type `can_produce` callback (from UnitType.define), then
    /// fires the global `on_can_produce` hook.
    pub(super) fn check_can_produce(
        &self,
        city_id: crate::id::CityId,
        unit_type_name: &str,
    ) -> Option<String> {
        // Gather context from world
        let (city_id_lua, city_name, cx, cy, population, player_id, is_coastal, cost) = {
            let world = self.world.borrow();
            let idx = match world.cities.get(city_id) {
                Some(i) => i,
                None => return Some("invalid city".to_string()),
            };
            let pos = world.cities.position[idx];
            let owner = world.cities.owner[idx];
            let pop = world.cities.population[idx];
            let name = world.cities.name[idx].clone();
            let cost = world
                .unit_types
                .get_by_name(unit_type_name)
                .map(|ut| ut.cost)
                .unwrap_or(0);

            // Check if city is coastal (any adjacent tile is coast or ocean)
            let neighbors = world.tiles.neighbors(pos.x, pos.y);
            let coastal = neighbors.iter().any(|n| {
                let tidx = world.tiles.idx(n.x, n.y);
                matches!(
                    world.tiles.terrain[tidx],
                    crate::tile::Terrain::Coast | crate::tile::Terrain::Ocean
                )
            });

            let lua_id = (city_id.generation as i64) << 32 | city_id.index as i64;
            (
                lua_id,
                name,
                pos.x,
                pos.y,
                pop,
                owner.0 as i64,
                coastal,
                cost,
            )
        };

        let code = format!(
            r#"
            local ctx = {{
                city_id = {city_id_lua},
                city_name = "{city_name}",
                x = {cx},
                y = {cy},
                population = {population},
                player_id = {player_id},
                is_coastal = {is_coastal},
                unit_type_name = "{unit_type_name}",
                cost = {cost},
                blocked = false,
                reason = ""
            }}
            local handlers = __unit_type_handlers["{unit_type_name}"]
            if handlers and handlers.can_produce then
                ctx = handlers.can_produce(ctx)
                if ctx.blocked then
                    return ctx
                end
            end
            ctx = fire_hook("on_can_produce", ctx)
            return ctx
            "#,
            is_coastal = if is_coastal { "true" } else { "false" },
        );

        match self.scripts.lua.load(&code).eval::<mlua::Table>() {
            Ok(table) => {
                if table.get::<bool>("blocked").unwrap_or(false) {
                    Some(
                        table
                            .get::<String>("reason")
                            .unwrap_or_else(|_| "cannot produce this item".to_string()),
                    )
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Checks whether a city can produce a given building.
    /// Returns None if production is allowed, Some(reason) if blocked.
    /// Checks: building exists, city doesn't already have it, tech requirements met,
    /// then calls optional `can_produce` callback from `__building_handlers`.
    pub(super) fn check_can_produce_building(
        &self,
        city_id: crate::id::CityId,
        building_id: &str,
    ) -> Option<String> {
        let (city_id_lua, city_name, cx, cy, population, player_id, is_coastal, cost) = {
            let world = self.world.borrow();
            let idx = match world.cities.get(city_id) {
                Some(i) => i,
                None => return Some("invalid city".to_string()),
            };

            let def = match world.building_registry.get(building_id) {
                Some(d) => d,
                None => return Some(format!("unknown building: {building_id}")),
            };

            // City already has this building
            if world.cities.buildings[idx].contains(&building_id.to_string()) {
                return Some(format!("city already has {building_id}"));
            }

            // Tech prerequisites
            let pos = world.cities.position[idx];
            let owner = world.cities.owner[idx];
            let player = world.players.get(owner.0 as usize);
            let researched = player.map(|p| &p.researched_techs);
            for req in &def.requires {
                if !researched.is_some_and(|techs| techs.contains(req)) {
                    return Some(format!("requires tech: {req}"));
                }
            }

            let pop = world.cities.population[idx];
            let name = world.cities.name[idx].clone();
            let building_cost = def.cost;

            // Check if city is coastal
            let neighbors = world.tiles.neighbors(pos.x, pos.y);
            let coastal = neighbors.iter().any(|n| {
                let tidx = world.tiles.idx(n.x, n.y);
                matches!(
                    world.tiles.terrain[tidx],
                    crate::tile::Terrain::Coast | crate::tile::Terrain::Ocean
                )
            });

            let lua_id = (city_id.generation as i64) << 32 | city_id.index as i64;
            (
                lua_id,
                name,
                pos.x,
                pos.y,
                pop,
                owner.0 as i64,
                coastal,
                building_cost,
            )
        };

        // Call optional can_produce callback from __building_handlers
        let code = format!(
            r#"
            local ctx = {{
                city_id = {city_id_lua},
                city_name = "{city_name}",
                x = {cx},
                y = {cy},
                population = {population},
                player_id = {player_id},
                is_coastal = {is_coastal},
                building_id = "{building_id}",
                cost = {cost},
                blocked = false,
                reason = ""
            }}
            local handlers = __building_handlers["{building_id}"]
            if handlers and handlers.can_produce then
                ctx = handlers.can_produce(ctx)
            end
            return ctx
            "#,
            is_coastal = if is_coastal { "true" } else { "false" },
        );

        match self.scripts.lua.load(&code).eval::<mlua::Table>() {
            Ok(table) => {
                if table.get::<bool>("blocked").unwrap_or(false) {
                    Some(
                        table
                            .get::<String>("reason")
                            .unwrap_or_else(|_| "cannot produce this building".to_string()),
                    )
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Returns the list of buildable production options for a city.
    /// Calls `check_can_produce` for each unit type and always appends wealth.
    pub(super) fn buildable_options(
        &self,
        city_id: crate::id::CityId,
    ) -> Vec<crate::protocol::ProductionOption> {
        let unit_types_info: Vec<(String, crate::types::UnitTypeId, i32)> = {
            let world = self.world.borrow();
            world
                .unit_types
                .iter()
                .map(|ut| (ut.name.clone(), ut.id, ut.cost))
                .collect()
        };

        let mut options = Vec::new();
        for (name, type_id, cost) in &unit_types_info {
            if self.check_can_produce(city_id, name).is_none() {
                options.push(crate::protocol::ProductionOption {
                    name: name.clone(),
                    item: crate::city::ProductionItem::Unit {
                        unit_type_id: *type_id,
                    },
                    cost: *cost,
                });
            }
        }

        // Buildings
        let buildings_info: Vec<(String, String, i32)> = {
            let world = self.world.borrow();
            world
                .building_registry
                .all()
                .iter()
                .map(|b| (b.id.clone(), b.name.clone(), b.cost))
                .collect()
        };

        for (bid, bname, bcost) in &buildings_info {
            if self.check_can_produce_building(city_id, bid).is_none() {
                options.push(crate::protocol::ProductionOption {
                    name: bname.clone(),
                    item: crate::city::ProductionItem::Building {
                        building_id: bid.clone(),
                    },
                    cost: *bcost,
                });
            }
        }

        // Wealth is always available
        options.push(crate::protocol::ProductionOption {
            name: "wealth".to_string(),
            item: crate::city::ProductionItem::Wealth,
            cost: 0,
        });

        options
    }

    /// Sets the production item for a city owned by the player.
    pub(super) fn handle_set_production(
        &mut self,
        player: PlayerId,
        city_id: crate::id::CityId,
        item: crate::city::ProductionItem,
    ) -> CommandResult {
        // Validate city exists and player owns it
        let current_producing = {
            let world = self.world.borrow();
            let idx = match world.cities.get(city_id) {
                Some(i) => i,
                None => return CommandResult::err(GameError::InvalidUnit),
            };
            if world.cities.owner[idx] != player {
                return CommandResult::err(GameError::InvalidUnit);
            }
            world.cities.producing[idx].clone()
        };

        // Resolve the item to get its name and cost
        let (item_name, cost) = match &item {
            crate::city::ProductionItem::Unit { unit_type_id } => {
                let world = self.world.borrow();
                match world.unit_types.get(*unit_type_id) {
                    Some(ut) => (ut.name.clone(), ut.cost),
                    None => {
                        return CommandResult::err(GameError::Custom(
                            "unknown unit type".to_string(),
                        ))
                    }
                }
            }
            crate::city::ProductionItem::Building { building_id } => {
                let world = self.world.borrow();
                match world.building_registry.get(building_id) {
                    Some(b) => (b.name.clone(), b.cost),
                    None => {
                        return CommandResult::err(GameError::Custom(
                            "unknown building".to_string(),
                        ))
                    }
                }
            }
            crate::city::ProductionItem::Wealth => ("wealth".to_string(), 0),
        };

        // Validate via can_produce callbacks and on_can_produce hook
        match &item {
            crate::city::ProductionItem::Unit { .. } => {
                if let Some(reason) = self.check_can_produce(city_id, &item_name) {
                    return CommandResult::err(GameError::Custom(reason));
                }
            }
            crate::city::ProductionItem::Building { building_id } => {
                if let Some(reason) = self.check_can_produce_building(city_id, building_id) {
                    return CommandResult::err(GameError::Custom(reason));
                }
            }
            crate::city::ProductionItem::Wealth => {}
        }

        // If changing production item, reset shield stockpile
        let reset_shields = current_producing.as_ref() != Some(&item);

        {
            let mut world = self.world.borrow_mut();
            let idx = world.cities.get(city_id).unwrap();
            world.cities.producing[idx] = Some(item);
            world.cities.production_cost[idx] = cost;
            if reset_shields {
                world.cities.shield_stockpile[idx] = 0;
            }
        }

        CommandResult::ok(Event::ProductionSet {
            city_id,
            item_name,
            cost,
        })
    }

    /// Processes city food and growth for all cities belonging to the given player.
    /// Called at the start of a player's turn. Returns events for growth/starvation.
    pub(super) fn process_city_food(&self, player: PlayerId) -> Vec<Event> {
        let mut events = Vec::new();

        // Collect city data for cities owned by this player
        let cities: Vec<(crate::id::CityId, usize)> = {
            let world = self.world.borrow();
            world
                .cities
                .iter_alive()
                .filter(|&(_, idx)| world.cities.owner[idx] == player)
                .collect()
        };

        for (city_id, _) in cities {
            let (food_per_turn, food_stockpile, population) = {
                let world = self.world.borrow();
                let idx = match world.cities.get(city_id) {
                    Some(i) => i,
                    None => continue,
                };
                (
                    world.cities.food_per_turn[idx],
                    world.cities.food_stockpile[idx],
                    world.cities.population[idx],
                )
            };

            // Fire on_city_process_food hook
            let code = format!(
                r#"
                local ctx = {{
                    city_id = {city_id_lua},
                    food_per_turn = {food_per_turn},
                    food_stockpile = {food_stockpile},
                    population = {population},
                    net_food = 0,
                    grew = false,
                    starved = false
                }}
                ctx = fire_hook("on_city_process_food", ctx)
                return ctx
                "#,
                city_id_lua = (city_id.generation as i64) << 32 | city_id.index as i64,
            );

            if let Ok(table) = self.scripts.lua.load(&code).eval::<mlua::Table>() {
                let new_pop = table.get::<i64>("population").unwrap_or(population as i64) as i32;
                let new_food_stockpile = table
                    .get::<i64>("food_stockpile")
                    .unwrap_or(food_stockpile as i64)
                    as i32;
                let grew = table.get::<bool>("grew").unwrap_or(false);
                let starved = table.get::<bool>("starved").unwrap_or(false);

                // Update city state
                {
                    let mut world = self.world.borrow_mut();
                    let idx = match world.cities.get(city_id) {
                        Some(i) => i,
                        None => continue,
                    };
                    world.cities.population[idx] = new_pop;
                    world.cities.food_stockpile[idx] = new_food_stockpile;
                }

                if grew {
                    events.push(Event::CityGrew {
                        city_id,
                        new_population: new_pop,
                    });
                    // Reassign tiles when population changes
                    self.reassign_city_tiles(city_id);
                }

                if starved {
                    events.push(Event::CityStarved {
                        city_id,
                        new_population: new_pop,
                    });
                    // Reassign tiles when population changes
                    self.reassign_city_tiles(city_id);
                }
            }
        }

        events
    }

    /// Processes city production for all cities belonging to the given player.
    /// Called at the start of a player's turn, after food processing.
    /// Returns events for production completion and unit spawning.
    pub(super) fn process_city_production(&mut self, player: PlayerId) -> Vec<Event> {
        let mut events = Vec::new();

        let cities: Vec<(crate::id::CityId, usize)> = {
            let world = self.world.borrow();
            world
                .cities
                .iter_alive()
                .filter(|&(_, idx)| world.cities.owner[idx] == player)
                .collect()
        };

        for (city_id, _) in cities {
            let (shields_per_turn, shield_stockpile, production_cost, producing, city_pos) = {
                let world = self.world.borrow();
                let idx = match world.cities.get(city_id) {
                    Some(i) => i,
                    None => continue,
                };
                (
                    world.cities.shields_per_turn[idx],
                    world.cities.shield_stockpile[idx],
                    world.cities.production_cost[idx],
                    world.cities.producing[idx].clone(),
                    world.cities.position[idx],
                )
            };

            if producing.is_none() {
                continue;
            }

            // Wealth: convert shields to gold, skip normal production pipeline
            if matches!(producing, Some(crate::city::ProductionItem::Wealth)) {
                let mut world = self.world.borrow_mut();
                if let Some(p) = world.players.get_mut(player.0 as usize) {
                    p.gold += shields_per_turn;
                }
                continue;
            }

            // Fire on_city_process_production hook
            let producing_name = match &producing {
                Some(crate::city::ProductionItem::Unit { unit_type_id }) => {
                    let world = self.world.borrow();
                    world
                        .unit_types
                        .get(*unit_type_id)
                        .map(|ut| ut.name.clone())
                        .unwrap_or_default()
                }
                Some(crate::city::ProductionItem::Building { building_id }) => {
                    let world = self.world.borrow();
                    world
                        .building_registry
                        .get(building_id)
                        .map(|b| b.name.clone())
                        .unwrap_or_default()
                }
                _ => continue,
            };

            let code = format!(
                r#"
                local ctx = {{
                    city_id = {city_id_lua},
                    producing = "{producing_name}",
                    shields_per_turn = {shields_per_turn},
                    shield_stockpile = {shield_stockpile},
                    production_cost = {production_cost},
                    production_complete = false,
                    overflow = 0
                }}
                ctx = fire_hook("on_city_process_production", ctx)
                return ctx
                "#,
                city_id_lua = (city_id.generation as i64) << 32 | city_id.index as i64,
            );

            if let Ok(table) = self.scripts.lua.load(&code).eval::<mlua::Table>() {
                let new_shield_stockpile = table
                    .get::<i64>("shield_stockpile")
                    .unwrap_or(shield_stockpile as i64)
                    as i32;
                let complete = table.get::<bool>("production_complete").unwrap_or(false);
                let overflow = table.get::<i64>("overflow").unwrap_or(0) as i32;

                if complete {
                    events.push(Event::ProductionComplete {
                        city_id,
                        item_name: producing_name.clone(),
                    });

                    // Handle completed production by type
                    let mut is_settler_type = false;
                    match &producing {
                        Some(crate::city::ProductionItem::Unit { unit_type_id }) => {
                            let (unit_id, settler) = {
                                let mut world = self.world.borrow_mut();
                                if let Some(template) = world.unit_types.get(*unit_type_id).cloned()
                                {
                                    let is_settler =
                                        template.actions.contains(&"build_city".to_string());
                                    let uid = world.units.spawn(
                                        *unit_type_id,
                                        player,
                                        city_pos,
                                        &template,
                                    );
                                    (Some(uid), is_settler)
                                } else {
                                    (None, false)
                                }
                            };
                            is_settler_type = settler;
                            if let Some(uid) = unit_id {
                                events.push(Event::UnitProduced {
                                    city_id,
                                    unit_id: uid,
                                    unit_type: producing_name.clone(),
                                    at: city_pos,
                                });
                            }
                        }
                        Some(crate::city::ProductionItem::Building { building_id }) => {
                            let bid = building_id.clone();
                            // Add building to city
                            {
                                let mut world = self.world.borrow_mut();
                                let idx = match world.cities.get(city_id) {
                                    Some(i) => i,
                                    None => continue,
                                };
                                if !world.cities.buildings[idx].contains(&bid) {
                                    world.cities.buildings[idx].push(bid.clone());
                                }
                            }

                            // Fire on_complete callback
                            let city_id_lua =
                                (city_id.generation as i64) << 32 | city_id.index as i64;
                            let on_complete_code = format!(
                                r#"
                                local handlers = __building_handlers["{bid}"]
                                if handlers and handlers.on_complete then
                                    handlers.on_complete({{ city_id = {city_id_lua}, building_id = "{bid}", player_id = {player_id} }})
                                end
                                "#,
                                player_id = player.0 as i64,
                            );
                            let _ = self.scripts.lua.load(&on_complete_code).exec();

                            events.push(Event::BuildingCompleted {
                                city_id,
                                building_id: bid,
                            });
                        }
                        _ => {}
                    }

                    // Reset production and set overflow shields.
                    // Also reduce population by 2 (min 1) if a settler was produced.
                    {
                        let mut world = self.world.borrow_mut();
                        let idx = match world.cities.get(city_id) {
                            Some(i) => i,
                            None => continue,
                        };
                        world.cities.shield_stockpile[idx] = overflow;

                        if is_settler_type {
                            world.cities.population[idx] =
                                (world.cities.population[idx] - 2).max(1);
                        }
                    }

                    // Reassign worked tiles if settler cost reduced population
                    if is_settler_type {
                        self.reassign_city_tiles(city_id);
                    }

                    // Update visibility for new unit (only needed for unit production)
                    if matches!(producing, Some(crate::city::ProductionItem::Unit { .. })) {
                        self.update_visibility(player);
                    }

                    // Auto-set cheapest production for the city
                    self.auto_set_cheapest_production(city_id);
                } else {
                    // Just update shield stockpile
                    let mut world = self.world.borrow_mut();
                    let idx = match world.cities.get(city_id) {
                        Some(i) => i,
                        None => continue,
                    };
                    world.cities.shield_stockpile[idx] = new_shield_stockpile;
                }
            }
        }

        events
    }

    /// Returns per-tile yields for a city's worked tiles.
    /// Returns (coord, food, shields, commerce) for each worked tile,
    /// with city center minimum applied.
    /// Returns None if the city doesn't exist or isn't owned by the player.
    pub fn query_city_tile_yields(
        &self,
        player: PlayerId,
        city_id: CityId,
    ) -> Option<Vec<(TileCoord, i32, i32, i32)>> {
        let (city_pos, worked_tiles) = {
            let world = self.world.borrow();
            let idx = world.cities.get(city_id)?;
            if world.cities.owner[idx] != player {
                return None;
            }
            (
                world.cities.position[idx],
                world.cities.worked_tiles[idx].clone(),
            )
        };

        let mut result = Vec::with_capacity(worked_tiles.len());
        for tile_pos in &worked_tiles {
            if *tile_pos == city_pos {
                let (food, shields, commerce) = self.calculate_tile_yield(tile_pos.x, tile_pos.y);
                // City center minimum: {2, 1, 1}
                result.push((*tile_pos, food.max(2), shields.max(1), commerce.max(1)));
            } else {
                let (food, shields, commerce) =
                    self.calculate_tile_yield_for_player(player, tile_pos.x, tile_pos.y);
                result.push((*tile_pos, food, shields, commerce));
            }
        }
        Some(result)
    }

    /// Returns all tiles in a city's workable radius.
    /// Returns None if the city doesn't exist.
    pub fn query_city_radius(&self, city_id: CityId) -> Option<Vec<TileCoord>> {
        let world = self.world.borrow();
        let idx = world.cities.get(city_id)?;
        let center = world.cities.position[idx];
        Some(Self::city_radius_static(&world.tiles, center))
    }
}
