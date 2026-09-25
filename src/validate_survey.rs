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
//!
//! **Two models live here.** The one above compares whole trails (an ideal solver) and was used to choose level 1's
//! first six positions; it knows only a repeller and is the *legacy* model. What the player actually has is the push
//! arrows drawn on each trail, read by eye with some error, and the **arrow-reading model** at the end of this file
//! (`read_kind`, `read_rate`, `arrow_report`) measures that: it is the one the tests of the later levels use and the
//! one `SURVEY_VALIDATION.md` leads with.
use crate::field::{v2, SourceKind, V2};
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
        if SurveySpec::from_level(l).typed() {
            // This model compares trails under a hidden *repeller*. For attractors and swirls its numbers would
            // describe a different game, so none are printed.
            s.push_str(&format!("\n## {} - {}\n\nNot validated here: this level hides attractors and swirls as well as repellers, which the trail-similarity model below does not represent. Its checks are in `tests/survey.rs` (`level_two_*`) and described in `docs/09-survey.md`.\n", l.id, l.title));
            continue;
        }
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


// ---------------------------------------------------------------------------------------------------------------
// The arrow-reading model: what a player can do with the arrows on screen.
// ---------------------------------------------------------------------------------------------------------------

/// Reading error of a careful reader: `(bias per trail, jitter per arrow)` in degrees.
pub const CAREFUL: (f64, f64) = (8.0, 6.0);
/// Reading error of a player extending the arrows by eye, with no guide lines.
pub const BY_EYE: (f64, f64) = (12.0, 10.0);

/// The point nearest to a set of weighted lines `(anchor, unit direction, weight)`, or `None` if the lines run so nearly
/// parallel that no crossing could be found by eye: the smaller eigenvalue of their 2x2 normal matrix must be at least 1%
/// of the larger, i.e. the lines must differ in direction by about 11 degrees or more. Scale-free (independent of the
/// weights).
pub fn nearest_to_lines(lines: &[(V2, V2, f64)]) -> Option<V2> {
    let (mut a, mut b, mut c, mut e, mut f) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(p, d, w) in lines {
        let (nxx, nxy, nyy) = (1.0 - d.x * d.x, -d.x * d.y, 1.0 - d.y * d.y);
        a += w * nxx;
        b += w * nxy;
        c += w * nyy;
        e += w * (nxx * p.x + nxy * p.y);
        f += w * (nxy * p.x + nyy * p.y);
    }
    let (tr, disc) = (a + c, ((a - c) * (a - c) + 4.0 * b * b).sqrt());
    let (lmax, lmin) = ((tr + disc) / 2.0, (tr - disc) / 2.0);
    if lmax <= 0.0 || lmin < 0.01 * lmax {
        return None;
    }
    let det = a * c - b * b;
    Some(v2((c * e - b * f) / det, (a * f - b * e) / det))
}

/// `nearest_to_lines` and how badly the lines miss it: the weighted mean squared distance from the point to the lines.
pub fn fit_lines(lines: &[(V2, V2, f64)]) -> Option<(V2, f64)> {
    let est = nearest_to_lines(lines)?;
    let w: f64 = lines.iter().map(|l| l.2).sum();
    let r: f64 = lines
        .iter()
        .map(|&(p, d, wt)| {
            let q = est - p;
            let off = q.x * d.y - q.y * d.x;
            wt * off * off
        })
        .sum();
    Some((est, r / w))
}

/// Decide kind and place from arrows `(position, push)` the way a careful reader does. If lines along the arrows cross,
/// it is a repeller or attractor (a repeller if the arrows point away from the crossing, an attractor if toward it); if
/// lines at right angles cross, a swirl (counter-clockwise or clockwise by the sense of the arrows about the crossing).
/// Whichever family of lines fits better wins.
pub fn read_kind(arrows: &[(V2, V2)]) -> Option<(SourceKind, V2)> {
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

/// How often the kind **and** the place read from the given scouts' arrows are right (kind named correctly, place within
/// `tol` of `src`) when the arrows are read with `(bias, jitter)` degrees of error (bias drawn once per trail, jitter per
/// arrow). Needs at least 3 arrows in all. Deterministic for a given `seed`.
pub fn read_rate(tol: f64, src: V2, kind: SourceKind, scouts: &[&Vec<(V2, V2)>], (bias_deg, jitter_deg): (f64, f64), seed: u64, trials: usize) -> f64 {
    let mut rng = crate::modes::Lcg(seed);
    let mut gauss = || {
        let u1 = (rng.below(1_000_000) as f64 + 1.0) / 1_000_001.0;
        let u2 = rng.below(1_000_000) as f64 / 1e6;
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    };
    let mut hit = 0;
    for _ in 0..trials {
        let mut read = vec![];
        for arrows in scouts {
            let bias = gauss() * bias_deg.to_radians();
            for &(p, u) in arrows.iter() {
                let a = u.y.atan2(u.x) + bias + gauss() * jitter_deg.to_radians();
                read.push((p, v2(a.cos(), a.sin()) * u.len()));
            }
        }
        if read.len() >= 3 && read_kind(&read).map_or(false, |(k, e)| k == kind && e.dist(src) <= tol) {
            hit += 1;
        }
    }
    hit as f64 / trials as f64
}

/// The arrow-reading report for Survey levels (Markdown): for each authored attempt, what the free scout shows and how
/// often a reader gets kind and place right with the free scout alone, with scouts released 1 or 2 off its line, and
/// with every pair of lattice scouts (the average is blind placement, the maximum is the best placement); and whether any
/// two positions of a level are close enough that an earlier answer could be reused.
pub fn arrow_report(levels: &[Level]) -> String {
    let mut s = String::new();
    s.push_str("## Arrow-reading model (what the player sees)\n\nThe player does not compare whole trails: they read the small push arrows drawn on each trail, and extend them by eye. This model reads those arrows with a **careful** error (8 degrees of bias per trail, 6 of jitter per arrow) or **by eye** (12 and 10), refuses lines that run within about 11 degrees of parallel, and counts a read as right only if the *kind* is right and the place is within the commit tolerance. \"Lateral\" scouts are released 3 units along the free scout's line, 1 (or 2) above and below it. \"Blind\" is the average over every pair of scouts on an 11 x 7 lattice, \"best\" the maximum. Source: `validate_survey::arrow_report`; the same helpers are used by `tests/survey.rs`.\n");
    for l in levels {
        let sp = SurveySpec::from_level(l);
        if sp.paired() {
            s.push_str(&format!("\n### {} - {}\n\nNot read with this model: two sources are hidden at once and the player compares whole trails with the dashes of a pair of markers rather than extending arrows. See the pair-identifiability section below.\n", l.id, l.title));
            continue;
        }
        s.push_str(&format!("\n### {} - {}\n\nsigma {}, strength {}, commit tolerance {}, felt out to {:.2}; rays {}, cleared band {}, kinds {}; goal {:?}.\n\n", l.id, l.title, sp.sigma, sp.k, sp.tol, sp.sense_radius(), if sp.rays { "on" } else { "off" }, if sp.band { "on" } else { "off" }, if sp.typed() { "mixed" } else { "repeller only" }, sp.goal));
        let mut closest = (f64::MAX, 0, 0);
        for i in 0..sp.pool.len() {
            for j in i + 1..sp.pool.len() {
                let d = sp.pool[i].dist(sp.pool[j]);
                if d < closest.0 {
                    closest = (d, i, j);
                }
            }
        }
        let verdict = if closest.0 > 2.0 * sp.tol { "no attempt can be passed by re-marking an earlier answer" } else { "**some attempt may be passable from memory of an earlier answer** (kinds can still differ)" };
        s.push_str(&format!("Closest pair of positions: {:.2} (attempts {} and {}); twice the tolerance is {:.2}: {}.\n\n", closest.0, closest.1 + 1, closest.2 + 1, 2.0 * sp.tol, verdict));
        s.push_str("| # | kind | position | free scout shows | free alone (careful / by eye) | lateral 1 | lateral 2 | blind (careful / by eye) | best pair |\n|---|---|---|---|---|---|---|---|---|\n");
        let mut pts = vec![];
        for x in -5..=5 {
            for y in -3..=3 {
                pts.push(v2(x as f64, y as f64));
            }
        }
        for i in 0..sp.pool.len() {
            let (src, kind) = (sp.pool[i], sp.kinds[i]);
            let arr = |st: V2| sp.push_samples(&sp.fly_kind(src, kind, st));
            let free = arr(sp.free);
            let (fx, fy) = (sp.free.x + 3.0, sp.free.y);
            let lat = |h: f64| read_rate(sp.tol, src, kind, &[&free, &arr(v2(fx, fy + h)), &arr(v2(fx, fy - h))], CAREFUL, 8 + i as u64, 200);
            let free_c = read_rate(sp.tol, src, kind, &[&free], CAREFUL, 1 + i as u64, 200);
            let free_e = read_rate(sp.tol, src, kind, &[&free], BY_EYE, 1 + i as u64, 200);
            let cached: Vec<Vec<(V2, V2)>> = pts.iter().map(|&p| arr(p)).collect();
            let (mut care, mut eye, mut best, mut n) = (0.0, 0.0, 0.0f64, 0.0);
            for a in 0..pts.len() {
                for b in a + 1..pts.len() {
                    let c = read_rate(sp.tol, src, kind, &[&free, &cached[a], &cached[b]], CAREFUL, (i * 100_000 + a * 97 + b) as u64, 4);
                    care += c;
                    best = best.max(c);
                    eye += read_rate(sp.tol, src, kind, &[&free, &cached[a], &cached[b]], BY_EYE, (i * 7_777 + a * 31 + b) as u64, 3);
                    n += 1.0;
                }
            }
            let shows = match free.len() {
                0 => "nothing".to_string(),
                1 => "1 arrow".to_string(),
                k => format!("{k} arrows"),
            };
            s.push_str(&format!("| {} | {} | ({:.2}, {:.2}) | {} | {:.0}% / {:.0}% | {:.0}% | {:.0}% | {:.0}% / {:.0}% | {:.0}% |\n", i + 1, kind.short(), src.x, src.y, shows, 100.0 * free_c, 100.0 * free_e, 100.0 * lat(1.0), 100.0 * lat(2.0), 100.0 * care / n, 100.0 * eye / n, 100.0 * best));
        }
    }
    s
}

/// Distance between two pairs of points when matched in the better assignment (the larger of the two matched distances).
fn pair_distance(a: [V2; 2], b: [V2; 2]) -> f64 {
    let straight = a[0].dist(b[0]).max(a[1].dist(b[1]));
    let swapped = a[0].dist(b[1]).max(a[1].dist(b[0]));
    straight.min(swapped)
}

fn trail_deviation(a: &Trajectory, b: &Trajectory) -> f64 {
    a.points.iter().zip(&b.points).map(|(p, q)| p.dist(*q)).fold(0.0, f64::max)
}

/// Identifiability for levels that hide two sources (Markdown). A hypothesis is a pair of points on a 1-unit lattice
/// (both on one point allowed); it is *consistent* with a set of scouts when every scout's trail under it stays within
/// 0.35 of the real trail (a gap of that size is visible as dashes against the solid line). For each attempt:
/// what the free scout alone leaves open; whether a naive set (the free scout and scouts released to either side of the
/// pair) still admits the *ghost*, both markers on the midpoint; and the best three paid scouts out of a pool of
/// candidates (chosen, among the sets that rule out the ghost and every near-ghost, a lattice pair with its markers within
/// 1.0 of each other, to leave the smallest worst-case error among consistent
/// hypotheses), with what is left and whether the ghost survives (it cannot, by construction, if a set is found). The lattice is coarse (the real positions are off it), so worst errors under about 1.0
/// are lattice resolution rather than ambiguity.
pub fn pair_report(levels: &[Level]) -> String {
    let (eps, kind) = (0.35, SourceKind::Repeller);
    let mut s = String::new();
    for l in levels {
        let sp = SurveySpec::from_level(l);
        if !sp.paired() {
            continue;
        }
        s.push_str(&format!("\n### {} - {}\n\nsigma {}, strength {}, commit tolerance {}, {} paid scouts; goal {:?}.\n\n", l.id, l.title, sp.sigma, sp.k, sp.tol, 3, sp.goal));
        let pairs: Vec<[V2; 2]> = (0..sp.pool.len()).map(|i| [sp.pool[i], sp.partners[i].unwrap()]).collect();
        let mut closest = (f64::MAX, 0, 0);
        for i in 0..pairs.len() {
            for j in i + 1..pairs.len() {
                let d = pair_distance(pairs[i], pairs[j]);
                if d < closest.0 {
                    closest = (d, i, j);
                }
            }
        }
        s.push_str(&format!("Closest two attempts: {:.2} (attempts {} and {}); twice the tolerance is {:.2}: {}.\n\n", closest.0, closest.1 + 1, closest.2 + 1, 2.0 * sp.tol, if closest.0 > 2.0 * sp.tol { "no marker pair can be reused for another attempt" } else { "**a marker pair may be reusable**" }));
        s.push_str("| # | separation | free scout alone (pairs left / worst error) | naive scouts admit the ghost | best three paid scouts | pairs left / worst error | ghost survives |\n|---|---|---|---|---|---|---|\n");
        let mut lattice = vec![];
        for x in -5..=5 {
            for y in -3..=3 {
                lattice.push(v2(x as f64, y as f64));
            }
        }
        let hyps: Vec<[V2; 2]> = (0..lattice.len()).flat_map(|i| (i..lattice.len()).map(move |j| (i, j))).map(|(i, j)| [lattice[i], lattice[j]]).collect();
        for (i, &truth) in pairs.iter().enumerate() {
            let mid = (truth[0] + truth[1]) * 0.5;
            let ghost = [mid, mid];
            let fly = |h: &[V2; 2], st: V2| sp.fly_sources(h, kind, st);
            let observed = |st: V2| fly(&truth, st);
            let consistent_with = |h: &[V2; 2], starts: &[V2]| starts.iter().all(|&st| trail_deviation(&observed(st), &fly(h, st)) <= eps);
            // free scout alone
            let (mut n_free, mut worst_free) = (0, 0.0f64);
            let free_obs = observed(sp.free);
            for h in &hyps {
                if trail_deviation(&free_obs, &fly(h, sp.free)) <= eps {
                    n_free += 1;
                    worst_free = worst_free.max(pair_distance(truth, *h));
                }
            }
            // naive: free scout and scouts to either side of the pair
            let naive: Vec<V2> = std::iter::once(sp.free).chain([1.6, -1.6, 2.4, -2.4].iter().map(|dy| v2(-2.0, mid.y + dy)).filter(|p| p.y.abs() <= 3.4)).collect();
            let ghost_naive = consistent_with(&ghost, &naive);
            // best three paid scouts from a pool
            let mut pool: Vec<V2> = vec![];
            for x in [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0] {
                for dy in [-2.0, -1.0, 0.0, 1.0, 2.0] {
                    let p = v2(x, (mid.y + dy).clamp(-3.4, 3.4));
                    if sp.zone.contains(p) && p.dist(sp.free) > 0.5 && !pool.iter().any(|q| q.dist(p) < 0.3) {
                        pool.push(p);
                    }
                }
            }
            let ok: Vec<Vec<bool>> = std::iter::once(sp.free)
                .chain(pool.iter().copied())
                .map(|st| {
                    let o = observed(st);
                    hyps.iter().map(|h| trail_deviation(&o, &fly(h, st)) <= eps).collect()
                })
                .collect();
            // does the ghost (not on the lattice) agree with each candidate scout? A set must rule it out.
            let ghost_ok: Vec<bool> = std::iter::once(sp.free).chain(pool.iter().copied()).map(|st| trail_deviation(&observed(st), &fly(&ghost, st)) <= eps).collect();
            // a "near ghost" is a pair of lattice points within 1.0 of each other: to a player it is the same mistake
            let stacky: Vec<bool> = hyps.iter().map(|h| h[0].dist(h[1]) <= 1.0).collect();
            let mut best: Option<(f64, usize, [usize; 3])> = None;
            for a in 0..pool.len() {
                for b in a + 1..pool.len() {
                    for c in b + 1..pool.len() {
                        if ghost_ok[0] && ghost_ok[a + 1] && ghost_ok[b + 1] && ghost_ok[c + 1] {
                            continue;
                        }
                        let (mut count, mut worst, mut near_ghost) = (0usize, 0.0f64, false);
                        for (h, hy) in hyps.iter().enumerate() {
                            if ok[0][h] && ok[a + 1][h] && ok[b + 1][h] && ok[c + 1][h] {
                                count += 1;
                                worst = worst.max(pair_distance(truth, *hy));
                                near_ghost |= stacky[h];
                            }
                        }
                        if near_ghost {
                            continue;
                        }
                        if count > 0 && best.map_or(true, |(w, n, _)| worst < w || (worst == w && count < n)) {
                            best = Some((worst, count, [a, b, c]));
                        }
                    }
                }
            }
            let (worst, count, trio) = best.unwrap_or((f64::NAN, 0, [0, 0, 0]));
            let starts: Vec<V2> = std::iter::once(sp.free).chain(trio.iter().map(|&k| pool[k])).collect();
            let ghost_best = consistent_with(&ghost, &starts);
            let sep = truth[0].dist(truth[1]);
            let trio_txt = trio.iter().map(|&k| format!("({:.1}, {:.1})", pool[k].x, pool[k].y)).collect::<Vec<_>>().join(" ");
            s.push_str(&format!("| {} | {:.1} | {} / {:.1} | {} | {} | {} / {:.1} | {} |\n", i + 1, sep, n_free, worst_free, if ghost_naive { "yes" } else { "no" }, trio_txt, count, worst, if ghost_best { "**yes**" } else { "no" }));
        }
    }
    s
}

/// The whole `SURVEY_VALIDATION.md`: the arrow-reading report for the current levels, then the legacy trail-similarity
/// report for the positions level 1 first shipped with.
pub fn full_report(current: &[Level], legacy: &[Level], random_truths: usize) -> String {
    let mut s = String::from("# Survey validation report\n\nGenerated by `cargo run --release --bin report -- --survey > SURVEY_VALIDATION.md`. See `docs/09-survey.md` for the method.\n\n");
    s.push_str(&arrow_report(current));
    if current.iter().any(|l| SurveySpec::from_level(l).paired()) {
        s.push_str("\n## Pair identifiability (hypothesis search)\n\nFor levels with two hidden sources. See `validate_survey::pair_report` for the method.\n");
        s.push_str(&pair_report(current));
    }
    let old = report(legacy, random_truths).replacen("# Survey validation report", "", 1);
    let old = old.replace("\n### ", "\n#### ").replace("\n## ", "\n### ");
    s.push_str("\n## Legacy: trail-similarity model\n\nThe first validator. It compares *whole trails* as an ideal solver would, knows only a repeller, and was used to choose level 1's first six positions, which are the ones listed here (**not** the level's current positions: those were changed so that the free scout shows arrows at once, which this model, by construction, reads as \"the free scout settles it\"). It is kept so that the model itself stays tested, and is not evidence about the current levels.\n");
    s.push_str(&old);
    s
}
