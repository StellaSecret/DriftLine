//! Deterministic RK4 simulation engine. Independent of rendering and UI.
use crate::field::{Field, V2};

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub min: V2,
    pub max: V2,
}
impl Rect {
    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect { min: V2 { x: x0, y: y0 }, max: V2 { x: x1, y: y1 } }
    }
    pub fn contains(&self, p: V2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
    pub fn clamp(&self, p: V2) -> V2 {
        V2 { x: p.x.clamp(self.min.x, self.max.x), y: p.y.clamp(self.min.y, self.max.y) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionKind {
    /// A goal the player wants to reach.
    Target,
    /// One of several candidate exits (labelled A, B, ...).
    Exit,
    /// Touching it ends the run as a failure.
    Hazard,
    /// Must be passed through before the goal counts (a "corridor" waypoint).
    Gate,
}

#[derive(Clone, Debug)]
pub struct Region {
    pub kind: RegionKind,
    pub center: V2,
    pub radius: f64,
    pub label: char,
}
impl Region {
    pub fn new(kind: RegionKind, x: f64, y: f64, radius: f64, label: char) -> Region {
        Region { kind, center: V2 { x, y }, radius, label }
    }
    pub fn target(x: f64, y: f64, r: f64) -> Region {
        Region::new(RegionKind::Target, x, y, r, 'T')
    }
    pub fn exit(x: f64, y: f64, r: f64, label: char) -> Region {
        Region::new(RegionKind::Exit, x, y, r, label)
    }
    pub fn hazard(x: f64, y: f64, r: f64) -> Region {
        Region::new(RegionKind::Hazard, x, y, r, 'X')
    }
    pub fn gate(x: f64, y: f64, r: f64) -> Region {
        Region::new(RegionKind::Gate, x, y, r, 'G')
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// Reached the target / exit with this region index.
    Reached(usize),
    /// Touched the hazard with this region index.
    Hazard(usize),
    /// Left the play field.
    Boundary,
    /// Ran out of simulated time.
    Timeout,
    /// Speed dropped to ~0 and stayed there.
    Stalled,
    /// Non-finite or runaway velocity.
    Invalid,
}

#[derive(Clone, Debug)]
pub struct SimConfig {
    pub dt: f64,
    pub max_time: f64,
    pub bounds: Rect,
    pub max_speed: f64,
    pub stall_speed: f64,
    pub stall_time: f64,
}
impl SimConfig {
    pub fn new(bounds: Rect, max_time: f64) -> SimConfig {
        SimConfig { dt: 0.02, max_time, bounds, max_speed: 50.0, stall_speed: 0.01, stall_time: 2.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Trajectory {
    /// Sampled every `dt`; points[0] is the release point.
    pub points: Vec<V2>,
    pub dt: f64,
    /// Field time at release.
    pub t0: f64,
    pub outcome: Outcome,
    /// Indices of gate regions passed (in order, no duplicates).
    pub gates: Vec<usize>,
}
impl Trajectory {
    pub fn duration(&self) -> f64 {
        (self.points.len().saturating_sub(1)) as f64 * self.dt
    }
    pub fn end(&self) -> V2 {
        *self.points.last().unwrap()
    }
    /// Position `s` seconds after release (linear interpolation, clamped).
    pub fn pos_at(&self, s: f64) -> V2 {
        let f = (s / self.dt).max(0.0);
        let i = f.floor() as usize;
        if i + 1 >= self.points.len() {
            return self.end();
        }
        let u = f - i as f64;
        let (a, b) = (self.points[i], self.points[i + 1]);
        a + (b - a) * u
    }
    /// Points visible up to `s` seconds after release (including the interpolated head).
    pub fn upto(&self, s: f64) -> Vec<V2> {
        let n = ((s / self.dt).floor() as usize + 1).min(self.points.len());
        let mut v: Vec<V2> = self.points[..n].to_vec();
        if n < self.points.len() {
            v.push(self.pos_at(s));
        }
        v
    }
}

pub fn rk4_step(field: &Field, p: V2, t: f64, dt: f64) -> V2 {
    rk4_step_u(field, p, t, dt, V2::ZERO)
}

/// RK4 for dp/dt = v(p, t) + u with a constant extra velocity `u` (a gust).
/// With `u = 0` this is bit-identical to the plain field step (adding 0.0 is exact).
pub fn rk4_step_u(field: &Field, p: V2, t: f64, dt: f64, u: V2) -> V2 {
    let k1 = field.eval(p, t) + u;
    let k2 = field.eval(p + k1 * (dt * 0.5), t + dt * 0.5) + u;
    let k3 = field.eval(p + k2 * (dt * 0.5), t + dt * 0.5) + u;
    let k4 = field.eval(p + k3 * dt, t + dt) + u;
    p + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0)
}

/// Closest approach of segment a-b to c: (distance, parameter along the segment in 0..=1).
pub fn seg_point(a: V2, b: V2, c: V2) -> (f64, f64) {
    let ab = b - a;
    let l2 = ab.dot(ab);
    let u = if l2 < 1e-18 { 0.0 } else { ((c - a).dot(ab) / l2).clamp(0.0, 1.0) };
    ((a + ab * u).dist(c), u)
}

/// Result of one integration sub-step.
pub struct SubStep {
    /// New position, or `None` if the state was numerically invalid.
    pub q: Option<V2>,
    /// Speed at the start of the sub-step (used for stall detection).
    pub speed: f64,
    /// Terminal outcome reached during this sub-step, if any.
    pub outcome: Option<Outcome>,
}

/// One RK4 sub-step plus all region / gate / bounds / validity checks. Shared by `simulate`
/// (whole trajectories) and `advance_beat` (turn-based Steer) so both use identical physics.
///
/// Contact detection tests the *whole segment* p->q against each region, so a probe cannot
/// tunnel through a target between two sampled positions. The segment is the chord of the RK4
/// step; its deviation from the true path is O(dt^2 * curvature), far below any target radius.
#[allow(clippy::too_many_arguments)]
pub fn substep(field: &Field, p: V2, t: f64, dt: f64, u: V2, regions: &[Region], cfg: &SimConfig, gates: &mut Vec<usize>) -> SubStep {
    let v = field.eval(p, t) + u;
    if !v.is_finite() || v.len() > cfg.max_speed {
        return SubStep { q: None, speed: f64::NAN, outcome: Some(Outcome::Invalid) };
    }
    let q = rk4_step_u(field, p, t, dt, u);
    if !q.is_finite() {
        return SubStep { q: None, speed: v.len(), outcome: Some(Outcome::Invalid) };
    }
    let mut terminal: Option<(f64, usize)> = None;
    for (idx, r) in regions.iter().enumerate() {
        if r.kind == RegionKind::Gate {
            continue;
        }
        let (d, w) = seg_point(p, q, r.center);
        if d <= r.radius && terminal.is_none_or(|(bu, _)| w < bu) {
            terminal = Some((w, idx));
        }
    }
    for (idx, r) in regions.iter().enumerate() {
        if r.kind != RegionKind::Gate || gates.contains(&idx) {
            continue;
        }
        let (d, w) = seg_point(p, q, r.center);
        if d <= r.radius && terminal.is_none_or(|(bu, _)| w <= bu) {
            gates.push(idx);
        }
    }
    let outcome = if let Some((_, idx)) = terminal {
        Some(if regions[idx].kind == RegionKind::Hazard { Outcome::Hazard(idx) } else { Outcome::Reached(idx) })
    } else if !cfg.bounds.contains(q) {
        Some(Outcome::Boundary)
    } else {
        None
    };
    SubStep { q: Some(q), speed: v.len(), outcome }
}

/// Integrate a probe released at `start` when the field clock reads `t0`.
/// Identical inputs always give bit-identical output.
pub fn simulate(field: &Field, start: V2, t0: f64, regions: &[Region], cfg: &SimConfig) -> Trajectory {
    let mut pts = vec![start];
    let mut gates: Vec<usize> = vec![];
    let mut p = start;
    let steps = (cfg.max_time / cfg.dt).ceil() as usize;
    let mut slow = 0usize;
    let slow_limit = (cfg.stall_time / cfg.dt) as usize;
    let mut outcome = Outcome::Timeout;
    for i in 0..steps {
        let t = t0 + i as f64 * cfg.dt;
        let s = substep(field, p, t, cfg.dt, V2::ZERO, regions, cfg, &mut gates);
        let Some(q) = s.q else {
            outcome = Outcome::Invalid;
            break;
        };
        pts.push(q);
        if let Some(o) = s.outcome {
            outcome = o;
            break;
        }
        if s.speed < cfg.stall_speed {
            slow += 1;
            if slow > slow_limit {
                outcome = Outcome::Stalled;
                break;
            }
        } else {
            slow = 0;
        }
        p = q;
    }
    Trajectory { points: pts, dt: cfg.dt, t0, outcome, gates }
}

/// Advance one *beat*: a fixed interval `beat_time` during which the constant gust `u` is added
/// to the field. Returns the sub-step positions (excluding the start), and the terminal outcome
/// if the probe reached a region, left the bounds, or became numerically invalid mid-beat.
/// The gust only exists inside this call; nothing persists to the next beat.
#[allow(clippy::too_many_arguments)]
pub fn advance_beat(field: &Field, start: V2, t0: f64, u: V2, beat_time: f64, regions: &[Region], cfg: &SimConfig, gates: &mut Vec<usize>) -> (Vec<V2>, Option<Outcome>) {
    let n = (beat_time / cfg.dt).round().max(1.0) as usize;
    let mut p = start;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let s = substep(field, p, t0 + i as f64 * cfg.dt, cfg.dt, u, regions, cfg, gates);
        let Some(q) = s.q else {
            return (pts, Some(Outcome::Invalid));
        };
        pts.push(q);
        if let Some(o) = s.outcome {
            return (pts, Some(o));
        }
        p = q;
    }
    (pts, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{v2, Term};

    fn cfg() -> SimConfig {
        SimConfig::new(Rect::new(-50.0, -50.0, 50.0, 50.0), 20.0)
    }

    #[test]
    fn uniform_flow_is_exact() {
        let f = Field::new().fixed(Term::Uniform(v2(1.0, 0.5)));
        let c = SimConfig { max_time: 4.0, ..cfg() };
        let t = simulate(&f, v2(0.0, 0.0), 0.0, &[], &c);
        let e = t.end();
        assert!((e.x - 4.0).abs() < 1e-9 && (e.y - 2.0).abs() < 1e-9);
        assert_eq!(t.outcome, Outcome::Timeout);
    }

    #[test]
    fn vortex_conserves_radius_under_rk4() {
        // Pure vortex: circular orbits. RK4 at dt = 0.02 must stay within 0.1% over several turns.
        let f = Field::new().fixed(Term::Vortex { c: v2(0.0, 0.0), k: 2.0, soft: 0.0 });
        let t = simulate(&f, v2(2.0, 0.0), 0.0, &[], &cfg());
        for p in &t.points {
            assert!((p.len() - 2.0).abs() < 2e-3, "radius drifted to {}", p.len());
        }
    }

    #[test]
    fn deterministic_replay() {
        let f = Field::new()
            .fixed(Term::Uniform(v2(0.4, 0.0)))
            .fixed(Term::Orbiting { kind: crate::field::SwirlKind::Vortex, c: v2(0.0, 0.0), orbit: 1.5, omega: 0.7, phase: 0.3, k: 1.2, soft: 0.5 });
        let a = simulate(&f, v2(-3.0, 0.4), 1.3, &[], &cfg());
        let b = simulate(&f, v2(-3.0, 0.4), 1.3, &[], &cfg());
        assert_eq!(a.points, b.points);
        assert_eq!(a.outcome, b.outcome);
    }

    #[test]
    fn detects_target_hazard_boundary_and_gate() {
        let f = Field::new().fixed(Term::Uniform(v2(1.0, 0.0)));
        let c = SimConfig { bounds: Rect::new(-1.0, -1.0, 10.0, 1.0), ..cfg() };
        let regs = [Region::gate(2.0, 0.0, 0.3), Region::target(5.0, 0.0, 0.3)];
        let t = simulate(&f, v2(0.0, 0.0), 0.0, &regs, &c);
        assert_eq!(t.outcome, Outcome::Reached(1));
        assert_eq!(t.gates, vec![0]);
        let regs = [Region::hazard(3.0, 0.0, 0.3), Region::target(5.0, 0.0, 0.3)];
        assert_eq!(simulate(&f, v2(0.0, 0.0), 0.0, &regs, &c).outcome, Outcome::Hazard(0));
        assert_eq!(simulate(&f, v2(0.0, 0.0), 0.0, &[], &c).outcome, Outcome::Boundary);
    }

    #[test]
    fn invalid_and_stalled_are_caught() {
        let bad = Field::new().fixed(Term::Custom(|_, _| v2(f64::NAN, 0.0)));
        assert_eq!(simulate(&bad, V2::ZERO, 0.0, &[], &cfg()).outcome, Outcome::Invalid);
        let fast = Field::new().fixed(Term::Uniform(v2(1e6, 0.0)));
        assert_eq!(simulate(&fast, V2::ZERO, 0.0, &[], &cfg()).outcome, Outcome::Invalid);
        let still = Field::new();
        assert_eq!(simulate(&still, V2::ZERO, 0.0, &[], &cfg()).outcome, Outcome::Stalled);
    }
}
