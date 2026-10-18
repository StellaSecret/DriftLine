//! Level definitions are plain data. Rendering and UI only read them.
use crate::field::{v2, Field, V2};
use crate::sim::{Outcome, Rect, Region, RegionKind, SimConfig, Trajectory};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Chapter {
    Position,
    Predict,
    Tempo,
    Experiment,
    Combo,
}
impl Chapter {
    pub const ALL: [Chapter; 5] = [Chapter::Position, Chapter::Predict, Chapter::Tempo, Chapter::Experiment, Chapter::Combo];
    pub fn name(self) -> &'static str {
        match self {
            Chapter::Position => "Position",
            Chapter::Predict => "Predict",
            Chapter::Tempo => "Tempo",
            Chapter::Experiment => "Experiment",
            Chapter::Combo => "Combinations",
        }
    }
    pub fn question(self) -> &'static str {
        match self {
            Chapter::Position => "Where should I start?",
            Chapter::Predict => "Where will it go?",
            Chapter::Tempo => "When should I launch?",
            Chapter::Experiment => "How does the system behave?",
            Chapter::Combo => "Mix what you have learned.",
        }
    }
}

/// A region of the plane (release zones).
#[derive(Clone, Debug)]
pub enum Zone {
    Circle(V2, f64),
    Rect(Rect),
}
impl Zone {
    pub fn contains(&self, p: V2) -> bool {
        match self {
            Zone::Circle(c, r) => p.dist(*c) <= *r,
            Zone::Rect(r) => r.contains(p),
        }
    }
    pub fn clamp(&self, p: V2) -> V2 {
        match self {
            Zone::Circle(c, r) => {
                let d = p - *c;
                if d.len() <= *r {
                    p
                } else {
                    *c + d * (*r / d.len())
                }
            }
            Zone::Rect(r) => r.clamp(p),
        }
    }
    pub fn center(&self) -> V2 {
        match self {
            Zone::Circle(c, _) => *c,
            Zone::Rect(r) => v2((r.min.x + r.max.x) * 0.5, (r.min.y + r.max.y) * 0.5),
        }
    }
    pub fn bbox(&self) -> Rect {
        match self {
            Zone::Circle(c, r) => Rect::new(c.x - r, c.y - r, c.x + r, c.y + r),
            Zone::Rect(r) => *r,
        }
    }
    /// n x n sample grid (edges included) restricted to the zone.
    pub fn grid(&self, n: usize) -> Vec<V2> {
        let b = self.bbox();
        let mut out = vec![];
        for i in 0..n {
            for j in 0..n {
                let p = v2(b.min.x + b.width() * i as f64 / (n - 1) as f64, b.min.y + b.height() * j as f64 / (n - 1) as f64);
                if self.contains(p) {
                    out.push(p);
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub zone: Zone,
    /// Index into `Level::regions` this probe must reach.
    pub goal: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reveal {
    Hidden,
    /// Show the first N seconds of the true path before the player commits.
    Partial(f64),
}

/// What an Experiment level asks the player to discover.
#[derive(Clone, Debug)]
pub enum Challenge {
    /// As intensity rises, does the exit point travel clockwise or counter-clockwise around `center`?
    ExitShift { center: V2 },
    /// Sweeping intensity upward: does the probe ever return to an exit it already left?
    Return,
    /// The test rig only runs up to `limit`; predict the outcome at the untestable `target` beyond it.
    Extrapolate { limit: f64, target: f64 },
    /// Find the intensity where the exit changes from `from` to `to`.
    Threshold { from: usize, to: usize, tol: f64 },
    /// Find the interval of intensity for which the probe leaves through `exit`.
    Interval { exit: usize, tol: f64 },
}

#[derive(Clone, Debug)]
pub enum Rules {
    /// Choose release position(s). Optional phase / shared-intensity controls are for combination levels.
    Position { slots: Vec<Slot>, period: Option<f64>, intensity: Option<(f64, f64)> },
    /// Commit to a forecast of the exit; `rounds` rounds, `pass` correct needed.
    Predict { zone: Zone, rounds: usize, pass: usize, reveal: Reveal },
    /// Choose the launch phase of a periodic field.
    Tempo { start: V2, period: f64, goal: usize },
    /// Probe the intensity knob, then report the rule you found.
    Experiment { start: V2, range: (f64, f64), challenge: Challenge },
}

#[derive(Clone, Debug)]
pub struct Level {
    pub id: &'static str,
    pub chapter: Chapter,
    pub title: &'static str,
    pub objective: &'static str,
    /// Progressive hints: explain field behaviour, never the answer.
    pub hints: Vec<&'static str>,
    pub field: Field,
    pub bounds: Rect,
    pub regions: Vec<Region>,
    pub rules: Rules,
    pub max_time: f64,
    pub seed: u64,
}

impl Level {
    pub fn sim_config(&self) -> SimConfig {
        SimConfig::new(self.bounds, self.max_time)
    }
    pub fn run(&self, field: &Field, start: V2, t0: f64) -> Trajectory {
        crate::sim::simulate(field, start, t0, &self.regions, &self.sim_config())
    }
    pub fn gates(&self) -> Vec<usize> {
        self.regions.iter().enumerate().filter(|(_, r)| r.kind == RegionKind::Gate).map(|(i, _)| i).collect()
    }
    pub fn exits(&self) -> Vec<usize> {
        self.regions.iter().enumerate().filter(|(_, r)| r.kind == RegionKind::Exit).map(|(i, _)| i).collect()
    }
    /// Success = reached `goal` after passing every gate.
    pub fn reached_goal(&self, t: &Trajectory, goal: usize) -> bool {
        t.outcome == Outcome::Reached(goal) && self.gates().iter().all(|g| t.gates.contains(g))
    }
    pub fn describe(&self, o: Outcome) -> String {
        match o {
            Outcome::Reached(i) => match self.regions[i].kind {
                RegionKind::Exit => format!("Left through exit {}", self.regions[i].label),
                _ => "Reached the target".to_string(),
            },
            Outcome::Hazard(_) => "Hit the hazard".to_string(),
            Outcome::Boundary => "Drifted out of the field".to_string(),
            Outcome::Timeout => "Ran out of time".to_string(),
            Outcome::Stalled => "Came to rest".to_string(),
            Outcome::Invalid => "Simulation error".to_string(),
        }
    }
}
