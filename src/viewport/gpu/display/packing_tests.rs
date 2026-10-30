use super::*;
use crate::sculpt::Material;

fn assert_update(previous: &DisplayMesh, next: &DisplayMesh) -> usize {
    let stride = previous.vertex_stride();
    assert_eq!(stride, next.vertex_stride());
    let mut packed = previous.vertices(stride);
    let mut uploaded = bytemuck::cast_slice::<_, u8>(&packed).to_vec();
    // The original full packer is an independent byte-for-byte oracle.
    let expected = next.vertices(stride);
    let expected_bytes = bytemuck::cast_slice::<_, u8>(&expected);
    let changed = uploaded
        .chunks_exact(stride * 4)
        .zip(expected_bytes.chunks_exact(stride * 4))
        .filter(|(a, b)| a != b)
        .count();
    let update = next.update_vertices(stride, &mut packed);
    assert_eq!(bytemuck::cast_slice::<_, u8>(&packed), expected_bytes);
    assert_eq!(update.changed_vertices, changed);
    assert!(update.ranges.len() <= 64);
    for range in update.ranges {
        assert_eq!(range.start % 4096, 0);
        assert!(range.end == uploaded.len() || range.end % 4096 == 0);
        uploaded[range.clone()].copy_from_slice(&expected_bytes[range]);
    }
    assert_eq!(
        uploaded, expected_bytes,
        "the reported writes must reconstruct the whole buffer"
    );
    let unchanged = next.update_vertices(stride, &mut packed);
    assert_eq!(unchanged.changed_vertices, 0);
    assert!(unchanged.ranges.is_empty());
    changed
}

#[test]
fn vertex_packing_sparse_geometry_material_mask_and_fan_edits_match_full_packing() {
    for extended in [false, true] {
        let mut previous = DisplayMesh::new(&Document::default());
        let mut material = Material::from_srgb([0.2, 0.4, 0.6], 0.65, 0.1);
        if extended {
            material.emission_strength = 0.5;
        }
        let geometry = Arc::make_mut(&mut previous.geometry);
        geometry.mesh.materials = vec![material; geometry.mesh.vertices.len()];
        let mut next = previous.clone();
        let geometry = Arc::make_mut(&mut next.geometry);
        let a = geometry.shading.vertices[0].0;
        let b = geometry.shading.vertices[20].0;
        geometry.mesh.vertices[a].0 += 0.05;
        geometry.mesh.mask[b] = 0.8;
        geometry.mesh.materials[b].base_color = [0.6, 0.2, 0.1];
        geometry.mesh.materials[b].roughness = 0.2;
        geometry.mesh.materials[b].metallic = 0.7;
        if extended {
            geometry.mesh.materials[b].layers[1].weight = 0.7;
            geometry.mesh.materials[b].layers[1].surface.emission = [2., 0.1, 0.2];
        }
        geometry.shading.vertices.swap(3, 4);
        geometry.shading.vertices[9].1 = Vec3(-0., f32::from_bits(0x7fc00001), 0.);
        let changed = assert_update(&previous, &next);
        assert!(
            changed > 0 && changed < 10,
            "localized edits must not repack the full mesh"
        );
    }
}

#[test]
fn vertex_packing_defaults_muted_models_and_active_masks_match_full_packing() {
    let previous = DisplayMesh::new(&Document::default());
    let mut explicit = previous.clone();
    let geometry = Arc::make_mut(&mut explicit.geometry);
    geometry.mesh.materials = vec![
        Material::default().with_link(Material::DEFAULT_PRESET_ID);
        geometry.mesh.vertices.len()
    ];
    assert_eq!(
        assert_update(&previous, &explicit),
        0,
        "material provenance is not a packed channel"
    );
    let mut next = explicit.clone();
    let geometry = Arc::make_mut(&mut next.geometry);
    geometry.active_vertices /= 2;
    for (i, muted) in geometry.muted_vertices.iter_mut().enumerate() {
        *muted = i >= geometry.active_vertices;
    }
    assert!(assert_update(&explicit, &next) > 0);
}

#[test]
fn vertex_packing_full_dirty_range_still_updates_later_records_and_handles_empty_data() {
    let previous = DisplayMesh::new(&Document::default());
    let mut next = previous.clone();
    for p in &mut Arc::make_mut(&mut next.geometry).mesh.vertices {
        p.0 += 0.1;
    }
    assert_eq!(assert_update(&previous, &next), next.shading.vertices.len());
    let mut empty = previous.clone();
    Arc::make_mut(&mut empty.geometry).shading.vertices.clear();
    assert_eq!(assert_update(&empty, &empty), 0);
}
