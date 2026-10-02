use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Definition of an action that units can perform (e.g. "build_city").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    pub id: String,
    pub name: String,
    pub hotkey: Option<String>,
    /// Number of turns to complete. 0 = instant.
    pub turns: i32,
    /// Whether the unit is consumed (destroyed) after the action completes.
    pub consumes_unit: bool,
    /// Optional INI animation key to play when the action starts (e.g. "ROAD", "MINE").
    pub animation_name: Option<String>,
    /// Optional icon position (col, row) in the UI atlas grid.
    pub icon_atlas_pos: Option<(i32, i32)>,
}

/// Registry of all defined actions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionRegistry {
    actions: Vec<ActionDef>,
    by_id: HashMap<String, usize>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new action definition. Returns true if newly registered,
    /// false if an action with that id already existed (and was replaced).
    pub fn register(&mut self, action: ActionDef) -> bool {
        if let Some(&idx) = self.by_id.get(&action.id) {
            self.actions[idx] = action;
            false
        } else {
            let idx = self.actions.len();
            self.by_id.insert(action.id.clone(), idx);
            self.actions.push(action);
            true
        }
    }

    /// Looks up an action by its id.
    pub fn get(&self, id: &str) -> Option<&ActionDef> {
        self.by_id.get(id).map(|&idx| &self.actions[idx])
    }

    /// Returns all registered actions.
    pub fn get_all(&self) -> &[ActionDef] {
        &self.actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut reg = ActionRegistry::new();
        let action = ActionDef {
            id: "build_city".to_string(),
            name: "Build City".to_string(),
            hotkey: Some("B".to_string()),
            turns: 0,
            consumes_unit: true,
            animation_name: None,
            icon_atlas_pos: None,
        };
        assert!(reg.register(action));
        let a = reg.get("build_city").unwrap();
        assert_eq!(a.name, "Build City");
        assert_eq!(a.turns, 0);
        assert!(a.consumes_unit);
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let reg = ActionRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_register_replaces_existing() {
        let mut reg = ActionRegistry::new();
        reg.register(ActionDef {
            id: "test".to_string(),
            name: "Test V1".to_string(),
            hotkey: None,
            turns: 0,
            consumes_unit: false,
            animation_name: None,
            icon_atlas_pos: None,
        });
        let replaced = reg.register(ActionDef {
            id: "test".to_string(),
            name: "Test V2".to_string(),
            hotkey: None,
            turns: 1,
            consumes_unit: true,
            animation_name: None,
            icon_atlas_pos: None,
        });
        assert!(!replaced);
        assert_eq!(reg.get("test").unwrap().name, "Test V2");
        assert_eq!(reg.get_all().len(), 1);
    }

    #[test]
    fn test_get_all() {
        let mut reg = ActionRegistry::new();
        reg.register(ActionDef {
            id: "a".to_string(),
            name: "A".to_string(),
            hotkey: None,
            turns: 0,
            consumes_unit: false,
            animation_name: None,
            icon_atlas_pos: None,
        });
        reg.register(ActionDef {
            id: "b".to_string(),
            name: "B".to_string(),
            hotkey: None,
            turns: 1,
            consumes_unit: true,
            animation_name: None,
            icon_atlas_pos: None,
        });
        assert_eq!(reg.get_all().len(), 2);
    }
}
