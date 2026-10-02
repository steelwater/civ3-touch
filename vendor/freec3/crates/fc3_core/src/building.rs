use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Definition of a building that can be constructed in a city.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    pub cost: i32,
    pub maintenance: i32,
    pub requires: Vec<String>,
}

/// Registry of all defined buildings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildingRegistry {
    buildings: Vec<BuildingDef>,
    by_id: HashMap<String, usize>,
}

impl BuildingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new building definition. Returns true if newly registered,
    /// false if a building with that id already existed (and was replaced).
    pub fn register(&mut self, building: BuildingDef) -> bool {
        if let Some(&idx) = self.by_id.get(&building.id) {
            self.buildings[idx] = building;
            false
        } else {
            let idx = self.buildings.len();
            self.by_id.insert(building.id.clone(), idx);
            self.buildings.push(building);
            true
        }
    }

    /// Looks up a building by its id.
    pub fn get(&self, id: &str) -> Option<&BuildingDef> {
        self.by_id.get(id).map(|&idx| &self.buildings[idx])
    }

    /// Returns all registered buildings.
    pub fn all(&self) -> &[BuildingDef] {
        &self.buildings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut reg = BuildingRegistry::new();
        let building = BuildingDef {
            id: "granary".to_string(),
            name: "Granary".to_string(),
            cost: 60,
            maintenance: 1,
            requires: vec!["pottery".to_string()],
        };
        assert!(reg.register(building));
        let b = reg.get("granary").unwrap();
        assert_eq!(b.name, "Granary");
        assert_eq!(b.cost, 60);
        assert_eq!(b.maintenance, 1);
        assert_eq!(b.requires, vec!["pottery".to_string()]);
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let reg = BuildingRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_register_replaces_existing() {
        let mut reg = BuildingRegistry::new();
        reg.register(BuildingDef {
            id: "test".to_string(),
            name: "Test V1".to_string(),
            cost: 10,
            maintenance: 0,
            requires: vec![],
        });
        let replaced = reg.register(BuildingDef {
            id: "test".to_string(),
            name: "Test V2".to_string(),
            cost: 20,
            maintenance: 1,
            requires: vec![],
        });
        assert!(!replaced);
        assert_eq!(reg.get("test").unwrap().name, "Test V2");
        assert_eq!(reg.all().len(), 1);
    }

    #[test]
    fn test_all() {
        let mut reg = BuildingRegistry::new();
        reg.register(BuildingDef {
            id: "palace".to_string(),
            name: "Palace".to_string(),
            cost: 200,
            maintenance: 0,
            requires: vec![],
        });
        reg.register(BuildingDef {
            id: "granary".to_string(),
            name: "Granary".to_string(),
            cost: 60,
            maintenance: 1,
            requires: vec!["pottery".to_string()],
        });
        assert_eq!(reg.all().len(), 2);
    }

    #[test]
    fn test_building_with_prerequisites() {
        let mut reg = BuildingRegistry::new();
        reg.register(BuildingDef {
            id: "library".to_string(),
            name: "Library".to_string(),
            cost: 80,
            maintenance: 1,
            requires: vec!["alphabet".to_string()],
        });
        let lib = reg.get("library").unwrap();
        assert_eq!(lib.requires, vec!["alphabet".to_string()]);
    }
}
