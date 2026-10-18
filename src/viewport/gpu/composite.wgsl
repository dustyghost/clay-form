@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vertex(@builtin(vertex_index) i: u32) -> Vertex {
 let uv=vec2f(f32((i<<1u)&2u),f32(i&2u));
 var out: Vertex; out.position=vec4f(uv*vec2f(2.,-2.)+vec2f(-1.,1.),0.,1.); out.uv=uv; return out;
}
@fragment fn fragment(v: Vertex) -> @location(0) vec4f { return textureSample(image,image_sampler,v.uv); }
// The old viewport shades in display RGB, like egui. Decode before an sRGB attachment
// encodes the output, so the surface format does not change material appearance.
@fragment fn fragment_srgb(v: Vertex) -> @location(0) vec4f {
 let c=textureSample(image,image_sampler,v.uv);
 let linear=select(pow((c.rgb+vec3f(0.055))/1.055,vec3f(2.4)),c.rgb/12.92,c.rgb<=vec3f(0.04045));
 return vec4f(linear,c.a);
}
