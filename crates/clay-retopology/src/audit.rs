//! Offline stage comparisons, enabled only by the `audit` feature.
//! Snapshots are developer fixtures produced by this exact crate revision, not
//! an interchange format or an app document format. Do not load untrusted files.
use crate::{
    Diagnostics, Layout, Options, Result, Stage, Surface, Vec3, field::Field, integer::Lattice,
};
use std::io::{Read, Write};
mod field_comparison;
mod integer_comparison;
mod orientation_repair;
mod patch_rebuild;
mod repair_patch;
pub use crate::resample::{Prepared as PreparedSurface, prepare as prepare_surface};
pub use field_comparison::FieldSnapshot;
mod repair_trace;
pub use repair_patch::RepairPatch;
pub fn solve_dimacs(
    reader: impl std::io::BufRead,
    millis: u64,
    explain_units: bool,
) -> Result<serde_json::Value> {
    crate::sat::solve_dimacs(reader, millis, explain_units)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    version: u32,
    #[serde(default = "already_repaired")]
    repaired: bool,
    source: Surface,
    field: Field,
    lattice: Lattice,
    diagnostics: Diagnostics,
}
fn already_repaired() -> bool {
    true
}
impl Snapshot {
    pub fn select_cycles(&self, cancel: &dyn Fn() -> bool) -> Result<crate::cycles::Search> {
        if self.repaired {
            return Err("Use an unrepaired snapshot for automatic cycle selection".into());
        }
        crate::cycles::select(&self.field, &self.lattice, cancel)
    }

    pub fn constrain_cycles_raw(
        &mut self,
        cycles: &[Vec<usize>],
        progress: &dyn Fn(Stage),
    ) -> Result<()> {
        if self.repaired {
            return Err("Use an unrepaired snapshot for cycle constraints".into());
        }
        self.lattice.constrain_cycles(&self.field, cycles)?;
        progress(Stage::Regularity {
            defects: self.lattice.defects(),
        });
        self.diagnostics.fixed_variables = self.lattice.correct(&|| false)?;
        self.lattice
            .subdivide(&mut self.field, 150_000, &|| false)?;
        self.diagnostics.samples = self.field.surface.positions.len();
        self.diagnostics.position_defects_after = self.lattice.defects();
        progress(Stage::Inversions {
            flipped: self.lattice.flipped(),
        });
        Ok(())
    }

    /// Unrepaired snapshot for the direction-only guide experiment. The normal
    /// production prepare path and this path share all validation/integer work.
    pub fn capture_direction_guides_raw(
        source: Surface,
        options: &Options,
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(Stage),
    ) -> Result<Self> {
        let (field, lattice, diagnostics) = crate::prepare_using(
            &source,
            options,
            cancel,
            progress,
            crate::field::solve_direction_guides,
        )?;
        Ok(Self {
            version: 1,
            repaired: false,
            source,
            field,
            lattice,
            diagnostics,
        })
    }
    pub fn capture(
        source: Surface,
        options: &Options,
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(Stage),
    ) -> Result<Self> {
        let (field, mut lattice, mut diagnostics) =
            crate::prepare(&source, options, cancel, progress)?;
        crate::repair(&mut lattice, &mut diagnostics, cancel, progress)?;
        Ok(Self {
            version: 1,
            repaired: true,
            source,
            field,
            lattice,
            diagnostics,
        })
    }
    pub fn capture_raw(
        source: Surface,
        options: &Options,
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(Stage),
    ) -> Result<Self> {
        let (field, lattice, diagnostics) = crate::prepare(&source, options, cancel, progress)?;
        Ok(Self {
            version: 1,
            repaired: false,
            source,
            field,
            lattice,
            diagnostics,
        })
    }
    pub fn repair(&mut self, progress: &dyn Fn(Stage)) -> Result<()> {
        if !self.repaired {
            crate::repair(
                &mut self.lattice,
                &mut self.diagnostics,
                &|| false,
                progress,
            )?;
            self.repaired = true;
        }
        Ok(())
    }
    /// Complete upstream extraction input, with the same fitted origins.
    pub fn write_extraction_input(&mut self, mut writer: impl Write) -> Result<()> {
        if !self.repaired {
            return Err("Repair snapshot before extracting".into());
        }
        self.lattice.fit(&mut self.field, &|| false)?;
        let f = &self.field;
        let l = &self.lattice;
        let mut write = || -> std::io::Result<()> {
            writeln!(
                writer,
                "{} {} {}",
                f.origins.len(),
                l.faces.len(),
                l.edges.len()
            )?;
            for i in 0..f.origins.len() {
                for p in [
                    f.surface.positions[i],
                    f.origins[i],
                    f.normals[i],
                    f.directions[i],
                ] {
                    write!(writer, "{:.17e} {:.17e} {:.17e} ", p.x, p.y, p.z)?;
                }
                writeln!(writer)?;
            }
            for (t, face) in f.surface.triangles.iter().zip(&l.faces) {
                for v in t {
                    write!(writer, "{v} ")?;
                }
                for e in face.edges {
                    write!(writer, "{e} ")?;
                }
                for r in face.rotations {
                    write!(writer, "{} ", r.rem_euclid(4))?;
                }
                writeln!(writer)?;
            }
            for e in &l.edges {
                writeln!(
                    writer,
                    "{} {} {} {} {} {}",
                    e.ends[0],
                    e.ends[1],
                    e.d.x,
                    e.d.y,
                    u8::from(e.fixed_axis != Some(0)),
                    u8::from(e.fixed_axis != Some(1))
                )?;
            }
            Ok(())
        };
        write().map_err(|e| e.to_string())
    }
    pub fn reconstruct_reference(
        mut self,
        mut input: impl Read,
        progress: &dyn Fn(Stage),
    ) -> Result<Layout> {
        if !self.repaired {
            return Err("Repair snapshot before extracting".into());
        }
        let mut text = String::new();
        input.read_to_string(&mut text).map_err(|e| e.to_string())?;
        let mut words = text.split_whitespace();
        fn read<T: std::str::FromStr>(words: &mut std::str::SplitWhitespace<'_>) -> Result<T> {
            words
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or("Invalid reference extraction".into())
        }
        let nv: usize = read(&mut words)?;
        let nf: usize = read(&mut words)?;
        let mut mesh = crate::integer::Extraction {
            positions: Vec::new(),
            quads: Vec::new(),
            samples: Vec::new(),
        };
        for _ in 0..nv {
            mesh.positions.push(Vec3::new(
                read(&mut words)?,
                read(&mut words)?,
                read(&mut words)?,
            ));
            let n: usize = read(&mut words)?;
            let samples: Vec<usize> = (0..n).map(|_| read(&mut words)).collect::<Result<_>>()?;
            if samples.iter().any(|&v| v >= self.field.origins.len()) {
                return Err("Invalid sample index".into());
            }
            mesh.samples.push(samples);
        }
        for _ in 0..nf {
            let q = [
                read(&mut words)?,
                read(&mut words)?,
                read(&mut words)?,
                read(&mut words)?,
            ];
            if q.iter().any(|&v| v >= nv) {
                return Err("Invalid reference vertex".into());
            }
            mesh.quads.push(q);
        }
        if words.next().is_some() {
            return Err("Trailing reference extraction data".into());
        }
        self.lattice.fit(&mut self.field, &|| false)?;
        crate::finish_extracted(
            &self.source,
            &self.field,
            &self.lattice,
            mesh,
            self.diagnostics,
            &|| false,
            progress,
        )
    }
    fn read_obj(mut input: impl Read) -> Result<(Vec<Vec3>, Vec<[usize; 4]>)> {
        let mut text = String::new();
        input.read_to_string(&mut text).map_err(|e| e.to_string())?;
        let mut positions = Vec::new();
        let mut quads = Vec::new();
        for line in text.lines() {
            let mut words = line.split_whitespace();
            match words.next() {
                Some("v") => {
                    let v: Vec<f64> = words
                        .map(|w| w.parse().map_err(|_| "Invalid OBJ coordinate"))
                        .collect::<std::result::Result<_, _>>()?;
                    let p: [f64; 3] = v.try_into().map_err(|_| "Expected three OBJ coordinates")?;
                    positions.push(Vec3::from_array(p));
                }
                Some("f") => {
                    let v: Vec<usize> = words
                        .map(|w| {
                            w.split('/')
                                .next()
                                .and_then(|n| n.parse::<usize>().ok())
                                .and_then(|n| n.checked_sub(1))
                                .ok_or("Invalid OBJ index")
                        })
                        .collect::<std::result::Result<_, _>>()?;
                    let q: [usize; 4] = v.try_into().map_err(|_| "Expected a quad OBJ")?;
                    quads.push(q);
                }
                _ => {}
            }
        }
        if quads.iter().flatten().any(|&v| v >= positions.len()) {
            return Err("Invalid OBJ vertex index".into());
        }
        Ok((positions, quads))
    }
    pub fn validate_obj(&self, input: impl Read) -> Result<f64> {
        let (positions, quads) = Self::read_obj(input)?;
        Ok(crate::surface::validate_output(
            &self.source,
            &self.field.spacing,
            positions,
            &quads,
            &|| false,
        )?
        .error)
    }
    pub fn diagnose_obj(&self, input: impl Read) -> Result<serde_json::Value> {
        let (positions, quads) = Self::read_obj(input)?;
        crate::surface::quality_report(&self.source, &self.field.spacing, &positions, &quads)
    }
    pub fn diagnose_candidate(mut self) -> Result<serde_json::Value> {
        if !self.repaired {
            return Err("Repair snapshot before extracting".into());
        }
        let mut mesh = self.extracted()?;
        crate::geometry::fit_dynamic(&self.field, &self.lattice.edges, &mut mesh, &|| false)?;
        let before_positions = mesh
            .positions
            .iter()
            .map(|v| v.to_array())
            .collect::<Vec<_>>();
        let before_quality = crate::surface::quality_report(
            &self.source,
            &self.field.spacing,
            &mesh.positions,
            &mesh.quads,
        )?;
        let fit = crate::surface::fit(
            &self.source,
            &self.field.spacing,
            &mut mesh.positions,
            &mesh.quads,
            &|| false,
        );
        let quality = crate::surface::quality_report(
            &self.source,
            &self.field.spacing,
            &mesh.positions,
            &mesh.quads,
        )?;
        Ok(
            serde_json::json!({"audit_only":true,"surface_fit_error":fit.err(),"quality":quality,"before_surface_fit":{"positions":before_positions,"quality":before_quality},
            "positions":mesh.positions.iter().map(|v|v.to_array()).collect::<Vec<_>>(),"quads":mesh.quads}),
        )
    }
    pub fn repair_coarse(&mut self) -> Result<()> {
        crate::coarse::repair_lattice(&mut self.lattice, &|| false)
    }
    pub fn read(reader: impl Read) -> Result<Self> {
        let result: Self = serde_json::from_reader(reader).map_err(|e| e.to_string())?;
        if result.version != 1 {
            return Err("Unsupported audit snapshot".into());
        }
        Ok(result)
    }
    pub fn write(&self, writer: impl Write) -> Result<()> {
        serde_json::to_writer(writer, self).map_err(|e| e.to_string())
    }
    /// Whitespace format consumed by the pinned upstream C++ reference harness.
    pub fn write_fixed_input(&self, mut writer: impl Write) -> std::io::Result<()> {
        let f = &self.field;
        writeln!(writer, "{} {}", f.origins.len(), self.lattice.edges.len())?;
        for i in 0..f.origins.len() {
            for p in [
                f.surface.positions[i],
                f.normals[i],
                f.directions[i],
                f.origins[i],
            ] {
                write!(writer, "{:.17e} {:.17e} {:.17e} ", p.x, p.y, p.z)?;
            }
            writeln!(writer, "{:.17e}", f.spacing[i])?;
        }
        for e in &self.lattice.edges {
            writeln!(writer, "{} {} {} {}", e.ends[0], e.ends[1], e.d.x, e.d.y)?;
        }
        // Reference comparisons currently cover the unconstrained stage only.
        if f.constraints.iter().any(|c| c.weight >= 1.) {
            return Err(std::io::Error::other(
                "Sharp/boundary reference export is not implemented",
            ));
        }
        Ok(())
    }
    fn extracted(&mut self) -> Result<crate::integer::Extraction> {
        self.lattice.fit(&mut self.field, &|| false)?;
        let mut mesh = self.lattice.extract(&self.field, &|| false)?;
        crate::quad::fix_valence(&mut mesh, &|| false)?;
        Ok(mesh)
    }
    pub fn write_extracted(&mut self, mut writer: impl Write, cleanup: bool) -> Result<()> {
        if !self.repaired {
            return Err("Repair snapshot before extracting".into());
        }
        self.lattice.fit(&mut self.field, &|| false)?;
        let mut mesh = self.lattice.extract(&self.field, &|| false)?;
        if cleanup {
            crate::quad::fix_valence(&mut mesh, &|| false)?;
        }
        let write = || -> std::io::Result<()> {
            writeln!(writer, "{} {}", mesh.positions.len(), mesh.quads.len())?;
            for (p, s) in mesh.positions.iter().zip(&mesh.samples) {
                write!(writer, "{:.17e} {:.17e} {:.17e} {}", p.x, p.y, p.z, s.len())?;
                for v in s {
                    write!(writer, " {v}")?;
                }
                writeln!(writer)?;
            }
            for [a, b, c, d] in mesh.quads {
                writeln!(writer, "{a} {b} {c} {d}")?;
            }
            Ok(())
        };
        write().map_err(|e| e.to_string())
    }
    pub fn write_dynamic_input(&mut self, mut writer: impl Write) -> Result<()> {
        let mesh = self.extracted()?;
        let f = &self.field;
        if f.constraints.iter().any(|c| c.weight >= 1.) {
            return Err("Constrained dynamic reference export is not implemented".into());
        }
        let (diffs, counts) = crate::geometry::dynamic_targets(f, &self.lattice.edges, &mesh);
        let write = || -> std::io::Result<()> {
            writeln!(
                writer,
                "{} {} {} {}",
                f.origins.len(),
                f.surface.triangles.len(),
                mesh.positions.len(),
                mesh.quads.len()
            )?;
            for i in 0..f.origins.len() {
                for p in [f.surface.positions[i], f.normals[i], f.directions[i]] {
                    write!(writer, "{:.17e} {:.17e} {:.17e} ", p.x, p.y, p.z)?;
                }
                writeln!(writer)?;
            }
            for [a, b, c] in &f.surface.triangles {
                writeln!(writer, "{a} {b} {c}")?;
            }
            for (p, samples) in mesh.positions.iter().zip(&mesh.samples) {
                write!(
                    writer,
                    "{:.17e} {:.17e} {:.17e} {}",
                    p.x,
                    p.y,
                    p.z,
                    samples.len()
                )?;
                for v in samples {
                    write!(writer, " {v}")?;
                }
                writeln!(writer)?;
            }
            for [a, b, c, d] in &mesh.quads {
                writeln!(writer, "{a} {b} {c} {d}")?;
            }
            for (d, count) in diffs.iter().zip(counts) {
                writeln!(writer, "{:.17e} {:.17e} {:.17e} {}", d.x, d.y, d.z, count)?;
            }
            Ok(())
        };
        write().map_err(|e| e.to_string())
    }
    pub fn trace_dynamic(&mut self, writer: impl Write) -> Result<()> {
        let mut mesh = self.extracted()?;
        let writer = std::cell::RefCell::new(writer);
        crate::geometry::fit_dynamic_observed(
            &self.field,
            &self.lattice.edges,
            &mut mesh,
            &|| false,
            &|iter, nearest, positions| {
                let mut out = writer.borrow_mut();
                writeln!(out, "ITER {iter} {}", positions.len()).map_err(|e| e.to_string())?;
                for (&s, p) in nearest.iter().zip(positions) {
                    writeln!(out, "{s} {:.17e} {:.17e} {:.17e}", p.x, p.y, p.z)
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            },
        )
    }
    pub fn dynamic_positions(&mut self) -> Result<Vec<Vec3>> {
        let mut mesh = self.extracted()?;
        crate::geometry::fit_dynamic(&self.field, &self.lattice.edges, &mut mesh, &|| false)?;
        Ok(mesh.positions)
    }
    pub fn write_coarse_input(&self, mut writer: impl Write) -> std::io::Result<()> {
        let l = &self.lattice;
        writeln!(writer, "{} {}", l.edges.len(), l.faces.len())?;
        for e in &l.edges {
            writeln!(
                writer,
                "{} {} {} {}",
                e.d.x,
                e.d.y,
                u8::from(e.fixed_axis != Some(0)),
                u8::from(e.fixed_axis != Some(1))
            )?;
        }
        for f in &l.faces {
            writeln!(
                writer,
                "{} {} {} {} {} {}",
                f.edges[0],
                f.edges[1],
                f.edges[2],
                f.rotations[0].rem_euclid(4),
                f.rotations[1].rem_euclid(4),
                f.rotations[2].rem_euclid(4)
            )?;
        }
        Ok(())
    }
    pub fn trace_coarse(&self, mut writer: impl Write) -> Result<()> {
        let levels = crate::coarse::Graph::from_lattice(&self.lattice)?.hierarchy(&|| false)?;
        for (i, g) in levels.iter().enumerate() {
            writeln!(writer, "LEVEL {i} {} {}", g.offsets.len(), g.faces.len())
                .map_err(|e| e.to_string())?;
            for (i, d) in g.offsets.iter().enumerate() {
                writeln!(
                    writer,
                    "{} {} {} {} {} {}",
                    d.x,
                    d.y,
                    g.allow[i][0],
                    g.allow[i][1],
                    if g.edge_faces[i][0] == crate::coarse::NONE {
                        -1
                    } else {
                        g.edge_faces[i][0] as i64
                    },
                    if g.edge_faces[i][1] == crate::coarse::NONE {
                        -1
                    } else {
                        g.edge_faces[i][1] as i64
                    }
                )
                .map_err(|e| e.to_string())?;
            }
            for (f, r) in g.faces.iter().zip(&g.rotations) {
                writeln!(
                    writer,
                    "{} {} {} {} {} {}",
                    f[0], f[1], f[2], r[0], r[1], r[2]
                )
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub fn fit_origins(&mut self, cancel: &dyn Fn() -> bool) -> Result<&[Vec3]> {
        self.lattice.fit(&mut self.field, cancel)?;
        Ok(&self.field.origins)
    }
    pub fn reconstruct(
        mut self,
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(Stage),
    ) -> Result<Layout> {
        if !self.repaired {
            crate::repair(&mut self.lattice, &mut self.diagnostics, cancel, progress)?;
        }
        crate::finish(
            &self.source,
            self.field,
            self.lattice,
            self.diagnostics,
            cancel,
            progress,
        )
    }
}

pub(crate) mod vectors {
    use crate::Vec3;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[Vec3], s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|p| p.to_array()))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<Vec3>, D::Error> {
        Ok(Vec::<[f64; 3]>::deserialize(d)?
            .into_iter()
            .map(Vec3::from_array)
            .collect())
    }
}
pub(crate) mod vector {
    use crate::Vec3;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(v: &Vec3, s: S) -> std::result::Result<S::Ok, S::Error> {
        v.to_array().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Vec3, D::Error> {
        Ok(Vec3::from_array(<[f64; 3]>::deserialize(d)?))
    }
}
pub(crate) mod offset {
    use glam::IVec2;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(v: &IVec2, s: S) -> std::result::Result<S::Ok, S::Error> {
        v.to_array().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<IVec2, D::Error> {
        Ok(IVec2::from_array(<[i32; 2]>::deserialize(d)?))
    }
}

pub(crate) mod offsets {
    use glam::IVec2;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[IVec2], s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|p| p.to_array()))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<IVec2>, D::Error> {
        Ok(Vec::<[i32; 2]>::deserialize(d)?
            .into_iter()
            .map(IVec2::from_array)
            .collect())
    }
}
