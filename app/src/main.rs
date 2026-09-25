use dioxus::prelude::*;
use peoplemodeler_core::{integrate, levels, WIN_R, XMAX, XMIN, YMAX, YMIN};

fn main() {
    dioxus::launch(App);
}

const W: f64 = 640.0;
const H: f64 = 420.0;

fn map_x(x: f64) -> f64 {
    (x - XMIN) / (XMAX - XMIN) * W
}
fn map_y(y: f64) -> f64 {
    H - (y - YMIN) / (YMAX - YMIN) * H
}

fn field_grid_x() -> Vec<f64> {
    let mut v = vec![];
    let mut gx = XMIN + 0.5;
    while gx < XMAX {
        v.push(gx);
        gx += 0.75;
    }
    v
}
fn field_grid_y() -> Vec<f64> {
    let mut v = vec![];
    let mut gy = YMIN + 0.4;
    while gy < YMAX {
        v.push(gy);
        gy += 0.6;
    }
    v
}

fn path_to_svg(pts: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let (px, py) = (map_x(*x), map_y(*y));
        if i == 0 {
            d.push_str(&format!("M {px:.1} {py:.1} "));
        } else {
            d.push_str(&format!("L {px:.1} {py:.1} "));
        }
    }
    d
}

#[component]
fn App() -> Element {
    let all_levels = use_hook(levels);
    let mut level_idx = use_signal(|| 0usize);
    let mut k = use_signal(|| all_levels[0].k_def);
    let mut launched: Signal<Option<Vec<(f64, f64)>>> = use_signal(|| None);
    let mut message: Signal<Option<(String, bool)>> = use_signal(|| None);

    let current = all_levels[level_idx()].clone();
    let launch_level = current.clone();
    let level_defaults: Vec<f64> = all_levels.iter().map(|lv| lv.k_def).collect();
    let preview = integrate(&current, k());
    let shown_points = launched().unwrap_or(preview.points.clone());
    let path_d = path_to_svg(&shown_points);

    rsx! {
        style { {STYLE} }
        div { class: "wrap",
            h1 { "🌊 DriftLine" }
            p { class: "sub",
                "Une sonde est larguée dans une zone traversée par des courants invisibles. "
                "Une fois lâchée, elle suit le courant sans jamais dévier. Règle l'intensité "
                "avant de la larguer pour qu'elle atteigne la balise, sans percuter les astéroïdes."
            }
            div { class: "panel",
                div { class: "levels",
                    for (i, lv) in all_levels.iter().cloned().enumerate() {
                        button {
                            key: "{i}",
                            class: if i == level_idx() { "active" } else { "" },
                            onclick: move |_| {
                                level_idx.set(i);
                                k.set(lv.k_def);
                                launched.set(None);
                                message.set(None);
                            },
                            "Zone {i + 1}"
                        }
                    }
                }
                div { class: "desc", "{current.title} — {current.desc}" }

                svg { view_box: "0 0 {W} {H}", class: "field-svg",
                    for gx in field_grid_x() {
                        for gy in field_grid_y() {
                            {
                                let slope = (current.field)(gx, gy, k());
                                let len = 0.28 / (1.0 + slope * slope).sqrt();
                                let (x1, y1) = (map_x(gx - len), map_y(gy - slope * len));
                                let (x2, y2) = (map_x(gx + len), map_y(gy + slope * len));
                                rsx! {
                                    line {
                                        key: "{gx}-{gy}",
                                        x1: "{x1:.1}", y1: "{y1:.1}",
                                        x2: "{x2:.1}", y2: "{y2:.1}",
                                        class: "arrow",
                                    }
                                }
                            }
                        }
                    }
                    for (i, o) in current.obstacles.iter().enumerate() {
                        circle {
                            key: "{i}",
                            cx: "{map_x(o.x):.1}", cy: "{map_y(o.y):.1}",
                            r: "{o.r * (W / (XMAX - XMIN)):.1}",
                            class: "obstacle",
                        }
                    }
                    circle {
                        cx: "{map_x(current.a.0):.1}", cy: "{map_y(current.a.1):.1}",
                        r: "6", class: "point-a",
                    }
                    circle {
                        cx: "{map_x(current.b.0):.1}", cy: "{map_y(current.b.1):.1}",
                        r: "{WIN_R * (W / (XMAX - XMIN)):.1}", class: "point-b",
                    }
                    path { d: "{path_d}", class: "path" }
                }

                div { class: "controls",
                    label { "Intensité du courant" }
                    input {
                        r#type: "range",
                        min: "{current.k_min}",
                        max: "{current.k_max}",
                        step: "0.05",
                        value: "{k()}",
                        oninput: move |evt| {
                            if let Ok(v) = evt.value().parse::<f64>() {
                                k.set(v);
                                launched.set(None);
                                message.set(None);
                            }
                        },
                    }
                    span { class: "kval", "{k():.2}" }
                }

                div { class: "btnrow",
                    button {
                        class: "action",
                        onclick: move |_| {
                            let res = integrate(&launch_level, k());
                            let text = if res.reached {
                                ("🎯 La sonde a atteint la balise !".to_string(), true)
                            } else if res.collided {
                                ("💥 La sonde a percuté un astéroïde. Ajuste l'intensité.".to_string(), false)
                            } else {
                                ("❌ La sonde a raté la balise. Ajuste l'intensité.".to_string(), false)
                            };
                            launched.set(Some(res.points));
                            message.set(Some(text));
                        },
                        "▶ Larguer la sonde"
                    }
                    button {
                        class: "ghost",
                        onclick: move |_| {
                            launched.set(None);
                            message.set(None);
                        },
                        "↺ Réinitialiser"
                    }
                    if matches!(message(), Some((_, true))) {
                        button {
                            class: "action next",
                            onclick: move |_| {
                                let next = (level_idx() + 1).min(level_defaults.len() - 1);
                                level_idx.set(next);
                                k.set(level_defaults[next]);
                                launched.set(None);
                                message.set(None);
                            },
                            "Zone suivante →"
                        }
                    }
                }

                if let Some((text, win)) = message() {
                    div { class: if win { "msg win" } else { "msg fail" }, "{text}" }
                }

                div { class: "legend",
                    span { span { class: "dot dot-a" } " Largage" }
                    span { span { class: "dot dot-b" } " Balise" }
                    span { span { class: "dot dot-o" } " Astéroïde" }
                }
            }
        }
    }
}

const STYLE: &str = r#"
:root{--bg:#0f1420;--panel:#171f30;--ink:#e8ecf4;--sub:#93a1c0;--accent:#5ee3c9;--accent2:#ff8b6b;--danger:#ff5d7a;--line:#2a3550;--field:#3a4a72}
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
.arrow{stroke:var(--field);stroke-width:1}
.obstacle{fill:var(--accent2);opacity:.35}
.point-a{fill:#4ade80}
.point-b{fill:var(--danger)}
.path{fill:none;stroke:var(--accent);stroke-width:2.4}
.controls{display:flex;align-items:center;gap:10px;margin-top:12px;flex-wrap:wrap}
.controls label{font-size:.85rem;color:var(--sub);min-width:140px}
input[type=range]{flex:1;min-width:140px;accent-color:var(--accent)}
.kval{font-variant-numeric:tabular-nums;font-size:.9rem;min-width:52px;text-align:right}
.btnrow{display:flex;gap:8px;margin-top:12px;flex-wrap:wrap}
.action{background:var(--accent);color:#04231d;border:none;padding:9px 16px;border-radius:9px;font-weight:600;cursor:pointer}
.next{background:var(--accent2)!important}
.ghost{background:transparent;border:1px solid var(--line);color:var(--ink);padding:9px 16px;border-radius:9px;cursor:pointer}
.msg{margin-top:10px;font-size:.9rem;font-weight:600}
.msg.win{color:var(--accent)}
.msg.fail{color:var(--danger)}
.legend{display:flex;gap:14px;font-size:.78rem;color:var(--sub);margin-top:8px;flex-wrap:wrap}
.dot{display:inline-block;width:9px;height:9px;border-radius:50%;vertical-align:middle}
.dot-a{background:#4ade80}
.dot-b{background:var(--danger)}
.dot-o{background:var(--accent2)}
"#;
