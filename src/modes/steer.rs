//! STEER - "What should I do next?"  Closed-loop, turn-based control.
//!
//! After every beat the simulation pauses and the player picks a *gust* (a one-beat velocity
//! contribution in one of the permitted directions) or waits. The probe obeys
//!
//! ```text
//! dp/dt = v(p) + u,   u = gust vector during the current beat, 0 otherwise,
//! ```
//!
//! integrated by the same RK4 sub-step (`sim::substep`) as every other mode. A gust never moves the
//! probe directly and never persists past its beat.
//!
//! State machine (spec section 3) and where each state lives:
//!   Ready            `Phase::Ready`: initial state, nothing chosen yet.
//!   AwaitingDecision `Phase::Awaiting`: probe stationary, input accepted.
//!   Advancing        `Phase::Advancing`: `choose` was accepted; input is rejected until `finish_beat`.
//!   Evaluating       resolved atomically inside `finish_beat` (success / bounds / validity / beats).
//!   Completed        `Phase::Completed(End)`: frozen.
//!   Retry            `retry()` restores the exact initial state and keeps the finished run as a ghost.
use crate::field::{Field, V2};
use crate::level::{GustDirs, Level, Rules};
use crate::sim::{advance_beat, Outcome, Region, SimConfig};

/// Number of earlier attempts kept as faint ghosts (the current run is never truncated).
pub const MAX_GHOSTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Wait,
    /// Gust in direction index `i` of the level's `GustDirs`.
    Gust(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailReason {
    Boundary,
    BeatsExhausted,
    Invalid,
    Hazard,
    MissedGate,
    WrongRegion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Success,
    Failed(FailReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChooseError {
    /// A beat is being advanced (or the run is not accepting input): duplicate input is rejected.
    Busy,
    Finished,
    NoBudget,
    BadDirection,
}

/// Everything about a run that is needed to continue it. Cheap to clone (used by exhaustive search).
#[derive(Clone, Debug)]
pub struct State {
    pub pos: V2,
    pub beat: usize,
    pub gusts_left: usize,
    pub gates: Vec<usize>,
    pub end: Option<End>,
}

/// One resolved beat.
#[derive(Clone, Debug)]
pub struct Beat {
    pub action: Action,
    pub from: V2,
    /// Natural field velocity at `from` (what the player saw before deciding).
    pub field_v: V2,
    /// Gust velocity used during this beat (zero for `Wait`).
    pub gust: V2,
    /// Sub-step positions during the beat (excluding `from`).
    pub points: Vec<V2>,
    pub next: State,
}
impl Beat {
    pub fn to(&self) -> V2 {
        self.next.pos
    }
}

/// An (attempted) run: the start plus the beats that were played.
#[derive(Clone, Debug)]
pub struct Run {
    pub start: V2,
    pub beats: Vec<Beat>,
    pub end: Option<End>,
}
impl Run {
    pub fn actions(&self) -> Vec<Action> {
        self.beats.iter().map(|b| b.action).collect()
    }
    /// Full polyline: start followed by every sub-step.
    pub fn path(&self) -> Vec<V2> {
        let mut v = vec![self.start];
        for b in &self.beats {
            v.extend_from_slice(&b.points);
        }
        v
    }
    pub fn gusts_used(&self) -> usize {
        self.beats.iter().filter(|b| matches!(b.action, Action::Gust(_))).count()
    }
}

/// Immutable description of a Steer level; `advance` is a pure function of (spec, state, action).
#[derive(Clone, Debug)]
pub struct SteerSpec {
    pub field: Field,
    pub regions: Vec<Region>,
    pub cfg: SimConfig,
    pub start: V2,
    pub beats: usize,
    pub budget: usize,
    pub strength: f64,
    pub dirs: GustDirs,
    pub beat_time: f64,
    pub goal: usize,
    pub gates: Vec<usize>,
}

impl SteerSpec {
    pub fn from_level(l: &Level) -> SteerSpec {
        let Rules::Steer { start, beats, gust_budget, gust_strength, dirs, beat_time, goal } = &l.rules else { panic!("not a Steer level") };
        SteerSpec {
            field: l.field.clone(),
            regions: l.regions.clone(),
            cfg: l.sim_config(),
            start: *start,
            beats: *beats,
            budget: *gust_budget,
            strength: *gust_strength,
            dirs: *dirs,
            beat_time: *beat_time,
            goal: *goal,
            gates: l.gates(),
        }
    }
    pub fn initial(&self) -> State {
        State { pos: self.start, beat: 0, gusts_left: self.budget, gates: vec![], end: None }
    }
    /// Gust velocity vector for an action.
    pub fn gust(&self, a: Action) -> V2 {
        match a {
            Action::Wait => V2::ZERO,
            Action::Gust(i) => self.dirs.unit(i) * self.strength,
        }
    }
    /// Natural field velocity at a position during beat `beat`.
    pub fn field_velocity(&self, p: V2, beat: usize) -> V2 {
        self.field.eval(p, beat as f64 * self.beat_time)
    }
    pub fn legal(&self, s: &State, a: Action) -> Result<(), ChooseError> {
        if s.end.is_some() {
            return Err(ChooseError::Finished);
        }
        match a {
            Action::Wait => Ok(()),
            Action::Gust(i) if i >= self.dirs.count() => Err(ChooseError::BadDirection),
            Action::Gust(_) if s.gusts_left == 0 => Err(ChooseError::NoBudget),
            Action::Gust(_) => Ok(()),
        }
    }
    /// Advance exactly one beat. Deterministic; allocates a new `State`.
    pub fn advance(&self, s: &State, a: Action) -> Result<Beat, ChooseError> {
        self.legal(s, a)?;
        let u = self.gust(a);
        let t = s.beat as f64 * self.beat_time;
        let mut gates = s.gates.clone();
        let (points, outcome) = advance_beat(&self.field, s.pos, t, u, self.beat_time, &self.regions, &self.cfg, &mut gates);
        let to = points.last().copied().unwrap_or(s.pos);
        let beat = s.beat + 1;
        let gusts_left = s.gusts_left - usize::from(matches!(a, Action::Gust(_)));
        let end = match outcome {
            Some(Outcome::Reached(i)) if i == self.goal => {
                if self.gates.iter().all(|g| gates.contains(g)) {
                    Some(End::Success)
                } else {
                    Some(End::Failed(FailReason::MissedGate))
                }
            }
            Some(Outcome::Reached(_)) => Some(End::Failed(FailReason::WrongRegion)),
            Some(Outcome::Hazard(_)) => Some(End::Failed(FailReason::Hazard)),
            Some(Outcome::Boundary) => Some(End::Failed(FailReason::Boundary)),
            Some(Outcome::Invalid) => Some(End::Failed(FailReason::Invalid)),
            Some(Outcome::Timeout) | Some(Outcome::Stalled) => Some(End::Failed(FailReason::BeatsExhausted)),
            None if beat >= self.beats => Some(End::Failed(FailReason::BeatsExhausted)),
            None => None,
        };
        Ok(Beat { action: a, from: s.pos, field_v: self.field_velocity(s.pos, s.beat), gust: u, points, next: State { pos: to, beat, gusts_left, gates, end } })
    }
    /// Re-simulate a recorded action sequence from the initial state (replay).
    pub fn replay(&self, actions: &[Action]) -> Result<Run, ChooseError> {
        let mut s = self.initial();
        let mut beats = vec![];
        for a in actions {
            let b = self.advance(&s, *a)?;
            s = b.next.clone();
            beats.push(b);
        }
        Ok(Run { start: self.start, beats, end: s.end })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Awaiting,
    Advancing,
    Completed(End),
}

pub struct SteerGame {
    pub level: Level,
    pub spec: SteerSpec,
    pub state: State,
    pub phase: Phase,
    pub history: Vec<Beat>,
    pending: Option<Beat>,
    /// Earlier completed attempts, oldest first, at most `MAX_GHOSTS`.
    pub ghosts: Vec<Run>,
    pub attempts: usize,
    pub won: bool,
}

impl SteerGame {
    pub fn new(level: Level) -> SteerGame {
        let spec = SteerSpec::from_level(&level);
        let state = spec.initial();
        SteerGame { level, spec, state, phase: Phase::Ready, history: vec![], pending: None, ghosts: vec![], attempts: 0, won: false }
    }
    pub fn accepting_input(&self) -> bool {
        matches!(self.phase, Phase::Ready | Phase::Awaiting)
    }
    pub fn beat(&self) -> usize {
        self.state.beat
    }
    pub fn gusts_left(&self) -> usize {
        self.state.gusts_left
    }
    pub fn pos(&self) -> V2 {
        self.state.pos
    }
    /// Natural field velocity at the probe's current position.
    pub fn field_velocity(&self) -> V2 {
        self.spec.field_velocity(self.state.pos, self.state.beat)
    }
    /// Instantaneous velocity if `a` were chosen now: field + gust (a vector sum, not a trajectory).
    pub fn net_velocity(&self, a: Action) -> V2 {
        self.field_velocity() + self.spec.gust(a)
    }
    pub fn gust_vector(&self, a: Action) -> V2 {
        self.spec.gust(a)
    }
    /// Commit a decision and start advancing one beat. Rejected while a beat is in progress.
    pub fn choose(&mut self, a: Action) -> Result<(), ChooseError> {
        if !self.accepting_input() {
            return Err(if matches!(self.phase, Phase::Completed(_)) { ChooseError::Finished } else { ChooseError::Busy });
        }
        let beat = self.spec.advance(&self.state, a)?;
        // The budget is spent the moment the decision is committed, so the HUD shows it at once.
        self.state.gusts_left = beat.next.gusts_left;
        self.pending = Some(beat);
        self.phase = Phase::Advancing;
        Ok(())
    }
    /// Points of the beat currently being advanced (for animation). Empty when idle.
    pub fn pending_points(&self) -> &[V2] {
        self.pending.as_ref().map_or(&[], |b| &b.points)
    }
    pub fn pending_beat(&self) -> Option<&Beat> {
        self.pending.as_ref()
    }
    /// Finish the beat that `choose` started and evaluate success / failure.
    pub fn finish_beat(&mut self) -> Option<End> {
        if self.phase != Phase::Advancing {
            return None;
        }
        let beat = self.pending.take()?;
        self.state = beat.next.clone();
        let end = self.state.end;
        self.history.push(beat);
        match end {
            Some(e) => {
                self.phase = Phase::Completed(e);
                if e == End::Success {
                    self.won = true;
                }
            }
            None => self.phase = Phase::Awaiting,
        }
        end
    }
    /// choose + finish in one call (headless play, search, replay).
    pub fn step(&mut self, a: Action) -> Result<Option<End>, ChooseError> {
        self.choose(a)?;
        Ok(self.finish_beat())
    }
    pub fn run(&self) -> Run {
        Run { start: self.spec.start, beats: self.history.clone(), end: self.state.end }
    }
    /// Restart from the exact initial state. A *completed* run becomes a ghost.
    pub fn retry(&mut self) -> Result<(), ChooseError> {
        if self.phase == Phase::Advancing {
            return Err(ChooseError::Busy);
        }
        if matches!(self.phase, Phase::Completed(_)) {
            self.ghosts.push(self.run());
            if self.ghosts.len() > MAX_GHOSTS {
                self.ghosts.remove(0);
            }
        }
        self.state = self.spec.initial();
        self.history.clear();
        self.pending = None;
        self.phase = Phase::Ready;
        self.attempts += 1;
        Ok(())
    }
    /// The run to replay: the finished current run, else the latest ghost.
    pub fn replayable(&self) -> Option<Run> {
        if matches!(self.phase, Phase::Completed(_)) {
            Some(self.run())
        } else {
            self.ghosts.last().cloned()
        }
    }
}
