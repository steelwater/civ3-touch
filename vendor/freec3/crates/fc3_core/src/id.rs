use serde::{Deserialize, Serialize};

/// A generational index for entity storage.
///
/// Uses `index` to locate the entity in arrays and `generation` to detect
/// stale handles after an entity has been destroyed and its slot reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenId {
    pub index: u32,
    pub generation: u32,
}

impl GenId {
    /// Sentinel value representing an invalid or uninitialized handle.
    pub const INVALID: GenId = GenId {
        index: u32::MAX,
        generation: u32::MAX,
    };
}

/// A handle to a unit in the `UnitStore`.
pub type UnitId = GenId;

/// A handle to a city in the `CityStore`.
pub type CityId = GenId;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_index_different_generation_not_equal() {
        let a = GenId {
            index: 0,
            generation: 0,
        };
        let b = GenId {
            index: 0,
            generation: 1,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn test_invalid_sentinel() {
        let id = GenId::INVALID;
        assert_eq!(id.index, u32::MAX);
        assert_eq!(id.generation, u32::MAX);
    }

    #[test]
    fn test_genid_serialization_roundtrip() {
        let id = GenId {
            index: 42,
            generation: 7,
        };
        let json = serde_json::to_string(&id).unwrap();
        let back: GenId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
