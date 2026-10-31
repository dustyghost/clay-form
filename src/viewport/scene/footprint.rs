//! Short-lived candidate lists for the fixed brush-footprint sample grid.
use super::picking::{face_bounds, may_cross_near};
use super::{Pos2, Scene, Vec3, egui, pick_faces};

pub(super) const SIDE: usize = 17;
const SAMPLES: usize = SIDE * SIDE;
// At most 512 KiB of live face IDs on 64-bit targets, plus Vec spare capacity.
const MAX_REFERENCES: usize = 65_536;
type Bins = [Vec<usize>; SAMPLES];

enum Candidates {
    Bins(Box<Bins>),
    Scan(Vec<usize>),
}

pub(super) struct Footprint {
    pub points: [Pos2; SAMPLES],
    candidates: Candidates,
}

impl Footprint {
    pub fn new(scene: &Scene, pos: Pos2, radius: f32) -> Self {
        Self::with_budget(scene, pos, radius, MAX_REFERENCES)
    }

    fn with_budget(scene: &Scene, pos: Pos2, radius: f32, budget: usize) -> Self {
        // Keep the original sample arithmetic, including edge/centre rounding.
        let points = std::array::from_fn(|i| {
            let u = (i % SIDE) as f32 / (SIDE - 1) as f32 * 2. - 1.;
            let v = 1. - (i / SIDE) as f32 / (SIDE - 1) as f32 * 2.;
            pos + egui::vec2(u, -v) * radius
        });
        if radius < 0. || !radius.is_finite() || !points.iter().all(|p| p.is_finite()) {
            return Self {
                points,
                candidates: Candidates::Scan(scene.all_faces.iter().map(|f| f.0).collect()),
            };
        }
        let bounds = egui::Rect::from_min_max(points[0], points[SAMPLES - 1]);
        let check_near = may_cross_near(scene);
        let candidates = Self::bins(scene, &points, bounds, check_near, budget)
            .map(Candidates::Bins)
            .unwrap_or_else(|| {
                // A wide triangle or many overlapping sheets can populate most
                // bins. Bound the extra storage and retain the previous coarse
                // candidate scan in that case, with conservative clip bounds.
                Candidates::Scan(
                    scene
                        .face_candidates(bounds)
                        .filter_map(|(face, _)| {
                            bounds
                                .intersects(face_bounds(scene, face, check_near))
                                .then_some(face)
                        })
                        .collect(),
                )
            });
        Self { points, candidates }
    }

    fn bins(
        scene: &Scene,
        points: &[Pos2; SAMPLES],
        bounds: egui::Rect,
        check_near: bool,
        budget: usize,
    ) -> Option<Box<Bins>> {
        let xs: [_; SIDE] = std::array::from_fn(|i| points[i].x);
        let ys: [_; SIDE] = std::array::from_fn(|i| points[i * SIDE].y);
        let mut bins: Box<Bins> = Box::new(std::array::from_fn(|_| Vec::new()));
        let mut references = 0;
        for (face, _) in scene.face_candidates(bounds) {
            let b = face_bounds(scene, face, check_near);
            if !bounds.intersects(b) {
                continue;
            }
            // Compare the exact sample coordinates rather than dividing by a
            // cell width. This also handles repeated coordinates at tiny radii.
            let x0 = xs.partition_point(|&x| x < b.min.x);
            let x1 = xs.partition_point(|&x| x <= b.max.x);
            let y0 = ys.partition_point(|&y| y < b.min.y);
            let y1 = ys.partition_point(|&y| y <= b.max.y);
            references += (x1 - x0) * (y1 - y0);
            if references > budget {
                return None;
            }
            // Preserve source traversal order, including equal-depth ties.
            for y in y0..y1 {
                for x in x0..x1 {
                    bins[y * SIDE + x].push(face);
                }
            }
        }
        Some(bins)
    }

    pub fn pick(&self, scene: &Scene, index: usize) -> Option<(Vec3, Vec3, usize)> {
        let faces = match &self.candidates {
            Candidates::Bins(bins) => &bins[index],
            Candidates::Scan(faces) => faces,
        };
        pick_faces(scene, faces.iter().copied(), self.points[index])
    }

    #[cfg(test)]
    pub fn reference(scene: &Scene, pos: Pos2, radius: f32) -> Self {
        let mut result = Self::with_budget(scene, pos, radius, 0);
        // Independent broad phase: test every front-facing face at every point.
        result.candidates = Candidates::Scan(scene.all_faces.iter().map(|f| f.0).collect());
        result
    }
}

#[cfg(test)]
mod tests;
