//! Civ3Touch M5 resource foundation. See docs/milestone-5-resources.md.
//! The small generated map uses prototype placement, not Conquests map generation.
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::tile::{Terrain, TileStore, Vegetation};
use crate::types::{PlayerId, ResourceId};
use crate::world::World;

pub struct ResourceDef {
    pub id: ResourceId,
    pub name: &'static str,
    pub food: i32,
    pub shields: i32,
    pub commerce: i32,
    pub reveal_tech: Option<&'static str>,
}

// Stable IDs are part of the M5 resource rules contract. Values are from the
// supplied Conquests Civilopedia, GCON_ResourcesB and GCON_ResourcesS.
pub const RESOURCES: [ResourceDef; 4] = [
    ResourceDef {
        id: ResourceId(0),
        name: "Wheat",
        food: 2,
        shields: 0,
        commerce: 0,
        reveal_tech: None,
    },
    ResourceDef {
        id: ResourceId(1),
        name: "Cattle",
        food: 2,
        shields: 1,
        commerce: 0,
        reveal_tech: None,
    },
    ResourceDef {
        id: ResourceId(2),
        name: "Gold",
        food: 0,
        shields: 0,
        commerce: 4,
        reveal_tech: None,
    },
    ResourceDef {
        id: ResourceId(3),
        name: "Horses",
        food: 0,
        shields: 0,
        commerce: 1,
        reveal_tech: Some("the_wheel"),
    },
];

pub fn disabled(enabled: &bool) -> bool {
    !enabled
}

pub fn definition(id: ResourceId) -> Option<&'static ResourceDef> {
    RESOURCES.get(id.0 as usize)
}

pub fn revealed(world: &World, player: PlayerId, tile: usize) -> Option<&'static ResourceDef> {
    if !world.resources_enabled {
        return None;
    }
    let player = world.players.iter().find(|p| p.id == player)?;
    let resource = definition(*world.tiles.resource.get(tile)?.as_ref()?)?;
    if resource
        .reveal_tech
        .is_some_and(|tech| !player.researched_techs.iter().any(|t| t == tech))
    {
        return None;
    }
    Some(resource)
}

pub fn place(tiles: &mut TileStore, seed: u64) {
    // Independent RNG: enabling resources must not consume the combat/start RNG.
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x4d35_7265_736f_7572);
    for i in 0..tiles.terrain.len() {
        if tiles.vegetation[i] != Vegetation::None || tiles.resource[i].is_some() {
            continue;
        }
        let choices: &[u16] = match tiles.terrain[i] {
            Terrain::Grassland | Terrain::Plains => &[0, 1, 3],
            Terrain::Hill => &[2, 3],
            Terrain::Mountain => &[2],
            _ => continue,
        };
        if rng.gen_range(0..8) == 0 {
            tiles.resource[i] = Some(ResourceId(choices[rng.gen_range(0..choices.len())]));
        }
    }
}
