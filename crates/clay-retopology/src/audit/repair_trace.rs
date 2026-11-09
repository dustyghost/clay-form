//! Durable, same-input coarse-repair checkpoints and per-patch diagnostics.
//! A checkpoint is an offline solver state, never an accepted model. Resume
//! with the original frozen input; an interrupted fine repair restarts from
//! the saved completed coarse pass rather than its partial fine changes.
use super::Snapshot;
use crate::{
    Result, Stage, Vec3,
    coarse::Graph,
    coarse_sat::{Checkpoint, Observer, PatchReport},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Saved {
    version: u32,
    input_fingerprint: String,
    state: Checkpoint,
}
#[derive(Serialize)]
struct StateRef<'a> {
    depth: usize,
    next_radius: usize,
    graph: &'a Graph,
}
#[derive(Serialize)]
struct SavedRef<'a> {
    version: u32,
    input_fingerprint: &'a str,
    state: StateRef<'a>,
}

// Streaming FNV-1a is an identity guard for trusted same-build audit fixtures,
// not a cryptographic hash or an interchange-format security boundary.
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
fn fingerprint(snapshot: &Snapshot) -> Result<String> {
    let mut hash = Fingerprint(0xcbf29ce484222325);
    serde_json::to_writer(&mut hash, snapshot).map_err(|e| e.to_string())?;
    Ok(format!("{:016x}", hash.0))
}

struct Trace {
    directory: PathBuf,
    events: BufWriter<File>,
    centers: Vec<Vec3>,
    input_fingerprint: String,
    started: Instant,
    write_checkpoint: bool,
}
impl Trace {
    fn event(&mut self, event: serde_json::Value) -> Result<()> {
        serde_json::to_writer(&mut self.events, &event).map_err(|e| e.to_string())?;
        writeln!(self.events).map_err(|e| e.to_string())?;
        self.events.flush().map_err(|e| e.to_string())
    }
    fn locations(&self, ids: &[usize]) -> Vec<serde_json::Value> {
        ids.iter().map(|&f| serde_json::json!({"source_face":f, "representative_center":self.centers[f].to_array()})).collect()
    }
    fn stage(&mut self, name: &str, graph: &Graph) -> Result<()> {
        let ids: Vec<_> = (0..graph.faces.len())
            .filter(|&f| graph.area(f) < 0)
            .map(|f| graph.source_faces[f])
            .collect();
        self.event(serde_json::json!({"event":"stage", "stage":name, "seconds":self.started.elapsed().as_secs_f64(), "inversions":self.locations(&ids)}))
    }
}
impl Observer for Trace {
    fn checkpoint(&mut self, depth: usize, next_radius: usize, graph: &Graph) -> Result<()> {
        if !self.write_checkpoint {
            // The experimental radius-outer schedule cannot resume through the
            // depth-outer checkpoint format. Keep diagnostic events only.
            let ids: Vec<_> = (0..graph.faces.len())
                .filter(|&f| graph.area(f) < 0)
                .map(|f| graph.source_faces[f])
                .collect();
            return self.event(serde_json::json!({"event":"rebuilt_pass", "seconds":self.started.elapsed().as_secs_f64(), "depth":depth, "radius":next_radius-1, "inversions":self.locations(&ids)}));
        }
        let pending = self.directory.join("checkpoint.pending.json");
        let saved = SavedRef {
            version: 1,
            input_fingerprint: &self.input_fingerprint,
            state: StateRef {
                depth,
                next_radius,
                graph,
            },
        };
        let mut writer = BufWriter::new(File::create(&pending).map_err(|e| e.to_string())?);
        serde_json::to_writer(&mut writer, &saved).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        drop(writer);
        fs::rename(pending, self.directory.join("checkpoint.json")).map_err(|e| e.to_string())?;
        let ids: Vec<_> = (0..graph.faces.len())
            .filter(|&f| graph.area(f) < 0)
            .map(|f| graph.source_faces[f])
            .collect();
        self.event(serde_json::json!({"event":"checkpoint", "seconds":self.started.elapsed().as_secs_f64(),"depth":depth,"next_radius":next_radius,"faces":graph.faces.len(),"edges":graph.offsets.len(),"inversions":self.locations(&ids)}))
    }
    fn patch(&mut self, report: &PatchReport) -> Result<()> {
        self.event(serde_json::json!({"event":"patch","seconds":self.started.elapsed().as_secs_f64(),"patch":report,"locations_before":self.locations(&report.inverted_source_faces_before),"locations_after":self.locations(&report.inverted_source_faces_after)}))
    }
}

impl Snapshot {
    pub fn repair_traced(
        &mut self,
        directory: &Path,
        seconds: f64,
        resume: Option<&Path>,
        progress: &dyn Fn(Stage),
    ) -> Result<()> {
        self.repair_traced_using(directory, seconds, resume, progress, false)
    }
    pub fn repair_traced_rebuilt(
        &mut self,
        directory: &Path,
        seconds: f64,
        progress: &dyn Fn(Stage),
    ) -> Result<()> {
        self.repair_traced_using(directory, seconds, None, progress, true)
    }
    fn repair_traced_using(
        &mut self,
        directory: &Path,
        seconds: f64,
        resume: Option<&Path>,
        progress: &dyn Fn(Stage),
        rebuild: bool,
    ) -> Result<()> {
        if self.repaired || !seconds.is_finite() || seconds <= 0. || seconds > 3600. {
            return Err(
                "Repair trace needs an unrepaired snapshot and a time budget in (0, 3600] seconds"
                    .into(),
            );
        }
        let input_fingerprint = fingerprint(self)?;
        let state = if let Some(path) = resume {
            let saved: Saved = serde_json::from_reader(BufReader::new(
                File::open(path).map_err(|e| e.to_string())?,
            ))
            .map_err(|e| e.to_string())?;
            if saved.version != 1 || saved.input_fingerprint != input_fingerprint {
                return Err("Repair checkpoint belongs to a different frozen input".into());
            }
            Some(saved.state)
        } else {
            None
        };
        // A new directory ensures atomic checkpoint replacement touches only
        // files this invocation owns. Existing diagnostic directories are kept.
        fs::create_dir(directory).map_err(|e| e.to_string())?;
        let started = Instant::now();
        let deadline = started + Duration::from_secs_f64(seconds);
        let cancel = || Instant::now() >= deadline;
        let positions = &self.field.surface.positions;
        let centers = self
            .field
            .surface
            .triangles
            .iter()
            .map(|t| (positions[t[0]] + positions[t[1]] + positions[t[2]]) / 3.)
            .collect();
        let mut trace = Trace {
            directory: directory.into(),
            events: BufWriter::new(
                File::create(directory.join("events.jsonl")).map_err(|e| e.to_string())?,
            ),
            centers,
            input_fingerprint,
            started,
            write_checkpoint: !rebuild,
        };
        let mut stage = "greedy";
        let result = (|| -> Result<()> {
            trace.stage("input", &Graph::from_lattice(&self.lattice)?)?;
            crate::coarse::repair_lattice(&mut self.lattice, &cancel)?;
            trace.stage("after_greedy", &Graph::from_lattice(&self.lattice)?)?;
            stage = "coarse_sat";
            if rebuild {
                crate::coarse_sat::repair_rebuilt_observed(
                    &mut self.lattice,
                    &cancel,
                    progress,
                    &mut trace,
                )?;
            } else {
                crate::coarse_sat::repair_observed(
                    &mut self.lattice,
                    &cancel,
                    progress,
                    &mut trace,
                    state,
                )?;
            }
            trace.stage("after_coarse_sat", &Graph::from_lattice(&self.lattice)?)?;
            stage = "fine_repair";
            self.lattice.remove_flips(&cancel, progress)?;
            self.repaired = true;
            stage = "complete";
            trace.stage("complete", &Graph::from_lattice(&self.lattice)?)?;
            Ok(())
        })();
        // A rejected attempt still needs accurate final counts. During coarse
        // cancellation this is the committed fine lattice; checkpoint.json
        // separately retains the last completed coarse pass.
        self.diagnostics.flipped_after = self.lattice.flipped();
        if result.is_err() {
            trace.stage("stopped", &Graph::from_lattice(&self.lattice)?)?;
        }
        let resume_input = if rebuild {
            None
        } else {
            Some(
                "Use the original frozen input plus checkpoint.json; attempted.json is not a coarse-stage resume checkpoint.",
            )
        };
        let completion = serde_json::json!({"stage":stage,"schedule":if rebuild {"radius_outer_rebuilt"} else {"depth_outer"},"seconds":started.elapsed().as_secs_f64(),"deadline_reached":cancel(),"repaired":self.repaired,"lattice_inversions":self.diagnostics.flipped_after,"error":result.as_ref().err(),"input_fingerprint":trace.input_fingerprint,"resume_input":resume_input});
        let mut writer = BufWriter::new(
            File::create(directory.join("completion.json")).map_err(|e| e.to_string())?,
        );
        serde_json::to_writer_pretty(&mut writer, &completion).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        let mut writer = BufWriter::new(
            File::create(directory.join("attempted.json")).map_err(|e| e.to_string())?,
        );
        self.write(&mut writer)?;
        writer.flush().map_err(|e| e.to_string())?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagnostics, Surface, field::Constraint, field::Field, integer::Lattice};
    use glam::IVec2;

    #[test]
    fn cancelled_trace_preserves_unrepaired_counts_and_locations() {
        let positions = vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y];
        let field = Field {
            surface: Surface {
                positions: positions.clone(),
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            },
            normals: vec![Vec3::Z; 4],
            directions: vec![Vec3::X; 4],
            origins: positions,
            spacing: vec![1.; 4],
            areas: vec![1.; 4],
            constraints: vec![Constraint::default(); 4],
        };
        let mut lattice = Lattice::new(&field, &|| false).unwrap();
        let coordinates = [IVec2::ZERO, IVec2::X, -IVec2::Y, -IVec2::X];
        for edge in &mut lattice.edges {
            edge.d = coordinates[edge.ends[1]] - coordinates[edge.ends[0]];
        }
        assert_eq!(lattice.flipped(), 2);
        let mut snapshot = Snapshot {
            version: 1,
            repaired: false,
            source: field.surface.clone(),
            field,
            lattice,
            diagnostics: Diagnostics::default(),
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("clay-repair-trace-{}-{stamp}", std::process::id()));
        // Positive, but rounds to zero Duration: cancellation is deterministic.
        assert!(
            snapshot
                .repair_traced(&directory, f64::MIN_POSITIVE, None, &|_| {})
                .is_err()
        );
        let read = |name: &str| -> serde_json::Value {
            serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap()
        };
        let completion = read("completion.json");
        assert_eq!(completion["lattice_inversions"], 2);
        assert_eq!(completion["deadline_reached"], true);
        assert_eq!(completion["repaired"], false);
        assert_eq!(read("attempted.json")["diagnostics"]["flipped_after"], 2);
        let events = fs::read_to_string(directory.join("events.jsonl")).unwrap();
        let stopped: serde_json::Value =
            serde_json::from_str(events.lines().last().unwrap()).unwrap();
        assert_eq!(stopped["stage"], "stopped");
        assert_eq!(stopped["inversions"].as_array().unwrap().len(), 2);
        fs::remove_dir_all(directory).unwrap();
    }
}
