//! Array layers operate on placements, never meshes. A linked array resolves the
//! source recipe once, then composes its leaf transforms at each repetition.
use super::*;
use std::collections::HashMap;
#[cfg(test)]
mod tests;
type CachedSlots = (Arc<Vec<Option<Placement>>>, usize);

pub const MAX_ARRAY_LAYERS: usize = 8;
const MAX_LINK_DEPTH: usize = 16;
pub(super) fn ones() -> Vec3 {
    Vec3(1., 1., 1.)
}
pub(super) fn unit_scale(v: &Vec3) -> bool {
    *v == ones()
}
fn zero(v: &Vec3) -> bool {
    *v == Vec3::default()
}
pub(super) fn valid_scale(v: Vec3) -> bool {
    [v.0, v.1, v.2]
        .into_iter()
        .all(|s| (0.001..=1000.).contains(&s))
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Basis(pub [Vec3; 3]);
impl Basis {
    pub fn vector(self, v: Vec3) -> Vec3 {
        self.0[0] * v.0 + self.0[1] * v.1 + self.0[2] * v.2
    }
    pub(super) fn compose(self, inner: Self) -> Self {
        Self(inner.0.map(|v| self.vector(v)))
    }
    pub(super) fn scaled(self, scale: f32) -> Self {
        Self(self.0.map(|v| v * scale))
    }
    pub(super) fn cofactors(self) -> Self {
        let [x, y, z] = self.0;
        Self([y.cross(z), z.cross(x), x.cross(y)])
    }
    pub(super) fn step(rotation: Vec3, scale: Vec3) -> Self {
        if rotation == Vec3::default() {
            return Self([
                Vec3(scale.0, 0., 0.),
                Vec3(0., scale.1, 0.),
                Vec3(0., 0., scale.2),
            ]);
        }
        Self([0, 1, 2].map(|i| rotate(axis(i), rotation) * [scale.0, scale.1, scale.2][i]))
    }
    pub(super) fn validate(self) -> Result<(), String> {
        let [x, y, z] = self.0;
        let lengths = [x.len(), y.len(), z.len()];
        if lengths.iter().any(|s| !(0.000999..=1000.01).contains(s))
            || x.dot(y.cross(z)) <= lengths.iter().product::<f32>() * 1e-6
        {
            return Err("Array scale is too small, large or flat after repeating; reduce the count or scale step".into());
        }
        Ok(())
    }
}
impl Transform {
    pub(super) fn prepend(&mut self, outer: Basis, rigid: bool) {
        if self.basis.is_none() && rigid {
            self.forward = outer.vector(self.forward);
            self.up = outer.vector(self.up);
        } else {
            let composed = outer.compose(self.linear());
            let x = composed.0[0].norm();
            self.forward = x;
            self.up = x.cross(composed.0[1].norm()).norm();
            self.basis = Some(composed);
        }
    }
    pub(super) fn linear(&self) -> Basis {
        self.basis
            .unwrap_or_else(|| Basis([self.forward, self.up.cross(self.forward), self.up]))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrayKind {
    Linear,
    Radial,
    Grid,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArrayLayer {
    pub kind: ArrayKind,
    pub count: usize,
    pub columns: usize,
    pub spacing: Vec3,
    pub center: Vec3,
    pub radius: f32,
    pub angle: f32,
    pub radial_axis: usize,
    pub rotate_copies: bool,
    #[serde(default, skip_serializing_if = "zero")]
    pub rotation: Vec3,
    #[serde(default = "ones", skip_serializing_if = "unit_scale")]
    pub size: Vec3,
    #[serde(default, skip_serializing_if = "zero")]
    pub step_offset: Vec3,
    #[serde(default, skip_serializing_if = "zero")]
    pub step_rotation: Vec3,
    #[serde(default = "ones", skip_serializing_if = "unit_scale")]
    pub step_scale: Vec3,
}
impl ArrayLayer {
    pub fn new(center: Vec3) -> Self {
        Self {
            kind: ArrayKind::Linear,
            count: 5,
            columns: 3,
            spacing: Vec3(2., 2., 0.),
            center,
            radius: 3.,
            angle: 360.,
            radial_axis: 2,
            rotate_copies: true,
            rotation: Vec3::default(),
            size: ones(),
            step_offset: Vec3::default(),
            step_rotation: Vec3::default(),
            step_scale: ones(),
        }
    }
    pub fn repeat() -> Self {
        Self {
            count: 2,
            spacing: Vec3(0., 2., 0.),
            ..Self::new(Vec3::default())
        }
    }
    fn expand(
        &self,
        input: &[Option<Placement>],
        anchor: Vec3,
        destination: Vec3,
    ) -> Result<Vec<Option<Placement>>, String> {
        if !(1..=MAX_INSTANCES).contains(&self.count)
            || !(1..=MAX_INSTANCES).contains(&self.columns)
            || !finite(self.spacing)
            || !finite(self.step_offset)
            || !finite(self.center)
            || self.radial_axis > 2
            || !(0.001..=1e5).contains(&self.radius)
            || !(0.01..=360.).contains(&self.angle)
            || !finite(self.rotation)
            || !valid_scale(self.size)
            || !finite(self.step_rotation)
            || !valid_scale(self.step_scale)
        {
            return Err(
                "Invalid array layer: check count, position, rotation, scale, radius and arc"
                    .into(),
            );
        }
        let n = input
            .len()
            .checked_mul(self.count)
            .filter(|&n| n <= MAX_INSTANCES)
            .ok_or("Array layers and linked sources may expand to at most 256 copies")?;
        let mut result = Vec::with_capacity(n);
        let whole = Basis::step(self.rotation, self.size);
        for i in 0..self.count {
            let index = i as f32;
            let s = self.step_scale;
            let mut basis = Basis::step(
                self.step_rotation * index,
                Vec3(s.0.powi(i as i32), s.1.powi(i as i32), s.2.powi(i as i32)),
            );
            let offset = match self.kind {
                ArrayKind::Linear => self.spacing * index,
                ArrayKind::Grid => Vec3(
                    self.spacing.0 * (i % self.columns) as f32,
                    self.spacing.1 * (i / self.columns) as f32,
                    0.,
                ),
                ArrayKind::Radial => {
                    let denom = if self.angle == 360. {
                        self.count
                    } else {
                        self.count.saturating_sub(1).max(1)
                    };
                    let angle = (self.angle * index / denom as f32).to_radians();
                    let axis_vector = axis(self.radial_axis);
                    if self.rotate_copies {
                        basis = Basis(basis.0.map(|v| v.rotated(axis_vector, angle)));
                    }
                    axis((self.radial_axis + 1) % 3).rotated(axis_vector, angle) * self.radius
                }
            };
            basis = whole.compose(basis);
            basis.validate()?;
            let centre = destination + whole.vector(offset + self.step_offset * index);
            let identity = basis.0 == [axis(0), axis(1), axis(2)];
            for &item in input {
                result.push(if let Some(mut p) = item {
                    p.transform.origin = centre + basis.vector(p.transform.origin - anchor);
                    if !identity {
                        p.transform.prepend(
                            basis,
                            unit_scale(&self.size) && (i == 0 || unit_scale(&self.step_scale)),
                        );
                    }
                    p.transform.validate()?;
                    Some(p)
                } else {
                    None
                });
            }
        }
        Ok(result)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Array {
    pub piece: Piece,
    // Keep the original one-layer project representation readable.
    #[serde(flatten)]
    pub base: ArrayLayer,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ArrayLayer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_array: Option<u64>,
}
impl std::ops::Deref for Array {
    type Target = ArrayLayer;
    fn deref(&self) -> &ArrayLayer {
        &self.base
    }
}
impl std::ops::DerefMut for Array {
    fn deref_mut(&mut self) -> &mut ArrayLayer {
        &mut self.base
    }
}
impl Array {
    pub fn new(piece: Piece) -> Self {
        Self {
            piece,
            base: ArrayLayer::new(piece.pivot),
            layers: Vec::new(),
            source_array: None,
        }
    }
    pub fn pivot(&self) -> Vec3 {
        self.layers.iter().fold(self.center, |p, l| p + l.center)
    }
    fn expand(
        &self,
        input: &[Option<Placement>],
        anchor: Vec3,
    ) -> Result<Vec<Option<Placement>>, String> {
        if self.layers.len() >= MAX_ARRAY_LAYERS {
            return Err("An array may have at most eight layers".into());
        }
        let mut pivot = self.center;
        let mut slots = self.base.expand(input, anchor, pivot)?;
        for layer in &self.layers {
            let destination = pivot + layer.center;
            slots = layer.expand(&slots, pivot, destination)?;
            pivot = destination;
        }
        Ok(slots)
    }
    pub(super) fn unlinked_slots(&self) -> Result<Vec<Option<Placement>>, String> {
        if self.source_array.is_some() {
            return Err("Resolve the linked array in its project".into());
        }
        self.piece.validate()?;
        let p = Placement {
            piece: self.piece,
            transform: Transform {
                origin: self.piece.pivot,
                forward: axis(self.piece.forward),
                up: axis(self.piece.up),
                ..Default::default()
            },
        };
        self.expand(&[Some(p)], self.piece.pivot)
    }
    pub fn placements(&self) -> Result<Vec<Placement>, String> {
        Ok(self.unlinked_slots()?.into_iter().flatten().collect())
    }
}
pub(super) fn numbered(slots: Vec<Option<Placement>>) -> Vec<(usize, Placement)> {
    slots
        .into_iter()
        .enumerate()
        .filter_map(|(i, p)| p.map(|p| (i, p)))
        .collect()
}

pub struct ArrayResolver<'a> {
    library: &'a Library,
    cache: HashMap<u64, CachedSlots>,
}
impl<'a> ArrayResolver<'a> {
    pub fn new(library: &'a Library) -> Self {
        Self {
            library,
            cache: HashMap::new(),
        }
    }
    pub fn placements(
        &mut self,
        set: &Set,
        paths: &[SurfacePath],
    ) -> Result<Vec<(usize, Placement)>, String> {
        if let Layout::Array(a) = &set.layout {
            let (slots, _) = self.slots(set, a, &mut Vec::new())?;
            return Ok(slots
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| p.map(|p| (i, p)))
                .collect());
        }
        set.placements(paths)
    }
    fn slots(
        &mut self,
        set: &Set,
        array: &Array,
        chain: &mut Vec<u64>,
    ) -> Result<CachedSlots, String> {
        if chain.contains(&set.id) {
            return Err("A linked array cannot repeat itself, even indirectly".into());
        }
        if chain.len() >= MAX_LINK_DEPTH {
            return Err("Linked arrays are limited to 16 levels".into());
        }
        if let Some(cached) = self.cache.get(&set.id) {
            if chain.len() + cached.1 > MAX_LINK_DEPTH {
                return Err("Linked arrays are limited to 16 levels".into());
            }
            return Ok(cached.clone());
        }
        chain.push(set.id);
        let (slots, depth) = if let Some(id) = array.source_array {
            let source = self
                .library
                .sets
                .iter()
                .find(|s| s.id == id)
                .ok_or("Linked source array is missing")?;
            let Layout::Array(input) = &source.layout else {
                return Err("Choose an array as the linked source".into());
            };
            let (slots, depth) = self.slots(source, input, chain)?;
            (array.expand(&slots, input.pivot())?, depth + 1)
        } else {
            (array.unlinked_slots()?, 1)
        };
        chain.pop();
        let result = (Arc::new(set.adjust_slots(slots)?), depth);
        self.cache.insert(set.id, result.clone());
        Ok(result)
    }
}
impl Library {
    pub fn ensure_array_unreferenced(&self, id: u64) -> Result<(), String> {
        if self
            .sets
            .iter()
            .any(|s| matches!(&s.layout,Layout::Array(a) if a.source_array==Some(id)))
        {
            return Err(
                "Other arrays use this source. Bake or remove those linked arrays first".into(),
            );
        }
        Ok(())
    }
}
