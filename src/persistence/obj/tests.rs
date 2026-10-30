use super::*;
const SQUARE: &str = "v 2 3 4\nv 4 3 4\nv 4 5 4\nv 2 5 4\n";
#[test]
fn zbrush_terminal_nul_padding_preserves_geometry_and_uvs() {
    let source = format!(
        "# File exported by ZBrush version 2021\n{SQUARE}vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\ng head\nf 1/1 2/2 3/3 4/4"
    );
    let expected = read(&source);
    for suffix in ["\0", "\n\n\0", "\r\n\0\0"] {
        assert_eq!(read(&format!("{source}{suffix}")), expected);
    }
    for source in [
        format!("{SQUARE}\0\nf 1 2 3 4\n"),
        format!("{SQUARE}f 1 2\0 3 4\n"),
        format!("{SQUARE}curv 0 1 1 2\n\0"),
    ] {
        assert!(parse(&source, &|| false).is_err());
    }
}

#[test]
#[ignore = "manual validation of an external OBJ supplied via CLAY_OBJ_REVIEW"]
fn external_obj_manual_review() {
    let path = std::env::var("CLAY_OBJ_REVIEW").expect("Set CLAY_OBJ_REVIEW to an OBJ path");
    let mesh = load_obj(&path).unwrap();
    mesh.validate().unwrap();
    println!(
        "Imported {} vertices, {} quads, {} triangles and {} UV triangle mappings",
        mesh.vertices.len(),
        mesh.quads.len(),
        mesh.triangles.len(),
        mesh.uv.corners.len()
    );
}

#[test]
fn uv_obj_roundtrip_preserves_seams_and_quad_topology() {
    let mesh = read(
        "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 2 0 0\nv 2 1 0\nvt 0 0\nvt 0.4 0\nvt 0.4 1\nvt 0 1\nvt 0.6 0\nvt 1 0\nvt 1 1\nvt 0.6 1\nf 1/1 2/2 3/3 4/4\nf 2/5 5/6 6/7 3/8\n",
    );
    let path = std::env::temp_dir().join(format!("clay-uv-roundtrip-{}.obj", std::process::id()));
    crate::persistence::export_obj(path.to_str().unwrap(), &mesh).unwrap();
    let loaded = load_obj(path.to_str().unwrap()).unwrap();
    assert_eq!(loaded.vertices, mesh.vertices);
    assert_eq!(loaded.quads, mesh.quads);
    assert_eq!(loaded.uv.corners, mesh.uv.corners);
    assert_eq!(loaded.uv.islands().len(), 2);
    let _ = std::fs::remove_file(path);
}
#[test]
fn material_obj_vertex_colors_load_in_linear_light_and_lossy_export_reports_limit() {
    let mesh = read("v 0 0 0 0.5 0.1 1\nv 1 0 0\nv 0 1 0\nf 1 2 3\n");
    assert_eq!(mesh.materials.len(), 3);
    assert!((mesh.materials[0].base_color[0] - 0.21404114).abs() < 1e-6);
    assert!(mesh.materials[1].is_default());
    let path =
        std::env::temp_dir().join(format!("clay-material-export-{}.obj", std::process::id()));
    assert!(
        crate::persistence::export_obj(&path.to_string_lossy(), &mesh)
            .unwrap_err()
            .contains(".clay")
    );
    assert!(!path.exists());
    assert!(parse("v 0 0 0 2 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3", &|| false).is_err());
}
fn read(s: &str) -> Mesh {
    parse(s, &|| false).unwrap()
}
fn signed_area(mesh: &Mesh) -> f32 {
    mesh.triangles
        .iter()
        .map(|&[a, b, c]| {
            (mesh.vertices[b] - mesh.vertices[a])
                .cross(mesh.vertices[c] - mesh.vertices[a])
                .2
                * 0.5
        })
        .sum()
}
#[test]
fn triangles_quads_indices_and_uv_seams_keep_positions_and_winding() {
    let mesh = read(&format!(
        "{SQUARE}vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\ng shell\nf -4/1/1 -3/2/1 -2/3/1 -1/4/1\nf 4//1 3//1 1//1\nf 1/2 2/1 3/4\n"
    ));
    assert_eq!(
        mesh.vertices.len(),
        4,
        "UV and normal indices must not split sculpt vertices"
    );
    assert_eq!(mesh.vertices[0], Vec3(2., 3., 4.));
    assert_eq!(mesh.quads, vec![[0, 1, 2, 3]]);
    assert_eq!(mesh.triangles.len(), 4);
    assert_eq!(signed_area(&mesh), 4.);
    assert_eq!(mesh.mask, vec![0.; 4]);
    assert_eq!(mesh.vertex_groups[0].name, "shell");
    assert_eq!(mesh.vertex_groups[0].weights, vec![1.; 4]);
    mesh.validate().unwrap();
}
#[test]
fn positive_forward_indices_and_negative_indices_use_different_bases() {
    let mesh = read(
        "f 1/1/1 2/2/1 3/3/1\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0\nvt 1\nvt 0.5\nvn 0 0 1\nf -3/-3/-1 -2/-2/-1 -1/-1/-1\nv 9 9 9\n",
    );
    assert_eq!(mesh.triangles.len(), 2);
    assert!(mesh.triangles.iter().flatten().all(|&i| i < 3));
    for suffix in ["f -5 -2 -1", "f 0 2 3", "f 1/4 2/1 3/1", "f 1//2 2//1 3//1"] {
        assert!(
            parse(&format!("{SQUARE}vt 0\nvn 0 0 1\n{suffix}"), &|| false).is_err(),
            "{suffix}"
        );
    }
}
#[test]
fn concave_polygons_and_reverse_winding_triangulate_without_filling_notches() {
    let vertices = "v 0 0 0\nv 3 0 0\nv 3 3 0\nv 1.5 1 0\nv 0 3 0\n";
    let forward = read(&format!("{vertices}f 1 2 3 4 5\n"));
    let reverse = read(&format!("{vertices}f 5 4 3 2 1\n"));
    assert_eq!(forward.triangles.len(), 3);
    assert!(forward.quads.is_empty());
    assert_eq!(signed_area(&forward), 6.);
    assert_eq!(signed_area(&reverse), -6.);
    for (mesh, sign) in [(&forward, 1.), (&reverse, -1.)] {
        assert!(mesh.triangles.iter().all(|&[a, b, c]| {
            (mesh.vertices[b] - mesh.vertices[a])
                .cross(mesh.vertices[c] - mesh.vertices[a])
                .2
                * sign
                > 0.
        }));
    }
    let quad = read("v 0 0 0\nv 2 0 0\nv 0.5 0.5 0\nv 0 2 0\nf 1 2 3 4\n");
    assert_eq!(quad.quads.len(), 1);
    assert_eq!(signed_area(&quad), 1.);
    quad.validate().unwrap();
}
#[test]
fn group_membership_resets_and_overlapping_groups_share_boundary_weights() {
    let mesh = read(&format!(
        "{SQUARE}g left shared\nf 1 2 3\ng right shared\nf 1 3 4\ng\nf 1 4 2\n"
    ));
    assert_eq!(
        mesh.vertex_groups
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        vec!["left", "shared", "right"]
    );
    assert_eq!(mesh.polygroups, vec![1, 3, 0]);
    assert_eq!(mesh.vertex_groups[0].weights, vec![1., 1., 1., 0.]);
    assert_eq!(mesh.vertex_groups[1].weights, vec![1.; 4]);
    assert_eq!(mesh.vertex_groups[2].weights, vec![1., 0., 1., 1.]);
    let long = "a".repeat(200);
    let mesh = read(&format!(
        "{SQUARE}g {long}1 {long}2\nf 1 2 3\ng {long}1\nf 1 3 4"
    ));
    assert_ne!(mesh.vertex_groups[0].name, mesh.vertex_groups[1].name);
    mesh.validate().unwrap();
}
#[test]
fn comments_bom_crlf_continuations_and_polygon_weights_are_supported() {
    let mesh = read(
        "\u{feff}# header\r\nv 0 0 0 2\r\nv 2 0 0 1 0 0\r\nv 0 2 0\r\nmtllib nonexistent.mtl\r\nusemtl red\r\no surface\r\ns off\r\nf 1 \\\r\n 2 3 # end\r\n",
    );
    assert_eq!(mesh.vertices[1], Vec3(2., 0., 0.));
    assert_eq!(signed_area(&mesh), 2.);
}
#[test]
fn invalid_faces_and_unsupported_geometry_are_rejected_with_context() {
    for face in [
        "f 1 2",
        "f 1 1 2",
        "f 1 2 8",
        "f 1/ 2/ 3/",
        "f 1///1 2 3",
        "f 1 3 2 4",
        "l 1 2",
        "curv 0 1 1 2",
        "csh echo test",
        "call other.obj",
    ] {
        let error = parse(&format!("{SQUARE}{face}\n"), &|| false)
            .err()
            .unwrap();
        assert!(error.contains("line 5"), "{error}");
    }
    for source in [
        "v NaN 0 0",
        "v 1e99 0 0",
        "v 0 0",
        "v 0 0 0\nv 1 0 0\nv 2 0 0\nf 1 2 3",
        "v 0 0 0\nf 1 \\",
    ] {
        assert!(parse(source, &|| false).is_err(), "{source}");
    }
    assert!(
        parse(
            "v 0 0 0\nv 2 0 0\nv 2 2 1\nv 1 3 0\nv 0 2 0\nf 1 2 3 4 5",
            &|| false
        )
        .unwrap_err()
        .contains("Non-planar")
    );
}
#[test]
fn size_and_cancellation_limits_apply_before_scene_commit() {
    assert!(
        parse(
            &format!("{}f 1 2 3", "v 0 0 0\n".repeat(max_vertices() + 1)),
            &|| false
        )
        .unwrap_err()
        .contains("vertex limit")
    );
    assert!(
        parse(
            &format!("{SQUARE}{}", "f 1 2 3 4\n".repeat(max_triangles() / 2 + 1)),
            &|| false
        )
        .unwrap_err()
        .contains("triangle limit")
    );
    assert!(
        parse(
            &format!(
                "{SQUARE}{}f 1 2 3",
                (0..65).map(|i| format!("g group{i}\n")).collect::<String>()
            ),
            &|| false
        )
        .unwrap_err()
        .contains("64 vertex groups")
    );
    assert!(
        parse(&format!("{SQUARE}f 1 2 3"), &|| true)
            .unwrap_err()
            .contains("cancelled")
    );
    let calls = std::cell::Cell::new(0);
    assert!(
        parse(&format!("{SQUARE}f 1 2 3 4"), &|| {
            calls.set(calls.get() + 1);
            calls.get() > 6
        })
        .unwrap_err()
        .contains("cancelled")
    );
    let huge_face = format!("{SQUARE}f {}", vec!["1"; MAX_CORNERS + 1].join(" "));
    assert!(parse(&huge_face, &|| false).unwrap_err().contains("3–256"));
}
#[test]
fn exported_quad_mesh_reimports_geometry_and_face_groups() {
    let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    primitive.divisions = [2; 3];
    let mut mesh = primitive.mesh().unwrap();
    mesh.create_vertex_group("Cube sides".into(), 0.5, true)
        .unwrap();
    let path = std::env::temp_dir().join(format!("clay-obj-roundtrip-{}.obj", std::process::id()));
    crate::persistence::export_obj(&path.to_string_lossy(), &mesh).unwrap();
    let imported = load_obj(&path.to_string_lossy()).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(mesh.vertices, imported.vertices);
    assert_eq!(mesh.quads, imported.quads);
    assert_eq!(mesh.triangles.len(), imported.triangles.len());
    assert_eq!(mesh.polygroups, imported.polygroups);
    assert_eq!(
        mesh.vertex_groups[0].weights,
        imported.vertex_groups[0].weights
    );
    assert_eq!(imported.vertex_groups[0].name, "group_1_Cube_sides");
    assert!(signed_area(&imported).abs() < 1e-5); // Closed surface retains balanced winding.
    imported.validate().unwrap();
}
#[test]
fn file_size_limit_is_checked_before_reading() {
    let path = std::env::temp_dir().join(format!("clay-obj-size-{}.obj", std::process::id()));
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_BYTES as u64 + 1).unwrap();
    assert!(
        load_obj(&path.to_string_lossy())
            .unwrap_err()
            .contains("128 MB")
    );
    drop(file);
    std::fs::remove_file(path).unwrap();
}
