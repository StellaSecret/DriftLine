#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use dioxus::prelude::*;
use peoplemodeler_core::{
    chapter_count, chapter_offset, chapter_size, generate_level_group, integrate_from,
    total_level_count, Chapter, Level, Outcome, ReleaseMode, SimResult, SimulationMode, Vector2,
    DEFAULT_SEED, WIN_R, XMAX, XMIN, YMAX, YMIN,
};

const W: f64 = 640.0;
const H: f64 = 420.0;
const X_SCALE: f64 = W / (XMAX - XMIN);
const Y_SCALE: f64 = H / (YMAX - YMIN);
const STEPS: [f64; 6] = [1.0, 0.5, 0.1, 0.05, 0.01, 0.001];
const HISTORY_LIMIT: usize = 3;
const HINT_LEN_PX: f64 = 44.0;

#[derive(Clone, Debug)]
struct Attempt {
    k: f64,
    result: SimResult,
}

#[derive(Clone, Copy)]
struct LevelChip {
    index: usize,
    class: &'static str,
    disabled: bool,
}

#[derive(Clone, Debug, Default)]
struct LevelProgress {
    history: Vec<Attempt>,
    active_attempt: Option<Attempt>,
    attempts: usize,
    solved: bool,
    release: Option<(f64, f64)>,
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
    level.rules.release != ReleaseMode::Fixed && level.rules.zone.contains(point)
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

fn launch_heading(level: &Level, a: (f64, f64), k: f64) -> (f64, f64, f64, f64) {
    let Vector2 { x: vx, y: vy } = level.flow_at(a.0, a.1, k);
    let length = Vector2 { x: vx, y: vy }.length();
    let (ux, uy) = if length <= f64::EPSILON {
        (1.0, 0.0)
    } else {
        (vx / length, vy / length)
    };
    let (sx, sy) = (map_x(a.0), map_y(a.1));
    let (dx, dy) = (map_x(a.0 + ux) - sx, map_y(a.1 + uy) - sy);
    let length = (dx * dx + dy * dy).sqrt();
    if length <= f64::EPSILON {
        return (sx, sy, sx + HINT_LEN_PX, sy);
    }
    (
        sx,
        sy,
        sx + dx / length * HINT_LEN_PX,
        sy + dy / length * HINT_LEN_PX,
    )
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
        signals.k.set(level.k_def);
        signals.step_idx.set(step_index(level.recommended_step));
    }
    signals.animation_visible.set(false);
}

fn open_level(levels: Signal<Vec<Level>>, mut signals: LevelSignals, index: usize) {
    if index >= unlocked_count(&(signals.progress)(), levels().len()) {
        return;
    }
    let Some(level) = levels().get(index).cloned() else {
        return;
    };
    signals.level_idx.set(index);
    signals.k.set(level.k_def);
    signals.step_idx.set(step_index(level.recommended_step));
    signals.animation_visible.set(false);
}

fn append_group(
    mut all_levels: Signal<Vec<Level>>,
    mut signals: LevelSignals,
    seed: u64,
    group: usize,
) {
    let mut levels = all_levels();
    levels.extend(generate_level_group(seed, group));
    all_levels.set(levels);
    let mut progress = (signals.progress)();
    progress.resize(
        progress.len() + chapter_size(group),
        LevelProgress::default(),
    );
    signals.progress.set(progress);
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

fn history_limit(mode: SimulationMode) -> usize {
    match mode {
        SimulationMode::Laboratory => HISTORY_LIMIT,
        SimulationMode::Exploration => usize::MAX,
    }
}

fn should_show_vectors(mode: SimulationMode, chapter: Chapter) -> bool {
    mode == SimulationMode::Laboratory || chapter == Chapter::Discover
}

fn chapter_index_of(level_index: usize) -> usize {
    (0..chapter_count())
        .find(|group| level_index < chapter_offset(*group) + chapter_size(*group))
        .unwrap_or_else(|| chapter_count() - 1)
}

fn solved_prefix(progress: &[LevelProgress]) -> usize {
    progress.iter().take_while(|level| level.solved).count()
}

fn unlocked_count(progress: &[LevelProgress], loaded: usize) -> usize {
    (solved_prefix(progress) + 1).min(loaded)
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

fn push_attempt(history: &mut Vec<Attempt>, attempt: Attempt, limit: usize) {
    if limit != usize::MAX && history.len() >= limit {
        history.remove(0);
    }
    history.push(attempt);
}

fn archive_active_attempt(
    progress: &mut Signal<Vec<LevelProgress>>,
    level_idx: usize,
    mode: SimulationMode,
) {
    let mut levels = progress();
    if let Some(level) = levels.get_mut(level_idx) {
        if let Some(attempt) = level.active_attempt.take() {
            push_attempt(&mut level.history, attempt, history_limit(mode));
        }
    }
    progress.set(levels);
}

fn reset_level_progress(
    progress: &mut Signal<Vec<LevelProgress>>,
    level_idx: usize,
    reset_attempts: bool,
) {
    let mut levels = progress();
    if let Some(level) = levels.get_mut(level_idx) {
        level.history.clear();
        level.active_attempt = None;
        if reset_attempts {
            level.attempts = 0;
        }
    }
    progress.set(levels);
}

fn exploration_blocked(mode: SimulationMode, level: &Level, progress: &LevelProgress) -> bool {
    mode == SimulationMode::Exploration && progress.attempts >= level.exploration_attempts
}

fn result_message(
    result: &SimResult,
    mode: SimulationMode,
    beacons_total: usize,
) -> (String, bool) {
    let progress_prefix = if beacons_total > 1 && result.visited > 0 {
        format!("{} balise(s) sur {}. ", result.visited, beacons_total)
    } else {
        String::new()
    };
    match mode {
        SimulationMode::Exploration => match result.outcome {
            Outcome::Reached => (
                format!(
                    "{progress_prefix}Parcours réussi : la sonde a touché {} balise(s). Zone suivante débloquée.",
                    beacons_total
                ),
                true,
            ),
            Outcome::Collision { obstacle, .. } => (
                format!(
                    "{progress_prefix}La sonde a percuté l'astéroïde {}. {}",
                    obstacle + 1,
                    closest_message(result)
                ),
                false,
            ),
            Outcome::NumericalFailure => (
                "Le courant n'a pas pu être calculé. Réinitialise la simulation.".to_string(),
                false,
            ),
            _ => (
                format!("Exploration: parcours terminé. {}", closest_message(result)),
                false,
            ),
        },
        SimulationMode::Laboratory => match result.outcome {
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
        },
    }
}

fn attempt_status(mode: SimulationMode, won: bool) -> &'static str {
    match (mode, won) {
        (_, true) => "succès",
        (SimulationMode::Exploration, false) => "terminé",
        (SimulationMode::Laboratory, false) => "échec",
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
    let all_levels = use_signal(|| generate_level_group(DEFAULT_SEED, 0));
    let initial_k = all_levels()[0].k_def;
    let initial_step = all_levels()[0].recommended_step;
    let total_levels = total_level_count();
    let level_idx = use_signal(|| 0usize);
    let mut mode = use_signal(|| SimulationMode::Exploration);
    let mut k = use_signal(|| initial_k);
    let mut step_idx = use_signal(|| step_index(initial_step));
    let mut progress = use_signal(|| vec![LevelProgress::default(); chapter_size(0)]);
    let mut show_levels = use_signal(|| false);
    let mut animation_id = use_signal(|| 0usize);
    let mut animation_visible = use_signal(|| false);
    let mut active_seed = use_signal(|| DEFAULT_SEED);
    let mut copy_status = use_signal(String::new);
    let mut entropy_loaded = use_signal(|| false);
    let level_signals = LevelSignals {
        level_idx,
        k,
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
            let next_seed = entropy_seed().await;
            let next_levels = generate_level_group(next_seed, 0);
            active_seed.set(next_seed);
            copy_status.set(String::new());
            replace_levels(all_levels, level_signals, next_levels);
        });
    });

    use_effect(move || {
        let loaded = all_levels().len();
        let solved = solved_prefix(&progress());
        if loaded < total_levels && solved + 1 >= loaded {
            let group = chapter_index_of(loaded);
            let seed = active_seed();
            spawn(async move {
                append_group(all_levels, level_signals, seed, group);
            });
        }
    });

    let levels_snapshot = all_levels();
    let current_level_idx = level_idx();
    let current = levels_snapshot[current_level_idx].clone();
    let launch_level = current.clone();
    let clickable_level = current.clone();
    let current_mode = mode();
    let current_step = STEPS[step_idx()];
    let current_progress = progress()[current_level_idx].clone();
    let release = release_of(&current, &current_progress);
    let preview = integrate_from(&current, release, k());
    let description = if current_mode == SimulationMode::Exploration {
        format!("Exploration libre — {}", current.desc)
    } else {
        current.desc.to_string()
    };
    let run_blocked = exploration_blocked(current_mode, &current, &current_progress);
    let active_result = current_progress
        .active_attempt
        .as_ref()
        .map(|attempt| attempt.result.clone());
    let show_preview = should_show_vectors(current_mode, current.chapter);
    let has_launched = active_result.is_some();
    let direction_hint = if !show_preview && !has_launched {
        Some(launch_heading(&current, release, k()))
    } else {
        None
    };
    let shown_points = active_result
        .as_ref()
        .map(|result| result.points.clone())
        .unwrap_or_else(|| {
            if show_preview {
                preview.points.clone()
            } else {
                Vec::new()
            }
        });
    let path_d = path_to_svg(&shown_points);
    let history_snapshot = current_progress.history;
    let history_paths: Vec<(String, String, bool)> = history_snapshot
        .iter()
        .map(|attempt| {
            (
                path_to_svg(&attempt.result.points),
                format_intensity(attempt.k, *STEPS.last().unwrap()),
                attempt.result.reached(),
            )
        })
        .collect();
    let closest_marker = active_result.as_ref().and_then(|result| {
        if result.reached() {
            None
        } else {
            result.closest
        }
    });
    // Exploration hides the field and the launched path, but leaving the
    // player with only a bare distance number after a miss gave no way to
    // adjust intentionally. This shows the actual local current at the
    // closest-approach point, once per failed attempt — a real clue, not a
    // full reveal of the field.
    let closest_direction_hint = if current_mode == SimulationMode::Exploration {
        closest_marker.map(|closest| launch_heading(&current, closest.point, k()))
    } else {
        None
    };
    let collision_point = match active_result.as_ref().map(|result| result.outcome) {
        Some(Outcome::Collision { point, .. }) => Some(point),
        _ => None,
    };
    let active_message = active_result
        .as_ref()
        .map(|result| result_message(result, current_mode, current.beacons.len()));
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
    let unlocked = unlocked_count(&progress_snapshot, levels_snapshot.len());
    let can_go_previous = current_level_idx > 0;
    let can_go_next = current_level_idx + 1 < unlocked;
    let level_groups: Vec<(String, Vec<LevelChip>)> = (0..chapter_count())
        .filter_map(|group| {
            let first = chapter_offset(group);
            if first >= levels_snapshot.len() {
                return None;
            }
            let last = (first + chapter_size(group)).min(levels_snapshot.len());
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
                })
                .collect();
            Some((levels_snapshot[first].chapter.title().to_string(), chips))
        })
        .collect();
    let show_levels_menu = show_levels();
    let remaining_attempts =
        current.exploration_attempts - current_progress.attempts.min(current.exploration_attempts);
    let window_hint = if current.k_window > 0.0 {
        format!("marge mesurée ±{:.2}", current.k_window)
    } else {
        String::new()
    };
    let mode_value = if current_mode == SimulationMode::Exploration {
        "exploration"
    } else {
        "laboratory"
    };
    let lab_goal = if current.beacons.len() > 1 {
        "Atteindre toutes les balises"
    } else {
        "Atteindre la balise"
    };
    let mode_hint = match (
        current_mode,
        should_show_vectors(current_mode, current.chapter),
    ) {
        (SimulationMode::Laboratory, _) => format!("{lab_goal} · vecteurs actifs"),
        (SimulationMode::Exploration, true) => "Libre · vecteurs tutoriel".to_string(),
        (SimulationMode::Exploration, false) => "Libre · vecteurs masqués".to_string(),
    };
    let history_title = if current_mode == SimulationMode::Exploration {
        "Historique d'exploration"
    } else {
        "Essais précédents"
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
                                open_level(all_levels, level_signals, current_level_idx - 1);
                            }
                        },
                        "‹ Précédente"
                    }
                    div { class: "level-heading",
                        span { class: "level-index", "Zone {current_level_idx + 1} / {total_levels}" }
                        span { class: "level-family", "Chapitre {current.chapter.group_index() + 1} · {current.chapter.title()} · {current.focus} ({current.step_index + 1}/{current.chapter.size()})" }
                    }
                    button {
                        r#type: "button",
                        class: "ghost nav-button",
                        disabled: !can_go_next,
                        onclick: move |_| {
                            if can_go_next {
                                open_level(all_levels, level_signals, current_level_idx + 1);
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
                                                open_level(all_levels, level_signals, chip.index);
                                            },
                                            "{chip.index + 1}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "mode-row",
                    label { "Mode" }
                    select {
                        class: "mode-select",
                        value: mode_value,
                        onchange: move |event| {
                            let next_mode = match event.value().as_str() {
                                "exploration" => SimulationMode::Exploration,
                                _ => SimulationMode::Laboratory,
                            };
                            mode.set(next_mode);
                            let reset_attempts = next_mode == SimulationMode::Laboratory;
                            let mut levels = progress();
                            for level in levels.iter_mut() {
                                level.history.clear();
                                level.active_attempt = None;
                                if reset_attempts {
                                    level.attempts = 0;
                                }
                            }
                            progress.set(levels);
                            animation_visible.set(false);
                        },
                        option { value: "laboratory", "Laboratoire" }
                        option { value: "exploration", "Exploration" }
                    }
                    span { class: "mode-hint", "{mode_hint}" }
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
                                let next_levels = generate_level_group(next_seed, 0);
                                active_seed.set(next_seed);
                                copy_status.set(String::new());
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
                    if !copy_status().is_empty() {
                        span { class: "seed-status", "{copy_status()}" }
                    }
                }
                div { class: "desc", "{current.title} — {description}" }

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
                        marker {
                            id: "hint-arrow",
                            view_box: "0 0 10 10",
                            ref_x: "8",
                            ref_y: "5",
                            marker_width: "5",
                            marker_height: "5",
                            orient: "auto",
                            path { d: "M 0 0 L 10 5 L 0 10 z", class: "hint-arrow-head" }
                        }
                        marker {
                            id: "hint-arrow-closest",
                            view_box: "0 0 10 10",
                            ref_x: "8",
                            ref_y: "5",
                            marker_width: "5",
                            marker_height: "5",
                            orient: "auto",
                            path { d: "M 0 0 L 10 5 L 0 10 z", class: "closest-hint-arrow-head" }
                        }
                    }
                    if should_show_vectors(current_mode, current.chapter) {
                        for gx in field_grid_x() {
                            for gy in field_grid_y() {
                                if let Some((x1, y1, x2, y2)) =
                                    vector_endpoints(&current, gx, gy, k())
                                {
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
                    if let Some((sx, sy, ex, ey)) = direction_hint {
                        line {
                            x1: "{sx:.2}", y1: "{sy:.2}",
                            x2: "{ex:.2}", y2: "{ey:.2}",
                            class: "direction-hint",
                            marker_end: "url(#hint-arrow)",
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
                    for (index, beacon) in current.beacons.iter().enumerate() {
                        ellipse {
                            key: "beacon-{index}",
                            cx: "{map_x(beacon.0):.2}", cy: "{map_y(beacon.1):.2}",
                            rx: "{(WIN_R * X_SCALE):.2}", ry: "{(WIN_R * Y_SCALE):.2}",
                            class: if index < visited_beacons {
                                "point-b beacon-done"
                            } else if current_mode == SimulationMode::Laboratory {
                                "point-b"
                            } else {
                                "point-b reference-point"
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
                    for (index, (history_path, _, _)) in history_paths.iter().enumerate() {
                        path {
                            key: "history-{index}",
                            d: "{history_path}",
                            class: "path history-path",
                        }
                    }
                    path {
                        path_length: "1",
                        d: "{path_d}",
                        class: path_class,
                    }
                    if let Some(closest) = closest_marker {
                        circle {
                            cx: "{map_x(closest.point.0):.2}",
                            cy: "{map_y(closest.point.1):.2}",
                            r: "6", class: closest_class,
                        }
                    }
                    if let Some((sx, sy, ex, ey)) = closest_direction_hint {
                        line {
                            x1: "{sx:.2}", y1: "{sy:.2}",
                            x2: "{ex:.2}", y2: "{ey:.2}",
                            class: "closest-direction-hint",
                            marker_end: "url(#hint-arrow-closest)",
                        }
                    }
                    if let Some(point) = collision_point {
                        circle {
                            cx: "{map_x(point.0):.2}", cy: "{map_y(point.1):.2}",
                            r: "6", class: collision_class,
                        }
                    }
                }

                div { class: "controls",
                    label { "Intensité du courant" }
                    div { class: "stepper",
                        button {
                            r#type: "button",
                            class: "step-button",
                            onclick: move |_| {
                                let next = normalize_intensity(
                                    k() - current_step,
                                    current.k_min,
                                    current.k_max,
                                    current_step,
                                );
                                if (next - k()).abs() > f64::EPSILON {
                                    archive_active_attempt(&mut progress, current_level_idx, current_mode);
                                }
                                k.set(next);
                            },
                            "−"
                        }
                        input {
                            r#type: "number",
                            class: "intensity-input",
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
                                        archive_active_attempt(&mut progress, current_level_idx, current_mode);
                                    }
                                    k.set(next);
                                }
                            },
                        }
                        button {
                            r#type: "button",
                            class: "step-button",
                            onclick: move |_| {
                                let next = normalize_intensity(
                                    k() + current_step,
                                    current.k_min,
                                    current.k_max,
                                    current_step,
                                );
                                if (next - k()).abs() > f64::EPSILON {
                                    archive_active_attempt(&mut progress, current_level_idx, current_mode);
                                }
                                k.set(next);
                            },
                            "+"
                        }
                    }
                    div { class: "step-heading",
                        label { class: "step-label", "Pas" }
                        if current.k_window > 0.0 && current_mode == SimulationMode::Laboratory {
                            span { class: "window-hint", "{window_hint}" }
                        }
                    }
                    select {
                        class: "step-select",
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
                                    archive_active_attempt(&mut progress, current_level_idx, current_mode);
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
                                    archive_active_attempt(&mut progress, current_level_idx, current_mode);
                                }
                                k.set(next);
                            }
                        },
                    }
                }

                div { class: "btnrow",
                    button {
                        class: "action",
                        disabled: run_blocked,
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
                            archive_active_attempt(&mut progress, current_level_idx, current_mode);
                            let mut levels = progress();
                            if let Some(level) = levels.get_mut(current_level_idx) {
                                level.active_attempt = Some(Attempt {
                                    k: launch_k,
                                    result,
                                });
                                if current_mode == SimulationMode::Exploration {
                                    level.attempts += 1;
                                }
                                if won {
                                    level.solved = true;
                                }
                            }
                            progress.set(levels);
                        },
                        "Larguer la sonde"
                    }
                    button {
                        class: "ghost",
                        onclick: move |_| {
                            reset_level_progress(&mut progress, current_level_idx, true);
                            let mut levels = progress();
                            if let Some(entry) = levels.get_mut(current_level_idx) {
                                entry.release = None;
                            }
                            progress.set(levels);
                            animation_visible.set(false);
                        },
                        "Réinitialiser"
                    }
                    if current_mode == SimulationMode::Exploration {
                        span { class: "budget",
                            "Tentatives restantes : {remaining_attempts}/{current.exploration_attempts}"
                        }
                    }
                }

                if run_blocked {
                    div { class: "msg fail",
                        "Budget d'exploration épuisé pour cette zone. Passe en Laboratoire ou réinitialise la zone."
                    }
                }

                if let Some((text, won)) = active_message {
                    div {
                        class: if won {
                            "msg win"
                        } else if current_mode == SimulationMode::Exploration {
                            "msg explore"
                        } else {
                            "msg fail"
                        },
                        "{text}"
                    }
                }

                if !history_paths.is_empty() {
                    div { class: "history",
                        span { class: "history-title", "{history_title}" }
                        for (index, (_, label, won)) in history_paths.iter().enumerate() {
                            span {
                                key: "history-label-{index}",
                                class: if *won {
                                    "history-entry success"
                                } else {
                                    "history-entry"
                                },
                                "intensité {label} · {attempt_status(current_mode, *won)}"
                            }
                        }
                    }
                }

                div { class: "legend",
                    span { span { class: "dot dot-a" } " Largage" }
                    span { span { class: "dot dot-b" } " Balise" }
                    if current_mode == SimulationMode::Laboratory {
                        span { span { class: "dot dot-o" } " Astéroïde" }
                    } else {
                        span { span { class: "dot dot-b" } " Référence" }
                        span { span { class: "dot dot-o" } " Astéroïde" }
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
.mode-row{display:flex;align-items:center;gap:9px;margin-bottom:10px;flex-wrap:wrap}
.mode-row label{font-size:.85rem;color:var(--sub)}
.mode-select{height:32px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--ink);padding:0 8px}
.mode-hint{font-size:.78rem;color:var(--sub)}
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
.reference-point{opacity:.35;stroke-dasharray:3 4}
.direction-hint{stroke:#4ade80;stroke-width:2.4;stroke-linecap:round}
.hint-arrow-head{fill:#4ade80}
.closest-point{fill:var(--closest);stroke:var(--bg);stroke-width:2}
.closest-direction-hint{stroke:var(--closest);stroke-width:2.4;stroke-linecap:round}
.closest-hint-arrow-head{fill:var(--closest)}
.collision-point{fill:var(--danger);stroke:#fff;stroke-width:2}
.path{fill:none;stroke:var(--accent);stroke-width:2.4}
.active-path{stroke-dasharray:1;stroke-dashoffset:1;animation:draw-path 1.2s ease-out forwards}
.diagnostic-point{opacity:0;animation:reveal-point .1s ease-out 1.1s forwards}
.history-path{stroke:var(--sub);stroke-width:1.6;stroke-dasharray:5 6;opacity:.55}
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
.budget{font-size:.78rem;color:var(--sub);align-self:center}
.step-label{margin-left:auto!important}
.step-select{height:34px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--ink);padding:0 7px}
.intensity-range{flex:1 0 100%;min-width:140px;accent-color:var(--accent)}
.btnrow{display:flex;gap:8px;margin-top:12px;flex-wrap:wrap}
.action{background:var(--accent);color:#04231d;border:none;padding:9px 16px;border-radius:9px;font-weight:600;cursor:pointer}
.next{background:var(--accent2)!important}
.ghost{background:transparent;border:1px solid var(--line);color:var(--ink);padding:9px 16px;border-radius:9px;cursor:pointer}
.msg{margin-top:10px;font-size:.9rem;font-weight:600;line-height:1.45}
.msg.win{color:var(--accent)}
.msg.explore{color:var(--accent2)}
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
            push_attempt(
                &mut history,
                attempt(k as f64),
                history_limit(SimulationMode::Laboratory),
            );
        }
        assert_eq!(history.len(), HISTORY_LIMIT);
        assert_eq!(history[0].k, 2.0);
        assert_eq!(history[2].k, 4.0);
    }

    #[test]
    fn exploration_history_has_no_limit() {
        let mut history = Vec::new();
        for k in 0..5 {
            push_attempt(
                &mut history,
                attempt(k as f64),
                history_limit(SimulationMode::Exploration),
            );
        }
        assert_eq!(history.len(), 5);
    }

    #[test]
    fn laboratory_always_shows_the_field_vectors() {
        for chapter in Chapter::all() {
            assert!(
                should_show_vectors(SimulationMode::Laboratory, chapter),
                "{}",
                chapter.title()
            );
        }
    }

    #[test]
    fn only_the_first_chapter_teaches_the_field_vectors() {
        assert!(should_show_vectors(
            SimulationMode::Exploration,
            Chapter::Discover
        ));
        for chapter in Chapter::all().iter().skip(1) {
            assert!(
                !should_show_vectors(SimulationMode::Exploration, *chapter),
                "{}",
                chapter.title()
            );
        }
    }

    #[test]
    fn zones_unlock_one_at_a_time() {
        let mut progress = vec![LevelProgress::default(); total_level_count()];
        assert_eq!(unlocked_count(&progress, progress.len()), 1);
        progress[0].solved = true;
        assert_eq!(unlocked_count(&progress, progress.len()), 2);
        progress[2].solved = true;
        assert_eq!(unlocked_count(&progress, progress.len()), 2);
        progress[1].solved = true;
        assert_eq!(unlocked_count(&progress, progress.len()), 4);
    }

    #[test]
    fn unlock_stops_at_the_loaded_levels() {
        let mut progress = vec![LevelProgress::default(); chapter_size(0)];
        for level in progress.iter_mut().take(chapter_size(0) - 1) {
            level.solved = true;
        }
        assert_eq!(
            unlocked_count(&progress, chapter_size(0) - 1),
            chapter_size(0) - 1
        );
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
        let (message, won) = result_message(&result, SimulationMode::Laboratory, 3);
        assert!(!won);
        assert!(message.starts_with("2 balise(s) sur 3."));
        let (_, won) = result_message(&result, SimulationMode::Laboratory, 1);
        assert!(!won);
    }

    fn calm_test_level() -> Level {
        Level {
            title: "test",
            desc: "test",
            focus: "test",
            chapter: Chapter::Discover,
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
            recommended_step: 0.5,
            exploration_attempts: 1,
            k_window: 0.0,
            a: (-4.5, 0.0),
            default_release: (-4.5, 0.0),
            probes: Vec::new(),
            ghosts: Vec::new(),
            beacons: Vec::new(),
            obstacles: Vec::new(),
        }
    }

    #[test]
    fn launch_heading_follows_the_flow() {
        let level = calm_test_level();
        let (sx, sy, ex, ey) = launch_heading(&level, level.a, 0.5);
        let flow = level.flow_at(level.a.0, level.a.1, 0.5);
        let length = flow.length();
        let (ux, uy) = (flow.x / length, flow.y / length);
        let (px, py) = (
            map_x(level.a.0 + ux) - map_x(level.a.0),
            map_y(level.a.1 + uy) - map_y(level.a.1),
        );
        let plength = (px * px + py * py).sqrt();
        let (dx, dy) = (ex - sx, ey - sy);
        let dlength = (dx * dx + dy * dy).sqrt();
        assert!((dlength - HINT_LEN_PX).abs() < 1e-9);
        assert!(((sy - map_y(level.a.1)).abs()) < 1e-9);
        assert!((dx / dlength - px / plength).abs() < 1e-9);
        assert!((dy / dlength - py / plength).abs() < 1e-9);
    }

    #[test]
    fn launch_heading_moves_with_the_intensity() {
        let level = calm_test_level();
        let (_, _, ex_low, ey_low) = launch_heading(&level, level.a, 0.0);
        let (_, _, ex_high, ey_high) = launch_heading(&level, level.a, 1.0);
        assert!((ex_low - ex_high).abs() > 1.0 || (ey_low - ey_high).abs() > 1.0);
    }

    #[test]
    fn seeds_are_sixteen_hex_digits() {
        let seed = 0x1234_5678_9ABC_DEF0;
        assert_eq!(format_seed(seed), "123456789abcdef0");
        assert_eq!(format_seed(1), "0000000000000001");
    }

    #[test]
    fn exploration_wins_report_success() {
        let result = SimResult {
            points: vec![(0.0, 0.0)],
            outcome: Outcome::Reached,
            closest: None,
            visited: 2,
        };
        let (message, won) = result_message(&result, SimulationMode::Exploration, 2);
        assert!(won);
        assert!(message.contains("Parcours réussi"));
        assert_eq!(attempt_status(SimulationMode::Exploration, true), "succès");
        assert_eq!(
            attempt_status(SimulationMode::Exploration, false),
            "terminé"
        );
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
