// Selection circle shader — draws a thin white ellipse on the selected unit's tile
// The ellipse is inscribed within the isometric diamond, giving a circle-on-tile effect.

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

struct SelectionCircleInstance {
    @location(0) position: vec2<f32>,  // logical tile coords
    @location(1) alpha: f32,           // opacity
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,   // local UV [-1, 1] range
    @location(1) alpha: f32,
};

// Quad vertices covering the diamond area (width=2, height=1 in iso space)
var<private> QUAD_VERTS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -0.5),
    vec2<f32>( 1.0, -0.5),
    vec2<f32>( 1.0,  0.5),
    vec2<f32>(-1.0, -0.5),
    vec2<f32>( 1.0,  0.5),
    vec2<f32>(-1.0,  0.5),
);

var<private> QUAD_UVS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: SelectionCircleInstance,
) -> VertexOutput {
    var adj_y = instance.position.y;
    if (camera.wrap_y > 0.5) {
        adj_y = instance.position.y - camera.map_height * floor((instance.position.x + instance.position.y) / camera.map_height);
    }

    // Iso center of tile: logical_to_iso(col + 0.5, row + 0.5)
    let iso_center = vec2<f32>(
        instance.position.x - adj_y,
        (instance.position.x + adj_y + 1.0) * 0.5
    );

    let iso_pos = iso_center + QUAD_VERTS[vertex_index];

    let offset = (iso_pos - camera.center) * camera.zoom;
    let clip_x = offset.x / (camera.viewport_w * 0.5);
    let clip_y = -offset.y / (camera.viewport_h * 0.5);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.uv = QUAD_UVS[vertex_index];
    out.alpha = instance.alpha;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // The UV is in [-1, 1] range. The isometric diamond maps to a circle in
    // this UV space: the diamond edges are at |u| + |v| = 1, and an ellipse
    // inscribed in the diamond has the equation (u^2 + v^2) = r^2 in a
    // normalized space. But since the diamond has aspect ratio 2:1, we use
    // the UV directly — the quad is 2 wide and 1 tall in iso space, so
    // uv.x maps to the full width and uv.y to the full height.
    //
    // We want an ellipse that fits nicely inside the diamond.
    // Use radius ~0.75 to leave some margin from the diamond edges.
    let radius = 0.72;
    let thickness = 0.06;

    let dist = length(in.uv);
    let ring = abs(dist - radius);

    // Smooth anti-aliased ring
    let aa = fwidth(dist) * 1.5;
    let mask = 1.0 - smoothstep(thickness - aa, thickness + aa, ring);

    if mask < 0.01 {
        discard;
    }

    return vec4<f32>(1.0, 1.0, 1.0, mask * in.alpha);
}
