//! Automated validation of Steer levels (spec section 8): exhaustive search over every legal gust
//! sequence, greedy-policy baselines, a leverage curve, and sequence-dependence statistics.
use crate::field::V2;
use crate::level::Level;
use crate::modes::steer::{Action, End, Run, SteerSpec};
use crate::sim::{advance_beat, Rect};
use std::collections::BTreeSet;

/// A gust inside a sequence: (beat index, direction index).
pub type GustAt = (usize, usize);

fn gusts_of(actions: &[Action]) -> Vec<GustAt> {
    actions.iter().enumerate().filter_map(|(b, a)| if let Action::Gust(d) = a { Some((b, *d)) } else { None }).collect()
}

/// Result of enumerating every legal terminal run (depth-first; a run that ends early is one leaf).
#[derive(Clone, Debug, Default)]
pub struct Enumeration {
    pub runs: usize,
    pub successes: usize,
    pub min_gusts: Option<usize>,
    pub no_gust_success: bool,
    pub success_by_gusts: Vec<usize>,
    /// Every successful action sequence (up to the point success was reached).
    pub solutions: Vec<Vec<Action>>,
}

pub fn enumerate(spec: &SteerSpec) -> Enumeration {
    let mut e = Enumeration { success_by_gusts: vec![0; spec.budget + 1], ..Default::default() };
    let mut prefix = vec![];
    dfs(spec, &spec.initial(), &mut prefix, &mut e);
    e
}

fn dfs(spec: &SteerSpec, s: &crate::modes::steer::State, prefix: &mut Vec<Action>, e: &mut Enumeration) {
    let mut actions = vec![Action::Wait];
    if s.gusts_left > 0 {
        actions.extend((0..spec.dirs.count()).map(Action::Gust));
    }
    for a in actions {
        let b = spec.advance(s, a).expect("legal by construction");
        prefix.push(a);
        match b.next.end {
            Some(end) => {
                e.runs += 1;
                if end == End::Success {
                    let g = gusts_of(prefix).len();
                    e.successes += 1;
                    e.success_by_gusts[g] += 1;
                    e.min_gusts = Some(e.min_gusts.map_or(g, |m| m.min(g)));
                    if g == 0 {
                        e.no_gust_success = true;
                    }
                    e.solutions.push(prefix.clone());
                }
            }
            None => dfs(spec, &b.next, prefix, e),
        }
        prefix.pop();
    }
}

/// Play `policy` to the end and return the run.
fn play(spec: &SteerSpec, mut policy: impl FnMut(&SteerSpec, &crate::modes::steer::State) -> Action) -> Run {
    let mut s = spec.initial();
    let mut beats = vec![];
    while s.end.is_none() {
        let a = policy(spec, &s);
        let b = spec.advance(&s, a).expect("policy must be legal");
        s = b.next.clone();
        beats.push(b);
    }
    Run { start: spec.start, beats, end: s.end }
}

pub fn natural_run(spec: &SteerSpec) -> Run {
    play(spec, |_, _| Action::Wait)
}

fn target(spec: &SteerSpec) -> (V2, f64) {
    let r = &spec.regions[spec.goal];
    (r.center, r.radius)
}

/// Compass direction (among the permitted ones) closest to the bearing from `p` to the target.
pub fn bearing_dir(spec: &SteerSpec, p: V2) -> usize {
    let (c, _) = target(spec);
    let d = c - p;
    (0..spec.dirs.count()).max_by(|a, b| spec.dirs.unit(*a).dot(d).partial_cmp(&spec.dirs.unit(*b).dot(d)).unwrap()).unwrap()
}

/// Greedy 1: always push toward the target (nearest permitted compass direction) while gusts remain.
pub fn greedy_bearing(spec: &SteerSpec) -> Run {
    play(spec, |sp, s| if s.gusts_left > 0 { Action::Gust(bearing_dir(sp, s.pos)) } else { Action::Wait })
}

/// Greedy 2: each beat take the legal action that minimises the probe's distance to the target at the
/// end of that beat (ties prefer waiting). Only a validation baseline: the UI never computes this.
pub fn greedy_lookahead(spec: &SteerSpec) -> Run {
    let (c, _) = target(spec);
    play(spec, |sp, s| {
        let mut best = (Action::Wait, f64::INFINITY);
        let mut acts = vec![Action::Wait];
        if s.gusts_left > 0 {
            acts.extend((0..sp.dirs.count()).map(Action::Gust));
        }
        for a in acts {
            let b = sp.advance(s, a).unwrap();
            let d = if b.next.end == Some(End::Success) { -1.0 } else { b.to().dist(c) };
            if d < best.1 - 1e-12 {
                best = (a, d);
            }
        }
        best.0
    })
}

/// Does ANY sequence whose every gust points toward the target (at the moment it is used) succeed?
/// This is the strongest form of "always push toward the target", over every possible timing.
pub fn toward_only_solves(spec: &SteerSpec) -> bool {
    fn rec(spec: &SteerSpec, s: &crate::modes::steer::State) -> bool {
        let mut acts = vec![Action::Wait];
        if s.gusts_left > 0 {
            acts.push(Action::Gust(bearing_dir(spec, s.pos)));
        }
        for a in acts {
            let b = spec.advance(s, a).unwrap();
            match b.next.end {
                Some(End::Success) => return true,
                Some(_) => {}
                None => {
                    if rec(spec, &b.next) {
                        return true;
                    }
                }
            }
        }
        false
    }
    rec(spec, &spec.initial())
}

/// Physics-only position after `beats` beats for an action sequence (no bounds, no regions), so the
/// leverage of an intervention can be compared even when the real run would have ended early.
pub fn free_final(spec: &SteerSpec, actions: &[Action]) -> V2 {
    let mut cfg = spec.cfg.clone();
    cfg.bounds = Rect::new(-1e6, -1e6, 1e6, 1e6);
    let mut p = spec.start;
    for b in 0..spec.beats {
        let a = actions.get(b).copied().unwrap_or(Action::Wait);
        let (pts, _) = advance_beat(&spec.field, p, b as f64 * spec.beat_time, spec.gust(a), spec.beat_time, &[], &cfg, &mut vec![]);
        p = pts.last().copied().unwrap_or(p);
    }
    p
}

/// Leverage of a single gust at each beat: the largest displacement of the final position (relative to
/// the no-gust baseline) over all directions, divided by the gust's nominal displacement
/// `strength * beat_time`. 1.0 means "moved the endpoint exactly as much as the push itself".
pub fn leverage_curve(spec: &SteerSpec) -> Vec<f64> {
    let base = free_final(spec, &[]);
    let nominal = spec.strength * spec.beat_time;
    (0..spec.beats)
        .map(|b| {
            (0..spec.dirs.count())
                .map(|d| {
                    let mut acts = vec![Action::Wait; spec.beats];
                    acts[b] = Action::Gust(d);
                    free_final(spec, &acts).dist(base) / nominal
                })
                .fold(0.0, f64::max)
        })
        .collect()
}

/// Everything the spec asks us to record for one level.
#[derive(Clone, Debug)]
pub struct LevelCheck {
    pub id: &'static str,
    pub beats: usize,
    pub budget: usize,
    pub runs: usize,
    pub successes: usize,
    pub min_gusts: Option<usize>,
    pub no_gust_success: bool,
    pub natural_miss_margin: f64,
    pub greedy_bearing: bool,
    pub greedy_lookahead: bool,
    pub toward_only: bool,
    pub leverage: Vec<f64>,
    /// share of single-gust placements (beat x direction) that succeed
    pub single_gust_success: f64,
    /// beats at which at least one successful sequence places a gust
    pub winning_gust_beats: Vec<usize>,
    /// a solution exists that uses a gust at (one of) the highest-leverage beat(s)
    pub high_leverage_helps: bool,
    /// some first gust at a high-leverage beat has no successful continuation at all
    pub high_leverage_can_mislead: bool,
    /// distinct sets of successful follow-up gusts across successful first gusts (>1 => depends on first)
    pub second_gust_variants: usize,
    /// share of successful sequences whose first gust is >=80 degrees away from the bearing to the target
    pub first_gust_sideways_share: f64,
    /// share of successful sequences whose first gust equals the greedy-lookahead first choice
    pub first_gust_greedy_share: f64,
}

pub fn check(level: &Level) -> LevelCheck {
    let spec = SteerSpec::from_level(level);
    let en = enumerate(&spec);
    let nat = natural_run(&spec);
    let (c, r) = target(&spec);
    let natural_min = nat.path().iter().map(|p| p.dist(c)).fold(f64::INFINITY, f64::min);
    let lev = leverage_curve(&spec);
    let maxlev = lev.iter().cloned().fold(0.0, f64::max);
    let high: Vec<usize> = (0..lev.len()).filter(|b| lev[*b] >= 0.8 * maxlev).collect();

    // single-gust placements
    let singles = spec.beats * spec.dirs.count();
    let single_ok = en.solutions.iter().filter(|s| gusts_of(s).len() == 1).count();
    let winning_gust_beats: Vec<usize> = en.solutions.iter().flat_map(|s| gusts_of(s)).map(|g| g.0).collect::<BTreeSet<_>>().into_iter().collect();
    let high_leverage_helps = en.solutions.iter().any(|s| gusts_of(s).iter().any(|g| high.contains(&g.0)));

    // group successful sequences by their first gust
    let mut by_first: std::collections::BTreeMap<GustAt, BTreeSet<Vec<GustAt>>> = Default::default();
    for s in &en.solutions {
        let g = gusts_of(s);
        if let Some(first) = g.first() {
            by_first.entry(*first).or_default().insert(g[1..].to_vec());
        }
    }
    let variants = by_first.values().collect::<BTreeSet<_>>().len();
    let mut mislead = false;
    for b in &high {
        for d in 0..spec.dirs.count() {
            if spec.budget > 1 && !by_first.contains_key(&(*b, d)) {
                mislead = true;
            }
        }
    }
    if spec.budget == 1 {
        // with one gust, "misleading" = a high-leverage gust that does not succeed
        for b in &high {
            for d in 0..spec.dirs.count() {
                if !en.solutions.iter().any(|s| gusts_of(s) == vec![(*b, d)]) {
                    mislead = true;
                }
            }
        }
    }

    // first-gust comparisons
    let gl = greedy_lookahead(&spec);
    let greedy_first = gl.beats.iter().enumerate().find_map(|(b, x)| if let Action::Gust(d) = x.action { Some((b, d)) } else { None });
    let mut sideways = 0;
    let mut greedy_same = 0;
    let mut with_gust = 0;
    for s in &en.solutions {
        let g = gusts_of(s);
        let Some(&(b, d)) = g.first() else { continue };
        with_gust += 1;
        // position of the probe when that first gust is applied (no earlier gust)
        let mut st = spec.initial();
        for a in &s[..b] {
            st = spec.advance(&st, *a).unwrap().next;
        }
        let bear = c - st.pos;
        let cosang = spec.dirs.unit(d).dot(bear) / bear.len().max(1e-9);
        if cosang <= 0.1737 {
            sideways += 1; // angle >= 80 degrees
        }
        if greedy_first == Some((b, d)) {
            greedy_same += 1;
        }
    }
    let share = |n: usize| if with_gust == 0 { 0.0 } else { n as f64 / with_gust as f64 };
    let _ = r;
    LevelCheck {
        id: level.id,
        beats: spec.beats,
        budget: spec.budget,
        runs: en.runs,
        successes: en.successes,
        min_gusts: en.min_gusts,
        no_gust_success: en.no_gust_success,
        natural_miss_margin: natural_min - r,
        greedy_bearing: greedy_bearing(&spec).end == Some(End::Success),
        greedy_lookahead: gl.end == Some(End::Success),
        toward_only: toward_only_solves(&spec),
        leverage: lev,
        single_gust_success: single_ok as f64 / singles as f64,
        winning_gust_beats,
        high_leverage_helps,
        high_leverage_can_mislead: mislead,
        second_gust_variants: variants,
        first_gust_sideways_share: share(sideways),
        first_gust_greedy_share: share(greedy_same),
    }
}

/// Markdown validation report for a set of Steer levels.
pub fn report(levels: &[Level]) -> String {
    let mut s = String::new();
    s.push_str("# Steer validation report\n\nExhaustive enumeration of every legal gust sequence (a run that ends early counts once).\n\n");
    s.push_str("| level | beats | gusts | sequences | solutions | min gusts | no-gust wins | greedy: bearing / lookahead / any toward-target timing |\n|---|---|---|---|---|---|---|---|\n");
    let checks: Vec<LevelCheck> = levels.iter().map(check).collect();
    let yn = |b: bool| if b { "solves" } else { "fails" };
    for c in &checks {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} / {} / {} |\n",
            c.id,
            c.beats,
            c.budget,
            c.runs,
            c.successes,
            c.min_gusts.map_or("none".into(), |m| m.to_string()),
            if c.no_gust_success { "YES (bad)" } else { "no" },
            yn(c.greedy_bearing),
            yn(c.greedy_lookahead),
            yn(c.toward_only)
        ));
    }
    s.push_str("\n## Per-level detail\n");
    for c in &checks {
        s.push_str(&format!("\n### {}\n", c.id));
        s.push_str(&format!("- natural path misses the target by {:.2} (distance beyond tolerance)\n", c.natural_miss_margin));
        s.push_str(&format!("- single-gust placements that succeed: {:.0}%; beats used by winning gusts: {:?}\n", 100.0 * c.single_gust_success, c.winning_gust_beats));
        s.push_str(&format!("- leverage per beat (final-position shift / nominal push): {}\n", c.leverage.iter().map(|x| format!("{:.2}", x)).collect::<Vec<_>>().join(", ")));
        s.push_str(&format!("- a solution uses a high-leverage beat: {}; a high-leverage gust can still fail: {}\n", c.high_leverage_helps, c.high_leverage_can_mislead));
        if c.budget > 1 {
            s.push_str(&format!("- distinct follow-up sets across successful first gusts: {} (>1 means the best next gust depends on the first)\n", c.second_gust_variants));
        }
        s.push_str(&format!("- first gust: {:.0}% of solutions start sideways/away (>=80 deg from the target bearing); {:.0}% start with the greedy choice\n", 100.0 * c.first_gust_sideways_share, 100.0 * c.first_gust_greedy_share));
    }
    s
}
