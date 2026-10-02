use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::UnitTypeId;

/// Definition of a unit type (e.g. "warrior", "settler").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitType {
    pub id: UnitTypeId,
    pub name: String,
    pub attack: i32,
    pub defense: i32,
    pub movement: i32,
    pub max_hp: i32,
    pub cost: i32,
    pub category: String,
    pub traits: Vec<String>,
    pub actions: Vec<String>,
    pub replaces: Option<UnitTypeId>,
    pub requires_civ: Option<String>,
    /// Path to the unit art INI file, relative to the resource directory.
    /// Example: "Art/Units/warrior/Warrior.INI"
    pub art_ini: Option<String>,
}

/// Registry of all defined unit types. Types are registered by name and
/// looked up by either name or ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitTypeRegistry {
    types: Vec<UnitType>,
    by_name: HashMap<String, UnitTypeId>,
}

impl UnitTypeRegistry {
    pub fn new() -> Self {
        UnitTypeRegistry {
            types: Vec::new(),
            by_name: HashMap::new(),
        }
    }

    /// Registers a unit type definition, assigning it the next available ID.
    /// The `id` field on the input is overwritten with the assigned ID.
    pub fn register(&mut self, mut unit_type: UnitType) -> UnitTypeId {
        let id = UnitTypeId(self.types.len() as u16);
        unit_type.id = id;
        self.by_name.insert(unit_type.name.clone(), id);
        self.types.push(unit_type);
        id
    }

    /// Looks up a unit type by its ID.
    pub fn get(&self, id: UnitTypeId) -> Option<&UnitType> {
        self.types.get(id.0 as usize)
    }

    /// Iterates over all registered unit types.
    pub fn iter(&self) -> impl Iterator<Item = &UnitType> {
        self.types.iter()
    }

    /// Looks up a unit type by its name.
    pub fn get_by_name(&self, name: &str) -> Option<&UnitType> {
        let id = self.by_name.get(name)?;
        self.get(*id)
    }
}

impl Default for UnitTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_warrior() -> UnitType {
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

    fn make_settler() -> UnitType {
        UnitType {
            id: UnitTypeId(0),
            name: "settler".to_string(),
            attack: 0,
            defense: 0,
            movement: 1,
            max_hp: 1,
            cost: 30,
            category: "civilian".to_string(),
            traits: vec!["found_city".to_string()],
            actions: vec![],
            replaces: None,
            requires_civ: None,
            art_ini: None,
        }
    }

    #[test]
    fn test_register_and_lookup_by_name() {
        let mut reg = UnitTypeRegistry::new();
        reg.register(make_warrior());
        let ut = reg.get_by_name("warrior").unwrap();
        assert_eq!(ut.attack, 1);
        assert_eq!(ut.defense, 1);
        assert_eq!(ut.movement, 1);
        assert_eq!(ut.max_hp, 3);
    }

    #[test]
    fn test_register_two_types() {
        let mut reg = UnitTypeRegistry::new();
        let w_id = reg.register(make_warrior());
        let s_id = reg.register(make_settler());

        assert_ne!(w_id, s_id);
        assert!(reg.get(w_id).is_some());
        assert!(reg.get(s_id).is_some());
        assert_eq!(reg.get(w_id).unwrap().name, "warrior");
        assert_eq!(reg.get(s_id).unwrap().name, "settler");
    }

    #[test]
    fn test_lookup_nonexistent_returns_none() {
        let reg = UnitTypeRegistry::new();
        assert!(reg.get_by_name("archer").is_none());
        assert!(reg.get(UnitTypeId(999)).is_none());
    }
}
