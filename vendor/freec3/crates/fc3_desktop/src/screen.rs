use fc3_core::id::CityId;

/// Which UI screen/mode is currently active.
/// Controls input dispatch and rendering.
#[derive(Debug, Clone)]
pub enum UIScreen {
    /// Normal map view — the main game screen.
    Overworld,
    /// Viewing a specific city's details.
    CityView { city_id: CityId },
    /// In-game menu overlay (rendered on top of the overworld).
    MainMenu { selected_index: usize },
}
