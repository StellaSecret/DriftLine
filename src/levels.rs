//! All content, as data. Field types, constraints and mechanics are combined here and nowhere else.
use crate::field::{v2, Field, SwirlKind, Term, V2};
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
/// Seconds of trail a scout leaves.
const SURVEY_TRAIL: f64 = 5.0;
/// Authored hidden positions (off the validator's 0.5 candidate grid on purpose), **in the order a player meets them**:
/// easy to hard, by how much the free trail already narrows the search (candidates left after it, from
/// `SURVEY_VALIDATION.md`: 27, 29, 105, 113, 130, 130). The last, (2.25, 2.25), sits in the patch where the free trail
/// sees nothing and even the adaptive player needs both scouts, so it comes last. A test keeps this order honest.
/// Chosen from positions that `validate_survey` accepts (solvable at both precisions, still ambiguous after the
/// free scout, adaptive play wins).
const SURVEY_POOL: [(f64, f64); 6] = [(-3.25, -2.25), (-2.75, 2.25), (-0.75, -2.25), (-0.25, 2.25), (3.25, -1.75), (2.25, 2.25)];

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

pub fn survey_levels() -> Vec<Level> {
    // Charted background: a steady stream to the right. The hidden source is not part of `field`.
    let mut l = base("sur-1", Chapter::Survey, "Something in the Stream",
        "Something hidden pushes the flow away from itself. Fly scouts, then mark where it is.",
        Field::new().fixed(uni(1.0, 0.0)),
        vec![],
        Rules::Survey { zone: zr(-5.5, -3.5, 5.5, 3.5), free: v2(-5.0, 0.0), scouts: 2, k: SURVEY_K, sigma: SURVEY_SIGMA, tol: SURVEY_TOL, rays: true, pool: SURVEY_POOL.iter().map(|&(x, y)| v2(x, y)).collect() });
    l.max_time = SURVEY_TRAIL;
    vec![hints(l, &[
        "The arrows show the flow as charted. Where a scout does something the arrows do not predict, the hidden source is acting on it.",
        "A scout that passes far from the source learns almost nothing: its trail only speaks for the places it went near.",
        "Two guesses for the source send a scout in different directions only from certain places. Put the next scout where they disagree.",
    ])]
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
