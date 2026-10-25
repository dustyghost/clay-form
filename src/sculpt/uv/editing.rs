use super::*;
#[derive(Clone, Copy)]
struct Bounds {
    lo: Point,
    hi: Point,
}
impl Bounds {
    fn points(points: impl Iterator<Item = Point>) -> Self {
        let mut bounds = Self {
            lo: [f32::INFINITY; 2],
            hi: [f32::NEG_INFINITY; 2],
        };
        for p in points {
            for (k, v) in p.into_iter().enumerate() {
                bounds.lo[k] = bounds.lo[k].min(v);
                bounds.hi[k] = bounds.hi[k].max(v);
            }
        }
        bounds
    }
    fn size(self) -> Point {
        [self.hi[0] - self.lo[0], self.hi[1] - self.lo[1]]
    }
    fn contains(self, other: Self) -> bool {
        (0..2).all(|k| self.lo[k] <= other.lo[k] && self.hi[k] >= other.hi[k])
    }
}
fn rotate(p: Point, angle: f32) -> Point {
    let (s, c) = angle.sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1]]
}
fn hull(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let mut result = Vec::new();
    for _ in 0..2 {
        let start = result.len();
        for &point in &points {
            while result.len() >= start + 2
                && area([result[result.len() - 2], result[result.len() - 1], point]) <= 0.
            {
                result.pop();
            }
            result.push(point);
        }
        result.pop();
        points.reverse();
    }
    result
}
// Cut occupied bounds out of maximal free rectangles, keeping disjointness from
// placed islands. The free rectangles may overlap each other until the next cut.
fn subtract(free: &mut Vec<Bounds>, used: Bounds, work: &mut usize) -> Option<()> {
    let mut next = Vec::new();
    for &r in free.iter() {
        if (0..2).any(|k| used.lo[k] >= r.hi[k] || used.hi[k] <= r.lo[k]) {
            next.push(r);
            continue;
        }
        for k in 0..2 {
            if used.lo[k] > r.lo[k] {
                let mut part = r;
                part.hi[k] = used.lo[k];
                next.push(part);
            }
            if used.hi[k] < r.hi[k] {
                let mut part = r;
                part.lo[k] = used.hi[k];
                next.push(part);
            }
        }
    }
    *work += next.len().saturating_mul(next.len());
    if *work > 8_000_000 {
        return None;
    }
    *free = next
        .iter()
        .enumerate()
        .filter(|(i, r)| {
            !next
                .iter()
                .enumerate()
                .any(|(j, s)| *i != j && s.contains(**r) && (!r.contains(*s) || j < *i))
        })
        .map(|(_, r)| *r)
        .collect();
    Some(())
}
#[derive(Clone, Copy, Default)]
struct Placement {
    origin: Point,
    turned: bool,
}
fn layout(
    rects: &[Bounds],
    fixed: &BTreeSet<usize>,
    order: &[usize],
    scale: f32,
    padding: f32,
) -> Option<Vec<Placement>> {
    let mut free = vec![Bounds {
        lo: [0.; 2],
        hi: [1.; 2],
    }];
    let mut work = 0;
    let mut placements = vec![Placement::default(); rects.len()];
    for &id in fixed {
        let r = rects[id];
        subtract(
            &mut free,
            Bounds {
                lo: r.lo.map(|v| (v - padding).max(0.)),
                hi: r.hi.map(|v| (v + padding).min(1.)),
            },
            &mut work,
        )?;
    }
    for &id in order {
        let size = rects[id].size().map(|v| v.max(1e-8) * scale + 2. * padding);
        let mut best: Option<(f32, f32, Bounds, bool)> = None;
        for &r in &free {
            for turned in [false, true] {
                let [w, h] = if turned { [size[1], size[0]] } else { size };
                let remaining = [r.size()[0] - w, r.size()[1] - h];
                if remaining.iter().any(|&v| v < 0.) {
                    continue;
                }
                let score = (
                    remaining[0].min(remaining[1]),
                    remaining[0].max(remaining[1]),
                );
                if best.as_ref().is_none_or(|b| {
                    score
                        .0
                        .total_cmp(&b.0)
                        .then(score.1.total_cmp(&b.1))
                        .is_lt()
                }) {
                    best = Some((
                        score.0,
                        score.1,
                        Bounds {
                            lo: r.lo,
                            hi: [r.lo[0] + w, r.lo[1] + h],
                        },
                        turned,
                    ));
                }
            }
        }
        let (_, _, used, turned) = best?;
        placements[id] = Placement {
            origin: used.lo.map(|v| v + padding),
            turned,
        };
        subtract(&mut free, used, &mut work)?;
    }
    Some(placements)
}
impl UvMap {
    /// Straighten islands using their convex hull; dense boundaries sample up to
    /// 128 edge directions to bound work. This rotates only, with no distortion.
    fn straighten(&mut self, faces: &[usize]) {
        let bounds = Bounds::points(faces.iter().flat_map(|&f| self.corners[f]));
        let center = [
            (bounds.lo[0] + bounds.hi[0]) * 0.5,
            (bounds.lo[1] + bounds.hi[1]) * 0.5,
        ];
        let points = hull(
            faces
                .iter()
                .flat_map(|&f| self.corners[f])
                .map(|p| [p[0] - center[0], p[1] - center[1]])
                .collect(),
        );
        if points.len() < 2 {
            return;
        }
        let size = bounds.size();
        let mut best = (size[0] * size[1], 0.);
        let step = points.len().div_ceil(128);
        for i in (0..points.len()).step_by(step) {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            let angle = -(b[1] - a[1]).atan2(b[0] - a[0]);
            // Equivalent quarter turns are tried by the packer; prefer small rotations here.
            let angle = (angle + std::f32::consts::FRAC_PI_4)
                .rem_euclid(std::f32::consts::FRAC_PI_2)
                - std::f32::consts::FRAC_PI_4;
            let size = Bounds::points(points.iter().map(|&p| rotate(p, angle))).size();
            let area = size[0] * size[1];
            if area < best.0 * (1. - 1e-6) {
                best = (area, angle);
            }
        }
        if best.1 != 0. {
            for &f in faces {
                for p in &mut self.corners[f] {
                    let q = rotate([p[0] - center[0], p[1] - center[1]], best.1);
                    *p = [q[0] + center[0], q[1] + center[1]];
                }
            }
        }
    }
    pub(super) fn arrange(
        &mut self,
        padding: f32,
        selection: Option<&[usize]>,
    ) -> Result<(), String> {
        let islands = self.islands();
        if islands.is_empty() {
            return Err("Generate UVs before arranging".into());
        }
        let selected = selection.map(|fs| fs.iter().copied().collect::<BTreeSet<_>>());
        let fixed: BTreeSet<_> = islands
            .iter()
            .enumerate()
            .filter(|(_, fs)| {
                selected
                    .as_ref()
                    .is_some_and(|selected| !fs.iter().any(|f| selected.contains(f)))
                    || fs.iter().any(|&f| (0..3).any(|c| self.mask(f, c) > 0.))
            })
            .map(|(id, _)| id)
            .collect();
        for (id, faces) in islands.iter().enumerate() {
            if !fixed.contains(&id) {
                self.straighten(faces);
            }
        }
        let rects: Vec<_> = islands
            .iter()
            .map(|faces| Bounds::points(faces.iter().flat_map(|&f| self.corners[f])))
            .collect();
        if fixed.len() == islands.len() {
            return Ok(());
        }
        let mut best_scale = 0.;
        let mut best = None;
        // Area order and longest-side order handle different mixes of wide/short islands.
        for by_area in [true, false] {
            let mut order: Vec<_> = (0..islands.len())
                .filter(|id| !fixed.contains(id))
                .collect();
            let score = |id: usize| {
                let s = rects[id].size();
                if by_area { s[0] * s[1] } else { s[0].max(s[1]) }
            };
            order.sort_by(|&a, &b| score(b).total_cmp(&score(a)).then(a.cmp(&b)));
            let mut low = 0.;
            let mut high = 1.
                / order
                    .iter()
                    .map(|&id| {
                        let s = rects[id].size();
                        s[0].max(s[1])
                    })
                    .fold(1e-8, f32::max);
            for _ in 0..28 {
                let mid = (low + high) * 0.5;
                if layout(&rects, &fixed, &order, mid, padding).is_some() {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            // Leave a tiny numerical margin so float rounding cannot cross the atlas edge.
            low *= 1. - 1e-6;
            if low > best_scale
                && let Some(placements) = layout(&rects, &fixed, &order, low, padding)
            {
                best_scale = low;
                best = Some(placements);
            }
        }
        let placements = best.filter(|_| best_scale > 1e-12).ok_or(
            "Unable to arrange islands at this padding; reduce padding or clear some UV masks",
        )?;
        for (id, faces) in islands.iter().enumerate() {
            if fixed.contains(&id) {
                continue;
            }
            let placement = placements[id];
            let bounds = rects[id];
            for &f in faces {
                for p in &mut self.corners[f] {
                    let q = [p[0] - bounds.lo[0], p[1] - bounds.lo[1]];
                    let q = if placement.turned {
                        [bounds.size()[1] - q[1], q[0]]
                    } else {
                        q
                    };
                    *p = [
                        placement.origin[0] + q[0] * best_scale,
                        placement.origin[1] + q[1] * best_scale,
                    ];
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mesh() -> Mesh {
        let mut m = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube)
            .mesh()
            .unwrap();
        m.uv = m.unwrap_uv(true, 0.01, &|| false).unwrap();
        m
    }
    fn rectangles(specs: &[(Point, Point, f32)]) -> Mesh {
        let mut m = Mesh::default();
        for &(size, origin, angle) in specs {
            let start = m.vertices.len();
            let q = [[0., 0.], [size[0], 0.], size, [0., size[1]]].map(|p| {
                let p = rotate(p, angle);
                [p[0] + origin[0], p[1] + origin[1]]
            });
            m.vertices.extend(q.map(|p| Vec3(p[0], p[1], 0.)));
            m.triangles
                .extend([[start, start + 1, start + 2], [start, start + 2, start + 3]]);
            m.uv.corners
                .extend([[q[0], q[1], q[2]], [q[0], q[2], q[3]]]);
        }
        m.mask = vec![0.; m.vertices.len()];
        m.uv.topology = m.triangles.clone();
        m
    }
    fn assert_spacing(uv: &UvMap, padding: f32) {
        let bounds: Vec<_> = uv
            .islands()
            .iter()
            .map(|fs| Bounds::points(fs.iter().flat_map(|&f| uv.corners[f])))
            .collect();
        for (i, r) in bounds.iter().enumerate() {
            assert!(
                r.lo.iter().all(|&v| v >= padding - 1e-6)
                    && r.hi.iter().all(|&v| v <= 1. - padding + 1e-6)
            );
            for s in &bounds[..i] {
                assert!(
                    (0..2).any(|k| r.lo[k] - s.hi[k] >= 2. * padding - 1e-6
                        || s.lo[k] - r.hi[k] >= 2. * padding - 1e-6),
                    "island bounds overlap or lose padding"
                );
            }
        }
    }
    #[test]
    fn auto_arrange_straightens_diagonal_islands_and_preserves_density() {
        let mut square = rectangles(&[([1., 1.], [0., 0.], std::f32::consts::FRAC_PI_4)]);
        square.uv.pack(0.01).unwrap();
        let coverage = square
            .uv
            .corners
            .iter()
            .map(|t| area(*t) * 0.5)
            .sum::<f32>();
        assert!(
            coverage > 0.95,
            "straightened square should cover 96% of the image: {coverage}"
        );
        assert_spacing(&square.uv, 0.01);
        assert_eq!(square.uv.inspect(&square).overlaps, 0);
        let mut mixed = rectangles(&[
            ([0.8, 0.3], [0., 0.], 0.5),
            ([0.8, 0.3], [0., 0.], -0.2),
            ([0.4, 0.5], [0., 0.], 0.7),
            ([0.4, 0.5], [0., 0.], -0.6),
            ([0.2, 0.9], [0., 0.], 0.4),
            ([0.3, 0.6], [0., 0.], 0.),
        ]);
        let before = mixed.uv.clone();
        mixed.uv.pack(0.01).unwrap();
        let scale = area(mixed.uv.corners[0]) / area(before.corners[0]);
        for (a, b) in before.corners.iter().zip(&mixed.uv.corners) {
            assert!(
                (area(*b) / area(*a) - scale).abs() < 1e-5,
                "all islands use the same scale without flipping"
            );
        }
        assert!(scale > 0.);
        assert_spacing(&mixed.uv, 0.01);
        assert_eq!(mixed.uv.inspect(&mixed).overlaps, 0);
    }
    #[test]
    fn arranging_selected_islands_keeps_others_and_masks_fixed_and_is_atomic() {
        let mut m = rectangles(&[
            ([0.3, 0.6], [0.01, 0.01], 0.),
            ([0.2, 0.2], [0.6, 0.6], 0.),
            ([0.15, 0.1], [0.4, 0.3], 0.6),
        ]);
        m.uv.paint_mask([0.6, 0.6], 0.01, false);
        let before = m.uv.clone();
        m.uv.pack_selected(&[2, 3, 4, 5], 0.005).unwrap();
        assert_eq!(
            &m.uv.corners[..4],
            &before.corners[..4],
            "unselected and masked islands stay bit-for-bit fixed"
        );
        assert_ne!(&m.uv.corners[4..], &before.corners[4..]);
        assert_spacing(&m.uv, 0.005);
        assert_eq!(m.uv.inspect(&m).overlaps, 0);
        let mut blocked = rectangles(&[([1., 1.], [0., 0.], 0.), ([0.1, 0.2], [0., 0.], 0.7)]);
        let before = blocked.uv.clone();
        assert!(blocked.uv.pack_selected(&[2, 3], 0.01).is_err());
        assert_eq!(
            blocked.uv, before,
            "failed packing must not leave islands rotated"
        );
        assert!(blocked.uv.pack_selected(&[], 0.01).is_err());
    }
    #[test]
    fn painted_uv_masks_pin_points_and_pack_keeps_protected_islands() {
        let mut mesh = mesh();
        let island = mesh.uv.islands()[0].clone();
        let blank = mesh.uv.clone();
        mesh.uv.paint_mask([10., 10.], 0.01, false);
        assert_eq!(mesh.uv, blank, "painting empty space is a no-op");
        let point = mesh.uv.corners[island[0]][0];
        mesh.uv.paint_mask(point, 0.05, false);
        assert_eq!(mesh.uv.mask(island[0], 0), 1.);
        let before = mesh.uv.clone();
        mesh.uv.transform(&island, [0.1, 0.02], 0.8, 0.1).unwrap();
        assert_eq!(mesh.uv.corners[island[0]][0], point);
        assert_ne!(mesh.uv.corners, before.corners);
        assert!(mesh.uv.remove_islands(&island).is_err());
        assert!(mesh.unwrap_uv(true, 0.01, &|| false).is_err());
        mesh.uv = before.clone();
        mesh.uv.pack(0.005).unwrap();
        for &f in &island {
            assert_eq!(mesh.uv.corners[f], before.corners[f]);
        }
        assert_eq!(mesh.uv.inspect(&mesh).overlaps, 0);
        assert_eq!(mesh.uv.inspect(&mesh).outside, 0);
        mesh.uv.paint_mask(point, 0.05, true);
        assert!(!mesh.uv.has_mask());
    }
    #[test]
    fn uv_removal_and_masks_roundtrip_and_validate() {
        let mut mesh = mesh();
        let geometry = mesh.triangles.clone();
        let island = mesh.uv.islands()[0].clone();
        mesh.uv.remove_islands(&island).unwrap();
        assert_eq!(mesh.uv.islands().len(), 5);
        assert_eq!(mesh.triangles, geometry);
        assert!(island.iter().all(|&f| !mesh.uv.mapped(f)));
        mesh.uv.pack(0.01).unwrap();
        mesh.uv.validate().unwrap();
        let json = serde_json::to_string(&mesh.uv).unwrap();
        assert_eq!(serde_json::from_str::<UvMap>(&json).unwrap(), mesh.uv);
        let path = std::env::temp_dir().join(format!("clay-uv-removal-{}.obj", std::process::id()));
        crate::persistence::export_obj(path.to_str().unwrap(), &mesh).unwrap();
        let imported = crate::persistence::load_obj(path.to_str().unwrap()).unwrap();
        assert_eq!(imported.triangles.len(), mesh.triangles.len());
        assert_eq!(imported.quads.len(), mesh.quads.len());
        assert_eq!(
            imported.uv.removed.iter().filter(|&&v| v).count(),
            island.len()
        );
        std::fs::remove_file(path).unwrap();
        mesh.uv.masks = vec![[f32::NAN; 3]; mesh.triangles.len()];
        assert!(mesh.uv.validate().is_err());
    }
}
