//! Weld only matching end boundaries of consecutive sections, never arbitrary
//! nearby surfaces. Remove the internal end caps before welding.
use super::*;
use crate::sculpt::VertexGroup;
use std::collections::{BTreeSet, HashSet};

struct End {
    vertices: Vec<usize>,
    edges: HashSet<(usize, usize)>,
    caps: Vec<usize>,
    on_plane: Vec<bool>,
}
impl End {
    fn new(
        mesh: &Mesh,
        piece: Piece,
        high: bool,
        remove_caps: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let piece = Piece {
            pivot: Vec3::default(),
            ..piece
        };
        let (lo, hi) = longitudinal_bounds(mesh, piece)?;
        let plane = if high { hi } else { lo };
        let epsilon = ((hi - lo) * 1e-5).max(1e-6);
        let on_plane: Vec<_> = mesh
            .vertices
            .iter()
            .map(|&p| (piece.local(p).0 - plane).abs() <= epsilon)
            .collect();
        let mut caps = Vec::new();
        let mut incidence = HashMap::<(usize, usize), (usize, (usize, usize))>::new();
        for (face, &[a, b, c]) in mesh.triangles.iter().enumerate() {
            if face % 4096 == 0 && cancel() {
                return Err("Operation cancelled".into());
            }
            if remove_caps && [a, b, c].iter().all(|&i| on_plane[i]) {
                caps.push(face);
                continue;
            }
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let entry = incidence.entry((a.min(b), a.max(b))).or_insert((0, (a, b)));
                entry.0 += 1;
            }
        }
        let edges: HashSet<_> = incidence
            .into_values()
            .filter_map(|(n, (a, b))| (n == 1 && on_plane[a] && on_plane[b]).then_some((a, b)))
            .collect();
        let vertices: Vec<_> = edges
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if vertices.len() < 3 || vertices.len() > 4096 {
            return Err("Each joining end needs a planar boundary with 3–4096 vertices along its Forward axis".into());
        }
        let mut incoming = HashSet::new();
        let mut outgoing = HashSet::new();
        for &(a, b) in &edges {
            if !outgoing.insert(a) || !incoming.insert(b) {
                return Err(
                    "Section end boundaries branch; use compatible simple end loops".into(),
                );
            }
        }
        if incoming != outgoing {
            return Err("Section end boundary is incomplete".into());
        }
        Ok(Self {
            vertices,
            edges,
            caps,
            on_plane,
        })
    }
}
fn root(parent: &mut [usize], mut v: usize) -> usize {
    while parent[v] != v {
        parent[v] = parent[parent[v]];
        v = parent[v];
    }
    v
}
fn matches(
    a: &End,
    b: &End,
    am: &Mesh,
    bm: &Mesh,
    tolerance: f32,
    work: &mut usize,
    cancel: &dyn Fn() -> bool,
) -> Result<HashMap<usize, usize>, String> {
    if a.vertices.len() != b.vertices.len() {
        return Err("Neighbouring ends have different boundary vertex counts; match their subdivisions or choose Separate".into());
    }
    let key = |p: Vec3| {
        (
            (p.0 / tolerance).floor() as i64,
            (p.1 / tolerance).floor() as i64,
            (p.2 / tolerance).floor() as i64,
        )
    };
    let mut grid = HashMap::<_, Vec<usize>>::new();
    for &i in &b.vertices {
        grid.entry(key(bm.vertices[i])).or_default().push(i);
    }
    let mut used = HashSet::new();
    let mut result = HashMap::new();
    for &i in &a.vertices {
        if cancel() {
            return Err("Operation cancelled".into());
        }
        let p = am.vertices[i];
        let (x, y, z) = key(p);
        let mut best = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(candidates) = grid.get(&(x + dx, y + dy, z + dz)) {
                        for &j in candidates {
                            *work += 1;
                            if *work > 2_000_000 {
                                return Err("Join matching budget exceeded; reduce Join distance or end detail".into());
                            }
                            let d = (p - bm.vertices[j]).len();
                            if !used.contains(&j)
                                && d <= tolerance
                                && best.is_none_or(|(distance, index)| {
                                    d < distance || (d == distance && j < index)
                                })
                            {
                                best = Some((d, j));
                            }
                        }
                    }
                }
            }
        }
        let (_,j)=best.ok_or("Neighbouring ends are farther apart than Join distance or their shapes differ; fit section lengths, adjust spacing, or choose Separate")?;
        used.insert(j);
        result.insert(i, j);
    }
    Ok(result)
}

/// Adjacent regions of one source share vertex IDs. Only newly repeated
/// sections need their end caps removed and their loops matched by position.
pub(super) struct Section<'a> {
    pub mesh: &'a Mesh,
    pub original_vertices: Option<&'a [usize]>,
    pub reversed: bool,
}
impl Section<'_> {
    fn original_seam(&self, neighbour: &Self) -> bool {
        self.original_vertices.is_some() && !std::ptr::eq(self.mesh, neighbour.mesh)
    }
}

pub(super) fn join<'a>(
    copies: &[GeneratedCopy],
    source: &impl Fn(Source) -> Option<&'a Mesh>,
    closed: bool,
    tolerance: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let sources = copies
        .iter()
        .map(|c| {
            source(c.placement.piece.source)
                .map(|mesh| Section {
                    mesh,
                    original_vertices: None,
                    reversed: false,
                })
                .ok_or_else(|| "Instance source is missing".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    join_parts(copies, &sources, closed, tolerance, cancel)
}

pub(super) fn join_parts(
    copies: &[GeneratedCopy],
    sources: &[Section],
    closed: bool,
    tolerance: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let n = copies.len();
    let mut offsets = Vec::with_capacity(n);
    let mut total = 0;
    let mut ends = Vec::with_capacity(n);
    let mut source_ends = HashMap::<_, Arc<End>>::new();
    for (i, c) in copies.iter().enumerate() {
        if cancel() {
            return Err("Operation cancelled".into());
        }
        let section = &sources[i];
        let mesh = section.mesh;
        if mesh.has_hidden_faces() {
            return Err("Joining requires fully visible sections".into());
        }
        offsets.push(total);
        total += c.mesh.vertices.len();
        let mut end_for = |outgoing: bool| -> Result<Arc<End>, String> {
            let neighbour = if outgoing {
                (i + 1) % n
            } else {
                (i + n - 1) % n
            };
            let remove_caps = !section.original_seam(&sources[neighbour]);
            let high = outgoing != section.reversed;
            let key = (
                mesh as *const Mesh as usize,
                c.placement.piece.forward,
                high,
                remove_caps,
            );
            if let Some(end) = source_ends.get(&key) {
                return Ok(end.clone());
            }
            let end = Arc::new(End::new(
                mesh,
                c.placement.piece,
                high,
                remove_caps,
                cancel,
            )?);
            source_ends.insert(key, end.clone());
            Ok(end)
        };
        ends.push((
            if i > 0 || closed {
                Some(end_for(false)?)
            } else {
                None
            },
            if i + 1 < n || closed {
                Some(end_for(true)?)
            } else {
                None
            },
        ));
    }
    let mut parent: Vec<_> = (0..total).collect();
    let mut seams = Vec::new();
    let mut work = 0;
    for i in 0..n.saturating_sub(1) + usize::from(closed) {
        if cancel() {
            return Err("Operation cancelled".into());
        }
        let j = (i + 1) % n;
        let a = ends[i].1.as_ref().unwrap();
        let b = ends[j].0.as_ref().unwrap();
        let original_seam = sources[i].original_seam(&sources[j]);
        let matching = if original_seam {
            let left = sources[i].original_vertices.unwrap();
            let right = sources[j].original_vertices.unwrap();
            let by_id: HashMap<_, _> = b.vertices.iter().map(|&v| (right[v], v)).collect();
            a.vertices
                .iter()
                .map(|&v| {
                    by_id
                        .get(&left[v])
                        .map(|&w| (v, w))
                        .ok_or("Source regions must share a welded edge loop")
                })
                .collect::<Result<HashMap<_, _>, _>>()?
        } else {
            matches(
                a,
                b,
                &copies[i].mesh,
                &copies[j].mesh,
                tolerance,
                &mut work,
                cancel,
            )?
        };
        if matching.len() != b.vertices.len()
            || a.edges
                .iter()
                .any(|&(x, y)| !b.edges.contains(&(matching[&y], matching[&x])))
        {
            return Err(
                "Neighbouring end topology or winding is incompatible; use matching end loops"
                    .into(),
            );
        }
        for (&a, &b) in &matching {
            let a = root(&mut parent, offsets[i] + a);
            let b = root(&mut parent, offsets[j] + b);
            parent[b] = a;
        }
        if !original_seam {
            seams.extend(
                a.edges
                    .iter()
                    .map(|&(a, b)| (offsets[i] + a, offsets[i] + b)),
            );
        }
    }
    for i in 0..total {
        parent[i] = root(&mut parent, i);
    }
    let seams: HashSet<_> = seams
        .into_iter()
        .map(|(a, b)| {
            let (a, b) = (parent[a], parent[b]);
            (a.min(b), a.max(b))
        })
        .collect();
    let mut positions = vec![Vec3::default(); total];
    let mut counts = vec![0usize; total];
    let mut masks = vec![0_f32; total];
    let mut group_names = Vec::<(String, bool, crate::sculpt::GroupPurpose)>::new();
    let mut group_maps = Vec::new();
    for (i, c) in copies.iter().enumerate() {
        let mut map = Vec::new();
        for g in &c.mesh.vertex_groups {
            let key = (g.name.clone(), g.visible, g.purpose);
            let index = if let Some(index) = group_names.iter().position(|k| k == &key) {
                index
            } else {
                group_names.push(key);
                group_names.len() - 1
            };
            if group_names.len() > 64 {
                return Err("Joined assembly exceeds 64 distinct vertex groups".into());
            }
            map.push(index);
        }
        group_maps.push(map);
        for (v, &p) in c.mesh.vertices.iter().enumerate() {
            let r = parent[offsets[i] + v];
            positions[r] = positions[r] + p;
            counts[r] += 1;
            masks[r] = masks[r].max(c.mesh.mask[v]);
        }
    }
    let mut result = Mesh::default();
    let mut indices = vec![usize::MAX; total];
    for (i, c) in copies.iter().enumerate() {
        let removed: HashSet<_> = ends[i]
            .0
            .iter()
            .chain(ends[i].1.iter())
            .flat_map(|e| e.caps.iter().copied())
            .collect();
        for (f, t) in c.mesh.triangles.iter().enumerate() {
            if f % 4096 == 0 && cancel() {
                return Err("Operation cancelled".into());
            }
            if removed.contains(&f) {
                continue;
            }
            let t = t.map(|v| parent[offsets[i] + v]);
            if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                return Err("Join collapsed a face; reduce Join distance".into());
            }
            for &v in &t {
                if indices[v] == usize::MAX {
                    indices[v] = result.vertices.len();
                    result.vertices.push(positions[v] * (1. / counts[v] as f32));
                    result.mask.push(masks[v]);
                }
            }
            result.triangles.push(t.map(|v| indices[v]));
            let group = c.mesh.polygroups.get(f).copied().unwrap_or(0);
            result.polygroups.push(if group == 0 {
                0
            } else {
                group_maps[i][group - 1] + 1
            });
        }
        for q in &c.mesh.quads {
            if ends[i]
                .0
                .iter()
                .chain(ends[i].1.iter())
                .any(|e| !e.caps.is_empty() && q.iter().all(|&v| e.on_plane[v]))
            {
                continue;
            }
            let q = q.map(|v| indices[parent[offsets[i] + v]]);
            if q.iter().all(|&v| v != usize::MAX) {
                result.quads.push(q);
            }
        }
        for &[a, b] in &c.mesh.creases {
            let (a, b) = (parent[offsets[i] + a], parent[offsets[i] + b]);
            if a != b
                && indices[a] != usize::MAX
                && indices[b] != usize::MAX
                && !seams.contains(&(a.min(b), a.max(b)))
            {
                let (a, b) = (indices[a], indices[b]);
                result.creases.push([a.min(b), a.max(b)]);
            }
        }
    }
    result.vertex_groups = group_names
        .into_iter()
        .map(|(name, visible, purpose)| VertexGroup {
            purpose,
            name,
            visible,
            weights: vec![0.; result.vertices.len()],
        })
        .collect();
    let has_materials = copies.iter().any(|c| !c.mesh.materials.is_empty());
    if has_materials {
        result.materials = vec![crate::sculpt::Material::default(); result.vertices.len()];
    }
    let mut material_counts = vec![0usize; result.vertices.len()];
    for (i, c) in copies.iter().enumerate() {
        for v in 0..c.mesh.vertices.len() {
            let new = indices[parent[offsets[i] + v]];
            if new == usize::MAX {
                continue;
            }
            for (g, &target) in group_maps[i].iter().enumerate() {
                result.vertex_groups[target].weights[new] = result.vertex_groups[target].weights
                    [new]
                    .max(c.mesh.vertex_groups[g].weights[v]);
            }
            if has_materials {
                let count = material_counts[new] as f32;
                result.materials[new] = if count == 0. {
                    c.mesh.material(v)
                } else {
                    crate::sculpt::Material::blend([
                        (result.materials[new].clone(), count / (count + 1.)),
                        (c.mesh.material(v), 1. / (count + 1.)),
                    ])
                };
                material_counts[new] += 1;
            }
        }
    }
    // Shoulder faces cut away between repeats can also remove their creased edges.
    result.retain_creases();
    result.canonicalize_polygroups();
    // UVs deliberately remain empty: different section images cannot be combined
    // into one mesh's channel assignments without an atlas operation.
    let mut edges = HashMap::<(usize, usize), (usize, i32)>::new();
    for &[a, b, c] in &result.triangles {
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let entry = edges.entry((a.min(b), a.max(b))).or_default();
            entry.0 += 1;
            entry.1 += if a < b { 1 } else { -1 };
        }
    }
    if edges.values().any(|&(n, d)| n > 2 || (n == 2 && d != 0)) {
        return Err("Join would produce a non-manifold or inconsistently oriented surface".into());
    }
    result.validate()?;
    Ok(result)
}
