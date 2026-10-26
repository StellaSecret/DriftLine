//! Field model: evaluates v(x, y, t; theta). Pure maths, no gameplay knowledge.
use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}
pub const fn v2(x: f64, y: f64) -> V2 {
    V2 { x, y }
}
impl V2 {
    pub const ZERO: V2 = v2(0.0, 0.0);
    pub fn dot(self, o: V2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    /// Rotate +90 degrees (counter-clockwise).
    pub fn perp(self) -> V2 {
        v2(-self.y, self.x)
    }
    pub fn dist(self, o: V2) -> f64 {
        (self - o).len()
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }
}
impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 {
        v2(self.x * k, self.y * k)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        v2(-self.x, -self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwirlKind {
    Radial,
    Vortex,
}

/// One building block of a field. Add new variants (or use `Custom`) to extend the
/// engine; gameplay code only ever calls `Field::eval`.
#[derive(Clone, Debug)]
pub enum Term {
    /// Constant velocity.
    Uniform(V2),
    /// v = k d / (|d|^2 + soft^2). k > 0 repels, k < 0 attracts.
    Radial { c: V2, k: f64, soft: f64 },
    /// Same falloff, perpendicular. k > 0 is counter-clockwise.
    Vortex { c: V2, k: f64, soft: f64 },
    /// v = k(-dx, +dy): inflow along x, outflow along y. Spatially sensitive.
    Saddle { c: V2, k: f64 },
    /// Uniform flow whose direction rotates in time.
    Rotating { speed: f64, omega: f64, phase: f64 },
    /// A radial/vortex source whose centre travels on a circle.
    Orbiting { kind: SwirlKind, c: V2, orbit: f64, omega: f64, phase: f64, k: f64, soft: f64 },
    /// inner * (offset + amp sin(omega t + phase)).
    Modulated { inner: Box<Term>, offset: f64, amp: f64, omega: f64, phase: f64 },
    /// Travelling shear wave: dir * amp * sin(wave . p - omega t).
    Wave { dir: V2, amp: f64, wave: V2, omega: f64 },
    /// A *local* radial push: v = k d exp(-|d|^2 / (2 sigma^2)). It peaks at |d| = sigma with speed
    /// k sigma e^(-1/2); it is still 45% of that at 2 sigma and 5.5% at 3 sigma, and below 0.25% from
    /// 4 sigma. Unlike `Radial` (a 1/r tail that is felt everywhere) it can only be detected by passing
    /// close. k > 0 repels, k < 0 attracts.
    Gauss { c: V2, k: f64, sigma: f64 },
    /// Escape hatch for any other function.
    Custom(fn(V2, f64) -> V2),
}

fn swirl(d: V2, k: f64, soft: f64, vortex: bool) -> V2 {
    let s = k / (d.dot(d) + soft * soft);
    if vortex {
        d.perp() * s
    } else {
        d * s
    }
}

impl Term {
    pub fn eval(&self, p: V2, t: f64) -> V2 {
        match self {
            Term::Uniform(v) => *v,
            Term::Radial { c, k, soft } => swirl(p - *c, *k, *soft, false),
            Term::Vortex { c, k, soft } => swirl(p - *c, *k, *soft, true),
            Term::Saddle { c, k } => {
                let d = p - *c;
                v2(-k * d.x, k * d.y)
            }
            Term::Rotating { speed, omega, phase } => {
                let a = omega * t + phase;
                v2(a.cos(), a.sin()) * *speed
            }
            Term::Orbiting { kind, c, orbit, omega, phase, k, soft } => {
                let a = omega * t + phase;
                let cc = *c + v2(a.cos(), a.sin()) * *orbit;
                swirl(p - cc, *k, *soft, *kind == SwirlKind::Vortex)
            }
            Term::Modulated { inner, offset, amp, omega, phase } => {
                inner.eval(p, t) * (offset + amp * (omega * t + phase).sin())
            }
            Term::Wave { dir, amp, wave, omega } => *dir * (amp * (wave.dot(p) - omega * t).sin()),
            Term::Gauss { c, k, sigma } => {
                let d = p - *c;
                d * (k * (-d.dot(d) / (2.0 * sigma * sigma)).exp())
            }
            Term::Custom(f) => f(p, t),
        }
    }
}

/// A term plus a flag saying whether the player's intensity knob scales it.
#[derive(Clone, Debug)]
pub struct Layer {
    pub term: Term,
    pub tunable: bool,
}

/// v(p, t; theta) with theta = { intensity }.
#[derive(Clone, Debug)]
pub struct Field {
    pub layers: Vec<Layer>,
    pub intensity: f64,
}

impl Default for Field {
    fn default() -> Self {
        Self::new()
    }
}

impl Field {
    pub fn new() -> Self {
        Field { layers: vec![], intensity: 1.0 }
    }
    /// Add a term that ignores the intensity knob.
    pub fn fixed(mut self, term: Term) -> Self {
        self.layers.push(Layer { term, tunable: false });
        self
    }
    /// Add a term scaled by the intensity knob.
    pub fn tunable(mut self, term: Term) -> Self {
        self.layers.push(Layer { term, tunable: true });
        self
    }
    pub fn eval(&self, p: V2, t: f64) -> V2 {
        let mut v = V2::ZERO;
        for l in &self.layers {
            let w = if l.tunable { self.intensity } else { 1.0 };
            v = v + l.term.eval(p, t) * w;
        }
        v
    }
    pub fn with_intensity(&self, intensity: f64) -> Field {
        let mut f = self.clone();
        f.intensity = intensity;
        f
    }
}
