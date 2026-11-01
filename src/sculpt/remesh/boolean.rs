//! Combine signed distances from closed operands on a common grid, bottom first.
use super::{Grid, Mesh, Vec3};
use crate::sculpt::BooleanRole;
use std::collections::HashMap;

pub(crate) struct Operand {
    pub role: BooleanRole,
    pub name: String,
    pub mesh: Mesh,
}

pub(crate) fn evaluate(
    operands: &[Operand],
    resolution: u32,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    if operands.first().is_none_or(|o| o.role != BooleanRole::Add) {
        return Err("Set the bottom visible Boolean operand to Add to form a base".into());
    }
    if !(16..=96).contains(&resolution) {
        return Err("Choose a Boolean resolution between 16 and 96".into());
    }
    let mut grid = Grid::fit(
        operands
            .iter()
            .filter(|o| o.role == BooleanRole::Add)
            .flat_map(|o| o.mesh.vertices.iter()),
        resolution,
    )?;
    // Keep the operand bounds off the sampling planes.
    grid.origin = grid.origin - Vec3(0.37, 0.41, 0.31) * grid.step;
    let mut field = vec![grid.step * 3.; grid.dims.iter().product()];
    let mut owners = vec![u8::MAX; field.len()];
    for (owner, operand) in operands.iter().enumerate() {
        if cancel() {
            return Err("Boolean evaluation cancelled".into());
        }
        operand
            .mesh
            .validate_closed()
            .map_err(|e| format!("{} — {e}", operand.name))?;
        let inside = grid.occupancy(&operand.mesh, cancel)?;
        let distance = grid.signed_field(&operand.mesh, &inside, cancel)?;
        for (i, d) in distance.into_iter().enumerate() {
            let d = if operand.role == BooleanRole::Subtract {
                -d
            } else {
                d
            };
            let wins = match operand.role {
                BooleanRole::Add => d < field[i],
                BooleanRole::Intersect | BooleanRole::Subtract => d > field[i],
                BooleanRole::Regular => {
                    return Err("Regular submodels must be excluded from the Boolean stack".into());
                }
            };
            if wins {
                field[i] = d;
                owners[i] = owner as u8;
            }
        }
    }
    let inside = field.iter().map(|&d| d < 0.).collect::<Vec<_>>();
    if !inside.iter().any(|&v| v) {
        return Ok(Mesh::default());
    }
    let planes = surface_crossings(&grid, &field, &owners, operands, cancel)?;
    let mut pending = HashMap::<[usize; 2], Plane>::new();
    let mut seams = Vec::new();
    let fitted = super::quads::extract_with(
        &grid,
        &field,
        cancel,
        true,
        |center, edges, low| fit_contour(&planes, center, edges, low, grid.step),
        |start, end, corners| {
            let Some(&(_, plane)) = planes.get(&(start, end)) else {
                return;
            };
            for i in 0..4 {
                let edge = crate::sculpt::uv::edge(corners[i], corners[(i + 1) % 4]);
                if let Some(other) = pending.remove(&edge) {
                    if plane.shading_boundary(other) {
                        seams.push(edge);
                    }
                } else {
                    pending.insert(edge, plane);
                }
            }
        },
    );
    drop(pending);
    drop(planes);
    // Singular saddle cells can require the tetrahedral representation.
    match fitted.and_then(|mut mesh| {
        mesh.validate_closed()?;
        mesh.set_shading_seams(seams);
        Ok(mesh)
    }) {
        Ok(mesh) => Ok(mesh),
        Err(e) if cancel() => Err(e),
        Err(_) => {
            let mesh = grid.triangles(&inside, &field, cancel)?;
            mesh.validate_closed()?;
            Ok(mesh)
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Plane {
    normal: Vec3,
    offset: f32,
    owner: u8,
}

impl Plane {
    fn shading_boundary(self, other: Self) -> bool {
        // Keep neighboring facets of a single curved operand smooth. At an
        // intersection, even a shallow change of surface can form a real rim.
        let threshold = if self.owner == other.owner {
            0.5
        } else {
            5_f32.to_radians().cos()
        };
        self.normal.dot(other.normal) < threshold
    }
}

type Crossings = HashMap<(usize, usize), (f32, Plane)>;

/// Record exact source planes only along sign-changing lattice edges. The
/// endpoint owners exclude faces buried by later operations in the stack.
fn surface_crossings(
    grid: &Grid,
    field: &[f32],
    owners: &[u8],
    operands: &[Operand],
    cancel: &dyn Fn() -> bool,
) -> Result<Crossings, String> {
    let mut cuts = Crossings::new();
    for (owner, operand) in operands.iter().enumerate() {
        for (i, face) in operand.mesh.triangles.iter().enumerate() {
            if i % 256 == 0 && cancel() {
                return Err("Boolean evaluation cancelled".into());
            }
            let world = face.map(|v| operand.mesh.vertices[v]);
            let [a, b, c] = world;
            let normal = (b - a).cross(c - a).norm();
            let plane = Plane {
                normal,
                offset: normal.dot(a),
                owner: owner as u8,
            };
            let [a, b, c] = world.map(|p| {
                let v = (p - grid.origin) * (1. / grid.step);
                [v.0, v.1, v.2]
            });
            for axis in 0..3 {
                let u = (axis + 1) % 3;
                let v = (axis + 2) % 3;
                let du = [b[u] - a[u], c[u] - a[u]];
                let dv = [b[v] - a[v], c[v] - a[v]];
                let det = du[0] * dv[1] - du[1] * dv[0];
                if det.abs() < 1e-7 {
                    continue;
                }
                let span = |k: usize| {
                    let lo = (a[k].min(b[k]).min(c[k]) - 1e-5).ceil().max(0.) as usize;
                    let hi =
                        ((a[k].max(b[k]).max(c[k]) + 1e-5).floor() as usize).min(grid.dims[k] - 1);
                    lo..=hi
                };
                for j in span(v) {
                    for k in span(u) {
                        let x = k as f32 - a[u];
                        let y = j as f32 - a[v];
                        let s = (x * dv[1] - y * du[1]) / det;
                        let t = (y * du[0] - x * dv[0]) / det;
                        if s < -1e-5 || t < -1e-5 || s + t > 1.00001 {
                            continue;
                        }
                        let hit = a[axis] + s * (b[axis] - a[axis]) + t * (c[axis] - a[axis]);
                        if !(0. ..(grid.dims[axis] - 1) as f32).contains(&hit) {
                            continue;
                        }
                        let mut at = [0; 3];
                        at[u] = k;
                        at[v] = j;
                        at[axis] = hit.floor() as usize;
                        let start = grid.index(at[0], at[1], at[2]);
                        at[axis] += 1;
                        let end = grid.index(at[0], at[1], at[2]);
                        if (field[start] < 0.) == (field[end] < 0.)
                            || (owners[start] != owner as u8 && owners[end] != owner as u8)
                        {
                            continue;
                        }
                        let guess = field[start] / (field[start] - field[end]);
                        let error = (hit.fract() - guess).abs();
                        // Orient the source plane toward the outside of the result,
                        // including subtraction and inward-wound source meshes.
                        let plane = if (det > 0.) == (field[start] < 0.) {
                            plane
                        } else {
                            Plane {
                                normal: plane.normal * -1.,
                                offset: -plane.offset,
                                ..plane
                            }
                        };
                        let entry = cuts.entry((start, end)).or_insert((error, plane));
                        if error < entry.0 {
                            *entry = (error, plane);
                        }
                    }
                }
            }
        }
    }
    Ok(cuts)
}

/// At a sharp rim the wall and cap cross different edges of the same voxel.
/// Fit their planes jointly. The small pull toward the crossing centroid
/// fixes the free directions on a flat face or straight crease.
fn fit_contour(
    planes: &Crossings,
    center: Vec3,
    edges: &[(usize, usize)],
    low: Vec3,
    step: f32,
) -> Vec3 {
    let anchor = 0.01;
    let mut rows = [
        Vec3(anchor, 0., 0.),
        Vec3(0., anchor, 0.),
        Vec3(0., 0., anchor),
    ];
    let mut rhs = Vec3::default();
    for &(a, b) in edges {
        if let Some((_, p)) = planes.get(&(a.min(b), a.max(b))) {
            let n = p.normal;
            rows[0] = rows[0] + n * n.0;
            rows[1] = rows[1] + n * n.1;
            rows[2] = rows[2] + n * n.2;
            rhs = rhs + n * (p.offset - n.dot(center));
        }
    }
    let [a, b, c] = rows;
    let bc = b.cross(c);
    let mut delta = (bc * rhs.0 + c.cross(a) * rhs.1 + a.cross(b) * rhs.2) * (1. / a.dot(bc));
    let inset = Vec3(1., 1., 1.) * (step * 0.02);
    let min = (low - center) + inset;
    let max = (low - center) + Vec3(step, step, step) - inset;
    if delta.0 < min.0
        || delta.0 > max.0
        || delta.1 < min.1
        || delta.1 > max.1
        || delta.2 < min.2
        || delta.2 > max.2
    {
        // A crease can graze the side of a cell. Minimize the plane error
        // along that side instead of clamping each coordinate independently.
        for _ in 0..6 {
            delta.0 = (delta.0 + (rhs.0 - a.dot(delta)) / a.0).clamp(min.0, max.0);
            delta.1 = (delta.1 + (rhs.1 - b.dot(delta)) / b.1).clamp(min.1, max.1);
            delta.2 = (delta.2 + (rhs.2 - c.dot(delta)) / c.2).clamp(min.2, max.2);
        }
    }
    center + delta
}
