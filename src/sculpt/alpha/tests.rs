use super::*;
use crate::{
    document::Document,
    sculpt::{Brush, Geometry, Kind, Primitive, PrimitiveKind, StrokeMode},
};

fn image(values: Vec<f32>, width: usize) -> Alpha {
    Alpha::Image {
        width,
        height: values.len() / width,
        pixels: values.into(),
    }
}
fn asset(alpha: Alpha) -> Asset {
    Asset {
        name: "Stone relief".into(),
        category: "Rock".into(),
        image: alpha,
    }
}
fn projection(alpha: Alpha) -> Projection {
    Projection {
        image: alpha,
        frame: Frame {
            origin: Vec3::default(),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
        },
        view: Vec3(0., 0., 1.),
        distance: 0.,
        position: [0.; 2],
        size: [2.; 2],
        rotation: 0.,
    }
}
#[test]
fn alpha_editor_crops_levels_inverts_and_fades_without_changing_source() {
    let source = image(vec![0., 0.25, 0.5, 0.75, 1., 0., 0.25, 0.5, 0.75, 1.], 5);
    let untouched = source.clone();
    assert_eq!(Adjustments::default().apply(&source).unwrap(), source);
    let mut edit = Adjustments {
        crop: [0.5, 0., 1., 1.],
        black: 0.5,
        white: 1.,
        invert: true,
        ..Default::default()
    };
    let cropped = edit.apply(&source).unwrap();
    assert_eq!(cropped.sample(0., 0.5), 1.);
    assert_eq!(cropped.sample(1., 0.5), 0.);
    edit.crop = [0., 0., 1., 1.];
    edit.black = 0.;
    edit.edge_fade = 0.2;
    assert_eq!(edit.apply(&source).unwrap().sample(0., 0.), 0.);
    assert_eq!(source, untouched);
    edit.white = edit.black;
    assert!(edit.apply(&source).is_err());
    edit.white = 1.;
    edit.crop[2] = f32::NAN;
    assert!(edit.apply(&source).is_err());
}
#[test]
fn alpha_library_roundtrip_reuse_and_removal_keep_brushes_and_stencil_independent() {
    let mut d = Document::default();
    let a = asset(image(vec![0., 0.125, 0.75, 1.], 2));
    let i = d.save_alpha(None, a.clone()).unwrap();
    d.set_alpha(0, a.image.clone()).unwrap();
    d.set_alpha(1, a.image.clone()).unwrap();
    let mut stencil = Stencil::new(a.clone());
    stencil.rotation = 32.;
    stencil.position = [0.1, -0.2];
    stencil.surface = Some(projection(a.image.clone()).frame);
    d.set_stencil(Some(stencil.clone())).unwrap();
    let project = d.project();
    let bytes = serde_json::to_vec(&project).unwrap();
    let restored = serde_json::from_slice(&bytes).unwrap();
    d.replace_project(restored, "test.clay").unwrap();
    assert!(project == d.project());
    assert!(!d.is_dirty());
    d.save_alpha(Some(i), a.clone()).unwrap();
    assert!(!d.is_dirty());
    let mut invalid = a.clone();
    invalid.name.clear();
    assert!(d.save_alpha(Some(i), invalid).is_err());
    assert!(!d.is_dirty());
    d.save_alpha(None, a.clone()).unwrap();
    d.remove_alpha(i).unwrap();
    assert_eq!(d.alphas(), std::slice::from_ref(&a));
    assert_eq!(d.stencil(), Some(&stencil));
    assert_eq!(d.brushes()[0].alpha, a.image);
    assert_eq!(d.brushes()[1].alpha, a.image);
    assert_eq!(d.history_counts(), (0, 0));
    let legacy: crate::sculpt::Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/v1-project.json")).unwrap();
    assert!(legacy.alphas.is_empty() && legacy.stencil.is_none());
    legacy.validate().unwrap();
}
#[test]
fn alpha_png_preserves_height_precision_and_rejects_bad_inputs() {
    let dir = std::env::temp_dir().join(format!("clay-alpha-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("height.png");
    let source = image(vec![0., 0.12345, 0.54321, 1.], 2);
    crate::persistence::save_alpha(path.to_str().unwrap(), &source).unwrap();
    let loaded = crate::persistence::load_alpha(path.to_str().unwrap()).unwrap();
    for (u, v) in [(0., 0.), (1., 0.), (0., 1.), (1., 1.)] {
        assert!((loaded.sample(u, v) - source.sample(u, v)).abs() < 2. / 65535.);
    }
    assert!(crate::persistence::load_alpha(dir.join("missing.png").to_str().unwrap()).is_err());
    let large = dir.join("oversized.png");
    image::GrayImage::new(2049, 1).save(&large).unwrap();
    assert!(crate::persistence::load_alpha(large.to_str().unwrap()).is_err());
    assert!(image(vec![f32::NAN], 1).validate().is_err());
    let mut stencil = Stencil::new(asset(source));
    stencil.size[0] = 0.;
    assert!(stencil.validate().is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn alpha_stencil_projection_tracks_rotation_perspective_and_mirrored_frames() {
    let mut p = projection(image(vec![0., 1., 0., 1.], 2));
    assert!((p.weight(Vec3(0.5, 0., 0.)) - 0.75).abs() < 1e-6);
    p.rotation = 90.;
    assert!((p.weight(Vec3(0., 0.5, 0.)) - 0.75).abs() < 1e-6);
    p.rotation = 0.;
    p.distance = 4.;
    assert!((p.weight(Vec3(0.25, 0., 2.)) - 0.75).abs() < 1e-6);
    assert_eq!(p.weight(Vec3(0., 0., 5.)), 0.);
    let symmetry = Symmetry {
        enabled: true,
        center: Vec3(0.3, 0., 0.),
        ..Default::default()
    };
    let copies = p.copies(&symmetry);
    assert!(
        (copies[0].weight(Vec3(0.25, 0., 2.)) - copies[1].weight(Vec3(0.35, 0., 2.))).abs() < 1e-6
    );
}
#[test]
fn alpha_stencil_respects_masks_hidden_faces_pressure_symmetry_and_undo_with_fixed_topology() {
    let mut primitive = Primitive::for_kind(PrimitiveKind::Plane);
    primitive.divisions = [20, 20, 1];
    let original = primitive.mesh().unwrap();
    let mut d = Document::default();
    d.replace_geometry(Geometry::new(original.clone())).unwrap();
    let mut stencil = projection(image(vec![0., 0., 1., 1., 0., 0., 1., 1.], 4));
    stencil.position = [0.7, 0.];
    stencil.size = [0.7, 1.4];
    d.set_symmetry(Symmetry {
        enabled: true,
        ..Default::default()
    })
    .unwrap();
    let baseline = d.geometry_snapshot();
    let before_history = d.history_counts();
    let dab = Dab {
        center: Vec3(0.65, 0., 0.),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.6,
        pressure: 0.4,
        delta: Vec3::default(),
        invert: false,
    };
    let brush = Brush::new(Kind::Standard);
    for _ in 0..3 {
        d.apply_dab_stenciled(&brush, dab, Some(&stencil));
    }
    d.end_stroke();
    assert_eq!(d.mesh().triangles, original.triangles);
    assert!(d.mesh().vertices.iter().any(|p| p.2 > 0.));
    for (i, p) in original.vertices.iter().enumerate() {
        if p.0.abs() < 0.6 {
            assert_eq!(d.mesh().vertices[i], *p);
        }
        if p.0 > 0. {
            let other = original
                .vertices
                .iter()
                .position(|q| (q.0 + p.0).abs() < 1e-5 && (q.1 - p.1).abs() < 1e-5)
                .unwrap();
            assert!((d.mesh().vertices[i].2 - d.mesh().vertices[other].2).abs() < 1e-5);
        }
    }
    assert_eq!(d.history_counts().0, before_history.0 + 1);
    let after = d.geometry_snapshot();
    d.undo();
    assert!(d.geometry_snapshot() == baseline);
    d.redo();
    assert!(d.geometry_snapshot() == after);
    // Masked and hidden vertices remain protected for every supported brush.
    for kind in Kind::ALL {
        let mut mesh = original.clone();
        mesh.mask.fill(1.);
        let before = mesh.vertices.clone();
        let mut brush = Brush::new(kind);
        brush.stroke_mode = StrokeMode::DragRect;
        if kind != Kind::Mask {
            mesh.stamp_many_stenciled(&brush, &[dab], std::slice::from_ref(&stencil));
            assert_eq!(mesh.vertices, before);
        }
        let mut mesh = original.clone();
        mesh.hidden_faces = vec![true; mesh.triangles.len()];
        let before = mesh.clone();
        mesh.stamp_many_stenciled(&brush, &[dab], std::slice::from_ref(&stencil));
        assert_eq!(mesh, before);
    }
    let mut low = original.clone();
    let mut high = original.clone();
    low.stamp_many_stenciled(&brush, &[dab], std::slice::from_ref(&stencil));
    high.stamp_many_stenciled(
        &brush,
        &[Dab {
            pressure: 0.8,
            ..dab
        }],
        &[stencil],
    );
    for (l, h) in low.vertices.iter().zip(&high.vertices) {
        assert!((h.2 - l.2 * 2.).abs() < 1e-6);
    }
}

#[test]
fn alpha_black_stencil_creates_no_history_or_adaptive_topology_and_invalid_library_is_atomic() {
    let mut d = Document::default();
    d.set_adaptive(crate::sculpt::AdaptiveSettings {
        enabled: true,
        ..Default::default()
    })
    .unwrap();
    d.mark_saved("test.clay");
    let before = d.geometry_snapshot();
    let history = d.history_counts();
    let dab = Dab {
        center: Vec3(0., 0., 1.),
        normal: Vec3(0., 0., 1.),
        right: Vec3(1., 0., 0.),
        up: Vec3(0., 1., 0.),
        radius: 0.6,
        pressure: 1.,
        delta: Vec3::default(),
        invert: false,
    };
    let stencil = projection(image(vec![0.], 1));
    for kind in Kind::ALL {
        let mut brush = Brush::new(kind);
        brush.stroke_mode = StrokeMode::DragRect;
        assert!(!d.apply_dab_stenciled(&brush, dab, Some(&stencil)));
        d.end_stroke();
        assert!(d.geometry_snapshot() == before);
        assert_eq!(d.history_counts(), history);
        assert!(!d.is_dirty());
    }
    let valid = asset(image(vec![0.5], 1));
    for _ in 0..64 {
        d.save_alpha(None, valid.clone()).unwrap();
    }
    d.mark_saved("test.clay");
    assert!(d.save_alpha(None, valid).is_err());
    assert_eq!(d.alphas().len(), 64);
    assert!(!d.is_dirty());
}
