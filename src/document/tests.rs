//! Document invariants, compatibility, and transactional editing tests.
use super::*;

#[cfg(test)]
mod core_tests {
    use super::*;
    use crate::sculpt::Vec3;
    #[test]
    fn unnamed_vertex_groups_get_unique_names_and_keep_explicit_names() {
        let mut d = Document::default();
        d.geometry.mesh.mask.fill(1.);
        d.create_vertex_group("".into(), false).unwrap();
        d.create_vertex_group("  ".into(), false).unwrap();
        d.create_vertex_group(" Group 3 ".into(), false).unwrap();
        d.create_vertex_group("".into(), false).unwrap();
        assert_eq!(
            d.mesh()
                .vertex_groups
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>(),
            ["Base", "Group 1", "Group 2", "Group 3", "Group 4"]
        );
        let before = d.geometry_snapshot();
        assert!(d.create_vertex_group("Group 1".into(), false).is_err());
        assert_eq!(d.geometry_snapshot(), before);
        d.undo();
        assert_eq!(d.mesh().vertex_groups.len(), 4);
        d.redo();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn starting_sphere_has_a_group_and_visibility_is_saved_and_undoable() {
        let mut d = Document::default();
        assert_eq!(d.mesh().vertex_groups[0].name, "Base");
        assert!(d.mesh().vertex_groups[0].weights.iter().all(|&w| w == 1.));
        assert!(d.mesh().polygroups.iter().all(|&id| id == 1));
        assert!(!d.is_dirty());
        d.geometry.subdivide().unwrap();
        let before = d.geometry_snapshot();
        d.set_vertex_group_visible(0, false).unwrap();
        assert!(!d.mesh().vertex_groups[0].visible);
        assert!(!d.mesh_settings().lower[0].vertex_groups[0].visible);
        assert_eq!(d.mesh().vertices, before.mesh.vertices);
        let json = serde_json::to_string(&d.project()).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        loaded.validate().unwrap();
        assert!(!loaded.mesh.vertex_groups[0].visible);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert!(!d.mesh().vertex_groups[0].visible);
        d.new_sphere();
        assert!(d.mesh().vertex_groups[0].visible);
        assert_eq!(d.mesh().vertex_groups.len(), 1);
        assert!(!d.can_undo());
    }
    #[test]
    fn mask_threshold_controls_grouping_and_deletion_per_submodel() {
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.mask.fill(0.6);
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        d.set_mask_threshold(0.8).unwrap();
        assert!(!d.delete_masked().unwrap());
        assert!(d.create_vertex_group("Surface".into(), false).is_err());
        d.set_mask_threshold(0.6).unwrap();
        d.end_edit();
        d.create_vertex_group("Surface".into(), false).unwrap();
        assert_eq!(d.mesh().polygroups, vec![1; 2]);
        let before = d.geometry_snapshot();
        assert!(d.delete_masked().unwrap());
        assert!(d.mesh().vertices.is_empty());
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.duplicate_model(false).unwrap();
        d.set_mask_threshold(0.9).unwrap();
        assert_eq!(d.model(0).unwrap().metadata.mask_threshold, 0.9);
        assert_eq!(d.model(1).unwrap().metadata.mask_threshold, 0.6);
        d.project().validate().unwrap();
    }
    #[test]
    fn intensity_preserves_soft_ratios_and_undo_restores_the_mask() {
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.mask = vec![0., 0.2, 0.4, 0.8];
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        let before = d.geometry_snapshot();
        d.set_mask_intensity(0.4).unwrap();
        assert!(
            d.mesh()
                .mask
                .iter()
                .zip([0., 0.1, 0.2, 0.4])
                .all(|(&a, b)| (a - b).abs() < 1e-6)
        );
        assert_eq!(d.mask_peak(), 0.4);
        d.set_mask_intensity(1.).unwrap();
        assert!(
            d.mesh()
                .mask
                .iter()
                .zip([0., 0.25, 0.5, 1.])
                .all(|(&a, b)| (a - b).abs() < 1e-6)
        );
        assert_eq!(d.mask_peak(), 1.);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn groups_and_mask_intensity_are_independent_undoable_and_survive_levels() {
        let mut geometry = Geometry::new(
            crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube)
                .mesh()
                .unwrap(),
        );
        geometry.subdivide().unwrap();
        geometry.mesh.mask[geometry.settings.lower[0].vertices.len()] = 0.8;
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        d.create_vertex_group("Fine detail".into(), false).unwrap();
        let grouped = d.geometry_snapshot();
        assert_eq!(grouped.submodels, before.submodels);
        assert!(
            grouped.settings.lower[0].vertex_groups[0]
                .weights
                .iter()
                .all(|&w| w == 0.)
        );
        assert_eq!(
            grouped.mesh.vertex_groups[0]
                .weights
                .iter()
                .filter(|&&w| w > 0.)
                .count(),
            1
        );
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), grouped);
        let count = d.history_counts().0;
        d.set_mask_intensity(0.).unwrap();
        d.set_mask_intensity(0.4).unwrap();
        assert!((d.mask_peak() - 0.4).abs() < 1e-6);
        assert_eq!(d.history_counts().0, count + 1);
        assert_eq!(d.mesh().vertex_groups, grouped.mesh.vertex_groups);
        d.end_edit();
        d.mask_from_group(0).unwrap();
        assert_eq!(d.mesh().mask, grouped.mesh.mask);
        d.set_mesh_level(1).unwrap();
        d.set_mesh_level(2).unwrap();
        assert_eq!(d.mesh().vertex_groups, grouped.mesh.vertex_groups);
        d.rename_vertex_group(0, "Detail".into()).unwrap();
        let project = d.project();
        project.validate().unwrap();
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(
            serde_json::to_value(loaded).unwrap(),
            serde_json::to_value(project).unwrap()
        );
    }
    #[test]
    fn thickness_group_preview_matches_apply_and_undo() {
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(
            crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
                .mesh()
                .unwrap(),
        ))
        .unwrap();
        d.set_temporary_thickness(0.2).unwrap();
        let model = d.model(0).unwrap();
        let mut options = model.metadata.clone();
        options.thickness_groups = true;
        options.thickness_mask_rim = true;
        d.set_model_metadata(0, model.name.into(), options.clone())
            .unwrap();
        let before = d.geometry_snapshot();
        let expected = d.mesh().solidify_with_metadata(&options).unwrap();
        d.apply_thickness().unwrap();
        assert_eq!(d.mesh(), &expected);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.mesh(), &expected);
    }
    #[test]
    fn whole_stroke_is_one_undo_step_and_new_edits_clear_redo() {
        let mut document = Document::default();
        let original = document.geometry.mesh.vertices[0].1;
        let brush = document.brushes[0].clone();
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        document.apply_dab(&brush, dab);
        document.apply_dab(&brush, dab);
        document.end_stroke();
        let sculpted = document.geometry.mesh.vertices[0].1;
        assert!(sculpted > original);
        document.undo();
        assert_eq!(document.geometry.mesh.vertices[0].1, original);
        assert!(!document.can_undo());
        document.redo();
        assert_eq!(document.geometry.mesh.vertices[0].1, sculpted);
        document.undo();
        document.apply_dab(&brush, dab);
        assert!(!document.can_redo());
    }
    #[test]
    fn invalid_project_does_not_replace_content_or_history() {
        let mut document = Document::default();
        document.invert_mask().unwrap();
        let original = serde_json::to_value(document.project()).unwrap();
        let mut invalid = document.project();
        invalid.mesh.mask.clear();
        assert!(document.replace_project(invalid, "invalid.clay").is_err());
        assert_eq!(serde_json::to_value(document.project()).unwrap(), original);
        assert!(document.is_dirty());
        assert!(document.can_undo());
        assert_eq!(document.current_path(), "sculpt.clay");
    }
}

#[cfg(test)]
mod mesh_tests {
    use super::*;
    #[test]
    fn topology_changes_restore_the_entire_stack_with_undo_and_project_roundtrip() {
        let mut document = Document::default();
        let original = document.mesh().clone();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document.replace_geometry(geometry).unwrap();
        assert_eq!(document.mesh_settings().level(), 2);
        document.set_mesh_level(1).unwrap();
        let project = document.project();
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
        loaded.validate().unwrap();
        let mut restored = Document::default();
        restored.replace_project(loaded, "roundtrip.clay").unwrap();
        restored.set_mesh_level(2).unwrap();
        assert_eq!(
            restored.mesh().triangles.len(),
            original.triangles.len() * 4
        );
        document.set_dynamic_mesh(true).unwrap();
        assert_eq!(document.mesh_settings().levels(), 1);
        document.undo();
        assert!(!document.mesh_settings().dynamic);
        assert_eq!(document.mesh_settings().levels(), 2);
        assert_eq!(document.mesh_settings().level(), 1);
        document.undo();
        assert_eq!(document.mesh_settings().level(), 2);
        document.undo();
        assert_eq!(document.mesh(), &original);
        assert_eq!(document.mesh_settings().levels(), 1);
        document.redo();
        assert_eq!(document.mesh_settings().levels(), 2);
    }
    #[test]
    fn invalid_stack_is_transactional_and_mask_strokes_do_not_request_remeshing() {
        let mut document = Document::default();
        let original = document.geometry_snapshot();
        let mut invalid = original.clone();
        invalid.settings.lower.push(original.mesh.clone());
        assert!(document.replace_geometry(invalid).is_err());
        assert_eq!(document.geometry_snapshot(), original);
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
        let brush = Brush::new(Kind::Mask);
        document.apply_dab(
            &brush,
            Dab {
                center: crate::sculpt::Vec3(0., 1., 0.),
                normal: crate::sculpt::Vec3(0., 1., 0.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: crate::sculpt::Vec3::default(),
                invert: false,
            },
        );
        assert!(!document.end_stroke());
        assert!(document.mesh().mask[0] > 0.);
    }
}

#[cfg(test)]
mod protected_stroke_tests {
    use super::*;
    #[test]
    fn black_alpha_and_fully_masked_strokes_do_not_trigger_auto_remesh() {
        let mut document = Document::default();
        let dab = Dab {
            center: crate::sculpt::Vec3(0., 1., 0.),
            normal: crate::sculpt::Vec3(0., 1., 0.),
            right: crate::sculpt::Vec3(1., 0., 0.),
            up: crate::sculpt::Vec3(0., 0., 1.),
            radius: 0.3,
            pressure: 1.,
            delta: crate::sculpt::Vec3::default(),
            invert: false,
        };
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        document.apply_dab(&brush, dab);
        assert!(!document.end_stroke());
        brush.alpha = Alpha::Soft;
        document.geometry.mesh.mask.fill(1.);
        document.apply_dab(&brush, dab);
        assert!(!document.end_stroke());
    }
}

#[cfg(test)]
mod delete_level_tests {
    use super::*;
    #[test]
    fn deleting_either_side_preserves_active_surface_and_restores_stack_on_undo() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.mesh.vertices.last_mut().unwrap().0 += 0.1;
        *geometry.mesh.mask.last_mut().unwrap() = 0.6;
        geometry.set_level(2).unwrap();
        document.replace_geometry(geometry).unwrap();
        let original = document.geometry_snapshot();
        document.delete_mesh_levels(true).unwrap();
        assert_eq!(document.mesh(), &original.mesh);
        assert_eq!(document.mesh_settings().level(), 1);
        assert_eq!(document.mesh_settings().upper, original.settings.upper);
        document.set_mesh_level(2).unwrap();
        document.mesh_settings().validate(document.mesh()).unwrap();
        document.undo();
        document.undo();
        assert_eq!(document.geometry_snapshot(), original);
        document.delete_mesh_levels(false).unwrap();
        assert_eq!(document.mesh(), &original.mesh);
        assert_eq!(document.mesh_settings().level(), 2);
        assert_eq!(document.mesh_settings().levels(), 2);
        assert_eq!(document.mesh_settings().lower, original.settings.lower);
        let mut fresh = document.geometry_snapshot();
        fresh.subdivide().unwrap();
        fresh.settings.validate(&fresh.mesh).unwrap();
        document.undo();
        assert_eq!(document.geometry_snapshot(), original);
        document.redo();
        assert!(document.mesh_settings().upper.is_empty());
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&document.project()).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.mesh_settings, document.mesh_settings().clone());
    }
    #[test]
    fn deleting_absent_levels_is_a_clean_no_op() {
        let mut document = Document::default();
        document.delete_mesh_levels(true).unwrap();
        document.delete_mesh_levels(false).unwrap();
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
    }
}

#[cfg(test)]
mod history_feedback_tests {
    use super::*;
    #[test]
    fn no_effect_strokes_keep_redo_and_each_mesh_edit_has_a_history_label() {
        let mut document = Document::default();
        document.set_dynamic_mesh(true).unwrap();
        document.undo();
        assert_eq!(document.redo_label(), Some("Switch mesh mode"));
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        document.apply_dab(
            &brush,
            Dab {
                center: crate::sculpt::Vec3(0., 1., 0.),
                normal: crate::sculpt::Vec3(0., 1., 0.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 0., 1.),
                radius: 0.3,
                pressure: 1.,
                delta: crate::sculpt::Vec3::default(),
                invert: false,
            },
        );
        document.end_stroke();
        assert_eq!(document.history_counts(), (0, 1));
        document.redo();
        assert!(document.mesh_settings().dynamic);
        document.undo();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document
            .replace_geometry_named(geometry, "Subdivide mesh")
            .unwrap();
        assert_eq!(document.undo_label(), Some("Subdivide mesh"));
        assert!(!document.can_redo());
        document.delete_mesh_levels(true).unwrap();
        assert_eq!(document.undo_label(), Some("Delete lower levels"));
        document.undo();
        assert_eq!(document.redo_label(), Some("Delete lower levels"));
        document.set_mesh_level(1).unwrap();
        assert_eq!(document.undo_label(), Some("Change subdivision level"));
        document.delete_mesh_levels(false).unwrap();
        assert_eq!(document.undo_label(), Some("Delete higher levels"));
    }
}

#[cfg(test)]
mod adaptive_mode_tests {
    use super::*;
    #[test]
    fn adaptive_mode_discards_levels_undo_restores_them_and_other_modes_exclude_it() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        document.replace_geometry(geometry).unwrap();
        let before = document.geometry_snapshot();
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(document.mesh_settings().levels(), 1);
        assert!(!document.mesh_settings().dynamic);
        assert_eq!(document.mesh(), &before.mesh);
        document.undo();
        assert_eq!(document.geometry_snapshot(), before);
        document.redo();
        assert!(document.mesh_settings().adaptive.enabled);
        document.set_dynamic_mesh(true).unwrap();
        assert!(!document.mesh_settings().adaptive.enabled);
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        document.set_dynamic_mesh(false).unwrap();
        assert!(!document.mesh_settings().adaptive.enabled);
    }
}

#[cfg(test)]
mod adaptive_validation_tests {
    use super::*;
    #[test]
    fn invalid_adaptive_project_is_rejected_and_live_detail_edits_do_not_flood_history() {
        let mut document = Document::default();
        let original = serde_json::to_value(document.project()).unwrap();
        let mut invalid = document.project();
        invalid.mesh.triangles.pop();
        invalid.mesh_settings.adaptive.enabled = true;
        assert!(
            document
                .replace_project(invalid, "invalid-adaptive.clay")
                .is_err()
        );
        assert_eq!(serde_json::to_value(document.project()).unwrap(), original);
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        let count = document.history_counts();
        for detail in [4., 5., 6., 7.] {
            let mut settings = document.mesh_settings().adaptive.clone();
            settings.detail = detail;
            document.set_adaptive(settings).unwrap();
        }
        assert_eq!(document.history_counts(), count);
        document.undo();
        assert!(!document.mesh_settings().adaptive.enabled);
    }
}

#[cfg(test)]
mod adaptive_smooth_tests {
    use super::*;
    use crate::sculpt::Vec3;

    #[test]
    fn smooth_cleanup_reduces_density_preserves_masks_and_undoes_as_one_stroke() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.mesh = geometry.mesh.subdivide().unwrap();
        geometry.mesh.vertices[0].1 += 0.06;
        for (v, mask) in geometry.mesh.vertices.iter().zip(&mut geometry.mesh.mask) {
            if v.1 < 0.5 {
                *mask = 1.;
            }
        }
        document
            .replace_geometry_named(geometry, "Dense test surface")
            .unwrap();
        document
            .set_adaptive(crate::sculpt::AdaptiveSettings {
                enabled: true,
                detail: 16.,
                ..Default::default()
            })
            .unwrap();
        let before = document.mesh().clone();
        let history = document.history_counts().0;
        let mut brush = Brush::new(Kind::Smooth);
        brush.strength = 0.6;
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.4,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        for _ in 0..6 {
            document.begin_sculpt_frame();
            let count = document.mesh().vertices.len();
            document.apply_dab(&brush, dab);
            assert!(document.mesh().vertices.len() <= count);
        }
        document.end_stroke();
        let after = document.mesh().clone();
        assert!(after.vertices.len() < before.vertices.len() - 50);
        assert!(
            after
                .vertices
                .iter()
                .map(|v| v.1)
                .fold(f32::NEG_INFINITY, f32::max)
                < 1.03
        );
        after.validate_closed().unwrap();
        let protected: std::collections::HashSet<_> = after
            .vertices
            .iter()
            .zip(&after.mask)
            .filter(|(_, m)| **m == 1.)
            .map(|(v, _)| (v.0.to_bits(), v.1.to_bits(), v.2.to_bits()))
            .collect();
        for (v, mask) in before.vertices.iter().zip(&before.mask) {
            if *mask == 1. {
                assert!(
                    protected.contains(&(v.0.to_bits(), v.1.to_bits(), v.2.to_bits())),
                    "protected vertex or mask changed"
                );
            }
        }
        assert_eq!(document.history_counts().0, history + 1);
        document.undo();
        assert_eq!(document.mesh(), &before);
        document.redo();
        assert_eq!(document.mesh(), &after);
    }
}

#[cfg(test)]
mod submodel_tests {
    use super::*;

    #[test]
    fn duplication_preserves_stack_and_switching_and_creation_undo_as_a_scene() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.mesh.mask[0] = 0.8;
        document.replace_geometry(geometry).unwrap();
        let source = document.geometry_snapshot();
        document.duplicate_model(false).unwrap();
        assert_eq!(document.mesh(), &source.mesh);
        assert_eq!(document.mesh_settings(), &source.settings);
        assert_eq!(
            document.submodel_names().collect::<Vec<_>>(),
            vec!["Copy 1", "Model"]
        );
        document.clear_mask();
        document.select_submodel(1).unwrap();
        assert_eq!(document.mesh().mask[0], 0.8);
        document.undo();
        assert_eq!(document.geometry.name, "Copy 1");
        assert_eq!(document.mesh().mask[0], 0.);
        document.undo();
        assert_eq!(document.mesh().mask[0], 0.8);
        document.undo();
        assert_eq!(document.geometry_snapshot(), source);
        document.redo();
        let project: Project =
            serde_json::from_slice(&serde_json::to_vec(&document.project()).unwrap()).unwrap();
        let mut restored = Document::default();
        restored.replace_project(project, "submodels.clay").unwrap();
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
        let before = restored.geometry_snapshot();
        let mut invalid = restored.project();
        invalid.submodels[0].mesh.mask.clear();
        assert!(restored.replace_project(invalid, "bad.clay").is_err());
        assert_eq!(restored.geometry_snapshot(), before);
    }

    #[test]
    fn extraction_compacts_selected_faces_keeps_source_and_fails_transactionally() {
        let mut document = Document::default();
        let before = document.geometry_snapshot();
        assert!(document.duplicate_model(true).is_err());
        assert_eq!(document.geometry_snapshot(), before);
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
        for (vertex, mask) in document
            .geometry
            .mesh
            .vertices
            .iter()
            .zip(&mut document.geometry.mesh.mask)
        {
            if vertex.1 > 0.7 {
                *mask = 0.5;
            }
        }
        let source = document.geometry_snapshot();
        let faces = source
            .mesh
            .triangles
            .iter()
            .filter(|face| face.iter().all(|&i| source.mesh.mask[i] >= 0.5))
            .count();
        document.duplicate_model(true).unwrap();
        assert_eq!(document.mesh().triangles.len(), faces);
        assert!(faces > 0 && faces < source.mesh.triangles.len());
        assert!(document.mesh().vertices.len() < source.mesh.vertices.len());
        assert!(document.mesh().mask.iter().all(|&mask| mask == 0.));
        assert_eq!(document.geometry.submodels[0].mesh, source.mesh);
        document.mesh().validate().unwrap();
        document.undo();
        assert_eq!(document.geometry_snapshot(), source);
        document.redo();
        assert_eq!(document.geometry.name, "Extract 1");
    }
}

#[cfg(test)]
mod scene_transform_tests {
    use super::*;
    use crate::sculpt::{ModelMetadata, Transform, Vec3};
    #[test]
    fn transforms_follow_all_levels_preserve_inactive_models_and_undo_once_per_drag() {
        let mut document = Document::default();
        let mut geometry = document.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.mesh.mask[0] = 1.;
        geometry.settings.lower[0].mask[0] = 1.;
        document.replace_geometry(geometry).unwrap();
        document.duplicate_model(false).unwrap();
        let before = document.geometry_snapshot();
        let count = document.history_counts();
        let transform = Transform {
            pivot: Vec3(0.2, 0.1, 0.),
            translation: Vec3(0.3, 0., 0.),
            scale: Vec3(0.8, 0.8, 0.8),
            ..Default::default()
        };
        for _ in 0..3 {
            document.apply_transform(transform, true).unwrap();
        }
        assert!(
            !document.end_stroke(),
            "transforms must not trigger automatic remeshing"
        );
        assert_eq!(document.history_counts().0, count.0 + 1);
        assert_eq!(document.geometry.submodels, before.submodels);
        assert_eq!(document.mesh().vertices[0], before.mesh.vertices[0]);
        assert_eq!(
            document.mesh_settings().lower[0].vertices[0],
            before.settings.lower[0].vertices[0]
        );
        let expected = transform.point(transform.point(transform.point(before.mesh.vertices[1])));
        assert!((document.mesh().vertices[1] - expected).len() < 1e-5);
        document.mesh_settings().validate(document.mesh()).unwrap();
        let after = document.geometry_snapshot();
        document.undo();
        assert_eq!(document.geometry_snapshot(), before);
        document.redo();
        assert_eq!(document.geometry_snapshot(), after);
        document.undo();
        document.apply_transform(transform, false).unwrap();
        document.end_stroke();
        assert!(
            (document.mesh().vertices[0] - transform.point(before.mesh.vertices[0])).len() < 1e-5
        );
        assert!(!document.can_redo());
    }
    #[test]
    fn invalid_identity_and_hidden_transforms_are_transactional() {
        let mut document = Document::default();
        let before = document.geometry_snapshot();
        assert!(
            !document
                .apply_transform(
                    Transform {
                        pivot: Vec3(0.17, 0.24, 0.33),
                        ..Default::default()
                    },
                    true
                )
                .unwrap()
        );
        assert!(
            document
                .apply_transform(
                    Transform {
                        scale: Vec3(0., 1., 1.),
                        ..Default::default()
                    },
                    false
                )
                .is_err()
        );
        assert!(
            document
                .apply_transform(
                    Transform {
                        translation: Vec3(f32::NAN, 0., 0.),
                        ..Default::default()
                    },
                    false
                )
                .is_err()
        );
        assert_eq!(document.geometry_snapshot(), before);
        assert!(!document.can_undo());
        document
            .set_model_metadata(
                0,
                "Model".into(),
                ModelMetadata {
                    visible: false,
                    group: String::new(),
                    ..Default::default()
                },
            )
            .unwrap();
        let hidden = document.geometry_snapshot();
        assert!(
            document
                .apply_transform(
                    Transform {
                        translation: Vec3(1., 0., 0.),
                        ..Default::default()
                    },
                    false
                )
                .is_err()
        );
        assert_eq!(document.geometry_snapshot(), hidden);
    }
    #[test]
    fn groups_order_and_visibility_survive_selection_roundtrip_and_undo() {
        let mut document = Document::default();
        document.duplicate_model(false).unwrap();
        document.duplicate_model(false).unwrap();
        document.add_group("Body".into()).unwrap();
        document.add_group("Details".into()).unwrap();
        document
            .set_model_metadata(
                0,
                "Eyes".into(),
                ModelMetadata {
                    visible: true,
                    group: "Details".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        document
            .set_model_metadata(
                2,
                "Base".into(),
                ModelMetadata {
                    visible: true,
                    group: "Body".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let names: Vec<_> = document.submodel_names().map(str::to_string).collect();
        document.select_submodel(2).unwrap();
        assert_eq!(
            document.submodel_names().collect::<Vec<_>>(),
            names.iter().map(String::as_str).collect::<Vec<_>>()
        );
        assert_eq!(document.active_model_index(), 2);
        document.reorder_model(2, 0).unwrap();
        assert_eq!(document.active_model_index(), 0);
        assert_eq!(document.submodel_names().next(), Some("Base"));
        document.set_group_visible("Body", false).unwrap();
        assert!(!document.active_visible());
        let hidden = document.geometry_snapshot();
        document.undo();
        assert!(document.active_visible());
        document.redo();
        assert_eq!(document.geometry_snapshot(), hidden);
        document.reorder_group(1, 0).unwrap();
        assert_eq!(document.groups(), &["Details", "Body"]);
        let json = serde_json::to_vec(&document.project()).unwrap();
        let project: Project = serde_json::from_slice(&json).unwrap();
        let mut restored = Document::default();
        restored.replace_project(project, "scene.clay").unwrap();
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
        restored.remove_group("Body").unwrap();
        assert_eq!(restored.model(0).unwrap().metadata.group, "");
        assert_eq!(restored.model_count(), 3);
        restored.undo();
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
        let mut invalid = restored.project();
        invalid.scene.active_index = 10;
        assert!(restored.replace_project(invalid, "bad.clay").is_err());
        let mut invalid = restored.project();
        invalid.submodels[0].metadata.group = "Missing".into();
        assert!(restored.replace_project(invalid, "bad.clay").is_err());
        assert_eq!(restored.geometry_snapshot(), document.geometry_snapshot());
    }
}

#[cfg(test)]
mod primitive_tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, Transform, Vec3};
    #[test]
    fn primitive_replacement_conversion_and_parameter_drags_restore_with_undo() {
        let mut d = Document::default();
        d.duplicate_model(false).unwrap();
        let mut geometry = d.geometry_snapshot();
        geometry.subdivide().unwrap();
        geometry.mesh.mask[0] = 0.7;
        d.replace_geometry(geometry).unwrap();
        let before = d.geometry_snapshot();
        d.initialize_primitive().unwrap();
        assert!(d.primitive().is_some());
        assert_eq!(d.mesh_settings().levels(), 1);
        assert!(d.mesh().mask.iter().all(|m| *m == 0.));
        assert_eq!(d.geometry.submodels, before.submodels);
        let initialized = d.geometry_snapshot();
        let count = d.history_counts();
        for size in [2.2, 2.4, 2.6] {
            let mut p = d.primitive().unwrap().clone();
            p.size.0 = size;
            d.update_primitive(p).unwrap();
        }
        d.end_primitive_edit();
        assert_eq!(d.history_counts().0, count.0 + 1);
        let preview = d.geometry_snapshot();
        d.convert_primitive().unwrap();
        assert!(d.primitive().is_none());
        assert_eq!(d.mesh(), &preview.mesh);
        d.undo();
        assert_eq!(d.geometry_snapshot(), preview);
        d.undo();
        assert_eq!(d.geometry_snapshot(), initialized);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), initialized);
    }
    #[test]
    fn live_primitive_blocks_brushes_topology_and_invalid_recipe_updates() {
        let mut d = Document::default();
        d.initialize_primitive().unwrap();
        let original = d.geometry_snapshot();
        let dab = Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.5,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        assert!(!d.apply_dab(&Brush::new(Kind::Standard), dab));
        assert!(!d.apply_dab(&Brush::new(Kind::Mask), dab));
        assert!(d.set_dynamic_mesh(true).is_err());
        assert!(
            d.set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .is_err()
        );
        let mut invalid = d.primitive().unwrap().clone();
        invalid.divisions[0] = 0;
        assert!(d.update_primitive(invalid).is_err());
        assert_eq!(d.geometry_snapshot(), original);
        let mut ring = Primitive::for_kind(PrimitiveKind::Circle);
        ring.filled = false;
        d.update_primitive(ring).unwrap();
        d.end_primitive_edit();
        assert!(d.convert_primitive().is_err());
    }
    #[test]
    fn primitive_recipes_follow_transforms_duplicate_switch_and_project_roundtrip() {
        let mut d = Document::default();
        d.initialize_primitive().unwrap();
        let transform = Transform {
            pivot: Vec3(0.1, 0.2, 0.3),
            translation: Vec3(0.4, 0., 0.),
            scale: Vec3(0.8, 1.2, 1.1),
            stretch: Some((Vec3(1., 1., 0.).norm(), 0.7)),
            rotation: Some((Vec3(1., 0., 1.).norm(), 0.7)),
        };
        d.apply_transform(transform, false).unwrap();
        d.end_stroke();
        d.mesh_settings().validate(d.mesh()).unwrap();
        let mut p = d.primitive().unwrap().clone();
        let center = p.center;
        let basis = p.basis;
        p.divisions = [3, 4, 5];
        p.size = Vec3(1.3, 2.1, 0.8);
        let canonical = Primitive {
            center: Vec3::default(),
            basis: Primitive::default().basis,
            ..p.clone()
        }
        .mesh()
        .unwrap();
        d.update_primitive(p).unwrap();
        d.end_primitive_edit();
        assert_eq!(d.primitive().unwrap().center, center);
        assert_eq!(d.primitive().unwrap().basis, basis);
        for (&point, &local) in d.mesh().vertices.iter().zip(&canonical.vertices) {
            assert!((point - transform.point(local)).len() < 1e-5);
        }
        d.duplicate_model(false).unwrap();
        d.select_submodel(1).unwrap();
        let json = serde_json::to_vec(&d.project()).unwrap();
        let project: Project = serde_json::from_slice(&json).unwrap();
        let mut loaded = Document::default();
        loaded.replace_project(project, "primitive.clay").unwrap();
        assert_eq!(loaded.geometry_snapshot(), d.geometry_snapshot());
        let mut invalid = loaded.project();
        invalid.mesh.vertices[0].0 += 0.1;
        assert!(loaded.replace_project(invalid, "bad.clay").is_err());
        assert_eq!(loaded.geometry_snapshot(), d.geometry_snapshot());
    }
}

#[cfg(test)]
mod brush_identity_tests {
    use super::*;
    #[test]
    fn brush_import_updates_identity_without_duplicates() {
        let mut doc = Document::default();
        let library = doc.brushes().to_vec();
        let count = library.len();
        doc.mark_saved("test.clay");
        assert_eq!(doc.import_brushes(library.clone()).unwrap().0, 0);
        assert!(!doc.is_dirty());
        let copy = doc.duplicate_brush(0).unwrap();
        assert_ne!(doc.brushes()[copy].identity(), doc.brushes()[0].identity());
        let mut brush = doc.brushes()[copy].clone();
        let id = brush.identity();
        brush.name = "Edited copy".into();
        brush.icon = Some(crate::brush_icon::default_icon(Kind::Crease));
        brush.id = Some(Brush::new(Kind::Mask).identity());
        doc.update_brush(copy, brush).unwrap();
        assert_eq!(doc.brushes()[copy].identity(), id);
        let mut imported = doc.brushes()[copy].clone();
        imported.strength = 0.37;
        assert_eq!(
            doc.import_brushes(vec![imported.clone(), imported])
                .unwrap(),
            (0, 1, Some(copy))
        );
        let exported = doc.brushes().to_vec();
        doc.mark_saved("test.clay");
        assert_eq!(doc.import_brushes(exported).unwrap().0, 0);
        assert!(!doc.is_dirty());
        assert_eq!(doc.brushes().len(), count + 1);
    }
    #[test]
    fn brush_legacy_import_and_failed_batch_are_safe() {
        let mut doc = Document::default();
        let mut old = Brush::new(Kind::Clay);
        old.id = None;
        assert_eq!(old.identity(), Brush::new(Kind::Clay).identity());
        old.name = "Legacy custom".into();
        let index = doc.import_brushes(vec![old.clone()]).unwrap().2.unwrap();
        assert_eq!(doc.import_brushes(vec![old]).unwrap(), (0, 0, Some(index)));
        let before = serde_json::to_value(doc.project()).unwrap();
        doc.mark_saved("test.clay");
        let mut new = Brush::new(Kind::Grab);
        new.renew_identity();
        let mut bad = new.clone();
        bad.strength = f32::NAN;
        assert!(doc.import_brushes(vec![new, bad]).is_err());
        assert_eq!(serde_json::to_value(doc.project()).unwrap(), before);
        assert!(!doc.is_dirty());
    }
}

#[cfg(test)]
mod mask_edit_tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    fn cube_geometry() -> Geometry {
        let mut p = Primitive::for_kind(PrimitiveKind::Cube);
        p.divisions = [1; 3];
        Geometry::new(p.mesh().unwrap())
    }
    #[test]
    fn invert_mask_preserves_soft_values_and_all_levels_with_undo() {
        let mut geometry = cube_geometry();
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.set_level(2).unwrap();
        for mesh in std::iter::once(&mut geometry.mesh)
            .chain(geometry.settings.lower.iter_mut())
            .chain(geometry.settings.upper.iter_mut())
        {
            for (i, m) in mesh.mask.iter_mut().enumerate() {
                *m = (i % 5) as f32 * 0.25;
            }
        }
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        let revision = d.display_revision();
        let count = d.history_counts().0;
        d.invert_mask().unwrap();
        let after = d.geometry_snapshot();
        assert_eq!(before.submodels, after.submodels);
        for (a, b) in std::iter::once(&before.mesh)
            .chain(before.settings.lower.iter())
            .chain(before.settings.upper.iter())
            .zip(
                std::iter::once(&after.mesh)
                    .chain(after.settings.lower.iter())
                    .chain(after.settings.upper.iter()),
            )
        {
            assert_eq!(a.vertices, b.vertices);
            assert!(
                a.mask
                    .iter()
                    .zip(&b.mask)
                    .all(|(a, b)| (*b - (1. - a)).abs() < 1e-6)
            );
        }
        assert_eq!(d.history_counts().0, count + 1);
        assert!(d.display_revision() > revision);
        assert_eq!(d.undo_label(), Some("Invert mask"));
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
        d.invert_mask().unwrap();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn delete_masked_cuts_and_compacts_preserving_remaining_quads_and_masks() {
        let mesh = Mesh {
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            vertices: vec![
                crate::sculpt::Vec3(0., 0., 0.),
                crate::sculpt::Vec3(1., 0., 0.),
                crate::sculpt::Vec3(1., 1., 0.),
                crate::sculpt::Vec3(0., 1., 0.),
                crate::sculpt::Vec3(2., 0., 0.),
                crate::sculpt::Vec3(3., 0., 0.),
                crate::sculpt::Vec3(3., 1., 0.),
                crate::sculpt::Vec3(2., 1., 0.),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3], [4, 5, 7], [5, 6, 7]],
            quads: vec![[0, 1, 2, 3], [4, 5, 6, 7]],
            mask: vec![0.5, 0.6, 0.9, 1., 0.2, 0.49, 0., 0.8],
        };
        let cut = mesh.delete_masked().unwrap().unwrap();
        assert_eq!(cut.vertices, &mesh.vertices[4..]);
        assert_eq!(cut.mask, &mesh.mask[4..]);
        assert_eq!(cut.quads, vec![[0, 1, 2, 3]]);
        assert_eq!(cut.triangles, vec![[0, 1, 3], [1, 2, 3]]);
        cut.validate().unwrap();
        let mut mesh = cube_geometry().mesh;
        let face = mesh.quads[0];
        for i in [face[0], face[1], face[2]] {
            mesh.mask[i] = 1.;
        }
        let cut = mesh.delete_masked().unwrap().unwrap();
        assert_eq!(cut.triangles.len(), mesh.triangles.len() - 1);
        assert_eq!(cut.quads.len(), mesh.quads.len() - 1);
        cut.validate().unwrap();
        assert!(cut.validate_closed().is_err());
    }
    #[test]
    fn delete_masked_resets_levels_and_closed_modes_and_undo_restores_everything() {
        let mut geometry = cube_geometry();
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.set_level(2).unwrap();
        let q = geometry.mesh.quads[0];
        for i in q {
            geometry.mesh.mask[i] = 1.;
        }
        geometry.settings.auto_remesh = true;
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        let count = d.history_counts().0;
        assert!(d.delete_masked().unwrap());
        let after = d.geometry_snapshot();
        assert_eq!(after.submodels, before.submodels);
        assert_eq!(d.mesh_settings().levels(), 1);
        assert!(
            !d.mesh_settings().auto_remesh
                && !d.mesh_settings().adaptive.enabled
                && !d.mesh_settings().dynamic
        );
        assert!(d.mesh().triangles.len() < before.mesh.triangles.len());
        assert!(d.mesh().validate_closed().is_err());
        assert_eq!(d.history_counts().0, count + 1);
        assert_eq!(d.undo_label(), Some("Delete masked surface"));
        d.project().validate().unwrap();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
    }
    #[test]
    fn deleting_all_masked_surface_leaves_an_empty_model_that_saves_and_undoes() {
        let mut d = Document::default();
        d.replace_geometry(cube_geometry()).unwrap();
        let original = d.geometry_snapshot();
        let count = d.history_counts();
        assert!(!d.delete_masked().unwrap());
        assert_eq!(d.history_counts(), count);
        d.invert_mask().unwrap();
        let masked = d.geometry_snapshot();
        assert!(d.delete_masked().unwrap());
        assert!(
            d.mesh().vertices.is_empty()
                && d.mesh().triangles.is_empty()
                && d.mesh().quads.is_empty()
        );
        let saved = serde_json::to_string(&d.project()).unwrap();
        let project: Project = serde_json::from_str(&saved).unwrap();
        project.validate().unwrap();
        let mut restored = Document::default();
        restored.replace_project(project, "empty.clay").unwrap();
        assert!(restored.mesh().vertices.is_empty());
        restored.initialize_primitive().unwrap();
        assert!(!restored.mesh().vertices.is_empty());
        d.undo();
        assert_eq!(d.geometry_snapshot(), masked);
        d.undo();
        assert_eq!(d.geometry_snapshot(), original);
    }
}

#[cfg(test)]
mod thickness_edit_tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    #[test]
    fn temporary_thickness_is_per_model_persistent_and_bakes_undoably() {
        let plane = Primitive::for_kind(PrimitiveKind::Plane);
        let mut geometry = Geometry::new(plane.mesh().unwrap());
        geometry.settings.primitive = Some(plane);
        let mut d = Document::default();
        d.replace_geometry(geometry).unwrap();
        let original = d.geometry_snapshot();
        let count = d.history_counts().0;
        d.set_temporary_thickness(0.1).unwrap();
        d.set_temporary_thickness(0.2).unwrap();
        assert_eq!(
            d.history_counts().0,
            count + 1,
            "one slider drag must be one undo step"
        );
        d.end_edit();
        assert_eq!(
            d.mesh(),
            &original.mesh,
            "preview must not change the editing surface"
        );
        let saved = serde_json::to_string(&d.project()).unwrap();
        let loaded: Project = serde_json::from_str(&saved).unwrap();
        loaded.validate().unwrap();
        let mut restored = Document::default();
        restored.replace_project(loaded, "thickness.clay").unwrap();
        assert_eq!(restored.model(0).unwrap().metadata.temporary_thickness, 0.2);
        d.duplicate_model(false).unwrap();
        d.set_temporary_thickness(0.3).unwrap();
        d.end_edit();
        assert_eq!(d.model(1).unwrap().metadata.temporary_thickness, 0.2);
        let before = d.geometry_snapshot();
        d.apply_thickness().unwrap();
        let after = d.geometry_snapshot();
        d.mesh().validate_closed().unwrap();
        assert!(d.primitive().is_none());
        assert_eq!(d.mesh_settings().levels(), 1);
        assert_eq!(d.model(0).unwrap().metadata.temporary_thickness, 0.);
        assert_eq!(before.submodels, after.submodels);
        assert_eq!(d.undo_label(), Some("Apply thickness"));
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
        let before = d.geometry_snapshot();
        assert!(d.set_temporary_thickness(0.2).is_err());
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn thickness_off_preserves_legacy_project_shape_and_invalid_values_fail_safely() {
        let mut d = Document::default();
        let before = serde_json::to_value(d.project()).unwrap();
        assert!(!before.to_string().contains("temporary_thickness"));
        let count = d.history_counts();
        assert!(d.set_temporary_thickness(f32::NAN).is_err());
        assert!(d.set_temporary_thickness(0.1).is_err());
        assert_eq!(d.history_counts(), count);
        assert_eq!(serde_json::to_value(d.project()).unwrap(), before);
        let mut invalid = d.project();
        invalid.scene.active_metadata.temporary_thickness = -1.;
        assert!(invalid.validate().is_err());
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;
    fn plane() -> Document {
        let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
        primitive.divisions = [2, 1, 1];
        let mut mesh = primitive.mesh().unwrap();
        mesh.mask = mesh
            .vertices
            .iter()
            .map(|p| if p.0 >= 0. { 0.8 } else { 0.2 })
            .collect();
        mesh.create_vertex_group("Base".into(), 0.5, true).unwrap();
        mesh.create_vertex_group("Right".into(), 0.5, false)
            .unwrap();
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        d
    }
    #[test]
    fn visibility_region_split_roundtrip_and_undo_preserve_attributes_and_levels() {
        let mut d = plane();
        let mut g = d.geometry_snapshot();
        g.subdivide().unwrap();
        d.replace_geometry(g).unwrap();
        let initial = d.geometry_snapshot();
        d.select_face_region(|p| p.0 > 0., true).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), d.mesh().triangles.len() / 2);
        assert_eq!(d.mesh().mask, initial.mesh.mask);
        assert_eq!(d.mesh().polygroups, initial.mesh.polygroups);
        assert!(d.mesh_settings().lower[0].has_hidden_faces());
        let saved = serde_json::to_string(&d.project()).unwrap();
        let mut restored = Document::default();
        restored
            .replace_project(serde_json::from_str(&saved).unwrap(), "visibility.clay")
            .unwrap();
        assert_eq!(restored.geometry_snapshot(), d.geometry_snapshot());
        d.set_mesh_level(1).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 2);
        d.set_mesh_level(2).unwrap();
        let before_split = d.geometry_snapshot();
        d.split_hidden().unwrap();
        assert_eq!(d.model_count(), 2);
        assert_eq!(d.active_model_index(), 0);
        assert_eq!(
            d.mesh().triangles.len() + d.model(1).unwrap().mesh.triangles.len(),
            initial.mesh.triangles.len()
        );
        for i in 0..2 {
            let model = d.model(i).unwrap();
            assert!(!model.mesh.has_hidden_faces());
            assert_eq!(model.settings.levels(), 1);
            assert_eq!(model.mesh.quads.len() * 2, model.mesh.triangles.len());
            assert_eq!(model.mesh.vertex_groups.len(), 2);
            for (v, p) in model.mesh.vertices.iter().enumerate() {
                let old = initial.mesh.vertices.iter().position(|q| q == p).unwrap();
                assert_eq!(model.mesh.mask[v], initial.mesh.mask[old]);
                assert_eq!(
                    model.mesh.vertex_groups[1].weights[v],
                    initial.mesh.vertex_groups[1].weights[old]
                );
            }
            model.mesh.validate().unwrap();
        }
        let split = d.geometry_snapshot();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before_split);
        d.redo();
        assert_eq!(d.geometry_snapshot(), split);
    }
    #[test]
    fn polygroup_clicks_eye_and_background_actions_work_with_region_hiding() {
        let mut d = plane();
        d.select_polygroup(2).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 2);
        assert!(
            (0..d.mesh().triangles.len())
                .all(|i| d.mesh().face_visible(i) == (d.mesh().polygroups[i] == 1))
        );
        d.select_polygroup(1).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 2);
        assert!(
            (0..d.mesh().triangles.len())
                .all(|i| d.mesh().face_visible(i) == (d.mesh().polygroups[i] == 2))
        );
        d.show_all_faces().unwrap();
        d.invert_visibility().unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 4);
        let before = d.geometry_snapshot();
        assert!(d.split_hidden().is_err());
        assert_eq!(d.geometry_snapshot(), before);
        d.show_all_faces().unwrap();
        d.select_face_region(|p| p.0 > 0., true).unwrap();
        d.set_vertex_group_visible(1, true).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 0);
        d.set_vertex_group_visible(0, false).unwrap();
        d.invert_visibility().unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 2);
        assert!(
            d.mesh()
                .triangles
                .iter()
                .enumerate()
                .all(|(i, _)| d.mesh().face_visible(i) == (d.mesh().polygroups[i] == 1))
        );
        d.show_all_faces().unwrap();
        let counts = d.history_counts();
        d.show_all_faces().unwrap();
        assert_eq!(d.history_counts(), counts);
        d.undo();
        let counts = d.history_counts();
        d.select_face_region(|_| false, true).unwrap();
        assert_eq!(d.history_counts(), counts, "empty hide must preserve redo");
    }
    #[test]
    fn polygroup_click_hides_each_group_until_last_click_flips_with_undo() {
        let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
        primitive.divisions = [3, 1, 1];
        let mut mesh = primitive.mesh().unwrap();
        mesh.create_vertex_group("Middle".into(), 0.5, true)
            .unwrap();
        mesh.mask = mesh
            .vertices
            .iter()
            .map(|p| if p.0 < 0. { 1. } else { 0. })
            .collect();
        mesh.create_vertex_group("Left".into(), 0.5, false).unwrap();
        mesh.mask = mesh
            .vertices
            .iter()
            .map(|p| if p.0 > 0. { 1. } else { 0. })
            .collect();
        mesh.create_vertex_group("Right".into(), 0.5, false)
            .unwrap();
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        d.select_polygroup(2).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 2);
        d.select_polygroup(3).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 4);
        let before = d.geometry_snapshot();
        d.select_polygroup(1).unwrap();
        assert_eq!(d.mesh().hidden_face_count(), 2);
        for (i, &id) in d.mesh().polygroups.iter().enumerate() {
            assert_eq!(d.mesh().face_visible(i), id != 1);
        }
        let after = d.geometry_snapshot();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
    }
    #[test]
    fn visibility_region_normalizes_groups_even_when_only_higher_level_changes() {
        let mut d = plane();
        let mut geometry = d.geometry_snapshot();
        geometry.subdivide().unwrap();
        d.replace_geometry(geometry).unwrap();
        d.set_vertex_group_visible(0, false).unwrap();
        let before = d.geometry_snapshot();
        let history = d.history_counts();
        d.select_face_region(|_| false, true).unwrap();
        assert_eq!(d.geometry_snapshot(), before);
        assert_eq!(d.history_counts(), history);
        d.select_face_region(|p| p.0 > 0. && p.0 < 0.4, true)
            .unwrap();
        assert!(d.mesh().hidden_face_count() > before.mesh.hidden_face_count());
        assert_eq!(
            d.mesh_settings().lower[0].hidden_face_count(),
            before.settings.lower[0].hidden_face_count()
        );
        d.mesh_settings().validate(d.mesh()).unwrap();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn hidden_faces_and_boundary_vertices_are_protected_from_sculpting() {
        let mut d = plane();
        d.select_face_region(|p| p.0 > 0., true).unwrap();
        let before = d.mesh().clone();
        let brush = Brush::new(Kind::Inflate);
        d.apply_dab(
            &brush,
            Dab {
                center: crate::sculpt::Vec3::default(),
                normal: crate::sculpt::Vec3(0., 0., 1.),
                right: crate::sculpt::Vec3(1., 0., 0.),
                up: crate::sculpt::Vec3(0., 1., 0.),
                radius: 10.,
                pressure: 1.,
                delta: crate::sculpt::Vec3::default(),
                invert: false,
            },
        );
        d.end_stroke();
        for (i, t) in before.triangles.iter().enumerate() {
            if !before.face_visible(i) {
                for &v in t {
                    assert_eq!(before.vertices[v], d.mesh().vertices[v]);
                }
            }
        }
        assert_ne!(before.vertices, d.mesh().vertices);
        let mut invalid = before.clone();
        invalid.hidden_faces.pop();
        assert!(invalid.validate().is_err());
        assert!(before.remesh(16).is_err());
        let refined = before.subdivide().unwrap();
        assert_eq!(refined.hidden_face_count(), before.hidden_face_count() * 4);
        let shell = before.solidify(0.1).unwrap();
        shell.validate_closed().unwrap();
        assert_eq!(shell.hidden_faces.len(), shell.triangles.len());
        assert!(shell.hidden_face_count() >= before.hidden_face_count() * 2);
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    #[test]
    fn merge_down_keeps_active_name_and_other_models_with_complete_undo() {
        let mut d = Document::default();
        d.duplicate_model(false).unwrap();
        d.duplicate_model(false).unwrap();
        d.select_submodel(1).unwrap();
        let mut geometry = d.geometry_snapshot();
        geometry.subdivide().unwrap();
        d.replace_geometry(geometry).unwrap();
        let before = d.geometry_snapshot();
        let untouched = d.model(0).unwrap().mesh.clone();
        let name = d.model(1).unwrap().name.to_string();
        let faces = d.mesh().triangles.len() + d.model(2).unwrap().mesh.triangles.len();
        let counts = d.history_counts();
        assert_eq!(d.merge_down_target(), Some(2));
        d.merge_down().unwrap();
        assert_eq!(d.history_counts(), (counts.0 + 1, 0));
        assert_eq!(d.undo_label(), Some("Merge down"));
        assert_eq!(d.active_model_index(), 1);
        assert_eq!(d.model_count(), 2);
        assert_eq!(d.model(1).unwrap().name, name);
        assert_eq!(d.mesh().triangles.len(), faces);
        assert_eq!(d.mesh_settings().levels(), 1);
        assert_eq!(d.model(0).unwrap().mesh, &untouched);
        d.project().validate().unwrap();
        let after = d.geometry_snapshot();
        let serialized = serde_json::to_string(&d.project()).unwrap();
        let mut loaded = Document::default();
        loaded
            .replace_project(serde_json::from_str(&serialized).unwrap(), "merged.clay")
            .unwrap();
        assert_eq!(loaded.geometry_snapshot(), after);
        let counts = d.history_counts();
        assert!(d.merge_down().is_err());
        assert_eq!(d.geometry_snapshot(), after);
        assert_eq!(d.history_counts(), counts);
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
        d.redo();
        assert_eq!(d.geometry_snapshot(), after);
    }
    #[test]
    fn merge_down_follows_grouped_panel_order_and_preserves_hidden_models() {
        let mut d = Document::default();
        d.duplicate_model(false).unwrap();
        d.add_group("Folder".into()).unwrap();
        let mut metadata = d.model(0).unwrap().metadata.clone();
        metadata.group = "Folder".into();
        metadata.visible = false;
        d.set_model_metadata(0, "Below in panel".into(), metadata)
            .unwrap();
        d.select_submodel(1).unwrap();
        // Ungrouped model 1 is shown above foldered model 0.
        assert_eq!(d.merge_down_target(), Some(0));
        let before = d.geometry_snapshot();
        let visible_faces = d.mesh().triangles.len();
        d.merge_down().unwrap();
        assert_eq!(d.active_model_index(), 0);
        assert_eq!(d.model_count(), 1);
        assert_eq!(d.mesh().triangles.len(), visible_faces * 2);
        assert_eq!(d.mesh().hidden_face_count(), visible_faces);
        assert_eq!(d.model(0).unwrap().metadata.group, "");
        assert!(d.active_visible());
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
    }
    #[test]
    fn merge_down_bakes_primitives_and_temporary_thickness() {
        let mut d = Document::default();
        d.initialize_primitive().unwrap();
        let mut plane = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
        plane.divisions = [1; 3];
        d.update_primitive(plane).unwrap();
        d.end_primitive_edit();
        d.set_temporary_thickness(0.1).unwrap();
        d.end_edit();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        let shell = d.mesh().solidify(0.1).unwrap();
        d.merge_down().unwrap();
        assert!(d.primitive().is_none());
        assert_eq!(d.model(0).unwrap().metadata.temporary_thickness, 0.);
        assert_eq!(d.mesh().triangles.len(), shell.triangles.len() * 2);
        d.mesh().validate_closed().unwrap();
        d.undo();
        assert_eq!(d.geometry_snapshot(), before);
    }
}

#[cfg(test)]
mod merge_failure_tests {
    use super::*;
    #[test]
    fn merge_down_rejects_unfilled_primitive_without_changing_scene_or_history() {
        let mut d = Document::default();
        d.initialize_primitive().unwrap();
        let mut circle = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Circle);
        circle.filled = false;
        d.update_primitive(circle).unwrap();
        d.end_primitive_edit();
        d.duplicate_model(false).unwrap();
        let before = d.geometry_snapshot();
        let history = d.history_counts();
        assert!(d.merge_down().unwrap_err().contains("Fill the primitive"));
        assert_eq!(d.geometry_snapshot(), before);
        assert_eq!(d.history_counts(), history);
    }
}

#[cfg(test)]
mod group_all_tests {
    use super::*;
    #[test]
    fn group_all_replaces_groups_on_every_level_and_undo_restores_them() {
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.mask.fill(0.7);
        mesh.create_vertex_group("Old".into(), 0.5, true).unwrap();
        mesh.create_vertex_group("Detail".into(), 0.5, false)
            .unwrap();
        mesh.vertex_groups[1].visible = false;
        let mut geometry = Geometry::new(mesh);
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        let mut doc = Document::default();
        doc.replace_geometry(geometry).unwrap();
        doc.set_mesh_level(2).unwrap();
        doc.duplicate_model(false).unwrap();
        let before = doc.geometry_snapshot();
        assert_eq!(
            doc.create_vertex_group("Detail".into(), true).unwrap(),
            0,
            "replacement may reuse an old name"
        );
        let after = doc.geometry_snapshot();
        assert_eq!(after.submodels, before.submodels);
        let levels = |g: &Geometry| {
            std::iter::once(&g.mesh)
                .chain(g.settings.lower.iter())
                .chain(g.settings.upper.iter())
                .cloned()
                .collect::<Vec<_>>()
        };
        for (new, old) in levels(&after).iter().zip(levels(&before)) {
            assert_eq!(new.vertex_groups.len(), 1);
            assert_eq!(new.vertex_groups[0].name, "Detail");
            assert!(new.vertex_groups[0].weights.iter().all(|&w| w == 1.));
            assert!(new.polygroups.iter().all(|&id| id == 1));
            assert_eq!(new.mask, old.mask);
            assert_eq!(new.vertices, old.vertices);
            assert_eq!(new.hidden_face_count(), old.hidden_face_count());
        }
        doc.project().validate().unwrap();
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), after);
        assert!(doc.create_vertex_group("x".repeat(129), true).is_err());
        assert_eq!(
            doc.geometry_snapshot(),
            after,
            "invalid replacement must not clear old data"
        );
    }
}

#[cfg(test)]
mod mask_filter_tests {
    use super::*;

    #[test]
    fn mask_filters_preserve_scene_attributes_and_all_levels_with_undo_and_save() {
        for sharpen in [false, true] {
            let mut primitive =
                crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
            primitive.divisions = [2, 2, 1];
            let mut mesh = primitive.mesh().unwrap();
            mesh.mask = mesh
                .vertices
                .iter()
                .map(|p| if p.0 < 0. { 0.2 } else { 0.8 })
                .collect();
            mesh.create_vertex_group("Saved weights".into(), 0., false)
                .unwrap();
            mesh.hidden_faces = vec![true; mesh.triangles.len()];
            let mut geometry = Geometry::new(mesh);
            geometry.subdivide().unwrap();
            geometry.subdivide().unwrap();
            geometry.set_level(2).unwrap();
            let mut doc = Document::default();
            doc.replace_geometry(geometry).unwrap();
            doc.duplicate_model(false).unwrap();
            let before = doc.geometry_snapshot();
            let count = doc.history_counts().0;
            if sharpen {
                doc.sharpen_mask().unwrap();
            } else {
                doc.blur_mask().unwrap();
            }
            let after = doc.geometry_snapshot();
            let mut masks_restored = after.clone();
            for (edited, original) in std::iter::once(&mut masks_restored.mesh)
                .chain(masks_restored.settings.lower.iter_mut())
                .chain(masks_restored.settings.upper.iter_mut())
                .zip(
                    std::iter::once(&before.mesh)
                        .chain(before.settings.lower.iter())
                        .chain(before.settings.upper.iter()),
                )
            {
                assert_ne!(
                    edited.mask, original.mask,
                    "every stored level must be filtered"
                );
                assert!(
                    edited
                        .mask
                        .iter()
                        .all(|m| m.is_finite() && (0. ..=1.).contains(m))
                );
                edited.mask.clone_from(&original.mask);
            }
            assert_eq!(
                masks_restored, before,
                "only active model masks may change, including hidden faces"
            );
            assert_eq!(doc.history_counts().0, count + 1);
            assert_eq!(
                doc.undo_label(),
                Some(if sharpen { "Sharpen mask" } else { "Blur mask" })
            );
            doc.undo();
            assert_eq!(doc.geometry_snapshot(), before);
            doc.redo();
            assert_eq!(doc.geometry_snapshot(), after);
            let json = serde_json::to_string(&doc.project()).unwrap();
            let mut restored = Document::default();
            restored
                .replace_project(serde_json::from_str(&json).unwrap(), "mask-filter.clay")
                .unwrap();
            assert_eq!(restored.geometry_snapshot(), after);
            // Ordinary level propagation must preserve the edited mask when returning.
            for level in [1, 3] {
                let active = doc.mesh().clone();
                doc.set_mesh_level(level).unwrap();
                doc.set_mesh_level(2).unwrap();
                assert_eq!(doc.mesh().vertices, active.vertices);
                assert_eq!(doc.mesh().vertex_groups, active.vertex_groups);
                for (&a, &b) in doc.mesh().mask.iter().zip(&active.mask) {
                    assert!((a - b).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn unchanged_mask_filters_preserve_clean_state_and_redo() {
        let mut doc = Document::default();
        doc.invert_mask().unwrap();
        doc.undo();
        doc.mark_saved("mask-filter.clay");
        let before = doc.geometry_snapshot();
        let counts = doc.history_counts();
        doc.blur_mask().unwrap();
        doc.sharpen_mask().unwrap();
        assert_eq!(doc.geometry_snapshot(), before);
        assert_eq!(doc.history_counts(), counts);
        assert!(!doc.is_dirty());
        doc.redo();
        assert!(doc.mesh().mask.iter().all(|m| *m == 1.));
    }
}

#[cfg(test)]
mod lasso_mask_tests {
    use super::*;

    #[test]
    fn mask_lasso_preserves_attributes_hidden_vertices_and_every_subdivision_level() {
        let mut primitive = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane);
        primitive.divisions = [4, 4, 1];
        let mut mesh = primitive.mesh().unwrap();
        mesh.mask = mesh
            .vertices
            .iter()
            .map(|p| if p.0 < 0. { 0.2 } else { 0.8 })
            .collect();
        mesh.create_vertex_group("Soft weights".into(), 0.1, false)
            .unwrap();
        let mut geometry = Geometry::new(mesh);
        geometry.subdivide().unwrap();
        geometry.subdivide().unwrap();
        geometry.set_level(2).unwrap();
        let mut doc = Document::default();
        doc.replace_geometry(geometry).unwrap();
        doc.select_face_region(|p| p.1 > 0.4, true).unwrap();
        doc.duplicate_model(false).unwrap();
        for erase in [false, true] {
            let before = doc.geometry_snapshot();
            let counts = doc.history_counts();
            doc.mask_region(|p| p.0 > 0., erase).unwrap();
            let after = doc.geometry_snapshot();
            let mut restored_masks = after.clone();
            for (edited, old) in std::iter::once(&mut restored_masks.mesh)
                .chain(restored_masks.settings.lower.iter_mut())
                .chain(restored_masks.settings.upper.iter_mut())
                .zip(
                    std::iter::once(&before.mesh)
                        .chain(before.settings.lower.iter())
                        .chain(before.settings.upper.iter()),
                )
            {
                let editable = old.sculptable_vertices();
                let mut changed = 0;
                for (i, p) in old.vertices.iter().enumerate() {
                    let expected = if editable[i] && p.0 > 0. {
                        if erase { 0. } else { 1. }
                    } else {
                        old.mask[i]
                    };
                    assert_eq!(edited.mask[i], expected);
                    changed += usize::from(edited.mask[i] != old.mask[i]);
                }
                assert!(changed > 0);
                edited.mask.clone_from(&old.mask);
            }
            assert_eq!(
                restored_masks, before,
                "no geometry, groups, visibility or inactive models may change"
            );
            assert_eq!(doc.history_counts().0, counts.0 + 1);
            assert_eq!(
                doc.undo_label(),
                Some(if erase {
                    "Erase mask lasso"
                } else {
                    "Mask lasso"
                })
            );
            let json = serde_json::to_string(&doc.project()).unwrap();
            let mut loaded = Document::default();
            loaded
                .replace_project(serde_json::from_str(&json).unwrap(), "lasso.clay")
                .unwrap();
            assert_eq!(loaded.geometry_snapshot(), after);
            doc.undo();
            assert_eq!(doc.geometry_snapshot(), before);
            doc.redo();
            assert_eq!(doc.geometry_snapshot(), after);
            for level in [1, 3] {
                doc.set_mesh_level(level).unwrap();
                doc.set_mesh_level(2).unwrap();
                assert_eq!(doc.mesh().vertices, after.mesh.vertices);
                assert_eq!(doc.mesh().vertex_groups, after.mesh.vertex_groups);
                assert_eq!(doc.mesh().hidden_faces, after.mesh.hidden_faces);
                for (i, (&actual, &expected)) in
                    doc.mesh().mask.iter().zip(&after.mesh.mask).enumerate()
                {
                    assert!(
                        (actual - expected).abs() < 1e-6,
                        "mask at vertex {i}: {actual} vs {expected}, level {level}"
                    );
                }
            }
        }
    }

    #[test]
    fn mask_lasso_no_hit_or_unchanged_mask_preserves_redo_and_clean_state() {
        let mut doc = Document::default();
        doc.invert_mask().unwrap();
        doc.undo();
        doc.mark_saved("lasso.clay");
        let before = doc.geometry_snapshot();
        doc.mask_region(|_| false, false).unwrap();
        doc.mask_region(|_| true, true).unwrap();
        assert_eq!(doc.geometry_snapshot(), before);
        assert_eq!(doc.history_counts(), (0, 1));
        assert!(!doc.is_dirty());
    }
}
