//! Survey: simulation correctness of the hidden source, the rules of the mode, and level validation
//! (the hypothesis-partition model of `validate_survey`, see docs/09-survey.md).
use vfield::modes::{CommitError, PlaceError, RetryError, SurveyGame, SurveySpec, MIN_SCOUT_SEPARATION, PUSH_EVERY, TestError, TESTS_PER_ROUND};
use vfield::sim::Outcome;
use vfield::validate_survey as vs;
use vfield::*;

fn level() -> Level {
    levels::survey_levels().remove(0)
}
fn game() -> SurveyGame {
    SurveyGame::new(level())
}
/// Same level with different rules parameters (tests of the validator's ability to say "no").
fn with(f: impl FnOnce(&mut f64, &mut Vec<V2>)) -> Level {
    let mut l = level();
    if let Rules::Survey { k, pool, kinds, .. } = &mut l.rules {
        f(k, pool);
        kinds.resize(pool.len(), SourceKind::Repeller);
    }
    l
}

// ---------------- the Gauss term ----------------
#[test]
fn gauss_is_a_local_radial_push_peaking_at_sigma() {
    let (k, sigma) = (1.6, 1.0);
    let f = Field::new().fixed(Term::Gauss { c: v2(0.0, 0.0), k, sigma });
    let at = |d: f64| f.eval(v2(d, 0.0), 0.0);
    // zero at the centre, outward (positive x) everywhere to the right, inward-symmetric on the left
    assert!(at(0.0).len() < 1e-12);
    assert!(at(0.7).x > 0.0 && at(2.0).x > 0.0);
    assert!(f.eval(v2(-1.0, 0.0), 0.0).x < 0.0);
    // peak speed k sigma e^(-1/2) at distance sigma, and it is the maximum
    let peak = k * sigma * (-0.5f64).exp();
    assert!((at(sigma).len() - peak).abs() < 1e-12);
    for d in [0.3, 0.6, 0.9, 1.1, 1.5, 2.0, 3.0] {
        assert!(at(d).len() <= peak + 1e-12, "d = {d}");
    }
    // local: at 4 sigma it is below 0.5% of the peak (unlike Radial, which is still ~1/r there)
    assert!(at(3.0 * sigma).len() > 0.04 * peak, "still 5.5% of the peak at 3 sigma: not negligible");
    assert!(at(4.0 * sigma).len() < 5e-3 * peak, "{}", at(4.0).len());
    let r = Field::new().fixed(Term::Radial { c: v2(0.0, 0.0), k, soft: 0.6 });
    assert!(r.eval(v2(4.0, 0.0), 0.0).len() > 100.0 * at(4.0).len());
    // negative k attracts
    let a = Field::new().fixed(Term::Gauss { c: v2(0.0, 0.0), k: -k, sigma });
    assert!(a.eval(v2(1.0, 0.0), 0.0).x < 0.0);
}

#[test]
fn the_true_field_is_the_chart_plus_the_source() {
    let g = game();
    let src = v2(1.75, 1.75);
    let tf = g.spec.true_field(src);
    for p in [v2(0.0, 0.0), v2(1.0, 1.0), v2(2.0, 2.5), v2(-3.0, -1.0)] {
        let chart = g.spec.field.eval(p, 0.0);
        let own = Field::new().fixed(Term::Gauss { c: src, k: g.spec.k, sigma: g.spec.sigma }).eval(p, 0.0);
        let sum = tf.eval(p, 0.0);
        assert!((sum.x - (chart.x + own.x)).abs() < 1e-12 && (sum.y - (chart.y + own.y)).abs() < 1e-12);
    }
    // and the chart itself never contains the source
    assert_eq!(g.spec.field.layers.len(), 1);
}

#[test]
fn a_distant_source_leaves_a_scout_on_the_charted_path_and_a_near_one_bends_it() {
    let spec = SurveySpec::from_level(&level());
    let free = spec.free;
    let straight = spec.fly(v2(30.0, 30.0), free);
    let end = straight.end();
    // the chart is a uniform stream at (1, 0): after 5 s the scout is 5 units downstream
    assert!((end.x - (free.x + 5.0)).abs() < 1e-3 && (end.y - free.y).abs() < 1e-3, "{end:?}");
    // a repeller just above the path pushes the scout down, one just below pushes it up
    let above = spec.fly(v2(-3.0, 1.0), free).end();
    let below = spec.fly(v2(-3.0, -1.0), free).end();
    assert!(above.y < -0.3 && below.y > 0.3, "{above:?} {below:?}");
    // a source 5 sigma away is invisible at the precision a player can use
    assert!(spec.fly(v2(-3.0, 5.0), free).end().dist(end) < 0.02);
}

#[test]
fn no_scout_ever_comes_to_rest_so_a_sink_cannot_give_the_answer_away() {
    // The first design flaw to avoid: any term that stalls a probe reveals its own position (Outcome::Stalled).
    let l = level();
    let spec = SurveySpec::from_level(&l);
    let m = vs::Model::new(&l);
    for &src in &spec.pool {
        for &s in &m.starts {
            let t = spec.fly(src, s);
            assert!(!matches!(t.outcome, Outcome::Stalled | Outcome::Invalid), "scout from {s:?} stalled with the source at {src:?}");
        }
    }
}

// ---------------- the rules of the mode ----------------
#[test]
fn a_round_starts_with_only_the_free_scout_and_nothing_revealed() {
    let g = game();
    let r = g.round();
    assert_eq!(r.scouts.len(), 1);
    assert!(r.scouts[0].free && r.scouts[0].start == g.spec.free);
    assert_eq!(r.paid(), 0);
    assert_eq!(g.scouts_left(), g.spec.budget);
    assert!(r.marker.is_none() && r.verdict.is_none() && r.revealed_source().is_none());
    assert!(!g.finished() && !g.won && g.attempts() == 1);
}

#[test]
fn the_scout_budget_is_enforced_and_the_free_scout_is_not_part_of_it() {
    let mut g = game();
    let budget = g.spec.budget;
    for i in 0..budget {
        assert!(g.place_scout(v2(-3.0 + 2.0 * i as f64, 2.0)).is_ok());
        assert_eq!(g.scouts_left(), budget - 1 - i);
    }
    assert_eq!(g.round().scouts.len(), budget + 1);
    assert_eq!(g.place_scout(v2(0.0, -2.5)).err(), Some(PlaceError::NoScouts));
    assert_eq!(g.round().scouts.len(), budget + 1, "a rejected scout must change nothing");
}

#[test]
fn scouts_must_be_inside_the_zone_and_may_not_repeat_a_trail() {
    let mut g = game();
    assert_eq!(g.place_scout(v2(9.0, 0.0)).err(), Some(PlaceError::OutOfZone));
    assert_eq!(g.place_scout(v2(0.0, 3.9)).err(), Some(PlaceError::OutOfZone));
    let free = g.spec.free;
    assert_eq!(g.place_scout(free).err(), Some(PlaceError::Duplicate));
    assert_eq!(g.place_scout(v2(free.x + 0.5 * MIN_SCOUT_SEPARATION, free.y)).err(), Some(PlaceError::Duplicate));
    assert_eq!(g.scouts_left(), g.spec.budget, "rejected placements are free");
    assert!(g.place_scout(v2(free.x + 2.0 * MIN_SCOUT_SEPARATION, free.y)).is_ok());
}

#[test]
fn scouts_are_deterministic_and_do_not_affect_each_other() {
    let (a, b) = (v2(-2.0, 1.5), v2(1.0, -2.0));
    let mut one = game();
    one.place_scout(a).unwrap();
    let alone = one.round().scouts[1].traj.clone();
    let mut two = game();
    two.place_scout(b).unwrap();
    two.place_scout(a).unwrap();
    assert_eq!(alone.points, two.round().scouts[2].traj.points, "a scout's trail must not depend on the others");
    let mut again = game();
    again.place_scout(a).unwrap();
    assert_eq!(alone.points, again.round().scouts[1].traj.points);
}

#[test]
fn the_source_stays_hidden_until_the_commit() {
    let mut g = game();
    g.place_scout(v2(0.0, 2.0)).unwrap();
    g.set_marker(v2(1.0, 1.0)).unwrap();
    assert!(g.round().revealed_source().is_none());
    g.commit().unwrap();
    assert!(g.round().revealed_source().is_some());
}

#[test]
fn a_commit_needs_a_marker_and_freezes_the_round() {
    let mut g = game();
    assert_eq!(g.commit().err(), Some(CommitError::NoMarker));
    g.set_marker(v2(0.0, 0.0)).unwrap();
    let v = g.commit().unwrap();
    assert!(g.finished());
    assert_eq!(g.round().verdict, Some(v));
    assert_eq!(g.commit().err(), Some(CommitError::Finished));
    assert_eq!(g.set_marker(v2(1.0, 1.0)).err(), Some(CommitError::Finished));
    assert_eq!(g.place_scout(v2(2.0, 2.0)).err(), Some(PlaceError::Finished));
    assert_eq!(g.round().marker, Some(v2(0.0, 0.0)), "the committed marker cannot move");
}

#[test]
fn the_marker_can_move_until_commit_and_is_clamped_to_the_zone() {
    let mut g = game();
    g.set_marker(v2(1.0, 1.0)).unwrap();
    g.set_marker(v2(-2.0, 0.5)).unwrap();
    assert_eq!(g.round().marker, Some(v2(-2.0, 0.5)));
    let p = g.set_marker(v2(50.0, -50.0)).unwrap();
    assert!(g.spec.zone.contains(p));
}

#[test]
fn the_verdict_is_decided_by_the_commit_tolerance() {
    let spec = SurveySpec::from_level(&level());
    let src = spec.source_for_attempt(0);
    let tol = spec.tol;
    for (dx, ok) in [(0.0, true), (0.9 * tol, true), (tol, true), (1.1 * tol, false), (3.0, false)] {
        let mut g = game();
        g.set_marker(v2(src.x + dx, src.y)).unwrap();
        let v = g.commit().unwrap();
        assert_eq!(v.ok, ok, "offset {dx}");
        assert!((v.distance - dx).abs() < 1e-9);
        assert_eq!(g.won, ok);
        assert_eq!(g.round().revealed_source(), Some(src));
    }
}

#[test]
fn a_retry_needs_a_commit_and_hides_the_source_somewhere_else() {
    let mut g = game();
    g.place_scout(v2(0.0, 2.0)).unwrap();
    assert_eq!(g.retry().err(), Some(RetryError::NotFinished), "abandoning a round would refund its scouts");
    assert_eq!(g.scouts_left(), g.spec.budget - 1);
    g.set_marker(v2(0.0, 0.0)).unwrap();
    g.commit().unwrap();
    g.retry().unwrap();
    assert_eq!(g.attempts(), 2);
    assert_eq!(g.history.len(), 1);
    assert_eq!(g.round().scouts.len(), 1, "a new round starts again from the free scout only");
    assert_eq!(g.scouts_left(), g.spec.budget);
    assert!(g.round().marker.is_none() && !g.finished());
    assert_ne!(g.spec.source_for_attempt(0), g.spec.source_for_attempt(1));
}

#[test]
fn every_authored_position_is_used_once_per_pass_and_the_first_pass_is_in_authored_order() {
    let spec = SurveySpec::from_level(&level());
    let n = spec.pool.len();
    let key = |a: &V2, b: &V2| a.x.partial_cmp(&b.x).unwrap().then(a.y.partial_cmp(&b.y).unwrap());
    let mut want = spec.pool.clone();
    want.sort_by(key);
    for pass in 0..4 {
        let mut seen: Vec<V2> = (0..n).map(|a| spec.source_for_attempt(pass * n + a)).collect();
        if pass == 0 {
            assert_eq!(seen, spec.pool, "the first pass follows the authored (easy to hard) order");
        }
        seen.sort_by(key);
        assert_eq!(seen, want, "pass {pass} visits every position exactly once");
    }
    for a in 1..6 * n {
        assert_ne!(spec.source_for_attempt(a), spec.source_for_attempt(a - 1), "attempt {a} repeats the previous position");
    }
}

#[test]
fn the_authored_order_goes_from_easy_to_hard() {
    // Hard = how many explanations the free trail leaves open (more = harder), then how many paid scouts the
    // adaptive player needs. The first attempt must not be among the hardest positions.
    let l = level();
    let m = vs::Model::new(&l);
    let ind = m.indist(vs::EPS_COARSE);
    let rank: Vec<(usize, usize)> = m
        .spec
        .pool
        .iter()
        .map(|&s| {
            let d = m.truth_devs(s);
            (m.after_free(&d, vs::EPS_COARSE).len(), m.adaptive_needs(&ind, &d, vs::EPS_COARSE, s).unwrap_or(99))
        })
        .collect();
    for w in rank.windows(2) {
        assert!(w[0] <= w[1], "pool is not ordered easy to hard: {rank:?}");
    }
    assert!(rank[0].0 < rank[rank.len() - 1].0, "the first position must be easier than the last: {rank:?}");
}

#[test]
fn the_charted_flight_does_not_depend_on_the_hidden_source() {
    let g = game();
    let start = v2(-2.0, 1.5);
    let ghost = g.spec.fly_charted(start);
    for &src in &g.spec.pool {
        assert_eq!(g.spec.fly_charted(start).end(), ghost.end());
        // the real trail is the charted one plus the source's push, so it can only differ from the ghost
        let real = g.spec.fly(src, start);
        assert_eq!(real.points[0], ghost.points[0]);
    }
    // a source far from the start leaves the real trail on the ghost
    let far = g.spec.fly(v2(5.0, -3.0), v2(-5.0, 3.0));
    assert!(far.end().dist(g.spec.fly_charted(v2(-5.0, 3.0)).end()) < 0.05);
}

#[test]
fn a_win_is_never_lost_by_playing_on() {
    let spec = SurveySpec::from_level(&level());
    let mut g = game();
    g.set_marker(spec.source_for_attempt(0)).unwrap();
    assert!(g.commit().unwrap().ok);
    g.retry().unwrap();
    g.set_marker(v2(5.0, 3.0)).unwrap();
    assert!(!g.commit().unwrap().ok);
    assert!(g.won, "a later miss must not undo completion");
}

#[test]
fn different_hidden_positions_give_different_free_trails() {
    // otherwise a retry would not need new thinking: the free scout is the player's first evidence
    let spec = SurveySpec::from_level(&level());
    let ends: Vec<V2> = (0..spec.pool.len()).map(|a| spec.fly(spec.source_for_attempt(a), spec.free).end()).collect();
    let distinct = ends.iter().enumerate().filter(|(i, e)| ends[..*i].iter().all(|o| o.dist(**e) > 0.05)).count();
    assert!(distinct >= 4, "only {distinct} distinct free trails among {} positions", ends.len());
}

// ---------------- the level and its validation ----------------
#[test]
fn survey_is_registered_as_a_chapter_with_three_levels() {
    assert!(Chapter::ALL.contains(&Chapter::Survey));
    assert_eq!(Chapter::Survey.name(), "Survey");
    let ls = levels::survey_levels();
    assert_eq!(ls.len(), 3);
    for id in ["sur-1", "sur-2", "sur-3"] {
        assert!(levels::all().iter().any(|l| l.id == id && l.chapter == Chapter::Survey));
    }
    for l in &ls {
        assert!(matches!(l.rules, Rules::Survey { .. }));
        assert_eq!(l.hints.len(), 3);
    }
}

#[test]
fn the_level_parameters_are_consistent() {
    let l = level();
    let spec = SurveySpec::from_level(&l);
    assert_eq!(spec.budget, 2);
    assert!((spec.tol - 0.75).abs() < 1e-12);
    assert!((l.max_time - 5.0).abs() < 1e-12, "the trail length is the level's time limit");
    assert!(l.regions.is_empty(), "scouts must run their full trail, nothing may end them early");
    assert!(spec.zone.contains(spec.free));
    assert!(spec.pool.len() >= 4);
    for (i, p) in spec.pool.iter().enumerate() {
        assert!(vs::HYP_RECT.contains(*p) && spec.zone.contains(*p), "{p:?}");
        // off the validator's 0.5 candidate grid, so it cannot flatter itself
        assert!(((p.x / vs::HYP_STEP).fract().abs() > 0.1) || ((p.y / vs::HYP_STEP).fract().abs() > 0.1), "{p:?} sits on the candidate grid");
        for q in &spec.pool[..i] {
            assert!(p.dist(*q) >= 1.5, "{p:?} and {q:?} are too close to feel like different puzzles");
        }
    }
}

#[test]
fn every_hidden_position_is_solvable_within_the_budget_at_both_precisions() {
    let l = level();
    let m = vs::Model::new(&l);
    for &src in &m.spec.pool {
        let d = m.truth_devs(src);
        assert!(m.oracle_solves(src, &d, vs::EPS_FINE), "{src:?} is unsolvable at the fine precision");
        assert!(m.oracle_solves(src, &d, vs::EPS_COARSE), "{src:?} is unsolvable at the coarse precision");
    }
    assert!(validate::solvable(&l));
    assert!(validate::oracle_win(&l));
}

#[test]
fn the_free_scout_never_settles_the_puzzle_by_itself() {
    let l = level();
    let m = vs::Model::new(&l);
    for &src in &m.spec.pool {
        let d = m.truth_devs(src);
        let c0 = m.after_free(&d, vs::EPS_COARSE);
        assert!(!m.ok(&c0, src), "{src:?}: the free trail alone already gives the answer");
        assert!(m.diameter(&c0) > 2.0 * m.spec.tol, "{src:?}: the free trail leaves {} explanations, all within {:.1}", c0.len(), m.diameter(&c0));
        // the ambiguity is real: at least two surviving explanations are farther apart than the commit tolerance
        let far = c0.iter().any(|&a| c0.iter().any(|&b| m.hyp[a].dist(m.hyp[b]) > m.spec.tol));
        assert!(far);
    }
}

#[test]
fn the_budget_is_a_real_constraint_for_a_player_who_does_not_know_the_answer() {
    // NB: an *omniscient* oracle needs one scout for everything (it knows where to look), so "fewest scouts the
    // oracle needs" says nothing about difficulty. The measure is the adaptive player, who does not know.
    let l = level();
    let m = vs::Model::new(&l);
    let ind = m.indist(vs::EPS_COARSE);
    let need: Vec<Option<usize>> = m.spec.pool.iter().map(|&s| m.adaptive_needs(&ind, &m.truth_devs(s), vs::EPS_COARSE, s)).collect();
    assert!(need.iter().all(|n| matches!(n, Some(1) | Some(2))), "{need:?}: every position must be solved, and none by the free scout alone");
    assert!(need.contains(&Some(2)), "{need:?}: no position makes even the adaptive player use its whole budget");
    // and over random continuous sources the second scout buys a clear improvement
    let p = vs::population(&m, &ind, vs::EPS_COARSE, &vs::random_sources(80, 77));
    assert!(p.adaptive - p.adaptive_one >= 0.05, "one scout {:.2}, full budget {:.2}", p.adaptive_one, p.adaptive);
}

#[test]
fn an_adaptive_player_wins_every_position_through_the_real_game_and_agrees_with_the_model() {
    // adaptive_play only ever sees the trails the game returns; the model-side player is told the truth's
    // trails directly. They must reach the same answer, or the validator is not modelling the game.
    let l = level();
    let m = vs::Model::new(&l);
    let ind = m.indist(vs::EPS_COARSE);
    for attempt in 0..m.spec.pool.len() {
        let src = m.spec.source_for_attempt(attempt);
        let by_game = vs::adaptive_play(&l, &m, &ind, attempt, vs::EPS_COARSE);
        let by_model = m.ok(&m.adaptive(&ind, &m.truth_devs(src), vs::EPS_COARSE), src);
        assert_eq!(by_game, by_model, "attempt {attempt} ({src:?})");
        assert!(by_game, "the adaptive player lost attempt {attempt} ({src:?})");
    }
}

#[test]
fn thoughtful_placement_beats_uninformed_placement_across_many_sources() {
    // Population test over *continuous* random sources (not the authored pool): the property that makes the
    // mechanic about what to observe. Measured at the coarse (human) precision.
    let l = level();
    let m = vs::Model::new(&l);
    let ind = m.indist(vs::EPS_COARSE);
    let p = vs::population(&m, &ind, vs::EPS_COARSE, &vs::random_sources(80, 77));
    eprintln!("{p:?}");
    assert!(p.adaptive >= 0.75, "adaptive {:.2}", p.adaptive);
    assert!(p.adaptive - p.grid >= 0.08, "adaptive {:.2} vs fixed pattern {:.2}", p.adaptive, p.grid);
    assert!(p.adaptive - p.random >= 0.08, "adaptive {:.2} vs random {:.2}", p.adaptive, p.random);
    assert!(p.free_only < 0.6, "the free scout alone solves {:.0}% of random sources", 100.0 * p.free_only);
    assert!(p.unsolvable <= 0.05, "{:.0}% of random sources cannot be solved by any scouts", 100.0 * p.unsolvable);
}

#[test]
fn the_validator_can_say_no() {
    // no hidden strength: every scout draws the same trail, so nothing can be learned
    let blind = with(|k, _| *k = 0.0);
    assert!(!validate::solvable(&blind), "a source with no effect cannot be located");
    // and a pool position the free scout already pins down must be flagged as trivial
    let l = with(|_, pool| *pool = vec![v2(-3.0, 0.5)]);
    let m = vs::Model::new(&l);
    let src = m.spec.pool[0];
    let c0 = m.after_free(&m.truth_devs(src), vs::EPS_COARSE);
    assert!(m.ok(&c0, src) || m.diameter(&c0) <= 2.0 * m.spec.tol, "expected a source on the free scout's path to be nearly settled by it ({} left)", c0.len());
}

// ---------------- checking an estimate against the evidence ----------------
#[test]
fn a_test_needs_a_marker_and_never_reads_the_hidden_source() {
    // Two rounds with different hidden positions, the same scouts and the same marker: the tested trails must be
    // identical, because they may only depend on the marker and the scouts' starts.
    let mut a = game();
    assert_eq!(a.test_estimate(), Err(TestError::NoMarker));
    assert_eq!(a.tests_left(), TESTS_PER_ROUND, "a refused test costs nothing");
    let mut b = game();
    b.set_marker(v2(0.0, 0.0)).unwrap();
    b.commit().unwrap();
    b.retry().unwrap();
    assert_ne!(a.spec.source_for_attempt(0), b.spec.source_for_attempt(1));
    for g in [&mut a, &mut b] {
        g.place_scout(v2(-3.0, 1.0)).unwrap();
        g.place_scout(v2(-1.0, -1.5)).unwrap();
        g.set_marker(v2(1.5, 0.5)).unwrap();
        assert_eq!(g.test_estimate(), Ok(true));
    }
    let (ta, tb) = (a.round().test.as_ref().unwrap(), b.round().test.as_ref().unwrap());
    assert_eq!(ta.at, tb.at);
    assert_eq!(ta.trails.len(), 3, "the free scout and both paid scouts");
    for (x, y) in ta.trails.iter().zip(&tb.trails) {
        assert_eq!(x.points, y.points);
    }
}

#[test]
fn a_test_on_the_true_source_reproduces_the_real_trails_and_a_wrong_one_does_not() {
    let mut g = game();
    let src = g.spec.source_for_attempt(0);
    // a scout released close to the source: far ones barely notice where a hypothetical source is
    g.place_scout(v2(src.x - 1.2, src.y + 0.4)).unwrap();
    g.set_marker(src).unwrap();
    g.test_estimate().unwrap();
    let t = g.round().test.as_ref().unwrap();
    for (sc, tr) in g.round().scouts.iter().zip(&t.trails) {
        assert_eq!(sc.traj.points, tr.points);
    }
    g.set_marker(v2(src.x + 1.0, src.y)).unwrap();
    g.test_estimate().unwrap();
    let t = g.round().test.as_ref().unwrap();
    assert!(g.round().scouts.iter().zip(&t.trails).any(|(sc, tr)| sc.traj.end().dist(tr.end()) > 0.1), "a wrong estimate must look wrong");
}

#[test]
fn tests_are_a_budget_that_the_same_estimate_does_not_spend_and_a_retry_refills() {
    let mut g = game();
    g.set_marker(v2(1.0, 1.0)).unwrap();
    assert_eq!(g.test_estimate(), Ok(true));
    assert_eq!(g.test_estimate(), Ok(false), "the same marker with the same scouts shows the same thing: free");
    assert_eq!(g.tests_left(), TESTS_PER_ROUND - 1);
    g.place_scout(v2(-3.0, 1.0)).unwrap();
    assert_eq!(g.test_estimate(), Ok(true), "a new scout makes the old result stale: testing again costs");
    for k in 0..TESTS_PER_ROUND - 2 {
        g.set_marker(v2(1.0 + 0.5 * (k + 1) as f64, 1.0)).unwrap();
        assert_eq!(g.test_estimate(), Ok(true));
    }
    assert_eq!(g.tests_left(), 0);
    g.set_marker(v2(-2.0, -2.0)).unwrap();
    assert_eq!(g.test_estimate(), Err(TestError::NoTestsLeft));
    g.commit().unwrap();
    assert_eq!(g.test_estimate(), Err(TestError::Finished));
    g.retry().unwrap();
    assert_eq!(g.tests_left(), TESTS_PER_ROUND, "a new attempt starts with a full budget");
    assert!(g.round().test.is_none());
}

#[test]
fn rays_run_back_along_the_push_through_the_source() {
    let g = game();
    assert!(g.spec.rays, "the teaching level draws rays");
    for &src in &g.spec.pool {
        let t = g.spec.fly(src, v2(src.x - 1.2, src.y + 0.5));
        let rays = g.spec.push_rays(&t);
        assert!(!rays.is_empty(), "{src:?}: a scout near the source must have rays");
        for (p, dir) in rays {
            // distance from the source to the ray's line
            let to = src - p;
            let off = (to.x * dir.y - to.y * dir.x).abs();
            assert!(off < 0.02, "{src:?}: the ray from {p:?} misses the source by {off}");
            assert!(to.dot(dir) > 0.0, "the ray points toward the source, not away from it");
        }
    }
    let mut off = game();
    off.spec.rays = false;
    let src = off.spec.pool[0];
    assert!(off.spec.push_rays(&off.spec.fly(src, v2(src.x - 1.2, src.y + 0.5))).is_empty(), "later levels can switch the aid off");
}

#[test]
fn the_commit_tolerance_is_the_calibrated_one() {
    // 0.5 was too tight for people (six misses at 0.52..1.05), 1.0 lets the free scout settle a position.
    let spec = SurveySpec::from_level(&level());
    assert_eq!(spec.tol, 0.75);
}

// ---------------- reading the evidence ----------------
/// Least-squares point nearest to weighted lines (anchor, unit direction, weight).
fn nearest_to_lines(lines: &[(V2, V2, f64)]) -> Option<V2> {
    let (mut a, mut b, mut c, mut e, mut f) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(p, d, w) in lines {
        let (nxx, nxy, nyy) = (1.0 - d.x * d.x, -d.x * d.y, 1.0 - d.y * d.y);
        a += w * nxx;
        b += w * nxy;
        c += w * nyy;
        e += w * (nxx * p.x + nxy * p.y);
        f += w * (nxy * p.x + nyy * p.y);
    }
    let det = a * c - b * b;
    (det.abs() > 1e-9).then(|| v2((c * e - b * f) / det, (a * f - b * e) / det))
}

#[test]
fn push_arrows_point_away_from_the_source_and_vanish_without_one() {
    let g = game();
    let src = g.spec.source_for_attempt(0);
    // no source in reach: the push is nil on a charted flight
    let calm = g.spec.fly_charted(v2(-4.0, 3.0));
    assert!(g.spec.push_samples_at(&calm, PUSH_EVERY, 0.01).is_empty(), "a source-free trail feels no extra push");
    // a scout passing near the source feels a push directed away from it
    let t = g.spec.fly(src, v2(src.x - 1.2, src.y + 0.5));
    let samples = g.spec.push_samples(&t);
    assert!(!samples.is_empty());
    for (p, push) in &samples {
        let away = (*p - src) * (1.0 / p.dist(src));
        assert!(push.dot(away) > 0.99 * push.len(), "push at {p:?} is not directed away from the source");
    }
}

#[test]
fn a_scout_near_the_source_locates_it_and_the_free_scout_alone_never_does() {
    // Scouting must matter. A scout released within the sensing range of the source (here 1.2-1.3 away, in two
    // directions) gives arrows that meet at the source; the free scout, whose line runs along y = 0, shows too
    // little on its own for any authored position (0.2 threshold; see PUSH_MIN).
    let g = game();
    for &src in &g.spec.pool {
        let arrows = |starts: &[V2]| -> Vec<(V2, V2, f64)> {
            let mut lines = vec![];
            for &start in starts {
                for (p, push) in g.spec.push_samples(&g.spec.fly(src, start)) {
                    lines.push((p, push * (-1.0 / push.len()), push.len() * push.len()));
                }
            }
            lines
        };
        let free_only = arrows(&[g.spec.free]);
        let located = nearest_to_lines(&free_only).map_or(false, |e| e.dist(src) <= g.spec.tol);
        assert!(free_only.len() < 3 || !located, "{src:?}: the free scout alone locates the source");

        let near = [v2(src.x - 1.2, src.y + 0.5), v2(src.x + 0.4, src.y - 1.1)];
        let near: Vec<V2> = near.into_iter().filter(|p| g.spec.zone.contains(*p)).collect();
        let est = nearest_to_lines(&arrows(&near)).expect("arrows from scouts near the source");
        assert!(est.dist(src) < 0.1, "{src:?}: arrows point to {est:?}");
    }
}

// ---------------- level 2: One Look Is a Line ----------------
fn spec2() -> SurveySpec {
    SurveySpec::from_level(&levels::survey_levels().remove(1))
}

type Arrows = Vec<(V2, V2)>;

fn arrows_of(sp: &SurveySpec, src: V2, start: V2) -> Arrows {
    sp.push_samples(&sp.fly(src, start))
}

/// How often the lines back along the given scouts' push arrows, read with a per-trail bias (8 deg) and per-arrow
/// jitter (6 deg), land within `tol` of `src`. Needs at least 3 arrows. Deterministic (seeded).
fn read_from(tol: f64, src: V2, scouts: &[&Arrows], seed: u64, trials: usize) -> f64 {
    read_rough(tol, src, scouts, (8.0, 6.0), seed, trials)
}

/// `read_from` with an explicit reading error: (bias per trail, jitter per arrow) in degrees.
fn read_rough(tol: f64, src: V2, scouts: &[&Arrows], (bias_deg, jitter_deg): (f64, f64), seed: u64, trials: usize) -> f64 {
    let mut rng = vfield::modes::Lcg(seed);
    let mut gauss = || {
        let u1 = (rng.below(1_000_000) as f64 + 1.0) / 1_000_001.0;
        let u2 = rng.below(1_000_000) as f64 / 1e6;
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    };
    let mut hit = 0;
    for _ in 0..trials {
        let (mut lines, mut n) = (vec![], 0);
        for arrows in scouts {
            let bias = gauss() * bias_deg.to_radians();
            for &(p, push) in arrows.iter() {
                n += 1;
                let a = (-push.y).atan2(-push.x) + bias + gauss() * jitter_deg.to_radians();
                lines.push((p, v2(a.cos(), a.sin()), push.len() * push.len()));
            }
        }
        if n >= 3 && nearest_to_lines(&lines).map_or(false, |e| e.dist(src) <= tol) {
            hit += 1;
        }
    }
    hit as f64 / trials as f64
}

fn read_success(sp: &SurveySpec, src: V2, starts: &[V2], seed: u64, trials: usize) -> f64 {
    let cached: Vec<Arrows> = starts.iter().map(|&s| arrows_of(sp, src, s)).collect();
    read_from(sp.tol, src, &cached.iter().collect::<Vec<_>>(), seed, trials)
}

#[test]
fn level_two_is_a_narrower_weaker_source_with_the_band_on_and_the_rays_off() {
    let sp = spec2();
    assert_eq!((sp.k, sp.sigma), (1.65, 0.8));
    assert!((sp.tol - 0.75 * sp.sigma).abs() < 1e-12, "tolerance is 0.75 sigma");
    assert!(sp.k * sp.sigma * (-0.5f64).exp() < 0.85, "peak push stays well under the stream speed (1.0): nothing stalls");
    assert!(sp.band && !sp.rays, "the band is the new aid; the rays are gone, so the arrows must be extended by eye");
    assert!((sp.sense_radius() - 1.87).abs() < 0.02, "sensing radius {}", sp.sense_radius());
    assert_eq!(sp.pool.len(), 6);
    assert!(!game().spec.band, "level 1 has no band");
}

#[test]
fn level_two_attempts_are_graze_head_on_graze_head_on_quiet_quiet() {
    let sp = spec2();
    let arrows = |i: usize| sp.push_samples(&sp.fly(sp.pool[i], sp.free)).len();
    for i in [0, 2] {
        assert!((1..=3).contains(&arrows(i)), "graze {i}: {} free arrows", arrows(i));
    }
    for i in [1, 3] {
        let src = sp.pool[i];
        assert_eq!(arrows(i), 1, "head-on {i}: one weak arrow, along the line");
        assert!(src.y.abs() < 0.1 && (1.3..=1.7).contains(&src.x), "head-on {i} sits on the free line just beyond its end: {src:?}");
    }
    for i in [4, 5] {
        let closest = sp.fly(sp.pool[i], sp.free).points.iter().map(|p| p.dist(sp.pool[i])).fold(f64::MAX, f64::min);
        assert_eq!(arrows(i), 0, "quiet {i}");
        assert!(closest > sp.sense_radius() + 1.0, "quiet {i} is well out of reach ({closest})");
    }
}

#[test]
fn level_two_free_scout_never_settles_and_same_line_scouts_add_nothing_to_head_on_and_quiet() {
    let sp = spec2();
    for (i, &src) in sp.pool.iter().enumerate() {
        assert!(read_success(&sp, src, &[sp.free], 10 + i as u64, 100) <= 0.1, "{src:?}: the free scout alone settles it");
    }
    // Two more scouts on the free scout's own line, released where it was: the same look again. Not claimed for the
    // grazes: a graze sits at the edge of the free scout's reach, so a scout released even a little further along the
    // line sees a bit farther and can help (62% at (0.5, -1.4) from a start 1 unit downstream). That is a longer
    // look along the same line, not a second viewpoint.
    for i in [1, 3, 4, 5] {
        let src = sp.pool[i];
        let s = read_success(&sp, src, &[sp.free, v2(-4.8, 0.0), v2(-4.6, 0.0)], 20 + i as u64, 100);
        assert!(s <= 0.1, "{src:?}: same-line scouts decide it ({s})");
    }
}

#[test]
fn level_two_a_scout_released_off_the_line_decides_graze_and_head_on_and_the_band_says_how_far_to_go_for_quiet() {
    let sp = spec2();
    for (i, &src) in sp.pool.iter().enumerate().take(4) {
        let s = read_success(&sp, src, &[sp.free, v2(-2.0, 1.0), v2(-2.0, -1.0)], 30 + i as u64, 100);
        assert!(s >= 0.8, "{src:?}: scouts 1 above and below solve it only {s}");
    }
    for (i, &src) in sp.pool.iter().enumerate().skip(4) {
        let near = read_success(&sp, src, &[sp.free, v2(-2.0, 1.0), v2(-2.0, -1.0)], 40 + i as u64, 100);
        let far = read_success(&sp, src, &[sp.free, v2(-2.0, 2.0), v2(-2.0, -2.0)], 50 + i as u64, 100);
        assert!(near <= 0.1, "{src:?}: a quiet source cannot be felt from 1 off the line ({near})");
        assert!(far >= 0.7, "{src:?}: but it can from 2 off ({far})");
    }
}

#[test]
fn level_two_blind_placement_is_not_enough_but_a_good_pair_always_exists() {
    let sp = spec2();
    let mut pts = vec![];
    for x in -5..=5 {
        for y in -3..=3 {
            pts.push(v2(x as f64, y as f64));
        }
    }
    for (k, &src) in sp.pool.iter().enumerate() {
        let free = arrows_of(&sp, src, sp.free);
        let cached: Vec<Arrows> = pts.iter().map(|&p| arrows_of(&sp, src, p)).collect();
        let (mut sum, mut best, mut n) = (0.0, 0.0f64, 0);
        for i in 0..pts.len() {
            for j in i + 1..pts.len() {
                let r = read_from(sp.tol, src, &[&free, &cached[i], &cached[j]], (k * 100_000 + i * 97 + j) as u64, 6);
                sum += r;
                best = best.max(r);
                n += 1;
            }
        }
        assert!(sum / (n as f64) < 0.55, "{src:?}: blind pairs average {}", sum / n as f64);
        assert!(best >= 0.9, "{src:?}: no pair solves it ({best})");
    }
}

#[test]
fn the_cleared_band_never_covers_the_hidden_source() {
    // The band is evidence that "it is not here"; if it ever covered the source it would be a lie. Every authored
    // position, every lattice start, both levels.
    for sp in [game().spec.clone(), spec2()] {
        let r = sp.sense_radius();
        assert!(r > 1.5);
        for &src in &sp.pool {
            let mut starts = vec![sp.free];
            for x in -5..=5 {
                for y in -3..=3 {
                    starts.push(v2(x as f64, y as f64));
                }
            }
            for s in starts {
                let (centres, radius) = sp.cleared(&sp.fly(src, s));
                assert!(radius > 0.0 && radius < r);
                for c in centres {
                    assert!(c.dist(src) > radius, "band from start {s:?} covers the source {src:?} (centre {c:?}, radius {radius})");
                }
            }
        }
    }
}

#[test]
fn the_cleared_band_is_there_when_a_scout_sees_nothing_and_leaves_a_gap_where_it_felt_something() {
    let sp = spec2();
    let quiet = sp.fly(sp.pool[5], sp.free);
    assert!(sp.push_samples(&quiet).is_empty());
    assert!(sp.cleared(&quiet).0.len() >= 20, "a silent trail clears the places it passed");
    // a graze: the spots with an arrow clear nothing
    let graze = sp.fly(sp.pool[0], sp.free);
    let arrows = sp.push_samples_at(&graze, vfield::modes::CLEARED_EVERY, vfield::modes::PUSH_MIN);
    assert!(!arrows.is_empty());
    let (centres, _) = sp.cleared(&graze);
    assert!(arrows.iter().all(|(p, _)| !centres.contains(p)));
}

#[test]
fn level_two_stays_a_decision_when_read_by_eye_without_rays() {
    // With no rays the player extends the arrows by eye: model that as 12 degrees of bias per trail and 10 of jitter
    // per arrow. A scout released well still decides the grazes and head-ons; blind placement still does not.
    let sp = spec2();
    let by_eye = (12.0, 10.0);
    for (i, &src) in sp.pool.iter().enumerate().take(4) {
        let a: Vec<Arrows> = [sp.free, v2(-2.0, 1.0), v2(-2.0, -1.0)].iter().map(|&s| arrows_of(&sp, src, s)).collect();
        let good = read_rough(sp.tol, src, &[&a[0], &a[1], &a[2]], by_eye, 60 + i as u64, 150);
        assert!(good >= 0.85, "{src:?}: well-placed scouts solve it only {good} when read by eye");
    }
    for (i, &src) in sp.pool.iter().enumerate().skip(4) {
        let a: Vec<Arrows> = [sp.free, v2(-2.0, 2.0), v2(-2.0, -2.0)].iter().map(|&s| arrows_of(&sp, src, s)).collect();
        let good = read_rough(sp.tol, src, &[&a[0], &a[1], &a[2]], by_eye, 70 + i as u64, 150);
        assert!(good >= 0.4, "{src:?}: even well-placed scouts solve the quiet ones only {good} when read by eye");
    }
}

// ---------------- level 3: Which Way It Pushes ----------------
fn spec3() -> SurveySpec {
    SurveySpec::from_level(&levels::survey_levels().remove(2))
}

#[test]
fn gauss_vortex_pushes_sideways_with_the_same_profile_and_the_kinds_map_to_terms() {
    let (k, sigma, c) = (1.3, 1.0, v2(0.0, 0.0));
    let ccw = Field::new().fixed(SourceKind::SwirlCcw.term(c, k, sigma));
    let cw = Field::new().fixed(SourceKind::SwirlCw.term(c, k, sigma));
    let rep = Field::new().fixed(SourceKind::Repeller.term(c, k, sigma));
    let att = Field::new().fixed(SourceKind::Attractor.term(c, k, sigma));
    for p in [v2(1.0, 0.0), v2(0.0, 1.5), v2(-1.2, -0.7)] {
        let (a, b, r, t) = (ccw.eval(p, 0.0), cw.eval(p, 0.0), rep.eval(p, 0.0), att.eval(p, 0.0));
        assert!(a.dot(p).abs() < 1e-12, "a swirl is at right angles to the line from its centre");
        assert!((a.len() - r.len()).abs() < 1e-12, "same speed profile as the repeller");
        assert!((a + b).len() < 1e-12 && (r + t).len() < 1e-12, "clockwise is the opposite of counter-clockwise, attract of repel");
        assert!(a.dot(p.perp()) > 0.0, "k > 0 is counter-clockwise");
        assert!(r.dot(p) > 0.0 && t.dot(p) < 0.0);
    }
}

#[test]
fn level_three_asks_for_a_kind_and_has_the_designed_attempts() {
    let sp = spec3();
    assert!(sp.typed() && sp.rays && !sp.band);
    assert_eq!((sp.k, sp.sigma, sp.tol), (1.3, 1.0, 0.75));
    assert!(sp.k * sp.sigma * (-0.5f64).exp() < 0.8, "peak push stays below the stream speed");
    use SourceKind::*;
    assert_eq!(sp.kinds, vec![Repeller, Attractor, SwirlCcw, Repeller, Attractor, SwirlCw, SwirlCcw]);
    // every kind has at least two positions, so a miss can be followed by another of the same kind
    for f in 0..3 {
        assert!(sp.kinds.iter().filter(|k| k.family() == f).count() >= 2, "family {f}");
    }
    assert!(!game().spec.typed() && !spec2().typed(), "the earlier levels are repeller-only");
    // a kind always travels with its position, in every pass
    for a in 0..30 {
        let (p, k) = (sp.source_for_attempt(a), sp.kind_for_attempt(a));
        assert!(sp.pool.iter().zip(&sp.kinds).any(|(q, kk)| *q == p && *kk == k), "attempt {a} separated a kind from its position");
    }
}

/// Lines through each arrow, weighted by strength squared: (best point, weighted mean squared offset).
fn fit_lines(lines: &[(V2, V2, f64)]) -> Option<(V2, f64)> {
    let est = nearest_to_lines(lines)?;
    let w: f64 = lines.iter().map(|l| l.2).sum();
    let r: f64 = lines.iter().map(|&(p, d, wt)| { let q = est - p; let off = q.x * d.y - q.y * d.x; wt * off * off }).sum();
    Some((est, r / w))
}

/// Decide kind and place from arrows the way a careful reader does: if lines along the arrows meet, it is a
/// repeller or attractor (which, by whether the arrows point away from the crossing or toward it); if lines at right
/// angles meet, a swirl (which way round, by the sense of the arrows about the crossing).
fn read_kind(arrows: &[(V2, V2)]) -> Option<(SourceKind, V2)> {
    let axial: Vec<_> = arrows.iter().map(|&(p, u)| (p, u * (1.0 / u.len()), u.len().powi(2))).collect();
    let normal: Vec<_> = arrows.iter().map(|&(p, u)| (p, u.perp() * (1.0 / u.len()), u.len().powi(2))).collect();
    let (ea, ra) = fit_lines(&axial)?;
    let (en, rn) = fit_lines(&normal)?;
    if ra <= rn {
        let s: f64 = arrows.iter().map(|&(p, u)| u.dot(p - ea) / u.len()).sum();
        Some((if s > 0.0 { SourceKind::Repeller } else { SourceKind::Attractor }, ea))
    } else {
        let s: f64 = arrows.iter().map(|&(p, u)| u.dot((p - en).perp()) / u.len()).sum();
        Some((if s > 0.0 { SourceKind::SwirlCcw } else { SourceKind::SwirlCw }, en))
    }
}

#[test]
fn level_three_is_complete_only_when_a_repeller_an_attractor_and_a_swirl_have_each_been_found() {
    let mut g = SurveyGame::new(levels::survey_levels().remove(2));
    assert_eq!(g.families_needed(), vec![0, 1, 2]);
    let mut kinds_met = vec![];
    for round in 0..3 {
        assert!(!g.complete() && !g.won, "round {round}: done too early");
        let (src, kind) = g.round().truth();
        kinds_met.push(kind.family());
        g.set_marker(src).unwrap();
        g.set_marker_kind(kind).unwrap();
        assert!(g.commit().unwrap().ok);
        assert!(g.found[kind.family()]);
        if round < 2 {
            assert!(!g.won, "one kind found is not the level");
            g.retry().unwrap();
        }
    }
    assert_eq!(kinds_met, vec![0, 1, 2], "the kinds come in the order repeller, attractor, swirl");
    assert!(g.complete() && g.won, "all three found: the level is complete, in three attempts");
    // after that it keeps serving attempts (the player chose to keep playing)
    g.retry().unwrap();
    assert!(g.round().index < g.spec.pool.len());
}

#[test]
fn level_three_a_miss_is_followed_by_another_of_the_same_kind_and_cycles_through_its_positions() {
    let mut g = SurveyGame::new(levels::survey_levels().remove(2));
    let mut seen = vec![];
    // miss four times in a row: the level keeps asking for the first kind not found (a repeller)
    for _ in 0..4 {
        let (src, kind) = g.round().truth();
        seen.push((g.round().index, kind));
        g.set_marker(v2(src.x + 3.0, src.y)).unwrap();
        g.set_marker_kind(kind).unwrap();
        assert!(!g.commit().unwrap().ok);
        g.retry().unwrap();
    }
    assert!(seen.iter().all(|(_, k)| *k == SourceKind::Repeller), "{seen:?}");
    assert_ne!(seen[0].0, seen[1].0, "a miss gives a different position of the same kind");
    assert_eq!(seen[0].0, seen[2].0, "with two positions, they alternate");
    // then find it: the next kind needed is the attractor
    let (src, kind) = g.round().truth();
    g.set_marker(src).unwrap();
    g.set_marker_kind(kind).unwrap();
    g.commit().unwrap();
    g.retry().unwrap();
    assert_eq!(g.round().truth().1, SourceKind::Attractor);
    assert!(!g.won);
}

#[test]
fn level_three_lines_along_the_arrows_meet_for_repellers_and_attractors_and_right_angles_meet_for_swirls() {
    let sp = spec3();
    for (i, &src) in sp.pool.iter().enumerate() {
        let kind = sp.kinds[i];
        let arrows = sp.push_samples(&sp.fly_kind(src, kind, sp.free));
        assert!(arrows.len() >= 6, "{kind:?} at {src:?}: the free scout shows only {} arrows", arrows.len());
        let axial: Vec<_> = arrows.iter().map(|&(p, u)| (p, u * (1.0 / u.len()), 1.0)).collect();
        let normal: Vec<_> = arrows.iter().map(|&(p, u)| (p, u.perp() * (1.0 / u.len()), 1.0)).collect();
        let ((ea, ra), (en, rn)) = (fit_lines(&axial).unwrap(), fit_lines(&normal).unwrap());
        if kind.is_swirl() {
            assert!(rn < 1e-4 && en.dist(src) < 0.1, "{kind:?}: right-angle lines should meet at the centre ({rn}, {en:?})");
            assert!(ra > 0.02, "{kind:?}: lines along a swirl's arrows must not meet ({ra})");
        } else {
            assert!(ra < 1e-4 && ea.dist(src) < 0.1, "{kind:?}: lines along the arrows should meet at the source ({ra}, {ea:?})");
            assert!(rn > 0.02, "{kind:?}: right-angle lines must not meet ({rn})");
        }
    }
}

#[test]
fn level_three_the_free_scout_alone_gives_the_kind_and_the_place_even_read_with_error() {
    // This is the level where the kinds are learned: it must be readable from the free scout alone (the roadmap's
    // "must hold"). Arrows read with 8 degrees of bias per trail and 6 of jitter per arrow.
    let sp = spec3();
    let mut rng = vfield::modes::Lcg(2024);
    let mut gauss = || {
        let u1 = (rng.below(1_000_000) as f64 + 1.0) / 1_000_001.0;
        let u2 = rng.below(1_000_000) as f64 / 1e6;
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    };
    for (i, &src) in sp.pool.iter().enumerate() {
        let kind = sp.kinds[i];
        let arrows = sp.push_samples(&sp.fly_kind(src, kind, sp.free));
        let mut hit = 0;
        for _ in 0..200 {
            let bias = gauss() * 8f64.to_radians();
            let read: Vec<_> = arrows.iter().map(|&(p, u)| {
                let a = u.y.atan2(u.x) + bias + gauss() * 6f64.to_radians();
                (p, v2(a.cos(), a.sin()) * u.len())
            }).collect();
            if let Some((k, e)) = read_kind(&read) {
                if k == kind && e.dist(src) <= sp.tol {
                    hit += 1;
                }
            }
        }
        assert!(hit >= 170, "{kind:?} at {src:?}: read right only {hit}/200 times from the free scout alone");
    }
}

#[test]
fn level_three_a_marker_of_the_wrong_kind_never_matches_the_dashes_however_close() {
    let mut g = SurveyGame::new(levels::survey_levels().remove(2));
    let mut kinds = std::collections::HashSet::new();
    for _ in 0..7 {
        let (src, kind) = g.round().truth();
        kinds.insert(kind);
        for wrong in SourceKind::ALL.into_iter().filter(|k| *k != kind) {
            g.set_marker(src).unwrap();
            g.set_marker_kind(wrong).unwrap();
            g.test_estimate().unwrap();
            let t = g.round().test.as_ref().unwrap();
            assert_eq!(t.kind, wrong);
            let gap = g.round().scouts[0].traj.points.iter().zip(&t.trails[0].points).map(|(a, b)| a.dist(*b)).fold(0.0, f64::max);
            assert!(gap > 0.15, "a {wrong:?} marker exactly on the {kind:?} at {src:?} matches the free trail (gap {gap})");
        }
        // answer correctly, so the next attempt is of the next kind
        g.set_marker_kind(kind).unwrap();
        g.set_marker(src).unwrap();
        g.commit().unwrap();
        g.retry().unwrap();
    }
    assert!(kinds.len() >= 4, "the run covered every kind of source: {kinds:?}");
}

#[test]
fn level_three_the_right_kind_in_the_right_place_reproduces_every_trail_exactly() {
    let mut g = SurveyGame::new(levels::survey_levels().remove(2));
    for a in 0..7 {
        let (src, kind) = g.round().truth();
        g.place_scout(v2(src.x - 1.0, src.y - 0.8)).unwrap();
        g.set_marker(src).unwrap();
        g.set_marker_kind(kind).unwrap();
        g.test_estimate().unwrap();
        let t = g.round().test.as_ref().unwrap();
        assert_eq!(t.trails.len(), 2);
        for (sc, tr) in g.round().scouts.iter().zip(&t.trails) {
            assert_eq!(sc.traj.points, tr.points, "attempt {a}: {kind:?}");
        }
        g.commit().unwrap();
        g.retry().unwrap();
    }
}

#[test]
fn level_three_a_commit_needs_the_right_kind_as_well_as_the_right_place() {
    let mut g = SurveyGame::new(levels::survey_levels().remove(2));
    let (src, kind) = g.round().truth();
    g.set_marker(src).unwrap();
    g.set_marker_kind(kind.next()).unwrap();
    let v = g.commit().unwrap();
    assert!(v.distance < 1e-9 && !v.kind_ok && !v.ok && !g.found[kind.family()], "right place, wrong kind is a miss");
    assert_eq!(g.round().revealed_kind(), Some(kind));
    g.retry().unwrap();
    let (src, kind) = g.round().truth();
    g.set_marker(v2(src.x + 0.3, src.y)).unwrap();
    g.set_marker_kind(kind).unwrap();
    let v = g.commit().unwrap();
    assert!(v.kind_ok && v.ok && g.found[kind.family()]);
    assert!(!g.won, "one kind is not the whole level");
    // changing the kind is free and does not touch the tests spent
    let mut h = SurveyGame::new(levels::survey_levels().remove(2));
    h.set_marker(v2(0.0, 0.0)).unwrap();
    h.set_marker_kind(SourceKind::SwirlCw).unwrap();
    assert_eq!(h.tests_left(), TESTS_PER_ROUND);
    assert_eq!(h.test_estimate(), Ok(true));
    assert_eq!(h.test_estimate(), Ok(false), "same marker, same kind: free");
    h.set_marker_kind(SourceKind::Attractor).unwrap();
    assert_eq!(h.test_estimate(), Ok(true), "a different kind is a different estimate");
}

