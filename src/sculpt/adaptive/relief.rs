//! Estimate displacement interpolation error on the undeformed stamp footprint.
use super::{Brush, Dab, HashMap, Mesh, Projection, Region, Vec3, edge};

fn height(brush: &Brush, dab: Dab, stencil: Option<&Projection>, p: Vec3, mask: f32) -> f32 {
    let offset = p - dab.center;
    let pressure = if brush.pressure {
        dab.pressure.powf(brush.pressure_curve)
    } else {
        1.
    };
    brush.weight(
        offset.dot(dab.right) / dab.radius,
        offset.dot(dab.up) / dab.radius,
    ) * stencil.map_or(1., |s| s.weight(p))
        * (1. - mask)
        * dab.radius
        * 0.6
        * brush.strength
        * pressure
}

pub(super) struct Sampling<'a> {
    brush: &'a Brush,
    region: Region<'a>,
    heights: Vec<HashMap<usize, f32>>,
    edges: HashMap<(usize, usize), f32>,
    tolerance: f32,
    minimum_edge: f32,
}
impl<'a> Sampling<'a> {
    pub fn new(brush: &'a Brush, region: Region<'a>, target: f32) -> Self {
        Self {
            brush,
            region,
            heights: vec![HashMap::new(); region.dabs.len()],
            edges: HashMap::new(),
            // Error-driven sizing follows the principle of Dunyach et al.,
            // "Adaptive Remeshing for Real-Time Mesh Deformation" (2013).
            // Here the alpha gives a direct sampled error estimate, rather
            // than estimating curvature from a mesh that lacks the new detail.
            // At the default 8 px detail these are 0.25 px error and 1 px edges.
            // This is a sampled estimate, not a strict bound for arbitrary images.
            tolerance: target / 32.,
            minimum_edge: target / 8.,
        }
    }
    fn vertex(&mut self, mesh: &Mesh, index: usize, vertex: usize) -> f32 {
        *self.heights[index].entry(vertex).or_insert_with(|| {
            height(
                self.brush,
                self.region.dabs[index],
                self.region.stencils.get(index),
                mesh.vertices[vertex],
                mesh.mask[vertex],
            )
        })
    }
    fn edge_error(&mut self, mesh: &Mesh, a: usize, b: usize) -> f32 {
        let key = edge(a, b);
        if let Some(&error) = self.edges.get(&key) {
            return error;
        }
        let mut error = 0_f32;
        if (mesh.vertices[a] - mesh.vertices[b]).len() > self.minimum_edge {
            for index in 0..self.region.dabs.len() {
                let left = self.vertex(mesh, index, a);
                let right = self.vertex(mesh, index, b);
                // Quarter samples catch an S-shaped profile that agrees at
                // its endpoints and midpoint. Cache shared edges once per pass.
                for t in [0.25, 0.5, 0.75] {
                    let p = mesh.vertices[a] * (1. - t) + mesh.vertices[b] * t;
                    let mask = mesh.mask[a] * (1. - t) + mesh.mask[b] * t;
                    let sampled = height(
                        self.brush,
                        self.region.dabs[index],
                        self.region.stencils.get(index),
                        p,
                        mask,
                    );
                    error = error.max((sampled - (left * (1. - t) + right * t)).abs());
                }
            }
        }
        self.edges.insert(key, error);
        error
    }
    pub fn candidate(&mut self, mesh: &Mesh, t: [usize; 3]) -> Option<((usize, usize), f32)> {
        let [a, b, c] = t;
        let edges = [(a, b), (b, c), (c, a)];
        let longest = edges
            .into_iter()
            .max_by(|&(a, b), &(x, y)| {
                (mesh.vertices[a] - mesh.vertices[b])
                    .len()
                    .total_cmp(&(mesh.vertices[x] - mesh.vertices[y]).len())
            })
            .unwrap();
        if (mesh.vertices[longest.0] - mesh.vertices[longest.1]).len() <= self.minimum_edge {
            return None;
        }
        let mut error = edges
            .into_iter()
            .map(|(a, b)| self.edge_error(mesh, a, b))
            .fold(0_f32, f32::max);
        let center = (mesh.vertices[a] + mesh.vertices[b] + mesh.vertices[c]) * (1. / 3.);
        let mask = (mesh.mask[a] + mesh.mask[b] + mesh.mask[c]) / 3.;
        for index in 0..self.region.dabs.len() {
            let linear = t
                .map(|v| self.vertex(mesh, index, v))
                .into_iter()
                .sum::<f32>()
                / 3.;
            let sampled = height(
                self.brush,
                self.region.dabs[index],
                self.region.stencils.get(index),
                center,
                mask,
            );
            error = error.max((sampled - linear).abs());
        }
        // Split the longest edge of the affected face, not whichever short
        // edge happens to cross a ridge. Repeated short-edge splits create
        // slivers that the deformation guard must reject.
        (error > self.tolerance).then_some((edge(longest.0, longest.1), error / self.tolerance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Alpha, Kind};

    #[test]
    fn sampling_finds_between_vertex_features_and_respects_opacity() {
        let mut brush = Brush::new(Kind::Standard);
        brush.inner_radius = 0.95;
        brush.alpha = Alpha::Image {
            width: 17,
            height: 1,
            pixels: (0..17)
                .map(|i| {
                    let x = i as f32 / 8. - 1.;
                    0.5 + 0.5 * (x * std::f32::consts::TAU).sin()
                })
                .collect::<Vec<_>>()
                .into(),
        };
        let mut mesh = Mesh {
            vertices: vec![Vec3(-0.5, 0., 0.), Vec3(0.5, 0., 0.), Vec3(0., 0.01, 0.)],
            triangles: vec![[0, 1, 2]],
            mask: vec![0.; 3],
            ..Default::default()
        };
        let mut dab = Dab {
            center: Vec3::default(),
            radius: 1.,
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        let candidate = |mesh: &Mesh, brush: &Brush, dab: Dab| {
            Sampling::new(
                brush,
                Region {
                    dabs: &[dab],
                    stencils: &[],
                },
                1.1,
            )
            .candidate(mesh, [0, 1, 2])
        };
        // Endpoints, midpoint and centroid all agree. The quarter-edge peaks
        // still need refinement, and it must choose the longest edge.
        assert_eq!(candidate(&mesh, &brush, dab).unwrap().0, (0, 1));
        mesh.mask.fill(1.);
        assert!(candidate(&mesh, &brush, dab).is_none());
        mesh.mask.fill(0.9);
        assert!(candidate(&mesh, &brush, dab).is_none());
        mesh.mask.fill(0.);
        dab.pressure = 0.;
        assert!(candidate(&mesh, &brush, dab).is_none());
        brush.pressure = false;
        assert!(candidate(&mesh, &brush, dab).is_some());
    }
}
