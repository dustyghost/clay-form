//! Ordered two-tree max flow, adapted from Stephan Diederich's Boost
//! boykov_kolmogorov_max_flow.hpp (2006). See LICENSE-Boost-MaxFlow.
//! Keep adjacency, active-node and orphan order: distinct maximum flows can
//! produce different quad layouts. No recursive traversal or external solver.
use crate::{Result, check};
use std::collections::VecDeque;
const NONE: usize = usize::MAX;

#[derive(Clone)]
struct Arc {
    from: usize,
    to: usize,
    capacity: i32,
    residual: i32,
}
#[derive(Clone)]
pub(super) struct Network {
    rows: Vec<Vec<usize>>,
    arcs: Vec<Arc>,
}
impl Network {
    pub fn new(nodes: usize) -> Self {
        Self {
            rows: vec![vec![]; nodes],
            arcs: vec![],
        }
    }
    pub fn add(&mut self, a: usize, b: usize, c: i32, reverse: i32) -> usize {
        assert!(a != b && c >= 0 && reverse >= 0);
        let e = self.arcs.len();
        self.rows[a].push(e);
        self.rows[b].push(e + 1);
        self.arcs.push(Arc {
            from: a,
            to: b,
            capacity: c,
            residual: c,
        });
        self.arcs.push(Arc {
            from: b,
            to: a,
            capacity: reverse,
            residual: reverse,
        });
        e
    }
    pub fn used(&self, e: usize) -> i32 {
        self.arcs[e].capacity - self.arcs[e].residual
    }
    pub fn solve(&mut self, small: bool, cancel: &dyn Fn() -> bool) -> Result<i32> {
        check(cancel)?;
        if small {
            return self.unit_paths(cancel);
        }
        let n = self.rows.len();
        let mut search = Search {
            net: self,
            tree: vec![0; n],
            parent: vec![NONE; n],
            distance: vec![0; n],
            stamp: vec![0; n],
            active: VecDeque::new(),
            queued: vec![false; n],
            orphans: VecDeque::new(),
            children: VecDeque::new(),
            time: 1,
            last: NONE,
            cursor: 0,
            flow: 0,
            work: 0,
            cancel,
        };
        search.tree[0] = 1;
        search.tree[n - 1] = 2;
        search.stamp[0] = 1;
        search.stamp[n - 1] = 1;
        search.direct()?;
        while let Some(edge) = search.grow()? {
            search.time += 1;
            search.augment(edge)?;
            search.adopt()?;
        }
        Ok(search.flow)
    }
    // Upstream ECMaxFlowHelper for supply < 20: one unit per ordered BFS.
    fn unit_paths(&mut self, cancel: &dyn Fn() -> bool) -> Result<i32> {
        let n = self.rows.len();
        let mut total = 0;
        loop {
            check(cancel)?;
            let mut parent = vec![NONE; n];
            parent[0] = self.arcs.len();
            let mut queue = VecDeque::from([0]);
            while let Some(v) = queue.pop_front() {
                for &e in &self.rows[v] {
                    let a = &self.arcs[e];
                    if a.residual > 0 && parent[a.to] == NONE {
                        parent[a.to] = e;
                        queue.push_back(a.to);
                        if a.to == n - 1 {
                            break;
                        }
                    }
                }
                if parent[n - 1] != NONE {
                    break;
                }
                if v.is_multiple_of(4096) {
                    check(cancel)?;
                }
            }
            if parent[n - 1] == NONE {
                return Ok(total);
            }
            let mut v = n - 1;
            while v != 0 {
                let e = parent[v];
                self.arcs[e].residual -= 1;
                self.arcs[e ^ 1].residual += 1;
                v = self.arcs[e].from;
            }
            total += 1;
        }
    }
}
struct Search<'a> {
    net: &'a mut Network,
    tree: Vec<u8>,
    parent: Vec<usize>,
    distance: Vec<usize>,
    stamp: Vec<usize>,
    active: VecDeque<usize>,
    queued: Vec<bool>,
    orphans: VecDeque<usize>,
    children: VecDeque<usize>,
    time: usize,
    last: usize,
    cursor: usize,
    flow: i32,
    work: usize,
    cancel: &'a dyn Fn() -> bool,
}
impl Search<'_> {
    fn tick(&mut self) -> Result<()> {
        self.work += 1;
        if self.work > 500_000_000 {
            return Err("Integer maximum-flow work budget exceeded".into());
        }
        if self.work.is_multiple_of(4096) {
            check(self.cancel)?;
        }
        Ok(())
    }
    fn activate(&mut self, v: usize) {
        if self.queued[v] {
            if self.last == v {
                self.last = NONE;
            }
        } else {
            self.queued[v] = true;
            self.active.push_back(v);
        }
    }
    fn direct(&mut self) -> Result<()> {
        let sink = self.tree.len() - 1;
        for i in 0..self.net.rows[0].len() {
            self.tick()?;
            let e = self.net.rows[0][i];
            let v = self.net.arcs[e].to;
            if v == sink {
                self.flow += self.net.arcs[e].residual;
                self.net.arcs[e].residual = 0;
                continue;
            }
            let to_sink = self.net.rows[v]
                .iter()
                .copied()
                .find(|&e| self.net.arcs[e].to == sink);
            if let Some(t) = to_sink {
                let a = self.net.arcs[e].residual;
                let b = self.net.arcs[t].residual;
                if a > b {
                    self.tree[v] = 1;
                    self.activate(v);
                    self.parent[v] = e;
                    self.distance[v] = 1;
                    self.stamp[v] = 1;
                    self.net.arcs[e].residual -= b;
                    self.net.arcs[t].residual = 0;
                    self.flow += b;
                } else if b > 0 {
                    self.tree[v] = 2;
                    self.activate(v);
                    self.parent[v] = t;
                    self.distance[v] = 1;
                    self.stamp[v] = 1;
                    self.net.arcs[t].residual -= a;
                    self.net.arcs[e].residual = 0;
                    self.flow += a;
                }
            } else if self.net.arcs[e].residual > 0 {
                self.tree[v] = 1;
                self.parent[v] = e;
                self.distance[v] = 1;
                self.stamp[v] = 1;
                self.activate(v);
            }
        }
        for i in 0..self.net.rows[sink].len() {
            let e = self.net.rows[sink][i] ^ 1;
            let v = self.net.arcs[e].from;
            if self.net.arcs[e].residual > 0 {
                self.tree[v] = 2;
                self.parent[v] = e;
                self.distance[v] = 1;
                self.stamp[v] = 1;
                self.activate(v);
            }
        }
        Ok(())
    }
    fn grow(&mut self) -> Result<Option<usize>> {
        while let Some(&v) = self.active.front() {
            self.tick()?;
            if self.parent[v] == NONE && v != 0 && v != self.tree.len() - 1 {
                self.active.pop_front();
                self.queued[v] = false;
                continue;
            }
            if self.last != v {
                self.last = v;
                self.cursor = 0;
            }
            while self.cursor < self.net.rows[v].len() {
                self.tick()?;
                let edge = self.net.rows[v][self.cursor];
                let e = if self.tree[v] == 1 { edge } else { edge ^ 1 };
                let u = self.net.arcs[edge].to;
                if self.net.arcs[e].residual > 0 {
                    if self.tree[u] == 0 {
                        self.tree[u] = self.tree[v];
                        self.parent[u] = e;
                        self.distance[u] = self.distance[v] + 1;
                        self.stamp[u] = self.stamp[v];
                        self.activate(u);
                    } else if self.tree[u] == self.tree[v] {
                        if self.stamp[u] <= self.stamp[v] && self.distance[u] > self.distance[v] + 1
                        {
                            self.parent[u] = e;
                            self.distance[u] = self.distance[v] + 1;
                            self.stamp[u] = self.stamp[v];
                        }
                    } else {
                        return Ok(Some(e));
                    }
                }
                self.cursor += 1;
            }
            self.active.pop_front();
            self.queued[v] = false;
            self.last = NONE;
        }
        Ok(None)
    }
    fn toward_terminal(&self, v: usize, e: usize) -> usize {
        if self.tree[v] == 1 {
            self.net.arcs[e].from
        } else {
            self.net.arcs[e].to
        }
    }
    fn augment(&mut self, e: usize) -> Result<()> {
        let sink = self.tree.len() - 1;
        let mut amount = self.net.arcs[e].residual;
        for (mut v, terminal) in [(self.net.arcs[e].from, 0), (self.net.arcs[e].to, sink)] {
            while v != terminal {
                self.tick()?;
                let p = self.parent[v];
                amount = amount.min(self.net.arcs[p].residual);
                v = self.toward_terminal(v, p);
            }
        }
        self.net.arcs[e].residual -= amount;
        self.net.arcs[e ^ 1].residual += amount;
        for (mut v, terminal) in [(self.net.arcs[e].from, 0), (self.net.arcs[e].to, sink)] {
            while v != terminal {
                self.tick()?;
                let p = self.parent[v];
                self.net.arcs[p].residual -= amount;
                self.net.arcs[p ^ 1].residual += amount;
                if self.net.arcs[p].residual == 0 {
                    self.parent[v] = NONE;
                    self.orphans.push_front(v);
                }
                v = self.toward_terminal(v, p);
            }
        }
        self.flow += amount;
        Ok(())
    }
    fn connected(&mut self, v: usize) -> Result<bool> {
        let terminal = if self.tree[v] == 1 {
            0
        } else {
            self.tree.len() - 1
        };
        let mut distance = 0;
        let mut u = v;
        loop {
            self.tick()?;
            if self.stamp[u] == self.time {
                distance += self.distance[u];
                break;
            }
            if u == terminal {
                self.stamp[u] = self.time;
                break;
            }
            if self.parent[u] == NONE {
                return Ok(false);
            }
            u = self.toward_terminal(u, self.parent[u]);
            distance += 1;
            if distance > self.tree.len() {
                return Err("Cycle in integer maximum-flow search tree".into());
            }
        }
        u = v;
        while self.stamp[u] != self.time {
            self.tick()?;
            self.distance[u] = distance;
            distance -= 1;
            self.stamp[u] = self.time;
            u = self.toward_terminal(u, self.parent[u]);
        }
        Ok(true)
    }
    fn adopt(&mut self) -> Result<()> {
        while let Some(v) = self
            .children
            .pop_front()
            .or_else(|| self.orphans.pop_front())
        {
            self.tick()?;
            let mut best = (usize::MAX, NONE);
            let source = self.tree[v] == 1;
            for i in 0..self.net.rows[v].len() {
                let edge = self.net.rows[v][i];
                let e = if source { edge ^ 1 } else { edge };
                let u = self.net.arcs[edge].to;
                if self.net.arcs[e].residual > 0
                    && self.tree[u] == self.tree[v]
                    && self.connected(u)?
                    && self.distance[u] < best.0
                {
                    best = (self.distance[u], e);
                }
            }
            if best.1 != NONE {
                self.parent[v] = best.1;
                self.distance[v] = best.0 + 1;
                self.stamp[v] = self.time;
            } else {
                self.stamp[v] = 0;
                let terminal = if source { 0 } else { self.tree.len() - 1 };
                for i in 0..self.net.rows[v].len() {
                    let edge = self.net.rows[v][i];
                    let e = if source { edge ^ 1 } else { edge };
                    let u = self.net.arcs[edge].to;
                    if self.tree[u] == self.tree[v] && u != terminal {
                        if self.net.arcs[e].residual > 0 {
                            self.activate(u);
                        }
                        if self.parent[u] != NONE && self.toward_terminal(u, self.parent[u]) == v {
                            self.parent[u] = NONE;
                            self.children.push_back(u);
                        }
                    }
                }
                self.tree[v] = 0;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximum_flow_matches_exhaustive_cuts_with_reverse_capacities() {
        let mut seed = 19u32;
        for _ in 0..200 {
            let mut network = Network::new(6);
            let mut capacities = Vec::new();
            for a in 0..6 {
                for b in a + 1..6 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let c = (seed % 4) as i32;
                    let reverse = if a == 0 || b == 5 {
                        0
                    } else {
                        ((seed >> 5) % 3) as i32
                    };
                    network.add(a, b, c, reverse);
                    capacities.push((a, b, c, reverse));
                }
            }
            let expected = (0..16)
                .map(|bits| {
                    let inside = |v| v == 0 || (v != 5 && bits & (1 << (v - 1)) != 0);
                    capacities
                        .iter()
                        .map(|&(a, b, c, r)| {
                            if inside(a) && !inside(b) {
                                c
                            } else if inside(b) && !inside(a) {
                                r
                            } else {
                                0
                            }
                        })
                        .sum::<i32>()
                })
                .min()
                .unwrap();
            assert_eq!(network.clone().solve(true, &|| false).unwrap(), expected);
            assert_eq!(network.solve(false, &|| false).unwrap(), expected);
        }
    }
    #[test]
    fn cancellation_is_observed() {
        let mut n = Network::new(2);
        n.add(0, 1, 1, 0);
        assert!(n.solve(false, &|| true).is_err());
    }
}
