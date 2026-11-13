use crate::sculpt::paths::PathMode;

pub(crate) enum PathCommand {
    Open,
    ActivateBrush,
    New(PathMode),
    NewRing,
    Select(u64),
    Save,
    SaveAndNext,
    SaveAndSculpt,
    Discard,
    Delete(u64),
    Reproject,
    Detach,
    Preview,
    StrokeEntirePath,
    Apply,
    CancelPreview,
    Close,
}
