use super::*;
const SIDE: usize = 64;

#[derive(Default)]
struct Node {
    center: Vec3,
    extent: Vec3,
    range: std::ops::Range<usize>,
    children: Option<(usize, usize)>,
}
/// A stroke-local world-space hierarchy and reusable projected front-surface bins.
/// Query bounds are unbounded along the view direction, retaining distant occluders.
#[derive(Default)]
pub(crate) struct DepthField {
    world_faces: Vec<[Vec3; 3]>,
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
        self.world_faces.clear();
        self.order.clear();
        self.nodes.clear();
    }
    fn build_node(&mut self, start: usize, end: usize) -> usize {
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
            let left = self.build_node(start, mid);
            let right = self.build_node(mid, end);
            self.nodes[index].children = Some((left, right));
        }
        index
    }
    pub fn prepare(&mut self, mesh: &Mesh, dab: &Dab, occluders: &[&Mesh]) {
        if self.world_faces.is_empty() {
            for surface in std::iter::once(mesh).chain(occluders.iter().copied()) {
                for (f, triangle) in surface.triangles.iter().enumerate() {
                    let index = self.world_faces.len();
                    self.world_faces.push(triangle.map(|i| surface.vertices[i]));
                    if surface.face_visible(f) {
                        self.order.push(index);
                    }
                }
            }
            self.faces.resize(self.world_faces.len(), [[0.; 3]; 3]);
            if !self.order.is_empty() {
                self.build_node(0, self.order.len());
            }
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
