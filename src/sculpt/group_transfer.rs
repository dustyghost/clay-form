//! Spatial transfer of group data after whole-surface voxel reconstruction.
use super::{Mesh, Vec3, VertexGroup};
struct Node {
    point: usize,
    axis: usize,
    left: usize,
    right: usize,
}
struct Tree {
    points: Vec<Vec3>,
    nodes: Vec<Node>,
    root: usize,
}
impl Tree {
    fn new(points: Vec<Vec3>) -> Self {
        fn build(points: &[Vec3], ids: &mut [usize], nodes: &mut Vec<Node>, depth: usize) -> usize {
            if ids.is_empty() {
                return usize::MAX;
            }
            let axis = depth % 3;
            let mid = ids.len() / 2;
            ids.select_nth_unstable_by(mid, |&a, &b| {
                component(points[a], axis)
                    .total_cmp(&component(points[b], axis))
                    .then(a.cmp(&b))
            });
            let point = ids[mid];
            let index = nodes.len();
            nodes.push(Node {
                point,
                axis,
                left: usize::MAX,
                right: usize::MAX,
            });
            let (left, right) = ids.split_at_mut(mid);
            let l = build(points, left, nodes, depth + 1);
            let r = build(points, &mut right[1..], nodes, depth + 1);
            nodes[index].left = l;
            nodes[index].right = r;
            index
        }
        let mut ids: Vec<_> = (0..points.len()).collect();
        let mut nodes = Vec::with_capacity(points.len());
        let root = build(&points, &mut ids, &mut nodes, 0);
        Self {
            points,
            nodes,
            root,
        }
    }
    fn nearest(&self, p: Vec3) -> usize {
        fn visit(tree: &Tree, index: usize, p: Vec3, best: &mut (usize, f32)) {
            if index == usize::MAX {
                return;
            }
            let n = &tree.nodes[index];
            let q = tree.points[n.point];
            let d = (q - p).dot(q - p);
            if d < best.1 {
                *best = (n.point, d);
            }
            let delta = component(p, n.axis) - component(q, n.axis);
            let (near, far) = if delta < 0. {
                (n.left, n.right)
            } else {
                (n.right, n.left)
            };
            visit(tree, near, p, best);
            if delta * delta <= best.1 {
                visit(tree, far, p, best);
            }
        }
        let mut best = (0, f32::INFINITY);
        visit(self, self.root, p, &mut best);
        best.0
    }
}
fn component(v: Vec3, axis: usize) -> f32 {
    match axis {
        0 => v.0,
        1 => v.1,
        _ => v.2,
    }
}
impl Mesh {
    pub(crate) fn transfer_groups_to(&self, dest: &mut Mesh) {
        if self.vertex_groups.is_empty() {
            return;
        }
        let tree = Tree::new(self.vertices.clone());
        let source: Vec<_> = dest.vertices.iter().map(|&p| tree.nearest(p)).collect();
        dest.vertex_groups = self
            .vertex_groups
            .iter()
            .map(|g| VertexGroup {
                visible: g.visible,
                name: g.name.clone(),
                weights: source.iter().map(|&i| g.weights[i]).collect(),
            })
            .collect();
        if !self.polygroups.is_empty() {
            let faces = Tree::new(
                self.triangles
                    .iter()
                    .map(|&[a, b, c]| {
                        (self.vertices[a] + self.vertices[b] + self.vertices[c]) * (1. / 3.)
                    })
                    .collect(),
            );
            dest.polygroups = dest
                .triangles
                .iter()
                .map(|&[a, b, c]| {
                    self.polygroups[faces.nearest(
                        (dest.vertices[a] + dest.vertices[b] + dest.vertices[c]) * (1. / 3.),
                    )]
                })
                .collect();
            dest.canonicalize_polygroups();
        }
    }
}
