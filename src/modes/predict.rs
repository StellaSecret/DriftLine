//! PREDICT - "Where will it go?"  A committed forecast before the reveal.
//!
//! Anti parameter-search design: the player controls nothing except the answer. A level is
//! `rounds` independent rounds (different fixed release points); a wrong answer cannot be
//! "fixed" by retrying the same round, and a retry of the level draws fresh rounds.
use crate::field::{v2, V2};
use crate::level::{Level, Reveal, Rules, Zone};
use crate::modes::Lcg;
use crate::sim::{Outcome, Trajectory};

#[derive(Clone, Debug)]
pub struct Round {
    pub start: V2,
    /// Precomputed true path. The UI must not draw it before `revealed`.
    pub traj: Trajectory,
    /// Region index of the correct exit.
    pub answer: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct RoundResult {
    pub chosen: usize,
    pub correct: bool,
}

pub struct PredictGame {
    pub level: Level,
    pub attempt_no: u64,
    pub rounds: Vec<Round>,
    pub idx: usize,
    pub selected: Option<usize>,
    pub revealed: bool,
    pub results: Vec<RoundResult>,
}

/// Release points whose outcome is a clear exit and is stable under small perturbation.
pub fn candidate_pool(level: &Level, zone: &Zone) -> Vec<(V2, Trajectory)> {
    let f = &level.field;
    let mut out = vec![];
    for p in zone.grid(11) {
        let t = level.run(f, p, 0.0);
        let Outcome::Reached(e) = t.outcome else { continue };
        if t.duration() < 3.0 {
            continue;
        }
        let stable = [v2(0.15, 0.0), v2(-0.15, 0.0), v2(0.0, 0.15), v2(0.0, -0.15)]
            .iter()
            .all(|d| level.run(f, p + *d, 0.0).outcome == Outcome::Reached(e));
        if stable {
            out.push((p, t));
        }
    }
    out
}

pub fn pick_rounds(level: &Level, attempt: u64) -> Vec<Round> {
    let Rules::Predict { zone, rounds, .. } = &level.rules else { panic!("not a Predict level") };
    let pool = candidate_pool(level, zone);
    let mut rng = Lcg(level.seed.wrapping_mul(7919).wrapping_add(attempt * 104729 + 1));
    let mut chosen: Vec<Round> = vec![];
    let mut guard = 0;
    while chosen.len() < *rounds && !pool.is_empty() && guard < 500 {
        guard += 1;
        let (p, t) = &pool[rng.below(pool.len())];
        let Outcome::Reached(e) = t.outcome else { continue };
        let used = |r: &Round| r.answer == e;
        // Prefer exits not used yet; accept repeats only after many failed draws.
        if chosen.iter().any(used) && guard < 200 {
            continue;
        }
        if chosen.iter().any(|r| r.start.dist(*p) < 0.8) {
            continue;
        }
        chosen.push(Round { start: *p, traj: t.clone(), answer: e });
    }
    chosen
}

impl PredictGame {
    pub fn new(level: Level) -> Self {
        let rounds = pick_rounds(&level, 0);
        PredictGame { level, attempt_no: 0, rounds, idx: 0, selected: None, revealed: false, results: vec![] }
    }
    pub fn reveal_mode(&self) -> Reveal {
        match &self.level.rules {
            Rules::Predict { reveal, .. } => *reveal,
            _ => unreachable!(),
        }
    }
    pub fn pass_mark(&self) -> usize {
        match &self.level.rules {
            Rules::Predict { pass, .. } => *pass,
            _ => unreachable!(),
        }
    }
    pub fn round(&self) -> &Round {
        &self.rounds[self.idx]
    }
    /// Choose (or change) the forecast. Locked once committed.
    pub fn select(&mut self, exit_region: usize) -> bool {
        if self.revealed || !self.level.exits().contains(&exit_region) {
            return false;
        }
        self.selected = Some(exit_region);
        true
    }
    /// Commit the forecast and reveal the true path.
    pub fn commit(&mut self) -> Option<RoundResult> {
        if self.revealed {
            return None;
        }
        let chosen = self.selected?;
        let r = RoundResult { chosen, correct: chosen == self.round().answer };
        self.results.push(r);
        self.revealed = true;
        Some(r)
    }
    pub fn next_round(&mut self) -> bool {
        if !self.revealed || self.idx + 1 >= self.rounds.len() {
            return false;
        }
        self.idx += 1;
        self.selected = None;
        self.revealed = false;
        true
    }
    pub fn finished(&self) -> bool {
        self.revealed && self.idx + 1 >= self.rounds.len()
    }
    pub fn correct_count(&self) -> usize {
        self.results.iter().filter(|r| r.correct).count()
    }
    pub fn passed(&self) -> bool {
        self.finished() && self.correct_count() >= self.pass_mark()
    }
    /// New set of rounds (different release points) - a retry never repeats a question.
    pub fn restart(&mut self) {
        self.attempt_no += 1;
        self.rounds = pick_rounds(&self.level, self.attempt_no);
        self.idx = 0;
        self.selected = None;
        self.revealed = false;
        self.results.clear();
    }
}
