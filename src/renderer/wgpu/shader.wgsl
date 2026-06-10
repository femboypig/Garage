struct Globals {
    screen_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var t_diffuse: texture_2d<f32>;
@group(0) @binding(2) var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    
    // Convert screen coordinates (pixels, top-left 0,0) to NDC (-1 to 1, Y up)
    let ndc_x = (model.position.x / globals.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (model.position.y / globals.screen_size.y) * 2.0;
    
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.tex_coords = model.tex_coords;
    out.color = model.color;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    // The texture contains the glyph alpha mask in the red channel.
    // For non-text elements (solid panels), we sample from a solid white pixel (value 1.0).
    return vec4<f32>(in.color.rgb, in.color.a * tex_color.r);
}
