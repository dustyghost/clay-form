//! UI intent; execution and file access belong to the application controller.
use crate::sculpt::{AdaptiveSettings, Brush, Kind, Symmetry};
#[derive(Clone)]
pub(crate) enum PendingAction {
    New,
    Open(String),
    Quit,
}
pub(crate) enum Command {
    Request(PendingAction),
    Save,
    Export,
    Undo,
    Redo,
    ClearMask,
    ResetView,
    ZoomIn,
    ZoomOut,
    SetZoom(f32),
    Subdivide,
    SetMeshLevel(usize),
    DeleteLowerLevels,
    DeleteHigherLevels,
    SetDynamicMesh(bool),
    SetAdaptive(AdaptiveSettings),
    SetRemeshSettings(u32, bool),
    Remesh,
    CancelMeshJob,
    SetSymmetry(Symmetry),
    AddBrush(Kind),
    DuplicateBrush(usize),
    UpdateBrush(usize, Brush),
    LoadAlpha(usize),
    SavePreset(usize),
    LoadPreset,
    SaveAndContinue,
    DiscardAndContinue,
    CancelPending,
}
