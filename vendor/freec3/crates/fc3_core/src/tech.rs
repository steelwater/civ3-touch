use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Definition of a technology that can be researched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechDef {
    pub id: String,
    pub name: String,
    pub cost: i32,
    pub requires: Vec<String>,
}

/// Registry of all defined technologies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TechRegistry {
    techs: Vec<TechDef>,
    by_id: HashMap<String, usize>,
}

impl TechRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new technology definition. Returns true if newly registered,
    /// false if a tech with that id already existed (and was replaced).
    pub fn register(&mut self, tech: TechDef) -> bool {
        if let Some(&idx) = self.by_id.get(&tech.id) {
            self.techs[idx] = tech;
            false
        } else {
            let idx = self.techs.len();
            self.by_id.insert(tech.id.clone(), idx);
            self.techs.push(tech);
            true
        }
    }

    /// Looks up a tech by its id.
    pub fn get(&self, id: &str) -> Option<&TechDef> {
        self.by_id.get(id).map(|&idx| &self.techs[idx])
    }

    /// Returns all registered techs.
    pub fn all(&self) -> &[TechDef] {
        &self.techs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut reg = TechRegistry::new();
        let tech = TechDef {
            id: "bronze_working".to_string(),
            name: "Bronze Working".to_string(),
            cost: 40,
            requires: vec![],
        };
        assert!(reg.register(tech));
        let t = reg.get("bronze_working").unwrap();
        assert_eq!(t.name, "Bronze Working");
        assert_eq!(t.cost, 40);
        assert!(t.requires.is_empty());
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let reg = TechRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_register_replaces_existing() {
        let mut reg = TechRegistry::new();
        reg.register(TechDef {
            id: "test".to_string(),
            name: "Test V1".to_string(),
            cost: 10,
            requires: vec![],
        });
        let replaced = reg.register(TechDef {
            id: "test".to_string(),
            name: "Test V2".to_string(),
            cost: 20,
            requires: vec![],
        });
        assert!(!replaced);
        assert_eq!(reg.get("test").unwrap().name, "Test V2");
        assert_eq!(reg.all().len(), 1);
    }

    #[test]
    fn test_all() {
        let mut reg = TechRegistry::new();
        reg.register(TechDef {
            id: "a".to_string(),
            name: "A".to_string(),
            cost: 10,
            requires: vec![],
        });
        reg.register(TechDef {
            id: "b".to_string(),
            name: "B".to_string(),
            cost: 20,
            requires: vec!["a".to_string()],
        });
        assert_eq!(reg.all().len(), 2);
    }

    #[test]
    fn test_tech_with_prerequisites() {
        let mut reg = TechRegistry::new();
        reg.register(TechDef {
            id: "alphabet".to_string(),
            name: "Alphabet".to_string(),
            cost: 40,
            requires: vec![],
        });
        reg.register(TechDef {
            id: "writing".to_string(),
            name: "Writing".to_string(),
            cost: 60,
            requires: vec!["alphabet".to_string()],
        });
        let writing = reg.get("writing").unwrap();
        assert_eq!(writing.requires, vec!["alphabet".to_string()]);
    }
}
