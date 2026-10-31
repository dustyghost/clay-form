use crate::sculpt::paths::PathMode;

pub(crate) enum PathCommand {
    Open,
    New(PathMode),
    NewRing,
    Select(u64),
    Save,
    SaveAndSculpt,
    Discard,
    Delete,
    Reproject,
    Detach,
    Preview,
    StrokeEntirePath,
    Apply,
    CancelPreview,
    Close,
}
