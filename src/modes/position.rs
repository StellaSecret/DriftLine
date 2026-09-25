//! POSITION - "Where should I start?"  Only the release point(s) are controllable.
use crate::field::V2;
use crate::level::{Level, Rules, Slot};
use crate::sim::Trajectory;

#[derive(Clone, Debug)]
pub struct Attempt {
    pub starts: Vec<V2>,
    pub trajs: Vec<Trajectory>,
    pub slot_ok: Vec<bool>,
    pub success: bool,
    pub phase: f64,
    pub intensity: f64,
}

pub struct PositionGame {
    pub level: Level,
    pub placed: Vec<Option<V2>>,
    /// Only used by combination levels (Position + Tempo).
    pub phase: f64,
    /// Only used by combination levels (shared intensity).
    pub intensity: f64,
    pub attempts: Vec<Attempt>,
    pub won: bool,
}

impl PositionGame {
    pub fn new(level: Level) -> Self {
        let n = Self::slots_of(&level).len();
        let intensity = match &level.rules {
            Rules::Position { intensity: Some((a, b)), .. } => (a + b) * 0.5,
            _ => level.field.intensity,
        };
        PositionGame { level, placed: vec![None; n], phase: 0.0, intensity, attempts: vec![], won: false }
    }
    fn slots_of(l: &Level) -> Vec<Slot> {
        match &l.rules {
            Rules::Position { slots, .. } => slots.clone(),
            _ => panic!("not a Position level"),
        }
    }
    pub fn slots(&self) -> Vec<Slot> {
        Self::slots_of(&self.level)
    }
    pub fn period(&self) -> Option<f64> {
        match &self.level.rules {
            Rules::Position { period, .. } => *period,
            _ => None,
        }
    }
    pub fn intensity_range(&self) -> Option<(f64, f64)> {
        match &self.level.rules {
            Rules::Position { intensity, .. } => *intensity,
            _ => None,
        }
    }
    /// Place the probe of whichever slot's zone contains `p`. Returns the slot index.
    pub fn place_at(&mut self, p: V2) -> Option<usize> {
        let i = self.slots().iter().position(|s| s.zone.contains(p))?;
        self.placed[i] = Some(p);
        Some(i)
    }
    /// Keyboard nudge, clamped to the slot's zone.
    pub fn nudge(&mut self, slot: usize, d: V2) {
        let zone = &self.slots()[slot].zone;
        let cur = self.placed[slot].unwrap_or_else(|| zone.center());
        self.placed[slot] = Some(zone.clamp(cur + d));
    }
    pub fn set_phase(&mut self, ph: f64) {
        if let Some(per) = self.period() {
            self.phase = ph.rem_euclid(per);
        }
    }
    pub fn set_intensity(&mut self, i: f64) {
        if let Some((a, b)) = self.intensity_range() {
            self.intensity = i.clamp(a, b);
        }
    }
    pub fn ready(&self) -> bool {
        self.placed.iter().all(|p| p.is_some())
    }
    pub fn launch(&mut self) -> Option<&Attempt> {
        if !self.ready() {
            return None;
        }
        let field = self.level.field.with_intensity(self.intensity);
        let slots = self.slots();
        let mut a = Attempt { starts: vec![], trajs: vec![], slot_ok: vec![], success: true, phase: self.phase, intensity: self.intensity };
        for (i, s) in slots.iter().enumerate() {
            let p = self.placed[i].unwrap();
            let t = self.level.run(&field, p, self.phase);
            let ok = self.level.reached_goal(&t, s.goal);
            a.success &= ok;
            a.starts.push(p);
            a.slot_ok.push(ok);
            a.trajs.push(t);
        }
        self.won |= a.success;
        self.attempts.push(a);
        self.attempts.last()
    }
}
