//! EXPERIMENT - "How does the system behave?"  The goal is a *rule*, not a lucky value.
//!
//! The player runs controlled tests with the intensity knob, then submits a conclusion. Every
//! conclusion is checked against ground truth computed by sweeping the real simulation, and each
//! challenge demands evidence first (e.g. you must have observed both exits before reporting the
//! threshold between them), so random poking cannot finish a level.
use crate::field::V2;
use crate::level::{Challenge, Level, Rules};
use crate::sim::{Outcome, Trajectory};

#[derive(Clone, Debug)]
pub struct Run {
    pub intensity: f64,
    pub traj: Trajectory,
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub enum Conclusion {
    Choice(usize),
    Estimates(Vec<f64>),
}

#[derive(Clone, Debug)]
pub struct Verdict {
    pub correct: bool,
    pub message: String,
}

pub struct ExperimentGame {
    pub level: Level,
    pub runs: Vec<Run>,
    pub solved: bool,
    pub feedback: String,
}

fn parts(l: &Level) -> (V2, (f64, f64), &Challenge) {
    match &l.rules {
        Rules::Experiment { start, range, challenge } => (*start, *range, challenge),
        _ => panic!("not an Experiment level"),
    }
}

/// Outcome of the true simulation at a given intensity.
pub fn probe(level: &Level, intensity: f64) -> Trajectory {
    let (start, _, _) = parts(level);
    level.run(&level.field.with_intensity(intensity), start, 0.0)
}

pub fn sweep(level: &Level, n: usize) -> Vec<(f64, Trajectory)> {
    let (_, (a, b), _) = parts(level);
    (0..n).map(|i| {
        let x = a + (b - a) * i as f64 / (n - 1) as f64;
        (x, probe(level, x))
    }).collect()
}

/// Unwrapped angle (radians, + = counter-clockwise) a path sweeps around `c`.
pub fn winding(t: &Trajectory, c: V2) -> f64 {
    use std::f64::consts::{PI, TAU};
    let mut total = 0.0;
    for w in t.points.windows(2) {
        let mut d = (w[1] - c).angle() - (w[0] - c).angle();
        while d > PI {
            d -= TAU
        }
        while d < -PI {
            d += TAU
        }
        total += d;
    }
    total
}

/// Bisect the boundary of a predicate between a point where it holds and one where it does not.
fn bisect(level: &Level, mut yes: f64, mut no: f64, pred: &dyn Fn(&Trajectory) -> bool) -> f64 {
    for _ in 0..40 {
        let m = 0.5 * (yes + no);
        if pred(&probe(level, m)) {
            yes = m
        } else {
            no = m
        }
    }
    0.5 * (yes + no)
}

/// Ground truth for the level's challenge.
#[derive(Clone, Debug, PartialEq)]
pub enum Truth {
    /// 0 = clockwise, 1 = counter-clockwise, 2 = neither / inconsistent
    Choice(usize),
    /// 0 = yes it returns, 1 = no
    ReturnChoice(usize),
    /// Exit region index at the target intensity (None = leaves between exits).
    ExitAt(Option<usize>),
    Estimates(Vec<f64>),
}

pub fn truth(level: &Level) -> Option<Truth> {
    let (_, (lo, hi), ch) = parts(level);
    match ch {
        Challenge::ExitShift { center } => {
            // Net winding angle of the whole path around the centre, as a function of intensity.
            let s = sweep(level, 61);
            let ang: Vec<f64> = s.iter().map(|(_, t)| winding(t, *center)).collect();
            let (mut pos, mut neg) = (0, 0);
            for w in ang.windows(2) {
                let d = w[1] - w[0];
                if d > 1e-3 {
                    pos += 1
                } else if d < -1e-3 {
                    neg += 1
                }
            }
            Some(Truth::Choice(match (pos, neg) {
                (p, 0) if p > 0 => 1,
                (0, n) if n > 0 => 0,
                _ => 2,
            }))
        }
        Challenge::Return => {
            let mut seq: Vec<usize> = vec![];
            for (_, t) in sweep(level, 241) {
                if let Outcome::Reached(e) = t.outcome {
                    if seq.last() != Some(&e) {
                        seq.push(e)
                    }
                }
            }
            let returns = seq.iter().enumerate().any(|(i, e)| seq[..i].contains(e));
            Some(Truth::ReturnChoice(if returns { 0 } else { 1 }))
        }
        Challenge::Extrapolate { target, .. } => match probe(level, *target).outcome {
            Outcome::Reached(e) => Some(Truth::ExitAt(Some(e))),
            Outcome::Boundary => Some(Truth::ExitAt(None)),
            _ => None,
        },
        Challenge::Threshold { from, to, .. } => {
            let s = sweep(level, 401);
            let isf = |t: &Trajectory| t.outcome == Outcome::Reached(*from);
            for w in s.windows(2) {
                if isf(&w[0].1) && w[1].1.outcome == Outcome::Reached(*to) {
                    return Some(Truth::Estimates(vec![bisect(level, w[0].0, w[1].0, &isf)]));
                }
            }
            None
        }
        Challenge::Interval { exit, .. } => {
            let s = sweep(level, 401);
            let is = |t: &Trajectory| t.outcome == Outcome::Reached(*exit);
            let first = s.iter().position(|(_, t)| is(t))?;
            let last = s.iter().rposition(|(_, t)| is(t))?;
            let a = if first == 0 { lo } else { bisect(level, s[first].0, s[first - 1].0, &is) };
            let b = if last + 1 >= s.len() { hi } else { bisect(level, s[last].0, s[last + 1].0, &is) };
            Some(Truth::Estimates(vec![a, b]))
        }
    }
}

impl ExperimentGame {
    pub fn new(level: Level) -> Self {
        let _ = parts(&level);
        ExperimentGame { level, runs: vec![], solved: false, feedback: String::new() }
    }
    pub fn range(&self) -> (f64, f64) {
        parts(&self.level).1
    }
    pub fn challenge(&self) -> &Challenge {
        parts(&self.level).2
    }
    /// Labels of the choices for choice-type challenges (empty for estimate challenges).
    pub fn choices(&self) -> Vec<String> {
        match self.challenge() {
            Challenge::ExitShift { .. } => vec!["Clockwise".into(), "Counter-clockwise".into(), "Neither / inconsistent".into()],
            Challenge::Return => vec!["Yes: it returns to an exit it left".into(), "No: every exit is visited once".into()],
            Challenge::Extrapolate { .. } => {
                let mut v: Vec<String> = self.level.exits().iter().map(|i| format!("Exit {}", self.level.regions[*i].label)).collect();
                v.push("Leaves between exits".into());
                v
            }
            _ => vec![],
        }
    }
    pub fn estimate_count(&self) -> usize {
        match self.challenge() {
            Challenge::Threshold { .. } => 1,
            Challenge::Interval { .. } => 2,
            _ => 0,
        }
    }
    pub fn question(&self) -> String {
        let l = &self.level;
        match self.challenge() {
            Challenge::ExitShift { .. } => "As intensity increases, which way does the probe's path turn around the centre?".into(),
            Challenge::Return => "Raise intensity from low to high. Does the probe ever return to an exit it already left?".into(),
            Challenge::Extrapolate { limit, target } => format!("The rig stops at {:.1}. What happens at intensity {:.1}?", limit, target),
            Challenge::Threshold { from, to, .. } => format!("At what intensity does the exit change from {} to {}?", l.regions[*from].label, l.regions[*to].label),
            Challenge::Interval { exit, .. } => format!("For which intensity interval does the probe leave through exit {}? Give both ends.", l.regions[*exit].label),
        }
    }
    /// Highest intensity the test rig can run (Extrapolate levels cap it).
    pub fn rig_limit(&self) -> f64 {
        match self.challenge() {
            Challenge::Extrapolate { limit, .. } => *limit,
            _ => self.range().1,
        }
    }
    pub fn is_locked(&self, i: f64) -> bool {
        i > self.rig_limit() + 1e-9
    }
    pub fn run(&mut self, intensity: f64) -> Result<usize, String> {
        let (a, b) = self.range();
        let i = intensity.clamp(a, b);
        if self.is_locked(i) {
            return Err("Beyond the test rig. Infer it from what you can run.".into());
        }
        let traj = probe(&self.level, i);
        self.runs.push(Run { intensity: i, traj, visible: true });
        Ok(self.runs.len() - 1)
    }
    pub fn reset(&mut self) {
        self.runs.clear();
        self.feedback.clear();
    }
    pub fn toggle_visible(&mut self, i: usize) {
        if let Some(r) = self.runs.get_mut(i) {
            r.visible = !r.visible;
        }
    }
    fn exit_label(&self, t: &Trajectory) -> Option<usize> {
        match t.outcome {
            Outcome::Reached(e) => Some(e),
            _ => None,
        }
    }
    /// Evidence checklist: what the player must have observed before concluding.
    pub fn evidence(&self) -> Vec<(String, bool)> {
        let (a, b) = self.range();
        let span = self.runs.iter().map(|r| r.intensity).fold(f64::NEG_INFINITY, f64::max)
            - self.runs.iter().map(|r| r.intensity).fold(f64::INFINITY, f64::min);
        let have_span = |frac: f64| self.runs.len() >= 2 && span >= frac * (b - a);
        match self.challenge() {
            Challenge::ExitShift { .. } => vec![
                ("3+ runs".into(), self.runs.len() >= 3),
                ("runs span 40% of the range".into(), have_span(0.4)),
            ],
            Challenge::Return => vec![
                ("5+ runs".into(), self.runs.len() >= 5),
                ("runs span 70% of the range".into(), have_span(0.7)),
            ],
            Challenge::Extrapolate { limit, .. } => vec![
                ("5+ runs".into(), self.runs.len() >= 5),
                ("runs span 60% of the testable range".into(), self.runs.len() >= 2 && span >= 0.6 * limit),
                ("a run in the top third of the rig".into(), self.runs.iter().any(|r| r.intensity >= limit * 0.66)),
            ],
            Challenge::Threshold { from, to, .. } => vec![
                (format!("seen exit {}", self.level.regions[*from].label), self.runs.iter().any(|r| self.exit_label(&r.traj) == Some(*from))),
                (format!("seen exit {}", self.level.regions[*to].label), self.runs.iter().any(|r| self.exit_label(&r.traj) == Some(*to))),
            ],
            Challenge::Interval { exit, .. } => vec![
                (format!("seen exit {}", self.level.regions[*exit].label), self.runs.iter().any(|r| self.exit_label(&r.traj) == Some(*exit))),
                ("seen a different result".into(), self.runs.iter().any(|r| self.exit_label(&r.traj) != Some(*exit))),
                ("3+ runs".into(), self.runs.len() >= 3),
            ],
        }
    }
    pub fn can_conclude(&self) -> bool {
        self.evidence().iter().all(|(_, ok)| *ok)
    }
    pub fn conclude(&mut self, c: Conclusion) -> Verdict {
        if !self.can_conclude() {
            return Verdict { correct: false, message: "Gather more evidence first.".into() };
        }
        let tr = truth(&self.level);
        let tol = match self.challenge() {
            Challenge::Threshold { tol, .. } | Challenge::Interval { tol, .. } => *tol,
            _ => 0.0,
        };
        let ok = match (&tr, &c) {
            (Some(Truth::Choice(t)), Conclusion::Choice(c)) => t == c,
            (Some(Truth::ReturnChoice(t)), Conclusion::Choice(c)) => t == c,
            (Some(Truth::ExitAt(e)), Conclusion::Choice(c)) => {
                let exits = self.level.exits();
                if *c == exits.len() { e.is_none() } else { e.is_some() && exits.get(*c) == e.as_ref() }
            }
            (Some(Truth::Estimates(t)), Conclusion::Estimates(e)) => {
                let mut e = e.clone();
                e.sort_by(|a, b| a.partial_cmp(b).unwrap());
                t.len() == e.len() && t.iter().zip(&e).all(|(a, b)| (a - b).abs() <= tol)
            }
            _ => false,
        };
        self.solved |= ok;
        let message = if ok {
            "Confirmed. Your rule matches the simulation.".to_string()
        } else {
            match self.challenge() {
                Challenge::Threshold { .. } | Challenge::Interval { .. } => "Not within tolerance. Bracket the change with more runs, then narrow it.".into(),
                _ => "That does not match the system. Compare your runs side by side before answering again.".into(),
            }
        };
        self.feedback = message.clone();
        Verdict { correct: ok, message }
    }
}
