//! Scalar-coordinate encoding of regularity and nonnegative grid area.
//! Adapted from QuadriFlow localsat.cpp at 810b7a0967c35b0dc85b4464e3835e26a756c967;
//! see LICENSE-QuadriFlow. Uses the existing Rust engine, cancellation and bounds.
use super::{Attempt, Budget, Statistics};
use crate::{Result, check};
use batsat::{Lit, Solver, SolverInterface, lbool};
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

/// The variable and sign of each rotated edge coordinate.
fn components(edge: usize, rotation: i32) -> [(usize, i32); 2] {
    let [x, y] = [2 * edge, 2 * edge + 1];
    match rotation.rem_euclid(4) {
        0 => [(x, 1), (y, 1)],
        1 => [(y, -1), (x, 1)],
        2 => [(x, -1), (y, -1)],
        _ => [(y, 1), (x, -1)],
    }
}

pub(crate) fn solve(
    domains: &[u16],
    faces: &[[usize; 3]],
    rotations: &[[i32; 3]],
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
    if faces.len() > 32_000 {
        finish!("rule_limit", None);
    }
    stats.domain_values = domains
        .iter()
        .filter(|d| d.count_ones() > 1)
        .map(|d| d.count_ones() as usize)
        .sum();
    if stats.domain_values > 120_000 {
        finish!("domain_limit", None);
    }
    let budget = Budget {
        cancel,
        deadline: started + Duration::from_millis(millis),
        checks: Cell::new(0),
        max_checks: millis as usize * 250,
    };
    let mut solver = Solver::new(Default::default(), budget);
    // Signed literals reproduce upstream's preferred-value transformation.
    // Keep the engine's normal phase saving rather than forcing each decision.
    let variables: Vec<[Lit; 3]> = (0..2 * domains.len())
        .map(|i| {
            let p = if i % 2 == 0 {
                preferred[i / 2] / 3
            } else {
                preferred[i / 2] % 3
            };
            std::array::from_fn(|v| Lit::new(solver.new_var_default(), v != usize::from(p)))
        })
        .collect();
    stats.boolean_variables = variables.len() * 3;
    macro_rules! clause {
        ($literals:expr) => {{
            stats.clauses += 1;
            if stats.clauses > 1_000_000 {
                stats.encoding_seconds = started.elapsed().as_secs_f64();
                finish!("clause_limit", None);
            }
            if !solver.add_clause_reuse(&mut $literals) {
                stats.encoding_seconds = started.elapsed().as_secs_f64();
                finish!("encoding_unsatisfiable", None);
            }
        }};
    }
    for row in &variables {
        clause!(vec![!row[0], !row[1]]);
        clause!(vec![!row[2], !row[1]]);
        clause!(vec![!row[0], !row[2]]);
        clause!(row.to_vec());
    }
    for (e, &domain) in domains.iter().enumerate() {
        if e % 1024 == 0 {
            check(cancel)?;
        }
        // Expose fixed/limited axes directly, as in upstream. Encoding a fixed
        // vector only as eight forbidden pairs hides its unit consequences.
        let mut axes = [[false; 3]; 2];
        for value in 0..9 {
            if domain & (1 << value) != 0 {
                axes[0][value / 3] = true;
                axes[1][value % 3] = true;
            }
        }
        for axis in 0..2 {
            if !axes[axis].iter().all(|&v| v) {
                clause!(
                    (0..3)
                        .filter(|&v| axes[axis][v])
                        .map(|v| variables[2 * e + axis][v])
                        .collect::<Vec<_>>()
                );
            }
        }
        // Preserve correlated 2D domains as well as independent axis locks.
        for value in 0..9 {
            if axes[0][value / 3] && axes[1][value % 3] && domain & (1 << value) == 0 {
                clause!(vec![
                    !variables[2 * e][value / 3],
                    !variables[2 * e + 1][value % 3]
                ]);
            }
        }
    }
    for (i, (&face, &rotation)) in faces.iter().zip(rotations).enumerate() {
        if i % 32 == 0 {
            check(cancel)?;
        }
        let c: [[(usize, i32); 2]; 3] = std::array::from_fn(|k| components(face[k], rotation[k]));
        for axis in 0..2 {
            for a in 0..3 {
                for b in 0..3 {
                    for d in 0..3 {
                        if c[0][axis].1 * (a as i32 - 1)
                            + c[1][axis].1 * (b as i32 - 1)
                            + c[2][axis].1 * (d as i32 - 1)
                            != 0
                        {
                            clause!(vec![
                                !variables[c[0][axis].0][a],
                                !variables[c[1][axis].0][b],
                                !variables[c[2][axis].0][d]
                            ]);
                        }
                    }
                }
            }
        }
        for a in 0..3 {
            for b in 0..3 {
                for d in 0..3 {
                    for e in 0..3 {
                        let area = c[0][0].1 * c[1][1].1 * (a as i32 - 1) * (b as i32 - 1)
                            - c[0][1].1 * c[1][0].1 * (d as i32 - 1) * (e as i32 - 1);
                        if area < 0 {
                            clause!(vec![
                                !variables[c[0][0].0][a],
                                !variables[c[1][1].0][b],
                                !variables[c[0][1].0][d],
                                !variables[c[1][0].0][e]
                            ]);
                        }
                    }
                }
            }
        }
    }
    stats.encoding_seconds = started.elapsed().as_secs_f64();
    solver.cb_mut().deadline = Instant::now()
        + Duration::from_millis(millis.max(if faces.len() > 1500 { 500 } else { 200 }));
    let search = Instant::now();
    let result = solver.solve_limited(&[]);
    stats.search_seconds = search.elapsed().as_secs_f64();
    stats.callback_checks = solver.cb_mut().checks.get();
    check(cancel)?;
    if result != lbool::TRUE {
        let outcome = if result == lbool::FALSE {
            "search_unsatisfiable"
        } else if stats.callback_checks > solver.cb_mut().max_checks {
            "search_check_limit"
        } else {
            "search_time_limit"
        };
        finish!(outcome, None);
    }
    let values: Vec<_> = variables
        .iter()
        .map(|row| {
            row.iter()
                .position(|&v| solver.value_lit(v) == lbool::TRUE)
                .ok_or("Incomplete scalar grid assignment".into())
        })
        .collect::<Result<_>>()?;
    let solution: Vec<u16> = values
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| 1 << (v[0] * 3 + v[1]))
        .collect();
    // Independent geometric/domain verification, including fixed boundaries.
    if solution.iter().zip(domains).any(|(&s, &d)| s & d == 0)
        || faces.iter().zip(rotations).any(|(&f, &r)| {
            let vectors: [glam::IVec2; 3] = std::array::from_fn(|k| {
                let v = solution[f[k]].trailing_zeros() as i32;
                crate::coarse::rot(glam::IVec2::new(v / 3 - 1, v % 3 - 1), r[k])
            });
            vectors.into_iter().sum::<glam::IVec2>() != glam::IVec2::ZERO
                || vectors[0].x * vectors[1].y - vectors[0].y * vectors[1].x < 0
        })
    {
        return Err("Scalar grid assignment failed geometric validation".into());
    }
    finish!("search_solved", Some(solution));
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec2;

    fn valid(values: &[u8], faces: &[[usize; 3]], rotations: &[[i32; 3]]) -> bool {
        faces.iter().zip(rotations).all(|(f, r)| {
            let v: [IVec2; 3] = std::array::from_fn(|k| {
                let a = i32::from(values[f[k]]);
                crate::coarse::rot(IVec2::new(a / 3 - 1, a % 3 - 1), r[k])
            });
            v.into_iter().sum::<IVec2>() == IVec2::ZERO && v[0].perp_dot(v[1]) >= 0
        })
    }
    fn brute(
        domains: &[u16],
        faces: &[[usize; 3]],
        rotations: &[[i32; 3]],
        values: &mut Vec<u8>,
    ) -> bool {
        if values.len() == domains.len() {
            return valid(values, faces, rotations);
        }
        for v in 0..9 {
            if domains[values.len()] & (1 << v) == 0 {
                continue;
            }
            values.push(v);
            if brute(domains, faces, rotations, values) {
                return true;
            }
            values.pop();
        }
        false
    }
    #[test]
    fn scalar_grid_matches_exhaustive_domains_frames_and_repeated_edges() {
        let mut state = 1234567u32;
        let mut rand = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let mut satisfiable = 0;
        for case in 0..96 {
            let faces = if case % 8 == 0 {
                [[0, 1, 0], [1, 2, 3]]
            } else {
                [[0, 1, 2], [2, 3, 4]]
            };
            let rotations =
                std::array::from_fn::<_, 2, _>(|_| std::array::from_fn(|_| (rand() % 4) as i32));
            let domains: Vec<u16> = (0..5)
                .map(|_| {
                    if case % 4 == 0 {
                        0x1ff
                    } else {
                        (rand() as u16 & 0x1ff).max(1)
                    }
                })
                .collect();
            let preferred: Vec<_> = domains.iter().map(|d| d.trailing_zeros() as u8).collect();
            let expected = brute(&domains, &faces, &rotations, &mut Vec::new());
            let attempt = solve(&domains, &faces, &rotations, &preferred, &|| false, 2000).unwrap();
            assert_eq!(
                attempt.solution.is_some(),
                expected,
                "case {case}: {}",
                attempt.stats.outcome
            );
            if let Some(solution) = attempt.solution {
                satisfiable += 1;
                assert!(solution.iter().zip(&domains).all(|(&s, &d)| s & d != 0));
                assert!(valid(
                    &solution
                        .iter()
                        .map(|s| s.trailing_zeros() as u8)
                        .collect::<Vec<_>>(),
                    &faces,
                    &rotations
                ));
            } else {
                assert!(attempt.stats.outcome.ends_with("unsatisfiable"));
            }
        }
        assert!(satisfiable > 0 && satisfiable < 96);
    }
    #[cfg(feature = "audit")]
    #[test]
    fn frozen_junction_fixed_boundary_is_infeasible() {
        let patch: crate::audit::RepairPatch =
            serde_json::from_str(include_str!("../../tests/data/repair-junction-fixed.json"))
                .unwrap();
        let result = solve(
            &patch.domains,
            &patch.faces,
            &patch.rotations,
            &patch.preferred,
            &|| false,
            8000,
        )
        .unwrap();
        assert!(result.solution.is_none());
        assert!(result.stats.outcome.ends_with("unsatisfiable"));
    }
    #[test]
    fn scalar_grid_obeys_cancellation_and_size_limits() {
        assert!(solve(&[], &[], &[], &[], &|| true, 200).is_err());
        assert_eq!(
            solve(&[], &vec![[0; 3]; 32001], &[], &[], &|| false, 200)
                .unwrap()
                .stats
                .outcome,
            "rule_limit"
        );
        assert_eq!(
            solve(&vec![0x1ff; 14000], &[], &[], &[], &|| false, 200)
                .unwrap()
                .stats
                .outcome,
            "domain_limit"
        );
    }
}
