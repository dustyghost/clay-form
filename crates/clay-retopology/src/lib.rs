//! Surface-field quadrangulation. The integer regularity and orientation
//! constraints follow Huang et al., QuadriFlow (2018), equations 3–9.
//! Geometry is reconstructed from lattice coordinates, not triangle quality scores.
#[cfg(feature = "audit")]
pub mod audit;
mod coarse;
mod coarse_sat;
#[cfg(feature = "audit")]
pub mod curvature;
#[cfg(feature = "audit")]
pub mod cycles;
mod extraction;
mod field;
mod flow;
mod geometry;
mod integer;
mod quad;
pub mod resample;
mod sat;
mod surface;
mod symmetry;
mod topology;

pub use glam::DVec3 as Vec3;

#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Surface {
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::vectors"))]
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Clone, Copy, Debug)]
pub struct Guide {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub radius: f64,
    pub strength: f64,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub target_quads: usize,
    /// Independent signed paint: -1 less detail, 0 automatic, +1 more detail.
    pub detail: Vec<f64>,
    pub guides: Vec<Guide>,
    pub adaptive: f64,
    pub preserve_sharp: bool,
    pub sharp_angle_degrees: f64,
    pub max_vertices: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            target_quads: 5000,
            detail: Vec::new(),
            guides: Vec::new(),
            adaptive: 0.,
            preserve_sharp: true,
            sharp_angle_degrees: 45.,
            max_vertices: 500_000,
        }
    }
}

impl Options {
    fn sharp_threshold(&self) -> Option<f64> {
        self.preserve_sharp
            .then(|| self.sharp_angle_degrees.to_radians().cos())
    }
}

#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub input_vertices: usize,
    pub samples: usize,
    pub position_defects_before: usize,
    pub position_defects_after: usize,
    pub fixed_variables: usize,
    pub flipped_before: usize,
    pub flipped_after: usize,
    pub max_fitting_distance: f64,
    pub max_surface_error: f64,
}

pub struct Layout {
    pub positions: Vec<Vec3>,
    pub quads: Vec<[usize; 4]>,
    /// Use these render triangles: some poles require the alternate diagonal.
    pub triangles: Vec<[usize; 3]>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Copy, Debug)]
pub enum Stage {
    Preparation {
        iteration: usize,
        vertices: usize,
    },
    RetryPreparation,
    OriginalSurfaceValidation,
    Sampling,
    Hierarchy {
        samples: usize,
    },
    Orientation,
    Position,
    Regularity {
        defects: usize,
    },
    Subdivision {
        long_edges: usize,
        fixed_variables: usize,
    },
    Inversions {
        flipped: usize,
    },
    Repair {
        radius: usize,
        flipped: usize,
    },
    CoarseRepair {
        radius: usize,
        flipped: usize,
    },
    Fitting,
    Extraction,
    SurfaceFit,
    QuadOptimization,
}

pub(crate) type Result<T> = std::result::Result<T, String>;
pub(crate) fn check(cancel: &dyn Fn() -> bool) -> Result<()> {
    if cancel() {
        Err("Retopology cancelled".into())
    } else {
        Ok(())
    }
}

pub fn reconstruct(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
) -> Result<Layout> {
    reconstruct_with_progress(source, options, cancel, &|_| {})
}

pub fn reconstruct_with_progress(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<Layout> {
    reconstruct_using(source, options, cancel, progress, field::solve)
}

fn reconstruct_using(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
    solve_field: FieldSolver,
) -> Result<Layout> {
    let (field, mut lattice, mut diagnostics) =
        prepare_using(source, options, cancel, progress, solve_field)?;
    repair(&mut lattice, &mut diagnostics, cancel, progress)?;
    finish(source, field, lattice, diagnostics, cancel, progress)
}

/// Redistribute irregular source triangles, reconstruct, then independently
/// check the completed quads against the untouched original. If grid repair
/// exhausts its budget, retry once with four preparation passes instead of five.
pub fn reconstruct_regularized_with_progress(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<Layout> {
    validate_request(source, options, cancel)?;
    match reconstruct_prepared(source, options, 5, cancel, progress) {
        Err(error) if error.ends_with(integer::INVERSION_BUDGET_ERROR) => {
            check(cancel)?;
            progress(Stage::RetryPreparation);
            reconstruct_prepared(source, options, 4, cancel, progress)
        }
        result => result,
    }
}

fn reconstruct_prepared(
    source: &Surface,
    options: &Options,
    passes: usize,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<Layout> {
    progress(Stage::Preparation {
        iteration: 0,
        vertices: source.positions.len(),
    });
    let prepared = resample::prepare(source, options, passes, cancel, &|step| {
        progress(Stage::Preparation {
            iteration: step.iteration,
            vertices: step.vertices,
        });
    })?;
    let working_options = Options {
        detail: prepared.detail,
        ..options.clone()
    };
    let solver: FieldSolver = if options.guides.iter().any(|guide| guide.strength > 0.) {
        field::solve_soft_directions
    } else {
        field::solve
    };
    let mut layout = reconstruct_using(
        &prepared.surface,
        &working_options,
        cancel,
        progress,
        solver,
    )?;
    progress(Stage::OriginalSurfaceValidation);
    validate_layout(source, options, &mut layout, cancel)?;
    Ok(layout)
}

/// Recheck a layout after a caller's coordinate conversion, without moving it.
/// The render diagonals are selected again for the coordinates actually published.
pub fn validate_layout(
    source: &Surface,
    options: &Options,
    layout: &mut Layout,
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    validate_request(source, options, cancel)?;
    let spacing = field::source_spacing(source, options, cancel)?;
    let validated = surface::validate_output(
        source,
        &spacing,
        layout.positions.clone(),
        &layout.quads,
        cancel,
    )?;
    layout.triangles = validated.triangles;
    layout.diagnostics.input_vertices = source.positions.len();
    layout.diagnostics.max_surface_error = validated.error;
    Ok(())
}

#[cfg(feature = "audit")]
fn prepare(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<(field::Field, integer::Lattice, Diagnostics)> {
    prepare_using(source, options, cancel, progress, field::solve)
}

type FieldSolver =
    fn(&Surface, &Options, &dyn Fn() -> bool, &dyn Fn(Stage)) -> Result<field::Field>;

fn prepare_using(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
    solve_field: FieldSolver,
) -> Result<(field::Field, integer::Lattice, Diagnostics)> {
    validate_request(source, options, cancel)?;
    let field = solve_field(source, options, cancel, progress)?;
    prepare_field(source, field, options.max_vertices, cancel, progress)
}

fn validate_request(source: &Surface, options: &Options, cancel: &dyn Fn() -> bool) -> Result<()> {
    check(cancel)?;
    if source.positions.len() > options.max_vertices {
        return Err("Source exceeds the retopology sample budget".into());
    }
    if !(1. ..=180.).contains(&options.sharp_angle_degrees) {
        return Err("Sharp bend angle must be between 1° and 180°".into());
    }
    if options.target_quads < 4
        || options.target_quads > options.max_vertices
        || (!options.detail.is_empty() && options.detail.len() != source.positions.len())
        || options
            .detail
            .iter()
            .any(|x| !x.is_finite() || !(-1. ..=1.).contains(x))
        || !options.adaptive.is_finite()
        || !(0. ..=1.).contains(&options.adaptive)
        || options.guides.iter().any(|g| {
            !g.position.is_finite()
                || !g.normal.is_finite()
                || !g.tangent.is_finite()
                || g.normal.length_squared() < 1e-12
                || g.tangent.length_squared() < 1e-12
                || !g.radius.is_finite()
                || g.radius <= 0.
                || !g.strength.is_finite()
                || !(0. ..=1.).contains(&g.strength)
        })
    {
        return Err("Invalid retopology budget or constraints".into());
    }
    topology::validate(source)?;
    check(cancel)
}

fn prepare_field(
    source: &Surface,
    mut field: field::Field,
    max_vertices: usize,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<(field::Field, integer::Lattice, Diagnostics)> {
    let mut diagnostics = Diagnostics {
        input_vertices: source.positions.len(),
        ..Default::default()
    };
    diagnostics.samples = field.surface.positions.len();
    let mut lattice = integer::Lattice::new(&field, cancel)?;
    diagnostics.position_defects_before = lattice.defects();
    diagnostics.flipped_before = lattice.flipped();
    progress(Stage::Regularity {
        defects: diagnostics.position_defects_before,
    });
    diagnostics.fixed_variables = lattice.correct_field(&field, cancel)?;
    progress(Stage::Subdivision {
        long_edges: lattice.long_edges(),
        fixed_variables: diagnostics.fixed_variables,
    });
    lattice.subdivide(&mut field, max_vertices, cancel)?;
    diagnostics.samples = field.surface.positions.len();
    diagnostics.position_defects_after = lattice.defects();
    progress(Stage::Inversions {
        flipped: lattice.flipped(),
    });
    Ok((field, lattice, diagnostics))
}
fn repair(
    lattice: &mut integer::Lattice,
    diagnostics: &mut Diagnostics,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<()> {
    coarse::repair_lattice(lattice, cancel)?;
    progress(Stage::Repair {
        radius: 0,
        flipped: lattice.flipped(),
    });
    coarse_sat::repair(lattice, cancel, progress)?;
    lattice.remove_flips(cancel, progress)?;
    diagnostics.flipped_after = lattice.flipped();
    Ok(())
}

fn finish(
    source: &Surface,
    mut field: field::Field,
    lattice: integer::Lattice,
    diagnostics: Diagnostics,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<Layout> {
    progress(Stage::Fitting);
    lattice.fit(&mut field, cancel)?;
    progress(Stage::Extraction);
    let mut extracted = lattice.extract(&field, cancel)?;
    progress(Stage::QuadOptimization);
    quad::fix_valence(&mut extracted, cancel)?;
    finish_extracted(
        source,
        &field,
        &lattice,
        extracted,
        diagnostics,
        cancel,
        progress,
    )
}

fn finish_extracted(
    source: &Surface,
    field: &field::Field,
    lattice: &integer::Lattice,
    mut extracted: integer::Extraction,
    mut diagnostics: Diagnostics,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<Layout> {
    let source_topology = topology::validate(source)?;
    let extracted_topology = topology::validate_connectivity(&Surface {
        positions: extracted.positions.clone(),
        triangles: surface::triangulate(&extracted.quads, cancel)?,
    })?;
    if extracted_topology != source_topology {
        return Err("Quad extraction changed a component, boundary, or surface handle".into());
    }
    geometry::fit_dynamic(field, &lattice.edges, &mut extracted, cancel)?;
    let integer::Extraction {
        mut positions,
        quads,
        ..
    } = extracted;
    progress(Stage::SurfaceFit);
    let fit = surface::fit(source, &field.spacing, &mut positions, &quads, cancel)?;
    diagnostics.max_fitting_distance = fit.displacement;
    diagnostics.max_surface_error = fit.error;
    let triangles = fit.triangles;
    let reconstructed = Surface {
        positions: positions.clone(),
        triangles: triangles.clone(),
    };
    if topology::validate(&reconstructed)? != source_topology {
        return Err("Retopology changed a component, boundary, or surface handle".into());
    }
    Ok(Layout {
        positions,
        quads,
        triangles,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn sphere(levels: usize) -> Surface {
        let mut s = Surface {
            positions: vec![Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z],
            triangles: vec![
                [0, 2, 4],
                [2, 1, 4],
                [1, 3, 4],
                [3, 0, 4],
                [2, 0, 5],
                [1, 2, 5],
                [3, 1, 5],
                [0, 3, 5],
            ],
        };
        for _ in 0..levels {
            let mut mid = BTreeMap::new();
            let mut triangles = Vec::new();
            for [a, b, c] in s.triangles {
                let mut midpoint = |u, v| {
                    *mid.entry(topology::edge(u, v)).or_insert_with(|| {
                        let i = s.positions.len();
                        s.positions
                            .push((s.positions[u] + s.positions[v]).normalize());
                        i
                    })
                };
                let ab = midpoint(a, b);
                let bc = midpoint(b, c);
                let ca = midpoint(c, a);
                triangles.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
            }
            s.triangles = triangles;
        }
        s
    }
    fn plane(n: usize) -> Surface {
        let mut surface = Surface {
            positions: vec![],
            triangles: vec![],
        };
        for y in 0..=n {
            for x in 0..=n {
                surface
                    .positions
                    .push(Vec3::new(x as f64 / n as f64, y as f64 / n as f64, 0.));
            }
        }
        for y in 0..n {
            for x in 0..n {
                let a = y * (n + 1) + x;
                let b = a + 1;
                let c = b + n + 1;
                let d = a + n + 1;
                surface.triangles.extend([[a, b, c], [a, c, d]]);
            }
        }
        surface
    }
    #[test]
    fn signed_density_has_more_less_and_neutral_regions_without_a_mask() {
        let s = plane(32);
        let detail = s
            .positions
            .iter()
            .map(|p| {
                if p.x < 0.33 {
                    1.
                } else if p.x > 0.66 {
                    -1.
                } else {
                    0.
                }
            })
            .collect();
        let options = Options {
            target_quads: 100,
            detail,
            preserve_sharp: false,
            ..Options::default()
        };
        let f = field::solve(&s, &options, &|| false, &|_| {}).unwrap();
        let nearest = |x: f64| {
            f.surface
                .positions
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (**a - Vec3::new(x, 0.5, 0.))
                        .length_squared()
                        .total_cmp(&(**b - Vec3::new(x, 0.5, 0.)).length_squared())
                })
                .unwrap()
                .0
        };
        let more = f.spacing[nearest(0.2)];
        let auto = f.spacing[nearest(0.5)];
        let less = f.spacing[nearest(0.8)];
        assert!((auto / more - 2.).abs() < 1e-10);
        assert!((less / auto - 2.).abs() < 1e-10);
        let budget: f64 = f
            .areas
            .iter()
            .zip(&f.spacing)
            .map(|(a, h)| a / (h * h))
            .sum();
        assert!((budget - 100.).abs() < 1e-8);
    }
    #[test]
    fn open_surface_reconstruction_preserves_its_boundary() {
        let source = plane(16);
        let options = Options {
            target_quads: 64,
            preserve_sharp: false,
            ..Options::default()
        };
        let result = reconstruct(&source, &options, &|| false).unwrap();
        assert!((40..=96).contains(&result.quads.len()));
        assert!(result.positions.iter().all(|p| p.z.abs() < 1e-10));
        assert!(result.diagnostics.max_surface_error < 0.05);
    }
    #[test]
    fn signed_density_changes_the_reconstructed_polygon_sizes() {
        let source = plane(24);
        let detail = source
            .positions
            .iter()
            .map(|p| ((0.5 - p.x) * 3.).clamp(-0.8, 0.8))
            .collect();
        let options = Options {
            target_quads: 100,
            detail,
            preserve_sharp: false,
            ..Options::default()
        };
        let result = reconstruct(&source, &options, &|| false).unwrap();
        let mut areas = [Vec::new(), Vec::new()];
        for q in &result.quads {
            let [a, b, c, d] = q.map(|v| result.positions[v]);
            let x = (a.x + b.x + c.x + d.x) * 0.25;
            let area = ((b - a).cross(c - a).length() + (c - a).cross(d - a).length()) * 0.5;
            if x < 0.3 {
                areas[0].push(area);
            } else if x > 0.7 {
                areas[1].push(area);
            }
        }
        assert!(!areas[0].is_empty() && !areas[1].is_empty());
        let average = |a: &Vec<f64>| a.iter().sum::<f64>() / a.len() as f64;
        assert!(
            average(&areas[0]) * 1.5 < average(&areas[1]),
            "more/less painting did not change output areas: {areas:?}"
        );
    }
    #[test]
    fn coplanar_source_refinement_does_not_allocate_nine_times_more_quads() {
        let source = plane(12);
        let mut refined = source.clone();
        for _ in 0..2 {
            let mut triangles = Vec::new();
            for [a, b, c] in refined.triangles {
                let p = (refined.positions[a] + refined.positions[b] + refined.positions[c]) / 3.;
                if p.x < 0.35 {
                    let i = refined.positions.len();
                    refined.positions.push(p);
                    triangles.extend([[a, b, i], [b, c, i], [c, a, i]]);
                } else {
                    triangles.push([a, b, c]);
                }
            }
            refined.triangles = triangles;
        }
        let options = Options {
            target_quads: 64,
            preserve_sharp: false,
            ..Options::default()
        };
        let density = |s: &Surface| {
            let r = reconstruct(s, &options, &|| false).unwrap();
            let count = r
                .quads
                .iter()
                .filter(|q| q.iter().map(|&v| r.positions[v].x).sum::<f64>() / 4. < 0.35)
                .count();
            (r.quads.len(), count)
        };
        let (a, local_a) = density(&source);
        let (b, local_b) = density(&refined);
        assert!(
            a.abs_diff(b) <= a / 4,
            "output budget changed excessively: {a} vs {b}"
        );
        assert!(
            local_a.abs_diff(local_b) <= local_a / 3,
            "local tessellation dictated output density: {local_a} vs {local_b}"
        );
    }
    #[test]
    fn separate_guides_both_influence_the_orientation_field() {
        let s = plane(32);
        let right = Vec3::new(1., 1., 0.).normalize();
        let guide = |x, tangent| Guide {
            position: Vec3::new(x, 0.5, 0.),
            normal: Vec3::Z,
            tangent,
            radius: 0.22,
            strength: 1.,
        };
        let o = Options {
            target_quads: 100,
            guides: vec![guide(0.25, Vec3::X), guide(0.75, right)],
            preserve_sharp: false,
            ..Options::default()
        };
        let f = field::solve(&s, &o, &|| false, &|_| {}).unwrap();
        for (x, desired) in [(0.25, Vec3::X), (0.75, right)] {
            let i = f
                .surface
                .positions
                .iter()
                .position(|p| (*p - Vec3::new(x, 0.5, 0.)).length() < 1e-10)
                .unwrap();
            let q = f.directions[i];
            let alignment = q
                .dot(desired)
                .abs()
                .max(f.normals[i].cross(q).dot(desired).abs());
            assert!(
                alignment > 0.995,
                "guide at {x} was not respected: {alignment}"
            );
        }
    }
    #[cfg(feature = "audit")]
    #[test]
    fn direction_only_audit_preserves_orientation_and_clears_position_hints() {
        let s = sphere(2);
        let mut o = Options {
            target_quads: 100,
            preserve_sharp: false,
            ..Options::default()
        };
        let base = field::solve(&s, &o, &|| false, &|_| {}).unwrap();
        let control = field::solve_direction_guides(&s, &o, &|| false, &|_| {}).unwrap();
        assert_eq!(base.directions, control.directions);
        assert_eq!(base.origins, control.origins);
        let n = base.normals[0];
        let q = n.cross(Vec3::Z).normalize();
        o.guides.push(Guide {
            position: s.positions[0],
            normal: n,
            tangent: q,
            radius: 0.6,
            strength: 1.,
        });
        let full = field::solve(&s, &o, &|| false, &|_| {}).unwrap();
        let direction = field::solve_direction_guides(&s, &o, &|| false, &|_| {}).unwrap();
        assert_eq!(full.directions, direction.directions);
        assert_eq!(base.spacing, direction.spacing);
        assert!(full.constraints.iter().any(|c| c.weight > 0.));
        assert!(direction.constraints.iter().all(|c| c.weight == 0.));
        let aligned = direction.directions[0]
            .dot(q)
            .abs()
            .max(n.cross(direction.directions[0]).dot(q).abs());
        assert!(aligned > 0.98, "direction hint was not followed: {aligned}");
        let error = field::solve_direction_guides(&plane(4), &o, &|| false, &|_| {})
            .err()
            .expect("open boundary should be rejected by this audit control");
        assert!(error.contains("hard boundary/sharp"));
    }
    #[test]
    fn closed_surface_reconstruction_has_no_holes_or_position_defects() {
        let s = sphere(3);
        let o = Options {
            target_quads: 100,
            preserve_sharp: false,
            ..Options::default()
        };
        let r = reconstruct_with_progress(&s, &o, &|| false, &|s| eprintln!("{s:?}")).unwrap();
        eprintln!("{} quads {:?}", r.quads.len(), r.diagnostics);
        assert_eq!(r.diagnostics.position_defects_after, 0);
        assert_eq!(r.diagnostics.flipped_after, 0);
        assert!((40..=180).contains(&r.quads.len()));
        assert!(
            r.positions
                .iter()
                .all(|p| (0.85..1.15).contains(&p.length()))
        );
        for &[a, b, c, d] in &r.quads {
            for [u, v, w] in [[a, b, c], [a, c, d]] {
                let normal =
                    (r.positions[v] - r.positions[u]).cross(r.positions[w] - r.positions[u]);
                assert!(
                    normal.dot(r.positions[u]) > 0.,
                    "reconstructed sphere face points inward"
                );
            }
        }
        let mut edges = BTreeMap::<_, usize>::new();
        for q in r.quads {
            for i in 0..4 {
                *edges
                    .entry(topology::edge(q[i], q[(i + 1) % 4]))
                    .or_default() += 1;
            }
        }
        assert!(edges.values().all(|&n| n == 2));
        assert_eq!(
            r.positions.len() as isize - edges.len() as isize + edges.len() as isize / 2,
            2
        );
    }
    #[test]
    fn asymmetric_curved_surface_reconstruction_preserves_the_sculpt() {
        let mut source = sphere(4);
        for p in &mut source.positions {
            let Vec3 { x, y, z } = *p;
            *p = Vec3::new(
                x * (1. + 0.24 * y),
                y * 1.4,
                z * (0.85 + 0.25 * (-((y - 0.25) / 0.4).powi(2)).exp()),
            );
        }
        let options = Options {
            target_quads: 300,
            preserve_sharp: false,
            ..Options::default()
        };
        let result = reconstruct(&source, &options, &|| false).unwrap();
        assert!((200..=360).contains(&result.quads.len()));
        assert!(result.diagnostics.max_surface_error < 0.05);
        assert_eq!(result.diagnostics.position_defects_after, 0);
        assert_eq!(result.diagnostics.flipped_after, 0);
    }
    #[test]
    fn original_validation_spacing_uses_original_paint_and_vertex_indices() {
        let source = sphere(2);
        let options = Options {
            target_quads: 80,
            preserve_sharp: false,
            detail: source.positions.iter().map(|p| p.y).collect(),
            adaptive: 0.4,
            ..Options::default()
        };
        let field = field::solve(&source, &options, &|| false, &|_| {}).unwrap();
        let spacing = field::source_spacing(&source, &options, &|| false).unwrap();
        assert_eq!(spacing, field.spacing[..source.positions.len()]);
    }

    #[test]
    fn regularized_closed_surface_supports_signed_detail_and_multiple_guides() {
        let source = sphere(3);
        let options = Options {
            target_quads: 180,
            preserve_sharp: false,
            detail: source
                .positions
                .iter()
                .map(|p| {
                    if p.y > 0.3 {
                        0.5
                    } else if p.y < -0.3 {
                        -0.5
                    } else {
                        0.
                    }
                })
                .collect(),
            guides: [Vec3::X, -Vec3::X]
                .into_iter()
                .map(|position| Guide {
                    position,
                    normal: position,
                    tangent: Vec3::Y,
                    radius: 0.5,
                    strength: 0.6,
                })
                .collect(),
            ..Options::default()
        };
        let result =
            reconstruct_regularized_with_progress(&source, &options, &|| false, &|_| {}).unwrap();
        assert_eq!(result.diagnostics.position_defects_after, 0);
        assert_eq!(result.diagnostics.flipped_after, 0);
        let mean_edge = |upper: bool| {
            let lengths: Vec<_> = result
                .quads
                .iter()
                .filter(|q| {
                    let center = q.iter().map(|&v| result.positions[v]).sum::<Vec3>() / 4.;
                    if upper {
                        center.y > 0.4
                    } else {
                        center.y < -0.4
                    }
                })
                .map(|q| {
                    (0..4)
                        .map(|k| result.positions[q[k]].distance(result.positions[q[(k + 1) % 4]]))
                        .sum::<f64>()
                        / 4.
                })
                .collect();
            assert!(lengths.len() > 5);
            lengths.iter().sum::<f64>() / lengths.len() as f64
        };
        assert!(mean_edge(true) < 0.8 * mean_edge(false));
    }

    #[test]
    fn regularized_reconstruction_checks_untouched_source_and_cancels_final_validation() {
        let mut source = sphere(2);
        for p in &mut source.positions {
            p.y *= 1.3;
        }
        let original = source.clone();
        let options = Options {
            target_quads: 80,
            preserve_sharp: false,
            ..Options::default()
        };
        let final_check = std::cell::Cell::new(false);
        let preparation_pass = std::cell::Cell::new(0);
        let result =
            reconstruct_regularized_with_progress(&source, &options, &|| false, &|s| match s {
                Stage::Preparation { iteration, .. } => preparation_pass.set(iteration),
                Stage::OriginalSurfaceValidation => final_check.set(true),
                _ => {}
            })
            .unwrap();
        assert!(final_check.get());
        assert_eq!(preparation_pass.get(), 5);
        assert_eq!(result.diagnostics.input_vertices, source.positions.len());
        assert_eq!(result.diagnostics.flipped_after, 0);
        assert_eq!(source.positions, original.positions);
        assert_eq!(source.triangles, original.triangles);
        let spacing = field::source_spacing(&source, &options, &|| false).unwrap();
        surface::validate_output(
            &original,
            &spacing,
            result.positions,
            &result.quads,
            &|| false,
        )
        .unwrap();
        final_check.set(false);
        let error =
            reconstruct_regularized_with_progress(&source, &options, &|| final_check.get(), &|s| {
                if matches!(s, Stage::OriginalSurfaceValidation) {
                    final_check.set(true);
                }
            })
            .err()
            .unwrap();
        assert_eq!(error, "Retopology cancelled");
    }

    #[test]
    fn cancellation_during_field_work_returns_no_partial_mesh() {
        let calls = std::cell::Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() > 10
        };
        let err = reconstruct(
            &sphere(2),
            &Options {
                target_quads: 80,
                ..Options::default()
            },
            &cancel,
        )
        .err()
        .unwrap();
        assert_eq!(err, "Retopology cancelled");
    }
}
