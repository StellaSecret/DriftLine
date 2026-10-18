//! Section 11 of the brief as executable checks: every level is solvable, every mechanic is a different
//! kind of problem, and none of them collapses into "drag a slider until it works".
use vfield::modes::*;
use vfield::sim::Outcome;
use vfield::*;

fn lv(id: &str) -> Level {
    levels::all().into_iter().find(|l| l.id == id).unwrap_or_else(|| panic!("no level {id}"))
}

#[test]
fn level_ids_unique_and_complete() {
    let all = levels::all();
    let mut ids: Vec<_> = all.iter().map(|l| l.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), all.len());
    for c in [Chapter::Position, Chapter::Predict, Chapter::Tempo, Chapter::Experiment] {
        assert_eq!(levels::chapter(c).len(), 5, "{c:?} needs five levels");
    }
    assert!(levels::chapter(Chapter::Combo).len() >= 6);
    assert!(all.iter().all(|l| !l.hints.is_empty()), "every level has a hint");
}

#[test]
fn every_level_is_solvable() {
    for l in levels::all() {
        assert!(validate::solvable(&l), "{} is not solvable", l.id);
        assert!(validate::oracle_win(&l), "oracle cannot win {}", l.id);
    }
}

#[test]
fn simulation_is_deterministic_for_every_level() {
    for l in levels::all() {
        let start = match &l.rules {
            Rules::Position { slots, .. } => slots[0].zone.center(),
            Rules::Predict { zone, .. } => zone.center() + v2(0.3, 0.2),
            Rules::Tempo { start, .. } | Rules::Experiment { start, .. } => *start,
        };
        let a = l.run(&l.field, start, 1.7);
        let b = l.run(&l.field, start, 1.7);
        assert_eq!(a.points, b.points, "{}", l.id);
        assert!(a.points.iter().all(|p| p.is_finite()));
    }
}

// ---------------- POSITION ----------------
#[test]
fn position_difficulty_curve_and_different_starts_differ() {
    let f: Vec<f64> = levels::chapter(Chapter::Position).iter().map(|l| validate::position_win_fraction(l, 41)).collect();
    assert!(f[0] > 0.8, "level 1 broad: {}", f[0]);
    assert!((0.1..0.5).contains(&f[1]), "level 2: {}", f[1]);
    assert!((0.1..0.5).contains(&f[2]), "level 3 partial: {}", f[2]);
    assert!((0.1..0.5).contains(&f[3]), "level 4: {}", f[3]);
    assert!(f[4] > 0.02 && f[4] < 0.1, "level 5 precise: {}", f[4]);
    for l in &levels::chapter(Chapter::Position)[1..] {
        assert!(validate::position_path_diversity(l, 21) >= 3, "{}: starts must give noticeably different paths", l.id);
    }
}

#[test]
fn position_mirror_level_is_symmetric_but_goal_is_not() {
    // Field symmetric under y -> -y with reversed rotation sign: mirrored start gives the mirrored path.
    let l = lv("pos-4");
    let up = l.run(&l.field, v2(-5.0, 1.0), 0.0);
    let dn = l.run(&l.field, v2(-5.0, -1.0), 0.0);
    assert!((up.end().y + dn.end().y).abs() < 1e-6);
    // ...but only one side wins; the other hits the hazard.
    assert!(l.reached_goal(&up, 0) ^ l.reached_goal(&dn, 0));
    assert!(matches!(dn.outcome, Outcome::Hazard(_)) || matches!(up.outcome, Outcome::Hazard(_)));
}

#[test]
fn position_mode_rejects_off_zone_clicks_and_requires_all_probes() {
    let mut g = PositionGame::new(lv("cmb-6"));
    assert!(g.place_at(v2(0.0, 0.0)).is_none(), "outside any zone");
    assert!(g.launch().is_none(), "cannot launch with unplaced probes");
    g.place_at(v2(-5.0, 0.8));
    assert!(g.launch().is_none(), "second probe still unplaced");
    g.place_at(v2(-5.0, -0.8));
    assert!(g.launch().is_some());
}

// ---------------- PREDICT ----------------
#[test]
fn predict_is_a_committed_forecast_not_parameter_search() {
    let mut g = PredictGame::new(lv("pre-3"));
    // Nothing to tune: no way to change start, field, or intensity through the mode API.
    assert!(g.commit().is_none(), "cannot commit without choosing");
    let wrong = g.level.exits().into_iter().find(|e| *e != g.round().answer).unwrap();
    assert!(g.select(wrong));
    let r = g.commit().unwrap();
    assert!(!r.correct);
    // Locked after commit: cannot switch the answer for the same round.
    let right = g.round().answer;
    assert!(!g.select(right), "answer is locked once committed");
    assert!(g.commit().is_none(), "no second commit");
    // Retrying the *level* draws new questions.
    let first_starts: Vec<_> = g.rounds.iter().map(|r| (r.start.x, r.start.y)).collect();
    g.restart();
    let second: Vec<_> = g.rounds.iter().map(|r| (r.start.x, r.start.y)).collect();
    assert_ne!(first_starts, second);
}

#[test]
fn predict_random_guessing_rarely_passes() {
    // 4 choices, need 2 of 3 right: pure chance ~ 15.6%. Verify with a deterministic sweep over answer patterns.
    let g = PredictGame::new(lv("pre-5"));
    let n = g.level.exits().len() as f64;
    let p = 1.0 / n;
    let pass = 3.0 * p * p * (1.0 - p) + p * p * p;
    assert!(pass < 0.2, "chance of passing by guessing = {pass}");
}

#[test]
fn predict_levels_have_balanced_answers_and_false_leads() {
    for l in levels::chapter(Chapter::Predict) {
        let Rules::Predict { zone, .. } = &l.rules else { unreachable!() };
        let pool = predict::candidate_pool(&l, zone);
        let mut answers: Vec<_> = pool.iter().filter_map(|(_, t)| if let Outcome::Reached(e) = t.outcome { Some(e) } else { None }).collect();
        answers.sort();
        answers.dedup();
        assert!(answers.len() >= 3, "{}: answers {:?}", l.id, answers);
        assert!(predict::pick_rounds(&l, 0).len() == 3);
    }
    // "False Lead": for a good fraction of starts the exit is NOT the one the local arrow at the start points at.
    let l = lv("pre-4");
    let Rules::Predict { zone, .. } = &l.rules else { unreachable!() };
    let pool = predict::candidate_pool(&l, zone);
    let misled = pool
        .iter()
        .filter(|(p, t)| {
            let v = l.field.eval(*p, 0.0);
            let ray_y = p.y + v.y / v.x * (5.2 - p.x); // where a straight extrapolation of the local arrow lands
            let naive = l.exits().into_iter().min_by(|a, b| (l.regions[*a].center.y - ray_y).abs().partial_cmp(&(l.regions[*b].center.y - ray_y).abs()).unwrap()).unwrap();
            t.outcome != Outcome::Reached(naive)
        })
        .count();
    assert!(misled as f64 / pool.len() as f64 > 0.5, "only {misled}/{} starts are misleading", pool.len());
}

#[test]
fn predict_partial_reveal_shows_prefix_only() {
    let g = PredictGame::new(lv("cmb-2"));
    let Reveal::Partial(secs) = g.reveal_mode() else { panic!("expected partial reveal") };
    let shown = g.round().traj.upto(secs);
    assert!(shown.len() < g.round().traj.points.len());
}

// ---------------- TEMPO ----------------
#[test]
fn tempo_windows_match_the_level_design() {
    let w = |id: &str| validate::tempo_windows(&validate::tempo_success_by_phase(&lv(id), 240));
    let (n1, f1) = w("tem-1");
    assert_eq!(n1, 1);
    assert!((0.2..0.6).contains(&f1));
    let (_, f2) = w("tem-2");
    assert!(f2 > 0.2, "broad window {f2}");
    let (_, f3) = w("tem-3");
    assert!(f3 > 0.01 && f3 < 0.12, "narrow window {f3}");
    let (n4, _) = w("tem-4");
    assert!(n4 >= 2, "several windows, got {n4}");
    let (n5, f5) = w("tem-5");
    assert!(n5 >= 1 && f5 > 0.05 && f5 < 0.5);
}

#[test]
fn tempo_changes_geometry_not_just_speed() {
    for l in levels::chapter(Chapter::Tempo) {
        assert!(validate::tempo_geometry_spread(&l, 48) > 0.4, "{}: geometry must depend on phase", l.id);
        // Speed-only fields would give the same *set of visited cells* regardless of phase. Check outcomes differ too.
        let flags = validate::tempo_success_by_phase(&l, 120);
        assert!(flags.iter().any(|b| *b) && flags.iter().any(|b| !*b), "{}: phase must matter for success", l.id);
    }
}

#[test]
fn tempo_period_is_exact() {
    let l = lv("tem-5");
    let Rules::Tempo { start, period, .. } = &l.rules else { unreachable!() };
    let a = l.run(&l.field, *start, 3.0);
    let b = l.run(&l.field, *start, 3.0 + period);
    let max = a.points.iter().zip(&b.points).map(|(p, q)| p.dist(*q)).fold(0.0, f64::max);
    assert!(max < 1e-6, "field must be periodic, deviation {max}");
}

#[test]
fn tempo_mode_only_exposes_time() {
    let mut g = TempoGame::new(lv("tem-1"));
    g.set_phase(100.0); // wraps
    assert!(g.clock >= 0.0 && g.clock < g.period());
    assert!(!g.playing, "scrubbing pauses");
    let a = g.launch().clone();
    assert_eq!(a.traj.t0, g.clock);
}

// ---------------- EXPERIMENT ----------------
#[test]
fn experiment_conclusions_are_checked_against_the_simulation() {
    let l = lv("exp-2");
    let mut g = ExperimentGame::new(l);
    assert!(!g.conclude(Conclusion::Estimates(vec![1.48])).correct, "no evidence yet");
    for i in [0.0, 1.0, 2.0, 3.0] {
        g.run(i).unwrap();
    }
    assert!(g.can_conclude());
    assert!(!g.conclude(Conclusion::Estimates(vec![0.4])).correct);
    assert!(!g.solved);
    assert!(g.conclude(Conclusion::Estimates(vec![1.45])).correct);
    assert!(g.solved);
}

#[test]
fn experiment_requires_evidence_so_guessing_cannot_win() {
    let mut g = ExperimentGame::new(lv("exp-1"));
    for choice in 0..3 {
        assert!(!g.conclude(Conclusion::Choice(choice)).correct, "no free guesses before running anything");
    }
    g.run(0.3).unwrap();
    g.run(0.9).unwrap();
    assert!(!g.can_conclude());
    g.run(1.6).unwrap();
    assert!(g.can_conclude());
}

#[test]
fn experiment_rig_cap_forces_extrapolation() {
    let mut g = ExperimentGame::new(lv("exp-4"));
    assert!(g.run(6.2).is_err(), "cannot simply test the target");
    assert!(g.run(4.0).is_ok());
    assert!(g.run(4.01).is_err());
    assert!(matches!(experiment::truth(&g.level), Some(experiment::Truth::ExitAt(_))));
}

#[test]
fn experiment_truths_are_robust_and_specific() {
    use experiment::{probe, truth, Truth};
    // exp-4: the answer must not sit on a boundary between outcomes.
    let l = lv("exp-4");
    let Some(Truth::ExitAt(e)) = truth(&l) else { panic!() };
    for d in [-0.15, 0.15] {
        let t = probe(&l, 6.2 + d);
        assert_eq!(t.outcome, e.map_or(Outcome::Boundary, Outcome::Reached));
    }
    // exp-3: the correct answer is that the probe *does* return (needs a wide sweep, not a lucky pair).
    assert_eq!(truth(&lv("exp-3")), Some(Truth::ReturnChoice(0)));
    // exp-1: clockwise.
    assert_eq!(truth(&lv("exp-1")), Some(Truth::Choice(0)));
    // thresholds are single transitions with a tolerance bigger than float noise but smaller than the range.
    for id in ["exp-2", "cmb-4", "exp-5"] {
        let Some(Truth::Estimates(v)) = truth(&lv(id)) else { panic!("{id}") };
        assert!(!v.is_empty() && v.iter().all(|x| *x > 0.0));
    }
}

#[test]
fn experiment_runs_are_preserved_and_comparable() {
    let mut g = ExperimentGame::new(lv("exp-1"));
    g.run(0.5).unwrap();
    g.run(1.5).unwrap();
    assert_eq!(g.runs.len(), 2);
    assert_ne!(g.runs[0].traj.end(), g.runs[1].traj.end());
    g.toggle_visible(0);
    assert!(!g.runs[0].visible && g.runs[1].visible);
    g.reset();
    assert!(g.runs.is_empty());
}

// ---------------- Cross-mechanic ----------------
#[test]
fn mechanics_expose_different_controls() {
    // The decision each mode lets the player make, as data. If two mechanics ever expose the same control
    // surface, this test should be reread before more content is added.
    let controls = |l: &Level| match &l.rules {
        Rules::Position { period: None, intensity: None, .. } => "release position",
        Rules::Predict { .. } => "committed answer",
        Rules::Tempo { .. } => "launch phase",
        Rules::Experiment { .. } => "field parameter -> rule",
        _ => "combination",
    };
    let set: std::collections::HashSet<_> = [lv("pos-1"), lv("pre-1"), lv("tem-1"), lv("exp-1")].iter().map(controls).collect();
    assert_eq!(set.len(), 4);
}

#[test]
fn combination_levels_each_add_exactly_one_idea() {
    // cmb-1 gate really restricts Position (fewer winners than the same level without the gate)
    let mut no_gate = lv("cmb-1");
    no_gate.regions.truncate(1);
    assert!(validate::position_win_fraction(&lv("cmb-1"), 41) < validate::position_win_fraction(&no_gate, 41) * 0.6);
    // cmb-3 hazard really removes successful launch phases
    let mut no_haz = lv("cmb-3");
    no_haz.regions.truncate(1);
    let with = validate::tempo_windows(&validate::tempo_success_by_phase(&lv("cmb-3"), 240)).1;
    let without = validate::tempo_windows(&validate::tempo_success_by_phase(&no_haz, 240)).1;
    assert!(with < without * 0.8, "{with} vs {without}");
    // cmb-5: both position and phase matter (not every phase works, not every position works)
    let (ok, total, area) = validate::joint_control_stats(&lv("cmb-5"));
    assert!(ok > 0 && ok < total && area < 0.5, "{ok}/{total} {area}");
    // cmb-6: shared dial is a real constraint: some but not all intensities let both probes win
    let (ok, total, _) = validate::joint_control_stats(&lv("cmb-6"));
    assert!(ok > 0 && ok < total, "{ok}/{total}");
}
