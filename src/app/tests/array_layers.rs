use super::*;
use crate::{
    command::InstanceCommand as C,
    sculpt::{Geometry, Vec3, instances::*},
};

fn change(g: &mut Guide, edit: impl FnOnce(&mut Array)) {
    let mut set = g.app.ui.instances.editor.clone().unwrap();
    let Layout::Array(a) = &mut set.layout else {
        unreachable!()
    };
    edit(a);
    g.app.ui.instances.editor = Some(set.clone());
    g.app
        .dispatch(Command::Instances(C::Preview(Box::new(set))), &g.ctx);
    assert!(!g.app.ui.instances.invalid, "{}", g.app.ui.status);
}
#[test]
#[ignore = "requires Vulkan; clicks layered and shared array controls"]
fn layered_array_controls_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-array-layers".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1440, 1000]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Right);
    g.app.ui.preferences.right_width = 355.;
    let mut block = Primitive::for_kind(PrimitiveKind::Cube);
    block.divisions = [1; 3];
    let mut mesh = block.mesh().unwrap();
    for p in &mut mesh.vertices {
        *p = Vec3(p.0 * 0.115, p.1 * 0.065, p.2 * 0.05);
    }
    mesh.materials =
        vec![crate::sculpt::Material::from_srgb([0.28, 0.38, 0.45], 0.5, 0.2); mesh.vertices.len()];
    let mut cap = block.mesh().unwrap();
    for p in &mut cap.vertices {
        *p = Vec3(p.0 * 0.06 - 0.015, p.1 * 0.043, p.2 * 0.024 + 0.061);
    }
    cap.materials =
        vec![crate::sculpt::Material::from_srgb([0.83, 0.57, 0.26], 0.3, 0.2); cap.vertices.len()];
    let mut geometry = Geometry::new(mesh.merged_with(&cap).unwrap());
    geometry.name = "Hull fitting".into();
    geometry.scene.active_metadata.visible = false;
    g.app.document.replace_geometry(geometry).unwrap();
    g.app.camera.yaw = 0.24;
    g.app.camera.pitch = -0.32;
    click_guide_label(&mut g, "Linked copy…");
    change(&mut g, |a| {
        assert_eq!(a.count, 1);
        a.rotation = Vec3(10., 0., 20.);
        a.size = Vec3(2., 1., 1.);
    });
    click_guide_label(&mut g, "Cancel preview");
    assert!(g.app.document.instances().sets.is_empty());
    g.app
        .dispatch(Command::Instances(C::NewLegacyArray), &g.ctx);
    g.frame(vec![]);
    change(&mut g, |a| {
        a.count = 5;
        a.spacing = Vec3(0.34, 0., 0.);
        a.center = Vec3(-0.72, -0.55, 0.);
    });
    click_guide_label(&mut g, "Add layer");
    assert_eq!(g.app.ui.instances.array_layer, 1);
    change(&mut g, |a| {
        let l = &mut a.layers[0];
        l.count = 4;
        l.spacing = Vec3(0., 0.31, 0.06);
        l.step_rotation = Vec3(0., 0., 8.);
        l.step_scale = Vec3(0.93, 1.1, 1.);
    });
    click_guide_label(&mut g, "Per-repeat transforms");
    g.save("20-layered-array-xyz");
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 20);
    click_guide_label(&mut g, "Layer 1");
    change(&mut g, |a| a.count = 4);
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 16);
    click_guide_label(&mut g, "Layer 2");
    click_guide_label(&mut g, "Remove layer");
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 4);
    g.app.ui.instances.editor.as_mut().unwrap().name = "Hull fitting row".into();
    click_guide_label(&mut g, "Show instances");
    click_guide_label(&mut g, "Apply instances");
    click_guide_label(&mut g, "Repeat this array…");
    change(&mut g, |a| {
        assert_eq!(a.source_array, Some(1));
        a.kind = ArrayKind::Radial;
        a.count = 6;
        a.center = Vec3::default();
        a.radius = 0.32;
        a.rotation = Vec3(12., 0., 6.);
        a.size = Vec3(1., 0.88, 1.1);
    });
    click_guide_label(&mut g, "Whole layer transform");
    g.app.camera.zoom = 0.76;
    g.save("21-linked-array-of-rows");
    click_guide_label(&mut g, "Apply instances");
    let link = g.app.document.instances().sets[1].clone();
    click_guide_label(&mut g, "Edit source array…");
    change(&mut g, |a| a.count = 3);
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 18);
    click_guide_label(&mut g, "Apply instances");
    assert_eq!(g.app.document.instances().sets[1], link);
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 24);
    g.app.dispatch(Command::Redo, &g.ctx);
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 18);
    let model = g.app.document.model(0).unwrap();
    let name = model.name.to_owned();
    let mut metadata = model.metadata.clone();
    metadata.visible = true;
    g.app
        .dispatch(Command::EditSubmodel(0, name.clone(), metadata), &g.ctx);
    let before = g.app.document.geometry_snapshot();
    click_guide_label(&mut g, "Bake all…");
    click_guide_label(&mut g, &format!("Merge into selected submodel: {name}"));
    g.save("bake-all-destinations");
    click_guide_label(&mut g, "Bake now");
    finish_mesh_job(&mut g.app);
    assert_eq!(g.app.document.model_count(), 1);
    assert_eq!(
        g.app.document.mesh().triangles.len(),
        before.mesh.triangles.len() * 19
    );
    assert_eq!(
        g.app.document.instances().sets.len(),
        1,
        "the hidden source array is retained"
    );
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), before);
}
