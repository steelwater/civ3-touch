use fc3_core::protocol::{CitySnapshot, TileSnapshot};
use fc3_core::types::TileCoord;

/// Data needed to render the city view screen.
pub struct CityViewData {
    pub city: CitySnapshot,
    pub tile_yields: Vec<(TileCoord, i32, i32, i32)>,
    pub tile_snapshots: Vec<TileSnapshot>,
}
