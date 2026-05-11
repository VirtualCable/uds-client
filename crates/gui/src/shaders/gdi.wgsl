struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Triangle strip covering full clip space
    let x = f32(i32(vertex_index & 1u) * 2 - 1);
    let y = f32(i32((vertex_index >> 1u) & 1u) * -2 + 1);
    let uv = vec2<f32>(
        f32(i32(vertex_index & 1u)),
        f32(i32((vertex_index >> 1u) & 1u)),
    );

    return VertexOutput(
        vec4<f32>(x, y, 0.0, 1.0),
        uv,
    );
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.uv);
}
