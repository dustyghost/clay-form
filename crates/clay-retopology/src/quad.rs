//! QuadriFlow FixValence adaptation (pinned provenance in geometry.rs).
//! Boundary fans are retained; closed fans remove valence two and split valence
//! greater than five. Source memberships survive every vertex split.
use crate::{Result, check, integer::Extraction, topology};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) const NONE: usize = usize::MAX;
pub(crate) fn next(h: usize) -> usize {
    h / 4 * 4 + (h + 1) % 4
}
pub(crate) fn prev(h: usize) -> usize {
    h / 4 * 4 + (h + 3) % 4
}
pub(crate) fn vertex(quads: &[[usize; 4]], h: usize) -> usize {
    quads[h / 4][h % 4]
}
pub(crate) struct Connections {
    pub twins: Vec<usize>,
    pub rings: Vec<Vec<usize>>,
    pub boundary: Vec<bool>,
}
pub(crate) fn connections(quads: &[[usize; 4]], n: usize) -> Result<Connections> {
    let mut directed = BTreeMap::new();
    let mut incident = vec![Vec::new(); n];
    for (i, q) in quads.iter().enumerate() {
        if q.iter().any(|&v| v >= n) || q.iter().copied().collect::<BTreeSet<_>>().len() != 4 {
            return Err("Invalid quad during geometry optimization".into());
        }
        for k in 0..4 {
            let h = i * 4 + k;
            if directed.insert((q[k], q[(k + 1) % 4]), h).is_some() {
                return Err("Repeated directed quad edge during geometry optimization".into());
            }
            incident[q[k]].push(h);
        }
    }
    let mut twins = vec![NONE; quads.len() * 4];
    for (&(a, b), &h) in &directed {
        if let Some(&t) = directed.get(&(b, a)) {
            twins[h] = t;
        }
    }
    let mut boundary = vec![false; n];
    let mut rings = vec![Vec::new(); n];
    for v in 0..n {
        if incident[v].is_empty() {
            continue;
        }
        let start = incident[v]
            .iter()
            .copied()
            .find(|&h| twins[h] == NONE)
            .unwrap_or(incident[v][0]);
        boundary[v] = twins[start] == NONE;
        let mut h = start;
        loop {
            rings[v].push(h);
            h = twins[prev(h)];
            if h == NONE || h == start {
                break;
            }
            if rings[v].len() >= incident[v].len() {
                return Err("Quad vertex fan does not close".into());
            }
        }
        if rings[v].len() != incident[v].len() {
            return Err("Disconnected quad vertex fan".into());
        }
    }
    Ok(Connections {
        twins,
        rings,
        boundary,
    })
}

pub(crate) fn fix_valence(mesh: &mut Extraction, cancel: &dyn Fn() -> bool) -> Result<()> {
    // Upstream erases the two incident quads and joins their four outer sides.
    for _ in 0..mesh.quads.len() {
        check(cancel)?;
        let graph = connections(&mesh.quads, mesh.positions.len())?;
        let mut marked = vec![false; mesh.positions.len()];
        let mut erased = vec![false; mesh.quads.len()];
        let mut changed = false;
        for (v, ring) in graph.rings.iter().enumerate() {
            if marked[v] || graph.boundary[v] || ring.len() != 2 {
                continue;
            }
            let [a, b] = [ring[0], ring[1]];
            let outer = [
                vertex(&mesh.quads, next(a)),
                vertex(&mesh.quads, next(next(a))),
                vertex(&mesh.quads, next(b)),
                vertex(&mesh.quads, next(next(b))),
            ];
            if outer.iter().any(|&i| marked[i]) {
                continue;
            }
            for &i in &outer {
                marked[i] = true;
            }
            if outer.iter().copied().collect::<BTreeSet<_>>().len() != 4 {
                // Upstream removes both faces of a collapsed two-quad bubble.
                // Retaining it creates an impossible surface component and a
                // singular position system when both sides reach one plane.
                erased[a / 4] = true;
            } else {
                mesh.quads[a / 4] = outer;
            }
            erased[b / 4] = true;
            changed = true;
        }
        if !changed {
            break;
        }
        mesh.quads = mesh
            .quads
            .iter()
            .enumerate()
            .filter(|(i, _)| !erased[*i])
            .map(|(_, q)| *q)
            .collect();
    }
    let budget = mesh.positions.len() * 2;
    loop {
        check(cancel)?;
        let graph = connections(&mesh.quads, mesh.positions.len())?;
        let valence: Vec<_> = graph
            .rings
            .iter()
            .enumerate()
            .map(|(v, r)| r.len() + usize::from(graph.boundary[v]))
            .collect();
        let mut queue: Vec<_> = (0..valence.len())
            .filter(|&v| valence[v] > 5 && !graph.boundary[v])
            .collect();
        queue.sort_by_key(|&v| std::cmp::Reverse((valence[v], v)));
        let mut marked = vec![false; mesh.positions.len()];
        let mut changed = false;
        for v in queue {
            if marked[v] {
                continue;
            }
            let ring: Vec<_> = std::iter::once(graph.rings[v][0])
                .chain(graph.rings[v][1..].iter().rev().copied())
                .collect();
            let neighbours: Vec<_> = ring.iter().map(|&h| vertex(&mesh.quads, next(h))).collect();
            if neighbours.iter().any(|&u| marked[u]) {
                continue;
            }
            let step = ring.len().div_ceil(2);
            let candidate = (0..ring.len() - step)
                .map(|i| {
                    let a = valence[neighbours[i]];
                    let b = valence[neighbours[i + step]];
                    ((a.max(b), a.min(b)), i + 1)
                })
                .min();
            let Some(((largest, _), start)) = candidate else {
                continue;
            };
            if largest >= valence[v] {
                continue;
            }
            let new = mesh.positions.len();
            for &h in &ring[start..start + step] {
                mesh.quads[h / 4][h % 4] = new;
            }
            mesh.quads
                .push([new, neighbours[start - 1], v, neighbours[start + step - 1]]);
            mesh.positions.push(mesh.positions[v]);
            mesh.samples.push(mesh.samples[v].clone());
            marked[v] = true;
            for u in neighbours {
                marked[u] = true;
            }
            changed = true;
        }
        if mesh.positions.len() > budget {
            return Err("Quad valence cleanup exceeded its vertex budget".into());
        }
        if !changed {
            break;
        }
    }
    let mut used = vec![false; mesh.positions.len()];
    for &v in mesh.quads.iter().flatten() {
        used[v] = true;
    }
    let mut remap = vec![NONE; used.len()];
    let mut positions = Vec::new();
    let mut samples = Vec::new();
    for (v, &keep) in used.iter().enumerate() {
        if keep {
            remap[v] = positions.len();
            positions.push(mesh.positions[v]);
            samples.push(mesh.samples[v].clone());
        }
    }
    for v in mesh.quads.iter_mut().flatten() {
        *v = remap[*v];
    }
    mesh.positions = positions;
    mesh.samples = samples;
    connections(&mesh.quads, mesh.positions.len())?;
    let triangles = crate::surface::triangulate(&mesh.quads, cancel)?;
    topology::validate_connectivity(&crate::Surface {
        positions: mesh.positions.clone(),
        triangles,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Surface, Vec3};
    fn top(mesh: &Extraction) -> topology::Topology {
        topology::validate_connectivity(&Surface {
            positions: mesh.positions.clone(),
            triangles: crate::surface::triangulate(&mesh.quads, &|| false).unwrap(),
        })
        .unwrap()
    }
    #[test]
    fn valence_two_cleanup_rejoins_a_split_cube_face() {
        let mut mesh = Extraction {
            positions: (0..9).map(|i| Vec3::new(i as f64, 0., 0.)).collect(),
            samples: (0..9).map(|i| vec![i]).collect(),
            quads: vec![
                [0, 3, 2, 8],
                [0, 8, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [1, 2, 6, 5],
                [2, 3, 7, 6],
                [3, 0, 4, 7],
            ],
        };
        let before = top(&mesh);
        fix_valence(&mut mesh, &|| false).unwrap();
        assert_eq!(top(&mesh), before);
        assert_eq!(mesh.quads.len(), 6);
        assert_eq!(mesh.positions.len(), 8);
        assert_eq!(mesh.samples.len(), 8);
    }
    #[test]
    fn collapsed_two_quad_bubble_is_removed_without_touching_a_valid_component() {
        let mut mesh = Extraction {
            positions: (0..12).map(|i| Vec3::new(i as f64, 0., 0.)).collect(),
            samples: (0..12).map(|i| vec![i]).collect(),
            quads: vec![
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [1, 2, 6, 5],
                [2, 3, 7, 6],
                [3, 0, 4, 7],
                [8, 9, 10, 11],
                [8, 11, 10, 9],
            ],
        };
        fix_valence(&mut mesh, &|| false).unwrap();
        assert_eq!(mesh.quads.len(), 6);
        assert_eq!(mesh.positions.len(), 8);
        assert!(mesh.samples.iter().all(|s| s[0] < 8));
        top(&mesh);
    }
    #[test]
    fn high_valence_split_preserves_closed_surface_and_source_membership() {
        let mut mesh = Extraction {
            positions: vec![Vec3::Z, -Vec3::Z],
            quads: vec![],
            samples: vec![],
        };
        for i in 0..6 {
            let angle = i as f64 * std::f64::consts::TAU / 6.;
            mesh.positions.push(Vec3::new(angle.cos(), angle.sin(), 0.));
        }
        let mut midpoint = BTreeMap::new();
        for i in 0..6 {
            let a = 2 + i;
            let b = 2 + (i + 1) % 6;
            for t in [[0, a, b], [1, b, a]] {
                let c = mesh.positions.len();
                mesh.positions
                    .push(t.iter().map(|&v| mesh.positions[v]).sum::<Vec3>() / 3.);
                let mut mids = [0; 3];
                for k in 0..3 {
                    mids[k] = *midpoint
                        .entry(topology::edge(t[k], t[(k + 1) % 3]))
                        .or_insert_with(|| {
                            let v = mesh.positions.len();
                            mesh.positions.push(
                                (mesh.positions[t[k]] + mesh.positions[t[(k + 1) % 3]]) * 0.5,
                            );
                            v
                        });
                }
                for k in 0..3 {
                    mesh.quads.push([t[k], mids[k], c, mids[(k + 2) % 3]]);
                }
            }
        }
        mesh.samples = (0..mesh.positions.len()).map(|i| vec![i]).collect();
        let before = top(&mesh);
        let original = mesh.positions.len();
        fix_valence(&mut mesh, &|| false).unwrap();
        assert_eq!(top(&mesh), before);
        assert_eq!(mesh.positions.len(), original + 2);
        assert!(
            connections(&mesh.quads, mesh.positions.len())
                .unwrap()
                .rings
                .iter()
                .all(|r| (3..=5).contains(&r.len()))
        );
        assert_eq!(
            mesh.samples.iter().filter(|s| s.as_slice() == [0]).count(),
            2
        );
        assert_eq!(
            mesh.samples.iter().filter(|s| s.as_slice() == [1]).count(),
            2
        );
    }
}
