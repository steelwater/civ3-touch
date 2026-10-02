/// Advisor types correspond to the five advisor categories in Civ3.
/// Each has a 4×4 sprite atlas of 150×150 portraits in Art/SmallHeads/.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AdvisorType {
    Culture,
    Domestic,
    Foreign,
    Military,
    Science,
}

/// Rows in the advisor atlas (top to bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Age {
    Ancient,
    Medieval,
    Industrial,
    Modern,
}

/// Columns in the advisor atlas (left to right).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Emotion {
    Happy,
    Angry,
    Sad,
    Surprised,
}

const CELL_SIZE: f32 = 150.0;
const GRID_COLS: usize = 4;
const GRID_ROWS: usize = 4;

impl AdvisorType {
    /// PCX filename within the Art/SmallHeads/ directory.
    #[allow(dead_code)]
    pub fn filename(self) -> &'static str {
        match self {
            AdvisorType::Culture => "popupCULTURE.pcx",
            AdvisorType::Domestic => "popupDOMESTIC.pcx",
            AdvisorType::Foreign => "popupFOREIGN.pcx",
            AdvisorType::Military => "popupMILITARY.pcx",
            AdvisorType::Science => "popupSCIENCE.pcx",
        }
    }
}

/// Compute the normalized UV rect `[u, v, w, h]` for a given age + emotion
/// within an advisor atlas of the given pixel dimensions.
pub fn advisor_uv_rect(age: Age, emotion: Emotion, atlas_width: u32, atlas_height: u32) -> [f32; 4] {
    let col = emotion as usize;
    let row = age as usize;
    debug_assert!(col < GRID_COLS);
    debug_assert!(row < GRID_ROWS);

    let px = col as f32 * CELL_SIZE;
    let py = row as f32 * CELL_SIZE;
    let aw = atlas_width as f32;
    let ah = atlas_height as f32;

    // Half-texel inset to prevent edge sampling artifacts.
    [
        (px + 0.5) / aw,
        (py + 0.5) / ah,
        (CELL_SIZE - 1.0) / aw,
        (CELL_SIZE - 1.0) / ah,
    ]
}
