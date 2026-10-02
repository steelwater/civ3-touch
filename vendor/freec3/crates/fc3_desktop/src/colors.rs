use fc3_core::tile::{Terrain, Vegetation};
use fc3_core::types::{ImprovementId, PlayerId};

/// Map terrain types to RGB colors for rendering.
pub fn terrain_color(terrain: Terrain) -> [f32; 3] {
    match terrain {
        Terrain::Grassland => [0.30, 0.60, 0.20],
        Terrain::Plains => [0.70, 0.65, 0.30],
        Terrain::Desert => [0.90, 0.85, 0.55],
        Terrain::Tundra => [0.70, 0.75, 0.70],
        Terrain::Hill => [0.55, 0.50, 0.30],
        Terrain::Mountain => [0.50, 0.48, 0.45],
        Terrain::Coast => [0.35, 0.55, 0.80],
        Terrain::Ocean => [0.15, 0.30, 0.60],
        Terrain::Ice => [0.85, 0.90, 0.95],
    }
}

/// Apply a vegetation tint over a base terrain color.
/// Uses 75% vegetation color + 25% base terrain for strong visual distinction.
pub fn vegetation_tint(base: [f32; 3], vegetation: Vegetation) -> [f32; 3] {
    let (tint, t) = match vegetation {
        Vegetation::None => return base,
        Vegetation::Forest => ([0.15, 0.45, 0.15], 0.75),
        Vegetation::Jungle => ([0.10, 0.40, 0.10], 0.75),
    };
    [
        base[0] * (1.0 - t) + tint[0] * t,
        base[1] * (1.0 - t) + tint[1] * t,
        base[2] * (1.0 - t) + tint[2] * t,
    ]
}

/// Map player IDs to distinct, high-contrast colors.
pub fn player_color(player: PlayerId) -> [f32; 3] {
    match player.0 {
        0 => [0.90, 0.20, 0.20], // Red
        1 => [0.20, 0.50, 0.90], // Blue
        2 => [0.20, 0.80, 0.20], // Green
        3 => [0.85, 0.70, 0.10], // Yellow
        4 => [0.70, 0.30, 0.85], // Purple
        5 => [0.90, 0.50, 0.15], // Orange
        6 => [0.15, 0.80, 0.80], // Cyan
        7 => [0.85, 0.40, 0.60], // Pink
        _ => [0.60, 0.60, 0.60], // Gray fallback
    }
}

/// Brown road tint: blend terrain color with brown.
pub fn road_tint(base: [f32; 3]) -> [f32; 3] {
    let road = [0.55, 0.40, 0.25];
    let t = 0.3;
    [
        base[0] * (1.0 - t) + road[0] * t,
        base[1] * (1.0 - t) + road[1] * t,
        base[2] * (1.0 - t) + road[2] * t,
    ]
}

/// Subtle improvement tint: mine=brown/amber, irrigation=blue-green.
pub fn improvement_tint(base: [f32; 3], improvement: ImprovementId) -> [f32; 3] {
    let (tint, t) = match improvement.0 {
        1 => ([0.65, 0.45, 0.20], 0.25), // Mine: amber/brown
        2 => ([0.20, 0.55, 0.50], 0.25), // Irrigation: blue-green
        _ => return base,
    };
    [
        base[0] * (1.0 - t) + tint[0] * t,
        base[1] * (1.0 - t) + tint[1] * t,
        base[2] * (1.0 - t) + tint[2] * t,
    ]
}

/// Blend player ownership color into tile at 15%.
pub fn ownership_tint(base: [f32; 3], player: PlayerId) -> [f32; 3] {
    let pc = player_color(player);
    let t = 0.15;
    [
        base[0] * (1.0 - t) + pc[0] * t,
        base[1] * (1.0 - t) + pc[1] * t,
        base[2] * (1.0 - t) + pc[2] * t,
    ]
}
