//! Offline patch excision/reconstruction. Never marks a failed lattice repaired.
use super::Snapshot;
use crate::{Result, Vec3, integer::Extraction, topology};
use std::collections::{BTreeMap, BTreeSet};

/// Strict simple boundary loops. A pinched or disconnected vertex fan is not
/// silently split: the caller must expand/reconsider its cut instead.
fn boundary_loops(mesh: &Extraction) -> Result<Vec<Vec<usize>>> {
    let graph = crate::quad::connections(&mesh.quads, mesh.positions.len())?;
    let mut outgoing = BTreeMap::new();
    let mut incoming = BTreeSet::new();
    for (h, &twin) in graph.twins.iter().enumerate() {
        if twin != crate::quad::NONE {
            continue;
        }
        let q = mesh.quads[h / 4];
        let a = q[h % 4];
        let b = q[(h + 1) % 4];
        if outgoing.insert(a, b).is_some() || !incoming.insert(b) {
            return Err("Patch boundary branches or touches itself".into());
        }
    }
    if outgoing.keys().copied().collect::<BTreeSet<_>>() != incoming {
        return Err("Patch boundary is not closed".into());
    }
    let mut loops = Vec::new();
    while let Some((&start, _)) = outgoing.first_key_value() {
        let mut ring = Vec::new();
        let mut v = start;
        loop {
            ring.push(v);
            v = outgoing
                .remove(&v)
                .ok_or("Patch boundary joins another loop")?;
            if v == start {
                break;
            }
        }
        if ring.len() < 3 {
            return Err("Patch boundary has fewer than three vertices".into());
        }
        loops.push(ring);
    }
    Ok(loops)
}

fn compact(mesh: &mut Extraction) {
    let used: BTreeSet<_> = mesh.quads.iter().flatten().copied().collect();
    let mut ids = vec![usize::MAX; mesh.positions.len()];
    let mut positions = Vec::new();
    let mut samples = Vec::new();
    for v in used {
        ids[v] = positions.len();
        positions.push(mesh.positions[v]);
        samples.push(std::mem::take(&mut mesh.samples[v]));
    }
    for q in &mut mesh.quads {
        *q = q.map(|v| ids[v]);
    }
    mesh.positions = positions;
    mesh.samples = samples;
}

impl Snapshot {
    pub fn rebuild_patches(
        mut self,
        halo: usize,
        fill: bool,
        seconds: f64,
        placement: &str,
    ) -> Result<serde_json::Value> {
        use std::time::{Duration, Instant};
        if !seconds.is_finite() || seconds <= 0. || seconds > 120. {
            return Err("Patch audit budget must be in (0,120]".into());
        }
        if !["dynamic", "source"].contains(&placement) {
            return Err("Unknown patch placement experiment".into());
        }
        let start = Instant::now();
        let deadline = start + Duration::from_secs_f64(seconds);
        let cancel = || Instant::now() >= deadline;
        // Match normal extraction ordering: fit sample origins before using
        // their averaged positions to choose a patch layout.
        if fill && placement == "dynamic" {
            self.lattice.fit(&mut self.field, &cancel)?;
        }
        let (mut mesh, normals, cut) =
            crate::extraction::cut_inversions(&self.lattice, &self.field, halo, &cancel)?;
        if placement == "source" {
            // Same connectivity, initialized from source memberships instead
            // of allowing the failed grid's position solve to displace it.
            for (p, samples) in mesh.positions.iter_mut().zip(&mesh.samples) {
                if samples.is_empty() {
                    return Err("Patch vertex has no source correspondence".into());
                }
                *p = samples
                    .iter()
                    .map(|&v| self.field.surface.positions[v])
                    .sum::<Vec3>()
                    / samples.len() as f64;
            }
        }
        let loops = boundary_loops(&mesh);
        let boundary_error = loops.as_ref().err().cloned();
        let rings = loops.unwrap_or_default();
        let boundaries:Vec<_>=rings.iter().map(|ring| {
            let center=ring.iter().map(|&v|mesh.positions[v]).sum::<Vec3>()/ring.len() as f64;
            serde_json::json!({"vertices":ring,"positions":ring.iter().map(|&v|mesh.positions[v].to_array()).collect::<Vec<_>>(),"length":ring.len(),"even":ring.len()%2==0,"center":center.to_array(),"perimeter":(0..ring.len()).map(|i|mesh.positions[ring[i]].distance(mesh.positions[ring[(i+1)%ring.len()]])).sum::<f64>()})
        }).collect();
        let before_quads = mesh.quads.len();
        let mut fill_error = None;
        let mut accepted = false;
        let mut acceptance_error = None;
        let mut candidate_stage = "cut";
        let mut added_patch_quads = 0;
        let mut cleanup_delta = 0isize;
        if fill && boundary_error.is_none() {
            // A bounded extension of QuadriFlow's fill is only attempted after
            // a clean-boundary check. Larger/odd loops need another construction.
            if rings.iter().any(|r| r.len() % 2 != 0 || r.len() > 32) {
                fill_error =
                    Some("Boundary exceeds the existing even/small-hole construction".into());
            } else {
                let old_quads = mesh.quads.clone();
                if let Err(e) =
                    crate::extraction::fill_quad_patches(&mut mesh, &normals, &rings, &cancel)
                {
                    mesh.quads = old_quads;
                    fill_error = Some(e);
                }
            }
            if fill_error.is_none() {
                added_patch_quads = mesh.quads.len() - before_quads;
                candidate_stage = "filled";
                compact(&mut mesh);
                let count = mesh.quads.len();
                if let Err(e) = crate::quad::fix_valence(&mut mesh, &cancel) {
                    acceptance_error = Some(e);
                }
                cleanup_delta = mesh.quads.len() as isize - count as isize;
                let topology =
                    crate::surface::triangulate(&mesh.quads, &cancel).and_then(|triangles| {
                        topology::validate_connectivity(&crate::Surface {
                            positions: mesh.positions.clone(),
                            triangles,
                        })
                    });
                let source_topology = topology::validate(&self.source)?;
                if acceptance_error.is_none()
                    && topology.as_ref().is_ok_and(|t| *t == source_topology)
                {
                    let fit = {
                        if placement == "source" {
                            Ok(())
                        } else {
                            crate::geometry::fit_dynamic(
                                &self.field,
                                &self.lattice.edges,
                                &mut mesh,
                                &cancel,
                            )
                        }
                    }
                    .and_then(|_| {
                        candidate_stage = "before_surface_projection";
                        let before = mesh.positions.clone();
                        let result = crate::surface::fit(
                            &self.source,
                            &self.field.spacing,
                            &mut mesh.positions,
                            &mesh.quads,
                            &cancel,
                        );
                        if result.is_err() {
                            mesh.positions = before;
                        }
                        result
                    });
                    match fit {
                        Ok(result) => {
                            candidate_stage = "projected";
                            let output = crate::Surface {
                                positions: mesh.positions.clone(),
                                triangles: result.triangles,
                            };
                            match topology::validate(&output) {
                                Ok(t) if t == source_topology => accepted = true,
                                Ok(_) => {
                                    acceptance_error =
                                        Some("Patch output changed source topology".into())
                                }
                                Err(e) => acceptance_error = Some(e),
                            }
                        }
                        Err(e) => acceptance_error = Some(e),
                    }
                } else if acceptance_error.is_none() {
                    acceptance_error =
                        Some(topology.err().unwrap_or(
                            "Patch fill changed a component, boundary or handle".into(),
                        ));
                }
            }
        }
        Ok(
            serde_json::json!({"audit_only":true,"placement":placement,"accepted":accepted,"validation_scope":{"accepted_means":"Passes existing topology, orientation and sampled surface-distance checks","self_intersections":"not_tested"},"original_lattice_repaired":self.repaired,"remaining_lattice_inversions":self.lattice.flipped(),"cut":cut,"boundary_error":boundary_error,"boundaries":boundaries,"fill_requested":fill,"fill_error":fill_error,"acceptance_error":acceptance_error,"added_quads":added_patch_quads,"cleanup_quad_delta":cleanup_delta,"candidate_stage":candidate_stage,"positions":mesh.positions.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"quads":mesh.quads,"seconds":start.elapsed().as_secs_f64(),"note":"Diagnostic candidate, not app output. Original source and integer offsets unchanged. Boundary vertex IDs refer to the pre-compaction cut mesh. Failed surface projection is rolled back."}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_tube(n: usize) -> Extraction {
        let positions = (0..2 * n)
            .map(|v| {
                let angle = (v % n) as f64 * std::f64::consts::TAU / n as f64;
                Vec3::new(angle.cos(), angle.sin(), (v / n) as f64)
            })
            .collect();
        Extraction {
            positions,
            quads: (0..n)
                .map(|i| [i, (i + 1) % n, (i + 1) % n + n, i + n])
                .collect(),
            samples: (0..2 * n).map(|i| vec![i]).collect(),
        }
    }

    #[test]
    fn quad_patch_fill_closes_28_vertex_loops_without_changing_retained_faces() {
        let mut mesh = open_tube(28);
        let positions = mesh.positions.clone();
        let retained = mesh.quads.clone();
        let rings = boundary_loops(&mesh).unwrap();
        assert_eq!(rings.iter().map(Vec::len).collect::<Vec<_>>(), vec![28, 28]);
        let normals = (0..56)
            .map(|v| if v < 28 { -Vec3::Z } else { Vec3::Z })
            .collect::<Vec<_>>();
        crate::extraction::fill_quad_patches(&mut mesh, &normals, &rings, &|| false).unwrap();
        assert_eq!(mesh.positions, positions);
        assert_eq!(&mesh.quads[..28], &retained);
        assert_eq!(mesh.quads.len(), 54);
        assert!(boundary_loops(&mesh).unwrap().is_empty());
        topology::validate(&crate::Surface {
            positions: mesh.positions,
            triangles: crate::surface::triangulate(&mesh.quads, &|| false).unwrap(),
        })
        .unwrap();
    }

    #[test]
    fn quad_patch_fill_rejects_odd_loops_and_cancels_without_partial_commit() {
        for n in [5, 28] {
            let mut mesh = open_tube(n);
            let before = mesh.quads.clone();
            let rings = boundary_loops(&mesh).unwrap();
            let normals = vec![Vec3::Z; mesh.positions.len()];
            let error =
                crate::extraction::fill_quad_patches(&mut mesh, &normals, &rings, &|| n == 28)
                    .unwrap_err();
            assert!(if n == 5 {
                error.contains("even")
            } else {
                error.contains("cancelled")
            });
            assert_eq!(mesh.quads, before);
        }
    }

    #[test]
    fn patch_boundary_rejects_pinched_vertex_fans() {
        let mesh = Extraction {
            positions: vec![Vec3::ZERO; 7],
            samples: vec![vec![]; 7],
            quads: vec![[0, 1, 2, 3], [0, 4, 5, 6]],
        };
        assert!(boundary_loops(&mesh).unwrap_err().contains("Disconnected"));
    }
}
