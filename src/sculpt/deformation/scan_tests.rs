use super::*;

// Frozen pre-optimization discovery and reduction loops.
fn reference_limit(
    mesh: &mut Mesh,
    before: &[Vec3],
    prevent_folds: bool,
    direction: Option<Vec3>,
    local_rejection: bool,
) -> bool {
    let active: Vec<_> = mesh
        .triangles
        .iter()
        .filter(|t| t.iter().any(|&v| before[v] != mesh.vertices[v]))
        .copied()
        .collect();
    if active.is_empty() {
        return mesh.vertices.iter().zip(before).any(|(a, b)| a != b);
    }
    // Individually well-oriented faces can still fold back against their
    // neighbour. Check shared edges too when moving onto a reference surface.
    // Preserve existing sharp folds, but never create a new opposing pair.
    let mut pairs = Vec::new();
    if prevent_folds {
        let mut edges = std::collections::HashMap::new();
        for (face, &[a, b, c]) in mesh.triangles.iter().enumerate() {
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let edge = if a < b { (a, b) } else { (b, a) };
                if let Some(other) = edges.insert(edge, face) {
                    let first = mesh.triangles[other];
                    let second = mesh.triangles[face];
                    if first
                        .iter()
                        .chain(&second)
                        .any(|&v| before[v] != mesh.vertices[v])
                    {
                        let agreement = normal_agreement(
                            face_normal(first.map(|v| before[v])),
                            face_normal(second.map(|v| before[v])),
                        );
                        pairs.push((other, face, agreement.min(0.)));
                    }
                }
            }
        }
    }
    let safe_pair = |vertices: &[Vec3], &(a, b, minimum): &(usize, usize, f64)| {
        normal_agreement(
            face_normal(mesh.triangles[a].map(|v| vertices[v])),
            face_normal(mesh.triangles[b].map(|v| vertices[v])),
        ) >= minimum - 1e-6
    };
    let mut reduce = vec![false; mesh.vertices.len()];
    for pass in 0..if local_rejection { 20 } else { 12 } {
        let mut unsafe_face = false;
        for &t in &active {
            if !safe_face(t.map(|v| before[v]), t.map(|v| mesh.vertices[v]), direction) {
                unsafe_face = true;
                for v in t {
                    reduce[v] = true;
                }
            }
        }
        for pair in &pairs {
            if !safe_pair(&mesh.vertices, pair) {
                unsafe_face = true;
                for v in mesh.triangles[pair.0]
                    .into_iter()
                    .chain(mesh.triangles[pair.1])
                {
                    reduce[v] = true;
                }
            }
        }
        if !unsafe_face {
            return mesh.vertices.iter().zip(before).any(|(a, b)| a != b);
        }
        for (v, flag) in reduce.iter_mut().enumerate() {
            if *flag {
                mesh.vertices[v] = if local_rejection && pass >= 8 {
                    // A few pre-existing slivers must not cancel smoothing
                    // everywhere else. Pin failed proposals, then recheck
                    // neighboring faces against the resulting mixed update.
                    before[v]
                } else {
                    before[v] + (mesh.vertices[v] - before[v]) * 0.5
                };
                *flag = false;
            }
        }
    }
    // Never commit an unchecked last reduction; pinning one vertex can
    // affect its neighbours. Reject a pathological dab without changing positions.
    if active
        .iter()
        .any(|&t| !safe_face(t.map(|v| before[v]), t.map(|v| mesh.vertices[v]), direction))
        || pairs.iter().any(|pair| !safe_pair(&mesh.vertices, pair))
    {
        mesh.vertices.copy_from_slice(before);
        return false;
    }
    mesh.vertices.iter().zip(before).any(|(a, b)| a != b)
}

#[test]
fn compact_discovery_and_sparse_reductions_match_original_guards() {
    let mut meshes = vec![
        Mesh::default(),
        Mesh {
            vertices: vec![Vec3::default(), Vec3(1., 0., 0.)],
            mask: vec![0.; 2],
            ..Default::default()
        },
        Mesh::starting_sphere(),
    ];
    let mut mixed = Mesh::starting_sphere();
    // Existing degeneracies and disconnected vertices must retain their handling.
    mixed.triangles.extend([[0, 0, 1], [0, 1, 1]]);
    mixed.vertices.push(Vec3(2., -0., 3.));
    mixed.mask.push(0.);
    meshes.push(mixed);
    let mut passes_reduced = 0;
    for base in meshes {
        let before = &base.vertices;
        for pattern in 0..6 {
            let mut proposal = base.clone();
            for (i, p) in proposal.vertices.iter_mut().enumerate() {
                match pattern {
                    0 => {}
                    1 if i % 17 == 0 => p.2 += 0.001,
                    2 if p.2 > 0.75 => p.2 -= 2.,
                    3 if i % 3 == 0 => *p = *p * -2.,
                    4 => *p = *p + Vec3(0.1, -0.05, 0.02),
                    5 if i + 1 == before.len() => p.0 += 0.25,
                    _ => {}
                }
            }
            for (folds, direction, local) in [
                (false, None, false),
                (false, Some(Vec3(0., 0., 1.)), false),
                (true, None, false),
                (true, None, true),
                (false, None, true),
            ] {
                let mut expected = proposal.clone();
                let expected_changed =
                    reference_limit(&mut expected, before, folds, direction, local);
                let mut actual = proposal.clone();
                let actual_changed = actual.limit_deformation(before, folds, direction, local);
                assert_eq!(actual_changed, expected_changed);
                let bits = |mesh: &Mesh| {
                    mesh.vertices
                        .iter()
                        .map(|v| [v.0.to_bits(), v.1.to_bits(), v.2.to_bits()])
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    bits(&actual),
                    bits(&expected),
                    "pattern={pattern} folds={folds} local={local}"
                );
                passes_reduced += usize::from(actual.vertices != proposal.vertices);
            }
        }
    }
    assert!(
        passes_reduced > 0,
        "fixtures must exercise rejection/reduction"
    );
}
