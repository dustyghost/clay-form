//! Continuous parameter edits share one history entry until release or a new edit kind.
use super::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditKind {
    Material(u64),
    Primitive,
    Thickness,
    MaskIntensity,
    MaskThreshold,
}
impl EditKind {
    fn label(self) -> &'static str {
        match self {
            Self::Material(_) => "Edit material",
            Self::Primitive => "Edit primitive parameters",
            Self::Thickness => "Edit thickness modifier",
            Self::MaskIntensity => "Mask intensity",
            Self::MaskThreshold => "Mask threshold",
        }
    }
}

pub(super) enum EditSession {
    Material(u64),
    Primitive,
    Thickness,
    MaskThreshold,
    MaskIntensity { peak: f32, masks: Vec<Vec<f32>> },
}
impl EditSession {
    fn kind(&self) -> EditKind {
        match self {
            Self::Material(id) => EditKind::Material(*id),
            Self::Primitive => EditKind::Primitive,
            Self::Thickness => EditKind::Thickness,
            Self::MaskThreshold => EditKind::MaskThreshold,
            Self::MaskIntensity { .. } => EditKind::MaskIntensity,
        }
    }
}
impl Document {
    /// Continuing the same kind preserves its baseline; a different command
    /// closes it. Preparing an unchanged edit never creates a history entry.
    pub(crate) fn prepare_edit(&mut self, next: Option<EditKind>) {
        if self.edit.as_ref().map(EditSession::kind) != next {
            self.end_edit();
        }
    }
    pub(super) fn begin_edit(&mut self, session: EditSession) {
        self.prepare_edit(Some(session.kind()));
        self.end_stroke();
        if self.edit.is_none() {
            self.record_edit(session.kind().label());
            self.edit = Some(session);
        }
    }
    pub(crate) fn end_edit(&mut self) {
        self.edit = None;
    }
    /// Finish a primitive drag without changing any other parameter session.
    pub fn end_primitive_edit(&mut self) {
        if matches!(self.edit, Some(EditSession::Primitive)) {
            self.end_edit();
        }
    }
}
