use serde::{Deserialize, Serialize};

use crate::id::CityId;
use crate::types::{ImprovementId, PlayerId, ResourceId, TileCoord};

/// Terrain types for map tiles.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Terrain {
    Grassland,
    Plains,
    Desert,
    Tundra,
    Ocean,
    Coast,
    Mountain,
    Hill,
    Ice,
}

/// Vegetation overlay on a tile. Forest and jungle are separate from terrain —
/// clearing vegetation reveals the base terrain underneath.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Vegetation {
    None,
    Forest,
    Jungle,
}

/// Per-player tile visibility level.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    /// Never seen by this player.
    Unseen,
    /// Previously visible but no longer in sight range.
    Revealed,
    /// Currently within sight range of one of the player's units.
    Visible,
}

/// Structure-of-Arrays storage for all map tiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileStore {
    pub width: u32,
    pub height: u32,
    pub wrap_x: bool,
    pub wrap_y: bool,
    pub terrain: Vec<Terrain>,
    pub vegetation: Vec<Vegetation>,
    pub river_edges: Vec<u8>,
    pub resource: Vec<Option<ResourceId>>,
    pub improvement: Vec<Option<ImprovementId>>,
    pub road_level: Vec<u8>,
    pub owner: Vec<Option<PlayerId>>,
    /// Which city (if any) is working each tile.
    pub worked_by: Vec<Option<CityId>>,
    pub visibility: Vec<Visibility>,
    num_players: u8,
}

impl TileStore {
    /// Creates a new tile store with the given dimensions, filling all tiles
    /// with `default_terrain`.
    pub fn new(
        width: u32,
        height: u32,
        wrap_x: bool,
        wrap_y: bool,
        default_terrain: Terrain,
        num_players: u8,
    ) -> Self {
        let len = (width * height) as usize;
        let vis_len = len * num_players as usize;
        TileStore {
            width,
            height,
            wrap_x,
            wrap_y,
            terrain: vec![default_terrain; len],
            vegetation: vec![Vegetation::None; len],
            river_edges: vec![0; len],
            resource: vec![None; len],
            improvement: vec![None; len],
            road_level: vec![0; len],
            owner: vec![None; len],
            worked_by: vec![None; len],
            visibility: vec![Visibility::Unseen; vis_len],
            num_players,
        }
    }

    /// Converts (x, y) coordinates to a flat array index, wrapping x if
    /// `wrap_x` is enabled.
    #[inline]
    pub fn idx(&self, x: u32, y: u32) -> usize {
        let x = if self.wrap_x { x % self.width } else { x };
        let y = if self.wrap_y { y % self.height } else { y };
        (y * self.width + x) as usize
    }

    /// Converts a flat index back to (x, y) coordinates.
    #[inline]
    pub fn coords(&self, idx: usize) -> TileCoord {
        let idx = idx as u32;
        TileCoord {
            x: idx % self.width,
            y: idx / self.width,
        }
    }

    /// Returns true if the given coordinates are within the map bounds.
    /// When `wrap_x` is true, any x value is in bounds (it wraps).
    pub fn in_bounds(&self, x: u32, y: u32) -> bool {
        let x_ok = self.wrap_x || x < self.width;
        let y_ok = self.wrap_y || y < self.height;
        x_ok && y_ok
    }

    /// Returns the valid neighboring tile coordinates (8-directional adjacency).
    pub fn neighbors(&self, x: u32, y: u32) -> Vec<TileCoord> {
        let mut result = Vec::with_capacity(8);
        for dy in [-1i64, 0, 1] {
            for dx in [-1i64, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let ny = y as i64 + dy;
                let actual_y = if self.wrap_y {
                    ny.rem_euclid(self.height as i64) as u32
                } else {
                    if ny < 0 || ny >= self.height as i64 {
                        continue;
                    }
                    ny as u32
                };
                let nx = x as i64 + dx;
                let actual_x = if self.wrap_x {
                    nx.rem_euclid(self.width as i64) as u32
                } else {
                    if nx < 0 || nx >= self.width as i64 {
                        continue;
                    }
                    nx as u32
                };
                result.push(TileCoord {
                    x: actual_x,
                    y: actual_y,
                });
            }
        }
        result
    }

    /// Returns the flat index into the per-player visibility array.
    #[inline]
    fn vis_idx(&self, player: PlayerId, x: u32, y: u32) -> usize {
        let tile_idx = self.idx(x, y);
        player.0 as usize * (self.width * self.height) as usize + tile_idx
    }

    /// Gets the visibility of a tile for a given player.
    pub fn get_visibility(&self, player: PlayerId, x: u32, y: u32) -> Visibility {
        self.visibility[self.vis_idx(player, x, y)]
    }

    /// Sets the visibility of a tile for a given player.
    pub fn set_visibility(&mut self, player: PlayerId, x: u32, y: u32, vis: Visibility) {
        let idx = self.vis_idx(player, x, y);
        self.visibility[idx] = vis;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idx_and_coords_are_inverses() {
        let store = TileStore::new(10, 10, false, false, Terrain::Grassland, 2);
        for y in 0..10 {
            for x in 0..10 {
                let idx = store.idx(x, y);
                let coord = store.coords(idx);
                assert_eq!(coord, TileCoord { x, y });
            }
        }
    }

    #[test]
    fn test_idx_wraps_x() {
        let store = TileStore::new(10, 10, true, false, Terrain::Grassland, 1);
        assert_eq!(store.idx(10, 0), store.idx(0, 0));
        assert_eq!(store.idx(15, 3), store.idx(5, 3));
    }

    #[test]
    fn test_in_bounds() {
        let store = TileStore::new(10, 10, false, false, Terrain::Grassland, 1);
        assert!(store.in_bounds(0, 0));
        assert!(store.in_bounds(9, 9));
        assert!(!store.in_bounds(10, 0));
        assert!(!store.in_bounds(0, 10));
    }

    #[test]
    fn test_in_bounds_wrap_x() {
        let store = TileStore::new(10, 10, true, false, Terrain::Grassland, 1);
        assert!(store.in_bounds(100, 0)); // x wraps, so always in bounds
        assert!(!store.in_bounds(0, 10)); // y never wraps
    }

    #[test]
    fn test_neighbors_corner_no_wrap() {
        let store = TileStore::new(10, 10, false, false, Terrain::Grassland, 1);
        let n = store.neighbors(0, 0);
        assert_eq!(n.len(), 3);
        assert!(n.contains(&TileCoord { x: 1, y: 0 }));
        assert!(n.contains(&TileCoord { x: 0, y: 1 }));
        assert!(n.contains(&TileCoord { x: 1, y: 1 }));
    }

    #[test]
    fn test_neighbors_edge_no_wrap() {
        let store = TileStore::new(10, 10, false, false, Terrain::Grassland, 1);
        let n = store.neighbors(5, 0);
        assert_eq!(n.len(), 5); // top edge, not corner
    }

    #[test]
    fn test_neighbors_center() {
        let store = TileStore::new(10, 10, false, false, Terrain::Grassland, 1);
        let n = store.neighbors(5, 5);
        assert_eq!(n.len(), 8);
    }

    #[test]
    fn test_neighbors_wrap_x() {
        let store = TileStore::new(10, 10, true, false, Terrain::Grassland, 1);
        let n = store.neighbors(0, 5);
        assert_eq!(n.len(), 8);
        // Left neighbors should wrap to x=9
        assert!(n.contains(&TileCoord { x: 9, y: 4 }));
        assert!(n.contains(&TileCoord { x: 9, y: 5 }));
        assert!(n.contains(&TileCoord { x: 9, y: 6 }));
    }

    #[test]
    fn test_new_creates_correct_sizes() {
        let store = TileStore::new(5, 8, false, false, Terrain::Ocean, 3);
        assert_eq!(store.terrain.len(), 40);
        assert_eq!(store.visibility.len(), 120); // 40 tiles * 3 players
        assert!(store.terrain.iter().all(|t| *t == Terrain::Ocean));
    }

    // --- Visibility tests ---

    #[test]
    fn test_default_visibility_is_unseen() {
        let store = TileStore::new(10, 10, false, false, Terrain::Grassland, 2);
        for p in 0..2 {
            for y in 0..10 {
                for x in 0..10 {
                    assert_eq!(store.get_visibility(PlayerId(p), x, y), Visibility::Unseen);
                }
            }
        }
    }

    #[test]
    fn test_visibility_per_player_isolation() {
        let mut store = TileStore::new(10, 10, false, false, Terrain::Grassland, 2);
        store.set_visibility(PlayerId(0), 3, 3, Visibility::Visible);
        assert_eq!(store.get_visibility(PlayerId(0), 3, 3), Visibility::Visible);
        assert_eq!(store.get_visibility(PlayerId(1), 3, 3), Visibility::Unseen);
    }

    #[test]
    fn test_visibility_roundtrip_serialization() {
        let mut store = TileStore::new(4, 4, false, false, Terrain::Grassland, 2);
        store.set_visibility(PlayerId(0), 1, 1, Visibility::Visible);
        store.set_visibility(PlayerId(1), 2, 2, Visibility::Revealed);

        let json = serde_json::to_string(&store).unwrap();
        let back: TileStore = serde_json::from_str(&json).unwrap();

        assert_eq!(back.get_visibility(PlayerId(0), 1, 1), Visibility::Visible);
        assert_eq!(back.get_visibility(PlayerId(1), 2, 2), Visibility::Revealed);
        assert_eq!(back.get_visibility(PlayerId(0), 0, 0), Visibility::Unseen);
    }

    // --- wrap_y tests ---

    #[test]
    fn test_idx_wraps_y() {
        let store = TileStore::new(10, 10, false, true, Terrain::Grassland, 1);
        assert_eq!(store.idx(5, 10), store.idx(5, 0));
        assert_eq!(store.idx(3, 15), store.idx(3, 5));
    }

    #[test]
    fn test_neighbors_wrap_y() {
        let store = TileStore::new(10, 10, false, true, Terrain::Grassland, 1);
        let n = store.neighbors(5, 0);
        assert_eq!(n.len(), 8);
        // Top neighbors should wrap to y=9
        assert!(n.contains(&TileCoord { x: 4, y: 9 }));
        assert!(n.contains(&TileCoord { x: 5, y: 9 }));
        assert!(n.contains(&TileCoord { x: 6, y: 9 }));
    }
}
