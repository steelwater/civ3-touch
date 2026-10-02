use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A value stored in a dynamic column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrValue {
    Int(i32),
    Float(f32),
    Bool(bool),
}

/// The type of a dynamic column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColType {
    Int,
    Float,
    Bool,
}

/// Metadata for a registered dynamic column.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColumnDef {
    col_type: ColType,
    col_index: usize,
    default: AttrValue,
}

/// Dynamic column storage for mod-defined entity attributes.
///
/// Allows mods to register named columns at runtime. Each column is stored
/// as a typed Vec, with a schema mapping names to column metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicColumns {
    schema: HashMap<String, ColumnDef>,
    int_cols: Vec<Vec<i32>>,
    float_cols: Vec<Vec<f32>>,
    bool_cols: Vec<Vec<bool>>,
    len: usize,
}

/// Errors that can occur when working with dynamic columns.
#[derive(Debug, thiserror::Error)]
pub enum DynColError {
    #[error("column '{0}' already exists")]
    AlreadyExists(String),
    #[error("column '{0}' not found")]
    NotFound(String),
    #[error("type mismatch for column '{name}': expected {expected:?}, got {got:?}")]
    TypeMismatch {
        name: String,
        expected: ColType,
        got: ColType,
    },
}

impl DynamicColumns {
    /// Creates a new empty dynamic column store.
    pub fn new() -> Self {
        DynamicColumns {
            schema: HashMap::new(),
            int_cols: Vec::new(),
            float_cols: Vec::new(),
            bool_cols: Vec::new(),
            len: 0,
        }
    }

    /// Registers a new named column with the given type and default value.
    pub fn register(
        &mut self,
        name: &str,
        col_type: ColType,
        default: AttrValue,
    ) -> Result<(), DynColError> {
        if self.schema.contains_key(name) {
            return Err(DynColError::AlreadyExists(name.to_string()));
        }

        let col_index = match col_type {
            ColType::Int => {
                let d = match &default {
                    AttrValue::Int(v) => *v,
                    _ => {
                        return Err(DynColError::TypeMismatch {
                            name: name.to_string(),
                            expected: ColType::Int,
                            got: attr_value_type(&default),
                        })
                    }
                };
                let idx = self.int_cols.len();
                self.int_cols.push(vec![d; self.len]);
                idx
            }
            ColType::Float => {
                let d = match &default {
                    AttrValue::Float(v) => *v,
                    _ => {
                        return Err(DynColError::TypeMismatch {
                            name: name.to_string(),
                            expected: ColType::Float,
                            got: attr_value_type(&default),
                        })
                    }
                };
                let idx = self.float_cols.len();
                self.float_cols.push(vec![d; self.len]);
                idx
            }
            ColType::Bool => {
                let d = match &default {
                    AttrValue::Bool(v) => *v,
                    _ => {
                        return Err(DynColError::TypeMismatch {
                            name: name.to_string(),
                            expected: ColType::Bool,
                            got: attr_value_type(&default),
                        })
                    }
                };
                let idx = self.bool_cols.len();
                self.bool_cols.push(vec![d; self.len]);
                idx
            }
        };

        self.schema.insert(
            name.to_string(),
            ColumnDef {
                col_type,
                col_index,
                default,
            },
        );
        Ok(())
    }

    /// Gets a value from a named column at the given entity index.
    /// Returns `None` if the column doesn't exist.
    pub fn get(&self, name: &str, entity_idx: usize) -> Option<AttrValue> {
        let def = self.schema.get(name)?;
        Some(match def.col_type {
            ColType::Int => AttrValue::Int(self.int_cols[def.col_index][entity_idx]),
            ColType::Float => AttrValue::Float(self.float_cols[def.col_index][entity_idx]),
            ColType::Bool => AttrValue::Bool(self.bool_cols[def.col_index][entity_idx]),
        })
    }

    /// Sets a value in a named column at the given entity index.
    pub fn set(
        &mut self,
        name: &str,
        entity_idx: usize,
        value: AttrValue,
    ) -> Result<(), DynColError> {
        let def = self
            .schema
            .get(name)
            .ok_or_else(|| DynColError::NotFound(name.to_string()))?;
        match (&def.col_type, &value) {
            (ColType::Int, AttrValue::Int(v)) => {
                self.int_cols[def.col_index][entity_idx] = *v;
            }
            (ColType::Float, AttrValue::Float(v)) => {
                self.float_cols[def.col_index][entity_idx] = *v;
            }
            (ColType::Bool, AttrValue::Bool(v)) => {
                self.bool_cols[def.col_index][entity_idx] = *v;
            }
            _ => {
                return Err(DynColError::TypeMismatch {
                    name: name.to_string(),
                    expected: def.col_type,
                    got: attr_value_type(&value),
                });
            }
        }
        Ok(())
    }

    /// Extends all columns to `new_len` entries, filling with defaults.
    pub fn grow(&mut self, new_len: usize) {
        if new_len <= self.len {
            return;
        }
        for (name, def) in &self.schema {
            let _ = name;
            match def.col_type {
                ColType::Int => {
                    let d = match &def.default {
                        AttrValue::Int(v) => *v,
                        _ => unreachable!(),
                    };
                    self.int_cols[def.col_index].resize(new_len, d);
                }
                ColType::Float => {
                    let d = match &def.default {
                        AttrValue::Float(v) => *v,
                        _ => unreachable!(),
                    };
                    self.float_cols[def.col_index].resize(new_len, d);
                }
                ColType::Bool => {
                    let d = match &def.default {
                        AttrValue::Bool(v) => *v,
                        _ => unreachable!(),
                    };
                    self.bool_cols[def.col_index].resize(new_len, d);
                }
            }
        }
        self.len = new_len;
    }
}

impl Default for DynamicColumns {
    fn default() -> Self {
        Self::new()
    }
}

fn attr_value_type(val: &AttrValue) -> ColType {
    match val {
        AttrValue::Int(_) => ColType::Int,
        AttrValue::Float(_) => ColType::Float,
        AttrValue::Bool(_) => ColType::Bool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_int_column_set_and_get() {
        let mut dc = DynamicColumns::new();
        dc.register("cargo", ColType::Int, AttrValue::Int(0))
            .unwrap();
        dc.grow(5);
        dc.set("cargo", 2, AttrValue::Int(42)).unwrap();
        assert_eq!(dc.get("cargo", 2), Some(AttrValue::Int(42)));
        assert_eq!(dc.get("cargo", 0), Some(AttrValue::Int(0)));
    }

    #[test]
    fn test_register_multiple_column_types() {
        let mut dc = DynamicColumns::new();
        dc.register("strength", ColType::Int, AttrValue::Int(10))
            .unwrap();
        dc.register("accuracy", ColType::Float, AttrValue::Float(0.5))
            .unwrap();
        dc.register("veteran", ColType::Bool, AttrValue::Bool(false))
            .unwrap();
        dc.grow(3);

        dc.set("strength", 0, AttrValue::Int(20)).unwrap();
        dc.set("accuracy", 1, AttrValue::Float(0.9)).unwrap();
        dc.set("veteran", 2, AttrValue::Bool(true)).unwrap();

        assert_eq!(dc.get("strength", 0), Some(AttrValue::Int(20)));
        assert_eq!(dc.get("accuracy", 1), Some(AttrValue::Float(0.9)));
        assert_eq!(dc.get("veteran", 2), Some(AttrValue::Bool(true)));
    }

    #[test]
    fn test_grow_fills_with_defaults() {
        let mut dc = DynamicColumns::new();
        dc.register("hp_bonus", ColType::Int, AttrValue::Int(5))
            .unwrap();
        dc.grow(3);
        assert_eq!(dc.get("hp_bonus", 0), Some(AttrValue::Int(5)));
        assert_eq!(dc.get("hp_bonus", 2), Some(AttrValue::Int(5)));

        dc.set("hp_bonus", 0, AttrValue::Int(99)).unwrap();
        dc.grow(5);
        assert_eq!(dc.get("hp_bonus", 0), Some(AttrValue::Int(99)));
        assert_eq!(dc.get("hp_bonus", 4), Some(AttrValue::Int(5)));
    }

    #[test]
    fn test_duplicate_register_returns_error() {
        let mut dc = DynamicColumns::new();
        dc.register("x", ColType::Int, AttrValue::Int(0)).unwrap();
        let err = dc.register("x", ColType::Int, AttrValue::Int(0));
        assert!(matches!(err, Err(DynColError::AlreadyExists(_))));
    }

    #[test]
    fn test_get_unregistered_returns_none() {
        let dc = DynamicColumns::new();
        assert_eq!(dc.get("nonexistent", 0), None);
    }

    #[test]
    fn test_type_mismatch_on_set() {
        let mut dc = DynamicColumns::new();
        dc.register("count", ColType::Int, AttrValue::Int(0))
            .unwrap();
        dc.grow(1);
        let err = dc.set("count", 0, AttrValue::Bool(true));
        assert!(matches!(err, Err(DynColError::TypeMismatch { .. })));
    }
}
