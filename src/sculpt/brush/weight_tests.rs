//! Frozen pre-preparation formula and an opt-in sampling benchmark.
use super::*;
use std::{hint::black_box, time::Instant};

fn legacy_weight(brush: &Brush, x: f32, y: f32) -> f32 {
    let (s, c) = brush.rotation.to_radians().sin_cos();
    let (x, y) = (x * c - y * s, x * s + y * c);
    let r = (x * x + y * y).sqrt();
    if r >= 1. {
        return 0.;
    }
    let fade = ((1. - r) / (1. - brush.inner_radius).max(0.0001))
        .clamp(0., 1.)
        .powf(brush.falloff);
    match &brush.alpha {
        Alpha::Soft => fade,
        Alpha::Hard => 1.,
        Alpha::Square => {
            if x.abs().max(y.abs()) < 0.7 {
                1.
            } else {
                0.
            }
        }
        Alpha::Image {
            width,
            height,
            pixels,
        } => {
            let px = (x + 1.) * 0.5 * (*width - 1) as f32;
            let py = (1. - y) * 0.5 * (*height - 1) as f32;
            let ix = px.floor() as usize;
            let iy = py.floor() as usize;
            let nx = (ix + 1).min(*width - 1);
            let ny = (iy + 1).min(*height - 1);
            let tx = px - ix as f32;
            let ty = py - iy as f32;
            let top = pixels[iy * width + ix] * (1. - tx) + pixels[iy * width + nx] * tx;
            let bottom = pixels[ny * width + ix] * (1. - tx) + pixels[ny * width + nx] * tx;
            (top * (1. - ty) + bottom * ty) * fade
        }
    }
}

fn alphas() -> Vec<Alpha> {
    vec![
        Alpha::Soft,
        Alpha::Hard,
        Alpha::Square,
        Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.37].into(),
        },
        Alpha::Image {
            width: 3,
            height: 2,
            pixels: vec![0., 0.2, 1., 0.7, 0.4, 0.1].into(),
        },
    ]
}

#[test]
fn prepared_weights_match_original_bits_at_edges_and_across_alpha_settings() {
    let mut brush = Brush::new(Kind::Standard);
    let mut coordinates: Vec<_> = (-17..=17).map(|i| i as f32 / 16.).collect();
    coordinates.extend([
        -0.,
        f32::from_bits(1),
        0.6999999,
        0.7,
        0.7000001,
        f32::from_bits(1_f32.to_bits() - 1),
    ]);
    for alpha in alphas() {
        brush.alpha = alpha;
        for rotation in [-720., -37., -0., 0., 37., 90., 180., 359.9] {
            brush.rotation = rotation;
            for inner in [0., 0.35, 0.9999, 1.] {
                brush.inner_radius = inner;
                for falloff in [0., 0.25, 1., 1.7, 4.] {
                    brush.falloff = falloff;
                    let prepared = brush.prepare_weight();
                    for &x in &coordinates {
                        for &y in &coordinates {
                            assert_eq!(
                                prepared.weight(x, y).to_bits(),
                                legacy_weight(&brush, x, y).to_bits(),
                                "rotation={rotation} inner={inner} falloff={falloff} point={x},{y}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn samples(brush: &Brush, points: &[(f32, f32)], prepared: bool) -> f32 {
    let mut sum = 0.;
    if prepared {
        let weight = brush.prepare_weight();
        for &(x, y) in points {
            sum += weight.weight(x, y);
        }
    } else {
        for &(x, y) in points {
            sum += legacy_weight(brush, x, y);
        }
    }
    sum
}

#[test]
#[ignore = "opt-in brush sampling timing; no wall-clock assertion"]
fn profile_prepared_brush_weights() {
    let points: Vec<_> = (0..256)
        .flat_map(|y| (0..256).map(move |x| (x as f32 / 127.5 - 1., y as f32 / 127.5 - 1.)))
        .collect();
    let mut rows = Vec::new();
    for (alpha_index, alpha) in alphas().into_iter().enumerate() {
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = alpha;
        brush.rotation = 37.;
        brush.inner_radius = 0.35;
        brush.falloff = 1.7;
        let expected = samples(&brush, &points, false);
        assert_eq!(expected.to_bits(), samples(&brush, &points, true).to_bits());
        for trial in 0..11 {
            // Alternate order; discard one warm-up pair.
            for prepared in if trial % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                let result = samples(black_box(&brush), black_box(&points), prepared);
                let ms = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(black_box(result).to_bits(), expected.to_bits());
                if trial > 0 {
                    rows.push(serde_json::json!({
                        "alpha_index": alpha_index, "prepared": prepared, "trial": trial,
                        "samples": points.len(), "ms": ms, "sum_bits": result.to_bits()
                    }));
                }
            }
        }
    }
    let path = std::env::var("CLAY_BRUSH_WEIGHT_PROFILE").expect("profile output path");
    std::fs::write(path, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}
