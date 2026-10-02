use std::collections::{HashMap, HashSet};
use std::time::Instant;

use fc3_core::id::UnitId;
use fc3_core::types::TileCoord;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Tracks all input state for the desktop client.
pub struct InputState {
    /// Currently selected unit (belongs to the human player).
    pub selected_unit: Option<UnitId>,
    /// Tile currently under the mouse cursor.
    pub hovered_tile: Option<TileCoord>,
    /// Current mouse position in screen pixels.
    pub mouse_screen_pos: (f32, f32),
    /// Currently held keyboard keys (for continuous pan).
    pub keys_held: HashSet<KeyCode>,
    /// Currently held mouse buttons.
    pub mouse_buttons_held: HashSet<MouseButton>,
    /// Goto mode: press G to enter, left-click to commit move, Escape to cancel.
    pub goto_mode: bool,
    /// Index for cycling through unhandled units with Enter.
    pub unit_cycle_index: usize,
    /// Which unit index to display per tile for stacked units the player has cycled through.
    pub tile_stack_index: HashMap<TileCoord, usize>,
    /// Last left-click tile and timestamp for double-click detection.
    pub last_left_click: Option<(TileCoord, Instant)>,
}

/// Maximum time between two clicks to count as a double-click.
pub const DOUBLE_CLICK_MS: u128 = 400;

impl InputState {
    pub fn new() -> Self {
        InputState {
            selected_unit: None,
            hovered_tile: None,
            mouse_screen_pos: (0.0, 0.0),
            keys_held: HashSet::new(),
            mouse_buttons_held: HashSet::new(),
            goto_mode: false,
            unit_cycle_index: 0,
            tile_stack_index: HashMap::new(),
            last_left_click: None,
        }
    }
}
