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
}
impl PrimitiveKind {
    pub const ALL: [Self; 9] = [
        Self::Plane,
        Self::Cube,
        Self::Circle,
        Self::UvSphere,
        Self::Icosphere,
        Self::Cylinder,
        Self::Cone,
        Self::Torus,
        Self::Grid,
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
        Ok(())
    }
    pub fn control_point(&self, point: Vec3) -> Vec3 {
        let p = point;
        self.center
            + self.basis[0] * (p.0 * self.size.0 * 0.5)
            + self.basis[1] * (p.1 * self.size.1 * 0.5)
            + self.basis[2] * (p.2 * self.size.2 * 0.5)
    }

    pub fn transformed(&self, transform: Transform) -> Result<Self, String> {
        transform.validate()?;
        let mut result = self.clone();
        result.center = transform.point(self.center);
        result.basis = self.basis.map(|v| transform.vector(v));
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
        creases: Vec::new(),
        materials: Vec::new(),
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
}
