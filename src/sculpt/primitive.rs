//! Editable primitive recipes and deterministic, welded triangle generation.
use super::{Mesh, Transform, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    f32::consts::{PI, TAU},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveKind {
    Plane,
    Cube,
    Circle,
    UvSphere,
    Icosphere,
    Cylinder,
    Cone,
    Torus,
    Grid,
    HumanHead,
}
impl PrimitiveKind {
    pub const ALL: [Self; 10] = [
        Self::Plane,
        Self::Cube,
        Self::Circle,
        Self::UvSphere,
        Self::Icosphere,
        Self::Cylinder,
        Self::Cone,
        Self::Torus,
        Self::Grid,
        Self::HumanHead,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Plane => "Plane",
            Self::Cube => "Cube",
            Self::Circle => "Circle",
            Self::UvSphere => "UV sphere",
            Self::Icosphere => "Icosphere",
            Self::Cylinder => "Cylinder",
            Self::Cone => "Cone",
            Self::Torus => "Torus",
            Self::Grid => "Grid",
            Self::HumanHead => "Human head",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadOptions {
    #[serde(default)]
    pub anatomy_version: u32,
    pub jaw_width: f32,
    pub jaw_height: f32,
    pub chin: f32,
    pub cheeks: f32,
    pub nose_length: f32,
    pub nose_width: f32,
    pub eye_spacing: f32,
    pub eye_depth: f32,
    pub brow: f32,
    pub ears: f32,
    pub eye_size: f32,
    pub eye_height: f32,
    pub mouth_width: f32,
    pub mouth_open: f32,
    pub lip_size: f32,
    pub upper_lid: f32,
    pub lower_lid: f32,
    #[serde(default)]
    pub neck: bool,
    pub neck_length: f32,
    pub neck_width: f32,
    pub neck_muscle: f32,
    pub throat: f32,
}
impl Default for HeadOptions {
    fn default() -> Self {
        Self {
            anatomy_version: 2,
            jaw_width: 0.5,
            jaw_height: 0.5,
            chin: 0.5,
            cheeks: 0.5,
            nose_length: 0.5,
            nose_width: 0.5,
            eye_spacing: 0.5,
            eye_depth: 0.5,
            brow: 0.5,
            ears: 0.5,
            eye_size: 0.5,
            eye_height: 0.5,
            mouth_width: 0.5,
            mouth_open: 0.,
            lip_size: 0.5,
            upper_lid: 0.,
            lower_lid: 0.,
            neck: true,
            neck_length: 0.5,
            neck_width: 0.5,
            neck_muscle: 0.5,
            throat: 0.35,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Primitive {
    pub kind: PrimitiveKind,
    pub size: Vec3,
    /// Cube spans per axis, plane/grid spans in X/Y, cylinder/cone height spans.
    pub divisions: [u32; 3],
    pub segments: u32,
    pub rings: u32,
    pub subdivisions: u32,
    pub radius: f32,
    pub top_radius: f32,
    pub tube_radius: f32,
    pub filled: bool,
    pub triangulated: bool,
    pub head: HeadOptions,
    pub center: Vec3,
    /// Affine placement preserves world/Action Line edits when regenerating.
    pub basis: [Vec3; 3],
}
impl Default for Primitive {
    fn default() -> Self {
        Self {
            kind: PrimitiveKind::Cube,
            size: Vec3(2., 2., 2.),
            divisions: [8; 3],
            segments: 48,
            rings: 24,
            subdivisions: 3,
            radius: 1.,
            top_radius: 0.,
            tube_radius: 0.25,
            filled: true,
            triangulated: false,
            head: HeadOptions::default(),
            center: Vec3::default(),
            basis: [Vec3(1., 0., 0.), Vec3(0., 1., 0.), Vec3(0., 0., 1.)],
        }
    }
}
impl Primitive {
    pub fn for_kind(kind: PrimitiveKind) -> Self {
        let mut result = Self {
            kind,
            ..Default::default()
        };
        if kind == PrimitiveKind::Plane {
            result.divisions = [1; 3];
        }
        if kind == PrimitiveKind::HumanHead {
            result.size = Vec3(1.8, 2.4, 1.9);
            result.segments = 192;
            result.rings = 128;
        }
        result
    }
    pub fn validate(&self) -> Result<(), String> {
        let finite = |p: Vec3| p.0.is_finite() && p.1.is_finite() && p.2.is_finite();
        if !finite(self.size)
            || [self.size.0, self.size.1, self.size.2]
                .iter()
                .any(|v| !(0.01..=100.).contains(v))
            || self.divisions.iter().any(|v| !(1..=128).contains(v))
            || !(3..=256).contains(&self.segments)
            || !(2..=128).contains(&self.rings)
            || !(1..=6).contains(&self.subdivisions)
            || !self.radius.is_finite()
            || !(0.01..=50.).contains(&self.radius)
            || !self.top_radius.is_finite()
            || !(0. ..=50.).contains(&self.top_radius)
            || !self.tube_radius.is_finite()
            || !(0.01..=25.).contains(&self.tube_radius)
            || (self.kind == PrimitiveKind::Torus
                && (self.tube_radius >= self.radius || self.rings < 3))
            || !finite(self.center)
            || self.basis.iter().any(|v| !finite(*v))
        {
            return Err("Invalid primitive dimensions or resolution".into());
        }
        let determinant = self.basis[0].cross(self.basis[1]).dot(self.basis[2]);
        if !determinant.is_finite() || determinant <= 1e-12 {
            return Err("Primitive placement is too small or invalid".into());
        }
        let h = &self.head;
        if h.anatomy_version > 2 {
            return Err("Unsupported head anatomy version".into());
        }
        if [
            h.jaw_width,
            h.jaw_height,
            h.chin,
            h.cheeks,
            h.nose_length,
            h.nose_width,
            h.eye_spacing,
            h.eye_depth,
            h.brow,
            h.ears,
            h.eye_size,
            h.eye_height,
            h.mouth_width,
            h.mouth_open,
            h.lip_size,
            h.upper_lid,
            h.lower_lid,
            h.neck_length,
            h.neck_width,
            h.neck_muscle,
            h.throat,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=2.).contains(v))
        {
            return Err("Head controls must be between zero and two".into());
        }
        Ok(())
    }
    pub fn control_point(&self, point: Vec3) -> Vec3 {
        let p = if self.kind == PrimitiveKind::HumanHead {
            head_point(point, &self.head)
        } else {
            point
        };
        self.center
            + self.basis[0] * (p.0 * self.size.0 * 0.5)
            + self.basis[1] * (p.1 * self.size.1 * 0.5)
            + self.basis[2] * (p.2 * self.size.2 * 0.5)
    }
    pub fn facial_control_point(&self, x: f32, y: f32) -> Vec3 {
        if self.head.anatomy_version == 0 {
            return self.control_point(Vec3(x, y, 1.).norm());
        }
        let radius = (1. - y * y).max(0.).sqrt();
        let (width, _, _) = head_profile(y);
        let u = (x / width.max(0.001)).clamp(-0.99, 0.99);
        self.control_point(Vec3(u * radius, y, (1. - u * u).sqrt() * radius))
    }
    pub fn transformed(&self, transform: Transform) -> Result<Self, String> {
        transform.validate()?;
        let mut result = self.clone();
        result.center = transform.point(self.center);
        result.basis = self
            .basis
            .map(|v| transform.point(self.center + v) - result.center);
        result.validate()?;
        Ok(result)
    }
    pub fn mesh(&self) -> Result<Mesh, String> {
        self.validate()?;
        let mut mesh = match self.kind {
            PrimitiveKind::Cube => cube(self.divisions),
            PrimitiveKind::Plane | PrimitiveKind::Grid => {
                grid(self.divisions[0], self.divisions[1])
            }
            PrimitiveKind::Circle => circle(self.segments, self.radius, self.filled),
            PrimitiveKind::UvSphere => uv_sphere(self.segments, self.rings),
            PrimitiveKind::HumanHead => {
                if self.head.anatomy_version == 0 {
                    uv_sphere(self.segments, self.rings)
                } else {
                    head_sphere(self.segments, self.rings)
                }
            }
            PrimitiveKind::Icosphere => icosphere(self.subdivisions)?,
            PrimitiveKind::Cylinder | PrimitiveKind::Cone => cylinder(
                self.segments,
                self.divisions[1],
                self.radius,
                if self.kind == PrimitiveKind::Cylinder {
                    self.radius
                } else {
                    self.top_radius
                },
                self.filled,
            ),
            PrimitiveKind::Torus => torus(self.segments, self.rings, self.radius, self.tube_radius),
        };
        if self.kind == PrimitiveKind::HumanHead {
            for vertex in &mut mesh.vertices {
                *vertex = head_point(*vertex, &self.head);
            }
            attach_neck(&mut mesh, &self.head, self.segments);
            add_head_parts(&mut mesh, &self.head, self.segments, self.rings);
        }
        for vertex in &mut mesh.vertices {
            let p = *vertex;
            let p = Vec3(
                p.0 * self.size.0 * 0.5,
                p.1 * self.size.1 * 0.5,
                p.2 * self.size.2 * 0.5,
            );
            *vertex = self.center + self.basis[0] * p.0 + self.basis[1] * p.1 + self.basis[2] * p.2;
        }
        if self.triangulated {
            mesh.quads.clear();
        }
        mesh.validate()?;
        Ok(mesh)
    }
}
fn empty() -> Mesh {
    Mesh {
        vertex_groups: Vec::new(),
        hidden_faces: Vec::new(),
        polygroups: Vec::new(),
        quads: Vec::new(),
        vertices: vec![],
        triangles: vec![],
        mask: vec![],
    }
}
fn vertex(mesh: &mut Mesh, p: Vec3) -> usize {
    let index = mesh.vertices.len();
    mesh.vertices.push(p);
    mesh.mask.push(0.);
    index
}
fn triangle(mesh: &mut Mesh, mut indices: [usize; 3], normal: Vec3) {
    let [a, b, c] = indices.map(|i| mesh.vertices[i]);
    if (b - a).cross(c - a).dot(normal) < 0. {
        indices.swap(1, 2);
    }
    mesh.triangles.push(indices);
}
fn quad(mesh: &mut Mesh, indices: [usize; 4], normal: Vec3) {
    let [a, b, c, d] = indices;
    let mut polygon = indices;
    if (mesh.vertices[b] - mesh.vertices[a])
        .cross(mesh.vertices[c] - mesh.vertices[a])
        .dot(normal)
        < 0.
    {
        polygon.swap(1, 3);
    }
    mesh.quads.push(polygon);
    triangle(mesh, [a, b, c], normal);
    triangle(mesh, [a, c, d], normal);
}
fn cube(spans: [u32; 3]) -> Mesh {
    let mut mesh = empty();
    let mut shared = HashMap::new();
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        for side in [0, spans[axis]] {
            let mut normal = [0.; 3];
            normal[axis] = if side == 0 { -1. } else { 1. };
            for j in 0..spans[v] {
                for i in 0..spans[u] {
                    let mut face = [0; 4];
                    for (corner, (du, dv)) in
                        [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate()
                    {
                        let mut key = [0; 3];
                        key[axis] = side;
                        key[u] = i + du;
                        key[v] = j + dv;
                        face[corner] = *shared.entry(key).or_insert_with(|| {
                            let p = std::array::from_fn::<_, 3, _>(|k| {
                                key[k] as f32 / spans[k] as f32 * 2. - 1.
                            });
                            vertex(&mut mesh, Vec3(p[0], p[1], p[2]))
                        });
                    }
                    quad(&mut mesh, face, Vec3(normal[0], normal[1], normal[2]));
                }
            }
        }
    }
    mesh
}
fn grid(x: u32, y: u32) -> Mesh {
    let mut mesh = empty();
    for j in 0..=y {
        for i in 0..=x {
            vertex(
                &mut mesh,
                Vec3(
                    i as f32 / x as f32 * 2. - 1.,
                    j as f32 / y as f32 * 2. - 1.,
                    0.,
                ),
            );
        }
    }
    for j in 0..y {
        for i in 0..x {
            let a = (j * (x + 1) + i) as usize;
            let b = a + 1;
            let d = a + (x + 1) as usize;
            quad(&mut mesh, [a, b, d + 1, d], Vec3(0., 0., 1.));
        }
    }
    mesh
}
fn circle(n: u32, radius: f32, filled: bool) -> Mesh {
    let mut mesh = empty();
    for i in 0..n {
        let a = TAU * i as f32 / n as f32;
        vertex(&mut mesh, Vec3(radius * a.cos(), radius * a.sin(), 0.));
    }
    if filled {
        let c = vertex(&mut mesh, Vec3::default());
        for i in 0..n {
            triangle(
                &mut mesh,
                [c, i as usize, ((i + 1) % n) as usize],
                Vec3(0., 0., 1.),
            );
        }
    }
    mesh
}
fn uv_sphere(segments: u32, rings: u32) -> Mesh {
    let n = segments as usize;
    let r = rings as usize;
    let mut mesh = empty();
    vertex(&mut mesh, Vec3(0., 1., 0.));
    for j in 1..r {
        let theta = PI * j as f32 / r as f32;
        for i in 0..n {
            let phi = TAU * i as f32 / n as f32;
            vertex(
                &mut mesh,
                Vec3(
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                ),
            );
        }
    }
    let bottom = vertex(&mut mesh, Vec3(0., -1., 0.));
    for i in 0..n {
        mesh.triangles.push([0, 1 + (i + 1) % n, 1 + i]);
        for j in 0..r - 2 {
            let a = 1 + j * n + i;
            let b = 1 + j * n + (i + 1) % n;
            mesh.quads.push([a, b, b + n, a + n]);
            mesh.triangles.extend([[a, b, a + n], [b, b + n, a + n]]);
        }
        mesh.triangles
            .push([bottom, 1 + (r - 2) * n + i, 1 + (r - 2) * n + (i + 1) % n]);
    }
    mesh
}
fn head_sphere(segments: u32, rings: u32) -> Mesh {
    let mut mesh = uv_sphere(segments, rings);
    const ROWS: [(f32, f32); 8] = [
        (0., 1.),
        (0.12, 0.5),
        (0.22, 0.18),
        (0.42, -0.12),
        (0.48, -0.30),
        (0.70, -0.60),
        (0.90, -0.90),
        (1., -1.),
    ];
    for p in &mut mesh.vertices {
        let t = p.1.clamp(-1., 1.).acos() / PI;
        let pair = ROWS.windows(2).find(|r| t <= r[1].0).unwrap();
        let f = (t - pair[0].0) / (pair[1].0 - pair[0].0);
        let y = pair[0].1 + (pair[1].1 - pair[0].1) * f;
        let old = (1. - p.1 * p.1).max(0.).sqrt();
        let radius = (1. - y * y).max(0.).sqrt();
        if old > 1e-5 {
            p.0 *= radius / old;
            p.2 *= radius / old;
        }
        p.1 = y;
    }
    mesh
}
fn icosphere(level: u32) -> Result<Mesh, String> {
    let mut mesh = empty();
    let g = (1. + 5f32.sqrt()) * 0.5;
    for p in [
        Vec3(-1., g, 0.),
        Vec3(1., g, 0.),
        Vec3(-1., -g, 0.),
        Vec3(1., -g, 0.),
        Vec3(0., -1., g),
        Vec3(0., 1., g),
        Vec3(0., -1., -g),
        Vec3(0., 1., -g),
        Vec3(g, 0., -1.),
        Vec3(g, 0., 1.),
        Vec3(-g, 0., -1.),
        Vec3(-g, 0., 1.),
    ] {
        vertex(&mut mesh, p.norm());
    }
    for a in 0..12 {
        for b in a + 1..12 {
            for c in b + 1..12 {
                let p = mesh.vertices[a];
                let normal = (mesh.vertices[b] - p).cross(mesh.vertices[c] - p);
                if normal.len() < 1e-5 {
                    continue;
                }
                let mut positive = false;
                let mut negative = false;
                for i in 0..12 {
                    if [a, b, c].contains(&i) {
                        continue;
                    }
                    let d = (mesh.vertices[i] - p).dot(normal);
                    positive |= d > 1e-5;
                    negative |= d < -1e-5;
                }
                if !(positive && negative) {
                    triangle(&mut mesh, [a, b, c], p);
                }
            }
        }
    }
    for _ in 1..level {
        mesh = mesh.subdivide()?;
        for v in &mut mesh.vertices {
            *v = v.norm();
        }
    }
    Ok(mesh)
}
fn cylinder(n: u32, spans: u32, bottom_radius: f32, top_radius: f32, caps: bool) -> Mesh {
    let mut mesh = empty();
    let mut rows = Vec::new();
    for j in 0..=spans {
        let t = j as f32 / spans as f32;
        let radius = bottom_radius + (top_radius - bottom_radius) * t;
        let y = t * 2. - 1.;
        if radius <= 1e-7 {
            rows.push(vec![vertex(&mut mesh, Vec3(0., y, 0.))]);
        } else {
            rows.push(
                (0..n)
                    .map(|i| {
                        let a = TAU * i as f32 / n as f32;
                        vertex(&mut mesh, Vec3(radius * a.cos(), y, radius * a.sin()))
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
    for j in 0..spans as usize {
        for i in 0..n as usize {
            let next = (i + 1) % n as usize;
            let low = &rows[j];
            let high = &rows[j + 1];
            let a = TAU * (i as f32 + 0.5) / n as f32;
            let normal = Vec3(a.cos(), 0., a.sin());
            if high.len() == 1 {
                triangle(&mut mesh, [low[i], low[next], high[0]], normal);
            } else if low.len() == 1 {
                triangle(&mut mesh, [low[0], high[next], high[i]], normal);
            } else {
                quad(&mut mesh, [low[i], low[next], high[next], high[i]], normal);
            }
        }
    }
    if caps {
        for (j, sign) in [(0, -1.), (spans as usize, 1.)] {
            let row = &rows[j];
            if row.len() == 1 {
                continue;
            }
            let c = vertex(&mut mesh, Vec3(0., sign, 0.));
            for i in 0..n as usize {
                triangle(
                    &mut mesh,
                    [c, row[i], row[(i + 1) % n as usize]],
                    Vec3(0., sign, 0.),
                );
            }
        }
    }
    mesh
}
fn torus(n: u32, r: u32, major: f32, minor: f32) -> Mesh {
    let mut mesh = empty();
    for i in 0..n {
        let u = TAU * i as f32 / n as f32;
        for j in 0..r {
            let v = TAU * j as f32 / r as f32;
            let radius = major + minor * v.cos();
            vertex(
                &mut mesh,
                Vec3(radius * u.cos(), minor * v.sin(), radius * u.sin()),
            );
        }
    }
    for i in 0..n {
        for j in 0..r {
            let a = (i * r + j) as usize;
            let b = (((i + 1) % n) * r + j) as usize;
            let c = (((i + 1) % n) * r + (j + 1) % r) as usize;
            let d = (i * r + (j + 1) % r) as usize;
            let u = TAU * (i as f32 + 0.5) / n as f32;
            let v = TAU * (j as f32 + 0.5) / r as f32;
            quad(
                &mut mesh,
                [a, b, c, d],
                Vec3(v.cos() * u.cos(), v.sin(), v.cos() * u.sin()),
            );
        }
    }
    mesh
}
// Coordinates are normalized to the head envelope; the face points toward +Z.
fn attach_neck(mesh: &mut Mesh, h: &HeadOptions, n: u32) {
    if !h.neck || h.anatomy_version < 2 {
        return;
    }
    let n = n as usize;
    let rows = (mesh.vertices.len() - 2) / n;
    let sampling = head_sphere(n as u32, (rows + 1) as u32);
    let cut = (0..rows)
        .rev()
        .find(|r| sampling.vertices[1 + r * n].1 > -0.90)
        .unwrap_or(rows.saturating_sub(2));
    let keep = 1 + (cut + 1) * n;
    mesh.vertices.truncate(keep);
    mesh.mask.truncate(keep);
    mesh.triangles.retain(|f| f.iter().all(|i| *i < keep));
    mesh.quads.retain(|f| f.iter().all(|i| *i < keep));
    let boundary = 1 + cut * n;
    let length = 0.40 + h.neck_length * 0.32;
    let width = 0.44 + h.neck_width * 0.16;
    let top_y = -0.91;
    let spans = 20;
    for j in 0..=spans {
        let t = j as f32 / spans as f32;
        let y = top_y - length * t;
        let transition = 1. - (-t * 12.).exp();
        for i in 0..n {
            let angle = TAU * i as f32 / n as f32;
            let (sin, cos) = angle.sin_cos();
            let rx = width + 0.035 * t * t;
            let rz = 0.35 + 0.045 * t;
            let center_z = -0.20 - 0.04 * t;
            let scm = 0.10 + 1.1 * t;
            let angular = |center: f32| {
                let d = (angle - center + PI).rem_euclid(TAU) - PI;
                (-(d / 0.15).powi(2)).exp()
            };
            let muscle = (0.012 + h.neck_muscle * 0.036)
                * (angular(scm) + angular(PI - scm))
                * (PI * t).sin();
            let mut p = Vec3((rx + muscle) * cos, y, center_z + (rz + muscle) * sin);
            let front = sin.max(0.).powi(8);
            p.2 += front * (0.012 + h.throat * 0.04) * (-((t - 0.35) / 0.18).powi(2)).exp();
            // Blend the first row into the underside of the jaw and skull.
            let parent = mesh.vertices[boundary + i];
            p.1 = (parent.1 - 0.035) * (1. - t) + (top_y - length) * t;
            p.0 = parent.0 * (1. - transition) + p.0 * transition;
            p.2 = parent.2 * (1. - transition) + p.2 * transition;
            vertex(mesh, p);
        }
        let previous = if j == 0 { boundary } else { keep + (j - 1) * n };
        let current = keep + j * n;
        for i in 0..n {
            let next = (i + 1) % n;
            mesh.quads
                .push([previous + i, previous + next, current + next, current + i]);
            mesh.triangles.extend([
                [previous + i, previous + next, current + i],
                [previous + next, current + next, current + i],
            ]);
        }
    }
    let base = keep + spans * n;
    let c = vertex(mesh, Vec3(0., top_y - length, -0.24));
    for i in 0..n {
        mesh.triangles.push([c, base + i, base + (i + 1) % n]);
    }
}
fn append_ears(mesh: &mut Mesh, h: &HeadOptions, n: u32, r: u32) {
    let scale = 0.8 + 0.4 * h.ears;
    for sign in [-1., 1.] {
        let mut part = uv_sphere(n.clamp(24, 64), r.clamp(16, 40));
        for v in &mut part.vertices {
            let ring = (v.1 * v.1 + v.2 * v.2).sqrt();
            let helix = 0.055
                * (-((ring - 0.80) / 0.13).powi(2)).exp()
                * ((v.1 + 0.85) / 0.35).clamp(0., 1.);
            let antihelix = 0.025
                * (-((ring - 0.45) / 0.12).powi(2)).exp()
                * ((0.35 - v.2) / 0.4).clamp(0., 1.);
            let tragus =
                0.030 * (-((v.1 + 0.10) / 0.16).powi(2) - ((v.2 - 0.65) / 0.18).powi(2)).exp();
            let lobe = 0.030 * (-((v.1 + 0.78) / 0.20).powi(2)).exp();
            let bowl = 0.035 * (-((v.1 + 0.03) / 0.40).powi(2) - (v.2 / 0.45).powi(2)).exp();
            let outward = if v.0 > 0. {
                0.035 * v.0 + helix + antihelix + tragus + lobe - bowl
            } else {
                0.025 * v.0
            };
            *v = Vec3(
                sign * (0.87 + scale * outward),
                -0.115 + scale * v.1 * 0.22,
                -0.13 + scale * v.2 * 0.12 * (0.72 + 0.28 * ((v.1 + 0.8) / 1.6).clamp(0., 1.)),
            );
        }
        if sign < 0. {
            for q in &mut part.quads {
                q.swap(1, 3);
            }
            for f in &mut part.triangles {
                f.swap(1, 2);
            }
        }
        let offset = mesh.vertices.len();
        mesh.quads
            .extend(part.quads.into_iter().map(|f| f.map(|i| i + offset)));
        mesh.vertices.extend(part.vertices);
        mesh.mask.extend(part.mask);
        mesh.triangles
            .extend(part.triangles.into_iter().map(|f| f.map(|i| i + offset)));
    }
}
// Smooth profile interpolation keeps the jaw and cranium independent of the
// spherical sampling lattice. Values are (height, width, front, rear).
fn head_profile(y: f32) -> (f32, f32, f32) {
    const PROFILE: [(f32, f32, f32, f32); 13] = [
        (-1., 0., 0.48, -0.20),
        (-0.94, 0.32, 0.67, -0.32),
        (-0.82, 0.51, 0.76, -0.46),
        (-0.62, 0.64, 0.71, -0.62),
        (-0.40, 0.73, 0.70, -0.77),
        (-0.20, 0.82, 0.69, -0.86),
        (0., 0.86, 0.68, -0.92),
        (0.20, 0.87, 0.73, -0.96),
        (0.40, 0.87, 0.73, -0.96),
        (0.60, 0.81, 0.66, -0.88),
        (0.78, 0.65, 0.51, -0.72),
        (0.92, 0.38, 0.30, -0.43),
        (1., 0., 0., 0.),
    ];
    if y > 0.92 {
        let r = ((1. - y) / 0.08).max(0.).sqrt();
        return (0.38 * r, 0.30 * r, -0.43 * r);
    }
    if y < -0.94 {
        let r = ((y + 1.) / 0.06).max(0.).sqrt();
        return (0.32 * r, 0.48 + 0.19 * r, -0.20 - 0.12 * r);
    }
    let i = PROFILE
        .windows(2)
        .position(|r| y <= r[1].0)
        .unwrap_or(PROFILE.len() - 2);
    let a = PROFILE[i];
    let b = PROFILE[i + 1];
    let t = ((y - a.0) / (b.0 - a.0)).clamp(0., 1.);
    // Catmull-Rom tangents produce smooth transitions between facial planes.
    let before = PROFILE[i.saturating_sub(1)];
    let after = PROFILE[(i + 2).min(PROFILE.len() - 1)];
    let interp = |av: f32, bv: f32, pv: f32, nv: f32| {
        let ma = (bv - pv) / (b.0 - before.0) * (b.0 - a.0);
        let mb = (nv - av) / (after.0 - a.0) * (b.0 - a.0);
        (2. * t * t * t - 3. * t * t + 1.) * av
            + (t * t * t - 2. * t * t + t) * ma
            + (-2. * t * t * t + 3. * t * t) * bv
            + (t * t * t - t * t) * mb
    };
    (
        interp(a.1, b.1, before.1, after.1).max(0.),
        interp(a.2, b.2, before.2, after.2),
        interp(a.3, b.3, before.3, after.3),
    )
}
fn head_eye(h: &HeadOptions) -> (f32, f32, f32, f32) {
    (
        0.29 + (h.eye_spacing - 0.5) * 0.11,
        0.035 + (h.eye_height - 0.5) * 0.12,
        0.158 + (h.eye_size - 0.5) * 0.045,
        0.59 - (h.eye_depth - 0.5) * 0.05,
    )
}
fn head_mouth(h: &HeadOptions) -> (f32, f32, f32) {
    (
        0.235 + (h.mouth_width - 0.5) * 0.085,
        if h.anatomy_version >= 2 { -0.54 } else { -0.48 },
        0.019,
    )
}
fn head_point(p: Vec3, h: &HeadOptions) -> Vec3 {
    if h.anatomy_version == 0 {
        return legacy_head_point(p, h);
    }
    if h.anatomy_version == 1 {
        return v1_head_point(p, h);
    }
    let g = |x: f32, y: f32, cx: f32, cy: f32, sx: f32, sy: f32| {
        (-((x - cx) / sx).powi(2) - ((y - cy) / sy).powi(2)).exp()
    };
    let (width, face, rear) = head_profile(p.1);
    let width = width
        * (1. - 0.05 * g(0., p.1, 0., 0.35, 1., 0.3) + 0.05 * g(0., p.1, 0., -0.15, 1., 0.15));
    let radius = (1. - p.1 * p.1).max(0.).sqrt();
    let (u, v) = if radius > 1e-5 {
        (p.0 / radius, p.2 / radius)
    } else {
        (0., 0.)
    };
    let jaw = g(0., p.1, 0., -0.65, 1., 0.3);
    let x = u * width * (1. + jaw * (h.jaw_width - 0.5) * 0.24);
    let y = p.1 + (-p.1 - 0.5).max(0.) * 0.22 + ((h.jaw_height - 0.5) * 0.10) * jaw * p.1;
    let front = v.max(0.).powf(0.6);
    let mut z = if v >= 0. {
        face * front
    } else {
        rear * v.abs()
    };
    let (ex, ey, er, ez) = head_eye(h);
    for sign in [-1., 1.] {
        let dx = x - sign * ex;
        let dy = y - ey;
        // Orbital plane, cheek arch and upper lid fold.
        z += front
            * (-0.045 * g(x, y, sign * ex, ey, 0.23, 0.13)
                + (0.025 + h.brow * 0.020) * g(x, y, sign * ex, ey + 0.115, 0.22, 0.05)
                + (0.035 + h.cheeks * 0.055) * g(x, y, sign * 0.49, ey - 0.17, 0.20, 0.12));
        let ew = er * 0.94;
        let corner = (dx / ew).clamp(-1., 1.);
        let slant = sign * dx * 0.035;
        let upper_height = (0.058 * (1. - h.upper_lid.clamp(0., 1.))).max(0.001);
        let lower_height = (0.038 * (1. - h.lower_lid.clamp(0., 1.))).max(0.001);
        let upper = upper_height * (1. - corner * corner).max(0.).powf(0.7);
        let lower = lower_height * (1. - corner * corner).max(0.).powf(0.65);
        let lid_y = dy - slant;
        let aperture = (dx / ew).powi(2)
            + (lid_y
                / if lid_y >= 0. {
                    upper_height
                } else {
                    lower_height
                })
            .powi(2);
        let distance = if lid_y >= 0. {
            lid_y - upper
        } else {
            -lid_y - lower
        };
        let ez = ez - 0.055;
        let eye_surface = ez + (er * er - dx * dx - dy * dy).max(0.).sqrt();
        // A fitted lid follows the eyeball. Inside the almond opening the skin
        // folds behind the globe, leaving only the exposed corneal portion.
        let eye_radius = (dx * dx + dy * dy).sqrt();
        let socket = if v > 0. {
            (1. - (eye_radius / (er * 1.25)).powi(8)).clamp(0., 1.)
        } else {
            0.
        };
        let lid_target = eye_surface + 0.009 + 0.014 * (-(distance / 0.018).powi(2)).exp();
        let fit = if v > 0. && eye_radius < er {
            1.
        } else {
            socket
        };
        z = z * (1. - fit) + z.max(lid_target) * fit;
        let inside = 1. / (1. + ((aperture - 1.) * 14.).clamp(-60., 60.).exp());
        z -= front * inside * 0.13 * (1. - h.upper_lid.min(h.lower_lid).clamp(0., 1.));
        z -= front * 0.016 * g(x, y, sign * ex, ey + 0.085, ew, 0.014);
    }
    // Bridge, rounded tip, nasal wings, columella and nostril creases.
    let nw = 0.10 + (h.nose_width - 0.5) * 0.035;
    let projection = 0.20 + (h.nose_length - 0.5) * 0.14;
    z += front
        * (projection * g(x, y, 0., -0.245, nw, 0.07)
            + 0.11 * g(x, y, 0., -0.075, 0.065, 0.16)
            + 0.035 * g(x, y, 0., -0.32, 0.027, 0.045));
    for sign in [-1., 1.] {
        z += front * 0.080 * g(x, y, sign * nw, -0.285, 0.055, 0.042);
        z -= front * 0.046 * g(x, y, sign * nw * 0.86, -0.32, 0.025, 0.012);
    }
    // Philtrum columns taper into a Cupid's bow; lips taper at the corners.
    z += front
        * 0.013
        * (g(x, y, 0.025, -0.425, 0.014, 0.06) + g(x, y, -0.025, -0.425, 0.014, 0.06));
    let (mw, my, opening) = head_mouth(h);
    let taper = (1. - (x / mw).powi(2)).max(0.);
    let bow = 0.010 * g(x, y, 0.055, my, 0.026, 1.) + 0.010 * g(x, y, -0.055, my, 0.026, 1.)
        - 0.006 * g(x, y, 0., my, 0.026, 1.);
    let upper = my + opening * taper + 0.016 + bow;
    let lower = my - opening * taper - 0.020;
    let fullness = 0.022 + h.lip_size * 0.020;
    z += front
        * taper
        * (fullness * (-((y - upper) / 0.023).powi(2)).exp()
            + fullness * 1.2 * (-((y - lower) / 0.029).powi(2)).exp());
    let oval = (x / mw).powi(2) + ((y - my) / (opening.max(0.012))).powi(2);
    let _ = oval;
    z -= front * 0.025 * g(x, y, 0., my - 0.105, 0.18, 0.035);
    z += front * (0.025 + (h.chin - 0.5) * 0.08) * g(x, y, 0., -0.78, 0.23, 0.12);
    let ear_r = ((p.2 + 0.10) / 0.15).powi(2) + ((p.1 + 0.105) / 0.19).powi(2);
    let ear = (0.075 + h.ears * 0.065)
        * (-((ear_r.sqrt() - 0.7) / 0.24).powi(2)).exp()
        * u.abs().powi(12);
    let _ = ear;
    let point = Vec3(x, y, z);
    let lower = ((-0.515 - y) / 0.065).clamp(0., 1.);
    let front_jaw = ((z + 0.2) / 0.6).clamp(0., 1.);
    let jaw_weight =
        lower * lower * (3. - 2. * lower) * front_jaw * front_jaw * (3. - 2. * front_jaw);
    let mut point = jaw_point(point, h, jaw_weight);
    // Carve the lining in posed coordinates, so the walls cannot stretch over
    // the lower dental arch when the jaw opens.
    let lower = jaw_point(Vec3(0., -0.56, 0.74), h, 1.).1;
    let upper = -0.532;
    let middle = (upper + lower) * 0.5;
    let opening = ((upper - lower) * 0.5).max(0.012);
    let oval = (x / mw).powi(2) + ((point.1 - middle) / opening).powi(2);
    let cavity = (-oval.powi(3)).exp() * front;
    point.2 = point.2 * (1. - cavity) + 0.16 * cavity;
    point
}
fn v1_head_point(p: Vec3, h: &HeadOptions) -> Vec3 {
    if h.anatomy_version == 0 {
        return legacy_head_point(p, h);
    }
    let g = |x: f32, y: f32, cx: f32, cy: f32, sx: f32, sy: f32| {
        (-((x - cx) / sx).powi(2) - ((y - cy) / sy).powi(2)).exp()
    };
    let (width, face, rear) = head_profile(p.1);
    let radius = (1. - p.1 * p.1).max(0.).sqrt();
    let (u, v) = if radius > 1e-5 {
        (p.0 / radius, p.2 / radius)
    } else {
        (0., 0.)
    };
    let jaw = g(0., p.1, 0., -0.65, 1., 0.3);
    let x = u * width * (1. + jaw * (h.jaw_width - 0.5) * 0.24);
    let y = p.1 + (-p.1 - 0.5).max(0.) * 0.22 + ((h.jaw_height - 0.5) * 0.10) * jaw * p.1;
    let front = v.max(0.).powf(0.6);
    let mut z = if v >= 0. {
        face * front
    } else {
        rear * v.abs()
    };
    let (ex, ey, er, ez) = head_eye(h);
    for sign in [-1., 1.] {
        let dx = x - sign * ex;
        let dy = y - ey;
        // Orbital plane, cheek arch and upper lid fold.
        z += front
            * (-0.045 * g(x, y, sign * ex, ey, 0.23, 0.13)
                + (0.045 + h.brow * 0.035) * g(x, y, sign * ex, ey + 0.115, 0.22, 0.05)
                + (0.035 + h.cheeks * 0.055) * g(x, y, sign * 0.49, ey - 0.17, 0.20, 0.12));
        let ew = er * 0.94;
        let corner = (dx / ew).clamp(-1., 1.);
        let slant = sign * dx * 0.035;
        let upper_height = (0.058 * (1. - h.upper_lid.clamp(0., 1.))).max(0.001);
        let lower_height = (0.038 * (1. - h.lower_lid.clamp(0., 1.))).max(0.001);
        let upper = upper_height * (1. - corner * corner).max(0.).powf(0.7);
        let lower = lower_height * (1. - corner * corner).max(0.).powf(0.65);
        let lid_y = dy - slant;
        let aperture = (dx / ew).powi(2)
            + (lid_y
                / if lid_y >= 0. {
                    upper_height
                } else {
                    lower_height
                })
            .powi(2);
        let distance = if lid_y >= 0. {
            lid_y - upper
        } else {
            -lid_y - lower
        };
        let eye_surface = ez + (er * er - dx * dx - dy * dy).max(0.).sqrt();
        // A fitted lid follows the eyeball. Inside the almond opening the skin
        // folds behind the globe, leaving only the exposed corneal portion.
        let eye_radius = (dx * dx + dy * dy).sqrt();
        let socket = if v > 0. {
            (1. - (eye_radius / (er * 1.25)).powi(8)).clamp(0., 1.)
        } else {
            0.
        };
        let lid_target = eye_surface + 0.009 + 0.014 * (-(distance / 0.018).powi(2)).exp();
        let fit = if eye_radius < er { 1. } else { socket };
        z = z * (1. - fit) + z.max(lid_target) * fit;
        let inside = 1. / (1. + ((aperture - 1.) * 14.).clamp(-60., 60.).exp());
        z -= front * inside * 0.13 * (1. - h.upper_lid.min(h.lower_lid).clamp(0., 1.));
        z -= front * 0.016 * g(x, y, sign * ex, ey + 0.085, ew, 0.014);
    }
    // Bridge, rounded tip, nasal wings, columella and nostril creases.
    let nw = 0.075 + (h.nose_width - 0.5) * 0.035;
    let projection = 0.20 + (h.nose_length - 0.5) * 0.14;
    z += front
        * (projection * g(x, y, 0., -0.205, nw, 0.07)
            + 0.11 * g(x, y, 0., -0.075, 0.047, 0.16)
            + 0.035 * g(x, y, 0., -0.275, 0.027, 0.045));
    for sign in [-1., 1.] {
        z += front * 0.080 * g(x, y, sign * nw, -0.245, 0.044, 0.042);
        z -= front * 0.046 * g(x, y, sign * nw * 0.86, -0.277, 0.025, 0.012);
    }
    // Philtrum columns taper into a Cupid's bow; lips taper at the corners.
    z += front
        * 0.013
        * (g(x, y, 0.025, -0.365, 0.014, 0.06) + g(x, y, -0.025, -0.365, 0.014, 0.06));
    let (mw, my, opening) = head_mouth(h);
    let taper = (1. - (x / mw).powi(2)).max(0.);
    let bow = 0.010 * g(x, y, 0.055, my, 0.026, 1.) + 0.010 * g(x, y, -0.055, my, 0.026, 1.)
        - 0.006 * g(x, y, 0., my, 0.026, 1.);
    let upper = my + opening * taper + 0.016 + bow;
    let lower = my - opening * taper - 0.020;
    let fullness = 0.027 + h.lip_size * 0.030;
    z += front
        * taper
        * (fullness * (-((y - upper) / 0.023).powi(2)).exp()
            + fullness * 1.2 * (-((y - lower) / 0.029).powi(2)).exp());
    let oval = (x / mw).powi(2) + ((y - my) / (opening.max(0.012))).powi(2);
    z -= front * 0.39 * (-oval.powi(3)).exp();
    z -= front * 0.025 * g(x, y, 0., my - 0.105, 0.18, 0.035);
    z += front * (0.025 + (h.chin - 0.5) * 0.08) * g(x, y, 0., -0.78, 0.23, 0.12);
    let ear_r = ((p.2 + 0.10) / 0.15).powi(2) + ((p.1 + 0.105) / 0.19).powi(2);
    let ear = (0.075 + h.ears * 0.065)
        * (-((ear_r.sqrt() - 0.7) / 0.24).powi(2)).exp()
        * u.abs().powi(12);
    let point = Vec3(x + p.0.signum() * ear, y, z);
    let lower = ((-0.455 - y) / 0.065).clamp(0., 1.);
    let front_jaw = ((z + 0.2) / 0.6).clamp(0., 1.);
    let jaw_weight =
        lower * lower * (3. - 2. * lower) * front_jaw * front_jaw * (3. - 2. * front_jaw);
    jaw_point(point, h, jaw_weight)
}
// Jaw hinges behind the lower face. Upper lip and upper teeth stay stationary.
fn jaw_point(p: Vec3, h: &HeadOptions, weight: f32) -> Vec3 {
    let angle = h.mouth_open * 0.32 * weight;
    let (s, c) = angle.sin_cos();
    let y = p.1 + 0.12;
    let z = p.2 + 0.25;
    Vec3(p.0, -0.12 + c * y - s * z, -0.25 + s * y + c * z)
}
fn legacy_head_point(p: Vec3, h: &HeadOptions) -> Vec3 {
    let g = |x: f32, y: f32, cx: f32, cy: f32, sx: f32, sy: f32| {
        (-((x - cx) / sx).powi(2) - ((y - cy) / sy).powi(2)).exp()
    };
    let jaw = ((0.18 - p.1) / 0.85).clamp(0., 1.);
    let x = p.0 * (1. - jaw * (0.38 - 0.18 * h.jaw_width).clamp(0.04, 0.42));
    let y = (p.1 + (-p.1 - 0.6).max(0.) * 0.35) * (1. + jaw * (h.jaw_height - 0.5) * 0.3);
    let front = (p.2.max(0.) * 3.).clamp(0., 1.);
    // Flatter facial plane, tapered jaw and a fuller rear cranium.
    let blend = g(x, y, 0., -0.03, 0.7, 0.85) * front;
    let mut z = p.2 * (1. - blend) + (0.83 + 0.06 * y) * blend;
    let ex = 0.26 + h.eye_spacing * 0.16;
    let ey = 0.12 + h.eye_height * 0.12;
    let er = 0.105 + h.eye_size * 0.05;
    for sign in [-1., 1.] {
        let dx = x - sign * ex;
        let socket = g(x, y, sign * ex, ey, er * 1.3, er * 0.72);
        let rim =
            (((dx / (er * 1.25)).powi(2) + ((y - ey) / (er * 0.65)).powi(2)).sqrt() - 1.).powi(2);
        z += front * (-socket * (0.10 + h.eye_depth * 0.08) + 0.035 * (-rim / 0.045).exp());
        z += front * (0.025 + h.brow * 0.055) * g(x, y, sign * ex, ey + er * 0.9, er * 1.4, 0.055);
        z += front * (0.025 + h.cheeks * 0.07) * g(x, y, sign * 0.45, -0.07, 0.18, 0.16);
        // Nasal wings and nostril indentations around a defined tip and bridge.
        z += front
            * 0.085
            * g(
                x,
                y,
                sign * (0.075 + h.nose_width * 0.04),
                -0.10,
                0.055,
                0.055,
            );
        z -= front
            * 0.055
            * g(
                x,
                y,
                sign * (0.07 + h.nose_width * 0.035),
                -0.135,
                0.029,
                0.023,
            );
    }
    z += front
        * ((0.22 + h.nose_length * 0.25) * g(x, y, 0., -0.055, 0.065 + h.nose_width * 0.045, 0.09)
            + (0.12 + h.nose_length * 0.06) * g(x, y, 0., 0.13, 0.058, 0.24));
    let mw = 0.19 + h.mouth_width * 0.10;
    let opening = 0.025 + h.mouth_open * 0.12;
    let my = -0.36;
    let oval = (x / mw).powi(2) + ((y - my) / opening).powi(2);
    // A recessed bowl with steep walls is the mouth cavity; a raised rim forms lips.
    let cavity = (-oval.powi(3)).exp();
    let lip = (-((oval.sqrt() - 1.) / 0.24).powi(2)).exp();
    z += front * (-0.52 * cavity + (0.035 + h.lip_size * 0.065) * lip);
    z += front * (0.045 + h.chin * 0.14) * g(x, y, 0., -0.68, 0.25, 0.14);
    // Ears have an outer helix and inset concha rather than spherical bumps.
    let ear_r = ((p.2 / 0.17).powi(2) + ((p.1 + 0.02) / 0.24).powi(2)).sqrt();
    let ear = (0.05 + h.ears * 0.12) * (-((ear_r - 0.65) / 0.3).powi(2)).exp() * p.0.abs().powi(8);
    Vec3(x + p.0.signum() * ear, y, z)
}
fn append_ellipsoid(
    mesh: &mut Mesh,
    center: Vec3,
    size: Vec3,
    segments: u32,
    rings: u32,
    iris: bool,
) {
    let mut part = uv_sphere(segments, rings);
    for v in &mut part.vertices {
        // Shallow iris rim and pupil recess are geometry, so they survive conversion.
        if iris && v.2 > 0. {
            let r = (v.0 * v.0 + v.1 * v.1).sqrt();
            v.2 +=
                0.035 * (-((r - 0.38) / 0.045).powi(2)).exp() - 0.04 * (-(r / 0.16).powi(4)).exp();
        }
        let shape = if !iris && segments == 12 {
            // Rounded rectangular tooth crowns rather than pointed ellipsoids.
            Vec3(
                v.0.signum() * v.0.abs().powf(0.55),
                v.1.signum() * v.1.abs().powf(0.55),
                v.2.signum() * v.2.abs().powf(0.55),
            )
        } else {
            *v
        };
        *v = center + Vec3(shape.0 * size.0, shape.1 * size.1, shape.2 * size.2);
    }
    let offset = mesh.vertices.len();
    mesh.quads
        .extend(part.quads.into_iter().map(|f| f.map(|i| i + offset)));
    mesh.vertices.extend(part.vertices);
    mesh.mask.extend(part.mask);
    mesh.triangles
        .extend(part.triangles.into_iter().map(|f| f.map(|i| i + offset)));
}
fn add_head_parts(mesh: &mut Mesh, h: &HeadOptions, segments: u32, rings: u32) {
    if h.anatomy_version == 1 {
        v1_add_head_parts(mesh, h, segments, rings);
        return;
    }
    if h.anatomy_version == 0 {
        legacy_add_head_parts(mesh, h, segments, rings);
        return;
    }
    let (ex, ey, er, ez) = head_eye(h);
    let ez = ez - 0.055;
    for sign in [-1., 1.] {
        append_ellipsoid(
            mesh,
            Vec3(sign * ex, ey, ez),
            Vec3(er, er, er),
            segments.clamp(24, 64),
            rings.clamp(16, 32),
            true,
        );
    }
    let (mw, my, _) = head_mouth(h);
    // Eight teeth per quadrant in a dental arch: incisors, canines, premolars,
    // then molars. Crowns retain their size as the jaw opens.
    for upper in [true, false] {
        for sign in [-1., 1.] {
            for i in 0..8 {
                let angle = (i as f32 + 0.5) / 8. * PI * 0.5;
                let (sin, cos) = angle.sin_cos();
                let center = Vec3(
                    sign * mw * 1.08 * sin,
                    my + if upper { 0.027 } else { -0.055 },
                    0.18 + 0.43 * cos,
                );
                let size = match i {
                    0 | 1 => Vec3(0.026, 0.041, 0.027),
                    2 => Vec3(0.025, 0.046, 0.033),
                    3 | 4 => Vec3(0.030, 0.036, 0.041),
                    _ => Vec3(0.035, 0.032, 0.047),
                };
                let start = mesh.vertices.len();
                append_ellipsoid(mesh, center, size, 12, 8, false);
                for v in &mut mesh.vertices[start..] {
                    let p = *v - center;
                    *v = center
                        + Vec3(
                            cos * p.0 + sign * sin * p.2,
                            p.1,
                            -sign * sin * p.0 + cos * p.2,
                        );
                    if !upper {
                        *v = jaw_point(*v, h, 1.);
                    }
                }
            }
        }
    }
    let start = mesh.vertices.len();
    append_ellipsoid(
        mesh,
        Vec3(0., my - 0.11, 0.31),
        Vec3(mw * 0.72, 0.040, 0.19),
        24,
        12,
        false,
    );
    for v in &mut mesh.vertices[start..] {
        *v = jaw_point(*v, h, 1.);
    }
    append_ears(mesh, h, segments, rings);
}
fn v1_add_head_parts(mesh: &mut Mesh, h: &HeadOptions, segments: u32, rings: u32) {
    if h.anatomy_version == 0 {
        legacy_add_head_parts(mesh, h, segments, rings);
        return;
    }
    let (ex, ey, er, ez) = head_eye(h);
    for sign in [-1., 1.] {
        append_ellipsoid(
            mesh,
            Vec3(sign * ex, ey, ez),
            Vec3(er, er, er),
            segments.clamp(24, 64),
            rings.clamp(16, 32),
            true,
        );
    }
    let (mw, my, _) = head_mouth(h);
    // Eight teeth per quadrant in a dental arch: incisors, canines, premolars,
    // then molars. Crowns retain their size as the jaw opens.
    for upper in [true, false] {
        for sign in [-1., 1.] {
            for i in 0..8 {
                let angle = (i as f32 + 0.5) / 8. * PI * 0.5;
                let (sin, cos) = angle.sin_cos();
                let center = Vec3(
                    sign * mw * 1.08 * sin,
                    my + if upper { 0.027 } else { -0.055 },
                    0.18 + 0.43 * cos,
                );
                let size = match i {
                    0 | 1 => Vec3(0.026, 0.041, 0.027),
                    2 => Vec3(0.025, 0.046, 0.033),
                    3 | 4 => Vec3(0.030, 0.036, 0.041),
                    _ => Vec3(0.035, 0.032, 0.047),
                };
                let start = mesh.vertices.len();
                append_ellipsoid(mesh, center, size, 12, 8, false);
                for v in &mut mesh.vertices[start..] {
                    let p = *v - center;
                    *v = center
                        + Vec3(
                            cos * p.0 + sign * sin * p.2,
                            p.1,
                            -sign * sin * p.0 + cos * p.2,
                        );
                    if !upper {
                        *v = jaw_point(*v, h, 1.);
                    }
                }
            }
        }
    }
    let start = mesh.vertices.len();
    append_ellipsoid(
        mesh,
        Vec3(0., my - 0.11, 0.31),
        Vec3(mw * 0.72, 0.040, 0.19),
        24,
        12,
        false,
    );
    for v in &mut mesh.vertices[start..] {
        *v = jaw_point(*v, h, 1.);
    }
}
fn legacy_add_head_parts(mesh: &mut Mesh, h: &HeadOptions, segments: u32, rings: u32) {
    let ex = 0.26 + h.eye_spacing * 0.16;
    let ey = 0.12 + h.eye_height * 0.12;
    let er = 0.105 + h.eye_size * 0.05;
    for sign in [-1., 1.] {
        append_ellipsoid(
            mesh,
            Vec3(sign * ex, ey, 0.72 - h.eye_depth * 0.025),
            Vec3(er, er, er),
            segments.clamp(16, 64),
            rings.clamp(12, 32),
            true,
        );
    }
    let mw = 0.19 + h.mouth_width * 0.10;
    let opening = 0.025 + h.mouth_open * 0.12;
    for upper in [true, false] {
        for i in 0..8 {
            let t = (i as f32 - 3.5) / 4.;
            let x = t * mw * 0.85;
            let y = -0.36
                + if upper {
                    opening * 0.45
                } else {
                    -opening * 0.65
                };
            append_ellipsoid(
                mesh,
                Vec3(x, y, 0.68 - 0.1 * t * t),
                Vec3(mw * 0.098, opening * 0.42, 0.045),
                12,
                8,
                false,
            );
        }
    }
    append_ellipsoid(
        mesh,
        Vec3(0., -0.36 - opening * 0.55, 0.48),
        Vec3(mw * 0.7, opening * 0.28, 0.13),
        24,
        12,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_primitives_are_closed_outward_and_have_no_collapsed_faces() {
        for kind in [
            PrimitiveKind::Cube,
            PrimitiveKind::UvSphere,
            PrimitiveKind::Icosphere,
            PrimitiveKind::Cylinder,
            PrimitiveKind::Cone,
            PrimitiveKind::Torus,
            PrimitiveKind::HumanHead,
        ] {
            let mesh = Primitive::for_kind(kind).mesh().unwrap();
            mesh.validate_closed()
                .unwrap_or_else(|error| panic!("{kind:?}: {error}"));
            let mut volume = 0.;
            for face in &mesh.triangles {
                let [a, b, c] = face.map(|i| mesh.vertices[i]);
                assert!(
                    (b - a).cross(c - a).len() > 1e-8,
                    "{kind:?}: collapsed face"
                );
                volume += a.dot(b.cross(c)) / 6.;
            }
            assert!(volume > 0., "{kind:?}: inward surface");
        }
    }
    #[test]
    fn cube_axis_resolution_welds_edges_and_icosphere_counts_follow_levels() {
        let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
        cube.divisions = [1; 3];
        let mesh = cube.mesh().unwrap();
        assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (8, 12));
        cube.divisions = [2, 3, 4];
        let mesh = cube.mesh().unwrap();
        assert_eq!(mesh.triangles.len(), 4 * (2 * 3 + 2 * 4 + 3 * 4));
        mesh.validate_closed().unwrap();
        for level in 1..=4 {
            let mut sphere = Primitive::for_kind(PrimitiveKind::Icosphere);
            sphere.subdivisions = level;
            let mesh = sphere.mesh().unwrap();
            assert_eq!(mesh.triangles.len(), 20 * 4usize.pow(level - 1));
            assert!(mesh.vertices.iter().all(|v| (v.len() - 1.).abs() < 1e-5));
        }
        let plane = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        assert_eq!((plane.vertices.len(), plane.triangles.len()), (4, 2));
    }
    #[test]
    fn resolution_and_nonfinite_inputs_fail_before_generation_and_open_shapes_are_supported() {
        let mut p = Primitive::default();
        p.divisions[0] = u32::MAX;
        assert!(p.mesh().is_err());
        p = Primitive::default();
        p.size.0 = f32::NAN;
        assert!(p.mesh().is_err());
        p = Primitive::for_kind(PrimitiveKind::Torus);
        p.rings = 2;
        assert!(p.mesh().is_err());
        p = Primitive::for_kind(PrimitiveKind::Circle);
        p.filled = false;
        let ring = p.mesh().unwrap();
        assert_eq!(ring.vertices.len(), 48);
        assert!(ring.triangles.is_empty());
        p = Primitive::for_kind(PrimitiveKind::Cylinder);
        p.filled = false;
        let tube = p.mesh().unwrap();
        tube.validate().unwrap();
        assert!(tube.validate_closed().is_err());
        p = Primitive::for_kind(PrimitiveKind::Cone);
        p.top_radius = 0.7;
        p.mesh().unwrap().validate_closed().unwrap();
    }
    #[test]
    fn head_has_closed_eye_teeth_and_tongue_shells_and_supports_extended_controls() {
        let mut p = Primitive::for_kind(PrimitiveKind::HumanHead);
        let base = uv_sphere(p.segments, p.rings);
        let mesh = p.mesh().unwrap();
        assert!(mesh.vertices.len() > base.vertices.len());
        // Every appended feature is a closed, outward surface; the skin cavity
        // remains closed as well, so conversion preserves valid sculpt geometry.
        mesh.validate_closed().unwrap();
        p.head.mouth_open = 1.8;
        p.head.eye_size = 1.4;
        p.head.nose_length = 1.8;
        p.head.jaw_width = 1.8;
        let changed = p.mesh().unwrap();
        changed.validate_closed().unwrap();
        assert!(
            mesh.triangles == changed.triangles,
            "facial controls retain topology"
        );
        let y = -0.54;
        let cavity = head_point(Vec3(0., y, (1f32 - y * y).sqrt()), &p.head);
        assert!(cavity.2.is_finite(), "posed mouth stays finite");
        let legacy: HeadOptions = serde_json::from_str("{\"jaw_width\":0.7}").unwrap();
        assert_eq!(legacy.eye_size, HeadOptions::default().eye_size);
    }
    #[test]
    fn head_lids_cover_the_globes_and_jaw_moves_only_the_lower_dental_arch() {
        let mut p = Primitive::for_kind(PrimitiveKind::HumanHead);
        p.head.neck = false;
        let rest = p.mesh().unwrap();
        let skin = head_sphere(p.segments, p.rings).vertices.len();
        let eye = uv_sphere(p.segments.clamp(24, 64), p.rings.clamp(16, 32))
            .vertices
            .len();
        let tooth = uv_sphere(12, 8).vertices.len();
        let upper = skin + 2 * eye;
        let lower = upper + 16 * tooth;
        let mut opened = p.clone();
        opened.head.mouth_open = 1.5;
        let posed = opened.mesh().unwrap();
        posed.validate_closed().unwrap();
        assert_eq!(rest.triangles, posed.triangles);
        assert_eq!(
            &rest.vertices[upper..lower],
            &posed.vertices[upper..lower],
            "upper arch is fixed"
        );
        assert_ne!(
            &rest.vertices[lower..],
            &posed.vertices[lower..],
            "lower arch and tongue follow the jaw"
        );
        let mut blink = p.clone();
        blink.head.upper_lid = 1.;
        blink.head.lower_lid = 1.;
        let closed = blink.mesh().unwrap();
        closed.validate_closed().unwrap();
        assert_eq!(
            &rest.vertices[skin..],
            &closed.vertices[skin..],
            "lids move over fixed eyes"
        );
        assert_ne!(&rest.vertices[..skin], &closed.vertices[..skin]);
        let (ex, ey, er, ez) = head_eye(&p.head);
        let ez = ez - 0.055;
        let lid = blink.facial_control_point(ex, ey);
        assert!(
            lid.2 > (ez + er) * p.size.2 * 0.5,
            "closed lids cover the eye surface"
        );
        let crown = p.control_point(Vec3(0., 1., 0.)).1;
        let chin = p.control_point(Vec3(0., -1., 0.)).1;
        let eye_y = ey * p.size.1 * 0.5;
        let proportion = (crown - eye_y) / (crown - chin);
        assert!(
            (0.46..0.54).contains(&proportion),
            "rest eye line is near half head height"
        );
    }
    #[test]
    fn head_face_controls_do_not_pull_rear_vertices_through_the_skull() {
        let h = HeadOptions::default();
        for x in [-0.31, 0.31] {
            let p = Vec3(x, 0.035, -(1f32 - x * x - 0.035 * 0.035).sqrt());
            let before = head_point(p, &h);
            assert!(before.2 < -0.5, "rear surface must stay behind the face");
            let mut closed = h.clone();
            closed.upper_lid = 1.;
            closed.lower_lid = 1.;
            closed.eye_depth = 1.5;
            let after = head_point(p, &closed);
            assert!(
                (before - after).len() < 1e-6,
                "lid controls affect only the front hemisphere"
            );
        }
    }
    #[test]
    fn neck_is_welded_to_the_skin_and_stays_closed_while_the_jaw_opens() {
        let p = Primitive::for_kind(PrimitiveKind::HumanHead);
        let mut skin = head_sphere(p.segments, p.rings);
        for v in &mut skin.vertices {
            *v = head_point(*v, &p.head);
        }
        attach_neck(&mut skin, &p.head, p.segments);
        skin.validate_closed().unwrap();
        assert!(
            skin.vertices.iter().any(|v| v.1 < -1.4),
            "neck extends below the jaw"
        );
        let mut neighbors = vec![vec![]; skin.vertices.len()];
        for f in &skin.triangles {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                neighbors[a].push(b);
                neighbors[b].push(a);
            }
        }
        let mut visited = vec![false; skin.vertices.len()];
        let mut stack = vec![0];
        visited[0] = true;
        while let Some(a) = stack.pop() {
            for &b in &neighbors[a] {
                if !visited[b] {
                    visited[b] = true;
                    stack.push(b);
                }
            }
        }
        assert!(
            visited.iter().all(|v| *v),
            "head and neck form one connected skin"
        );
        let mut opened = p.clone();
        opened.head.mouth_open = 1.5;
        opened.head.neck_length = 1.2;
        let open = opened.mesh().unwrap();
        open.validate_closed().unwrap();
        assert!(
            p.mesh().unwrap().triangles == open.triangles,
            "jaw and neck edits retain topology"
        );
    }
    #[test]
    fn version_one_heads_keep_their_surface_until_the_user_updates_anatomy() {
        let mut p = Primitive::for_kind(PrimitiveKind::HumanHead);
        p.head.anatomy_version = 1;
        p.head.neck = false;
        let mesh = p.mesh().unwrap();
        let mut expected = head_sphere(p.segments, p.rings);
        for v in &mut expected.vertices {
            *v = v1_head_point(*v, &p.head);
        }
        v1_add_head_parts(&mut expected, &p.head, p.segments, p.rings);
        for v in &mut expected.vertices {
            *v = Vec3(
                v.0 * p.size.0 * 0.5,
                v.1 * p.size.1 * 0.5,
                v.2 * p.size.2 * 0.5,
            );
        }
        assert!(
            mesh == expected,
            "existing saved recipes regenerate unchanged"
        );
        let mut value = serde_json::to_value(&p).unwrap();
        for name in ["neck", "neck_length", "neck_width", "neck_muscle", "throat"] {
            value["head"].as_object_mut().unwrap().remove(name);
        }
        let loaded: Primitive = serde_json::from_value(value).unwrap();
        assert!(loaded.mesh().unwrap() == mesh);
    }
    #[test]
    fn saved_head_without_anatomy_version_retains_its_previous_surface() {
        let mut legacy = Primitive::for_kind(PrimitiveKind::HumanHead);
        legacy.head.anatomy_version = 0;
        legacy.head.mouth_open = 0.35;
        legacy.head.nose_length = 1.2;
        let expected = legacy.mesh().unwrap();
        let mut data = serde_json::to_value(&legacy).unwrap();
        data["head"]
            .as_object_mut()
            .unwrap()
            .remove("anatomy_version");
        data["head"].as_object_mut().unwrap().remove("upper_lid");
        data["head"].as_object_mut().unwrap().remove("lower_lid");
        let loaded: Primitive = serde_json::from_value(data).unwrap();
        assert_eq!(loaded.head.anatomy_version, 0);
        assert_eq!(loaded.mesh().unwrap(), expected);
    }
    #[test]
    fn head_base_is_symmetric_and_face_controls_change_the_surface() {
        let p = Primitive::for_kind(PrimitiveKind::HumanHead);
        let original = p.mesh().unwrap();
        let mut changed = p.clone();
        changed.head.nose_length = 1.;
        changed.head.jaw_width = 1.;
        changed.head.eye_depth = 1.;
        changed.head.ears = 1.;
        let mesh = changed.mesh().unwrap();
        assert_eq!(mesh.triangles, original.triangles);
        assert_ne!(mesh.vertices, original.vertices);
        mesh.validate_closed().unwrap();
        for point in [Vec3(0.3, 0.25, 0.9).norm(), Vec3(0.5, -0.5, 0.7).norm()] {
            let a = p.control_point(point);
            let b = p.control_point(Vec3(-point.0, point.1, point.2));
            assert!(
                (a.0 + b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6 && (a.2 - b.2).abs() < 1e-6
            );
        }
        if let Ok(path) = std::env::var("CLAY_PRIMITIVE_PREVIEW") {
            std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
            let mut opened = p.clone();
            opened.head.mouth_open = 1.;
            std::fs::write(
                format!("{path}.open.json"),
                serde_json::to_vec(&opened.mesh().unwrap()).unwrap(),
            )
            .unwrap();
            let mut closed = p.clone();
            closed.head.upper_lid = 1.;
            closed.head.lower_lid = 1.;
            std::fs::write(
                format!("{path}.blink.json"),
                serde_json::to_vec(&closed.mesh().unwrap()).unwrap(),
            )
            .unwrap();
        }
    }
}
