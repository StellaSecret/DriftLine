#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use dioxus::prelude::*;
use peoplemodeler_core::{
    chapter_count, generate_level_group, integrate_from, slot_chapter, slot_count,
    slot_is_tutorial, slot_offset, slot_size, tutorial_hint, tutorial_level_for, Chapter, Level,
    Outcome, ReleaseMode, SimResult, TraceRule, Vector2, DEFAULT_SEED, WIN_R, XMAX, XMIN, YMAX,
    YMIN,
};

const W: f64 = 640.0;
const H: f64 = 420.0;
const X_SCALE: f64 = W / (XMAX - XMIN);
const Y_SCALE: f64 = H / (YMAX - YMIN);
const STEPS: [f64; 6] = [1.0, 0.5, 0.1, 0.05, 0.01, 0.001];
const HISTORY_LIMIT: usize = 3;
/// The dial is a continuous moment in the field's cycle: enough steps that it
/// reads as a slider, few enough that the preview stays honest.
const PHASE_SLIDER_STEPS: usize = 360;
const SAVE_KEY: &str = "driftline.save.v1";

#[derive(Clone, Debug)]
struct Attempt {
    k: f64,
    result: SimResult,
}

#[derive(Clone)]
struct LevelChip {
    index: usize,
    class: &'static str,
    disabled: bool,
    label: String,
}

#[derive(Clone, Debug, Default)]
struct LevelProgress {
    history: Vec<Attempt>,
    active_attempt: Option<Attempt>,
    solved: bool,
    release: Option<(f64, f64)>,
}

/// What a returning player gets back. Only the durable facts are saved: the
/// attempt history lives in the session, it is not a save.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SaveData {
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    variant: Option<usize>,
    #[serde(default)]
    progress: Vec<SavedLevel>,
    #[serde(default)]
    seen: Vec<bool>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SavedLevel {
    #[serde(default)]
    solved: bool,
    #[serde(default)]
    release: Option<(f64, f64)>,
}

fn save_data(seed: u64, variant: usize, progress: &[LevelProgress], seen: &[bool]) -> SaveData {
    SaveData {
        seed: Some(seed),
        variant: Some(variant),
        progress: progress
            .iter()
            .map(|level| SavedLevel {
                solved: level.solved,
                release: level.release,
            })
            .collect(),
        seen: seen.to_vec(),
    }
}

async fn read_save() -> Option<SaveData> {
    let script = format!("return window.localStorage.getItem('{SAVE_KEY}')");
    let value = dioxus::document::eval(&script).await.ok()?;
    let text = value.as_str()?;
    serde_json::from_str(text).ok()
}

async fn write_save(data: &SaveData) {
    let Ok(json) = serde_json::to_string(data) else {
        return;
    };
    let script = format!("window.localStorage.setItem('{SAVE_KEY}', {json}); return true;");
    let _ = dioxus::document::eval(&script).await;
}

fn main() {
    dioxus::launch(App);
}

fn map_x(x: f64) -> f64 {
    (x - XMIN) / (XMAX - XMIN) * W
}

fn map_y(y: f64) -> f64 {
    H - (y - YMIN) / (YMAX - YMIN) * H
}

fn invert_map_x(px: f64) -> f64 {
    XMIN + px / W * (XMAX - XMIN)
}

fn invert_map_y(py: f64) -> f64 {
    YMAX - py / H * (YMAX - YMIN)
}

fn release_of(level: &Level, progress: &LevelProgress) -> (f64, f64) {
    match level.rules.release {
        ReleaseMode::Fixed => level.default_release,
        _ => progress.release.unwrap_or(level.default_release),
    }
}

fn accepts_release(level: &Level, point: (f64, f64)) -> bool {
    level.knobs().release
        && level.rules.release != ReleaseMode::Fixed
        && level.rules.zone.contains(point)
}

async fn handle_field_click(
    event: Event<MouseData>,
    mut progress: Signal<Vec<LevelProgress>>,
    level_idx: usize,
    level: Level,
) {
    let pointer = event.client_coordinates();
    let script = field_pick_script(pointer.x, pointer.y);
    let Ok(value) = dioxus::document::eval(&script).await else {
        return;
    };
    let Some(text) = value.as_str().map(str::to_owned) else {
        return;
    };
    let Some(world) = text
        .split_once(',')
        .and_then(|(x, y)| Some((x.parse::<f64>().ok()?, y.parse::<f64>().ok()?)))
        .map(|(px, py)| (invert_map_x(px), invert_map_y(py)))
    else {
        return;
    };
    if !accepts_release(&level, world) {
        return;
    }
    let mut levels = progress();
    if let Some(entry) = levels.get_mut(level_idx) {
        entry.active_attempt = None;
        entry.release = Some(world);
    }
    progress.set(levels);
}

/// `dioxus::document::eval` runs the script as a function body, so the value has to
/// come back through an explicit `return`; a bare expression yields `undefined`.
fn field_pick_script(client_x: f64, client_y: f64) -> String {
    format!(
        "const svg = document.querySelector('svg.field-svg'); \
         if (!svg) return ''; \
         const box = svg.getBoundingClientRect(); \
         return [({client_x} - box.left) / box.width * {W}, \
                 ({client_y} - box.top) / box.height * {H}].join(',');"
    )
}

fn field_grid_x() -> Vec<f64> {
    let mut values = Vec::new();
    let mut x = XMIN + 0.5;
    while x < XMAX {
        values.push(x);
        x += 0.75;
    }
    values
}

fn field_grid_y() -> Vec<f64> {
    let mut values = Vec::new();
    let mut y = YMIN + 0.4;
    while y < YMAX {
        values.push(y);
        y += 0.6;
    }
    values
}

fn path_to_svg(points: &[(f64, f64)]) -> String {
    let mut path = String::new();
    for (index, (x, y)) in points.iter().enumerate() {
        let (px, py) = (map_x(*x), map_y(*y));
        if index == 0 {
            path.push_str(&format!("M {px:.2} {py:.2} "));
        } else {
            path.push_str(&format!("L {px:.2} {py:.2} "));
        }
    }
    path
}

fn vector_endpoints(level: &Level, x: f64, y: f64, k: f64) -> Option<(f64, f64, f64, f64)> {
    let Vector2 { x: vx, y: vy } = level.flow_at(x, y, k);
    let length = Vector2 { x: vx, y: vy }.length();
    if length <= f64::EPSILON {
        return None;
    }
    let (ux, uy) = (vx / length, vy / length);
    Some((
        map_x(x - ux * 0.28),
        map_y(y - uy * 0.28),
        map_x(x + ux * 0.28),
        map_y(y + uy * 0.28),
    ))
}

fn step_index(step: f64) -> usize {
    STEPS
        .iter()
        .position(|candidate| (candidate - step).abs() < 1e-9)
        .unwrap_or(2)
}

fn decimal_places(step: f64) -> usize {
    if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    }
}

fn format_intensity(value: f64, step: f64) -> String {
    format!("{value:.precision$}", precision = decimal_places(step))
}

fn normalize_intensity(value: f64, min: f64, max: f64, step: f64) -> f64 {
    let clamped = value.clamp(min, max);
    let normalized = (clamped / step).round() * step;
    let bounded = normalized.clamp(min, max);
    if bounded.abs() < step * 1e-6 {
        0.0
    } else {
        bounded
    }
}

#[derive(Clone, Copy)]
struct LevelSignals {
    level_idx: Signal<usize>,
    k: Signal<f64>,
    phase: Signal<f64>,
    step_idx: Signal<usize>,
    progress: Signal<Vec<LevelProgress>>,
    animation_visible: Signal<bool>,
}

fn replace_levels(
    mut all_levels: Signal<Vec<Level>>,
    mut signals: LevelSignals,
    next_levels: Vec<Level>,
) {
    let first_level = next_levels.first().cloned();
    let level_count = next_levels.len();
    all_levels.set(next_levels);
    signals
        .progress
        .set(vec![LevelProgress::default(); level_count]);
    signals.level_idx.set(0);
    if let Some(level) = first_level {
        signals.k.set(launch_intensity(&level));
        signals.phase.set(level.phase_def);
        signals.step_idx.set(step_index(level.recommended_step));
    }
    signals.animation_visible.set(false);
}

fn open_level(
    levels: Signal<Vec<Level>>,
    mut signals: LevelSignals,
    mut tutorial_seen: Signal<Vec<bool>>,
    index: usize,
) {
    if index >= unlocked_count(&(signals.progress)(), &(tutorial_seen)(), levels().len()) {
        return;
    }
    let Some(level) = levels().get(index).cloned() else {
        return;
    };
    if slot_is_tutorial(index) {
        mark_tutorial_seen(&mut tutorial_seen, index);
    }
    signals.level_idx.set(index);
    signals.k.set(launch_intensity(&level));
    signals.phase.set(level.phase_def);
    signals.step_idx.set(step_index(level.recommended_step));
    signals.animation_visible.set(false);
}

/// Opening a chapter's teaching level is what unlocks the rest of the chapter.
fn mark_tutorial_seen(seen: &mut Signal<Vec<bool>>, index: usize) {
    let mut next = seen();
    let chapter = slot_chapter(index);
    if next.len() < chapter_count() {
        next.resize(chapter_count(), false);
    }
    if next.get(chapter) == Some(&true) {
        return;
    }
    if let Some(entry) = next.get_mut(chapter) {
        *entry = true;
    }
    seen.set(next);
}

fn append_group(
    mut all_levels: Signal<Vec<Level>>,
    mut signals: LevelSignals,
    mut tutorial_seen: Signal<Vec<bool>>,
    seed: u64,
    group: usize,
    variant: usize,
) {
    let mut levels = all_levels();
    levels.extend(chapter_slots(seed, group, variant));
    all_levels.set(levels);
    let mut progress = (signals.progress)();
    progress.resize(progress.len() + slot_size(group), LevelProgress::default());
    signals.progress.set(progress);
    let mut seen = tutorial_seen();
    if seen.len() < chapter_count() {
        seen.resize(chapter_count(), false);
    }
    tutorial_seen.set(seen);
}

/// Re-rolls the loaded tutorials for a new variant. The real-game levels are
/// untouched: only the teaching slot of every loaded chapter is rebuilt, and
/// the player is sent back to the first slot to see it.
fn reroll_variant(
    mut all_levels: Signal<Vec<Level>>,
    mut signals: LevelSignals,
    mut tutorial_seen: Signal<Vec<bool>>,
    mut tutorial_variant: Signal<usize>,
    variant: usize,
) {
    let loaded = all_levels().len();
    let mut levels = all_levels();
    for chapter in 0..chapter_count() {
        let slot = slot_offset(chapter);
        if slot >= loaded {
            break;
        }
        levels[slot] = tutorial_level_for(Chapter::all()[chapter], variant);
    }
    all_levels.set(levels);
    let mut seen = tutorial_seen();
    seen.resize(chapter_count(), false);
    seen.fill(false);
    seen[0] = true;
    tutorial_seen.set(seen);
    let mut progress = (signals.progress)();
    for chapter in 0..chapter_count() {
        let slot = slot_offset(chapter);
        if slot < progress.len() {
            progress[slot] = LevelProgress::default();
        }
    }
    signals.progress.set(progress);
    signals.level_idx.set(0);
    if let Some(level) = all_levels().first().cloned() {
        signals.k.set(launch_intensity(&level));
        signals.phase.set(level.phase_def);
        signals.step_idx.set(step_index(level.recommended_step));
    }
    signals.animation_visible.set(false);
    tutorial_variant.set(variant);
}

fn format_seed(seed: u64) -> String {
    format!("{seed:016x}")
}

async fn entropy_seed() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        dioxus::document::eval("return Math.floor(Math.random() * 4294967296)")
            .await
            .ok()
            .and_then(|value| value.as_u64())
            .filter(|seed| *seed != 0)
            .unwrap_or(1)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(DEFAULT_SEED)
            .max(1)
    }
}

fn copy_seed(seed: u64) {
    spawn(async move {
        let script = format!("navigator.clipboard?.writeText('{}')", format_seed(seed));
        let _ = dioxus::document::eval(&script).await;
    });
}

/// The Position chapter asks where to release, so its intensity is shown but not
/// driven. The level still leaves the dial live: this is a lock on the control,
/// not a change to what the generator publishes.
fn intensity_editable(level: &Level) -> bool {
    level.knobs().intensity && level.chapter != Chapter::Position
}

/// The intensity a probe is dropped at. A dial the player cannot move is set to
/// the intensity the level was solved at rather than to the default the dial
/// starts from: on a chapter with a free intensity those are two different
/// values, and launching at the default is a level nobody can win.
fn launch_intensity(level: &Level) -> f64 {
    if intensity_editable(level) {
        level.k_def
    } else {
        level.k_solution
    }
}

/// The play slots of one chapter: its teaching level first, then its
/// real-game levels. Same seed, so a chapter is reproducible from its seed.
fn chapter_slots(seed: u64, group: usize, variant: usize) -> Vec<Level> {
    let mut levels = vec![tutorial_level_for(Chapter::all()[group], variant)];
    levels.extend(generate_level_group(seed, group));
    levels
}

fn solved_prefix(progress: &[LevelProgress]) -> usize {
    progress.iter().take_while(|level| level.solved).count()
}

/// A slot is playable when the run of solved levels reaches it, or once its
/// chapter's teaching level has been opened. The lesson is the way in, never a
/// lock: skipping it costs nothing but the badge.
fn slot_unlocked(progress: &[LevelProgress], seen: &[bool], index: usize, loaded: usize) -> bool {
    if index >= loaded {
        return false;
    }
    index < solved_prefix(progress) + 1 || seen.get(slot_chapter(index)).copied().unwrap_or(false)
}

fn unlocked_count(progress: &[LevelProgress], seen: &[bool], loaded: usize) -> usize {
    (0..loaded)
        .find(|index| !slot_unlocked(progress, seen, *index, loaded))
        .unwrap_or(loaded)
}

fn level_button_class(active: bool, unlocked: bool, solved: bool) -> &'static str {
    if active {
        "active"
    } else if !unlocked {
        "locked"
    } else if solved {
        "solved"
    } else {
        "open"
    }
}

fn push_attempt(history: &mut Vec<Attempt>, attempt: Attempt) {
    if history.len() >= HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(attempt);
}

fn archive_active_attempt(progress: &mut Signal<Vec<LevelProgress>>, level_idx: usize) {
    let mut levels = progress();
    if let Some(level) = levels.get_mut(level_idx) {
        if let Some(attempt) = level.active_attempt.take() {
            push_attempt(&mut level.history, attempt);
        }
    }
    progress.set(levels);
}

fn reset_level_progress(progress: &mut Signal<Vec<LevelProgress>>, level_idx: usize) {
    let mut levels = progress();
    if let Some(level) = levels.get_mut(level_idx) {
        level.history.clear();
        level.active_attempt = None;
    }
    progress.set(levels);
}

fn result_message(result: &SimResult, beacons_total: usize) -> (String, bool) {
    let progress_prefix = if beacons_total > 1 && result.visited > 0 {
        format!("{} balise(s) sur {}. ", result.visited, beacons_total)
    } else {
        String::new()
    };
    match result.outcome {
        Outcome::Reached => {
            if beacons_total > 1 {
                (
                    format!("La sonde a touché les {beacons_total} balises."),
                    true,
                )
            } else {
                ("La sonde a atteint la balise.".to_string(), true)
            }
        }
        Outcome::Collision { obstacle, .. } => (
            format!(
                "La sonde a percuté l'astéroïde {}. Le point rouge indique l'impact.",
                obstacle + 1
            ),
            false,
        ),
        Outcome::LeftField { .. } => (
            format!(
                "{progress_prefix}La sonde a quitté la zone. {}",
                closest_message(result)
            ),
            false,
        ),
        Outcome::TimeLimit => (
            format!(
                "{progress_prefix}La sonde a circulé trop longtemps. {}",
                closest_message(result)
            ),
            false,
        ),
        Outcome::NumericalFailure => (
            "Le courant n'a pas pu être calculé. Réinitialise la simulation.".to_string(),
            false,
        ),
    }
}

fn attempt_status(won: bool) -> &'static str {
    if won {
        "succès"
    } else {
        "échec"
    }
}

fn closest_message(result: &SimResult) -> String {
    result
        .closest
        .map(|closest| {
            format!(
                "Son passage le plus proche de la balise est à {:.2} unité.",
                closest.distance
            )
        })
        .unwrap_or_else(|| "Aucun passage n'a pu être mesuré.".to_string())
}

#[component]
fn App() -> Element {
    let all_levels = use_signal(|| chapter_slots(DEFAULT_SEED, 0, 0));
    let initial_k = launch_intensity(&all_levels()[0]);
    let initial_step = all_levels()[0].recommended_step;
    let total_levels = slot_count();
    let level_idx = use_signal(|| 0usize);
    let mut k = use_signal(|| initial_k);
    let mut phase = use_signal(|| all_levels()[0].phase_def);
    let mut step_idx = use_signal(|| step_index(initial_step));
    let mut progress = use_signal(|| vec![LevelProgress::default(); slot_size(0)]);
    let mut tutorial_seen = use_signal(|| vec![false; chapter_count()]);
    let mut tutorial_variant = use_signal(|| 0usize);
    let mut show_levels = use_signal(|| false);
    let mut animation_id = use_signal(|| 0usize);
    let mut animation_visible = use_signal(|| false);
    let mut active_seed = use_signal(|| DEFAULT_SEED);
    let mut copy_status = use_signal(String::new);
    let mut entropy_loaded = use_signal(|| false);
    let mut restored = use_signal(|| false);
    let level_signals = LevelSignals {
        level_idx,
        k,
        phase,
        step_idx,
        progress,
        animation_visible,
    };

    use_effect(move || {
        let has_active_attempt = progress()
            .get(level_idx())
            .is_some_and(|level| level.active_attempt.is_some());
        let should_animate = animation_id() > 0 && has_active_attempt;
        if animation_visible() != should_animate {
            animation_visible.set(should_animate);
        }
    });

    use_effect(move || {
        if entropy_loaded() {
            return;
        }
        entropy_loaded.set(true);
        spawn(async move {
            let save = read_save().await;
            let (next_seed, next_variant) = match save.as_ref() {
                Some(save) => (
                    save.seed.filter(|seed| *seed != 0).unwrap_or(DEFAULT_SEED),
                    save.variant.unwrap_or(0),
                ),
                None => (entropy_seed().await, 0),
            };
            let next_levels = chapter_slots(next_seed, 0, next_variant);
            let mut next_seen = match save.as_ref() {
                Some(save) if save.seen.len() == chapter_count() => save.seen.clone(),
                _ => vec![false; chapter_count()],
            };
            // The first slot is the first teaching level: the run starts inside it.
            next_seen[0] = true;
            let mut next_progress = vec![LevelProgress::default(); next_levels.len()];
            if let Some(save) = save {
                for (entry, saved) in next_progress.iter_mut().zip(save.progress) {
                    entry.solved = saved.solved;
                    entry.release = saved.release;
                }
            }
            active_seed.set(next_seed);
            tutorial_variant.set(next_variant);
            tutorial_seen.set(next_seen);
            copy_status.set(String::new());
            replace_levels(all_levels, level_signals, next_levels);
            progress.set(next_progress);
            restored.set(true);
        });
    });

    // Durable state is written back on every change; the payload is a few
    // hundred bytes, so there is nothing to debounce. It waits for the restore
    // to finish first: writing the empty default over a real save would be the
    // one bug a player could never recover from.
    use_effect(move || {
        if !restored() {
            return;
        }
        let data = save_data(
            active_seed(),
            tutorial_variant(),
            &progress(),
            &tutorial_seen(),
        );
        spawn(async move {
            write_save(&data).await;
        });
    });

    use_effect(move || {
        let loaded = all_levels().len();
        let reached = unlocked_count(&progress(), &tutorial_seen(), loaded);
        if loaded < total_levels && reached >= loaded {
            let group = slot_chapter(loaded);
            let seed = active_seed();
            let variant = tutorial_variant();
            spawn(async move {
                append_group(
                    all_levels,
                    level_signals,
                    tutorial_seen,
                    seed,
                    group,
                    variant,
                );
            });
        }
    });

    let levels_snapshot = all_levels();
    let current_level_idx = level_idx();
    let base_level = levels_snapshot[current_level_idx].clone();
    // The dial freezes the field: everything below simulates and draws the
    // snapshot the player dialled into, never the live field.
    let current = base_level.resolved(phase());
    let knobs = current.knobs();
    let intensity_editable = intensity_editable(&current);
    let launch_level = current.clone();
    let clickable_level = current.clone();
    let current_step = STEPS[step_idx()];
    let current_progress = progress()[current_level_idx].clone();
    let is_tutorial = slot_is_tutorial(current_level_idx);
    let visibility = current.rules.visibility;
    let release = release_of(&current, &current_progress);
    let preview = integrate_from(&current, release, k());
    let description = current.desc.to_string();
    let active_result = current_progress
        .active_attempt
        .as_ref()
        .map(|attempt| attempt.result.clone());
    let has_launched = active_result.is_some();
    let shown_points = active_result
        .as_ref()
        .map(|result| result.points.clone())
        .unwrap_or_else(|| preview.points.clone());
    // A hidden trajectory shows only the part the player has earned: a blind
    // start reveals the run as it flies, a hidden path shows nothing until the
    // probe has run at all, and both complete once the beacon has been found.
    let revealed = visibility
        .shown_points(shown_points.len(), has_launched)
        .min(shown_points.len());
    // A blind start keeps the beacon back while the player is aiming, so the
    // first launch is a real bet. The player sees it while the probe is in
    // flight, and it stays known from then on: what was measured counts as
    // remembered, so only the attempt that has to be guessed is the first one.
    let reveal_beacons = visibility.reveals_beacon_before_launch()
        || has_launched
        || !current_progress.history.is_empty();
    let shown_path_d = path_to_svg(&shown_points[..revealed]);
    // Only a blind start trails a faint remainder; a hidden path has no tail to
    // give away, so nothing is drawn in flight.
    let blind_path_d = if visibility.draws_in_flight_tail() {
        path_to_svg(&shown_points[revealed..])
    } else {
        String::new()
    };
    let ghost_paths: Vec<String> = base_level
        .ghosts
        .iter()
        .map(|ghost| path_to_svg(ghost))
        .collect();
    // A hidden path would spoil its own lesson: earlier trajectories are
    // dropped while their marks stay, so each attempt is read off the field.
    let keep_history_paths = visibility.keeps_recorded_paths();
    let history_snapshot = current_progress.history;
    let history_paths: Vec<(String, String, bool)> = history_snapshot
        .iter()
        .map(|attempt| {
            (
                if keep_history_paths {
                    path_to_svg(&attempt.result.points)
                } else {
                    String::new()
                },
                format_intensity(attempt.k, *STEPS.last().unwrap()),
                attempt.result.reached(),
            )
        })
        .collect();
    // A level that keeps its options leaves every tried intensity on the dial,
    // so a response the player cannot read off the field can still be bracketed
    // on purpose instead of re-guessed. The span is the level's own range, so
    // a tick always sits where that number was actually set.
    let trace_ticks: Vec<(f64, bool)> = if current.rules.traces == TraceRule::KeepOptions {
        let span = current.k_max - current.k_min;
        history_snapshot
            .iter()
            .map(|attempt| {
                let offset = if span > 0.0 {
                    (attempt.k - current.k_min) / span * 100.0
                } else {
                    0.0
                };
                (offset.clamp(0.0, 100.0), attempt.result.reached())
            })
            .collect()
    } else {
        Vec::new()
    };
    let closest_marker = active_result.as_ref().and_then(|result| {
        if result.reached() {
            None
        } else {
            result.closest
        }
    });
    let collision_point = match active_result.as_ref().map(|result| result.outcome) {
        Some(Outcome::Collision { point, .. }) => Some(point),
        _ => None,
    };
    let active_message = active_result
        .as_ref()
        .map(|result| result_message(result, current.beacons.len()));
    let path_class = if animation_visible() && active_result.is_some() {
        "path active-path"
    } else {
        "path"
    };
    let closest_class = if animation_visible() {
        "closest-point diagnostic-point"
    } else {
        "closest-point"
    };
    let collision_class = if animation_visible() {
        "collision-point diagnostic-point"
    } else {
        "collision-point"
    };
    let beacons_total = current.beacons.len();
    let visited_beacons = active_result
        .as_ref()
        .map(|result| result.visited)
        .unwrap_or(0);
    let progress_snapshot = progress();
    let seen_snapshot = tutorial_seen();
    let unlocked = unlocked_count(&progress_snapshot, &seen_snapshot, levels_snapshot.len());
    let can_go_previous = current_level_idx > 0;
    let can_go_next = current_level_idx + 1 < unlocked;
    let level_groups: Vec<(String, Vec<LevelChip>)> = (0..chapter_count())
        .filter_map(|group| {
            let first = slot_offset(group);
            if first >= levels_snapshot.len() {
                return None;
            }
            let last = (first + slot_size(group)).min(levels_snapshot.len());
            let chips = (first..last)
                .map(|index| LevelChip {
                    index,
                    class: level_button_class(
                        index == current_level_idx,
                        index < unlocked,
                        progress_snapshot
                            .get(index)
                            .is_some_and(|level| level.solved),
                    ),
                    disabled: index >= unlocked,
                    label: if slot_is_tutorial(index) {
                        "T".to_string()
                    } else {
                        (index + 1).to_string()
                    },
                })
                .collect();
            Some((levels_snapshot[first].chapter.title().to_string(), chips))
        })
        .collect();
    let show_levels_menu = show_levels();
    let window_hint = if current.k_window > 0.0 {
        format!("marge mesurée ±{:.2}", current.k_window)
    } else {
        String::new()
    };
    let lab_goal = if current.beacons.len() > 1 {
        "Atteindre toutes les balises"
    } else {
        "Atteindre la balise"
    };
    let tutorial_solved = current_progress.solved;
    let variant_label = ["A", "B", "C"]
        .get(tutorial_variant().min(2))
        .copied()
        .unwrap_or("A");
    let phase_span = base_level.phase_max - base_level.phase_min;
    let phase_step = if phase_span > 0.0 {
        phase_span / PHASE_SLIDER_STEPS as f64
    } else {
        0.0
    };
    let phase_readout = if phase_span > 0.0 {
        format!(
            "{:.0} % du cycle",
            (phase() - base_level.phase_min) / phase_span * 100.0
        )
    } else {
        String::new()
    };
    // Nothing to dial: the level has one moment, and showing a dead slider
    // would be a lie about what the level asks for.
    let show_dial = base_level.has_timeline();
    let pinned_knobs = [
        (!intensity_editable).then_some("intensité"),
        (!knobs.release).then_some("largage"),
        (!knobs.phase).then_some("phase"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<&str>>()
    .join(" · ");
    let pinned_label = if pinned_knobs.is_empty() {
        String::new()
    } else {
        format!("fixé : {pinned_knobs}")
    };
    let launch_label = if knobs.intensity {
        "Larguer la sonde"
    } else {
        "Voir la démonstration"
    };

    rsx! {
        style { {STYLE} }
        div { class: "wrap",
            h1 { "DriftLine" }
            p { class: "sub",
                "Une sonde est larguée dans une zone traversée par des courants invisibles. "
                "Une fois lâchée, elle suit le courant sans jamais dévier. En Laboratoire, règle l'intensité "
                "avant de la larguer pour atteindre la balise sans percuter les obstacles."
            }
            div { class: "panel",
                div { class: "level-nav",
                    button {
                        r#type: "button",
                        class: "ghost nav-button",
                        disabled: !can_go_previous,
                        onclick: move |_| {
                            if current_level_idx > 0 {
                                open_level(
                                    all_levels,
                                    level_signals,
                                    tutorial_seen,
                                    current_level_idx - 1,
                                );
                            }
                        },
                        "‹ Précédente"
                    }
                    div { class: "level-heading",
                        span { class: "level-index",
                            if is_tutorial {
                                "Tutoriel {current.chapter.group_index() + 1} · variante {variant_label}"
                            } else {
                                "Zone {current_level_idx + 1} / {total_levels}"
                            }
                        }
                        span { class: "level-family", "Chapitre {current.chapter.group_index() + 1} · {current.chapter.title()} · {current.focus} ({current.step_index + 1}/{current.chapter.size()})" }
                    }
                    button {
                        r#type: "button",
                        class: "ghost nav-button",
                        disabled: !can_go_next,
                        onclick: move |_| {
                            if can_go_next {
                                open_level(
                                    all_levels,
                                    level_signals,
                                    tutorial_seen,
                                    current_level_idx + 1,
                                );
                            }
                        },
                        "Suivante ›"
                    }
                    button {
                        r#type: "button",
                        class: "ghost small-button",
                        onclick: move |_| {
                            let next = !show_levels();
                            show_levels.set(next);
                        },
                        if show_levels_menu { "Masquer les zones" } else { "Toutes les zones" }
                    }
                }
                if show_levels_menu {
                    div { class: "levels-menu",
                        for (title, indexes) in level_groups {
                            div { class: "levels-group",
                                span { class: "levels-group-title", "{title}" }
                                div { class: "levels-group-row",
                                    for chip in indexes {
                                        button {
                                            key: "{chip.index}",
                                            class: chip.class,
                                            disabled: chip.disabled,
                                            onclick: move |_| {
                                                open_level(
                                                    all_levels,
                                                    level_signals,
                                                    tutorial_seen,
                                                    chip.index,
                                                );
                                            },
                                            "{chip.label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if is_tutorial {
                    div { class: "tutorial-banner",
                        span { class: "tutorial-badge",
                            if tutorial_solved { "Tutoriel ✓" } else { "Tutoriel" }
                        }
                        span { class: "tutorial-hint", "{tutorial_hint(current.chapter)}" }
                        span { class: "tutorial-gate", "Tu peux passer : ce tutoriel n'est pas obligatoire." }
                    }
                }
                div { class: "desc", "{current.title} — {description} · {lab_goal}" }
                if !pinned_label.is_empty() {
                    div { class: "knob-note", "{pinned_label}" }
                }

                svg {
                    view_box: "0 0 {W} {H}", class: "field-svg",
                    onclick: move |event: Event<MouseData>| {
                        spawn(handle_field_click(
                            event,
                            progress,
                            current_level_idx,
                            clickable_level.clone(),
                        ));
                    },
                    defs {
                        marker {
                            id: "flow-arrow",
                            view_box: "0 0 10 10",
                            ref_x: "8",
                            ref_y: "5",
                            marker_width: "4",
                            marker_height: "4",
                            orient: "auto",
                            path { d: "M 0 0 L 10 5 L 0 10 z", class: "arrow-head" }
                        }
                    }
                    if visibility.shows_field_vectors() {
                        for gx in field_grid_x() {
                            for gy in field_grid_y() {
                                if let Some((x1, y1, x2, y2)) = vector_endpoints(&current, gx, gy, k()) {
                                    line {
                                        key: "{gx}-{gy}",
                                        x1: "{x1:.2}", y1: "{y1:.2}",
                                        x2: "{x2:.2}", y2: "{y2:.2}",
                                        class: "arrow",
                                        marker_end: "url(#flow-arrow)",
                                    }
                                }
                            }
                        }
                    }
                    for (index, obstacle) in current.obstacles.iter().enumerate() {
                        ellipse {
                            key: "{index}",
                            cx: "{map_x(obstacle.x):.2}", cy: "{map_y(obstacle.y):.2}",
                            rx: "{(obstacle.r * X_SCALE):.2}", ry: "{(obstacle.r * Y_SCALE):.2}",
                            class: "obstacle",
                        }
                    }
                    if current.rules.release != ReleaseMode::Fixed {
                        rect {
                            x: "{map_x(current.rules.zone.min.0):.2}",
                            y: "{map_y(current.rules.zone.max.1):.2}",
                            width: "{(map_x(current.rules.zone.max.0) - map_x(current.rules.zone.min.0)):.2}",
                            height: "{(map_y(current.rules.zone.min.1) - map_y(current.rules.zone.max.1)):.2}",
                            class: "release-zone",
                        }
                    }
                    circle {
                        cx: "{map_x(release.0):.2}", cy: "{map_y(release.1):.2}",
                        r: if current.rules.release == ReleaseMode::Fixed { "6" } else { "7" },
                        class: "point-a release-handle",
                    }
                    if !visibility.reveals_beacon_before_launch()
                        && !reveal_beacons
                        && !current.beacons.is_empty()
                    {
                        text {
                            x: "16", y: "26", class: "blind-note",
                            "balise cachée · la trajectoire se révèle en vol",
                        }
                    }
                    if !visibility.keeps_recorded_paths() && !has_launched {
                        text {
                            x: "16", y: "26", class: "blind-note",
                            "trajectoire cachée · les flèches suffisent",
                        }
                    }
                    for (index, beacon) in current
                        .beacons
                        .iter()
                        .enumerate()
                        .filter(|_| reveal_beacons)
                    {
                        ellipse {
                            key: "beacon-{index}",
                            cx: "{map_x(beacon.0):.2}", cy: "{map_y(beacon.1):.2}",
                            rx: "{(WIN_R * X_SCALE):.2}", ry: "{(WIN_R * Y_SCALE):.2}",
                            class: if index < visited_beacons {
                                "point-b beacon-done"
                            } else {
                                "point-b"
                            },
                        }
                        if beacons_total > 1 {
                            text {
                                key: "beacon-label-{index}",
                                x: "{map_x(beacon.0):.2}", y: "{map_y(beacon.1) - 14.0:.2}",
                                class: "beacon-label", text_anchor: "middle",
                                "{index + 1}"
                            }
                        }
                    }
                    for (index, ghost) in ghost_paths.iter().enumerate() {
                        path {
                            key: "ghost-{index}",
                            d: "{ghost}",
                            class: "path ghost-path",
                        }
                    }
                    for (index, (history_path, _, _)) in history_paths.iter().enumerate() {
                        if !history_path.is_empty() {
                            path {
                                key: "history-{index}",
                                d: "{history_path}",
                                class: "path history-path",
                            }
                        }
                    }
                    if !blind_path_d.is_empty() {
                        path {
                            d: "{blind_path_d}",
                            class: "path blind-path",
                        }
                    }
                    path {
                        path_length: "1",
                        d: "{shown_path_d}",
                        class: path_class,
                    }
                    if let Some(closest) = closest_marker {
                        circle {
                            cx: "{map_x(closest.point.0):.2}",
                            cy: "{map_y(closest.point.1):.2}",
                            r: "6", class: closest_class,
                        }
                    }
                    if let Some(point) = collision_point {
                        circle {
                            cx: "{map_x(point.0):.2}", cy: "{map_y(point.1):.2}",
                            r: "6", class: collision_class,
                        }
                    }
                }

                div { class: "seed-row",
                    button {
                        r#type: "button",
                        class: "action small-button",
                        onclick: move |_| {
                            spawn(async move {
                                let mut next_seed = entropy_seed().await;
                                if next_seed == active_seed() {
                                    next_seed = next_seed.wrapping_add(1);
                                }
                                let variant = tutorial_variant();
                                let next_levels = chapter_slots(next_seed, 0, variant);
                                active_seed.set(next_seed);
                                copy_status.set(String::new());
                                tutorial_seen.set(vec![false; chapter_count()]);
                                replace_levels(all_levels, level_signals, next_levels);
                            });
                        },
                        "Nouvelle zone"
                    }
                    button {
                        r#type: "button",
                        class: "ghost small-button",
                        onclick: move |_| {
                            let seed = active_seed();
                            copy_status.set(format!("Graine copiée : {}.", format_seed(seed)));
                            copy_seed(seed);
                        },
                        "Copier la graine"
                    }
                    label { class: "variant-label", "Tutoriel" }
                    select {
                        class: "step-select",
                        value: "{variant_label}",
                        onchange: move |event| {
                            let Some(variant) = ["A", "B", "C"]
                                .iter()
                                .position(|label| *label == event.value())
                            else {
                                return;
                            };
                            reroll_variant(
                                all_levels,
                                level_signals,
                                tutorial_seen,
                                tutorial_variant,
                                variant,
                            );
                        },
                        for (index, label) in ["A", "B", "C"].iter().enumerate() {
                            option {
                                key: "variant-{index}",
                                value: "{label}",
                                "variante {label}"
                            }
                        }
                    }
                    if !copy_status().is_empty() {
                        span { class: "seed-status", "{copy_status()}" }
                    }
                }
                div { class: "controls",
                    label { "Intensité du courant" }
                    div { class: "stepper",
                        button {
                            r#type: "button",
                            class: "step-button",
                            disabled: !intensity_editable,
                            onclick: move |_| {
                                let next = normalize_intensity(
                                    k() - current_step,
                                    current.k_min,
                                    current.k_max,
                                    current_step,
                                );
                                if (next - k()).abs() > f64::EPSILON {
                                    archive_active_attempt(&mut progress, current_level_idx);
                                }
                                k.set(next);
                            },
                            "−"
                        }
                        input {
                            r#type: "number",
                            class: "intensity-input",
                            disabled: !intensity_editable,
                            min: "{current.k_min}",
                            max: "{current.k_max}",
                            step: "{current_step}",
                            value: "{format_intensity(k(), current_step)}",
                            oninput: move |event| {
                                if let Ok(value) = event.value().parse::<f64>() {
                                    let next = normalize_intensity(
                                        value,
                                        current.k_min,
                                        current.k_max,
                                        current_step,
                                    );
                                    if (next - k()).abs() > f64::EPSILON {
                                        archive_active_attempt(&mut progress, current_level_idx);
                                    }
                                    k.set(next);
                                }
                            },
                        }
                        button {
                            r#type: "button",
                            class: "step-button",
                            disabled: !intensity_editable,
                            onclick: move |_| {
                                let next = normalize_intensity(
                                    k() + current_step,
                                    current.k_min,
                                    current.k_max,
                                    current_step,
                                );
                                if (next - k()).abs() > f64::EPSILON {
                                    archive_active_attempt(&mut progress, current_level_idx);
                                }
                                k.set(next);
                            },
                            "+"
                        }
                    }
                    div { class: "step-heading",
                        label { class: "step-label", "Pas" }
                        if current.k_window > 0.0 {
                            span { class: "window-hint", "{window_hint}" }
                        }
                    }
                    select {
                        class: "step-select",
                        disabled: !intensity_editable,
                        value: "{format_intensity(current_step, current_step)}",
                        onchange: move |event| {
                            if let Some(index) = event
                                .value()
                                .parse::<f64>()
                                .ok()
                                .and_then(|value| {
                                    STEPS
                                        .iter()
                                        .position(|step| (step - value).abs() < 1e-9)
                                })
                            {
                                let next_step = STEPS[index];
                                let next = normalize_intensity(
                                    k(),
                                    current.k_min,
                                    current.k_max,
                                    next_step,
                                );
                                if (next - k()).abs() > f64::EPSILON {
                                    archive_active_attempt(&mut progress, current_level_idx);
                                }
                                k.set(next);
                                step_idx.set(index);
                            }
                        },
                        for step in STEPS {
                            option {
                                value: "{format_intensity(step, step)}",
                                "{format_intensity(step, step)}"
                            }
                        }
                    }
                    input {
                        r#type: "range",
                        class: "intensity-range",
                        disabled: !intensity_editable,
                        min: "{current.k_min}",
                        max: "{current.k_max}",
                        step: "{current_step}",
                        value: "{k()}",
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<f64>() {
                                let next = normalize_intensity(
                                    value,
                                    current.k_min,
                                    current.k_max,
                                    current_step,
                                );
                                if (next - k()).abs() > f64::EPSILON {
                                    archive_active_attempt(&mut progress, current_level_idx);
                                }
                                k.set(next);
                            }
                        },
                    }
                    if !trace_ticks.is_empty() {
                        div { class: "trace-scale",
                            span { class: "trace-label", "essais gardés" }
                            div { class: "trace-track",
                                for (index, (offset, reached)) in trace_ticks.iter().enumerate() {
                                    div {
                                        key: "trace-{index}",
                                        class: if *reached {
                                            "trace-tick won"
                                        } else {
                                            "trace-tick"
                                        },
                                        style: "left:{offset}%",
                                    }
                                }
                            }
                        }
                    }
                }

                if show_dial {
                    div { class: "controls",
                        div { class: "step-heading",
                            label { class: "step-label", "Phase du champ" }
                            span { class: "window-hint", "{phase_readout}" }
                        }
                        input {
                            r#type: "range",
                            class: "intensity-range phase-range",
                            min: "{base_level.phase_min}",
                            max: "{base_level.phase_max}",
                            step: "{phase_step}",
                            value: "{phase()}",
                            disabled: !knobs.phase,
                            oninput: move |event| {
                                if let Ok(value) = event.value().parse::<f64>() {
                                    if (value - phase()).abs() > f64::EPSILON {
                                        archive_active_attempt(
                                            &mut progress,
                                            current_level_idx,
                                        );
                                    }
                                    phase.set(value);
                                }
                            },
                        }
                    }
                }

                div { class: "btnrow",
                    button {
                        class: "action",
                        onclick: move |_| {
                            let launch_k = normalize_intensity(
                                k(),
                                launch_level.k_min,
                                launch_level.k_max,
                                current_step,
                            );
                            k.set(launch_k);
                            let result = integrate_from(&launch_level, release, launch_k);
                            let won = result.reached();
                            animation_visible.set(false);
                            animation_id.set(animation_id() + 1);
                            archive_active_attempt(&mut progress, current_level_idx);
                            let mut levels = progress();
                            if let Some(level) = levels.get_mut(current_level_idx) {
                                level.active_attempt = Some(Attempt {
                                    k: launch_k,
                                    result,
                                });
                                if won {
                                    level.solved = true;
                                }
                            }
                            progress.set(levels);
                        },
                        "{launch_label}"
                    }
                    button {
                        class: "ghost",
                        onclick: move |_| {
                            reset_level_progress(&mut progress, current_level_idx);
                            let mut levels = progress();
                            if let Some(entry) = levels.get_mut(current_level_idx) {
                                entry.release = None;
                            }
                            progress.set(levels);
                            animation_visible.set(false);
                        },
                        "Réinitialiser"
                    }
                }

                if let Some((text, won)) = active_message {
                    div {
                        class: if won { "msg win" } else { "msg fail" },
                        "{text}"
                    }
                }

                if !history_paths.is_empty() {
                    div { class: "history",
                        span { class: "history-title", "Essais précédents" }
                        for (index, (_, label, won)) in history_paths.iter().enumerate() {
                            span {
                                key: "history-label-{index}",
                                class: if *won {
                                    "history-entry success"
                                } else {
                                    "history-entry"
                                },
                                "intensité {label} · {attempt_status(*won)}"

                            }
                        }
                    }
                }

                div { class: "legend",
                    span { span { class: "dot dot-a" } " Largage" }
                    span { span { class: "dot dot-b" } " Balise" }
                    span { span { class: "dot dot-o" } " Astéroïde" }
                    if !ghost_paths.is_empty() {
                        span { span { class: "dot dot-g" } " Fils voisins (échouent)" }
                    }
                    if !visibility.reveals_beacon_before_launch() && !reveal_beacons {
                        span { span { class: "dot dot-c" } " Balise cachée" }
                    }
                    if closest_marker.is_some() {
                        span { span { class: "dot dot-c" } " Passage le plus proche" }
                    }
                    if collision_point.is_some() {
                        span { span { class: "dot dot-i" } " Point d'impact" }
                    }
                }
            }
        }
    }
}

const STYLE: &str = r#"
:root{--bg:#0f1420;--panel:#171f30;--ink:#e8ecf4;--sub:#93a1c0;--accent:#5ee3c9;--accent2:#ff8b6b;--danger:#ff5d7a;--line:#2a3550;--field:#7183b5;--closest:#ffd166}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);font-family:-apple-system,sans-serif}
.wrap{max-width:720px;margin:0 auto;padding:16px}
h1{font-size:1.3rem;margin:0 0 4px}
.sub{color:var(--sub);font-size:.88rem;line-height:1.5}
.panel{background:var(--panel);border:1px solid var(--line);border-radius:14px;padding:14px;margin-top:12px}
.level-nav{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:10px}
.level-nav .nav-button{padding:6px 10px;font-size:.8rem}
.level-nav .nav-button:disabled{opacity:.35;cursor:not-allowed}
.level-heading{display:flex;flex-direction:column;flex:1;min-width:150px}
.level-index{font-size:.9rem;font-weight:600}
.level-family{font-size:.72rem;color:var(--sub)}
.levels-menu{display:flex;flex-direction:column;gap:6px;background:var(--bg);border:1px solid var(--line);border-radius:10px;padding:8px;margin-bottom:10px}
.levels-group{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.levels-group-title{font-size:.7rem;color:var(--sub);min-width:96px;text-transform:uppercase;letter-spacing:.04em}
.levels-group-row{display:flex;gap:4px;flex-wrap:wrap}
.levels-group button{width:30px;height:30px;border-radius:8px;border:1px solid var(--line);background:transparent;color:var(--sub);font-size:.78rem;cursor:pointer}
.levels-group button.solved{border-color:var(--accent);color:var(--accent)}
.levels-group button.open{color:var(--ink)}
.levels-group button.locked{opacity:.35;cursor:not-allowed}
.levels-group button.active{background:var(--accent);color:#04231d;border-color:var(--accent);font-weight:600}
.seed-row{display:flex;align-items:center;gap:7px;margin-bottom:10px;flex-wrap:wrap}
.small-button{padding:7px 10px;font-size:.78rem}
.seed-status{font-size:.76rem;color:var(--accent)}
.desc{font-size:.85rem;color:var(--sub);margin-bottom:10px;line-height:1.5}
.field-svg{width:100%;height:auto;background:var(--bg);border:1px solid var(--line);border-radius:10px}
.release-zone{fill:var(--accent);opacity:.07;stroke:var(--accent);stroke-opacity:.55;stroke-width:1;stroke-dasharray:5 4}
.release-handle{stroke:var(--accent);stroke-width:2}
.arrow{stroke:var(--field);stroke-width:1.2}
.arrow-head{fill:var(--field)}
.obstacle{fill:var(--accent2);opacity:.35}
.point-a{fill:#4ade80}
.beacon-done{fill:rgba(94,227,201,.16);stroke:var(--accent)}
.beacon-label{fill:var(--ink);font-size:11px;font-weight:600;paint-order:stroke;stroke:var(--bg);stroke-width:2px}
.point-b{fill:var(--danger)}
.closest-point{fill:var(--closest);stroke:var(--bg);stroke-width:2}
.dot-g{background:var(--accent2)}
.collision-point{fill:var(--danger);stroke:#fff;stroke-width:2}
.path{fill:none;stroke:var(--accent);stroke-width:2.4}
.active-path{stroke-dasharray:1;stroke-dashoffset:1;animation:draw-path 1.2s ease-out forwards}
.diagnostic-point{opacity:0;animation:reveal-point .1s ease-out 1.1s forwards}
.history-path{stroke:var(--sub);stroke-width:1.6;stroke-dasharray:5 6;opacity:.55}
.ghost-path{stroke:var(--accent2);stroke-width:1.6;stroke-dasharray:2 5;opacity:.5}
.blind-path{stroke:var(--sub);stroke-width:1.4;stroke-dasharray:3 6;opacity:.4}
.blind-note{fill:var(--sub);font-size:12px;letter-spacing:.4px}
.trace-scale{display:flex;align-items:center;gap:8px;flex:1 0 100%;min-width:140px;margin-top:2px}
.trace-label{font-size:10px;letter-spacing:1.2px;text-transform:uppercase;color:var(--sub);
  white-space:nowrap}
.trace-track{position:relative;flex:1;height:9px;border-bottom:1px solid var(--line)}
.trace-tick{position:absolute;bottom:0;width:2px;height:6px;background:var(--sub);
  transform:translateX(-1px);opacity:.75}
.trace-tick.won{background:var(--accent);height:9px}
.tutorial-banner{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin:10px 0 2px;
  padding:10px 12px;border:1px solid var(--accent);border-radius:8px;background:rgba(94,227,201,.08)}
.tutorial-badge{font-size:11px;letter-spacing:1.4px;text-transform:uppercase;color:var(--accent);
  border:1px solid var(--accent);border-radius:999px;padding:2px 8px}
.tutorial-hint{flex:1 1 240px;color:var(--ink)}
.tutorial-gate{color:var(--sub);font-size:12px}
.knob-note{color:var(--sub);font-size:12px;margin:2px 0 6px}
.variant-label{color:var(--sub);font-size:12px}
.phase-range{accent-color:var(--accent2)}
.controls{display:flex;align-items:center;gap:10px;margin-top:12px;flex-wrap:wrap}
.controls label{font-size:.85rem;color:var(--sub)}
.controls>label:first-child{min-width:140px}
.stepper{display:flex;align-items:center;border:1px solid var(--line);border-radius:9px;overflow:hidden}
.step-button{width:34px;height:34px;border:0;background:var(--line);color:var(--ink);font-size:1.1rem;cursor:pointer}
.step-button:hover{background:var(--accent);color:#04231d}
.intensity-input{width:74px;height:34px;border:0;background:var(--bg);color:var(--ink);padding:0 7px;text-align:center;font:inherit;font-variant-numeric:tabular-nums}
.intensity-input:focus,.step-select:focus{outline:2px solid var(--accent);outline-offset:-2px}
.step-heading{display:flex;flex-direction:column;gap:2px}
.window-hint{font-size:.68rem;color:var(--closest)}
.step-label{margin-left:auto!important}
.step-select{height:34px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--ink);padding:0 7px}
.intensity-range{flex:1 0 100%;min-width:140px;accent-color:var(--accent)}
.btnrow{display:flex;gap:8px;margin-top:12px;flex-wrap:wrap}
.action{background:var(--accent);color:#04231d;border:none;padding:9px 16px;border-radius:9px;font-weight:600;cursor:pointer}
.next{background:var(--accent2)!important}
.ghost{background:transparent;border:1px solid var(--line);color:var(--ink);padding:9px 16px;border-radius:9px;cursor:pointer}
.msg{margin-top:10px;font-size:.9rem;font-weight:600;line-height:1.45}
.msg.win{color:var(--accent)}
.msg.fail{color:var(--danger)}
.history{display:flex;gap:7px;align-items:center;flex-wrap:wrap;margin-top:10px;font-size:.76rem;color:var(--sub)}
.history-title{font-weight:600;color:var(--ink)}
.history-entry{border:1px solid var(--line);border-radius:12px;padding:3px 8px}
.history-entry.success{border-color:var(--accent);color:var(--accent)}
.legend{display:flex;gap:14px;font-size:.78rem;color:var(--sub);margin-top:8px;flex-wrap:wrap}
.dot{display:inline-block;width:9px;height:9px;border-radius:50%;vertical-align:middle}
.dot-a{background:#4ade80}
.dot-b{background:var(--danger)}
.dot-o{background:var(--accent2)}
.dot-c{background:var(--closest)}
.dot-i{background:#fff}
@keyframes draw-path{to{stroke-dashoffset:0}}
@keyframes reveal-point{to{opacity:1}}
@media (prefers-reduced-motion: reduce){.active-path{animation:none;stroke-dashoffset:0}.diagnostic-point{animation:none;opacity:1}}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use peoplemodeler_core::{total_level_count, Knobs};

    fn attempt(k: f64) -> Attempt {
        Attempt {
            k,
            result: SimResult {
                points: vec![(0.0, 0.0)],
                outcome: Outcome::TimeLimit,
                closest: None,
                visited: 1,
            },
        }
    }

    #[test]
    fn intensity_is_clamped_and_quantized() {
        assert_eq!(normalize_intensity(4.0, -1.0, 1.0, 0.05), 1.0);
        assert_eq!(normalize_intensity(0.123, -1.0, 1.0, 0.01), 0.12);
        assert_eq!(normalize_intensity(-0.001, -1.0, 1.0, 0.001), -0.001);
    }

    #[test]
    fn history_keeps_only_three_attempts() {
        let mut history = Vec::new();
        for k in 0..5 {
            push_attempt(&mut history, attempt(k as f64));
        }
        assert_eq!(history.len(), HISTORY_LIMIT);
        assert_eq!(history[0].k, 2.0);
        assert_eq!(history[2].k, 4.0);
    }

    #[test]
    fn zones_unlock_one_at_a_time() {
        let loaded = slot_count();
        let seen = vec![false; chapter_count()];
        let mut progress = vec![LevelProgress::default(); loaded];
        assert_eq!(unlocked_count(&progress, &seen, loaded), 1);
        progress[0].solved = true;
        assert_eq!(unlocked_count(&progress, &seen, loaded), 2);
        progress[2].solved = true;
        assert_eq!(unlocked_count(&progress, &seen, loaded), 2);
        progress[1].solved = true;
        assert_eq!(unlocked_count(&progress, &seen, loaded), 4);
    }

    #[test]
    fn opening_a_tutorial_unlocks_the_chapter_without_solving_it() {
        let loaded = slot_count();
        let door = slot_offset(1);
        assert_eq!(door, slot_size(0));
        let mut progress = vec![LevelProgress::default(); loaded];
        let mut seen = vec![false; chapter_count()];
        // Solving chapter 1 lands the player on chapter 2's teaching level: it
        // is the door, and it is always reachable.
        for level in progress.iter_mut().take(door) {
            level.solved = true;
        }
        assert!(slot_unlocked(&progress, &seen, door, loaded));
        // The real levels behind it stay shut until the lesson is met.
        assert!(!slot_unlocked(&progress, &seen, door + 1, loaded));
        // Skipping it is allowed: opening the tutorial is the whole price.
        seen[1] = true;
        assert!(slot_unlocked(&progress, &seen, door, loaded));
        assert!(slot_unlocked(&progress, &seen, door + 1, loaded));
        // Chapter 3 has not been met, so its door is still shut.
        assert!(!slot_unlocked(&progress, &seen, slot_offset(2), loaded));
    }

    #[test]
    fn a_tutorial_slot_is_the_head_of_its_chapter() {
        for chapter in 0..chapter_count() {
            assert!(slot_is_tutorial(slot_offset(chapter)));
            assert!(!slot_is_tutorial(slot_offset(chapter) + 1));
            assert_eq!(slot_chapter(slot_offset(chapter)), chapter);
        }
        assert_eq!(slot_count(), total_level_count() + chapter_count());
    }

    #[test]
    fn a_chapter_opens_with_its_teaching_level() {
        let chapter = Chapter::all()[0];
        let slots = chapter_slots(DEFAULT_SEED, 0, 0);
        assert_eq!(slots.len(), slot_size(0));
        let teaching = &slots[0];
        assert_eq!(teaching.chapter, chapter);
        assert_eq!(teaching.step_index, 0);
        for (index, level) in slots.iter().enumerate().skip(1) {
            assert_eq!(level.chapter, chapter);
            assert_eq!(level.step_index, index - 1);
            assert!(level.knobs() != Knobs::NONE);
        }
    }

    #[test]
    fn the_save_round_trips_the_durable_facts() {
        let mut progress = vec![LevelProgress::default(); 3];
        progress[0].solved = true;
        progress[1].release = Some((-2.5, 1.25));
        let seen = vec![true, false, true];
        let data = save_data(0xABCD, 1, &progress, &seen);
        let json = serde_json::to_string(&data).expect("serialisable");
        let back: SaveData = serde_json::from_str(&json).expect("reversible");
        assert_eq!(back.seed, Some(0xABCD));
        assert_eq!(back.variant, Some(1));
        assert!(back.progress[0].solved);
        assert_eq!(back.progress[1].release, Some((-2.5, 1.25)));
        assert_eq!(back.seen, seen);
        // A save from an older build has to load, not fail.
        let empty: SaveData = serde_json::from_str("{}").expect("defaults");
        assert!(empty.progress.is_empty());
        assert!(empty.seen.is_empty());
    }

    #[test]
    fn unlock_stops_at_the_loaded_levels() {
        let loaded = slot_size(0);
        let seen = vec![false; chapter_count()];
        let mut progress = vec![LevelProgress::default(); loaded];
        for level in progress.iter_mut().take(loaded - 1) {
            level.solved = true;
        }
        assert_eq!(unlocked_count(&progress, &seen, loaded - 1), loaded - 1);
    }

    #[test]
    fn level_button_states_are_distinct() {
        assert_eq!(level_button_class(true, true, false), "active");
        assert_eq!(level_button_class(false, false, false), "locked");
        assert_eq!(level_button_class(false, true, true), "solved");
        assert_eq!(level_button_class(false, true, false), "open");
    }

    #[test]
    fn multi_beacon_failures_report_partial_progress() {
        let result = SimResult {
            points: vec![(0.0, 0.0)],
            outcome: Outcome::TimeLimit,
            closest: None,
            visited: 2,
        };
        let (message, won) = result_message(&result, 3);
        assert!(!won);
        assert!(message.starts_with("2 balise(s) sur 3."));
        let (_, won) = result_message(&result, 1);
        assert!(!won);
    }

    fn calm_test_level() -> Level {
        Level {
            title: "test",
            desc: "test",
            focus: "test",
            chapter: Chapter::Predict,
            rules: peoplemodeler_core::LevelRules::default(),
            step_index: 0,
            field: peoplemodeler_core::FlowField::Calm {
                drift_x: -2.0,
                baseline_y: 1.0,
                gain_y: 3.0,
            },
            k_min: -1.0,
            k_max: 1.0,
            k_def: 0.0,
            k_solution: 0.0,
            phase_min: 0.0,
            phase_max: 0.0,
            phase_def: 0.0,
            phase_sweep: 1.0,
            recommended_step: 0.5,
            k_window: 0.0,
            a: (-4.5, 0.0),
            default_release: (-4.5, 0.0),
            ghosts: Vec::new(),
            beacons: Vec::new(),
            obstacles: Vec::new(),
        }
    }

    #[test]
    fn the_position_chapter_locks_the_intensity() {
        let levels: Vec<Level> = (0..chapter_count())
            .flat_map(|group| chapter_slots(DEFAULT_SEED, group, 0))
            .collect();
        let position: Vec<&Level> = levels
            .iter()
            .filter(|level| level.chapter == Chapter::Position)
            .collect();
        assert!(!position.is_empty(), "aucun niveau de Position");
        for level in position {
            assert!(
                !intensity_editable(level),
                "l'intensité reste éditable sur {}",
                level.focus
            );
            // Locked means locked at the intensity the level was solved at: the
            // default the dial starts from is not a winning setting here.
            assert_eq!(
                launch_intensity(level),
                level.k_solution,
                "lancement à la mauvaise intensité sur {}",
                level.focus
            );
        }
        // The lock is the chapter's, not the generator's: another chapter that
        // leaves the dial live must still let the player move it.
        let other: Vec<&Level> = levels
            .iter()
            .filter(|level| level.chapter != Chapter::Position && level.knobs().intensity)
            .collect();
        assert!(!other.is_empty(), "aucun autre chapitre à intensité libre");
        for level in other {
            assert!(
                intensity_editable(level),
                "intensité bloquée sur {}",
                level.focus
            );
            assert_eq!(
                launch_intensity(level),
                level.k_def,
                "lancement hors du réglage par défaut sur {}",
                level.focus
            );
        }
    }

    #[test]
    fn seeds_are_sixteen_hex_digits() {
        let seed = 0x1234_5678_9ABC_DEF0;
        assert_eq!(format_seed(seed), "123456789abcdef0");
        assert_eq!(format_seed(1), "0000000000000001");
    }

    #[test]
    fn svg_coordinates_invert_the_field_mapping() {
        for world in [
            (XMIN, YMIN),
            (XMAX, YMAX),
            (0.0, 0.0),
            (-4.5, 1.25),
            (3.75, -2.0),
        ] {
            let (px, py) = (map_x(world.0), map_y(world.1));
            assert!((px - 0.0).abs() < 1e-9 || (px - W).abs() < 1e-9 || (0.0..=W).contains(&px));
            assert!((0.0..=H).contains(&py));
            let back = (invert_map_x(px), invert_map_y(py));
            assert!((back.0 - world.0).abs() < 1e-9, "{world:?} -> {back:?}");
            assert!((back.1 - world.1).abs() < 1e-9, "{world:?} -> {back:?}");
        }
        assert!((invert_map_y(0.0) - YMAX).abs() < 1e-9);
        assert!((invert_map_y(H) - YMIN).abs() < 1e-9);
    }

    #[test]
    fn fixed_release_levels_ignore_the_stored_point() {
        let level = calm_test_level();
        let progress = LevelProgress {
            release: Some((0.0, 1.0)),
            ..LevelProgress::default()
        };
        assert_eq!(release_of(&level, &progress), level.default_release);
        assert!(!accepts_release(&level, (0.0, 1.0)));
        assert!(!accepts_release(&level, level.default_release));
    }

    #[test]
    fn zone_release_levels_only_accept_points_inside() {
        let mut level = calm_test_level();
        level.rules.release = ReleaseMode::Zone;
        level.rules.zone = peoplemodeler_core::Rect::new((-3.0, -1.0), (-1.0, 1.0));
        let default = release_of(&level, &LevelProgress::default());
        assert_eq!(default, level.default_release);
        assert!(accepts_release(&level, (-2.0, 0.0)));
        assert!(accepts_release(&level, (-3.0, -1.0)));
        assert!(!accepts_release(&level, (-0.5, 0.0)));
        assert!(!accepts_release(&level, (-2.0, 1.5)));
        let progress = LevelProgress {
            release: Some((-2.0, 0.5)),
            ..LevelProgress::default()
        };
        assert_eq!(release_of(&level, &progress), (-2.0, 0.5));
    }

    #[test]
    fn the_pick_script_returns_its_value() {
        // `eval` wraps the script as `return (async function(){ <script> })()`, so a
        // script without its own `return` resolves to `undefined` and the click is dropped.
        let script = field_pick_script(120.0, 340.0);
        assert!(script.contains("return ["), "{script}");
        assert!(!script.contains("=>"), "{script}");
        assert!(script.contains("120"), "{script}");
        assert!(script.contains("340"), "{script}");
        assert!(script.contains(&format!("* {W}")), "{script}");
        assert!(script.contains(&format!("* {H}")), "{script}");
    }
}
