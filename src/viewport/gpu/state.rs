//! Renderer-neutral view and preview inputs; document data remains on the CPU.
use super::*;

#[derive(Default)]
pub(super) struct Preview {
    pub(super) show_polygroups: bool,
    pub(super) backface_opacity: f32,
    pub(super) brush: Option<BrushPreview>,
    pub(super) symmetry: Option<(Symmetry, f32)>,
}
pub(super) struct BrushPreview {
    pub(super) center: Vec3,
    pub(super) normal: Vec3,
    pub(super) radius: f32,
    pub(super) brush: Brush,
    pub(super) color: [f32; 3],
}

pub(super) struct Params {
    pub(super) background: [u8; 3],
    pub(super) right: Vec3,
    pub(super) up: Vec3,
    pub(super) view: Vec3,
    pub(super) projection: [f32; 4],
    pub(super) depth: f32,
    pub(super) perspective: [f32; 3],
}
impl Params {
    pub(super) fn new(
        rect: egui::Rect,
        right: Vec3,
        up: Vec3,
        view: Vec3,
        scale: f32,
        center: egui::Pos2,
        extent: f32,
    ) -> Self {
        Self {
            right,
            up,
            view,
            projection: [
                scale * 2. / rect.width(),
                scale * 2. / rect.height(),
                (center.x - rect.center().x) * 2. / rect.width(),
                -(center.y - rect.center().y) * 2. / rect.height(),
            ],
            depth: 1. / extent,
            perspective: [0.; 3],
            background: [23, 26, 31],
        }
    }
    pub(super) fn set_perspective(&mut self, distance: f32, extent: f32) {
        if distance > 0. {
            let near = distance * 0.001;
            let far = distance + extent;
            self.perspective = [
                1. / distance,
                (far + near) / (far - near),
                -2. * far * near / ((far - near) * distance),
            ];
        } else {
            self.perspective = [0.; 3];
        }
    }
}
