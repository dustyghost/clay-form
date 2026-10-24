//! Polygon boundaries, persistent hard edges, and explicit topology edits.
use super::{Geometry, MAX_TRIANGLES, MAX_VERTICES, Mesh, MeshSettings, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(super) type Edge = [usize; 2];
pub(super) fn edge(a: usize, b: usize) -> Edge {
    [a.min(b), a.max(b)]
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderTarget {
    Holes,
    Masked,
    VisibleGroups,
    Selected,
}
impl BorderTarget {
    pub const ALL: [Self; 4] = [
        Self::Holes,
        Self::Masked,
        Self::VisibleGroups,
        Self::Selected,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Holes => "Hole borders",
            Self::Masked => "Masked region borders",
            Self::VisibleGroups => "Visible polygroup borders",
            Self::Selected => "Selected edges",
        }
    }
}
#[derive(Clone, Debug)]
pub enum EdgeEdit {
    Crease {
        target: BorderTarget,
        selected: Vec<Edge>,
        remove: bool,
    },
    ClearCreases,
    Loops {
        target: BorderTarget,
        selected: Vec<Edge>,
        count: u32,
        width: f32,
    },
    Relax {
        target: BorderTarget,
        selected: Vec<Edge>,
        amount: f32,
    },
    SmoothSubdivide(f32),
}
impl EdgeEdit {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Crease { remove: false, .. } => "Crease edges",
            Self::Crease { remove: true, .. } => "Uncrease edges",
            Self::ClearCreases => "Clear creases",
            Self::Loops { .. } => "Add support loops",
            Self::Relax { .. } => "Relax borders",
            Self::SmoothSubdivide(_) => "Smooth subdivide",
        }
    }
}
#[derive(Clone)]
pub(crate) struct Polygon {
    pub vertices: Vec<usize>,
    pub group: usize,
    pub hidden: bool,
    pub visible: bool,
}
impl Polygon {
    pub fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.vertices
            .iter()
            .copied()
            .zip(self.vertices.iter().copied().cycle().skip(1))
    }
}
pub(super) fn cancelled(cancel: &impl Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Edge operation cancelled".into())
    } else {
        Ok(())
    }
}

impl Mesh {
    pub(crate) fn polygons(&self) -> Vec<Polygon> {
        let key = |mut t: [usize; 3]| {
            t.sort_unstable();
            t
        };
        let faces: HashMap<_, _> = self
            .triangles
            .iter()
            .enumerate()
            .map(|(i, &t)| (key(t), i))
            .collect();
        let mut used = BTreeSet::new();
        let mut result = Vec::new();
        for &[a, b, c, d] in &self.quads {
            let pair = [[[a, b, c], [a, c, d]], [[a, b, d], [b, c, d]]]
                .into_iter()
                .find_map(|ts| Some([*faces.get(&key(ts[0]))?, *faces.get(&key(ts[1]))?]));
            if let Some(pair) = pair {
                used.extend(pair);
                result.push(Polygon {
                    vertices: vec![a, b, c, d],
                    group: self.polygroups.get(pair[0]).copied().unwrap_or(0),
                    hidden: pair
                        .iter()
                        .any(|&i| self.hidden_faces.get(i).copied().unwrap_or(false)),
                    visible: pair.iter().all(|&i| self.face_visible(i)),
                });
            }
        }
        for (i, t) in self.triangles.iter().enumerate() {
            if !used.contains(&i) {
                result.push(Polygon {
                    vertices: t.to_vec(),
                    group: self.polygroups.get(i).copied().unwrap_or(0),
                    hidden: self.hidden_faces.get(i).copied().unwrap_or(false),
                    visible: self.face_visible(i),
                });
            }
        }
        result
    }
    pub fn polygon_edges(&self) -> Vec<Edge> {
        self.polygons()
            .iter()
            .flat_map(|p| p.edges().map(|(a, b)| edge(a, b)))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub(super) fn validate_creases(&self) -> Result<(), String> {
        if self.creases.is_empty() {
            return Ok(());
        }
        let edges: BTreeSet<_> = self.polygon_edges().into_iter().collect();
        if self.creases.len() > MAX_TRIANGLES * 3
            || self.creases.windows(2).any(|p| p[0] >= p[1])
            || self
                .creases
                .iter()
                .any(|&[a, b]| a >= b || b >= self.vertices.len() || !edges.contains(&[a, b]))
        {
            return Err("Invalid crease edges".into());
        }
        Ok(())
    }
    pub(super) fn retain_creases(&mut self) {
        if self.creases.is_empty() {
            return;
        }
        let edges: BTreeSet<_> = self.polygon_edges().into_iter().collect();
        self.creases.retain(|e| edges.contains(e));
        self.creases.sort_unstable();
        self.creases.dedup();
    }
    pub(super) fn reindex_creases(&self, out: &mut Mesh, indices: &[usize]) {
        if self.creases.is_empty() {
            out.creases.clear();
            return;
        }
        out.creases = self
            .creases
            .iter()
            .filter_map(|&[a, b]| {
                let (a, b) = (indices[a], indices[b]);
                (a != usize::MAX && b != usize::MAX && a != b).then(|| edge(a, b))
            })
            .collect();
        out.retain_creases();
    }
    pub fn border_edges(
        &self,
        target: BorderTarget,
        threshold: f32,
        selected: &[Edge],
    ) -> Result<Vec<Edge>, String> {
        self.validate()?;
        let polygons = self.polygons();
        let regions = regions(&polygons, self, target, threshold);
        let boundaries = boundaries(&polygons, &regions, target)?;
        let visible: BTreeSet<_> = polygons
            .iter()
            .filter(|p| p.visible)
            .flat_map(|p| p.edges().map(|(a, b)| edge(a, b)))
            .collect();
        Ok(if target == BorderTarget::Selected {
            selected
                .iter()
                .copied()
                .filter(|e| visible.contains(e))
                .collect::<BTreeSet<_>>()
        } else {
            boundaries
                .values()
                .flatten()
                .map(|&(a, b, _)| edge(a, b))
                .collect()
        }
        .into_iter()
        .collect())
    }
    /// Follow a crease/border chain first, then an unambiguous quad edge loop.
    pub fn expand_edge_loop(&self, selected: &[Edge]) -> Result<Vec<Edge>, String> {
        self.validate()?;
        let polygons = self.polygons();
        let mut incidence: BTreeMap<Edge, Vec<usize>> = BTreeMap::new();
        for (i, p) in polygons.iter().enumerate() {
            if p.visible {
                for (a, b) in p.edges() {
                    incidence.entry(edge(a, b)).or_default().push(i);
                }
            }
        }
        let mut at_vertex: BTreeMap<usize, Vec<Edge>> = BTreeMap::new();
        for &e in incidence.keys() {
            for v in e {
                at_vertex.entry(v).or_default().push(e);
            }
        }
        let mut out: BTreeSet<_> = selected
            .iter()
            .copied()
            .filter(|e| incidence.contains_key(e))
            .collect();
        let mut pending: Vec<_> = out.iter().copied().collect();
        while let Some(current) = pending.pop() {
            for vertex in current {
                let candidates: Vec<_> = at_vertex[&vertex]
                    .iter()
                    .copied()
                    .filter(|e| {
                        *e != current
                            && if self.creases.binary_search(&current).is_ok() {
                                self.creases.binary_search(e).is_ok()
                            } else if incidence[&current].len() == 1 {
                                incidence[e].len() == 1
                            } else {
                                !incidence[e].iter().any(|f| incidence[&current].contains(f))
                            }
                    })
                    .collect();
                if candidates.len() == 1 && out.insert(candidates[0]) {
                    pending.push(candidates[0]);
                }
            }
        }
        Ok(out.into_iter().collect())
    }
    pub(super) fn empty_topology(&self) -> Self {
        let mut out = self.clone();
        out.triangles.clear();
        out.quads.clear();
        out.polygroups.clear();
        out.hidden_faces.clear();
        out.creases.clear();
        out
    }
    pub(super) fn push_polygon(&mut self, vertices: &[usize], group: usize, hidden: bool) {
        if vertices.len() == 4 {
            self.quads
                .push([vertices[0], vertices[1], vertices[2], vertices[3]]);
        }
        for i in 1..vertices.len() - 1 {
            self.triangles
                .push([vertices[0], vertices[i], vertices[i + 1]]);
            self.polygroups.push(group);
            self.hidden_faces.push(hidden);
        }
    }
    pub(super) fn append_blend(
        &mut self,
        source: &Mesh,
        weights: &[(usize, f32)],
        position: Vec3,
    ) -> usize {
        let index = self.vertices.len();
        self.append_material(source, weights);
        self.vertices.push(position);
        self.mask.push(
            weights
                .iter()
                .map(|&(i, w)| source.mask[i] * w)
                .sum::<f32>()
                .clamp(0., 1.),
        );
        for (out, g) in self.vertex_groups.iter_mut().zip(&source.vertex_groups) {
            out.weights.push(
                weights
                    .iter()
                    .map(|&(i, w)| g.weights[i] * w)
                    .sum::<f32>()
                    .clamp(0., 1.),
            );
        }
        index
    }
}

fn regions(
    polygons: &[Polygon],
    mesh: &Mesh,
    target: BorderTarget,
    threshold: f32,
) -> Vec<Option<usize>> {
    polygons
        .iter()
        .map(|p| {
            if !p.visible {
                None
            } else {
                match target {
                    BorderTarget::Masked => p
                        .vertices
                        .iter()
                        .all(|&v| super::mask_selected(mesh.mask[v], threshold))
                        .then_some(0),
                    BorderTarget::VisibleGroups => Some(p.group),
                    _ => Some(0),
                }
            }
        })
        .collect()
}
type Boundaries = BTreeMap<usize, Vec<(usize, usize, usize)>>;
fn boundaries(
    polygons: &[Polygon],
    regions: &[Option<usize>],
    target: BorderTarget,
) -> Result<Boundaries, String> {
    let mut incidence: BTreeMap<Edge, Vec<(usize, usize, usize)>> = BTreeMap::new();
    for (i, p) in polygons.iter().enumerate() {
        for (a, b) in p.edges() {
            incidence.entry(edge(a, b)).or_default().push((a, b, i));
        }
    }
    let mut result: Boundaries = BTreeMap::new();
    for faces in incidence.values() {
        if faces.len() > 2 || (faces.len() == 2 && faces[0].0 == faces[1].0) {
            return Err("Edge tools require consistently oriented manifold polygons".into());
        }
        for &(a, b, i) in faces {
            if let Some(region) = regions[i] {
                let boundary = if target == BorderTarget::Holes {
                    faces.len() == 1
                } else {
                    faces.len() == 1 || faces.iter().any(|&(_, _, j)| regions[j] != Some(region))
                };
                if boundary {
                    result.entry(region).or_default().push((a, b, i));
                }
            }
        }
    }
    Ok(result)
}

impl Geometry {
    pub fn edit_edges(
        &mut self,
        edit: &EdgeEdit,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        cancelled(cancel)?;
        if !self.scene.active_metadata.visible {
            return Err("Show the active model before editing edges".into());
        }
        if self.settings.primitive.is_some() {
            return Err("Convert the primitive to a sculpting mesh first".into());
        }
        if self.scene.active_metadata.temporary_thickness > 0. {
            return Err("Apply or disable the Thickness modifier before editing base-mesh edges; rim controls are in Submodels".into());
        }
        let threshold = self.scene.active_metadata.mask_threshold;
        let mut out = self.mesh.clone();
        match edit {
            EdgeEdit::ClearCreases => out.creases.clear(),
            EdgeEdit::Crease {
                target,
                selected,
                remove,
            } => {
                let edges = out.border_edges(*target, threshold, selected)?;
                if edges.is_empty() {
                    return Err("No edges match this target".into());
                }
                let mut creases: BTreeSet<_> = out.creases.iter().copied().collect();
                for e in edges {
                    if *remove {
                        creases.remove(&e);
                    } else {
                        creases.insert(e);
                    }
                }
                out.creases = creases.into_iter().collect();
            }
            EdgeEdit::Loops {
                target,
                selected,
                count,
                width,
            } => {
                out = out.support_loops(*target, threshold, selected, *count, *width, cancel)?;
            }
            EdgeEdit::Relax {
                target,
                selected,
                amount,
            } => {
                out = out.relax_borders(*target, threshold, selected, *amount, cancel)?;
            }
            EdgeEdit::SmoothSubdivide(amount) => {
                if out.has_hidden_faces() {
                    return Err("Show all faces before smooth subdivision".into());
                }
                out = out.smooth_subdivide(*amount, cancel)?;
            }
        }
        cancelled(cancel)?;
        out.validate()?;
        if out == self.mesh {
            return Ok(());
        }
        self.mesh = out;
        self.settings = MeshSettings {
            resolution: self.settings.resolution,
            ..Default::default()
        };
        Ok(())
    }
}

mod loops;
mod smooth;

#[cfg(test)]
mod tests;
