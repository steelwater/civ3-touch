use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::action::ActionRegistry;
use crate::building::BuildingRegistry;
use crate::city::CityStore;
use crate::civilization::CivRegistry;
use crate::id::{CityId, UnitId};
use crate::protocol::{CitySnapshot, Event, FringeTile, PlayerView, TileSnapshot, UnitSnapshot};
use crate::tech::TechRegistry;
use crate::tile::{Terrain, TileStore, Visibility};
use crate::types::PlayerId;
use crate::unit::UnitStore;
use crate::unit_type::UnitTypeRegistry;

/// Configuration for creating a new game world.
///
/// **Dimension constraint**: When both `wrap_x` and `wrap_y` are true (torus topology),
/// `width` must be a multiple of `height`. The isometric display maps tiles to vertical
/// bands based on their diagonal `(x + y) % height`. If `width % height != 0`, tiles
/// that are logically adjacent across the x-boundary land on different diagonal bands,
/// creating a visible vertical gap at the seam that cannot be fixed in the renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    pub width: u32,
    pub height: u32,
    pub wrap_x: bool,
    #[serde(default)]
    pub wrap_y: bool,
    pub num_players: u8,
    pub seed: u64,
}

/// A player in the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub alive: bool,
    pub gold: i32,
    pub science: i32,
    pub culture: i32,
    pub researching: Option<String>,
    pub researched_techs: Vec<String>,
    pub civilization: Option<String>,
}

/// All game state in one struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    /// Omitted in legacy saves so their exact replay verification remains valid.
    #[serde(default, skip_serializing_if = "crate::resource::disabled")]
    pub resources_enabled: bool,
    pub tiles: TileStore,
    pub units: UnitStore,
    pub cities: CityStore,
    pub unit_types: UnitTypeRegistry,
    pub action_registry: ActionRegistry,
    pub tech_registry: TechRegistry,
    pub building_registry: BuildingRegistry,
    pub civilization_registry: CivRegistry,
    pub players: Vec<Player>,
    pub turn: u32,
    pub current_player: PlayerId,
    #[serde(skip, default = "default_rng")]
    pub rng: ChaCha8Rng,
    /// The seed used to create this world's RNG, stored for serialization.
    pub rng_seed: u64,
    /// Number of RNG calls made, for deterministic reconstruction.
    pub rng_calls: u64,
    /// Pending events queued by Lua API functions during action on_complete.
    #[serde(skip, default)]
    pub pending_events: Vec<Event>,
    /// Pending city IDs that need post-processing (reassign tiles, hooks).
    #[serde(skip, default)]
    pub pending_city_ids: Vec<CityId>,
    /// Pending unit IDs that were consumed by Lua during action on_complete.
    #[serde(skip, default)]
    pub pending_consumed_units: Vec<UnitId>,
}

fn default_rng() -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(0)
}

impl World {
    /// Reconstructs the RNG from the stored seed and replays `rng_calls`
    /// draws to restore the exact RNG state. Must be called after
    /// deserialization.
    pub fn restore_rng(&mut self) {
        use rand::RngCore;
        self.rng = ChaCha8Rng::seed_from_u64(self.rng_seed);
        for _ in 0..self.rng_calls {
            self.rng.next_u64();
        }
    }

    /// Builds the view of the world visible to the given player.
    /// Only tiles with `Revealed` or `Visible` status are included.
    /// Only units on `Visible` tiles are included.
    pub fn build_player_view(&self, player: PlayerId) -> PlayerView {
        let mut visible_tiles = Vec::new();
        let mut known_units = Vec::new();

        // Collect visible/revealed tiles
        let total = (self.tiles.width * self.tiles.height) as usize;
        for i in 0..total {
            let coord = self.tiles.coords(i);
            let vis = self.tiles.get_visibility(player, coord.x, coord.y);
            if vis == Visibility::Unseen {
                continue;
            }
            visible_tiles.push(TileSnapshot {
                resource: crate::resource::revealed(self, player, i).map(|r| r.name.to_string()),
                coord,
                terrain: self.tiles.terrain[i],
                vegetation: self.tiles.vegetation[i],
                improvement: self.tiles.improvement[i],
                road_level: self.tiles.road_level[i],
                owner: self.tiles.owner[i],
                visibility: vis,
            });
        }

        // Collect fringe tiles: unseen neighbors of discovered tiles (terrain only)
        let mut fringe_tiles = Vec::new();
        {
            let mut fringe_seen = std::collections::HashSet::new();
            for snap in &visible_tiles {
                for nbr in self.tiles.neighbors(snap.coord.x, snap.coord.y) {
                    let vis = self.tiles.get_visibility(player, nbr.x, nbr.y);
                    if vis == Visibility::Unseen && fringe_seen.insert((nbr.x, nbr.y)) {
                        let idx = self.tiles.idx(nbr.x, nbr.y);
                        fringe_tiles.push(FringeTile {
                            coord: nbr,
                            terrain: self.tiles.terrain[idx],
                            vegetation: self.tiles.vegetation[idx],
                        });
                    }
                }
            }
        }

        // Collect units on Visible tiles
        for (_uid, idx) in self.units.iter_alive() {
            let pos = self.units.position[idx];
            let vis = self.tiles.get_visibility(player, pos.x, pos.y);
            if vis != Visibility::Visible {
                continue;
            }
            let type_id = self.units.unit_type[idx];
            let ut_ref = self.unit_types.get(type_id);
            let type_name = ut_ref.map(|ut| ut.name.clone()).unwrap_or_default();
            let category = ut_ref.map(|ut| ut.category.clone()).unwrap_or_default();
            let art_ini = ut_ref.and_then(|ut| ut.art_ini.clone());
            // Only expose destination for own units (fog of war)
            let destination = if self.units.owner[idx] == player {
                self.units.destination[idx]
            } else {
                None
            };
            let current_action_animation = self.units.current_action[idx]
                .as_ref()
                .and_then(|aid| self.action_registry.get(aid))
                .and_then(|def| def.animation_name.clone());
            known_units.push(UnitSnapshot {
                id: _uid,
                unit_type_name: type_name,
                category,
                owner: self.units.owner[idx],
                position: pos,
                hp: self.units.hp[idx],
                max_hp: self.units.max_hp[idx],
                attack: ut_ref.map(|ut| ut.attack).unwrap_or(0),
                defense: ut_ref.map(|ut| ut.defense).unwrap_or(0),
                movement: self.units.movement[idx],
                max_movement: self.units.max_movement[idx],
                fortified: self.units.fortified[idx],
                skipped: self.units.skipped[idx],
                current_action: self.units.current_action[idx].clone(),
                current_action_animation,
                destination,
                direction: self.units.direction[idx],
                art_ini,
            });
        }

        // Collect city data
        let mut own_cities = Vec::new();
        let mut known_cities = Vec::new();

        for (cid, idx) in self.cities.iter_alive() {
            let pos = self.cities.position[idx];
            let city_owner = self.cities.owner[idx];

            if city_owner == player {
                // Own city: full detail
                let producing_name = self.cities.producing[idx].as_ref().map(|item| match item {
                    crate::city::ProductionItem::Unit { unit_type_id } => self
                        .unit_types
                        .get(*unit_type_id)
                        .map(|ut| ut.name.clone())
                        .unwrap_or_default(),
                    crate::city::ProductionItem::Building { building_id } => self
                        .building_registry
                        .get(building_id)
                        .map(|b| b.name.clone())
                        .unwrap_or_default(),
                    crate::city::ProductionItem::Wealth => "wealth".to_string(),
                });
                let pop = self.cities.population[idx];
                let prod_cost = if self.cities.producing[idx].is_some() {
                    Some(self.cities.production_cost[idx])
                } else {
                    None
                };
                own_cities.push(CitySnapshot {
                    id: cid,
                    name: self.cities.name[idx].clone(),
                    owner: city_owner,
                    position: pos,
                    population: pop,
                    food_stockpile: Some(self.cities.food_stockpile[idx]),
                    food_per_turn: Some(self.cities.food_per_turn[idx]),
                    shield_stockpile: Some(self.cities.shield_stockpile[idx]),
                    shields_per_turn: Some(self.cities.shields_per_turn[idx]),
                    commerce_per_turn: Some(self.cities.commerce_per_turn[idx]),
                    producing: producing_name,
                    worked_tiles: Some(self.cities.worked_tiles[idx].clone()),
                    production_cost: prod_cost,
                    food_growth_threshold: Some(10 + 2 * pop),
                    buildings: Some(self.cities.buildings[idx].clone()),
                });
            } else {
                // Enemy city on visible tile: limited info
                let vis = self.tiles.get_visibility(player, pos.x, pos.y);
                if vis == Visibility::Visible || vis == Visibility::Revealed {
                    known_cities.push(CitySnapshot {
                        id: cid,
                        name: self.cities.name[idx].clone(),
                        owner: city_owner,
                        position: pos,
                        population: self.cities.population[idx],
                        food_stockpile: None,
                        food_per_turn: None,
                        shield_stockpile: None,
                        shields_per_turn: None,
                        commerce_per_turn: None,
                        producing: None,
                        worked_tiles: None,
                        production_cost: None,
                        food_growth_threshold: None,
                        buildings: None,
                    });
                }
            }
        }

        let (gold, science, culture, researching, researched_techs, civilization) = self
            .players
            .get(player.0 as usize)
            .map(|p| {
                (
                    p.gold,
                    p.science,
                    p.culture,
                    p.researching.clone(),
                    p.researched_techs.clone(),
                    p.civilization.clone(),
                )
            })
            .unwrap_or((0, 0, 0, None, Vec::new(), None));

        let civ_def = civilization
            .as_ref()
            .and_then(|cid| self.civilization_registry.get(cid));
        let civ_adjective = civ_def.map(|def| def.adjective.clone());
        let civ_name = civ_def.map(|def| def.name.clone());
        let civ_noun = civ_def.map(|def| def.noun.clone());

        let researching_name = researching
            .as_ref()
            .and_then(|tid| self.tech_registry.get(tid))
            .map(|tdef| tdef.name.clone());

        // Calculate science per turn and gold per turn from owned cities
        let mut science_per_turn = 0i32;
        let mut total_commerce = 0i32;
        let mut total_maintenance = 0i32;
        for (_, idx) in self.cities.iter_alive() {
            if self.cities.owner[idx] == player {
                let commerce = self.cities.commerce_per_turn[idx];
                science_per_turn += commerce;
                total_commerce += commerce;
                // Sum building maintenance
                for bid in &self.cities.buildings[idx] {
                    if let Some(bdef) = self.building_registry.get(bid) {
                        total_maintenance += bdef.maintenance;
                    }
                }
            }
        }
        let gold_per_turn = total_commerce - total_maintenance;

        // Calculate research turns left
        let research_turns_left = researching.as_ref().and_then(|tech_id| {
            self.tech_registry.get(tech_id).map(|tdef| {
                let remaining = tdef.cost - science;
                if science_per_turn > 0 {
                    (remaining + science_per_turn - 1) / science_per_turn // ceil division
                } else if remaining <= 0 {
                    0
                } else {
                    -1 // infinite
                }
            })
        });

        PlayerView {
            player,
            turn: self.turn,
            gold,
            science,
            culture,
            researching,
            researching_name,
            researched_techs,
            science_per_turn,
            gold_per_turn,
            research_turns_left,
            civilization,
            civ_adjective,
            civ_name,
            civ_noun,
            map_width: self.tiles.width,
            map_height: self.tiles.height,
            wrap_x: self.tiles.wrap_x,
            wrap_y: self.tiles.wrap_y,
            visible_tiles,
            fringe_tiles,
            known_units,
            own_cities,
            known_cities,
        }
    }

    /// Builds a debug view with all tiles as Visible, all units shown, and all cities shown.
    /// Uses the given player for economy/research data.
    pub fn build_debug_view(&self, player: PlayerId) -> PlayerView {
        let mut view = self.build_player_view(player);

        // Include all tiles as Visible
        let total = (self.tiles.width * self.tiles.height) as usize;
        let mut all_tiles = Vec::with_capacity(total);
        for i in 0..total {
            let coord = self.tiles.coords(i);
            all_tiles.push(TileSnapshot {
                resource: crate::resource::revealed(self, player, i).map(|r| r.name.to_string()),
                coord,
                terrain: self.tiles.terrain[i],
                vegetation: self.tiles.vegetation[i],
                improvement: self.tiles.improvement[i],
                road_level: self.tiles.road_level[i],
                owner: self.tiles.owner[i],
                visibility: Visibility::Visible,
            });
        }
        view.visible_tiles = all_tiles;
        view.fringe_tiles = Vec::new(); // No fringe when all tiles visible

        // Include all units
        let mut all_units = Vec::new();
        for (uid, idx) in self.units.iter_alive() {
            let type_id = self.units.unit_type[idx];
            let ut_ref = self.unit_types.get(type_id);
            let type_name = ut_ref.map(|ut| ut.name.clone()).unwrap_or_default();
            let category = ut_ref.map(|ut| ut.category.clone()).unwrap_or_default();
            let art_ini = ut_ref.and_then(|ut| ut.art_ini.clone());
            let current_action_animation = self.units.current_action[idx]
                .as_ref()
                .and_then(|aid| self.action_registry.get(aid))
                .and_then(|def| def.animation_name.clone());
            all_units.push(UnitSnapshot {
                id: uid,
                unit_type_name: type_name,
                category,
                owner: self.units.owner[idx],
                position: self.units.position[idx],
                hp: self.units.hp[idx],
                max_hp: self.units.max_hp[idx],
                attack: ut_ref.map(|ut| ut.attack).unwrap_or(0),
                defense: ut_ref.map(|ut| ut.defense).unwrap_or(0),
                movement: self.units.movement[idx],
                max_movement: self.units.max_movement[idx],
                fortified: self.units.fortified[idx],
                skipped: self.units.skipped[idx],
                current_action: self.units.current_action[idx].clone(),
                current_action_animation,
                destination: self.units.destination[idx],
                direction: self.units.direction[idx],
                art_ini,
            });
        }
        view.known_units = all_units;

        // Include all non-owned cities
        let mut known_cities = Vec::new();
        for (cid, idx) in self.cities.iter_alive() {
            let city_owner = self.cities.owner[idx];
            if city_owner != player {
                known_cities.push(CitySnapshot {
                    id: cid,
                    name: self.cities.name[idx].clone(),
                    owner: city_owner,
                    position: self.cities.position[idx],
                    population: self.cities.population[idx],
                    food_stockpile: None,
                    food_per_turn: None,
                    shield_stockpile: None,
                    shields_per_turn: None,
                    commerce_per_turn: None,
                    producing: None,
                    worked_tiles: None,
                    production_cost: None,
                    food_growth_threshold: None,
                    buildings: None,
                });
            }
        }
        view.known_cities = known_cities;

        view
    }

    /// Creates a new world with noise-based terrain from the given configuration.
    pub fn new_with_terrain(config: &WorldConfig) -> Self {
        let tiles = crate::mapgen::generate_map(
            config.width,
            config.height,
            config.wrap_x,
            config.wrap_y,
            config.num_players,
            config.seed,
        );
        let players: Vec<Player> = (0..config.num_players)
            .map(|i| Player {
                id: PlayerId(i),
                name: format!("Player {}", i),
                alive: true,
                gold: 0,
                science: 0,
                culture: 0,
                researching: None,
                researched_techs: Vec::new(),
                civilization: None,
            })
            .collect();

        World {
            tiles,
            resources_enabled: false,
            units: UnitStore::new(),
            cities: CityStore::new(),
            unit_types: UnitTypeRegistry::new(),
            action_registry: ActionRegistry::new(),
            tech_registry: TechRegistry::new(),
            building_registry: BuildingRegistry::new(),
            civilization_registry: CivRegistry::new(),
            players,
            turn: 1,
            current_player: PlayerId(0),
            rng: ChaCha8Rng::seed_from_u64(config.seed),
            rng_seed: config.seed,
            rng_calls: 0,
            pending_events: Vec::new(),
            pending_city_ids: Vec::new(),
            pending_consumed_units: Vec::new(),
        }
    }

    /// Creates a new world with flat grassland from the given configuration.
    ///
    /// # Panics
    ///
    /// Panics if both `wrap_x` and `wrap_y` are true and `width % height != 0`.
    pub fn new(config: &WorldConfig) -> Self {
        assert!(
            !(config.wrap_x && config.wrap_y) || config.width.is_multiple_of(config.height),
            "Torus maps (wrap_x + wrap_y) require width ({}) to be a multiple of height ({})",
            config.width,
            config.height,
        );
        let players: Vec<Player> = (0..config.num_players)
            .map(|i| Player {
                id: PlayerId(i),
                name: format!("Player {}", i),
                alive: true,
                gold: 0,
                science: 0,
                culture: 0,
                researching: None,
                researched_techs: Vec::new(),
                civilization: None,
            })
            .collect();

        World {
            tiles: TileStore::new(
                config.width,
                config.height,
                config.wrap_x,
                config.wrap_y,
                Terrain::Grassland,
                config.num_players,
            ),
            resources_enabled: false,
            units: UnitStore::new(),
            cities: CityStore::new(),
            unit_types: UnitTypeRegistry::new(),
            action_registry: ActionRegistry::new(),
            tech_registry: TechRegistry::new(),
            building_registry: BuildingRegistry::new(),
            civilization_registry: CivRegistry::new(),
            players,
            turn: 1,
            current_player: PlayerId(0),
            rng: ChaCha8Rng::seed_from_u64(config.seed),
            rng_seed: config.seed,
            rng_calls: 0,
            pending_events: Vec::new(),
            pending_city_ids: Vec::new(),
            pending_consumed_units: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> WorldConfig {
        WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        }
    }

    #[test]
    fn test_world_new() {
        let world = World::new(&test_config());
        assert_eq!(world.tiles.width, 10);
        assert_eq!(world.tiles.height, 10);
        assert!(world.tiles.wrap_x);
        assert_eq!(world.units.count(), 0);
        assert_eq!(world.players.len(), 2);
        assert_eq!(world.turn, 1);
        assert_eq!(world.current_player, PlayerId(0));
        assert!(world.players[0].alive);
        assert!(world.players[1].alive);
    }

    #[test]
    fn test_world_serialization_roundtrip() {
        use crate::tile::Terrain;
        use crate::types::{TileCoord, UnitTypeId};
        use crate::unit_type::UnitType;

        let mut world = World::new(&test_config());

        // Set some terrain
        let idx = world.tiles.idx(3, 3);
        world.tiles.terrain[idx] = Terrain::Mountain;

        // Register a unit type and spawn a unit
        let warrior = UnitType {
            id: UnitTypeId(0),
            name: "warrior".to_string(),
            attack: 1,
            defense: 1,
            movement: 1,
            max_hp: 3,
            cost: 10,
            category: "melee".to_string(),
            traits: vec![],
            actions: vec![],
            replaces: None,
            requires_civ: None,
            art_ini: None,
        };
        let wid = world.unit_types.register(warrior.clone());
        let uid = world
            .units
            .spawn(wid, PlayerId(0), TileCoord { x: 5, y: 5 }, &warrior);

        // Serialize
        let json = serde_json::to_string(&world).unwrap();
        let mut back: World = serde_json::from_str(&json).unwrap();
        back.restore_rng();

        // Verify
        let back_idx = back.tiles.idx(3, 3);
        assert_eq!(back.tiles.terrain[back_idx], Terrain::Mountain);
        assert_eq!(back.units.count(), 1);
        let unit_idx = back.units.get(uid).unwrap();
        assert_eq!(back.units.position[unit_idx], TileCoord { x: 5, y: 5 });
        assert_eq!(back.players.len(), 2);
        assert_eq!(back.players[0].name, "Player 0");
        assert_eq!(back.unit_types.get_by_name("warrior").unwrap().attack, 1);
    }

    #[test]
    fn test_player_view_respects_fog_of_war() {
        use crate::types::{TileCoord, UnitTypeId};
        use crate::unit_type::UnitType;

        let mut world = World::new(&test_config());

        let warrior = UnitType {
            id: UnitTypeId(0),
            name: "warrior".to_string(),
            attack: 1,
            defense: 1,
            movement: 1,
            max_hp: 3,
            cost: 10,
            category: "melee".to_string(),
            traits: vec![],
            actions: vec![],
            replaces: None,
            requires_civ: None,
            art_ini: None,
        };
        let wid = world.unit_types.register(warrior.clone());

        // Player 0 has visibility around (0,0)
        world
            .tiles
            .set_visibility(PlayerId(0), 0, 0, Visibility::Visible);
        world
            .tiles
            .set_visibility(PlayerId(0), 1, 0, Visibility::Visible);
        world
            .tiles
            .set_visibility(PlayerId(0), 0, 1, Visibility::Visible);

        // Player 1's unit at (9,9) — not visible to player 0
        let _p1_unit = world
            .units
            .spawn(wid, PlayerId(1), TileCoord { x: 9, y: 9 }, &warrior);

        // Player 0's unit at (0,0) — visible to player 0
        let _p0_unit = world
            .units
            .spawn(wid, PlayerId(0), TileCoord { x: 0, y: 0 }, &warrior);

        let view = world.build_player_view(PlayerId(0));

        // Should include only visible/revealed tiles
        assert_eq!(view.visible_tiles.len(), 3);
        assert_eq!(view.player, PlayerId(0));
        assert_eq!(view.turn, 1);

        // Should only include player 0's unit (on visible tile)
        // Player 1's unit at (9,9) is on an Unseen tile — not included
        assert_eq!(view.known_units.len(), 1);
        assert_eq!(view.known_units[0].owner, PlayerId(0));
        assert_eq!(view.known_units[0].position, TileCoord { x: 0, y: 0 });
    }

    #[test]
    fn test_player_view_includes_enemy_units_on_visible_tiles() {
        use crate::types::{TileCoord, UnitTypeId};
        use crate::unit_type::UnitType;

        let mut world = World::new(&test_config());

        let warrior = UnitType {
            id: UnitTypeId(0),
            name: "warrior".to_string(),
            attack: 1,
            defense: 1,
            movement: 1,
            max_hp: 3,
            cost: 10,
            category: "melee".to_string(),
            traits: vec![],
            actions: vec![],
            replaces: None,
            requires_civ: None,
            art_ini: None,
        };
        let wid = world.unit_types.register(warrior.clone());

        // Player 0 can see tile (5,5)
        world
            .tiles
            .set_visibility(PlayerId(0), 5, 5, Visibility::Visible);

        // Player 1's unit at (5,5) — visible to player 0
        let _p1_unit = world
            .units
            .spawn(wid, PlayerId(1), TileCoord { x: 5, y: 5 }, &warrior);

        let view = world.build_player_view(PlayerId(0));
        assert_eq!(view.known_units.len(), 1);
        assert_eq!(view.known_units[0].owner, PlayerId(1));
    }

    #[test]
    fn test_player_view_revealed_tiles_no_units() {
        use crate::types::{TileCoord, UnitTypeId};
        use crate::unit_type::UnitType;

        let mut world = World::new(&test_config());

        let warrior = UnitType {
            id: UnitTypeId(0),
            name: "warrior".to_string(),
            attack: 1,
            defense: 1,
            movement: 1,
            max_hp: 3,
            cost: 10,
            category: "melee".to_string(),
            traits: vec![],
            actions: vec![],
            replaces: None,
            requires_civ: None,
            art_ini: None,
        };
        let wid = world.unit_types.register(warrior.clone());

        // Tile (3,3) is Revealed (previously seen, no longer in sight)
        world
            .tiles
            .set_visibility(PlayerId(0), 3, 3, Visibility::Revealed);

        // Enemy unit on the Revealed tile — should NOT appear in view
        let _p1_unit = world
            .units
            .spawn(wid, PlayerId(1), TileCoord { x: 3, y: 3 }, &warrior);

        let view = world.build_player_view(PlayerId(0));

        // Tile should be included (it's Revealed)
        assert_eq!(view.visible_tiles.len(), 1);
        assert_eq!(view.visible_tiles[0].visibility, Visibility::Revealed);

        // But no units (not Visible, just Revealed)
        assert_eq!(view.known_units.len(), 0);
    }

    #[test]
    fn test_player_view_serializes() {
        let world = World::new(&test_config());
        let view = world.build_player_view(PlayerId(0));
        let json = serde_json::to_string(&view).unwrap();
        let _back: crate::protocol::PlayerView = serde_json::from_str(&json).unwrap();
    }
}
