// Screen-space colored quad shader for UI rectangles.
// No bind groups needed — viewport dims embedded per-instance.

struct UIRectInstance {
    @location(0) rect: vec4<f32>,       // x, y, w, h in pixels
    @location(1) color: vec4<f32>,      // RGBA
    @location(2) viewport: vec2<f32>,   // viewport_w, viewport_h
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
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
    instance: UIRectInstance,
) -> VertexOutput {
    let local = QUAD_VERTICES[vertex_index];
    let px = instance.rect.x + local.x * instance.rect.z;
    let py = instance.rect.y + local.y * instance.rect.w;
    let ndc_x = px / instance.viewport.x * 2.0 - 1.0;
    let ndc_y = 1.0 - py / instance.viewport.y * 2.0;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
