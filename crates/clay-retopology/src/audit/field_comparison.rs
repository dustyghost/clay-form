//! Offline, uniform, unconstrained field transplantation before integer correction.
use super::Snapshot;
use crate::{Options, Result, Stage, Surface, Vec3, field::Field, topology};
use std::io::{Read, Write};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FieldSnapshot {
    version: u32,
    pub(super) source: Surface,
    pub(super) field: Field,
}

impl FieldSnapshot {
    /// Recompute only fields on retained common fine input, without resampling.
    pub fn resolve_native(&mut self, progress: &dyn Fn(Stage)) -> Result<()> {
        self.validate()?;
        crate::field::resolve_fields(&mut self.field, progress)?;
        self.validate()
    }

    pub fn capture(source: Surface, target: usize, progress: &dyn Fn(Stage)) -> Result<Self> {
        if source.positions.len() > 150_000 || !(4..=150_000).contains(&target) {
            return Err("Field comparison exceeds the sample/quad budget".into());
        }
        topology::validate(&source)?;
        let options = Options {
            target_quads: target,
            preserve_sharp: false,
            max_vertices: 150_000,
            ..Options::default()
        };
        let field = crate::field::solve(&source, &options, &|| false, progress)?;
        let snapshot = Self {
            version: 1,
            source,
            field,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub(super) fn validate(&self) -> Result<()> {
        let f = &self.field;
        let n = f.surface.positions.len();
        if self.version != 1
            || n == 0
            || n > 150_000
            || [
                f.normals.len(),
                f.areas.len(),
                f.spacing.len(),
                f.constraints.len(),
                f.directions.len(),
                f.origins.len(),
            ]
            .iter()
            .any(|&len| len != n)
        {
            return Err("Invalid field comparison dimensions/version".into());
        }
        topology::validate(&self.source)?;
        topology::validate(&f.surface)?;
        let h = f.spacing[0];
        if !h.is_finite()
            || h <= 0.
            || f.spacing.iter().any(|&s| s != h)
            || f.constraints.iter().any(|c| c.weight != 0.)
            || f.areas.iter().any(|a| !a.is_finite() || *a <= 0.)
        {
            return Err("Field comparison needs uniform, unconstrained finite input".into());
        }
        for (&normal, &q) in f.normals.iter().zip(&f.directions) {
            if !normal.is_finite()
                || (normal.length() - 1.).abs() > 1e-6
                || !q.is_finite()
                || (q.length() - 1.).abs() > 1e-6
                || q.dot(normal).abs() > 1e-6
            {
                return Err("Invalid field comparison normal/direction".into());
            }
        }
        if f.origins.iter().any(|o| !o.is_finite()) {
            return Err("Invalid field comparison origin".into());
        }
        Ok(())
    }

    /// Mix-up guard for trusted local experiments, not a security signature.
    fn fingerprint(&self) -> String {
        let mut hash = 0xcbf29ce484222325u64;
        let mut add = |value: u64| {
            for byte in value.to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        };
        let f = &self.field;
        add(f.surface.positions.len() as u64);
        add(f.surface.triangles.len() as u64);
        for v in 0..f.surface.positions.len() {
            for value in f.surface.positions[v]
                .to_array()
                .into_iter()
                .chain(f.normals[v].to_array())
                .chain([f.areas[v], f.spacing[v]])
            {
                add(value.to_bits());
            }
        }
        for &v in f.surface.triangles.iter().flatten() {
            add(v as u64);
        }
        format!("{hash:016x}")
    }

    pub fn read(input: impl Read) -> Result<Self> {
        let result: Self = serde_json::from_reader(input).map_err(|e| e.to_string())?;
        result.validate()?;
        Ok(result)
    }

    pub fn write(&self, output: impl Write) -> Result<()> {
        serde_json::to_writer(output, self).map_err(|e| e.to_string())
    }

    pub fn export_reference(&self, mut output: impl Write) -> Result<()> {
        self.validate()?;
        let f = &self.field;
        let neighbours = crate::field::ordered_neighbours(&f.surface, &|| false)?;
        let write = || -> std::io::Result<()> {
            writeln!(
                output,
                "CLAYFIELD1 {} {} {} {:.17e}",
                self.fingerprint(),
                f.surface.positions.len(),
                f.surface.triangles.len(),
                f.spacing[0]
            )?;
            for v in 0..f.surface.positions.len() {
                for value in f.surface.positions[v]
                    .to_array()
                    .into_iter()
                    .chain(f.normals[v].to_array())
                {
                    write!(output, "{value:.17e} ")?;
                }
                writeln!(output, "{:.17e}", f.areas[v])?;
            }
            for [a, b, c] in &f.surface.triangles {
                writeln!(output, "{a} {b} {c}")?;
            }
            for row in neighbours {
                write!(output, "{}", row.len())?;
                for v in row {
                    write!(output, " {v}")?;
                }
                writeln!(output)?;
            }
            Ok(())
        };
        write().map_err(|e| e.to_string())
    }

    pub fn import_reference(&mut self, mut input: impl Read) -> Result<()> {
        self.validate()?;
        let mut text = String::new();
        input.read_to_string(&mut text).map_err(|e| e.to_string())?;
        let mut words = text.split_whitespace();
        let n = self.field.surface.positions.len();
        if words.next() != Some("CLAYFIELD1")
            || words.next() != Some(self.fingerprint().as_str())
            || words.next().and_then(|w| w.parse::<usize>().ok()) != Some(n)
        {
            return Err("Reference field belongs to different sampled input".into());
        }
        let mut directions = Vec::with_capacity(n);
        let mut origins = Vec::with_capacity(n);
        for v in 0..n {
            let mut values = [0.; 6];
            for x in &mut values {
                *x = words
                    .next()
                    .ok_or("Truncated reference field")?
                    .parse::<f64>()
                    .map_err(|_| "Invalid reference scalar")?;
            }
            let q = Vec3::new(values[0], values[1], values[2]);
            let o = Vec3::new(values[3], values[4], values[5]);
            let normal = self.field.normals[v];
            if !q.is_finite()
                || !o.is_finite()
                || (q.length() - 1.).abs() > 1e-6
                || q.dot(normal).abs() > 1e-6
                || (o - self.field.surface.positions[v]).dot(normal).abs()
                    > 1e-6 * self.field.spacing[v]
            {
                return Err("Reference field is not finite/unit/tangent".into());
            }
            directions.push(q);
            origins.push(o);
        }
        if words.next().is_some() {
            return Err("Trailing reference field data".into());
        }
        self.field.directions = directions;
        self.field.origins = origins;
        Ok(())
    }

    /// Initial grid on the saved fine field, before correction or subdivision.
    pub fn export_initial_grid(&self, output: impl Write) -> Result<()> {
        self.validate()?;
        super::orientation_repair::write_initial_grid_input(&self.field, output)
    }

    pub fn integerize(self, progress: &dyn Fn(Stage)) -> Result<Snapshot> {
        self.validate()?;
        let (field, lattice, diagnostics) =
            crate::prepare_field(&self.source, self.field, 150_000, &|| false, progress)?;
        Ok(Snapshot {
            version: 1,
            repaired: false,
            source: self.source,
            field,
            lattice,
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Surface {
        Surface {
            positions: vec![Vec3::X, Vec3::Y, -Vec3::X, -Vec3::Y, Vec3::Z, -Vec3::Z],
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
        }
    }

    fn identity(snapshot: &FieldSnapshot) -> String {
        let f = &snapshot.field;
        let mut text = format!(
            "CLAYFIELD1 {} {}\n",
            snapshot.fingerprint(),
            f.origins.len()
        );
        for (&q, &o) in f.directions.iter().zip(&f.origins) {
            for x in q.to_array().into_iter().chain(o.to_array()) {
                text.push_str(&format!("{x:.17e} "));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn field_snapshot_roundtrip_preserves_every_float_bit() {
        let mut mesh = source();
        mesh.positions[0] *= 1.234_567_891_234_568;
        mesh.positions[1] *= 0.8765432187654321;
        let snapshot = FieldSnapshot::capture(mesh, 19, &|_| {}).unwrap();
        let mut saved = Vec::new();
        snapshot.write(&mut saved).unwrap();
        let reloaded = FieldSnapshot::read(saved.as_slice()).unwrap();
        let mut resaved = Vec::new();
        reloaded.write(&mut resaved).unwrap();
        assert_eq!(snapshot.fingerprint(), reloaded.fingerprint());
        assert_eq!(saved, resaved);
    }

    #[test]
    fn identity_field_import_preserves_native_integer_pipeline() {
        let mut capture = FieldSnapshot::capture(source(), 16, &|_| {}).unwrap();
        let original = serde_json::to_string(&capture).unwrap();
        capture
            .import_reference(identity(&capture).as_bytes())
            .unwrap();
        assert_eq!(serde_json::to_string(&capture).unwrap(), original);
        let replay = serde_json::to_value(capture.integerize(&|_| {}).unwrap()).unwrap();
        let options = Options {
            target_quads: 16,
            preserve_sharp: false,
            max_vertices: 150_000,
            ..Options::default()
        };
        let direct = Snapshot::capture_raw(source(), &options, &|| false, &|_| {}).unwrap();
        assert_eq!(replay, serde_json::to_value(direct).unwrap());
    }

    #[test]
    fn field_import_rejects_mismatch_and_late_errors_without_mutation() {
        let mut capture = FieldSnapshot::capture(source(), 16, &|_| {}).unwrap();
        let original = serde_json::to_string(&capture).unwrap();
        let valid = identity(&capture);
        for bad in [
            valid.replacen(&capture.fingerprint(), "wrong-input", 1),
            format!("{valid} trailing"),
            valid[..valid.len() / 2].to_string(),
        ] {
            assert!(capture.import_reference(bad.as_bytes()).is_err());
            assert_eq!(serde_json::to_string(&capture).unwrap(), original);
        }
        let mut parts = valid.split_whitespace().collect::<Vec<_>>();
        let last = parts.len() - 1;
        parts[last] = "NaN";
        assert!(
            capture
                .import_reference(parts.join(" ").as_bytes())
                .is_err()
        );
        assert_eq!(serde_json::to_string(&capture).unwrap(), original);
    }
}
