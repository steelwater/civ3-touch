// Unit instanced rendering shader — isometric projection
// Renders textured unit sprites from a FLIC atlas, with SDF fallback

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

@group(1) @binding(0) var unit_texture: texture_2d<f32>;
@group(1) @binding(1) var unit_sampler: sampler;

struct UnitInstance {
    @location(0) position: vec2<f32>,  // tile center
    @location(1) color: vec3<f32>,     // player color
    @location(2) shape: f32,           // 0=circle, 1=diamond
    @location(3) hp_frac: f32,         // 0.0-1.0 HP fraction
    @location(4) stack_count: f32,     // 1.0=single, 2.0-5.0=stacked
    @location(5) offset: vec2<f32>,    // stacking offset
    @location(6) atlas_rect: vec4<f32>, // [u, v, w, h]; z==0 means SDF fallback
    @location(7) sprite_size: vec2<f32>, // iso-space [width, height]
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) local_uv: vec2<f32>,
    @location(2) shape: f32,
    @location(3) hp_frac: f32,
    @location(4) atlas_rect: vec4<f32>,
    @location(5) stack_count: f32,
    @location(6) sprite_size: vec2<f32>,
};

var<private> QUAD_VERTICES: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: UnitInstance,
) -> VertexOutput {
    let local = QUAD_VERTICES[vertex_index];

    // Apply y-axis wrapping adjustment if active
    var adj_y = instance.position.y;
    if (camera.wrap_y > 0.5) {
        adj_y = instance.position.y - camera.map_height * floor((instance.position.x + instance.position.y) / camera.map_height);
    }

    // Compute iso center of the tile
    let tile_pos = vec2<f32>(instance.position.x, adj_y) + instance.offset + vec2<f32>(0.5, 0.5);
    let iso_center = vec2<f32>(
        tile_pos.x - tile_pos.y,
        (tile_pos.x + tile_pos.y) * 0.5
    );

    var half_w: f32;
    var half_h: f32;
    var vert_offset: f32;

    if instance.atlas_rect.z > 0.0 {
        // Textured sprite: use pixel-perfect sizing from sprite_size
        // sprite_size is in iso units (pixel dimensions / 64.0, matching terrain density)
        half_w = instance.sprite_size.x * 0.5;
        half_h = instance.sprite_size.y * 0.5;
        // Anchor bottom of sprite below tile center — sprite grows upward
        // local.y=1 (feet) → +0.25, local.y=0 (head) → +0.25 - 2*half_h
        // The +0.25 nudge places feet at the lower quarter of the tile diamond
        vert_offset = -half_h * 2.0 + 0.25;
    } else {
        // SDF fallback: original fixed-size quad
        let unit_size = 0.6;
        half_w = unit_size;
        half_h = unit_size * 0.5;
        vert_offset = 0.0;
    }

    let world_pos = iso_center + vec2<f32>(
        (local.x - 0.5) * half_w * 2.0,
        local.y * half_h * 2.0 + vert_offset
    );

    let offset = (world_pos - camera.center) * camera.zoom;
    let clip_x = offset.x / (camera.viewport_w * 0.5);
    let clip_y = -offset.y / (camera.viewport_h * 0.5);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.color = instance.color;
    out.local_uv = local;
    out.shape = instance.shape;
    out.hp_frac = instance.hp_frac;
    out.atlas_rect = instance.atlas_rect;
    out.stack_count = instance.stack_count;
    out.sprite_size = instance.sprite_size;
    return out;
}

// Fixed iso-space dimensions for HP bar and stack indicator
const BAR_W: f32 = 0.06;       // bar width in iso units
const BAR_H: f32 = 0.5;        // bar height in iso units
const BAR_MARGIN: f32 = 0.02;  // gap from right edge of sprite
const BAR_BOTTOM: f32 = 0.08;  // gap from bottom (feet) of sprite
const STACK_H: f32 = 0.12;     // stack indicator region height
const STACK_GAP: f32 = 0.02;   // gap between HP bar bottom and stack indicator top

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Textured path: sample from atlas
    if in.atlas_rect.z > 0.0 {
        // Convert UV to iso-space coordinates measured from sprite edges
        let iso_x = in.local_uv.x * in.sprite_size.x;
        let iso_y = in.local_uv.y * in.sprite_size.y;
        let dist_from_right = in.sprite_size.x - iso_x;
        let dist_from_bottom = in.sprite_size.y - iso_y;

        // HP bar: fixed-size, anchored to right edge near bottom
        let bar_right = BAR_MARGIN;
        let bar_left = BAR_MARGIN + BAR_W;
        let bar_bot = BAR_BOTTOM;
        let bar_top = BAR_BOTTOM + BAR_H;

        if dist_from_right >= bar_right && dist_from_right < bar_left
           && dist_from_bottom >= bar_bot && dist_from_bottom < bar_top {
            // Bar fills from bottom to top
            let bar_frac = (dist_from_bottom - bar_bot) / BAR_H;
            if bar_frac <= in.hp_frac {
                let hp_color = mix(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), in.hp_frac);
                return vec4<f32>(hp_color, 1.0);
            } else {
                return vec4<f32>(0.2, 0.2, 0.2, 1.0);
            }
        }

        // Stack indicator lines: same column, just below HP bar
        if in.stack_count > 1.5
           && dist_from_right >= bar_right && dist_from_right < bar_left
           && dist_from_bottom >= (bar_bot - STACK_GAP - STACK_H)
           && dist_from_bottom < (bar_bot - STACK_GAP) {
            let n = i32(in.stack_count);
            let region_bot = bar_bot - STACK_GAP - STACK_H;
            let local_y = dist_from_bottom - region_bot;
            let line_h = STACK_H / f32(n * 2 - 1);
            let slot = i32(local_y / line_h);
            if slot % 2 == 0 && slot / 2 < n {
                return vec4<f32>(1.0, 1.0, 1.0, 1.0);
            }
        }

        let atlas_uv = in.atlas_rect.xy + in.local_uv * in.atlas_rect.zw;
        let texel = textureSample(unit_texture, unit_sampler, atlas_uv);

        // Discard transparent pixels
        if texel.a < 0.1 {
            discard;
        }

        return texel;
    }

    // SDF fallback (original rendering)
    let uv = in.local_uv * 2.0 - 1.0; // [-1, 1]

    var dist: f32;
    if in.shape < 0.5 {
        // Circle SDF
        dist = length(uv) - 0.75;
    } else {
        // Diamond SDF
        let d = abs(uv);
        dist = (d.x + d.y) - 0.75;
    }

    // Discard outside shape
    if dist > 0.05 {
        discard;
    }

    // Black outline
    if dist > -0.08 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    var color = in.color;

    // HP bar at bottom (below the main shape)
    let bar_y = in.local_uv.y;
    if bar_y > 0.85 && in.local_uv.x > 0.15 && in.local_uv.x < 0.85 {
        let bar_frac = (in.local_uv.x - 0.15) / 0.7;
        if bar_frac <= in.hp_frac {
            let hp_color = mix(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), in.hp_frac);
            return vec4<f32>(hp_color, 1.0);
        } else {
            return vec4<f32>(0.2, 0.2, 0.2, 1.0);
        }
    }

    // Stack indicator lines for SDF path (below HP bar, right side)
    if in.stack_count > 1.5 && in.local_uv.y > 0.92 && in.local_uv.x > 0.6 && in.local_uv.x < 0.85 {
        let n = i32(in.stack_count);
        let region_top = 0.92;
        let region_bot = 0.99;
        let region_h = region_bot - region_top;
        let line_h = region_h / f32(n * 2 - 1);
        let local_y = in.local_uv.y - region_top;
        let slot = i32(local_y / line_h);
        if slot % 2 == 0 && slot / 2 < n {
            return vec4<f32>(1.0, 1.0, 1.0, 1.0);
        }
    }

    return vec4<f32>(color, 1.0);
}
