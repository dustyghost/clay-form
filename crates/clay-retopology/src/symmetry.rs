// Tangent-lattice helpers adapted from quadrs (MIT); see LICENSE-quadrs.
use crate::Vec3;
use glam::IVec2;

const EPS: f64 = 1e-12;

/// Local tangent frame: `q` is one RoSy axis and `n` is the surface normal.
#[derive(Clone, Copy)]
pub struct Frame {
    pub q: Vec3,
    pub n: Vec3,
}

impl Frame {
    pub fn new(q: Vec3, n: Vec3) -> Self {
        Self { q, n }
    }
}

/// Local sample: vertex position `p`, lattice origin `o`, and its tangent frame.
#[derive(Clone, Copy)]
pub struct Sample {
    pub p: Vec3,
    pub o: Vec3,
    pub frame: Frame,
}

impl Sample {
    pub fn new(p: Vec3, o: Vec3, frame: Frame) -> Self {
        Self { p, o, frame }
    }
}

pub struct OrientationMatch {
    pub lhs: (Vec3, i32),
    pub rhs: (Vec3, i32),
}

pub struct PositionMatch {
    pub lhs: (Vec3, IVec2),
    pub rhs: (Vec3, IVec2),
}

pub trait RoSy4 {
    fn match_orientation(lhs: Frame, rhs: Frame) -> OrientationMatch;
    fn match_position(lhs: Sample, rhs: Sample, scale: f64, inv_scale: f64) -> PositionMatch;
}

pub struct Extrinsic;

impl RoSy4 for Extrinsic {
    fn match_orientation(lhs: Frame, rhs: Frame) -> OrientationMatch {
        let a = [lhs.q, lhs.n.cross(lhs.q)];
        let b = [rhs.q, rhs.n.cross(rhs.q)];
        let mut best = (0usize, 0usize, f64::NEG_INFINITY);
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                let score = x.dot(*y).abs();
                if score > best.2 {
                    best = (i, j, score);
                }
            }
        }
        let dp = a[best.0].dot(b[best.1]);
        OrientationMatch {
            lhs: (a[best.0], best.0 as i32),
            rhs: (
                b[best.1] * dp.signum(),
                best.1 as i32 + if dp < 0.0 { 2 } else { 0 },
            ),
        }
    }

    fn match_position(lhs: Sample, rhs: Sample, scale: f64, inv_scale: f64) -> PositionMatch {
        let middle = middle_point(lhs.p, lhs.frame.n, rhs.p, rhs.frame.n);
        let o0p = position_floor_index_4(lhs.o, lhs.frame.q, lhs.frame.n, middle, inv_scale);
        let o1p = position_floor_index_4(rhs.o, rhs.frame.q, rhs.frame.n, middle, inv_scale);
        let mut best = (0i32, 0i32, f64::INFINITY);
        for i in 0..4 {
            let lhs_index = IVec2::new((i & 1) + o0p.x, ((i & 2) >> 1) + o0p.y);
            let o0t = position_from_index(lhs.o, lhs.frame.q, lhs.frame.n, lhs_index, scale);
            for j in 0..4 {
                let rhs_index = IVec2::new((j & 1) + o1p.x, ((j & 2) >> 1) + o1p.y);
                let o1t = position_from_index(rhs.o, rhs.frame.q, rhs.frame.n, rhs_index, scale);
                let cost = (o0t - o1t).length_squared();
                if cost < best.2 {
                    best = (i, j, cost);
                }
            }
        }
        let lhs_index = IVec2::new((best.0 & 1) + o0p.x, ((best.0 & 2) >> 1) + o0p.y);
        let rhs_index = IVec2::new((best.1 & 1) + o1p.x, ((best.1 & 2) >> 1) + o1p.y);
        PositionMatch {
            lhs: (
                position_from_index(lhs.o, lhs.frame.q, lhs.frame.n, lhs_index, scale),
                lhs_index,
            ),
            rhs: (
                position_from_index(rhs.o, rhs.frame.q, rhs.frame.n, rhs_index, scale),
                rhs_index,
            ),
        }
    }
}

pub fn rotate_vector_into_plane(q: Vec3, source_normal: Vec3, target_normal: Vec3) -> Vec3 {
    let cos_theta = source_normal.dot(target_normal);
    if cos_theta < 0.9999 {
        let axis = source_normal.cross(target_normal);
        let denom = axis.dot(axis).max(EPS);
        q * cos_theta + axis.cross(q) + axis * (axis.dot(q) * (1.0 - cos_theta) / denom)
    } else {
        q
    }
}

fn middle_point(p0: Vec3, n0: Vec3, p1: Vec3, n1: Vec3) -> Vec3 {
    let n0p0 = n0.dot(p0);
    let n0p1 = n0.dot(p1);
    let n1p0 = n1.dot(p0);
    let n1p1 = n1.dot(p1);
    let n0n1 = n0.dot(n1);
    let denom = 1.0 / (1.0 - n0n1 * n0n1 + 1e-4);
    let lambda0 = 2.0 * (n0p1 - n0p0 - n0n1 * (n1p0 - n1p1)) * denom;
    let lambda1 = 2.0 * (n1p0 - n1p1 - n0n1 * (n0p1 - n0p0)) * denom;
    (p0 + p1) * 0.5 - (n0 * lambda0 + n1 * lambda1) * 0.25
}

fn position_floor_index_4(o: Vec3, q: Vec3, n: Vec3, p: Vec3, inv_scale: f64) -> IVec2 {
    let t = n.cross(q);
    let d = p - o;
    IVec2::new(
        (q.dot(d) * inv_scale).floor() as i32,
        (t.dot(d) * inv_scale).floor() as i32,
    )
}

fn position_from_index(o: Vec3, q: Vec3, n: Vec3, index: IVec2, scale: f64) -> Vec3 {
    let t = n.cross(q);
    o + (q * index.x as f64 + t * index.y as f64) * scale
}
