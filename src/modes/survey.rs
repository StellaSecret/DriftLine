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
use crate::field::{Field, SourceKind, V2};
use crate::level::{Level, Rules, Zone};
use crate::modes::Lcg;
use crate::sim::{simulate, SimConfig, Trajectory};

/// A new scout may not start closer than this to an earlier one (it would repeat that trail).
pub const MIN_SCOUT_SEPARATION: f64 = 0.05;

/// A scout only *shows* push at least this strong. This is the scout's sensing range, the knob that decides
/// whether scouting matters: with 0.12 the free scout alone located 4 of the 6 positions (80-88%, with a
/// realistic 12 degree reading error), so most of the level did not need a paid scout. With 0.2 the free scout
/// alone locates none; scouts placed anywhere reach 18-36% (one or two); scouts placed within 1.8 of the source
/// reach 90%. Where the scout goes decides the evidence. (Strength 1.6, sigma 1: a push of 0.2 is felt out to 2.43 from the source.)
pub const PUSH_MIN: f64 = 0.2;
/// Seconds between arrows along a trail.
pub const PUSH_EVERY: f64 = 0.3;
/// Seconds between the spots that clear the band, and how much smaller than the sensing radius the cleared discs
/// are (the measured push is a smoothed estimate, so a source right at the edge must not be cleared by mistake).
pub const CLEARED_EVERY: f64 = 0.2;
pub const CLEARED_MARGIN: f64 = 0.15;
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
    /// Shade the places a source would already have been felt (see `cleared`).
    pub band: bool,
    pub pool: Vec<V2>,
    /// What the source at `pool[i]` does (see `Rules::Survey`).
    pub kinds: Vec<SourceKind>,
    pub seed: u64,
}

impl SurveySpec {
    pub fn from_level(l: &Level) -> SurveySpec {
        let Rules::Survey { zone, free, scouts, k, sigma, tol, rays, band, pool, kinds } = &l.rules else { panic!("not a Survey level") };
        assert!(!pool.is_empty(), "a Survey level needs at least one hidden position");
        assert_eq!(pool.len(), kinds.len(), "one kind per hidden position");
        SurveySpec { field: l.field.clone(), cfg: l.sim_config(), zone: zone.clone(), free: *free, budget: *scouts, k: *k, sigma: *sigma, tol: *tol, rays: *rays, band: *band, pool: pool.clone(), kinds: kinds.clone(), seed: l.seed }
    }

    /// The charted field plus the hidden source at `source`.
    pub fn true_field(&self, source: V2) -> Field {
        self.true_field_kind(source, SourceKind::Repeller)
    }

    /// The charted field plus a hidden source of `kind` at `source`.
    pub fn true_field_kind(&self, source: V2, kind: SourceKind) -> Field {
        self.field.clone().fixed(kind.term(source, self.k, self.sigma))
    }

    /// A scout released at `start` under the true field (a repeller at `source`). Deterministic.
    pub fn fly(&self, source: V2, start: V2) -> Trajectory {
        self.fly_kind(source, SourceKind::Repeller, start)
    }

    /// A scout released at `start` with a source of `kind` at `source`.
    pub fn fly_kind(&self, source: V2, kind: SourceKind, start: V2) -> Trajectory {
        simulate(&self.true_field_kind(source, kind), start, 0.0, &[], &self.cfg)
    }

    /// Does this level ask the player to tell kinds of source apart (some attempt is not a repeller)?
    pub fn typed(&self) -> bool {
        self.kinds.iter().any(|k| *k != SourceKind::Repeller)
    }

    /// The hidden position of attempt `attempt` (0-based). The first pass through the pool is in authored order
    /// (easy to hard: see `SURVEY_POOL`), so a new player meets the gentle positions first. Later passes are a
    /// seeded permutation of the pool per pass, so a long session cannot be won by remembering the order.
    pub fn index_for_attempt(&self, attempt: usize) -> usize {
        let n = self.pool.len();
        let (pass, i) = (attempt / n, attempt % n);
        if pass == 0 {
            return i;
        }
        let mut order: Vec<usize> = (0..n).collect();
        let mut rng = Lcg(self.seed ^ (pass as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for k in (1..n).rev() {
            order.swap(k, rng.below(k + 1));
        }
        // a pass never opens on the position the previous pass ended on
        if n > 1 && order[0] == self.index_for_attempt(pass * n - 1) {
            order.swap(0, 1);
        }
        order[i]
    }

    /// The hidden position of attempt `attempt` (see `index_for_attempt`).
    pub fn source_for_attempt(&self, attempt: usize) -> V2 {
        self.pool[self.index_for_attempt(attempt)]
    }

    /// What the hidden source of attempt `attempt` does.
    pub fn kind_for_attempt(&self, attempt: usize) -> SourceKind {
        self.kinds[self.index_for_attempt(attempt)]
    }

    /// The extra push a scout felt, sampled every `every` seconds along its trail: (position, push), where push is
    /// the measured velocity (centred difference over +-0.1 s) minus the charted flow there, kept where it is at
    /// least `min` strong. It is the hidden source's own term: for a repeller it points straight away from the source
    /// (lines drawn back along the arrows meet at it), for an attractor straight toward it (same lines), for a swirl
    /// at right angles to it (lines at right angles to the arrows meet at its centre). It needs only the trail
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
        self.push_all(traj, every).into_iter().filter(|(_, push)| push.len() >= min).collect()
    }

    /// Every sample of the measured push along a trail, strong or not (see `push_samples`).
    pub fn push_all(&self, traj: &Trajectory, every: f64) -> Vec<(V2, V2)> {
        let (p, k, h) = (&traj.points, ((every / traj.dt).round() as usize).max(1), ((0.1 / traj.dt).round() as usize).max(1));
        let mut out = vec![];
        let mut i = h;
        while i + h < p.len() {
            let v = (p[i + h] - p[i - h]) * (1.0 / (2.0 * h as f64 * traj.dt));
            out.push((p[i], v - self.field.eval(p[i], traj.t0 + i as f64 * traj.dt)));
            i += k;
        }
        out
    }

    /// The distance from the source at which its push falls to `PUSH_MIN` (the outer root of k d e^(-d^2/2sigma^2)):
    /// beyond it a scout shows nothing. 0 if the source is too weak to ever reach `PUSH_MIN`.
    pub fn sense_radius(&self) -> f64 {
        let push = |d: f64| self.k * d * (-d * d / (2.0 * self.sigma * self.sigma)).exp();
        if push(self.sigma) < PUSH_MIN {
            return 0.0;
        }
        let (mut lo, mut hi) = (self.sigma, 12.0 * self.sigma);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if push(mid) >= PUSH_MIN { lo = mid } else { hi = mid }
        }
        lo
    }

    /// The cleared band of a trail: `(centres, radius)`. Wherever the scout felt *less* than `PUSH_MIN` (and so did
    /// the spots just before and after it), a source within `radius` (the sensing radius, less a margin for the
    /// velocity estimate) of that spot would have pushed it harder, so the source is not there. The band is the
    /// union of those discs. It depends only on the trail and the known strength and width, never on the hidden
    /// position, and (tested) never contains it.
    ///
    /// The neighbours matter because the push also vanishes at the source's own centre: a scout that passes straight
    /// through it feels almost nothing right there, but the spots on either side felt plenty.
    pub fn cleared(&self, traj: &Trajectory) -> (Vec<V2>, f64) {
        let all = self.push_all(traj, CLEARED_EVERY);
        let quiet = |i: usize| all[i].1.len() < PUSH_MIN;
        let centres = (0..all.len())
            .filter(|&i| quiet(i) && (i == 0 || quiet(i - 1)) && (i + 1 == all.len() || quiet(i + 1)))
            .map(|i| all[i].0)
            .collect();
        (centres, (self.sense_radius() - CLEARED_MARGIN).max(0.0))
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

    /// Position only (kind not considered).
    pub fn judge(&self, source: V2, marker: V2) -> Verdict {
        self.judge_typed(source, SourceKind::Repeller, marker, SourceKind::Repeller)
    }

    /// Position and kind: found only if the marker is within `tol` of the source *and* names the right kind.
    pub fn judge_typed(&self, source: V2, kind: SourceKind, marker: V2, marker_kind: SourceKind) -> Verdict {
        let distance = marker.dist(source);
        let kind_ok = kind == marker_kind;
        Verdict { distance, kind_ok, ok: distance <= self.tol && kind_ok }
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
    /// The marker named the right kind of source (always true in a level with only repellers).
    pub kind_ok: bool,
    /// Found: close enough, and the right kind.
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
    /// The kind the player has given the marker (the selector in a typed level; Repeller otherwise).
    pub marker_kind: SourceKind,
    /// Which entry of the pool this round's hidden source is.
    pub index: usize,
    source: V2,
    kind: SourceKind,
}

/// "If the source were at `at`, the scouts flown so far would have drawn `trails`": a hypothesis checked against the
/// scouts that existed when it was tested (trails line up with `Round::scouts`).
#[derive(Clone, Debug)]
pub struct EstimateTest {
    pub at: V2,
    /// The kind of source the test assumed.
    pub kind: SourceKind,
    pub trails: Vec<Trajectory>,
}
impl Round {
    fn new(spec: &SurveySpec, attempt: usize, index: usize) -> Round {
        let source = spec.pool[index];
        let kind = spec.kinds[index];
        let free = Scout { start: spec.free, traj: spec.fly_kind(source, kind, spec.free), free: true };
        Round { attempt, scouts: vec![free], marker: None, verdict: None, test: None, tests_used: 0, marker_kind: SourceKind::Repeller, index, source, kind }
    }
    /// The hidden source and its kind. For tests, validators and tools only: the interface must go through
    /// `revealed_source` / `revealed_kind`, which wait for the commit.
    pub fn truth(&self) -> (V2, SourceKind) {
        (self.source, self.kind)
    }
    /// What the hidden source did, but only once the round is committed.
    pub fn revealed_kind(&self) -> Option<SourceKind> {
        self.verdict.map(|_| self.kind)
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
    /// Which families of source (see `SourceKind::family`) have been found at least once.
    pub found: [bool; 3],
}

/// Which pool entry the next attempt hides. In a level with one kind of source it is `index_for_attempt` (authored order
/// first, then reshuffled passes). In a level with several kinds, the kinds are what the player has to meet, so the next
/// attempt is of the first kind (repeller, attractor, swirl) still not found, taking that kind's positions in authored
/// order and starting over when it has none left; once every kind is found it goes back to `index_for_attempt`.
/// `seen` is the pool index of every earlier attempt, oldest first.
fn pick_index(spec: &SurveySpec, seen: &[usize], found: &[bool; 3], attempt: usize) -> usize {
    if spec.typed() {
        if let Some(fam) = (0..3).find(|f| !found[*f] && spec.kinds.iter().any(|k| k.family() == *f)) {
            let of_family: Vec<usize> = (0..spec.pool.len()).filter(|&i| spec.kinds[i].family() == fam).collect();
            let tried = seen.iter().filter(|&&i| spec.kinds[i].family() == fam).count();
            return of_family[tried % of_family.len()];
        }
    }
    spec.index_for_attempt(attempt)
}

impl SurveyGame {
    pub fn new(level: Level) -> SurveyGame {
        let spec = SurveySpec::from_level(&level);
        let round = Round::new(&spec, 0, pick_index(&spec, &[], &[false; 3], 0));
        SurveyGame { level, spec, round, history: vec![], won: false, found: [false; 3] }
    }

    /// The families this level asks for: those that occur among its sources.
    pub fn families_needed(&self) -> Vec<usize> {
        (0..3).filter(|f| self.spec.kinds.iter().any(|k| k.family() == *f)).collect()
    }

    /// Is the level done? A level with several kinds of source is done when each kind has been found at least once;
    /// any other level, when one attempt has been found.
    pub fn complete(&self) -> bool {
        if self.spec.typed() {
            self.families_needed().iter().all(|f| self.found[*f])
        } else {
            self.found.iter().any(|f| *f)
        }
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
            if t.at == m && t.kind == self.round.marker_kind && t.trails.len() == self.round.scouts.len() {
                return Ok(false);
            }
        }
        if self.tests_left() == 0 {
            return Err(TestError::NoTestsLeft);
        }
        let kind = self.round.marker_kind;
        let trails = self.round.scouts.iter().map(|s| self.spec.fly_kind(m, kind, s.start)).collect();
        self.round.test = Some(EstimateTest { at: m, kind, trails });
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
        let traj = self.spec.fly_kind(self.round.source, self.round.kind, p);
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

    /// Say what kind of source the marker stands for (the selector in a typed level). Free until the commit.
    pub fn set_marker_kind(&mut self, kind: SourceKind) -> Result<(), CommitError> {
        if self.finished() {
            return Err(CommitError::Finished);
        }
        self.round.marker_kind = kind;
        Ok(())
    }

    /// Lock the estimate in. The source is revealed and the round is frozen.
    pub fn commit(&mut self) -> Result<Verdict, CommitError> {
        if self.finished() {
            return Err(CommitError::Finished);
        }
        let marker = self.round.marker.ok_or(CommitError::NoMarker)?;
        let v = self.spec.judge_typed(self.round.source, self.round.kind, marker, self.round.marker_kind);
        self.round.verdict = Some(v);
        if v.ok {
            self.found[self.round.kind.family()] = true;
        }
        self.won = self.complete();
        Ok(v)
    }

    /// Start the next round, with a different hidden position. Only after a commit.
    pub fn retry(&mut self) -> Result<(), RetryError> {
        if !self.finished() {
            return Err(RetryError::NotFinished);
        }
        let attempt = self.round.attempt + 1;
        let mut seen: Vec<usize> = self.history.iter().map(|r| r.index).collect();
        seen.push(self.round.index);
        let next = Round::new(&self.spec, attempt, pick_index(&self.spec, &seen, &self.found, attempt));
        let done = std::mem::replace(&mut self.round, next);
        self.history.push(done);
        Ok(())
    }
}
