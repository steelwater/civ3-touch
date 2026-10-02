use crate::types::PlayerId;
use crate::world::World;

use super::Engine;

impl Engine {
    /// Updates fog of war visibility for a player based on their units and cities.
    pub fn update_visibility(&self, player: PlayerId) {
        let mut world = self.world.borrow_mut();

        let width = world.tiles.width;
        let height = world.tiles.height;

        // First, downgrade all Visible tiles to Revealed
        for y in 0..height {
            for x in 0..width {
                if world.tiles.get_visibility(player, x, y) == crate::tile::Visibility::Visible {
                    world
                        .tiles
                        .set_visibility(player, x, y, crate::tile::Visibility::Revealed);
                }
            }
        }

        // Collect visibility sources: units + cities
        let mut sight_data: Vec<(crate::types::TileCoord, i32)> = Vec::new();

        // Units: default sight range 1, hills give +1
        for (_, idx) in world.units.iter_alive() {
            if world.units.owner[idx] != player {
                continue;
            }
            let pos = world.units.position[idx];
            let tile_idx = world.tiles.idx(pos.x, pos.y);
            let on_hill = world.tiles.terrain[tile_idx] == crate::tile::Terrain::Hill;
            let sight_range = if on_hill { 2 } else { 1 };
            sight_data.push((pos, sight_range));
        }

        // Cities: sight range 2 from border tiles
        let city_positions: Vec<crate::types::TileCoord> = world
            .cities
            .iter_alive()
            .filter(|&(_, idx)| world.cities.owner[idx] == player)
            .map(|(_, idx)| world.cities.position[idx])
            .collect();
        for city_pos in city_positions {
            let border = Self::city_border_tiles_static(&world.tiles, city_pos);
            for tile in border {
                sight_data.push((tile, 2));
            }
        }

        // Set tiles within sight range to Visible
        for (pos, range) in sight_data {
            // Set the source's own tile as visible
            world
                .tiles
                .set_visibility(player, pos.x, pos.y, crate::tile::Visibility::Visible);

            // Set tiles within sight range
            self.set_visible_in_range(&mut world, player, pos, range);
        }
    }

    /// Sets all tiles within `range` of `center` to Visible.
    fn set_visible_in_range(
        &self,
        world: &mut World,
        player: PlayerId,
        center: crate::types::TileCoord,
        range: i32,
    ) {
        let width = world.tiles.width as i64;
        let height = world.tiles.height as i64;
        let wrap_x = world.tiles.wrap_x;
        let wrap_y = world.tiles.wrap_y;

        for dy in -range as i64..=range as i64 {
            for dx in -range as i64..=range as i64 {
                let ny = center.y as i64 + dy;
                let actual_y = if wrap_y {
                    ny.rem_euclid(height) as u32
                } else {
                    if ny < 0 || ny >= height {
                        continue;
                    }
                    ny as u32
                };
                let nx = center.x as i64 + dx;
                let actual_x = if wrap_x {
                    nx.rem_euclid(width) as u32
                } else {
                    if nx < 0 || nx >= width {
                        continue;
                    }
                    nx as u32
                };
                world.tiles.set_visibility(
                    player,
                    actual_x,
                    actual_y,
                    crate::tile::Visibility::Visible,
                );
            }
        }
    }
}
