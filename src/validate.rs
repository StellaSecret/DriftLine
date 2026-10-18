//! Automated evidence that levels are solvable and that the mechanics differ.
use crate::level::{Level, Rules};
use crate::modes::{experiment, predict, ExperimentGame, PositionGame, TempoGame};
use crate::sim::Outcome;

/// Fraction of a Position level's (single) release zone that wins, on an n x n grid.
pub fn position_win_fraction(level: &Level, n: usize) -> f64 {
    let Rules::Position { slots, .. } = &level.rules else { return 0.0 };
    let s = &slots[0];
    let g = s.zone.grid(n);
    let wins = g.iter().filter(|p| level.reached_goal(&level.run(&level.field, **p, 0.0), s.goal)).count();
    wins as f64 / g.len().max(1) as f64
}

/// Distinct end sectors (quadrant of the final point) over a grid: proxy for "different starts, different paths".
pub fn position_path_diversity(level: &Level, n: usize) -> usize {
    let Rules::Position { slots, .. } = &level.rules else { return 0 };
    let mut seen = std::collections::HashSet::new();
    for p in slots[0].zone.grid(n) {
        let t = level.run(&level.field, p, 0.0);
        let e = t.end();
        seen.insert((matches!(t.outcome, Outcome::Reached(_)), e.x > 0.0, e.y > 0.0));
    }
    seen.len()
}

/// Success flag for every launch phase in [0, period).
pub fn tempo_success_by_phase(level: &Level, n: usize) -> Vec<bool> {
    let Rules::Tempo { start, period, goal } = &level.rules else { return vec![] };
    (0..n).map(|i| level.reached_goal(&level.run(&level.field, *start, period * i as f64 / n as f64), *goal)).collect()
}

/// Number of contiguous (cyclic) success windows and the success fraction.
pub fn tempo_windows(flags: &[bool]) -> (usize, f64) {
    let n = flags.len();
    let windows = (0..n).filter(|&i| flags[i] && !flags[(i + n - 1) % n]).count();
    (windows, flags.iter().filter(|b| **b).count() as f64 / n as f64)
}

/// Fraction of launch phases whose trajectory *shape* differs from phase 0 by more than 1 unit at some
/// common arc-length fraction (Tempo must change geometry, not just speed).
pub fn tempo_geometry_spread(level: &Level, n: usize) -> f64 {
    let Rules::Tempo { start, period, .. } = &level.rules else { return 0.0 };
    let base = level.run(&level.field, *start, 0.0);
    let arc = |t: &crate::sim::Trajectory| -> Vec<f64> {
        let mut acc = vec![0.0];
        for w in t.points.windows(2) {
            acc.push(acc.last().unwrap() + w[0].dist(w[1]));
        }
        acc
    };
    let at_frac = |t: &crate::sim::Trajectory, f: f64| {
        let a = arc(t);
        let target = a.last().unwrap() * f;
        let i = a.iter().position(|x| *x >= target).unwrap_or(a.len() - 1);
        t.points[i]
    };
    let mut differ = 0;
    for i in 0..n {
        let t = level.run(&level.field, *start, period * i as f64 / n as f64);
        let d = (0..=4).map(|k| at_frac(&base, 0.25 * k as f64).dist(at_frac(&t, 0.25 * k as f64))).fold(0.0, f64::max);
        if d > 1.0 {
            differ += 1;
        }
    }
    differ as f64 / n as f64
}

/// Is the level winnable at all? (Searches the control space the mode exposes.)
pub fn solvable(level: &Level) -> bool {
    match &level.rules {
        Rules::Position { slots, period, intensity } => {
            let phases: Vec<f64> = match period {
                Some(p) => (0..24).map(|i| p * i as f64 / 24.0).collect(),
                None => vec![0.0],
            };
            let ints: Vec<f64> = match intensity {
                Some((a, b)) => (0..13).map(|i| a + (b - a) * i as f64 / 12.0).collect(),
                None => vec![level.field.intensity],
            };
            ints.iter().any(|&i| {
                let f = level.field.with_intensity(i);
                phases.iter().any(|&ph| {
                    slots.iter().all(|s| s.zone.grid(45).iter().any(|p| level.reached_goal(&level.run(&f, *p, ph), s.goal)))
                })
            })
        }
        Rules::Predict { zone, rounds, pass, .. } => {
            let pool = predict::candidate_pool(level, zone);
            let exits: std::collections::HashSet<_> = pool.iter().filter_map(|(_, t)| if let Outcome::Reached(e) = t.outcome { Some(e) } else { None }).collect();
            // need enough stable starts and at least two different answers (otherwise a guess always wins)
            pool.len() >= *rounds * 3 && exits.len() >= 2 && predict::pick_rounds(level, 0).len() == *rounds && *pass <= *rounds
        }
        Rules::Tempo { .. } => tempo_success_by_phase(level, 240).iter().any(|b| *b),
        Rules::Experiment { .. } => experiment::truth(level).is_some(),
    }
}

/// For Position levels with extra controls (phase and/or shared intensity): over a grid of control
/// settings, how many settings admit a winning release for every probe, and the mean fraction of
/// release area that wins per slot.
pub fn joint_control_stats(level: &Level) -> (usize, usize, f64) {
    let Rules::Position { slots, period, intensity } = &level.rules else { return (0, 0, 0.0) };
    let phases: Vec<f64> = match period { Some(p) => (0..12).map(|i| p * i as f64 / 12.0).collect(), None => vec![0.0] };
    let ints: Vec<f64> = match intensity { Some((a, b)) => (0..13).map(|i| a + (b - a) * i as f64 / 12.0).collect(), None => vec![level.field.intensity] };
    let (mut ok, mut total, mut area, mut cells) = (0, 0, 0.0, 0.0);
    for &i in &ints {
        let f = level.field.with_intensity(i);
        for &ph in &phases {
            total += 1;
            let mut all = true;
            for s in slots {
                let g = s.zone.grid(21);
                let w = g.iter().filter(|p| level.reached_goal(&level.run(&f, **p, ph), s.goal)).count();
                area += w as f64 / g.len() as f64;
                cells += 1.0;
                all &= w > 0;
            }
            if all { ok += 1 }
        }
    }
    (ok, total, area / cells)
}

/// One-line machine-readable summary used by the `report` binary.
pub fn summarize(level: &Level) -> String {
    match &level.rules {
        Rules::Position { slots, period, intensity } if slots.len() == 1 && period.is_none() && intensity.is_none() => {
            format!("win-fraction {:>5.1}%  path-diversity {}", 100.0 * position_win_fraction(level, 41), position_path_diversity(level, 21))
        }
        Rules::Position { .. } => {
            let (ok, total, frac) = joint_control_stats(level);
            format!("{} control settings admit a win out of {} ({:.0}%); mean per-slot win area {:.1}%", ok, total, 100.0 * ok as f64 / total as f64, 100.0 * frac)
        }
        Rules::Predict { zone, .. } => {
            let pool = predict::candidate_pool(level, zone);
            let mut exits: Vec<_> = pool.iter().filter_map(|(_, t)| if let Outcome::Reached(e) = t.outcome { Some(level.regions[e].label) } else { None }).collect();
            exits.sort();
            exits.dedup();
            format!("stable starts {:>3}, distinct answers {:?}", pool.len(), exits)
        }
        Rules::Tempo { .. } => {
            let f = tempo_success_by_phase(level, 240);
            let (w, frac) = tempo_windows(&f);
            format!("windows {}  success-phase {:>5.1}%  geometry-spread {:>5.1}%", w, 100.0 * frac, 100.0 * tempo_geometry_spread(level, 60))
        }
        Rules::Experiment { .. } => format!("truth = {:?}", experiment::truth(level)),
    }
}

/// Convenience used by tests: play a level to a win with an oracle (no UI).
pub fn oracle_win(level: &Level) -> bool {
    match &level.rules {
        Rules::Position { slots, .. } if slots.len() == 1 && matches!(&level.rules, Rules::Position { period: None, intensity: None, .. }) => {
            let mut g = PositionGame::new(level.clone());
            for p in slots[0].zone.grid(31) {
                g.place_at(p);
                if g.launch().is_some_and(|a| a.success) {
                    return true;
                }
            }
            false
        }
        Rules::Tempo { period, .. } => {
            let mut g = TempoGame::new(level.clone());
            for i in 0..240 {
                g.set_phase(period * i as f64 / 240.0);
                if g.launch().success {
                    return true;
                }
            }
            false
        }
        Rules::Experiment { range, .. } => {
            let mut g = ExperimentGame::new(level.clone());
            let n = 13;
            for i in 0..n {
                let _ = g.run(range.0 + (range.1 - range.0) * i as f64 / (n - 1) as f64);
            }
            // The oracle answers from ground truth.
            use experiment::Truth;
            match experiment::truth(level) {
                Some(Truth::Choice(c)) | Some(Truth::ReturnChoice(c)) => g.conclude(crate::modes::Conclusion::Choice(c)).correct,
                Some(Truth::ExitAt(e)) => {
                    let c = match e {
                        Some(e) => level.exits().iter().position(|x| *x == e).unwrap(),
                        None => level.exits().len(),
                    };
                    g.conclude(crate::modes::Conclusion::Choice(c)).correct
                }
                Some(Truth::Estimates(v)) => g.conclude(crate::modes::Conclusion::Estimates(v)).correct,
                None => false,
            }
        }
        _ => solvable(level),
    }
}
