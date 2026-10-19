use super::*;
use crate::{document::Document, persistence::load_project, sculpt::*};

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "clay-diagnostics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn document() -> Document {
    let mut doc = Document::default();
    let project =
        serde_json::from_str(include_str!("../../tests/fixtures/v1-project.json")).unwrap();
    doc.replace_project(project, "/private/person/source.clay")
        .unwrap();
    doc
}
fn stroke(doc: &mut Document, strength: f32) {
    let mut brush = Brush::new(Kind::Standard);
    brush.strength = strength;
    assert!(doc.apply_dab(
        &brush,
        Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 2.,
            pressure: 0.65,
            delta: Vec3::default(),
            invert: false,
        }
    ));
    doc.end_stroke();
}
#[test]
fn session_roundtrip_preserves_stages_settings_and_live_undo_redo() {
    let temp = Temp::new();
    let mut doc = document();
    let original = doc.project();
    doc.set_diagnostic_context(serde_json::json!({"selected_brush": 0, "camera": {"zoom": 2.}}));
    stroke(&mut doc, 0.2);
    let first = doc.project();
    let mut symmetry = doc.symmetry().clone();
    symmetry.enabled = true;
    doc.set_symmetry(symmetry.clone()).unwrap();
    doc.set_remesh_settings(32, false).unwrap();
    stroke(&mut doc, 0.7);
    let second = doc.project();
    let mut geometry = doc.geometry_snapshot();
    geometry.subdivide().unwrap();
    doc.replace_geometry_named(geometry, "Subdivide mesh")
        .unwrap();
    let third = doc.project();
    doc.undo();
    doc.redo();
    assert!(doc.project() == third);
    doc.undo(); // Export with both undo and redo stages retained.
    let live = doc.project();
    let counts = doc.history_counts();
    let dirty = doc.is_dirty();
    let folder = temp.0.join("bundle");
    doc.export_diagnostics(&folder, Environment::default())
        .unwrap();
    assert!(doc.project() == live);
    assert_eq!(doc.history_counts(), counts);
    assert_eq!(doc.is_dirty(), dirty);
    assert_eq!(doc.current_path(), "/private/person/source.clay");
    let manifest = load_manifest(&folder).unwrap();
    assert_eq!(manifest.current_stage, 2);
    assert_eq!(manifest.stages.len(), 4);
    assert_eq!(
        manifest
            .transitions
            .iter()
            .map(|t| (t.from, t.to, t.id))
            .collect::<Vec<_>>(),
        vec![(0, 1, 1), (1, 2, 2), (2, 3, 3)]
    );
    assert_eq!(manifest.transitions[2].label, "Subdivide mesh");
    let first_settings = manifest.transitions[0]
        .settings_at_edit_start
        .as_ref()
        .unwrap();
    let second_settings = manifest.transitions[1]
        .settings_at_edit_start
        .as_ref()
        .unwrap();
    assert_eq!(first_settings.active_brush.as_ref().unwrap().strength, 0.2);
    assert_eq!(second_settings.active_brush.as_ref().unwrap().strength, 0.7);
    assert!(!first_settings.symmetry.enabled);
    assert_eq!(second_settings.symmetry, symmetry);
    assert_eq!(first_settings.topology.resolution, 48);
    assert_eq!(second_settings.topology.resolution, 32);
    assert_eq!(
        first_settings.first_dab.as_ref().unwrap()["pressure"]
            .as_f64()
            .unwrap() as f32,
        0.65
    );
    let stages = manifest
        .stages
        .iter()
        .map(|s| load_project(folder.join(&s.file).to_str().unwrap()).unwrap())
        .collect::<Vec<_>>();
    for (saved, expected) in stages.iter().zip([&original, &first, &second, &third]) {
        assert_eq!(saved.mesh, expected.mesh);
        assert_eq!(saved.submodels, expected.submodels);
        let mut restored = Document::default();
        restored
            .replace_project(saved.clone(), "restored.clay")
            .unwrap();
        assert_eq!(restored.mesh(), &expected.mesh);
    }
    assert!(stages[2] == live);
    assert_eq!(stages[3].mesh_settings, third.mesh_settings);
    let summary = std::fs::read_to_string(folder.join("README.txt")).unwrap();
    assert!(summary.contains("Subdivide mesh"));
    assert!(summary.contains("unavailable"));
    let encoded = std::fs::read_to_string(folder.join("manifest.json")).unwrap();
    assert!(!encoded.contains("/private/person"));
    doc.redo();
    assert!(doc.project() == third);
    doc.undo();
    assert!(doc.project() == live);
}
#[test]
fn bounded_retention_counts_evictions_and_discarded_branches_and_survives_new() {
    let mut doc = document();
    doc.set_history_retention(Retention {
        edits: 2,
        memory_mib: 16,
    })
    .unwrap();
    for _ in 0..4 {
        stroke(&mut doc, 0.2);
    }
    let m = doc.diagnostic_manifest(Environment::default());
    assert_eq!(m.evicted_edits, 2);
    assert_eq!(m.transitions[0].id, 3);
    doc.undo();
    stroke(&mut doc, 0.3);
    let m = doc.diagnostic_manifest(Environment::default());
    assert_eq!(m.discarded_redo_edits, 1);
    assert_eq!(m.transitions[1].id, 5);
    doc.new_sphere();
    assert_eq!(doc.history_retention().edits, 2);
    assert_eq!(doc.history_counts(), (0, 0));
    assert_eq!(
        doc.diagnostic_manifest(Environment::default())
            .evicted_edits,
        0
    );
    assert!(
        doc.set_history_retention(Retention {
            edits: 257,
            memory_mib: 16
        })
        .is_err()
    );
}
#[test]
fn supported_geometry_includes_submodels_levels_masks_visibility_groups_and_creases() {
    let temp = Temp::new();
    let mut doc = document();
    let mut g = doc.geometry_snapshot();
    g.mesh
        .create_vertex_group("Region".into(), 0.5, true)
        .unwrap();
    g.mesh.creases.push([0, 1]);
    g.mesh.hidden_faces = vec![true];
    g.submodels.push(Submodel {
        name: "Other".into(),
        mesh: g.mesh.clone(),
        mesh_settings: MeshSettings::default(),
        metadata: ModelMetadata::default(),
    });
    g.subdivide().unwrap();
    doc.replace_geometry_named(g, "Topology and attributes")
        .unwrap();
    let current = doc.project();
    doc.undo();
    doc.export_diagnostics(&temp.0.join("bundle"), Environment::default())
        .unwrap();
    let restored = load_project(temp.0.join("bundle/stage-001.clay").to_str().unwrap()).unwrap();
    assert!(restored == current);
}
#[test]
fn export_refuses_existing_destinations_and_active_edits_without_changes() {
    let temp = Temp::new();
    let mut doc = document();
    let sentinel = temp.0.join("sentinel");
    std::fs::write(&sentinel, "keep me").unwrap();
    assert!(
        doc.export_diagnostics(&temp.0, Environment::default())
            .is_err()
    );
    assert!(
        doc.export_diagnostics(&sentinel, Environment::default())
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "keep me");
    doc.begin_stroke();
    assert!(
        doc.export_diagnostics(&temp.0.join("active"), Environment::default())
            .is_err()
    );
    assert!(!temp.0.join("active").exists());
    assert!(doc.edit_in_progress());
}
#[test]
fn manifest_rejects_future_schemas_broken_order_and_paths() {
    let doc = document();
    let mut m = doc.diagnostic_manifest(Environment::default());
    m.schema_version += 1;
    assert!(m.validate().is_err());
    m.schema_version = SCHEMA_VERSION;
    m.stages[0].file = "../outside.clay".into();
    assert!(m.validate().is_err());
    m.stages[0].file = "stage-000.clay".into();
    m.current_stage = 1;
    assert!(m.validate().is_err());
}
#[test]
fn streaming_output_enforces_aggregate_budget_without_large_allocations() {
    let temp = Temp::new();
    let mut writer = BundleWriter { remaining: 16 };
    writer.text(&temp.0.join("first"), "1234567890").unwrap();
    assert_eq!(writer.remaining, 6);
    assert!(writer.text(&temp.0.join("second"), "1234567").is_err());
    assert!(std::fs::metadata(temp.0.join("second")).unwrap().len() <= 6);
    assert_eq!(
        std::fs::read_to_string(temp.0.join("first")).unwrap(),
        "1234567890"
    );
}

#[test]
fn failed_export_cleans_partial_new_bundle_and_preserves_siblings_and_document() {
    let temp = Temp::new();
    let mut doc = document();
    stroke(&mut doc, 0.2);
    doc.undo();
    let before = doc.project();
    let counts = doc.history_counts();
    std::fs::write(temp.0.join("keep.clay"), "existing").unwrap();
    let folder = temp.0.join("incomplete");
    // Enough for the summary, insufficient for the complete manifest and projects.
    assert!(
        doc.export_diagnostics_bounded(&folder, Environment::default(), 4096)
            .is_err()
    );
    assert!(!folder.exists());
    assert_eq!(
        std::fs::read_to_string(temp.0.join("keep.clay")).unwrap(),
        "existing"
    );
    assert!(doc.project() == before);
    assert_eq!(doc.history_counts(), counts);
    assert!(doc.can_redo());
}

#[test]
fn all_redo_stages_remain_in_edit_order_and_current_settings_are_separate() {
    let temp = Temp::new();
    let mut doc = document();
    let mut expected = vec![doc.mesh().clone()];
    for strength in [0.2, 0.4, 0.6] {
        stroke(&mut doc, strength);
        expected.push(doc.mesh().clone());
    }
    for _ in 0..3 {
        doc.undo();
    }
    doc.set_symmetry(Symmetry {
        enabled: true,
        ..Default::default()
    })
    .unwrap();
    let folder = temp.0.join("redo");
    doc.export_diagnostics(&folder, Default::default()).unwrap();
    let m = load_manifest(&folder).unwrap();
    assert_eq!(m.current_stage, 0);
    assert_eq!(doc.history_counts(), (0, 3));
    assert!(m.current_settings.symmetry.enabled);
    assert!(
        m.transitions
            .iter()
            .all(|t| !t.settings_at_edit_start.as_ref().unwrap().symmetry.enabled)
    );
    for (stage, mesh) in m.stages.iter().zip(expected) {
        assert_eq!(
            load_project(folder.join(&stage.file).to_str().unwrap())
                .unwrap()
                .mesh,
            mesh
        );
    }
    assert_eq!(
        m.transitions.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[test]
fn only_the_current_stage_contains_current_reference_images() {
    let temp = Temp::new();
    let mut doc = document();
    stroke(&mut doc, 0.2);
    doc.add_reference(crate::reference::tests::image()).unwrap();
    doc.export_diagnostics(&temp.0.join("references"), Default::default())
        .unwrap();
    let current = load_project(temp.0.join("references/stage-001.clay").to_str().unwrap()).unwrap();
    let earlier = load_project(temp.0.join("references/stage-000.clay").to_str().unwrap()).unwrap();
    assert!(current == doc.project());
    assert_eq!(current.references.len(), 1);
    assert!(earlier.references.is_empty());
}
