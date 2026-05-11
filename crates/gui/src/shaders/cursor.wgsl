struct CursorUniforms {
    pos: vec2<f32>,
    size: vec2<f32>,
    scale: f32,
    _pad: f32,
    screen: vec2<f32>,
}

@group(0) @binding(0) var<uniform> u: CursorUniforms;
@group(0) @binding(1) var t: texture_2d<f32>;
@group(0) @binding(2) var s: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let x = f32(i32(vi & 1u));
    let y = f32(i32((vi >> 1u) & 1u));
    let scaled_size = u.size * u.scale;
    let screen_pos = vec2<f32>(u.pos.x + x * scaled_size.x, u.pos.y + y * scaled_size.y);
    let ndc = vec2<f32>(
        (screen_pos.x / u.screen.x) * 2.0 - 1.0,
        1.0 - (screen_pos.y / u.screen.y) * 2.0,
    );
    return VertexOutput(vec4<f32>(ndc, 0.0, 1.0), vec2<f32>(x, y));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t, s, in.uv);
}
