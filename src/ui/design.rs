//! Shared control sizes, palette, and font-independent action icons.
use eframe::egui::{self, Color32, Vec2};
#[derive(Clone, Copy, Debug)]
pub(super) enum Glyph {
    New,
    Open,
    Save,
    Export,
    Undo,
    Redo,
    Settings,
    Edit,
    Mesh,
    Transform,
    Slice,
    Primitive,
    Layers,
    Camera,
    Copy,
    Restore,
    Plus,
    Minus,
    View,
    Wire,
    Symmetry,
    Mask,
    Close,
    Delete,
    Pin,
    Unpin,
    DockLeft,
    DockRight,
}
impl Glyph {
    fn lines(self) -> Vec<Vec<[f32; 2]>> {
        use Glyph::*;
        match self {
            Pin | Unpin => {
                let mut lines = vec![
                    vec![
                        [8., 3.],
                        [17., 3.],
                        [16., 11.],
                        [20., 15.],
                        [5., 15.],
                        [9., 11.],
                        [8., 3.],
                    ],
                    vec![[12., 15.], [12., 22.]],
                ];
                if matches!(self, Unpin) {
                    lines.push(vec![[3., 3.], [22., 22.]]);
                }
                lines
            }
            DockLeft | DockRight => {
                let mut lines = vec![
                    vec![[3., 4.], [21., 4.], [21., 20.], [3., 20.], [3., 4.]],
                    vec![[9., 4.], [9., 20.]],
                    vec![[5., 8.], [7., 8.]],
                    vec![[5., 12.], [7., 12.]],
                    vec![[5., 16.], [7., 16.]],
                ];
                if matches!(self, DockRight) {
                    for line in &mut lines {
                        for p in line {
                            p[0] = 24. - p[0];
                        }
                    }
                }
                lines
            }
            Slice => vec![
                vec![
                    [4., 7.],
                    [4., 4.],
                    [20., 4.],
                    [20., 20.],
                    [4., 20.],
                    [4., 17.],
                ],
                vec![[2., 16.], [22., 8.]],
            ],
            Close => vec![vec![[5., 5.], [19., 19.]], vec![[19., 5.], [5., 19.]]],
            Delete => vec![
                vec![[4., 6.], [20., 6.]],
                vec![[9., 6.], [9., 3.], [15., 3.], [15., 6.]],
                vec![[6., 6.], [7., 21.], [17., 21.], [18., 6.]],
                vec![[10., 10.], [10., 17.]],
                vec![[14., 10.], [14., 17.]],
            ],
            New => vec![
                vec![
                    [7., 3.],
                    [16., 3.],
                    [20., 7.],
                    [20., 21.],
                    [7., 21.],
                    [7., 3.],
                ],
                vec![[10., 12.], [17., 12.]],
                vec![[13.5, 8.5], [13.5, 15.5]],
            ],
            Open => vec![
                vec![
                    [3., 19.],
                    [3., 6.],
                    [10., 6.],
                    [12., 9.],
                    [21., 9.],
                    [18., 19.],
                    [3., 19.],
                ],
                vec![[3., 12.], [19., 12.]],
            ],
            Save => vec![
                vec![
                    [4., 3.],
                    [18., 3.],
                    [21., 6.],
                    [21., 21.],
                    [4., 21.],
                    [4., 3.],
                ],
                vec![[8., 3.], [8., 9.], [17., 9.], [17., 3.]],
                vec![[8., 21.], [8., 14.], [17., 14.], [17., 21.]],
            ],
            Export => vec![
                vec![[4., 13.], [4., 21.], [20., 21.], [20., 13.]],
                vec![[12., 16.], [12., 3.]],
                vec![[7., 8.], [12., 3.], [17., 8.]],
            ],
            Undo => vec![
                vec![[9., 4.], [4., 9.], [9., 14.]],
                vec![[4., 9.], [14., 9.], [18., 11.], [20., 15.], [19., 19.]],
            ],
            Redo => Undo
                .lines()
                .into_iter()
                .map(|v| v.into_iter().map(|[x, y]| [24. - x, y]).collect())
                .collect(),
            Settings => vec![
                vec![[4., 6.], [20., 6.]],
                vec![[4., 12.], [20., 12.]],
                vec![[4., 18.], [20., 18.]],
                vec![[9., 3.], [9., 9.]],
                vec![[16., 9.], [16., 15.]],
                vec![[8., 15.], [8., 21.]],
            ],
            Edit => vec![
                vec![
                    [4., 20.],
                    [6., 14.],
                    [17., 3.],
                    [21., 7.],
                    [10., 18.],
                    [4., 20.],
                ],
                vec![[14., 6.], [18., 10.]],
            ],
            Mesh | Primitive => vec![
                vec![
                    [12., 2.],
                    [21., 7.],
                    [21., 17.],
                    [12., 22.],
                    [3., 17.],
                    [3., 7.],
                    [12., 2.],
                ],
                vec![[3., 7.], [12., 12.], [21., 7.]],
                vec![[12., 12.], [12., 22.]],
            ],
            Transform => vec![
                vec![[12., 2.], [12., 22.]],
                vec![[2., 12.], [22., 12.]],
                vec![[8., 6.], [12., 2.], [16., 6.]],
                vec![[18., 8.], [22., 12.], [18., 16.]],
            ],
            Layers => vec![
                vec![[2., 7.], [12., 2.], [22., 7.], [12., 12.], [2., 7.]],
                vec![[2., 12.], [12., 17.], [22., 12.]],
                vec![[2., 17.], [12., 22.], [22., 17.]],
            ],
            Camera => vec![
                vec![
                    [3., 7.],
                    [8., 7.],
                    [10., 4.],
                    [15., 4.],
                    [17., 7.],
                    [21., 7.],
                    [21., 20.],
                    [3., 20.],
                    [3., 7.],
                ],
                circle(12., 13., 4.),
            ],
            Copy => vec![
                vec![[8., 8.], [21., 8.], [21., 21.], [8., 21.], [8., 8.]],
                vec![[16., 5.], [16., 3.], [3., 3.], [3., 16.], [5., 16.]],
            ],
            Restore => vec![
                vec![[3., 3.], [3., 9.], [9., 9.]],
                vec![
                    [3., 9.],
                    [6., 5.],
                    [12., 3.],
                    [18., 6.],
                    [21., 12.],
                    [18., 18.],
                    [12., 21.],
                    [6., 18.],
                ],
            ],
            Plus => vec![vec![[5., 12.], [19., 12.]], vec![[12., 5.], [12., 19.]]],
            Minus => vec![vec![[5., 12.], [19., 12.]]],
            View => vec![
                vec![
                    [2., 12.],
                    [7., 6.],
                    [12., 4.],
                    [17., 6.],
                    [22., 12.],
                    [17., 18.],
                    [12., 20.],
                    [7., 18.],
                    [2., 12.],
                ],
                circle(12., 12., 3.),
            ],
            Wire => vec![
                vec![
                    [3., 3.],
                    [21., 3.],
                    [21., 21.],
                    [3., 21.],
                    [3., 3.],
                    [21., 21.],
                ],
                vec![[21., 3.], [3., 21.]],
                vec![[12., 3.], [12., 21.]],
                vec![[3., 12.], [21., 12.]],
            ],
            Symmetry => vec![
                vec![[12., 2.], [12., 22.]],
                vec![[8., 5.], [3., 12.], [8., 19.], [8., 5.]],
                vec![[16., 5.], [21., 12.], [16., 19.], [16., 5.]],
            ],
            Mask => vec![
                vec![
                    [3., 5.],
                    [21., 5.],
                    [19., 17.],
                    [12., 21.],
                    [5., 17.],
                    [3., 5.],
                ],
                vec![[6., 9.], [9., 9.]],
                vec![[15., 9.], [18., 9.]],
            ],
        }
    }
}
fn circle(x: f32, y: f32, r: f32) -> Vec<[f32; 2]> {
    (0..=20)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 20.;
            [x + r * a.cos(), y + r * a.sin()]
        })
        .collect()
}
fn icon(ctx: &egui::Context, glyph: Glyph) -> egui::TextureId {
    let key = egui::Id::new(format!("action-icon-{glyph:?}"));
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return texture.id();
    }
    let segments: Vec<_> = glyph
        .lines()
        .into_iter()
        .flat_map(|line| line.windows(2).map(|p| (p[0], p[1])).collect::<Vec<_>>())
        .collect();
    let mut pixels = Vec::with_capacity(48 * 48 * 4);
    for y in 0..48 {
        for x in 0..48 {
            let p = egui::pos2((x as f32 + 0.5) / 2., (y as f32 + 0.5) / 2.);
            let distance = segments
                .iter()
                .map(|(a, b)| {
                    let a = egui::pos2(a[0], a[1]);
                    let b = egui::pos2(b[0], b[1]);
                    let v = b - a;
                    let t = ((p - a).dot(v) / v.length_sq().max(0.001)).clamp(0., 1.);
                    p.distance(a + v * t)
                })
                .fold(f32::INFINITY, f32::min);
            pixels.extend([
                255,
                255,
                255,
                ((1.15 - distance) * 2.).clamp(0., 1.).mul_add(255., 0.) as u8,
            ]);
        }
    }
    let texture = ctx.load_texture(
        format!("action-{glyph:?}"),
        egui::ColorImage::from_rgba_unmultiplied([48, 48], &pixels),
        egui::TextureOptions::LINEAR,
    );
    let id = texture.id();
    ctx.data_mut(|d| d.insert_temp(key, texture));
    id
}
pub(super) fn button(
    ui: &egui::Ui,
    glyph: Glyph,
    label: impl Into<String>,
) -> egui::Button<'static> {
    egui::Button::image_and_text(
        egui::Image::new((icon(ui.ctx(), glyph), Vec2::splat(16.))).tint(ui.visuals().text_color()),
        label.into(),
    )
    .min_size(egui::vec2(0., 28.))
}
/// Compact, font-independent control with a readable accessibility label and tooltip.
pub(super) fn icon_button(ui: &mut egui::Ui, glyph: Glyph, label: &str) -> egui::Response {
    let response = ui.add(
        egui::Button::image(
            egui::Image::new((icon(ui.ctx(), glyph), Vec2::splat(16.)))
                .tint(ui.visuals().text_color()),
        )
        .min_size(Vec2::splat(26.)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(("icon_button", label)), response.rect));
    response.on_hover_text(label)
}
pub(super) fn menu(
    ui: &mut egui::Ui,
    glyph: Glyph,
    label: &str,
    contents: impl FnOnce(&mut egui::Ui),
) -> bool {
    egui::containers::menu::MenuButton::from_button(button(ui, glyph, label))
        .ui(ui, contents)
        .0
        .clicked()
}
pub(super) fn theme(ctx: &egui::Context) {
    let key = egui::Id::new("clay-design-installed");
    if ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) {
        return;
    }
    ctx.style_mut(|style| {
        style.spacing.button_padding = egui::vec2(8., 5.);
        style.spacing.item_spacing = egui::vec2(6., 6.);
        style.spacing.interact_size.y = 28.;
        let v = &mut style.visuals;
        v.panel_fill = Color32::from_rgb(26, 29, 34);
        v.window_fill = v.panel_fill;
        v.extreme_bg_color = Color32::from_rgb(18, 21, 26);
        v.selection.bg_fill = Color32::from_rgb(29, 78, 94);
        v.selection.stroke = egui::Stroke::new(1_f32, Color32::from_rgb(88, 195, 215));
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(35, 40, 47);
        v.widgets.inactive.bg_fill = Color32::from_rgb(35, 40, 47);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1_f32, Color32::from_rgb(50, 57, 65));
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(48, 57, 66);
        v.widgets.hovered.bg_fill = v.widgets.hovered.weak_bg_fill;
        for state in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
        ] {
            state.corner_radius = egui::CornerRadius::same(4);
        }
    });
    ctx.data_mut(|d| d.insert_temp(key, true));
}
pub(super) fn brush_tile(
    ui: &mut egui::Ui,
    texture: egui::TextureId,
    name: &str,
    selected: bool,
    width: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 82.), egui::Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        ui.painter().rect(
            rect,
            5.,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        if selected {
            ui.painter().rect_stroke(
                rect,
                5.,
                ui.visuals().selection.stroke,
                egui::StrokeKind::Inside,
            );
        }
        let image = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + 31.),
            Vec2::splat(48.),
        );
        ui.painter().image(
            texture,
            image,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
        let mut job = egui::text::LayoutJob::simple_singleline(
            name.to_owned(),
            egui::FontId::proportional(13.),
            visuals.text_color(),
        );
        job.wrap.max_width = (width - 10.).max(1.);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(
            egui::pos2(rect.center().x - galley.size().x / 2., rect.top() + 61.),
            galley,
            visuals.text_color(),
        );
    }
    response.on_hover_text(name)
}

/// Apply workspace colours only when changed, preserving runtime style settings.
pub(super) fn appearance(ctx: &egui::Context, appearance: &crate::preferences::Appearance) {
    use crate::preferences::Appearance;
    let key = egui::Id::new("workspace_appearance");
    if ctx.data(|d| d.get_temp::<Appearance>(key)).as_ref() == Some(appearance) {
        return;
    }
    let colour = |rgb: [u8; 3]| Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    let panel = colour(appearance.panels);
    let button = colour(appearance.buttons);
    let accent = colour(appearance.accent);
    let text = colour(appearance.text);
    let mix = |a: Color32, b: Color32, amount: f32| {
        let a = a.to_array();
        let b = b.to_array();
        Color32::from_rgb(
            (a[0] as f32 * (1. - amount) + b[0] as f32 * amount) as u8,
            (a[1] as f32 * (1. - amount) + b[1] as f32 * amount) as u8,
            (a[2] as f32 * (1. - amount) + b[2] as f32 * amount) as u8,
        )
    };
    ctx.style_mut(|style| {
        style.spacing.item_spacing = Vec2::splat(appearance.spacing as f32);
        let v = &mut style.visuals;
        v.dark_mode = appearance.panels.iter().map(|c| *c as u32).sum::<u32>() < 384;
        v.panel_fill = panel;
        v.window_fill = panel;
        v.extreme_bg_color = mix(panel, button, 0.4);
        v.faint_bg_color = mix(panel, button, 0.3);
        v.override_text_color = Some(text);
        v.hyperlink_color = accent;
        v.selection.bg_fill = mix(panel, accent, 0.35);
        v.selection.stroke = egui::Stroke::new(1_f32, text);
        v.window_stroke = egui::Stroke::new(1_f32, mix(panel, text, 0.25));
        v.window_corner_radius = egui::CornerRadius::same(appearance.rounding);
        for (state, fill) in [
            (&mut v.widgets.noninteractive, panel),
            (&mut v.widgets.inactive, button),
            (&mut v.widgets.hovered, mix(button, accent, 0.25)),
            (&mut v.widgets.active, mix(button, accent, 0.45)),
            (&mut v.widgets.open, mix(button, accent, 0.25)),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = egui::Stroke::new(1_f32, mix(fill, text, 0.2));
            state.fg_stroke = egui::Stroke::new(1_f32, text);
            state.corner_radius = egui::CornerRadius::same(appearance.rounding);
        }
    });
    ctx.data_mut(|d| {
        d.insert_temp(key, appearance.clone());
        d.insert_temp(
            egui::Id::new("workspace_viewport_background"),
            colour(appearance.viewport),
        );
    });
}
