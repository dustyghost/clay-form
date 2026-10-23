//! Material-specific relief and tapered creases; no baked-in lighting.
use super::{
    catalog::{cells, grain, hash, noise, smooth},
    ridge,
};

pub(super) struct Stroke {
    points: Vec<[f32; 2]>,
    width: f32,
    height: f32,
    bounds: [f32; 4],
}
impl Stroke {
    fn new(points: Vec<[f32; 2]>, width: f32, height: f32) -> Self {
        let mut bounds = [
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ];
        for p in &points {
            bounds[0] = bounds[0].min(p[0] - width);
            bounds[1] = bounds[1].max(p[0] + width);
            bounds[2] = bounds[2].min(p[1] - width);
            bounds[3] = bounds[3].max(p[1] + width);
        }
        Self {
            points,
            width,
            height,
            bounds,
        }
    }
    fn sample(&self, x: f32, y: f32) -> f32 {
        if x < self.bounds[0] || x > self.bounds[1] || y < self.bounds[2] || y > self.bounds[3] {
            return 0.;
        }
        let mut value = 0_f32;
        for (i, pair) in self.points.windows(2).enumerate() {
            let [a, b] = [pair[0], pair[1]];
            let d = [b[0] - a[0], b[1] - a[1]];
            let u = (((x - a[0]) * d[0] + (y - a[1]) * d[1])
                / (d[0] * d[0] + d[1] * d[1]).max(1e-12))
            .clamp(0., 1.);
            let t = (i as f32 + u) / (self.points.len() - 1) as f32;
            let envelope = smooth(t / 0.24) * smooth((1. - t) / 0.32);
            let distance = ((x - a[0] - u * d[0]).powi(2) + (y - a[1] - u * d[1]).powi(2)).sqrt();
            value = value.max(
                self.height * envelope * ridge(distance, self.width * (0.18 + 0.82 * envelope)),
            );
        }
        value
    }
}
pub(super) fn strokes(paths: &[Stroke], x: f32, y: f32) -> f32 {
    paths.iter().map(|p| p.sample(x, y)).fold(0., f32::max)
}
pub(super) fn crows_feet() -> Vec<Stroke> {
    let mut paths = Vec::new();
    for (i, (angle, length)) in [
        (-0.61f32, 1.12),
        (-0.29, 1.39),
        (0.02, 1.24),
        (0.31, 1.45),
        (0.65, 1.02),
    ]
    .into_iter()
    .enumerate()
    {
        let k = i as i32;
        let start = [
            -0.76 + 0.28 * hash(k, 0, 5),
            -0.04 + angle * 0.16 + 0.04 * (hash(k, 0, 7) - 0.5),
        ];
        let points = (0..=20)
            .map(|j| {
                let t = j as f32 / 20.;
                let bend =
                    0.06 * (t * 4.1 + i as f32).sin() * t + 0.018 * (t * 12. + i as f32).sin() * t;
                [
                    start[0] + length * t * angle.cos() - bend * angle.sin(),
                    start[1] + length * t * angle.sin() + bend * angle.cos(),
                ]
            })
            .collect();
        paths.push(Stroke::new(
            points,
            0.013 + 0.009 * hash(k, 0, 11),
            0.6 + 0.4 * hash(k, 0, 13),
        ));
    }
    // Short, weaker secondary wrinkles, rather than more equal spokes.
    for i in 0..3 {
        let x = -0.12 + i as f32 * 0.20;
        let y = -0.22 + i as f32 * 0.22;
        paths.push(Stroke::new(
            (0..=10)
                .map(|j| {
                    let t = j as f32 / 10.;
                    [x + t * 0.42, y + t * 0.13 + 0.035 * (t * 4.).sin()]
                })
                .collect(),
            0.011,
            0.32,
        ));
    }
    paths
}
pub(super) fn wrinkles(deep: bool) -> Vec<Stroke> {
    let count = if deep { 5 } else { 15 };
    (0..count)
        .map(|i| {
            let seed = hash(i, 0, 211);
            let start = -0.93 + 0.28 * hash(i, 0, 213);
            let end = 0.48 + 0.46 * hash(i, 0, 215);
            let y = -0.76 + 1.52 * i as f32 / (count - 1) as f32;
            let points = (0..=24)
                .map(|j| {
                    let t = j as f32 / 24.;
                    let x = start + (end - start) * t;
                    [
                        x,
                        y + 0.055 * (x * 3.4 + seed * 6.).sin()
                            + 0.022 * (x * 9. + seed * 13.).sin()
                            + 0.035 * (noise(x * 5., i as f32, 217) - 0.5),
                    ]
                })
                .collect();
            Stroke::new(
                points,
                if deep {
                    0.045 + 0.032 * seed
                } else {
                    0.009 + 0.009 * seed
                },
                0.45 + 0.55 * hash(i, 0, 219),
            )
        })
        .collect()
}
pub(super) fn plaster() -> Vec<Stroke> {
    let mut paths = Vec::new();
    for (i, (start, end)) in [
        ([-0.86, 0.63], [0.78, -0.32]),
        ([-0.65, -0.79], [0.24, 0.70]),
        ([0.08, -0.14], [0.84, 0.72]),
    ]
    .into_iter()
    .enumerate()
    {
        paths.push(Stroke::new(
            (0..=18)
                .map(|j| {
                    let t = j as f32 / 18.;
                    let kink = (hash(j, i as i32, 221) - 0.5) * 0.085;
                    [
                        start[0] + (end[0] - start[0]) * t + kink,
                        start[1] + (end[1] - start[1]) * t + kink * 0.65,
                    ]
                })
                .collect(),
            0.012 + 0.004 * i as f32,
            1. - i as f32 * 0.16,
        ));
    }
    for i in 0..8 {
        let t = 0.15 + i as f32 * 0.09;
        let start = [-0.86 + 1.64 * t, 0.63 - 0.95 * t];
        let side = if i % 2 == 0 { 1. } else { -1. };
        let length = 0.18 + 0.26 * hash(i, 0, 223);
        paths.push(Stroke::new(
            (0..=8)
                .map(|j| {
                    let u = j as f32 / 8.;
                    [
                        start[0] + u * length * 0.6 + 0.018 * (u * 12.).sin(),
                        start[1] + side * u * length,
                    ]
                })
                .collect(),
            0.006 + 0.004 * hash(i, 0, 225),
            0.45,
        ));
    }
    paths
}
pub(super) fn cobbles(x: f32, y: f32) -> f32 {
    let row = (y * 4.).floor() as i32;
    let mut value = 0_f32;
    for j in row - 1..=row + 1 {
        let shift = if j.rem_euclid(2) == 0 { 0. } else { 0.5 };
        let col = (x * 3.2 - shift).floor() as i32;
        for i in col - 1..=col + 1 {
            let cx = (i as f32 + 0.5 + shift + 0.35 * (hash(i, j, 231) - 0.5)) / 3.2;
            let cy = (j as f32 + 0.5 + 0.30 * (hash(i, j, 233) - 0.5)) / 4.;
            let u = (x - cx) / (0.112 + 0.030 * hash(i, j, 235));
            let v = (y - cy) / (0.088 + 0.025 * hash(i, j, 237));
            let r = (u.abs().powf(2.7) + v.abs().powf(2.7)).powf(1. / 2.7);
            let dome = (1. - r * r).max(0.).powf(0.28);
            value = value.max(
                dome * (0.66 + 0.27 * hash(i, j, 239))
                    * (0.94 + 0.06 * grain(x * 28., y * 28., 241)),
            );
        }
    }
    value
}
pub(super) fn flagstones(x: f32, y: f32) -> f32 {
    let u = x * 2.7 + 0.20 * (noise(x * 5., y * 5., 243) - 0.5);
    let v = y * 2.4 + 0.20 * (noise(x * 5., y * 5., 245) - 0.5);
    let (_, gap) = cells(u, v, 247);
    smooth((gap - 0.022) / 0.055)
        * (0.73 + 0.15 * noise(x * 2., y * 2., 249) + 0.12 * grain(x * 17., y * 17., 251))
}
pub(super) fn earth(x: f32, y: f32) -> f32 {
    let u = x * 3.2 + 0.28 * (noise(x * 4., y * 4., 253) - 0.5);
    let v = y * 3.4 + 0.28 * (noise(x * 4., y * 4., 255) - 0.5);
    let (_, gap) = cells(u, v, 257);
    let shard = smooth((gap - 0.022) / 0.065);
    let curled = 0.34 * (-((gap - 0.105) / 0.085).powi(2)).exp();
    shard * (0.48 + curled + 0.12 * grain(x * 30., y * 30., 259))
}
pub(super) fn wood(x: f32, y: f32) -> f32 {
    let u = x + 0.045 * (y * 2.3).sin() + 0.018 * (noise(x * 3., y * 5., 261) - 0.5);
    let dx = u - 0.22;
    let dy = y + 0.13;
    let r2 = (dx / 0.17).powi(2) + (dy / 0.37).powi(2);
    let flow = u - dx / r2.max(1.);
    let phase = flow * 14.
        + 0.55 * (flow * 7.).sin()
        + 0.28 * grain(x * 5., y * 3., 263)
        + 0.10 * (y * 9. + x * 3.).sin();
    let ring = (phase.rem_euclid(1.) - 0.5).abs();
    let lines = ridge(ring, 0.18 + 0.06 * noise(x * 3., y * 4., 265));
    let fine = ridge(((phase * 2. + 0.17).rem_euclid(1.) - 0.5).abs(), 0.12) * 0.22;
    let knot = ridge(((r2.sqrt() * 2.8).rem_euclid(1.) - 0.5).abs(), 0.3) * 0.7;
    let outside = smooth((r2 - 0.85) / 0.45);
    ((lines.max(fine) * (0.70 + 0.30 * grain(x * 20., y * 2., 267))) * outside
        + knot * (1. - outside))
        * super::border(x, y)
}
pub(super) fn pebbles() -> impl Fn(f32, f32) -> f32 {
    let mut stones: Vec<(f32, f32, f32, f32, f32, f32, f32)> = Vec::new();
    for i in 0..220 {
        let cx = -0.86 + 1.72 * hash(i, 0, 269);
        let cy = -0.86 + 1.72 * hash(i, 0, 271);
        let rx = 0.085 + 0.075 * hash(i, 0, 273);
        let ry = rx * (0.72 + 0.4 * hash(i, 0, 275));
        if stones.iter().any(|&(x, y, r, _, _, _, _)| {
            ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() < 0.90 * (r + rx)
        }) {
            continue;
        }
        let angle = hash(i, 0, 277) * std::f32::consts::TAU;
        stones.push((
            cx,
            cy,
            rx,
            ry,
            angle.cos(),
            angle.sin(),
            0.55 + 0.45 * hash(i, 0, 279),
        ));
        if stones.len() == 42 {
            break;
        }
    }
    move |x, y| {
        let mut value = 0_f32;
        for &(cx, cy, rx, ry, c, s, h) in &stones {
            let dx = x - cx;
            let dy = y - cy;
            let r2 = ((dx * c + dy * s) / rx).powi(2) + ((-dx * s + dy * c) / ry).powi(2);
            value = value.max((1. - r2).max(0.).powf(0.65) * h);
        }
        value * super::border(x, y)
    }
}
pub(super) fn blocks(x: f32, y: f32) -> f32 {
    let row = (y * 2.8).floor() as i32;
    let u = (x * (1.8 + 0.75 * hash(row, 0, 321)) + hash(row, 0, 323)).rem_euclid(1.);
    let v = (y * 2.8).rem_euclid(1.);
    let edge = u.min(1. - u).min(v.min(1. - v) * 0.8);
    smooth((edge - 0.025 - 0.045 * grain(x * 24., y * 24., 325)) / 0.075)
        * (0.67 + 0.20 * grain(x * 10., y * 10., 327) + 0.13 * noise(x * 2., y * 2., 329))
}
pub(super) fn fingerprint(x: f32, y: f32) -> f32 {
    let u = x + 0.09 * (y * 2.4).sin();
    let r = (u * u + (y + 0.24).max(0.).powi(2) * 0.55).sqrt();
    let phase = r * 19. + 0.22 * noise(x * 2., y * 3., 331);
    ridge((phase.rem_euclid(1.) - 0.5).abs(), 0.23)
        * smooth((1. - (x / 0.78).powi(2) - ((y + 0.03) / 0.92).powi(2)) / 0.23)
}
pub(super) fn scar(x: f32, y: f32) -> f32 {
    let path = 0.075 * (y * 4.).sin() + 0.025 * (noise(y * 9., 0., 333) - 0.5);
    let width = 0.035 + 0.03 * noise(y * 8., 0., 335);
    let profile = (-2. * ((x - path) / width).powi(2)).exp();
    profile * (0.65 + 0.35 * grain(x * 26., y * 17., 337)) * smooth((0.85 - y.abs()) / 0.25)
}
pub(super) fn bark(x: f32, y: f32) -> f32 {
    let u = x * 5.2 + 0.25 * noise(x * 4., y * 3., 281);
    let v = y * 1.8 + 0.35 * noise(x * 3., y * 3., 283);
    let (_, gap) = cells(u, v, 285);
    let plate = smooth((gap - 0.025) / 0.13);
    let fissures = ridge(
        ((x * 19. + 0.5 * noise(x * 6., y * 2., 287)).rem_euclid(1.) - 0.5).abs(),
        0.1,
    );
    plate * (0.55 + 0.30 * grain(x * 20., y * 8., 289)) * (1. - 0.18 * fissures)
}
pub(super) fn strata(x: f32, y: f32) -> f32 {
    let wave = y * 4.2 + 0.70 * noise(x * 1.4, y * 1.1, 291) + 0.18 * noise(x * 7., y * 3., 293);
    let layer = wave.floor() as i32;
    let t = wave.rem_euclid(1.);
    smooth(t / (0.06 + 0.1 * hash(layer, 0, 295)))
        * smooth((1. - t) / 0.65)
        * (0.55 + 0.24 * hash(layer, 0, 297) + 0.21 * grain(x * 27., y * 16., 299))
}
pub(super) fn fur() -> Vec<Stroke> {
    let mut paths = Vec::new();
    for row in 0..6 {
        for col in 0..18 {
            let x = -0.95 + col as f32 * 0.112 + 0.07 * (hash(col, row, 301) - 0.5);
            let y = -0.94 + row as f32 * 0.32 + 0.18 * hash(col, row, 303);
            let length = 0.25 + 0.36 * hash(col, row, 305);
            let bend = 0.07 + 0.10 * hash(col, row, 307);
            paths.push(Stroke::new(
                (0..=8)
                    .map(|j| {
                        let t = j as f32 / 8.;
                        [
                            x + bend * t * t + 0.018 * (t * 3. + row as f32).sin(),
                            y + length * t,
                        ]
                    })
                    .collect(),
                0.008 + 0.008 * hash(col, row, 309),
                0.5 + 0.5 * hash(col, row, 311),
            ));
        }
    }
    paths
}
pub(super) fn hammered(x: f32, y: f32) -> f32 {
    let mut dent = 0_f32;
    for j in -4..=4 {
        for i in -4..=4 {
            let cx = i as f32 * 0.23 + 0.16 * (hash(i, j, 313) - 0.5);
            let cy = j as f32 * 0.23 + 0.16 * (hash(i, j, 315) - 0.5);
            let radius = 0.11 + 0.08 * hash(i, j, 317);
            let r = (((x - cx) / radius).powi(2) + ((y - cy) / (radius * 0.85)).powi(2)).sqrt();
            dent = dent.max((1. - r * r).max(0.).powi(2) * (0.4 + 0.4 * hash(i, j, 319)));
        }
    }
    1. - dent
}
