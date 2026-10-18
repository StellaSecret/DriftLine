//! TEMPO - "When should I launch?"  Only the launch phase of a periodic field is chosen.
use crate::level::{Level, Rules};
use crate::sim::Trajectory;

#[derive(Clone, Debug)]
pub struct TempoAttempt {
    pub phase: f64,
    pub traj: Trajectory,
    pub success: bool,
}

pub struct TempoGame {
    pub level: Level,
    /// Field clock shown to the player, in [0, period).
    pub clock: f64,
    pub playing: bool,
    pub attempts: Vec<TempoAttempt>,
    pub won: bool,
}

impl TempoGame {
    pub fn new(level: Level) -> Self {
        assert!(matches!(level.rules, Rules::Tempo { .. }));
        TempoGame { level, clock: 0.0, playing: true, attempts: vec![], won: false }
    }
    pub fn period(&self) -> f64 {
        match &self.level.rules {
            Rules::Tempo { period, .. } => *period,
            _ => unreachable!(),
        }
    }
    /// Let the field evolve in real time so the player can watch it.
    pub fn tick(&mut self, dt: f64) {
        if self.playing {
            self.clock = (self.clock + dt).rem_euclid(self.period());
        }
    }
    /// Scrub the timeline (pauses playback).
    pub fn set_phase(&mut self, ph: f64) {
        self.clock = ph.rem_euclid(self.period());
        self.playing = false;
    }
    /// Commit: release the probe at the current clock phase.
    pub fn launch(&mut self) -> &TempoAttempt {
        let (start, goal) = match &self.level.rules {
            Rules::Tempo { start, goal, .. } => (*start, *goal),
            _ => unreachable!(),
        };
        let traj = self.level.run(&self.level.field, start, self.clock);
        let success = self.level.reached_goal(&traj, goal);
        self.won |= success;
        self.attempts.push(TempoAttempt { phase: self.clock, traj, success });
        self.attempts.last().unwrap()
    }
}
