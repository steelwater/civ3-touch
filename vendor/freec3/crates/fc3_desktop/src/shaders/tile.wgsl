// Tile instanced rendering shader — isometric diamond tiles
// Renders terrain tiles with fog of war overlay, ownership tint, and grid lines
// Supports optional texture atlas sampling via atlas_rect, overlay_rect, and fog_rect

struct CameraUniform {
    center: vec2<f32>,
    zoom: f32,
    aspect: f32,
    viewport_w: f32,
    viewport_h: f32,
    map_height: f32,
    wrap_y: f32,     // 0.0 = false, 1.0 = true
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0) var terrain_texture: texture_2d<f32>;
@group(1) @binding(1) var terrain_sampler: sampler;

struct TileInstance {
    @location(0) position: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) border_color: vec3<f32>,
    @location(3) border_mask: f32,     // 4-bit bitmask: N=1, E=2, S=4, W=8
    @location(4) atlas_rect: vec4<f32>, // [u, v, w, h] in atlas UV space; all-zero = color-only
    @location(5) overlay_rect: vec4<f32>, // second atlas layer; all-zero = none
    @location(6) veg_rect: vec4<f32>,  // vegetation overlay (forest/jungle); all-zero = none
    @location(7) fog_rect: vec4<f32>,  // fog of war overlay; all-zero = none
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) local_uv: vec2<f32>,
    @location(2) border_color: vec3<f32>,
    @location(3) border_mask: f32,
    @location(4) atlas_rect: vec4<f32>,
    @location(5) overlay_rect: vec4<f32>,
    @location(6) veg_rect: vec4<f32>,
    @location(7) fog_rect: vec4<f32>,
};

// Diamond vertices relative to iso center (width=2, height=1 in iso space)
var<private> DIAMOND_VERTS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, -0.5),  // top
    vec2<f32>(1.0, 0.0),   // right
    vec2<f32>(0.0, 0.5),   // bottom
    vec2<f32>(0.0, -0.5),  // top
    vec2<f32>(0.0, 0.5),   // bottom
    vec2<f32>(-1.0, 0.0),  // left
);

// UVs for diamond-in-rectangle texture mapping
var<private> DIAMOND_UVS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.5, 0.0),   // top
    vec2<f32>(1.0, 0.5),   // right
    vec2<f32>(0.5, 1.0),   // bottom
    vec2<f32>(0.5, 0.0),   // top
    vec2<f32>(0.5, 1.0),   // bottom
    vec2<f32>(0.0, 0.5),   // left
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: TileInstance,
) -> VertexOutput {
    // Apply y-axis wrapping adjustment if active
    var adj_y = instance.position.y;
    if (camera.wrap_y > 0.5) {
        adj_y = instance.position.y - camera.map_height * floor((instance.position.x + instance.position.y) / camera.map_height);
    }

    // Iso center of tile: logical_to_iso(col + 0.5, row + 0.5)
    let iso_center = vec2<f32>(
        instance.position.x - adj_y,
        (instance.position.x + adj_y + 1.0) * 0.5
    );
    let iso_pos = iso_center + DIAMOND_VERTS[vertex_index];

    // Camera transform: center on camera, scale by zoom
    let offset = (iso_pos - camera.center) * camera.zoom;
    let clip_x = offset.x / (camera.viewport_w * 0.5);
    let clip_y = -offset.y / (camera.viewport_h * 0.5);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.color = instance.color;
    out.local_uv = DIAMOND_UVS[vertex_index];
    out.border_color = instance.border_color;
    out.border_mask = instance.border_mask;
    out.atlas_rect = instance.atlas_rect;
    out.overlay_rect = instance.overlay_rect;
    out.veg_rect = instance.veg_rect;
    out.fog_rect = instance.fog_rect;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Determine base color: sample texture if atlas_rect is set, otherwise use instance color
    var color: vec3<f32>;
    if in.atlas_rect.z > 0.0 {
        let atlas_uv = in.atlas_rect.xy + in.local_uv * in.atlas_rect.zw;
        let texel = textureSample(terrain_texture, terrain_sampler, atlas_uv);
        if texel.a > 0.5 {
            color = texel.rgb;
        } else {
            color = in.color;
        }
    } else {
        color = in.color;
    }

    // Overlay layer (e.g. hills): blend on top if present
    if in.overlay_rect.z > 0.0 {
        let overlay_uv = in.overlay_rect.xy + in.local_uv * in.overlay_rect.zw;
        let overlay = textureSample(terrain_texture, terrain_sampler, overlay_uv);
        if overlay.a > 0.5 {
            color = overlay.rgb;
        }
    }

    // Vegetation overlay (forest/jungle): blend on top of terrain+hills
    if in.veg_rect.z > 0.0 {
        let veg_uv = in.veg_rect.xy + in.local_uv * in.veg_rect.zw;
        let veg = textureSample(terrain_texture, terrain_sampler, veg_uv);
        if veg.a > 0.5 {
            color = veg.rgb;
        }
    }

    let uv = in.local_uv;
    let d_sum = uv.x + uv.y;
    let d_diff = uv.x - uv.y;

    // Distance to each diamond edge (positive = inside)
    let near_tl = d_sum - 0.5;       // top-left edge (W)
    let near_tr = 0.5 - d_diff;      // top-right edge (N)
    let near_br = 1.5 - d_sum;       // bottom-right edge (E)
    let near_bl = d_diff + 0.5;      // bottom-left edge (S)

    // Grid lines along diamond edges
    let edge_width = 0.04;
    let dist_to_edge = min(min(near_tl, near_tr), min(near_br, near_bl));
    if dist_to_edge < edge_width {
        color = mix(color, vec3<f32>(0.15, 0.15, 0.15), 0.5);
    }

    // Territory borders on specific diamond edges
    let border_width = 0.08;
    let mask = u32(in.border_mask);
    if (mask & 1u) != 0u && near_tr < border_width { color = in.border_color; }  // N
    if (mask & 2u) != 0u && near_br < border_width { color = in.border_color; }  // E
    if (mask & 4u) != 0u && near_bl < border_width { color = in.border_color; }  // S
    if (mask & 8u) != 0u && near_tl < border_width { color = in.border_color; }  // W

    // Fog of war (applied last so it darkens everything including grid/borders).
    // Multiply filter: black (0,0,0) fully hides, white (1,1,1) no change, gray dims.
    // Fog tiles are cell-centered, so offset UV by half a tile to align the fog
    // sprite center with the grid cell (at the diamond's top vertex).
    if in.fog_rect.z > 0.0 {
        let fog_uv = in.fog_rect.xy + in.local_uv * in.fog_rect.zw;
        let fog = textureSample(terrain_texture, terrain_sampler, fog_uv);
        if fog.a > 0.5 {
            color = color * fog.rgb;
        }
    }

    return vec4<f32>(color, 1.0);
}
