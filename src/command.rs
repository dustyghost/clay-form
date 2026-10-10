//! UI intent; execution and file access belong to the application controller.
use crate::sculpt::Brush;
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
    DuplicateBrush(usize),
    UpdateBrush(usize, Brush),
    LoadAlpha(usize),
    SavePreset(usize),
    LoadPreset,
    SaveAndContinue,
    DiscardAndContinue,
    CancelPending,
}
