// City instanced rendering shader — textured city sprites
// Renders city sprites from a PCX sprite sheet atlas

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

@group(1) @binding(0) var city_texture: texture_2d<f32>;
@group(1) @binding(1) var city_sampler: sampler;

struct CityInstance {
    @location(0) position: vec2<f32>,  // tile position
    @location(1) color: vec3<f32>,     // player color
    @location(2) population: f32,      // for sizing
    @location(3) atlas_rect: vec4<f32>, // [u, v, w, h] in atlas UV space
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) local_uv: vec2<f32>,
    @location(2) atlas_rect: vec4<f32>,
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
    instance: CityInstance,
) -> VertexOutput {
    let local = QUAD_VERTICES[vertex_index];

    // Apply y-axis wrapping adjustment if active
    var adj_y = instance.position.y;
    if (camera.wrap_y > 0.5) {
        adj_y = instance.position.y - camera.map_height * floor((instance.position.x + instance.position.y) / camera.map_height);
    }

    // Compute iso center of tile
    let tile_pos = vec2<f32>(instance.position.x, adj_y) + vec2<f32>(0.5, 0.5);
    let iso_center = vec2<f32>(
        tile_pos.x - tile_pos.y,
        (tile_pos.x + tile_pos.y) * 0.5
    );

    // Pixel-perfect sizing: 64 px per iso unit matches terrain tiles (128x64 px = 2x1 iso)
    let sprite_w = 167.0 / 64.0;  // 2.609375 iso units
    let sprite_h = 95.0 / 64.0;   // 1.484375 iso units

    // Position sprite centered horizontally, sitting on the tile
    // Offset downward so the sprite base aligns with tile center
    let world_pos = iso_center + vec2<f32>(
        (local.x - 0.5) * sprite_w,
        (local.y - 1.0) * sprite_h + sprite_h * 0.55
    );

    let offset = (world_pos - camera.center) * camera.zoom;
    let clip_x = offset.x / (camera.viewport_w * 0.5);
    let clip_y = -offset.y / (camera.viewport_h * 0.5);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.color = instance.color;
    out.local_uv = local;
    out.atlas_rect = instance.atlas_rect;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // If atlas rect is set, sample the texture
    if in.atlas_rect.z > 0.0 {
        let atlas_uv = in.atlas_rect.xy + in.local_uv * in.atlas_rect.zw;
        let texel = textureSample(city_texture, city_sampler, atlas_uv);
        // Discard transparent pixels
        if texel.a < 0.5 {
            discard;
        }
        return texel;
    }

    // Fallback: colored square (same as old behavior)
    let uv = in.local_uv;
    let outer_border = 0.06;
    if uv.x < outer_border || uv.x > (1.0 - outer_border) || uv.y < outer_border || uv.y > (1.0 - outer_border) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let inner_border = 0.15;
    if uv.x < inner_border || uv.x > (1.0 - inner_border) || uv.y < inner_border || uv.y > (1.0 - inner_border) {
        return vec4<f32>(in.color, 1.0);
    }
    let inner_color = mix(in.color, vec3<f32>(1.0, 1.0, 1.0), 0.3);
    return vec4<f32>(inner_color, 1.0);
}
