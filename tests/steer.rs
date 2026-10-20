//! Steer: simulation correctness (spec section 8) and level validation.
use vfield::modes::steer::{Action, End, FailReason, Phase, MAX_GHOSTS};
use vfield::modes::{ChooseError, SteerGame, SteerSpec};
use vfield::sim::{simulate, Rect};
use vfield::validate_steer as vs;
use vfield::*;

fn mk(field: Field, regions: Vec<Region>, start: V2, beats: usize, budget: usize, strength: f64, dirs: GustDirs) -> Level {
    Level {
        id: "test",
        chapter: Chapter::Steer,
        title: "t",
        objective: "t",
        hints: vec![],
        field,
        bounds: Rect::new(-6.0, -4.0, 6.0, 4.0),
        regions,
        rules: Rules::Steer { start, beats, gust_budget: budget, gust_strength: strength, dirs, beat_time: 1.0, goal: 0 },
        max_time: 40.0,
        seed: 1,
    }
}
fn far_target() -> Vec<Region> {
    vec![Region::target(5.5, 3.5, 0.1)]
}
fn swirl() -> Field {
    Field::new().fixed(Term::Uniform(v2(0.4, 0.1))).fixed(Term::Vortex { c: v2(0.0, 0.0), k: 1.5, soft: 0.8 })
}
fn steer_level(id: &str) -> Level {
    levels::steer_levels().into_iter().find(|l| l.id == id).unwrap()
}

// ------------------------------------------------------------- simulation
#[test]
fn no_gust_reproduces_the_natural_trajectory() {
    let l = mk(swirl(), vec![], v2(-3.0, 0.5), 5, 0, 1.0, GustDirs::Eight);
    let mut g = SteerGame::new(l.clone());
    for _ in 0..5 {
        g.step(Action::Wait).unwrap();
    }
    let natural = simulate(&l.field, v2(-3.0, 0.5), 0.0, &[], &l.sim_config());
    let path = g.run().path();
    assert_eq!(path.len(), 5 * 50 + 1);
    assert_eq!(path, natural.points[..path.len()].to_vec(), "Wait must be bit-identical to the plain field integration");
}

#[test]
fn a_gust_changes_velocity_not_position() {
    // Uniform (1,0) field, gust of strength 2 to the north for one beat.
    let l = mk(Field::new().fixed(Term::Uniform(v2(1.0, 0.0))), vec![], v2(-3.0, 0.0), 3, 1, 2.0, GustDirs::Four);
    let mut g = SteerGame::new(l);
    g.step(Action::Gust(1)).unwrap(); // N
    let beat = &g.history[0];
    // no teleport: the first sub-step moves by exactly (field + gust) * dt
    let first = beat.points[0] - v2(-3.0, 0.0);
    assert!((first.x - 0.02).abs() < 1e-12 && (first.y - 0.04).abs() < 1e-12, "first sub-step {:?}", first);
    // over the beat the displacement is the integral of the velocity (1,2)*1s
    let d = beat.to() - v2(-3.0, 0.0);
    assert!((d.x - 1.0).abs() < 1e-9 && (d.y - 2.0).abs() < 1e-9, "beat displacement {:?}", d);
    // continuous path: no sub-step jump larger than speed * dt
    let mut prev = v2(-3.0, 0.0);
    for p in &beat.points {
        assert!(p.dist(prev) < (1.0f64 + 4.0).sqrt() * 0.02 + 1e-9);
        prev = *p;
    }
}

#[test]
fn a_gust_disappears_after_its_beat() {
    let l = mk(Field::new().fixed(Term::Uniform(v2(1.0, 0.0))), vec![], v2(-3.0, 0.0), 3, 1, 2.0, GustDirs::Four);
    let mut g = SteerGame::new(l);
    g.step(Action::Gust(1)).unwrap();
    let y_after_gust = g.pos().y;
    g.step(Action::Wait).unwrap();
    g.step(Action::Wait).unwrap();
    assert!((g.pos().y - y_after_gust).abs() < 1e-12, "no residual velocity: y must stay put");
    assert!((g.pos().x - 0.0).abs() < 1e-9, "x advanced at the field speed only");
    assert_eq!(g.history[1].gust, V2::ZERO);
    assert_eq!(g.history[2].gust, V2::ZERO);
}

#[test]
fn net_velocity_is_the_vector_sum_and_gusts_have_equal_magnitude() {
    let l = mk(swirl(), vec![], v2(-3.0, 0.5), 5, 2, 0.7, GustDirs::Eight);
    let g = SteerGame::new(l);
    for d in 0..8 {
        let u = g.gust_vector(Action::Gust(d));
        assert!((u.len() - 0.7).abs() < 1e-12, "direction {d}");
        let sum = g.field_velocity() + u;
        assert_eq!(g.net_velocity(Action::Gust(d)), sum);
    }
    assert_eq!(g.net_velocity(Action::Wait), g.field_velocity());
    // four-direction mode: exactly the cardinal directions
    let l4 = mk(swirl(), vec![], v2(-3.0, 0.5), 5, 2, 1.0, GustDirs::Four);
    let g4 = SteerGame::new(l4);
    let dirs: Vec<V2> = (0..4).map(|d| g4.gust_vector(Action::Gust(d))).collect();
    assert_eq!(dirs, vec![v2(1.0, 0.0), v2(0.0, 1.0), v2(-1.0, 0.0), v2(0.0, -1.0)]);
}

#[test]
fn replay_reproduces_the_same_trajectory() {
    let l = steer_level("ste-4");
    let spec = SteerSpec::from_level(&l);
    let sol = vs::enumerate(&spec).solutions[0].clone();
    let mut g = SteerGame::new(l.clone());
    for a in &sol {
        g.step(*a).unwrap();
    }
    assert_eq!(g.state.end, Some(End::Success));
    let run = g.run();
    let again = spec.replay(&run.actions()).unwrap();
    assert_eq!(run.path(), again.path(), "same field, start, beat time and actions => identical path");
    assert_eq!(run.end, again.end);
    // and from a fresh game via the public API
    let mut g2 = SteerGame::new(l);
    for a in run.actions() {
        g2.step(a).unwrap();
    }
    assert_eq!(g2.run().path(), run.path());
}

#[test]
fn gust_budget_cannot_go_negative() {
    let l = mk(swirl(), far_target(), v2(-3.0, 0.5), 6, 1, 1.0, GustDirs::Eight);
    let mut g = SteerGame::new(l);
    g.step(Action::Gust(0)).unwrap();
    assert_eq!(g.gusts_left(), 0);
    assert_eq!(g.choose(Action::Gust(2)), Err(ChooseError::NoBudget));
    assert_eq!(g.gusts_left(), 0, "failed request must not change the budget");
    assert_eq!(g.beat(), 1, "and must not advance the beat");
    assert!(g.step(Action::Wait).is_ok(), "waiting is always allowed");
    assert_eq!(g.choose(Action::Gust(99)), Err(ChooseError::BadDirection));
}

#[test]
fn doing_nothing_costs_nothing_and_using_all_gusts_is_not_failure() {
    let l = mk(Field::new().fixed(Term::Uniform(v2(1.0, 0.0))), vec![Region::target(2.0, 0.0, 0.3)], v2(-3.0, 0.0), 6, 1, 1.0, GustDirs::Eight);
    let mut g = SteerGame::new(l);
    g.step(Action::Wait).unwrap();
    assert_eq!(g.gusts_left(), 1);
    g.step(Action::Gust(2)).unwrap(); // N: drifts off the line
    g.step(Action::Gust(6)).ok(); // budget exhausted -> Err, state unchanged
    assert_eq!(g.gusts_left(), 0);
    assert_eq!(g.phase, Phase::Awaiting, "an empty budget alone never ends the run");
}

#[test]
fn success_is_detected_mid_beat_and_does_not_tunnel() {
    // 1) reached mid-beat: resolves immediately, run frozen on that beat
    let l = mk(Field::new().fixed(Term::Uniform(v2(1.0, 0.0))), vec![Region::target(-1.5, 0.0, 0.3)], v2(-3.0, 0.0), 5, 0, 1.0, GustDirs::Four);
    let mut g = SteerGame::new(l);
    assert_eq!(g.step(Action::Wait), Ok(None));
    assert_eq!(g.step(Action::Wait), Ok(Some(End::Success)));
    assert_eq!(g.beat(), 2);
    assert_eq!(g.phase, Phase::Completed(End::Success));
    assert!(g.history[1].to().x < -0.5, "probe stops at the target, not at the end of the beat");
    // 2) a fast probe and a tiny target: sampled positions straddle the target, the swept segment hits it
    let l = mk(Field::new().fixed(Term::Uniform(v2(30.0, 0.0))), vec![Region::target(-4.7, 0.0, 0.05)], v2(-5.5, 0.0), 2, 0, 1.0, GustDirs::Four);
    let mut g = SteerGame::new(l);
    assert_eq!(g.step(Action::Wait), Ok(Some(End::Success)), "0.6 units per sub-step, 0.05 tolerance: must not tunnel");
}

#[test]
fn bounds_invalid_state_and_exhausted_beats_end_the_run() {
    // leaving the bounds
    let l = mk(Field::new().fixed(Term::Uniform(v2(4.0, 0.0))), vec![Region::target(0.0, 3.0, 0.2)], v2(-3.0, 0.0), 5, 0, 1.0, GustDirs::Four);
    let mut g = SteerGame::new(l);
    assert_eq!(g.step(Action::Wait), Ok(None)); // x = 1
    assert_eq!(g.step(Action::Wait), Ok(None)); // x = 5
    assert_eq!(g.step(Action::Wait), Ok(Some(End::Failed(FailReason::Boundary)))); // crosses x = 6 during beat 3
    assert!(g.history[2].to().x > 5.9 && g.history[2].to().x < 6.2, "run stops right at the boundary");
    assert_eq!(g.choose(Action::Wait), Err(ChooseError::Finished), "frozen after the end");
    // numerically invalid field
    let l = mk(Field::new().fixed(Term::Custom(|_, _| v2(f64::NAN, 0.0))), far_target(), v2(0.0, 0.0), 5, 1, 1.0, GustDirs::Four);
    let mut g = SteerGame::new(l);
    assert_eq!(g.step(Action::Wait), Ok(Some(End::Failed(FailReason::Invalid))));
    assert!(g.pos().is_finite(), "invalid values never reach the state");
    // runaway speed counts as invalid too
    let l = mk(Field::new().fixed(Term::Uniform(v2(1e6, 0.0))), far_target(), v2(0.0, 0.0), 5, 1, 1.0, GustDirs::Four);
    assert_eq!(SteerGame::new(l).step(Action::Wait), Ok(Some(End::Failed(FailReason::Invalid))));
    // beats exhausted
    let l = mk(Field::new().fixed(Term::Uniform(v2(0.01, 0.0))), far_target(), v2(0.0, 0.0), 3, 0, 1.0, GustDirs::Four);
    let mut g = SteerGame::new(l);
    assert_eq!(g.step(Action::Wait), Ok(None));
    assert_eq!(g.step(Action::Wait), Ok(None));
    assert_eq!(g.step(Action::Wait), Ok(Some(End::Failed(FailReason::BeatsExhausted))));
}

#[test]
fn duplicate_input_cannot_advance_the_simulation_twice() {
    let mut g = SteerGame::new(mk(swirl(), far_target(), v2(-3.0, 0.5), 6, 2, 1.0, GustDirs::Eight));
    assert_eq!(g.phase, Phase::Ready);
    g.choose(Action::Gust(0)).unwrap();
    assert_eq!(g.phase, Phase::Advancing);
    // everything is rejected while the beat is in flight
    assert_eq!(g.choose(Action::Gust(1)), Err(ChooseError::Busy));
    assert_eq!(g.choose(Action::Wait), Err(ChooseError::Busy));
    assert_eq!(g.retry(), Err(ChooseError::Busy));
    assert_eq!(g.gusts_left(), 1, "only one gust spent");
    assert_eq!(g.beat(), 0, "beat counter advances only when the beat finishes");
    g.finish_beat();
    assert_eq!(g.finish_beat(), None, "finishing twice does nothing");
    assert_eq!(g.beat(), 1);
    assert_eq!(g.history.len(), 1);
    assert_eq!(g.phase, Phase::Awaiting);
}

#[test]
fn retry_restores_the_initial_state_and_keeps_a_ghost() {
    let l = steer_level("ste-1");
    let mut g = SteerGame::new(l.clone());
    let initial_pos = g.pos();
    for _ in 0..5 {
        if g.step(Action::Wait).unwrap().is_some() {
            break;
        }
    }
    assert!(matches!(g.phase, Phase::Completed(_)));
    let first = g.run();
    g.retry().unwrap();
    assert_eq!((g.pos(), g.beat(), g.gusts_left(), g.phase), (initial_pos, 0, 1, Phase::Ready));
    assert!(g.history.is_empty());
    assert_eq!(g.ghosts.len(), 1);
    assert_eq!(g.ghosts[0].path(), first.path());
    // ghosts are capped but the current run's history is never truncated
    for _ in 0..(MAX_GHOSTS + 3) {
        for _ in 0..5 {
            if g.step(Action::Wait).unwrap().is_some() {
                break;
            }
        }
        g.retry().unwrap();
    }
    assert_eq!(g.ghosts.len(), MAX_GHOSTS);
    for _ in 0..5 {
        let _ = g.step(Action::Wait);
    }
    assert!(!g.history.is_empty() && g.history.len() <= 5, "the current run keeps its full history ({} beats)", g.history.len());
    // replay of the last finished run equals the run itself
    let r = g.replayable().unwrap();
    assert_eq!(SteerSpec::from_level(&l).replay(&r.actions()).unwrap().path(), r.path());
}

// ------------------------------------------------------------- level design
#[test]
fn steer_chapter_matches_the_spec_configuration() {
    let ls = levels::chapter(Chapter::Steer);
    assert_eq!(ls.len(), 5);
    let cfg: Vec<(usize, usize)> = ls
        .iter()
        .map(|l| match &l.rules {
            Rules::Steer { beats, gust_budget, .. } => (*beats, *gust_budget),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(cfg, vec![(5, 1), (6, 2), (6, 1), (7, 2), (8, 2)]);
    for l in &ls {
        assert!(!l.hints.is_empty());
        assert!(vs::enumerate(&SteerSpec::from_level(l)).successes > 0, "{} must be solvable", l.id);
    }
}

#[test]
fn no_level_is_solved_by_doing_nothing() {
    for l in levels::chapter(Chapter::Steer) {
        let en = vs::enumerate(&SteerSpec::from_level(&l));
        assert!(!en.no_gust_success, "{}", l.id);
        assert_ne!(vs::natural_run(&SteerSpec::from_level(&l)).end, Some(End::Success));
    }
}

#[test]
fn level1_natural_path_narrowly_misses_and_one_gust_is_forgiving() {
    let c = vs::check(&steer_level("ste-1"));
    assert!(c.natural_miss_margin > 0.0 && c.natural_miss_margin < 0.3, "narrow miss, got {}", c.natural_miss_margin);
    assert_eq!(c.min_gusts, Some(1));
    assert!(c.single_gust_success >= 0.2, "forgiving: {:.0}% of placements work", 100.0 * c.single_gust_success);
    assert!(c.winning_gust_beats.len() >= 3, "works at several beats, so it is about velocity, not a magic moment");
}

#[test]
fn level2_leverage_varies_and_a_high_leverage_gust_helps_but_can_mislead() {
    let l = steer_level("ste-2");
    let spec = SteerSpec::from_level(&l);
    let lev = vs::leverage_curve(&spec);
    let (mx, mn) = (lev.iter().cloned().fold(0.0, f64::max), lev.iter().cloned().fold(f64::INFINITY, f64::min));
    assert!(mx / mn >= 4.0, "leverage must vary meaningfully: {:?}", lev);
    assert!(lev.windows(2).all(|w| w[0] + 1e-9 >= w[1]), "sensitivity falls as the probe leaves the divergent zone: {:?}", lev);
    let en = vs::enumerate(&spec);
    let c = vs::check(&l);
    assert!(c.high_leverage_helps);
    // every single-gust solution uses one of the higher-leverage beats; late gusts alone cannot solve it
    let singles: Vec<usize> = en
        .solutions
        .iter()
        .filter(|s| s.iter().filter(|a| matches!(a, Action::Gust(_))).count() == 1)
        .flat_map(|s| s.iter().enumerate().filter(|(_, a)| matches!(a, Action::Gust(_))).map(|(b, _)| b))
        .collect();
    assert!(!singles.is_empty());
    assert!(singles.iter().all(|b| lev[*b] >= 0.5 * mx), "single-gust wins at beats {:?} vs leverage {:?}", singles, lev);
    assert!(c.high_leverage_can_mislead, "some high-leverage first gusts have no winning continuation");
    assert!(c.second_gust_variants > 1, "the best second gust depends on the first");
    assert!(!c.toward_only && !c.greedy_bearing && !c.greedy_lookahead);
}

#[test]
fn level3_timing_matters_and_greedy_fails() {
    let l = steer_level("ste-3");
    let spec = SteerSpec::from_level(&l);
    let c = vs::check(&l);
    assert!(!c.greedy_bearing && !c.greedy_lookahead && !c.toward_only, "always pushing toward the target must fail");
    assert_eq!(c.min_gusts, Some(1));
    // the same gust works early and fails late
    let max_win = *c.winning_gust_beats.iter().max().unwrap();
    assert!((1..=3).contains(&max_win), "window {:?}", c.winning_gust_beats);
    let en = vs::enumerate(&spec);
    for late in 4..spec.beats {
        for d in 0..8 {
            let mut acts = vec![Action::Wait; late];
            acts.push(Action::Gust(d));
            assert!(!en.solutions.iter().any(|s| s.starts_with(&acts)), "gust {d} at beat {late} should not recover the run");
        }
    }
    // the winning first gust is the *same* direction early and late-in-window (timing, not aim)
    assert!(c.winning_gust_beats.len() >= 2);
}

#[test]
fn level4_needs_an_indirect_gust() {
    let l = steer_level("ste-4");
    let c = vs::check(&l);
    assert!(!c.greedy_bearing && !c.greedy_lookahead && !c.toward_only);
    assert_eq!(c.min_gusts, Some(2), "one nudge is not enough");
    assert!(c.first_gust_sideways_share >= 0.5, "{:.0}%", 100.0 * c.first_gust_sideways_share);
    // concrete witness: a solution whose first gust points >= 80 degrees away from the bearing to the target
    let spec = SteerSpec::from_level(&l);
    let en = vs::enumerate(&spec);
    let tc = spec.regions[spec.goal].center;
    let witness = en.solutions.iter().any(|s| {
        let Some((b, Action::Gust(d))) = s.iter().cloned().enumerate().find(|(_, a)| matches!(a, Action::Gust(_))) else { return false };
        let mut st = spec.initial();
        for a in &s[..b] {
            st = spec.advance(&st, *a).unwrap().next;
        }
        let bear = tc - st.pos;
        spec.dirs.unit(d).dot(bear) / bear.len() <= 0.1737
    });
    assert!(witness);
}

#[test]
fn level5_requires_a_setup_gust() {
    let l = steer_level("ste-5");
    let c = vs::check(&l);
    assert!(!c.greedy_bearing && !c.greedy_lookahead && !c.toward_only);
    assert_eq!(c.min_gusts, Some(2), "both gusts are needed");
    assert_eq!(c.single_gust_success, 0.0);
    assert!(c.second_gust_variants > 1, "the follow-up gust depends on the first");
    assert!(c.first_gust_greedy_share < 0.2, "the first gust is rarely the greedy one");
    let Rules::Steer { .. } = &l.rules else { unreachable!() };
    assert!(l.regions[0].radius <= 0.3, "narrow target");
}

#[test]
fn validation_report_covers_every_level() {
    let r = vs::report(&levels::chapter(Chapter::Steer));
    for id in ["ste-1", "ste-2", "ste-3", "ste-4", "ste-5"] {
        assert!(r.contains(id), "{id}");
    }
    assert!(r.contains("leverage per beat"));
}
