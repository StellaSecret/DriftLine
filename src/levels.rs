//! All content, as data. Field types, constraints and mechanics are combined here and nowhere else.
use crate::field::{v2, Field, SourceKind, SwirlKind, Term, V2};
use crate::level::*;
use crate::sim::{Rect, Region};
use std::f64::consts::TAU;

const B: Rect = Rect::new(-6.0, -4.0, 6.0, 4.0);

fn radial(x: f64, y: f64, k: f64, soft: f64) -> Term {
    Term::Radial { c: v2(x, y), k, soft }
}
fn vortex(x: f64, y: f64, k: f64, soft: f64) -> Term {
    Term::Vortex { c: v2(x, y), k, soft }
}
fn uni(x: f64, y: f64) -> Term {
    Term::Uniform(v2(x, y))
}
fn right_exits() -> Vec<Region> {
    vec![
        Region::exit(5.2, 3.0, 0.85, 'A'),
        Region::exit(5.2, 1.0, 0.85, 'B'),
        Region::exit(5.2, -1.0, 0.85, 'C'),
        Region::exit(5.2, -3.0, 0.85, 'D'),
    ]
}
fn ring_exits() -> Vec<Region> {
    // Corner exits: together they catch most of the boundary a spiral can reach.
    vec![
        Region::exit(5.6, 3.6, 2.2, 'A'),
        Region::exit(5.6, -3.6, 2.2, 'B'),
        Region::exit(-5.6, -3.6, 2.2, 'C'),
        Region::exit(-5.6, 3.6, 2.2, 'D'),
    ]
}
fn zr(x0: f64, y0: f64, x1: f64, y1: f64) -> Zone {
    Zone::Rect(Rect::new(x0, y0, x1, y1))
}
fn base(id: &'static str, chapter: Chapter, title: &'static str, objective: &'static str, field: Field, regions: Vec<Region>, rules: Rules) -> Level {
    Level { id, chapter, title, objective, hints: vec![], field, bounds: B, regions, rules, max_time: 40.0, seed: id.bytes().fold(7u64, |a, b| a * 31 + b as u64) }
}
fn hints(mut l: Level, h: &[&'static str]) -> Level {
    l.hints = h.to_vec();
    l
}
fn pos(slots: Vec<Slot>) -> Rules {
    Rules::Position { slots, period: None, intensity: None }
}
fn slot(zone: Zone, goal: usize) -> Slot {
    Slot { zone, goal }
}
fn pred(zone: Zone, reveal: Reveal) -> Rules {
    Rules::Predict { zone, rounds: 3, pass: 2, reveal }
}

// Periodic helpers for Tempo (period 12 s)
const T12: f64 = 12.0;
fn square_up(_p: V2, t: f64) -> V2 {
    v2(0.0, (4.0 * (TAU * t / T12).sin()).tanh())
}

// Leverage field for Steer level 2: a rightward stream whose vertical divergence is strong around
// x = 0 (a small sideways offset entering that zone is amplified) and mildly convergent beyond it.
fn lever_field(p: V2, _t: f64) -> V2 {
    let dz = (-(p.x / 1.5).powi(2)).exp();
    let k = 1.3 * dz - 0.35 * (1.0 - dz) * (p.x > 0.0) as i32 as f64;
    v2(1.4, k * p.y)
}

// Survey tuning (see docs/09-survey.md and SURVEY_VALIDATION.md for how these were chosen).
const SURVEY_K: f64 = 1.6;
const SURVEY_SIGMA: f64 = 1.0;
/// How close the committed marker must be to count as found. 0.5 was the first guess and was never calibrated;
/// in play, six consecutive misses landed at 0.52, 0.55, 0.61, 0.66, 0.90 and 1.05 (people located the region but
/// not to 0.5). 1.0 (= sigma) was tried and rejected: the free scout alone then settles (-2.75, 2.25), which the level
/// must never allow. 0.75 keeps every authored position valid (see SURVEY_VALIDATION.md) and is the middle ground.
const SURVEY_TOL: f64 = 0.75;
/// Level 5 of Survey ("The Gyre"): the same source as level 3 (sigma 0.8, k 1.65, tolerance 0.6) in a background that
/// circulates. The charted flow is a counter-clockwise vortex about the centre of the arena (`GYRE_K`, core 1.5) with a
/// gentle inward drift (`GYRE_DRIFT`), so a scout does not fly straight: it spirals in, at about 0.9-1.1 (peak push 0.8
/// stays under it, nothing stalls). Where it will have been after five seconds is no longer where it started, so the
/// player has to choose a release point for where the scout will *pass*, and the charted-path preview shows exactly that.
/// The free scout is released at (-4, 0), flies down the left side and ends at (-0.9, -3.2).
/// The positions are in play order: a graze on the left (two weak arrows from the free scout), one far along the free
/// scout's own streamline (it ends 5 s short of it: a scout released *where the free one stopped* gets there), then quiet
/// ones that none of the free scout's ground reaches. Every one shows the free scout at most two arrows, is read by no one
/// scout alone, is solved by well-placed scouts, and no two are within twice the tolerance (tested).
const GYRE_K: f64 = 4.0;
const GYRE_SOFT: f64 = 1.5;
const GYRE_DRIFT: f64 = 0.6;
const SURVEY5_POOL: [(f64, f64); 6] = [(-3.0, 1.0), (2.5, -0.5), (0.5, 2.0), (1.0, -1.5), (-1.5, 2.5), (3.5, -1.5)];
/// Level 4 of Survey ("Two of Them"): the same source as level 3 (sigma 0.8, k 1.65, tolerance 0.6), two of it at a time,
/// in order of separation 4.5 (one above the other), 4.0 (diagonal), then side by side along the stream at 3.2, 2.8, 2.4
/// and 2.2. Every pair lies about 1.5-1.7 above or below the free scout's line, within its first reach, so the free
/// scout **always has a lead**: it comes within 1.6-2.1 of both sources and shows 2-5 arrows. (The first layout put
/// pairs anywhere in the arena: the free scout felt at most one source, and in three attempts neither, so the player
/// found the first blind and had nothing to follow to the second; "what should I do?".) Side by side, the arrows look
/// like one source in the middle and the stacked ghost fits the free trail (deviation about 0.2): the lead says
/// "something is there", and telling one from two is the level. Each pair is also at its own place, no two within twice
/// the tolerance of each other. (first, second).
const SURVEY4_K: f64 = 1.65;
const SURVEY4_SIGMA: f64 = 0.8;
const SURVEY4_TOL: f64 = 0.6;
const SURVEY4_POOL: [((f64, f64), (f64, f64)); 6] = [
    ((-3.7, 1.7), (-0.75, -1.7)),
    ((-0.3, 1.7), (-2.4, -1.7)),
    ((-0.3, 1.6), (-3.5, 1.6)),
    ((0.2, -1.6), (-2.6, -1.6)),
    ((0.3, 1.5), (-2.1, 1.5)),
    ((-1.9, -1.5), (-4.1, -1.5)),
];
/// Level 3 of Survey ("One Look Is a Line"): peak push 0.8 (k * sigma * e^-0.5), felt out to 1.87 at `PUSH_MIN`.
const SURVEY3_K: f64 = 1.65;
const SURVEY3_SIGMA: f64 = 0.8;
/// 0.75 sigma, as for level 1.
const SURVEY3_TOL: f64 = 0.6;
/// Hidden positions and kinds in play order: graze, head-on, quiet, graze, graze, quiet, so that the three attempts
/// it takes to finish the level (`SurveyGoal::Finds(3)`) are one of each situation and, as it happens, one of each kind.
/// There is one head-on only: it must sit 1.4-1.6 beyond the free trail's end, so a second one would be within the
/// tolerance of the first and a player could pass it by re-marking the earlier answer. No two positions are within
/// twice the tolerance of each other (tested).
/// The free scout flies y = 0 from x = -5 and ends at x = 0 (sensing radius 1.87). Graze sources sit about 1.5 from
/// that line or its end, head-on ones on it 1.4-1.6 beyond its end (one weak arrow, and the slowdown is too small to
/// read from the dashes), quiet ones more than 2.4 off it. Checked by `tests/survey.rs` (`level_three_*`).
const SURVEY3_POOL: [(f64, f64, SourceKind); 6] = [
    (-1.5, 1.6, SourceKind::Repeller),
    (1.55, 0.05, SourceKind::Attractor),
    (3.0, 3.0, SourceKind::SwirlCcw),
    (0.5, -1.6, SourceKind::SwirlCw),
    (0.4, 1.5, SourceKind::Repeller),
    (2.5, -3.3, SourceKind::Attractor),
];
/// Level 2 of Survey ("Which Way It Pushes"): peak push 0.79, tolerance 0.75 (sigma 1). The level is complete when a
/// repeller, an attractor and a swirl have each been found, and the next attempt is always of the first kind still
/// missing (`pick_index` in modes/survey.rs), so a miss gives another of the same kind. Each kind has two or three
/// positions, in the order they are used: repeller, repeller; attractor, attractor; swirl CCW, swirl CW, then a hard
/// swirl close to the free trail.
const SURVEY2_K: f64 = 1.3;
const SURVEY2_SIGMA: f64 = 1.0;
const SURVEY2_TOL: f64 = 0.75;
const SURVEY2_POOL: [(f64, f64, SourceKind); 7] = [
    (-2.5, 1.2, SourceKind::Repeller),
    (-1.5, -1.3, SourceKind::Attractor),
    (-3.0, 1.1, SourceKind::SwirlCcw),
    (-2.6, -0.9, SourceKind::Repeller),
    (-2.2, 1.4, SourceKind::Attractor),
    (-1.0, -1.2, SourceKind::SwirlCw),
    (-1.8, 0.4, SourceKind::SwirlCcw),
];
/// The six positions level 1 first shipped with, chosen by the trail-similarity validator at a time when the free scout
/// showed no arrows on any of them (see `survey1_legacy`, `validate_survey::report` and `tests/survey.rs`).
pub const SURVEY_LEGACY_POOL: [(f64, f64); 6] = [(-3.25, -2.25), (-2.75, 2.25), (-0.75, -2.25), (-0.25, 2.25), (3.25, -1.75), (2.25, 2.25)];
/// Seconds of trail a scout leaves.
const SURVEY_TRAIL: f64 = 5.0;
/// Level 1's hidden positions, in play order. The first two are **grazes** (about 2 from the free scout's path): it
/// feels the source and shows 5 arrows, so the arrows and rays are on screen from the first attempt, yet reading them
/// with error rarely settles it from the free scout alone (2-5% careful, 0% by eye) and a scout released well solves it
/// (92%). The other four are out of the free scout's reach (no arrows: a search). The level is complete after one find,
/// so a player who reads the first attempt well never meets the searches. No two positions are within twice the
/// tolerance (tested), so an earlier answer cannot be reused. The six positions the level first shipped with, chosen by
/// the older trail-similarity validator when the free scout showed no arrows on any of them, are `SURVEY_LEGACY_POOL`.
const SURVEY_POOL: [(f64, f64); 6] = [(-2.4, 2.0), (-1.5, -1.9), (-3.25, -2.25), (-0.25, 2.25), (3.25, -1.75), (2.25, 2.25)];

pub fn position_levels() -> Vec<Level> {
    vec![
        hints(base("pos-1", Chapter::Position, "Open Water", "Drop the probe anywhere in the zone so it reaches the target.",
            Field::new().fixed(uni(0.6, 0.0)).fixed(radial(4.0, 0.5, -2.4, 0.6)),
            vec![Region::target(4.0, 0.5, 0.6)],
            pos(vec![slot(zr(-5.5, -3.0, -3.8, 3.0), 0)])),
            &["The target pulls everything toward it. Arrows near it all point inward.", "Almost any start works here. Watch where the arrows lead."]),
        hints(base("pos-2", Chapter::Position, "Two Roads", "The vortex splits the flow. Find a start that ends at the target.",
            Field::new().fixed(uni(0.7, 0.0)).fixed(vortex(0.0, 0.0, -2.5, 0.8)),
            vec![Region::target(4.0, -2.0, 0.6)],
            pos(vec![slot(zr(-5.5, -3.0, -4.0, 3.0), 0)])),
            &["Clockwise vortex: above it the flow runs right, below it the flow runs left.", "Starts above and below the centre line take very different roads."]),
        hints(base("pos-3", Chapter::Position, "Saddle Point", "Only part of the zone can reach the target. Which part?",
            Field::new().fixed(Term::Saddle { c: v2(4.0, 0.0), k: 0.5 }),
            vec![Region::target(3.0, 2.5, 0.8)],
            pos(vec![slot(Zone::Circle(v2(-4.2, 0.0), 1.8), 0)])),
            &["Flow squeezes in along the horizontal axis and is flung out vertically.", "Probes above the axis are thrown up, those below are thrown down."]),
        hints(base("pos-4", Chapter::Position, "Mirror Mirror", "The field is symmetric. The goal is not. Avoid the hazard.",
            Field::new().fixed(uni(0.5, 0.0)).fixed(vortex(0.0, 1.6, 1.5, 0.7)).fixed(vortex(0.0, -1.6, -1.5, 0.7)),
            vec![Region::target(4.5, 1.2, 0.6), Region::hazard(4.5, -1.2, 0.6)],
            pos(vec![slot(zr(-5.5, -3.0, -4.0, 3.0), 0)])),
            &["The two vortices are mirror images, so a path and its mirror path look equally good.", "The mirror path of a winning start ends in the hazard."]),
        hints(base("pos-5", Chapter::Position, "The Needle", "A tiny target and a sensitive field. Aim carefully.",
            Field::new().fixed(Term::Saddle { c: v2(5.0, 0.0), k: 0.7 }),
            vec![Region::target(3.8, 1.6, 0.3)],
            pos(vec![slot(Zone::Circle(v2(-4.5, 0.0), 1.2), 0)])),
            &["Near a saddle, tiny differences in the start grow quickly.", "Use the arrow keys for fine adjustments of the release point."]),
    ]
}

pub fn predict_levels() -> Vec<Level> {
    vec![
        hints(base("pre-1", Chapter::Predict, "Straight Shot", "Which exit will the probe leave through?",
            Field::new().fixed(uni(1.0, 0.0)), right_exits(),
            pred(zr(-5.5, -3.0, -4.0, 3.0), Reveal::Hidden)),
            &["A uniform flow moves everything in one direction at one speed."]),
        hints(base("pre-2", Chapter::Predict, "Gentle Curve", "The flow bends. Follow it.",
            Field::new().fixed(uni(1.0, 0.0)).fixed(vortex(-1.0, -5.0, -4.0, 1.0)), right_exits(),
            pred(zr(-5.5, -3.0, -4.0, 3.0), Reveal::Hidden)),
            &["A distant vortex adds a steady sideways push that bends every path."]),
        hints(base("pre-3", Chapter::Predict, "Spiral Out", "Probes spiral around the centre. Where do they escape?",
            Field::new().fixed(vortex(0.0, 0.0, 2.0, 0.6)).fixed(radial(0.0, 0.0, 0.8, 0.6)), ring_exits(),
            pred(Zone::Circle(v2(0.0, 0.0), 2.2), Reveal::Hidden)),
            &["The vortex turns the probe while the outward push makes it spiral outward.", "The turning direction tells you which exit comes first."]),
        hints(base("pre-4", Chapter::Predict, "False Lead", "The arrows near the start point at one exit. Think further ahead.",
            Field::new().fixed(uni(0.9, 0.0)).fixed(vortex(1.0, 0.0, 3.0, 0.5)), right_exits(),
            pred(zr(-5.5, -3.0, -4.0, 3.0), Reveal::Hidden)),
            &["Do not trust the first arrows. A strong vortex waits halfway across.", "Where the probe meets the vortex decides which way it is carried."]),
        hints(base("pre-5", Chapter::Predict, "Crosswinds", "Several features act in turn. Extrapolate step by step.",
            Field::new().fixed(uni(0.06, 0.0)).fixed(vortex(-1.17, -0.58, 2.52, 0.6)).fixed(vortex(-0.86, -0.99, -1.34, 0.6)).fixed(radial(1.03, 0.55, 1.04, 0.6)),
            ring_exits(),
            pred(Zone::Circle(v2(0.0, 0.0), 2.8), Reveal::Hidden)),
            &["Break the path into stages: which feature does the probe meet first, and where does it leave that feature?", "A pair of opposite vortices near the centre acts like a jet. The repeller on the right sends what reaches it outward."]),
    ]
}

pub fn tempo_levels() -> Vec<Level> {
    let w = TAU / T12;
    let tempo = |start: V2, goal: usize| Rules::Tempo { start, period: T12, goal };
    vec![
        hints(base("tem-1", Chapter::Tempo, "Two Moods", "The field has two phases. Launch in the right one.",
            Field::new().fixed(uni(3.0, 0.0)).fixed(Term::Custom(square_up)),
            vec![Region::target(4.5, 3.0, 0.9)], tempo(v2(-4.5, 0.0), 0)),
            &["For half the cycle the field lifts the probe, for the other half it pushes it down.", "Watch the timeline while the field plays."]),
        hints(base("tem-2", Chapter::Tempo, "Slow Tide", "A broad window of good launch times.",
            Field::new().fixed(uni(2.5, 0.0)).fixed(Term::Modulated { inner: Box::new(uni(0.0, 1.0)), offset: 0.0, amp: 1.0, omega: w, phase: 0.0 }),
            vec![Region::target(4.5, 2.6, 0.9)], tempo(v2(-4.5, 0.0), 0)),
            &["The vertical push swings smoothly up and down. The probe feels the sum over its whole trip."]),
        hints(base("tem-3", Chapter::Tempo, "Knife Edge", "Only a narrow window works.",
            Field::new().fixed(uni(2.5, 0.0)).fixed(Term::Modulated { inner: Box::new(uni(0.0, 1.0)), offset: 0.0, amp: 1.0, omega: w, phase: 0.0 }),
            vec![Region::target(4.5, 0.0, 0.4)], tempo(v2(-4.5, 0.0), 0)),
            &["The probe must feel equal up and down pushes. That balance happens at very particular phases."]),
        hints(base("tem-4", Chapter::Tempo, "Double Beat", "The field repeats twice per cycle: several windows exist.",
            Field::new().fixed(uni(2.5, 0.0)).fixed(Term::Modulated { inner: Box::new(uni(0.0, 1.6)), offset: 0.0, amp: 1.0, omega: 2.0 * w, phase: 0.0 }),
            vec![Region::target(4.5, 2.0, 0.7)], tempo(v2(-4.5, 0.0), 0)),
            &["Two full swings fit in one cycle, so the same outcome can be reached at more than one phase."]),
        hints(base("tem-5", Chapter::Tempo, "Moving Vortex", "A vortex circles the centre. Time your pass.",
            Field::new().fixed(uni(2.5, 0.0)).fixed(Term::Orbiting { kind: SwirlKind::Vortex, c: v2(0.0, 0.0), orbit: 2.0, omega: w, phase: 0.0, k: 3.0, soft: 0.6 }),
            vec![Region::target(4.5, 2.0, 0.7)], tempo(v2(-4.5, 0.0), 0)),
            &["Where the vortex is when the probe passes decides whether it is lifted or dropped.", "The vortex travels one lap per cycle."]),
    ]
}

fn spiral() -> Field {
    Field::new().fixed(radial(0.0, 0.0, 1.2, 0.6)).tunable(vortex(0.0, 0.0, -1.0, 0.6))
}

pub fn experiment_levels() -> Vec<Level> {
    let ex = |start: V2, range: (f64, f64), challenge: Challenge| Rules::Experiment { start, range, challenge };
    vec![
        hints(base("exp-1", Chapter::Experiment, "Which Way?", "Learn how intensity moves the exit, then report the rule.",
            spiral(), ring_exits(),
            ex(v2(1.4, 0.0), (0.0, 2.0), Challenge::ExitShift { center: v2(0.0, 0.0) })),
            &["Run several intensities and keep the runs visible: compare where each one leaves.", "Look at the order of the exits as the number grows."]),
        hints(base("exp-2", Chapter::Experiment, "The Switch", "Find the intensity where the exit changes.",
            Field::new().fixed(uni(1.5, 0.0)).tunable(vortex(0.0, 0.0, 2.0, 0.8)),
            vec![Region::exit(4.0, 2.0, 2.0, 'A'), Region::exit(4.0, -2.0, 2.0, 'B')],
            ex(v2(-4.5, 0.5), (0.0, 3.0), Challenge::Threshold { from: 0, to: 1, tol: 0.15 })),
            &["Find one value for each exit, then narrow the gap between them like a search.", "Preserved runs show which side of the switch you are on."]),
        hints(base("exp-3", Chapter::Experiment, "Round Trip", "Does the probe ever come back to an earlier exit?",
            spiral(), ring_exits(),
            ex(v2(1.4, 0.0), (0.0, 8.0), Challenge::Return)),
            &["Sweep the whole range and note the exits in order.", "The exit keeps moving around the ring as the vortex strengthens."]),
        hints(base("exp-4", Chapter::Experiment, "Beyond the Rig", "The test rig stops at intensity 4. Predict what happens at 6.2.",
            spiral(), ring_exits(),
            ex(v2(1.4, 0.0), (0.0, 8.0), Challenge::Extrapolate { limit: 4.0, target: 6.2 })),
            &["You cannot test 6.2, so look for a rule. Measure how far each exit sits from the last one as intensity grows.", "The winding angle grows steadily with intensity. A full extra turn brings the probe back to the same exit."]),
        hints(base("exp-5", Chapter::Experiment, "The Window", "Find the interval that sends the probe through exit D.",
            spiral(), ring_exits(),
            ex(v2(1.4, 0.0), (0.0, 5.0), Challenge::Interval { exit: 3, tol: 0.2 })),
            &["Exit D is reached for a band of intensities, not a single value.", "Find both edges of the band."]),
    ]
}

fn steer(start: V2, beats: usize, budget: usize, strength: f64, goal: usize) -> Rules {
    Rules::Steer { start, beats, gust_budget: budget, gust_strength: strength, dirs: GustDirs::Eight, beat_time: 1.0, goal }
}

pub fn steer_levels() -> Vec<Level> {
    vec![
        hints(base("ste-1", Chapter::Steer, "First Gust", "Nudge the probe so it reaches the target. A gust changes its velocity for one beat.",
            Field::new().fixed(uni(1.6, 0.05)),
            vec![Region::target(2.9, 0.1, 0.5)],
            steer(v2(-5.0, -0.9), 5, 1, 1.0, 0)),
            &["Left alone, the probe drifts past just below the target.", "A gust adds to the velocity for one beat only. The field carries the probe on afterwards."]),
        hints(base("ste-2", Chapter::Steer, "Leverage", "The same gust matters more in some places than others. Find where.",
            Field::new().fixed(Term::Custom(lever_field)),
            vec![Region::target(3.2, -1.6, 0.5)],
            steer(v2(-5.0, 0.15), 6, 2, 0.35, 0)),
            &["Watch how the arrows spread apart around the middle of the field: offsets grow there.", "A small push before the spreading zone is magnified. The same push afterwards is not."]),
        hints(base("ste-3", Chapter::Steer, "Tipping Point", "A saddle splits the flow. Cross to the other side in time.",
            Field::new().fixed(uni(1.6, 0.0)).fixed(Term::Saddle { c: v2(-0.7, 0.0), k: 0.27 }),
            vec![Region::target(2.62, -1.41, 0.7)],
            steer(v2(-5.0, 0.3), 6, 1, 1.0, 0)),
            &["Above the centre line the flow is thrown up, below it down. The probe starts just above it.", "The dividing line is close now. Every beat the probe drifts further from it."]),
        hints(base("ste-4", Chapter::Steer, "The Long Way", "The flow circles the centre. Pushing straight at the target will not get you there.",
            Field::new().fixed(vortex(0.0, 0.0, 8.5, 0.8)).fixed(radial(0.0, 0.0, -0.45, 0.8)),
            vec![Region::target(3.3, 0.3, 0.5)],
            steer(v2(-2.1, 0.0), 7, 2, 1.0, 0)),
            &["The flow carries the probe around the centre, anticlockwise, and slowly inward.", "The target is on a wider orbit than the probe. Which way does the probe need to move to join it?"]),
        hints(base("ste-5", Chapter::Steer, "Set Up", "The first gust is for the second one. The target is small.",
            Field::new().fixed(uni(0.65, 0.0)).fixed(vortex(-1.9, 0.43, 1.41, 0.7)).fixed(vortex(-0.06, 0.6, -1.21, 0.7)).fixed(radial(1.71, -0.27, 1.37, 0.7)),
            vec![Region::target(0.82, 1.79, 0.3)],
            steer(v2(-5.0, 0.94), 8, 2, 1.0, 0)),
            &["A good first move changes what the second move can do. It may not look like progress.", "Do not just close the distance. Look for the flow that will do the work for you."]),
    ]
}

/// Level 1 of Survey with its original six positions, for the legacy validator only.
pub fn survey1_legacy() -> Level {
    let mut l = survey_levels().remove(0);
    if let Rules::Survey { pool, kinds, .. } = &mut l.rules {
        *pool = SURVEY_LEGACY_POOL.iter().map(|&(x, y)| v2(x, y)).collect();
        *kinds = vec![SourceKind::Repeller; pool.len()];
    }
    l
}

pub fn survey_levels() -> Vec<Level> {
    // Charted background: a steady stream to the right. The hidden source is not part of `field`.
    let mut l = base("sur-1", Chapter::Survey, "Something in the Stream",
        "Something hidden pushes the flow away from itself. Fly scouts, then mark where it is.",
        Field::new().fixed(uni(1.0, 0.0)),
        vec![],
        Rules::Survey { zone: zr(-5.5, -3.5, 5.5, 3.5), free: v2(-5.0, 0.0), scouts: 2, k: SURVEY_K, sigma: SURVEY_SIGMA, tol: SURVEY_TOL, rays: true, band: false, preview: false, pool: SURVEY_POOL.iter().map(|&(x, y)| v2(x, y)).collect(), kinds: vec![SourceKind::Repeller; SURVEY_POOL.len()], partners: vec![None; SURVEY_POOL.len()], goal: SurveyGoal::Finds(1) });
    l.max_time = SURVEY_TRAIL;
    let one = hints(l, &[
        "The grey arrows show the flow with nothing hidden. Where a scout's path bends away from them, something hidden is pushing it.",
        "The small arrows on a trail show that push, and they point away from the source. Extend them backwards in your head, or follow the faint lines: where they cross is the source.",
        "A scout only feels the source from about 2.4 away. If your first scouts felt nothing, release the next one nearer to where you suspect it.",
    ]);

    // One Look Is a Line: same repeller, narrower and weaker (peak push 0.8, below the stream speed so nothing
    // stalls), and the hidden positions are chosen so that the free scout never settles one. Graze: at the edge of
    // the free scout's reach, so it shows one or two weak arrows (a long thin wedge). Head-on: just beyond the end of
    // its trail, on its own line, so its arrows are collinear and say "on this line", not where. Quiet: out of reach,
    // so it shows nothing. The shaded band turns that silence into evidence.
    let mut two = base("sur-3", Chapter::Survey, "One Look Is a Line",
        "Something hidden pushes the flow. One scout may only tell you which line it is on. No guide lines now.",
        Field::new().fixed(uni(1.0, 0.0)),
        vec![],
        Rules::Survey { zone: zr(-5.5, -3.5, 5.5, 3.5), free: v2(-5.0, 0.0), scouts: 2, k: SURVEY3_K, sigma: SURVEY3_SIGMA, tol: SURVEY3_TOL, rays: false, band: true, preview: false, pool: SURVEY3_POOL.iter().map(|&(x, y, _)| v2(x, y)).collect(), kinds: SURVEY3_POOL.iter().map(|&(_, _, k)| k).collect(), partners: vec![None; SURVEY3_POOL.len()], goal: SurveyGoal::Finds(3) });
    two.max_time = SURVEY_TRAIL;
    // The hints for this level are chosen by what the free scout showed (see `survey_hint_texts` in app.rs);
    // these are the general ones, shown if that cannot be told.
    let two = hints(two, &[
        "Extend the small arrows: the source is somewhere along each of those lines (at right angles to them, if it is a swirl).",
        "To find where along the line, send a scout from a bit above or below it. Its arrows give a different line, and where the lines cross is the source.",
        "The shaded ground is where a source would already have been felt. It is not there.",
    ]);

    // Which Way It Pushes: the source may push away, pull in, or push sideways, and the marker must say which.
    // Peak push 0.79 (below the stream speed, so nothing stalls). All six sources are within reach of the free
    // scout and its type is readable from it alone: this is the level where the player learns the kinds.
    let mut three = base("sur-2", Chapter::Survey, "Which Way It Pushes",
        "Something hidden pushes the flow. It may push away, pull in, or push sideways. Say which, and where.",
        Field::new().fixed(uni(1.0, 0.0)),
        vec![],
        Rules::Survey { zone: zr(-5.5, -3.5, 5.5, 3.5), free: v2(-5.0, 0.0), scouts: 2, k: SURVEY2_K, sigma: SURVEY2_SIGMA, tol: SURVEY2_TOL, rays: true, band: false, preview: false,
            pool: SURVEY2_POOL.iter().map(|&(x, y, _)| v2(x, y)).collect(), kinds: SURVEY2_POOL.iter().map(|&(_, _, k)| k).collect(), partners: vec![None; SURVEY2_POOL.len()], goal: SurveyGoal::EachKind });
    three.max_time = SURVEY_TRAIL;
    let three = hints(three, &[
        "Look at the small arrows around the spot where the trails bend. Pointing away from it: a repeller. Pointing toward it: an attractor. Going around it: a swirl. The key under the field shows all three.",
        "The faint lines run along each arrow. For a repeller or an attractor they cross at the source. A swirl's arrows point sideways, so those lines never cross: press P to switch to lines at right angles, which cross at a swirl's centre.",
        "Set the marker's kind with Y, then test it. A marker of the wrong kind never matches the dashes, even in the right place.",
    ]);
    // Two of Them: two identical repellers are hidden together and the player has two markers. Seen from beyond their
    // separation two sources look like one in the middle, so a marker pair stacked at the midpoint fits the trails of
    // scouts that pass to the side (to within 0.14 of the real trail on every side-by-side pair measured); only a
    // scout that flies along the line between them, or passes close to one, tells "one here" from "two flanking it".
    // The first two attempts have the sources one above the other or on a diagonal (the stacked pair does not fit),
    // the rest side by side. Each pair is solvable with three paid scouts and no two pairs are within twice the
    // tolerance of each other (tested). The level is complete after four finds, so it reaches separation 2.8. The
    // guide lines are on, as in the roadmap's description of the trap ("the rays converge there"): the first version
    // switched them off, and a player found it too hard. Rays from a scout beside one source meet at that source,
    // from far away at the midpoint.
    let mut four = base("sur-4", Chapter::Survey, "Two of Them",
        "Two sources are hidden, and from far away they can look like one. Mark both.",
        Field::new().fixed(uni(1.0, 0.0)),
        vec![],
        Rules::Survey { zone: zr(-5.5, -3.5, 5.5, 3.5), free: v2(-5.0, 0.0), scouts: 3, k: SURVEY4_K, sigma: SURVEY4_SIGMA, tol: SURVEY4_TOL, rays: true, band: false, preview: false,
            pool: SURVEY4_POOL.iter().map(|&(a, _)| v2(a.0, a.1)).collect(), kinds: vec![SourceKind::Repeller; SURVEY4_POOL.len()],
            partners: SURVEY4_POOL.iter().map(|&(_, b)| Some(v2(b.0, b.1))).collect(), goal: SurveyGoal::Finds(4) });
    four.max_time = SURVEY_TRAIL;
    let four = hints(four, &[
        "From far away two sources can look like one in the middle. If both markers sitting on one spot fit the dashes, that may be what you are seeing.",
        "A scout that passes close to one source feels it almost alone, and one that flies along the line between them feels both in turn. Release one there.",
        "Test your markers. If no pair of markers on one spot matches the dashes, pull them apart until it does.",
    ]);
    // The Gyre: the charted background circulates (a counter-clockwise vortex with a slow inward drift), so a scout curves
    // as it flies and the question becomes where to release it so that its path will pass the source. One repeller, the
    // level 3 source; the band is back (one source, so silence is evidence again) and so is the free scout's lead only as
    // a graze, and a charted-path preview follows the cursor while aiming. No guide lines: reading the arrows is level 3.
    let mut five = base("sur-5", Chapter::Survey, "The Gyre",
        "The flow circulates, so a scout curves as it flies. Something hidden pushes it. Release scouts where their paths will pass it.",
        Field::new().fixed(vortex(0.0, 0.0, GYRE_K, GYRE_SOFT)).fixed(radial(0.0, 0.0, -GYRE_DRIFT, GYRE_SOFT)),
        vec![],
        Rules::Survey { zone: zr(-5.5, -3.5, 5.5, 3.5), free: v2(-4.0, 0.0), scouts: 2, k: SURVEY3_K, sigma: SURVEY3_SIGMA, tol: SURVEY3_TOL, rays: false, band: true, preview: true,
            pool: SURVEY5_POOL.iter().map(|&(x, y)| v2(x, y)).collect(), kinds: vec![SourceKind::Repeller; SURVEY5_POOL.len()], partners: vec![None; SURVEY5_POOL.len()], goal: SurveyGoal::Finds(3) });
    five.max_time = SURVEY_TRAIL;
    let five = hints(five, &[
        "The pale line that follows your cursor is where a scout released there would fly with nothing hidden. A scout only feels the source if that line passes within about 2 of it.",
        "The shaded ground is where a source would already have been felt. Release the next scout so its pale line crosses ground that is not shaded: the part of a line the free scout already flew will show you nothing new.",
        "The flow carries every scout round the same way, and the free scout stopped after five seconds. Its pale line from where it stopped shows where the flow takes a scout next.",
    ]);
    // The order is the teaching order: what the kinds look like (sur-2), where to send scouts (sur-3), then two sources
    // at once (sur-4), then a background that curves the scouts' paths (sur-5).
    vec![one, three, two, four, five]
}

pub fn combo_levels() -> Vec<Level> {
    let w = TAU / T12;
    vec![
        hints(base("cmb-1", Chapter::Combo, "Through the Gate", "Position + corridor: pass the gate, then reach the target.",
            Field::new().fixed(uni(0.7, 0.0)).fixed(vortex(0.0, 0.0, -2.5, 0.8)),
            vec![Region::target(4.0, -2.0, 0.6), Region::gate(0.0, 2.0, 0.4)],
            pos(vec![slot(zr(-5.5, -3.0, -4.0, 3.0), 0)])),
            &["The gate sits on a route you already know from Two Roads."]),
        hints(base("cmb-2", Chapter::Combo, "Early Clue", "Predict + partial visibility: use the revealed opening of the path.",
            Field::new().fixed(uni(0.9, 0.0)).fixed(vortex(1.0, 0.0, 3.0, 0.5)), right_exits(),
            pred(zr(-5.5, -3.0, -4.0, 3.0), Reveal::Partial(2.5))),
            &["The first seconds are shown. Extrapolate from the heading it has when it ends."]),
        hints(base("cmb-3", Chapter::Combo, "Dodge", "Tempo + constraint: reach the target without touching the hazard.",
            Field::new().fixed(uni(2.5, 0.0)).fixed(Term::Modulated { inner: Box::new(uni(0.0, 1.0)), offset: 0.0, amp: 1.0, omega: w, phase: 0.0 }),
            vec![Region::target(4.5, 2.6, 0.9), Region::hazard(0.5, 0.8, 0.7)],
            Rules::Tempo { start: v2(-4.5, 0.0), period: T12, goal: 0 }),
            &["Some launch times lift the probe straight through the hazard. Later ones pass above it."]),
        hints(base("cmb-4", Chapter::Combo, "Three Exits", "Experiment + multiple exits: find where exit C turns into exit D.",
            spiral(), ring_exits(),
            Rules::Experiment { start: v2(1.4, 0.0), range: (0.0, 3.0), challenge: Challenge::Threshold { from: 2, to: 3, tol: 0.15 } }),
            &["Both exits must be seen before you can report the change."]),
        hints(base("cmb-5", Chapter::Combo, "Right Place, Right Time", "Position + Tempo: choose where and when.",
            Field::new().fixed(uni(2.5, 0.0)).fixed(Term::Orbiting { kind: SwirlKind::Vortex, c: v2(0.0, 0.0), orbit: 2.0, omega: w, phase: 0.0, k: 3.0, soft: 0.6 }),
            vec![Region::target(4.5, 2.0, 0.7)],
            Rules::Position { slots: vec![slot(zr(-5.5, -1.0, -4.0, 1.0), 0)], period: Some(T12), intensity: None }),
            &["Both choices matter: a good place may need a different time than a poor one."]),
        hints(base("cmb-6", Chapter::Combo, "Shared Dial", "Two probes, one intensity: make both reach their targets.",
            Field::new().fixed(uni(0.75, 0.0)).tunable(vortex(-0.63, -0.94, 2.7, 0.8)),
            vec![Region::target(4.0, 3.02, 0.6), Region::target(4.0, -2.13, 0.6)],
            Rules::Position { slots: vec![slot(zr(-5.5, 0.3, -4.0, 1.33), 0), slot(zr(-5.5, -1.33, -4.0, -0.3), 1)], period: None, intensity: Some((0.0, 3.0)) }),
            &["One dial bends both paths at once. A setting that suits the top probe can strand the bottom one.", "Try a dial value first, then look for a release point in each zone."]),
    ]
}

pub fn all() -> Vec<Level> {
    let mut v = position_levels();
    v.extend(predict_levels());
    v.extend(tempo_levels());
    v.extend(experiment_levels());
    v.extend(steer_levels());
    v.extend(survey_levels());
    v.extend(combo_levels());
    v
}

pub fn chapter(c: Chapter) -> Vec<Level> {
    all().into_iter().filter(|l| l.chapter == c).collect()
}
