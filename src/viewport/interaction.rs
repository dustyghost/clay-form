//! Temporary interaction state. Dropping a session cancels private stroke previews.
use super::*;

pub(super) struct SnakeStroke {
    pub center: Vec3,
    pub normal: Vec3,
    pub mobility: f32,
}
#[derive(Default)]
pub(crate) struct InteractionSession {
    pub(super) sculpt: SculptSession,
    pub(crate) visibility_hold: Option<crate::preferences::HoldBinding>,
    pub(crate) guided: guided::GuidedPaths,
    pub(crate) attach_stencil: bool,
    pub(super) stencil_overlay: stencil::Overlay,
    pub(crate) projection_preview: Option<std::sync::Arc<crate::sculpt::Mesh>>,
    pub(crate) projection_mode: bool,
    pub(crate) gesture: Option<Gesture>,
    pub(super) gizmo_dragged: bool,
    pub(super) gizmo_start: Option<Pos2>,
    pub(super) gizmo_snap: Option<(f32, f32)>,
    pub(super) transform: transform::TransformControls,
    pub(super) primitive: primitive::PrimitiveControls,
    pub(super) slice: Option<slice::SliceDrag>,
    pub(super) selection: Option<visibility::SelectionDrag>,
    pub(super) visibility_path: Option<visibility_path::VisibilityPath>,
    pub(crate) visibility_path_action: Option<crate::preferences::Action>,
    pub(super) slice_path: Option<slice::SlicePathDraft>,
}
#[derive(Default)]
pub(super) struct SculptSession {
    pub stroke: bool,
    pub stroke_brush: Option<String>,
    pub decimate_removed: Option<(usize, usize)>,
    pub pending_stroke: Option<stroke::PendingStroke>,
    pub stabilizer: Option<stabilizer::Stabilizer>,
    pub last: Option<Pos2>,
    pub snake: Option<SnakeStroke>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_drops_transient_state_without_resetting_persistent_selection() {
        let mut viewport = Viewport {
            edge_selection: vec![[0, 1]],
            edge_revision: 12,
            ..Default::default()
        };
        viewport.paths.active = true;
        viewport.session.gesture = Some(Gesture::Sculpt);
        viewport.session.sculpt.stroke = true;
        viewport.session.sculpt.last = Some(Pos2::new(1., 2.));
        viewport.session.sculpt.stroke_brush = Some("test".into());
        viewport.session.attach_stencil = true;
        let mut document = Document::default();
        document.begin_stroke();
        viewport.cancel(&mut document);
        assert!(viewport.session.gesture.is_none());
        assert!(!viewport.session.sculpt.stroke);
        assert!(viewport.session.sculpt.last.is_none());
        assert!(viewport.session.sculpt.stroke_brush.is_none());
        assert!(!viewport.session.attach_stencil);
        assert!(viewport.paths.active);
        assert_eq!(viewport.edge_selection, vec![[0, 1]]);
        assert_eq!(viewport.edge_revision, 12);
        assert!(!document.edit_in_progress());
    }
}
