//! Renders `docs/LEVEL_CATALOG.md` from the real level data. Parameters only (no simulation), so the
//! output is exactly reproducible on every platform; `tests/docs.rs` fails if the committed file is stale.
//! Regenerate with: `cargo run --release --bin catalog > docs/LEVEL_CATALOG.md`
use crate::field::{SwirlKind, Term, V2};
use crate::level::{Challenge, GustDirs, Level, Reveal, Rules, Zone};
use crate::levels;
use crate::sim::{Rect, RegionKind};

fn n(x: f64) -> String {
    format!("{}", x)
}
fn v(p: V2) -> String {
    format!("({}, {})", n(p.x), n(p.y))
}
fn rect(r: &Rect) -> String {
    format!("x in [{}, {}], y in [{}, {}]", n(r.min.x), n(r.max.x), n(r.min.y), n(r.max.y))
}
fn zone(z: &Zone) -> String {
    match z {
        Zone::Circle(c, r) => format!("circle centre {} radius {}", v(*c), n(*r)),
        Zone::Rect(r) => format!("rectangle {}", rect(r)),
    }
}

/// Human-readable formula for a term. `Custom` functions cannot be inspected, so the two used in the
/// shipped levels are described here by level id (keep in sync with `levels.rs`).
fn term(t: &Term, level_id: &str) -> String {
    match t {
        Term::Uniform(p) => format!("uniform flow {}", v(*p)),
        Term::Radial { c, k, soft } => format!("radial about {}: v = k d / (|d|^2 + soft^2), k = {}, soft = {} ({})", v(*c), n(*k), n(*soft), if *k < 0.0 { "attracts" } else { "repels" }),
        Term::Vortex { c, k, soft } => format!("vortex about {}: v = k perp(d) / (|d|^2 + soft^2), k = {}, soft = {} ({})", v(*c), n(*k), n(*soft), if *k > 0.0 { "counter-clockwise" } else { "clockwise" }),
        Term::Gauss { c, k, sigma } => format!("local radial push about {}: v = k d exp(-|d|^2 / (2 sigma^2)), k = {}, sigma = {} ({}; peak speed k sigma e^-1/2 at distance sigma; 5.5% of the peak at 3 sigma, 0.2% at 4 sigma)", v(*c), n(*k), n(*sigma), if *k < 0.0 { "attracts" } else { "repels" }),
        Term::Saddle { c, k } => format!("saddle at {}: v = k(-dx, +dy), k = {} (inflow along x, outflow along y)", v(*c), n(*k)),
        Term::Rotating { speed, omega, phase } => format!("rotating uniform flow: speed {}, direction angle = {} t + {}", n(*speed), n(*omega), n(*phase)),
        Term::Orbiting { kind, c, orbit, omega, phase, k, soft } => format!(
            "orbiting {} source: centre {} + {} (cos(a), sin(a)), a = {} t + {}; k = {}, soft = {}",
            if *kind == SwirlKind::Vortex { "vortex" } else { "radial" },
            v(*c),
            n(*orbit),
            n(*omega),
            n(*phase),
            n(*k),
            n(*soft)
        ),
        Term::Modulated { inner, offset, amp, omega, phase } => format!("modulated: [{}] x ({} + {} sin({} t + {}))", term(inner, level_id), n(*offset), n(*amp), n(*omega), n(*phase)),
        Term::Wave { dir, amp, wave, omega } => format!("travelling wave: {} x {} sin({} . p - {} t)", v(*dir), n(*amp), v(*wave), n(*omega)),
        Term::Custom(_) => match level_id {
            "tem-1" => "custom square wave: v = (0, tanh(4 sin(2 pi t / 12)))".to_string(),
            "ste-2" => "custom leverage field: dz = exp(-(x/1.5)^2); k = 1.3 dz - 0.35 (1 - dz) [x > 0]; v = (1.4, k y)".to_string(),
            _ => "custom function (see levels.rs)".to_string(),
        },
    }
}

fn rules(l: &Level) -> Vec<String> {
    match &l.rules {
        Rules::Position { slots, period, intensity } => {
            let mut out = vec!["mechanic: **Position**".to_string()];
            for (i, s) in slots.iter().enumerate() {
                out.push(format!("release zone {}: {}; its probe must reach region {} ({})", i + 1, zone(&s.zone), s.goal, l.regions[s.goal].label));
            }
            out.push(match period {
                Some(p) => format!("launch phase is also chosen, period {} s", n(*p)),
                None => "launch phase: fixed (0)".into(),
            });
            out.push(match intensity {
                Some((a, b)) => format!("shared intensity is also chosen, range [{}, {}]", n(*a), n(*b)),
                None => format!("intensity: fixed ({})", n(l.field.intensity)),
            });
            out
        }
        Rules::Predict { zone: z, rounds, pass, reveal } => vec![
            "mechanic: **Predict**".to_string(),
            format!("start zone (release points are drawn from it): {}", zone(z)),
            format!("rounds: {}; correct answers needed: {}", rounds, pass),
            match reveal {
                Reveal::Hidden => "reveal before commit: nothing".to_string(),
                Reveal::Partial(s) => format!("reveal before commit: the first {} s of the true path", n(*s)),
            },
        ],
        Rules::Tempo { start, period, goal } => vec!["mechanic: **Tempo**".to_string(), format!("fixed start {}", v(*start)), format!("field period {} s", n(*period)), format!("goal region {} ({})", goal, l.regions[*goal].label)],
        Rules::Experiment { start, range, challenge } => {
            let ch = match challenge {
                Challenge::ExitShift { center } => format!("ExitShift about {}: clockwise / counter-clockwise / neither", v(*center)),
                Challenge::Return => "Return: does the exit sequence revisit an exit?".to_string(),
                Challenge::Extrapolate { limit, target } => format!("Extrapolate: rig runs up to intensity {}, question asks about intensity {}", n(*limit), n(*target)),
                Challenge::Threshold { from, to, tol } => format!("Threshold: intensity where exit {} becomes exit {}, tolerance +/-{}", l.regions[*from].label, l.regions[*to].label, n(*tol)),
                Challenge::Interval { exit, tol } => format!("Interval: both ends of the intensity band leaving through exit {}, tolerance +/-{}", l.regions[*exit].label, n(*tol)),
            };
            vec!["mechanic: **Experiment**".to_string(), format!("fixed start {}", v(*start)), format!("intensity range [{}, {}]", n(range.0), n(range.1)), format!("challenge: {}", ch)]
        }
        Rules::Survey { zone: z, free, scouts, k, sigma, tol, rays, pool } => vec![
            "mechanic: **Survey**".to_string(),
            "the field layers above are the *charted background*; the hidden source is added to them and is not listed".to_string(),
            format!("hidden source: local repeller (Gauss term), strength k = {}, width sigma = {}, position unknown", n(*k), n(*sigma)),
            format!("scouts may be placed anywhere in {}; each leaves a {} s trail", zone(z), n(l.max_time)),
            format!("free scout flown at the start of every attempt from {}; {} further scouts can be placed", v(*free), scouts),
            format!("one commit per attempt; correct within {} of the source", n(*tol)),
            format!("estimate tests per attempt: {} (each shows what the scouts would have done if the source were at the marker)", crate::modes::TESTS_PER_ROUND),
            format!("reading aid: faint rays back along each push arrow {}", if *rays { "on" } else { "off" }),
            format!("hidden positions, one per attempt, easy to hard on the first pass, then reshuffled each pass: {}", pool.iter().map(|p| v(*p)).collect::<Vec<_>>().join(", ")),
        ],
        Rules::Steer { start, beats, gust_budget, gust_strength, dirs, beat_time, goal } => vec![
            "mechanic: **Steer**".to_string(),
            format!("fixed start {}", v(*start)),
            format!("beats {}; gust budget {}; gust strength {}; beat length {} s", beats, gust_budget, n(*gust_strength), n(*beat_time)),
            format!("gust directions: {}", if *dirs == GustDirs::Eight { "8 (E, NE, N, NW, W, SW, S, SE)" } else { "4 (E, N, W, S)" }),
            format!("goal region {} ({})", goal, l.regions[*goal].label),
        ],
    }
}

pub fn render() -> String {
    let mut s = String::new();
    s.push_str("# Level catalog\n\n");
    s.push_str("Generated from `src/levels.rs` by `src/catalog.rs`; do not edit by hand. Regenerate with\n`cargo run --release --bin catalog > docs/LEVEL_CATALOG.md` (a test fails if this file is out of date).\n\n");
    s.push_str("World coordinates: x to the right, y up. Every level uses the bounds x in [-6, 6], y in [-4, 4]. Region indices are positions in the level's region list. Levels are listed in play order.\n");
    for l in levels::all() {
        s.push_str(&format!("\n## {} - {} ({})\n\n", l.id, l.title, l.chapter.name()));
        s.push_str(&format!("- objective: {}\n", l.objective));
        for (i, h) in l.hints.iter().enumerate() {
            s.push_str(&format!("- hint {}: {}\n", i + 1, h));
        }
        s.push_str(&format!("- bounds: {}; simulation time limit: {} s; seed: {}\n", rect(&l.bounds), n(l.max_time), l.seed));
        s.push_str("- field layers (summed; a *tunable* layer is multiplied by the intensity):\n");
        for (i, ly) in l.field.layers.iter().enumerate() {
            s.push_str(&format!("  {}. {} - {}\n", i + 1, if ly.tunable { "tunable" } else { "fixed" }, term(&ly.term, l.id)));
        }
        if l.field.layers.iter().any(|x| x.tunable) {
            s.push_str(&format!("  (default intensity {})\n", n(l.field.intensity)));
        }
        s.push_str("- regions:\n");
        for (i, r) in l.regions.iter().enumerate() {
            let kind = match r.kind {
                RegionKind::Target => "target",
                RegionKind::Exit => "exit",
                RegionKind::Hazard => "hazard",
                RegionKind::Gate => "gate",
            };
            s.push_str(&format!("  {}. {} '{}' centre {} radius {}\n", i, kind, r.label, v(r.center), n(r.radius)));
        }
        s.push_str("- rules:\n");
        for r in rules(&l) {
            s.push_str(&format!("  - {}\n", r));
        }
    }
    s
}
