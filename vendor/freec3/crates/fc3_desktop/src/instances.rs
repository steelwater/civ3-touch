/// GPU instance data for tile rendering. Must match tile.wgsl TileInstance.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileInstance {
    pub position: [f32; 2],
    pub color: [f32; 3],
    pub border_color: [f32; 3],
    pub border_mask: f32,
    pub atlas_rect: [f32; 4], // [u, v, w, h] in atlas UV space; [0,0,0,0] = color-only
    pub overlay_rect: [f32; 4], // second atlas layer blended on top; [0,0,0,0] = none
    pub veg_rect: [f32; 4], // vegetation overlay (forest/jungle); [0,0,0,0] = none
    pub fog_rect: [f32; 4], // fog of war overlay; [0,0,0,0] = none
}

impl TileInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x3,  // color
        2 => Float32x3,  // border_color
        3 => Float32,    // border_mask
        4 => Float32x4,  // atlas_rect
        5 => Float32x4,  // overlay_rect
        6 => Float32x4,  // veg_rect
        7 => Float32x4,  // fog_rect
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TileInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// GPU instance data for unit rendering. Must match unit.wgsl UnitInstance.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UnitInstance {
    pub position: [f32; 2],
    pub color: [f32; 3],
    pub shape: f32, // 0=circle (military), 1=diamond (civilian)
    pub hp_frac: f32,
    pub stack_count: f32, // 1.0 = single unit, 2.0-5.0 = stacked (draws indicator lines)
    pub offset: [f32; 2],
    pub atlas_rect: [f32; 4], // [u, v, w, h] in atlas UV space; [0,0,0,0] = SDF fallback
    pub sprite_size: [f32; 2], // iso-space [width, height] for pixel-perfect sizing
}

impl UnitInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x3,  // color
        2 => Float32,    // shape
        3 => Float32,    // hp_frac
        4 => Float32,    // stack_count
        5 => Float32x2,  // offset
        6 => Float32x4,  // atlas_rect
        7 => Float32x2,  // sprite_size
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UnitInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// GPU instance data for city rendering. Must match city.wgsl CityInstance.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CityInstance {
    pub position: [f32; 2],
    pub color: [f32; 3],
    pub population: f32,
    pub atlas_rect: [f32; 4], // [u, v, w, h] in atlas UV space; [0,0,0,0] = color-only fallback
}

impl CityInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x3,  // color
        2 => Float32,    // population
        3 => Float32x4,  // atlas_rect
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CityInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// GPU instance data for path line segments. Must match path_line.wgsl PathLineInstance.
/// Each instance is a thick line segment between two tile midpoints.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PathLineInstance {
    pub start_pos: [f32; 2], // logical tile coords of start tile
    pub end_pos: [f32; 2],   // logical tile coords of end tile
    pub color: [f32; 4],     // RGBA
}

impl PathLineInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,  // start_pos
        1 => Float32x2,  // end_pos
        2 => Float32x4,  // color
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PathLineInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// GPU instance data for the selection circle indicator. Must match selection_circle.wgsl.
/// Each instance draws a thin white ellipse on a tile in world space.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SelectionCircleInstance {
    pub position: [f32; 2], // logical tile coords
    pub alpha: f32,         // opacity (pulsing)
    pub _pad: f32,
}

impl SelectionCircleInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32,    // alpha
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SelectionCircleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// GPU instance data for screen-space UI rectangles. Must match ui_rect.wgsl UIRectInstance.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UIRectInstance {
    pub rect: [f32; 4],     // x, y, w, h in pixels
    pub color: [f32; 4],    // RGBA
    pub viewport: [f32; 2], // viewport_w, viewport_h
}

impl UIRectInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x4,  // rect
        1 => Float32x4,  // color
        2 => Float32x2,  // viewport
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UIRectInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// GPU instance data for screen-space textured UI quads. Must match ui_textured_rect.wgsl.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UITexturedRectInstance {
    pub rect: [f32; 4],       // x, y, w, h in pixels
    pub atlas_rect: [f32; 4], // u, v, w, h in normalized UV
    pub viewport: [f32; 2],   // viewport_w, viewport_h
}

impl UITexturedRectInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x4,  // rect
        1 => Float32x4,  // atlas_rect
        2 => Float32x2,  // viewport
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UITexturedRectInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}
