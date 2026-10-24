use super::*;
use crate::sculpt::{Geometry, Kind, MeshSettings, Primitive, PrimitiveKind, Vec3};

fn plane() -> Mesh {
    let mut p = Primitive::for_kind(PrimitiveKind::Plane);
    p.divisions = [8, 8, 1];
    p.mesh().unwrap()
}
fn dab() -> Dab {
    Dab {
        center: Vec3(0., 0., 0.),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 2.,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    }
}

#[test]
fn material_paint_pressure_masks_hidden_groups_channels_and_symmetry_overlap() {
    let mut mesh = plane();
    mesh.create_vertex_group("Original group".into(), 0.5, true)
        .unwrap();
    mesh.mask[40] = 1.;
    mesh.hidden_faces = mesh
        .triangles
        .iter()
        .map(|t| t.iter().any(|&i| mesh.vertices[i].0 < -0.5))
        .collect();
    let before = mesh.clone();
    let eligible = mesh.sculptable_vertices();
    let mut brush = Brush::new(Kind::Paint);
    brush.strength = 0.7;
    let mut half = mesh.clone();
    let mut overlap = mesh.clone();
    assert!(mesh.paint_material(&brush, &[dab()], &[]));
    assert!(half.paint_material(
        &brush,
        &[Dab {
            pressure: 0.5,
            ..dab()
        }],
        &[]
    ));
    assert!(overlap.paint_material(&brush, &[dab(), dab()], &[]));
    assert_eq!(
        overlap, mesh,
        "mirrored copies must not double opacity at the seam"
    );
    assert_eq!(mesh.vertices, before.vertices);
    assert_eq!(mesh.mask, before.mask);
    assert_eq!(mesh.vertex_groups, before.vertex_groups);
    assert_eq!(mesh.polygroups, before.polygroups);
    for (i, &allowed) in eligible.iter().enumerate() {
        if !allowed || before.mask[i] == 1. {
            assert_eq!(mesh.material(i), before.material(i));
        }
        for c in 0..3 {
            assert!(
                (half.material(i).base_color[c]
                    - (mesh.material(i).base_color[c] + before.material(i).base_color[c]) * 0.5)
                    .abs()
                    < 1e-6
            );
        }
    }
    let colors = mesh.materials.clone();
    brush.paint_channels = PaintChannels::Roughness;
    brush.material.roughness = 0.01;
    mesh.paint_material(&brush, &[dab()], &[]);
    for (a, b) in mesh.materials.iter().zip(colors) {
        assert_eq!(a.base_color, b.base_color);
        assert_eq!(a.metallic, b.metallic);
    }
    mesh.validate().unwrap();
}

#[test]
fn material_subdivision_interpolates_linear_channels_and_retains_high_level_detail() {
    let mut mesh = plane();
    mesh.materials = mesh
        .vertices
        .iter()
        .map(|p| Material {
            base_color: [(p.0 + 1.) * 0.5, 0., 0.],
            roughness: (p.1 + 1.) * 0.5,
            metallic: 0.3,
        })
        .collect();
    let mut geometry = Geometry::new(mesh.clone());
    geometry.subdivide().unwrap();
    for (i, p) in geometry.mesh.vertices.iter().enumerate() {
        assert!((geometry.mesh.material(i).base_color[0] - (p.0 + 1.) * 0.5).abs() < 1e-6);
        assert!((geometry.mesh.material(i).roughness - (p.1 + 1.) * 0.5).abs() < 1e-6);
    }
    let last = geometry.mesh.materials.len() - 1;
    geometry.mesh.materials[last].metallic = 0.9;
    let high = geometry.mesh.clone();
    geometry.set_level(1).unwrap();
    geometry.set_level(2).unwrap();
    assert_eq!(geometry.mesh, high);
    geometry.set_level(1).unwrap();
    geometry.mesh.materials[0].roughness += 0.1;
    geometry.set_level(2).unwrap();
    assert_eq!(geometry.mesh.materials[last].metallic, 0.9);
    assert!((geometry.mesh.materials[0].roughness - 0.1).abs() < 1e-6);
    geometry.settings.validate(&geometry.mesh).unwrap();
}

#[test]
fn material_topology_reindex_shell_merge_adaptive_and_voxel_transfer() {
    let copper = Material::examples()[4].1;
    let mut mesh = Mesh::sphere();
    mesh.materials = vec![copper; mesh.vertices.len()];
    let shell = plane();
    let mut painted_shell = shell.clone();
    painted_shell.materials = vec![copper; shell.vertices.len()];
    let thick = painted_shell.solidify(0.1).unwrap();
    assert!(thick.materials.iter().all(|m| *m == copper));
    assert_eq!(thick.materials.len(), thick.vertices.len());
    mesh.mask.fill(1.);
    let extracted = mesh.extract_masked().unwrap();
    assert_eq!(extracted.materials, mesh.materials);
    mesh.mask.fill(0.);
    let merged = mesh.merged_with(&shell).unwrap();
    assert_eq!(&merged.materials[..mesh.vertices.len()], &mesh.materials);
    assert!(
        merged.materials[mesh.vertices.len()..]
            .iter()
            .all(Material::is_default)
    );
    let settings = super::super::AdaptiveSettings {
        enabled: true,
        ..Default::default()
    };
    let mut budget = super::super::AdaptiveBudget::default();
    mesh.adapt_stenciled(
        &Brush::new(Kind::Standard),
        super::super::AdaptiveRegion {
            dabs: &[Dab {
                center: Vec3(0., 0., 1.),
                radius: 0.4,
                ..dab()
            }],
            stencils: &[],
        },
        &settings,
        &mut budget,
        true,
    );
    mesh.validate().unwrap();
    assert!(mesh.materials.iter().all(|m| *m == copper));
    let result = mesh.remesh(16).unwrap();
    assert_eq!(result.materials.len(), result.vertices.len());
    for m in &result.materials {
        for (a, b) in m.values().into_iter().zip(copper.values()) {
            assert!((a - b).abs() < 2e-6);
        }
    }
}

#[test]
fn material_surface_transfer_is_barycentric_and_cancelled_atomically() {
    let mut source = plane();
    source.materials = source
        .vertices
        .iter()
        .map(|p| Material {
            base_color: [(p.0 + 1.) * 0.5, (p.1 + 1.) * 0.5, 0.],
            ..Material::default()
        })
        .collect();
    let mut dest = plane();
    for p in &mut dest.vertices {
        p.0 *= 0.91;
        p.1 *= 0.83;
        p.2 = 0.02;
    }
    let before = dest.clone();
    assert!(source.transfer_materials_to(&mut dest, &|| true).is_err());
    assert_eq!(dest, before);
    source.transfer_materials_to(&mut dest, &|| false).unwrap();
    for (i, p) in dest.vertices.iter().enumerate() {
        assert!((dest.material(i).base_color[0] - (p.0 + 1.) * 0.5).abs() < 1e-5);
        assert!((dest.material(i).base_color[1] - (p.1 + 1.) * 0.5).abs() < 1e-5);
    }
}

#[test]
fn material_slicing_mirroring_modeling_and_projection_preserve_surface_values() {
    use crate::sculpt::{
        Axis, MirrorOptions, ModelingSymmetry, ProjectionOptions, SliceMode, SliceOptions,
        SlicePlane, TopologyEdit,
    };
    let mut mesh = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
    mesh.materials = mesh
        .vertices
        .iter()
        .map(|p| Material {
            base_color: [(p.0 + 1.) * 0.5, (p.1 + 1.) * 0.5, 0.3],
            roughness: 0.4,
            metallic: 0.6,
        })
        .collect();
    let (cut, other) = mesh
        .slice(
            SlicePlane {
                origin: Vec3(0.13, 0., 0.),
                normal: Vec3(1., 0., 0.),
            },
            SliceOptions {
                mode: SliceMode::Separate,
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
    for part in [&cut, other.as_ref().unwrap()] {
        part.validate().unwrap();
        for (i, p) in part.vertices.iter().enumerate() {
            assert!((part.material(i).base_color[0] - (p.0 + 1.) * 0.5).abs() < 1e-6);
        }
    }
    let mirror = mesh
        .mirror_half(
            MirrorOptions {
                axis: Axis::X,
                source_positive: true,
                offset: 0.,
            },
            &|| false,
        )
        .unwrap();
    for (i, p) in mirror.vertices.iter().enumerate() {
        assert!((mirror.material(i).base_color[0] - (p.0.abs() + 1.) * 0.5).abs() < 1e-6);
    }
    let edge = mesh.polygon_edges()[0];
    let split = mesh
        .edit_model_topology(
            TopologyEdit::Split,
            &[],
            &[edge],
            &[],
            &ModelingSymmetry::new(
                &mesh,
                &crate::sculpt::Symmetry {
                    enabled: false,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .unwrap();
    split.validate().unwrap();
    for (i, p) in split.vertices.iter().enumerate() {
        assert!((split.material(i).base_color[0] - (p.0 + 1.) * 0.5).abs() < 1e-6);
    }
    let mut target = mesh.clone();
    for p in &mut target.vertices {
        *p = *p * 1.02;
    }
    let (projected, _) = target
        .project_details(
            &mesh,
            ProjectionOptions {
                max_distance: 0.2,
                strength: 1.,
            },
            &|| false,
        )
        .unwrap();
    assert_eq!(
        projected.materials, target.materials,
        "detail projection retains the target's paint"
    );
}

#[test]
fn material_legacy_defaults_validation_and_example_roundtrips() {
    let legacy: crate::sculpt::Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/v1-project.json")).unwrap();
    assert!(legacy.mesh.materials.is_empty());
    legacy.validate().unwrap();
    let mut mesh = plane();
    for (_, material) in Material::examples() {
        material.validate().unwrap();
        mesh.materials = vec![material; mesh.vertices.len()];
        let restored: Mesh = serde_json::from_str(&serde_json::to_string(&mesh).unwrap()).unwrap();
        assert_eq!(mesh, restored);
        MeshSettings::default().validate(&restored).unwrap();
    }
    mesh.materials[0].metallic = f32::NAN;
    assert!(mesh.validate().is_err());
    mesh.materials.clear();
    mesh.materials.push(Material::default());
    assert!(mesh.validate().is_err());
}
