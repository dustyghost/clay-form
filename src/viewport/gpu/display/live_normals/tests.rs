use super::*;

fn grid() -> Mesh {
    let mut mesh = Mesh::default();
    for sheet in 0..2 {
        let offset = mesh.vertices.len();
        for y in 0..32 {
            for x in 0..32 {
                let x = (x as f32 - 16.) / 16.;
                mesh.vertices.push(Vec3(
                    x,
                    y as f32 / 16.,
                    x.max(0.) * 2. + sheet as f32 * 0.0001,
                ));
            }
        }
        for y in 0..31 {
            for x in 0..31 {
                let a = offset + y * 32 + x;
                mesh.triangles
                    .extend([[a, a + 1, a + 33], [a, a + 33, a + 32]]);
            }
        }
    }
    // Repeated corners, reversed/duplicate faces, a sliver and an isolated vertex.
    mesh.triangles
        .extend([[17, 17, 18], [32, 33, 65], [32, 65, 33]]);
    mesh.vertices[300] = mesh.vertices[299] + Vec3(1e-7, 0., 0.);
    mesh.vertices.push(Vec3(4., 4., 4.));
    mesh.hidden_faces = (0..mesh.triangles.len()).map(|i| i % 5 == 0).collect();
    mesh
}

fn assert_bits(a: &[(usize, Vec3)], b: &[(usize, Vec3)]) {
    assert_eq!(a.len(), b.len());
    for (&(ai, a), &(bi, b)) in a.iter().zip(b) {
        assert_eq!(ai, bi);
        assert_eq!(
            [a.0.to_bits(), a.1.to_bits(), a.2.to_bits()],
            [b.0.to_bits(), b.1.to_bits(), b.2.to_bits()]
        );
    }
}

#[test]
fn compact_corners_preserve_source_order_degenerates_and_empty_vertices() {
    let mesh = grid();
    let adjacency = VertexCorners::new(&mesh).unwrap();
    for vertex in 0..mesh.vertices.len() {
        let expected: Vec<_> = mesh
            .triangles
            .iter()
            .flatten()
            .enumerate()
            .filter(|&(_, &v)| v == vertex)
            .map(|(c, _)| c as u32)
            .collect();
        assert_eq!(adjacency.incident(vertex), expected);
    }
    assert_eq!(adjacency.corners.capacity(), mesh.triangles.len() * 3);
    assert_eq!(adjacency.offsets.capacity(), mesh.vertices.len() + 1);
    assert!(adjacency.incident(mesh.vertices.len() - 1).is_empty());
    let empty = VertexCorners::new(&Mesh::default()).unwrap();
    assert_eq!(empty.offsets, [0]);
    assert!(empty.corners.is_empty());
}

#[test]
fn local_fans_match_full_bits_across_seams_boundaries_and_repeated_edits() {
    let mut mesh = grid();
    let adjacency = VertexCorners::new(&mesh).unwrap();
    let shading = mesh.surface_shading();
    assert!(
        shading.vertices.len() > mesh.vertices.len(),
        "fixture must split smoothing fans"
    );
    let mut previous = full_normals(&mesh, &shading.triangles, &shading.vertices).vertices;
    let mut seed = 0x1234_u32;
    for step in 0..80 {
        let old_positions = mesh.vertices.clone();
        // Start with boundary, degenerate, sliver and ridge edits, then jump
        // between unrelated parts of both nearby sheets.
        let vertex = if step < 5 {
            [0, 17, 300, 16, 1024 + 16][step]
        } else {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed as usize % 2048
        };
        mesh.vertices[vertex].2 += if step % 2 == 0 { 0.017 } else { -0.021 };
        let changed = changed_vertices(&old_positions, &mesh.vertices).unwrap();
        let updated = adjacency
            .update(&mesh, &shading.triangles, &previous, &changed)
            .unwrap();
        assert!(updated.local);
        assert!(updated.evaluated_faces < mesh.triangles.len() / 10);
        let full = full_normals(&mesh, &shading.triangles, &previous);
        assert_bits(&updated.vertices, &full.vertices);
        // A nearby but disconnected sheet cannot acquire this edit's normals.
        for (before, after) in previous.iter().zip(&updated.vertices) {
            if (before.0 < 1024) != (vertex < 1024) {
                assert_bits(std::slice::from_ref(before), std::slice::from_ref(after));
            }
        }
        previous = updated.vertices;
    }
}

#[test]
fn change_discovery_includes_signed_zero_and_rejects_broad_edits() {
    let before = vec![Vec3::default(); 128];
    let mut after = before.clone();
    after[7].0 = -0.;
    after[100].2 = 0.1;
    assert_eq!(changed_vertices(&before, &after).unwrap(), [7, 100]);
    for p in &mut after {
        p.1 = 0.2;
    }
    assert!(changed_vertices(&before, &after).is_none());
}

#[test]
fn scattered_changes_and_high_valence_choose_the_full_pass() {
    let mesh = grid();
    let shading = mesh.surface_shading();
    let adjacency = VertexCorners::new(&mesh).unwrap();
    let changed: Vec<_> = (0..2048).step_by(8).collect();
    assert!(
        adjacency
            .update(&mesh, &shading.triangles, &shading.vertices, &changed)
            .is_none()
    );
    let mut star = Mesh::default();
    star.vertices.push(Vec3::default());
    for i in 0..512 {
        let a = i as f32 * std::f32::consts::TAU / 512.;
        star.vertices.push(Vec3(a.cos(), a.sin(), 0.));
        star.triangles.push([0, i + 1, (i + 1) % 512 + 1]);
    }
    let shading = star.surface_shading();
    assert!(
        VertexCorners::new(&star)
            .unwrap()
            .update(&star, &shading.triangles, &shading.vertices, &[0])
            .is_none()
    );
}
