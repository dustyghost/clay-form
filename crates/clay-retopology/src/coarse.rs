//! QuadriFlow DownsampleEdgeGraph, adapted from hierarchy.cpp at
//! 810b7a0967c35b0dc85b4464e3835e26a756c967. See LICENSE-QuadriFlow.
//! Contract disjoint zero-offset edges and transport frames along their strips.
use crate::{Result, check, integer::Lattice};
use glam::IVec2;
pub(crate) const NONE: usize = usize::MAX;
pub(crate) fn rot(d: IVec2, k: i32) -> IVec2 {
    match k.rem_euclid(4) {
        0 => d,
        1 => IVec2::new(-d.y, d.x),
        2 => -d,
        _ => IVec2::new(d.y, -d.x),
    }
}
#[derive(Clone, Debug)]
#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Graph {
    pub faces: Vec<[usize; 3]>,
    pub rotations: Vec<[i32; 3]>,
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::offsets"))]
    pub offsets: Vec<IVec2>,
    pub allow: Vec<[u8; 2]>,
    pub edge_faces: Vec<[usize; 2]>,
    pub source_faces: Vec<usize>,
}
pub(crate) struct Mapping {
    pub edges: Vec<i32>,
    pub rotations: Vec<i32>,
    pub faces: Vec<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShrinkOutcome {
    Applied,
    Boundary,
    Transport,
    Locked,
    Long,
    NewInversion,
    Closure,
    NoImprovement,
}
impl Graph {
    pub fn from_lattice(lattice: &Lattice) -> Result<Self> {
        let mut edge_faces = vec![[NONE; 2]; lattice.edges.len()];
        for (f, face) in lattice.faces.iter().enumerate() {
            for &e in &face.edges {
                let pair = &mut edge_faces[e];
                if pair[0] == NONE {
                    pair[0] = f;
                } else if pair[1] == NONE {
                    pair[1] = f;
                } else {
                    return Err("Nonmanifold integer edge graph".into());
                }
            }
        }
        Ok(Self {
            faces: lattice.faces.iter().map(|f| f.edges).collect(),
            rotations: lattice
                .faces
                .iter()
                .map(|f| f.rotations.map(|r| r.rem_euclid(4)))
                .collect(),
            offsets: lattice.edges.iter().map(|e| e.d).collect(),
            allow: lattice
                .edges
                .iter()
                .map(|e| std::array::from_fn(|k| u8::from(e.fixed_axis != Some(k))))
                .collect(),
            source_faces: (0..lattice.faces.len()).collect(),
            edge_faces,
        })
    }
    pub fn coarsen(&self, cancel: &dyn Fn() -> bool) -> Result<(Self, Mapping)> {
        let mut fixed = vec![0u8; self.faces.len()];
        for (i, face) in self.faces.iter().enumerate() {
            if (0..3)
                .map(|k| rot(self.offsets[face[k]], self.rotations[i][k]))
                .sum::<IVec2>()
                != IVec2::ZERO
            {
                fixed[i] = 1;
            }
        }
        let mut mapping = Mapping {
            edges: vec![-1; self.offsets.len()],
            rotations: vec![0; self.offsets.len()],
            faces: vec![NONE; self.faces.len()],
        };
        for (e, &d) in self.offsets.iter().enumerate() {
            if e % 4096 == 0 {
                check(cancel)?;
            }
            let pair = self.edge_faces[e];
            if d != IVec2::ZERO || pair.iter().any(|&f| f != NONE && fixed[f] != 0) {
                continue;
            }
            for &f in &pair {
                if f == NONE {
                    continue;
                }
                for &ne in &self.faces[f] {
                    for &nf in &self.edge_faces[ne] {
                        if nf != NONE && fixed[nf] == 0 {
                            fixed[nf] = 1;
                        }
                    }
                }
            }
            for f in pair {
                if f != NONE {
                    fixed[f] = 2;
                }
            }
            mapping.edges[e] = -2;
        }
        for (e, pair) in self.edge_faces.iter().enumerate() {
            if mapping.edges[e] != -2 && pair.iter().all(|&f| f == NONE || fixed[f] == 2) {
                mapping.edges[e] = -3;
            }
        }
        let mut pairs = Vec::new();
        for i in 0..self.offsets.len() {
            if i % 4096 == 0 {
                check(cancel)?;
            }
            if mapping.edges[i] != -1 {
                continue;
            }
            let pair = self.edge_faces[i];
            if pair.iter().all(|&f| f == NONE || fixed[f] < 2) {
                mapping.edges[i] = pairs.len() as i32;
                pairs.push(pair);
                continue;
            }
            let f0 = if pair[0] == NONE || pair[1] == NONE || fixed[pair[0]] < 2 {
                pair[0]
            } else {
                pair[1]
            };
            let mut e = i;
            let mut f = f0;
            let mut path = vec![(i, 0)];
            loop {
                let pair = self.edge_faces[e];
                if pair[0] == f {
                    f = pair[1];
                } else if pair[1] == f {
                    f = pair[0];
                } else {
                    return Err("Broken coarse edge strip".into());
                }
                if f == NONE || fixed[f] < 2 {
                    let mut orientation = 0;
                    for &(edge, shift) in &path {
                        orientation = (orientation + shift) % 4;
                        mapping.edges[edge] = pairs.len() as i32;
                        mapping.rotations[edge] = orientation;
                    }
                    pairs.push([f0, f]);
                    break;
                }
                let k = self.faces[f]
                    .iter()
                    .position(|&x| x == e)
                    .ok_or("Missing edge in coarse face")?;
                if let Some(j) =
                    (0..3).find(|&j| self.faces[f][j] != e && mapping.edges[self.faces[f][j]] != -2)
                {
                    e = self.faces[f][j];
                    path.push((e, (self.rotations[f][j] - self.rotations[f][k] + 6) % 4));
                } else {
                    if self.offsets[e] != IVec2::ZERO {
                        return Err("Nonzero self-edge in coarse contraction".into());
                    }
                    for &(edge, _) in &path {
                        mapping.edges[edge] = pairs.len() as i32;
                        mapping.rotations[edge] = 0;
                    }
                    pairs.push([f0, f0]);
                    break;
                }
                if path.len() > self.offsets.len() {
                    return Err("Cyclic coarse edge strip".into());
                }
            }
        }
        let mut coarse = Self {
            faces: vec![],
            rotations: vec![],
            offsets: vec![IVec2::ZERO; pairs.len()],
            allow: vec![[1; 2]; pairs.len()],
            edge_faces: pairs,
            source_faces: vec![],
        };
        for (i, &upper) in mapping.edges.iter().enumerate() {
            if upper < 0 {
                continue;
            }
            let upper = upper as usize;
            let dimension = mapping.rotations[i] as usize % 2;
            if mapping.rotations[i] == 0 {
                coarse.offsets[upper] = self.offsets[i];
            }
            for k in 0..2 {
                let allow = self.allow[i][(k + dimension) % 2];
                if allow == 0 || allow == 2 {
                    coarse.allow[upper][k] = allow;
                }
            }
        }
        for (i, face) in self.faces.iter().enumerate() {
            let upper = face.map(|e| mapping.edges[e]);
            if upper.iter().any(|&e| e < 0) {
                continue;
            }
            mapping.faces[i] = coarse.faces.len();
            coarse.faces.push(upper.map(|e| e as usize));
            coarse.rotations.push(std::array::from_fn(|k| {
                (self.rotations[i][k] + 4 - mapping.rotations[face[k]]) % 4
            }));
            coarse.source_faces.push(self.source_faces[i]);
        }
        for pair in &mut coarse.edge_faces {
            for f in pair {
                if *f != NONE {
                    *f = mapping.faces[*f];
                }
            }
        }
        Ok((coarse, mapping))
    }
    pub fn hierarchy(self, cancel: &dyn Fn() -> bool) -> Result<Vec<Self>> {
        let mut levels = vec![self];
        for _ in 0..99 {
            let (coarse, _) = levels.last().unwrap().coarsen(cancel)?;
            if coarse.offsets.len() == levels.last().unwrap().offsets.len() {
                break;
            }
            levels.push(coarse);
        }
        Ok(levels)
    }
}

pub(crate) fn repair_lattice(lattice: &mut Lattice, cancel: &dyn Fn() -> bool) -> Result<()> {
    let repaired = Graph::from_lattice(lattice)?.repair(cancel)?;
    for (e, d) in lattice.edges.iter_mut().zip(repaired.offsets) {
        e.d = d;
    }
    for (f, r) in lattice.faces.iter_mut().zip(repaired.rotations) {
        f.rotations = r;
    }
    if lattice.defects() != 0 {
        return Err("Coarse flip repair changed a regularity equation".into());
    }
    Ok(())
}
impl Graph {
    pub(crate) fn area(&self, f: usize) -> i32 {
        let a = rot(self.offsets[self.faces[f][0]], self.rotations[f][0]);
        let b = rot(self.offsets[self.faces[f][1]], self.rotations[f][1]);
        a.x * b.y - a.y * b.x
    }
    pub(crate) fn halfedges(&self) -> Result<Vec<usize>> {
        let mut twins = vec![NONE; self.faces.len() * 3];
        for (e, &[a, b]) in self.edge_faces.iter().enumerate() {
            if a == NONE || b == NONE {
                continue;
            }
            let ha = a * 3
                + self.faces[a]
                    .iter()
                    .position(|&x| x == e)
                    .ok_or("Missing coarse edge")?;
            let hb = b * 3
                + self.faces[b]
                    .iter()
                    .rposition(|&x| x == e)
                    .ok_or("Missing coarse reverse edge")?;
            twins[ha] = hb;
            twins[hb] = ha;
        }
        Ok(twins)
    }
    fn shrink(&mut self, start: usize, twins: &[usize]) -> Result<bool> {
        Ok(self.shrink_checked(start, twins, 1, false)? == ShrinkOutcome::Applied)
    }
    fn shrink_checked(
        &mut self,
        start: usize,
        twins: &[usize],
        max_length: i32,
        strict: bool,
    ) -> Result<ShrinkOutcome> {
        if start == NONE {
            return Ok(ShrinkOutcome::Boundary);
        }
        let prev = |h: usize| h / 3 * 3 + (h + 2) % 3;
        let next = |h: usize| h / 3 * 3 + (h + 1) % 3;
        let mut h = start;
        let mut steps = 0;
        loop {
            h = prev(h);
            if twins[h] == NONE {
                break;
            }
            h = twins[h];
            if h == start {
                break;
            }
            steps += 1;
            if steps > twins.len() {
                return Err("Coarse shrink fan does not close".into());
            }
        }
        let beginning = h;
        let first = self.offsets[self.faces[h / 3][h % 3]];
        let mut delta = first;
        let mut faces = Vec::new();
        let mut shifts = Vec::new();
        loop {
            faces.push(h / 3);
            shifts.push((self.faces[h / 3][h % 3], delta));
            h = twins[h];
            if h == NONE {
                return Ok(ShrinkOutcome::Boundary);
            }
            delta = -rot(delta, self.rotations[h / 3][h % 3]);
            h = next(h);
            delta = rot(delta, -self.rotations[h / 3][h % 3]);
            if h == beginning {
                break;
            }
            if shifts.len() > twins.len() {
                return Err("Coarse shrink transport does not close".into());
            }
        }
        if delta != first {
            return Ok(ShrinkOutcome::Transport);
        }
        let mut values = std::collections::BTreeMap::new();
        for &(e, d) in &shifts {
            if (0..2).any(|k| d[k] != 0 && self.allow[e][k] == 0) {
                return Ok(ShrinkOutcome::Locked);
            }
            let value = values.entry(e).or_insert(self.offsets[e]);
            *value -= d;
            if value.abs().max_element() > max_length
                || (value.abs().max_element() > 1 && value.x != 0 && value.y != 0)
            {
                return Ok(ShrinkOutcome::Long);
            }
        }
        // The experiment checks every face incident to any changed edge, not
        // only the walked fan. This also covers repeated/coarsened edges.
        let affected: Vec<_> = if strict {
            values
                .keys()
                .flat_map(|&e| self.edge_faces[e])
                .filter(|&f| f != NONE)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        let old_areas: Vec<_> = affected.iter().map(|&f| self.area(f)).collect();
        let before = faces.iter().filter(|&&f| self.area(f) < 0).count();
        for (&e, v) in &mut values {
            std::mem::swap(&mut self.offsets[e], v);
        }
        let after = faces.iter().filter(|&&f| self.area(f) < 0).count();
        let outcome = if strict
            && affected
                .iter()
                .zip(&old_areas)
                .any(|(&f, &old)| old >= 0 && self.area(f) < 0)
        {
            ShrinkOutcome::NewInversion
        } else if strict
            && affected.iter().any(|&f| {
                (0..3)
                    .map(|k| rot(self.offsets[self.faces[f][k]], self.rotations[f][k]))
                    .sum::<IVec2>()
                    != IVec2::ZERO
            })
        {
            ShrinkOutcome::Closure
        } else if after < before {
            ShrinkOutcome::Applied
        } else {
            ShrinkOutcome::NoImprovement
        };
        if outcome == ShrinkOutcome::Applied {
            return Ok(outcome);
        }
        for (&e, v) in &mut values {
            std::mem::swap(&mut self.offsets[e], v);
        }
        Ok(outcome)
    }
    #[cfg(feature = "audit")]
    pub(crate) fn shrink_preserving_orientation(
        &mut self,
        max_length: i32,
        cancel: &dyn Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if !(1..=2).contains(&max_length) {
            return Err("Strict contraction length must be one or two".into());
        }
        let twins = self.halfedges()?;
        let initial = (0..self.faces.len()).filter(|&f| self.area(f) < 0).count();
        let mut outcomes = std::collections::BTreeMap::<String, usize>::new();
        let mut passes = Vec::new();
        for _ in 0..=initial {
            check(cancel)?;
            let flipped: Vec<_> = (0..self.faces.len())
                .filter(|&f| self.area(f) < 0)
                .collect();
            let mut applied = 0;
            for f in flipped.iter().copied() {
                check(cancel)?;
                if self.area(f) >= 0 {
                    continue;
                }
                'face: for k in 0..3 {
                    let h = f * 3 + k;
                    for start in [h, twins[h]] {
                        let outcome = self.shrink_checked(start, &twins, max_length, true)?;
                        *outcomes.entry(format!("{outcome:?}")).or_default() += 1;
                        if outcome == ShrinkOutcome::Applied {
                            applied += 1;
                            break 'face;
                        }
                    }
                }
            }
            let after = (0..self.faces.len()).filter(|&f| self.area(f) < 0).count();
            passes
                .push(serde_json::json!({"before":flipped.len(),"after":after,"applied":applied}));
            if applied == 0 {
                break;
            }
            if after >= flipped.len() {
                return Err("Strict contraction did not decrease global inversions".into());
            }
        }
        Ok(
            serde_json::json!({"initial":initial,"max_length":max_length,"passes":passes,"outcomes":outcomes}),
        )
    }
    /// Same effective one-pass candidate ordering as upstream FixFlip. Its
    /// max_len=2 retry has an already-drained queue, so only length one is tried.
    fn shrink_pass(&mut self, cancel: &dyn Fn() -> bool) -> Result<bool> {
        let twins = self.halfedges()?;
        let flipped: Vec<_> = (0..self.faces.len())
            .filter(|&f| self.area(f) < 0)
            .collect();
        let mut changed = false;
        for f in flipped {
            check(cancel)?;
            if self.area(f) >= 0 {
                continue;
            }
            for k in 0..3 {
                let h = f * 3 + k;
                if self.shrink(h, &twins)? || self.shrink(twins[h], &twins)? {
                    changed = true;
                    break;
                }
            }
        }
        Ok(changed)
    }
    /// An explicit stack replaces upstream recursion and propagates corrections
    /// back through every saved graph map before returning to the source lattice.
    fn repair(self, cancel: &dyn Fn() -> bool) -> Result<Self> {
        let initial_flips = (0..self.faces.len()).filter(|&f| self.area(f) < 0).count();
        let mut levels = vec![self];
        let mut maps = Vec::new();
        for pass in 0..=initial_flips {
            check(cancel)?;
            for _ in 0..99 {
                let (coarse, map) = levels.last().unwrap().coarsen(cancel)?;
                if coarse.offsets.len() == levels.last().unwrap().offsets.len() {
                    break;
                }
                levels.push(coarse);
                maps.push(map);
            }
            if !levels.last_mut().unwrap().shrink_pass(cancel)? {
                break;
            }
            if pass == initial_flips {
                return Err("Coarse flip repair exceeded its decreasing-area budget".into());
            }
        }
        while let Some(map) = maps.pop() {
            check(cancel)?;
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
                if upper == NONE {
                    continue;
                }
                fine.rotations[f] = std::array::from_fn(|k| {
                    (coarse.rotations[upper][k] + map.rotations[fine.faces[f][k]]).rem_euclid(4)
                });
            }
        }
        Ok(levels.pop().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    fn read_fixture(input: &str) -> Graph {
        let mut tokens = input.split_whitespace();
        let e: usize = tokens.next().unwrap().parse().unwrap();
        let f: usize = tokens.next().unwrap().parse().unwrap();
        let mut g = Graph {
            faces: vec![],
            rotations: vec![],
            offsets: vec![],
            allow: vec![],
            edge_faces: vec![[NONE; 2]; e],
            source_faces: (0..f).collect(),
        };
        for _ in 0..e {
            let v: Vec<i32> = tokens
                .by_ref()
                .take(4)
                .map(|s| s.parse().unwrap())
                .collect();
            g.offsets.push(IVec2::new(v[0], v[1]));
            g.allow.push([v[2] as u8, v[3] as u8]);
        }
        for i in 0..f {
            let v: Vec<i32> = tokens
                .by_ref()
                .take(6)
                .map(|s| s.parse().unwrap())
                .collect();
            let face = [v[0] as usize, v[1] as usize, v[2] as usize];
            for &e in &face {
                if g.edge_faces[e][0] == NONE {
                    g.edge_faces[e][0] = i;
                } else {
                    g.edge_faces[e][1] = i;
                }
            }
            g.faces.push(face);
            g.rotations.push([v[3], v[4], v[5]]);
        }
        g
    }
    #[test]
    fn every_coarse_level_matches_pinned_upstream_on_a_periodic_grid() {
        let g = read_fixture(include_str!("../tests/data/coarse-torus.input"));
        assert!(
            g.clone()
                .hierarchy(&|| true)
                .unwrap_err()
                .contains("cancelled")
        );
        let mut actual = String::new();
        let levels = g.hierarchy(&|| false).unwrap();
        for (l, g) in levels.iter().enumerate() {
            writeln!(actual, "LEVEL {l} {} {}", g.offsets.len(), g.faces.len()).unwrap();
            for (i, d) in g.offsets.iter().enumerate() {
                writeln!(
                    actual,
                    "{} {} {} {} {} {}",
                    d.x, d.y, g.allow[i][0], g.allow[i][1], g.edge_faces[i][0], g.edge_faces[i][1]
                )
                .unwrap();
            }
            for (f, r) in g.faces.iter().zip(&g.rotations) {
                writeln!(
                    actual,
                    "{} {} {} {} {} {}",
                    f[0], f[1], f[2], r[0], r[1], r[2]
                )
                .unwrap();
            }
            for (face, r) in g.faces.iter().zip(&g.rotations) {
                assert_eq!(
                    (0..3).map(|k| rot(g.offsets[face[k]], r[k])).sum::<IVec2>(),
                    IVec2::ZERO
                );
            }
        }
        assert_eq!(actual, include_str!("../tests/data/coarse-torus.expected"));
        assert_eq!(levels.last().unwrap().faces.len(), 32);
    }
    #[test]
    fn connected_fan_flip_repair_matches_pinned_upstream() {
        let graph = read_fixture(include_str!("../tests/data/coarse-flipped.input"));
        assert_eq!(
            (0..graph.faces.len())
                .filter(|&f| graph.area(f) < 0)
                .count(),
            1
        );
        let repaired = graph.repair(&|| false).unwrap();
        let mut actual = String::new();
        writeln!(
            actual,
            "{} {}",
            repaired.offsets.len(),
            repaired.faces.len()
        )
        .unwrap();
        for (d, allow) in repaired.offsets.iter().zip(&repaired.allow) {
            writeln!(actual, "{} {} {} {}", d.x, d.y, allow[0], allow[1]).unwrap();
        }
        for (f, r) in repaired.faces.iter().zip(&repaired.rotations) {
            writeln!(
                actual,
                "{} {} {} {} {} {}",
                f[0], f[1], f[2], r[0], r[1], r[2]
            )
            .unwrap();
        }
        assert_eq!(
            actual,
            include_str!("../tests/data/coarse-flipped.expected")
        );
        assert!((0..repaired.faces.len()).all(|f| repaired.area(f) >= 0));
    }

    #[test]
    fn strict_contraction_preserves_neighbours_and_rolls_back_rejected_moves() {
        let mut applied = 0;
        let mut rejected_new_inversion = 0;
        for input in [
            include_str!("../tests/data/coarse-flipped.input"),
            include_str!("../tests/data/orientation-fan.input"),
        ] {
            let graph = read_fixture(input);
            let twins = graph.halfedges().unwrap();
            let areas: Vec<_> = (0..graph.faces.len()).map(|f| graph.area(f)).collect();
            for max_length in [1, 2] {
                for start in 0..twins.len() {
                    let mut candidate = graph.clone();
                    let outcome = candidate
                        .shrink_checked(start, &twins, max_length, true)
                        .unwrap();
                    if outcome == ShrinkOutcome::Applied {
                        applied += 1;
                        for (f, &old_area) in areas.iter().enumerate() {
                            assert!(old_area < 0 || candidate.area(f) >= 0);
                            assert_eq!(
                                (0..3)
                                    .map(|k| rot(
                                        candidate.offsets[candidate.faces[f][k]],
                                        candidate.rotations[f][k]
                                    ))
                                    .sum::<IVec2>(),
                                IVec2::ZERO
                            );
                        }
                    } else {
                        rejected_new_inversion +=
                            usize::from(outcome == ShrinkOutcome::NewInversion);
                        assert_eq!(candidate.offsets, graph.offsets);
                    }
                    let mut locked = graph.clone();
                    locked.allow.fill([0, 0]);
                    assert_ne!(
                        locked
                            .shrink_checked(start, &twins, max_length, true)
                            .unwrap(),
                        ShrinkOutcome::Applied
                    );
                    assert_eq!(locked.offsets, graph.offsets);
                }
            }
        }
        assert!(applied > 0);
        assert!(rejected_new_inversion > 0);
    }
}
