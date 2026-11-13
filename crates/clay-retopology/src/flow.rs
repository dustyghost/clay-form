//! Integral min-cost flow using successive shortest augmenting paths.
//! Residual reverse arcs allow earlier choices to be revised; potentials keep
//! reduced costs nonnegative so each shortest-path pass can use Dijkstra.
use crate::{Result, check};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, VecDeque},
};

#[derive(Clone)]
struct Arc {
    to: usize,
    reverse: usize,
    capacity: i32,
    cost: i64,
}
#[derive(Clone, Copy)]
pub(crate) struct Handle {
    node: usize,
    arc: usize,
    capacity: i32,
}
pub(crate) struct Network {
    rows: Vec<Vec<Arc>>,
}
impl Network {
    pub fn new(nodes: usize) -> Self {
        Self {
            rows: vec![Vec::new(); nodes],
        }
    }
    pub fn add(&mut self, from: usize, to: usize, capacity: i32, cost: i64) -> Handle {
        assert!(from != to && capacity >= 0 && cost >= 0);
        let a = self.rows[from].len();
        let b = self.rows[to].len();
        self.rows[from].push(Arc {
            to,
            reverse: b,
            capacity,
            cost,
        });
        self.rows[to].push(Arc {
            to: from,
            reverse: a,
            capacity: 0,
            cost: -cost,
        });
        Handle {
            node: from,
            arc: a,
            capacity,
        }
    }
    pub fn used(&self, h: Handle) -> i32 {
        h.capacity - self.rows[h.node][h.arc].capacity
    }
    pub fn send(
        &mut self,
        source: usize,
        sink: usize,
        amount: i32,
        cancel: &dyn Fn() -> bool,
    ) -> Result<()> {
        let n = self.rows.len();
        let mut potential = vec![0i64; n];
        let mut sent = 0;
        let mut distance = vec![i64::MAX; n];
        while sent < amount {
            check(cancel)?;
            distance.fill(i64::MAX);
            distance[source] = 0;
            let mut heap = BinaryHeap::from([Reverse((0i64, source))]);
            let mut visited = 0;
            while let Some(Reverse((d, u))) = heap.pop() {
                if d != distance[u] {
                    continue;
                }
                if u == sink {
                    break;
                }
                visited += 1;
                if visited % 16384 == 0 {
                    check(cancel)?;
                }
                for a in &self.rows[u] {
                    if a.capacity == 0 {
                        continue;
                    }
                    let cost = a.cost + potential[u] - potential[a.to];
                    if cost < 0 {
                        return Err("Invalid residual cost in retopology flow".into());
                    }
                    let next = d + cost;
                    if next < distance[a.to] {
                        distance[a.to] = next;
                        heap.push(Reverse((next, a.to)));
                    }
                }
            }
            if distance[sink] == i64::MAX {
                return Err("The integer surface grid has no feasible correction".into());
            }
            for i in 0..n {
                potential[i] += distance[i].min(distance[sink]);
            }
            // Send a blocking flow through all cheapest paths before running
            // Dijkstra again. Dense meshes can have thousands of independent
            // unit corrections at the same distance.
            let delta = self.blocking(source, sink, amount - sent, &potential, cancel)?;
            if delta == 0 {
                return Err("Retopology flow made no progress".into());
            }
            sent += delta;
        }
        Ok(())
    }
    fn blocking(
        &mut self,
        source: usize,
        sink: usize,
        limit: i32,
        potential: &[i64],
        cancel: &dyn Fn() -> bool,
    ) -> Result<i32> {
        let mut levels = vec![usize::MAX; self.rows.len()];
        levels[source] = 0;
        let mut queue = VecDeque::from([source]);
        while let Some(u) = queue.pop_front() {
            for a in &self.rows[u] {
                if a.capacity > 0
                    && a.cost + potential[u] - potential[a.to] == 0
                    && levels[a.to] == usize::MAX
                {
                    levels[a.to] = levels[u] + 1;
                    queue.push_back(a.to);
                }
            }
        }
        let mut current = vec![0; self.rows.len()];
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut u = source;
        let mut total = 0;
        let mut work = 0;
        loop {
            work += 1;
            if work % 16384 == 0 {
                check(cancel)?;
            }
            if u == sink {
                let mut delta = limit - total;
                for &(v, i) in &path {
                    delta = delta.min(self.rows[v][i].capacity);
                }
                for &(v, i) in &path {
                    let a = &mut self.rows[v][i];
                    let to = a.to;
                    let reverse = a.reverse;
                    a.capacity -= delta;
                    self.rows[to][reverse].capacity += delta;
                }
                total += delta;
                if total == limit {
                    break;
                }
                u = source;
                path.clear();
                continue;
            }
            let next = (current[u]..self.rows[u].len()).find(|&i| {
                let a = &self.rows[u][i];
                a.capacity > 0
                    && levels[a.to] == levels[u] + 1
                    && a.cost + potential[u] - potential[a.to] == 0
            });
            if let Some(i) = next {
                current[u] = i;
                path.push((u, i));
                u = self.rows[u][i].to;
            } else {
                levels[u] = usize::MAX;
                current[u] = self.rows[u].len();
                if let Some((v, i)) = path.pop() {
                    u = v;
                    current[u] = i + 1;
                } else {
                    break;
                }
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn residual_paths_revise_a_greedy_assignment() {
        let mut n = Network::new(6);
        n.add(0, 1, 1, 0);
        n.add(0, 2, 1, 0);
        let a = n.add(1, 3, 1, 0);
        let b = n.add(1, 4, 1, 1);
        let c = n.add(2, 3, 1, 1);
        let d = n.add(2, 4, 1, 9);
        n.add(3, 5, 1, 0);
        n.add(4, 5, 1, 0);
        n.send(0, 5, 2, &|| false).unwrap();
        assert_eq!([n.used(a), n.used(b), n.used(c), n.used(d)], [0, 1, 1, 0]);
    }
    #[test]
    fn infeasible_and_cancelled_do_not_report_success() {
        let mut n = Network::new(2);
        assert!(n.send(0, 1, 1, &|| false).is_err());
        n.add(0, 1, 1, 0);
        assert!(n.send(0, 1, 1, &|| true).unwrap_err().contains("cancelled"));
    }
    #[test]
    fn blocking_flow_matches_exhaustive_small_assignment_costs() {
        for seed in 0..20 {
            let costs: Vec<Vec<i64>> = (0..3)
                .map(|a| {
                    (0..3)
                        .map(|b| ((seed * 17 + a * 11 + b * 7 + a * b * 13) % 19) as i64)
                        .collect()
                })
                .collect();
            let mut network = Network::new(8);
            let mut handles = Vec::new();
            for i in 0..3 {
                network.add(0, 1 + i, 1, 0);
                network.add(4 + i, 7, 1, 0);
            }
            for (a, row) in costs.iter().enumerate() {
                for (b, &cost) in row.iter().enumerate() {
                    handles.push((network.add(a + 1, b + 4, 1, cost), cost));
                }
            }
            network.send(0, 7, 3, &|| false).unwrap();
            let actual: i64 = handles
                .iter()
                .map(|&(h, c)| network.used(h) as i64 * c)
                .sum();
            let optimal = [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ]
            .iter()
            .map(|p| (0..3).map(|a| costs[a][p[a]]).sum::<i64>())
            .min()
            .unwrap();
            assert_eq!(actual, optimal, "assignment seed {seed}");
        }
    }
}
