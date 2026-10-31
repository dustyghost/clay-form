//! Polygonal Wavefront OBJ adapter; no materials, external files, or executable records.
//! Syntax reference: original Wavefront Appendix B1, mirrored at
//! https://github.com/mokiat/java-data-front/blob/master/documents/OBJ%20Specification.txt
use crate::sculpt::{Mesh, Vec3, VertexGroup, max_triangles, max_vertices};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
};

const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_LINE: usize = 1024 * 1024;
const MAX_CORNERS: usize = 256;
struct Face {
    vertices: Vec<usize>,
    uv: Vec<Option<usize>>,
    groups: Vec<usize>,
    line: usize,
}

pub fn load_obj(path: &str) -> Result<Mesh, String> {
    load_obj_cancellable(path, &|| false)
}
pub(crate) fn load_obj_cancellable(
    path: &str,
    cancelled: &impl Fn() -> bool,
) -> Result<Mesh, String> {
    check_cancel(cancelled)?;
    let metadata = std::fs::metadata(path).map_err(|e| format!("Could not open OBJ: {e}"))?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
        return Err("OBJ must be a regular file no larger than 128 MB".into());
    }
    let file = std::fs::File::open(path).map_err(|e| format!("Could not open OBJ: {e}"))?;
    let mut reader = file.take(MAX_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    let mut chunk = [0; 16384];
    loop {
        check_cancel(cancelled)?;
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if bytes.len() + n > MAX_BYTES {
            return Err("OBJ exceeds 128 MB limit".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "OBJ must be UTF-8 text")?;
    parse(text, cancelled)
}
fn check_cancel(cancelled: &impl Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        Err("OBJ import cancelled".into())
    } else {
        Ok(())
    }
}
fn index(text: &str, count: usize) -> Result<usize, String> {
    let value: i64 = text.parse().map_err(|_| "Invalid OBJ index")?;
    if value == 0 {
        return Err("OBJ indices start at 1; zero is invalid".into());
    }
    let index = if value > 0 {
        value - 1
    } else {
        count as i64 + value
    };
    if index < 0 || index >= max_triangles() as i64 * 3 {
        return Err("OBJ index is out of range".into());
    }
    Ok(index as usize)
}
fn numbers(args: &[&str]) -> Result<Vec<f32>, String> {
    args.iter()
        .map(|s| {
            s.parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| "Coordinates must be finite numbers".into())
        })
        .collect()
}
fn group_id(
    name: &str,
    names: &mut Vec<String>,
    ids: &mut HashMap<String, usize>,
) -> Result<usize, String> {
    if let Some(&id) = ids.get(name) {
        return Ok(id);
    }
    if names.len() >= 64 {
        return Err("OBJ exceeds 64 vertex groups".into());
    }
    let mut base: String = name.chars().filter(|c| !c.is_control()).collect();
    while base.len() > 112 {
        base.pop();
    }
    if base.is_empty() {
        return Err("Empty OBJ group name".into());
    }
    let mut label = base.clone();
    let mut suffix = 2;
    while names.contains(&label) {
        label = format!("{base} ({suffix})");
        suffix += 1;
    }
    names.push(label);
    let id = names.len();
    ids.insert(name.into(), id);
    Ok(id)
}
fn parse(text: &str, cancelled: &impl Fn() -> bool) -> Result<Mesh, String> {
    if text.len() > MAX_BYTES {
        return Err("OBJ exceeds 128 MB limit".into());
    }
    let mut vertices = Vec::new();
    let mut materials = Vec::new();
    let mut has_colors = false;
    let mut faces = Vec::new();
    let (mut textures, mut normals) = (0, 0);
    let mut texcoords = Vec::new();
    let (mut texture_ref, mut normal_ref) = (0, 0);
    let mut names = Vec::new();
    let mut ids = HashMap::new();
    let mut groups = Vec::new();
    let mut triangle_count = 0;
    let mut logical = String::new();
    let mut start = 1;
    // ZBrush can append a C-string terminator after the final OBJ record.
    // Accept only terminal NUL padding, not NULs embedded in the document.
    let text = text.trim_end_matches('\0');
    for (line, physical) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        check_cancel(cancelled)?;
        let physical = physical.split('#').next().unwrap().trim_end();
        if logical.is_empty() {
            start = line + 1;
        }
        if logical.len() + physical.len() > MAX_LINE {
            return Err(format!("OBJ line {start}: statement exceeds 1 MB"));
        }
        let continued = physical.strip_suffix('\\');
        logical.push_str(continued.unwrap_or(physical));
        if continued.is_some() {
            logical.push(' ');
            continue;
        }
        let result = (|| {
            let mut words = logical.split_whitespace();
            let Some(kind) = words.next() else {
                return Ok(());
            };
            match kind {
                "v" | "vt" | "vn" => {
                    let args: Vec<_> = words.take(9).collect();
                    let valid = match kind {
                        "v" => matches!(args.len(), 3 | 4 | 6 | 7),
                        "vt" => (1..=3).contains(&args.len()),
                        _ => args.len() == 3,
                    };
                    if !valid {
                        return Err(format!("Invalid {kind} coordinate count"));
                    }
                    let p = numbers(&args)?;
                    match kind {
                        "v" => {
                            if vertices.len() >= max_vertices() {
                                return Err(format!(
                                    "OBJ exceeds configured vertex limit ({})",
                                    max_vertices()
                                ));
                            }
                            // w is a free-form weight; polygon positions are the literal x/y/z.
                            vertices.push(Vec3(p[0], p[1], p[2]));
                            let mut material = crate::sculpt::Material::default();
                            if p.len() >= 6 {
                                // Conventional v x y z r g b [a], display-encoded RGB.
                                if p[3..6].iter().any(|v| !(0. ..=1.).contains(v)) {
                                    return Err("OBJ vertex colours must be from 0 to 1".into());
                                }
                                if p.len() == 7 && p[6] != 1. {
                                    return Err(
                                        "OBJ vertex opacity is not supported; use opaque colours"
                                            .into(),
                                    );
                                }
                                material.base_color =
                                    [p[3], p[4], p[5]].map(crate::sculpt::material::srgb_to_linear);
                                has_colors = true;
                            }
                            materials.push(material);
                        }
                        "vt" => {
                            textures += 1;
                            texcoords.push([p[0], p.get(1).copied().unwrap_or(0.)]);
                        }
                        _ => normals += 1,
                    }
                    if textures > max_triangles() * 3 || normals > max_triangles() * 3 {
                        return Err("OBJ has too many texture coordinates or normals".into());
                    }
                }
                "f" => {
                    let corners: Vec<_> = words.take(MAX_CORNERS + 1).collect();
                    if !(3..=MAX_CORNERS).contains(&corners.len()) {
                        return Err("Faces need 3–256 corners".into());
                    }
                    triangle_count += corners.len() - 2;
                    if triangle_count > max_triangles() {
                        return Err(format!(
                            "OBJ exceeds configured triangle limit ({}) after triangulation",
                            max_triangles()
                        ));
                    }
                    let mut face = Vec::with_capacity(corners.len());
                    let mut uv = Vec::with_capacity(corners.len());
                    for corner in corners {
                        let parts: Vec<_> = corner.split('/').collect();
                        if parts.len() > 3
                            || parts[0].is_empty()
                            || (parts.len() > 1 && parts.last() == Some(&""))
                        {
                            return Err(
                                "Invalid face corner; use v, v/vt, v//vn, or v/vt/vn".into()
                            );
                        }
                        face.push(index(parts[0], vertices.len())?);
                        if parts.len() > 1 && !parts[1].is_empty() {
                            let i = index(parts[1], textures)?;
                            texture_ref = texture_ref.max(i + 1);
                            uv.push(Some(i));
                        } else {
                            uv.push(None);
                        }
                        if parts.len() == 3 {
                            normal_ref = normal_ref.max(index(parts[2], normals)? + 1);
                        }
                    }
                    if face.iter().collect::<HashSet<_>>().len() != face.len() {
                        return Err("Face repeats a vertex".into());
                    }
                    faces.push(Face {
                        vertices: face,
                        uv,
                        groups: groups.clone(),
                        line: start,
                    });
                }
                "g" => {
                    groups.clear();
                    for name in words {
                        let id = group_id(name, &mut names, &mut ids)?;
                        if !groups.contains(&id) {
                            groups.push(id);
                        }
                    }
                }
                // Geometry-only import: don't open MTLs, alter normals, or split at UV seams.
                "o" | "s" | "mtllib" | "usemtl" | "usemap" | "maplib" | "bevel" | "c_interp"
                | "d_interp" | "lod" => {}
                _ => {
                    return Err(format!(
                        "Unsupported OBJ record '{kind}'; export polygon faces only"
                    ));
                }
            }
            Ok(())
        })();
        result.map_err(|e: String| format!("OBJ line {start}: {e}"))?;
        logical.clear();
    }
    if !logical.is_empty() {
        return Err(format!("OBJ line {start}: unfinished line continuation"));
    }
    if faces.is_empty() {
        return Err("OBJ contains no polygon faces".into());
    }
    if texture_ref > textures || normal_ref > normals {
        return Err("OBJ texture or normal index is out of range".into());
    }
    let mut mesh = Mesh {
        uv: Default::default(),
        creases: Vec::new(),
        mask: vec![0.; vertices.len()],
        materials: if has_colors { materials } else { Vec::new() },
        vertex_groups: names
            .into_iter()
            .map(|name| VertexGroup {
                name,
                visible: true,
                weights: vec![0.; vertices.len()],
            })
            .collect(),
        vertices,
        triangles: Vec::with_capacity(triangle_count),
        quads: Vec::new(),
        polygroups: Vec::with_capacity(triangle_count),
        hidden_faces: Vec::new(),
    };
    let has_uv = faces.iter().any(|f| f.uv.iter().any(Option::is_some));
    for face in faces {
        check_cancel(cancelled)?;
        let triangles = triangulate(&mesh.vertices, &face.vertices, cancelled)
            .map_err(|e| format!("OBJ line {}: {e}", face.line))?;
        for &group in &face.groups {
            for &vertex in &face.vertices {
                mesh.vertex_groups[group - 1].weights[vertex] = 1.;
            }
        }
        mesh.polygroups.extend(std::iter::repeat_n(
            face.groups.first().copied().unwrap_or(0),
            triangles.len(),
        ));
        if has_uv {
            mesh.uv.removed.extend(triangles.iter().map(|t| {
                t.iter()
                    .any(|v| face.uv[face.vertices.iter().position(|i| i == v).unwrap()].is_none())
            }));
            mesh.uv.corners.extend(triangles.iter().map(|t| {
                t.map(|v| {
                    face.uv[face.vertices.iter().position(|&i| i == v).unwrap()]
                        .map(|i| texcoords[i])
                        .unwrap_or([0.; 2])
                })
            }));
        }
        mesh.triangles.extend(triangles);
        if face.vertices.len() == 4 {
            mesh.quads.push(face.vertices.try_into().unwrap());
        }
    }
    if has_uv {
        mesh.uv.topology = mesh.triangles.clone();
    }
    mesh.canonicalize_polygroups();
    mesh.validate()?;
    Ok(mesh)
}

fn triangulate(
    vertices: &[Vec3],
    face: &[usize],
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<[usize; 3]>, String> {
    let points: Vec<[f64; 3]> = face
        .iter()
        .map(|&i| {
            vertices
                .get(i)
                .map(|p| [f64::from(p.0), f64::from(p.1), f64::from(p.2)])
                .ok_or("Vertex index is out of range")
        })
        .collect::<Result<_, _>>()?;
    let origin = points[0];
    let extent = points
        .iter()
        .flat_map(|p| (0..3).map(move |i| (p[i] - origin[i]).abs()))
        .fold(0_f64, f64::max);
    if extent == 0. {
        return Err("Degenerate face".into());
    }
    let points: Vec<_> = points
        .iter()
        .map(|p| std::array::from_fn::<_, 3, _>(|i| (p[i] - origin[i]) / extent))
        .collect();
    let mut normal = [0.; 3];
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        for axis in 0..3 {
            normal[axis] +=
                a[(axis + 1) % 3] * b[(axis + 2) % 3] - a[(axis + 2) % 3] * b[(axis + 1) % 3];
        }
    }
    let axis = (0..3)
        .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
        .unwrap();
    if normal[axis].abs() < 1e-12 {
        return Err("Degenerate or self-intersecting face".into());
    }
    if face.len() > 4
        && points.iter().any(|p| {
            (0..3).map(|i| p[i] * normal[i]).sum::<f64>().abs() > normal[axis].abs() * 1e-5
        })
    {
        return Err("Non-planar n-gon; triangulate it in the source application".into());
    }
    let points: Vec<[f64; 2]> = points
        .iter()
        .map(|p| [p[(axis + 1) % 3], p[(axis + 2) % 3]])
        .collect();
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let between = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
        (0..2).all(|i| p[i] >= a[i].min(b[i]) - 1e-12 && p[i] <= a[i].max(b[i]) + 1e-12)
    };
    for i in 0..points.len() {
        check_cancel(cancelled)?;
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        for j in i + 2..points.len() {
            if i == 0 && j + 1 == points.len() {
                continue;
            }
            let c = points[j];
            let d = points[(j + 1) % points.len()];
            let (ab_c, ab_d, cd_a, cd_b) = (
                cross(a, b, c),
                cross(a, b, d),
                cross(c, d, a),
                cross(c, d, b),
            );
            if (ab_c * ab_d < 0. && cd_a * cd_b < 0.)
                || (ab_c.abs() < 1e-12 && between(a, b, c))
                || (ab_d.abs() < 1e-12 && between(a, b, d))
                || (cd_a.abs() < 1e-12 && between(c, d, a))
                || (cd_b.abs() < 1e-12 && between(c, d, b))
            {
                return Err("Self-intersecting face".into());
            }
        }
    }
    let coordinates: Vec<_> = points.iter().flatten().copied().collect();
    let indices =
        earcutr::earcut(&coordinates, &[], 2).map_err(|_| "Could not triangulate face")?;
    if indices.len() != (face.len() - 2) * 3 {
        return Err(
            "Face has collapsed or collinear corners; triangulate it in the source application"
                .into(),
        );
    }
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let mut t = [t[0], t[1], t[2]];
            let area = cross(points[t[0]], points[t[1]], points[t[2]]);
            if area.abs() < 1e-12 {
                return Err("Degenerate triangle".into());
            }
            if area * normal[axis] < 0. {
                t.swap(1, 2);
            }
            Ok(t.map(|i| face[i]))
        })
        .collect()
}

#[cfg(test)]
mod tests;

/// Emit a quad only when both rendering triangles agree at their shared UV corners.
pub(super) fn export_uv_obj(path: &str, mesh: &Mesh) -> Result<(), String> {
    use std::fmt::Write;
    let mut text = String::from("# Clay Form seam-aware UV mesh\n");
    for v in &mesh.vertices {
        writeln!(text, "v {} {} {}", v.0, v.1, v.2).unwrap();
    }
    for q in mesh.uv.corners.iter().flatten() {
        writeln!(text, "vt {} {}", q[0], q[1]).unwrap();
    }
    let mut corner_ids = HashMap::<(usize, [usize; 3]), usize>::new();
    let key = |mut t: [usize; 3]| {
        t.sort_unstable();
        t
    };
    for (f, t) in mesh.triangles.iter().enumerate() {
        for (c, &v) in t.iter().enumerate() {
            corner_ids.insert((v, key(*t)), f * 3 + c + 1);
        }
    }
    let mut used = HashSet::new();
    for polygon in mesh.polygons() {
        let mut ids = HashMap::new();
        let mut faces = Vec::new();
        let mut coherent = true;
        let mut mapped = None;
        // A polygon has at most four corners; find its triangles by their keys.
        let candidates: Vec<_> = if polygon.vertices.len() == 4 {
            let v = &polygon.vertices;
            vec![
                [v[0], v[1], v[2]],
                [v[0], v[2], v[3]],
                [v[0], v[1], v[3]],
                [v[1], v[2], v[3]],
            ]
        } else {
            vec![polygon.vertices.clone().try_into().unwrap()]
        };
        for t in candidates {
            if let Some(&first) = corner_ids.get(&(t[0], key(t))) {
                let f = (first - 1) / 3;
                faces.push(f);
                if mapped.is_some_and(|old| old != mesh.uv.mapped(f)) {
                    coherent = false;
                }
                mapped = Some(mesh.uv.mapped(f));
                for c in 0..3 {
                    let v = mesh.triangles[f][c];
                    let id = f * 3 + c + 1;
                    if let Some(&old) = ids.get(&v) {
                        let old: usize = old;
                        if mesh.uv.mapped(f)
                            && mesh.uv.corners[(old - 1) / 3][(old - 1) % 3]
                                != mesh.uv.corners[f][c]
                        {
                            coherent = false;
                        }
                    } else {
                        ids.insert(v, id);
                    }
                }
            }
        }
        if !coherent || faces.is_empty() {
            continue;
        }
        if polygon.group > 0 {
            writeln!(text, "g group_{}", polygon.group).unwrap();
        } else {
            writeln!(text, "g").unwrap();
        }
        text.push('f');
        for v in polygon.vertices {
            if mapped == Some(true) {
                write!(text, " {}/{}", v + 1, ids[&v]).unwrap();
            } else {
                write!(text, " {}", v + 1).unwrap();
            }
        }
        text.push('\n');
        used.extend(faces);
    }
    for (f, t) in mesh.triangles.iter().enumerate() {
        if !used.contains(&f) {
            text.push('f');
            for (c, v) in t.iter().enumerate() {
                if mesh.uv.mapped(f) {
                    write!(text, " {}/{}", v + 1, f * 3 + c + 1).unwrap();
                } else {
                    write!(text, " {}", v + 1).unwrap();
                }
            }
            text.push('\n');
        }
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}
