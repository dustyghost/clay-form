//! Bounded, cancellable SAT encoding of the local integer-grid constraints.
use crate::{Result, check};
use batsat::Callbacks;
use batsat::{Lit, Solver, SolverInterface, lbool};
use std::{cell::Cell, time::Instant};
use std::{collections::VecDeque, time::Duration};
mod grid;
pub(crate) use grid::solve as solve_grid;

pub(crate) struct Rule {
    pub vars: [usize; 3],
    pub tuples: Vec<[u8; 3]>,
}
struct Budget<'a> {
    cancel: &'a dyn Fn() -> bool,
    deadline: Instant,
    checks: Cell<usize>,
    max_checks: usize,
}
impl Callbacks for Budget<'_> {
    fn stop(&self) -> bool {
        self.checks.set(self.checks.get() + 1);
        (self.cancel)() || self.checks.get() > self.max_checks || Instant::now() >= self.deadline
    }
}

#[cfg(test)]
pub(crate) fn solve(
    domains: Vec<u16>,
    rules: &[Rule],
    preferred: &[u8],
    cancel: &dyn Fn() -> bool,
) -> Result<Option<Vec<u16>>> {
    solve_with_budget(domains, rules, preferred, cancel, 200)
}

pub(crate) fn solve_with_budget(
    domains: Vec<u16>,
    rules: &[Rule],
    preferred: &[u8],
    cancel: &dyn Fn() -> bool,
    millis: u64,
) -> Result<Option<Vec<u16>>> {
    Ok(solve_reported(domains, rules, preferred, cancel, millis)?.solution)
}

#[cfg_attr(feature = "audit", derive(serde::Serialize))]
#[derive(Default)]
pub(crate) struct Statistics {
    pub outcome: &'static str,
    pub propagation_seconds: f64,
    pub encoding_seconds: f64,
    pub search_seconds: f64,
    pub total_seconds: f64,
    pub domain_values: usize,
    pub tuples: usize,
    pub callback_checks: usize,
    pub boolean_variables: usize,
    pub clauses: usize,
}
pub(crate) struct Attempt {
    pub solution: Option<Vec<u16>>,
    pub stats: Statistics,
}
/// Offline same-CNF engine comparison; not part of the remeshing pipeline.
#[cfg(feature = "audit")]
pub(crate) fn solve_dimacs(
    mut reader: impl std::io::BufRead,
    millis: u64,
    explain_units: bool,
) -> Result<serde_json::Value> {
    if !(1..=30_000).contains(&millis) {
        return Err("CNF audit budget must be in 1..30000 ms".into());
    }
    let started = Instant::now();
    let budget = Budget {
        cancel: &|| false,
        deadline: started + Duration::from_millis(millis),
        checks: Cell::new(0),
        max_checks: millis as usize * 250,
    };
    let mut solver = Solver::new(Default::default(), budget);
    let mut assumptions = Vec::new();
    if explain_units {
        // This audit mode accepts the upstream exporter: one complete clause
        // per line. Unit clauses are its fixed-coordinate boundary values.
        let mut clauses = String::new();
        let mut count = 0;
        let mut nvars = 0;
        for line in reader.lines() {
            let line = line.map_err(|e| e.to_string())?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('c') || line.starts_with('p') {
                continue;
            }
            let values: Vec<i32> = line
                .split_whitespace()
                .map(|s| s.parse().map_err(|_| "Invalid CNF integer".to_string()))
                .collect::<Result<_>>()?;
            if values.last() != Some(&0) || values[..values.len() - 1].contains(&0) {
                return Err("CNF explanation requires one clause per line".into());
            }
            nvars = nvars.max(values.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0));
            if values.len() == 2 {
                let v = values[0];
                assumptions.push(Lit::new(solver.var_of_int(v.unsigned_abs() - 1), v > 0));
            } else {
                count += 1;
                clauses.push_str(line);
                clauses.push('\n');
            }
        }
        let text = format!("p cnf {nvars} {count}\n{clauses}");
        batsat::dimacs::parse(&mut text.as_bytes(), &mut solver, true, false)
            .map_err(|e| e.to_string())?;
    } else {
        batsat::dimacs::parse(&mut reader, &mut solver, true, false).map_err(|e| e.to_string())?;
    }
    let encoding_seconds = started.elapsed().as_secs_f64();
    solver.cb_mut().deadline = Instant::now() + Duration::from_millis(millis);
    let result = solver.solve_limited(&assumptions);
    let core: Vec<i32> = if explain_units && result == lbool::FALSE {
        assumptions
            .iter()
            .filter(|&&lit| solver.unsat_core_contains_var(lit.var()))
            .map(|lit| {
                let v = lit.var().idx() as i32 + 1;
                if lit.sign() { v } else { -v }
            })
            .collect()
    } else {
        Vec::new()
    };
    let solution = if result == lbool::TRUE {
        Some(
            (0..solver.num_vars())
                .map(|i| {
                    let v = solver.var_of_int(i);
                    if solver.value_lit(Lit::new(v, true)) == lbool::TRUE {
                        i as i32 + 1
                    } else {
                        -(i as i32 + 1)
                    }
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    let outcome = if result == lbool::TRUE {
        "sat"
    } else if result == lbool::FALSE {
        "unsat"
    } else {
        "budget"
    };
    Ok(
        serde_json::json!({"outcome":outcome,"seconds":started.elapsed().as_secs_f64(),"encoding_seconds":encoding_seconds,"callback_checks":solver.cb_mut().checks.get(),"solution":solution,"fixed_units":assumptions.len(),"unsat_core_units":core}),
    )
}
pub(crate) fn solve_reported(
    mut domains: Vec<u16>,
    rules: &[Rule],
    preferred: &[u8],
    cancel: &dyn Fn() -> bool,
    millis: u64,
) -> Result<Attempt> {
    let started = Instant::now();
    let mut stats = Statistics::default();
    macro_rules! finish {
        ($outcome:expr, $solution:expr) => {{
            stats.outcome = $outcome;
            stats.total_seconds = started.elapsed().as_secs_f64();
            return Ok(Attempt {
                solution: $solution,
                stats,
            });
        }};
    }
    check(cancel)?;
    if rules.len() > 32_000 {
        finish!("rule_limit", None);
    }
    let mut links = vec![Vec::new(); domains.len()];
    for (i, r) in rules.iter().enumerate() {
        for v in r.vars {
            links[v].push(i);
        }
    }
    let mut pending: VecDeque<_> = (0..rules.len()).collect();
    let mut queued = vec![true; rules.len()];
    let mut work = 0;
    while let Some(ri) = pending.pop_front() {
        work += 1;
        if work % 1024 == 0 {
            check(cancel)?;
        }
        queued[ri] = false;
        let r = &rules[ri];
        let mut support = [0u16; 3];
        for t in &r.tuples {
            if (0..3).all(|i| domains[r.vars[i]] & (1 << t[i]) != 0) {
                for i in 0..3 {
                    support[i] |= 1 << t[i];
                }
            }
        }
        for i in 0..3 {
            let next = domains[r.vars[i]] & support[i];
            if next == 0 {
                stats.propagation_seconds = started.elapsed().as_secs_f64();
                finish!("propagation_unsatisfiable", None);
            }
            if next != domains[r.vars[i]] {
                domains[r.vars[i]] = next;
                for &other in &links[r.vars[i]] {
                    if other != ri && !queued[other] {
                        pending.push_back(other);
                        queued[other] = true;
                    }
                }
            }
        }
    }
    stats.propagation_seconds = started.elapsed().as_secs_f64();
    if domains.iter().all(|d| d.count_ones() == 1) {
        finish!("propagation_solved", Some(domains));
    }
    // Budget the unresolved problem after propagation, rather than rejecting a
    // large patch whose dense source triangles mostly represent fixed zero edges.
    let values: usize = domains
        .iter()
        .filter(|d| d.count_ones() > 1)
        .map(|d| d.count_ones() as usize)
        .sum();
    stats.domain_values = values;
    if values > 120_000 {
        finish!("domain_limit", None);
    }
    let encoding_started = Instant::now();
    let budget = Budget {
        cancel,
        deadline: Instant::now() + Duration::from_millis(200),
        checks: Cell::new(0),
        max_checks: millis as usize * 250,
    };
    let mut solver = Solver::new(Default::default(), budget);
    let truth = Lit::new(solver.new_var(lbool::TRUE, false), true);
    solver.add_clause_reuse(&mut vec![truth]);
    let mut variables = vec![[!truth; 9]; domains.len()];
    for (i, &domain) in domains.iter().enumerate() {
        if domain.count_ones() == 1 {
            variables[i][domain.trailing_zeros() as usize] = truth;
            continue;
        }
        let mut values = Vec::new();
        for (value, slot) in variables[i].iter_mut().enumerate() {
            if domain & (1 << value) != 0 {
                let var = solver.new_var(lbool::new(value == preferred[i] as usize), true);
                let lit = Lit::new(var, true);
                *slot = lit;
                values.push(lit);
            }
        }
        for a in 0..values.len() {
            for b in 0..a {
                solver.add_clause_reuse(&mut vec![!values[a], !values[b]]);
            }
        }
        solver.add_clause_reuse(&mut values);
    }
    let mut tuple_count = 0;
    for (ri, r) in rules.iter().enumerate() {
        if ri % 64 == 0 {
            check(cancel)?;
        }
        let allowed: Vec<_> = r
            .tuples
            .iter()
            .filter(|t| (0..3).all(|i| domains[r.vars[i]] & (1 << t[i]) != 0))
            .collect();
        if allowed.len() > 1 {
            tuple_count += allowed.len();
            if tuple_count > 300_000 {
                stats.tuples = tuple_count;
                stats.encoding_seconds = encoding_started.elapsed().as_secs_f64();
                finish!("tuple_limit", None);
            }
        }
        if allowed.len() == 1 {
            for i in 0..3 {
                if !solver.add_clause_reuse(&mut vec![variables[r.vars[i]][allowed[0][i] as usize]])
                {
                    stats.encoding_seconds = encoding_started.elapsed().as_secs_f64();
                    finish!("encoding_unsatisfiable", None);
                }
            }
            continue;
        }
        let mut cases = Vec::new();
        for t in allowed {
            let case = Lit::new(solver.new_var(lbool::UNDEF, false), true);
            cases.push(case);
            for i in 0..3 {
                solver.add_clause_reuse(&mut vec![!case, variables[r.vars[i]][t[i] as usize]]);
            }
        }
        if !solver.add_clause_reuse(&mut cases) {
            stats.encoding_seconds = encoding_started.elapsed().as_secs_f64();
            finish!("encoding_unsatisfiable", None);
        }
    }
    stats.tuples = tuple_count;
    stats.encoding_seconds = encoding_started.elapsed().as_secs_f64();
    solver.cb_mut().deadline = Instant::now()
        + Duration::from_millis(millis.max(if rules.len() > 1500 { 500 } else { 200 }));
    let search_started = Instant::now();
    let result = solver.solve_limited(&[]);
    stats.search_seconds = search_started.elapsed().as_secs_f64();
    stats.callback_checks = solver.cb_mut().checks.get();
    check(cancel)?;
    if result != lbool::TRUE {
        let outcome = if result == lbool::FALSE {
            "search_unsatisfiable"
        } else if stats.callback_checks > solver.cb_mut().max_checks {
            "search_check_limit"
        } else if Instant::now() >= solver.cb_mut().deadline {
            "search_time_limit"
        } else {
            "search_incomplete"
        };
        finish!(outcome, None);
    }
    let mut solution = Vec::new();
    for row in variables {
        let value = row
            .iter()
            .position(|&lit| solver.value_lit(lit) == lbool::TRUE)
            .ok_or("Incomplete integer-grid SAT assignment")?;
        solution.push(1 << value);
    }
    // Validate the decoded assignment independently of the encoding/solver.
    if rules.iter().any(|r| {
        !r.tuples
            .iter()
            .any(|t| (0..3).all(|i| solution[r.vars[i]] & (1 << t[i]) != 0))
    }) {
        return Err("Integer-grid SAT assignment failed validation".into());
    }
    finish!("search_solved", Some(solution));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "audit")]
    #[test]
    fn audit_cnf_core_identifies_required_fixed_units() {
        let input = b"p cnf 2 3\n1 2 0\n-1 0\n-2 0\n";
        let result = solve_dimacs(input.as_slice(), 2000, true).unwrap();
        assert_eq!(result["outcome"], "unsat");
        let mut core = result["unsat_core_units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect::<Vec<_>>();
        core.sort_unstable();
        assert_eq!(core, vec![-2, -1]);
        assert_eq!(
            solve_dimacs(b"p cnf 2 2\n1 2 0\n-1 0\n".as_slice(), 2000, true).unwrap()["outcome"],
            "sat"
        );
    }
    #[test]
    fn diagnostics_distinguish_infeasibility_from_work_limits() {
        let rules = vec![Rule {
            vars: [0, 1, 2],
            tuples: vec![[0, 0, 0]],
        }];
        let failed = solve_reported(vec![2, 1, 1], &rules, &[0; 3], &|| false, 200).unwrap();
        assert_eq!(failed.stats.outcome, "propagation_unsatisfiable");
        let limited =
            solve_reported(vec![0x1ff; 14_000], &[], &vec![0; 14_000], &|| false, 200).unwrap();
        assert_eq!(limited.stats.outcome, "domain_limit");
        let solved = solve_reported(vec![3; 3], &rules, &[0; 3], &|| false, 200).unwrap();
        assert_eq!(solved.stats.outcome, "propagation_solved");
        assert_eq!(solved.solution, Some(vec![1; 3]));
    }
    #[test]
    fn learns_across_overlapping_local_constraints() {
        let rules = vec![
            Rule {
                vars: [0, 1, 2],
                tuples: vec![[0, 0, 1], [1, 1, 0]],
            },
            Rule {
                vars: [1, 2, 3],
                tuples: vec![[0, 0, 1], [1, 0, 1], [1, 1, 0]],
            },
        ];
        let s = solve(vec![3; 4], &rules, &[0; 4], &|| false)
            .unwrap()
            .unwrap();
        assert_eq!(s, vec![2, 2, 1, 2]);
        assert!(
            solve(vec![1, 3, 3, 3], &rules, &[0; 4], &|| false)
                .unwrap()
                .is_none()
        );
        assert!(
            solve(vec![3; 4], &rules, &[0; 4], &|| true)
                .unwrap_err()
                .contains("cancelled")
        );
    }
    #[test]
    fn encoding_matches_exhaustive_small_domain_problems() {
        for seed in 0..24 {
            let mut rules = Vec::new();
            for index in 0..4 {
                let mut tuples = Vec::new();
                for a in 0..3 {
                    for b in 0..3 {
                        for c in 0..3 {
                            if (seed * 17 + index * 23 + a * 11 + b * 7 + c * 3 + a * b * 13) % 19
                                < 8
                            {
                                tuples.push([a as u8, b as u8, c as u8]);
                            }
                        }
                    }
                }
                rules.push(Rule {
                    vars: [index, (index + 1) % 5, (index + 3) % 5],
                    tuples,
                });
            }
            let mut domains = vec![7; 5];
            if seed % 2 == 0 {
                domains[0] = 1;
            }
            let expected = (0..243).any(|mut code| {
                let mut a = [0u8; 5];
                for v in &mut a {
                    *v = (code % 3) as u8;
                    code /= 3;
                }
                (0..5).all(|i| domains[i] & (1 << a[i]) != 0)
                    && rules.iter().all(|r| {
                        r.tuples
                            .iter()
                            .any(|t| (0..3).all(|i| a[r.vars[i]] == t[i]))
                    })
            });
            let actual = solve(domains, &rules, &[0; 5], &|| false).unwrap();
            assert_eq!(actual.is_some(), expected, "SAT encoding seed {seed}");
        }
    }
}
