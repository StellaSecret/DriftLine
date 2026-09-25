pub const XMIN: f64 = -6.0;
pub const XMAX: f64 = 6.0;
pub const YMIN: f64 = -4.0;
pub const YMAX: f64 = 4.0;
pub const WIN_R: f64 = 0.55;
pub const OBJ_R: f64 = 0.18;
pub const LEVELS_PER_GROUP: usize = 5;

const STEP: f64 = 0.02;
const CHEAP_STEP: f64 = 0.05;
const CHEAP_STEPS: usize = 700;
const MAX_STEPS: usize = 3_000;
const EPSILON: f64 = 1e-9;
const COARSE_STEP_MAX: f64 = 0.5;
const WINDOW_PROBE_LIMIT: usize = 32;
const SPREAD_PROBES: usize = 49;
const SPREAD_LIMIT: f64 = 0.18;
const REFINE_MULTIPLE: f64 = 2.0;

pub type Point = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationMode {
    Laboratory,
    Exploration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mechanic {
    Steady,
    Vortex,
    Zones,
    Opposed,
    Sensitive,
    Beacons,
}

impl Mechanic {
    pub fn all() -> [Mechanic; 6] {
        [
            Mechanic::Steady,
            Mechanic::Vortex,
            Mechanic::Zones,
            Mechanic::Opposed,
            Mechanic::Sensitive,
            Mechanic::Beacons,
        ]
    }

    pub fn group_index(self) -> usize {
        Mechanic::all()
            .iter()
            .position(|mechanic| *mechanic == self)
            .unwrap_or(0)
    }

    pub fn title(self) -> &'static str {
        MECHANICS[self.group_index()].title
    }
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub y_min: f64,
    pub y_max: f64,
    pub drift_x: f64,
    pub base_y: f64,
    pub gain_y: f64,
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
    Bands {
        bands: Vec<Band>,
    },
    Vortex {
        center_x: f64,
        center_y: f64,
        drift_x: f64,
        gain_x: f64,
        rotation: f64,
        radial: f64,
    },
    Opposed {
        split_y: f64,
        drift_x: f64,
        vertical_push: f64,
        center_y: f64,
        restoring: f64,
        gain_y: f64,
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
            Self::Bands { bands } => {
                let clamped = y.clamp(bands[0].y_min, bands[bands.len() - 1].y_max);
                let band = bands
                    .iter()
                    .find(|band| clamped >= band.y_min && clamped <= band.y_max)
                    .unwrap_or(bands.last().unwrap());
                Vector2 {
                    x: band.drift_x,
                    y: band.base_y + band.gain_y * k,
                }
            }
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
            Self::Opposed {
                split_y,
                drift_x,
                vertical_push,
                center_y,
                restoring,
                gain_y,
            } => {
                let direction = if y < *split_y { -1.0 } else { 1.0 };
                Vector2 {
                    x: *drift_x,
                    y: direction * *vertical_push + gain_y * k + restoring * (center_y - y),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub title: &'static str,
    pub desc: &'static str,
    pub focus: &'static str,
    pub mechanic: Mechanic,
    pub step_index: usize,
    pub field: FlowField,
    pub k_min: f64,
    pub k_max: f64,
    pub k_def: f64,
    pub recommended_step: f64,
    pub exploration_attempts: usize,
    pub k_window: f64,
    pub a: Point,
    pub beacons: Vec<Point>,
    pub obstacles: Vec<Obstacle>,
}

impl Level {
    pub fn flow_at(&self, x: f64, y: f64, k: f64) -> Vector2 {
        self.field.evaluate(x, y, k)
    }

    pub fn group_index(&self) -> usize {
        self.mechanic.group_index()
    }

    pub fn global_index(&self) -> usize {
        self.group_index() * LEVELS_PER_GROUP + self.step_index + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Calm,
    Retention,
    Waves,
    Bands,
    Vortex,
    Opposed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LevelPlan {
    field: FieldKind,
    k_span: f64,
    step: f64,
    obstacles: usize,
    min_radius: f64,
    max_radius: f64,
    bands: usize,
    beacons: usize,
    gain: f64,
    k_window_steps: Option<f64>,
    exploration_attempts: usize,
    focus: &'static str,
}

impl LevelPlan {
    const fn new(field: FieldKind, k_span: f64, step: f64) -> Self {
        Self {
            field,
            k_span,
            step,
            obstacles: 0,
            min_radius: 0.0,
            max_radius: 0.0,
            bands: 2,
            beacons: 1,
            gain: 1.0,
            k_window_steps: None,
            exploration_attempts: 3,
            focus: "",
        }
    }

    const fn obstacles(mut self, count: usize, min_radius: f64, max_radius: f64) -> Self {
        self.obstacles = count;
        self.min_radius = min_radius;
        self.max_radius = max_radius;
        self
    }

    const fn bands(mut self, count: usize) -> Self {
        self.bands = count;
        self
    }

    const fn beacons(mut self, count: usize) -> Self {
        self.beacons = count;
        self
    }

    const fn gain(mut self, gain: f64) -> Self {
        self.gain = gain;
        self
    }

    const fn window(mut self, steps: f64) -> Self {
        self.k_window_steps = Some(steps);
        self
    }

    const fn attempts(mut self, count: usize) -> Self {
        self.exploration_attempts = count;
        self
    }

    const fn focus(mut self, focus: &'static str) -> Self {
        self.focus = focus;
        self
    }
}

struct MechanicSpec {
    mechanic: Mechanic,
    title: &'static str,
    desc: &'static str,
    plans: [LevelPlan; LEVELS_PER_GROUP],
}

const MECHANICS: [MechanicSpec; 6] = [
    MechanicSpec {
        mechanic: Mechanic::Steady,
        title: "Courants constants",
        desc: "Un courant régulier traverse la zone. Règle son intensité pour que la sonde dérive jusqu'à la balise.",
        plans: [
            LevelPlan::new(FieldKind::Calm, 1.0, 0.05)
                .attempts(3)
                .window(2.0)
                .focus("dérive pure"),
            LevelPlan::new(FieldKind::Calm, 1.5, 0.05)
                .attempts(3)
                .window(2.0)
                .focus("dérive inclinée"),
            LevelPlan::new(FieldKind::Retention, 1.0, 0.05)
                .obstacles(1, 0.5, 0.7)
                .attempts(4)
                .gain(2.0)
                .window(4.0)
                .focus("hauteur d'équilibre"),
            LevelPlan::new(FieldKind::Retention, 2.0, 0.02)
                .obstacles(2, 0.5, 0.8)
                .attempts(4)
                .gain(3.0)
                .window(6.0)
                .focus("équilibre mobile"),
            LevelPlan::new(FieldKind::Retention, 3.0, 0.02)
                .obstacles(2, 0.45, 0.75)
                .attempts(5)
                .gain(3.0)
                .window(6.0)
                .focus("contre-courant"),
        ],
    },
    MechanicSpec {
        mechanic: Mechanic::Vortex,
        title: "Courants tourbillonnants",
        desc: "Le courant tourbillonne autour de son axe. Vise juste, car de petites réglages changent beaucoup la trajectoire.",
        plans: [
            LevelPlan::new(FieldKind::Vortex, 1.5, 0.02)
                .obstacles(1, 0.5, 0.8)
                .attempts(3)
                .gain(5.0)
                .window(8.0)
                .focus("rotation simple"),
            LevelPlan::new(FieldKind::Vortex, 2.0, 0.02)
                .obstacles(1, 0.45, 0.75)
                .attempts(3)
                .gain(5.0)
                .window(8.0)
                .focus("arc décalé"),
            LevelPlan::new(FieldKind::Vortex, 2.5, 0.02)
                .obstacles(2, 0.45, 0.8)
                .attempts(4)
                .gain(5.0)
                .window(5.0)
                .focus("deux obstacles"),
            LevelPlan::new(FieldKind::Vortex, 3.0, 0.01)
                .obstacles(2, 0.4, 0.7)
                .attempts(4)
                .gain(8.0)
                .window(6.0)
                .focus("pas fin"),
            LevelPlan::new(FieldKind::Vortex, 3.0, 0.01)
                .obstacles(3, 0.35, 0.6)
                .attempts(5)
                .gain(8.0)
                .window(6.0)
                .focus("tourbillon serré"),
        ],
    },
    MechanicSpec {
        mechanic: Mechanic::Zones,
        title: "Plusieurs zones de courants",
        desc: "Le courant change de caractère selon la zone traversée. Anticipe le virage avant la séparation.",
        plans: [
            LevelPlan::new(FieldKind::Waves, 1.5, 0.05)
                .obstacles(1, 0.45, 0.7)
                .attempts(3)
                .window(2.0)
                .focus("houle douce"),
            LevelPlan::new(FieldKind::Waves, 2.0, 0.05)
                .obstacles(1, 0.45, 0.75)
                .attempts(3)
                .window(2.0)
                .focus("période courte"),
            LevelPlan::new(FieldKind::Bands, 1.5, 0.05)
                .bands(2)
                .obstacles(1, 0.45, 0.7)
                .attempts(4)
                .window(2.0)
                .focus("deux bandes"),
            LevelPlan::new(FieldKind::Bands, 2.0, 0.02)
                .bands(2)
                .obstacles(2, 0.4, 0.7)
                .attempts(4)
                .window(4.0)
                .focus("bandes décalées"),
            LevelPlan::new(FieldKind::Bands, 3.0, 0.02)
                .bands(3)
                .obstacles(2, 0.4, 0.7)
                .attempts(5)
                .window(6.0)
                .focus("trois bandes"),
        ],
    },
    MechanicSpec {
        mechanic: Mechanic::Opposed,
        title: "Courants opposés",
        desc: "Deux zones de courant tirent vers des directions opposées. Explore la hauteur d'équilibre qui fait franchir la séparation.",
        plans: [
            LevelPlan::new(FieldKind::Opposed, 1.5, 0.05)
                .obstacles(1, 0.45, 0.7)
                .attempts(3)
                .window(3.0)
                .focus("deux moitiés"),
            LevelPlan::new(FieldKind::Opposed, 2.0, 0.05)
                .obstacles(2, 0.4, 0.65)
                .attempts(3)
                .window(3.0)
                .focus("séparation haute"),
            LevelPlan::new(FieldKind::Opposed, 2.5, 0.02)
                .obstacles(2, 0.4, 0.7)
                .attempts(4)
                .window(3.0)
                .focus("séparation basse"),
            LevelPlan::new(FieldKind::Opposed, 3.0, 0.02)
                .obstacles(2, 0.4, 0.65)
                .attempts(4)
                .gain(3.0)
                .window(5.0)
                .focus("fort rappel"),
            LevelPlan::new(FieldKind::Opposed, 3.0, 0.01)
                .obstacles(3, 0.35, 0.6)
                .attempts(5)
                .gain(2.0)
                .window(3.0)
                .focus("équilibre instable"),
        ],
    },
    MechanicSpec {
        mechanic: Mechanic::Sensitive,
        title: "Zones très sensibles",
        desc: "Une seule poignée de réglages atteint la balise. Affine pas à pas et surveille la marge.",
        plans: [
            LevelPlan::new(FieldKind::Calm, 1.0, 0.01)
                .obstacles(1, 0.4, 0.6)
                .attempts(3)
                .gain(1.5)
                .window(3.0)
                .focus("marge large"),
            LevelPlan::new(FieldKind::Waves, 0.8, 0.01)
                .obstacles(1, 0.4, 0.6)
                .attempts(3)
                .gain(2.0)
                .window(4.0)
                .focus("marge moyenne"),
            LevelPlan::new(FieldKind::Bands, 0.6, 0.01)
                .bands(2)
                .obstacles(1, 0.35, 0.55)
                .attempts(4)
                .gain(2.5)
                .window(3.0)
                .focus("bandes serrées"),
            LevelPlan::new(FieldKind::Bands, 0.5, 0.01)
                .bands(3)
                .obstacles(2, 0.35, 0.55)
                .attempts(4)
                .gain(3.0)
                .window(2.0)
                .focus("bandes étroites"),
            LevelPlan::new(FieldKind::Opposed, 0.4, 0.01)
                .obstacles(2, 0.35, 0.55)
                .attempts(5)
                .gain(3.0)
                .window(2.0)
                .focus("marge extrême"),
        ],
    },
    MechanicSpec {
        mechanic: Mechanic::Beacons,
        title: "Plusieurs balises",
        desc: "La sonde doit toucher toutes les balises pendant un seul largage. Chaque réglage arbitre entre elles.",
        plans: [
            LevelPlan::new(FieldKind::Calm, 1.5, 0.05)
                .obstacles(1, 0.45, 0.7)
                .beacons(2)
                .attempts(4)
                .window(2.0)
                .focus("deux balises"),
            LevelPlan::new(FieldKind::Waves, 2.0, 0.05)
                .obstacles(1, 0.45, 0.7)
                .beacons(2)
                .attempts(4)
                .window(2.0)
                .focus("deux balises en houle"),
            LevelPlan::new(FieldKind::Vortex, 2.0, 0.02)
                .obstacles(1, 0.4, 0.65)
                .beacons(3)
                .attempts(5)
                .gain(5.0)
                .window(10.0)
                .focus("trois balises"),
            LevelPlan::new(FieldKind::Bands, 2.5, 0.02)
                .bands(2)
                .obstacles(2, 0.4, 0.65)
                .beacons(3)
                .attempts(5)
                .window(3.0)
                .focus("trois balises étroites"),
            LevelPlan::new(FieldKind::Opposed, 3.0, 0.02)
                .obstacles(2, 0.4, 0.65)
                .beacons(4)
                .attempts(6)
                .gain(3.0)
                .window(3.0)
                .focus("quatre balises"),
        ],
    },
];

pub fn mechanic_count() -> usize {
    MECHANICS.len()
}

pub const DEFAULT_SEED: u64 = 0xD1F7_1A2B_3C4D_5E6F;
const GENERATION_ATTEMPTS: usize = 32;
const RELAXED_WINDOW_SCALE: f64 = 2.0;
const RELAXED_GOLDEN: u64 = 0xC2B2_AE3D_27D4_EB4F;
const GOLDEN_RATIO: u64 = 0x9E37_79B9_7F4A_7C15;
const FIELD_MARGIN: f64 = 0.35;
const ANCHOR_MARGIN: f64 = 0.35;
const OBSTACLE_MARGIN: f64 = 0.12;
const MIN_BEACON_DISTANCE: f64 = 3.0;
const BEACON_SPACING: f64 = WIN_R * 2.2;
const BEACON_CANDIDATES: usize = 2;
const K_PROBES: usize = 8;
const PLACEMENT_ATTEMPTS: usize = 96;

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

fn canonical_field(plan: &LevelPlan) -> FlowField {
    let gain = plan.gain;
    match plan.field {
        FieldKind::Calm => FlowField::Calm {
            drift_x: 1.0,
            baseline_y: 0.0,
            gain_y: gain,
        },
        FieldKind::Retention => FlowField::Retention {
            drift_x: 1.0,
            center_y: 0.0,
            restoring: 1.0,
            gain_y: gain,
        },
        FieldKind::Waves => FlowField::Waves {
            drift_x: 1.0,
            amplitude: 1.0,
            frequency: 1.0,
            phase: 0.0,
            gain_y: 0.5 * gain,
        },
        FieldKind::Bands => FlowField::Bands {
            bands: canonical_bands(plan.bands, gain),
        },
        FieldKind::Vortex => FlowField::Vortex {
            center_x: 0.0,
            center_y: 0.0,
            drift_x: 1.0,
            gain_x: 0.12 * gain,
            rotation: 0.18,
            radial: 0.18,
        },
        FieldKind::Opposed => FlowField::Opposed {
            split_y: 0.0,
            drift_x: 1.0,
            vertical_push: 0.55,
            center_y: 0.0,
            restoring: 0.8,
            gain_y: 0.8 * gain,
        },
    }
}

fn canonical_bands(count: usize, gain: f64) -> Vec<Band> {
    let height = (YMAX - YMIN) / count as f64;
    (0..count)
        .map(|index| {
            let sign = if index % 2 == 0 { 1.0 } else { -1.0 };
            Band {
                y_min: YMIN + height * index as f64,
                y_max: YMIN + height * (index + 1) as f64,
                drift_x: 1.0,
                base_y: 0.0,
                gain_y: 0.8 * sign * gain,
            }
        })
        .collect()
}

fn shell_level(spec: &MechanicSpec, plan: &LevelPlan, field: FlowField) -> Level {
    Level {
        title: spec.title,
        desc: spec.desc,
        focus: plan.focus,
        mechanic: spec.mechanic,
        step_index: 0,
        field,
        k_min: -plan.k_span,
        k_max: plan.k_span,
        k_def: 0.0,
        recommended_step: plan.step,
        exploration_attempts: plan.exploration_attempts,
        k_window: 0.0,
        a: (-5.0, 0.0),
        beacons: Vec::new(),
        obstacles: Vec::new(),
    }
}

fn with_plan_index(mut level: Level, step_index: usize) -> Level {
    level.step_index = step_index;
    level
}

fn theme_seed(seed: u64, group: usize, step: usize) -> u64 {
    seed ^ ((group as u64 + 1) * 37 + step as u64 + 1).wrapping_mul(GOLDEN_RATIO)
}

fn random_field(plan: &LevelPlan, rng: &mut SeededRng) -> FlowField {
    let gain = plan.gain;
    match plan.field {
        FieldKind::Calm => FlowField::Calm {
            drift_x: rng.range(0.78, 1.18),
            baseline_y: rng.range(-0.25, 0.25),
            gain_y: rng.range(0.75, 1.25) * gain,
        },
        FieldKind::Retention => FlowField::Retention {
            drift_x: rng.range(0.82, 1.15),
            center_y: rng.range(-0.9, 0.9),
            restoring: rng.range(0.7, 1.2),
            gain_y: rng.range(0.75, 1.2) * gain,
        },
        FieldKind::Waves => FlowField::Waves {
            drift_x: rng.range(0.8, 1.18),
            amplitude: rng.range(0.5, 1.0),
            frequency: rng.range(0.7, 1.35),
            phase: rng.range(0.0, std::f64::consts::TAU),
            gain_y: rng.range(0.35, 0.75) * gain,
        },
        FieldKind::Bands => {
            let count = plan.bands;
            let height = (YMAX - YMIN) / count as f64;
            let bands = (0..count)
                .map(|index| {
                    let sign = if index % 2 == 0 { 1.0 } else { -1.0 };
                    Band {
                        y_min: YMIN + height * index as f64,
                        y_max: YMIN + height * (index + 1) as f64,
                        drift_x: rng.range(0.8, 1.18),
                        base_y: rng.range(-0.35, 0.35),
                        gain_y: rng.range(0.45, 0.95) * sign * gain,
                    }
                })
                .collect();
            FlowField::Bands { bands }
        }
        FieldKind::Vortex => FlowField::Vortex {
            center_x: rng.range(-1.1, 1.1),
            center_y: rng.range(-0.9, 0.9),
            drift_x: rng.range(0.8, 1.15),
            gain_x: rng.range(0.08, 0.16) * gain,
            rotation: rng.range(0.13, 0.23),
            radial: rng.range(0.1, 0.22),
        },
        FieldKind::Opposed => FlowField::Opposed {
            split_y: rng.range(-1.2, 1.2),
            drift_x: rng.range(0.82, 1.15),
            vertical_push: rng.range(0.45, 0.9),
            center_y: rng.range(-0.7, 0.7),
            restoring: rng.range(0.6, 1.0),
            gain_y: rng.range(0.65, 1.05) * gain,
        },
    }
}

fn random_obstacles(
    plan: &LevelPlan,
    a: Point,
    beacons: &[Point],
    rng: &mut SeededRng,
) -> Vec<Obstacle> {
    let mut obstacles = Vec::with_capacity(plan.obstacles);
    for _ in 0..plan.obstacles {
        let mut placed = None;
        for _ in 0..PLACEMENT_ATTEMPTS {
            let radius = rng.range(plan.min_radius, plan.max_radius);
            let x = rng.range(
                XMIN + radius + OBJ_R + OBSTACLE_MARGIN,
                XMAX - radius - OBJ_R - OBSTACLE_MARGIN,
            );
            let y = rng.range(
                YMIN + radius + OBJ_R + OBSTACLE_MARGIN,
                YMAX - radius - OBJ_R - OBSTACLE_MARGIN,
            );
            let obstacle = Obstacle { x, y, r: radius };
            let clear_of_anchors = point_distance((x, y), a) > radius + OBJ_R + FIELD_MARGIN
                && beacons.iter().all(|beacon| {
                    point_distance((x, y), *beacon) > radius + OBJ_R + WIN_R + FIELD_MARGIN
                });
            let clear_of_obstacles = obstacles.iter().all(|other: &Obstacle| {
                point_distance((x, y), (other.x, other.y)) > radius + other.r + FIELD_MARGIN
            });
            if clear_of_anchors && clear_of_obstacles {
                placed = Some(obstacle);
                break;
            }
        }
        if let Some(obstacle) = placed {
            obstacles.push(obstacle);
        }
    }
    obstacles
}

fn point_distance(first: Point, second: Point) -> f64 {
    (first.0 - second.0).hypot(first.1 - second.1)
}

fn clamped_index(value: f64, max_index: usize) -> usize {
    if max_index == 0 {
        0
    } else {
        (value.round() as isize).clamp(0, max_index as isize) as usize
    }
}

fn point_in_field(point: Point, margin: f64) -> bool {
    point.0.is_finite()
        && point.1.is_finite()
        && point.0 >= XMIN + margin
        && point.0 <= XMAX - margin
        && point.1 >= YMIN + margin
        && point.1 <= YMAX - margin
}

fn obstacle_free(level: &Level, point: Point) -> bool {
    level
        .obstacles
        .iter()
        .all(|obstacle| point_distance(point, (obstacle.x, obstacle.y)) > obstacle.r + OBJ_R)
}

fn valid_geometry(level: &Level) -> bool {
    if !point_in_field(level.a, ANCHOR_MARGIN)
        || level.beacons.is_empty()
        || !level
            .beacons
            .iter()
            .all(|beacon| point_in_field(*beacon, ANCHOR_MARGIN + WIN_R))
    {
        return false;
    }

    for (index, beacon) in level.beacons.iter().enumerate() {
        if point_distance(level.a, *beacon) < MIN_BEACON_DISTANCE
            || level.beacons[index + 1..]
                .iter()
                .any(|other| point_distance(*beacon, *other) < BEACON_SPACING)
        {
            return false;
        }
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
            || level.beacons.iter().any(|beacon| {
                point_distance(*beacon, (obstacle.x, obstacle.y))
                    <= obstacle.r + OBJ_R + WIN_R + FIELD_MARGIN
            })
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

fn coarse_step(level: &Level) -> f64 {
    (REFINE_MULTIPLE * 2.0 * level.recommended_step).clamp(0.05, COARSE_STEP_MAX)
}

fn grid_candidates(level: &Level, coarse: f64) -> Vec<f64> {
    let span = level.k_max - level.k_min;
    let count = (span / coarse).ceil().max(1.0) as usize;
    (0..=count)
        .map(|index| (level.k_min + index as f64 * coarse).min(level.k_max))
        .collect()
}

fn coarse_ranked(level: &Level, coarse: f64) -> Vec<f64> {
    let mut ranked: Vec<(f64, f64)> = grid_candidates(level, coarse)
        .into_iter()
        .map(|k| (coarse_distance(level, k), k))
        .collect();
    ranked.sort_by(|first, second| {
        first
            .0
            .partial_cmp(&second.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.into_iter().map(|(_, k)| k).collect()
}

fn refine_candidate(level: &Level, seed: f64, coarse: f64) -> Option<f64> {
    let step = level.recommended_step;
    let count = (2.0 * coarse / step).ceil().max(1.0) as usize;
    let lowest = (level.k_min / step).ceil() * step;
    let highest = (level.k_max / step).floor() * step;
    (0..=count)
        .map(|index| snap_to_grid(level, seed - coarse + index as f64 * step))
        .filter(|k| *k >= lowest - EPSILON && *k <= highest + EPSILON)
        .find(|k| integrate(level, *k).reached())
}

pub fn solve(level: &Level) -> Option<f64> {
    let coarse = coarse_step(level);
    coarse_ranked(level, coarse)
        .into_iter()
        .find_map(|k| refine_candidate(level, k, coarse))
}

fn snap_to_grid(level: &Level, k: f64) -> f64 {
    let step = level.recommended_step;
    let steps = ((k - level.k_min) / step).round();
    (level.k_min + steps * step).clamp(level.k_min, level.k_max)
}

fn k_window(level: &Level, winning_k: f64) -> f64 {
    let step = level.recommended_step;
    let mut total = 0.0;
    for direction in [1.0, -1.0] {
        for probe in 1..=WINDOW_PROBE_LIMIT {
            let candidate = winning_k + direction * probe as f64 * step;
            if candidate < level.k_min || candidate > level.k_max {
                break;
            }
            if !integrate(level, candidate).reached() {
                break;
            }
            total += step;
        }
    }
    total / 2.0
}

fn k_spread(level: &Level) -> f64 {
    let step = level.recommended_step;
    let count = ((level.k_max - level.k_min) / step).ceil().max(1.0) as usize;
    let probes = count.min(SPREAD_PROBES);
    let mut hits = 0;
    for probe in 0..probes {
        let k = snap_to_grid(level, level.k_min + (probe * count / probes) as f64 * step);
        if integrate(level, k).reached() {
            hits += 1;
        }
    }
    hits as f64 / probes as f64
}

fn trajectory(level: &Level, k: f64) -> Vec<Point> {
    integrate_with_mode(level, k, SimulationMode::Exploration).points
}

fn sharpened_beacons(level: &Level, k: f64, wanted: usize) -> Vec<Point> {
    let step = level.recommended_step;
    let mut probes = vec![0.0, k - step, k + step, k - 2.0 * step, k + 2.0 * step];
    probes.retain(|probe| *probe >= level.k_min - EPSILON && *probe <= level.k_max + EPSILON);
    if probes.is_empty() {
        return Vec::new();
    }
    let neighbours: Vec<Vec<Point>> = probes
        .iter()
        .map(|probe| trajectory(level, *probe))
        .collect();
    let middle = trajectory(level, k);
    if middle.len() < 8 {
        return Vec::new();
    }
    let samples = middle.len();
    let stride = (samples / 48).max(1);
    let mut ranked: Vec<(f64, Point)> = Vec::new();
    for index in (samples / 4..samples * 9 / 10).step_by(stride) {
        let mut worst = f64::INFINITY;
        for path in &neighbours {
            if index < path.len() {
                worst = worst.min(point_distance(middle[index], path[index]));
            }
        }
        if !worst.is_finite() {
            continue;
        }
        let point = middle[index];
        if point_in_field(point, ANCHOR_MARGIN + WIN_R)
            && point_distance(point, level.a) > 3.0
            && obstacle_free(level, point)
            && !ranked
                .iter()
                .any(|(_, current)| point_distance(*current, point) < WIN_R)
        {
            ranked.push((worst, point));
            ranked.sort_by(|first, second| {
                second
                    .0
                    .partial_cmp(&first.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            ranked.truncate(wanted);
        }
    }
    ranked.into_iter().map(|(_, point)| point).collect()
}

fn beacon_slot_clear(level: &Level, point: Point, taken: &[Point]) -> bool {
    point_in_field(point, ANCHOR_MARGIN + WIN_R)
        && point_distance(point, level.a) > MIN_BEACON_DISTANCE
        && taken
            .iter()
            .all(|other| point_distance(point, *other) > BEACON_SPACING)
        && obstacle_free(level, point)
}

fn sample_beacons(level: &Level, k: f64, count: usize) -> Option<Vec<Point>> {
    if count == 0 {
        return Some(Vec::new());
    }
    let mut open = level.clone();
    open.beacons.clear();
    let path = trajectory(&open, k);
    if path.len() < 4 {
        return None;
    }
    let mut placed: Vec<Point> = Vec::with_capacity(count);
    let mut taken: Vec<Point> = level.beacons.clone();
    for _ in 0..count {
        let mut best: Option<(f64, Point)> = None;
        for &point in path.iter() {
            if !beacon_slot_clear(level, point, &taken) {
                continue;
            }
            let clearance = taken
                .iter()
                .map(|other| point_distance(point, *other))
                .fold(f64::INFINITY, f64::min);
            if best.is_none_or(|(current, _)| clearance > current) {
                best = Some((clearance, point));
            }
        }
        placed.push(best?.1);
        taken.push(best?.1);
    }
    Some(placed)
}

const CANONICAL_OBSTACLES: [(f64, f64, f64); 3] =
    [(0.0, 2.2, 0.7), (2.2, -2.0, 0.65), (3.6, 1.6, 0.5)];

fn plan_obstacle_count(level: &Level) -> usize {
    MECHANICS[level.group_index()].plans[level.step_index].obstacles
}

fn augment_obstacles(mut level: Level, winning_k: f64) -> Level {
    if !level.obstacles.is_empty() {
        return level;
    }
    let limit = plan_obstacle_count(&level);
    for &(x, y, r) in CANONICAL_OBSTACLES.iter() {
        if level.obstacles.len() >= limit {
            break;
        }
        let clear = point_in_field((x, y), r + OBJ_R + OBSTACLE_MARGIN)
            && point_distance((x, y), level.a) > r + OBJ_R + FIELD_MARGIN
            && level
                .beacons
                .iter()
                .all(|beacon| point_distance(*beacon, (x, y)) > r + OBJ_R + WIN_R + FIELD_MARGIN)
            && level.obstacles.iter().all(|other| {
                point_distance((x, y), (other.x, other.y)) > r + other.r + FIELD_MARGIN
            });
        if !clear {
            continue;
        }
        level.obstacles.push(Obstacle { x, y, r });
        if !integrate(&level, winning_k).reached() {
            level.obstacles.pop();
        }
    }
    level
}

fn finish_level(
    mut level: Level,
    plan: &LevelPlan,
    winning_k: f64,
    window_scale: f64,
    rng: &mut SeededRng,
) -> Option<Level> {
    if !integrate(&level, winning_k).reached() {
        return None;
    }
    if plan.beacons > 1 {
        level
            .beacons
            .extend(sample_beacons(&level, winning_k, plan.beacons - 1)?);
    }
    level.obstacles = random_obstacles(plan, level.a, &level.beacons, rng);
    if !valid_geometry(&level) || !integrate(&level, winning_k).reached() {
        return None;
    }
    level = augment_obstacles(level, winning_k);
    if !integrate(&level, winning_k).reached() {
        return None;
    }
    level.k_window = k_window(&level, winning_k);
    if plan
        .k_window_steps
        .is_some_and(|steps| level.k_window > window_scale * steps * level.recommended_step)
    {
        return None;
    }
    if integrate(&level, 0.0).reached() || k_spread(&level) > SPREAD_LIMIT {
        return None;
    }
    Some(level)
}

fn random_level(
    spec: &MechanicSpec,
    plan: &LevelPlan,
    window_scale: f64,
    k_probes: usize,
    rng: &mut SeededRng,
) -> Option<Level> {
    let field = random_field(plan, rng);
    let a = (rng.range(-5.2, -4.0), rng.range(-2.6, 2.6));
    let mut base = shell_level(spec, plan, field);
    base.a = a;

    for (k, beacon) in sharpened_candidates(&base, k_probes) {
        let mut level = base.clone();
        level.beacons = vec![beacon];
        if let Some(next) = finish_level(level, plan, k, window_scale, rng) {
            return Some(next);
        }
    }
    None
}

fn sharpened_candidates(level: &Level, k_probes: usize) -> Vec<(f64, Point)> {
    let mut candidates = Vec::new();
    for probe in 0..k_probes {
        let k = snap_to_grid(
            level,
            level.k_min + (level.k_max - level.k_min) * (probe + 1) as f64 / (k_probes + 1) as f64,
        );
        if k.abs() < EPSILON {
            continue;
        }
        for beacon in sharpened_beacons(level, k, BEACON_CANDIDATES) {
            candidates.push((k, beacon));
        }
    }
    candidates
}

fn canonical_shell(spec: &MechanicSpec, plan: &LevelPlan, step_index: usize) -> Level {
    with_plan_index(shell_level(spec, plan, canonical_field(plan)), step_index)
}

fn canonical_level(spec: &MechanicSpec, plan: &LevelPlan, step_index: usize) -> Level {
    let shell = canonical_shell(spec, plan, step_index);

    let mut rng = SeededRng::new(RELAXED_GOLDEN);
    for (k, beacon) in sharpened_candidates(&shell, K_PROBES) {
        let mut level = shell.clone();
        level.beacons = vec![beacon];
        if let Some(next) = finish_level(level, plan, k, RELAXED_WINDOW_SCALE, &mut rng) {
            return next;
        }
    }

    for (k, beacon) in trajectory_level_beacons(&shell) {
        let mut level = shell.clone();
        level.beacons = vec![beacon];
        if let Some(next) = finish_level(level, plan, k, RELAXED_WINDOW_SCALE, &mut rng) {
            return next;
        }
    }

    FALLBACKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut level = shell;
    level.beacons = vec![(4.0, 0.0)];
    level
}

pub static FALLBACKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

const TRAJECTORY_K_PROBES: usize = 15;
const TRAJECTORY_FRACTIONS: [f64; 6] = [0.7, 0.55, 0.8, 0.45, 0.62, 0.9];

fn trajectory_level_beacons(level: &Level) -> Vec<(f64, Point)> {
    let mut candidates = Vec::new();
    for probe in 0..=TRAJECTORY_K_PROBES {
        let k = snap_to_grid(
            level,
            level.k_min + (level.k_max - level.k_min) * probe as f64 / TRAJECTORY_K_PROBES as f64,
        );
        if k.abs() < EPSILON {
            continue;
        }
        let path = trajectory(level, k);
        if path.len() < 8 {
            continue;
        }
        for fraction in TRAJECTORY_FRACTIONS {
            let beacon = path[clamped_index(fraction * (path.len() - 1) as f64, path.len() - 1)];
            if point_in_field(beacon, ANCHOR_MARGIN + WIN_R)
                && point_distance(beacon, level.a) > MIN_BEACON_DISTANCE
            {
                candidates.push((k, beacon));
            }
        }
    }
    candidates
}
fn generate_level(spec: &MechanicSpec, step_index: usize, seed: u64) -> Level {
    let plan = spec.plans[step_index];
    for (pass, window_scale, attempts, k_probes) in [
        (0u64, 1.0, GENERATION_ATTEMPTS, K_PROBES),
        (1, 1.0, GENERATION_ATTEMPTS, K_PROBES * 2),
        (2, RELAXED_WINDOW_SCALE, GENERATION_ATTEMPTS / 4, K_PROBES),
    ] {
        for attempt in 0..attempts {
            let mut rng = SeededRng::new(
                seed.wrapping_add(
                    (attempt as u64 + 1)
                        .wrapping_mul(GOLDEN_RATIO)
                        .wrapping_add(pass.wrapping_mul(RELAXED_GOLDEN)),
                ),
            );
            if let Some(level) = random_level(spec, &plan, window_scale, k_probes, &mut rng) {
                if valid_field(&level) {
                    return with_plan_index(level, step_index);
                }
            }
        }
    }
    canonical_level(spec, &plan, step_index)
}

pub fn generate_level_group(seed: u64, group: usize) -> Vec<Level> {
    let spec = &MECHANICS[group.min(MECHANICS.len() - 1)];
    (0..LEVELS_PER_GROUP)
        .map(|step_index| generate_level(spec, step_index, theme_seed(seed, group, step_index)))
        .collect()
}

pub fn generate_levels(seed: u64) -> Vec<Level> {
    (0..MECHANICS.len())
        .flat_map(|group| generate_level_group(seed, group))
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
    pub visited: usize,
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

fn integrate_advanced(level: &Level, k: f64, _mode: SimulationMode, cheap: bool) -> SimResult {
    let (step, max_steps) = if cheap {
        (CHEAP_STEP, CHEAP_STEPS)
    } else {
        (STEP, MAX_STEPS)
    };
    let mut point = level.a;
    let mut points = vec![point];
    let mut visited = vec![false; level.beacons.len()];
    let mut visited_count = 0usize;
    let mut target = level.beacons.first().copied().unwrap_or(point);
    let mut closest = closest_on_segment(point, point, target);

    for _ in 0..max_steps {
        let next = rk4(level, point, k, step);
        if !next.0.is_finite() || !next.1.is_finite() {
            return SimResult {
                points,
                outcome: Outcome::NumericalFailure,
                closest: Some(closest),
                visited: visited_count,
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

        let mut hits: Vec<(usize, f64)> = Vec::new();
        for (index, beacon) in level.beacons.iter().enumerate() {
            if visited[index] {
                continue;
            }
            if let Some(t) = segment_circle_intersection(point, next, *beacon, WIN_R) {
                hits.push((index, t));
            }
        }
        let complete =
            !level.beacons.is_empty() && hits.len() == level.beacons.len() - visited_count;
        for (index, _) in hits.iter() {
            visited[*index] = true;
        }
        visited_count += hits.len();
        if complete && visited_count == level.beacons.len() {
            let t = hits.iter().map(|(_, t)| *t).fold(0.0_f64, f64::max);
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

        let travel_to = event
            .as_ref()
            .map(|(t, _)| lerp_point(point, next, *t))
            .unwrap_or(next);
        if let Some(index) = visited.iter().position(|done| !done) {
            let next_target = level.beacons[index];
            if next_target != target {
                target = next_target;
                closest = closest_on_segment(point, point, target);
            }
        }
        closest = closer(closest, closest_on_segment(point, travel_to, target));

        if let Some((t, outcome)) = event {
            if t > 0.0 {
                points.push(travel_to);
            }
            return SimResult {
                points,
                outcome,
                closest: Some(closest),
                visited: visited_count,
            };
        }

        point = next;
        points.push(point);
    }

    SimResult {
        points,
        outcome: Outcome::TimeLimit,
        closest: Some(closest),
        visited: visited_count,
    }
}

pub fn integrate(level: &Level, k: f64) -> SimResult {
    integrate_with_mode(level, k, SimulationMode::Laboratory)
}

pub fn integrate_with_mode(level: &Level, k: f64, mode: SimulationMode) -> SimResult {
    integrate_advanced(level, k, mode, false)
}

fn coarse_distance(level: &Level, k: f64) -> f64 {
    integrate_advanced(level, k, SimulationMode::Exploration, true)
        .closest
        .map(|closest| closest.distance)
        .unwrap_or(f64::INFINITY)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_force_reaches(level: &Level, step: f64) -> bool {
        let count = ((level.k_max - level.k_min) / step).ceil() as usize;
        (0..=count)
            .map(|index| level.k_min + index as f64 * step)
            .filter(|k| *k <= level.k_max)
            .any(|k| integrate(level, k).reached())
    }

    fn calm_level(beacons: Vec<Point>) -> Level {
        Level {
            title: "Test",
            desc: "",
            focus: "",
            mechanic: Mechanic::Steady,
            step_index: 0,
            field: FlowField::Calm {
                drift_x: 1.0,
                baseline_y: 0.0,
                gain_y: 1.0,
            },
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            recommended_step: 0.01,
            exploration_attempts: 3,
            k_window: 0.0,
            a: (0.0, 0.0),
            beacons,
            obstacles: Vec::new(),
        }
    }

    #[test]
    fn all_levels_load() {
        let levels = levels();
        assert_eq!(levels.len(), MECHANICS.len() * LEVELS_PER_GROUP);
        for (group, mechanic) in Mechanic::all().iter().enumerate() {
            for step in 0..LEVELS_PER_GROUP {
                let level = &levels[group * LEVELS_PER_GROUP + step];
                assert_eq!(level.mechanic, *mechanic);
                assert_eq!(level.step_index, step);
                assert_eq!(level.global_index(), group * LEVELS_PER_GROUP + step + 1);
                assert_eq!(level.beacons.len(), MECHANICS[group].plans[step].beacons);
            }
        }
    }

    #[test]
    fn mechanics_follow_the_teaching_order() {
        assert_eq!(
            Mechanic::all(),
            [
                Mechanic::Steady,
                Mechanic::Vortex,
                Mechanic::Zones,
                Mechanic::Opposed,
                Mechanic::Sensitive,
                Mechanic::Beacons
            ]
        );
    }

    #[test]
    fn all_levels_are_winnable() {
        for level in levels() {
            assert!(
                solve(&level).is_some(),
                "aucun réglage ne gagne : {} {}",
                level.title,
                level.focus
            );
        }
    }

    #[test]
    fn solver_agrees_with_a_brute_force_scan() {
        for level in levels().iter().take(2).chain(levels().iter().skip(25)) {
            let solved = solve(level);
            assert_eq!(
                solved.is_some(),
                brute_force_reaches(level, level.recommended_step / 5.0),
                "désaccord du solveur : {} {}",
                level.title,
                level.focus
            );
            if let Some(k) = solved {
                assert!(integrate(level, k).reached());
            }
        }
    }

    #[test]
    fn exploration_budgets_grow_with_difficulty() {
        let levels = levels();
        for level in &levels {
            assert!(
                (3..=6).contains(&level.exploration_attempts),
                "budget {} hors bornes sur {}",
                level.exploration_attempts,
                level.focus
            );
        }
        for group in 0..mechanic_count() {
            let budgets: Vec<usize> = levels
                [group * LEVELS_PER_GROUP..(group + 1) * LEVELS_PER_GROUP]
                .iter()
                .map(|level| level.exploration_attempts)
                .collect();
            assert!(
                budgets.windows(2).all(|pair| pair[0] <= pair[1]),
                "budgets décroissants dans {:?}: {:?}",
                MECHANICS[group].title,
                budgets
            );
        }
    }

    #[test]
    fn every_level_keeps_a_narrow_but_playable_window() {
        for (group, spec) in MECHANICS.iter().enumerate() {
            for step in 0..LEVELS_PER_GROUP {
                let level = &levels()[group * LEVELS_PER_GROUP + step];
                let max_window = spec.plans[step]
                    .k_window_steps
                    .expect("chaque plan doit viser une marge")
                    * level.recommended_step;
                assert!(
                    level.k_window <= max_window,
                    "marge {} hors cible {} : {}",
                    level.k_window,
                    max_window,
                    level.focus
                );
            }
        }
    }

    #[test]
    fn the_default_intensity_is_never_a_free_win() {
        for (index, level) in levels().iter().enumerate() {
            assert!(
                !integrate(level, 0.0).reached(),
                "zone {} gagnée à l'intensité par défaut : {}",
                index + 1,
                level.focus
            );
        }
    }

    fn grid_win_ratio(level: &Level) -> f64 {
        let step = level.recommended_step;
        let count = ((level.k_max - level.k_min) / step).ceil().max(1.0) as usize;
        let wins = (0..=count)
            .filter(|index| {
                let k = level.k_min + *index as f64 * step;
                k <= level.k_max + EPSILON && integrate(level, k).reached()
            })
            .count();
        wins as f64 / (count + 1) as f64
    }

    #[test]
    fn generated_levels_keep_the_winning_intensities_rare() {
        for seed in [DEFAULT_SEED, 7, 0xdead] {
            for (index, level) in generate_levels(seed).iter().enumerate() {
                let ratio = grid_win_ratio(level);
                assert!(
                    ratio > 0.0 && ratio <= 0.3,
                    "zone {} (graine {seed:x}) gagnante dans {:.0}% de la grille",
                    index + 1,
                    ratio * 100.0
                );
            }
        }
    }

    #[test]
    fn multi_beacon_levels_need_every_beacon() {
        let level = calm_level(vec![(2.0, 0.0), (2.0, 1.0)]);
        let first_only = integrate(&level, 0.0);
        assert!(!first_only.reached());
        assert_eq!(first_only.visited, 1);
        assert!(first_only.closest.is_some());
        let closer = level.beacons[1];
        assert!(first_only
            .closest
            .is_some_and(|closest| (closest.point.0 - closer.0).abs() < 1e-9));
    }

    #[test]
    fn multi_beacon_generated_levels_are_solved_together() {
        for level in generate_level_group(DEFAULT_SEED, Mechanic::Beacons.group_index()) {
            assert!(level.beacons.len() >= 2);
            let k = solve(&level).expect("les balises multiples doivent rester solubles");
            let result = integrate(&level, k);
            assert!(result.reached());
            assert_eq!(result.visited, level.beacons.len());
        }
    }

    #[test]
    fn band_field_switches_direction_between_bands() {
        let level = canonical_level(&MECHANICS[2], &MECHANICS[2].plans[4], 4);
        let FlowField::Bands { bands } = &level.field else {
            panic!("le groupe zones attend un champ à bandes");
        };
        assert_eq!(bands.len(), 3);
        let first = level.flow_at(0.0, (bands[0].y_min + bands[0].y_max) / 2.0, 0.5);
        let second = level.flow_at(0.0, (bands[1].y_min + bands[1].y_max) / 2.0, 0.5);
        assert!(first.y * second.y < 0.0);
        let below = level.flow_at(0.0, YMIN - 0.5, 0.5);
        let inside = level.flow_at(0.0, (bands[0].y_min + bands[0].y_max) / 2.0, 0.5);
        assert!((below.y - inside.y).abs() < 1e-12);
    }

    #[test]
    fn lazy_generation_matches_eager_generation() {
        let eager = generate_levels(42);
        for group in 0..mechanic_count() {
            assert_eq!(
                generate_level_group(42, group),
                eager[group * LEVELS_PER_GROUP..(group + 1) * LEVELS_PER_GROUP]
            );
        }
    }

    #[test]
    fn canonical_fallbacks_stay_playable() {
        for spec in MECHANICS.iter() {
            for step in 0..LEVELS_PER_GROUP {
                let level = canonical_level(spec, &spec.plans[step], step);
                assert!(
                    valid_geometry(&level),
                    "géométrie: {} {}",
                    spec.title,
                    level.focus
                );
                assert!(valid_field(&level), "champ: {} {}", spec.title, level.focus);
                assert!(
                    solve(&level).is_some(),
                    "repli insoluble: {} {}",
                    spec.title,
                    level.focus
                );
            }
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
        for seed in 0..3 {
            for level in generate_levels(seed) {
                assert!(
                    valid_geometry(&level),
                    "géométrie invalide: {} {}",
                    level.title,
                    level.focus
                );
                assert!(valid_field(&level), "champ invalide: {}", level.title);
                assert!(
                    solve(&level).is_some(),
                    "niveau impossible: {} {}",
                    level.title,
                    level.focus
                );
            }
        }
    }

    #[test]
    fn opposed_field_reverses_vertical_current() {
        let level = canonical_level(&MECHANICS[3], &MECHANICS[3].plans[0], 0);
        let below = level.flow_at(0.0, -0.1, 0.0);
        let above = level.flow_at(0.0, 0.1, 0.0);
        assert!(below.y < 0.0);
        assert!(above.y > 0.0);
    }

    #[test]
    fn same_field_gives_same_path() {
        let level = &levels()[2];
        assert_eq!(integrate(level, 0.2), integrate(level, 0.2));
    }

    #[test]
    fn collision_is_detected_between_sample_points() {
        let mut level = calm_level(vec![(5.0, 5.0)]);
        level.obstacles = vec![Obstacle {
            x: 0.01,
            y: 0.0,
            r: 0.1,
        }];
        let result = integrate(&level, 0.0);
        assert!(matches!(
            result.outcome,
            Outcome::Collision { obstacle: 0, .. }
        ));
    }

    #[test]
    fn exploration_counts_the_beacons_and_stops_on_obstacles() {
        let mut level = calm_level(vec![(1.0, 0.0), (2.0, 0.0)]);
        level.obstacles = vec![Obstacle {
            x: 0.01,
            y: 0.0,
            r: 0.1,
        }];
        let result = integrate_with_mode(&level, 0.0, SimulationMode::Exploration);
        assert!(!result.reached());
        assert!(result.collided());
        let laboratory = integrate_with_mode(&level, 0.0, SimulationMode::Laboratory);
        assert!(matches!(laboratory.outcome, Outcome::Collision { .. }));
    }

    #[test]
    fn exploration_visits_every_beacon_around_obstacles() {
        let mut level = calm_level(vec![(1.0, 0.0), (2.0, 0.0)]);
        level.obstacles = vec![Obstacle {
            x: 1.0,
            y: 0.9,
            r: 0.1,
        }];
        let result = integrate_with_mode(&level, 0.0, SimulationMode::Exploration);
        assert!(result.reached());
        assert!(!result.collided());
        assert_eq!(result.visited, 2);
    }

    #[test]
    fn exploration_reports_partial_beacon_runs() {
        let level = calm_level(vec![(1.0, 0.0), (1.0, 4.0)]);
        let result = integrate_with_mode(&level, 0.0, SimulationMode::Exploration);
        assert!(!result.reached());
        assert_eq!(result.visited, 1);
        assert!(matches!(result.outcome, Outcome::LeftField { .. }));
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
        let level = canonical_level(&MECHANICS[0], &MECHANICS[0].plans[0], 0);
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
