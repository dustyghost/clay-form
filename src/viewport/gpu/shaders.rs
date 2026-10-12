//! Desktop OpenGL 3.3 shaders, with matching CPU brush-alpha semantics.
pub(super) const SURFACE_VERTEX: &str = r#"#version 330 core
layout(location=0) in vec3 a_position;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec3 a_color;
layout(location=3) in float a_mask;
uniform vec3 u_right, u_up, u_view;
uniform vec4 u_projection;
uniform float u_depth;
out vec3 v_normal, v_color, v_position;
out float v_mask, v_active;
void main() {
 gl_Position = vec4(dot(a_position,u_right)*u_projection.x+u_projection.z,
 dot(a_position,u_up)*u_projection.y+u_projection.w,-dot(a_position,u_view)*u_depth,1.0);
 v_normal=a_normal; v_color=a_color; v_mask=max(a_mask,0.0); v_active=step(0.0,a_mask); v_position=a_position;
}"#;
pub(super) const SURFACE_FRAGMENT: &str = r#"#version 330 core
in vec3 v_normal, v_color, v_position;
in float v_mask, v_active;
uniform vec3 u_right, u_up, u_view;
uniform int u_brush_enabled, u_alpha_kind, u_guide_count, u_backface;
uniform float u_surface_opacity;
uniform vec3 u_brush_center, u_brush_normal, u_brush_color, u_guide_center;
uniform float u_brush_radius, u_guide_opacity;
uniform vec4 u_brush_shape;
uniform sampler2D u_alpha_image, u_poly_colors;
uniform int u_show_polygroups;
uniform vec3 u_guide_normals[32], u_guide_colors[32];
uniform vec4 u_guide_sides[32];
out vec4 color;
float alpha_weight(vec2 q) {
 q=vec2(q.x*u_brush_shape.w-q.y*u_brush_shape.z,q.x*u_brush_shape.z+q.y*u_brush_shape.w);
 float r=length(q);
 if(r>=1.0) return 0.0;
 float fade=pow(clamp((1.0-r)/max(1.0-u_brush_shape.x,0.0001),0.0,1.0),u_brush_shape.y);
 if(u_alpha_kind==0) return fade;
 if(u_alpha_kind==1) return 1.0;
 if(u_alpha_kind==2) return max(abs(q.x),abs(q.y))<0.7 ? 1.0 : 0.0;
 vec2 size=vec2(textureSize(u_alpha_image,0));
 vec2 uv=(vec2(q.x+1.0,1.0-q.y)*0.5*(size-1.0)+0.5)/size;
 return texture(u_alpha_image,uv).r*fade;
}
void main() {
 vec3 light=normalize(u_view+u_up*0.7-u_right*0.5);
 float shade=0.25+0.75*max(dot(normalize(v_normal)*(u_backface==1 ? -1.0 : 1.0),light),0.0);
 vec3 base=v_color;
 if(u_show_polygroups==1) {
   int width=textureSize(u_poly_colors,0).x;
   vec4 group=texelFetch(u_poly_colors,ivec2(gl_PrimitiveID%width,gl_PrimitiveID/width),0);
   if(group.a>0.5) base=mix(v_color,group.rgb,mix(0.5,1.0,v_active));
 }
 vec3 rgb=base*shade*(1.0-v_mask*0.75);
 if(u_backface==1) { color=vec4(rgb,u_surface_opacity); return; }
 if(v_active>0.5) {
   for(int i=0;i<u_guide_count;i++) {
     vec3 offset=v_position-u_guide_center;
     float distance=dot(offset,u_guide_normals[i]);
     float width=fwidth(distance);
     if(width>0.000001 && (u_guide_sides[i].w<0.5 || dot(offset,u_guide_sides[i].xyz)>=0.0)) {
       float guide=1.0-smoothstep(0.3,1.0,abs(distance)/width);
       rgb=mix(rgb,u_guide_colors[i],guide*u_guide_opacity);
     }
   }
   vec3 offset=v_position-u_brush_center;
   if(u_brush_enabled==1 && length(offset)<=u_brush_radius*1.5 && dot(normalize(v_normal),u_brush_normal)>=0.1) {
     float weight=alpha_weight(vec2(dot(offset,u_right),dot(offset,u_up))/u_brush_radius);
     rgb=mix(rgb,u_brush_color,weight*(90.0/255.0));
   }
 }
 color=vec4(rgb,1.0);
}"#;
pub(super) const LINE_VERTEX: &str = r#"#version 330 core
layout(location=0) in vec3 a_position;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec3 a_other_normal;
layout(location=3) in vec3 a_edge_color;
uniform vec3 u_right, u_up, u_view;
uniform vec4 u_projection;
uniform float u_depth;
out float v_front;
out vec3 v_edge_color;
void main() {
 gl_Position=vec4(dot(a_position,u_right)*u_projection.x+u_projection.z,
 dot(a_position,u_up)*u_projection.y+u_projection.w,-dot(a_position,u_view)*u_depth,1.0);
 v_front=max(dot(a_normal,u_view),dot(a_other_normal,u_view));
 v_edge_color=a_edge_color;
}"#;
pub(super) const LINE_FRAGMENT: &str = r#"#version 330 core
in float v_front;
in vec3 v_edge_color;
uniform int u_transparent;
out vec4 color;
void main() {
 if(u_transparent==0 && v_front<=0.0) discard;
 if(v_edge_color.r>=0.0) { color=vec4(v_edge_color,1.0); return; }
 color=u_transparent==1 ? vec4(170.0/255.0,185.0/255.0,200.0/255.0,1.0) : vec4(0.0,0.0,0.0,0.43);
}"#;
