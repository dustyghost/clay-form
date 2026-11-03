//! Attached arrays retain their source mesh and replace only its evaluated display.
use super::*;
use crate::sculpt::{Vec3, instances::*};
#[cfg(test)]
mod tests;

impl Document {
    pub(crate) fn array_bounds(&self, id: u64) -> Option<[Vec3; 2]> {
        let mut cache = std::collections::HashMap::new();
        let mut lo = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut hi = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        let mut found = false;
        for (_, _, p) in self
            .instance_placements()
            .ok()?
            .iter()
            .filter(|(set, _, _)| *set == id)
        {
            let b = cache.entry(p.piece.source).or_insert_with(|| {
                let mesh = self.instance_source(p.piece.source).unwrap();
                let mut lo = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
                let mut hi = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
                for v in &mesh.vertices {
                    lo = Vec3(lo.0.min(v.0), lo.1.min(v.1), lo.2.min(v.2));
                    hi = Vec3(hi.0.max(v.0), hi.1.max(v.1), hi.2.max(v.2));
                }
                [lo, hi]
            });
            for x in [b[0].0, b[1].0] {
                for y in [b[0].1, b[1].1] {
                    for z in [b[0].2, b[1].2] {
                        let v = p.point(Vec3(x, y, z));
                        lo = Vec3(lo.0.min(v.0), lo.1.min(v.1), lo.2.min(v.2));
                        hi = Vec3(hi.0.max(v.0), hi.1.max(v.1), hi.2.max(v.2));
                        found = true;
                    }
                }
            }
        }
        found.then_some([lo, hi])
    }
    pub(crate) fn display_owner_model(&self, owner: usize) -> Option<usize> {
        if owner < self.model_count() {
            return Some(owner);
        }
        let copies = self.instance_placements().ok()?;
        self.array_owner(copies.get(owner - self.model_count())?.0)
    }
    pub(crate) fn array_layer_extent(&self, set: &Set, layer: usize) -> Result<Vec3, String> {
        ArrayResolver::new(self.instances())
            .with_sources(|s| self.instance_source(s))
            .layer_extent(set, layer)
    }
    pub fn attached_array(&self, index: usize) -> Option<&Set> {
        let id = self.instance_model_id(index);
        self.instances()
            .sets
            .iter()
            .find(|s| matches!(&s.layout, Layout::Array(a) if a.owner == Some(id)))
    }
    pub(crate) fn array_owner(&self, set: u64) -> Option<usize> {
        let a = self
            .instances()
            .sets
            .iter()
            .find(|s| s.id == set)
            .and_then(|s| match &s.layout {
                Layout::Array(a) => Some(a),
                _ => None,
            })?;
        let owner = a.owner?;
        (0..self.model_count()).find(|&i| self.instance_model_id(i) == owner)
    }
    pub(crate) fn editing_array_source(&self) -> bool {
        self.attached_array(self.active_model_index())
            .is_some_and(|s| self.array_source_edit == Some(s.id))
    }
    pub(crate) fn source_is_displayed(&self, index: usize) -> bool {
        self.attached_array(index).is_none_or(|s| {
            !s.visible
                || (index == self.active_model_index() && self.array_source_edit == Some(s.id))
        })
    }
    pub(crate) fn set_array_source_mode(&mut self, source: bool) -> Result<(), String> {
        if self.instance_preview_active() {
            self.apply_instance_preview()?;
        }
        self.end_edit();
        self.end_stroke();
        self.selected_instance = None;
        let next = source
            .then(|| self.attached_array(self.active_model_index()).map(|s| s.id))
            .flatten();
        if next != self.array_source_edit {
            self.array_source_edit = next;
            self.instances_display_changed();
        }
        Ok(())
    }
    pub(crate) fn create_submodel_array(&mut self) -> Result<u64, String> {
        if let Some(s) = self.attached_array(self.active_model_index()) {
            return Ok(s.id);
        }
        let piece = self.instance_piece(self.active_model_index())?;
        let Source::Model(owner) = piece.source else {
            unreachable!()
        };
        let mut array = Array::new(piece);
        array.owner = Some(owner);
        array.count = 2;
        array.spacing = Vec3(1., 0., 0.);
        array.spacing_mode = ArraySpacing::Relative;
        let id = self.next_instance_set_id()?;
        let set = Set {
            id,
            name: format!(
                "{} array",
                self.model(self.active_model_index()).unwrap().name
            ),
            visible: true,
            layout: Layout::Array(array),
            adjustments: Vec::new(),
        };
        if let Err(e) = self
            .preview_instance_set(set)
            .and_then(|()| self.apply_instance_preview())
        {
            self.cancel_instance_preview();
            return Err(e);
        }
        self.array_source_edit = None;
        Ok(id)
    }
    pub(crate) fn array_copy_is_source_duplicate(&self, set: u64, p: Placement) -> bool {
        let Some(index) = self.array_owner(set) else {
            return false;
        };
        if self.array_source_edit != Some(set) || index != self.active_model_index() {
            return false;
        }
        [Vec3::default(), axis(0), axis(1), axis(2)]
            .into_iter()
            .all(|v| (p.point(v) - v).len() < 1e-6)
    }
    pub(crate) fn array_world_pivot(&self) -> Option<Vec3> {
        let set = self.attached_array(self.active_model_index())?;
        let Layout::Array(a) = &set.layout else {
            return None;
        };
        Some(a.output.point(a.pivot()))
    }
    pub(crate) fn transform_attached_array(
        &mut self,
        t: crate::sculpt::Transform,
    ) -> Result<bool, String> {
        let Some(mut set) = self.attached_array(self.active_model_index()).cloned() else {
            return Ok(false);
        };
        let Layout::Array(a) = &mut set.layout else {
            unreachable!()
        };
        a.output.origin = t.point(a.output.origin);
        a.output.basis = Some(Basis(
            [axis(0), axis(1), axis(2)].map(|v| t.vector(a.output.vector(v) * a.output.scale)),
        ));
        a.output.scale = 1.;
        self.preview_instance_set(set)?;
        Ok(true)
    }
}
