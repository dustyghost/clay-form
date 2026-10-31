use super::*;
const SIDE: usize = 64;

#[derive(Default, Debug, PartialEq)]
struct Node {
    center: Vec3,
    extent: Vec3,
    range: std::ops::Range<usize>,
    children: Option<(usize, usize)>,
}
struct FaceBounds {
    low: [f32; 3],
    high: [f32; 3],
    sum: [f32; 3],
}
impl FaceBounds {
    fn new(points: &[Vec3; 3]) -> Self {
        let points = points.map(|p| [p.0, p.1, p.2]);
        Self {
            low: std::array::from_fn(|axis| {
                points.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min)
            }),
            high: std::array::from_fn(|axis| {
                points
                    .iter()
                    .map(|p| p[axis])
                    .fold(f32::NEG_INFINITY, f32::max)
            }),
            sum: std::array::from_fn(|axis| points.iter().map(|p| p[axis]).sum()),
        }
    }
}
/// A validated world-space hierarchy and reusable projected front-surface bins.
/// Query bounds are unbounded along the view direction, retaining distant occluders.
#[derive(Default)]
pub(crate) struct DepthField {
    world_faces: Vec<[Vec3; 3]>,
    visibility: Vec<bool>,
    needs_validation: bool,
    #[cfg(test)]
    builds: usize,
    faces: Vec<[[f32; 3]; 3]>,
    order: Vec<usize>,
    nodes: Vec<Node>,
    bins: Vec<Vec<usize>>,
    pub active_faces: Vec<usize>,
    pub active_vertices: Vec<usize>,
    vertex_marks: Vec<u32>,
    stamp: u32,
    pending: Vec<usize>,
}
impl DepthField {
    fn reset(&mut self) {
        // Material/image paint does not move geometry. Retain the hierarchy,
        // but validate exact input values before the next stroke can use it.
        self.needs_validation = true;
    }
    #[cfg(test)]
    fn build_node_reference(&mut self, start: usize, end: usize) -> usize {
        let mut low = [f32::INFINITY; 3];
        let mut high = [f32::NEG_INFINITY; 3];
        for &f in &self.order[start..end] {
            for p in self.world_faces[f] {
                for (axis, v) in [p.0, p.1, p.2].into_iter().enumerate() {
                    low[axis] = low[axis].min(v);
                    high[axis] = high[axis].max(v);
                }
            }
        }
        let center = std::array::from_fn::<_, 3, _>(|i| (low[i] + high[i]) * 0.5);
        let extent = std::array::from_fn::<_, 3, _>(|i| (high[i] - low[i]) * 0.5);
        let index = self.nodes.len();
        self.nodes.push(Node {
            center: Vec3(center[0], center[1], center[2]),
            extent: Vec3(extent[0], extent[1], extent[2]),
            range: start..end,
            children: None,
        });
        if end - start > 16 {
            let axis = (0..3)
                .max_by(|&a, &b| extent[a].total_cmp(&extent[b]))
                .unwrap();
            let mid = (start + end) / 2;
            let faces = &self.world_faces;
            let coordinate = |f: usize| faces[f].iter().map(|p| [p.0, p.1, p.2][axis]).sum::<f32>();
            self.order[start..end].select_nth_unstable_by(mid - start, |&a, &b| {
                coordinate(a).total_cmp(&coordinate(b))
            });
            let left = self.build_node_reference(start, mid);
            let right = self.build_node_reference(mid, end);
            self.nodes[index].children = Some((left, right));
        }
        index
    }
    fn matches(&self, mesh: &Mesh, occluders: &[&Mesh]) -> bool {
        let count: usize = std::iter::once(mesh)
            .chain(occluders.iter().copied())
            .map(|m| m.triangles.len())
            .sum();
        if count != self.world_faces.len() {
            return false;
        }
        let mut index = 0;
        for surface in std::iter::once(mesh).chain(occluders.iter().copied()) {
            for (face, triangle) in surface.triangles.iter().enumerate() {
                if surface.face_visible(face) != self.visibility[index]
                    || triangle
                        .iter()
                        .zip(self.world_faces[index])
                        .any(|(&i, old)| {
                            let p = surface.vertices[i];
                            [p.0.to_bits(), p.1.to_bits(), p.2.to_bits()]
                                != [old.0.to_bits(), old.1.to_bits(), old.2.to_bits()]
                        })
                {
                    return false;
                }
                index += 1;
            }
        }
        true
    }
    fn build_node(&mut self, bounds: &[FaceBounds], start: usize, end: usize) -> usize {
        let mut low = [f32::INFINITY; 3];
        let mut high = [f32::NEG_INFINITY; 3];
        for &f in &self.order[start..end] {
            for axis in 0..3 {
                low[axis] = low[axis].min(bounds[f].low[axis]);
                high[axis] = high[axis].max(bounds[f].high[axis]);
            }
        }
        let center = std::array::from_fn::<_, 3, _>(|i| (low[i] + high[i]) * 0.5);
        let extent = std::array::from_fn::<_, 3, _>(|i| (high[i] - low[i]) * 0.5);
        let index = self.nodes.len();
        self.nodes.push(Node {
            center: Vec3(center[0], center[1], center[2]),
            extent: Vec3(extent[0], extent[1], extent[2]),
            range: start..end,
            children: None,
        });
        if end - start > 16 {
            let axis = (0..3)
                .max_by(|&a, &b| extent[a].total_cmp(&extent[b]))
                .unwrap();
            let mid = (start + end) / 2;
            self.order[start..end].select_nth_unstable_by(mid - start, |&a, &b| {
                bounds[a].sum[axis].total_cmp(&bounds[b].sum[axis])
            });
            let left = self.build_node(bounds, start, mid);
            let right = self.build_node(bounds, mid, end);
            self.nodes[index].children = Some((left, right));
        }
        index
    }
    pub fn prepare(&mut self, mesh: &Mesh, dab: &Dab, occluders: &[&Mesh]) {
        if self.needs_validation {
            if !self.matches(mesh, occluders) {
                self.world_faces.clear();
                self.visibility.clear();
                self.order.clear();
                self.nodes.clear();
            }
            self.needs_validation = false;
        }
        #[cfg(test)]
        super::profile::mark("depth_validate");
        if self.world_faces.is_empty() {
            for surface in std::iter::once(mesh).chain(occluders.iter().copied()) {
                for (f, triangle) in surface.triangles.iter().enumerate() {
                    let index = self.world_faces.len();
                    self.world_faces.push(triangle.map(|i| surface.vertices[i]));
                    let visible = surface.face_visible(f);
                    self.visibility.push(visible);
                    if visible {
                        self.order.push(index);
                    }
                }
            }
            self.faces.resize(self.world_faces.len(), [[0.; 3]; 3]);
            #[cfg(test)]
            super::profile::mark("depth_copy");
            if !self.order.is_empty() {
                // Bounds and centroid comparisons otherwise repeat triangle
                // loads/arithmetic at every level. This temporary array is
                // released after construction; traversal storage is unchanged.
                let bounds: Vec<_> = self.world_faces.iter().map(FaceBounds::new).collect();
                self.build_node(&bounds, 0, self.order.len());
                #[cfg(test)]
                {
                    self.builds += 1;
                }
            }
            #[cfg(test)]
            super::profile::mark("depth_build");
        }
        self.bins.resize_with(SIDE * SIDE, Vec::new);
        for bin in &mut self.bins {
            bin.clear();
        }
        self.active_faces.clear();
        self.active_vertices.clear();
        self.pending.clear();
        self.vertex_marks.resize(mesh.vertices.len(), 0);
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.vertex_marks.fill(0);
            self.stamp = 1;
        }
        if !self.nodes.is_empty() {
            self.pending.push(0);
        }
        while let Some(index) = self.pending.pop() {
            let node = &self.nodes[index];
            let relative = node.center - dab.center;
            let outside = [dab.right, dab.up].into_iter().any(|axis| {
                relative.dot(axis).abs()
                    > dab.radius
                        + node.extent.0 * axis.0.abs()
                        + node.extent.1 * axis.1.abs()
                        + node.extent.2 * axis.2.abs()
            });
            if outside {
                continue;
            }
            if let Some((left, right)) = node.children {
                self.pending.push(left);
                self.pending.push(right);
                continue;
            }
            for &f in &self.order[node.range.clone()] {
                let points = self.world_faces[f].map(|point| {
                    let p = point - dab.center;
                    [
                        p.dot(dab.right) / dab.radius,
                        p.dot(dab.up) / dab.radius,
                        p.dot(dab.normal),
                    ]
                });
                let xy = points.map(|p| [(p[0] + 1.) * 0.5, (1. - p[1]) * 0.5]);
                if (0..2).any(|k| xy.iter().all(|p| p[k] < 0.) || xy.iter().all(|p| p[k] > 1.)) {
                    continue;
                }
                self.faces[f] = points;
                if f < mesh.triangles.len() {
                    self.active_faces.push(f);
                    for i in mesh.triangles[f] {
                        if self.vertex_marks[i] != self.stamp {
                            self.vertex_marks[i] = self.stamp;
                            self.active_vertices.push(i);
                        }
                    }
                }
                let low: [usize; 2] = std::array::from_fn(|k| {
                    (xy.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min) * SIDE as f32)
                        .floor()
                        .clamp(0., (SIDE - 1) as f32) as usize
                });
                let high: [usize; 2] = std::array::from_fn(|k| {
                    (xy.iter().map(|p| p[k]).fold(f32::NEG_INFINITY, f32::max) * SIDE as f32)
                        .floor()
                        .clamp(0., (SIDE - 1) as f32) as usize
                });
                for y in low[1]..=high[1] {
                    for x in low[0]..=high[0] {
                        self.bins[y * SIDE + x].push(f);
                    }
                }
            }
        }
    }
    pub fn projected_face(&self, face: usize) -> [[f32; 3]; 3] {
        self.faces[face]
    }
    pub fn visible(&self, point: Vec3, dab: &Dab, own_face: Option<usize>) -> bool {
        let p = point - dab.center;
        let q = [
            (p.dot(dab.right) / dab.radius + 1.) * 0.5,
            (1. - p.dot(dab.up) / dab.radius) * 0.5,
        ];
        if q.iter().any(|v| !(0. ..=1.).contains(v)) {
            return false;
        }
        let x = (q[0] * SIDE as f32) as usize;
        let y = (q[1] * SIDE as f32) as usize;
        let depth = p.dot(dab.normal);
        !self.bins[y.min(SIDE - 1) * SIDE + x.min(SIDE - 1)]
            .iter()
            .any(|&f| {
                if own_face == Some(f) {
                    return false;
                }
                let face = self.faces[f];
                barycentric(q, face.map(|p| [(p[0] + 1.) * 0.5, (1. - p[1]) * 0.5])).is_some_and(
                    |w| (0..3).map(|c| face[c][2] * w[c]).sum::<f32>() > depth + dab.radius * 1e-5,
                )
            })
    }
}

#[derive(Default)]
pub(crate) struct PaintScratch {
    pub depth: DepthField,
    pub uv_current: Option<bool>,
    pub normals: Vec<Vec3>,
    pub eligible: Vec<bool>,
    pub weights: Vec<(f32, bool)>,
    pub touched: Vec<usize>,
    pub pixels: std::collections::HashMap<usize, (f32, bool)>,
    pub gutter: super::padding::GutterCache,
}
impl PaintScratch {
    pub fn begin_stroke(&mut self) {
        self.uv_current = None;
        self.normals.clear();
        self.eligible.clear();
        self.depth.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_reference(mesh: &Mesh, occluders: &[&Mesh], depth: &DepthField, dab: &Dab) {
        let mut reference = DepthField {
            world_faces: depth.world_faces.clone(),
            order: depth
                .visibility
                .iter()
                .enumerate()
                .filter_map(|(i, &v)| v.then_some(i))
                .collect(),
            ..Default::default()
        };
        if !reference.order.is_empty() {
            reference.build_node_reference(0, reference.order.len());
        }
        assert_eq!(
            depth.order, reference.order,
            "partition and traversal order"
        );
        assert_eq!(depth.nodes, reference.nodes, "exact hierarchy bounds");
        // Compare the retained cache with a fresh hierarchy and projection too.
        reference.world_faces.clear();
        reference.nodes.clear();
        reference.order.clear();
        reference.prepare(mesh, dab, occluders);
        assert_eq!(depth.active_faces, reference.active_faces);
        assert_eq!(depth.active_vertices, reference.active_vertices);
        for y in -5..=5 {
            for x in -5..=5 {
                let p = dab.center
                    + dab.right * (x as f32 * dab.radius / 5.)
                    + dab.up * (y as f32 * dab.radius / 5.);
                assert_eq!(depth.visible(p, dab, None), reference.visible(p, dab, None));
            }
        }
    }
    #[test]
    fn paint_hierarchy_bounds_preserve_reference_partitions_for_reordered_faces() {
        let mut mesh = Mesh::starting_sphere();
        let dab = super::super::tests::dab();
        for case in 0..4 {
            match case {
                1 => mesh.triangles.reverse(),
                2 => mesh.triangles.rotate_left(137),
                3 => {
                    for p in &mut mesh.vertices {
                        p.2 = 0.;
                    }
                }
                _ => {}
            }
            let mut depth = DepthField::default();
            depth.prepare(&mesh, &dab, &[]);
            assert_reference(&mesh, &[], &depth, &dab);
        }
    }
    #[test]
    fn paint_hierarchy_reuse_validates_positions_visibility_and_occluders() {
        let original = Mesh::starting_sphere();
        let barrier = super::super::tests::sheets();
        let dab = super::super::tests::dab();
        for case in 0..7 {
            let mut mesh = original.clone();
            let mut occluder = barrier.clone();
            let mut depth = DepthField::default();
            depth.prepare(&mesh, &dab, &[&occluder]);
            let builds = depth.builds;
            depth.reset();
            // Painting and masks leave the depth hierarchy unchanged.
            mesh.mask[0] = 1.;
            mesh.materials = vec![crate::sculpt::Material::default(); mesh.vertices.len()];
            depth.prepare(&mesh, &dab, &[&occluder]);
            assert_eq!(depth.builds, builds);
            match case {
                0 => mesh.vertices[0].0 += 0.1,
                1 => mesh.triangles[0].swap(0, 1),
                2 => {
                    mesh.hidden_faces = vec![false; mesh.triangles.len()];
                    mesh.hidden_faces[0] = true;
                }
                3 => mesh.vertex_groups[0].visible = false,
                4 => occluder.vertices[0].2 += 0.1,
                5 => {
                    occluder.hidden_faces = vec![true; occluder.triangles.len()];
                }
                _ => {
                    mesh.triangles.pop();
                }
            }
            depth.reset();
            depth.prepare(&mesh, &dab, &[&occluder]);
            assert!(depth.builds > builds, "case {case}");
            assert_reference(&mesh, &[&occluder], &depth, &dab);
        }
    }
    #[test]
    fn paint_spatial_candidates_reuse_static_geometry_and_keep_distant_occluders() {
        let mut mesh = super::super::tests::sheets();
        let template = mesh.clone();
        for n in 1..=64 {
            let base = mesh.vertices.len();
            mesh.vertices.extend(
                template
                    .vertices
                    .iter()
                    .map(|&p| p + Vec3(n as f32 * 10., 0., 0.)),
            );
            mesh.triangles
                .extend(template.triangles.iter().map(|t| t.map(|i| i + base)));
            mesh.mask.extend_from_slice(&template.mask);
        }
        let dab = super::super::tests::dab();
        let mut depth = DepthField::default();
        depth.prepare(&mesh, &dab, &[]);
        assert_eq!(
            depth.active_faces.len(),
            4,
            "off-brush faces are excluded before projection/rasterization"
        );
        assert_eq!(depth.active_vertices.len(), 8);
        assert!(depth.visible(Vec3::default(), &dab, None));
        let storage = depth.world_faces.as_ptr();
        depth.prepare(&mesh, &dab, &[]);
        assert_eq!(depth.world_faces.as_ptr(), storage);
        let mut barrier = template;
        for p in &mut barrier.vertices {
            p.2 += 100.;
        }
        depth.reset();
        depth.prepare(&mesh, &dab, &[&barrier]);
        assert!(
            !depth.visible(Vec3::default(), &dab, None),
            "front occluders are retained even far outside the brush sphere"
        );
    }
}
