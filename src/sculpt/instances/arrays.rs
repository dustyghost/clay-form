//! Array layers operate on placements, never meshes. A linked array resolves the
//! source recipe once, then composes its leaf transforms at each repetition.
use super::*;
use std::collections::HashMap;
#[cfg(test)]
mod tests;
type CachedSlots = (Arc<Vec<Option<Placement>>>, usize);
type Sources<'a> = Box<dyn Fn(Source) -> Option<&'a Mesh> + 'a>;

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
    pub(crate) fn inverse_vector(self, v: Vec3) -> Vec3 {
        let [x, y, z] = self.0;
        let det = x.dot(y.cross(z));
        Vec3(v.dot(y.cross(z)), v.dot(z.cross(x)), v.dot(x.cross(y))) * (1. / det)
    }
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
    pub(crate) fn step(rotation: Vec3, scale: Vec3) -> Self {
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
    pub(crate) fn linear(&self) -> Basis {
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArraySpacing {
    #[default]
    Distance,
    Relative,
    Span,
}
impl ArraySpacing {
    fn is_distance(&self) -> bool {
        *self == Self::Distance
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArrayLayer {
    pub kind: ArrayKind,
    pub count: usize,
    pub columns: usize,
    pub spacing: Vec3,
    #[serde(default, skip_serializing_if = "ArraySpacing::is_distance")]
    pub spacing_mode: ArraySpacing,
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
            spacing_mode: ArraySpacing::Distance,
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
        extent: Vec3,
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
            let spacing = match self.spacing_mode {
                ArraySpacing::Distance => self.spacing,
                ArraySpacing::Relative => Vec3(
                    self.spacing.0 * extent.0,
                    self.spacing.1 * extent.1,
                    self.spacing.2 * extent.2,
                ),
                ArraySpacing::Span => {
                    self.spacing * (1. / self.count.saturating_sub(1).max(1) as f32)
                }
            };
            let offset = match self.kind {
                ArrayKind::Linear => spacing * index,
                ArrayKind::Grid => Vec3(
                    spacing.0 * (i % self.columns) as f32,
                    spacing.1 * (i / self.columns) as f32,
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
    /// Absent for legacy standalone sets. The owner's base surface is replaced
    /// by this evaluated result in the viewport, while its mesh remains editable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<u64>,
    #[serde(default, skip_serializing_if = "Transform::is_identity")]
    pub output: Transform,
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
    pub fn layer(&self, index: usize) -> Option<&ArrayLayer> {
        if index == 0 {
            Some(&self.base)
        } else {
            self.layers.get(index - 1)
        }
    }
    pub fn layer_mut(&mut self, index: usize) -> Option<&mut ArrayLayer> {
        if index == 0 {
            Some(&mut self.base)
        } else {
            self.layers.get_mut(index - 1)
        }
    }
    pub fn new(piece: Piece) -> Self {
        Self {
            piece,
            base: ArrayLayer::new(piece.pivot),
            layers: Vec::new(),
            source_array: None,
            owner: None,
            output: Transform::default(),
        }
    }
    pub fn pivot(&self) -> Vec3 {
        self.layers.iter().fold(self.center, |p, l| p + l.center)
    }
    pub fn output_pivot(&self) -> Vec3 {
        self.output.point(self.pivot())
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
        let mut slots = self.base.expand(input, anchor, pivot, ones())?;
        for layer in &self.layers {
            let destination = pivot + layer.center;
            slots = layer.expand(&slots, pivot, destination, ones())?;
            pivot = destination;
        }
        self.transform_output(&mut slots)?;
        Ok(slots)
    }
    fn transform_output(&self, slots: &mut [Option<Placement>]) -> Result<(), String> {
        self.output.validate()?;
        if self.output.is_identity() {
            return Ok(());
        }
        for p in slots.iter_mut().flatten() {
            p.transform.origin = self.output.point(p.transform.origin);
            p.transform
                .prepend(self.output.linear().scaled(self.output.scale), false);
            p.transform.validate()?;
        }
        Ok(())
    }
    pub(super) fn unlinked_slots(&self) -> Result<Vec<Option<Placement>>, String> {
        if self.base.spacing_mode == ArraySpacing::Relative
            || self
                .layers
                .iter()
                .any(|l| l.spacing_mode == ArraySpacing::Relative)
        {
            return Err("Resolve relative array spacing with its source geometry".into());
        }
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
    sources: Option<Sources<'a>>,
    bounds: HashMap<Source, [Vec3; 2]>,
}
impl<'a> ArrayResolver<'a> {
    pub fn layer_extent(&mut self, set: &Set, layer: usize) -> Result<Vec3, String> {
        let Layout::Array(a) = &set.layout else {
            return Err("Not an array".into());
        };
        let mut slots = if let Some(id) = a.source_array {
            let source = self
                .library
                .sets
                .iter()
                .find(|s| s.id == id)
                .ok_or("Source array missing")?;
            self.placements(source, &[])?
                .into_iter()
                .map(|(_, p)| Some(p))
                .collect::<Vec<_>>()
        } else {
            vec![Some(Placement {
                piece: a.piece,
                transform: Transform {
                    origin: a.piece.pivot,
                    forward: axis(a.piece.forward),
                    up: axis(a.piece.up),
                    ..Default::default()
                },
            })]
        };
        let mut pivot = a.center;
        let mut anchor = a.piece.pivot;
        for i in 0..layer {
            let stage = a.layer(i).ok_or("Layer missing")?;
            if i > 0 {
                pivot = pivot + stage.center;
            }
            let extent = if stage.spacing_mode == ArraySpacing::Relative {
                self.extent(&slots)?
            } else {
                ones()
            };
            slots = stage.expand(&slots, anchor, pivot, extent)?;
            anchor = pivot;
        }
        self.extent(&slots)
    }
    pub fn new(library: &'a Library) -> Self {
        Self {
            library,
            cache: HashMap::new(),
            sources: None,
            bounds: HashMap::new(),
        }
    }
    pub fn with_sources(mut self, source: impl Fn(Source) -> Option<&'a Mesh> + 'a) -> Self {
        self.sources = Some(Box::new(source));
        self
    }
    fn extent(&mut self, slots: &[Option<Placement>]) -> Result<Vec3, String> {
        let mut min = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for p in slots.iter().flatten() {
            let bounds = if let Some(b) = self.bounds.get(&p.piece.source) {
                *b
            } else {
                let mesh = self
                    .sources
                    .as_ref()
                    .and_then(|f| f(p.piece.source))
                    .ok_or("Relative spacing needs source geometry")?;
                let mut lo = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
                let mut hi = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
                for v in &mesh.vertices {
                    lo = Vec3(lo.0.min(v.0), lo.1.min(v.1), lo.2.min(v.2));
                    hi = Vec3(hi.0.max(v.0), hi.1.max(v.1), hi.2.max(v.2));
                }
                if mesh.vertices.is_empty() {
                    lo = Vec3::default();
                    hi = lo;
                }
                self.bounds.insert(p.piece.source, [lo, hi]);
                [lo, hi]
            };
            for x in [bounds[0].0, bounds[1].0] {
                for y in [bounds[0].1, bounds[1].1] {
                    for z in [bounds[0].2, bounds[1].2] {
                        let v = p.point(Vec3(x, y, z));
                        min = Vec3(min.0.min(v.0), min.1.min(v.1), min.2.min(v.2));
                        max = Vec3(max.0.max(v.0), max.1.max(v.1), max.2.max(v.2));
                    }
                }
            }
        }
        Ok(if slots.iter().any(Option::is_some) {
            max - min
        } else {
            ones()
        })
    }
    fn expand_array(
        &mut self,
        array: &Array,
        input: &[Option<Placement>],
        anchor: Vec3,
    ) -> Result<Vec<Option<Placement>>, String> {
        let mut pivot = array.center;
        let extent = if array.spacing_mode == ArraySpacing::Relative {
            self.extent(input)?
        } else {
            ones()
        };
        let mut slots = array.base.expand(input, anchor, pivot, extent)?;
        for layer in &array.layers {
            let destination = pivot + layer.center;
            let extent = if layer.spacing_mode == ArraySpacing::Relative {
                self.extent(&slots)?
            } else {
                ones()
            };
            slots = layer.expand(&slots, pivot, destination, extent)?;
            pivot = destination;
        }
        array.transform_output(&mut slots)?;
        Ok(slots)
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
            (
                self.expand_array(array, &slots, input.output_pivot())?,
                depth + 1,
            )
        } else {
            array.piece.validate()?;
            let p = Placement {
                piece: array.piece,
                transform: Transform {
                    origin: array.piece.pivot,
                    forward: axis(array.piece.forward),
                    up: axis(array.piece.up),
                    ..Default::default()
                },
            };
            (self.expand_array(array, &[Some(p)], array.piece.pivot)?, 1)
        };
        chain.pop();
        let result = (Arc::new(set.adjust_slots(slots)?), depth);
        self.cache.insert(set.id, result.clone());
        Ok(result)
    }
}
impl Library {
    pub fn array_is_repeated(&self, id: u64) -> bool {
        self.sets
            .iter()
            .any(|s| matches!(&s.layout, Layout::Array(a) if a.source_array == Some(id)))
    }
    pub fn ensure_array_unreferenced(&self, id: u64) -> Result<(), String> {
        if self.array_is_repeated(id) {
            return Err(
                "Other arrays use this source. Bake or remove those linked arrays first".into(),
            );
        }
        Ok(())
    }
}
