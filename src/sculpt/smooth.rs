//! Local implicit surface fairing with fixed vertices outside the brush.
//!
//! Inspired by Desbrun et al. (1999), Implicit Fairing of Irregular Meshes.
//! Lumped area and a brush-space timestep make the diffusion scale independent
//! of triangle density. All three displacement coordinates share a nonnegative
//! cotangent operator. Restricting motion to the old vertex normals traps steep
//! grooves: their sidewalls move sideways into slivers instead of relaxing.
//! Weight clamping and deformation guards make this a modified operator, not
//! the paper's unmodified cotangent flow or a volume-preserving filter.
use super::{Brush, Dab, Mesh, alpha::Projection};

struct Edge {
    a: usize,
    b: usize,
    weight: f64,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

// Solve displacement for one coordinate. Inactive endpoints are fixed.
fn solve(diagonal: &[f64], edges: &[Edge], rhs: &[f64]) -> Vec<f64> {
    let apply = |x: &[f64], out: &mut [f64]| {
        for ((out, x), d) in out.iter_mut().zip(x).zip(diagonal) {
            *out = x * d;
        }
        for e in edges {
            out[e.a] -= e.weight * x[e.b];
            out[e.b] -= e.weight * x[e.a];
        }
    };
    let mut x = vec![0.; rhs.len()];
    let mut residual = rhs.to_vec();
    let mut z: Vec<_> = residual.iter().zip(diagonal).map(|(r, d)| r / d).collect();
    let mut p = z.clone();
    let mut rz = dot(&residual, &z);
    let tolerance = dot(rhs, rhs) * 1e-10;
    let mut ap = vec![0.; rhs.len()];
    for _ in 0..48 {
        if dot(&residual, &residual) <= tolerance || rz <= 0. {
            break;
        }
        apply(&p, &mut ap);
        let denominator = dot(&p, &ap);
        if !denominator.is_finite() || denominator <= 0. {
            break;
        }
        let alpha = rz / denominator;
        for i in 0..x.len() {
            x[i] += alpha * p[i];
            residual[i] -= alpha * ap[i];
            z[i] = residual[i] / diagonal[i];
        }
        let next = dot(&residual, &z);
        let beta = next / rz;
        for i in 0..p.len() {
            p[i] = z[i] + beta * p[i];
        }
        rz = next;
    }
    x
}

impl Mesh {
    pub(super) fn smooth_surface(
        &mut self,
        brush: &Brush,
        dabs: &[Dab],
        stencils: &[Projection],
    ) -> bool {
        if brush.strength <= 0. || dabs.is_empty() {
            return false;
        }
        let normals = self.normals();
        // Classify the local surface sheet, not each individual groove wall.
        // An isolated turned normal must not pin a speck forever. Averaging
        // only topological neighbors keeps a nearby opposing sheet separate.
        let mut facing = normals.clone();
        for &[a, b, c] in &self.triangles {
            let sum = normals[a] + normals[b] + normals[c];
            for i in [a, b, c] {
                facing[i] = facing[i] + sum;
            }
        }
        let mut permitted = self.sculptable_vertices();
        for &[a, b] in &self.creases {
            permitted[a] = false;
            permitted[b] = false;
        }
        let mut indices = vec![usize::MAX; self.vertices.len()];
        let mut active = Vec::new();
        let mut times = Vec::new();
        let mut limits = Vec::new();
        let mut signs = Vec::new();
        let footprint = brush.prepare_weight();
        let pressures: Vec<_> = dabs.iter().map(|dab| brush.dab_pressure(dab)).collect();
        for (i, &position) in self.vertices.iter().enumerate() {
            if !permitted[i] || self.mask[i] >= 1. {
                continue;
            }
            let mut strongest = 0.;
            let mut radius = 0.;
            let mut sign = 1.;
            for (j, dab) in dabs.iter().enumerate() {
                let offset = position - dab.center;
                // Include steep sidewalls, but not the back of a thin surface.
                if offset.len() > dab.radius * 1.5 || facing[i].dot(dab.normal) < 0. {
                    continue;
                }
                let pressure = pressures[j];
                let influence = footprint.weight(
                    offset.dot(dab.right) / dab.radius,
                    offset.dot(dab.up) / dab.radius,
                ) * pressure
                    * brush.strength
                    * (1. - self.mask[i])
                    * stencils.get(j).map_or(1., |s| s.weight(position));
                if influence > strongest {
                    strongest = influence;
                    radius = dab.radius;
                    sign = if brush.subtract ^ dab.invert { -1. } else { 1. };
                }
            }
            if strongest > 1e-7 && radius > 0. {
                indices[i] = active.len();
                active.push(i);
                times.push((radius as f64).powi(2) * 0.01 * strongest as f64);
                limits.push(radius * 0.1);
                signs.push(sign);
            }
        }
        if active.is_empty() {
            return false;
        }
        let mut mass = vec![0.; active.len()];
        let mut raw_edges = Vec::new();
        // One face scan includes the complete one-ring, including fixed neighbors
        // outside the brush. Subsequent solver iterations visit only this patch.
        for &[a, b, c] in &self.triangles {
            if [a, b, c].iter().all(|&i| indices[i] == usize::MAX) {
                continue;
            }
            let area = (self.vertices[b] - self.vertices[a])
                .cross(self.vertices[c] - self.vertices[a])
                .len() as f64;
            if area <= 0. {
                continue;
            }
            for (i, j, k) in [(a, b, c), (b, c, a), (c, a, b)] {
                if indices[i] != usize::MAX {
                    mass[indices[i]] += area / 6.;
                }
                let weight = (self.vertices[i] - self.vertices[k])
                    .dot(self.vertices[j] - self.vertices[k]) as f64
                    / (2. * area);
                raw_edges.push((i.min(j), i.max(j), weight));
            }
        }
        raw_edges.sort_unstable_by_key(|&(a, b, _)| (a, b));
        let mut diagonal: Vec<_> = mass.iter().zip(&times).map(|(m, t)| m / t).collect();
        let mut rhs = [
            vec![0.; active.len()],
            vec![0.; active.len()],
            vec![0.; active.len()],
        ];
        let mut edges = Vec::new();
        for pair in raw_edges.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
            let (a, b, _) = pair[0];
            // Clamp the sum across the edge, not each triangle's cotangent.
            // Negative weights on non-Delaunay edges can cause overshoot.
            let weight = pair.iter().map(|e| e.2).sum::<f64>().max(0.);
            if weight == 0. {
                continue;
            }
            for (i, j) in [(a, b), (b, a)] {
                let index = indices[i];
                if index != usize::MAX {
                    diagonal[index] += weight;
                    let delta = self.vertices[j] - self.vertices[i];
                    for (axis, value) in [delta.0, delta.1, delta.2].into_iter().enumerate() {
                        rhs[axis][index] += weight * value as f64;
                    }
                }
            }
            if indices[a] != usize::MAX && indices[b] != usize::MAX {
                edges.push(Edge {
                    a: indices[a],
                    b: indices[b],
                    weight,
                });
            }
        }
        for d in &mut diagonal {
            *d = d.max(1e-30);
        }
        let displacement = rhs.map(|rhs| solve(&diagonal, &edges, &rhs));
        let before = self.vertices.clone();
        for (j, &i) in active.iter().enumerate() {
            let delta = super::Vec3(
                displacement[0][j] as f32,
                displacement[1][j] as f32,
                displacement[2][j] as f32,
            );
            let length = delta.len();
            if length.is_finite() {
                self.vertices[i] = self.vertices[i]
                    + delta * (limits[j] / length.max(limits[j])).min(1.) * signs[j];
            }
        }
        // Limit individual triangle inversions and collapse. Adjacent normals
        // may cross 90 degrees during fairing without either face folding;
        // forbidding that transition pins already creased or smoothed patches.
        self.limit_smoothing_deformation(&before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Alpha, Kind, Vec3};

    fn dab() -> Dab {
        Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.65,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        }
    }

    fn grid(n: usize, noisy: bool) -> Mesh {
        let mut m = Mesh::default();
        for y in 0..=n {
            for x in 0..=n {
                // Unequal spacing in x, to catch tangential vertex drift.
                let u = (x as f32 / n as f32).powf(1.6) * 2. - 1.;
                let v = y as f32 / n as f32 * 2. - 1.;
                let z = if noisy {
                    0.04 * (u * 12.).cos() * (v * 12.).cos()
                } else {
                    0.
                };
                m.vertices.push(Vec3(u, v, 1. + z));
                m.mask.push(0.);
            }
        }
        for y in 0..n {
            for x in 0..n {
                let a = y * (n + 1) + x;
                m.triangles
                    .extend([[a, a + 1, a + n + 2], [a, a + n + 2, a + n + 1]]);
            }
        }
        m
    }

    #[test]
    fn irregular_plane_stays_flat_and_zero_influence_is_exact() {
        let mut m = grid(20, false);
        let before = m.clone();
        for _ in 0..4 {
            m.stamp(&Brush::new(Kind::Smooth), dab());
        }
        assert_eq!(m.triangles, before.triangles);
        for (p, q) in m.vertices.iter().zip(&before.vertices) {
            assert_eq!(p.2, q.2, "smoothing must not emboss an irregular flat mesh");
            assert!(
                (*p - *q).len() < 1e-5,
                "only rounding drift on this Delaunay plane"
            );
        }
        let noisy = grid(20, true);
        for mode in 0..3 {
            let mut m = noisy.clone();
            let mut b = Brush::new(Kind::Smooth);
            let mut d = dab();
            match mode {
                0 => b.strength = 0.,
                1 => d.pressure = 0.,
                _ => {
                    b.alpha = Alpha::Image {
                        width: 1,
                        height: 1,
                        pixels: vec![0.].into(),
                    }
                }
            }
            m.stamp(&b, d);
            assert_eq!(m, noisy);
        }
    }

    #[test]
    fn smoothing_reduces_noise_at_both_densities_and_preserves_protection() {
        let mut reductions = Vec::new();
        for n in [24, 48] {
            let mut m = grid(n, true);
            for (p, mask) in m.vertices.iter().zip(&mut m.mask) {
                if p.0 < -0.35 {
                    *mask = 1.;
                }
            }
            let before = m.clone();
            let rms = |m: &Mesh| {
                let heights: Vec<_> = m
                    .vertices
                    .iter()
                    .filter(|p| p.0.abs() < 0.2 && p.1.abs() < 0.2)
                    .map(|p| (p.2 - 1.).powi(2))
                    .collect();
                (heights.iter().sum::<f32>() / heights.len() as f32).sqrt()
            };
            let initial = rms(&m);
            for _ in 0..10 {
                m.stamp(&Brush::new(Kind::Smooth), dab());
            }
            let ratio = rms(&m) / initial;
            eprintln!("{n}: noise ratio {ratio}");
            assert!(ratio < 0.7);
            reductions.push(ratio);
            assert_eq!(m.triangles, before.triangles);
            assert_eq!(m.mask, before.mask);
            for (i, &mask) in m.mask.iter().enumerate() {
                if mask == 1. {
                    assert_eq!(m.vertices[i], before.vertices[i]);
                }
            }
            m.validate().unwrap();
        }
        assert!(
            (reductions[0] - reductions[1]).abs() < 0.15,
            "smoothing scale must not follow the tessellation"
        );
    }

    #[test]
    fn hidden_faces_creases_and_symmetry_overlap_are_protected() {
        let mut m = grid(24, true);
        let hidden: Vec<_> = m
            .triangles
            .iter()
            .enumerate()
            .filter(|(_, t)| t.iter().any(|&i| m.vertices[i].0 < -0.2))
            .map(|(i, _)| i)
            .collect();
        m.hidden_faces = vec![false; m.triangles.len()];
        for &i in &hidden {
            m.hidden_faces[i] = true;
        }
        m.creases = vec![[312, 313]];
        let before = m.clone();
        let b = Brush::new(Kind::Smooth);
        m.stamp(&b, dab());
        for i in hidden
            .iter()
            .flat_map(|&i| before.triangles[i])
            .chain([312, 313])
        {
            assert_eq!(m.vertices[i], before.vertices[i]);
        }
        assert_eq!(m.hidden_faces, before.hidden_faces);
        assert_eq!(m.creases, before.creases);
        let mut overlap = before;
        overlap.stamp_many(&b, &[dab(), dab()]);
        assert_eq!(m, overlap, "symmetry copies must not multiply smoothing");
    }

    #[test]
    fn nearby_opposing_sheet_is_untouched() {
        let mut m = grid(24, true);
        let count = m.vertices.len();
        let back: Vec<_> = m
            .vertices
            .iter()
            .map(|&p| Vec3(p.0, p.1, p.2 - 0.03))
            .collect();
        let triangles = m.triangles.clone();
        m.vertices.extend(&back);
        m.mask.resize(m.vertices.len(), 0.);
        m.triangles.extend(
            triangles
                .iter()
                .map(|t| [t[0] + count, t[2] + count, t[1] + count]),
        );
        let before = m.vertices[..count].to_vec();
        for _ in 0..8 {
            m.stamp(&Brush::new(Kind::Smooth), dab());
        }
        assert_ne!(m.vertices[..count], before);
        assert_eq!(m.vertices[count..], back);
    }

    #[test]
    fn exported_groove_loses_relief_instead_of_only_improving_triangle_metrics() {
        // A local patch from smoothing-no-work/stages 002 and 003. Keep the complete
        // brush neighborhood plus a fixed halo; no UI or alpha regeneration.
        for (case, fixture) in [
            (
                "stamped",
                include_str!("../../tests/fixtures/smoothing-groove.json"),
            ),
            (
                "already smoothed",
                include_str!("../../tests/fixtures/smoothing-groove-after-stroke.json"),
            ),
        ] {
            let mut m: Mesh = serde_json::from_str(fixture).unwrap();
            let before = m.clone();
            let center = Vec3(-0.5204021, 0.07797206, 0.84631824);
            let selected: Vec<_> = m
                .vertices
                .iter()
                .enumerate()
                .filter(|(_, p)| (**p - center).len() < 0.025)
                .map(|(i, _)| i)
                .collect();
            let relief = |m: &Mesh| {
                let radii: Vec<_> = selected.iter().map(|&i| m.vertices[i].len()).collect();
                let mean = radii.iter().sum::<f32>() / radii.len() as f32;
                (radii.iter().map(|r| (r - mean).powi(2)).sum::<f32>() / radii.len() as f32).sqrt()
            };
            let initial = relief(&m);
            let mut brush = Brush::new(Kind::Smooth);
            brush.strength = 0.5;
            let d = Dab {
                center,
                normal: Vec3(-0.291, 0.0458, 0.9556).norm(),
                right: Vec3(0.9567, 0., 0.2912).norm(),
                up: Vec3(0.01334, 0.99895, -0.04384).norm(),
                radius: 0.065,
                ..dab()
            };
            for _ in 0..32 {
                m.stamp(&brush, d);
            }
            let residual = relief(&m) / initial;
            eprintln!("{case}: exported groove residual {residual} (initial {initial})");
            assert!(
                residual < 0.5,
                "the steep exported relief must visibly flatten"
            );
            assert_eq!(m.triangles, before.triangles);
            for (p, q) in m.vertices.iter().zip(&before.vertices) {
                if (*q - center).len() > d.radius * 1.5 {
                    assert_eq!(p, q);
                }
            }
            m.validate().unwrap();
        }
    }

    #[test]
    fn steep_groove_relaxes_instead_of_pinning_its_sidewalls() {
        let n = 48;
        let mut m = Mesh::default();
        for y in 0..=n {
            for x in 0..=n {
                let u = (x as f32 / n as f32 - 0.5) * 0.5;
                let v = (y as f32 / n as f32 - 0.5) * 0.5;
                let depth = 0.08 * (-(u / 0.008).powi(2)).exp();
                m.vertices.push(Vec3(u, v, 1. - depth));
                m.mask.push(0.);
            }
        }
        for y in 0..n {
            for x in 0..n {
                let a = y * (n + 1) + x;
                m.triangles
                    .extend([[a, a + 1, a + n + 2], [a, a + n + 2, a + n + 1]]);
            }
        }
        let before = m.clone();
        let center = (n / 2) * (n + 1) + n / 2;
        let mut brush = Brush::new(Kind::Smooth);
        brush.strength = 0.5;
        for _ in 0..24 {
            m.stamp(
                &brush,
                Dab {
                    radius: 0.18,
                    ..dab()
                },
            );
        }
        let remaining = 1. - m.vertices[center].2;
        eprintln!("groove depth 0.08 -> {remaining}");
        assert!(
            remaining < 0.04,
            "Smooth must remove at least half the steep relief"
        );
        assert_eq!(m.triangles, before.triangles);
        for (p, q) in m.vertices.iter().zip(&before.vertices) {
            if q.0.abs() > 0.19 || q.1.abs() > 0.19 {
                assert_eq!(p, q);
            }
        }
        m.validate().unwrap();
    }

    #[test]
    fn smooth_can_remove_a_pulled_out_bump_without_changing_topology() {
        let mut m = Mesh::sphere();
        let pull = Brush::new(Kind::Snake);
        let mut d = dab();
        d.radius = 0.4;
        d.delta = Vec3(0., 0., 0.012);
        for _ in 0..12 {
            m.stamp(&pull, d);
            d.center = d.center + pull.snake_motion(d);
        }
        let before = m.clone();
        let peak = |m: &Mesh| {
            m.vertices
                .iter()
                .map(|p| p.2)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        assert!(peak(&before) > 1.1);
        let mut smooth = Brush::new(Kind::Smooth);
        smooth.strength = 0.7;
        let d = Dab {
            radius: 0.65,
            ..dab()
        };
        for _ in 0..32 {
            m.stamp(&smooth, d);
        }
        eprintln!("pulled peak {} -> {}", peak(&before), peak(&m));
        assert!(peak(&m) < 1. + (peak(&before) - 1.) * 0.65);
        assert_eq!(m.triangles, before.triangles);
        m.validate_closed().unwrap();
    }
}
