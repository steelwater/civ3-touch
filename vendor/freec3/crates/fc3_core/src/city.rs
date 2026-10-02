use serde::{Deserialize, Serialize};

use crate::dynamic::DynamicColumns;
use crate::id::{CityId, GenId};
use crate::types::{PlayerId, TileCoord, UnitTypeId};

/// What a city is currently producing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProductionItem {
    Unit { unit_type_id: UnitTypeId },
    Building { building_id: String },
    Wealth,
}

/// Structure-of-Arrays storage for all cities in the game.
///
/// Uses generational indices for safe handle reuse, same pattern as UnitStore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CityStore {
    // Bookkeeping
    pub generation: Vec<u32>,
    pub alive: Vec<bool>,
    free_list: Vec<u32>,
    count: usize,

    // Identity
    pub name: Vec<String>,
    pub owner: Vec<PlayerId>,

    // Spatial
    pub position: Vec<TileCoord>,

    // Population
    pub population: Vec<i32>,

    // Food
    pub food_stockpile: Vec<i32>,
    pub food_per_turn: Vec<i32>,

    // Production
    pub shield_stockpile: Vec<i32>,
    pub shields_per_turn: Vec<i32>,

    // Commerce
    pub commerce_per_turn: Vec<i32>,

    // Build queue
    pub producing: Vec<Option<ProductionItem>>,
    pub production_cost: Vec<i32>,

    // Worked tiles per city
    pub worked_tiles: Vec<Vec<TileCoord>>,

    // Buildings constructed in each city
    pub buildings: Vec<Vec<String>>,

    // Mod-defined attributes
    pub dynamic: DynamicColumns,
}

impl CityStore {
    /// Creates a new empty city store.
    pub fn new() -> Self {
        CityStore {
            generation: Vec::new(),
            alive: Vec::new(),
            free_list: Vec::new(),
            count: 0,
            name: Vec::new(),
            owner: Vec::new(),
            position: Vec::new(),
            population: Vec::new(),
            food_stockpile: Vec::new(),
            food_per_turn: Vec::new(),
            shield_stockpile: Vec::new(),
            shields_per_turn: Vec::new(),
            commerce_per_turn: Vec::new(),
            producing: Vec::new(),
            production_cost: Vec::new(),
            worked_tiles: Vec::new(),
            buildings: Vec::new(),
            dynamic: DynamicColumns::new(),
        }
    }

    /// Returns the number of living cities.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Spawns a new city, reusing a free-list slot if available.
    pub fn spawn(&mut self, name: String, owner: PlayerId, position: TileCoord) -> CityId {
        let idx = if let Some(free_idx) = self.free_list.pop() {
            let i = free_idx as usize;
            self.alive[i] = true;
            self.name[i] = name;
            self.owner[i] = owner;
            self.position[i] = position;
            self.population[i] = 1;
            self.food_stockpile[i] = 0;
            self.food_per_turn[i] = 0;
            self.shield_stockpile[i] = 0;
            self.shields_per_turn[i] = 0;
            self.commerce_per_turn[i] = 0;
            self.producing[i] = None;
            self.production_cost[i] = 0;
            self.worked_tiles[i] = Vec::new();
            self.buildings[i] = Vec::new();
            i
        } else {
            let i = self.generation.len();
            self.generation.push(0);
            self.alive.push(true);
            self.name.push(name);
            self.owner.push(owner);
            self.position.push(position);
            self.population.push(1);
            self.food_stockpile.push(0);
            self.food_per_turn.push(0);
            self.shield_stockpile.push(0);
            self.shields_per_turn.push(0);
            self.commerce_per_turn.push(0);
            self.producing.push(None);
            self.production_cost.push(0);
            self.worked_tiles.push(Vec::new());
            self.buildings.push(Vec::new());
            self.dynamic.grow(i + 1);
            i
        };
        self.count += 1;
        GenId {
            index: idx as u32,
            generation: self.generation[idx],
        }
    }

    /// Destroys a city by its handle. Returns `true` if the city was alive
    /// and successfully destroyed, `false` if the handle was stale.
    pub fn destroy(&mut self, id: CityId) -> bool {
        let idx = id.index as usize;
        if idx >= self.generation.len() {
            return false;
        }
        if self.generation[idx] != id.generation || !self.alive[idx] {
            return false;
        }
        self.alive[idx] = false;
        self.generation[idx] += 1;
        self.free_list.push(id.index);
        self.count -= 1;
        true
    }

    /// Returns the raw array index if the given handle is valid.
    pub fn get(&self, id: CityId) -> Option<usize> {
        let idx = id.index as usize;
        if idx >= self.generation.len() {
            return None;
        }
        if self.generation[idx] == id.generation && self.alive[idx] {
            Some(idx)
        } else {
            None
        }
    }

    /// Returns true if the handle refers to a living city.
    pub fn is_alive(&self, id: CityId) -> bool {
        self.get(id).is_some()
    }

    /// Iterates over all living cities, yielding `(CityId, raw_index)`.
    pub fn iter_alive(&self) -> impl Iterator<Item = (CityId, usize)> + '_ {
        self.alive
            .iter()
            .enumerate()
            .filter(|(_, alive)| **alive)
            .map(|(i, _)| {
                let id = GenId {
                    index: i as u32,
                    generation: self.generation[i],
                };
                (id, i)
            })
    }
}

impl Default for CityStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_store() {
        let store = CityStore::new();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_spawn_city_fields() {
        let mut store = CityStore::new();
        let pos = TileCoord { x: 5, y: 5 };
        let id = store.spawn("Alexandria".to_string(), PlayerId(0), pos);

        assert_eq!(store.count(), 1);
        let idx = store.get(id).unwrap();
        assert_eq!(store.name[idx], "Alexandria");
        assert_eq!(store.owner[idx], PlayerId(0));
        assert_eq!(store.position[idx], pos);
        assert_eq!(store.population[idx], 1);
        assert_eq!(store.food_stockpile[idx], 0);
        assert_eq!(store.shield_stockpile[idx], 0);
        assert!(store.producing[idx].is_none());
    }

    #[test]
    fn test_spawn_destroy_reuse_slot() {
        let mut store = CityStore::new();
        let pos = TileCoord { x: 0, y: 0 };

        let id0 = store.spawn("A".to_string(), PlayerId(0), pos);
        let id1 = store.spawn("B".to_string(), PlayerId(0), pos);
        let _id2 = store.spawn("C".to_string(), PlayerId(0), pos);
        assert_eq!(store.count(), 3);

        assert!(store.destroy(id1));
        assert_eq!(store.count(), 2);

        let id3 = store.spawn("D".to_string(), PlayerId(1), TileCoord { x: 9, y: 9 });
        assert_eq!(id3.index, id1.index);
        assert_ne!(id3.generation, id1.generation);
        assert_eq!(store.count(), 3);

        assert!(!store.is_alive(id1));
        assert!(store.is_alive(id0));
        assert!(store.is_alive(id3));
    }

    #[test]
    fn test_stale_city_id_returns_none() {
        let mut store = CityStore::new();
        let id = store.spawn("Rome".to_string(), PlayerId(0), TileCoord { x: 0, y: 0 });
        store.destroy(id);
        assert!(store.get(id).is_none());
        assert!(!store.is_alive(id));
    }

    #[test]
    fn test_iter_alive_skips_destroyed() {
        let mut store = CityStore::new();
        let pos = TileCoord { x: 0, y: 0 };

        let _id0 = store.spawn("A".to_string(), PlayerId(0), pos);
        let id1 = store.spawn("B".to_string(), PlayerId(0), pos);
        let _id2 = store.spawn("C".to_string(), PlayerId(0), pos);
        let id3 = store.spawn("D".to_string(), PlayerId(0), pos);
        let _id4 = store.spawn("E".to_string(), PlayerId(0), pos);

        store.destroy(id1);
        store.destroy(id3);

        let alive: Vec<_> = store.iter_alive().collect();
        assert_eq!(alive.len(), 3);
    }

    #[test]
    fn test_production_item_serialization() {
        let item = ProductionItem::Unit {
            unit_type_id: UnitTypeId(5),
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: ProductionItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }
}
