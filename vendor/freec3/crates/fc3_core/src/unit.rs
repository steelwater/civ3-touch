use serde::{Deserialize, Serialize};

use crate::dynamic::DynamicColumns;
use crate::id::{GenId, UnitId};
use crate::types::{Direction, PlayerId, TileCoord, UnitTypeId};
use crate::unit_type::UnitType;

/// Movement values are stored as integer-thirds internally.
/// 1 movement point in game terms = 3 internal units.
/// This avoids floating point entirely while supporting fractional costs:
/// - Road cost = 1 (1/3 movement point)
/// - Grassland/Plains/etc = 3 (1 movement point)
/// - Hill/Forest/Jungle = 6 (2 movement points)
pub const MOVEMENT_SCALE: i32 = 3;

/// Structure-of-Arrays storage for all units in the game.
///
/// Uses generational indices for safe handle reuse. Dead slots are tracked
/// in a free list and recycled on the next `spawn()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitStore {
    // Bookkeeping
    pub generation: Vec<u32>,
    pub alive: Vec<bool>,
    free_list: Vec<u32>,
    count: usize,

    // Identity
    pub unit_type: Vec<UnitTypeId>,
    pub owner: Vec<PlayerId>,

    // Spatial
    pub position: Vec<TileCoord>,

    // Turn state
    pub movement: Vec<i32>,
    pub max_movement: Vec<i32>,

    // Combat
    pub hp: Vec<i32>,
    pub max_hp: Vec<i32>,

    // Status
    pub fortified: Vec<bool>,
    pub has_moved: Vec<bool>,
    pub skipped: Vec<bool>,

    // Action tracking
    pub current_action: Vec<Option<String>>,
    pub action_turns_left: Vec<i32>,

    // Navigation
    pub destination: Vec<Option<TileCoord>>,

    // Visual
    pub direction: Vec<Direction>,

    // Mod-defined attributes
    pub dynamic: DynamicColumns,
}

impl UnitStore {
    /// Creates a new empty unit store.
    pub fn new() -> Self {
        UnitStore {
            generation: Vec::new(),
            alive: Vec::new(),
            free_list: Vec::new(),
            count: 0,
            unit_type: Vec::new(),
            owner: Vec::new(),
            position: Vec::new(),
            movement: Vec::new(),
            max_movement: Vec::new(),
            hp: Vec::new(),
            max_hp: Vec::new(),
            fortified: Vec::new(),
            has_moved: Vec::new(),
            skipped: Vec::new(),
            current_action: Vec::new(),
            action_turns_left: Vec::new(),
            destination: Vec::new(),
            direction: Vec::new(),
            dynamic: DynamicColumns::new(),
        }
    }

    /// Returns the number of living units.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Spawns a new unit, reusing a free-list slot if available.
    /// Copies stats from the `template` unit type.
    pub fn spawn(
        &mut self,
        unit_type_id: UnitTypeId,
        owner: PlayerId,
        position: TileCoord,
        template: &UnitType,
    ) -> UnitId {
        let scaled_movement = template.movement * MOVEMENT_SCALE;
        let idx = if let Some(free_idx) = self.free_list.pop() {
            let i = free_idx as usize;
            self.alive[i] = true;
            self.unit_type[i] = unit_type_id;
            self.owner[i] = owner;
            self.position[i] = position;
            self.movement[i] = scaled_movement;
            self.max_movement[i] = scaled_movement;
            self.hp[i] = template.max_hp;
            self.max_hp[i] = template.max_hp;
            self.fortified[i] = false;
            self.has_moved[i] = false;
            self.skipped[i] = false;
            self.current_action[i] = None;
            self.action_turns_left[i] = 0;
            self.destination[i] = None;
            self.direction[i] = Direction::default();
            i
        } else {
            let i = self.generation.len();
            self.generation.push(0);
            self.alive.push(true);
            self.unit_type.push(unit_type_id);
            self.owner.push(owner);
            self.position.push(position);
            self.movement.push(scaled_movement);
            self.max_movement.push(scaled_movement);
            self.hp.push(template.max_hp);
            self.max_hp.push(template.max_hp);
            self.fortified.push(false);
            self.has_moved.push(false);
            self.skipped.push(false);
            self.current_action.push(None);
            self.action_turns_left.push(0);
            self.destination.push(None);
            self.direction.push(Direction::default());
            self.dynamic.grow(i + 1);
            i
        };
        self.count += 1;
        GenId {
            index: idx as u32,
            generation: self.generation[idx],
        }
    }

    /// Destroys a unit by its handle. Returns `true` if the unit was alive
    /// and successfully destroyed, `false` if the handle was stale.
    pub fn destroy(&mut self, id: UnitId) -> bool {
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

    /// Returns the raw array index if the given handle is valid (generation
    /// matches and slot is alive).
    pub fn get(&self, id: UnitId) -> Option<usize> {
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

    /// Returns true if the handle refers to a living unit.
    pub fn is_alive(&self, id: UnitId) -> bool {
        self.get(id).is_some()
    }

    /// Iterates over all living units, yielding `(UnitId, raw_index)`.
    pub fn iter_alive(&self) -> impl Iterator<Item = (UnitId, usize)> + '_ {
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

impl Default for UnitStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_warrior_type() -> UnitType {
        UnitType {
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
        }
    }

    #[test]
    fn test_empty_store() {
        let store = UnitStore::new();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_spawn_one_unit_fields_match() {
        let mut store = UnitStore::new();
        let template = make_warrior_type();
        let pos = TileCoord { x: 5, y: 5 };
        let id = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);

        assert_eq!(store.count(), 1);
        let idx = store.get(id).unwrap();
        assert_eq!(store.unit_type[idx], UnitTypeId(0));
        assert_eq!(store.owner[idx], PlayerId(0));
        assert_eq!(store.position[idx], pos);
        // Movement is scaled by MOVEMENT_SCALE (3): template.movement=1 → runtime=3
        assert_eq!(store.movement[idx], 1 * MOVEMENT_SCALE);
        assert_eq!(store.max_movement[idx], 1 * MOVEMENT_SCALE);
        assert_eq!(store.hp[idx], 3);
        assert_eq!(store.max_hp[idx], 3);
        assert!(!store.fortified[idx]);
        assert!(!store.has_moved[idx]);
    }

    #[test]
    fn test_spawn_destroy_reuse_slot() {
        let mut store = UnitStore::new();
        let template = make_warrior_type();
        let pos = TileCoord { x: 0, y: 0 };

        let id0 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        let id1 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        let _id2 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);

        assert_eq!(store.count(), 3);

        // Destroy the middle one
        assert!(store.destroy(id1));
        assert_eq!(store.count(), 2);

        // Spawn again — should reuse slot 1
        let id3 = store.spawn(
            UnitTypeId(0),
            PlayerId(1),
            TileCoord { x: 9, y: 9 },
            &template,
        );
        assert_eq!(id3.index, id1.index);
        assert_ne!(id3.generation, id1.generation);
        assert_eq!(store.count(), 3);

        // Old handle is stale
        assert!(!store.is_alive(id1));
        assert!(store.is_alive(id0));
        assert!(store.is_alive(id3));
    }

    #[test]
    fn test_stale_id_returns_none() {
        let mut store = UnitStore::new();
        let template = make_warrior_type();
        let id = store.spawn(
            UnitTypeId(0),
            PlayerId(0),
            TileCoord { x: 0, y: 0 },
            &template,
        );
        store.destroy(id);
        assert!(store.get(id).is_none());
        assert!(!store.is_alive(id));
    }

    #[test]
    fn test_iter_alive_skips_dead() {
        let mut store = UnitStore::new();
        let template = make_warrior_type();
        let pos = TileCoord { x: 0, y: 0 };

        let _id0 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        let id1 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        let _id2 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        let id3 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        let _id4 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);

        store.destroy(id1);
        store.destroy(id3);

        let alive: Vec<_> = store.iter_alive().collect();
        assert_eq!(alive.len(), 3);
    }

    #[test]
    fn test_spawn_100_destroy_even_indexed() {
        let mut store = UnitStore::new();
        let template = make_warrior_type();
        let pos = TileCoord { x: 0, y: 0 };

        let ids: Vec<_> = (0..100)
            .map(|_| store.spawn(UnitTypeId(0), PlayerId(0), pos, &template))
            .collect();
        assert_eq!(store.count(), 100);

        for (i, id) in ids.iter().enumerate() {
            if i % 2 == 0 {
                store.destroy(*id);
            }
        }
        assert_eq!(store.count(), 50);

        let alive: Vec<_> = store.iter_alive().collect();
        assert_eq!(alive.len(), 50);
    }

    #[test]
    fn test_generation_increments_on_reuse() {
        let mut store = UnitStore::new();
        let template = make_warrior_type();
        let pos = TileCoord { x: 0, y: 0 };

        let id1 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        assert_eq!(id1.generation, 0);

        store.destroy(id1);
        let id2 = store.spawn(UnitTypeId(0), PlayerId(0), pos, &template);
        assert_eq!(id2.index, id1.index);
        assert_eq!(id2.generation, 1);

        // The old handle is invalid
        assert!(store.get(id1).is_none());
        assert!(store.get(id2).is_some());
    }
}
