//! Shared geometry sources and persistent, bounded placement recipes.
//! Instances never own a copy of a live model's mesh.
use super::{Mesh, Vec3, paths::SurfacePath};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};
mod bake;
pub(crate) use bake::{BakeDestination, BakeRequest, BakeScope};
mod path_output;
pub use path_output::{GeneratedCopy, JoinMode, PathRegions, ResolvedCopies};
mod arrays;
pub use arrays::{
    Array, ArrayKind, ArrayLayer, ArrayResolver, ArraySpacing, Basis, MAX_ARRAY_LAYERS,
};
mod brushes;
mod thickness;
pub use brushes::{BrushBundle, BrushGroup};
pub(crate) use brushes::{SHIP_GREEBLES_NAME, ship_greebles};
pub use thickness::{
    MAX_THICKNESS, MAX_THICKNESS_POINTS, MIN_THICKNESS, Thickness, ThicknessMode, ThicknessPoint,
};

pub const MAX_INSTANCES: usize = 256;
pub const MAX_ASSETS: usize = 64;
pub const MAX_VERTICES: usize = 1_000_000;
pub const MAX_TRIANGLES: usize = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectOptions {
    pub scale: f32,
    pub spacing: f32,
    pub offset: f32,
    pub rotation: f32,
    pub independent: bool,
    #[serde(default)]
    pub stamp: bool,
}

impl Default for ObjectOptions {
    fn default() -> Self {
        Self {
            scale: 0.2,
            spacing: 0.25,
            offset: 0.,
            rotation: 0.,
            independent: false,
            stamp: false,
        }
    }
}
impl ObjectOptions {
    pub fn validate(&self) -> Result<(), String> {
        if !self.scale.is_finite()
            || !(0.001..=1000.).contains(&self.scale)
            || !self.spacing.is_finite()
            || !(0.001..=1000.).contains(&self.spacing)
            || !self.offset.is_finite()
            || self.offset.abs() > 1e5
            || !self.rotation.is_finite()
            || self.rotation.abs() > 3600.
        {
            return Err("Invalid object brush size, spacing, rotation or offset".into());
        }
        Ok(())
    }
}

pub fn finite(v: Vec3) -> bool {
    [v.0, v.1, v.2]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1e6)
}
fn name_valid(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 128
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    Model(u64),
    Asset(u64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub mesh: Arc<Mesh>,
    /// View of the source when captured, used for the brush library thumbnail.
    #[serde(default = "default_capture_angles")]
    pub capture_angles: [f32; 3],
    pub pivot: Vec3,
    pub forward: usize,
    pub up: usize,
    #[serde(default)]
    pub options: ObjectOptions,
}

fn default_capture_angles() -> [f32; 3] {
    [0.35, -0.25, 0.]
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub source: Source,
    pub pivot: Vec3,
    pub forward: usize,
    pub up: usize,
}
impl Piece {
    pub fn validate(&self) -> Result<(), String> {
        if matches!(self.source, Source::Model(0) | Source::Asset(0))
            || !finite(self.pivot)
            || self.forward > 2
            || self.up > 2
            || self.forward == self.up
        {
            return Err("Choose distinct forward/up axes and a finite contact point".into());
        }
        Ok(())
    }
    pub fn local(&self, p: Vec3) -> Vec3 {
        let p = p - self.pivot;
        let forward = axis(self.forward);
        let up = axis(self.up);
        Vec3(p.dot(forward), p.dot(up.cross(forward)), p.dot(up))
    }
}
pub fn axis(index: usize) -> Vec3 {
    match index {
        0 => Vec3(1., 0., 0.),
        1 => Vec3(0., 1., 0.),
        _ => Vec3(0., 0., 1.),
    }
}
pub fn rotate(mut p: Vec3, degrees: Vec3) -> Vec3 {
    for (i, angle) in [degrees.0, degrees.1, degrees.2].into_iter().enumerate() {
        p = p.rotated(axis(i), angle.to_radians());
    }
    p
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<Basis>,
    pub origin: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub scale: f32,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            basis: None,
            origin: Vec3::default(),
            forward: axis(0),
            up: axis(2),
            scale: 1.,
        }
    }
}
impl Transform {
    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(b) = self.basis {
            b.scaled(self.scale).validate()?;
        }
        if !finite(self.origin)
            || !finite(self.forward)
            || !finite(self.up)
            || (self.forward.len() - 1.).abs() > 1e-3
            || (self.up.len() - 1.).abs() > 1e-3
            || self.forward.dot(self.up).abs() > 1e-3
            || !self.scale.is_finite()
            || !(0.001..=1000.).contains(&self.scale)
        {
            return Err(
                "Placement needs an orthogonal frame and scale between 0.001 and 1000".into(),
            );
        }
        Ok(())
    }
    pub fn vector(&self, p: Vec3) -> Vec3 {
        if let Some(b) = self.basis {
            b.vector(p)
        } else {
            self.forward * p.0 + self.up.cross(self.forward) * p.1 + self.up * p.2
        }
    }
    pub fn point(&self, p: Vec3) -> Vec3 {
        self.origin + self.vector(p) * self.scale
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub piece: Piece,
    pub transform: Transform,
}
impl Placement {
    pub fn point(&self, p: Vec3) -> Vec3 {
        self.transform.point(self.piece.local(p))
    }
    pub fn normal_basis(&self) -> Basis {
        // Map source-space normals once per placement, including inherited shear.
        let mut piece = self.piece;
        piece.pivot = Vec3::default();
        let normals = self.transform.basis.map(Basis::cofactors);
        Basis(std::array::from_fn(|i| {
            let local = piece.local(axis(i));
            normals.map_or_else(|| self.transform.vector(local), |b| b.vector(local))
        }))
    }
    pub fn mesh(&self, source: &Mesh) -> Result<Mesh, String> {
        self.piece.validate()?;
        self.transform.validate()?;
        let mut mesh = source.clone();
        for p in &mut mesh.vertices {
            *p = self.point(*p);
        }
        mesh.validate()?;
        Ok(mesh)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathAssembly {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regions: Option<PathRegions>,
    pub path: u64,
    pub start: Option<Piece>,
    pub middle: Piece,
    pub end: Option<Piece>,
    pub count: usize,
    pub spacing: f32,
    pub fit_count: bool,
    pub twist: f32,
    pub scale: f32,
    #[serde(default)]
    pub bend: bool,
    #[serde(default)]
    pub fit_sections: bool,
    #[serde(default)]
    pub join: JoinMode,
    #[serde(default = "default_join_distance")]
    pub join_distance: f32,
    #[serde(default)]
    pub thickness: Thickness,
}
pub fn default_join_distance() -> f32 {
    0.05
}
impl PathAssembly {
    pub fn placements(&self, path: &SurfacePath) -> Result<Vec<Placement>, String> {
        if !(1..=MAX_INSTANCES - 2).contains(&self.count)
            || !self.spacing.is_finite()
            || !(0.001..=1e5).contains(&self.spacing)
            || !self.twist.is_finite()
            || self.twist.abs() > 3600.
            || !self.scale.is_finite()
            || !(0.001..=1000.).contains(&self.scale)
            || !self.join_distance.is_finite()
            || !(0.00001..=1000.).contains(&self.join_distance)
        {
            return Err(
                "Invalid path assembly spacing, count, scale, twist or join distance".into(),
            );
        }
        self.thickness.validate()?;
        for p in [self.start, Some(self.middle), self.end]
            .into_iter()
            .flatten()
        {
            p.validate()?;
        }
        path.validate()?;
        // The existing path sampler transports frames and closes the seam.
        let curve = path.dense_curve();
        let length: f32 = curve
            .windows(2)
            .map(|p| (p[1].position - p[0].position).len())
            .sum();
        if length < 1e-5 {
            return Err("Path is too short for an assembly".into());
        }
        if let Some(regions) = &self.regions {
            regions.validate()?;
            if self.start.is_some()
                || self.end.is_some()
                || !self.bend
                || !self.fit_sections
                || !self.fit_count
                || self.join != JoinMode::Weld
            {
                return Err("Grouped path objects use one source model, fitted middle repeats and welded boundaries".into());
            }
            return Ok(vec![Placement {
                piece: self.middle,
                transform: Transform::default(),
            }]);
        }
        let samples = path.sample((length / 1024.).max(1e-5))?;
        let mut samples = samples;
        if path.closed {
            let mut seam = *samples.first().ok_or("Path has no samples")?;
            seam.distance = length;
            samples.push(seam);
        }
        let length = samples.last().ok_or("Path has no samples")?.distance;
        let has_start = !path.closed && self.start.is_some();
        let has_end = !path.closed && self.end.is_some();
        let count = if self.fit_count {
            self.count
        } else {
            ((length / self.spacing).floor() as usize + usize::from(!path.closed))
                .saturating_sub(usize::from(has_start) + usize::from(has_end))
        };
        if count == 0 || count > MAX_INSTANCES - 2 {
            return Err("Increase spacing to keep the assembly within 254 middle pieces".into());
        }
        let slots = count + usize::from(has_start) + usize::from(has_end);
        let step = if self.fit_count {
            length / (slots - usize::from(!path.closed)).max(1) as f32
        } else {
            self.spacing
        };
        let sample_at = |distance: f32| {
            let d = distance.clamp(0., length);
            let i = samples
                .partition_point(|s| s.distance < d)
                .clamp(1, samples.len() - 1);
            let a = samples[i - 1];
            let b = samples[i];
            let f = (d - a.distance) / (b.distance - a.distance).max(1e-8);
            let tangent = (a.tangent * (1. - f) + b.tangent * f).norm();
            let n = a.normal * (1. - f) + b.normal * f;
            let up = (n - tangent * n.dot(tangent))
                .norm()
                .rotated(tangent, self.twist.to_radians());
            Transform {
                basis: None,
                origin: a.position * (1. - f) + b.position * f,
                forward: tangent,
                up,
                scale: self.scale,
            }
        };
        let mut out = Vec::new();
        if has_start {
            out.push(Placement {
                piece: self.start.unwrap(),
                transform: sample_at(0.),
            });
        }
        for i in 0..count {
            let d = if slots == 1 {
                length * 0.5
            } else {
                (i + usize::from(has_start)) as f32 * step
            };
            if d > length + 1e-5 {
                return Err(
                    "Caps and repeats exceed the path length; use Fit count or increase spacing"
                        .into(),
                );
            }
            out.push(Placement {
                piece: self.middle,
                transform: sample_at(d),
            });
        }
        if has_end {
            out.push(Placement {
                piece: self.end.unwrap(),
                transform: sample_at(length),
            });
        }
        for p in &out {
            p.transform.validate()?;
        }
        Ok(out)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Layout {
    Manual(Vec<Placement>),
    Array(Array),
    Path(PathAssembly),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adjustment {
    pub slot: usize,
    pub offset: Vec3,
    pub rotation: Vec3,
    #[serde(default = "arrays::ones", skip_serializing_if = "arrays::unit_scale")]
    pub axis_scale: Vec3,
    pub scale: f32,
    pub visible: bool,
    /// A detached copy is omitted even when the recipe is regenerated.
    pub detached: bool,
}
impl Adjustment {
    pub fn new(slot: usize) -> Self {
        Self {
            slot,
            offset: Vec3::default(),
            rotation: Vec3::default(),
            axis_scale: arrays::ones(),
            scale: 1.,
            visible: true,
            detached: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Set {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub layout: Layout,
    pub adjustments: Vec<Adjustment>,
}
impl Set {
    /// Include the stable set number because several arrays or strokes can share a name.
    pub(crate) fn target_name(&self, slot: Option<usize>) -> String {
        let name = format!("{} #{}", self.name, self.id);
        match slot {
            Some(slot) if !self.joined() => format!("{name} · Copy {}", slot + 1),
            _ => name,
        }
    }

    pub fn placements(&self, paths: &[SurfacePath]) -> Result<Vec<(usize, Placement)>, String> {
        if let Layout::Array(a) = &self.layout {
            return Ok(arrays::numbered(self.adjust_slots(a.unlinked_slots()?)?));
        }
        let base =
            match &self.layout {
                Layout::Manual(p) => p.clone(),
                Layout::Path(a) => a.placements(paths.iter().find(|p| p.id == a.path).ok_or(
                    "Assembly path is missing; undo its deletion or remove the assembly",
                )?)?,
                Layout::Array(_) => unreachable!(),
            };
        Ok(arrays::numbered(
            self.adjust_slots(base.into_iter().map(Some).collect())?,
        ))
    }
    fn adjust_slots(
        &self,
        mut base: Vec<Option<Placement>>,
    ) -> Result<Vec<Option<Placement>>, String> {
        if self.id == 0 || !name_valid(&self.name) || self.adjustments.len() > MAX_INSTANCES {
            return Err("Invalid instance set".into());
        }
        if base.len() > MAX_INSTANCES {
            return Err("Instance sets are limited to 256 copies".into());
        }
        let mut slots = HashSet::new();
        for a in &self.adjustments {
            if a.slot >= MAX_INSTANCES
                || !slots.insert(a.slot)
                || !finite(a.offset)
                || !finite(a.rotation)
                || !arrays::valid_scale(a.axis_scale)
                || !a.scale.is_finite()
                || !(0.001..=1000.).contains(&a.scale)
            {
                return Err("Invalid instance adjustment".into());
            }
        }
        for (slot, entry) in base.iter_mut().enumerate() {
            let Some(p) = entry else { continue };
            p.piece.validate()?;
            p.transform.validate()?;
            if let Some(a) = self.adjustments.iter().find(|a| a.slot == slot) {
                if a.detached || !a.visible {
                    *entry = None;
                    continue;
                }
                p.transform.origin = p.transform.origin + a.offset;
                if a.rotation != Vec3::default() || !arrays::unit_scale(&a.axis_scale) {
                    p.transform.prepend(
                        Basis::step(a.rotation, a.axis_scale),
                        arrays::unit_scale(&a.axis_scale),
                    );
                }
                p.transform.scale *= a.scale;
                p.transform.validate()?;
            }
        }
        Ok(base)
    }
    pub fn sources_mut(&mut self, mut f: impl FnMut(&mut Source)) {
        match &mut self.layout {
            Layout::Manual(p) => {
                for p in p {
                    f(&mut p.piece.source);
                }
            }
            Layout::Array(a) if a.source_array.is_none() => f(&mut a.piece.source),
            Layout::Array(_) => {}
            Layout::Path(a) => {
                f(&mut a.middle.source);
                for p in [&mut a.start, &mut a.end].into_iter().flatten() {
                    f(&mut p.source);
                }
            }
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub assets: Vec<Asset>,
    pub sets: Vec<Set>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub brushes: Vec<BrushGroup>,
}
impl Library {
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty() && self.sets.is_empty() && self.brushes.is_empty()
    }
    pub fn validate(&self, models: &[(u64, &Mesh)], paths: &[SurfacePath]) -> Result<(), String> {
        self.validate_brushes()?;
        super::texture::validate_library(self.assets.iter().map(|a| a.mesh.as_ref()), &[])?;
        if self.assets.len() > MAX_ASSETS || self.sets.len() > MAX_INSTANCES {
            return Err("Too many object assets or instance sets".into());
        }
        let mut ids = HashSet::new();
        for a in &self.assets {
            if a.id == 0 || !ids.insert(a.id) || !name_valid(&a.name) {
                return Err("Invalid or duplicate object asset identity".into());
            }
            Piece {
                source: Source::Asset(a.id),
                pivot: a.pivot,
                forward: a.forward,
                up: a.up,
            }
            .validate()?;
            a.mesh.validate()?;
            a.options.validate()?;
            if !a.capture_angles.iter().all(|a| a.is_finite()) {
                return Err("Invalid object thumbnail orientation".into());
            }
            if a.mesh.vertices.len() > 100_000 || a.mesh.triangles.len() > 200_000 {
                return Err("Object brush exceeds 100,000 vertices / 200,000 triangles".into());
            }
        }
        if self
            .assets
            .iter()
            .map(|a| a.mesh.vertices.len())
            .sum::<usize>()
            > MAX_VERTICES
        {
            return Err("Object library exceeds one million source vertices".into());
        }
        self.validate_sets(models, paths)
    }
    pub(crate) fn validate_sets(
        &self,
        models: &[(u64, &Mesh)],
        paths: &[SurfacePath],
    ) -> Result<(), String> {
        if self.sets.len() > MAX_INSTANCES {
            return Err("Too many instance sets".into());
        }
        let mut ids = HashSet::new();
        let mut count = 0;
        let mut owners = HashSet::new();
        let mut resolver = ArrayResolver::new(self).with_sources(|s| match s {
            Source::Model(id) => models.iter().find(|(i, _)| *i == id).map(|(_, m)| *m),
            Source::Asset(id) => self
                .assets
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.mesh.as_ref()),
        });
        for set in &self.sets {
            if let Layout::Array(a) = &set.layout
                && let Some(owner) = a.owner
                && (owner == 0
                    || !owners.insert(owner)
                    || a.piece.source != Source::Model(owner)
                    || a.source_array.is_some()
                    || !models.iter().any(|(id, _)| *id == owner))
            {
                return Err("Invalid or duplicate submodel array ownership".into());
            }
            if !ids.insert(set.id) {
                return Err("Duplicate instance set identity".into());
            }
            let mut probe = set.clone();
            let mut missing = false;
            probe.sources_mut(|source| {
                missing |= match source {
                    Source::Model(id) => !models.iter().any(|(mid, _)| mid == id),
                    Source::Asset(id) => !self.assets.iter().any(|a| a.id == *id),
                }
            });
            if missing {
                return Err("Instance source is missing".into());
            }
            let placements = resolver.placements(set, paths)?;
            count += match &set.layout {
                Layout::Path(a) if a.regions.is_some() => {
                    a.count + 2 * usize::from(!paths.iter().any(|p| p.id == a.path && p.closed))
                }
                _ => placements.len(),
            };
        }
        if count > MAX_INSTANCES {
            return Err("Projects are limited to 256 instances".into());
        }
        Ok(())
    }
}

impl super::Geometry {
    pub(crate) fn instance_source(&self, source: Source) -> Option<&Mesh> {
        match source {
            Source::Model(id) if self.scene.active_metadata.model_id == id => Some(&self.mesh),
            Source::Model(id) => self
                .submodels
                .iter()
                .find(|m| m.metadata.model_id == id)
                .map(|m| &m.mesh),
            Source::Asset(id) => self
                .scene
                .instances
                .assets
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.mesh.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn piece() -> Piece {
        Piece {
            source: Source::Model(1),
            pivot: Vec3::default(),
            forward: 0,
            up: 2,
        }
    }
    #[test]
    fn reusable_array_layouts_and_adjustments_are_predictable() {
        let mut a = Array::new(piece());
        a.count = 4;
        a.spacing = Vec3(2., 3., 0.);
        assert_eq!(
            a.placements().unwrap()[3].transform.origin,
            Vec3(6., 9., 0.)
        );
        a.kind = ArrayKind::Grid;
        a.columns = 2;
        assert_eq!(
            a.placements().unwrap()[3].transform.origin,
            Vec3(2., 3., 0.)
        );
        a.kind = ArrayKind::Radial;
        a.radius = 2.;
        a.angle = 360.;
        a.radial_axis = 2;
        let p = a.placements().unwrap();
        for x in &p {
            x.transform.validate().unwrap();
            assert!((x.transform.origin.len() - 2.).abs() < 1e-5);
        }
        assert!((p[0].transform.origin - p[3].transform.origin).len() > 1.);
        let mut adjustment = Adjustment::new(2);
        adjustment.offset = Vec3(0., 0., 3.);
        adjustment.rotation = Vec3(15., 20., 30.);
        adjustment.scale = 2.;
        let set = Set {
            id: 1,
            name: "Ring".into(),
            visible: true,
            layout: Layout::Array(a),
            adjustments: vec![adjustment],
        };
        let out = set.placements(&[]).unwrap();
        assert_eq!(out[0].1, p[0]);
        assert_eq!(out[2].1.transform.origin.2, 3.);
    }
    #[test]
    fn reusable_closed_path_has_no_duplicate_seam_and_keeps_frames() {
        use super::super::paths::*;
        let path = SurfacePath {
            id: 1,
            target: 1,
            name: "Loop".into(),
            mode: PathMode::Free,
            points: (0..16)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / 16.;
                    PathPoint {
                        position: Vec3(angle.cos(), angle.sin(), 0.),
                        normal: axis(2),
                    }
                })
                .collect(),
            closed: true,
            seam: 0,
            smoothing: 0.5,
            corner_rounding: Vec::new(),
            surface_signature: 0,
            attached_route: None,
        };
        let a = PathAssembly {
            regions: None,
            path: 1,
            start: Some(piece()),
            middle: piece(),
            end: Some(piece()),
            count: 12,
            spacing: 0.1,
            fit_count: true,
            twist: 25.,
            scale: 0.1,
            bend: false,
            fit_sections: false,
            join: JoinMode::Separate,
            join_distance: default_join_distance(),
            thickness: Default::default(),
        };
        let p = a.placements(&path).unwrap();
        assert_eq!(p.len(), 12);
        let distances: Vec<_> = (0..p.len())
            .map(|i| (p[i].transform.origin - p[(i + 1) % p.len()].transform.origin).len())
            .collect();
        assert!(distances.iter().all(|&d| d > 0.3));
        let min = distances.iter().copied().fold(f32::INFINITY, f32::min);
        let max = distances.iter().copied().fold(0., f32::max);
        assert!(max / min < 1.1, "closed-loop spacing: {distances:?}");
        for i in 0..p.len() {
            assert!(p[i].transform.up.dot(p[(i + 1) % p.len()].transform.up) > 0.8);
        }
    }
}
