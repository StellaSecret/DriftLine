//! Validation of Survey levels (see `docs/09-survey.md`). Nothing here is part of the game.
//!
//! **Model.** The hidden source is one of a finite set of *candidates* on a 0.5 grid (`hyp`). A scout
//! released at a lattice point `s` would draw a different trail under each candidate. Two candidates
//! are *indistinguishable from scout `s`* when their trails never differ by more than `eps`, so the
//! evidence a scout gives is the set of candidates whose predicted trail matches the observed one.
//! (If no candidate matches, the best-fitting ones are kept.) The grid is a device of this validator
//! only: the game's real solution space is continuous and the source can be anywhere.
//!
//! **Three different thresholds** (they answer different questions, see the doc):
//! * numerical precision: do the simulated trails differ at all (not used for decisions);
//! * `EPS_FINE`: a difference a careful player could see on screen; a level must be *solvable* at it;
//! * `EPS_COARSE`: the precision of *mental* inference, a difficulty parameter. The level should still
//!   be non-trivial (free scout alone does not solve it) and solvable by an adaptive player at it.
//! The commit tolerance (`SurveySpec::tol`) is separate again.
//!
//! **Players** compared: a fixed pattern of scouts, uniformly random scouts, an adaptive player that
//! picks each scout to minimise the expected size of the remaining candidate set, and an oracle that
//! knows the truth and tries *every* combination of scouts (a level is solvable iff the oracle wins).
use crate::field::{v2, V2};
use crate::level::Level;
use crate::modes::{Lcg, SurveyGame, SurveySpec, MIN_SCOUT_SEPARATION};
use crate::sim::{Rect, Trajectory};

pub const EPS_FINE: f64 = 0.15;
pub const EPS_COARSE: f64 = 0.4;
/// Region where sources are authored, and the spacing of the candidate grid.
pub const HYP_RECT: Rect = Rect::new(-4.0, -3.0, 4.0, 3.0);
pub const HYP_STEP: f64 = 0.5;
/// Samples compared along a trail, one every `SAMPLE_DT` seconds (34 x 0.15 s = 5.1 s).
const NS: usize = 34;
const SAMPLE_DT: f64 = 0.15;

pub struct Model {
    pub spec: SurveySpec,
    pub hyp: Vec<V2>,
    /// Lattice scouts (integer points inside the zone) followed by the free scout (last).
    pub starts: Vec<V2>,
    pub nh: usize,
    pub ns: usize,
    obs: Vec<V2>,
}

fn samples(t: &Trajectory) -> [V2; NS] {
    let mut out = [V2::ZERO; NS];
    for (q, o) in out.iter_mut().enumerate() {
        *o = t.pos_at(q as f64 * SAMPLE_DT);
    }
    out
}
fn dev(a: &[V2], b: &[V2]) -> f64 {
    a.iter().zip(b).map(|(p, q)| p.dist(*q)).fold(0.0, f64::max)
}

impl Model {
    pub fn new(level: &Level) -> Model {
        let spec = SurveySpec::from_level(level);
        let mut hyp = vec![];
        let (nx, ny) = ((HYP_RECT.width() / HYP_STEP).round() as usize, (HYP_RECT.height() / HYP_STEP).round() as usize);
        for i in 0..=nx {
            for j in 0..=ny {
                hyp.push(v2(HYP_RECT.min.x + HYP_STEP * i as f64, HYP_RECT.min.y + HYP_STEP * j as f64));
            }
        }
        let mut starts = vec![];
        for x in -5..=5 {
            for y in -3..=3 {
                let p = v2(x as f64, y as f64);
                if spec.zone.contains(p) && p.dist(spec.free) >= MIN_SCOUT_SEPARATION {
                    starts.push(p);
                }
            }
        }
        starts.push(spec.free);
        let (nh, ns) = (hyp.len(), starts.len());
        let mut obs = vec![V2::ZERO; ns * nh * NS];
        for s in 0..ns {
            for h in 0..nh {
                let tr = samples(&spec.fly(hyp[h], starts[s]));
                obs[(s * nh + h) * NS..(s * nh + h + 1) * NS].copy_from_slice(&tr);
            }
        }
        Model { spec, hyp, starts, nh, ns, obs }
    }

    pub fn free_index(&self) -> usize {
        self.ns - 1
    }

    fn row(&self, s: usize, h: usize) -> &[V2] {
        &self.obs[(s * self.nh + h) * NS..(s * self.nh + h + 1) * NS]
    }

    /// For one observed trail from scout `s`: how far each candidate's predicted trail is from it.
    pub fn devs(&self, s: usize, observed: &Trajectory) -> Vec<f64> {
        let o = samples(observed);
        (0..self.nh).map(|h| dev(self.row(s, h), &o)).collect()
    }

    /// `d[s * nh + h]` for every scout, when the true source is `source`.
    pub fn truth_devs(&self, source: V2) -> Vec<f64> {
        let mut d = Vec::with_capacity(self.ns * self.nh);
        for s in 0..self.ns {
            d.extend(self.devs(s, &self.spec.fly(source, self.starts[s])));
        }
        d
    }

    /// Keep the candidates within `eps` of what was seen (or the best fit if none is).
    pub fn refine_with(&self, c: &[usize], dv: &[f64], eps: f64) -> Vec<usize> {
        let mn = c.iter().map(|&h| dv[h]).fold(f64::INFINITY, f64::min);
        let lim = eps.max(mn) + 1e-12;
        c.iter().copied().filter(|&h| dv[h] <= lim).collect()
    }
    fn refine(&self, c: &[usize], s: usize, d: &[f64], eps: f64) -> Vec<usize> {
        self.refine_with(c, &d[s * self.nh..(s + 1) * self.nh], eps)
    }

    pub fn centroid(&self, c: &[usize]) -> V2 {
        let n = c.len() as f64;
        let (x, y) = c.iter().fold((0.0, 0.0), |a, &h| (a.0 + self.hyp[h].x, a.1 + self.hyp[h].y));
        v2(x / n, y / n)
    }
    /// Would committing the centroid of `c` be accepted?
    pub fn ok(&self, c: &[usize], source: V2) -> bool {
        self.centroid(c).dist(source) <= self.spec.tol
    }
    pub fn diameter(&self, c: &[usize]) -> f64 {
        let mut m: f64 = 0.0;
        for &a in c {
            for &b in c {
                m = m.max(self.hyp[a].dist(self.hyp[b]));
            }
        }
        m
    }

    /// `ind[(s*nh + a)*nh + b]`: candidates `a` and `b` look the same from scout `s` at precision `eps`.
    pub fn indist(&self, eps: f64) -> Vec<bool> {
        let (nh, ns) = (self.nh, self.ns);
        let mut ind = vec![false; ns * nh * nh];
        for s in 0..ns {
            for a in 0..nh {
                for b in a..nh {
                    let same = dev(self.row(s, a), self.row(s, b)) <= eps;
                    ind[(s * nh + a) * nh + b] = same;
                    ind[(s * nh + b) * nh + a] = same;
                }
            }
        }
        ind
    }

    /// Greedy information choice: the lattice scout minimising the expected remaining candidates,
    /// assuming the truth is uniform over `c`. Scouts too close to a used one are skipped.
    pub fn best_scout(&self, ind: &[bool], c: &[usize], used: &[usize]) -> usize {
        let mut best = (f64::INFINITY, 0);
        for s in 0..self.ns - 1 {
            if used.iter().any(|&u| self.starts[u].dist(self.starts[s]) < MIN_SCOUT_SEPARATION) {
                continue;
            }
            let mut sum = 0usize;
            for &a in c {
                for &b in c {
                    if ind[(s * self.nh + a) * self.nh + b] {
                        sum += 1;
                    }
                }
            }
            let e = sum as f64 / c.len() as f64;
            if e < best.0 {
                best = (e, s);
            }
        }
        best.1
    }

    fn lattice_near(&self, p: V2) -> usize {
        (0..self.ns - 1).min_by(|&a, &b| self.starts[a].dist(p).partial_cmp(&self.starts[b].dist(p)).unwrap()).unwrap()
    }

    /// Candidates left after the free scout alone.
    pub fn after_free(&self, d: &[f64], eps: f64) -> Vec<usize> {
        let all: Vec<usize> = (0..self.nh).collect();
        self.refine(&all, self.free_index(), d, eps)
    }

    /// Is there any set of at most `max_scouts` scouts (besides the free one) after which the centroid
    /// of the surviving candidates is accepted? Exhaustive over the lattice.
    pub fn oracle_solves_within(&self, source: V2, d: &[f64], eps: f64, max_scouts: usize) -> bool {
        fn dfs(m: &Model, source: V2, d: &[f64], eps: f64, c: &[usize], depth: usize, from: usize) -> bool {
            if m.ok(c, source) {
                return true;
            }
            if depth == 0 {
                return false;
            }
            for s in from..m.ns - 1 {
                let c2 = m.refine(c, s, d, eps);
                if dfs(m, source, d, eps, &c2, depth - 1, s + 1) {
                    return true;
                }
            }
            false
        }
        let c0 = self.after_free(d, eps);
        dfs(self, source, d, eps, &c0, max_scouts, 0)
    }

    /// Solvable within the level's scout budget.
    pub fn oracle_solves(&self, source: V2, d: &[f64], eps: f64) -> bool {
        self.oracle_solves_within(source, d, eps, self.spec.budget)
    }

    /// The fewest paid scouts a player who *already knows the answer* would need (a lower bound that
    /// proves solvability; it is NOT a measure of difficulty, such a player always knows where to look).
    pub fn min_scouts(&self, source: V2, d: &[f64], eps: f64) -> Option<usize> {
        (0..=self.spec.budget).find(|&n| self.oracle_solves_within(source, d, eps, n))
    }

    /// Adaptive player (model only), with the level's budget: returns the final candidate set.
    pub fn adaptive(&self, ind: &[bool], d: &[f64], eps: f64) -> Vec<usize> {
        self.adaptive_n(ind, d, eps, self.spec.budget)
    }

    /// The same player with `n` paid scouts (to see how much each extra scout is worth).
    pub fn adaptive_n(&self, ind: &[bool], d: &[f64], eps: f64, n: usize) -> Vec<usize> {
        let mut c = self.after_free(d, eps);
        let mut used = vec![self.free_index()];
        for _ in 0..n {
            let s = self.best_scout(ind, &c, &used);
            used.push(s);
            c = self.refine(&c, s, d, eps);
        }
        c
    }

    /// How many paid scouts the adaptive player has placed when its estimate first becomes acceptable.
    /// Unlike `min_scouts`, this player does not know the answer, so it measures what the budget costs a
    /// real player. `None` if it never gets there within the budget.
    pub fn adaptive_needs(&self, ind: &[bool], d: &[f64], eps: f64, source: V2) -> Option<usize> {
        (0..=self.spec.budget).find(|&n| self.ok(&self.adaptive_n(ind, d, eps, n), source))
    }

    /// A fixed, observation-independent pattern of scouts.
    pub fn fixed_pattern(&self, d: &[f64], eps: f64) -> Vec<usize> {
        const PATTERN: [(f64, f64); 5] = [(-3.0, -2.0), (3.0, 2.0), (3.0, -2.0), (-3.0, 2.0), (0.0, 0.0)];
        let mut c = self.after_free(d, eps);
        for &(x, y) in PATTERN.iter().take(self.spec.budget) {
            c = self.refine(&c, self.lattice_near(v2(x, y)), d, eps);
        }
        c
    }

    /// Uniformly random distinct scouts, ignoring what they showed.
    pub fn random_scouts(&self, d: &[f64], eps: f64, rng: &mut Lcg) -> Vec<usize> {
        let mut c = self.after_free(d, eps);
        let mut used: Vec<usize> = vec![];
        while used.len() < self.spec.budget {
            let s = rng.below(self.ns - 1);
            if !used.contains(&s) {
                used.push(s);
                c = self.refine(&c, s, d, eps);
            }
        }
        c
    }
}

/// The same adaptive player, but driven through the real `SurveyGame` and using only what the game
/// shows (the trails it returns). It must agree with `Model::adaptive`.
pub fn adaptive_play(level: &Level, model: &Model, ind: &[bool], attempt: usize, eps: f64) -> bool {
    let mut g = SurveyGame::new(level.clone());
    for _ in 0..attempt {
        g.set_marker(g.spec.zone.center()).unwrap();
        g.commit().unwrap();
        g.retry().unwrap();
    }
    let free = model.free_index();
    let all: Vec<usize> = (0..model.nh).collect();
    let mut c = model.refine_with(&all, &model.devs(free, &g.round().scouts[0].traj), eps);
    let mut used = vec![free];
    while g.scouts_left() > 0 {
        let s = model.best_scout(ind, &c, &used);
        used.push(s);
        let traj = g.place_scout(model.starts[s]).expect("a lattice scout is always placeable").traj.clone();
        c = model.refine_with(&c, &model.devs(s, &traj), eps);
    }
    g.set_marker(model.centroid(&c)).unwrap();
    g.commit().unwrap().ok
}

#[derive(Clone, Debug)]
pub struct TruthReport {
    pub source: V2,
    /// Candidates left after the free scout at `EPS_COARSE`, and their spread.
    pub free_candidates: usize,
    pub free_diameter: f64,
    pub free_solves: bool,
    pub solvable_fine: bool,
    pub solvable_coarse: bool,
    /// Paid scouts the adaptive (non-omniscient) player needs at `EPS_COARSE`.
    pub adaptive_needs_coarse: Option<usize>,
    pub adaptive_coarse: bool,
    pub grid_coarse: bool,
    pub random_coarse: f64,
}

pub fn truth_report(m: &Model, ind_coarse: &[bool], source: V2) -> TruthReport {
    let d = m.truth_devs(source);
    let c0 = m.after_free(&d, EPS_COARSE);
    let mut rng = Lcg(0x5eed ^ (source.x.to_bits() ^ source.y.to_bits().rotate_left(7)));
    let draws = 20;
    let random_ok = (0..draws).filter(|_| m.ok(&m.random_scouts(&d, EPS_COARSE, &mut rng), source)).count();
    TruthReport {
        source,
        free_candidates: c0.len(),
        free_diameter: m.diameter(&c0),
        free_solves: m.ok(&c0, source),
        solvable_fine: m.oracle_solves(source, &d, EPS_FINE),
        solvable_coarse: m.oracle_solves(source, &d, EPS_COARSE),
        adaptive_needs_coarse: m.adaptive_needs(ind_coarse, &d, EPS_COARSE, source),
        adaptive_coarse: m.ok(&m.adaptive(ind_coarse, &d, EPS_COARSE), source),
        grid_coarse: m.ok(&m.fixed_pattern(&d, EPS_COARSE), source),
        random_coarse: random_ok as f64 / draws as f64,
    }
}

/// Success rates over many random continuous sources (at `eps`).
#[derive(Clone, Debug)]
pub struct Population {
    pub n: usize,
    pub free_only: f64,
    pub ambiguous_after_free: f64,
    pub grid: f64,
    pub random: f64,
    pub adaptive: f64,
    /// The adaptive player with a single paid scout (what the rest of the budget is worth).
    pub adaptive_one: f64,
    pub unsolvable: f64,
}

pub fn random_sources(n: usize, seed: u64) -> Vec<V2> {
    let mut rng = Lcg(seed);
    let mut unit = move || (rng.next() % 1_000_000) as f64 / 1e6;
    (0..n).map(|_| v2(HYP_RECT.min.x + HYP_RECT.width() * unit(), HYP_RECT.min.y + HYP_RECT.height() * unit())).collect()
}

pub fn population(m: &Model, ind: &[bool], eps: f64, truths: &[V2]) -> Population {
    let mut rng = Lcg(2024);
    let (mut free, mut amb, mut grid, mut rand, mut adap, mut one, mut unsolv, mut draws) = (0, 0, 0, 0, 0, 0, 0, 0);
    for &t in truths {
        let d = m.truth_devs(t);
        let c0 = m.after_free(&d, eps);
        free += m.ok(&c0, t) as usize;
        amb += (m.diameter(&c0) > m.spec.tol) as usize;
        grid += m.ok(&m.fixed_pattern(&d, eps), t) as usize;
        for _ in 0..5 {
            rand += m.ok(&m.random_scouts(&d, eps, &mut rng), t) as usize;
            draws += 1;
        }
        adap += m.ok(&m.adaptive(ind, &d, eps), t) as usize;
        one += m.ok(&m.adaptive_n(ind, &d, eps, 1), t) as usize;
        unsolv += (!m.oracle_solves(t, &d, eps)) as usize;
    }
    let n = truths.len() as f64;
    Population { n: truths.len(), free_only: free as f64 / n, ambiguous_after_free: amb as f64 / n, grid: grid as f64 / n, random: rand as f64 / draws as f64, adaptive: adap as f64 / n, adaptive_one: one as f64 / n, unsolvable: unsolv as f64 / n }
}

pub struct LevelCheck {
    pub pool: Vec<TruthReport>,
    pub coarse: Population,
    pub fine: Population,
}

pub fn check(level: &Level, random_truths: usize) -> LevelCheck {
    let m = Model::new(level);
    let (ind_c, ind_f) = (m.indist(EPS_COARSE), m.indist(EPS_FINE));
    let pool = m.spec.pool.iter().map(|&s| truth_report(&m, &ind_c, s)).collect();
    let truths = random_sources(random_truths, 77);
    LevelCheck { pool, coarse: population(&m, &ind_c, EPS_COARSE, &truths), fine: population(&m, &ind_f, EPS_FINE, &truths) }
}

/// Hard requirement: every hidden position the level can use is solvable within the scout budget at
/// the fine threshold (checked exhaustively over lattice scouts).
pub fn solvable(level: &Level) -> bool {
    let m = Model::new(level);
    m.spec.pool.iter().all(|&s| m.oracle_solves(s, &m.truth_devs(s), EPS_FINE))
}

pub fn summarize(level: &Level) -> String {
    let c = check(level, 60);
    let n = c.pool.len();
    let count = |f: &dyn Fn(&TruthReport) -> bool| c.pool.iter().filter(|t| f(t)).count();
    format!(
        "pool {}: solvable(fine) {}/{}, ambiguous after free scout {}/{}, adaptive(coarse) wins {}/{}; 60 random sources (coarse): free-only {:.0}% grid {:.0}% random {:.0}% adaptive {:.0}%",
        n,
        count(&|t| t.solvable_fine),
        n,
        count(&|t| t.free_diameter > 0.5),
        n,
        count(&|t| t.adaptive_coarse),
        n,
        100.0 * c.coarse.free_only,
        100.0 * c.coarse.grid,
        100.0 * c.coarse.random,
        100.0 * c.coarse.adaptive
    )
}

/// Markdown report written to `SURVEY_VALIDATION.md` by `cargo run --release --bin report -- --survey`.
pub fn report(levels: &[Level], random_truths: usize) -> String {
    let mut s = String::new();
    s.push_str("# Survey validation report\n\nGenerated by `cargo run --release --bin report -- --survey > SURVEY_VALIDATION.md`. See `docs/09-survey.md` for the method.\n");
    for l in levels {
        let c = check(l, random_truths);
        let spec = SurveySpec::from_level(l);
        s.push_str(&format!("\n## {} - {}\n\nscouts {}, trail {} s, source strength k = {}, width sigma = {}, commit tolerance {}, candidates on a {} grid, precision thresholds fine {} / coarse {}.\n\n", l.id, l.title, spec.budget, l.max_time, spec.k, spec.sigma, spec.tol, HYP_STEP, EPS_FINE, EPS_COARSE));
        s.push_str("### Authored hidden positions\n\n| source | candidates left after free scout | their spread | free scout alone solves | solvable (fine) | solvable (coarse) | scouts the adaptive player needs (coarse) | adaptive (coarse) | fixed pattern (coarse) | random scouts (coarse) |\n|---|---|---|---|---|---|---|---|---|---|\n");
        for t in &c.pool {
            let need = t.adaptive_needs_coarse.map_or("-".to_string(), |n| n.to_string());
            s.push_str(&format!("| ({}, {}) | {} | {:.1} | {} | {} | {} | {} | {} | {} | {:.0}% |\n", t.source.x, t.source.y, t.free_candidates, t.free_diameter, yn(t.free_solves), yn(t.solvable_fine), yn(t.solvable_coarse), need, yn(t.adaptive_coarse), yn(t.grid_coarse), 100.0 * t.random_coarse));
        }
        s.push_str(&format!("\n### {} random continuous sources\n\nSuccess = the centroid of the surviving candidates is within the commit tolerance. \"Unsolvable\" = no combination of {} lattice scouts achieves that.\n\n| precision | free scout alone | still ambiguous after it | fixed pattern | random scouts | adaptive (1 scout) | adaptive (full budget) | unsolvable |\n|---|---|---|---|---|---|---|---|\n", c.coarse.n, spec.budget));
        for (name, p) in [("fine", &c.fine), ("coarse", &c.coarse)] {
            s.push_str(&format!("| {} | {:.0}% | {:.0}% | {:.0}% | {:.0}% | {:.0}% | {:.0}% | {:.0}% |\n", name, 100.0 * p.free_only, 100.0 * p.ambiguous_after_free, 100.0 * p.grid, 100.0 * p.random, 100.0 * p.adaptive_one, 100.0 * p.adaptive, 100.0 * p.unsolvable));
        }
    }
    s
}

fn yn(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}
