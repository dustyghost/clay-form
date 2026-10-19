//! Exact topological correspondence, with a small scale-relative position tolerance.
use super::*;
use crate::sculpt::{Axis, Symmetry};

pub struct ModelingSymmetry {
    pub(super) vertices: Vec<Vec<usize>>,
    pub(super) bases: Vec<[Vec3; 3]>,
    pub(super) faces: Vec<Vec<usize>>,
    pub(super) settings: Symmetry,
}
impl ModelingSymmetry {
    pub fn new(mesh: &Mesh, settings: &Symmetry) -> Result<Self, String> {
        settings.validate()?;
        let polygons = mesh.polygons();
        let identity = (
            [Axis::X.vector(), Axis::Y.vector(), Axis::Z.vector()],
            false,
        );
        let mut transforms = vec![identity];
        if settings.enabled {
            for (i, &enabled) in settings.mirror.iter().enumerate() {
                if enabled {
                    let axis = Axis::ALL[i].vector();
                    let copies: Vec<_> = transforms
                        .iter()
                        .map(|(basis, reverse)| {
                            (basis.map(|v| v - axis * (2. * v.dot(axis))), !reverse)
                        })
                        .collect();
                    transforms.extend(copies);
                }
            }
            if settings.radial {
                let base = transforms.clone();
                transforms.clear();
                for step in 0..settings.radial_count {
                    let angle = std::f32::consts::TAU * step as f32 / settings.radial_count as f32;
                    transforms.extend(base.iter().map(|(basis, reverse)| {
                        (
                            basis.map(|v| settings.radial_axis.rotate(v, angle)),
                            *reverse,
                        )
                    }));
                }
            }
        }
        let mut out = Self {
            vertices: vec![(0..mesh.vertices.len()).collect()],
            bases: vec![identity.0],
            faces: vec![(0..polygons.len()).collect()],
            settings: settings.clone(),
        };
        if transforms.len() == 1 {
            return Ok(out);
        }
        mesh.validate()?;
        let mut low = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut high = low * (-1.);
        for p in &mesh.vertices {
            low = Vec3(low.0.min(p.0), low.1.min(p.1), low.2.min(p.2));
            high = Vec3(high.0.max(p.0), high.1.max(p.1), high.2.max(p.2));
        }
        let min_edge = mesh
            .polygon_edges()
            .iter()
            .map(|e| (mesh.vertices[e[0]] - mesh.vertices[e[1]]).len())
            .fold(f32::INFINITY, f32::min);
        let tolerance = ((high - low).len() * 1e-5).min(min_edge * 1e-3).max(1e-7);
        let bucket = |p: Vec3| {
            (
                (p.0 / tolerance).floor() as i64,
                (p.1 / tolerance).floor() as i64,
                (p.2 / tolerance).floor() as i64,
            )
        };
        let mut bins = BTreeMap::<_, Vec<_>>::new();
        for (i, &p) in mesh.vertices.iter().enumerate() {
            bins.entry(bucket(p)).or_default().push(i);
        }
        let mut by_corners = BTreeMap::new();
        for (i, p) in polygons.iter().enumerate() {
            let mut key = p.vertices.clone();
            key.sort_unstable();
            if by_corners.insert(key, i).is_some() {
                return Err("Symmetry needs unique, matching faces on every side".into());
            }
        }
        for (basis, reverse) in transforms.into_iter().skip(1) {
            let mut mapping = Vec::with_capacity(mesh.vertices.len());
            let mut used = BTreeSet::new();
            for &p in &mesh.vertices {
                let v = p - settings.center;
                let target = settings.center + basis[0] * v.0 + basis[1] * v.1 + basis[2] * v.2;
                let (x, y, z) = bucket(target);
                let mut found = None;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if let Some(candidates) = bins.get(&(x + dx, y + dy, z + dz)) {
                                for &i in candidates {
                                    if (mesh.vertices[i] - target).len() <= tolerance
                                        && found.replace(i).is_some()
                                    {
                                        return Err("Symmetry has ambiguous coincident vertices; clean up the mesh first".into());
                                    }
                                }
                            }
                        }
                    }
                }
                let i=found.ok_or("Modeling symmetry requires matching geometry on every enabled side; disable symmetry or make the sides match")?;
                if !used.insert(i) {
                    return Err("Symmetry vertices do not match one-to-one".into());
                }
                mapping.push(i);
            }
            let mut face_map = Vec::with_capacity(polygons.len());
            for p in &polygons {
                let mut cycle: Vec<_> = p.vertices.iter().map(|&v| mapping[v]).collect();
                let mut key = cycle.clone();
                key.sort_unstable();
                let &f = by_corners.get(&key).ok_or(
                    "Modeling symmetry requires matching face topology on every enabled side",
                )?;
                let other = &polygons[f];
                if p.visible != other.visible || p.hidden != other.hidden {
                    return Err("Show matching faces on every symmetry side before modeling".into());
                }
                if reverse {
                    cycle.reverse();
                }
                if !(0..cycle.len()).any(|offset| {
                    (0..cycle.len()).all(|i| cycle[i] == other.vertices[(i + offset) % cycle.len()])
                }) {
                    return Err("Symmetry face winding does not match".into());
                }
                face_map.push(f);
            }
            if !out.vertices.contains(&mapping) {
                out.bases.push(basis);
                out.vertices.push(mapping);
                out.faces.push(face_map);
            }
        }
        Ok(out)
    }
    pub fn vertex_copies(&self, v: usize) -> Vec<usize> {
        self.vertices
            .iter()
            .filter_map(|m| m.get(v).copied())
            .collect()
    }
    pub fn face_copies(&self, f: usize) -> Vec<usize> {
        self.faces
            .iter()
            .filter_map(|m| m.get(f).copied())
            .collect()
    }
    /// Preserve direction for off-center loop cuts, including reversed seeds.
    pub fn edge_copies(&self, e: [usize; 2]) -> Vec<[usize; 2]> {
        self.vertices
            .iter()
            .filter_map(|m| Some([*m.get(e[0])?, *m.get(e[1])?]))
            .collect()
    }
    pub fn edges(&self, seeds: &[[usize; 2]]) -> Vec<[usize; 2]> {
        seeds
            .iter()
            .flat_map(|&e| self.edge_copies(e))
            .map(|e| edge(e[0], e[1]))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn validate_result(&self, mesh: &Mesh) -> Result<(), String> {
        Self::new(mesh, &self.settings).map(|_| ())
    }
}
impl Mesh {
    pub fn extrude_symmetric(
        &self,
        selected: &[usize],
        distance: f32,
        join: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<Extrusion, String> {
        self.extrude_symmetric_contact(
            selected,
            distance,
            ExtrudeContact {
                join,
                loops: false,
                snap: 0.04,
            },
            symmetry,
        )
    }
    pub fn extrude_symmetric_contact(
        &self,
        selected: &[usize],
        distance: f32,
        contact: ExtrudeContact,
        symmetry: &ModelingSymmetry,
    ) -> Result<Extrusion, String> {
        let join = contact.join;
        if symmetry.vertices.len() == 1 && !contact.loops {
            return self.extrude_contact(selected, distance, contact);
        }
        self.validate()?;
        let polygons = self.polygons();
        if selected
            .iter()
            .any(|&f| polygons.get(f).is_none_or(|p| !p.visible))
        {
            return Err("Select visible faces on the active mesh".into());
        }
        let mut remaining: BTreeSet<_> = selected
            .iter()
            .flat_map(|&f| symmetry.face_copies(f))
            .collect();
        if remaining.is_empty() {
            return Err("Select faces to extrude".into());
        }
        let mut groups = Vec::<Vec<usize>>::new();
        while let Some(first) = remaining.pop_first() {
            let direction = normal(self, &polygons[first]);
            let mut group = vec![first];
            let mut pending = vec![first];
            while let Some(f) = pending.pop() {
                let adjacent: Vec<_> = remaining
                    .iter()
                    .copied()
                    .filter(|&i| {
                        normal(self, &polygons[i]).dot(direction) > 0.9999
                            && polygons[f].edges().any(|(a, b)| {
                                polygons[i].edges().any(|(c, d)| edge(a, b) == edge(c, d))
                            })
                    })
                    .collect();
                for i in adjacent {
                    remaining.remove(&i);
                    group.push(i);
                    pending.push(i);
                }
            }
            groups.push(group);
        }
        let mut group_vertices = BTreeSet::new();
        for group in &groups {
            let vertices: BTreeSet<_> = group
                .iter()
                .flat_map(|&f| polygons[f].vertices.iter().copied())
                .collect();
            if vertices.iter().any(|v| group_vertices.contains(v)) {
                return Err(
                    "Selected regions meet at a corner or fold across an edge; choose flat regions with no shared vertices".into(),
                );
            }
            group_vertices.extend(vertices);
        }
        // Ordinary extrusion retains per-face tunnel eligibility. Fuse instead
        // requires contact for the entire connected region, including mirror seams.
        if !contact.loops && join && distance < 0. {
            groups = groups
                .into_iter()
                .flat_map(|group| {
                    if group.len() > 1
                        && group.iter().all(|&f| {
                            self.extrude_faces(&[f], distance, true)
                                .is_ok_and(|r| r.joined)
                        })
                    {
                        group.into_iter().map(|f| vec![f]).collect::<Vec<_>>()
                    } else {
                        vec![group]
                    }
                })
                .collect();
        }
        let mut result = Extrusion {
            mesh: self.clone(),
            distance,
            joined: false,
            collapsed: false,
            partial_contact: false,
            varying_distance: false,
            rim: Vec::new(),
        };
        // Resolve contacts against the unchanged source. Earlier regions must
        // not create new targets or make a later region pass its original stop.
        let plans = if contact.loops && groups.len() > 1 {
            groups
                .iter()
                .map(|group| {
                    self.extrude_contact(group, distance, contact)
                        .map(|r| (r.distance, r.joined, r.collapsed))
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let mut first_distance: Option<f32> = None;
        let mut free_regions = 0;
        let mut joined_targets = Vec::<(Vec<Vec3>, f32)>::new();
        for (index, group) in groups.into_iter().enumerate() {
            // Earlier edits compact vertex indices. Locate untouched source caps
            // by their original corners, rather than reusing stale indices.
            let current = result.mesh.polygons();
            let indices: Vec<_> = group
                .iter()
                .filter_map(|&f| {
                    current.iter().position(|p| {
                        p.vertices.len() == polygons[f].vertices.len()
                            && polygons[f].vertices.iter().all(|&v| {
                                p.vertices
                                    .iter()
                                    .any(|&w| self.vertices[v] == result.mesh.vertices[w])
                            })
                    })
                })
                .collect();
            if indices.is_empty()
                && group.iter().all(|&f| {
                    joined_targets.iter().any(|(target, tolerance)| {
                        target.len() == polygons[f].vertices.len()
                            && polygons[f].vertices.iter().all(|&v| {
                                target
                                    .iter()
                                    .any(|&p| (p - self.vertices[v]).len() <= *tolerance)
                            })
                    })
                })
            {
                continue; // Both selected ends of this tunnel were already joined.
            }
            if indices.len() != group.len() {
                return Err("Selected extrusion regions overlap; the edit was cancelled".into());
            }
            let next = result.mesh.extrude_contact(&indices, distance, contact)?;
            if let Some(&(depth, joined, collapsed)) = plans.get(index)
                && (next.joined != joined
                    || next.collapsed != collapsed
                    || (next.distance - depth).abs() > depth.abs().max(1.) * 1e-5)
            {
                return Err(
                    "Selected regions interfere with each other's contact; extrude them separately"
                        .into(),
                );
            }
            if let Some(first) = first_distance {
                result.varying_distance |=
                    (next.distance - first).abs() > first.abs().max(1.) * 1e-5;
            } else {
                first_distance = Some(next.distance);
            }
            free_regions += usize::from(!next.joined && !next.collapsed);
            if next.joined {
                let first = &polygons[group[0]];
                let direction = normal(self, first);
                let tolerance = (first
                    .edges()
                    .map(|(a, b)| (self.vertices[a] - self.vertices[b]).len())
                    .fold(0_f32, f32::max)
                    * 1e-4)
                    .max(1e-7);
                joined_targets.extend(group.iter().map(|&f| {
                    (
                        polygons[f]
                            .vertices
                            .iter()
                            .map(|&v| self.vertices[v] + direction * next.distance)
                            .collect(),
                        tolerance,
                    )
                }));
            }
            result.mesh = next.mesh;
            result.distance = if result.varying_distance {
                distance
            } else {
                next.distance
            };
            result.joined |= next.joined;
            result.collapsed |= next.collapsed;
            result.rim.extend(next.rim);
        }
        result.partial_contact = free_regions > 0 && (result.joined || result.collapsed);
        symmetry.validate_result(&result.mesh)?;
        Ok(result)
    }
}
