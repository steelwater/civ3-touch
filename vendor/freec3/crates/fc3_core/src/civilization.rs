use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Definition of a civilization that can be assigned to a player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CivDef {
    pub id: String,
    pub name: String,
    pub ruler_name: String,
    pub adjective: String,
    pub noun: String,
}

/// Registry of all defined civilizations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CivRegistry {
    civs: Vec<CivDef>,
    by_id: HashMap<String, usize>,
}

impl CivRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new civilization definition. Returns true if newly registered,
    /// false if a civ with that id already existed (and was replaced).
    pub fn register(&mut self, civ: CivDef) -> bool {
        if let Some(&idx) = self.by_id.get(&civ.id) {
            self.civs[idx] = civ;
            false
        } else {
            let idx = self.civs.len();
            self.by_id.insert(civ.id.clone(), idx);
            self.civs.push(civ);
            true
        }
    }

    /// Looks up a civ by its id.
    pub fn get(&self, id: &str) -> Option<&CivDef> {
        self.by_id.get(id).map(|&idx| &self.civs[idx])
    }

    /// Returns all registered civs.
    pub fn all(&self) -> &[CivDef] {
        &self.civs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut reg = CivRegistry::new();
        let civ = CivDef {
            id: "rome".to_string(),
            name: "Roman Empire".to_string(),
            ruler_name: "Caesar".to_string(),
            adjective: "Roman".to_string(),
            noun: "Romans".to_string(),
        };
        assert!(reg.register(civ));
        let c = reg.get("rome").unwrap();
        assert_eq!(c.name, "Roman Empire");
        assert_eq!(c.ruler_name, "Caesar");
        assert_eq!(c.adjective, "Roman");
        assert_eq!(c.noun, "Romans");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let reg = CivRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_register_replaces_existing() {
        let mut reg = CivRegistry::new();
        reg.register(CivDef {
            id: "test".to_string(),
            name: "Test V1".to_string(),
            ruler_name: "Leader V1".to_string(),
            adjective: "Testy".to_string(),
            noun: "Testers".to_string(),
        });
        let replaced = reg.register(CivDef {
            id: "test".to_string(),
            name: "Test V2".to_string(),
            ruler_name: "Leader V2".to_string(),
            adjective: "Testier".to_string(),
            noun: "Testers2".to_string(),
        });
        assert!(!replaced);
        assert_eq!(reg.get("test").unwrap().name, "Test V2");
        assert_eq!(reg.all().len(), 1);
    }

    #[test]
    fn test_all() {
        let mut reg = CivRegistry::new();
        reg.register(CivDef {
            id: "a".to_string(),
            name: "A".to_string(),
            ruler_name: "Leader A".to_string(),
            adjective: "Aish".to_string(),
            noun: "As".to_string(),
        });
        reg.register(CivDef {
            id: "b".to_string(),
            name: "B".to_string(),
            ruler_name: "Leader B".to_string(),
            adjective: "Bish".to_string(),
            noun: "Bs".to_string(),
        });
        assert_eq!(reg.all().len(), 2);
    }
}
