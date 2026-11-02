use super::*;
use crate::{
    command::PendingAction,
    sculpt::{
        Geometry, Mesh, Vec3,
        instances::{Layout, Source},
    },
};

fn plate(extent: Vec3, offset: Vec3) -> Mesh {
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [1; 3];
    let mut mesh = cube.mesh().unwrap();
    for p in &mut mesh.vertices {
        *p = Vec3(p.0 * extent.0, p.1 * extent.1, p.2 * extent.2) + offset;
    }
    mesh
}
#[test]
#[ignore = "requires Vulkan; creates a personal multi-object brush and paints two different parts"]
fn object_kit_library_and_strokes_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-object-kits".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let personal = directory.join(format!("test-profile-{}", std::process::id()));
    let mut g = Guide::new(&directory, [1440, 1000]);
    g.interactive = true;
    g.panel(Tool::Library, Dock::Right);
    g.panel(Tool::Brush, Dock::Left);
    g.app.ui.preferences.menus.clear();
    g.app
        .ui
        .preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == Tool::Library)
        .unwrap()
        .open = true;
    g.app.ui.preferences.right_width = 345.;
    g.app.preferences_path = Some(personal.join("preferences.json"));
    let mut vent = plate(Vec3(0.15, 0.095, 0.02), Vec3::default());
    vent.materials =
        vec![crate::sculpt::Material::from_srgb([0.23, 0.27, 0.33], 0.5, 0.2); vent.vertices.len()];
    for i in -2..=2 {
        let mut slat = plate(Vec3(0.009, 0.077, 0.015), Vec3(i as f32 * 0.05, 0., 0.034));
        slat.materials = vec![
            crate::sculpt::Material::from_srgb([0.66, 0.73, 0.79], 0.4, 0.15);
            slat.vertices.len()
        ];
        vent = vent.merged_with(&slat).unwrap();
    }
    let mut geometry = Geometry::new(vent);
    geometry.name = "Hull vent".into();
    g.app.document.replace_geometry(geometry).unwrap();
    g.app.camera.zoom = 3.;
    g.app.camera.yaw = 0.32;
    g.app.camera.pitch = -0.28;
    click_guide_label(&mut g, "Add brush");
    click_guide_label(&mut g, "Object brush");
    assert!(
        g.app
            .viewport
            .object_brush
            .as_ref()
            .unwrap()
            .piece
            .is_none()
    );
    g.frame(vec![]);
    let name = g
        .ctx
        .read_response(egui::Id::new(("object_brush_name", 1u64)))
        .unwrap()
        .rect
        .center();
    g.frame(vec![
        egui::Event::PointerMoved(name),
        primary_event(name, true),
        primary_event(name, false),
    ]);
    let key = |key, modifiers| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    g.frame(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        egui::Event::Text("MyOB".into()),
    ]);
    g.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert_eq!(g.app.document.instances().brushes[0].name, "MyOB");
    assert_eq!(g.app.ui.instances.group, Some(1), "{}", g.app.ui.status);
    click_guide_label(&mut g, "Use base centre");
    g.save("17-object-capture-orientation");
    for label in ["Contact", "Forward +X", "Up +Z"] {
        assert!(
            g.text_positions.iter().any(|(text, _)| text == label),
            "{label} visible on the model"
        );
    }
    click_guide_label(&mut g, "Capture active model");
    assert_eq!(
        g.app.document.instances().assets[0].pivot,
        Vec3(0., 0., -0.02)
    );
    // The second part has a different source-space Up axis.
    let mut dome = Primitive::for_kind(PrimitiveKind::UvSphere).mesh().unwrap();
    for p in &mut dome.vertices {
        *p = Vec3(p.0 * 0.10, p.1 * 0.052 - 0.028, p.2 * 0.10);
    }
    dome.materials = vec![
        crate::sculpt::Material::from_srgb([0.86, 0.56, 0.26], 0.3, 0.12);
        dome.vertices.len()
    ];
    let mut mount = Primitive::for_kind(PrimitiveKind::Cylinder).mesh().unwrap();
    for p in &mut mount.vertices {
        *p = Vec3(p.0 * 0.11, p.1 * 0.012 - 0.072, p.2 * 0.11);
    }
    mount.materials = vec![
        crate::sculpt::Material::from_srgb([0.38, 0.44, 0.51], 0.45, 0.2);
        mount.vertices.len()
    ];
    dome = mount.merged_with(&dome).unwrap();
    let mut geometry = g.app.document.geometry_snapshot();
    geometry.mesh = dome;
    geometry.name = "Sensor dome".into();
    g.app.document.replace_geometry(geometry).unwrap();
    g.app.ui.instances.up = 1;
    click_guide_label(&mut g, "Use base centre");
    click_guide_label(&mut g, "Capture active model");
    assert_eq!(g.app.document.instances().brushes[0].objects, vec![1, 2]);
    assert!(g.app.document.instances().assets[1].pivot.1 < -0.07);
    click_guide_label(&mut g, "Capture a model into this brush");

    let mut geometry = g.app.document.geometry_snapshot();
    geometry.mesh = Mesh::starting_sphere();
    geometry.name = "Practice hull".into();
    g.app.document.replace_geometry(geometry).unwrap();
    g.app.camera = Default::default();
    click_guide_label(&mut g, "Clay");
    click_guide_label(&mut g, "MyOB");
    assert_eq!(
        g.app
            .viewport
            .object_brush
            .as_ref()
            .unwrap()
            .piece
            .unwrap()
            .source,
        Source::Asset(1)
    );
    g.app.ui.instances.brush.scale = 1.;
    g.app.ui.instances.brush.spacing = 0.38;
    let rect = g.frame(vec![]).0;
    let centre = rect.center();
    g.stroke(
        centre + egui::vec2(-150., -45.),
        centre + egui::vec2(150., -45.),
    );
    let first = g.app.document.instances().sets[0].clone();
    click_guide_label(&mut g, "Choose model…");
    assert!(g.app.ui.customization.popup_menu.is_some());
    // Choose from the popup with the library hidden, as on a compact workspace.
    g.panel(Tool::Library, Dock::Hidden);
    g.frame(vec![]);
    g.save("22-object-brush-picker");
    click_guide_label(&mut g, "Sensor dome");
    assert!(g.app.ui.customization.popup_menu.is_none());
    g.panel(Tool::Library, Dock::Right);
    g.frame(vec![]);

    assert_eq!(
        g.app
            .viewport
            .object_brush
            .as_ref()
            .unwrap()
            .piece
            .unwrap()
            .source,
        Source::Asset(2)
    );
    g.app.ui.instances.brush.scale = 0.95;
    g.app.ui.instances.brush.spacing = 0.27;
    g.stroke(
        centre + egui::vec2(-150., 52.),
        centre + egui::vec2(150., 52.),
    );
    assert_eq!(g.app.document.instances().sets[0], first);
    for (i, set) in g.app.document.instances().sets.iter().enumerate() {
        let Layout::Manual(items) = &set.layout else {
            panic!()
        };
        assert!(
            items.len() >= 3
                && items
                    .iter()
                    .all(|p| p.piece.source == Source::Asset(i as u64 + 1))
        );
    }
    assert_eq!(g.app.document.instances().sets.len(), 2);
    g.app.camera.yaw = 0.16;
    g.app.camera.pitch = -0.16;
    g.save("18-ship-fittings-strokes");
    click_guide_label(&mut g, "Clay");
    assert!(g.app.viewport.object_brush.is_none());
    click_guide_label(&mut g, "MyOB");
    g.frame(vec![]);
    g.frame(vec![]);
    click_guide_label(&mut g, "Reuse in other projects");
    click_guide_label(&mut g, "Save brush to my library");
    assert_eq!(g.app.ui.instances.personal_brushes, vec!["MyOB"]);
    g.save("19-ship-fittings-library");
    g.app.ui.select_brush(0, &g.app.document);
    g.frame(vec![]);
    g.app.document.project().validate().unwrap();
    g.app.execute_pending(PendingAction::New, &g.ctx);
    click_guide_label(&mut g, "Add brush");
    click_guide_label(&mut g, "Saved object brushes");
    click_guide_label(&mut g, "MyOB");
    let library = g.app.document.instances();
    assert_eq!(library.brushes.len(), 1);
    assert_eq!(library.brushes[0].name, "MyOB");
    assert_eq!(library.assets[0].options.spacing, 0.38);
    assert_eq!(library.assets[1].up, 1);
    assert_eq!(library.assets[1].options.spacing, 0.27);
    assert!(library.sets.is_empty());
    check_primitive_handles(&mut g);
    g.app.ui.diagnostics_open = false;
    let prior = g.app.document.instance_placements().unwrap();
    let captured = g.app.document.instances().assets.len();
    click_guide_label(&mut g, "Remove model from brush");
    assert_eq!(g.app.document.instances().brushes[0].objects, vec![2]);
    assert_eq!(g.app.document.instances().assets.len(), captured);
    assert_eq!(g.app.document.instance_placements().unwrap(), prior);
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.instances().brushes[0].objects, vec![1, 2]);
    std::fs::remove_dir_all(personal).unwrap();
}

fn check_primitive_handles(g: &mut Guide) {
    let populated = g.app.ui.instances.group.unwrap();
    g.app
        .dispatch(Command::Action(Action::NewBrush("Object".into())), &g.ctx);
    g.app.dispatch(Command::RequestPrimitive, &g.ctx);
    g.app.dispatch(Command::ConfirmPrimitive, &g.ctx);
    let cone = Primitive::for_kind(PrimitiveKind::Cone);
    g.app.dispatch(Command::UpdatePrimitive(cone), &g.ctx);
    g.app.camera = Default::default();
    g.frame(vec![]);
    let (rect, _) = g.frame(vec![]);
    let scale = rect.width().min(rect.height()) * 0.35;

    // An empty brush and a brush containing models must both yield to a shape handle.
    for group in [g.app.ui.instances.group.unwrap(), populated] {
        g.app.dispatch(
            Command::Instances(crate::command::InstanceCommand::ChooseBrush(group)),
            &g.ctx,
        );
        let radius = g.app.document.primitive().unwrap().top_radius;
        g.frame(vec![egui::Event::PointerMoved(rect.center())]);
        assert!(
            g.text_positions
                .iter()
                .any(|(text, _)| text.starts_with("Sides · ")),
            "primitive controls must remain visible over the surface"
        );
        let handle = rect.center() + egui::vec2(radius * scale + 16., -scale);
        g.frame(vec![egui::Event::PointerMoved(handle)]);
        assert!(
            g.text_positions
                .iter()
                .any(|(text, pos)| text == "Top radius" && rect.contains(*pos))
        );
        g.frame(vec![primary_event(handle, true)]);
        let end = handle + egui::vec2(28., 0.);
        g.frame(vec![egui::Event::PointerMoved(end)]);
        g.frame(vec![primary_event(end, false)]);
        assert!(g.app.document.primitive().unwrap().top_radius > radius);
        assert!(g.app.document.instances().sets.is_empty());
        assert_eq!(g.app.ui.instances.group, Some(group));
    }
    g.save("object-brush-cone-handles");
    // A stroke that starts on the surface owns its release, even over a handle.
    let radius = g.app.document.primitive().unwrap().top_radius;
    let handle = rect.center() + egui::vec2(radius * scale + 16., -scale);
    g.frame(vec![
        egui::Event::PointerMoved(rect.center()),
        primary_event(rect.center(), true),
        egui::Event::PointerMoved(handle),
        primary_event(handle, false),
    ]);
    assert_eq!(g.app.document.instances().sets.len(), 1);
    assert_eq!(g.app.document.primitive().unwrap().top_radius, radius);

    let previous = g.app.ui.status.clone();
    g.app.ui.diagnostics_open = true;
    g.frame(vec![]);
    assert_eq!(
        g.app.ui.status, previous,
        "idle primitive controls changed the status while a dialog was open"
    );
}

#[test]
#[ignore = "requires Vulkan; verifies object stamps raise and indent an existing sculpt surface"]
fn object_stamp_controls_offscreen_review() {
    use crate::command::InstanceCommand as C;
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-object-stamps".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1440, 1000]);
    g.interactive = true;
    g.panel(Tool::Brush, Dock::Right);
    g.app
        .document
        .replace_geometry(Geometry::new(
            Primitive::for_kind(PrimitiveKind::Cone).mesh().unwrap(),
        ))
        .unwrap();
    g.app.dispatch(
        Command::Instances(C::CreateBrush("Relief shapes".into())),
        &g.ctx,
    );
    g.app.dispatch(
        Command::Instances(C::Capture("Cone".into(), Vec3(0., -1., 0.), 0, 1)),
        &g.ctx,
    );
    let mut target = g.app.document.geometry_snapshot();
    let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
    plane.divisions = [128, 128, 1];
    target.mesh = plane.mesh().unwrap();
    target.name = "Relief surface".into();
    g.app.document.replace_geometry(target).unwrap();
    g.app
        .dispatch(Command::Instances(C::ChooseBrush(1)), &g.ctx);
    g.app.ui.preferences.right_width = 330.;
    g.app.ui.instances.brush.scale = 0.34;
    g.app.camera.pitch = -0.38;
    let (rect, _) = g.frame(vec![]);
    click_guide_label(&mut g, "Stamp mesh");
    assert!(g.app.ui.instances.brush.stamp);
    let before = g.app.document.geometry_snapshot();
    let left = rect.center() + egui::vec2(-110., 0.);
    let orange_pixels = |rgba: &[u8]| {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .filter(|p| {
                (p[0] as i16 - 255).abs() < 8
                    && (p[1] as i16 - 140).abs() < 8
                    && (p[2] as i16 - 100).abs() < 8
            })
            .count()
    };
    let (_, normal) = g.frame(vec![egui::Event::PointerMoved(left)]);
    let normal_orange = orange_pixels(&normal);
    g.frame(vec![
        egui::Event::PointerMoved(left),
        primary_event(left, true),
    ]);
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.save("object-stamp-raised-preview");
    g.frame(vec![primary_event(left, false)]);
    assert!(
        g.app.document.mesh().vertices.iter().any(|p| p.2 > 0.5),
        "{}",
        g.app.ui.status
    );
    let raised = g.app.document.geometry_snapshot();
    let right = rect.center() + egui::vec2(110., 0.);
    g.modifiers = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    let (_, inward) = g.frame(vec![egui::Event::PointerMoved(right)]);
    assert!(
        orange_pixels(&inward) > normal_orange + 25,
        "Alt changes the actual stamp outline to the shared orange"
    );
    g.save("object-stamp-alt-orange");
    g.frame(vec![
        egui::Event::PointerMoved(right),
        egui::Event::PointerButton {
            pos: right,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: g.modifiers,
        },
    ]);
    g.save("object-stamp-inward-preview");
    g.frame(vec![
        egui::Event::PointerButton {
            pos: right,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: g.modifiers,
        },
        egui::Event::PointerGone,
    ]);
    g.modifiers = Default::default();
    assert!(
        g.app.document.mesh().vertices.iter().any(|p| p.2 < -0.5),
        "{}",
        g.app.ui.status
    );
    assert!(g.app.document.instances().sets.is_empty());
    assert_eq!(g.app.document.model_count(), 1);
    g.save("object-stamps-raised-and-inward");
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), raised);
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), before);
    click_guide_label(&mut g, "Create instance");
    assert!(!g.app.ui.instances.brush.stamp);
}

#[test]
#[ignore = "requires Vulkan; verifies the built-in object brush tile in a fresh project"]
fn ship_greebles_default_library_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-ship-greebles".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1440, 1000]);
    g.interactive = true;
    g.panel(Tool::Library, Dock::Right);
    g.app
        .ui
        .preferences
        .panels
        .iter_mut()
        .find(|p| p.tool == Tool::Library)
        .unwrap()
        .open = true;
    g.app.ui.preferences.right_width = 340.;
    let before = g.app.document.geometry_snapshot();
    g.save("ship-greebles-default-tile");
    assert_eq!(
        g.app.document.geometry_snapshot(),
        before,
        "displaying a built-in brush keeps the scene intact"
    );
    assert!(!g.app.document.is_dirty());
    click_guide_label(&mut g, "Ship greebles");
    g.save("ship-greebles-default-selected");
    assert_eq!(g.app.document.instances().assets.len(), 3);
    assert_eq!(g.app.document.instances().brushes.len(), 1);
    assert!(g.app.viewport.object_brush.is_some());
    assert!(
        g.app
            .ui
            .preferences
            .panels
            .iter()
            .find(|p| p.tool == Tool::Brush)
            .unwrap()
            .open
    );
    let (_, pixels) = g.frame(vec![]);
    assert!(!pixels.is_empty());
    let count = g
        .text_positions
        .iter()
        .filter(|(s, _)| s == "Ship greebles")
        .count();
    assert!(count <= 3, "the imported brush replaces the built-in tile");
    g.app.dispatch(Command::Undo, &g.ctx);
    g.save("ship-greebles-default-after-undo");
    assert_eq!(g.app.document.geometry_snapshot(), before);
    click_guide_label(&mut g, "Ship greebles");
    assert_eq!(g.app.document.instances().assets.len(), 3);
}
