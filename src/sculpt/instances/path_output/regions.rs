//! A connected source stays intact; only transient evaluation extracts its regions.
use super::*;
use std::collections::HashSet;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathRegions {
    pub start: String,
    pub middle: String,
    pub end: String,
}
impl PathRegions {
    pub fn named(mesh: &Mesh) -> Self {
        let find = |role: &str| {
            mesh.vertex_groups
                .iter()
                .find(|g| g.purpose.is_selection() && g.name.eq_ignore_ascii_case(role))
                .map_or_else(String::new, |g| g.name.clone())
        };
        Self {
            start: find("Start"),
            middle: find("Middle"),
            end: find("End"),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let names = [&self.start, &self.middle, &self.end];
        if names.iter().any(|n| !name_valid(n))
            || names.into_iter().collect::<HashSet<_>>().len() != 3
        {
            return Err("Assign three different vertex groups to Start, Middle and End".into());
        }
        Ok(())
    }
    fn faces(&self, mesh: &Mesh) -> Result<Vec<usize>, String> {
        self.validate()?;
        let mut groups = Vec::new();
        for name in [&self.start, &self.middle, &self.end] {
            groups.push(
                mesh.vertex_groups
                    .iter()
                    .position(|g| g.purpose.is_selection() && &g.name == name)
                    .ok_or_else(|| {
                        format!("Path region group ‘{name}’ is missing; reassign its role")
                    })?,
            );
        }
        let labelled = !mesh.polygroups.is_empty();
        mesh.triangles.iter().enumerate().map(|(f, triangle)| {
            let mut selected = None;
            for (role, &group) in groups.iter().enumerate() {
                let member = if labelled { mesh.polygroups[f] == group + 1 }
                    else { triangle.iter().all(|&v| mesh.vertex_groups[group].weights[v] >= 0.5) };
                if member {
                    if selected.is_some() { return Err("Path region weights overlap across a face. Assign adjoining face regions with shared boundary vertices only".into()); }
                    selected = Some(role);
                }
            }
            selected.ok_or_else(|| "Assign every source face to exactly one Start, Middle or End group. Use face selections, with edge loops at the region boundaries".into())
        }).collect()
    }
    /// Pick the axis whose two cross-sections actually separate the three regions.
    /// Only do this when binding groups, so subsequent manual axis edits stick.
    pub(crate) fn orient_piece(&self, mesh: &Mesh, piece: &mut Piece) {
        if mesh.vertices.len() > MAX_GENERATED_VERTICES
            || mesh.triangles.len() > MAX_GENERATED_TRIANGLES
        {
            return;
        }
        let Ok(faces) = self.faces(mesh) else { return };
        for forward in 0..3 {
            let candidate = Piece { forward, ..*piece };
            if self.bounds(mesh, candidate, &faces).is_err() {
                continue;
            }
            piece.forward = forward;
            // Captures already have an authored surface normal. For models,
            // lay the thin dimension against the surface, keeping Up on ties.
            if !matches!(piece.source, Source::Asset(_)) || piece.up == forward {
                let extent = |i| {
                    let dir = axis(i);
                    let (lo, hi) = mesh
                        .vertices
                        .iter()
                        .map(|p| p.dot(dir))
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                            (lo.min(v), hi.max(v))
                        });
                    hi - lo
                };
                let mut axes = (0..3).filter(|&i| i != forward);
                let a = axes.next().unwrap();
                let b = axes.next().unwrap();
                let (x, y) = (extent(a), extent(b));
                piece.up = if x < y || (x == y && piece.up == a) {
                    a
                } else {
                    b
                };
            }
            break;
        }
    }

    fn bounds(&self, mesh: &Mesh, piece: Piece, faces: &[usize]) -> Result<RegionBounds, String> {
        let mut bounds = [(f32::INFINITY, f32::NEG_INFINITY); 3];
        for (face, &role) in mesh.triangles.iter().zip(faces) {
            for &v in face {
                let x = piece.local(mesh.vertices[v]).0;
                bounds[role].0 = bounds[role].0.min(x);
                bounds[role].1 = bounds[role].1.max(x);
            }
        }
        if bounds
            .iter()
            .any(|&(lo, hi)| !lo.is_finite() || hi - lo < 1e-5)
        {
            return Err(
                "Each path region needs surface faces and some length along Forward".into(),
            );
        }
        let reversed = bounds[0].0 > bounds[2].0;
        if reversed {
            bounds = bounds.map(|(lo, hi)| (-hi, -lo));
        }
        let epsilon = ((bounds[2].1 - bounds[0].0).abs() * 1e-5).max(1e-6);
        if (bounds[0].1 - bounds[1].0).abs() > epsilon
            || (bounds[1].1 - bounds[2].0).abs() > epsilon
        {
            return Err("Start, Middle and End must run in that order along Forward, meeting at planar edge loops. Check the Forward axis and group assignments".into());
        }
        Ok(RegionBounds {
            spans: bounds,
            reversed,
        })
    }
}

struct RegionBounds {
    spans: [(f32, f32); 3],
    reversed: bool,
}

impl PathAssembly {
    pub(crate) fn region_section_centres(
        &self,
        mesh: &Mesh,
        length: f32,
        closed: bool,
    ) -> Result<Vec<f32>, String> {
        let groups = self.regions.as_ref().ok_or("No path regions assigned")?;
        let faces = groups.faces(mesh)?;
        let bounds = groups.bounds(mesh, self.middle, &faces)?;
        Ok(intervals(self, &bounds.spans, length, closed)?
            .into_iter()
            .map(|(_, lo, hi)| (lo + hi) * 0.5)
            .collect())
    }
}

fn intervals(
    recipe: &PathAssembly,
    bounds: &[(f32, f32); 3],
    length: f32,
    closed: bool,
) -> Result<Vec<(usize, f32, f32)>, String> {
    let start = if closed {
        0.
    } else {
        (bounds[0].1 - bounds[0].0) * recipe.scale
    };
    let end = if closed {
        0.
    } else {
        (bounds[2].1 - bounds[2].0) * recipe.scale
    };
    if length - start - end <= 1e-5 {
        return Err("The path is shorter than its Start and End regions. Lengthen the path or reduce Piece scale".into());
    }
    let step = (length - start - end) / recipe.count as f32;
    let mut spans = Vec::new();
    if !closed {
        spans.push((0, 0., start));
    }
    for i in 0..recipe.count {
        spans.push((1, start + i as f32 * step, start + (i + 1) as f32 * step));
    }
    if !closed {
        spans.push((2, length - end, length));
    }
    Ok(spans)
}

fn extract(mesh: &Mesh, faces: &[usize], role: usize) -> Result<(Mesh, Vec<usize>), String> {
    let mut result = Mesh::default();
    let mut indices = vec![usize::MAX; mesh.vertices.len()];
    let mut old_vertices = Vec::new();
    let mut old_faces = Vec::new();
    let mut triangles = HashSet::new();
    let key = |mut t: [usize; 3]| {
        t.sort_unstable();
        t
    };
    for (f, t) in mesh.triangles.iter().enumerate() {
        if faces[f] != role {
            continue;
        }
        for &v in t {
            if indices[v] == usize::MAX {
                indices[v] = result.vertices.len();
                old_vertices.push(v);
                result.vertices.push(mesh.vertices[v]);
                result.mask.push(mesh.mask[v]);
            }
        }
        result.triangles.push(t.map(|v| indices[v]));
        triangles.insert(key(*t));
        old_faces.push(f);
    }
    for &[a, b, c, d] in &mesh.quads {
        if (triangles.contains(&key([a, b, c])) && triangles.contains(&key([a, c, d])))
            || (triangles.contains(&key([a, b, d])) && triangles.contains(&key([b, c, d])))
        {
            result.quads.push([a, b, c, d].map(|v| indices[v]));
        }
    }
    mesh.reindex_groups(&mut result, &old_vertices, &old_faces);
    result.validate()?;
    Ok((result, old_vertices))
}

pub(super) fn generate<'a>(
    set: &Set,
    recipe: &PathAssembly,
    paths: &[SurfacePath],
    source: impl Fn(Source) -> Option<&'a Mesh>,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<GeneratedCopy>, String> {
    let placements = set.placements(paths)?;
    if placements.is_empty() {
        return Ok(Vec::new());
    }
    if !set.adjustments.is_empty() {
        return Err("Transform the saved path to move a grouped path object; make it editable for independent mesh edits".into());
    }
    let path = paths
        .iter()
        .find(|p| p.id == recipe.path)
        .ok_or("Assembly path is missing")?;
    let mesh = source(recipe.middle.source).ok_or("Path source model is missing")?;
    if mesh.has_hidden_faces() {
        return Err("Show every source face before building a grouped path object".into());
    }
    if mesh.vertices.len() > MAX_GENERATED_VERTICES
        || mesh.triangles.len() > MAX_GENERATED_TRIANGLES
    {
        return Err("Reduce source detail before building the path object (250,000 vertices / 500,000 triangles)".into());
    }
    let groups = recipe.regions.as_ref().unwrap();
    let faces = groups.faces(mesh)?;
    let bounds = groups.bounds(mesh, recipe.middle, &faces)?;
    let mut parts = Vec::new();
    let mut members = Vec::new();
    for role in 0..3 {
        if cancel() {
            return Err("Operation cancelled".into());
        }
        let (mut part, vertices) = extract(mesh, &faces, role)?;
        // Boundary membership in another source region must not be repeated
        // with Middle. Welding restores shared memberships at the actual joins.
        let roles = [&groups.start, &groups.middle, &groups.end];
        for group in &mut part.vertex_groups {
            if roles
                .iter()
                .enumerate()
                .any(|(r, name)| r != role && **name == group.name)
            {
                group.weights.fill(0.);
            }
        }
        parts.push(part);
        members.push(vertices);
    }
    for i in 0..2 {
        let left: HashSet<_> = members[i].iter().collect();
        if members[i + 1]
            .iter()
            .filter(|v| left.contains(v))
            .take(3)
            .count()
            < 3
        {
            return Err("The source regions must share welded boundary vertices. Weld the source and assign adjoining face regions".into());
        }
    }
    let frames = Frames::new(path, recipe.twist)?;
    let spans = intervals(recipe, &bounds.spans, frames.length, path.closed)?;
    let vertices: usize = spans.iter().map(|&(r, _, _)| parts[r].vertices.len()).sum();
    let triangles: usize = spans
        .iter()
        .map(|&(r, _, _)| parts[r].triangles.len())
        .sum();
    if vertices > MAX_GENERATED_VERTICES || triangles > MAX_GENERATED_TRIANGLES {
        return Err("Path preview exceeds 250,000 vertices / 500,000 triangles. Reduce middle repeats or source detail".into());
    }
    let mut copies = Vec::new();
    let mut sources = Vec::new();
    for (slot, &(role, start, end)) in spans.iter().enumerate() {
        let mut part = parts[role].clone();
        let (lo, hi) = bounds.spans[role];
        for (v, p) in part.vertices.iter_mut().enumerate() {
            if v % 1024 == 0 && cancel() {
                return Err("Operation cancelled".into());
            }
            let mut local = recipe.middle.local(*p);
            if bounds.reversed {
                // Reverse both tangent and side to preserve winding and the authored Up.
                local.0 = -local.0;
                local.1 = -local.1;
            }
            let distance = start + (local.0 - lo) / (hi - lo) * (end - start);
            let frame = frames.at(distance);
            let thickness = recipe.thickness.at(distance / frames.length, path.closed);
            *p = frame.origin
                + frame.vector(Vec3(0., local.1, local.2)) * (recipe.scale * thickness);
        }
        part.validate()?;
        copies.push(GeneratedCopy {
            slot,
            placement: placements[0].1,
            mesh: part,
        });
        sources.push(weld::Section {
            mesh: &parts[role],
            original_vertices: Some(&members[role]),
            reversed: bounds.reversed,
        });
    }
    let tolerance =
        ((bounds.spans[2].1 - bounds.spans[0].0) * recipe.scale * 1e-5).clamp(0.00001, 0.01);
    let mesh = weld::join_parts(&copies, &sources, path.closed, tolerance, cancel)
        .map_err(|error| format!("Path region boundaries: {error}"))?;
    Ok(vec![GeneratedCopy {
        slot: 0,
        placement: placements[0].1,
        mesh,
    }])
}
