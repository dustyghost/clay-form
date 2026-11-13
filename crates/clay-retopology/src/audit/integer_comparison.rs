//! Transplant a corrected fine integer grid; keep native subdivision and repair.
use super::{FieldSnapshot, Snapshot};
use crate::{Diagnostics, Result, Stage, integer::Lattice};
use glam::IVec2;
use std::io::{Read, Write};

#[derive(serde::Serialize, serde::Deserialize)]
struct Reference {
    input_fingerprint: String,
    edges: Vec<[i64; 4]>,
    faces: Vec<[usize; 6]>,
}

struct Fingerprint(u64);
impl Write for Fingerprint {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl FieldSnapshot {
    // Hash the complete export, including Q/O and initial edge/face order.
    // FNV is a trusted-local mix-up guard, not a security signature.
    fn integer_fingerprint(&self) -> Result<String> {
        let mut hash = Fingerprint(0xcbf29ce484222325);
        self.export_initial_grid(&mut hash)?;
        Ok(format!("{:016x}", hash.0))
    }

    /// Native correction without subdivision, for stage-matched comparison.
    pub fn export_corrected_grid(&self, output: impl Write) -> Result<()> {
        self.validate()?;
        let mut lattice = Lattice::new(&self.field, &|| false)?;
        lattice.correct_field(&self.field, &|| false)?;
        self.export_lattice(lattice, output)
    }

    /// Rust port of the original default mapper, before subdivision/repair.
    pub fn export_native_mapping(&self, output: impl Write) -> Result<()> {
        self.validate()?;
        let lattice = crate::integer::solve_reference(&self.field, &|| false)?;
        self.export_lattice(lattice, output)
    }

    fn export_lattice(&self, lattice: Lattice, output: impl Write) -> Result<()> {
        let reference = Reference {
            input_fingerprint: self.integer_fingerprint()?,
            edges: lattice
                .edges
                .iter()
                .map(|e| {
                    [
                        e.ends[0] as i64,
                        e.ends[1] as i64,
                        i64::from(e.d.x),
                        i64::from(e.d.y),
                    ]
                })
                .collect(),
            faces: lattice
                .faces
                .iter()
                .map(|f| {
                    [
                        f.edges[0],
                        f.edges[1],
                        f.edges[2],
                        f.rotations[0] as usize,
                        f.rotations[1] as usize,
                        f.rotations[2] as usize,
                    ]
                })
                .collect(),
        };
        serde_json::to_writer(output, &reference).map_err(|e| e.to_string())
    }

    pub fn integerize_reference(
        mut self,
        input: impl Read,
        progress: &dyn Fn(Stage),
    ) -> Result<Snapshot> {
        self.validate()?;
        let reference: Reference = serde_json::from_reader(input).map_err(|e| e.to_string())?;
        if reference.input_fingerprint != self.integer_fingerprint()? {
            return Err("Reference integer grid belongs to different fine fields".into());
        }
        let mut lattice = Lattice::new(&self.field, &|| false)?;
        let mut diagnostics = Diagnostics {
            input_vertices: self.source.positions.len(),
            position_defects_before: lattice.defects(),
            flipped_before: lattice.flipped(),
            ..Diagnostics::default()
        };
        if reference.edges.len() != lattice.edges.len()
            || reference.faces.len() != lattice.faces.len()
        {
            return Err("Reference integer grid changed dimensions".into());
        }
        for (edge, values) in lattice.edges.iter_mut().zip(reference.edges) {
            if values[..2] != [edge.ends[0] as i64, edge.ends[1] as i64]
                || values[2..].iter().any(|v| !(-32..=32).contains(v))
                || edge.fixed_axis.is_some_and(|axis| values[axis + 2] != 0)
            {
                return Err(
                    "Reference integer edge changed endpoints or violated bounds/locks".into(),
                );
            }
            edge.d = IVec2::new(values[2] as i32, values[3] as i32);
        }
        for (face, values) in lattice.faces.iter_mut().zip(reference.faces) {
            if values[..3] != face.edges || values[3..].iter().any(|&r| r > 3) {
                return Err("Invalid reference integer face".into());
            }
            // A face may choose any quarter-turn gauge; its three relative
            // edge transports must still agree with the input direction field.
            let delta = (values[3] as i32 - face.rotations[0]).rem_euclid(4);
            for k in 0..3 {
                if (values[k + 3] as i32 - face.rotations[k]).rem_euclid(4) != delta {
                    return Err("Reference face changed relative edge transport".into());
                }
                face.rotations[k] = values[k + 3] as i32;
            }
        }
        if lattice.defects() != 0 {
            return Err("Reference integer correction did not close every cell".into());
        }
        progress(Stage::Subdivision {
            long_edges: lattice.long_edges(),
            fixed_variables: 0,
        });
        lattice.subdivide(&mut self.field, 150_000, &|| false)?;
        diagnostics.samples = self.field.surface.positions.len();
        diagnostics.position_defects_after = lattice.defects();
        progress(Stage::Inversions {
            flipped: lattice.flipped(),
        });
        Ok(Snapshot {
            version: 1,
            repaired: false,
            source: self.source,
            field: self.field,
            lattice,
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Surface, Vec3};

    fn fields() -> FieldSnapshot {
        let surface = Surface {
            positions: vec![
                Vec3::X * 1.13,
                Vec3::Y * 0.91,
                -Vec3::X,
                -Vec3::Y,
                Vec3::Z,
                -Vec3::Z,
            ],
            triangles: vec![
                [0, 1, 4],
                [1, 2, 4],
                [2, 3, 4],
                [3, 0, 4],
                [1, 0, 5],
                [2, 1, 5],
                [3, 2, 5],
                [0, 3, 5],
            ],
        };
        FieldSnapshot::capture(surface, 19, &|_| {}).unwrap()
    }

    #[test]
    fn corrected_grid_transplant_preserves_native_subdivision() {
        let fields = fields();
        let mut reference = Vec::new();
        fields.export_corrected_grid(&mut reference).unwrap();
        let mut saved = Vec::new();
        fields.write(&mut saved).unwrap();
        let direct = fields.integerize(&|_| {}).unwrap();
        let imported = FieldSnapshot::read(saved.as_slice())
            .unwrap()
            .integerize_reference(reference.as_slice(), &|_| {})
            .unwrap();
        assert_eq!(
            serde_json::to_value(direct.lattice).unwrap(),
            serde_json::to_value(imported.lattice).unwrap()
        );
        assert_eq!(
            serde_json::to_value(direct.field).unwrap(),
            serde_json::to_value(imported.field).unwrap()
        );
    }

    #[test]
    fn integer_import_rejects_wrong_fields_connectivity_and_transport() {
        let fields = fields();
        let mut saved = Vec::new();
        fields.write(&mut saved).unwrap();
        let mut reference = Vec::new();
        fields.export_corrected_grid(&mut reference).unwrap();
        let good: serde_json::Value = serde_json::from_slice(&reference).unwrap();
        for variant in 0..5 {
            let mut bad = good.clone();
            match variant {
                0 => bad["input_fingerprint"] = "wrong-fields".into(),
                1 => bad["edges"][0][0] = 999.into(),
                2 => bad["faces"][0][3] = ((bad["faces"][0][3].as_u64().unwrap() + 1) % 4).into(),
                3 => bad["edges"][0][2] = 33.into(),
                _ => bad["edges"][0][2] = (bad["edges"][0][2].as_i64().unwrap() + 1).into(),
            }
            let text = serde_json::to_vec(&bad).unwrap();
            assert!(
                FieldSnapshot::read(saved.as_slice())
                    .unwrap()
                    .integerize_reference(text.as_slice(), &|_| {})
                    .is_err()
            );
        }
    }
}
