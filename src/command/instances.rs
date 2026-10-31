use crate::sculpt::{Vec3, instances::Set};
pub(crate) enum InstanceCommand {
    NewArray,
    NewPath,
    Edit(u64),
    Preview(Box<Set>),
    Apply,
    Cancel,
    Remove(u64),
    Select(u64, usize),
    Detach(u64, usize),
    Capture(String, Vec3, usize, usize),
    StartBrush(u64),
    StopBrush,
    Bake(u64, bool),
    ExportAsset(u64, String),
    ImportAsset(String),
    RemoveAsset(u64),
}
