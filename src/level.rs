//! Level definitions are plain data. Rendering and UI only read them.
use crate::field::{v2, Field, V2};
use crate::sim::{Outcome, Rect, Region, RegionKind, SimConfig, Trajectory};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Chapter {
    Position,
    Predict,
    Tempo,
    Experiment,
    Steer,
    Survey,
    Combo,
}
impl Chapter {
    pub const ALL: [Chapter; 7] = [Chapter::Position, Chapter::Predict, Chapter::Tempo, Chapter::Experiment, Chapter::Steer, Chapter::Survey, Chapter::Combo];
    pub fn name(self) -> &'static str {
        match self {
            Chapter::Position => "Position",
            Chapter::Predict => "Predict",
            Chapter::Tempo => "Tempo",
            Chapter::Experiment => "Experiment",
            Chapter::Steer => "Steer",
            Chapter::Survey => "Survey",
            Chapter::Combo => "Combinations",
        }
    }
    pub fn question(self) -> &'static str {
        match self {
            Chapter::Position => "Where should I start?",
            Chapter::Predict => "Where will it go?",
            Chapter::Tempo => "When should I launch?",
            Chapter::Experiment => "How does the system behave?",
            Chapter::Steer => "What should I do next?",
            Chapter::Survey => "Where is the hidden source?",
            Chapter::Combo => "Mix what you have learned.",
        }
    }
}

/// Permitted gust directions. Index 0 is East and indices run counter-clockwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GustDirs {
    /// E, N, W, S
    Four,
    /// E, NE, N, NW, W, SW, S, SE
    Eight,
}
impl GustDirs {
    pub fn count(self) -> usize {
        match self {
            GustDirs::Four => 4,
            GustDirs::Eight => 8,
        }
    }
    /// Unit vector of direction `i` (exact for the cardinal directions).
    pub fn unit(self, i: usize) -> V2 {
        const S: f64 = std::f64::consts::FRAC_1_SQRT_2;
        const T: [V2; 8] = [v2(1.0, 0.0), v2(S, S), v2(0.0, 1.0), v2(-S, S), v2(-1.0, 0.0), v2(-S, -S), v2(0.0, -1.0), v2(S, -S)];
        match self {
            GustDirs::Eight => T[i % 8],
            GustDirs::Four => T[(i % 4) * 2],
        }
    }
    pub fn name(self, i: usize) -> &'static str {
        const N: [&str; 8] = ["E", "NE", "N", "NW", "W", "SW", "S", "SE"];
        match self {
            GustDirs::Eight => N[i % 8],
            GustDirs::Four => N[(i % 4) * 2],
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
    /// Turn-based control: after every beat the player picks a gust (or waits). The field, start and
    /// target are fixed and fully visible; a gust adds `gust_strength` to the velocity for one beat.
    Steer { start: V2, beats: usize, gust_budget: usize, gust_strength: f64, dirs: GustDirs, beat_time: f64, goal: usize },
    /// Hidden-source survey. `Level::field` is the *charted* background. A hidden local repeller
    /// (`Term::Gauss`, known strength `k` and width `sigma`) adds to it at one position, drawn from
    /// `pool` (a different one each attempt). One scout is flown for free from `free`; the player
    /// places up to `scouts` more inside `zone` (each leaves a `Level::max_time`-second trail), then
    /// commits one estimate of the source position, correct within `tol`.
    /// `rays`: draw each push arrow as a long faint line back toward the source, so where they cross can be read
    /// off directly. On in the teaching level; later levels turn it off and the player must extend the arrows
    /// by eye.
    Survey { zone: Zone, free: V2, scouts: usize, k: f64, sigma: f64, tol: f64, rays: bool, pool: Vec<V2> },
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
