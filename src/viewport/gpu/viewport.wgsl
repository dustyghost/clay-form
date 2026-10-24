// Shared packed inputs retain CPU mesh indexing without optional primitive-index features.
struct Uniforms {
 right: vec4f, up: vec4f, view: vec4f, projection: vec4f, perspective: vec4f,
 brush_center: vec4f, brush_normal: vec4f, brush_color: vec4f, brush_shape: vec4f,
 guide_center: vec4f, flags: vec4f, light_position: vec4f, light_colour: vec4f, material_info: vec4f,
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
 @location(6) material: vec2f,
 @location(7) bary: vec3f, @location(8) @interpolate(flat) ids: vec3u,
 @location(3) mask: f32, @location(4) is_active: f32, @location(5) @interpolate(flat) group: vec4f,
}
@vertex fn surface_vertex(@builtin(vertex_index) index: u32) -> Surface {
 let i=indices[index]*u32(u.material_info.x);
 let p=vec3f(vertices[i],vertices[i+1u],vertices[i+2u]);
 let mask=vertices[i+9u];
 var out: Surface;
 let face=index/3u*3u; out.ids=vec3u(indices[face],indices[face+1u],indices[face+2u]);
 out.bary=vec3f(select(0.,1.,index%3u==0u),select(0.,1.,index%3u==1u),select(0.,1.,index%3u==2u));
 out.clip=project(p); out.position=p;
 out.normal=vec3f(vertices[i+3u],vertices[i+4u],vertices[i+5u]);
 out.color=vec3f(vertices[i+6u],vertices[i+7u],vertices[i+8u]);
 out.material=vec2f(vertices[i+10u],vertices[i+11u]);
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
fn display_rgb(linear: vec3f) -> vec3f {
 let c=max(linear,vec3f(0.));
 return select(1.055*pow(c,vec3f(1./2.4))-0.055,12.92*c,c<=vec3f(0.0031308));
}
// Isotropic GGX metallic/roughness, dielectric IOR 1.5. Adjustable viewport illumination.
fn material_light(base: vec3f, roughness: f32, metallic: f32, ior: f32, specular: f32, subsurface: f32, radius: vec3f, scale: f32, n: vec3f, eye: vec3f, light: vec3f, radiance: vec3f) -> vec3f {
 let h=normalize(light+eye);
 let nl=max(dot(n,light),0.); let nv=max(dot(n,eye),0.001);
 let nh=max(dot(n,h),0.); let vh=max(dot(eye,h),0.);
 let a=pow(max(roughness,0.045),2.); let a2=a*a;
 let d=a2/(3.14159265*pow(nh*nh*(a2-1.)+1.,2.));
 let gv=2.*nv/(nv+sqrt(a2+(1.-a2)*nv*nv));
 let gl=2.*nl/max(nl+sqrt(a2+(1.-a2)*nl*nl),0.00001);
 let f0=mix(vec3f(pow((ior-1.)/(ior+1.),2.)*specular*2.),base,metallic);
 let f=f0+(vec3f(1.)-f0)*pow(1.-vh,5.);
 let spec=d*gv*gl*f/max(4.*nv*max(nl,0.001),0.00001);
 let diffuse=(vec3f(1.)-f)*(1.-metallic)*base/3.14159265;
 // Broad neutral fill keeps dark sides and metals readable; no scene reflections.
 let ambient=base*(0.10*(1.-metallic)+0.16*metallic);
 let wrap=clamp(radius*scale*10.,vec3f(0.),vec3f(1.));
 let scattered=max((vec3f(dot(n,light))+wrap)/(vec3f(1.)+wrap),vec3f(0.))/(vec3f(1.)+wrap);
 let diffuse_light=mix(vec3f(nl),scattered,subsurface);
 return ambient*u.light_colour.w+(diffuse*diffuse_light+spec*nl)*radiance;
}
fn channel(v: Surface, offset: u32) -> f32 {
 if u.material_info.x==12. {if offset==12u {return 1.5;} if offset==13u {return 0.5;} return 0.;}
 let stride=u32(u.material_info.x);
 return dot(vec3f(vertices[v.ids.x*stride+offset],vertices[v.ids.y*stride+offset],vertices[v.ids.z*stride+offset]),v.bary);
}
fn channels(v: Surface, offset: u32) -> vec3f { return vec3f(channel(v,offset),channel(v,offset+1u),channel(v,offset+2u)); }
@fragment fn surface_fragment(v: Surface) -> @location(0) vec4f {
 let delta=u.light_position.xyz-v.position*u.light_position.w;
 let light=delta/max(length(delta),0.000001);
 let attenuation=select(1.,16./max(dot(delta,delta),0.01),u.light_position.w==1.);
 let shade=0.25+0.75*max(dot(normalize(v.normal)*select(1.,-1.,u.flags.z==1.),light),0.);
 let normal=normalize(v.normal)*select(1.,-1.,u.flags.z==1.);
 let eye=normalize(u.view.xyz-v.position*u.perspective.x);
 var linear=material_light(v.color,clamp(v.material.x,0.,1.),clamp(v.material.y,0.,1.),channel(v,12u),channel(v,13u),channel(v,18u),channels(v,19u),channel(v,22u),normal,eye,light,u.light_colour.xyz*attenuation)+channels(v,14u)*channel(v,17u);
 for(var i=0u;i<3u;i++) {
  let start=23u+i*18u; let weight=channel(v,start+16u); let add=channel(v,start+17u);
  if weight>0. {
   let layer=material_light(channels(v,start),channel(v,start+3u),channel(v,start+4u),channel(v,start+5u),channel(v,start+6u),channel(v,start+11u),channels(v,start+12u),channel(v,start+15u),normal,eye,light,u.light_colour.xyz*attenuation)+channels(v,start+7u)*channel(v,start+10u);
   linear=linear*(1.-weight*(1.-add))+layer*weight;
  }
 }
 var rgb=display_rgb(linear);
 if u.flags.x==1. && v.group.a>0.5 { rgb=mix(rgb,v.group.rgb*shade,mix(0.5,1.,v.is_active)); }
 rgb*=1.-v.mask*0.75;
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
