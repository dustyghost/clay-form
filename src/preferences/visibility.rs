//! Remembered visibility choices are independent of the active sculpt tool.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum VisibilityShape {
    #[default]
    Rectangle,
    Lasso,
    Line,
    Path,
}
impl VisibilityShape {
    pub const ALL: [Self; 4] = [Self::Rectangle, Self::Lasso, Self::Line, Self::Path];
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Lasso => "Lasso",
            Self::Line => "Line",
            Self::Path => "Closed path",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct VisibilitySettings {
    pub shape: VisibilityShape,
    pub hide: bool,
    pub flip: bool,
}
impl VisibilitySettings {
    pub fn tool(self) -> crate::viewport::SelectionTool {
        crate::viewport::SelectionTool::Visibility(self)
    }
}
