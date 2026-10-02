use serde::{Deserialize, Serialize};

/// Identifies a player. Supports up to 255 players (u8::MAX is reserved as a sentinel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

/// A tile coordinate on the map grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
}

/// Identifies a unit type in the `UnitTypeRegistry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitTypeId(pub u16);

/// Identifies a resource type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u16);

/// Identifies an improvement type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImprovementId(pub u16);

/// The direction a unit is facing (last direction of travel).
///
/// Used to select the correct sprite column from FLIC animation atlases.
/// FLIC atlas columns: 0=SW, 1=S, 2=SE, 3=E, 4=NE, 5=N, 6=NW, 7=W.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Direction {
    N,
    NE,
    E,
    #[default]
    SE,
    S,
    SW,
    W,
    NW,
}

impl Direction {
    /// Maps a logical tile delta (dx, dy) to a direction.
    ///
    /// The isometric coordinate system maps logical deltas to screen compass
    /// directions as follows:
    /// - (0,-1) → NE, (+1,-1) → E, (+1,0) → SE, (+1,+1) → S
    /// - (0,+1) → SW, (-1,+1) → W, (-1,0) → NW, (-1,-1) → N
    ///
    /// Returns `None` if dx and dy are both zero.
    pub fn from_delta(dx: i32, dy: i32) -> Option<Direction> {
        match (dx.signum(), dy.signum()) {
            (0, -1) => Some(Direction::NE),
            (1, -1) => Some(Direction::E),
            (1, 0) => Some(Direction::SE),
            (1, 1) => Some(Direction::S),
            (0, 1) => Some(Direction::SW),
            (-1, 1) => Some(Direction::W),
            (-1, 0) => Some(Direction::NW),
            (-1, -1) => Some(Direction::N),
            _ => None,
        }
    }

    /// Returns the FLIC atlas column index for this direction.
    ///
    /// FLIC columns: 0=SW, 1=S, 2=SE, 3=E, 4=NE, 5=N, 6=NW, 7=W.
    pub fn flic_index(self) -> u32 {
        match self {
            Direction::SW => 0,
            Direction::S => 1,
            Direction::SE => 2,
            Direction::E => 3,
            Direction::NE => 4,
            Direction::N => 5,
            Direction::NW => 6,
            Direction::W => 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_id_copy_and_roundtrip() {
        let p = PlayerId(3);
        let p2 = p;
        assert_eq!(p, p2);
        let json = serde_json::to_string(&p).unwrap();
        let back: PlayerId = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn test_tile_coord_copy_and_roundtrip() {
        let c = TileCoord { x: 10, y: 20 };
        let c2 = c;
        assert_eq!(c, c2);
        let json = serde_json::to_string(&c).unwrap();
        let back: TileCoord = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_unit_type_id_roundtrip() {
        let id = UnitTypeId(42);
        let json = serde_json::to_string(&id).unwrap();
        let back: UnitTypeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn test_resource_id_roundtrip() {
        let id = ResourceId(7);
        let json = serde_json::to_string(&id).unwrap();
        let back: ResourceId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn test_improvement_id_roundtrip() {
        let id = ImprovementId(99);
        let json = serde_json::to_string(&id).unwrap();
        let back: ImprovementId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn test_direction_default_is_se() {
        assert_eq!(Direction::default(), Direction::SE);
    }

    #[test]
    fn test_direction_from_delta_all_eight() {
        assert_eq!(Direction::from_delta(0, -1), Some(Direction::NE));
        assert_eq!(Direction::from_delta(1, -1), Some(Direction::E));
        assert_eq!(Direction::from_delta(1, 0), Some(Direction::SE));
        assert_eq!(Direction::from_delta(1, 1), Some(Direction::S));
        assert_eq!(Direction::from_delta(0, 1), Some(Direction::SW));
        assert_eq!(Direction::from_delta(-1, 1), Some(Direction::W));
        assert_eq!(Direction::from_delta(-1, 0), Some(Direction::NW));
        assert_eq!(Direction::from_delta(-1, -1), Some(Direction::N));
    }

    #[test]
    fn test_direction_from_delta_zero_is_none() {
        assert_eq!(Direction::from_delta(0, 0), None);
    }

    #[test]
    fn test_direction_from_delta_large_values_use_signum() {
        // Large deltas should use signum, so (5, -3) → (+1, -1) → E
        assert_eq!(Direction::from_delta(5, -3), Some(Direction::E));
        assert_eq!(Direction::from_delta(-10, 0), Some(Direction::NW));
    }

    #[test]
    fn test_direction_flic_index() {
        assert_eq!(Direction::SW.flic_index(), 0);
        assert_eq!(Direction::S.flic_index(), 1);
        assert_eq!(Direction::SE.flic_index(), 2);
        assert_eq!(Direction::E.flic_index(), 3);
        assert_eq!(Direction::NE.flic_index(), 4);
        assert_eq!(Direction::N.flic_index(), 5);
        assert_eq!(Direction::NW.flic_index(), 6);
        assert_eq!(Direction::W.flic_index(), 7);
    }

    #[test]
    fn test_direction_serialization_roundtrip() {
        for dir in [
            Direction::N,
            Direction::NE,
            Direction::E,
            Direction::SE,
            Direction::S,
            Direction::SW,
            Direction::W,
            Direction::NW,
        ] {
            let json = serde_json::to_string(&dir).unwrap();
            let back: Direction = serde_json::from_str(&json).unwrap();
            assert_eq!(dir, back);
        }
    }
}
