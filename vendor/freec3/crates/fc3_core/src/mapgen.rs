use noise::{NoiseFn, Perlin};
use rand::Rng;

use crate::tile::{Terrain, TileStore, Vegetation};
use crate::types::TileCoord;

/// Generates a map with noise-based terrain.
///
/// Uses multiple noise layers:
/// - Elevation (continent shapes + local variation)
/// - Moisture (for biome variation)
/// - Vegetation (forest/jungle placement)
pub fn generate_map(
    width: u32,
    height: u32,
    wrap_x: bool,
    wrap_y: bool,
    num_players: u8,
    seed: u64,
) -> TileStore {
    let mut tiles = TileStore::new(
        width,
        height,
        wrap_x,
        wrap_y,
        Terrain::Grassland,
        num_players,
    );

    // Create noise generators with different seeds derived from the main seed
    let elevation_noise = Perlin::new((seed & 0xFFFFFFFF) as u32);
    let continent_noise = Perlin::new(((seed >> 16) & 0xFFFFFFFF) as u32);
    let moisture_noise = Perlin::new(((seed >> 32) & 0xFFFFFFFF) as u32);
    let vegetation_noise = Perlin::new(((seed >> 48) as u32).wrapping_add(1));

    let w = width as f64;
    let h = height as f64;

    for y in 0..height {
        for x in 0..width {
            let idx = tiles.idx(x, y);

            let nx = x as f64 / w;
            let ny = y as f64 / h;

            // Elevation: local noise * continent shapes
            let local_elev = elevation_noise.get([nx * 6.0, ny * 6.0]);
            let continent = continent_noise.get([nx * 2.5, ny * 2.5]);
            let elevation = (local_elev * 0.4 + continent * 0.6).clamp(-1.0, 1.0);

            // Moisture for biome variation
            let moisture = moisture_noise.get([nx * 4.0, ny * 4.0]);

            // Vegetation density
            let vegetation = vegetation_noise.get([nx * 5.0, ny * 5.0]);

            // Latitude: 0.0 at poles, 1.0 at equator
            let latitude = if wrap_y {
                // With Y wrapping, poles are on diagonals: (x+y) % H == 0 (north)
                // and (x+y) % H == H-1 (south). Distance from nearest pole diagonal
                // determines latitude.
                let s = (x + y) % height;
                let dist_from_pole = s.min(height - 1 - s) as f64;
                dist_from_pole / (height as f64 / 2.0)
            } else {
                1.0 - (2.0 * ny - 1.0).abs()
            };

            let (terrain, veg) = assign_terrain(elevation, moisture, vegetation, latitude);
            tiles.terrain[idx] = terrain;
            tiles.vegetation[idx] = veg;
        }
    }

    // Place Ice on pole diagonals when wrap_y is enabled
    if wrap_y {
        for y in 0..height {
            for x in 0..width {
                let sum = (x + y) % height;
                if sum == 0 || sum == height - 1 {
                    let idx = tiles.idx(x, y);
                    tiles.terrain[idx] = Terrain::Ice;
                    tiles.vegetation[idx] = Vegetation::None;
                }
            }
        }
    }

    tiles
}

/// Assigns terrain and vegetation based on elevation, moisture, vegetation noise, and latitude.
fn assign_terrain(
    elevation: f64,
    moisture: f64,
    vegetation: f64,
    latitude: f64,
) -> (Terrain, Vegetation) {
    // Deep water
    if elevation < -0.3 {
        return (Terrain::Ocean, Vegetation::None);
    }
    // Shallow water
    if elevation < -0.1 {
        return (Terrain::Coast, Vegetation::None);
    }
    // Mountains
    if elevation > 0.6 {
        return (Terrain::Mountain, Vegetation::None);
    }
    // Hills
    if elevation > 0.3 {
        // Check for forest overlay on hills
        if vegetation > 0.3 && latitude > 0.25 && latitude < 0.85 {
            return (Terrain::Hill, Vegetation::Forest);
        }
        return (Terrain::Hill, Vegetation::None);
    }

    // Lowlands: assign by latitude
    let base = assign_lowland_biome(latitude, moisture);

    // Vegetation overlay (only on grassland/plains)
    if matches!(base, Terrain::Grassland | Terrain::Plains) && vegetation > 0.3 {
        if latitude > 0.6 {
            // Tropical zone -> jungle
            return (base, Vegetation::Jungle);
        }
        if latitude > 0.25 {
            // Temperate zone -> forest
            return (base, Vegetation::Forest);
        }
    }

    (base, Vegetation::None)
}

/// Assigns lowland biome based on latitude and moisture.
fn assign_lowland_biome(latitude: f64, moisture: f64) -> Terrain {
    // latitude: 0.0 = poles, 1.0 = equator
    if latitude < 0.15 {
        // Polar regions
        Terrain::Tundra
    } else if latitude < 0.25 {
        // Sub-polar
        Terrain::Plains
    } else if latitude > 0.7 && moisture < -0.1 {
        // Near equator, dry -> desert
        Terrain::Desert
    } else if latitude < 0.4 {
        // Temperate-cool -> plains with some grassland
        if moisture > 0.1 {
            Terrain::Grassland
        } else {
            Terrain::Plains
        }
    } else {
        // Middle latitudes -> grassland
        Terrain::Grassland
    }
}

/// Computes starting positions for players, spread across the map on land tiles.
pub fn compute_starting_positions(
    tiles: &TileStore,
    num_players: u8,
    rng: &mut impl Rng,
) -> Vec<TileCoord> {
    let min_distance = (std::cmp::max(tiles.width, tiles.height) / num_players as u32).max(8);
    let mut positions = Vec::new();

    // Collect candidate land tiles (prefer grassland/plains)
    let mut preferred = Vec::new();
    let mut acceptable = Vec::new();
    for y in 0..tiles.height {
        for x in 0..tiles.width {
            let idx = tiles.idx(x, y);
            match tiles.terrain[idx] {
                Terrain::Grassland | Terrain::Plains => preferred.push(TileCoord { x, y }),
                Terrain::Desert | Terrain::Tundra | Terrain::Hill => {
                    acceptable.push(TileCoord { x, y })
                }
                Terrain::Ocean | Terrain::Coast | Terrain::Mountain | Terrain::Ice => {}
            }
        }
    }

    // Try preferred tiles first, then fall back to acceptable
    let mut candidates: Vec<TileCoord> = preferred;
    candidates.extend(acceptable);

    // If no land tiles at all, fall back to any tile
    if candidates.is_empty() {
        for y in 0..tiles.height {
            for x in 0..tiles.width {
                candidates.push(TileCoord { x, y });
            }
        }
    }

    let mut current_min_dist = min_distance;
    let mut attempts = 0;
    let max_attempts = 1000;

    while positions.len() < num_players as usize && attempts < max_attempts {
        let idx = rng.gen_range(0..candidates.len());
        let pos = candidates[idx];

        let far_enough = positions.iter().all(|p: &TileCoord| {
            chebyshev_distance(
                *p,
                pos,
                tiles.width,
                tiles.wrap_x,
                tiles.height,
                tiles.wrap_y,
            ) >= current_min_dist
        });

        if far_enough {
            positions.push(pos);
        }

        attempts += 1;

        // Relax distance constraint if we're struggling
        if attempts % 200 == 0 && current_min_dist > 3 {
            current_min_dist = current_min_dist * 2 / 3;
        }
    }

    // If we still don't have enough positions, just pick any remaining candidates
    while positions.len() < num_players as usize {
        let idx = rng.gen_range(0..candidates.len());
        positions.push(candidates[idx]);
    }

    positions
}

/// Chebyshev distance between two tile coordinates, accounting for wrapping.
fn chebyshev_distance(
    a: TileCoord,
    b: TileCoord,
    map_width: u32,
    wrap_x: bool,
    map_height: u32,
    wrap_y: bool,
) -> u32 {
    let dx = if wrap_x {
        let raw = (a.x as i64 - b.x as i64).unsigned_abs() as u32;
        raw.min(map_width - raw)
    } else {
        (a.x as i64 - b.x as i64).unsigned_abs() as u32
    };
    let dy = if wrap_y {
        let raw = (a.y as i64 - b.y as i64).unsigned_abs() as u32;
        raw.min(map_height - raw)
    } else {
        (a.y as i64 - b.y as i64).unsigned_abs() as u32
    };
    dx.max(dy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_generate_map_correct_dimensions() {
        let tiles = generate_map(40, 25, true, false, 2, 42);
        assert_eq!(tiles.width, 40);
        assert_eq!(tiles.height, 25);
        assert_eq!(tiles.terrain.len(), 40 * 25);
    }

    #[test]
    fn test_generate_map_deterministic() {
        let tiles1 = generate_map(40, 25, true, false, 2, 42);
        let tiles2 = generate_map(40, 25, true, false, 2, 42);
        assert_eq!(tiles1.terrain, tiles2.terrain);
        assert_eq!(tiles1.vegetation, tiles2.vegetation);
    }

    #[test]
    fn test_generate_map_has_ocean_and_land() {
        let tiles = generate_map(40, 25, true, false, 2, 42);
        let total = tiles.terrain.len();
        let ocean_count = tiles
            .terrain
            .iter()
            .filter(|t| matches!(t, Terrain::Ocean | Terrain::Coast))
            .count();
        let land_count = total - ocean_count;

        let ocean_pct = ocean_count as f64 / total as f64;
        let land_pct = land_count as f64 / total as f64;
        assert!(
            ocean_pct > 0.1,
            "map should have at least 10% water, got {:.1}%",
            ocean_pct * 100.0
        );
        assert!(
            land_pct > 0.3,
            "map should have at least 30% land, got {:.1}%",
            land_pct * 100.0
        );
    }

    #[test]
    fn test_generate_map_has_terrain_variety() {
        let tiles = generate_map(40, 25, true, false, 2, 42);
        let mut terrain_types = std::collections::HashSet::new();
        for t in &tiles.terrain {
            terrain_types.insert(std::mem::discriminant(t));
        }
        assert!(
            terrain_types.len() >= 3,
            "map should have at least 3 terrain types, got {}",
            terrain_types.len()
        );
    }

    #[test]
    fn test_generate_map_mountains_reasonable() {
        let tiles = generate_map(40, 25, true, false, 2, 42);
        let total = tiles.terrain.len();
        let mountain_count = tiles
            .terrain
            .iter()
            .filter(|t| matches!(t, Terrain::Mountain))
            .count();
        let mountain_pct = mountain_count as f64 / total as f64;
        assert!(
            mountain_pct < 0.15,
            "mountains should be < 15% of tiles, got {:.1}%",
            mountain_pct * 100.0
        );
    }

    #[test]
    fn test_generate_map_different_seeds_different_maps() {
        let tiles1 = generate_map(40, 25, true, false, 2, 42);
        let tiles2 = generate_map(40, 25, true, false, 2, 99);
        assert_ne!(
            tiles1.terrain, tiles2.terrain,
            "different seeds should make different maps"
        );
    }

    // ── Starting position tests ─────────────────────────────────────

    #[test]
    fn test_compute_starting_positions_count() {
        let tiles = TileStore::new(40, 25, true, false, Terrain::Grassland, 2);
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let positions = compute_starting_positions(&tiles, 2, &mut rng);
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn test_starting_positions_on_land() {
        let tiles = generate_map(40, 25, true, false, 2, 42);
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let positions = compute_starting_positions(&tiles, 2, &mut rng);
        for pos in &positions {
            let idx = tiles.idx(pos.x, pos.y);
            assert!(
                !matches!(
                    tiles.terrain[idx],
                    Terrain::Ocean | Terrain::Coast | Terrain::Mountain
                ),
                "starting position ({},{}) should be on land, got {:?}",
                pos.x,
                pos.y,
                tiles.terrain[idx]
            );
        }
    }

    #[test]
    fn test_starting_positions_separated() {
        let tiles = TileStore::new(40, 25, true, false, Terrain::Grassland, 2);
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let positions = compute_starting_positions(&tiles, 2, &mut rng);
        let dist = chebyshev_distance(
            positions[0],
            positions[1],
            tiles.width,
            tiles.wrap_x,
            tiles.height,
            tiles.wrap_y,
        );
        assert!(
            dist >= 8,
            "positions should be at least 8 apart, got {dist}"
        );
    }

    #[test]
    fn test_starting_positions_mostly_ocean_still_works() {
        let mut tiles = TileStore::new(20, 20, true, false, Terrain::Ocean, 2);
        // Put a small island
        for x in 5..8 {
            for y in 5..8 {
                let idx = tiles.idx(x, y);
                tiles.terrain[idx] = Terrain::Grassland;
            }
        }
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let positions = compute_starting_positions(&tiles, 2, &mut rng);
        assert_eq!(
            positions.len(),
            2,
            "should find 2 positions even on mostly-ocean map"
        );
    }

    #[test]
    fn test_starting_positions_deterministic() {
        let tiles = TileStore::new(40, 25, true, false, Terrain::Grassland, 2);
        let mut rng1 = ChaCha8Rng::seed_from_u64(42);
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);
        let pos1 = compute_starting_positions(&tiles, 2, &mut rng1);
        let pos2 = compute_starting_positions(&tiles, 2, &mut rng2);
        assert_eq!(pos1, pos2);
    }

    #[test]
    fn test_starting_positions_on_generated_map() {
        // Test on a real generated map with varied terrain
        let tiles = generate_map(40, 25, true, false, 2, 123);
        let mut rng = ChaCha8Rng::seed_from_u64(123);
        let positions = compute_starting_positions(&tiles, 2, &mut rng);
        assert_eq!(positions.len(), 2);
        for pos in &positions {
            let idx = tiles.idx(pos.x, pos.y);
            assert!(
                !matches!(
                    tiles.terrain[idx],
                    Terrain::Ocean | Terrain::Coast | Terrain::Mountain
                ),
                "start at ({},{}) should be on land",
                pos.x,
                pos.y
            );
        }
    }

    #[test]
    fn test_chebyshev_distance_basic() {
        let a = TileCoord { x: 0, y: 0 };
        let b = TileCoord { x: 3, y: 4 };
        assert_eq!(chebyshev_distance(a, b, 10, false, 10, false), 4);
    }

    #[test]
    fn test_chebyshev_distance_wrap() {
        let a = TileCoord { x: 0, y: 0 };
        let b = TileCoord { x: 9, y: 0 };
        assert_eq!(chebyshev_distance(a, b, 10, true, 10, false), 1);
        assert_eq!(chebyshev_distance(a, b, 10, false, 10, false), 9);
    }

    // ── wrap_y tests ────────────────────────────────────────────────

    #[test]
    fn test_ice_poles_on_wrap_y_map() {
        let tiles = generate_map(40, 32, true, true, 2, 42);
        let h = tiles.height;
        for y in 0..tiles.height {
            for x in 0..tiles.width {
                let sum = (x + y) % h;
                let idx = tiles.idx(x, y);
                if sum == 0 || sum == h - 1 {
                    assert_eq!(
                        tiles.terrain[idx],
                        Terrain::Ice,
                        "tile ({x},{y}) with (x+y)%H={sum} should be Ice"
                    );
                } else {
                    assert_ne!(
                        tiles.terrain[idx],
                        Terrain::Ice,
                        "tile ({x},{y}) with (x+y)%H={sum} should NOT be Ice"
                    );
                }
            }
        }
    }

    #[test]
    fn test_starting_positions_avoid_ice() {
        let tiles = generate_map(40, 32, true, true, 2, 42);
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let positions = compute_starting_positions(&tiles, 2, &mut rng);
        for pos in &positions {
            let idx = tiles.idx(pos.x, pos.y);
            assert_ne!(
                tiles.terrain[idx],
                Terrain::Ice,
                "starting position ({},{}) should not be on Ice",
                pos.x,
                pos.y
            );
        }
    }
}
