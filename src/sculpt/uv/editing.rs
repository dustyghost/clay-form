use super::*;
impl UvMap {
    // Place movable islands around fixed masked bounds. Candidate corners come
    // from occupied rectangles; never translate or rescale a protected island.
    pub(super) fn pack_around_masks(
        &mut self,
        padding: f32,
        islands: &[Vec<usize>],
        rects: &[(usize, Point, Point)],
        fixed: &BTreeSet<usize>,
    ) -> Result<(), String> {
        let obstacles: Vec<_> = rects
            .iter()
            .filter(|r| fixed.contains(&r.0))
            .map(|&(_, lo, size)| (lo, [lo[0] + size[0], lo[1] + size[1]]))
            .collect();
        if obstacles
            .iter()
            .any(|(lo, hi)| (0..2).any(|k| lo[k] < 0. || hi[k] > 1.))
        {
            return Err("Masked islands must be inside 0–1 before packing".into());
        }
        let layout = |scale: f32| -> Option<Vec<Point>> {
            let mut occupied = obstacles.clone();
            let mut work = 0usize;
            let mut origins = vec![[0.; 2]; islands.len()];
            for &(id, _, size) in rects {
                if fixed.contains(&id) {
                    continue;
                }
                let size = size.map(|v| v * scale);
                let mut xs = vec![padding];
                let mut ys = vec![padding];
                for (_, hi) in &occupied {
                    xs.push(hi[0] + 2. * padding);
                    ys.push(hi[1] + 2. * padding);
                }
                xs.sort_by(f32::total_cmp);
                xs.dedup();
                ys.sort_by(f32::total_cmp);
                ys.dedup();
                let mut found = None;
                'search: for &y in &ys {
                    for &x in &xs {
                        let hi = [x + size[0], y + size[1]];
                        if hi.iter().any(|v| *v + padding > 1.) {
                            continue;
                        }
                        work += occupied.len();
                        if work > 250_000 {
                            return None;
                        }
                        if occupied.iter().any(|(a, b)| {
                            x < b[0] + 2. * padding
                                && hi[0] + 2. * padding > a[0]
                                && y < b[1] + 2. * padding
                                && hi[1] + 2. * padding > a[1]
                        }) {
                            continue;
                        }
                        found = Some(([x, y], hi));
                        break 'search;
                    }
                }
                let (lo, hi) = found?;
                origins[id] = lo;
                occupied.push((lo, hi));
            }
            Some(origins)
        };
        if fixed.len() == islands.len() {
            return Ok(());
        }
        let mut low = 0.;
        let mut high = 1.
            / rects
                .iter()
                .filter(|r| !fixed.contains(&r.0))
                .map(|r| r.2[0].max(r.2[1]))
                .fold(1e-8, f32::max);
        for _ in 0..40 {
            let mid = (low + high) * 0.5;
            if layout(mid).is_some() {
                low = mid;
            } else {
                high = mid;
            }
        }
        if low < 1e-8 {
            return Err("No room around masked islands; reduce padding or clear some masks".into());
        }
        let origins = layout(low).ok_or("Unable to pack around UV masks")?;
        for &(id, lo, _) in rects {
            if fixed.contains(&id) {
                continue;
            }
            for &f in &islands[id] {
                for q in &mut self.corners[f] {
                    for k in 0..2 {
                        q[k] = (q[k] - lo[k]) * low + origins[id][k];
                    }
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
