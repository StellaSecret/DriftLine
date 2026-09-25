use dioxus::prelude::*;
use peoplemodeler_core::{
    integrate, levels, Level, Outcome, SimResult, Vector2, WIN_R, XMAX, XMIN, YMAX, YMIN,
};

const W: f64 = 640.0;
const H: f64 = 420.0;
const X_SCALE: f64 = W / (XMAX - XMIN);
const Y_SCALE: f64 = H / (YMAX - YMIN);
const STEPS: [f64; 6] = [1.0, 0.5, 0.1, 0.05, 0.01, 0.001];
const HISTORY_LIMIT: usize = 3;

#[derive(Clone, Debug)]
struct Attempt {
    k: f64,
    result: SimResult,
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
    let Vector2 { x: vx, y: vy } = (level.field)(x, y, k);
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

fn push_attempt(history: &mut Vec<Attempt>, attempt: Attempt) {
    if history.len() == HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(attempt);
}

fn archive_active_attempt(
    active_attempt: &mut Signal<Option<Attempt>>,
    history: &mut Signal<Vec<Attempt>>,
) {
    if let Some(attempt) = active_attempt() {
        let mut previous = history();
        push_attempt(&mut previous, attempt);
        history.set(previous);
    }
    active_attempt.set(None);
}

fn result_message(result: &SimResult) -> (String, bool) {
    match result.outcome {
        Outcome::Reached => ("La sonde a atteint la balise.".to_string(), true),
        Outcome::Collision { obstacle, .. } => (
            format!(
                "La sonde a percuté l'astéroïde {}. Le point rouge indique l'impact.",
                obstacle + 1
            ),
            false,
        ),
        Outcome::LeftField { .. } => (
            format!("La sonde a quitté la zone. {}", closest_message(result)),
            false,
        ),
        Outcome::TimeLimit => (
            format!(
                "La sonde a circulé trop longtemps. {}",
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
    let all_levels = use_hook(levels);
    let mut level_idx = use_signal(|| 0usize);
    let mut k = use_signal(|| all_levels[0].k_def);
    let mut step_idx = use_signal(|| step_index(all_levels[0].recommended_step));
    let mut history: Signal<Vec<Attempt>> = use_signal(Vec::new);
    let mut active_attempt: Signal<Option<Attempt>> = use_signal(|| None);
    let mut animation_id = use_signal(|| 0usize);
    let mut animation_visible = use_signal(|| false);

    use_effect(move || {
        let should_animate = animation_id() > 0 && active_attempt().is_some();
        if animation_visible() != should_animate {
            animation_visible.set(should_animate);
        }
    });

    let current = all_levels[level_idx()];
    let launch_level = current;
    let current_step = STEPS[step_idx()];
    let preview = integrate(&current, k());
    let active_result = active_attempt().map(|attempt| attempt.result);
    let shown_points = active_result
        .as_ref()
        .map(|result| result.points.clone())
        .unwrap_or_else(|| preview.points.clone());
    let path_d = path_to_svg(&shown_points);
    let history_snapshot = history();
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
    let collision_point = match active_result.as_ref().map(|result| result.outcome) {
        Some(Outcome::Collision { point, .. }) => Some(point),
        _ => None,
    };
    let active_message = active_result.as_ref().map(result_message);
    let active_won = active_result.as_ref().is_some_and(SimResult::reached);
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
    let has_next_level = level_idx() + 1 < all_levels.len();

    rsx! {
        style { {STYLE} }
        div { class: "wrap",
            h1 { "DriftLine" }
            p { class: "sub",
                "Une sonde est larguée dans une zone traversée par des courants invisibles. "
                "Une fois lâchée, elle suit le courant sans jamais dévier. Règle l'intensité "
                "avant de la larguer pour qu'elle atteigne la balise, sans percuter les astéroïdes."
            }
            div { class: "panel",
                div { class: "levels",
                    for (index, level) in all_levels.iter().copied().enumerate() {
                        button {
                            key: "{index}",
                            class: if index == level_idx() { "active" } else { "" },
                            onclick: move |_| {
                                level_idx.set(index);
                                k.set(level.k_def);
                                step_idx.set(step_index(level.recommended_step));
                                history.set(Vec::new());
                                active_attempt.set(None);
                            },
                            "Zone {index + 1}"
                        }
                    }
                }
                div { class: "desc", "{current.title} — {current.desc}" }

                svg { view_box: "0 0 {W} {H}", class: "field-svg",
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
                    for (index, obstacle) in current.obstacles.iter().enumerate() {
                        ellipse {
                            key: "{index}",
                            cx: "{map_x(obstacle.x):.2}", cy: "{map_y(obstacle.y):.2}",
                            rx: "{(obstacle.r * X_SCALE):.2}", ry: "{(obstacle.r * Y_SCALE):.2}",
                            class: "obstacle",
                        }
                    }
                    circle {
                        cx: "{map_x(current.a.0):.2}", cy: "{map_y(current.a.1):.2}",
                        r: "6", class: "point-a",
                    }
                    ellipse {
                        cx: "{map_x(current.b.0):.2}", cy: "{map_y(current.b.1):.2}",
                        rx: "{(WIN_R * X_SCALE):.2}", ry: "{(WIN_R * Y_SCALE):.2}", class: "point-b",
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
                                    archive_active_attempt(&mut active_attempt, &mut history);
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
                                        archive_active_attempt(&mut active_attempt, &mut history);
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
                                    archive_active_attempt(&mut active_attempt, &mut history);
                                }
                                k.set(next);
                            },
                            "+"
                        }
                    }
                    label { class: "step-label", "Pas" }
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
                                    archive_active_attempt(&mut active_attempt, &mut history);
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
                                    archive_active_attempt(&mut active_attempt, &mut history);
                                }
                                k.set(next);
                            }
                        },
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
                            let result = integrate(&launch_level, launch_k);
                            animation_visible.set(false);
                            animation_id.set(animation_id() + 1);
                            archive_active_attempt(&mut active_attempt, &mut history);
                            active_attempt.set(Some(Attempt {
                                k: launch_k,
                                result,
                            }));
                        },
                        "Larguer la sonde"
                    }
                    button {
                        class: "ghost",
                        onclick: move |_| {
                            history.set(Vec::new());
                            active_attempt.set(None);
                        },
                        "Réinitialiser"
                    }
                    if active_won && has_next_level {
                        button {
                            class: "action next",
                            onclick: move |_| {
                                let next = (level_idx() + 1).min(all_levels.len() - 1);
                                level_idx.set(next);
                                k.set(all_levels[next].k_def);
                                step_idx.set(step_index(all_levels[next].recommended_step));
                                history.set(Vec::new());
                                active_attempt.set(None);
                            },
                            "Zone suivante"
                        }
                    }
                }

                if let Some((text, won)) = active_message {
                    div { class: if won { "msg win" } else { "msg fail" }, "{text}" }
                }

                if !history_paths.is_empty() {
                    div { class: "history",
                        span { class: "history-title", "Essais précédents" }
                        for (index, (_, label, won)) in history_paths.iter().enumerate() {
                            span {
                                key: "history-label-{index}",
                                class: if *won { "history-entry success" } else { "history-entry" },
                                "intensité {label} · {attempt_status(*won)}"
                            }
                        }
                    }
                }

                div { class: "legend",
                    span { span { class: "dot dot-a" } " Largage" }
                    span { span { class: "dot dot-b" } " Balise" }
                    span { span { class: "dot dot-o" } " Astéroïde" }
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
.levels{display:flex;gap:6px;flex-wrap:wrap;margin-bottom:10px}
.levels button{background:transparent;border:1px solid var(--line);color:var(--sub);padding:6px 12px;border-radius:20px;font-size:.85rem;cursor:pointer}
.levels button.active{background:var(--accent);color:#04231d;border-color:var(--accent);font-weight:600}
.desc{font-size:.85rem;color:var(--sub);margin-bottom:10px;line-height:1.5}
.field-svg{width:100%;height:auto;background:var(--bg);border:1px solid var(--line);border-radius:10px}
.arrow{stroke:var(--field);stroke-width:1.2}
.arrow-head{fill:var(--field)}
.obstacle{fill:var(--accent2);opacity:.35}
.point-a{fill:#4ade80}
.point-b{fill:var(--danger)}
.closest-point{fill:var(--closest);stroke:var(--bg);stroke-width:2}
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

    fn attempt(k: f64) -> Attempt {
        Attempt {
            k,
            result: SimResult {
                points: vec![(0.0, 0.0)],
                outcome: Outcome::TimeLimit,
                closest: None,
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
}
