//! Manual release-mode edit timings; no timing assertions in the test suite.
use clay_form::{
    document::Document,
    sculpt::{Geometry, Mesh, Transform, Vec3},
};
use std::time::Instant;
fn main() {
    println!(
        "levels,models,active_vertices,first_transform_ms,subsequent_transform_ms,mask_region_ms"
    );
    for (levels, models) in [(0, 1), (2, 1), (2, 4)] {
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        for _ in 0..levels {
            geometry.subdivide().unwrap();
        }
        let mut document = Document::default();
        let mut project = document.project();
        project.mesh = geometry.mesh;
        project.mesh_settings = geometry.settings;
        document.replace_project(project, "profile.clay").unwrap();
        for _ in 1..models {
            document.duplicate_model(false).unwrap();
        }
        let transform = Transform {
            translation: Vec3(0.001, 0., 0.),
            ..Default::default()
        };
        let start = Instant::now();
        document.apply_transform(transform, false).unwrap();
        let first = start.elapsed().as_secs_f64() * 1000.;
        let mut subsequent = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            document.apply_transform(transform, false).unwrap();
            subsequent.push(start.elapsed().as_secs_f64() * 1000.);
        }
        subsequent.sort_by(f64::total_cmp);
        document.end_stroke();
        let start = Instant::now();
        document.mask_region(|p| p.0 > 0., false).unwrap();
        let mask = start.elapsed().as_secs_f64() * 1000.;
        println!(
            "{levels},{models},{},{first:.3},{:.3},{mask:.3}",
            document.mesh().vertices.len(),
            subsequent[2]
        );
    }
}
