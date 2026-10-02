// Screen-space textured quad shader for UI boxes.
// Bind group 0: texture + sampler (the UI atlas).

struct UITexturedRectInstance {
    @location(0) rect: vec4<f32>,       // x, y, w, h in pixels
    @location(1) atlas_rect: vec4<f32>, // u, v, w, h in normalized UV
    @location(2) viewport: vec2<f32>,   // viewport_w, viewport_h
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var t_atlas: texture_2d<f32>;
@group(0) @binding(1)
var s_atlas: sampler;

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
    instance: UITexturedRectInstance,
) -> VertexOutput {
    let local = QUAD_VERTICES[vertex_index];
    let px = instance.rect.x + local.x * instance.rect.z;
    let py = instance.rect.y + local.y * instance.rect.w;
    let ndc_x = px / instance.viewport.x * 2.0 - 1.0;
    let ndc_y = 1.0 - py / instance.viewport.y * 2.0;

    // Compute UV within the atlas sub-rect
    let uv = vec2<f32>(
        instance.atlas_rect.x + local.x * instance.atlas_rect.z,
        instance.atlas_rect.y + local.y * instance.atlas_rect.w,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_atlas, s_atlas, in.uv);
}
