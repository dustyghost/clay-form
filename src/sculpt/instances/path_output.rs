//! Optional path deformation and neighbour joins. Generated meshes are transient;
//! projects retain their source links and recipes, and the document caches output.
use super::*;
use crate::sculpt::paths::PathSample;
use std::collections::HashMap;
#[cfg(test)]
mod tests;
mod weld;

pub const MAX_GENERATED_VERTICES: usize = 250_000;
pub const MAX_GENERATED_TRIANGLES: usize = 500_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinMode {
    #[default]
    Separate,
    Weld,
}

#[derive(Debug, Default, PartialEq)]
pub struct ResolvedCopies {
    pub placements: Vec<(u64, usize, Placement)>,
    pub meshes: HashMap<(u64, usize), Arc<Mesh>>,
}
impl std::ops::Deref for ResolvedCopies {
    type Target = Vec<(u64, usize, Placement)>;
    fn deref(&self) -> &Self::Target {
        &self.placements
    }
}
pub struct GeneratedCopy {
    pub slot: usize,
    pub placement: Placement,
    pub mesh: Mesh,
}

struct Frames {
    samples: Vec<PathSample>,
    length: f32,
    closed: bool,
    twist: f32,
}
impl Frames {
    fn new(path: &SurfacePath, twist: f32) -> Result<Self, String> {
        path.validate()?;
        let length: f32 = path
            .dense_curve()
            .windows(2)
            .map(|p| (p[1].position - p[0].position).len())
            .sum();
        if length < 1e-5 {
            return Err("Path is too short to bend sections".into());
        }
        let mut samples = path.sample((length / 1024.).max(1e-5))?;
        if path.closed {
            let mut seam = *samples.first().ok_or("Path has no samples")?;
            seam.distance = length;
            samples.push(seam);
        }
        if samples.len() < 2 {
            return Err("Path needs at least two samples".into());
        }
        let length = samples.last().unwrap().distance;
        Ok(Self {
            samples,
            length,
            closed: path.closed,
            twist,
        })
    }
    fn at(&self, distance: f32) -> Transform {
        let d = if self.closed {
            distance.rem_euclid(self.length)
        } else {
            distance.clamp(0., self.length)
        };
        let i = self
            .samples
            .partition_point(|s| s.distance < d)
            .clamp(1, self.samples.len() - 1);
        let (a, b) = (self.samples[i - 1], self.samples[i]);
        let t = (d - a.distance) / (b.distance - a.distance).max(1e-8);
        let mut forward = (a.tangent * (1. - t) + b.tangent * t).norm();
        if forward.len() < 0.5 {
            forward = a.tangent;
        }
        let normal = a.normal * (1. - t) + b.normal * t;
        let mut up = (normal - forward * normal.dot(forward)).norm();
        if up.len() < 0.5 {
            up = a.binormal.cross(forward).norm();
        }
        up = up.rotated(forward, self.twist.to_radians());
        let mut origin = a.position * (1. - t) + b.position * t;
        if !self.closed {
            origin = origin + forward * (distance - d);
        }
        Transform {
            basis: None,
            origin,
            forward,
            up,
            scale: 1.,
        }
    }
}

impl Set {
    pub fn joined(&self) -> bool {
        matches!(&self.layout, Layout::Path(a) if a.join == JoinMode::Weld)
    }
    pub fn generated<'a>(
        &self,
        paths: &[SurfacePath],
        source: impl Fn(Source) -> Option<&'a Mesh>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<Vec<GeneratedCopy>>, String> {
        let Layout::Path(recipe) = &self.layout else {
            return Ok(None);
        };
        if !recipe.bend && recipe.join == JoinMode::Separate {
            return Ok(None);
        }
        let path = paths
            .iter()
            .find(|p| p.id == recipe.path)
            .ok_or("Assembly path is missing")?;
        let base = recipe.placements(path)?;
        let placements = self.placements(paths)?;
        if recipe.join == JoinMode::Weld && placements.len() != base.len() {
            return Err("Show every section and restore detached copies before joining neighbours, or choose Separate".into());
        }
        let (mut vertices, mut triangles) = (0usize, 0usize);
        for (_, p) in &placements {
            let mesh = source(p.piece.source).ok_or("Instance source is missing")?;
            vertices = vertices.saturating_add(mesh.vertices.len());
            triangles = triangles.saturating_add(mesh.triangles.len());
        }
        if vertices > MAX_GENERATED_VERTICES || triangles > MAX_GENERATED_TRIANGLES {
            return Err("Bending/joining previews are limited to 250,000 vertices / 500,000 triangles; reduce source detail or repeats".into());
        }
        let frames = Frames::new(path, recipe.twist)?;
        let n = base.len();
        let step = if recipe.fit_count {
            frames.length / (n - usize::from(!path.closed)).max(1) as f32
        } else {
            recipe.spacing
        };
        let distances = recipe.section_distances(frames.length, path.closed, n);
        let mut copies = Vec::with_capacity(placements.len());
        for (slot, placement) in placements {
            if cancel() {
                return Err("Operation cancelled".into());
            }
            let original = source(placement.piece.source).ok_or("Instance source is missing")?;
            let mesh = if recipe.bend {
                let mut mesh = original.clone();
                let (lo, hi) = longitudinal_bounds(original, placement.piece)?;
                let start = if slot == 0 {
                    if path.closed { -step * 0.5 } else { 0. }
                } else {
                    (distances[slot - 1] + distances[slot]) * 0.5
                };
                let end = if slot + 1 == n {
                    if path.closed {
                        frames.length - step * 0.5
                    } else if recipe.fit_count || recipe.end.is_some() || n == 1 {
                        frames.length
                    } else {
                        (distances[slot] + step * 0.5).min(frames.length)
                    }
                } else {
                    (distances[slot] + distances[slot + 1]) * 0.5
                };
                for (i, p) in mesh.vertices.iter_mut().enumerate() {
                    if i % 1024 == 0 && cancel() {
                        return Err("Operation cancelled".into());
                    }
                    let local = placement.piece.local(*p);
                    let d = if recipe.fit_sections {
                        start + (local.0 - lo) / (hi - lo) * (end - start)
                    } else {
                        distances[slot] + local.0 * recipe.scale
                    };
                    let frame = frames.at(d);
                    let thickness = recipe.thickness.at(d / frames.length, path.closed);
                    let world = frame.origin
                        + frame.vector(Vec3(0., local.1, local.2)) * (recipe.scale * thickness);
                    // Per-copy edits apply after deformation, around the original pivot.
                    let b = base[slot].transform;
                    let delta = world - b.origin;
                    let relative = Vec3(
                        delta.dot(b.forward),
                        delta.dot(b.up.cross(b.forward)),
                        delta.dot(b.up),
                    ) * (1. / b.scale);
                    *p = placement.transform.point(relative);
                }
                mesh.validate()?;
                mesh
            } else {
                placement.mesh(original)?
            };
            copies.push(GeneratedCopy {
                slot,
                placement,
                mesh,
            });
        }
        if recipe.join == JoinMode::Weld {
            let first = copies.first().ok_or("No sections to join")?;
            let (slot, placement) = (first.slot, first.placement);
            let joined = weld::join(&copies, &source, path.closed, recipe.join_distance, cancel)?;
            copies = vec![GeneratedCopy {
                slot,
                placement,
                mesh: joined,
            }];
        }
        Ok(Some(copies))
    }
}

impl PathAssembly {
    pub(crate) fn section_distances(&self, length: f32, closed: bool, n: usize) -> Vec<f32> {
        let step = if self.fit_count {
            length / (n - usize::from(!closed)).max(1) as f32
        } else {
            self.spacing
        };
        (0..n)
            .map(|i| {
                if n == 1 {
                    length * 0.5
                } else if !closed && self.end.is_some() && i + 1 == n {
                    length
                } else {
                    i as f32 * step
                }
            })
            .collect()
    }
}

fn longitudinal_bounds(mesh: &Mesh, piece: Piece) -> Result<(f32, f32), String> {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in &mesh.vertices {
        let x = piece.local(v).0;
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if !lo.is_finite() || !hi.is_finite() || hi - lo < 1e-5 {
        return Err("Sections need nonzero length along their Forward axis".into());
    }
    Ok((lo, hi))
}
