//! Capture corrected integer lattices and replay position fitting independently.
use clay_retopology::{
    Guide, Options, Surface, Vec3,
    audit::{FieldSnapshot, Snapshot},
};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    time::Instant,
};
fn create(path: &str) -> std::io::Result<BufWriter<File>> {
    Ok(BufWriter::new(
        File::options().write(true).create_new(true).open(path)?,
    ))
}
#[derive(serde::Deserialize)]
struct GuideInput {
    position: [f64; 3],
    normal: [f64; 3],
    tangent: [f64; 3],
    radius: f64,
    strength: f64,
}

fn read_guides(path: &str) -> Result<Vec<Guide>, Box<dyn std::error::Error>> {
    let inputs: Vec<GuideInput> = serde_json::from_reader(BufReader::new(File::open(path)?))?;
    Ok(inputs
        .into_iter()
        .map(|g| Guide {
            position: Vec3::from_array(g.position),
            normal: Vec3::from_array(g.normal),
            tangent: Vec3::from_array(g.tangent),
            radius: g.radius,
            strength: g.strength,
        })
        .collect())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let start = Instant::now();
    let progress = |s| eprintln!("{:.3}s {s:?}", start.elapsed().as_secs_f64());
    match args.get(1).map(String::as_str) {
        Some("prepare-surface") if args.len()==6 || args.len()==7 => {
            let mut doc:serde_json::Value=serde_json::from_reader(BufReader::new(File::open(&args[2])?))?;
            let positions:Vec<[f64;3]>=serde_json::from_value(doc["mesh"]["vertices"].clone())?;
            let triangles=serde_json::from_value(doc["mesh"]["triangles"].clone())?;
            let source=Surface {positions:positions.into_iter().map(Vec3::from_array).collect(),triangles};
            let options=Options {target_quads:args[5].parse()?,preserve_sharp:false,max_vertices:150_000,..Options::default()};
            let prepared=clay_retopology::audit::prepare_surface(&source,&options,args.get(6).map(|s|s.parse()).transpose()?.unwrap_or(5),&||false,&|step|eprintln!("{:.3}s {step:?}",start.elapsed().as_secs_f64()))?;
            let mut out=create(&args[4])?;serde_json::to_writer_pretty(&mut out,&prepared.report)?;out.flush()?;
            doc["mesh"]=serde_json::json!({"vertices":prepared.surface.positions.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"triangles":prepared.surface.triangles,"quads":[],"mask":vec![0.;prepared.detail.len()],"vertex_groups":[],"polygroups":[]});
            let mut out=create(&args[3])?;serde_json::to_writer(&mut out,&doc)?;out.flush()?;
        }
        Some("resolve-field") if args.len()==4 => {
            let mut field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            field.resolve_native(&progress)?;
            let mut out=create(&args[3])?;field.write(&mut out)?;out.flush()?;
        }
        Some("capture-field") if args.len()==5 => {
            let doc:serde_json::Value=serde_json::from_reader(BufReader::new(File::open(&args[2])?))?;
            let positions:Vec<[f64;3]>=serde_json::from_value(doc["mesh"]["vertices"].clone())?;
            let triangles=serde_json::from_value(doc["mesh"]["triangles"].clone())?;
            drop(doc);
            let source=Surface {positions:positions.into_iter().map(Vec3::from_array).collect(),triangles};
            let field=FieldSnapshot::capture(source,args[4].parse()?,&progress)?;
            let mut out=create(&args[3])?;field.write(&mut out)?;out.flush()?;
        }
        Some("export-field") if args.len()==4 => {
            let field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            let mut out=create(&args[3])?;field.export_reference(&mut out)?;out.flush()?;
        }
        Some("import-field") if args.len()==5 => {
            let mut field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            field.import_reference(BufReader::new(File::open(&args[3])?))?;
            let mut out=create(&args[4])?;field.write(&mut out)?;out.flush()?;
        }
        Some("export-field-grid") if args.len()==4 => {
            let field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            let mut out=create(&args[3])?;field.export_initial_grid(&mut out)?;out.flush()?;
        }
        Some("export-corrected-grid") if args.len()==4 => {
            let field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            let mut out=create(&args[3])?;field.export_corrected_grid(&mut out)?;out.flush()?;
        }
        Some("export-native-mapping") if args.len()==4 => {
            let field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            let mut out=create(&args[3])?;field.export_native_mapping(&mut out)?;out.flush()?;
        }
        Some("import-integer-grid") if args.len()==5 => {
            let field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            let snapshot=field.integerize_reference(BufReader::new(File::open(&args[3])?),&progress)?;
            let mut out=create(&args[4])?;snapshot.write(&mut out)?;out.flush()?;
        }
        Some("integerize-field") if args.len()==4 => {
            let field=FieldSnapshot::read(BufReader::new(File::open(&args[2])?))?;
            let snapshot=field.integerize(&progress)?;
            let mut out=create(&args[3])?;snapshot.write(&mut out)?;out.flush()?;
        }
        Some("rebuild-patches") if args.len()==7 || args.len()==8 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let report=snapshot.rebuild_patches(args[4].parse()?,args[5].parse()?,args[6].parse()?,args.get(7).map(String::as_str).unwrap_or("dynamic"))?;
            let mut out=create(&args[3])?;serde_json::to_writer(&mut out,&report)?;out.flush()?;
        }
        Some("export-initial-grid") if args.len()==4 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let mut out=create(&args[3])?;snapshot.write_initial_grid_input(&mut out)?;out.flush()?;
        }
        Some("orientation-diagnostic") if args.len()==4 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let report=snapshot.orientation_diagnostic()?;
            let mut out=create(&args[3])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
        }
        Some("strict-contract") if args.len()==7 => {
            let mut snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let report=snapshot.contract_preserving_orientation(args[5].parse()?,args[6].parse()?)?;
            let mut out=create(&args[3])?;snapshot.write(&mut out)?;out.flush()?;
            let mut out=create(&args[4])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
        }
        Some(mode @ ("reflow-inversions" | "reflow-inversions-pinned")) if args.len()==6 => {
            let mut snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let report=snapshot.reflow_inversions(args[5].parse()?,mode=="reflow-inversions-pinned")?;
            let mut out=create(&args[3])?; snapshot.write(&mut out)?; out.flush()?;
            let mut out=create(&args[4])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
        }
        Some("repair-trace-rebuild") if args.len()==5 => {
            let mut snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            snapshot.repair_traced_rebuilt(std::path::Path::new(&args[3]),args[4].parse()?,&progress)?;
        }
        Some(mode @ ("solve-cnf" | "explain-cnf")) if args.len()==5 => {
            let report=clay_retopology::audit::solve_dimacs(BufReader::new(File::open(&args[2])?),args[4].parse()?,mode=="explain-cnf")?;
            let mut out=create(&args[3])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
        }
        Some("export-repair-patches") if args.len()==6 => {
            let mut snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let count=snapshot.export_repair_patches(std::path::Path::new(&args[3]),args[4].parse()?,args[5].parse()?)?;
            eprintln!("{count} frozen local repair patches");
        }
        Some(mode @ ("solve-repair-patch" | "solve-scalar-patch")) if args.len()==5 => {
            let patch:clay_retopology::audit::RepairPatch=serde_json::from_reader(BufReader::new(File::open(&args[2])?))?;
            let report=if mode=="solve-scalar-patch" {patch.solve_scalar(args[4].parse()?)?} else {patch.solve(args[4].parse()?)?};
            let mut out=create(&args[3])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
        }
        Some("repair-trace") if args.len()==5 || args.len()==6 => {
            let mut snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            snapshot.repair_traced(std::path::Path::new(&args[3]),args[4].parse()?,args.get(5).map(std::path::Path::new),&progress)?;
        }
        Some("curvature-guides") if args.len()==6 => {
            let doc:serde_json::Value=serde_json::from_reader(BufReader::new(File::open(&args[2])?))?;
            let positions:Vec<[f64;3]>=serde_json::from_value(doc["mesh"]["vertices"].clone())?;
            let triangles=serde_json::from_value(doc["mesh"]["triangles"].clone())?;
            let source=Surface {positions:positions.into_iter().map(Vec3::from_array).collect(),triangles};
            let report=clay_retopology::curvature::estimate(&source,args[3].parse()?,&||false)?;
            eprintln!("{} geometric candidates, {} hints",report.candidate_count,report.guides.len());
            let mut out=create(&args[4])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
            let mut out=create(&args[5])?;serde_json::to_writer(&mut out,&report.guides)?;out.flush()?;
        }
        Some("repair-diagnostic") if args.len()==4 => {
            let mut snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let result=snapshot.repair(&progress);
            // Preserve a failed attempt for locating inverted cells. Its
            // repaired flag remains false; this does not create accepted output.
            let mut out=create(&args[3])?;snapshot.write(&mut out)?;out.flush()?;
            result?;
        }
        Some("select-cycles") if args.len()==5 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let report=snapshot.select_cycles(&||false)?;
            let cycles:Vec<_>=report.selected.iter().map(|c|&c.vertices).collect();
            eprintln!("{} geometric sections, {} selected cycles",report.candidates.len(),cycles.len());
            let mut out=create(&args[3])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
            let mut out=create(&args[4])?;serde_json::to_writer(&mut out,&cycles)?;out.flush()?;
        }
        Some("constrain-cycles") if args.len()==5 => {
            let mut snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let cycles:Vec<Vec<usize>>=serde_json::from_reader(BufReader::new(File::open(&args[3])?))?;
            snapshot.constrain_cycles_raw(&cycles,&progress)?;
            let mut out=create(&args[4])?;snapshot.write(&mut out)?;out.flush()?;
        }
        Some("diagnose-obj") if args.len()==5 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let report=snapshot.diagnose_obj(BufReader::new(File::open(&args[3])?))?;
            let mut out=create(&args[4])?;serde_json::to_writer_pretty(&mut out,&report)?;out.flush()?;
        }
        Some("diagnose-candidate") if args.len()==4 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let report=snapshot.diagnose_candidate()?;
            let mut out=create(&args[3])?;serde_json::to_writer(&mut out,&report)?;out.flush()?;
        }
        Some("validate-obj") if args.len()==4 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let error=snapshot.validate_obj(BufReader::new(File::open(&args[3])?))?;
            eprintln!("Unmodified reference mesh passed; maximum sampled surface error {error}");
        }
        Some("resume-reference") if args.len()==5 => {
            let snapshot=Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            let result=snapshot.reconstruct_reference(BufReader::new(File::open(&args[3])?),&progress)?;
            eprintln!("{} vertices, {} quads; {:?}",result.positions.len(),result.quads.len(),result.diagnostics);
            let mut out=create(&args[4])?;
            for p in result.positions {writeln!(out,"v {} {} {}",p.x,p.y,p.z)?;}
            for [a,b,c,d] in result.quads {writeln!(out,"f {} {} {} {}",a+1,b+1,c+1,d+1)?;}
            out.flush()?;
        }
        Some(mode @ ("capture" | "capture-raw" | "capture-guided" | "capture-raw-guided" | "capture-direction-guided"))
            if args.len() == if mode.ends_with("-guided") { 6 } else { 5 } => {
            let doc: serde_json::Value = serde_json::from_reader(BufReader::new(File::open(&args[2])?))?;
            let positions: Vec<[f64;3]> = serde_json::from_value(doc["mesh"]["vertices"].clone())?;
            let triangles = serde_json::from_value(doc["mesh"]["triangles"].clone())?;
            let source = Surface { positions: positions.into_iter().map(Vec3::from_array).collect(), triangles };
            drop(doc);
            let guides = if mode.ends_with("-guided") { read_guides(&args[5])? } else { Vec::new() };
            let options = Options { target_quads: args[4].parse()?, guides, preserve_sharp: false, max_vertices: 150_000, ..Options::default() };
            let snapshot = if mode=="capture-direction-guided" { Snapshot::capture_direction_guides_raw(source,&options,&||false,&progress)? } else if mode=="capture" || mode=="capture-guided" { Snapshot::capture(source, &options, &|| false, &progress)? } else {Snapshot::capture_raw(source,&options,&||false,&progress)?};
            let mut file = create(&args[3])?;
            snapshot.write(&mut file)?;
            file.flush()?;
        }
        Some(mode @ ("extract" | "extract-raw" | "repair" | "export-extraction" | "export" | "fit" | "resume" | "export-dynamic" | "fit-dynamic" | "trace-dynamic" | "export-coarse" | "trace-coarse" | "repair-coarse")) if args.len() == 4 => {
            let mut snapshot = Snapshot::read(BufReader::new(File::open(&args[2])?))?;
            if mode=="extract" || mode=="extract-raw" {
                let mut out=create(&args[3])?;snapshot.write_extracted(&mut out,mode=="extract")?;out.flush()?;
            } else if mode == "repair" {
                snapshot.repair(&progress)?;
                let mut out=create(&args[3])?;snapshot.write(&mut out)?;out.flush()?;
            } else if mode=="export-extraction" {
                let mut out=create(&args[3])?;snapshot.write_extraction_input(&mut out)?;out.flush()?;
            } else if mode == "repair-coarse" {
                snapshot.repair_coarse()?;let mut out=create(&args[3])?;snapshot.write_coarse_input(&mut out)?;out.flush()?;
            } else if mode == "export-coarse" {
                let mut out=create(&args[3])?;snapshot.write_coarse_input(&mut out)?;out.flush()?;
            } else if mode == "trace-coarse" {
                let mut out=create(&args[3])?;snapshot.trace_coarse(&mut out)?;out.flush()?;
            } else if mode == "trace-dynamic" {
                let mut out=create(&args[3])?;
                snapshot.trace_dynamic(&mut out)?;
                out.flush()?;
            } else if mode == "export-dynamic" {
                let mut out = create(&args[3])?;
                snapshot.write_dynamic_input(&mut out)?;
                out.flush()?;
            } else if mode == "fit-dynamic" {
                let positions = snapshot.dynamic_positions()?;
                let mut out = create(&args[3])?;
                for p in positions {writeln!(out, "{:.17e} {:.17e} {:.17e}",p.x,p.y,p.z)?;}
                out.flush()?;
            } else if mode == "export" {
                let mut out = create(&args[3])?;
                snapshot.write_fixed_input(&mut out)?;
                out.flush()?;
            } else if mode == "fit" {
                let positions = snapshot.fit_origins(&|| false)?;
                let mut out = create(&args[3])?;
                for p in positions { writeln!(out, "{:.17e} {:.17e} {:.17e}", p.x, p.y, p.z)?; }
                out.flush()?;
            } else {
                let result = snapshot.reconstruct(&|| false, &progress)?;
                eprintln!("{} vertices, {} quads; {:?}", result.positions.len(), result.quads.len(), result.diagnostics);
                let mut out = create(&args[3])?;
                for p in result.positions { writeln!(out, "v {} {} {}", p.x, p.y, p.z)?; }
                for [a,b,c,d] in result.quads { writeln!(out, "f {} {} {} {}", a+1,b+1,c+1,d+1)?; }
                out.flush()?;
            }
        }
        _ => return Err("usage: audit capture source.clay snapshot.json target-quads | audit export/fit/resume snapshot.json output".into()),
    }
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    eprintln!(
        "{:.3}s finished; {}",
        start.elapsed().as_secs_f64(),
        status
            .lines()
            .find(|l| l.starts_with("VmHWM:"))
            .unwrap_or("")
    );
    Ok(())
}
