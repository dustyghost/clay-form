//! Solid Booleans on source triangles. Corner provenance maps cut pieces back
//! to their original attributes and restores quads whose two triangles survive.
use super::*;
use crate::sculpt::{BooleanRole, boolean::Operand};
use boolmesh::{
    Vec4,
    prelude::{Manifold, OpType, compute_boolean},
};
use std::collections::HashMap;

pub(crate) fn evaluate(operands: &[Operand], cancel: &dyn Fn() -> bool) -> Result<Mesh, String> {
    let Some((base, _)) = operands.split_first() else {
        return Err("Choose an Add operand as the base".into());
    };
    if base.role != BooleanRole::Add {
        return Err("Set the bottom visible Boolean operand to Add to form a base".into());
    }
    let mut result = base.mesh.clone();
    for (i, operand) in operands.iter().enumerate() {
        cancelled(cancel)?;
        let mesh = &operand.mesh;
        validate_surface(mesh)
            .and_then(|()| mesh.validate_closed())
            .and_then(|()| check_uv(mesh))
            .and_then(|()| spatial::validate_connector(mesh, 0, cancel))
            .and_then(|()| spatial::enclosed_triangles(mesh, true, cancel).map(|_| ()))
            .map_err(|e| format!("{}: {e}", operand.name))?;
        if i == 0 {
            continue;
        }
        let op = match operand.role {
            BooleanRole::Add => OpType::Add,
            BooleanRole::Subtract => OpType::Subtract,
            BooleanRole::Intersect => OpType::Intersect,
            BooleanRole::Regular => {
                return Err("Regular submodels must stay outside the Boolean stack".into());
            }
        };
        if result.triangles.is_empty() {
            if operand.role == BooleanRole::Add {
                result = mesh.clone();
            } else {
                check_protection(mesh, &result)?;
            }
            continue;
        }
        result =
            apply_pair(&result, mesh, op, cancel).map_err(|e| format!("{}: {e}", operand.name))?;
    }
    Ok(result)
}

fn manifold(mesh: &Mesh, offset: usize) -> Result<Manifold<Vec4>, String> {
    let pos: Vec<_> = mesh
        .vertices
        .iter()
        .flat_map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
        .collect();
    let corners = mesh.triangles.as_flattened();
    let mut tags = Vec::with_capacity(corners.len());
    for f in 0..mesh.triangles.len() {
        let id = (offset + f) as f64;
        tags.extend([
            Vec4::new(1., 0., 0., id),
            Vec4::new(0., 1., 0., id),
            Vec4::new(0., 0., 1., id),
        ]);
    }
    let mut solid = Manifold::new_with_values(&pos, corners, &tags)?;
    // Cut positions are stored as f32 in Clay. Simplify intersections that
    // would otherwise become collapsed triangles when converted back to f32.
    solid.eps = solid.eps.max(solid.bb.scale() * f64::from(f32::EPSILON));
    solid.tol = solid.tol.max(solid.eps);
    // Distinguish adjacent source triangles even on flat surfaces: a planar
    // grid, UV seam or painted face must not dissolve into a larger polygon.
    for (f, group) in solid.coplanar.iter_mut().enumerate() {
        *group = f as i32;
    }
    Ok(solid)
}

fn apply_pair(a: &Mesh, b: &Mesh, op: OpType, cancel: &dyn Fn() -> bool) -> Result<Mesh, String> {
    let source = merge(a, b).map_err(|e| format!("Preparing source attributes: {e}"))?;
    let left = manifold(a, 0)?;
    let right = manifold(b, a.triangles.len())?;
    cancelled(cancel)?;
    let solid = match compute_boolean(&left, &right, op) {
        Ok(solid) => solid,
        // In boolmesh 0.2.2 a valid empty result has no position matrix.
        Err(reason) if reason == "empty pos matrix" => {
            let empty = Mesh::default();
            check_protection(&source, &empty)?;
            return Ok(empty);
        }
        Err(reason) => return Err(reason),
    };
    cancelled(cancel)?;
    if solid.nf > max_triangles() || solid.nv > max_vertices() {
        return Err("Surface Boolean exceeds the mesh capacity".into());
    }
    let seams = a
        .shading_edges()
        .iter()
        .copied()
        .chain(
            b.shading_edges()
                .iter()
                .map(|&e| e.map(|v| v + a.vertices.len())),
        )
        .collect();
    let mesh = reconstruct(&source, &solid, a.triangles.len(), &seams, cancel)?;
    check_protection(&source, &mesh)?;
    Ok(mesh)
}

fn check_protection(source: &Mesh, result: &Mesh) -> Result<(), String> {
    if source.has_hidden_faces() || source.mask.iter().any(|&weight| weight > 0.) {
        source.check_model_protection(result, true).map_err(|_| {
            "The cut would change a masked or hidden face. Clear its mask or show the face first.".to_string()
        })?;
    }
    Ok(())
}

fn weights(tag: Vec4) -> Result<[f32; 3], String> {
    let w = [tag.x, tag.y, tag.z];
    if !tag.is_finite()
        || w.iter().any(|&v| !(-1e-5..=1. + 1e-5).contains(&v))
        || (w.iter().sum::<f64>() - 1.).abs() > 1e-5
    {
        return Err("The surface cut could not interpolate a source corner".into());
    }
    let w = w.map(|v| v.clamp(0., 1.) as f32);
    let sum = w.iter().sum::<f32>();
    Ok(w.map(|v| v / sum))
}

fn reconstruct(
    source: &Mesh,
    solid: &Manifold<Vec4>,
    right_start: usize,
    shading: &BTreeSet<[usize; 2]>,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let mut out = Mesh::default();
    if solid.nf == 0 {
        return Ok(out);
    }
    let uv = source.uv.current(source);
    if uv {
        out.uv.textures.clone_from(&source.uv.textures);
    }
    out.vertex_groups = source
        .vertex_groups
        .iter()
        .map(|g| crate::sculpt::VertexGroup {
            purpose: g.purpose,
            name: g.name.clone(),
            visible: g.visible,
            weights: Vec::new(),
        })
        .collect();
    let mut vertices = vec![usize::MAX; solid.ps.len()];
    let mut intact = vec![None; source.triangles.len()];
    let mut crease_edges = BTreeSet::new();
    let mut uv_edges = BTreeSet::new();
    let mut shade_edges = BTreeSet::new();
    let mut owners = HashMap::new();
    for f in 0..solid.nf {
        if f % 256 == 0 {
            cancelled(cancel)?;
        }
        let tags: [Vec4; 3] = std::array::from_fn(|k| solid.vals[f * 3 + k]);
        let id = tags[0].w.round();
        if id < 0.
            || id >= source.triangles.len() as f64
            || tags.iter().any(|tag| (tag.w - id).abs() > 1e-5)
        {
            return Err("The surface cut lost its source face correspondence".into());
        }
        let face = id as usize;
        let tri = source.triangles[face];
        let weights = [weights(tags[0])?, weights(tags[1])?, weights(tags[2])?];
        let mut corners = [0; 3];
        let mut original = [None; 3];
        for k in 0..3 {
            let v = solid.hs[3 * f + k].tail;
            let p = solid.ps[v];
            let pos = Vec3(p.x as f32, p.y as f32, p.z as f32);
            original[k] = tri.iter().position(|&i| source.vertices[i] == pos);
            if vertices[v] == usize::MAX {
                let blend = std::array::from_fn::<_, 3, _>(|j| (tri[j], weights[k][j]));
                vertices[v] = out.append_blend(source, &blend, pos);
            }
            corners[k] = vertices[v];
        }
        if let [Some(i), Some(j), Some(k)] = original {
            let mut mapped = [0; 3];
            for (index, v) in [i, j, k].into_iter().zip(corners) {
                mapped[index] = v;
            }
            intact[face] = Some((mapped, j != (i + 1) % 3));
        }
        out.triangles.push(corners);
        out.polygroups
            .push(source.polygroups.get(face).copied().unwrap_or(0));
        out.hidden_faces
            .push(source.hidden_faces.get(face).copied().unwrap_or(false));
        if uv {
            let coords = source.uv.corners[face];
            let masks = source.uv.masks.get(face).copied().unwrap_or_default();
            out.uv.corners.push(
                weights.map(|w| {
                    std::array::from_fn(|axis| (0..3).map(|j| w[j] * coords[j][axis]).sum())
                }),
            );
            out.uv
                .masks
                .push(weights.map(|w| (0..3).map(|j| w[j] * masks[j]).sum::<f32>().clamp(0., 1.)));
            out.uv.removed.push(!source.uv.mapped(face));
        }
        for k in 0..3 {
            let next = (k + 1) % 3;
            let e = edge(corners[k], corners[next]);
            let owner = face >= right_start;
            if owners.insert(e, owner).is_some_and(|other| other != owner) {
                shade_edges.insert(e);
            }
            for j in 0..3 {
                if weights[k][j] <= 1e-8 && weights[next][j] <= 1e-8 {
                    let old = edge(tri[(j + 1) % 3], tri[(j + 2) % 3]);
                    if source.creases.binary_search(&old).is_ok() {
                        crease_edges.insert(e);
                    }
                    if source.uv.seams.binary_search(&old).is_ok() {
                        uv_edges.insert(e);
                    }
                    if shading.contains(&old) {
                        shade_edges.insert(e);
                    }
                }
            }
        }
    }
    for (quad, faces) in source
        .quads
        .iter()
        .zip(crate::sculpt::uv::polygon_triangles(source))
    {
        let [a, b] = faces[..] else { continue };
        if let (Some((va, reverse)), Some((vb, other_reverse))) = (intact[a], intact[b])
            && reverse == other_reverse
        {
            // A re-triangulated or separated corner can share a position
            // without sharing a mesh vertex. Only restore a real quad.
            if !source.triangles[a].iter().zip(va).all(|(&v, index)| {
                source.triangles[b]
                    .iter()
                    .position(|&w| w == v)
                    .is_none_or(|k| vb[k] == index)
            }) {
                continue;
            }
            let mut mapped = quad.map(|v| {
                source.triangles[a]
                    .iter()
                    .position(|&w| w == v)
                    .map(|k| va[k])
                    .unwrap_or_else(|| {
                        vb[source.triangles[b].iter().position(|&w| w == v).unwrap()]
                    })
            });
            if reverse {
                mapped.swap(1, 3);
            }
            out.quads.push(mapped);
        }
    }
    if uv {
        out.uv.topology.clone_from(&out.triangles);
    }
    out.canonicalize_polygroups();
    if !out.hidden_faces.iter().any(|&hidden| hidden) {
        out.hidden_faces.clear();
    }
    out.creases = crease_edges.into_iter().collect();
    out.uv.seams = uv_edges.into_iter().collect();
    out.set_shading_seams(shade_edges.into_iter().collect());
    validate_surface(&out).map_err(|e| format!("Clipped surface: {e}"))?;
    out.validate_closed()
        .map_err(|e| format!("Clipped solid: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Material, Primitive};
    use BooleanRole::{Add, Intersect, Subtract};

    fn block(size: f32, center: Vec3) -> Mesh {
        Primitive {
            size: Vec3(size, size, size),
            center,
            divisions: [1; 3],
            ..Default::default()
        }
        .mesh()
        .unwrap()
    }

    fn stack(parts: &[(BooleanRole, &Mesh)]) -> Result<Mesh, String> {
        let operands: Vec<_> = parts
            .iter()
            .map(|&(role, mesh)| Operand {
                role,
                name: role.label().into(),
                mesh: mesh.clone(),
            })
            .collect();
        evaluate(&operands, &|| false)
    }

    fn volume(mesh: &Mesh) -> f32 {
        mesh.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| mesh.vertices[i]);
                a.dot(b.cross(c)) / 6.
            })
            .sum()
    }

    #[test]
    fn surface_boolean_coplanar_overlap_and_disjoint_or_identical_solids() {
        let a = block(2., Vec3::default());
        let b = block(2., Vec3(1., 0., 0.));
        let far = block(2., Vec3(3., 0., 0.));
        for (operand, role, expected) in [
            (&b, Add, 12.),
            (&b, Subtract, 4.),
            (&b, Intersect, 4.),
            (&far, Add, 16.),
            (&far, Subtract, 8.),
            (&far, Intersect, 0.),
            (&a, Add, 8.),
            (&a, Subtract, 0.),
            (&a, Intersect, 8.),
        ] {
            let mesh = stack(&[(Add, &a), (role, operand)]).unwrap();
            assert!(
                (volume(&mesh) - expected).abs() < 1e-4,
                "{role:?}: {}",
                volume(&mesh)
            );
            if expected > 0. {
                mesh.validate_closed().unwrap();
            } else {
                assert!(mesh.triangles.is_empty());
            }
        }
        let restored = stack(&[(Add, &a), (Intersect, &far), (Subtract, &a), (Add, &b)]).unwrap();
        assert_eq!(restored, b);
    }

    #[test]
    fn surface_boolean_interpolates_uvs_along_a_partial_cut() {
        let mut base = block(2., Vec3::default());
        let cutter = block(1., Vec3(0.8, 0.25, 0.25));
        let uv = |p: Vec3| [(p.0 + 1.) / 2., (p.1 + 1.) / 2.];
        base.uv = UvMap {
            topology: base.triangles.clone(),
            corners: base
                .triangles
                .iter()
                .map(|t| t.map(|i| uv(base.vertices[i])))
                .collect(),
            ..Default::default()
        };
        base.uv.masks = base.uv.corners.iter().map(|c| c.map(|p| p[1])).collect();
        let result = stack(&[(Add, &base), (Subtract, &cutter)]).unwrap();
        assert!(result.uv.current(&result));
        let mut clipped_vertices = 0;
        for (face, t) in result
            .triangles
            .iter()
            .enumerate()
            .filter(|(i, _)| result.uv.mapped(*i))
        {
            for (corner, &index) in t.iter().enumerate() {
                let point = result.vertices[index];
                let expected = uv(point);
                assert!((result.uv.corners[face][corner][0] - expected[0]).abs() < 1e-6);
                assert!((result.uv.corners[face][corner][1] - expected[1]).abs() < 1e-6);
                assert!((result.uv.masks[face][corner] - expected[1]).abs() < 1e-6);
                clipped_vertices += usize::from(!base.vertices.contains(&point));
            }
        }
        assert!(clipped_vertices > 0 && !result.shading_edges().is_empty());
        base.mask.fill(1.);
        assert!(
            stack(&[(Add, &base), (Subtract, &cutter)])
                .unwrap_err()
                .contains("masked")
        );
    }

    #[test]
    fn surface_boolean_enclosed_cavity_keeps_winding_quads_and_attributes() {
        let mut outer = block(2., Vec3::default());
        let mut inner = block(1., Vec3::default());
        outer
            .create_vertex_group("Detail".into(), 0.5, true)
            .unwrap();
        outer.materials = vec![
            Material {
                base_color: [0.2, 0.3, 0.4],
                ..Default::default()
            };
            outer.vertices.len()
        ];
        outer.mask[0] = 0.4;
        outer.hidden_faces.resize(outer.triangles.len(), false);
        outer.hidden_faces[0] = true;
        inner
            .create_vertex_group("Cut wall".into(), 0.5, true)
            .unwrap();
        inner.polygroups = vec![1; inner.triangles.len()];
        inner.uv = UvMap {
            topology: inner.triangles.clone(),
            corners: inner
                .triangles
                .iter()
                .map(|t| t.map(|v| [inner.vertices[v].0 + 0.5, inner.vertices[v].1 + 0.5]))
                .collect(),
            ..Default::default()
        };
        let mesh = stack(&[(Add, &outer), (Subtract, &inner)]).unwrap();
        assert!((volume(&mesh) - 7.).abs() < 1e-4);
        assert_eq!(mesh.quads.len(), outer.quads.len() + inner.quads.len());
        assert_eq!(mesh.hidden_faces.iter().filter(|&&v| v).count(), 1);
        assert!(mesh.uv.current(&mesh));
        for (f, t) in mesh.triangles.iter().enumerate() {
            if mesh.polygroups[f] == 2 {
                assert!(mesh.uv.mapped(f));
                let [a, b, c] = t.map(|v| mesh.vertices[v]);
                assert!((b - a).cross(c - a).dot(a) < 0.);
                assert_eq!(
                    mesh.uv.corners[f],
                    t.map(|v| [mesh.vertices[v].0 + 0.5, mesh.vertices[v].1 + 0.5])
                );
            }
        }
        let v = mesh
            .vertices
            .iter()
            .position(|p| *p == outer.vertices[0])
            .unwrap();
        assert_eq!(mesh.mask[v], 0.4);
        assert_eq!(mesh.materials[v], outer.materials[0]);
        assert_eq!(
            mesh.vertex_groups[0].weights[v],
            outer.vertex_groups[0].weights[0]
        );
        let protected = stack(&[(Add, &inner), (Subtract, &outer)]).unwrap_err();
        assert!(
            protected.contains("mask") || protected.contains("hidden"),
            "{protected}"
        );
    }
}
