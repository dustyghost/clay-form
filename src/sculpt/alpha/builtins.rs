//! Small deterministic height patterns, cached once and embedded when used.
use super::{Alpha, Asset};
use std::sync::OnceLock;
mod catalog;

pub fn builtins() -> &'static [Asset] {
    static ASSETS: OnceLock<Vec<Asset>> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let mut assets: Vec<_> = [
            ("Soft", Alpha::Soft),
            ("Hard", Alpha::Hard),
            ("Square", Alpha::Square),
        ]
        .into_iter()
        .map(|(name, image)| Asset {
            name: name.into(),
            category: "Built-in".into(),
            image,
        })
        .collect();
        // A logarithmic spiral with a gently widening raised ribbon.
        let spiral: Vec<_> = (0..=320)
            .map(|i| {
                let angle = i as f32 / 320. * 15.7;
                let radius = 0.045 * (0.185 * angle).exp();
                [radius * angle.cos(), radius * angle.sin()]
            })
            .collect();
        assets.push(pattern("Nature's curve", |x, y| {
            spiral
                .windows(2)
                .map(|line| {
                    ridge(
                        segment_distance([x, y], line[0], line[1]),
                        0.022 + 0.022 * (x * x + y * y).sqrt(),
                    )
                })
                .fold(0_f32, f32::max)
        }));
        assets.push(pattern("Leaf veins", |x, y| {
            let t = ((y + 0.86) / 1.72).clamp(0., 1.);
            let width = 0.52 * (std::f32::consts::PI * t).sin().max(0.).powf(0.8);
            if y.abs() >= 0.86 || x.abs() >= width {
                return 0.;
            }
            let edge = ((width - x.abs()) / 0.03).clamp(0., 1.);
            let mut vein = ridge(x.abs(), 0.027);
            for side in [-1., 1.] {
                for i in 0..6 {
                    let base = -0.65 + i as f32 * 0.23;
                    let end = base + 0.25;
                    let extent = 0.48
                        * (std::f32::consts::PI * ((end + 0.86) / 1.72))
                            .sin()
                            .max(0.)
                            .powf(0.8);
                    vein = vein.max(ridge(
                        segment_distance([x, y], [0., base], [side * extent, end]),
                        0.018,
                    ));
                }
            }
            (0.16 + 0.84 * vein) * edge
        }));
        assets.push(pattern("Wood grain", |x, y| {
            let knot = (-((x - 0.2).powi(2) * 7. + (y + 0.15).powi(2) * 2.)).exp();
            let phase = x * 36. + 2.5 * (y * 3.).sin() + 0.8 * (y * 8.).sin() + 7. * knot;
            (0.5 + 0.5 * phase.sin()).powf(4.) * border(x, y)
        }));
        assets.push(pattern("Pebbles", |x, y| {
            let mut value = 0_f32;
            for row in 0..5 {
                for col in 0..5 {
                    let seed = (row * 7 + col * 13) as f32;
                    let cx = -0.78 + col as f32 * 0.39 + if row % 2 == 0 { 0. } else { 0.13 };
                    let cy = -0.78 + row as f32 * 0.39;
                    let rx = 0.16 + 0.025 * seed.sin();
                    let ry = 0.15 + 0.025 * (seed * 1.7).cos();
                    let r2 = ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2);
                    value = value.max((1. - r2).max(0.).powf(0.65));
                }
            }
            value * border(x, y)
        }));
        catalog::append(&mut assets);
        assets
    })
}
fn pattern(name: &str, sample: impl Fn(f32, f32) -> f32) -> Asset {
    const SIDE: usize = 256;
    Asset {
        name: name.into(),
        category: "Built-in · Nature".into(),
        image: Alpha::Image {
            width: SIDE,
            height: SIDE,
            pixels: (0..SIDE * SIDE)
                .map(|i| {
                    sample(
                        (i % SIDE) as f32 / (SIDE - 1) as f32 * 2. - 1.,
                        1. - (i / SIDE) as f32 / (SIDE - 1) as f32 * 2.,
                    )
                    .clamp(0., 1.)
                })
                .collect::<Vec<_>>()
                .into(),
        },
    }
}
fn border(x: f32, y: f32) -> f32 {
    ((0.94 - x.abs().max(y.abs())) / 0.12).clamp(0., 1.)
}
fn ridge(distance: f32, width: f32) -> f32 {
    let t = (1. - distance / width).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1])
        / (d[0] * d[0] + d[1] * d[1]).max(1e-12))
    .clamp(0., 1.);
    ((p[0] - a[0] - t * d[0]).powi(2) + (p[1] - a[1] - t * d[1]).powi(2)).sqrt()
}
