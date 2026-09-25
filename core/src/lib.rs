pub const XMIN: f64 = -6.0;
pub const XMAX: f64 = 6.0;
pub const YMIN: f64 = -4.0;
pub const YMAX: f64 = 4.0;
pub const WIN_R: f64 = 0.55;
pub const OBJ_R: f64 = 0.18;

const STEP: f64 = 0.02;
const MAX_STEPS: usize = 3_000;
const EPSILON: f64 = 1e-9;

pub type Point = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationMode {
    Mission,
    Exploration,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Vector2 {
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FlowField {
    Calm {
        drift_x: f64,
        baseline_y: f64,
        gain_y: f64,
    },
    Retention {
        drift_x: f64,
        center_y: f64,
        restoring: f64,
        gain_y: f64,
    },
    Waves {
        drift_x: f64,
        amplitude: f64,
        frequency: f64,
        phase: f64,
        gain_y: f64,
    },
    Vortex {
        center_x: f64,
        center_y: f64,
        drift_x: f64,
        gain_x: f64,
        rotation: f64,
        radial: f64,
    },
}

impl FlowField {
    pub fn evaluate(&self, x: f64, y: f64, k: f64) -> Vector2 {
        match self {
            Self::Calm {
                drift_x,
                baseline_y,
                gain_y,
            } => Vector2 {
                x: *drift_x,
                y: baseline_y + gain_y * k,
            },
            Self::Retention {
                drift_x,
                center_y,
                restoring,
                gain_y,
            } => Vector2 {
                x: *drift_x,
                y: gain_y * k + restoring * (center_y - y),
            },
            Self::Waves {
                drift_x,
                amplitude,
                frequency,
                phase,
                gain_y,
            } => Vector2 {
                x: *drift_x,
                y: amplitude * (frequency * x + phase).sin() + gain_y * k,
            },
            Self::Vortex {
                center_x,
                center_y,
                drift_x,
                gain_x,
                rotation,
                radial,
            } => Vector2 {
                x: drift_x + gain_x * k - radial * (y - center_y),
                y: rotation * (x - center_x),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub title: &'static str,
    pub desc: &'static str,
    pub field: FlowField,
    pub k_min: f64,
    pub k_max: f64,
    pub k_def: f64,
    pub recommended_step: f64,
    pub a: Point,
    pub b: Point,
    pub obstacles: Vec<Obstacle>,
}

impl Level {
    pub fn flow_at(&self, x: f64, y: f64, k: f64) -> Vector2 {
        self.field.evaluate(x, y, k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeKind {
    Calm,
    Retention,
    Waves,
    Vortex,
}

#[derive(Clone, Copy)]
struct ThemeSpec {
    kind: ThemeKind,
    title: &'static str,
    desc: &'static str,
    k_min: f64,
    k_max: f64,
    recommended_step: f64,
    obstacle_count: usize,
}

const THEMES: [ThemeSpec; 4] = [
    ThemeSpec {
        kind: ThemeKind::Calm,
        title: "Courant calme",
        desc: "Un courant régulier traverse la zone. Règle son intensité pour que la sonde dérive jusqu'à la balise.",
        k_min: -1.0,
        k_max: 1.0,
        recommended_step: 0.05,
        obstacle_count: 0,
    },
    ThemeSpec {
        kind: ThemeKind::Retention,
        title: "Zone de retenue",
        desc: "Le courant ramène toujours la sonde vers une hauteur d'équilibre. Contourne les obstacles.",
        k_min: -3.0,
        k_max: 3.0,
        recommended_step: 0.05,
        obstacle_count: 1,
    },
    ThemeSpec {
        kind: ThemeKind::Waves,
        title: "Houle",
        desc: "Le courant ondule sur toute la zone. Slalome entre les obstacles.",
        k_min: -3.0,
        k_max: 3.0,
        recommended_step: 0.05,
        obstacle_count: 2,
    },
    ThemeSpec {
        kind: ThemeKind::Vortex,
        title: "Tourbillon",
        desc: "Un courant tourbillonne autour de son axe. Vise juste, car de petites réglages changent beaucoup la trajectoire.",
        k_min: -3.0,
        k_max: 3.0,
        recommended_step: 0.01,
        obstacle_count: 2,
    },
];

pub const DEFAULT_SEED: u64 = 0xD1F7_1A2B_3C4D_5E6F;
const GENERATION_ATTEMPTS: usize = 96;
const GOLDEN_RATIO: u64 = 0x9E37_79B9_7F4A_7C15;
const FIELD_MARGIN: f64 = 0.35;
const ANCHOR_MARGIN: f64 = 0.35;
const OBSTACLE_MARGIN: f64 = 0.12;
const MIN_ANCHOR_DISTANCE: f64 = 4.5;

struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_RATIO);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.unit()
    }
}

fn canonical_field(kind: ThemeKind) -> FlowField {
    match kind {
        ThemeKind::Calm => FlowField::Calm {
            drift_x: 1.0,
            baseline_y: 0.0,
            gain_y: 1.0,
        },
        ThemeKind::Retention => FlowField::Retention {
            drift_x: 1.0,
            center_y: 0.0,
            restoring: 1.0,
            gain_y: 1.0,
        },
        ThemeKind::Waves => FlowField::Waves {
            drift_x: 1.0,
            amplitude: 1.0,
            frequency: 1.0,
            phase: 0.0,
            gain_y: 0.5,
        },
        ThemeKind::Vortex => FlowField::Vortex {
            center_x: 0.0,
            center_y: 0.0,
            drift_x: 1.0,
            gain_x: 0.12,
            rotation: 0.18,
            radial: 0.18,
        },
    }
}

fn canonical_level(theme: ThemeSpec) -> Level {
    let (a, b, obstacles) = match theme.kind {
        ThemeKind::Calm => ((-5.0, 0.0), (4.0, 1.0), Vec::new()),
        ThemeKind::Retention => (
            (-5.0, 0.0),
            (4.0, 1.0),
            vec![Obstacle {
                x: 0.0,
                y: 2.5,
                r: 0.9,
            }],
        ),
        ThemeKind::Waves => (
            (-5.0, 0.0),
            (5.0, 1.0),
            vec![
                Obstacle {
                    x: -1.0,
                    y: 2.2,
                    r: 0.7,
                },
                Obstacle {
                    x: 2.0,
                    y: -2.2,
                    r: 0.7,
                },
            ],
        ),
        ThemeKind::Vortex => (
            (-5.0, 3.0),
            (-1.25, 0.7),
            vec![
                Obstacle {
                    x: 0.0,
                    y: 0.0,
                    r: 1.0,
                },
                Obstacle {
                    x: 3.0,
                    y: 2.0,
                    r: 0.6,
                },
            ],
        ),
    };

    Level {
        title: theme.title,
        desc: theme.desc,
        field: canonical_field(theme.kind),
        k_min: theme.k_min,
        k_max: theme.k_max,
        k_def: 0.0,
        recommended_step: theme.recommended_step,
        a,
        b,
        obstacles,
    }
}

fn theme_seed(seed: u64, index: usize) -> u64 {
    seed ^ (index as u64 + 1).wrapping_mul(GOLDEN_RATIO)
}

fn random_field(kind: ThemeKind, rng: &mut SeededRng) -> FlowField {
    match kind {
        ThemeKind::Calm => FlowField::Calm {
            drift_x: rng.range(0.78, 1.18),
            baseline_y: rng.range(-0.25, 0.25),
            gain_y: rng.range(0.75, 1.25),
        },
        ThemeKind::Retention => FlowField::Retention {
            drift_x: rng.range(0.82, 1.15),
            center_y: rng.range(-0.9, 0.9),
            restoring: rng.range(0.7, 1.2),
            gain_y: rng.range(0.75, 1.2),
        },
        ThemeKind::Waves => FlowField::Waves {
            drift_x: rng.range(0.8, 1.18),
            amplitude: rng.range(0.5, 1.0),
            frequency: rng.range(0.7, 1.35),
            phase: rng.range(0.0, std::f64::consts::TAU),
            gain_y: rng.range(0.35, 0.75),
        },
        ThemeKind::Vortex => FlowField::Vortex {
            center_x: rng.range(-1.1, 1.1),
            center_y: rng.range(-0.9, 0.9),
            drift_x: rng.range(0.8, 1.15),
            gain_x: rng.range(0.08, 0.16),
            rotation: rng.range(0.13, 0.23),
            radial: rng.range(0.1, 0.22),
        },
    }
}

fn random_obstacles(
    theme: ThemeSpec,
    a: Point,
    b: Point,
    rng: &mut SeededRng,
) -> Option<Vec<Obstacle>> {
    let (min_radius, max_radius) = match theme.kind {
        ThemeKind::Calm => return Some(Vec::new()),
        ThemeKind::Retention => (0.65, 0.95),
        ThemeKind::Waves => (0.45, 0.75),
        ThemeKind::Vortex => (0.45, 0.85),
    };
    let mut obstacles = Vec::with_capacity(theme.obstacle_count);

    for _ in 0..theme.obstacle_count {
        let mut placed = None;
        for _ in 0..96 {
            let radius = rng.range(min_radius, max_radius);
            let x = rng.range(
                XMIN + radius + OBJ_R + OBSTACLE_MARGIN,
                XMAX - radius - OBJ_R - OBSTACLE_MARGIN,
            );
            let y = rng.range(
                YMIN + radius + OBJ_R + OBSTACLE_MARGIN,
                YMAX - radius - OBJ_R - OBSTACLE_MARGIN,
            );
            let obstacle = Obstacle { x, y, r: radius };
            let separated_from_anchors = point_distance((x, y), a) > radius + OBJ_R + FIELD_MARGIN
                && point_distance((x, y), b) > radius + OBJ_R + WIN_R + FIELD_MARGIN;
            let separated_from_obstacles = obstacles.iter().all(|other: &Obstacle| {
                point_distance((x, y), (other.x, other.y)) > radius + other.r + FIELD_MARGIN
            });
            if separated_from_anchors && separated_from_obstacles {
                placed = Some(obstacle);
                break;
            }
        }
        obstacles.push(placed?);
    }

    Some(obstacles)
}

fn point_distance(first: Point, second: Point) -> f64 {
    (first.0 - second.0).hypot(first.1 - second.1)
}

fn random_level(theme: ThemeSpec, rng: &mut SeededRng) -> Option<Level> {
    let a = match theme.kind {
        ThemeKind::Vortex => (rng.range(-5.2, -3.6), rng.range(-2.8, 2.8)),
        _ => (rng.range(-5.2, -3.6), rng.range(-2.8, 2.8)),
    };
    let b = match theme.kind {
        ThemeKind::Vortex => (rng.range(0.8, 4.8), rng.range(-2.8, 2.8)),
        _ => (rng.range(2.4, 5.2), rng.range(-2.8, 2.8)),
    };
    if point_distance(a, b) < MIN_ANCHOR_DISTANCE {
        return None;
    }
    let field = random_field(theme.kind, rng);
    let obstacles = random_obstacles(theme, a, b, rng)?;
    let level = Level {
        title: theme.title,
        desc: theme.desc,
        field,
        k_min: theme.k_min,
        k_max: theme.k_max,
        k_def: 0.0,
        recommended_step: theme.recommended_step,
        a,
        b,
        obstacles,
    };
    valid_geometry(&level).then_some(level)
}

fn valid_geometry(level: &Level) -> bool {
    if !point_in_field(level.a, ANCHOR_MARGIN)
        || !point_in_field(level.b, ANCHOR_MARGIN + WIN_R)
        || point_distance(level.a, level.b) < MIN_ANCHOR_DISTANCE
    {
        return false;
    }

    for obstacle in &level.obstacles {
        if !obstacle.r.is_finite()
            || obstacle.r <= 0.0
            || obstacle.x - obstacle.r - OBJ_R < XMIN
            || obstacle.x + obstacle.r + OBJ_R > XMAX
            || obstacle.y - obstacle.r - OBJ_R < YMIN
            || obstacle.y + obstacle.r + OBJ_R > YMAX
            || point_distance(level.a, (obstacle.x, obstacle.y))
                <= obstacle.r + OBJ_R + FIELD_MARGIN
            || point_distance(level.b, (obstacle.x, obstacle.y))
                <= obstacle.r + OBJ_R + WIN_R + FIELD_MARGIN
        {
            return false;
        }
    }

    level.obstacles.iter().enumerate().all(|(index, obstacle)| {
        level.obstacles[index + 1..].iter().all(|other| {
            point_distance((obstacle.x, obstacle.y), (other.x, other.y))
                > obstacle.r + other.r + FIELD_MARGIN
        })
    })
}

fn point_in_field(point: Point, margin: f64) -> bool {
    point.0.is_finite()
        && point.1.is_finite()
        && point.0 >= XMIN + margin
        && point.0 <= XMAX - margin
        && point.1 >= YMIN + margin
        && point.1 <= YMAX - margin
}

fn valid_field(level: &Level) -> bool {
    let x_samples = [XMIN, -3.0, 0.0, 3.0, XMAX];
    let y_samples = [YMIN, -1.5, 0.0, 1.5, YMAX];
    let k_samples = [level.k_min, 0.0, level.k_max];
    x_samples.iter().all(|x| {
        y_samples.iter().all(|y| {
            k_samples.iter().all(|k| {
                let vector = level.flow_at(*x, *y, *k);
                vector.x.is_finite() && vector.y.is_finite()
            })
        })
    })
}

fn is_winnable(level: &Level) -> bool {
    let step = level.recommended_step;
    let count = ((level.k_max - level.k_min) / step).round() as usize;
    (0..=count).any(|index| {
        let k = level.k_min + index as f64 * step;
        integrate_with_mode(level, k, SimulationMode::Mission).reached()
    })
}

fn generate_level(theme: ThemeSpec, seed: u64) -> Level {
    for attempt in 0..GENERATION_ATTEMPTS {
        let mut rng =
            SeededRng::new(seed.wrapping_add((attempt as u64).wrapping_mul(GOLDEN_RATIO)));
        if let Some(level) = random_level(theme, &mut rng) {
            if valid_field(&level) && is_winnable(&level) {
                return level;
            }
        }
    }
    canonical_level(theme)
}

pub fn generate_levels(seed: u64) -> Vec<Level> {
    THEMES
        .iter()
        .enumerate()
        .map(|(index, theme)| generate_level(*theme, theme_seed(seed, index)))
        .collect()
}

pub fn levels() -> Vec<Level> {
    generate_levels(DEFAULT_SEED)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Reached,
    Collision { obstacle: usize, point: Point },
    LeftField { point: Point },
    TimeLimit,
    NumericalFailure,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestApproach {
    pub point: Point,
    pub distance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimResult {
    pub points: Vec<Point>,
    pub outcome: Outcome,
    pub closest: Option<ClosestApproach>,
}

impl SimResult {
    pub fn reached(&self) -> bool {
        self.outcome == Outcome::Reached
    }

    pub fn collided(&self) -> bool {
        matches!(self.outcome, Outcome::Collision { .. })
    }
}

fn rk4(level: &Level, point: Point, k: f64, dt: f64) -> Point {
    let (x, y) = point;
    let k1 = level.flow_at(x, y, k);
    let k2 = level.flow_at(x + dt * 0.5, y + dt * 0.5 * k1.y, k);
    let k3 = level.flow_at(x + dt * 0.5, y + dt * 0.5 * k2.y, k);
    let k4 = level.flow_at(x + dt, y + dt * k3.y, k);
    (
        x + dt * (k1.x + 2.0 * k2.x + 2.0 * k3.x + k4.x) / 6.0,
        y + dt * (k1.y + 2.0 * k2.y + 2.0 * k3.y + k4.y) / 6.0,
    )
}

fn lerp_point(from: Point, to: Point, t: f64) -> Point {
    (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
}

fn segment_circle_intersection(from: Point, to: Point, center: Point, radius: f64) -> Option<f64> {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let fx = from.0 - center.0;
    let fy = from.1 - center.1;
    let a = dx * dx + dy * dy;
    let c = fx * fx + fy * fy - radius * radius;

    if c <= 0.0 {
        return Some(0.0);
    }
    if a <= EPSILON {
        return None;
    }

    let b = 2.0 * (fx * dx + fy * dy);
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }

    let root = discriminant.sqrt();
    let first = (-b - root) / (2.0 * a);
    let second = (-b + root) / (2.0 * a);
    [first, second]
        .into_iter()
        .filter(|t| (0.0..=1.0).contains(t))
        .min_by(|a, b| a.partial_cmp(b).unwrap())
}

fn segment_boundary_exit(from: Point, to: Point) -> Option<f64> {
    let mut exit = f64::INFINITY;
    let axes = [
        (from.0, to.0 - from.0, XMIN, XMAX),
        (from.1, to.1 - from.1, YMIN, YMAX),
    ];

    for (position, delta, min, max) in axes {
        if delta > EPSILON {
            exit = exit.min((max - position) / delta);
        } else if delta < -EPSILON {
            exit = exit.min((min - position) / delta);
        }
    }

    (exit.is_finite() && (0.0..=1.0).contains(&exit)).then_some(exit)
}

fn closest_on_segment(from: Point, to: Point, target: Point) -> ClosestApproach {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared <= EPSILON {
        0.0
    } else {
        (((target.0 - from.0) * dx + (target.1 - from.1) * dy) / length_squared).clamp(0.0, 1.0)
    };
    let point = lerp_point(from, to, t);
    let distance = (point.0 - target.0).hypot(point.1 - target.1);
    ClosestApproach { point, distance }
}

fn closer(current: ClosestApproach, candidate: ClosestApproach) -> ClosestApproach {
    if candidate.distance < current.distance {
        candidate
    } else {
        current
    }
}

pub fn integrate(level: &Level, k: f64) -> SimResult {
    integrate_with_mode(level, k, SimulationMode::Mission)
}

pub fn integrate_with_mode(level: &Level, k: f64, mode: SimulationMode) -> SimResult {
    let mut point = level.a;
    let mut points = vec![point];
    let mut closest = closest_on_segment(point, point, level.b);

    for _ in 0..MAX_STEPS {
        let next = rk4(level, point, k, STEP);
        if !next.0.is_finite() || !next.1.is_finite() {
            return SimResult {
                points,
                outcome: Outcome::NumericalFailure,
                closest: Some(closest),
            };
        }

        let mut event: Option<(f64, Outcome)> = segment_boundary_exit(point, next)
            .map(|t| {
                (
                    t,
                    Outcome::LeftField {
                        point: lerp_point(point, next, t),
                    },
                )
            })
            .filter(|(t, _)| *t <= 1.0);

        if mode == SimulationMode::Mission {
            if let Some(t) = segment_circle_intersection(point, next, level.b, WIN_R) {
                if event.as_ref().is_none_or(|(best, _)| t < *best) {
                    event = Some((t, Outcome::Reached));
                }
            }

            for (index, obstacle) in level.obstacles.iter().enumerate() {
                if let Some(t) = segment_circle_intersection(
                    point,
                    next,
                    (obstacle.x, obstacle.y),
                    obstacle.r + OBJ_R,
                ) {
                    if event.as_ref().is_none_or(|(best, _)| t < *best) {
                        event = Some((
                            t,
                            Outcome::Collision {
                                obstacle: index,
                                point: lerp_point(point, next, t),
                            },
                        ));
                    }
                }
            }
        }

        let travel_to = event
            .as_ref()
            .map(|(t, _)| lerp_point(point, next, *t))
            .unwrap_or(next);
        closest = closer(closest, closest_on_segment(point, travel_to, level.b));

        if let Some((t, outcome)) = event {
            if t > 0.0 {
                points.push(travel_to);
            }
            return SimResult {
                points,
                outcome,
                closest: Some(closest),
            };
        }

        point = next;
        points.push(point);
    }

    SimResult {
        points,
        outcome: Outcome::TimeLimit,
        closest: Some(closest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_winnable(level: &Level) -> bool {
        let steps = ((level.k_max - level.k_min) / 0.01).round() as usize;
        (0..=steps)
            .map(|step| level.k_min + step as f64 * 0.01)
            .any(|k| integrate(level, k).reached())
    }

    #[test]
    fn all_levels_load() {
        assert_eq!(levels().len(), 4);
    }

    #[test]
    fn all_levels_are_winnable() {
        for level in levels() {
            assert!(
                is_winnable(&level),
                "aucun réglage ne gagne : {}",
                level.title
            );
        }
    }

    #[test]
    fn generated_levels_are_reproducible() {
        assert_eq!(generate_levels(42), generate_levels(42));
    }

    #[test]
    fn different_seeds_change_the_variation() {
        assert_ne!(generate_levels(42), generate_levels(43));
    }

    #[test]
    fn generated_levels_stay_bounded_and_winnable() {
        for seed in 0..16 {
            for level in generate_levels(seed) {
                assert!(
                    valid_geometry(&level),
                    "géométrie invalide: {}",
                    level.title
                );
                assert!(valid_field(&level), "champ invalide: {}", level.title);
                assert!(is_winnable(&level), "niveau impossible: {}", level.title);
            }
        }
    }

    #[test]
    fn same_field_gives_same_path() {
        let level = &levels()[2];
        assert_eq!(integrate(level, 0.2), integrate(level, 0.2));
    }

    #[test]
    fn collision_is_detected_between_sample_points() {
        let level = Level {
            title: "Test",
            desc: "",
            field: FlowField::Calm {
                drift_x: 1.0,
                baseline_y: 0.0,
                gain_y: 1.0,
            },
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            recommended_step: 0.01,
            a: (0.0, 0.0),
            b: (5.0, 5.0),
            obstacles: vec![Obstacle {
                x: 0.01,
                y: 0.0,
                r: 0.1,
            }],
        };
        let result = integrate(&level, 0.0);
        assert!(matches!(
            result.outcome,
            Outcome::Collision { obstacle: 0, .. }
        ));
    }

    #[test]
    fn exploration_disables_goal_and_obstacles() {
        let level = Level {
            title: "Exploration",
            desc: "",
            field: FlowField::Calm {
                drift_x: 1.0,
                baseline_y: 0.0,
                gain_y: 1.0,
            },
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            recommended_step: 0.01,
            a: (0.0, 0.0),
            b: (5.0, 5.0),
            obstacles: vec![Obstacle {
                x: 0.01,
                y: 0.0,
                r: 0.1,
            }],
        };
        let result = integrate_with_mode(&level, 0.0, SimulationMode::Exploration);
        assert!(matches!(result.outcome, Outcome::LeftField { .. }));
        assert!(!result.reached());
        assert!(!result.collided());
    }

    #[test]
    fn closest_point_uses_the_whole_segment() {
        let closest = closest_on_segment((0.0, 0.0), (1.0, 0.0), (0.5, 0.2));
        assert_eq!(closest.point, (0.5, 0.0));
        assert!((closest.distance - 0.2).abs() < 1e-12);
    }

    #[test]
    fn segment_intersection_handles_crossing_miss_and_tangent() {
        let crossing = segment_circle_intersection((0.0, 0.0), (1.0, 0.0), (0.5, 0.0), 0.1);
        assert!((crossing.unwrap() - 0.4).abs() < 1e-12);
        assert!(segment_circle_intersection((0.0, 0.0), (1.0, 0.0), (0.5, 0.5), 0.1).is_none());
        let tangent = segment_circle_intersection((0.0, 0.0), (1.0, 0.0), (0.5, 1.0), 1.0);
        assert!((tangent.unwrap() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn rk4_preserves_a_constant_field() {
        let level = canonical_level(THEMES[0]);
        let point = rk4(&level, (0.0, 0.0), 0.5, 0.1);
        assert!((point.0 - 0.1).abs() < 1e-12);
        assert!((point.1 - 0.05).abs() < 1e-12);
    }

    #[test]
    fn boundary_exit_is_calculated_on_the_segment() {
        let exit = segment_boundary_exit((5.0, 0.0), (7.0, 0.0));
        assert!((exit.unwrap() - 0.5).abs() < 1e-12);
    }
}
