//! Coarse SAT patches using QuadriFlow's connected-fan marking idea and the
//! existing Rust bounded SAT encoding. Not a port of its external SAT process.
#[cfg(feature = "audit")]
use crate::sat::Rule;
use crate::{
    Result, check,
    coarse::{Graph, NONE, rot},
    integer::Lattice,
};
use glam::IVec2;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
fn encode(d: IVec2) -> u8 {
    ((d.x + 1) * 3 + d.y + 1) as u8
}
fn decode(v: u8) -> IVec2 {
    IVec2::new(i32::from(v / 3) - 1, i32::from(v % 3) - 1)
}

#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub(crate) struct Checkpoint {
    pub depth: usize,
    pub next_radius: usize,
    pub graph: Graph,
}

#[cfg_attr(feature = "audit", derive(serde::Serialize))]
#[cfg_attr(not(feature = "audit"), allow(dead_code))]
pub(crate) struct PatchReport {
    pub phase: &'static str,
    pub depth: usize,
    pub radius: usize,
    pub faces: usize,
    pub flexible_edges: usize,
    pub fixed_boundary_edges: usize,
    pub locked_coordinates: usize,
    pub inverted_source_faces_before: Vec<usize>,
    pub inverted_source_faces_after: Vec<usize>,
    pub sat: Option<crate::sat::Statistics>,
}

pub(crate) trait Observer {
    fn checkpoint(&mut self, _depth: usize, _next_radius: usize, _graph: &Graph) -> Result<()> {
        Ok(())
    }
    fn patch(&mut self, _report: &PatchReport) -> Result<()> {
        Ok(())
    }
    /// Returning true captures a problem without solving it (offline audit).
    fn problem(
        &mut self,
        _report: &PatchReport,
        _graph: &Graph,
        _faces: &BTreeSet<usize>,
        _edges: &BTreeSet<usize>,
        _flexible: &BTreeSet<usize>,
    ) -> Result<bool> {
        Ok(false)
    }
}
struct Silent;
impl Observer for Silent {}

pub(crate) fn repair(
    lattice: &mut Lattice,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(crate::Stage),
) -> Result<()> {
    repair_observed(lattice, cancel, progress, &mut Silent, None)
}

pub(crate) fn repair_observed(
    lattice: &mut Lattice,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(crate::Stage),
    observer: &mut impl Observer,
    resume: Option<Checkpoint>,
) -> Result<()> {
    if lattice.flipped() == 0 {
        return Ok(());
    }
    let mut levels = vec![Graph::from_lattice(lattice)?];
    let mut maps = Vec::new();
    for _ in 0..99 {
        let (coarse, map) = levels.last().unwrap().coarsen(cancel)?;
        if coarse.offsets.len() == levels.last().unwrap().offsets.len() {
            break;
        }
        levels.push(coarse);
        maps.push(map);
    }
    let mut next_radius = 1;
    if let Some(saved) = resume {
        let Some(original) = levels.get(saved.depth) else {
            return Err("Invalid repair checkpoint depth".into());
        };
        if !(1..=5).contains(&saved.next_radius)
            || original.faces != saved.graph.faces
            || original.source_faces != saved.graph.source_faces
            || original.allow != saved.graph.allow
            || original.edge_faces != saved.graph.edge_faces
            || original.offsets.len() != saved.graph.offsets.len()
            || original.rotations.len() != saved.graph.rotations.len()
        {
            return Err("Repair checkpoint does not match the frozen hierarchy".into());
        }
        next_radius = saved.next_radius;
        maps.truncate(saved.depth);
        levels.truncate(saved.depth + 1);
        levels[saved.depth] = saved.graph;
    }
    while let Some(graph) = levels.last_mut() {
        let depth = maps.len();
        if depth <= 5 {
            observer.checkpoint(depth, next_radius, graph)?;
            for radius in next_radius..=4 {
                check(cancel)?;
                let flipped = (0..graph.faces.len())
                    .filter(|&f| graph.area(f) < 0)
                    .count();
                if flipped == 0 {
                    break;
                }
                progress(crate::Stage::CoarseRepair { radius, flipped });
                solve_patches(graph, depth, radius, cancel, observer)?;
                observer.checkpoint(depth, radius + 1, graph)?;
            }
        }
        next_radius = 1;
        let Some(map) = maps.pop() else {
            break;
        };
        let coarse = levels.pop().unwrap();
        let fine = levels.last_mut().unwrap();
        for (e, &upper) in map.edges.iter().enumerate() {
            fine.offsets[e] = if upper < 0 {
                IVec2::ZERO
            } else {
                rot(coarse.offsets[upper as usize], -map.rotations[e])
            };
        }
        for (f, &upper) in map.faces.iter().enumerate() {
            if upper != NONE {
                fine.rotations[f] = std::array::from_fn(|k| {
                    (coarse.rotations[upper][k] + map.rotations[fine.faces[f][k]]).rem_euclid(4)
                });
            }
        }
    }
    let graph = levels.pop().unwrap();
    for (e, d) in lattice.edges.iter_mut().zip(graph.offsets) {
        e.d = d;
    }
    for (f, r) in lattice.faces.iter_mut().zip(graph.rotations) {
        f.rotations = r;
    }
    if lattice.defects() != 0 {
        return Err("Coarse SAT changed a regularity equation".into());
    }
    Ok(())
}

/// Controlled audit of upstream's radius-outer schedule. Rebuild the zero-edge
/// contraction hierarchy from the committed fine graph between widening passes.
/// No coordinate locks, source geometry or face orientation constraints change.
#[cfg(feature = "audit")]
pub(crate) fn repair_rebuilt_observed(
    lattice: &mut Lattice,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(crate::Stage),
    observer: &mut impl Observer,
) -> Result<()> {
    for radius in 1..=4 {
        check(cancel)?;
        if lattice.flipped() == 0 {
            break;
        }
        let mut levels = vec![Graph::from_lattice(lattice)?];
        let mut maps = Vec::new();
        for _ in 0..5 {
            let (coarse, map) = levels.last().unwrap().coarsen(cancel)?;
            if coarse.offsets.len() == levels.last().unwrap().offsets.len() {
                break;
            }
            levels.push(coarse);
            maps.push(map);
        }
        loop {
            let graph = levels.last_mut().unwrap();
            let depth = maps.len();
            let flipped = (0..graph.faces.len())
                .filter(|&f| graph.area(f) < 0)
                .count();
            if flipped != 0 {
                progress(crate::Stage::CoarseRepair { radius, flipped });
                solve_patches(graph, depth, radius, cancel, observer)?;
            }
            observer.checkpoint(depth, radius + 1, graph)?;
            let Some(map) = maps.pop() else { break };
            let coarse = levels.pop().unwrap();
            let fine = levels.last_mut().unwrap();
            for (e, &upper) in map.edges.iter().enumerate() {
                fine.offsets[e] = if upper < 0 {
                    IVec2::ZERO
                } else {
                    rot(coarse.offsets[upper as usize], -map.rotations[e])
                };
            }
            for (f, &upper) in map.faces.iter().enumerate() {
                if upper != NONE {
                    fine.rotations[f] = std::array::from_fn(|k| {
                        (coarse.rotations[upper][k] + map.rotations[fine.faces[f][k]]).rem_euclid(4)
                    });
                }
            }
        }
        let graph = levels.pop().unwrap();
        for (e, d) in lattice.edges.iter().zip(&graph.offsets) {
            if let Some(axis) = e.fixed_axis
                && e.d[axis] != d[axis]
            {
                return Err("Rebuilt coarse repair changed a locked coordinate".into());
            }
        }
        for (e, d) in lattice.edges.iter_mut().zip(graph.offsets) {
            e.d = d;
        }
        for (f, r) in lattice.faces.iter_mut().zip(graph.rotations) {
            f.rotations = r;
        }
        if lattice.defects() != 0 {
            return Err("Rebuilt coarse repair changed a regularity equation".into());
        }
    }
    Ok(())
}

pub(crate) fn solve_patches(
    graph: &mut Graph,
    depth: usize,
    radius: usize,
    cancel: &dyn Fn() -> bool,
    observer: &mut impl Observer,
) -> Result<()> {
    let twins = graph.halfedges()?;
    let next = |h: usize| h / 3 * 3 + (h + 1) % 3;
    let prev = |h: usize| h / 3 * 3 + (h + 2) % 3;
    let edge = |h: usize| graph.faces[h / 3][h % 3];
    let mut marked = vec![false; twins.len()];
    let mut queue = VecDeque::new();
    for f in 0..graph.faces.len() {
        if graph.area(f) < 0 {
            for (k, mark) in marked[f * 3..f * 3 + 3].iter_mut().enumerate() {
                *mark = true;
                queue.push_back((f * 3 + k, 0));
            }
        }
    }
    // Mark complete vertex fans and cross edges by their integer L1 length;
    // source sampling density does not define the search radius.
    while let Some((start, depth)) = queue.pop_front() {
        check(cancel)?;
        let mut fan = vec![start];
        let mut h = start;
        for _ in 0..twins.len() {
            let t = twins[h];
            if t == NONE {
                break;
            }
            h = next(t);
            if h == start {
                break;
            }
            fan.push(h);
        }
        if twins[h] == NONE {
            h = start;
            for _ in 0..twins.len() {
                let t = twins[prev(h)];
                if t == NONE {
                    break;
                }
                h = t;
                if h == start {
                    break;
                }
                fan.push(h);
            }
        }
        for &h in &fan {
            marked[h] = true;
        }
        for h in fan {
            let t = twins[h];
            if t == NONE {
                continue;
            }
            let length = graph.offsets[edge(t)].abs().element_sum() as usize;
            if depth + length <= radius && !marked[t] {
                marked[t] = true;
                if length == 0 {
                    queue.push_front((t, depth));
                } else {
                    queue.push_back((t, depth + length));
                }
            }
        }
    }
    let mut flexible = vec![false; graph.offsets.len()];
    for (h, &mark) in marked.iter().enumerate() {
        if mark {
            flexible[edge(h)] = true;
        }
    }
    for (e, flex) in flexible.iter_mut().enumerate() {
        if graph.edge_faces[e][0] == graph.edge_faces[e][1] || graph.allow[e] == [0, 0] {
            *flex = false;
        }
    }
    let mut visited = vec![false; flexible.len()];
    for start in 0..flexible.len() {
        if !flexible[start] || visited[start] {
            continue;
        }
        let mut component = BTreeSet::new();
        let mut faces = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        while let Some(e) = queue.pop_front() {
            component.insert(e);
            for f in graph.edge_faces[e] {
                if f == NONE {
                    continue;
                }
                faces.insert(f);
                for &other in &graph.faces[f] {
                    if flexible[other] && !visited[other] {
                        visited[other] = true;
                        queue.push_back(other);
                    }
                }
            }
        }
        if faces.iter().all(|&f| graph.area(f) >= 0) {
            continue;
        }
        let mut report = PatchReport {
            phase: "start",
            depth,
            radius,
            faces: faces.len(),
            flexible_edges: component.len(),
            fixed_boundary_edges: 0,
            locked_coordinates: component
                .iter()
                .map(|&e| graph.allow[e].iter().filter(|&&v| v == 0).count())
                .sum(),
            inverted_source_faces_before: faces
                .iter()
                .filter(|&&f| graph.area(f) < 0)
                .map(|&f| graph.source_faces[f])
                .collect(),
            inverted_source_faces_after: Vec::new(),
            sat: None,
        };
        if faces.len() > 32_000 || component.len() > 120_000 / 9 {
            report.phase = "patch_size_limit";
            observer.patch(&report)?;
            continue;
        }
        let edges: BTreeSet<_> = faces.iter().flat_map(|&f| graph.faces[f]).collect();
        report.fixed_boundary_edges = edges.len() - component.len();
        observer.patch(&report)?;
        if observer.problem(&report, graph, &faces, &edges, &component)? {
            continue;
        }
        let ids: BTreeMap<_, _> = edges.iter().enumerate().map(|(i, &e)| (e, i)).collect();
        let preferred: Vec<_> = edges.iter().map(|&e| encode(graph.offsets[e])).collect();
        let domains: Vec<u16> = edges
            .iter()
            .map(|&e| {
                if !component.contains(&e) {
                    return 1 << encode(graph.offsets[e]);
                }
                (0..9)
                    .filter(|&v| {
                        (0..2)
                            .all(|k| graph.allow[e][k] != 0 || decode(v)[k] == graph.offsets[e][k])
                    })
                    .fold(0, |d, v| d | (1 << v))
            })
            .collect();
        let local_faces: Vec<_> = faces
            .iter()
            .map(|&f| graph.faces[f].map(|e| ids[&e]))
            .collect();
        let local_rotations: Vec<_> = faces.iter().map(|&f| graph.rotations[f]).collect();
        let attempt = crate::sat::solve_grid(
            &domains,
            &local_faces,
            &local_rotations,
            &preferred,
            cancel,
            2000,
        )?;
        if let Some(solution) = attempt.solution {
            for (i, &e) in edges.iter().enumerate() {
                graph.offsets[e] = decode(solution[i].trailing_zeros() as u8);
            }
        }
        report.phase = "finish";
        report.sat = Some(attempt.stats);
        report.inverted_source_faces_after = faces
            .iter()
            .filter(|&&f| graph.area(f) < 0)
            .map(|&f| graph.source_faces[f])
            .collect();
        observer.patch(&report)?;
    }
    Ok(())
}

#[cfg(feature = "audit")]
pub(crate) fn face_rule(vars: [usize; 3], rotations: [i32; 3]) -> Rule {
    let mut tuples = Vec::new();
    for a in 0..9 {
        for b in 0..9 {
            let x = rot(decode(a), rotations[0]);
            let y = rot(decode(b), rotations[1]);
            let z = -x - y;
            if z.abs().max_element() <= 1 && x.x * y.y - x.y * y.x >= 0 {
                tuples.push([a, b, encode(rot(z, -rotations[2]))]);
            }
        }
    }
    Rule { vars, tuples }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Vec3,
        field::{Constraint, Field},
    };
    fn folded() -> Lattice {
        let p = vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y];
        let f = Field {
            surface: crate::Surface {
                positions: p.clone(),
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            },
            normals: vec![Vec3::Z; 4],
            directions: vec![Vec3::X; 4],
            origins: p,
            spacing: vec![1.; 4],
            areas: vec![1.; 4],
            constraints: vec![Constraint::default(); 4],
        };
        let mut l = Lattice::new(&f, &|| false).unwrap();
        let coordinates = [IVec2::ZERO, IVec2::X, -IVec2::Y, -IVec2::X];
        for e in &mut l.edges {
            e.d = coordinates[e.ends[1]] - coordinates[e.ends[0]];
        }
        assert_eq!(l.defects(), 0);
        assert_eq!(l.flipped(), 2);
        l
    }
    struct StopAfterPass(Option<Checkpoint>);
    #[cfg(feature = "audit")]
    #[test]
    fn rebuilt_schedule_repairs_with_locks_and_obeys_cancellation() {
        let mut lattice = folded();
        for edge in &mut lattice.edges {
            edge.fixed_axis = Some(0);
        }
        let locked: Vec<_> = lattice.edges.iter().map(|e| e.d.x).collect();
        let mut cancelled = folded();
        assert!(repair_rebuilt_observed(&mut cancelled, &|| true, &|_| {}, &mut Silent).is_err());
        assert_eq!(cancelled.flipped(), 2);
        repair_rebuilt_observed(&mut lattice, &|| false, &|_| {}, &mut Silent).unwrap();
        assert_eq!(lattice.flipped(), 0);
        assert_eq!(lattice.defects(), 0);
        assert_eq!(
            lattice.edges.iter().map(|e| e.d.x).collect::<Vec<_>>(),
            locked
        );
    }
    impl Observer for StopAfterPass {
        fn checkpoint(&mut self, depth: usize, next_radius: usize, graph: &Graph) -> Result<()> {
            if next_radius > 1 {
                self.0 = Some(Checkpoint {
                    depth,
                    next_radius,
                    graph: graph.clone(),
                });
                return Err("test checkpoint stop".into());
            }
            Ok(())
        }
    }
    #[test]
    fn repair_checkpoint_resume_matches_uninterrupted_and_rejects_wrong_graph() {
        let mut uninterrupted = folded();
        repair(&mut uninterrupted, &|| false, &|_| {}).unwrap();
        assert_eq!(uninterrupted.flipped(), 0);
        let mut interrupted = folded();
        let mut observer = StopAfterPass(None);
        assert!(
            repair_observed(&mut interrupted, &|| false, &|_| {}, &mut observer, None).is_err()
        );
        let saved = observer.0.unwrap();
        #[cfg(feature = "audit")]
        let saved: Checkpoint =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut invalid = saved.clone();
        invalid.graph.source_faces[0] += 1;
        assert!(
            repair_observed(
                &mut folded(),
                &|| false,
                &|_| {},
                &mut Silent,
                Some(invalid)
            )
            .is_err()
        );
        let mut resumed = folded();
        repair_observed(&mut resumed, &|| false, &|_| {}, &mut Silent, Some(saved)).unwrap();
        assert_eq!(resumed.defects(), 0);
        assert_eq!(
            resumed.edges.iter().map(|e| e.d).collect::<Vec<_>>(),
            uninterrupted.edges.iter().map(|e| e.d).collect::<Vec<_>>()
        );
        assert_eq!(
            resumed
                .faces
                .iter()
                .map(|f| f.rotations)
                .collect::<Vec<_>>(),
            uninterrupted
                .faces
                .iter()
                .map(|f| f.rotations)
                .collect::<Vec<_>>()
        );
    }
}
