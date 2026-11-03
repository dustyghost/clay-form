//! Remove buried polygons and a narrow collar, then stitch the retained surfaces.
//! This is a local patch union: the seam follows source polygon boundaries.
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct FusionReport {
    pub removed_faces: [usize; 2],
    pub joins: usize,
    pub retained_faces: usize,
    pub retained_triangles: usize,
    pub enclosed_faces: usize,
}

fn cut(
    mesh: &Mesh,
    other: &spatial::Index<'_>,
    contacts: &[bool],
    enclosed: &[bool],
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
        // Only the completed surface can tell whether this entire shell is
        // still enclosed. Carry it through stitching without using it as a cut.
        if enclosed[faces[i][0]] {
            continue;
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
    let removed_count = removed.iter().filter(|&&x| x).count();
    Ok((
        remove_polygons(mesh, &polygons, &faces, &removed, cancel)?,
        removed_count,
    ))
}

// Preserve source vertex indices and face order for material/UV transfer and
// preview overlays. Document commit later compacts unreferenced vertices.
fn remove_polygons(
    mesh: &Mesh,
    polygons: &[super::super::edges::Polygon],
    faces: &[Vec<usize>],
    removed: &[bool],
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let mut remove_triangle = vec![false; mesh.triangles.len()];
    for (i, p) in polygons.iter().enumerate() {
        if removed[i] {
            if !p.visible || p.vertices.iter().any(|&v| mesh.mask[v] > 0.) {
                return Err("Fusion would remove hidden or masked faces. Show and unmask the affected geometry first.".into());
            }
            for &f in &faces[i] {
                remove_triangle[f] = true;
            }
        }
    }
    cancelled(cancel)?;
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
    Ok(out)
}

impl FusionReport {
    /// Preserve the source/patch boundary in triangle order when a shell is culled.
    fn finish_closed(
        mut self,
        mesh: Mesh,
        check_source_crossings: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Mesh, Self), String> {
        let enclosed = spatial::enclosed_triangles(&mesh, false, cancel)?;
        if !enclosed.iter().any(|&inside| inside) {
            return Ok((mesh, self));
        }
        // Intersection fusion already checked its sources. Open ends only
        // checked new connector faces, so verify the shells are disjoint
        // before treating a single sample as proof of containment.
        if check_source_crossings {
            spatial::validate_connector(&mesh, 0, cancel)?;
        }
        let polygons = mesh.polygons();
        let faces = super::super::uv::polygon_triangles(&mesh);
        let removed: Vec<_> = faces.iter().map(|fs| enclosed[fs[0]]).collect();
        for (fs, &remove) in faces.iter().zip(&removed) {
            if remove {
                self.enclosed_faces += 1;
                if fs[0] < self.retained_triangles {
                    self.retained_faces -= 1;
                }
            }
        }
        self.retained_triangles -= enclosed[..self.retained_triangles]
            .iter()
            .filter(|&&x| x)
            .count();
        let out = remove_polygons(&mesh, &polygons, &faces, &removed, cancel)?;
        out.validate_closed()?;
        Ok((out, self))
    }
}

pub(crate) fn join_open_ends(
    mesh: &Mesh,
    first: &[usize],
    second: &[usize],
    options: JoinOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<(Mesh, FusionReport), String> {
    let out = bridge(mesh, first, second, options, cancel)?;
    let report = FusionReport {
        removed_faces: [0; 2],
        joins: 1,
        retained_faces: mesh.polygons().len(),
        retained_triangles: mesh.triangles.len(),
        enclosed_faces: 0,
    };
    if boundaries(&out)?.is_empty() {
        report.finish_closed(out, true, cancel)
    } else {
        Ok((out, report))
    }
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
    let prepare = |mesh: &Mesh| -> Result<Vec<bool>, String> {
        cancelled(cancel)?;
        validate_surface(mesh)?;
        mesh.validate_closed().map_err(|_| {
            "Intersection fusion requires closed solids; use Join open ends for holes".to_string()
        })?;
        spatial::validate_connector(mesh, 0, cancel)?;
        spatial::enclosed_triangles(mesh, true, cancel)
    };
    let enclosed_a = prepare(a)?;
    let enclosed_b = prepare(b)?;
    // Interior input shells must not punch extra cut loops or change parity
    // inside the other operand. Keep their original face IDs for the overlay.
    let exterior = |mask: &[bool]| (0..mask.len()).filter(|&i| !mask[i]).collect();
    let ia = spatial::Index::from_faces(a, exterior(&enclosed_a), cancel)?;
    let ib = spatial::Index::from_faces(b, exterior(&enclosed_b), cancel)?;
    let mut ca = vec![false; a.triangles.len()];
    let mut cb = vec![false; b.triangles.len()];
    for (i, t) in a.triangles.iter().enumerate() {
        if i % 128 == 0 {
            cancelled(cancel)?;
        }
        if enclosed_a[i] {
            continue;
        }
        ib.contacts(t.map(|v| a.vertices[v]), |j| {
            ca[i] = true;
            cb[j] = true;
        });
    }
    if !ca.iter().any(|&x| x) {
        return Err("The surfaces do not cross. Move them into overlap, or use Join open ends for separated holes".into());
    }
    let (a, na) = cut(a, &ib, &ca, &enclosed_a, collar, cancel)?;
    let (b, nb) = cut(b, &ia, &cb, &enclosed_b, collar, cancel)?;
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
    FusionReport {
        removed_faces: [na, nb],
        joins: pairs.len(),
        retained_faces,
        retained_triangles,
        enclosed_faces: 0,
    }
    .finish_closed(out, false, cancel)
}
