// Shared packed inputs retain CPU mesh indexing without optional primitive-index features.
struct Uniforms {
 right: vec4f, up: vec4f, view: vec4f, projection: vec4f, perspective: vec4f,
 brush_center: vec4f, brush_normal: vec4f, brush_color: vec4f, brush_shape: vec4f,
 guide_center: vec4f, flags: vec4f,
 guide_normals: array<vec4f,32>, guide_sides: array<vec4f,32>, guide_colors: array<vec4f,32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var alpha_image: texture_2d<f32>;
@group(0) @binding(2) var<storage,read> vertices: array<f32>;
@group(0) @binding(3) var<storage,read> indices: array<u32>;
@group(0) @binding(4) var<storage,read> groups: array<vec4f>;
fn project(p: vec3f) -> vec4f {
 let z=dot(p,u.view.xyz);
 let w=1.-z*u.perspective.x;
 var depth=-z*u.perspective.w;
 if u.perspective.x>0. { depth=u.perspective.y*w+u.perspective.z; }
 // OpenGL [-w,w] depth becomes WebGPU [0,w]. Y stays up in clip space.
 return vec4f(dot(p,u.right.xyz)*u.projection.x+u.projection.z*w,
 dot(p,u.up.xyz)*u.projection.y+u.projection.w*w,(depth+w)*0.5,w);
}
struct Surface {
 @builtin(position) clip: vec4f,
 @location(0) normal: vec3f, @location(1) color: vec3f, @location(2) position: vec3f,
 @location(3) mask: f32, @location(4) is_active: f32, @location(5) @interpolate(flat) group: vec4f,
}
@vertex fn surface_vertex(@builtin(vertex_index) index: u32) -> Surface {
 let i=indices[index]*10u;
 let p=vec3f(vertices[i],vertices[i+1u],vertices[i+2u]);
 let mask=vertices[i+9u];
 var out: Surface;
 out.clip=project(p); out.position=p;
 out.normal=vec3f(vertices[i+3u],vertices[i+4u],vertices[i+5u]);
 out.color=vec3f(vertices[i+6u],vertices[i+7u],vertices[i+8u]);
 out.mask=max(mask,0.); out.is_active=step(0.,mask); out.group=groups[index/3u];
 return out;
}
// Bilinear R32Float sampling without requiring FLOAT32_FILTERABLE.
fn image_weight(q: vec2f) -> f32 {
 let size=vec2i(textureDimensions(alpha_image));
 let xy=vec2f(q.x+1.,1.-q.y)*0.5*vec2f(size-vec2i(1));
 let lo=vec2i(floor(xy)); let hi=min(lo+vec2i(1),size-vec2i(1)); let t=fract(xy);
 let a=textureLoad(alpha_image,lo,0).r; let b=textureLoad(alpha_image,vec2i(hi.x,lo.y),0).r;
 let c=textureLoad(alpha_image,vec2i(lo.x,hi.y),0).r; let d=textureLoad(alpha_image,hi,0).r;
 return mix(mix(a,b,t.x),mix(c,d,t.x),t.y);
}
fn alpha_weight(input: vec2f) -> f32 {
 let q=vec2f(input.x*u.brush_shape.w-input.y*u.brush_shape.z,input.x*u.brush_shape.z+input.y*u.brush_shape.w);
 let r=length(q);
 if r>=1. { return 0.; }
 let fade=pow(clamp((1.-r)/max(1.-u.brush_shape.x,0.0001),0.,1.),u.brush_shape.y);
 if u.brush_color.w==0. { return fade; }
 if u.brush_color.w==1. { return 1.; }
 if u.brush_color.w==2. { return select(0.,1.,max(abs(q.x),abs(q.y))<0.7); }
 return image_weight(q)*fade;
}
@fragment fn surface_fragment(v: Surface) -> @location(0) vec4f {
 let light=normalize(u.view.xyz+u.up.xyz*0.7-u.right.xyz*0.5);
 let shade=0.25+0.75*max(dot(normalize(v.normal)*select(1.,-1.,u.flags.z==1.),light),0.);
 var base=v.color;
 if u.flags.x==1. && v.group.a>0.5 { base=mix(v.color,v.group.rgb,mix(0.5,1.,v.is_active)); }
 var rgb=base*shade*(1.-v.mask*0.75);
 // Derivatives must be evaluated uniformly, before per-fragment conditions.
 var widths: array<f32,32>;
 for(var i=0u;i<u32(u.right.w);i++) { widths[i]=fwidth(dot(v.position-u.guide_center.xyz,u.guide_normals[i].xyz)); }
 if u.flags.z==1. { return vec4f(rgb,u.flags.w); }
 if v.is_active>0.5 {
  for(var i=0u;i<u32(u.right.w);i++) {
   let offset=v.position-u.guide_center.xyz;
   let distance=dot(offset,u.guide_normals[i].xyz); let width=widths[i];
   if u.guide_normals[i].w>0. && width>0.000001 && (u.guide_sides[i].w<0.5 || dot(offset,u.guide_sides[i].xyz)>=0.) {
    let guide=1.-smoothstep(0.3,1.,abs(distance)/width);
    rgb=mix(rgb,u.guide_colors[i].xyz,guide*u.guide_center.w);
   }
  }
  let offset=v.position-u.brush_center.xyz;
  if u.brush_normal.w==1. && length(offset)<=u.brush_center.w*1.5 && dot(normalize(v.normal),u.brush_normal.xyz)>=0.1 {
   let weight=alpha_weight(vec2f(dot(offset,u.right.xyz),dot(offset,u.up.xyz))/u.brush_center.w);
   // Lift faint image details for placement without changing sculpt strength.
   let opacity=select(weight*(90./255.),sqrt(clamp(weight,0.,1.))*0.75,u.brush_color.w==3.);
   rgb=mix(rgb,u.brush_color.xyz,opacity);
  }
 }
 return vec4f(rgb,1.);
}
struct Line { @builtin(position) clip: vec4f, @location(0) front: f32, @location(1) color: vec3f }
@vertex fn line_vertex(@location(0) p: vec3f, @location(1) normal: vec3f, @location(2) other: vec3f, @location(3) color: vec3f) -> Line {
 var out: Line; out.clip=project(p);
 let toward_eye=u.view.xyz-p*u.perspective.x;
 out.front=max(dot(normal,toward_eye),dot(other,toward_eye)); out.color=color; return out;
}
@fragment fn line_fragment(v: Line) -> @location(0) vec4f {
 if u.flags.y==0. && v.front<=0. { discard; }
 if v.color.r>=0. { return vec4f(v.color,1.); }
 if u.flags.y==1. { return vec4f(170./255.,185./255.,200./255.,1.); }
 return vec4f(0.,0.,0.,0.43);
}
