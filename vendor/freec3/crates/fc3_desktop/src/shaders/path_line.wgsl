// Path line segment shader — draws thick lines between tile midpoints in isometric space
// Each instance is one line segment (two endpoints). The vertex shader generates a quad
// (6 vertices = 2 triangles) oriented along the line with a configurable thickness.

struct CameraUniform {
    center: vec2<f32>,
    zoom: f32,
    aspect: f32,
    viewport_w: f32,
    viewport_h: f32,
    map_height: f32,
    wrap_y: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct PathLineInstance {
    @location(0) start_pos: vec2<f32>,  // logical tile coords
    @location(1) end_pos: vec2<f32>,    // logical tile coords
    @location(2) color: vec4<f32>,      // RGBA
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Half-width of the line in isometric world units
const LINE_HALF_WIDTH: f32 = 0.018;

fn tile_to_iso(pos: vec2<f32>) -> vec2<f32> {
    // Tile midpoint: logical (x+0.5, y+0.5) -> iso
    var adj_y = pos.y;
    if (camera.wrap_y > 0.5) {
        adj_y = pos.y - camera.map_height * floor((pos.x + pos.y) / camera.map_height);
    }
    let lx = pos.x + 0.5;
    let ly = adj_y + 0.5;
    return vec2<f32>(lx - ly, (lx + ly) * 0.5);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: PathLineInstance,
) -> VertexOutput {
    let iso_start = tile_to_iso(instance.start_pos);
    let iso_end = tile_to_iso(instance.end_pos);

    let dir = iso_end - iso_start;
    let len = length(dir);

    // Perpendicular direction (rotated 90 degrees)
    var perp: vec2<f32>;
    if (len > 0.001) {
        perp = vec2<f32>(-dir.y, dir.x) / len;
    } else {
        perp = vec2<f32>(0.0, 1.0);
    }

    let half_w = perp * LINE_HALF_WIDTH;

    // Generate quad: 6 vertices for 2 triangles
    // 0--1
    // |\ |
    // | \|
    // 3--2
    var pos: vec2<f32>;
    switch (vertex_index) {
        case 0u: { pos = iso_start - half_w; }
        case 1u: { pos = iso_end - half_w; }
        case 2u: { pos = iso_end + half_w; }
        case 3u: { pos = iso_start - half_w; }
        case 4u: { pos = iso_end + half_w; }
        case 5u: { pos = iso_start + half_w; }
        default: { pos = iso_start; }
    }

    let offset = (pos - camera.center) * camera.zoom;
    let clip_x = offset.x / (camera.viewport_w * 0.5);
    let clip_y = -offset.y / (camera.viewport_h * 0.5);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
