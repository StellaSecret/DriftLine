//! SURVEY - "Where is the hidden source?"  Choose what to observe, then commit one estimate.
//!
//! The level's field is the *charted* background. One hidden local repeller (`Term::Gauss`, known
//! strength and width, unknown position) is added to it. The player gets one free scout, places a
//! limited number of further scouts anywhere in the zone, and finally marks where the source is.
//! A scout is a probe released at a chosen point that leaves a fixed-length trail under the *true*
//! field; nothing else about the hidden source is ever shown before the commit.
//!
//! State machine:
//!   Scouting   `round.verdict == None`: `place_scout` (while budget remains), `set_marker`, `commit`.
//!   Committed  `round.verdict == Some(_)`: frozen, the source is revealed; only `retry` is accepted.
//!   Retry      `retry()` starts the next round with a *different* hidden position (so a failed attempt
//!              cannot be repaired by remembering where the source was).
//!
//! `SurveySpec` is pure: `fly` is the only function that sees the hidden position and it is used by the
//! game to simulate a scout the player actually placed (and by the validators, which are not the game).
use crate::field::{Field, Term, V2};
use crate::level::{Level, Rules, Zone};
use crate::modes::Lcg;
use crate::sim::{simulate, SimConfig, Trajectory};

/// A new scout may not start closer than this to an earlier one (it would repeat that trail).
pub const MIN_SCOUT_SEPARATION: f64 = 0.05;

/// A scout only *shows* push at least this strong. This is the scout's sensing range, the knob that decides
/// whether scouting matters: with 0.12 the free scout alone located 4 of the 6 positions (80-88%, with a
/// realistic 12 degree reading error), so most of the level did not need a paid scout. With 0.2 the free scout
/// alone locates none; scouts placed anywhere reach 18-36% (one or two); scouts placed within 1.8 of the source
/// reach 90%. Where the scout goes decides the evidence. (Strength 1.6, sigma 1: 0.2 is felt out to about 2.)
pub const PUSH_MIN: f64 = 0.2;
/// Seconds between arrows along a trail.
pub const PUSH_EVERY: f64 = 0.3;
/// How many times per attempt the player may test an estimate (see `SurveyGame::test_estimate`). Few enough that
/// which estimate to test is a decision; enough to confirm or reject a couple of ideas. Resets on retry.
pub const TESTS_PER_ROUND: usize = 3;

#[derive(Clone, Debug)]
pub struct SurveySpec {
    /// The charted background (the level's field; it does not contain the hidden source).
    pub field: Field,
    pub cfg: SimConfig,
    pub zone: Zone,
    pub free: V2,
    pub budget: usize,
    pub k: f64,
    pub sigma: f64,
    pub tol: f64,
    /// Draw rays back along the push arrows (a teaching aid; see `Rules::Survey`).
    pub rays: bool,
    pub pool: Vec<V2>,
    pub seed: u64,
}

impl SurveySpec {
    pub fn from_level(l: &Level) -> SurveySpec {
        let Rules::Survey { zone, free, scouts, k, sigma, tol, rays, pool } = &l.rules else { panic!("not a Survey level") };
        assert!(!pool.is_empty(), "a Survey level needs at least one hidden position");
        SurveySpec { field: l.field.clone(), cfg: l.sim_config(), zone: zone.clone(), free: *free, budget: *scouts, k: *k, sigma: *sigma, tol: *tol, rays: *rays, pool: pool.clone(), seed: l.seed }
    }

    /// The charted field plus the hidden source at `source`.
    pub fn true_field(&self, source: V2) -> Field {
        self.field.clone().fixed(Term::Gauss { c: source, k: self.k, sigma: self.sigma })
    }

    /// A scout released at `start` under the true field. Deterministic.
    pub fn fly(&self, source: V2, start: V2) -> Trajectory {
        simulate(&self.true_field(source), start, 0.0, &[], &self.cfg)
    }

    /// The hidden position of attempt `attempt` (0-based). The first pass through the pool is in authored order
    /// (easy to hard: see `SURVEY_POOL`), so a new player meets the gentle positions first. Later passes are a
    /// seeded permutation of the pool per pass, so a long session cannot be won by remembering the order.
    pub fn source_for_attempt(&self, attempt: usize) -> V2 {
        let n = self.pool.len();
        let (pass, i) = (attempt / n, attempt % n);
        if pass == 0 {
            return self.pool[i];
        }
        let mut order: Vec<usize> = (0..n).collect();
        let mut rng = Lcg(self.seed ^ (pass as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for k in (1..n).rev() {
            order.swap(k, rng.below(k + 1));
        }
        // a pass never opens on the position the previous pass ended on
        if n > 1 && self.pool[order[0]] == self.source_for_attempt(pass * n - 1) {
            order.swap(0, 1);
        }
        self.pool[order[i]]
    }

    /// The extra push a scout felt, sampled every `every` seconds along its trail: (position, push), where push is
    /// the measured velocity (centred difference over +-0.1 s) minus the charted flow there, kept where it is at
    /// least `min` strong. It is the hidden source's own term, so it points straight away from the source, which
    /// is what a person can read: lines drawn back along these arrows meet at the source. It needs only the trail
    /// and the charted field (what the scout already shows), never the hidden position.
    ///
    /// Why this and not the end-point displacement: measured against an ideal fit, triangulating the end
    /// displacements locates the source to a median error of 1.99 (1% within 0.5), because a trail integrates
    /// the push along a curving path; the local push locates it exactly.
    pub fn push_samples(&self, traj: &Trajectory) -> Vec<(V2, V2)> {
        self.push_samples_at(traj, PUSH_EVERY, PUSH_MIN)
    }

    /// `push_samples` with explicit spacing and strength threshold (tests, experiments).
    pub fn push_samples_at(&self, traj: &Trajectory, every: f64, min: f64) -> Vec<(V2, V2)> {
        let (p, k, h) = (&traj.points, ((every / traj.dt).round() as usize).max(1), ((0.1 / traj.dt).round() as usize).max(1));
        let mut out = vec![];
        let mut i = h;
        while i + h < p.len() {
            let v = (p[i + h] - p[i - h]) * (1.0 / (2.0 * h as f64 * traj.dt));
            let push = v - self.field.eval(p[i], traj.t0 + i as f64 * traj.dt);
            if push.len() >= min {
                out.push((p[i], push));
            }
            i += k;
        }
        out
    }

    /// Rays for the reading aid: from every other push sample, the unit direction **back** along the push, i.e.
    /// toward the source. Extended far enough, rays from one trail (or several) cross at the source. Empty if the
    /// level has the aid off.
    pub fn push_rays(&self, traj: &Trajectory) -> Vec<(V2, V2)> {
        if !self.rays {
            return vec![];
        }
        self.push_samples_at(traj, 2.0 * PUSH_EVERY, PUSH_MIN).into_iter().map(|(p, push)| (p, push * (-1.0 / push.len()))).collect()
    }

    /// Where a scout released at `start` would fly under the *charted* field alone, with no hidden source.
    /// It depends only on what the player can already see, so it reveals nothing about the source; drawn next
    /// to the real trail it turns "this trail bent" into a displacement away from the source.
    pub fn fly_charted(&self, start: V2) -> Trajectory {
        simulate(&self.field, start, 0.0, &[], &self.cfg)
    }

    pub fn judge(&self, source: V2, marker: V2) -> Verdict {
        let distance = marker.dist(source);
        Verdict { distance, ok: distance <= self.tol }
    }
}

#[derive(Clone, Debug)]
pub struct Scout {
    pub start: V2,
    pub traj: Trajectory,
    /// The free scout flown at the start of every round.
    pub free: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Verdict {
    /// Distance from the committed marker to the true source.
    pub distance: f64,
    pub ok: bool,
}

#[derive(Clone, Debug)]
pub struct Round {
    pub attempt: usize,
    pub scouts: Vec<Scout>,
    pub marker: Option<V2>,
    pub verdict: Option<Verdict>,
    /// The last estimate the player tested, with the trails it predicted (see `SurveyGame::test_estimate`).
    pub test: Option<EstimateTest>,
    /// Tests spent this attempt.
    pub tests_used: usize,
    source: V2,
}

/// "If the source were at `at`, the scouts flown so far would have drawn `trails`": a hypothesis checked against the
/// scouts that existed when it was tested (trails line up with `Round::scouts`).
#[derive(Clone, Debug)]
pub struct EstimateTest {
    pub at: V2,
    pub trails: Vec<Trajectory>,
}
impl Round {
    fn new(spec: &SurveySpec, attempt: usize) -> Round {
        let source = spec.source_for_attempt(attempt);
        let free = Scout { start: spec.free, traj: spec.fly(source, spec.free), free: true };
        Round { attempt, scouts: vec![free], marker: None, verdict: None, test: None, tests_used: 0, source }
    }
    /// The hidden position, but only once the round is committed.
    pub fn revealed_source(&self) -> Option<V2> {
        self.verdict.map(|_| self.source)
    }
    /// Scouts the player paid for (everything except the free one).
    pub fn paid(&self) -> usize {
        self.scouts.iter().filter(|s| !s.free).count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceError {
    /// The round is already committed.
    Finished,
    NoScouts,
    OutOfZone,
    /// Too close to an earlier scout: it would repeat that trail.
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitError {
    Finished,
    NoMarker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestError {
    /// There is no marker to test.
    NoMarker,
    /// All tests for this attempt are spent.
    NoTestsLeft,
    /// The attempt is already committed.
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryError {
    /// The current round has not been committed: abandoning it would refund its scouts.
    NotFinished,
}

pub struct SurveyGame {
    pub level: Level,
    pub spec: SurveySpec,
    round: Round,
    /// Committed rounds, oldest first.
    pub history: Vec<Round>,
    pub won: bool,
}

impl SurveyGame {
    pub fn new(level: Level) -> SurveyGame {
        let spec = SurveySpec::from_level(&level);
        let round = Round::new(&spec, 0);
        SurveyGame { level, spec, round, history: vec![], won: false }
    }
    pub fn round(&self) -> &Round {
        &self.round
    }
    pub fn finished(&self) -> bool {
        self.round.verdict.is_some()
    }
    pub fn scouts_left(&self) -> usize {
        self.spec.budget - self.round.paid()
    }
    pub fn tests_left(&self) -> usize {
        TESTS_PER_ROUND - self.round.tests_used
    }

    /// Test the current marker: fly every scout placed so far again **as if the source were at the marker**, and
    /// keep the result (`Round::test`) next to the real trails. Costs one of `TESTS_PER_ROUND`; testing the same
    /// marker again with the same scouts is free (it would show the same thing). Returns whether a test was spent.
    ///
    /// A pure function of the marker and the scouts' start points: the hidden position is never read (tested), so
    /// it checks a guess against the evidence the scouts gave and adds none. The budget makes *which* estimate to
    /// test a decision instead of letting the player slide the marker until the dashes match.
    pub fn test_estimate(&mut self) -> Result<bool, TestError> {
        if self.finished() {
            return Err(TestError::Finished);
        }
        let m = self.round.marker.ok_or(TestError::NoMarker)?;
        if let Some(t) = &self.round.test {
            if t.at == m && t.trails.len() == self.round.scouts.len() {
                return Ok(false);
            }
        }
        if self.tests_left() == 0 {
            return Err(TestError::NoTestsLeft);
        }
        let trails = self.round.scouts.iter().map(|s| self.spec.fly(m, s.start)).collect();
        self.round.test = Some(EstimateTest { at: m, trails });
        self.round.tests_used += 1;
        Ok(true)
    }

    /// Rounds played so far, including the current one.
    pub fn attempts(&self) -> usize {
        self.history.len() + 1
    }

    /// Release a scout at `p`. It flies at once (its trail is shown in full) and costs one scout.
    pub fn place_scout(&mut self, p: V2) -> Result<&Scout, PlaceError> {
        if self.finished() {
            return Err(PlaceError::Finished);
        }
        if self.scouts_left() == 0 {
            return Err(PlaceError::NoScouts);
        }
        if !self.spec.zone.contains(p) {
            return Err(PlaceError::OutOfZone);
        }
        if self.round.scouts.iter().any(|s| s.start.dist(p) < MIN_SCOUT_SEPARATION) {
            return Err(PlaceError::Duplicate);
        }
        let traj = self.spec.fly(self.round.source, p);
        self.round.scouts.push(Scout { start: p, traj, free: false });
        Ok(self.round.scouts.last().unwrap())
    }

    /// Put (or move) the estimate marker. It is clamped into the zone and can be moved until commit.
    pub fn set_marker(&mut self, p: V2) -> Result<V2, CommitError> {
        if self.finished() {
            return Err(CommitError::Finished);
        }
        let p = self.spec.zone.clamp(p);
        self.round.marker = Some(p);
        Ok(p)
    }

    /// Lock the estimate in. The source is revealed and the round is frozen.
    pub fn commit(&mut self) -> Result<Verdict, CommitError> {
        if self.finished() {
            return Err(CommitError::Finished);
        }
        let marker = self.round.marker.ok_or(CommitError::NoMarker)?;
        let v = self.spec.judge(self.round.source, marker);
        self.round.verdict = Some(v);
        self.won |= v.ok;
        Ok(v)
    }

    /// Start the next round, with a different hidden position. Only after a commit.
    pub fn retry(&mut self) -> Result<(), RetryError> {
        if !self.finished() {
            return Err(RetryError::NotFinished);
        }
        let next = Round::new(&self.spec, self.round.attempt + 1);
        let done = std::mem::replace(&mut self.round, next);
        self.history.push(done);
        Ok(())
    }
}
