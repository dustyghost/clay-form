//! Signed detail uses tagged weight channels, independent of masks and groups.
//! Existing subdivision/remesh weight transport and project/undo serialization
//! carry these channels alongside the mesh, including across topology changes.
use crate::sculpt::{Brush, Dab, GroupPurpose, Material, Mesh, VertexGroup};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DensityPaint {
    More,
    Less,
    Automatic,
}
impl DensityPaint {
    pub fn value(self) -> f32 {
        match self {
            Self::More => 1.,
            Self::Less => -1.,
            Self::Automatic => 0.,
        }
    }
}
impl Mesh {
    pub(crate) fn require_density_channels(&self) -> Result<(), String> {
        let missing = [GroupPurpose::RetopologyMore, GroupPurpose::RetopologyLess]
            .iter()
            .filter(|r| !self.vertex_groups.iter().any(|g| g.purpose == **r))
            .count();
        if self.vertex_groups.len() + missing > 64 {
            Err("Free two group slots before painting retopology detail".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn ensure_density_channels(&mut self) -> Result<(), String> {
        self.require_density_channels()?;
        for role in [GroupPurpose::RetopologyMore, GroupPurpose::RetopologyLess] {
            if !self.vertex_groups.iter().any(|g| g.purpose == role) {
                self.vertex_groups.push(VertexGroup {
                    purpose: role,
                    visible: true,
                    name: self.unique_group_name(if role == GroupPurpose::RetopologyMore {
                        "Retopology more detail"
                    } else {
                        "Retopology less detail"
                    }),
                    weights: vec![0.; self.vertices.len()],
                });
            }
        }
        Ok(())
    }
    pub(crate) fn retopology_detail(&self) -> Vec<f32> {
        let mut more = vec![0_f32; self.vertices.len()];
        let mut less = more.clone();
        for group in &self.vertex_groups {
            let values = match group.purpose {
                GroupPurpose::RetopologyMore => &mut more,
                GroupPurpose::RetopologyLess => &mut less,
                _ => continue,
            };
            for (v, &w) in values.iter_mut().zip(&group.weights) {
                *v = v.max(w);
            }
        }
        more.iter()
            .zip(less)
            .map(|(a, b)| (a - b).clamp(-1., 1.))
            .collect()
    }
    pub(crate) fn paint_retopology_density(
        &mut self,
        brush: &Brush,
        dabs: &[Dab],
        mode: DensityPaint,
    ) -> Result<bool, String> {
        let original = self.retopology_detail();
        let mut values = original.clone();
        let normals = self.normals();
        let editable = self.sculptable_vertices();
        let footprint = brush.prepare_weight();
        for (i, &p) in self.vertices.iter().enumerate() {
            if !editable[i] {
                continue;
            }
            let mut influence = 0_f32;
            for dab in dabs {
                let offset = p - dab.center;
                if offset.len() > dab.radius * 1.5 || normals[i].dot(dab.normal) < 0.1 {
                    continue;
                }
                influence = influence.max(
                    footprint.weight(
                        offset.dot(dab.right) / dab.radius,
                        offset.dot(dab.up) / dab.radius,
                    ) * brush.strength
                        * brush.dab_pressure(dab),
                );
            }
            if influence > 0. {
                values[i] = original[i] + (mode.value() - original[i]) * influence.clamp(0., 1.);
                if mode == DensityPaint::Automatic && values[i].abs() < 0.005 {
                    values[i] = 0.;
                }
            }
        }
        if original == values {
            return Ok(false);
        }
        self.ensure_density_channels()?;
        for group in &mut self.vertex_groups {
            let sign = match group.purpose {
                GroupPurpose::RetopologyMore => 1.,
                GroupPurpose::RetopologyLess => -1.,
                _ => continue,
            };
            for (i, &v) in values.iter().enumerate() {
                if v != original[i] {
                    group.weights[i] = (v * sign).max(0.);
                }
            }
        }
        Ok(true)
    }
    pub(crate) fn density_overlay(&self) -> Self {
        let mut mesh = self.clone();
        mesh.materials = self
            .retopology_detail()
            .into_iter()
            .map(|v| {
                let color = if v > 0. {
                    [0.95, 0.35, 0.12]
                } else {
                    [0.18, 0.55, 0.98]
                };
                let amount = v.abs();
                Material::from_srgb(color.map(|c| 0.65 * (1. - amount) + c * amount), 0.75, 0.)
            })
            .collect();
        mesh.mask.fill(0.);
        mesh
    }
}
