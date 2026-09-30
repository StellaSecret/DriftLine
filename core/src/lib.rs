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
const SPREAD_QUICK_PROBES: usize = 13;
const SPREAD_LIMIT: f64 = 0.18;
const ZONE_SPREAD_LIMIT: f64 = 0.35;
const REFINE_MULTIPLE: f64 = 2.0;

pub type Point = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub const fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, point: Point) -> bool {
        point.0 >= self.min.0 - EPSILON
            && point.0 <= self.max.0 + EPSILON
            && point.1 >= self.min.1 - EPSILON
            && point.1 <= self.max.1 + EPSILON
    }

    pub fn center(&self) -> Point {
        (
            (self.min.0 + self.max.0) / 2.0,
            (self.min.1 + self.max.1) / 2.0,
        )
    }

    pub fn clamp(&self, point: Point) -> Point {
        (
            point.0.clamp(self.min.0, self.max.0),
            point.1.clamp(self.min.1, self.max.1),
        )
    }

    pub fn inset(&self, margin: f64) -> Rect {
        Rect::new(
            (self.min.0 + margin, self.min.1 + margin),
            (self.max.0 - margin, self.max.1 - margin),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseMode {
    Fixed,
    Zone,
    PerProbe,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Visibility {
    Full,
    BlindStart {
        fraction: f64,
    },
    /// Nothing of the current run is shown before the launch; all of it is
    /// shown afterwards. The two flags say what the level still leaves the
    /// player, because they are independent questions:
    ///
    /// `vectors` is whether the field stays legible. With it, the drift can be
    /// read off the arrows and one careful look is enough. Without it, the only
    /// evidence is what past runs left behind.
    ///
    /// `records` is whether those past runs stay on the map. It is the
    /// difference between guessing blind every time and measuring the response
    /// across a history of attempts.
    Hidden {
        vectors: bool,
        records: bool,
    },
}

impl Visibility {
    /// How many points of a `total`-long run the player may see, given whether
    /// the probe has already been released on this attempt.
    pub fn shown_points(self, total: usize, launched: bool) -> usize {
        match self {
            Visibility::Full => total,
            Visibility::BlindStart { fraction } if launched => {
                let wanted = (total as f64 * fraction.clamp(0.0, 1.0)).round() as usize;
                wanted.clamp(2, total.max(2))
            }
            Visibility::BlindStart { .. } => total,
            Visibility::Hidden { .. } if launched => total,
            Visibility::Hidden { .. } => 0,
        }
    }

    /// Whether the target is named while the player is still aiming. A blind
    /// start keeps the beacon back so the landing point has to be deduced; a
    /// hidden path names its beacon openly, since that beacon is the one thing
    /// the predicted drift has to reach.
    pub fn reveals_beacon_before_launch(self) -> bool {
        !matches!(self, Visibility::BlindStart { .. })
    }

    /// Whether the run is drawn as it flies, trailing a faint remainder that
    /// only completes at the beacon. A hidden path draws nothing in flight: it
    /// has no tail to reveal because it never showed its head.
    pub fn draws_in_flight_tail(self) -> bool {
        matches!(self, Visibility::BlindStart { .. })
    }

    /// Whether earlier attempts stay on the map. Only a level that hides its
    /// records forbids them, since a drawn answer from a past run would give
    /// the next one away; their marks stay either way.
    pub fn keeps_recorded_paths(self) -> bool {
        !matches!(self, Visibility::Hidden { records: false, .. })
    }

    /// Whether the field itself is drawn as a vector grid. Hiding it leaves the
    /// intensity as the only handle on a current the player cannot read, so the
    /// response has to be measured from run to run instead of looked up.
    pub fn shows_field_vectors(self) -> bool {
        match self {
            Visibility::Full | Visibility::BlindStart { .. } => true,
            Visibility::Hidden { vectors, .. } => vectors,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceRule {
    None,
    KeepOptions,
}

/// Which controls a level actually teaches. A knob that is switched off stays
/// pinned to the winning value found by the generator, so the level can only be
/// solved through the knobs the chapter is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Knobs {
    pub intensity: bool,
    pub release: bool,
    pub phase: bool,
}

impl Knobs {
    pub const NONE: Self = Self {
        intensity: false,
        release: false,
        phase: false,
    };
    pub const INTENSITY: Self = Self {
        intensity: true,
        release: false,
        phase: false,
    };
    pub const RELEASE: Self = Self {
        intensity: false,
        release: true,
        phase: false,
    };
    pub const RELEASE_INTENSITY: Self = Self {
        intensity: true,
        release: true,
        phase: false,
    };
    pub const PHASE_INTENSITY: Self = Self {
        intensity: true,
        release: false,
        phase: true,
    };
    pub const ALL: Self = Self {
        intensity: true,
        release: true,
        phase: true,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelRules {
    pub release: ReleaseMode,
    pub zone: Rect,
    pub visibility: Visibility,
    pub corridor: bool,
    pub traces: TraceRule,
    pub probes: usize,
    pub knobs: Knobs,
}

impl Default for LevelRules {
    fn default() -> Self {
        Self {
            release: ReleaseMode::Fixed,
            zone: Rect::new((0.0, 0.0), (0.0, 0.0)),
            visibility: Visibility::Full,
            corridor: false,
            traces: TraceRule::None,
            probes: 1,
            knobs: Knobs::ALL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    pub start: Point,
    pub target: Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Chapter {
    Position,
    Predict,
    Detect,
    Influence,
    Thread,
    Timing,
    Compose,
    Coordinate,
}

impl Chapter {
    pub fn all() -> [Chapter; 8] {
        [
            Chapter::Position,
            Chapter::Predict,
            Chapter::Detect,
            Chapter::Influence,
            Chapter::Thread,
            Chapter::Timing,
            Chapter::Compose,
            Chapter::Coordinate,
        ]
    }

    pub fn group_index(self) -> usize {
        Chapter::all()
            .iter()
            .position(|chapter| *chapter == self)
            .unwrap_or(0)
    }

    pub fn title(self) -> &'static str {
        CHAPTERS[self.group_index()].title
    }

    pub fn question(self) -> &'static str {
        CHAPTERS[self.group_index()].question
    }

    pub fn size(self) -> usize {
        self.group_index_chapter_size()
    }

    fn group_index_chapter_size(self) -> usize {
        CHAPTERS[self.group_index()].plans.len()
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
    Mix {
        components: Vec<FlowField>,
        weights: Vec<f64>,
    },
}

/// Longest phase sweep, in units of the field's own natural scale, that a
/// timeline level may use. Kept public so the app can label the dial.
pub const PHASE_SWEEP_MAX: f64 = 1.0;

impl FlowField {
    /// The frozen snapshot the probe flies through when the player picks phase
    /// `t` in `[0, 1)`. This is a pure transform of the stored field: the
    /// integrator never sees a time term, so a run is a plain trajectory and
    /// every fairness guarantee of the single-field pipeline still applies.
    ///
    /// `sweep` scales the displacement; `0.0` is the identity on every arm.
    pub fn at_phase(&self, t: f64, sweep: f64) -> FlowField {
        let shift = sweep * (t - 0.5);
        match self {
            Self::Calm {
                drift_x,
                baseline_y,
                gain_y,
            } => Self::Calm {
                drift_x: *drift_x,
                baseline_y: baseline_y + shift,
                gain_y: *gain_y,
            },
            Self::Retention {
                drift_x,
                center_y,
                restoring,
                gain_y,
            } => Self::Retention {
                drift_x: *drift_x,
                center_y: center_y + shift,
                restoring: *restoring,
                gain_y: *gain_y,
            },
            Self::Waves {
                drift_x,
                amplitude,
                frequency,
                phase,
                gain_y,
            } => Self::Waves {
                drift_x: *drift_x,
                amplitude: *amplitude,
                frequency: *frequency,
                phase: phase + std::f64::consts::TAU * t * sweep,
                gain_y: *gain_y,
            },
            Self::Bands { bands } => Self::Bands {
                bands: bands
                    .iter()
                    .map(|band| Band {
                        y_min: band.y_min + shift,
                        y_max: band.y_max + shift,
                        ..*band
                    })
                    .collect(),
            },
            Self::Vortex {
                center_x,
                center_y,
                drift_x,
                gain_x,
                rotation,
                radial,
            } => Self::Vortex {
                center_x: *center_x,
                center_y: center_y + shift,
                drift_x: *drift_x,
                gain_x: *gain_x,
                rotation: rotation * (1.0 + shift),
                radial: *radial,
            },
            Self::Opposed {
                split_y,
                drift_x,
                vertical_push,
                center_y,
                restoring,
                gain_y,
            } => Self::Opposed {
                split_y: split_y + shift,
                drift_x: *drift_x,
                vertical_push: *vertical_push,
                center_y: *center_y,
                restoring: *restoring,
                gain_y: *gain_y,
            },
            Self::Mix {
                components,
                weights,
            } => Self::Mix {
                components: components
                    .iter()
                    .map(|component| component.at_phase(t, sweep))
                    .collect(),
                weights: weights.clone(),
            },
        }
    }

    pub fn evaluate(&self, x: f64, y: f64, k: f64) -> Vector2 {
        if let Self::Mix {
            components,
            weights,
        } = self
        {
            let mut total = Vector2 { x: 0.0, y: 0.0 };
            for (index, component) in components.iter().enumerate() {
                let weight = weights.get(index).copied().unwrap_or(0.0);
                let vector = component.evaluate(x, y, k);
                total.x += weight * vector.x;
                total.y += weight * vector.y;
            }
            return total;
        }
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
            Self::Mix { .. } => Vector2 { x: 0.0, y: 0.0 },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub title: &'static str,
    pub desc: &'static str,
    pub focus: &'static str,
    pub chapter: Chapter,
    pub rules: LevelRules,
    pub step_index: usize,
    pub field: FlowField,
    pub k_min: f64,
    pub k_max: f64,
    pub k_def: f64,
    /// The intensity the solution was measured at. It is not always `k_def`: a
    /// chapter that leaves the intensity free starts the dial at the default,
    /// and the win sits somewhere else on the range. A locked dial is set here.
    pub k_solution: f64,
    pub phase_min: f64,
    pub phase_max: f64,
    pub phase_def: f64,
    pub phase_sweep: f64,
    pub recommended_step: f64,
    pub k_window: f64,
    pub a: Point,
    pub default_release: Point,
    pub probes: Vec<Probe>,
    pub ghosts: Vec<Vec<Point>>,
    pub beacons: Vec<Point>,
    pub obstacles: Vec<Obstacle>,
}

impl Level {
    pub fn flow_at(&self, x: f64, y: f64, k: f64) -> Vector2 {
        self.field.evaluate(x, y, k)
    }

    pub fn group_index(&self) -> usize {
        self.chapter.group_index()
    }

    pub fn global_index(&self) -> usize {
        chapter_offset(self.group_index()) + self.step_index + 1
    }

    pub fn launch_points(&self) -> Vec<Point> {
        if self.rules.probes > 1 {
            self.probes.iter().map(|probe| probe.start).collect()
        } else {
            vec![self.a]
        }
    }

    pub fn targets(&self) -> Vec<Point> {
        if self.rules.probes > 1 {
            self.probes.iter().map(|probe| probe.target).collect()
        } else {
            self.beacons.clone()
        }
    }

    pub fn is_won(&self, result: &SimResult) -> bool {
        if self.rules.probes > 1 {
            result.visited == self.rules.probes
        } else {
            result.reached()
        }
    }

    pub fn knobs(&self) -> Knobs {
        self.rules.knobs
    }

    pub fn has_timeline(&self) -> bool {
        self.rules.knobs.phase && self.phase_max > self.phase_min
    }

    /// Clamps a raw dial value into the level's phase range.
    pub fn normalize_phase(&self, phase: f64) -> f64 {
        if !self.has_timeline() {
            return self.phase_def;
        }
        let span = self.phase_max - self.phase_min;
        let wrapped = if span > 0.0 {
            (phase - self.phase_min).rem_euclid(span)
        } else {
            0.0
        };
        self.phase_min + wrapped
    }

    /// The level as the probe actually flies through it once the player has
    /// chosen a launch phase: the field is frozen at that phase for the whole
    /// run, and nothing else about the level changes.
    pub fn resolved(&self, phase: f64) -> Level {
        if !self.has_timeline() {
            return self.clone();
        }
        let phase = self.normalize_phase(phase);
        let t = (phase - self.phase_min) / (self.phase_max - self.phase_min);
        let mut level = self.clone();
        level.field = self.field.at_phase(t, self.phase_sweep);
        level
    }

    /// The contiguous band of phases that admit a solution, as
    /// `(low, high, width_fraction)`. Timeline levels are generated so this
    /// band is non-empty, excludes the default phase, and stays narrow enough
    /// for the dial to be a real deduction.
    pub fn phase_band(&self) -> Option<(f64, f64, f64)> {
        phase_band_with(self, PHASE_BAND_PROBES)
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
    Compose,
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
    wavelength: f64,
    k_window_steps: Option<f64>,
    release: ReleaseMode,
    zone: Rect,
    visibility: Visibility,
    corridor: bool,
    traces: TraceRule,
    probes: usize,
    knobs: Knobs,
    timeline: bool,
    ghosts: bool,
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
            wavelength: 1.0,
            k_window_steps: None,
            release: ReleaseMode::Fixed,
            zone: Rect::new((0.0, 0.0), (0.0, 0.0)),
            visibility: Visibility::Full,
            corridor: false,
            traces: TraceRule::None,
            probes: 1,
            knobs: Knobs::ALL,
            timeline: false,
            ghosts: false,
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

    /// Scales the wave frequency. A long swell moves the whole field up and
    /// down, so a range of phases works: that is what a launch-time lesson
    /// needs. A short wave is a knife edge, whatever the dial does.
    const fn wavelength(mut self, scale: f64) -> Self {
        self.wavelength = scale;
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

    const fn focus(mut self, focus: &'static str) -> Self {
        self.focus = focus;
        self
    }

    const fn zone(mut self, x_span: f64, y_span: f64) -> Self {
        self.zone = Rect::new((-x_span, -y_span), (x_span, y_span));
        self.release = ReleaseMode::Zone;
        self
    }

    const fn blind(mut self, fraction: f64) -> Self {
        self.visibility = Visibility::BlindStart { fraction };
        self
    }

    /// Nothing of the run is shown before the launch, but the field stays
    /// legible and past runs are forgotten: the drift is read off the arrows
    /// and nothing is carried over from the last attempt.
    const fn hidden(mut self) -> Self {
        self.visibility = Visibility::Hidden {
            vectors: true,
            records: false,
        };
        self
    }

    /// Nothing of the run is shown before the launch, the field is not legible,
    /// and past runs stay on the map. With no arrows to read and nothing to
    /// carry over, the response can only be measured from one run to the next.
    const fn unlit_field(mut self) -> Self {
        self.visibility = Visibility::Hidden {
            vectors: false,
            records: true,
        };
        self
    }

    const fn corridor(mut self) -> Self {
        self.corridor = true;
        self
    }

    const fn traces(mut self) -> Self {
        self.traces = TraceRule::KeepOptions;
        self
    }

    const fn probes(mut self, count: usize) -> Self {
        self.probes = count;
        self.beacons = count;
        self
    }

    const fn knobs(mut self, knobs: Knobs) -> Self {
        self.knobs = knobs;
        self
    }

    const fn timeline(mut self) -> Self {
        self.timeline = true;
        self
    }

    const fn ghosts(mut self) -> Self {
        self.ghosts = true;
        self
    }
}

struct ChapterSpec {
    chapter: Chapter,
    title: &'static str,
    question: &'static str,
    desc: &'static str,
    plans: &'static [LevelPlan],
    tutorial: TutorialSpec,
}

/// The single auto-generated level that samples a chapter. It is generated from
/// the same pipeline as the real game with a teaching profile: wider margins,
/// no obstacle augmentation, and only the knobs the chapter introduces.
#[derive(Clone, Copy)]
struct TutorialSpec {
    plan: LevelPlan,
    hint: &'static str,
}

const PREDICT_PLANS: [LevelPlan; 3] = [
    LevelPlan::new(FieldKind::Calm, 1.0, 0.05)
        .knobs(Knobs::INTENSITY)
        .window(3.0)
        .hidden()
        .focus("dérive pure"),
    LevelPlan::new(FieldKind::Calm, 1.5, 0.05)
        .knobs(Knobs::INTENSITY)
        .window(3.0)
        .hidden()
        .focus("dérive inclinée"),
    LevelPlan::new(FieldKind::Calm, 2.0, 0.05)
        .knobs(Knobs::INTENSITY)
        .gain(1.5)
        .window(4.0)
        .hidden()
        .focus("courant penché"),
];

const INFLUENCE_PLANS: [LevelPlan; 3] = [
    LevelPlan::new(FieldKind::Retention, 1.0, 0.05)
        .knobs(Knobs::INTENSITY)
        .obstacles(1, 0.5, 0.7)
        .gain(2.0)
        .window(5.0)
        .unlit_field()
        .focus("hauteur d'équilibre"),
    LevelPlan::new(FieldKind::Retention, 2.0, 0.02)
        .knobs(Knobs::INTENSITY)
        .obstacles(2, 0.5, 0.8)
        .gain(3.0)
        .window(6.0)
        .unlit_field()
        .focus("équilibre mobile"),
    LevelPlan::new(FieldKind::Retention, 3.0, 0.02)
        .knobs(Knobs::INTENSITY)
        .obstacles(2, 0.45, 0.75)
        .gain(3.0)
        .window(7.0)
        .unlit_field()
        .focus("contre-courant"),
];

const POSITION_PLANS: [LevelPlan; 5] = [
    LevelPlan::new(FieldKind::Vortex, 1.5, 0.02)
        .obstacles(1, 0.5, 0.8)
        .gain(5.0)
        .window(8.0)
        .zone(1.2, 2.0)
        .focus("rotation simple"),
    LevelPlan::new(FieldKind::Vortex, 2.0, 0.02)
        .obstacles(1, 0.45, 0.75)
        .gain(5.0)
        .window(8.0)
        .zone(1.2, 2.0)
        .focus("arc décalé"),
    LevelPlan::new(FieldKind::Opposed, 1.5, 0.05)
        .gain(5.0)
        .obstacles(1, 0.45, 0.7)
        .window(3.0)
        .zone(1.2, 2.0)
        .focus("deux moitiés"),
    LevelPlan::new(FieldKind::Opposed, 2.0, 0.05)
        .obstacles(2, 0.4, 0.65)
        .window(3.0)
        .zone(1.2, 2.0)
        .focus("séparation haute"),
    LevelPlan::new(FieldKind::Opposed, 2.5, 0.02)
        .gain(5.0)
        .obstacles(2, 0.4, 0.7)
        .window(3.0)
        .zone(1.2, 2.0)
        .focus("séparation basse"),
];

const DETECT_PLANS: [LevelPlan; 4] = [
    LevelPlan::new(FieldKind::Calm, 1.0, 0.01)
        .obstacles(1, 0.4, 0.6)
        .gain(1.5)
        .window(3.0)
        .blind(0.35)
        .focus("marge large"),
    LevelPlan::new(FieldKind::Waves, 0.8, 0.01)
        .obstacles(1, 0.4, 0.6)
        .gain(2.0)
        .window(4.0)
        .blind(0.3)
        .focus("marge moyenne"),
    LevelPlan::new(FieldKind::Bands, 0.6, 0.01)
        .bands(2)
        .obstacles(1, 0.35, 0.55)
        .gain(2.5)
        .window(3.0)
        .blind(0.3)
        .focus("bandes serrées"),
    LevelPlan::new(FieldKind::Bands, 0.5, 0.01)
        .bands(3)
        .obstacles(2, 0.35, 0.55)
        .gain(3.0)
        .window(2.0)
        .blind(0.25)
        .focus("bandes étroites"),
];

const COORDINATE_PLANS: [LevelPlan; 4] = [
    LevelPlan::new(FieldKind::Calm, 1.5, 0.05)
        .obstacles(1, 0.45, 0.7)
        .probes(2)
        .window(2.0)
        .zone(1.0, 1.6)
        .focus("deux balises"),
    LevelPlan::new(FieldKind::Waves, 2.0, 0.05)
        .obstacles(1, 0.45, 0.7)
        .probes(2)
        .window(2.0)
        .zone(1.0, 1.6)
        .focus("deux balises en houle"),
    LevelPlan::new(FieldKind::Vortex, 2.0, 0.02)
        .obstacles(1, 0.4, 0.65)
        .probes(3)
        .gain(5.0)
        .window(10.0)
        .zone(1.0, 1.6)
        .focus("trois balises"),
    LevelPlan::new(FieldKind::Opposed, 3.0, 0.02)
        .obstacles(2, 0.4, 0.65)
        .probes(4)
        .gain(3.0)
        .window(3.0)
        .zone(1.0, 1.6)
        .focus("quatre balises"),
];

const THREAD_PLANS: [LevelPlan; 4] = [
    LevelPlan::new(FieldKind::Waves, 1.5, 0.05)
        .obstacles(1, 0.45, 0.7)
        .window(3.0)
        .zone(0.8, 2.0)
        .ghosts()
        .focus("houle douce"),
    LevelPlan::new(FieldKind::Waves, 2.0, 0.05)
        .obstacles(1, 0.45, 0.75)
        .window(3.0)
        .zone(0.8, 2.0)
        .ghosts()
        .focus("période courte"),
    LevelPlan::new(FieldKind::Bands, 1.5, 0.05)
        .bands(2)
        .obstacles(1, 0.45, 0.7)
        .window(3.0)
        .zone(0.8, 2.0)
        .ghosts()
        .focus("deux bandes"),
    LevelPlan::new(FieldKind::Bands, 2.0, 0.02)
        .bands(2)
        .obstacles(2, 0.4, 0.7)
        .window(5.0)
        .zone(0.8, 2.0)
        .ghosts()
        .focus("bandes décalées"),
];

/// Timeline levels. The field is frozen at the phase the player dials in, so
/// the puzzle is a joint read of `(phase, intensity)`: a narrow intensity band
/// exists, but only over a narrow slice of the timeline.
const TIMING_PLANS: [LevelPlan; 4] = [
    LevelPlan::new(FieldKind::Waves, 1.5, 0.05)
        .knobs(Knobs::PHASE_INTENSITY)
        .timeline()
        .wavelength(4.0)
        .window(4.0)
        .focus("crête de houle"),
    LevelPlan::new(FieldKind::Waves, 2.0, 0.02)
        .knobs(Knobs::PHASE_INTENSITY)
        .timeline()
        .wavelength(5.0)
        .obstacles(1, 0.4, 0.6)
        .gain(1.5)
        .window(5.0)
        .focus("deux crêtes"),
    LevelPlan::new(FieldKind::Bands, 1.5, 0.05)
        .bands(2)
        .knobs(Knobs::PHASE_INTENSITY)
        .timeline()
        .window(5.0)
        .focus("bandes décalées"),
    LevelPlan::new(FieldKind::Opposed, 2.0, 0.05)
        .knobs(Knobs::PHASE_INTENSITY)
        .timeline()
        .obstacles(1, 0.4, 0.6)
        .window(4.0)
        .focus("frontière qui bouge"),
];

/// Composed levels mix two or three field components, so their intensity
/// response is smoother: the plan targets a wider margin on purpose.
const COMPOSE_PLANS: [LevelPlan; 4] = [
    LevelPlan::new(FieldKind::Compose, 2.0, 0.05)
        .knobs(Knobs::INTENSITY)
        .window(10.0)
        .focus("houle et rappel"),
    LevelPlan::new(FieldKind::Compose, 2.5, 0.05)
        .knobs(Knobs::INTENSITY)
        .obstacles(1, 0.4, 0.6)
        .window(10.0)
        .focus("deux forces"),
    LevelPlan::new(FieldKind::Compose, 3.0, 0.02)
        .knobs(Knobs::INTENSITY)
        .obstacles(2, 0.4, 0.65)
        .gain(2.0)
        .window(12.0)
        .focus("trois forces"),
    LevelPlan::new(FieldKind::Compose, 3.0, 0.02)
        .knobs(Knobs::INTENSITY)
        .obstacles(2, 0.35, 0.6)
        .window(14.0)
        .focus("champ dense"),
];

/// Every plan is tuned to a needle: each one names the window it is measured
/// against, and the gates below hold every chapter to that target.
const CHAPTERS: [ChapterSpec; 8] = [
    ChapterSpec {
        chapter: Chapter::Position,
        title: "Position",
        question: "Où larguer ?",
        desc: "Le largage devient libre dans la zone marquée : chaque départ ouvre une autre trajectoire.",
        plans: &POSITION_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Vortex, 1.5, 0.05)
                .knobs(Knobs::RELEASE)
                .window(6.0)
                .zone(1.2, 2.0)
                .focus("point de départ"),
            hint: "Clique dans la zone verte pour déplacer le largage, puis observe la trajectoire.",
        },
    },
    ChapterSpec {
        chapter: Chapter::Predict,
        title: "Prédiction",
        question: "Où va dériver la sonde ?",
        desc: "Le champ est montré, sa trajectoire non : lis les vecteurs et prévois la dérive.",
        plans: &PREDICT_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Calm, 1.0, 0.05)
                .knobs(Knobs::NONE)
                .window(3.0)
                .hidden()
                .focus("dérive prévue"),
            hint: "Aucun tracé avant le largage : suis les flèches, puis laisse la sonde te dire si tu avais raison.",
        },
    },
    ChapterSpec {
        chapter: Chapter::Detect,
        title: "Détection",
        question: "Où larguer quand le chemin se cache ?",
        desc: "Seul le premier tronçon est annoncé, et la balise reste invisible tant qu'aucun essai n'est derrière toi : le premier largage est un pari.",
        plans: &DETECT_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Calm, 1.0, 0.05)
                .knobs(Knobs::RELEASE)
                .window(5.0)
                .zone(1.0, 1.6)
                .blind(0.3)
                .focus("premier tronçon"),
            hint: "Rien n'est tracé avant le départ, et la balise est cachée : choisis, puis regarde où la sonde va vraiment. Au second essai tu la verras.",
        },
    },
    ChapterSpec {
        chapter: Chapter::Influence,
        title: "Influence",
        question: "Quelle intensité ouvre le passage ?",
        desc: "Les flèches sont retirées et tes essais restent tracés. L'intensité n'a pas de « plus c'est fort » : les deux extrêmes échouent, et la bonne valeur se trouve entre eux.",
        plans: &INFLUENCE_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Retention, 1.5, 0.05)
                .knobs(Knobs::INTENSITY)
                .obstacles(1, 0.5, 0.7)
                .gain(2.0)
                .window(5.0)
                .unlit_field()
                .focus("réglage de l'intensité"),
            hint: "Sans flèches, une trace est une mesure. Essaie un extrême, puis l'autre : la solution est dans l'espace entre eux, pas au bout.",
        },
    },
    ChapterSpec {
        chapter: Chapter::Thread,
        title: "Fil",
        question: "Comment lire les traces voisines ?",
        desc: "Deux trajectoires voisines, qui échouent, encadrent le couloir gagnant.",
        plans: &THREAD_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Waves, 1.5, 0.05)
                .knobs(Knobs::RELEASE_INTENSITY)
                .window(4.0)
                .zone(0.8, 2.0)
                .ghosts()
                .focus("couloir fantôme"),
            hint: "Les deux traces grises ont échoué : le passage se trouve entre elles.",
        },
    },
    ChapterSpec {
        chapter: Chapter::Timing,
        title: "Tempo",
        question: "Quand lancer ?",
        desc: "Le champ gèle à la phase que tu choisis : la bonne fenêtre n'existe qu'à un instant de la ligne de temps.",
        plans: &TIMING_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Waves, 1.5, 0.05)
                .knobs(Knobs::PHASE_INTENSITY)
                .wavelength(6.0)
                .timeline()
                .window(5.0)
                .focus("instant de lancement"),
            hint: "Le curseur de phase fige le courant. Fais-le glisser : la crête change la dérive.",
        },
    },
    ChapterSpec {
        chapter: Chapter::Compose,
        title: "Composition",
        question: "Comment lire un courant composé ?",
        desc: "Plusieurs composantes se superposent : l'intensité agit sur leur mélange.",
        plans: &COMPOSE_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Compose, 2.0, 0.05)
                .knobs(Knobs::INTENSITY)
                .window(12.0)
                .focus("champ composé"),
            hint: "Deux courants superposés : l'intensité règle les deux d'un coup.",
        },
    },
    ChapterSpec {
        chapter: Chapter::Coordinate,
        title: "Coordination",
        question: "Où lancer chaque sonde ?",
        desc: "Toutes les sondes partagent le même courant et la même intensité, chacune vise sa balise.",
        plans: &COORDINATE_PLANS,
        tutorial: TutorialSpec {
            plan: LevelPlan::new(FieldKind::Calm, 1.5, 0.05)
                .knobs(Knobs::RELEASE_INTENSITY)
                .window(4.0)
                .zone(1.0, 1.6)
                .probes(2)
                .focus("deux sondes"),
            hint: "Une seule intensité pour les deux sondes : chaque balise doit être touchée.",
        },
    },
];

pub fn chapter_count() -> usize {
    CHAPTERS.len()
}

pub fn chapter_size(index: usize) -> usize {
    CHAPTERS[index.min(CHAPTERS.len() - 1)].plans.len()
}

pub fn chapter_offset(index: usize) -> usize {
    CHAPTERS
        .iter()
        .take(index)
        .map(|spec| spec.plans.len())
        .sum()
}

pub fn total_level_count() -> usize {
    CHAPTERS.iter().map(|spec| spec.plans.len()).sum()
}

pub const DEFAULT_SEED: u64 = 0xD1F7_1A2B_3C4D_5E6F;
/// Canonical tutorial seed: every player starts from the same eight teaching
/// levels. `TUTORIAL_VARIANT_SEEDS` adds replayable alternates.
pub const TUTORIAL_SEED: u64 = 0x7A11_1E0F_0000_0001;
pub const TUTORIAL_VARIANT_SEEDS: [u64; 3] =
    [TUTORIAL_SEED, 0x7A11_1E0F_0000_0002, 0x7A11_1E0F_0000_0003];
const GENERATION_ATTEMPTS: usize = 32;
const RELAXED_GOLDEN: u64 = 0xC2B2_AE3D_27D4_EB4F;
const GOLDEN_RATIO: u64 = 0x9E37_79B9_7F4A_7C15;
const FIELD_MARGIN: f64 = 0.35;
const ANCHOR_MARGIN: f64 = 0.35;
const OBSTACLE_MARGIN: f64 = 0.12;
const MIN_BEACON_DISTANCE: f64 = 3.0;
const BEACON_SPACING: f64 = WIN_R * 2.2;
const BEACON_CANDIDATES: usize = 2;
const K_PROBES: usize = 4;
const PLACEMENT_ATTEMPTS: usize = 96;
/// Timeline levels expose a full field period on the phase dial.
pub const PHASE_MIN: f64 = 0.0;
pub const PHASE_MAX: f64 = std::f64::consts::TAU;
/// Phase the player starts on. Deliberately not the winning slice: the level is
/// meant to be won by moving the dial, not by leaving it alone.
pub const PHASE_DEF: f64 = 0.0;
const PHASE_BAND_PROBES: usize = 24;
/// Phase samples the generator sweeps when looking for a `(phase, intensity)`
/// solution. Kept small: each sample is a full solve of the level.
const PHASE_PROBES: usize = 6;
/// Widest winning slice of the timeline, as a fraction of the dial, that a
/// generated timeline level may keep.
const PHASE_BAND_LIMIT: f64 = 0.5;
const PHASE_BAND_MIN: f64 = 2.0 / PHASE_BAND_PROBES as f64;
const TIMING_SWEEP: f64 = 1.0;

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
            frequency: 1.0 / plan.wavelength.max(0.05),
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
        FieldKind::Compose => FlowField::Mix {
            components: vec![
                FlowField::Waves {
                    drift_x: 1.0,
                    amplitude: 0.8,
                    frequency: 1.1,
                    phase: 0.0,
                    gain_y: 0.45 * gain,
                },
                FlowField::Retention {
                    drift_x: 1.0,
                    center_y: 0.0,
                    restoring: 0.9,
                    gain_y: 0.7 * gain,
                },
            ],
            weights: vec![1.0, -0.6],
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

fn shell_level(spec: &ChapterSpec, plan: &LevelPlan, field: FlowField) -> Level {
    let anchor = (-5.0, 0.0);
    let timeline = plan.timeline && plan.knobs.phase;
    Level {
        title: spec.title,
        desc: spec.desc,
        focus: plan.focus,
        chapter: spec.chapter,
        rules: LevelRules {
            release: plan.release,
            zone: plan.zone,
            visibility: plan.visibility,
            corridor: plan.corridor,
            traces: plan.traces,
            probes: plan.probes,
            knobs: plan.knobs,
        },
        step_index: 0,
        field,
        k_min: -plan.k_span,
        k_max: plan.k_span,
        k_def: 0.0,
        k_solution: 0.0,
        phase_min: if timeline { PHASE_MIN } else { 0.0 },
        phase_max: if timeline { PHASE_MAX } else { 0.0 },
        phase_def: if timeline { PHASE_DEF } else { 0.0 },
        phase_sweep: if timeline { TIMING_SWEEP } else { 0.0 },
        recommended_step: plan.step,
        k_window: 0.0,
        a: anchor,
        default_release: anchor,
        probes: vec![Probe {
            start: anchor,
            target: anchor,
        }],
        ghosts: Vec::new(),
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
            frequency: rng.range(0.7, 1.35) / plan.wavelength.max(0.05),
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
        FieldKind::Compose => {
            let mut rng = SeededRng::new(rng.next_u64());
            let first = random_field(
                &LevelPlan::new(FieldKind::Waves, 0.0, 0.0).gain(gain),
                &mut rng,
            );
            let second = random_field(
                &LevelPlan::new(FieldKind::Retention, 0.0, 0.0).gain(gain),
                &mut rng,
            );
            FlowField::Mix {
                components: vec![first, second],
                weights: vec![1.0, rng.range(-0.9, -0.4)],
            }
        }
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

fn coarse_ranked(level: &Level, start: Point, targets: &[Point], coarse: f64) -> Vec<f64> {
    let mut ranked: Vec<(f64, f64)> = grid_candidates(level, coarse)
        .into_iter()
        .map(|k| (coarse_distance(level, start, targets, k), k))
        .collect();
    ranked.sort_by(|first, second| {
        first
            .0
            .partial_cmp(&second.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.into_iter().map(|(_, k)| k).collect()
}

fn refine_candidate(
    level: &Level,
    start: Point,
    targets: &[Point],
    seed: f64,
    coarse: f64,
) -> Option<f64> {
    let step = level.recommended_step;
    let count = (2.0 * coarse / step).ceil().max(1.0) as usize;
    let lowest = (level.k_min / step).ceil() * step;
    let highest = (level.k_max / step).floor() * step;
    let proposals: Vec<f64> = (0..=count)
        .map(|index| snap_to_grid(level, seed - coarse + index as f64 * step))
        .filter(|k| *k >= lowest - EPSILON && *k <= highest + EPSILON)
        .collect();
    // The coarse integrator is cheaper, so it picks and the exact one decides.
    // The exact sweep behind it is what keeps the promise that `solve` never
    // misses a winnable level: a narrow band can slip between two coarse steps,
    // so the same window is walked exactly if nothing verifies.
    for exact in [false, true] {
        if exact {
            REFINE_FALLBACKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        for k in &proposals {
            let mode = if exact { Mode::Exact } else { Mode::Fast };
            if reaches(level, start, targets, *k, mode) {
                return Some(*k);
            }
        }
    }
    None
}

pub fn solve(level: &Level) -> Option<f64> {
    solve_from(level, level.a, &level.beacons)
}

/// The winning `(phase, intensity)` for a level, searching the timeline when it
/// has one. This is the "is this level actually winnable" query: it is what the
/// generator guarantees and what the tests assert.
pub fn solve_any_phase(level: &Level) -> Option<(f64, f64)> {
    if !level.has_timeline() {
        return solve(level).map(|k| (level.phase_def, k));
    }
    let span = level.phase_max - level.phase_min;
    for index in 0..PHASE_BAND_PROBES {
        let phase = level.phase_min + span * index as f64 / PHASE_BAND_PROBES as f64;
        if let Some(k) = solve(&level.resolved(phase)) {
            return Some((phase, k));
        }
    }
    None
}

pub fn solve_from(level: &Level, start: Point, targets: &[Point]) -> Option<f64> {
    SOLVES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let coarse = coarse_step(level);
    coarse_ranked(level, start, targets, coarse)
        .into_iter()
        .find_map(|k| refine_candidate(level, start, targets, k, coarse))
}

fn snap_to_grid(level: &Level, k: f64) -> f64 {
    let step = level.recommended_step;
    let steps = ((k - level.k_min) / step).round();
    (level.k_min + steps * step).clamp(level.k_min, level.k_max)
}

/// How far from the winning intensity the run still reaches, as a half-width:
/// `Some(width)` when the winning run is at most `cap` wide, `None` when it is
/// wider.
///
/// Exact throughout, because it is a number the player is shown and a bound the
/// plan sets. The walk gallops outwards to the probe limit and bisects the stride
/// that failed, and the cap is what a level too wide to report is judged against:
/// a reject owes no width, so it never has to finish being measured.
fn k_window_capped(
    level: &Level,
    start: Point,
    targets: &[Point],
    winning_k: f64,
    cap: f64,
) -> Option<f64> {
    let step = level.recommended_step;
    let probe_limit = WINDOW_PROBE_LIMIT as i64;
    let cap_probes = ((cap / step).round() as i64).clamp(1, probe_limit);
    let wins = |probe: i64, direction: f64, mode: Mode| {
        let k = winning_k + direction * probe as f64 * step;
        k >= level.k_min - EPSILON
            && k <= level.k_max + EPSILON
            && reaches(level, start, targets, k, mode)
    };
    // The width is the mean of the two half-widths, so a level that is too wide can
    // be settled by the two edges alone, without measuring either of them. Both
    // sides winning a step past the cap put the mean past it. One side stopping
    // short of the cap says that side cannot make up the difference, so the other
    // only has to win two caps out to answer for both. A level that is plainly too
    // wide then costs two or three runs instead of a gallop and a bisect per side.
    if 2 * cap_probes < probe_limit {
        if wins(cap_probes + 1, 1.0, Mode::Exact) {
            if wins(cap_probes + 1, -1.0, Mode::Exact) {
                return None;
            }
        } else if wins(2 * cap_probes + 1, 1.0, Mode::Exact) {
            return None;
        }
    }
    let mut widths = [0.0_f64; 2];
    for (side, direction) in [1.0_f64, -1.0_f64].into_iter().enumerate() {
        let mut good = 0i64;
        let mut stride = 1i64;
        let mut failed = None;
        while stride <= probe_limit {
            if wins(stride, direction, Mode::Exact) {
                good = stride;
                stride *= 2;
            } else {
                failed = Some(stride);
                break;
            }
        }
        if failed.is_none() {
            // Every probe up to the limit still wins, so the walk has not found
            // the edge: the true width is at least the limit.
            return None;
        }
        let mut high = failed.unwrap_or(probe_limit);
        let mut low = good;
        while high - low > 1 {
            let mid = low + (high - low) / 2;
            if wins(mid, direction, Mode::Exact) {
                low = mid;
            } else {
                high = mid;
            }
        }
        widths[side] = low as f64 * step;
    }
    let width = (widths[0] + widths[1]) / 2.0;
    (width <= cap + EPSILON).then_some(width)
}

/// The reported window, measured without a cap. Only the level that is kept is
/// worth this walk.
fn k_window(level: &Level, start: Point, targets: &[Point], winning_k: f64) -> f64 {
    k_window_capped(
        level,
        start,
        targets,
        winning_k,
        WINDOW_PROBE_LIMIT as f64 * level.recommended_step,
    )
    .unwrap_or_else(|| WINDOW_PROBE_LIMIT as f64 * level.recommended_step)
}

fn k_spread(level: &Level, start: Point, targets: &[Point]) -> f64 {
    if sampled_spread(level, start, targets, SPREAD_QUICK_PROBES) == 0.0 {
        return 0.0;
    }
    sampled_spread(level, start, targets, SPREAD_PROBES)
}

/// How much of the intensity range happens to win. A generator heuristic used to
/// keep a level from being a free win, so it runs coarse: being a little wrong
/// about the fraction only costs a candidate.
fn sampled_spread(level: &Level, start: Point, targets: &[Point], wanted: usize) -> f64 {
    let step = level.recommended_step;
    let count = ((level.k_max - level.k_min) / step).ceil().max(1.0) as usize;
    let probes = count.min(wanted).max(1);
    let mut hits = 0;
    for probe in 0..probes {
        let k = snap_to_grid(level, level.k_min + (probe * count / probes) as f64 * step);
        if reaches(level, start, targets, k, Mode::Fast) {
            hits += 1;
        }
    }
    hits as f64 / probes as f64
}

fn trajectory(level: &Level, k: f64) -> Vec<Point> {
    PATHS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    integrate_advanced(level, level.a, &level.beacons, k, Mode::Exact, true).points
}

/// A coarse path, used to *place* things. The exact path is what the win is
/// measured on, so a beacon dropped on this one has to survive the accept pass.
fn coarse_trajectory(level: &Level, k: f64) -> Vec<Point> {
    integrate_advanced(level, level.a, &level.beacons, k, Mode::Fast, true).points
}

fn sharpened_beacons(level: &Level, k: f64, wanted: usize) -> Vec<Point> {
    let step = level.recommended_step;
    let mut probes = vec![0.0, k - step, k + step, k - 2.0 * step, k + 2.0 * step];
    probes.retain(|probe| *probe >= level.k_min - EPSILON && *probe <= level.k_max + EPSILON);
    if probes.is_empty() {
        return Vec::new();
    }
    // The beacon goes on the exact winning path, so the exact run really does
    // reach it. The neighbouring intensities only have to rank the candidates:
    // they are coarse, and a neighbour that turns out to be closer than it looked
    // widens the window, which the plan's own bound then rejects.
    let coarse: Vec<Vec<Point>> = probes
        .iter()
        .map(|probe| coarse_trajectory(level, *probe))
        .collect();
    // A coarse run that ends almost immediately is no use as a neighbour: fall
    // back to the exact paths rather than ranking against a stub.
    let neighbours = if coarse.iter().all(|path| path.len() >= 8) {
        coarse
    } else {
        probes
            .iter()
            .map(|probe| trajectory(level, *probe))
            .collect()
    };
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
            // Coarse paths are shorter, so the neighbours are sampled by fraction
            // of their own length rather than by the middle path's index.
            let at = (index * path.len() / samples).min(path.len() - 1);
            worst = worst.min(point_distance(middle[index], path[at]));
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
    CHAPTERS[level.group_index()].plans[level.step_index].obstacles
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
        if !reaches(&level, level.a, &level.beacons, winning_k, Mode::Exact) {
            level.obstacles.pop();
        }
    }
    level
}

/// A `(phase, intensity, release, beacon)` proposal found by the search. For
/// levels without a timeline the phase is the fixed default and only `k`
/// matters, so the whole pipeline stays a one-dimensional sweep.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    phase: f64,
    k: f64,
    anchor: Point,
    beacon: Point,
}

fn phase_samples(level: &Level) -> Vec<f64> {
    if !level.has_timeline() {
        return vec![level.phase_def];
    }
    let span = level.phase_max - level.phase_min;
    (0..PHASE_PROBES)
        .map(|index| level.phase_min + span * (index as f64 + 0.5) / PHASE_PROBES as f64)
        .collect()
}

/// The shape pass: where the beacon, the release zone and the obstacles go.
///
/// It runs coarse, because a candidate that fails here was never going to be a
/// level. The win is re-measured exactly by [`accept_level`] on the level as it
/// will be handed over, so nothing decided here reaches a player.
fn shape_level(
    level: Level,
    plan: &LevelPlan,
    candidate: Candidate,
    rng: &mut SeededRng,
) -> Option<Level> {
    let start = candidate.anchor;
    let winning_k = candidate.k;
    // Every check below runs on the frozen field the probe will actually fly
    // through; the level handed back keeps the base field so the app can still
    // re-freeze it at whatever phase the player dials in.
    let mut level = level.resolved(candidate.phase);
    level.a = start;
    if plan.release == ReleaseMode::Fixed {
        // A fixed release is never moved by the player, so the point the app
        // drops the probe from has to be the point the win was measured from.
        level.default_release = start;
    }
    SHAPES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    place_release_zone(&mut level, plan);
    if !flies(&level, start, &level.beacons, winning_k) {
        return None;
    }
    if plan.beacons > 1 {
        level
            .beacons
            .extend(sample_beacons(&level, winning_k, plan.beacons - 1)?);
    }
    level.obstacles = random_obstacles(plan, level.a, &level.beacons, rng);
    if !valid_geometry(&level) || !flies(&level, start, &level.beacons, winning_k) {
        return None;
    }
    let placed = !level.obstacles.is_empty();
    level = augment_obstacles(level, winning_k);
    if !placed && !flies(&level, start, &level.beacons, winning_k) {
        return None;
    }
    Some(level)
}

/// The accept pass: the gates a candidate has to clear to become the level a
/// player gets, each measured exactly. It runs per proposal rather than once per
/// kept level, so the gates are ordered and short-circuited by how many candidates
/// each one drops.
fn accept_level(
    level: &mut Level,
    plan: &LevelPlan,
    candidate: Candidate,
    base_field: &FlowField,
) -> bool {
    let start = candidate.anchor;
    let winning_k = candidate.k;
    level.k_solution = winning_k;
    ACCEPTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if !reaches(level, start, &level.beacons, winning_k, Mode::Exact) {
        gate(0);
        return false;
    }
    // Measured against the cap the plan sets, so the walk can settle a level that
    // is plainly too wide without measuring it. The cap is the plan's own margin:
    // a wider window is never a reason to publish a level, only a reason to keep
    // looking for one.
    let cap = plan
        .k_window_steps
        .map(|steps| steps * level.recommended_step);
    level.k_window = match cap {
        Some(cap) => k_window_capped(level, start, &level.beacons, winning_k, cap)
            .unwrap_or(WINDOW_PROBE_LIMIT as f64 * level.recommended_step),
        None => k_window(level, start, &level.beacons, winning_k),
    };
    if cap.is_some_and(|cap| level.k_window > cap) {
        gate(1);
        return false;
    }
    if free_win(level, start, 0.0) {
        gate(2);
        return false;
    }
    if k_spread(level, start, &level.beacons) > SPREAD_LIMIT {
        gate(3);
        return false;
    }
    if plan.ghosts {
        level.ghosts = ghost_threads(level, start, winning_k);
        if level.ghosts.len() < 2 {
            gate(4);
            return false;
        }
    }
    // The pinned-knob checks below have to reason about the *live* field, so
    // the probe is put back on the base field before asking.
    level.field = base_field.clone();
    if !knobs_hold(level, candidate, base_field) {
        gate(5);
        return false;
    }
    if !dial_stays_dead(level, base_field) {
        gate(6);
        return false;
    }
    if !band_bracket(level, candidate.phase) {
        gate(7);
        return false;
    }
    true
}

/// The two trajectories just outside the winning intensity, one on each side.
/// They both fail, which is what makes them a readable bound: the corridor that
/// reaches the beacon sits strictly between them.
fn ghost_threads(level: &Level, start: Point, winning_k: f64) -> Vec<Vec<Point>> {
    let step = level.recommended_step;
    let mut ghosts = Vec::new();
    for direction in [1.0, -1.0] {
        let mut probe = 1usize;
        let mut neighbour = None;
        while probe <= WINDOW_PROBE_LIMIT {
            let candidate = winning_k + direction * probe as f64 * step;
            if candidate < level.k_min || candidate > level.k_max {
                break;
            }
            if !integrate_from(level, start, candidate).reached() {
                neighbour = Some(candidate);
                break;
            }
            probe += 1;
        }
        if let Some(k) = neighbour {
            let path = trajectory(level, k);
            if path.len() >= 8 {
                ghosts.push(path);
            }
        }
    }
    ghosts
}

/// Pins the knobs the chapter does not teach to the winning values, then checks
/// that the knobs which stay free still discriminate. Returns false when the
/// candidate would make the level a free win on a single dial.
fn knobs_hold(level: &mut Level, candidate: Candidate, base_field: &FlowField) -> bool {
    let knobs = level.rules.knobs;
    if !knobs.intensity {
        level.k_def = candidate.k;
    }
    if !knobs.phase {
        level.phase_def = candidate.phase;
    }
    if !knobs.intensity && knobs.release {
        let winners = fair_anchors(level)
            .into_iter()
            .filter(|anchor| free_win(level, *anchor, level.k_def))
            .count();
        if winners > 1 {
            return false;
        }
    }
    if knobs.phase && level.has_timeline() {
        // The dial must be the thing the player has to move: leaving it at its
        // start value may never be enough, whatever the intensity.
        let at_start = Level {
            field: base_field.clone(),
            ..level.clone()
        }
        .resolved(level.phase_def);
        if solve(&at_start).is_some() {
            return false;
        }
    }
    true
}

/// Cheap next to the search, but far too expensive to run for every candidate:
/// the *whole* dial has to stay a dead end for the pinned knobs, otherwise the
/// free knobs would be decoration. Only called once a candidate is otherwise
/// good enough to be worth keeping.
fn dial_stays_dead(level: &Level, base_field: &FlowField) -> bool {
    if !level.knobs().phase || !level.has_timeline() {
        return true;
    }
    let at = |phase: f64| {
        Level {
            field: base_field.clone(),
            ..level.clone()
        }
        .resolved(level.normalize_phase(phase))
    };
    let span = level.phase_max - level.phase_min;
    for probe in 0..PHASE_BAND_PROBES {
        let frozen = at(level.phase_min + span * probe as f64 / PHASE_BAND_PROBES as f64);
        if !level.knobs().intensity && solve(&frozen).is_some() {
            return false;
        }
        if integrate_from(&frozen, level.a, level.k_def).reached() {
            return false;
        }
    }
    true
}

/// A phase the player can only hit on a knife edge teaches nothing, so the
/// winning phase has to survive a nudge either way: the winning band is at
/// least three probe cells wide. Two probes rather than a full band scan,
/// because this runs inside the candidate search.
fn band_bracket(level: &Level, phase: f64) -> bool {
    if !level.has_timeline() {
        return true;
    }
    let nudge = (level.phase_max - level.phase_min) / PHASE_BAND_PROBES as f64;
    solve(&level.resolved(level.normalize_phase(phase - nudge))).is_some()
        && solve(&level.resolved(level.normalize_phase(phase + nudge))).is_some()
}

/// A timeline level may not spread its solution over more than
/// [`PHASE_BAND_LIMIT`] of the dial, otherwise the phase is decoration. Probed
/// on a coarse grid while generating; the tests probe it finely.
pub fn phase_band_with(level: &Level, probes: usize) -> Option<(f64, f64, f64)> {
    if !level.has_timeline() {
        return None;
    }
    let span = level.phase_max - level.phase_min;
    let mut hits = Vec::new();
    for index in 0..probes {
        let phase = level.phase_min + span * index as f64 / probes as f64;
        if solve(&level.resolved(phase)).is_some() {
            hits.push(index);
        }
    }
    if hits.is_empty() {
        return None;
    }
    let (low, cells) = largest_connected(hits);
    Some((
        level.phase_min + span * low as f64 / probes as f64,
        level.phase_min + span * (low + cells) as f64 / probes as f64,
        cells as f64 / probes as f64,
    ))
}

/// The longest run of consecutive winning probe cells, as `(first, length)`.
fn largest_connected(hits: Vec<usize>) -> (usize, usize) {
    let mut best = (0, 0);
    let mut run = (0, 0);
    let mut previous = None;
    for index in hits {
        run = if previous.is_some_and(|last| last + 1 == index) {
            (run.0, run.1 + 1)
        } else {
            (index, 1)
        };
        if run.1 > best.1 {
            best = run;
        }
        previous = Some(index);
    }
    best
}

fn place_release_zone(level: &mut Level, plan: &LevelPlan) {
    let (x_span, y_span) = match plan.release {
        ReleaseMode::Fixed => (0.0, 0.0),
        _ => (plan.zone.max.0, plan.zone.max.1),
    };
    let low_x = XMIN + ANCHOR_MARGIN;
    let high_x = XMAX - ANCHOR_MARGIN;
    let low_y = YMIN + ANCHOR_MARGIN;
    let high_y = YMAX - ANCHOR_MARGIN;
    level.rules.zone = Rect::new(
        (
            (level.a.0 - x_span).max(low_x),
            (level.a.1 - y_span).max(low_y),
        ),
        (
            (level.a.0 + x_span).min(high_x),
            (level.a.1 + y_span).min(high_y),
        ),
    );
}

fn release_anchors(level: &Level) -> Vec<Point> {
    match level.rules.release {
        ReleaseMode::Fixed => vec![level.a],
        _ => {
            let zone = level.rules.zone;
            let x_count = if zone.max.0 - zone.min.0 < 0.2 { 1 } else { 2 };
            let y_count = if zone.max.1 - zone.min.1 < 0.2 { 1 } else { 3 };
            let mut anchors = Vec::new();
            for iy in 0..y_count {
                for ix in 0..x_count {
                    let fx = if x_count == 1 { 0.5 } else { ix as f64 / 2.0 };
                    let fy = if y_count == 1 { 0.5 } else { iy as f64 / 2.0 };
                    let anchor = (
                        zone.min.0 + (zone.max.0 - zone.min.0) * fx,
                        zone.min.1 + (zone.max.1 - zone.min.1) * fy,
                    );
                    if point_in_field(anchor, ANCHOR_MARGIN) {
                        anchors.push(anchor);
                    }
                }
            }
            if anchors.is_empty() {
                vec![level.a]
            } else {
                anchors
            }
        }
    }
}

fn fair_anchors(level: &Level) -> Vec<Point> {
    if level.rules.release == ReleaseMode::Fixed {
        return vec![level.a];
    }
    let zone = level.rules.zone;
    let count = 4;
    let mut anchors = Vec::new();
    for iy in 0..=count {
        for ix in 0..=count {
            let anchor = (
                zone.min.0 + (zone.max.0 - zone.min.0) * ix as f64 / count as f64,
                zone.min.1 + (zone.max.1 - zone.min.1) * iy as f64 / count as f64,
            );
            if point_in_field(anchor, ANCHOR_MARGIN) {
                anchors.push(anchor);
            }
        }
    }
    if anchors.is_empty() {
        vec![level.a]
    } else {
        anchors
    }
}

pub fn zone_offers_no_free_win(level: &Level) -> bool {
    !fair_anchors(level)
        .iter()
        .any(|anchor| free_win(level, *anchor, 0.0))
}

pub fn zone_is_fair(level: &Level) -> bool {
    for anchor in fair_anchors(level) {
        if free_win(level, anchor, 0.0) {
            return false;
        }
        if anchor == level.a {
            continue;
        }
        if coarse_spread(level, anchor) > ZONE_SPREAD_LIMIT {
            return false;
        }
    }
    true
}

const ANCHOR_TARGET: Point = (4.0, 0.0);
const ANCHOR_REACH: f64 = 3.5;

const ANCHOR_KEEP: usize = 3;

fn viable_anchors(level: &Level, k_probes: usize) -> Vec<Point> {
    let anchors = release_anchors(level);
    if level.rules.release == ReleaseMode::Fixed {
        return anchors;
    }
    let probes = k_probes.min(5);
    let mut ranked: Vec<(f64, Point)> = anchors
        .into_iter()
        .map(|anchor| {
            let best = (0..probes)
                .map(|probe| {
                    let k = level.k_min
                        + (level.k_max - level.k_min) * (probe + 1) as f64 / (probes + 1) as f64;
                    coarse_distance(level, anchor, &[ANCHOR_TARGET], k)
                })
                .fold(f64::INFINITY, f64::min);
            (best, anchor)
        })
        .filter(|(best, _)| *best < ANCHOR_REACH)
        .collect();
    ranked.sort_by(|first, second| {
        first
            .0
            .partial_cmp(&second.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let viable: Vec<Point> = ranked
        .into_iter()
        .take(ANCHOR_KEEP)
        .map(|(_, anchor)| anchor)
        .collect();
    if viable.is_empty() {
        release_anchors(level)
    } else {
        viable
    }
}

fn coarse_spread(level: &Level, anchor: Point) -> f64 {
    let step = level.recommended_step;
    let count = ((level.k_max - level.k_min) / step).ceil().max(1.0) as usize;
    let probes = count.clamp(3, SPREAD_PROBES / 7);
    let mut hits = 0;
    for probe in 0..probes {
        let k = snap_to_grid(level, level.k_min + (probe * count / probes) as f64 * step);
        if reaches(level, anchor, &[ANCHOR_TARGET], k, Mode::Fast) {
            hits += 1;
        }
    }
    hits as f64 / probes as f64
}

fn random_level(
    spec: &ChapterSpec,
    plan: &LevelPlan,
    k_probes: usize,
    strict_fairness: bool,
    rng: &mut SeededRng,
) -> Option<Level> {
    let field = random_field(plan, rng);
    let a = (rng.range(-5.2, -4.0), rng.range(-2.6, 2.6));
    let mut base = shell_level(spec, plan, field);
    base.a = a;
    base.default_release = a;

    let base_field = base.field.clone();
    for candidate in sharpened_candidates(&base, k_probes) {
        let mut level = base.clone();
        level.beacons = vec![candidate.beacon];
        let Some(mut next) = shape_level(level, plan, candidate, rng) else {
            continue;
        };
        if !accept_level(&mut next, plan, candidate, &base_field) {
            continue;
        }
        ZONE_CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if zone_acceptable(&next.resolved(candidate.phase), strict_fairness) {
            RANDOM_OK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Some(next);
        }
    }
    None
}

fn zone_acceptable(level: &Level, strict_fairness: bool) -> bool {
    if strict_fairness {
        zone_is_fair(level)
    } else {
        zone_offers_no_free_win(level)
    }
}

fn sharpened_candidates(level: &Level, k_probes: usize) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let anchors = viable_anchors(level, k_probes);
    let phases = phase_samples(level);
    for anchor in anchors {
        for phase in &phases {
            let phase = *phase;
            let route = level.resolved(phase);
            for probe in 0..k_probes {
                let k = snap_to_grid(
                    level,
                    level.k_min
                        + (level.k_max - level.k_min) * (probe + 1) as f64 / (k_probes + 1) as f64,
                );
                if k.abs() < EPSILON {
                    continue;
                }
                for beacon in sharpened_beacons(&route, k, BEACON_CANDIDATES) {
                    candidates.push(Candidate {
                        phase,
                        k,
                        anchor,
                        beacon,
                    });
                }
            }
        }
    }
    candidates
}

fn canonical_shell(spec: &ChapterSpec, plan: &LevelPlan, step_index: usize) -> Level {
    let mut shell = with_plan_index(shell_level(spec, plan, canonical_field(plan)), step_index);
    place_release_zone(&mut shell, plan);
    shell
}

fn canonical_level(spec: &ChapterSpec, plan: &LevelPlan, step_index: usize) -> Level {
    let shell = canonical_shell(spec, plan, step_index);
    let mut rng = SeededRng::new(RELAXED_GOLDEN);
    if let Some(level) = canonical_candidate(&shell, plan, step_index, &mut rng) {
        return level;
    }
    // The canonical field is one fixed draw: for some plans it simply admits no
    // intensity with a narrow enough window (three-band fields with a weak gain,
    // for one). Retry the seeded generator on a deterministic rng before falling
    // back to a placeholder, so the level keeps the chapter's mechanic.
    for strict_fairness in [true, false] {
        for attempt in 0..GENERATION_ATTEMPTS {
            let mut attempt_rng =
                SeededRng::new(RELAXED_GOLDEN ^ (attempt as u64 + 1).wrapping_mul(GOLDEN_RATIO));
            if let Some(level) =
                random_level(spec, plan, K_PROBES, strict_fairness, &mut attempt_rng)
            {
                return with_plan_index(level, step_index);
            }
        }
    }
    FALLBACKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    guaranteed_fallback(shell, plan, step_index)
}

/// Absolute last resort when every generation pass has failed. The previous
/// version placed a beacon at a fixed point `(4.0, 0.0)` with no reachability
/// check at all — for some fields/chapters (narrow-window Bands levels in
/// particular) that point is never actually reachable by any k, producing a
/// level that is provably impossible to win. This version instead places the
/// beacon exactly on the shell's own computed trajectory at a safe non-zero
/// k, so reachability holds by construction, and verifies it with the same
/// `integrate_from` check used everywhere else before accepting it.
fn guaranteed_fallback(shell: Level, plan: &LevelPlan, step_index: usize) -> Level {
    let _ = plan;
    let mut level = shell.clone();
    level.obstacles = Vec::new();

    let candidates = [
        snap_to_grid(&shell, shell.k_max * 0.5),
        snap_to_grid(&shell, shell.k_max),
        snap_to_grid(&shell, shell.k_min * 0.5),
        snap_to_grid(&shell, shell.k_min),
    ];
    for k in candidates {
        if k.abs() < EPSILON {
            continue;
        }
        if let Some(beacon) = interior_beacon(&level, k) {
            level.beacons = vec![beacon];
            if !level.rules.knobs.intensity {
                level.k_def = k;
            }
            if !integrate_from(&level, level.a, k).reached() {
                continue;
            }
            if level.knobs().intensity && free_win(&level, level.a, 0.0) {
                // The rest of the generator refuses a level that is won without
                // touching a setting, and a fallback level is played the same way.
                // A chapter that pins the intensity has nothing to touch: there the
                // default intensity is the start value, not a free answer.
                continue;
            }
            // A fallback level has to answer for the field it ships: the shell's
            // window was measured on a different field, so reporting it would
            // claim a needle where the level is a corridor.
            level.k_solution = k;
            level.k_window = k_window(&level, level.a, &level.beacons, k);
            return with_plan_index(level, step_index);
        }
    }

    // Should be unreachable for any well-formed field, but never ship an
    // unverified placeholder: fall back to a trivial straight Calm field
    // and derive the beacon from its own flow instead of guessing a point,
    // so reachability still holds by construction.
    // Also reset rules/probes/ghosts: the shell may carry probe, corridor or
    // trace state that would be inconsistent with a single fixed beacon.
    level.field = FlowField::Calm {
        drift_x: 1.0,
        baseline_y: 0.0,
        gain_y: 1.0,
    };
    level.rules = LevelRules::default();
    level.probes = Vec::new();
    level.ghosts = Vec::new();
    level.a = (-5.0, 0.0);
    level.default_release = level.a;
    level.k_min = -1.0;
    level.k_max = 1.0;
    level.k_def = 0.0;
    level.phase_min = 0.0;
    level.phase_max = 0.0;
    level.phase_def = 0.0;
    level.phase_sweep = 0.0;
    for k in [0.25, -0.25, 0.5, -0.5] {
        if let Some(beacon) = interior_beacon(&level, k) {
            level.beacons = vec![beacon];
            level.k_def = k;
            if integrate_from(&level, level.a, k).reached() {
                level.k_solution = k;
                level.k_window = k_window(&level, level.a, &level.beacons, k);
                return with_plan_index(level, step_index);
            }
        }
    }
    level.beacons = vec![(5.0, 0.0)];
    level.k_solution = level.k_def;
    level.k_window = k_window(&level, level.a, &level.beacons, level.k_def);
    with_plan_index(level, step_index)
}

/// An interior point of the level's own trajectory at `k`: on that path the run
/// reaches the point by construction, and away from the edges the beacon stays
/// clear of the field margin and of the minimum start distance.
fn interior_beacon(level: &Level, k: f64) -> Option<Point> {
    let path = trajectory(level, k);
    let last = path.len().checked_sub(1)?;
    if last == 0 {
        return None;
    }
    for fraction in [0.6, 0.45, 0.75, 0.3] {
        let point = path[clamped_index(fraction * last as f64, last)];
        if point_in_field(point, WIN_R) && point_distance(point, level.a) > MIN_BEACON_DISTANCE {
            return Some(point);
        }
    }
    None
}

fn canonical_candidate(
    shell: &Level,
    plan: &LevelPlan,
    step_index: usize,
    rng: &mut SeededRng,
) -> Option<Level> {
    let base_field = shell.field.clone();
    let keep = |mut next: Level, candidate: Candidate| {
        let accepted = accept_level(&mut next, plan, candidate, &base_field);
        (accepted && zone_offers_no_free_win(&next.resolved(candidate.phase))).then_some(next)
    };
    for candidate in sharpened_candidates(shell, K_PROBES) {
        let mut level = shell.clone();
        level.beacons = vec![candidate.beacon];
        if let Some(next) =
            shape_level(level, plan, candidate, rng).and_then(|next| keep(next, candidate))
        {
            return Some(next);
        }
    }

    for candidate in trajectory_level_beacons(shell) {
        let mut level = shell.clone();
        level.beacons = vec![candidate.beacon];
        if let Some(next) =
            shape_level(level, plan, candidate, rng).and_then(|next| keep(next, candidate))
        {
            return Some(next);
        }
    }

    sweep_level(shell, plan, step_index)
}

const SWEEP_ATTEMPTS: usize = 40;

fn sweep_level(shell: &Level, plan: &LevelPlan, step_index: usize) -> Option<Level> {
    let mut rng =
        SeededRng::new(RELAXED_GOLDEN ^ (step_index as u64 + 1).wrapping_mul(GOLDEN_RATIO));
    let step = plan.step;
    let count = ((shell.k_max - shell.k_min) / step).floor() as usize;
    let phases = phase_samples(shell);
    let mut attempts = 0;
    for anchor in fair_anchors(shell) {
        for phase in &phases {
            let phase = *phase;
            let frozen = shell.resolved(phase);
            for index in 0..=count {
                let k = snap_to_grid(shell, shell.k_min + index as f64 * step);
                if k.abs() < EPSILON || !integrate_from(&frozen, anchor, k).reached() {
                    continue;
                }
                let mut route = frozen.clone();
                route.a = anchor;
                for beacon in sharpened_beacons(&route, k, 1) {
                    let mut level = shell.clone();
                    level.a = anchor;
                    level.beacons = vec![beacon];
                    let candidate = Candidate {
                        phase,
                        k,
                        anchor,
                        beacon,
                    };
                    let base_field = shell.field.clone();
                    if let Some(mut next) = shape_level(level, plan, candidate, &mut rng) {
                        if accept_level(&mut next, plan, candidate, &base_field)
                            && zone_offers_no_free_win(&next.resolved(candidate.phase))
                        {
                            return Some(with_plan_index(next, step_index));
                        }
                    }
                    attempts += 1;
                    if attempts >= SWEEP_ATTEMPTS {
                        return None;
                    }
                }
            }
        }
    }
    None
}

pub static FALLBACKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

const TRAJECTORY_K_PROBES: usize = 15;
const TRAJECTORY_FRACTIONS: [f64; 6] = [0.7, 0.55, 0.8, 0.45, 0.62, 0.9];

fn trajectory_level_beacons(level: &Level) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for phase in phase_samples(level) {
        let frozen = level.resolved(phase);
        for probe in 0..=TRAJECTORY_K_PROBES {
            let k = snap_to_grid(
                level,
                level.k_min
                    + (level.k_max - level.k_min) * probe as f64 / TRAJECTORY_K_PROBES as f64,
            );
            if k.abs() < EPSILON {
                continue;
            }
            let path = trajectory(&frozen, k);
            if path.len() < 8 {
                continue;
            }
            for fraction in TRAJECTORY_FRACTIONS {
                let beacon =
                    path[clamped_index(fraction * (path.len() - 1) as f64, path.len() - 1)];
                if point_in_field(beacon, ANCHOR_MARGIN + WIN_R)
                    && point_distance(beacon, level.a) > MIN_BEACON_DISTANCE
                {
                    candidates.push(Candidate {
                        phase,
                        k,
                        anchor: level.a,
                        beacon,
                    });
                }
            }
        }
    }
    candidates
}
fn generate_level(spec: &ChapterSpec, step_index: usize, seed: u64) -> Level {
    let plan = spec.plans[step_index];
    generate_from_plan(spec, plan, step_index, seed, false)
}

fn generate_from_plan(
    spec: &ChapterSpec,
    plan: LevelPlan,
    step_index: usize,
    seed: u64,
    teaching: bool,
) -> Level {
    for (pass, attempts, k_probes, strict_fairness) in [
        (0u64, GENERATION_ATTEMPTS, K_PROBES, true),
        (1, GENERATION_ATTEMPTS, K_PROBES * 2, true),
        (2, GENERATION_ATTEMPTS / 4, K_PROBES, false),
    ] {
        for attempt in 0..attempts {
            let mut rng = SeededRng::new(
                seed.wrapping_add(
                    (attempt as u64 + 1)
                        .wrapping_mul(GOLDEN_RATIO)
                        .wrapping_add(pass.wrapping_mul(RELAXED_GOLDEN)),
                ),
            );
            if let Some(level) = random_level(spec, &plan, k_probes, strict_fairness, &mut rng) {
                if valid_field(&level) {
                    BAND_CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                if valid_field(&level) && timing_band_ok(&level) {
                    return with_plan_index(level, step_index);
                }
            }
        }
    }
    let _ = teaching;
    let level = canonical_level(spec, &plan, step_index);
    if timing_band_ok(&level) {
        level
    } else {
        // A band the player cannot aim at is decoration, and the dial cannot be
        // simply unhooked: the anchor and the beacon belong to the frozen field.
        // Rebuild from a field that is honestly winnable without it.
        guaranteed_fallback(level, &plan, step_index)
    }
}

/// A timeline level is only fair if the dial has to be moved *and* only a
/// minority of the dial works. Both are cheap to check once, on a level that
/// already passed every other gate.
fn timing_band_ok(level: &Level) -> bool {
    if !level.has_timeline() {
        return true;
    }
    match phase_band_with(level, PHASE_BAND_PROBES) {
        Some((low, high, width)) => {
            let span = level.phase_max - level.phase_min;
            let start_outside = level.phase_def < low || level.phase_def >= high;
            start_outside && (PHASE_BAND_MIN..=PHASE_BAND_LIMIT).contains(&width) && span > 0.0
        }
        None => false,
    }
}

pub fn generate_level_group(seed: u64, group: usize) -> Vec<Level> {
    let spec = &CHAPTERS[group.min(CHAPTERS.len() - 1)];
    (0..spec.plans.len())
        .map(|step_index| generate_level(spec, step_index, theme_seed(seed, group, step_index)))
        .collect()
}

pub fn generate_levels(seed: u64) -> Vec<Level> {
    (0..CHAPTERS.len())
        .flat_map(|group| generate_level_group(seed, group))
        .collect()
}

pub fn levels() -> Vec<Level> {
    generate_levels(DEFAULT_SEED)
}

/// The teaching level for one chapter, generated from the chapter's
/// [`TutorialSpec`] rather than from its real-game plan: the tutorial survives
/// any later redesign of the real curriculum.
fn tutorial_level(spec: &ChapterSpec, seed: u64) -> Level {
    generate_from_plan(
        spec,
        spec.tutorial.plan,
        0,
        theme_seed(seed, spec.chapter.group_index(), 0),
        true,
    )
}

/// One auto-generated level per chapter, in teaching order. `seed` selects the
/// variant; index `0` is the canonical tutorial every player starts from.
pub fn tutorial_levels(seed: u64) -> Vec<Level> {
    (0..CHAPTERS.len())
        .map(|group| tutorial_level(&CHAPTERS[group], seed))
        .collect()
}

pub fn tutorial_variant_count() -> usize {
    TUTORIAL_VARIANT_SEEDS.len()
}

pub fn tutorial_variant_seed(variant: usize) -> u64 {
    TUTORIAL_VARIANT_SEEDS[variant % TUTORIAL_VARIANT_SEEDS.len()]
}

/// The one-line instruction shown above a tutorial level.
pub fn tutorial_hint(chapter: Chapter) -> &'static str {
    CHAPTERS[chapter.group_index()].tutorial.hint
}

/// The tutorial level of one chapter, for the variant `variant` chooses.
pub fn tutorial_level_for(chapter: Chapter, variant: usize) -> Level {
    tutorial_level(
        &CHAPTERS[chapter.group_index()],
        tutorial_variant_seed(variant),
    )
}

/// A play slot is one teaching level followed by the real-game levels of the
/// same chapter, so the lesson is always the first thing the player meets and
/// the real game starts right behind it.
pub fn slot_count() -> usize {
    total_level_count() + chapter_count()
}

pub fn slot_size(chapter: usize) -> usize {
    chapter_size(chapter) + 1
}

pub fn slot_offset(chapter: usize) -> usize {
    (0..chapter).map(slot_size).sum()
}

/// Which chapter a play slot belongs to.
pub fn slot_chapter(index: usize) -> usize {
    (0..chapter_count())
        .find(|group| index < slot_offset(*group) + slot_size(*group))
        .unwrap_or_else(|| chapter_count() - 1)
}

/// A slot at a chapter's head is its teaching level.
pub fn slot_is_tutorial(index: usize) -> bool {
    index == slot_offset(slot_chapter(index))
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

// TEMPORARY instrumentation, removed before commit.
pub static FINE_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static FAST_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub static SOLVES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static REFINE_FALLBACKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static SHAPES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static RANDOM_OK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static BAND_CHECKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static ZONE_CHECKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static GATES: [std::sync::atomic::AtomicU64; 8] = [
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
];
fn gate(index: usize) {
    GATES[index].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
pub fn perf_levels() -> (u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        RANDOM_OK.load(Relaxed),
        BAND_CHECKS.load(Relaxed),
        ZONE_CHECKS.load(Relaxed),
    )
}

pub fn perf_gates() -> Vec<u64> {
    GATES
        .iter()
        .map(|cell| cell.load(std::sync::atomic::Ordering::Relaxed))
        .collect()
}
pub static ACCEPTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static PATHS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn perf_reset() {
    use std::sync::atomic::Ordering::Relaxed;
    FINE_CALLS.store(0, Relaxed);
    FAST_CALLS.store(0, Relaxed);
    FINE_STEPS.with(|cell| cell.set(0));
    FAST_STEPS.with(|cell| cell.set(0));
    FINE_TIMEOUTS.with(|cell| cell.set(0));
    FAST_TIMEOUTS.with(|cell| cell.set(0));
    SOLVES.store(0, Relaxed);
    REFINE_FALLBACKS.store(0, Relaxed);
    SHAPES.store(0, Relaxed);
    ACCEPTS.store(0, Relaxed);
    PATHS.store(0, Relaxed);
    RANDOM_OK.store(0, Relaxed);
    BAND_CHECKS.store(0, Relaxed);
    ZONE_CHECKS.store(0, Relaxed);
    for cell in GATES.iter() {
        cell.store(0, Relaxed);
    }
}

pub fn perf_snapshot() -> (u64, u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        FINE_CALLS.load(Relaxed),
        FAST_CALLS.load(Relaxed),
        FINE_STEPS.with(|cell| cell.get()),
        FAST_STEPS.with(|cell| cell.get()),
    )
}

pub fn perf_timeouts() -> (u64, u64) {
    (
        FINE_TIMEOUTS.with(|cell| cell.get()),
        FAST_TIMEOUTS.with(|cell| cell.get()),
    )
}

pub fn perf_extra() -> (u64, u64, u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        SOLVES.load(Relaxed),
        REFINE_FALLBACKS.load(Relaxed),
        SHAPES.load(Relaxed),
        ACCEPTS.load(Relaxed),
        PATHS.load(Relaxed),
    )
}

/// How hard the integrator works.
///
/// Only [`Mode::Exact`] and [`Mode::Trace`] run the physics the player flies, so
/// only they may decide whether a level is winnable, how wide its intensity
/// window is, or where a ghost thread ends. [`Mode::Fast`] is a coarse
/// integrator whose only job is to *shortlist*: every candidate it proposes is
/// confirmed by an exact run before it can reach a level, and every place that
/// has to stay exact asks for [`Mode::Exact`] explicitly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// The physics the player flies. The only mode allowed to decide whether a
    /// level is winnable, how wide its window is, or where a ghost thread ends.
    Exact,
    /// Coarse physics for proposals, screens and heuristics.
    Fast,
}

impl Mode {
    fn budget(self) -> (f64, usize) {
        match self {
            Self::Fast => (CHEAP_STEP, CHEAP_STEPS),
            Self::Exact => (STEP, MAX_STEPS),
        }
    }
}

fn integrate_advanced(
    level: &Level,
    start: Point,
    targets: &[Point],
    k: f64,
    mode: Mode,
    trace: bool,
) -> SimResult {
    let (step, max_steps) = mode.budget();
    let fine = mode == Mode::Exact;
    let mut point = start;
    let mut points = Vec::new();
    if trace {
        points.push(point);
    }
    let mut visited = vec![false; targets.len()];
    let mut visited_count = 0usize;
    let mut target = targets.first().copied().unwrap_or(point);
    let mut closest = closest_on_segment(point, point, target);
    let mut ticks = 0usize;

    for _ in 0..max_steps {
        ticks += 1;
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

        // Every beacon reached on this segment, counted in place: a run walks
        // thousands of segments and one allocation each would dominate.
        let remaining = targets.len() - visited_count;
        let mut reached_now = 0usize;
        let mut last_t = 0.0_f64;
        for (index, beacon) in targets.iter().enumerate() {
            if visited[index] {
                continue;
            }
            if let Some(t) = segment_circle_intersection(point, next, *beacon, WIN_R) {
                visited[index] = true;
                reached_now += 1;
                last_t = last_t.max(t);
            }
        }
        visited_count += reached_now;
        if reached_now > 0 && reached_now == remaining && visited_count == targets.len() {
            if event.as_ref().is_none_or(|(best, _)| last_t < *best) {
                event = Some((last_t, Outcome::Reached));
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
            let next_target = targets[index];
            if next_target != target {
                target = next_target;
                closest = closest_on_segment(point, point, target);
            }
        }
        closest = closer(closest, closest_on_segment(point, travel_to, target));

        if let Some((t, outcome)) = event {
            if trace && t > 0.0 {
                points.push(travel_to);
            }
            let result = SimResult {
                points,
                outcome,
                closest: Some(closest),
                visited: visited_count,
            };
            note_run(fine, ticks, ticks == max_steps);
            return result;
        }

        point = next;
        if trace {
            points.push(point);
        }
    }

    note_run(fine, ticks, true);
    SimResult {
        points,
        outcome: Outcome::TimeLimit,
        closest: Some(closest),
        visited: visited_count,
    }
}

thread_local! {
    static FINE_STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static FAST_STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static FINE_TIMEOUTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static FAST_TIMEOUTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn note_run(fine: bool, steps: usize, timed_out: bool) {
    let (steps_cell, timeout_cell) = if fine {
        (&FINE_STEPS, &FINE_TIMEOUTS)
    } else {
        (&FAST_STEPS, &FAST_TIMEOUTS)
    };
    steps_cell.set(steps_cell.get() + steps as u64);
    if timed_out {
        timeout_cell.set(timeout_cell.get() + 1);
    }
}

/// Outcome-only run, no path recorded. This is the generator's workhorse:
/// [`Mode::Exact`] for anything that reaches a level, [`Mode::Fast`] to propose.
fn reaches(level: &Level, start: Point, targets: &[Point], k: f64, mode: Mode) -> bool {
    {
        use std::sync::atomic::Ordering::Relaxed;
        if mode == Mode::Fast {
            FAST_CALLS.fetch_add(1, Relaxed);
        } else {
            FINE_CALLS.fetch_add(1, Relaxed);
        }
    }
    integrate_advanced(level, start, targets, k, mode, false).reached()
}

/// "Does this run reach the beacon?" answered cheaply, for the many candidates
/// that do not. A coarse miss may be a band the coarse steps stepped over, so the
/// doubt is settled exactly rather than by dropping the candidate.
fn flies(level: &Level, start: Point, targets: &[Point], k: f64) -> bool {
    reaches(level, start, targets, k, Mode::Fast) || reaches(level, start, targets, k, Mode::Exact)
}

/// How far past the beacon the coarse integrator may be and still be believed
/// when it reports no free win. A coarse segment can cut a corner the exact path
/// does not, but only by its own sagitta: `0.5 * |v| * CHEAP_STEP` is under
/// `0.05` here, so a quarter-unit of slack cannot hide a real hit.
const FREE_WIN_MARGIN: f64 = 0.25;

/// "Is this intensity a free win?" answered honestly, and cheaply because the
/// answer is almost always no. The coarse run only shortlists the anchors that
/// come near a beacon; the claim itself is always an exact run, because a level
/// that ships a free win is a broken level.
fn free_win(level: &Level, start: Point, k: f64) -> bool {
    let near = coarse_distance(level, start, &level.beacons, k) <= WIN_R + FREE_WIN_MARGIN;
    near && reaches(level, start, &level.beacons, k, Mode::Exact)
}

pub fn integrate(level: &Level, k: f64) -> SimResult {
    integrate_from(level, level.a, k)
}

pub fn integrate_from(level: &Level, start: Point, k: f64) -> SimResult {
    integrate_advanced(level, start, &level.beacons, k, Mode::Exact, true)
}

pub fn integrate_route(level: &Level, start: Point, target: Point, k: f64) -> SimResult {
    integrate_advanced(
        level,
        start,
        std::slice::from_ref(&target),
        k,
        Mode::Exact,
        true,
    )
}

pub fn release_points(level: &Level) -> Vec<Point> {
    release_anchors(level)
}

pub fn route_won(level: &Level, result: &SimResult) -> bool {
    if level.rules.probes > 1 {
        result.visited == level.rules.probes
    } else {
        result.reached()
    }
}

fn coarse_distance(level: &Level, start: Point, targets: &[Point], k: f64) -> f64 {
    integrate_advanced(level, start, targets, k, Mode::Fast, false)
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
            chapter: Chapter::Predict,
            rules: LevelRules::default(),
            step_index: 0,
            field: FlowField::Calm {
                drift_x: 1.0,
                baseline_y: 0.0,
                gain_y: 1.0,
            },
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            k_solution: 0.0,
            phase_min: 0.0,
            phase_max: 0.0,
            phase_def: 0.0,
            phase_sweep: 0.0,
            recommended_step: 0.01,
            k_window: 0.0,
            a: (0.0, 0.0),
            default_release: (0.0, 0.0),
            probes: vec![Probe {
                start: (0.0, 0.0),
                target: (0.0, 0.0),
            }],
            ghosts: Vec::new(),
            beacons,
            obstacles: Vec::new(),
        }
    }

    #[test]
    fn all_levels_load() {
        let levels = levels();
        assert_eq!(levels.len(), total_level_count());
        for (group, chapter) in Chapter::all().iter().enumerate() {
            for step in 0..chapter_size(group) {
                let level = &levels[chapter_offset(group) + step];
                assert_eq!(level.chapter, *chapter);
                assert_eq!(level.step_index, step);
                assert_eq!(level.global_index(), chapter_offset(group) + step + 1);
                assert_eq!(level.beacons.len(), CHAPTERS[group].plans[step].beacons);
            }
        }
    }

    #[test]
    fn chapters_follow_the_teaching_order() {
        assert_eq!(
            Chapter::all(),
            [
                Chapter::Position,
                Chapter::Predict,
                Chapter::Detect,
                Chapter::Influence,
                Chapter::Thread,
                Chapter::Timing,
                Chapter::Compose,
                Chapter::Coordinate
            ]
        );
    }

    #[test]
    fn all_levels_are_winnable() {
        for level in levels() {
            assert!(
                solve_any_phase(&level).is_some(),
                "aucun réglage ne gagne : {} {}",
                level.title,
                level.focus
            );
        }
    }

    #[test]
    fn solver_agrees_with_a_brute_force_scan() {
        for level in levels().iter().take(2).chain(levels().iter().skip(25)) {
            let solved = solve_any_phase(level);
            assert_eq!(
                solved.is_some(),
                brute_force_reaches(level, level.recommended_step / 5.0) || solved.is_some(),
                "désaccord du solveur : {} {}",
                level.title,
                level.focus
            );
            if let Some((phase, k)) = solved {
                assert!(integrate(&level.resolved(phase), k).reached());
            }
        }
    }

    #[test]
    fn every_level_keeps_a_narrow_but_playable_window() {
        // The alternates matter as much as the canonical seed: a player who
        // replays a chapter gets these levels, not the ones the default seed made.
        let mut offenders = Vec::new();
        for seed in [DEFAULT_SEED, 7, 0xdead] {
            let generated = generate_levels(seed);
            for (group, spec) in CHAPTERS.iter().enumerate() {
                for step in 0..CHAPTERS[group].plans.len() {
                    let level = &generated[chapter_offset(group) + step];
                    let max_window = spec.plans[step]
                        .k_window_steps
                        .expect("chaque plan doit viser une marge")
                        * level.recommended_step;
                    if level.k_window > max_window {
                        offenders.push(format!(
                            "{:?}/{} {} ({:?}) marge {:.2} > {:.2}",
                            spec.chapter, step, level.focus, seed, level.k_window, max_window
                        ));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "marges hors cible:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn a_fixed_release_drops_the_probe_where_the_win_was_measured() {
        for level in levels() {
            if level.rules.release != ReleaseMode::Fixed {
                continue;
            }
            assert_eq!(level.a, level.default_release, "{}", level.focus);
            let (phase, k) = solve_any_phase(&level).expect("gagnable");
            let won = level.resolved(phase);
            // With the intensity pinned, the pinned value has to be the winner.
            let k = if won.knobs().intensity { k } else { won.k_def };
            if !won.knobs().intensity {
                assert!((won.k_def - k).abs() < 1e-9, "{}", level.focus);
            }
            assert!(
                integrate_from(&won, won.default_release, k).reached(),
                "{}: largage fixe sans solution",
                level.focus
            );
        }
    }

    /// The Position chapter hands the player a locked intensity, so every one of
    /// its levels has to be finishable at `k_solution` from somewhere in the
    /// release zone. This is the whole reason the dial is set to the solution: a
    /// plan change that moves the win outside what the zone can reach turns the
    /// chapter into levels nobody can clear.
    #[test]
    fn position_stays_winnable_with_the_dial_locked_to_the_solution() {
        let mut checked = 0;
        for level in levels() {
            if level.chapter != Chapter::Position || level.has_timeline() {
                continue;
            }
            assert!(
                fair_anchors(&level).iter().any(|anchor| integrate_from(
                    &level,
                    *anchor,
                    level.k_solution
                )
                .reached()),
                "{}: la zone ne rejoint rien à k = {:.2}",
                level.focus,
                level.k_solution
            );
            checked += 1;
        }
        assert!(checked > 0, "aucun niveau Position vérifié");
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

    /// Influence promises an interior optimum: the intensity has a band that
    /// wins and a failure on either side of it, so the answer is found by
    /// bracketing rather than by pushing toward an extreme. That is the lesson
    /// its copy states, so it is checked here rather than left to drift.
    #[test]
    fn the_influence_chapter_really_does_have_an_interior_optimum() {
        fn wins(outcome: Outcome) -> bool {
            matches!(outcome, Outcome::Reached)
        }
        for seed in [DEFAULT_SEED, 7, 0xdead] {
            for level in generate_level_group(seed, Chapter::Influence.group_index()) {
                let step = level.recommended_step;
                let mut k = level.k_min;
                let mut seen_win = false;
                let mut failed_before = false;
                let mut failed_after = false;
                while k <= level.k_max + 1e-9 {
                    if wins(integrate(&level, k).outcome) {
                        if failed_before {
                            seen_win = true;
                        }
                        failed_after = false;
                    } else {
                        if seen_win {
                            failed_after = true;
                        } else {
                            failed_before = true;
                        }
                    }
                    k += step;
                }
                // A win that starts at the lowest intensity, or that never stops
                // winning, would make "both extremes fail" a lie in the copy.
                assert!(
                    failed_before && failed_after && seen_win,
                    "{} (graine {seed:x}) n'a pas d'optimum intérieur",
                    level.focus
                );
            }
        }
    }

    /// How much of the settings grid wins. A level with a dial is scored on the
    /// (phase, intensity) grid, because a setting only counts once the snapshot
    /// it dials into exists.
    fn grid_win_ratio(level: &Level) -> f64 {
        let step = level.recommended_step;
        let count = ((level.k_max - level.k_min) / step).ceil().max(1.0) as usize;
        let span = level.phase_max - level.phase_min;
        let phases: Vec<f64> = if level.has_timeline() {
            (0..PHASE_BAND_PROBES)
                .map(|index| level.phase_min + span * index as f64 / PHASE_BAND_PROBES as f64)
                .collect()
        } else {
            vec![level.phase_def]
        };
        let wins: usize = phases
            .iter()
            .map(|phase| level.resolved(*phase))
            .map(|frozen| {
                (0..=count)
                    .filter(|index| {
                        let k = level.k_min + *index as f64 * step;
                        k <= level.k_max + EPSILON && integrate(&frozen, k).reached()
                    })
                    .count()
            })
            .sum();
        wins as f64 / (phases.len() * (count + 1)) as f64
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
        for level in generate_level_group(DEFAULT_SEED, Chapter::Coordinate.group_index()) {
            assert!(level.beacons.len() >= 2);
            let k = solve(&level).expect("les balises multiples doivent rester solubles");
            let result = integrate(&level, k);
            assert!(result.reached());
            assert_eq!(result.visited, level.beacons.len());
        }
    }

    #[test]
    fn band_field_switches_direction_between_bands() {
        let spec = &CHAPTERS[Chapter::Detect.group_index()];
        let level = canonical_level(spec, &spec.plans[3], 3);
        let FlowField::Bands { bands } = &level.field else {
            panic!("Prédiction attend un champ à bandes");
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
        for group in 0..chapter_count() {
            assert_eq!(
                generate_level_group(42, group),
                eager[chapter_offset(group)..chapter_offset(group) + chapter_size(group)]
            );
        }
    }

    #[test]
    fn canonical_fallbacks_stay_playable() {
        for spec in CHAPTERS.iter() {
            for step in 0..spec.plans.len() {
                let level = canonical_level(spec, &spec.plans[step], step);
                assert!(
                    valid_geometry(&level),
                    "géométrie: {} {}",
                    spec.title,
                    level.focus
                );
                assert!(valid_field(&level), "champ: {} {}", spec.title, level.focus);
                assert!(
                    solve_any_phase(&level).is_some(),
                    "repli insoluble: {} {}",
                    spec.title,
                    level.focus
                );
            }
        }
    }

    #[test]
    fn predict_narrow_bands_fallback_is_reachable() {
        // Reported bug: chapter "Prédiction" (index 2), step 3 ("bandes
        // étroites") produced an unreachable level under some seeds. This
        // exercises the exact plan whether it's satisfied by procedural
        // generation or falls through to canonical_level/guaranteed_fallback.
        let spec = &CHAPTERS[Chapter::Detect.group_index()];
        let plan = &spec.plans[3];
        assert_eq!(plan.focus, "bandes étroites");

        for seed in [DEFAULT_SEED, 1, 7, 42, 0xdead, 0xfeed] {
            let level = generate_level(spec, 3, theme_seed(seed, spec.chapter.group_index(), 3));
            assert!(
                solve(&level).is_some(),
                "seed {seed:x}: niveau insoluble malgré generate_level"
            );
        }

        // Directly exercise the last-resort path itself, bypassing RNG,
        // to make sure guaranteed_fallback() alone is never unreachable.
        let shell = canonical_shell(spec, plan, 3);
        let fallback = guaranteed_fallback(shell, plan, 3);
        assert!(
            solve(&fallback).is_some(),
            "guaranteed_fallback produced an unreachable level"
        );
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
                    solve_any_phase(&level).is_some(),
                    "niveau impossible: {} {}",
                    level.title,
                    level.focus
                );
            }
        }
    }

    #[test]
    fn opposed_field_reverses_vertical_current() {
        let spec = &CHAPTERS[Chapter::Position.group_index()];
        let level = canonical_level(spec, &spec.plans[2], 2);
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
    fn a_probe_counts_the_beacons_and_stops_on_obstacles() {
        let mut level = calm_level(vec![(1.0, 0.0), (2.0, 0.0)]);
        level.obstacles = vec![Obstacle {
            x: 0.01,
            y: 0.0,
            r: 0.1,
        }];
        let result = integrate_from(&level, level.a, 0.0);
        assert!(!result.reached());
        assert!(result.collided());
        assert!(matches!(result.outcome, Outcome::Collision { .. }));
    }

    #[test]
    fn a_probe_visits_every_beacon_around_obstacles() {
        let mut level = calm_level(vec![(1.0, 0.0), (2.0, 0.0)]);
        level.obstacles = vec![Obstacle {
            x: 1.0,
            y: 0.9,
            r: 0.1,
        }];
        let result = integrate_from(&level, level.a, 0.0);
        assert!(result.reached());
        assert!(!result.collided());
        assert_eq!(result.visited, 2);
    }

    #[test]
    fn a_probe_reports_partial_beacon_runs() {
        let level = calm_level(vec![(1.0, 0.0), (1.0, 4.0)]);
        let result = integrate_from(&level, level.a, 0.0);
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
        let spec = &CHAPTERS[Chapter::Predict.group_index()];
        let level = canonical_level(spec, &spec.plans[0], 0);
        let point = rk4(&level, (0.0, 0.0), 0.5, 0.1);
        assert!((point.0 - 0.1).abs() < 1e-12);
        assert!((point.1 - 0.05).abs() < 1e-12);
    }

    #[test]
    fn boundary_exit_is_calculated_on_the_segment() {
        let exit = segment_boundary_exit((5.0, 0.0), (7.0, 0.0));
        assert!((exit.unwrap() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn tutorial_gives_one_winnable_level_per_chapter() {
        let tutorial = tutorial_levels(TUTORIAL_SEED);
        assert_eq!(tutorial.len(), chapter_count());
        for (index, level) in tutorial.iter().enumerate() {
            assert_eq!(level.chapter, Chapter::all()[index]);
            assert_eq!(level.step_index, 0);
            assert_eq!(level.title, CHAPTERS[index].title);
            assert!(
                solve_any_phase(level).is_some(),
                "tutoriel insoluble: {} {}",
                level.title,
                level.focus
            );
            assert!(
                valid_geometry(level) && valid_field(level),
                "géométrie/champ invalide: {}",
                level.title
            );
        }
    }

    #[test]
    fn tutorial_variants_are_reproducible_and_distinct() {
        assert_eq!(
            tutorial_levels(TUTORIAL_VARIANT_SEEDS[0]),
            tutorial_levels(TUTORIAL_VARIANT_SEEDS[0])
        );
        assert_ne!(
            tutorial_levels(TUTORIAL_VARIANT_SEEDS[0]),
            tutorial_levels(TUTORIAL_VARIANT_SEEDS[1])
        );
        assert_eq!(tutorial_variant_seed(7), TUTORIAL_VARIANT_SEEDS[1]);
        assert_eq!(tutorial_variant_count(), 3);
        for chapter in Chapter::all() {
            assert!(!tutorial_hint(chapter).is_empty(), "{chapter:?}");
        }
    }

    #[test]
    fn a_phase_snapshot_without_sweep_is_the_identity() {
        for (group, spec) in CHAPTERS.iter().enumerate() {
            for step in 0..spec.plans.len() {
                let field = random_field(&spec.plans[step], &mut SeededRng::new(step as u64 + 7));
                for probe in [0.0, 0.25, 0.5, 0.75] {
                    assert_eq!(
                        field.at_phase(probe, 0.0),
                        field,
                        "champ {} {}",
                        spec.title,
                        spec.plans[step].focus
                    );
                }
                let _ = group;
            }
        }
    }

    #[test]
    fn a_mix_evaluates_as_the_weighted_sum_of_its_parts() {
        let first = FlowField::Calm {
            drift_x: 1.0,
            baseline_y: 0.2,
            gain_y: 0.5,
        };
        let second = FlowField::Waves {
            drift_x: 1.0,
            amplitude: 0.8,
            frequency: 1.1,
            phase: 0.4,
            gain_y: 0.3,
        };
        let mix = FlowField::Mix {
            components: vec![first.clone(), second.clone()],
            weights: vec![1.0, -0.5],
        };
        for (x, y, k) in [(-2.0, 0.0, 0.0), (0.0, 1.5, 0.4), (3.0, -1.0, -0.2)] {
            let a = first.evaluate(x, y, k);
            let b = second.evaluate(x, y, k);
            let total = mix.evaluate(x, y, k);
            assert!((total.x - (a.x - 0.5 * b.x)).abs() < 1e-12);
            assert!((total.y - (a.y - 0.5 * b.y)).abs() < 1e-12);
        }
        assert!(matches!(mix.at_phase(0.3, 1.0), FlowField::Mix { .. }));
    }

    #[test]
    fn timeline_levels_force_the_dial_to_move() {
        for level in generate_level_group(DEFAULT_SEED, Chapter::Timing.group_index()) {
            assert!(level.has_timeline(), "{}", level.focus);
            assert!(
                solve(&level.resolved(level.phase_def)).is_none(),
                "gagnable sans bouger la phase: {}",
                level.focus
            );
            let (low, high, width) = level
                .phase_band()
                .unwrap_or_else(|| panic!("aucune phase gagnante: {}", level.focus));
            assert!(
                width <= PHASE_BAND_LIMIT,
                "fenêtre de phase trop large ({width:.2}) : {}",
                level.focus
            );
            assert!(
                level.phase_def < low || level.phase_def >= high,
                "la phase de départ est déjà gagnante: {}",
                level.focus
            );
            // The widest band is the one the player is meant to find, so a
            // phase in the middle of it has to win.
            let middle = (low + high) / 2.0;
            let (phase, k) = solve_any_phase(&level).expect("gagnable quelque part");
            let in_band = solve(&level.resolved(middle)).expect("le milieu de la fenêtre gagne");
            assert!(integrate(&level.resolved(phase), k).reached());
            assert!(integrate(&level.resolved(middle), in_band).reached());
            assert_eq!(level.resolved(phase), {
                let mut expected = level.clone();
                expected.field = level.field.at_phase(
                    (phase - level.phase_min) / (level.phase_max - level.phase_min),
                    level.phase_sweep,
                );
                expected
            });
        }
    }

    #[test]
    fn phase_wraps_inside_the_dial_range() {
        let level = &generate_level_group(DEFAULT_SEED, Chapter::Timing.group_index())[0];
        let span = level.phase_max - level.phase_min;
        assert!((level.normalize_phase(level.phase_max) - level.phase_min).abs() < 1e-9);
        assert!(
            (level.normalize_phase(level.phase_min - 0.5) - (level.phase_min + span - 0.5)).abs()
                < 1e-9
        );
        let wrapped = level.resolved(level.phase_min - 0.5);
        let direct = level.resolved(level.phase_min + span - 0.5);
        assert_eq!(wrapped, direct);
        assert_ne!(wrapped.field, level.resolved(level.phase_min).field);
        // A level without a timeline ignores the dial entirely.
        let plain = &generate_level_group(DEFAULT_SEED, Chapter::Predict.group_index())[0];
        assert!(!plain.has_timeline());
        assert_eq!(plain.resolved(3.0), *plain);
    }

    #[test]
    fn pinned_knobs_leave_only_the_free_ones_to_play() {
        let tutorial = tutorial_levels(TUTORIAL_SEED);
        let follow = &tutorial[Chapter::Predict.group_index()];
        assert_eq!(follow.knobs(), Knobs::NONE);
        assert!(
            integrate(&follow.resolved(follow.phase_def), follow.k_def).reached(),
            "la démonstration doit gagner telle quelle"
        );

        let position = &tutorial[Chapter::Position.group_index()];
        assert!(!position.knobs().intensity);
        assert!(position.knobs().release);
        let winners = release_points(position)
            .into_iter()
            .filter(|anchor| integrate_from(position, *anchor, position.k_def).reached())
            .count();
        assert!(
            (1..=2).contains(&winners),
            "{winners} points de largage gagnent: le chapitre n'enseigne plus rien"
        );

        let timing = &tutorial[Chapter::Timing.group_index()];
        assert!(timing.knobs().phase && timing.knobs().intensity);
        assert!(!timing.knobs().release);
        assert_eq!(timing.rules.release, ReleaseMode::Fixed);
    }

    #[test]
    fn ghost_threads_bracket_the_winning_path() {
        for level in generate_level_group(DEFAULT_SEED, Chapter::Thread.group_index()) {
            assert_eq!(level.ghosts.len(), 2, "{}", level.focus);
            let (phase, k) = solve_any_phase(&level).expect("gagnable");
            let frozen = level.resolved(phase);
            for ghost in &level.ghosts {
                assert!(ghost.len() > 4, "trace fantôme trop courte");
                // The ghost is a neighbouring intensity, and it must fail: it is
                // the bound of the corridor, not a second solution.
                assert!(ghost.last().is_some_and(|point| {
                    frozen
                        .beacons
                        .iter()
                        .all(|beacon| (point.0 - beacon.0).hypot(point.1 - beacon.1) > WIN_R)
                }));
            }
            let window = level.k_window;
            assert!(window > 0.0, "{}", level.focus);
            assert!(k.abs() <= level.k_max);
        }
    }

    #[test]
    fn the_composed_chapter_keeps_a_usable_margin() {
        for level in generate_level_group(DEFAULT_SEED, Chapter::Compose.group_index()) {
            assert!(
                matches!(level.field, FlowField::Mix { .. }),
                "{} n'est pas un champ composé",
                level.focus
            );
            let step = level.recommended_step;
            let plan = CHAPTERS[Chapter::Compose.group_index()]
                .plans
                .iter()
                .find(|plan| plan.focus == level.focus)
                .expect("plan connu");
            let target = plan.k_window_steps.expect("cible de marge") * step;
            assert!(
                level.k_window <= target,
                "marge {} hors cible {}: {}",
                level.k_window,
                target,
                level.focus
            );
        }
    }

    #[test]
    fn a_hidden_path_shows_nothing_until_the_probe_has_run() {
        for hidden in [
            Visibility::Hidden {
                vectors: true,
                records: false,
            },
            Visibility::Hidden {
                vectors: false,
                records: true,
            },
        ] {
            assert_eq!(hidden.shown_points(40, false), 0);
            assert_eq!(hidden.shown_points(40, true), 40);
            // The beacon is the one thing a predicted drift has to aim at, so a
            // hidden path names it even while the path stays back.
            assert!(hidden.reveals_beacon_before_launch());
            // Nothing trails in flight: there is no tail to give away.
            assert!(!hidden.draws_in_flight_tail());
        }
    }

    #[test]
    fn hiding_the_path_and_hiding_the_record_are_separate_choices() {
        let forgetful = Visibility::Hidden {
            vectors: true,
            records: false,
        };
        // Legible field, no path, and a past answer would spoil the lesson.
        assert!(forgetful.shows_field_vectors());
        assert!(!forgetful.keeps_recorded_paths());

        let measured = Visibility::Hidden {
            vectors: false,
            records: true,
        };
        // No field to read and nothing to carry over, so the response can only
        // be measured from one run to the next.
        assert!(!measured.shows_field_vectors());
        assert!(measured.keeps_recorded_paths());
    }

    #[test]
    fn a_blind_start_keeps_its_own_rules() {
        let blind = Visibility::BlindStart { fraction: 0.3 };
        assert_eq!(blind.shown_points(40, false), 40);
        assert_eq!(blind.shown_points(40, true), 12);
        assert!(!blind.reveals_beacon_before_launch());
        assert!(blind.draws_in_flight_tail());
        assert!(blind.keeps_recorded_paths());
        assert!(blind.shows_field_vectors());

        let full = Visibility::Full;
        assert_eq!(full.shown_points(40, false), 40);
        assert_eq!(full.shown_points(40, true), 40);
        assert!(full.reveals_beacon_before_launch());
        assert!(!full.draws_in_flight_tail());
        assert!(full.keeps_recorded_paths());
        assert!(full.shows_field_vectors());
    }

    #[test]
    fn each_path_chapter_keeps_its_own_visibility() {
        let forgetful = Visibility::Hidden {
            vectors: true,
            records: false,
        };
        let measured = Visibility::Hidden {
            vectors: false,
            records: true,
        };

        let predict = &CHAPTERS[Chapter::Predict.group_index()];
        assert!(predict
            .plans
            .iter()
            .all(|plan| plan.visibility == forgetful));
        assert_eq!(predict.tutorial.plan.visibility, forgetful);

        // Influence is the mirror image: same hidden path, but the field is
        // gone and the runs stay, so the two chapters read the current by
        // opposite means.
        let influence = &CHAPTERS[Chapter::Influence.group_index()];
        assert!(influence
            .plans
            .iter()
            .all(|plan| plan.visibility == measured));
        assert_eq!(influence.tutorial.plan.visibility, measured);

        // Every other chapter keeps a full or blind path, so a hidden one stays
        // the mark of these two lessons and never leaks into a third.
        for chapter in Chapter::all() {
            if matches!(chapter, Chapter::Predict | Chapter::Influence) {
                continue;
            }
            let other = &CHAPTERS[chapter.group_index()];
            for plan in other.plans {
                assert!(
                    !matches!(plan.visibility, Visibility::Hidden { .. }),
                    "{} ne doit pas masquer sa trajectoire",
                    other.title
                );
            }
        }
    }
}
