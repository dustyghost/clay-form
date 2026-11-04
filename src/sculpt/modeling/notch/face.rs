//! The surface grid defines the next depth, even if the selected face has no depth edges.
use super::*;
use std::collections::HashMap;

pub(crate) struct FaceNotch {
    face: usize,
    corners: [usize; 4],
    restore: Vec3,
    pub(super) travel: Vec3,
    // Each exposed side is removed. Its far edge provides existing corners for the recess.
    sides: [Option<usize>; 4],
    inner: [Option<usize>; 4],
}

impl Mesh {
    pub(crate) fn face_cell_notch(&self, face: usize) -> Option<CellNotch> {
        let corners = *self.quads.get(face)?;
        let [a, b, c, d] = corners.map(|v| self.vertices[v]);
        let u = b - a;
        let v = d - a;
        let tolerance = u.len().min(v.len()) * 1e-4;
        if tolerance <= 1e-10
            || (a + u + v - c).len() > tolerance
            || u.norm().dot(v.norm()).abs() > 1e-4
        {
            return None;
        }
        let inward = v.cross(u).norm();
        // A diagonal neighbor has three untouched corners. Use it to recover the original
        // face plane if an earlier Move already translated the selected square.
        let mut restore = None;
        let mut across = HashMap::new();
        for (f, q) in self.quads.iter().enumerate() {
            for i in 0..4 {
                across.insert((q[(i + 1) % 4], q[i]), f);
            }
            if q.iter().filter(|p| corners.contains(p)).count() == 1 {
                let i = q.iter().position(|p| corners.contains(p))?;
                let shift = (self.vertices[q[(i + 1) % 4]] - self.vertices[q[i]])
                    + (self.vertices[q[(i + 3) % 4]] - self.vertices[q[(i + 2) % 4]]);
                if restore.is_some_and(|previous: Vec3| (previous - shift).len() > tolerance) {
                    return None;
                }
                restore = Some(shift);
            }
        }
        let restore = restore.filter(|d| d.len() > tolerance).unwrap_or_default();
        let position = |i: usize| {
            self.vertices[i]
                + if corners.contains(&i) {
                    restore
                } else {
                    Vec3::default()
                }
        };
        let mut sides = [None; 4];
        let mut inner = [None; 4];
        let mut depth = None;
        for i in 0..4 {
            let j = (i + 1) % 4;
            let mut from = corners[i];
            let mut to = corners[j];
            let along = (position(to) - position(from)).cross(inward * -1.).norm();
            // Follow the row of rectangles to its first perpendicular depth edge.
            loop {
                let &neighbor = across.get(&(from, to))?;
                let q = self.quads[neighbor];
                let k = q.iter().position(|&v| v == from)?;
                let (far_from, far_to) = (q[(k + 1) % 4], q[(k + 2) % 4]);
                let step = position(far_from) - position(from);
                if (position(far_to) - position(to) - step).len() > tolerance {
                    return None;
                }
                let height = step.dot(inward);
                if height.abs() > tolerance && (step - inward * height).len() <= tolerance {
                    if depth.is_some_and(|d: f32| (d - height.abs()).abs() > tolerance) {
                        return None;
                    }
                    depth = Some(height.abs());
                    if from == corners[i] && height > 0. {
                        sides[i] = Some(neighbor);
                        for (k, vertex) in [(i, far_from), (j, far_to)] {
                            if inner[k].replace(vertex).is_some_and(|v| v != vertex) {
                                return None;
                            }
                        }
                    }
                    break;
                }
                if step.dot(along) <= tolerance
                    || (step - along * step.dot(along)).len() > tolerance
                {
                    return None;
                }
                (from, to) = (far_from, far_to);
            }
        }
        let travel = restore + inward * depth?;
        (travel.len() > tolerance).then_some(CellNotch::Face(FaceNotch {
            face,
            corners,
            restore,
            travel,
            sides,
            inner,
        }))
    }
}

impl FaceNotch {
    pub(super) fn apply(
        &self,
        mesh: &Mesh,
        protect: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<(Mesh, BTreeMap<usize, usize>), String> {
        let polygons = mesh.polygons();
        let mut out = mesh.clone();
        let mut removed = BTreeSet::new();
        let mut added = Vec::new();
        let mut corners_used = BTreeSet::new();
        let copies: BTreeSet<_> = symmetry.face_copies(self.face).into_iter().collect();
        for face in copies {
            let mirrored;
            let plan = if face == self.face {
                self
            } else {
                let Some(CellNotch::Face(copy)) = mesh.face_cell_notch(face) else {
                    return Err("The mirrored face needs a matching grid cell".into());
                };
                mirrored = copy;
                &mirrored
            };
            if plan
                .corners
                .iter()
                .chain(plan.inner.iter().flatten())
                .any(|&v| !corners_used.insert(v))
            {
                return Err("Mirrored face notches must occupy separate cells".into());
            }
            mesh.protect_model_vertices(&plan.corners, protect)?;
            plan.check_clear(mesh)?;
            removed.insert(face);
            removed.extend(plan.sides.iter().flatten().copied());
            let mut cap = [0; 4];
            for (i, &v) in plan.corners.iter().enumerate() {
                out.vertices[v] = mesh.vertices[v] + plan.restore;
                cap[i] = plan.inner[i].unwrap_or_else(|| {
                    out.append_blend(mesh, &[(v, 1.)], mesh.vertices[v] + plan.travel)
                });
            }
            added.push((Some(face), cap, polygons[face].group));
            for i in 0..4 {
                let j = (i + 1) % 4;
                let (a, b) = (plan.corners[i], plan.corners[j]);
                if plan.sides[i].is_none() {
                    added.push((None, [a, b, cap[j], cap[i]], polygons[face].group));
                }
                if mesh.creases.binary_search(&edge(a, b)).is_ok() {
                    out.creases.push(edge(cap[i], cap[j]));
                }
            }
        }

        // Keep untouched triangles, including their original diagonals and labels.
        let triangles = crate::sculpt::uv::polygon_triangles(mesh);
        let removed_tris: BTreeSet<_> = removed
            .iter()
            .flat_map(|&f| triangles[f].iter().copied())
            .collect();
        out.quads = mesh
            .quads
            .iter()
            .enumerate()
            .filter(|(i, _)| !removed.contains(i))
            .map(|(_, q)| *q)
            .collect();
        out.triangles.clear();
        out.polygroups.clear();
        out.hidden_faces.clear();
        for (i, t) in mesh.triangles.iter().enumerate() {
            if !removed_tris.contains(&i) {
                out.triangles.push(*t);
                out.polygroups
                    .push(mesh.polygroups.get(i).copied().unwrap_or(0));
                out.hidden_faces
                    .push(mesh.hidden_faces.get(i).copied().unwrap_or(false));
            }
        }
        let mut caps = BTreeMap::new();
        for (face, q, group) in added {
            if let Some(f) = face {
                caps.insert(f, out.quads.len());
            }
            out.push_polygon(&q, group, false);
        }
        out.retain_creases();
        out.compact_model_vertices();
        out.canonicalize_polygroups();
        Ok((out, caps))
    }

    fn check_clear(&self, mesh: &Mesh) -> Result<(), String> {
        let [a, b, _, d] = self.corners.map(|i| mesh.vertices[i] + self.restore);
        let axes = [b - a, d - a, self.travel - self.restore];
        let dual = axes.map(|v| v * (1. / v.dot(v)));
        let tolerance = axes.iter().map(|v| v.len()).fold(f32::INFINITY, f32::min) * 1e-4;
        for (i, &corner) in self.corners.iter().enumerate() {
            let target = mesh.vertices[corner] + self.travel;
            if mesh.vertices.iter().enumerate().any(|(v, p)| {
                !self.corners.contains(&v)
                    && Some(v) != self.inner[i]
                    && (*p - target).len() <= tolerance
            }) {
                return Err("Another surface occupies this grid cell".into());
            }
        }
        let exposed: Vec<_> = std::iter::once(self.corners)
            .chain(self.sides.iter().flatten().map(|&f| mesh.quads[f]))
            .collect();
        for triangle in &mesh.triangles {
            if exposed
                .iter()
                .any(|q| triangle.iter().all(|v| q.contains(v)))
            {
                continue;
            }
            let points = triangle.map(|i| {
                let shift = if self.corners.contains(&i) {
                    self.restore
                } else {
                    Vec3::default()
                };
                (mesh.vertices[i] - a) + shift
            });
            let bounds = dual.map(|axis| {
                let [a, b, c] = points.map(|p| p.dot(axis));
                (a.min(b).min(c), a.max(b).max(c))
            });
            let [(xmin, xmax), (ymin, ymax), (near, far)] = bounds;
            if far <= 1e-4 || near > 1. + 1e-4 {
                continue;
            }
            let interior = |lo: f32, hi: f32| hi > 1e-4 && lo < 1. - 1e-4;
            let on_side = |lo: f32, hi: f32| {
                (lo.abs().max(hi.abs()) < 1e-4) || ((1. - lo).abs().max((1. - hi).abs()) < 1e-4)
            };
            let x = interior(xmin, xmax);
            let y = interior(ymin, ymax);
            // A detached sheet flush with a new wall also blocks the cut. Contact
            // just along the back rim is normal on an exterior edge or corner.
            if (x && y)
                || (near < 1. - 1e-4 && ((x && on_side(ymin, ymax)) || (y && on_side(xmin, xmax))))
            {
                return Err("Another surface occupies this grid cell".into());
            }
        }
        Ok(())
    }
}
