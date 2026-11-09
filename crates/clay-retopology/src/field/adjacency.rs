// Directed-edge fan order adapted from QuadriFlow (BSD-3-Clause).
// Copyright (c) 2018, Jingwei Huang, Yichao Zhou, Matthias Niessner, Jonathan
// Shewchuk, Leonidas Guibas. Full terms: ../../LICENSE-QuadriFlow.
// Pinned dedge.cpp::compute_direct_graph and adjacent-matrix.cpp.
use crate::{Result, Surface, check, topology};
use std::collections::BTreeMap;

const NONE: usize = usize::MAX;
fn next(h: usize) -> usize {
    h / 3 * 3 + (h + 1) % 3
}
fn prev(h: usize) -> usize {
    h / 3 * 3 + (h + 2) % 3
}

/// The running field average and stable hierarchy matching depend on neighbour
/// order. Follow the surface fan from the lowest halfedge (or first boundary),
/// exactly as the serial reference does; sorting vertex IDs changes the solve.
pub(crate) fn ordered_neighbours(
    surface: &Surface,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<usize>>> {
    let mut first = vec![NONE; surface.positions.len()];
    let mut degree = vec![0; first.len()];
    let mut twins = vec![NONE; surface.triangles.len() * 3];
    let mut edges = BTreeMap::<[usize; 2], usize>::new();
    for (f, t) in surface.triangles.iter().enumerate() {
        if f % 1024 == 0 {
            check(cancel)?;
        }
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let h = f * 3 + k;
            first[a] = first[a].min(h);
            degree[a] += 1;
            if let Some(&other) = edges.get(&topology::edge(a, b)) {
                if twins[other] != NONE || surface.triangles[other / 3][other % 3] != b {
                    return Err("Field adjacency needs consistently oriented manifold edges".into());
                }
                twins[h] = other;
                twins[other] = h;
            } else {
                edges.insert(topology::edge(a, b), h);
            }
        }
    }
    drop(edges);
    let vertex = |h: usize| surface.triangles[h / 3][h % 3];
    let mut result = Vec::with_capacity(first.len());
    for (v, &initial) in first.iter().enumerate() {
        if v % 1024 == 0 {
            check(cancel)?;
        }
        if initial == NONE {
            return Err("Field adjacency has an isolated vertex".into());
        }
        let mut start = initial;
        let mut h = initial;
        // Walk backwards to the first boundary, retaining the minimum edge
        // index for a closed fan. Bound both walks to reject disconnected fans.
        for _ in 0..degree[v] {
            start = start.min(h);
            let opposite = twins[prev(h)];
            if opposite == NONE {
                start = h;
                break;
            }
            h = opposite;
            if h == initial {
                break;
            }
        }
        let mut row = vec![vertex(prev(start))];
        h = start;
        let mut visited = 0;
        for _ in 0..degree[v] {
            visited += 1;
            let opposite = twins[h];
            if opposite == NONE {
                row.push(vertex(next(h)));
                break;
            }
            let following = next(opposite);
            if following == start {
                break;
            }
            row.push(vertex(next(h)));
            h = following;
        }
        if visited != degree[v] {
            return Err("Field adjacency has disconnected vertex fans".into());
        }
        result.push(row);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;

    #[test]
    fn boundary_fan_keeps_both_end_neighbours() {
        let mesh = Surface {
            positions: vec![Vec3::ZERO; 4],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        assert_eq!(
            ordered_neighbours(&mesh, &|| false).unwrap(),
            vec![vec![3, 2, 1], vec![0, 2], vec![1, 0, 3], vec![2, 0]]
        );
        assert!(ordered_neighbours(&mesh, &|| true).is_err());
    }

    #[test]
    fn disconnected_vertex_fans_are_rejected() {
        let mesh = Surface {
            positions: vec![Vec3::ZERO; 5],
            triangles: vec![[0, 1, 2], [0, 3, 4]],
        };
        assert!(ordered_neighbours(&mesh, &|| false).is_err());
    }
}
