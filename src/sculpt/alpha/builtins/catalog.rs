//! Additional sculpting heights: deterministic patterns
//! with a protected feathered border for stamping.
use super::{Alpha, Asset, border, natural, pattern, ridge, segment_distance};

pub(super) fn append(assets: &mut Vec<Asset>) {
    add(assets, "Weathered bricks", "Masonry", |x, y| {
        bricks(x, y, false)
    });
    add(assets, "Broken bricks", "Masonry", |x, y| {
        bricks(x, y, true)
    });
    add(assets, "Old cobbles", "Masonry", natural::cobbles);
    add(assets, "Rough stone blocks", "Masonry", natural::blocks);
    add(assets, "Flagstones", "Masonry", natural::flagstones);
    let cracks = natural::plaster();
    add(assets, "Cracked plaster", "Masonry", |x, y| {
        natural::strokes(&cracks, x, y)
    });

    add(assets, "Circuit traces", "Circuits", |x, y| {
        circuit(x, y, false)
    });
    add(assets, "Circuit hub", "Circuits", |x, y| {
        circuit(x, y, true)
    });
    add(assets, "Microchip", "Circuits", |x, y| {
        let mut h = rounded_box(x, y, 0.34, 0.29, 0.035) * 0.75;
        for i in 0..9 {
            let p = -0.24 + i as f32 * 0.06;
            for side in [-1., 1.] {
                h = h.max(rounded_box(x - p, y - side * 0.40, 0.017, 0.10, 0.01));
                h = h.max(rounded_box(x - side * 0.45, y - p, 0.10, 0.017, 0.01));
            }
        }
        h.max(ridge(
            ((x + 0.20).powi(2) + (y - 0.16).powi(2)).sqrt(),
            0.045,
        ))
    });
    add(assets, "Data grid", "Circuits", |x, y| {
        let ix = (x * 8.).floor() as i32;
        let iy = (y * 8.).floor() as i32;
        if hash(ix, iy, 13) < 0.25 {
            return 0.;
        }
        rounded_box(
            (x * 8.).rem_euclid(1.) - 0.5,
            (y * 8.).rem_euclid(1.) - 0.5,
            0.30,
            0.30,
            0.12,
        ) * (0.4 + 0.6 * hash(ix, iy, 17))
    });
    add(assets, "Cable bundle", "Circuits", |x, y| {
        let phase = x * 7. + 0.25 * (y * 3.5).sin();
        let cord = (1. - ((phase.rem_euclid(1.) - 0.5) / 0.35).powi(2))
            .max(0.)
            .sqrt();
        cord * (0.78 + 0.22 * (y * 75. + x * 4.).cos().abs())
    });
    add(assets, "Riveted plate", "Circuits", |x, y| {
        let plate = rounded_box(x, y, 0.73, 0.73, 0.08);
        let inset = rounded_box(x, y, 0.60, 0.60, 0.03);
        let mut h = plate * (0.28 + 0.15 * inset);
        for i in 0..5 {
            let p = -0.58 + i as f32 * 0.29;
            for side in [-1., 1.] {
                h = h.max(ridge(
                    ((x - p).powi(2) + (y - side * 0.67).powi(2)).sqrt(),
                    0.045,
                ));
                h = h.max(ridge(
                    ((x - side * 0.67).powi(2) + (y - p).powi(2)).sqrt(),
                    0.045,
                ));
            }
        }
        h
    });

    add(assets, "Hull panels", "Sci-fi", |x, y| {
        let u = (x * 2.6 + (y * 1.8).floor() * 0.3).rem_euclid(1.) - 0.5;
        let v = (y * 1.8).rem_euclid(1.) - 0.5;
        let plate = rounded_box(u, v, 0.47, 0.46, 0.045);
        let inset = rounded_box(u + 0.05, v, 0.31, 0.29, 0.025);
        let bars =
            rounded_box(u - 0.36, v, 0.025, 0.25, 0.01) * (0.5 + 0.5 * (v * 55.).cos()).powf(3.);
        plate * (0.35 + 0.40 * inset + 0.25 * bars)
    });
    add(assets, "Hex armor", "Sci-fi", |x, y| {
        let (r, gap) = hex_cell(x * 5.3, y * 5.3);
        smooth((gap - 0.045) / 0.15) * (0.6 + 0.4 * (1. - r).max(0.))
    });
    add(assets, "Vent grille", "Sci-fi", |x, y| {
        let frame =
            rounded_box(x, y, 0.78, 0.72, 0.07) * (1. - rounded_box(x, y, 0.65, 0.59, 0.025));
        let slat = ridge(((y * 7.).rem_euclid(1.) - 0.5).abs(), 0.30)
            * rounded_box(x, y, 0.65, 0.58, 0.07);
        frame.max(slat * (0.75 + 0.25 * x))
    });
    add(assets, "Reactor rings", "Sci-fi", |x, y| {
        let r = (x * x + y * y).sqrt();
        let a = y.atan2(x);
        let rings = ridge((r - 0.34).abs(), 0.035).max(ridge((r - 0.67).abs(), 0.04));
        let segments =
            ridge((r - 0.50).abs(), 0.095) * smooth(((a * 6.).cos().abs() - 0.22) / 0.16);
        rings.max(segments * 0.75).max(ridge(r, 0.20) * 0.65)
    });
    add(assets, "Alien glyphs", "Sci-fi", |x, y| {
        let ix = (x * 2.8).floor() as i32;
        let iy = (y * 2.8).floor() as i32;
        let p = [
            (x * 2.8).rem_euclid(1.) - 0.5,
            (y * 2.8).rem_euclid(1.) - 0.5,
        ];
        let segments = [
            ([-0.27, -0.3], [-0.27, 0.3]),
            ([-0.27, 0.3], [0.22, 0.3]),
            ([-0.27, 0.0], [0.27, -0.3]),
            ([0.27, -0.3], [0.27, 0.15]),
            ([-0.2, -0.3], [0.13, -0.3]),
            ([-0.15, 0.0], [0.27, 0.15]),
        ];
        segments
            .into_iter()
            .enumerate()
            .map(|(i, (a, b))| {
                if i == 0 || hash(ix, iy, i as u32 + 33) > 0.35 {
                    ridge(segment_distance(p, a, b), 0.055)
                } else {
                    0.
                }
            })
            .fold(0_f32, f32::max)
    });
    add(assets, "Mech seams", "Sci-fi", |x, y| {
        let u = (x * 3.).rem_euclid(1.) - 0.5;
        let v = (y * 3.).rem_euclid(1.) - 0.5;
        let seam = ridge((u.abs() + v.abs() - 0.58).abs(), 0.055);
        seam.max(ridge((u.abs() - 0.42).abs(), 0.035) * ridge(v.abs(), 0.20))
    });

    let fine = natural::wrinkles(false);
    let deep = natural::wrinkles(true);
    let crows = natural::crows_feet();
    add(assets, "Fine wrinkles", "Skin", |x, y| {
        natural::strokes(&fine, x, y)
    });
    add(assets, "Deep folds", "Skin", |x, y| {
        natural::strokes(&deep, x, y)
    });
    add(assets, "Crow's feet", "Skin", |x, y| {
        natural::strokes(&crows, x, y)
    });

    add(assets, "Skin pores", "Skin", |x, y| {
        let (r, _) = cells(x * 16., y * 16., 67);
        ridge(r, 0.27) * (0.6 + 0.4 * grain(x * 14., y * 14., 19))
    });
    add(assets, "Fingerprint", "Skin", natural::fingerprint);
    add(assets, "Scar tissue", "Skin", natural::scar);

    add(assets, "Tree bark", "Organic", natural::bark);
    add(assets, "Rock strata", "Organic", natural::strata);
    add(assets, "Cracked earth", "Organic", natural::earth);

    add(assets, "Scales", "Organic", |x, y| {
        let (u, v, _) = stagger(x, y, 5., 6.);
        let r = (((u - 0.5) / 0.48).powi(2) + ((v - 0.3) / 0.66).powi(2)).sqrt();
        smooth((1. - r) / 0.15) * (0.35 + 0.65 * (1. - v))
    });
    let fur = natural::fur();
    add(assets, "Fur strands", "Organic", |x, y| {
        natural::strokes(&fur, x, y)
    });

    add(assets, "Woven cloth", "Materials", |x, y| {
        let ix = (x * 11.).floor() as i32;
        let iy = (y * 11.).floor() as i32;
        let u = (x * 11.).rem_euclid(1.) - 0.5;
        let v = (y * 11.).rem_euclid(1.) - 0.5;
        let (cross, along) = if (ix + iy).rem_euclid(2) == 0 {
            (u, v)
        } else {
            (v, u)
        };
        ridge(cross.abs(), 0.44) * (0.7 + 0.3 * (along * std::f32::consts::PI).cos())
    });
    add(assets, "Leather grain", "Materials", |x, y| {
        let (r, gap) = cells(x * 14., y * 14., 107);
        smooth(gap / 0.15) * (0.45 + 0.35 * (1. - r.min(1.)) + 0.20 * grain(x * 50., y * 50., 109))
    });
    add(assets, "Hammered metal", "Materials", natural::hammered);
}

fn add(assets: &mut Vec<Asset>, name: &str, category: &str, sample: impl Fn(f32, f32) -> f32) {
    let mut asset = pattern(name, |x, y| sample(x, y) * border(x, y));
    asset.category = format!("Built-in · {category}");
    if let Alpha::Image { pixels, .. } = &mut asset.image {
        let peak = pixels.iter().copied().fold(0_f32, f32::max).max(1e-6);
        for value in std::sync::Arc::make_mut(pixels) {
            *value /= peak;
        }
    }
    assets.push(asset);
}
pub(super) fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
pub(super) fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0x00ff_ffff) as f32 / 16777215.
}
pub(super) fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let u = smooth(x - x.floor());
    let v = smooth(y - y.floor());
    (hash(ix, iy, seed) * (1. - u) + hash(ix + 1, iy, seed) * u) * (1. - v)
        + (hash(ix, iy + 1, seed) * (1. - u) + hash(ix + 1, iy + 1, seed) * u) * v
}
pub(super) fn grain(x: f32, y: f32, seed: u32) -> f32 {
    0.55 * noise(x, y, seed)
        + 0.30 * noise(x * 2., y * 2., seed + 1)
        + 0.15 * noise(x * 4., y * 4., seed + 2)
}
fn stagger(x: f32, y: f32, cols: f32, rows: f32) -> (f32, f32, f32) {
    let row = (y * rows).floor() as i32;
    let u = x * cols + if row.rem_euclid(2) == 0 { 0. } else { 0.5 };
    (
        u.rem_euclid(1.),
        (y * rows).rem_euclid(1.),
        hash(u.floor() as i32, row, 3),
    )
}
fn bricks(x: f32, y: f32, broken: bool) -> f32 {
    let (u, v, seed) = stagger(x, y, 2.5, 5.);
    let edge = u.min(1. - u).min(v.min(1. - v) * 0.65);
    let erosion = 0.018 + 0.055 * grain(x * 29., y * 29., 5);
    let face = smooth((edge - erosion) / 0.045);
    let rough = 0.74 + 0.10 * grain(x * 48., y * 48., 7) + 0.16 * seed;
    let damage = if broken && seed > 0.36 {
        let crack =
            (u - (0.20 + 0.55 * seed) - 0.12 * (v * (5. + seed * 4.) + seed * 9.).sin()).abs();
        smooth((crack - 0.015) / 0.04) * smooth(((u - 0.5).abs() + v - 0.13 * seed) / 0.15)
    } else {
        1.
    };
    face * rough * damage
}
pub(super) fn cells(x: f32, y: f32, seed: u32) -> (f32, f32) {
    let mut first = f32::INFINITY;
    let mut second = f32::INFINITY;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let ix = x.floor() as i32 + dx;
            let iy = y.floor() as i32 + dy;
            let cx = ix as f32 + 0.2 + 0.6 * hash(ix, iy, seed);
            let cy = iy as f32 + 0.2 + 0.6 * hash(ix, iy, seed + 1);
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            if d < first {
                second = first;
                first = d;
            } else {
                second = second.min(d);
            }
        }
    }
    (first, second - first)
}
fn rounded_box(x: f32, y: f32, w: f32, h: f32, bevel: f32) -> f32 {
    smooth((w - x.abs()).min(h - y.abs()) / bevel)
}
fn hex_cell(x: f32, y: f32) -> (f32, f32) {
    let mut first = f32::INFINITY;
    let mut second = f32::INFINITY;
    let row = (y / 0.866).floor() as i32;
    for j in row - 1..=row + 2 {
        for i in x.floor() as i32 - 1..=x.floor() as i32 + 1 {
            let cx = i as f32 + if j.rem_euclid(2) == 0 { 0. } else { 0.5 };
            let cy = j as f32 * 0.866;
            let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            if r < first {
                second = first;
                first = r;
            } else {
                second = second.min(r);
            }
        }
    }
    (first, second - first)
}
fn circuit(x: f32, y: f32, hub: bool) -> f32 {
    let mut h = 0_f32;
    for side in [-1., 1.] {
        for i in 0..6 {
            let t = i as f32;
            let a = [side * (0.13 + t * 0.11), -0.84];
            let b = [a[0], -0.40 + t * 0.10];
            let c = [a[0] + side * 0.15, b[1] + 0.15];
            let d = [c[0], 0.55 + 0.045 * t];
            let p = if hub { [y, x] } else { [x, y] };
            for (a, b) in [(a, b), (b, c), (c, d)] {
                h = h.max(ridge(segment_distance(p, a, b), 0.014));
            }
            let r = ((p[0] - d[0]).powi(2) + (p[1] - d[1]).powi(2)).sqrt();
            h = h.max(ridge((r - 0.028).abs(), 0.012));
        }
    }
    if hub {
        h = h.max(rounded_box(x, y, 0.22, 0.28, 0.04) * 0.7);
        h = h.max(ridge((x.abs().max(y.abs()) - 0.31).abs(), 0.02));
    }
    h
}
