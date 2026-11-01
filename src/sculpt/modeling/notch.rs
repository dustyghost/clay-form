//! One-cell cuts from a grid face or convex ridge.
use super::*;

mod face;
use face::FaceNotch;

pub(crate) enum CellNotch {
    Ridge(RidgeNotch),
    Face(FaceNotch),
}

pub(crate) struct RidgeNotch {
    edge: [usize; 2],
    faces: [[usize; 4]; 2],
    outer: [Vec3; 2],
    pub(crate) travel: Vec3,
}

impl Mesh {
    pub(crate) fn cell_notch(&self, [a, b]: [usize; 2]) -> Option<CellNotch> {
        let [a, b] = edge(a, b);
        let star: Vec<_> = self
            .quads
            .iter()
            .filter(|q| q.contains(&a) || q.contains(&b))
            .collect();
        let quad = |from, to| {
            star.iter().find_map(|&&q| {
                let i = q.iter().position(|&v| v == from)?;
                let mut corners = q;
                corners.rotate_left(i);
                (corners[1] == to).then_some(corners)
            })
        };
        let front = quad(a, b)?;
        let top = quad(b, a)?;
        let position = |i: usize| self.vertices[i];
        let axis = position(b) - position(a);
        let tolerance = axis.len() * 1e-4;
        // Recover the outer ridge from the undisturbed corners of its two neighbors.
        // This also works when a previous Move already dragged the edge partway in.
        let outer_corner = |v, left, right, outward: Vec3| {
            let [_, _, left_far, next] = quad(v, left)?;
            let [_, _, other, right_far] = quad(right, v)?;
            let origin = position(left) + (position(next) - position(left_far));
            let check = position(right) + (position(other) - position(right_far));
            if next != other
                || (origin - check).len() > tolerance
                || (position(next) - origin).norm().dot(outward) < 0.9999
            {
                return None;
            }
            Some(if (position(v) - origin).len() <= tolerance {
                position(v)
            } else {
                origin
            })
        };
        let outer = [
            outer_corner(a, front[3], top[2], axis.norm() * -1.)?,
            outer_corner(b, top[3], front[2], axis.norm())?,
        ];
        let down = position(front[3]) - outer[0];
        let back = position(top[2]) - outer[0];
        // Both faces must describe the same rectangular cell, with the material
        // on the inside of the ridge. Flat, concave and skewed fans are ordinary Moves.
        if (outer[1] - outer[0] - axis).len() > tolerance
            || (position(front[2]) - outer[1] - down).len() > tolerance
            || (position(top[3]) - outer[1] - back).len() > tolerance
            || axis.cross(down).dot(back) >= 0.
            || axis.norm().dot(down.norm()).abs() > 1e-4
            || axis.norm().dot(back.norm()).abs() > 1e-4
            || down.norm().dot(back.norm()).abs() > 1e-4
        {
            return None;
        }
        let travel = outer[0] + down + back - position(a);
        (travel.len() > tolerance).then_some(CellNotch::Ridge(RidgeNotch {
            edge: [a, b],
            faces: [front, top],
            outer,
            travel,
        }))
    }
}

impl CellNotch {
    pub(crate) fn travel(&self) -> Vec3 {
        match self {
            Self::Ridge(ridge) => ridge.travel,
            Self::Face(face) => face.travel,
        }
    }

    /// Snap on approach and on overshoot along the same drag, using local model units.
    pub(crate) fn reached(&self, movement: Vec3) -> bool {
        let travel = self.travel();
        let t = (travel.dot(movement) / movement.dot(movement).max(1e-20)).clamp(0., 1.);
        (travel - movement * t).len() <= travel.len() * 0.08
    }

    pub(crate) fn apply(
        &self,
        mesh: &Mesh,
        protect: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<(Mesh, BTreeMap<usize, usize>), String> {
        let (out, caps) = match self {
            Self::Ridge(ridge) => (ridge.apply(mesh, protect, symmetry)?, BTreeMap::new()),
            Self::Face(face) => face.apply(mesh, protect, symmetry)?,
        };
        mesh.check_model_protection(&out, protect)?;
        manifold(&out)?;
        symmetry.validate_result(&out)?;
        Ok((out, caps))
    }
}
impl RidgeNotch {
    fn apply(
        &self,
        mesh: &Mesh,
        protect: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        let mut out = mesh.clone();
        self.cut(mesh, &mut out, protect)?;
        let mut touched = BTreeSet::from(self.edge);
        for e in symmetry.edges(&[self.edge]) {
            if e == self.edge {
                continue;
            }
            let Some(CellNotch::Ridge(cell)) = mesh.cell_notch(e) else {
                return Err("The mirrored edge needs a matching grid cell".into());
            };
            if e.iter().any(|&v| !touched.insert(v)) {
                return Err("Overlapping symmetric notches need separate ridge segments".into());
            }
            cell.cut(mesh, &mut out, protect)?;
        }
        out.retain_creases();
        Ok(out)
    }

    fn cut(&self, mesh: &Mesh, out: &mut Mesh, protect: bool) -> Result<(), String> {
        let [a, b] = self.edge;
        let [front, top] = self.faces;
        mesh.protect_model_vertices(&[a, b], protect)?;
        // The inner walls must fit in empty space. Reject even ambiguous triangle
        // bounds conservatively; in particular, never lay a wall on the far side of a thin sheet.
        let axes = [
            self.outer[1] - self.outer[0],
            mesh.vertices[front[3]] - self.outer[0],
            mesh.vertices[top[2]] - self.outer[0],
        ];
        let dual = axes.map(|v| v * (1. / v.dot(v)));
        for t in &mesh.triangles {
            if t.contains(&a) || t.contains(&b) {
                continue;
            }
            let points = t.map(|i| mesh.vertices[i] - self.outer[0]);
            if dual.iter().enumerate().all(|(i, axis)| {
                let p = points.map(|v| v.dot(*axis));
                let lo = if i == 0 { -1e-4 } else { 1e-4 };
                p.iter().any(|&x| x > lo) && p.iter().any(|&x| x < 1. + 1e-4)
            }) {
                return Err("Another surface occupies this grid cell".into());
            }
        }
        let outer = [
            out.append_blend(mesh, &[(a, 1.)], self.outer[0]),
            out.append_blend(mesh, &[(b, 1.)], self.outer[1]),
        ];
        let outside = |v| {
            if v == a {
                outer[0]
            } else if v == b {
                outer[1]
            } else {
                v
            }
        };
        // Keep the selected indices on the moving edge. Neighbor triangles retain
        // their winding, diagonals and attributes, with the outer corners restored.
        for face in out
            .quads
            .iter_mut()
            .map(|q| q.as_mut_slice())
            .chain(out.triangles.iter_mut().map(|t| t.as_mut_slice()))
        {
            if !self
                .faces
                .iter()
                .any(|q| face.iter().all(|v| q.contains(v)))
            {
                for v in face {
                    *v = outside(*v);
                }
            }
        }
        for v in [a, b] {
            out.vertices[v] = mesh.vertices[v] + self.travel;
        }
        for &[u, v] in &mesh.creases {
            if (outside(u) != u || outside(v) != v) && edge(u, v) != self.edge {
                out.creases.push(edge(outside(u), outside(v)));
            }
        }
        let group = mesh
            .triangles
            .iter()
            .position(|t| t.iter().all(|v| front.contains(v)))
            .and_then(|i| mesh.polygroups.get(i))
            .copied()
            .unwrap_or(0);
        out.polygroups.resize(out.triangles.len(), 0);
        out.hidden_faces.resize(out.triangles.len(), false);
        out.push_polygon(&[front[3], outer[0], top[2], a], group, false);
        out.push_polygon(&[top[3], outer[1], front[2], b], group, false);
        out.canonicalize_polygroups();
        Ok(())
    }
}
