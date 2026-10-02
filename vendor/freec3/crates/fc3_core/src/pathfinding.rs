// Pathfinding module for FC3.
//
// ## Design Decision: Movement Cost Cache Key Granularity
//
// The `MovementCostCache` is keyed by `(UnitTypeId, destination TileCoord)`,
// NOT by `(UnitId, from, to)`.
//
// Rationale: In Civ3, the movement cost to enter a tile depends on the tile's
// terrain and the unit's type (land/sea/air), not on which specific tile you're
// coming from. The exception is river crossings (which cost extra when crossing
// a river edge). For river support, the cache key may later need to include the
// edge direction, but for now terrain-only is correct.
//
// If a mod needs per-unit costs (e.g., a unit with a special ability that halves
// forest cost), it should use traits checked at pathfinding time, not per-unit
// cache entries. The cache is per unit *type*.
//
// ## Integer Costs
//
// All movement costs are `i32`. After the integer-thirds migration (Task 7.5),
// 1 full movement point = 3 internal units, road cost = 1, grassland = 3,
// hill/forest = 6, etc. The pathfinder is scale-agnostic — it works with
// whatever i32 values the cost function provides.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::types::{TileCoord, UnitTypeId};

/// Sentinel value for impassable tiles. The Lua hook uses -1 to signal
/// impassable; we convert to this constant at the boundary.
pub const IMPASSABLE: i32 = i32::MAX;

/// Caches per-turn movement costs keyed by (unit type, destination tile).
/// Avoids redundant Lua hook calls during pathfinding.
pub struct MovementCostCache {
    pub costs: HashMap<(UnitTypeId, TileCoord), i32>,
}

impl MovementCostCache {
    pub fn new() -> Self {
        Self {
            costs: HashMap::new(),
        }
    }

    /// Clears all cached values. Called at turn start or when terrain changes.
    pub fn invalidate(&mut self) {
        self.costs.clear();
    }

    /// Returns the cached cost or computes it via `compute_fn`, caches the
    /// result, and returns it.
    pub fn get_or_compute(
        &mut self,
        unit_type: UnitTypeId,
        tile: TileCoord,
        compute_fn: impl FnOnce(UnitTypeId, TileCoord) -> i32,
    ) -> i32 {
        *self
            .costs
            .entry((unit_type, tile))
            .or_insert_with(|| compute_fn(unit_type, tile))
    }
}

impl Default for MovementCostCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a successful pathfinding search.
#[derive(Debug, Clone)]
pub struct PathResult {
    /// Full path from start to goal, inclusive.
    pub tiles: Vec<TileCoord>,
    /// Cumulative movement cost at each step (index 0 is always 0 for start tile).
    pub costs: Vec<i32>,
    /// Total movement cost of the path (equal to `costs.last()`).
    pub total_cost: i32,
}

/// Chebyshev distance between two tiles, accounting for map wrapping.
/// Returns the minimum distance considering wrap_x/wrap_y if enabled.
pub fn chebyshev_heuristic(
    a: TileCoord,
    b: TileCoord,
    map_width: u32,
    wrap_x: bool,
    map_height: u32,
    wrap_y: bool,
) -> i32 {
    let mut dx = (a.x as i64 - b.x as i64).unsigned_abs() as i32;
    if wrap_x && map_width > 0 {
        dx = dx.min(map_width as i32 - dx);
    }
    let mut dy = (a.y as i64 - b.y as i64).unsigned_abs() as i32;
    if wrap_y && map_height > 0 {
        dy = dy.min(map_height as i32 - dy);
    }
    dx.max(dy)
}

/// A* node for the priority queue.
#[derive(Debug, Clone, Eq, PartialEq)]
struct AStarNode {
    coord: TileCoord,
    f_cost: i32,
    g_cost: i32,
    /// Number of tiles in the path so far (for tie-breaking: prefer fewer tiles).
    steps: u32,
    /// Absolute cross-product of (node−start) × (goal−start). Measures how far
    /// this node deviates from the straight line between start and goal.
    /// Used as a third-level tie-breaker so that equal-cost paths prefer
    /// straighter lines instead of wandering.
    line_deviation: u64,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: lower f_cost is better. Tie-break on fewer steps,
        // then on smaller deviation from the start→goal line.
        other
            .f_cost
            .cmp(&self.f_cost)
            .then(other.steps.cmp(&self.steps))
            .then(other.line_deviation.cmp(&self.line_deviation))
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Finds the shortest path from `start` to `goal` using A*.
///
/// - `cost_fn(from, to) -> i32`: cost to move from `from` to `to`. Returns
///   `IMPASSABLE` if the tile cannot be entered. The `from` parameter allows
///   edge-dependent costs like zone of control penalties.
/// - `neighbors_fn(tile) -> Vec<TileCoord>`: returns adjacent tiles.
/// - `heuristic_fn(from, to) -> i32`: admissible heuristic (e.g., Chebyshev distance).
///
/// Returns `None` if the goal is unreachable or impassable.
pub fn find_path(
    start: TileCoord,
    goal: TileCoord,
    mut cost_fn: impl FnMut(TileCoord, TileCoord) -> i32,
    neighbors_fn: impl Fn(TileCoord) -> Vec<TileCoord>,
    heuristic_fn: impl Fn(TileCoord, TileCoord) -> i32,
) -> Option<PathResult> {
    // Trivial case: start == goal
    if start == goal {
        return Some(PathResult {
            tiles: vec![start],
            costs: vec![0],
            total_cost: 0,
        });
    }

    // Goal itself is impassable → no path
    if cost_fn(start, goal) == IMPASSABLE {
        return None;
    }

    let mut open = BinaryHeap::new();
    let mut g_costs: HashMap<TileCoord, i32> = HashMap::new();
    let mut came_from: HashMap<TileCoord, TileCoord> = HashMap::new();

    // Precompute the start→goal vector for cross-product tie-breaking.
    // This measures how far each explored node deviates from the straight
    // line, so equal-cost paths prefer going in a direct line.
    let dx_goal = goal.x as i64 - start.x as i64;
    let dy_goal = goal.y as i64 - start.y as i64;

    let h = heuristic_fn(start, goal);
    open.push(AStarNode {
        coord: start,
        f_cost: h,
        g_cost: 0,
        steps: 0,
        line_deviation: 0,
    });
    g_costs.insert(start, 0);

    while let Some(current) = open.pop() {
        if current.coord == goal {
            // Reconstruct path
            let mut path = vec![goal];
            let mut node = goal;
            while let Some(&prev) = came_from.get(&node) {
                path.push(prev);
                node = prev;
            }
            path.reverse();

            // Build cumulative costs
            let mut costs = Vec::with_capacity(path.len());
            costs.push(0);
            let mut cumulative = 0i32;
            for i in 1..path.len() {
                let tile_cost = cost_fn(path[i - 1], path[i]);
                cumulative += tile_cost;
                costs.push(cumulative);
            }

            return Some(PathResult {
                tiles: path,
                costs,
                total_cost: cumulative,
            });
        }

        // Skip if we've found a better path to this node already
        let current_g = match g_costs.get(&current.coord) {
            Some(&g) if g < current.g_cost => continue,
            _ => current.g_cost,
        };

        for neighbor in neighbors_fn(current.coord) {
            let move_cost = cost_fn(current.coord, neighbor);
            if move_cost == IMPASSABLE {
                continue;
            }

            let new_g = current_g.saturating_add(move_cost);
            let existing_g = g_costs.get(&neighbor).copied().unwrap_or(i32::MAX);

            if new_g < existing_g {
                g_costs.insert(neighbor, new_g);
                came_from.insert(neighbor, current.coord);
                let h = heuristic_fn(neighbor, goal);
                let dx = neighbor.x as i64 - start.x as i64;
                let dy = neighbor.y as i64 - start.y as i64;
                let cross = (dx * dy_goal - dx_goal * dy).unsigned_abs();
                open.push(AStarNode {
                    coord: neighbor,
                    f_cost: new_g.saturating_add(h),
                    g_cost: new_g,
                    steps: current.steps + 1,
                    line_deviation: cross,
                });
            }
        }
    }

    None // Goal unreachable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{Terrain, TileStore, Vegetation};

    // ── Movement Cost Cache tests ──────────────────────────────────

    #[test]
    fn test_cache_first_lookup_calls_compute() {
        let mut cache = MovementCostCache::new();
        let mut called = false;
        let cost = cache.get_or_compute(UnitTypeId(0), TileCoord { x: 1, y: 1 }, |_, _| {
            called = true;
            3
        });
        assert!(called);
        assert_eq!(cost, 3);
    }

    #[test]
    fn test_cache_second_lookup_uses_cache() {
        let mut cache = MovementCostCache::new();
        let tile = TileCoord { x: 2, y: 3 };
        let ut = UnitTypeId(0);

        let mut call_count = 0;
        cache.get_or_compute(ut, tile, |_, _| {
            call_count += 1;
            5
        });
        // Second call should NOT invoke compute_fn
        let cost = cache.get_or_compute(ut, tile, |_, _| {
            call_count += 1;
            99
        });
        assert_eq!(call_count, 1);
        assert_eq!(cost, 5);
    }

    #[test]
    fn test_cache_invalidate_forces_recompute() {
        let mut cache = MovementCostCache::new();
        let tile = TileCoord { x: 0, y: 0 };
        let ut = UnitTypeId(0);

        cache.get_or_compute(ut, tile, |_, _| 3);
        cache.invalidate();

        let mut called_after = false;
        let cost = cache.get_or_compute(ut, tile, |_, _| {
            called_after = true;
            7
        });
        assert!(called_after);
        assert_eq!(cost, 7);
    }

    #[test]
    fn test_cache_different_unit_types_independent() {
        let mut cache = MovementCostCache::new();
        let tile = TileCoord { x: 1, y: 1 };

        let cost_a = cache.get_or_compute(UnitTypeId(0), tile, |_, _| 3);
        let cost_b = cache.get_or_compute(UnitTypeId(1), tile, |_, _| 6);

        assert_eq!(cost_a, 3);
        assert_eq!(cost_b, 6);
    }

    // ── Test helper: map_from_ascii ────────────────────────────────

    /// Creates a TileStore from an ASCII grid. Each character maps to a terrain:
    /// - `.` = Grassland
    /// - `M` = Mountain
    /// - `H` = Hill
    /// - `F` = Forest (Grassland + Vegetation::Forest)
    /// - `J` = Jungle (Grassland + Vegetation::Jungle)
    /// - `O` = Ocean
    /// - `C` = Coast
    /// - `P` = Plains
    /// - `D` = Desert
    /// - `T` = Tundra
    /// - `R` = Road (Grassland with road_level=1)
    ///
    /// Lines are separated by newlines. First line = row 0 (y=0).
    fn map_from_ascii(grid: &str, wrap_x: bool) -> TileStore {
        let lines: Vec<&str> = grid
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        let height = lines.len() as u32;
        let width = lines[0].len() as u32;
        let mut store = TileStore::new(width, height, wrap_x, false, Terrain::Grassland, 2);

        for (y, line) in lines.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                let idx = store.idx(x as u32, y as u32);
                match ch {
                    '.' => store.terrain[idx] = Terrain::Grassland,
                    'M' => store.terrain[idx] = Terrain::Mountain,
                    'H' => store.terrain[idx] = Terrain::Hill,
                    'F' => {
                        store.terrain[idx] = Terrain::Grassland;
                        store.vegetation[idx] = Vegetation::Forest;
                    }
                    'J' => {
                        store.terrain[idx] = Terrain::Grassland;
                        store.vegetation[idx] = Vegetation::Jungle;
                    }
                    'O' => store.terrain[idx] = Terrain::Ocean,
                    'C' => store.terrain[idx] = Terrain::Coast,
                    'P' => store.terrain[idx] = Terrain::Plains,
                    'D' => store.terrain[idx] = Terrain::Desert,
                    'T' => store.terrain[idx] = Terrain::Tundra,
                    'I' => store.terrain[idx] = Terrain::Ice,
                    'R' => {
                        store.terrain[idx] = Terrain::Grassland;
                        store.road_level[idx] = 1;
                    }
                    _ => panic!("unknown terrain char: {ch}"),
                }
            }
        }

        store
    }

    /// Simple cost function from a TileStore: grassland/plains/etc = 1, hill/forest/jungle = 2, road = 1, mountain/ocean = IMPASSABLE
    fn simple_cost_fn(tiles: &TileStore) -> impl Fn(TileCoord, TileCoord) -> i32 + '_ {
        move |_from: TileCoord, to: TileCoord| {
            let idx = tiles.idx(to.x, to.y);
            if tiles.road_level[idx] > 0 {
                return 1;
            }
            match tiles.terrain[idx] {
                Terrain::Mountain | Terrain::Ocean | Terrain::Ice => IMPASSABLE,
                Terrain::Hill => 2,
                _ => {
                    // Check vegetation overlay
                    match tiles.vegetation[idx] {
                        Vegetation::Forest | Vegetation::Jungle => 2,
                        Vegetation::None => 1,
                    }
                }
            }
        }
    }

    fn make_neighbors_fn(tiles: &TileStore) -> impl Fn(TileCoord) -> Vec<TileCoord> + '_ {
        move |coord: TileCoord| tiles.neighbors(coord.x, coord.y)
    }

    fn make_heuristic(
        map_width: u32,
        wrap_x: bool,
        map_height: u32,
        wrap_y: bool,
    ) -> impl Fn(TileCoord, TileCoord) -> i32 {
        move |a, b| chebyshev_heuristic(a, b, map_width, wrap_x, map_height, wrap_y)
    }

    // ── A* Pathfinding tests ───────────────────────────────────────

    #[test]
    fn test_path_start_equals_goal() {
        let tiles = map_from_ascii("...", false);
        let start = TileCoord { x: 1, y: 0 };
        let result = find_path(
            start,
            start,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();
        assert_eq!(path.tiles, vec![start]);
        assert_eq!(path.total_cost, 0);
    }

    #[test]
    fn test_path_straight_line_flat_terrain() {
        // 10x1 strip of grassland
        let tiles = map_from_ascii("..........", false);
        let start = TileCoord { x: 0, y: 0 };
        let goal = TileCoord { x: 9, y: 0 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();
        assert_eq!(path.tiles.len(), 10); // start + 9 steps
        assert_eq!(path.tiles[0], start);
        assert_eq!(*path.tiles.last().unwrap(), goal);
        assert_eq!(path.total_cost, 9);
    }

    #[test]
    fn test_path_around_obstacle() {
        // Wall of mountains at column 2, rows 0-3. Gap at row 4.
        let tiles = map_from_ascii(
            "..M..
             ..M..
             ..M..
             ..M..
             .....",
            false,
        );
        let start = TileCoord { x: 0, y: 0 };
        let goal = TileCoord { x: 4, y: 0 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();
        // Path must not pass through any mountain tile (column 2, rows 0-3)
        for tile in &path.tiles {
            if tile.x == 2 && tile.y < 4 {
                panic!("path went through mountain at {:?}", tile);
            }
        }
        assert_eq!(*path.tiles.last().unwrap(), goal);
    }

    #[test]
    fn test_path_no_path_exists() {
        // Goal surrounded by mountains
        let tiles = map_from_ascii(
            ".MMM.
             .M.M.
             .MMM.
             .....",
            false,
        );
        let start = TileCoord { x: 0, y: 3 };
        let goal = TileCoord { x: 2, y: 1 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_path_diagonal_movement() {
        // 5x5 grassland, diagonal path should be 4 steps
        let tiles = map_from_ascii(
            ".....
             .....
             .....
             .....
             .....",
            false,
        );
        let start = TileCoord { x: 0, y: 0 };
        let goal = TileCoord { x: 4, y: 4 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();
        // Diagonal: 4 steps (0,0)->(1,1)->(2,2)->(3,3)->(4,4)
        assert_eq!(path.tiles.len(), 5);
        assert_eq!(path.total_cost, 4);
    }

    #[test]
    fn test_path_prefers_straight_line_on_uniform_terrain() {
        // 10x10 grassland. Going from (0,2) to (9,5), the path should stay
        // close to the direct line, not wander north then come back.
        let tiles = map_from_ascii(
            "..........
             ..........
             ..........
             ..........
             ..........
             ..........
             ..........
             ..........
             ..........
             ..........",
            false,
        );
        let start = TileCoord { x: 0, y: 2 };
        let goal = TileCoord { x: 9, y: 5 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();

        // The path should never go above y=2 (the start row). On uniform terrain
        // with this start/goal, a straight-ish path goes from y=2 down to y=5.
        for tile in &path.tiles {
            assert!(
                tile.y >= 2,
                "path wandered north to {:?} — should stay on or below start row",
                tile
            );
        }

        // The y-coordinates should be monotonically non-decreasing (moving toward goal)
        for i in 1..path.tiles.len() {
            assert!(
                path.tiles[i].y >= path.tiles[i - 1].y,
                "path went north from {:?} to {:?}",
                path.tiles[i - 1],
                path.tiles[i]
            );
        }
    }

    #[test]
    fn test_path_prefers_roads() {
        // Row 0 and 2 are grassland, row 1 is road
        let tiles = map_from_ascii(
            ".....
             RRRRR
             .....",
            false,
        );
        let start = TileCoord { x: 0, y: 0 };
        let goal = TileCoord { x: 4, y: 0 };
        // Direct route: 4 cost. Via roads: 1(down) + 3*1(road) + 1(up) = 5. Actually...
        // Road cost is 1 in this simple_cost_fn, same as grassland. So no preference.
        // For this test, use a cost fn where roads cost less.
        let cost_fn = |_from: TileCoord, to: TileCoord| {
            let idx = tiles.idx(to.x, to.y);
            if tiles.road_level[idx] > 0 {
                return 1; // 1/3 of grassland in integer-thirds, but here just 1
            }
            match tiles.terrain[idx] {
                Terrain::Mountain | Terrain::Ocean | Terrain::Ice => IMPASSABLE,
                _ => 3, // Make non-road grassland cost 3
            }
        };
        let result = find_path(
            start,
            goal,
            cost_fn,
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();
        // Via road: down(3) + road road road(3) + up(3) = 9
        // Direct: 3 + 3 + 3 + 3 = 12
        // Road is cheaper, but diagonal to road:
        // (0,0) -> (1,1) diag cost 1 (road) + (2,1) 1 + (3,1) 1 + (4,0) diag cost 3 = 6
        // Actually let's just verify total cost is less than direct 12
        assert!(
            path.total_cost < 12,
            "road path should be cheaper than direct; total_cost = {}",
            path.total_cost
        );
        assert_eq!(*path.tiles.last().unwrap(), goal);
    }

    #[test]
    fn test_path_wrapping_shortest() {
        // 20x5 wrapping map. All grassland.
        let row = "....................";
        let grid = format!("{row}\n{row}\n{row}\n{row}\n{row}");
        let tiles = map_from_ascii(&grid, true);
        assert_eq!(tiles.width, 20);

        let start = TileCoord { x: 1, y: 2 };
        let goal = TileCoord { x: 19, y: 2 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();
        // Wrapping left: (1,2) -> (0,2) -> (19,2) = 2 cost
        assert_eq!(path.total_cost, 2);
        assert_eq!(path.tiles.len(), 3);
    }

    #[test]
    fn test_path_hill_costs() {
        // 1x5 strip: ..H..
        let tiles = map_from_ascii("..H..", false);
        let start = TileCoord { x: 0, y: 0 };
        let goal = TileCoord { x: 4, y: 0 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        let path = result.unwrap();
        // Cost: 1 + 2 + 1 + 1 = 5 (enter tile 1=grass, tile 2=hill, tile 3=grass, tile 4=grass)
        assert_eq!(path.total_cost, 5);
    }

    #[test]
    fn test_path_goal_impassable_returns_none() {
        let tiles = map_from_ascii("..M", false);
        let start = TileCoord { x: 0, y: 0 };
        let goal = TileCoord { x: 2, y: 0 }; // Mountain
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(tiles.width, tiles.wrap_x, tiles.height, tiles.wrap_y),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_heuristic_wrapping() {
        let a = TileCoord { x: 1, y: 2 };
        let b = TileCoord { x: 19, y: 2 };
        // Without wrapping: dx=18, dy=0 → 18
        assert_eq!(chebyshev_heuristic(a, b, 20, false, 20, false), 18);
        // With wrapping: dx=min(18, 20-18)=2, dy=0 → 2
        assert_eq!(chebyshev_heuristic(a, b, 20, true, 20, false), 2);
    }

    #[test]
    fn test_heuristic_diagonal() {
        let a = TileCoord { x: 0, y: 0 };
        let b = TileCoord { x: 3, y: 4 };
        assert_eq!(chebyshev_heuristic(a, b, 10, false, 10, false), 4);
    }

    // ── Performance benchmarks (Task 7.12) ────────────────────────

    #[test]
    fn test_benchmark_pathfinding_80x80() {
        use rand::Rng;
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        // Generate an 80x80 map with mixed terrain using a seeded RNG
        let mut rng = ChaCha8Rng::seed_from_u64(12345);
        let width = 80u32;
        let height = 80u32;
        let mut tiles = TileStore::new(width, height, true, false, Terrain::Grassland, 2);

        // Scatter terrain: ~15% mountain, ~20% hill, ~15% forest, rest grassland
        for y in 0..height {
            for x in 0..width {
                let idx = tiles.idx(x, y);
                let r: f32 = rng.gen();
                if r < 0.15 {
                    tiles.terrain[idx] = Terrain::Mountain;
                } else if r < 0.35 {
                    tiles.terrain[idx] = Terrain::Hill;
                } else if r < 0.50 {
                    tiles.terrain[idx] = Terrain::Grassland;
                    tiles.vegetation[idx] = Vegetation::Forest;
                } else {
                    tiles.terrain[idx] = Terrain::Grassland;
                };
            }
        }

        // Generate 100 random start/goal pairs
        let mut pairs: Vec<(TileCoord, TileCoord)> = Vec::new();
        for _ in 0..100 {
            let sx = rng.gen_range(0..width);
            let sy = rng.gen_range(0..height);
            let gx = rng.gen_range(0..width);
            let gy = rng.gen_range(0..height);

            // Ensure start and goal are passable
            let si = tiles.idx(sx, sy);
            tiles.terrain[si] = Terrain::Grassland;
            let gi = tiles.idx(gx, gy);
            tiles.terrain[gi] = Terrain::Grassland;

            pairs.push((TileCoord { x: sx, y: sy }, TileCoord { x: gx, y: gy }));
        }

        let cost_fn = simple_cost_fn(&tiles);
        let neighbors_fn = make_neighbors_fn(&tiles);
        let heuristic_fn = make_heuristic(width, true, height, false);

        let start_time = std::time::Instant::now();
        let mut paths_found = 0u32;

        for (start, goal) in &pairs {
            if find_path(*start, *goal, &cost_fn, &neighbors_fn, &heuristic_fn).is_some() {
                paths_found += 1;
            }
        }

        let elapsed = start_time.elapsed();
        let median_us = elapsed.as_micros() / 100;
        println!(
            "Pathfinding benchmark: 100 calls on 80x80 map. Total: {:?}. \
             Avg: {}us. Paths found: {}/100.",
            elapsed, median_us, paths_found
        );

        // Target: median < 1ms in release (150us measured), < 5ms in debug
        // Debug builds are ~10x slower; the real benchmark is in release mode.
        assert!(
            median_us < 5000,
            "pathfinding too slow: avg {}us per call (target < 5000us in debug)",
            median_us
        );
    }

    // ── Stress / edge-case tests (Task 7.13) ──────────────────────

    #[test]
    fn test_stress_large_map_labyrinth() {
        use rand::Rng;
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        // 100x100 map with a labyrinth of mountains (~40% mountain)
        let mut rng = ChaCha8Rng::seed_from_u64(99999);
        let width = 100u32;
        let height = 100u32;
        let mut tiles = TileStore::new(width, height, false, false, Terrain::Grassland, 2);

        for y in 0..height {
            for x in 0..width {
                let idx = tiles.idx(x, y);
                let r: f32 = rng.gen();
                tiles.terrain[idx] = if r < 0.40 {
                    Terrain::Mountain
                } else {
                    Terrain::Grassland
                };
            }
        }

        // Ensure start and goal are passable
        let start = TileCoord { x: 0, y: 0 };
        let goal = TileCoord { x: 99, y: 99 };
        let si = tiles.idx(0, 0);
        let gi = tiles.idx(99, 99);
        tiles.terrain[si] = Terrain::Grassland;
        tiles.terrain[gi] = Terrain::Grassland;

        let t = std::time::Instant::now();
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(width, false, height, false),
        );
        let elapsed = t.elapsed();

        println!(
            "Labyrinth 100x100: {:?}. Path found: {}. Length: {}",
            elapsed,
            result.is_some(),
            result.as_ref().map(|p| p.tiles.len()).unwrap_or(0)
        );

        // Should finish in reasonable time (< 100ms)
        assert!(
            elapsed.as_millis() < 100,
            "labyrinth too slow: {:?}",
            elapsed
        );
    }

    #[test]
    fn test_stress_all_mountains_except_start_goal() {
        // 10x10 map entirely mountains except start and goal — returns None
        let mut tiles = TileStore::new(10, 10, false, false, Terrain::Mountain, 2);
        let si = tiles.idx(0, 0);
        let gi = tiles.idx(9, 9);
        tiles.terrain[si] = Terrain::Grassland;
        tiles.terrain[gi] = Terrain::Grassland;

        let result = find_path(
            TileCoord { x: 0, y: 0 },
            TileCoord { x: 9, y: 9 },
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(10, false, 10, false),
        );
        assert!(result.is_none(), "no path through all-mountain map");
    }

    // ── wrap_y tests ───────────────────────────────────────────────

    #[test]
    fn test_heuristic_wrapping_y() {
        let a = TileCoord { x: 2, y: 1 };
        let b = TileCoord { x: 2, y: 19 };
        // Without y-wrapping: dy=18 → 18
        assert_eq!(chebyshev_heuristic(a, b, 20, false, 20, false), 18);
        // With y-wrapping: dy=min(18, 20-18)=2 → 2
        assert_eq!(chebyshev_heuristic(a, b, 20, false, 20, true), 2);
    }

    #[test]
    fn test_path_wrapping_y_shortest() {
        // 5x20 map with wrap_y. Path from (2,1) to (2,19) should wrap through y boundary.
        let mut rows = Vec::new();
        for _ in 0..20 {
            rows.push(".....");
        }
        let grid = rows.join("\n");
        let lines: Vec<&str> = grid
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        let height = lines.len() as u32;
        let width = lines[0].len() as u32;
        let tiles = TileStore::new(width, height, false, true, Terrain::Grassland, 2);

        let start = TileCoord { x: 2, y: 1 };
        let goal = TileCoord { x: 2, y: 19 };
        let result = find_path(
            start,
            goal,
            simple_cost_fn(&tiles),
            make_neighbors_fn(&tiles),
            make_heuristic(width, false, height, true),
        );
        let path = result.unwrap();
        // Wrapping: (2,1) -> (2,0) -> (2,19) = 2 steps, cost 2
        assert_eq!(path.total_cost, 2);
        assert_eq!(path.tiles.len(), 3);
    }
}
