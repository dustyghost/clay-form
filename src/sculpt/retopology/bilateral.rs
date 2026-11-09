//! Reconstruct one inset half and bridge its reflected boundary with quads.
use super::{Mesh, Vec3, check, native, solver_positions};
use crate::sculpt::{MirrorOptions, SlicePlane, projection::Surface};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub(crate) enum SymmetrySource {
    #[default]
    Original,
    Positive,
    Negative,
}

pub(super) fn reflect(p: Vec3, axis: Vec3) -> Vec3 {
    p - axis * (2. * p.dot(axis))
}

pub(super) fn reflect_point(p: Vec3, axis: Vec3, offset: f32) -> Vec3 {
    p - axis * (2. * (p.dot(axis) - offset))
}

pub(super) fn mirrored_reference(
    source: &Mesh,
    options: MirrorOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let axis = options.axis.vector();
    let extent = source
        .vertices
        .iter()
        .map(|p| (p.dot(axis) - options.offset).abs())
        .fold(0., f32::max);
    let mut input = source.clone();
    snap_to_plane(&mut input, axis, options.offset, extent * 0.000005, cancel)?;
    input.mirror_half(options, &cancel)
}

fn snap_to_plane(
    mesh: &mut Mesh,
    axis: Vec3,
    offset: f32,
    tolerance: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    for (i, point) in mesh.vertices.iter_mut().enumerate() {
        if i % 1024 == 0 {
            check(cancel)?;
        }
        let distance = point.dot(axis) - offset;
        if distance.abs() <= tolerance {
            *point = *point - axis * distance;
        }
    }
    Ok(())
}

pub(super) fn reconstruct(
    reference: &Mesh,
    settings: &clay_retopology::Options,
    axis: Vec3,
    options: super::Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(clay_retopology::Stage),
) -> Result<clay_retopology::Layout, String> {
    reference.validate_closed().map_err(|_| {
        "Symmetric retopology requires a closed manifold source. Use Symmetry Off for open meshes."
    })?;
    let offset = options.symmetry_center.dot(axis);
    let spacing = target_spacing(reference, settings.target_quads);
    let projection = Surface::new(reference, &cancel)?;
    let mut work = 50_000_000;
    let tolerance = spacing * 0.25;
    // Surface distances allow different triangulations across the plane.
    for (i, p) in reference
        .vertices
        .iter()
        .copied()
        .chain(reference.triangles.iter().map(|t| {
            let [a, b, c] = t.map(|i| reference.vertices[i]);
            (a + b + c) * (1. / 3.)
        }))
        .enumerate()
    {
        if i % 256 == 0 {
            check(cancel)?;
        }
        if projection
            .nearest(reflect_point(p, axis, offset), tolerance, None, &mut work)?
            .is_none()
        {
            return Err("The source differs across this plane. Choose Keep + and mirror, Keep − and mirror, or set symmetry to Off.".into());
        }
    }

    // The plane goes through the bridge faces, so the solved half ends a
    // half-cell away. Snap near-plane source points before slicing to avoid
    // leaving tiny clipped triangles in the half's boundary.
    let inset = offset + spacing * 0.5;
    let mut input = reference.clone();
    snap_to_plane(&mut input, axis, inset, spacing * 0.0004, cancel)?;
    let (split, sides) = input.split_plane(
        SlicePlane {
            origin: axis * inset,
            normal: axis,
        },
        &cancel,
    )?;
    let half = split.slice_subset(&sides, true);
    if half.triangles.is_empty() {
        return Err("The selected axis leaves no surface on the retained side".into());
    }
    let half_source = clay_retopology::Surface {
        positions: solver_positions(&half.vertices)?,
        triangles: half.triangles.clone(),
    };
    let mut half_settings = settings.clone();
    half_settings.target_quads = (settings.target_quads / 2).max(4);
    half_settings.detail = half
        .retopology_detail()
        .into_iter()
        .map(|w| f64::from(w * options.density))
        .collect();
    let normal = native(axis);
    let offset = f64::from(offset);
    for guide in &mut half_settings.guides {
        if guide.position.dot(normal) < offset {
            guide.position -= normal * (2. * (guide.position.dot(normal) - offset));
            guide.normal -= normal * (2. * guide.normal.dot(normal));
            guide.tangent -= normal * (2. * guide.tangent.dot(normal));
        }
    }

    let mut result = clay_retopology::reconstruct_regularized_with_progress(
        &half_source,
        &half_settings,
        cancel,
        progress,
    )?;
    check(cancel)?;
    let count = result.positions.len();
    if result.positions.iter().any(|p| p.dot(normal) <= offset) {
        return Err("Symmetric retopology crossed the mirror plane before stitching".into());
    }
    let bridge = bridge_faces(&result.quads, count)?;
    let mut reflected = Vec::with_capacity(count);
    for (i, &p) in result.positions.iter().enumerate() {
        if i % 256 == 0 {
            check(cancel)?;
        }
        let point = p - normal * (2. * (p.dot(normal) - offset));
        let query = Vec3(point.x as f32, point.y as f32, point.z as f32);
        let (_, fitted, _) = projection
            .nearest(query, tolerance, None, &mut work)?
            .ok_or("Could not fit the mirrored quad layout to the other side")?;
        if f64::from(fitted.dot(axis)) >= offset {
            return Err("Fitting the mirrored layout crossed the symmetry plane".into());
        }
        reflected.push(native(fitted));
    }
    append_reflection(&mut result, reflected, bridge);
    // The caller validates the full stitched layout, choosing render diagonals
    // for the actual app coordinates before transferring any attributes.
    Ok(result)
}

fn target_spacing(reference: &Mesh, target_quads: usize) -> f32 {
    let area = reference
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| native(reference.vertices[i]));
            (b - a).cross(c - a).length() * 0.5
        })
        .sum::<f64>();
    (area / target_quads as f64).sqrt() as f32
}

fn bridge_faces(quads: &[[usize; 4]], count: usize) -> Result<Vec<[usize; 4]>, String> {
    let mut edges = BTreeMap::new();
    for face in quads {
        for i in 0..4 {
            let (a, b) = (face[i], face[(i + 1) % 4]);
            edges
                .entry([a.min(b), a.max(b)])
                .and_modify(|boundary| *boundary = None)
                .or_insert(Some([a, b]));
        }
    }
    let bridge: Vec<_> = edges
        .into_values()
        .flatten()
        .map(|[a, b]| [b, a, a + count, b + count])
        .collect();
    if bridge.is_empty() {
        return Err(
            "Symmetric retopology needs a boundary to bridge across the mirror plane".into(),
        );
    }
    Ok(bridge)
}

fn append_reflection(
    result: &mut clay_retopology::Layout,
    reflected: Vec<clay_retopology::Vec3>,
    bridge: Vec<[usize; 4]>,
) {
    let count = result.positions.len();
    result.positions.extend(reflected);
    result.quads.extend_from_within(..);
    let half_faces = result.quads.len() / 2;
    for face in &mut result.quads[half_faces..] {
        face.reverse();
        *face = face.map(|i| i + count);
    }
    result.quads.extend(bridge);
}

pub(super) fn mirror_layout(
    source: &Mesh,
    reference: &Mesh,
    settings: &clay_retopology::Options,
    axis: Vec3,
    options: super::Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(clay_retopology::Stage),
) -> Result<clay_retopology::Layout, String> {
    source.validate_closed().map_err(|_| {
        "Symmetric retopology requires a closed manifold source. Use Symmetry Off for open meshes."
    })?;
    let direction = if options.symmetry_source == SymmetrySource::Positive {
        axis
    } else {
        axis * -1.
    };
    let input = clay_retopology::Surface {
        positions: solver_positions(&source.vertices)?,
        triangles: source.triangles.clone(),
    };
    let result =
        clay_retopology::reconstruct_regularized_with_progress(&input, settings, cancel, progress)?;
    progress(clay_retopology::Stage::SurfaceFit);
    bridge_layout(
        result,
        reference,
        direction,
        options.symmetry_center.dot(direction),
        settings.target_quads,
        options.symmetry_centreline,
        cancel,
    )
}

pub(super) fn bridge_layout(
    mut result: clay_retopology::Layout,
    reference: &Mesh,
    direction: Vec3,
    offset: f32,
    target_quads: usize,
    centreline: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<clay_retopology::Layout, String> {
    check(cancel)?;
    let normal = native(direction);
    let offset = f64::from(offset);
    // Keep complete quads from the chosen side of the successful full solve.
    // Cutting individual quads here would introduce standalone triangles.
    for p in &mut result.positions {
        *p = native(Vec3(p.x as f32, p.y as f32, p.z as f32));
    }
    result
        .quads
        .retain(|q| q.iter().all(|&i| result.positions[i].dot(normal) > offset));
    let used: BTreeSet<_> = result.quads.iter().flatten().copied().collect();
    let mut indices = vec![usize::MAX; result.positions.len()];
    result.positions = used
        .into_iter()
        .enumerate()
        .map(|(new, old)| {
            indices[old] = new;
            result.positions[old]
        })
        .collect();
    for q in &mut result.quads {
        *q = q.map(|i| indices[i]);
    }
    let count = result.positions.len();
    let bridge = bridge_faces(&result.quads, count)?;
    let reflected = result
        .positions
        .iter()
        .map(|p| *p - normal * (2. * (p.dot(normal) - offset)))
        .collect();
    let bridge_start = result.quads.len() * 2;
    append_reflection(&mut result, reflected, bridge);
    repair_strip(
        reference,
        direction,
        offset,
        target_quads,
        count,
        bridge_start,
        &mut result,
        cancel,
    )?;
    if centreline {
        space_centreline_strip(
            reference,
            direction,
            offset,
            target_quads,
            count,
            bridge_start,
            &mut result,
            cancel,
        )?;
        split_centreline(&mut result, direction, offset as f32, cancel)?;
    }
    Ok(result)
}

// Work on the retained side and reflect every accepted move. Only the strip
// and nearby faces can move, so the chosen hand and its guide flow stay intact.
fn repair_strip(
    reference: &Mesh,
    direction: Vec3,
    offset: f64,
    target: usize,
    count: usize,
    bridge_start: usize,
    result: &mut clay_retopology::Layout,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let axis = native(direction);
    let spacing = f64::from(target_spacing(reference, target));
    let surface = Surface::new(reference, &cancel)?;
    let mut budget = 50_000_000;
    let mut neighbours = vec![BTreeSet::new(); count];
    let mut perimeter = BTreeSet::new();
    for face in &result.quads {
        for i in 0..4 {
            let (a, b) = (face[i], face[(i + 1) % 4]);
            perimeter.insert([a.min(b), a.max(b)]);
            let (a, b) = (a % count, b % count);
            if a != b {
                neighbours[a].insert(b);
                neighbours[b].insert(a);
            }
        }
    }
    let seam: BTreeSet<_> = result.quads[bridge_start..]
        .iter()
        .flatten()
        .map(|i| i % count)
        .collect();
    let mut band = seam.clone();
    for _ in 0..3 {
        let ring: Vec<_> = band
            .iter()
            .flat_map(|&i| neighbours[i].iter().copied())
            .collect();
        band.extend(ring);
    }
    let nearby: Vec<_> = result.quads[..bridge_start / 2]
        .iter()
        .chain(&result.quads[bridge_start..])
        .filter(|q| q.iter().any(|i| band.contains(&(i % count))))
        .copied()
        .collect();
    let base = result.positions[..count].to_vec();
    let mut best = None;
    for _ in 0..=8 {
        check(cancel)?;
        let mut folded = 0;
        let mut active = BTreeSet::new();
        for &[a, b, c, d] in &nearby {
            let mut best_diagonal = -1_f64;
            for ([u, v], tris) in [
                ([a, c], [[a, b, c], [a, c, d]]),
                ([b, d], [[a, b, d], [b, c, d]]),
            ] {
                if perimeter.contains(&[u.min(v), u.max(v)]) {
                    continue;
                }
                let mut alignment = 1_f64;
                for t in tris {
                    let [p, q, r] = t.map(|i| result.positions[i]);
                    let normal = (q - p).cross(r - p).normalize_or_zero();
                    let (_, source_normal) =
                        strip_projection(&surface, reference, (p + q + r) / 3., &mut budget)?;
                    alignment = alignment.min(normal.dot(source_normal));
                }
                best_diagonal = best_diagonal.max(alignment);
            }
            if best_diagonal <= 0. {
                folded += 1;
            }
            if best_diagonal < 0.1 {
                active.extend([a, b, c, d].map(|i| i % count));
            }
        }
        if folded == 0 {
            return Ok(());
        }
        if best.as_ref().is_none_or(|(n, _)| folded < *n) {
            best = Some((folded, result.positions.clone()));
        }
        let adjacent: Vec<_> = active
            .iter()
            .flat_map(|&i| neighbours[i].iter().copied())
            .collect();
        active.extend(adjacent);
        let old = result.positions[..count].to_vec();
        for i in active.intersection(&band).copied() {
            check(cancel)?;
            let average = neighbours[i]
                .iter()
                .map(|&j| old[j])
                .sum::<clay_retopology::Vec3>()
                / neighbours[i].len() as f64;
            let mut proposed = old[i] * 0.6 + average * 0.4;
            if seam.contains(&i) {
                proposed += axis * (offset + spacing * 0.26 - proposed.dot(axis)).max(0.);
            }
            let (point, normal) = strip_projection(&surface, reference, proposed, &mut budget)?;
            let (_, initial_normal) = strip_projection(&surface, reference, base[i], &mut budget)?;
            if point.dot(axis) > offset + spacing * 0.003
                && normal.dot(initial_normal) > 0.
                && point.distance(base[i]) < spacing * 0.95
            {
                result.positions[i] = point;
                result.positions[i + count] = point - axis * (2. * (point.dot(axis) - offset));
            }
        }
    }
    if let Some((_, positions)) = best {
        result.positions = positions;
    }
    Ok(())
}

// The existing bridge may be valid even when a transverse edge is tiny.
// Open up those pairs before splitting them into two shorter edges.
fn space_centreline_strip(
    reference: &Mesh,
    direction: Vec3,
    offset: f64,
    target: usize,
    count: usize,
    bridge_start: usize,
    result: &mut clay_retopology::Layout,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let spacing = f64::from(target_spacing(reference, target));
    let axis = native(direction);
    let surface = Surface::new(reference, &cancel)?;
    let mut budget = 50_000_000;
    let seam: BTreeSet<_> = result.quads[bridge_start..]
        .iter()
        .flatten()
        .copied()
        .filter(|&i| i < count)
        .collect();
    for i in seam {
        check(cancel)?;
        let p = result.positions[i];
        let distance = p.dot(axis) - offset;
        if distance >= spacing * 0.28 {
            continue;
        }
        let (point, normal) = strip_projection(
            &surface,
            reference,
            p + axis * (spacing * 0.28 - distance),
            &mut budget,
        )?;
        let (_, before) = strip_projection(&surface, reference, p, &mut budget)?;
        if point.dot(axis) - offset >= spacing * 0.20 && normal.dot(before) > 0.5 {
            result.positions[i] = point;
            result.positions[i + count] = point - axis * (2. * (point.dot(axis) - offset));
        }
    }
    Ok(())
}

pub(super) fn split_centreline(
    layout: &mut clay_retopology::Layout,
    axis: Vec3,
    offset: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let axis = native(axis);
    let offset = f64::from(offset);
    let mut quads = Vec::with_capacity(layout.quads.len());
    let mut intersections = BTreeMap::new();
    for (i, &face) in layout.quads.iter().enumerate() {
        if i % 256 == 0 {
            check(cancel)?;
        }
        let signed = face.map(|i| layout.positions[i].dot(axis) - offset);
        if !signed.iter().any(|&v| v > 0.) || !signed.iter().any(|&v| v < 0.) {
            quads.push(face);
            continue;
        }
        let start = (0..4)
            .find(|&k| {
                signed[k] * signed[(k + 1) % 4] > 0.
                    && signed[(k + 2) % 4] * signed[(k + 3) % 4] > 0.
                    && signed[k] * signed[(k + 2) % 4] < 0.
            })
            .ok_or("Could not split the bridge into quads at the centreline")?;
        let [a, b, c, d] = std::array::from_fn(|k| face[(start + k) % 4]);
        let mut midpoint = |i: usize, j: usize| {
            *intersections
                .entry([i.min(j), i.max(j)])
                .or_insert_with(|| {
                    let p = layout.positions[i];
                    let q = layout.positions[j];
                    let da = p.dot(axis) - offset;
                    let db = q.dot(axis) - offset;
                    let mid = p + (q - p) * (da / (da - db));
                    let index = layout.positions.len();
                    layout.positions.push(mid - axis * (mid.dot(axis) - offset));
                    index
                })
        };
        let bc = midpoint(b, c);
        let da = midpoint(d, a);
        quads.extend([[a, b, bc, da], [da, bc, c, d]]);
    }
    if intersections.is_empty() {
        return Err("There are no cross-plane quads to split at the centreline".into());
    }
    layout.quads = quads;
    Ok(())
}

fn strip_projection(
    surface: &Surface<'_>,
    reference: &Mesh,
    p: clay_retopology::Vec3,
    budget: &mut usize,
) -> Result<(clay_retopology::Vec3, clay_retopology::Vec3), String> {
    let (face, point, _) = surface
        .nearest(
            Vec3(p.x as f32, p.y as f32, p.z as f32),
            f32::MAX.sqrt(),
            None,
            budget,
        )?
        .ok_or("Could not fit the cross-plane quad strip")?;
    let [a, b, c] = reference.triangles[face].map(|i| native(reference.vertices[i]));
    Ok((native(point), (b - a).cross(c - a).normalize_or_zero()))
}
