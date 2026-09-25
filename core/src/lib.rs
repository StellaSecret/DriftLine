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

pub type FieldFn = fn(f64, f64, f64) -> Vector2;

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

#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub title: &'static str,
    pub desc: &'static str,
    pub field: FieldFn,
    pub k_min: f64,
    pub k_max: f64,
    pub k_def: f64,
    pub recommended_step: f64,
    pub a: Point,
    pub b: Point,
    pub obstacles: &'static [Obstacle],
}

fn field1(_x: f64, _y: f64, k: f64) -> Vector2 {
    Vector2 { x: 1.0, y: k }
}

fn field2(_x: f64, y: f64, k: f64) -> Vector2 {
    Vector2 { x: 1.0, y: k - y }
}

fn field3(x: f64, _y: f64, k: f64) -> Vector2 {
    Vector2 {
        x: 1.0,
        y: x.sin() + k * 0.5,
    }
}

fn field4_vortex(x: f64, y: f64, k: f64) -> Vector2 {
    Vector2 {
        x: 1.0 + k * 0.12 - y * 0.18,
        y: x * 0.18,
    }
}

pub fn levels() -> Vec<Level> {
    vec![
        Level {
            title: "Courant calme",
            desc: "Un courant régulier traverse la zone. Règle son intensité pour que la sonde dérive jusqu'à la balise.",
            field: field1,
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            recommended_step: 0.05,
            a: (-5.0, 0.0),
            b: (4.0, 1.0),
            obstacles: &[],
        },
        Level {
            title: "Zone de retenue",
            desc: "Le courant ramène toujours la sonde vers une hauteur d'équilibre. Contourne l'astéroïde central.",
            field: field2,
            k_min: -3.0,
            k_max: 3.0,
            k_def: 0.0,
            recommended_step: 0.05,
            a: (-5.0, 0.0),
            b: (4.0, 1.0),
            obstacles: &[Obstacle {
                x: 0.0,
                y: 2.5,
                r: 0.9,
            }],
        },
        Level {
            title: "Houle",
            desc: "Le courant ondule sur toute la zone. Slalome entre les deux astéroïdes.",
            field: field3,
            k_min: -3.0,
            k_max: 3.0,
            k_def: 0.0,
            recommended_step: 0.05,
            a: (-5.0, 0.0),
            b: (5.0, 1.0),
            obstacles: &[
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
        },
        Level {
            title: "Tourbillon",
            desc: "Un courant tourbillonne autour de son axe. Vise juste, car de petites réglages changent beaucoup la trajectoire.",
            field: field4_vortex,
            k_min: -3.0,
            k_max: 3.0,
            k_def: 0.0,
            recommended_step: 0.01,
            a: (-5.0, 3.0),
            b: (-1.25, 0.7),
            obstacles: &[
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
        },
    ]
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
    let k1 = (level.field)(x, y, k);
    let k2 = (level.field)(x + dt * 0.5, y + dt * 0.5 * k1.y, k);
    let k3 = (level.field)(x + dt * 0.5, y + dt * 0.5 * k2.y, k);
    let k4 = (level.field)(x + dt, y + dt * k3.y, k);
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
    fn same_field_gives_same_path() {
        let level = &levels()[2];
        assert_eq!(integrate(level, 0.2), integrate(level, 0.2));
    }

    #[test]
    fn collision_is_detected_between_sample_points() {
        let level = Level {
            title: "Test",
            desc: "",
            field: field1,
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            recommended_step: 0.01,
            a: (0.0, 0.0),
            b: (5.0, 5.0),
            obstacles: &[Obstacle {
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
            field: field1,
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            recommended_step: 0.01,
            a: (0.0, 0.0),
            b: (5.0, 5.0),
            obstacles: &[Obstacle {
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
        let level = &levels()[0];
        let point = rk4(level, (0.0, 0.0), 0.5, 0.1);
        assert!((point.0 - 0.1).abs() < 1e-12);
        assert!((point.1 - 0.05).abs() < 1e-12);
    }

    #[test]
    fn boundary_exit_is_calculated_on_the_segment() {
        let exit = segment_boundary_exit((5.0, 0.0), (7.0, 0.0));
        assert!((exit.unwrap() - 0.5).abs() < 1e-12);
    }
}
