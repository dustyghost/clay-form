//! Remove buried polygons and a narrow collar, then stitch the retained surfaces.
//! This is a local patch union: the seam follows source polygon boundaries.
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct FusionReport {
    pub removed_faces: [usize; 2],
    pub joins: usize,
    pub retained_faces: usize,
    pub retained_triangles: usize,
}

fn cut(
    mesh: &Mesh,
    other: &spatial::Index<'_>,
    contacts: &[bool],
    collar: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(Mesh, usize), String> {
    let polygons = mesh.polygons();
    let faces = super::super::uv::polygon_triangles(mesh);
    let mut removed = vec![false; polygons.len()];
    for (i, p) in polygons.iter().enumerate() {
        if i % 256 == 0 {
            cancelled(cancel)?;
        }
        removed[i] = if faces[i].iter().any(|&f| contacts[f]) {
            true
        } else {
            let center = p
                .vertices
                .iter()
                .fold(Vec3::default(), |sum, &v| sum + mesh.vertices[v])
                * (1. / p.vertices.len() as f32);
            other.inside(center)?
        };
    }
    let mut incident = vec![Vec::new(); mesh.vertices.len()];
    for (i, p) in polygons.iter().enumerate() {
        for &v in &p.vertices {
            incident[v].push(i);
        }
    }
    for _ in 0..collar {
        cancelled(cancel)?;
        let old = removed.clone();
        for (i, p) in polygons.iter().enumerate() {
            if old[i] {
                for &v in &p.vertices {
                    for &f in &incident[v] {
                        removed[f] = true;
                    }
                }
            }
        }
    }
    let mut remove_triangle = vec![false; mesh.triangles.len()];
    for (i, p) in polygons.iter().enumerate() {
        if removed[i] {
            if !p.visible || p.vertices.iter().any(|&v| mesh.mask[v] > 0.) {
                return Err("The fusion would remove hidden or masked faces; show and unmask the joining region first".into());
            }
            for &f in &faces[i] {
                remove_triangle[f] = true;
            }
        }
    }
    let removed_count = removed.iter().filter(|&&x| x).count();
    let mut out = mesh.clone();
    out.triangles.clear();
    out.polygroups.clear();
    out.hidden_faces.clear();
    out.quads = polygons
        .iter()
        .enumerate()
        .filter(|(i, p)| !removed[*i] && p.vertices.len() == 4)
        .map(|(_, p)| [p.vertices[0], p.vertices[1], p.vertices[2], p.vertices[3]])
        .collect();
    let uv = mesh.uv.current(mesh);
    if uv {
        out.uv.corners.clear();
        out.uv.masks.clear();
        out.uv.removed.clear();
    }
    for (i, &t) in mesh.triangles.iter().enumerate() {
        if !remove_triangle[i] {
            out.triangles.push(t);
            out.polygroups
                .push(mesh.polygroups.get(i).copied().unwrap_or(0));
            out.hidden_faces
                .push(mesh.hidden_faces.get(i).copied().unwrap_or(false));
            if uv {
                out.uv.corners.push(mesh.uv.corners[i]);
                out.uv
                    .masks
                    .push(mesh.uv.masks.get(i).copied().unwrap_or([0.; 3]));
                out.uv.removed.push(!mesh.uv.mapped(i));
            }
        }
    }
    let edges: BTreeSet<_> = out
        .triangles
        .iter()
        .flat_map(|t| (0..3).map(move |i| edge(t[i], t[(i + 1) % 3])))
        .collect();
    out.creases.retain(|e| edges.contains(e));
    out.uv.seams.retain(|e| edges.contains(e));
    if uv {
        out.uv.topology = out.triangles.clone();
    }
    validate_surface(&out)?;
    Ok((out, removed_count))
}

pub(crate) fn fuse(
    a: &Mesh,
    b: &Mesh,
    collar: usize,
    options: JoinOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<(Mesh, FusionReport), String> {
    if collar > 8 {
        return Err("Choose at most eight local collar rings".into());
    }
    options.validate()?;
    check_uv(a)?;
    check_uv(b)?;
    for mesh in [a, b] {
        cancelled(cancel)?;
        validate_surface(mesh)?;
        mesh.validate_closed().map_err(|_| {
            "Intersection fusion requires closed solids; use Join open ends for holes".to_string()
        })?;
        spatial::require_outward_shells(mesh, cancel)?;
        spatial::validate_connector(mesh, 0, cancel)?;
    }
    let ia = spatial::Index::new(a, cancel)?;
    let ib = spatial::Index::new(b, cancel)?;
    let mut ca = vec![false; a.triangles.len()];
    let mut cb = vec![false; b.triangles.len()];
    for (i, t) in a.triangles.iter().enumerate() {
        if i % 128 == 0 {
            cancelled(cancel)?;
        }
        ib.contacts(t.map(|v| a.vertices[v]), |j| {
            ca[i] = true;
            cb[j] = true;
        });
    }
    if !ca.iter().any(|&x| x) {
        return Err("The surfaces do not cross. Move them into overlap, or use Join open ends for separated holes".into());
    }
    let (a, na) = cut(a, &ib, &ca, collar, cancel)?;
    let (b, nb) = cut(b, &ia, &cb, collar, cancel)?;
    let la = boundaries(&a)?;
    let lb = boundaries(&b)?;
    if la.is_empty() || la.len() != lb.len() || la.len() > 16 {
        return Err(
            "The cut does not form matching simple joining loops; adjust overlap or collar rings"
                .into(),
        );
    }
    // Pair only mutually closest loops. Ambiguous many-to-one joins need an
    // explicit choice rather than connecting unrelated parts by accident.
    let distance = |a: &Boundary, b: &Boundary| (a.center - b.center).len();
    let mut pairs = Vec::new();
    let mut used = BTreeSet::new();
    for (i, first) in la.iter().enumerate() {
        let j = (0..lb.len())
            .min_by(|&a, &b| distance(first, &lb[a]).total_cmp(&distance(first, &lb[b])))
            .unwrap();
        let reverse = (0..la.len())
            .min_by(|&a, &b| distance(&la[a], &lb[j]).total_cmp(&distance(&la[b], &lb[j])))
            .unwrap();
        if reverse != i || !used.insert(j) {
            return Err(
                "Several intersections have ambiguous partners; fuse one region at a time".into(),
            );
        }
        pairs.push((
            first.vertices.clone(),
            lb[j]
                .vertices
                .iter()
                .map(|v| v + a.vertices.len())
                .collect::<Vec<_>>(),
        ));
    }
    let mut out = merge(&a, &b)?;
    let retained_faces = out.polygons().len();
    let retained_triangles = out.triangles.len();
    for (first, second) in &pairs {
        out = bridge(&out, first, second, options, cancel)
            .map_err(|e| format!("{e}. Try adjusting the collar rings or source overlap."))?;
    }
    out.validate_closed()?;
    Ok((
        out,
        FusionReport {
            removed_faces: [na, nb],
            joins: pairs.len(),
            retained_faces,
            retained_triangles,
        },
    ))
}
