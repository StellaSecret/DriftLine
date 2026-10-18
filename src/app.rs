//! Screens, input handling and the front end of each gameplay mode. All rules live in `vfield::modes`.
use crate::render as r;
use crate::ui::{self, alpha, rgb, text, text_c, wrap, Input, H, W};
use macroquad::prelude::*;
use std::collections::HashSet;
use vfield::modes::*;
use vfield::sim::{Outcome, Trajectory};
use vfield::{levels, Chapter, Level, Reveal, Rules, V2};

const PX0: f32 = 1014.0; // panel content x
const PW: f32 = 236.0; // panel content width

fn gray() -> Color {
    rgb(170, 176, 192)
}
fn good_c() -> Color {
    rgb(0, 190, 140)
}
fn bad_c() -> Color {
    rgb(240, 120, 60)
}

// ------------------------------------------------------------------ state
pub struct Prefs {
    grid: usize,
    hc: bool,
    unlock_all: bool,
}

enum Screen {
    Menu,
    Select(Chapter),
    Play(Box<Session>),
}

#[derive(PartialEq, Clone, Copy)]
enum Action {
    None,
    Menu,
    Next,
}

struct Anim {
    items: Vec<(Trajectory, Color)>,
    time: f64,
    speed: f64,
}
impl Anim {
    fn new(items: Vec<(Trajectory, Color)>) -> Anim {
        let d = items.iter().map(|i| i.0.duration()).fold(0.0, f64::max);
        Anim { items, time: 0.0, speed: (d / 2.2).clamp(2.0, 12.0) }
    }
    fn dur(&self) -> f64 {
        self.items.iter().map(|i| i.0.duration()).fold(0.0, f64::max)
    }
    fn done(&self) -> bool {
        self.time >= self.dur() + 0.3
    }
}

#[derive(Default)]
struct St {
    anim: Option<Anim>,
    msg: String,
    good: Option<bool>,
    hints: usize,
    banner: bool,
    was_complete: bool,
    active_slot: usize,
    exp_i: f64,
    exp_choice: Option<usize>,
    exp_est: [f64; 2],
}
impl St {
    fn flying(&self) -> bool {
        self.anim.as_ref().is_some_and(|a| !a.done())
    }
    fn msg_visible(&self) -> bool {
        !self.flying()
    }
}

enum Game {
    Position(PositionGame),
    Predict(PredictGame),
    Tempo(TempoGame),
    Experiment(ExperimentGame),
}
impl Game {
    fn level(&self) -> &Level {
        match self {
            Game::Position(g) => &g.level,
            Game::Predict(g) => &g.level,
            Game::Tempo(g) => &g.level,
            Game::Experiment(g) => &g.level,
        }
    }
    fn complete(&self) -> bool {
        match self {
            Game::Position(g) => g.won,
            Game::Predict(g) => g.passed(),
            Game::Tempo(g) => g.won,
            Game::Experiment(g) => g.solved,
        }
    }
}

struct Session {
    idx: usize,
    game: Game,
    st: St,
}

fn make_session(idx: usize, level: Level) -> Session {
    let mut st = St::default();
    let game = match &level.rules {
        Rules::Position { .. } => Game::Position(PositionGame::new(level)),
        Rules::Predict { .. } => Game::Predict(PredictGame::new(level)),
        Rules::Tempo { .. } => Game::Tempo(TempoGame::new(level)),
        Rules::Experiment { range, .. } => {
            st.exp_i = range.0;
            st.exp_est = [range.0 + (range.1 - range.0) * 0.33, range.0 + (range.1 - range.0) * 0.66];
            Game::Experiment(ExperimentGame::new(level))
        }
    };
    Session { idx, game, st }
}

// ------------------------------------------------------------------ app
pub struct App {
    screen: Screen,
    levels: Vec<Level>,
    progress: HashSet<String>,
    prefs: Prefs,
}

#[cfg(not(target_arch = "wasm32"))]
const PROGRESS_FILE: &str = "vfield_progress.txt";

fn load_progress() -> HashSet<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(s) = std::fs::read_to_string(PROGRESS_FILE) {
            return s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        }
    }
    HashSet::new()
}
fn save_progress(p: &HashSet<String>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut v: Vec<_> = p.iter().cloned().collect();
        v.sort();
        let _ = std::fs::write(PROGRESS_FILE, v.join("\n"));
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = p;
    }
}

impl App {
    pub fn new() -> App {
        App { screen: Screen::Menu, levels: levels::all(), progress: load_progress(), prefs: Prefs { grid: 2, hc: false, unlock_all: false } }
    }

    fn chapter_levels(&self, c: Chapter) -> Vec<usize> {
        self.levels.iter().enumerate().filter(|(_, l)| l.chapter == c).map(|(i, _)| i).collect()
    }
    fn done(&self, i: usize) -> bool {
        self.progress.contains(self.levels[i].id)
    }
    fn chapter_unlocked(&self, c: Chapter) -> bool {
        if self.prefs.unlock_all || c != Chapter::Combo {
            return true;
        }
        ["pos-1", "pre-1", "tem-1", "exp-1"].iter().all(|id| self.progress.contains(*id))
    }
    fn level_unlocked(&self, c: Chapter, k: usize) -> bool {
        let ids = self.chapter_levels(c);
        self.prefs.unlock_all || (self.chapter_unlocked(c) && (k == 0 || self.done(ids[k - 1])))
    }

    pub fn frame(&mut self) {
        clear_background(rgb(14, 16, 22));
        let inp = ui::begin_frame();
        draw_rectangle(0.0, 0.0, W, H, rgb(14, 16, 22));
        if is_key_pressed(KeyCode::U) {
            self.prefs.unlock_all = !self.prefs.unlock_all;
        }
        let screen = std::mem::replace(&mut self.screen, Screen::Menu);
        self.screen = match screen {
            Screen::Menu => self.menu(&inp),
            Screen::Select(c) => self.select(c, &inp),
            Screen::Play(mut s) => {
                let act = play_frame(&mut s, &inp, &mut self.prefs);
                let c = s.game.level().chapter;
                if s.game.complete() && !s.st.was_complete {
                    s.st.was_complete = true;
                    s.st.banner = true;
                    if self.progress.insert(s.game.level().id.to_string()) {
                        save_progress(&self.progress);
                    }
                }
                match act {
                    Action::None => Screen::Play(s),
                    Action::Menu => Screen::Select(c),
                    Action::Next => {
                        let ids = self.chapter_levels(c);
                        match ids.iter().position(|i| *i == s.idx) {
                            Some(p) if p + 1 < ids.len() => Screen::Play(Box::new(make_session(ids[p + 1], self.levels[ids[p + 1]].clone()))),
                            _ => Screen::Select(c),
                        }
                    }
                }
            }
        };
    }

    fn menu(&mut self, inp: &Input) -> Screen {
        text_c("VECTOR FIELD", W / 2.0, 100.0, 64.0, WHITE);
        text_c("Read the flow. Steer the probe.", W / 2.0, 150.0, 26.0, gray());
        let mut next = Screen::Menu;
        for (k, c) in Chapter::ALL.iter().enumerate() {
            let r = Rect::new(48.0 + k as f32 * 240.0, 200.0, 224.0, 300.0);
            let unlocked = self.chapter_unlocked(*c);
            let ids = self.chapter_levels(*c);
            let n = ids.iter().filter(|i| self.done(**i)).count();
            let hot = r.contains(inp.mouse) && unlocked;
            draw_rectangle(r.x, r.y, r.w, r.h, if hot { rgb(54, 62, 82) } else { rgb(36, 41, 56) });
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0, if unlocked { chapter_color(*c) } else { rgb(70, 74, 86) });
            text_c(c.name(), r.x + r.w / 2.0, r.y + 40.0, 30.0, if unlocked { WHITE } else { gray() });
            wrap(c.question(), r.x + 16.0, r.y + 90.0, r.w - 32.0, 22.0, gray());
            text_c(&format!("{}/{} complete", n, ids.len()), r.x + r.w / 2.0, r.y + 250.0, 20.0, WHITE);
            if !unlocked {
                text_c("Locked", r.x + r.w / 2.0, r.y + 170.0, 22.0, bad_c());
                wrap("Finish level 1 of the four mechanics.", r.x + 16.0, r.y + 200.0, r.w - 32.0, 16.0, gray());
            }
            if hot && inp.pressed {
                next = Screen::Select(*c);
            }
        }
        let lines = [
            "Mouse: click and drag.   Space / Enter: launch or confirm.   H: hint.   R: retry.   V: replay.   Esc: back.",
            "G: arrow grid density.   K: high-contrast arrows.   U: unlock every level.",
        ];
        for (i, l) in lines.iter().enumerate() {
            text_c(l, W / 2.0, 570.0 + i as f32 * 28.0, 20.0, gray());
        }
        text_c(if self.prefs.unlock_all { "All levels unlocked" } else { "" }, W / 2.0, 640.0, 20.0, good_c());
        next
    }

    fn select(&mut self, c: Chapter, inp: &Input) -> Screen {
        if is_key_pressed(KeyCode::Escape) {
            return Screen::Menu;
        }
        text(c.name(), 60.0, 90.0, 52.0, WHITE);
        text(c.question(), 62.0, 126.0, 26.0, gray());
        let ids = self.chapter_levels(c);
        let mut next = Screen::Select(c);
        for (k, i) in ids.iter().enumerate() {
            let r = Rect::new(60.0, 160.0 + k as f32 * 86.0, 900.0, 74.0);
            let open = self.level_unlocked(c, k);
            let hot = open && r.contains(inp.mouse);
            draw_rectangle(r.x, r.y, r.w, r.h, if hot { rgb(54, 62, 82) } else { rgb(36, 41, 56) });
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if open { chapter_color(c) } else { rgb(70, 74, 86) });
            text(&format!("{}", k + 1), r.x + 22.0, r.y + 48.0, 40.0, if open { WHITE } else { gray() });
            text(self.levels[*i].title, r.x + 80.0, r.y + 32.0, 28.0, if open { WHITE } else { gray() });
            text(self.levels[*i].objective, r.x + 80.0, r.y + 58.0, 19.0, gray());
            if self.done(*i) {
                let (cx, cy) = (r.x + r.w - 40.0, r.y + 37.0);
                draw_circle_lines(cx, cy, 16.0, 3.0, good_c());
                draw_line(cx - 7.0, cy, cx - 2.0, cy + 6.0, 3.0, good_c());
                draw_line(cx - 2.0, cy + 6.0, cx + 8.0, cy - 6.0, 3.0, good_c());
            } else if !open {
                let (cx, cy) = (r.x + r.w - 40.0, r.y + 37.0);
                draw_rectangle(cx - 10.0, cy - 2.0, 20.0, 14.0, gray());
                draw_circle_lines(cx, cy - 4.0, 7.0, 3.0, gray());
            }
            if hot && inp.pressed {
                next = Screen::Play(Box::new(make_session(*i, self.levels[*i].clone())));
            }
        }
        text("Esc: back", 60.0, 780.0, 20.0, gray());
        next
    }
}

fn chapter_color(c: Chapter) -> Color {
    match c {
        Chapter::Position => rgb(204, 121, 167),
        Chapter::Predict => rgb(86, 180, 233),
        Chapter::Tempo => rgb(240, 228, 66),
        Chapter::Experiment => rgb(0, 158, 115),
        Chapter::Combo => rgb(230, 159, 0),
    }
}

// ------------------------------------------------------------------ shared play UI
fn short(l: &Level, o: Outcome) -> String {
    match o {
        Outcome::Reached(i) => match l.regions[i].kind {
            vfield::RegionKind::Exit => format!("exit {}", l.regions[i].label),
            _ => "target".into(),
        },
        Outcome::Hazard(_) => "hazard".into(),
        Outcome::Boundary => "between exits".into(),
        Outcome::Timeout => "timed out".into(),
        Outcome::Stalled => "came to rest".into(),
        Outcome::Invalid => "error".into(),
    }
}

fn panel_header(l: &Level) -> f32 {
    draw_rectangle(1000.0, 80.0, 260.0, 700.0, rgb(26, 30, 42));
    draw_rectangle_lines(1000.0, 80.0, 260.0, 700.0, 2.0, rgb(60, 66, 84));
    let n = l.id.rsplit('-').next().unwrap_or("");
    text(&format!("{} {}", l.chapter.name().to_uppercase(), n), PX0, 108.0, 22.0, chapter_color(l.chapter));
    text(l.chapter.question(), PX0, 130.0, 18.0, gray());
    150.0
}

fn show_msg(st: &St) {
    if st.msg.is_empty() || !st.msg_visible() {
        return;
    }
    let col = match st.good {
        Some(true) => good_c(),
        Some(false) => bad_c(),
        None => WHITE,
    };
    wrap(&st.msg, PX0, 616.0, PW, 18.0, col);
}

fn do_retry(g: &mut Game, st: &mut St) {
    match g {
        Game::Predict(p) => {
            if p.finished() {
                p.restart();
            }
        }
        Game::Experiment(e) => e.reset(),
        _ => {}
    }
    st.anim = None;
    st.msg.clear();
    st.good = None;
    st.banner = false;
}

fn retry_enabled(g: &Game, st: &St) -> bool {
    match g {
        Game::Predict(p) => p.finished(),
        Game::Experiment(e) => !e.runs.is_empty(),
        _ => st.anim.is_some() || !st.msg.is_empty(),
    }
}

fn play_frame(s: &mut Session, inp: &Input, prefs: &mut Prefs) -> Action {
    let dt = get_frame_time() as f64;
    if is_key_pressed(KeyCode::G) {
        prefs.grid = (prefs.grid + 1) % 4;
    }
    if is_key_pressed(KeyCode::K) {
        prefs.hc = !prefs.hc;
    }
    if is_key_pressed(KeyCode::Escape) {
        return Action::Menu;
    }
    if let Some(a) = &mut s.st.anim {
        a.time += dt * a.speed;
    }
    match &mut s.game {
        Game::Position(g) => pos_frame(g, &mut s.st, inp, prefs),
        Game::Predict(g) => predict_frame(g, &mut s.st, inp, prefs),
        Game::Tempo(g) => tempo_frame(g, &mut s.st, inp, prefs, dt),
        Game::Experiment(g) => exp_frame(g, &mut s.st, inp, prefs, dt),
    }
    let lvl = s.game.level().clone();
    // title bar
    text(lvl.title, 20.0, 40.0, 34.0, WHITE);
    text(lvl.objective, 20.0, 68.0, 21.0, gray());
    show_msg(&s.st);
    // hints
    let nh = lvl.hints.len();
    if s.st.hints > 0 {
        let k = s.st.hints.min(nh);
        let h = 16.0 + k as f32 * 44.0;
        draw_rectangle(r::AX + 8.0, r::AY + r::AH - 8.0 - h, 540.0, h, Color::new(0.05, 0.06, 0.09, 0.88));
        draw_rectangle_lines(r::AX + 8.0, r::AY + r::AH - 8.0 - h, 540.0, h, 2.0, r::c_gate());
        for i in 0..k {
            wrap(&format!("Hint {}: {}", i + 1, lvl.hints[i]), r::AX + 18.0, r::AY + r::AH - 8.0 - h + 28.0 + i as f32 * 44.0, 520.0, 18.0, WHITE);
        }
    }
    // footer buttons
    let acc = rgb(120, 130, 160);
    if ui::button(inp, Rect::new(PX0, 700.0, 116.0, 34.0), "Hint (H)", s.st.hints < nh, acc) || (is_key_pressed(KeyCode::H) && s.st.hints < nh) {
        s.st.hints += 1;
    }
    let retry_label = if matches!(s.game, Game::Experiment(_)) { "Reset (R)" } else { "Retry (R)" };
    let can_retry = retry_enabled(&s.game, &s.st);
    if ui::button(inp, Rect::new(PX0 + 120.0, 700.0, 116.0, 34.0), retry_label, can_retry, acc) || (is_key_pressed(KeyCode::R) && can_retry) {
        do_retry(&mut s.game, &mut s.st);
    }
    let can_replay = s.st.anim.is_some();
    if ui::button(inp, Rect::new(PX0, 742.0, 116.0, 34.0), "Replay (V)", can_replay, acc) || (is_key_pressed(KeyCode::V) && can_replay) {
        if let Some(a) = &mut s.st.anim {
            a.time = 0.0;
        }
    }
    let mut action = Action::None;
    if ui::button(inp, Rect::new(PX0 + 120.0, 742.0, 116.0, 34.0), "Menu (Esc)", true, acc) {
        action = Action::Menu;
    }
    // completion banner
    let complete = s.game.complete();
    if complete && s.st.msg_visible() && s.st.banner {
        let b = Rect::new(r::AX + r::AW / 2.0 - 240.0, r::AY + 12.0, 480.0, 176.0);
        draw_rectangle(b.x, b.y, b.w, b.h, Color::new(0.06, 0.08, 0.12, 0.96));
        draw_rectangle_lines(b.x, b.y, b.w, b.h, 4.0, good_c());
        text_c("Level complete", b.x + b.w / 2.0, b.y + 36.0, 40.0, good_c());
        let detail = match &s.game {
            Game::Position(g) => format!("Launches used: {}", g.attempts.len()),
            Game::Predict(g) => format!("Forecasts correct: {} of {}", g.correct_count(), g.rounds.len()),
            Game::Tempo(g) => format!("Launches used: {}", g.attempts.len()),
            Game::Experiment(g) => format!("Experiments run: {}", g.runs.len()),
        };
        text_c(&detail, b.x + b.w / 2.0, b.y + 74.0, 22.0, WHITE);
        if ui::button(inp, Rect::new(b.x + 20.0, b.y + 106.0, 200.0, 50.0), "Next (N)", true, good_c()) {
            action = Action::Next;
        }
        if ui::button(inp, Rect::new(b.x + 260.0, b.y + 106.0, 200.0, 50.0), "Keep playing", true, rgb(120, 130, 160)) {
            s.st.banner = false;
        }
    }
    if complete && is_key_pressed(KeyCode::N) {
        action = Action::Next;
    }
    action
}

fn space() -> bool {
    is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter)
}

fn draw_anim(st: &St) {
    if let Some(a) = &st.anim {
        for (t, c) in &a.items {
            let pts = t.upto(a.time);
            r::draw_path(&pts, *c, 3.5);
            if a.time < t.duration() {
                r::draw_probe(t.pos_at(a.time), *c);
            } else {
                r::draw_marker_end(t.end(), *c, "");
            }
        }
    }
}

// ------------------------------------------------------------------ POSITION
fn pos_frame(g: &mut PositionGame, st: &mut St, inp: &Input, prefs: &Prefs) {
    let lvl = g.level.clone();
    let slots = g.slots();
    let multi = slots.len() > 1;
    let flying = st.flying();
    let ft = match (&st.anim, flying) {
        (Some(a), true) => g.phase + a.time.min(a.dur()),
        _ => g.phase,
    };
    r::begin_area();
    r::draw_field(&lvl.field.with_intensity(g.intensity), ft, prefs.grid, prefs.hc);
    for s in &slots {
        r::draw_zone(&s.zone);
    }
    for reg in &lvl.regions {
        r::draw_region(reg, false);
    }
    let keep = if st.anim.is_some() { g.attempts.len().saturating_sub(1) } else { g.attempts.len() };
    for a in g.attempts[..keep].iter().rev().take(3) {
        for t in &a.trajs {
            r::draw_path(&t.points, alpha(r::c_probe(), 0.22), 2.0);
        }
    }
    draw_anim(st);
    for (i, p) in g.placed.iter().enumerate() {
        if let Some(p) = p {
            r::draw_start(*p);
            if st.anim.is_none() {
                r::draw_probe(*p, if multi { r::run_color(i) } else { r::c_probe() });
            }
            if multi {
                let sp = r::w2s(*p);
                text(&format!("{}", i + 1), sp.x - 24.0, sp.y + 6.0, 22.0, WHITE);
            }
        }
    }
    r::end_area();

    // input
    if !flying && inp.pressed && r::in_area(inp.mouse) {
        if let Some(i) = g.place_at(r::s2w(inp.mouse)) {
            st.active_slot = i;
            st.anim = None;
            st.msg.clear();
            st.good = None;
        }
    }
    if multi && is_key_pressed(KeyCode::Tab) {
        st.active_slot = (st.active_slot + 1) % slots.len();
    }
    if !flying {
        let sp = if is_key_down(KeyCode::LeftShift) { 0.25 } else { 1.2 } * get_frame_time() as f64;
        let mut d = V2::ZERO;
        if is_key_down(KeyCode::Left) {
            d.x -= sp;
        }
        if is_key_down(KeyCode::Right) {
            d.x += sp;
        }
        if is_key_down(KeyCode::Up) {
            d.y += sp;
        }
        if is_key_down(KeyCode::Down) {
            d.y -= sp;
        }
        if d != V2::ZERO {
            g.nudge(st.active_slot.min(slots.len() - 1), d);
            st.anim = None;
            st.msg.clear();
            st.good = None;
        }
    }

    // panel
    let mut y = panel_header(&lvl);
    y = wrap("Click inside a shaded zone to place the probe, or use the arrow keys (Shift for fine steps).", PX0, y + 8.0, PW, 17.0, gray());
    if multi {
        y = wrap(&format!("Probe {} selected (Tab switches). Every probe must reach its own target.", st.active_slot + 1), PX0, y + 4.0, PW, 17.0, WHITE);
    }
    if let Some((lo, hi)) = g.intensity_range() {
        y += 8.0;
        text(&format!("Shared intensity: {:.2}", g.intensity), PX0, y + 14.0, 19.0, WHITE);
        let mut v = g.intensity;
        if ui::slider(inp, Rect::new(PX0, y + 24.0, PW, 16.0), &mut v, lo, hi, r::c_zone()) {
            g.set_intensity(v);
            st.anim = None;
        }
        if is_key_pressed(KeyCode::Minus) {
            g.set_intensity(g.intensity - 0.1);
        }
        if is_key_pressed(KeyCode::Equal) {
            g.set_intensity(g.intensity + 0.1);
        }
        y += 52.0;
    }
    if let Some(per) = g.period() {
        y = wrap(&format!("Launch phase {:.1} s. Click the timeline below, or press , and .", g.phase), PX0, y + 8.0, PW, 17.0, WHITE);
        if !flying {
            if is_key_pressed(KeyCode::Comma) {
                g.set_phase(g.phase - 0.25);
            }
            if is_key_pressed(KeyCode::Period) {
                g.set_phase(g.phase + 0.25);
            }
        }
        let marks: Vec<(f64, bool)> = g.attempts.iter().map(|a| (a.phase, a.success)).collect();
        let cur = if flying { ft } else { g.phase };
        if let Some(p) = ui::timeline(inp, Rect::new(r::AX, 752.0, r::AW, 28.0), per, cur, &marks, "Launch phase (triangle = reached, cross = missed)") {
            if !flying {
                g.set_phase(p);
                st.anim = None;
            }
        }
    }
    y += 10.0;
    let go = ui::button(inp, Rect::new(PX0, y, PW, 44.0), "Launch (Space)", g.ready() && !flying, chapter_color(Chapter::Position));
    text(&format!("Launches: {}", g.attempts.len()), PX0, y + 66.0, 18.0, gray());
    if (go || space()) && g.ready() && !flying {
        if let Some(a) = g.launch().cloned() {
            let items = a.trajs.iter().enumerate().map(|(i, t)| (t.clone(), if multi { r::run_color(i) } else { r::c_probe() })).collect();
            st.anim = Some(Anim::new(items));
            st.good = Some(a.success);
            st.msg = if a.success {
                "Success! Every probe reached its target.".into()
            } else {
                let mut parts = vec![];
                for (i, t) in a.trajs.iter().enumerate() {
                    let goal = slots[i].goal;
                    let txt = if a.slot_ok[i] {
                        "reached its target".to_string()
                    } else if t.outcome == Outcome::Reached(goal) {
                        "reached the target but skipped the gate".to_string()
                    } else {
                        short(&lvl, t.outcome)
                    };
                    parts.push(if multi { format!("Probe {}: {}.", i + 1, txt) } else { format!("Result: {}.", txt) });
                }
                parts.join(" ")
            };
        }
    }
}

// ------------------------------------------------------------------ PREDICT
fn predict_frame(g: &mut PredictGame, st: &mut St, inp: &Input, prefs: &Prefs) {
    let lvl = g.level.clone();
    let round = g.round().clone();
    let exits = lvl.exits();
    r::begin_area();
    r::draw_field(&lvl.field, 0.0, prefs.grid, prefs.hc);
    for (i, reg) in lvl.regions.iter().enumerate() {
        r::draw_region(reg, g.selected == Some(i));
    }
    if !g.revealed {
        if let Reveal::Partial(secs) = g.reveal_mode() {
            let pts = round.traj.upto(secs);
            r::draw_path(&pts, alpha(r::c_probe(), 0.6), 3.0);
            r::draw_marker_end(*pts.last().unwrap(), r::c_probe(), "clue ends here");
        }
    }
    if let Some(sel) = g.selected {
        let c = r::w2s(lvl.regions[sel].center);
        let from = r::w2s(round.start);
        let edge = c + (from - c).normalize_or_zero() * (lvl.regions[sel].radius as f32 * r::PX);
        r::dashed_line(from, edge, r::c_predict(), 2.5);
        text("your forecast", (c.x - 40.0).clamp(r::AX + 4.0, r::AX + r::AW - 110.0), (c.y + lvl.regions[sel].radius as f32 * r::PX + 18.0).min(r::AY + r::AH - 6.0), 18.0, r::c_predict());
    }
    draw_anim(st);
    r::draw_start(round.start);
    if st.anim.is_none() {
        r::draw_probe(round.start, r::c_probe());
    }
    r::end_area();

    // input: select
    if !g.revealed {
        for e in &exits {
            let lab = lvl.regions[*e].label;
            let key = match lab {
                'A' => KeyCode::A,
                'B' => KeyCode::B,
                'C' => KeyCode::C,
                'D' => KeyCode::D,
                _ => KeyCode::Unknown,
            };
            if (inp.pressed && r::hit_region(inp, &lvl.regions[*e])) || is_key_pressed(key) {
                g.select(*e);
            }
        }
    }
    // panel
    let mut y = panel_header(&lvl);
    y = wrap(&format!("Round {} of {}. Where will the probe leave? Commit a forecast before the path is shown.", g.idx + 1, g.rounds.len()), PX0, y + 8.0, PW, 17.0, WHITE);
    y += 6.0;
    for i in 0..g.rounds.len() {
        let (x, yy) = (PX0 + i as f32 * 36.0, y);
        draw_rectangle_lines(x, yy, 28.0, 28.0, 2.0, gray());
        if let Some(rr) = g.results.get(i) {
            if rr.correct {
                draw_rectangle(x, yy, 28.0, 28.0, alpha(good_c(), 0.3));
                draw_line(x + 6.0, yy + 15.0, x + 12.0, yy + 21.0, 3.0, good_c());
                draw_line(x + 12.0, yy + 21.0, x + 23.0, yy + 7.0, 3.0, good_c());
            } else {
                draw_rectangle(x, yy, 28.0, 28.0, alpha(bad_c(), 0.3));
                draw_line(x + 7.0, yy + 7.0, x + 21.0, yy + 21.0, 3.0, bad_c());
                draw_line(x + 21.0, yy + 7.0, x + 7.0, yy + 21.0, 3.0, bad_c());
            }
        }
    }
    y += 44.0;
    text(&format!("Need {} correct.", g.pass_mark()), PX0, y, 18.0, gray());
    y += 20.0;
    text("Your forecast:", PX0, y + 14.0, 18.0, WHITE);
    y += 22.0;
    for (k, e) in exits.iter().enumerate() {
        let sel = g.selected == Some(*e);
        let rect = Rect::new(PX0 + k as f32 * 60.0, y, 54.0, 40.0);
        if ui::button(inp, rect, &lvl.regions[*e].label.to_string(), !g.revealed, if sel { r::c_predict() } else { rgb(120, 130, 160) }) {
            g.select(*e);
        }
        if sel {
            draw_rectangle_lines(rect.x - 3.0, rect.y - 3.0, rect.w + 6.0, rect.h + 6.0, 3.0, r::c_predict());
        }
    }
    y += 54.0;
    let can_commit = g.selected.is_some() && !g.revealed;
    let commit = ui::button(inp, Rect::new(PX0, y, PW, 44.0), "Commit forecast (Space)", can_commit, chapter_color(Chapter::Predict));
    y += 54.0;
    if can_commit && (commit || space()) {
        if let Some(res) = g.commit() {
            st.anim = Some(Anim::new(vec![(round.traj.clone(), r::c_probe())]));
            let actual = lvl.regions[round.answer].label;
            let chosen = lvl.regions[res.chosen].label;
            st.good = Some(res.correct);
            let mut m = if res.correct { format!("Correct! It left through exit {}.", actual) } else { format!("Not quite. You said {}; it left through exit {}.", chosen, actual) };
            if g.finished() {
                m += &format!(" Score {} of {}.", g.correct_count(), g.rounds.len());
                if !g.passed() {
                    m += " Retry for fresh questions.";
                }
            }
            st.msg = m;
        }
    }
    if g.revealed && !st.flying() && !g.finished() {
        let next = ui::button(inp, Rect::new(PX0, y, PW, 44.0), "Next round (Space)", true, chapter_color(Chapter::Predict));
        if next || space() {
            g.next_round();
            st.anim = None;
            st.msg.clear();
            st.good = None;
        }
    } else if g.finished() && !g.passed() && !st.flying() && space() {
        st.anim = None;
        st.msg.clear();
        st.good = None;
        st.banner = false;
        g.restart();
    }
}

// ------------------------------------------------------------------ TEMPO
fn tempo_frame(g: &mut TempoGame, st: &mut St, inp: &Input, prefs: &Prefs, dt: f64) {
    let lvl = g.level.clone();
    let (start, period) = match &lvl.rules {
        Rules::Tempo { start, period, .. } => (*start, *period),
        _ => unreachable!(),
    };
    let flying = st.flying();
    if !flying {
        g.tick(dt);
    }
    let ft = match (&st.anim, flying) {
        (Some(a), true) => a.items[0].0.t0 + a.time.min(a.dur()),
        _ => g.clock,
    };
    r::begin_area();
    r::draw_field(&lvl.field, ft, prefs.grid, prefs.hc);
    for reg in &lvl.regions {
        r::draw_region(reg, false);
    }
    draw_anim(st);
    r::draw_start(start);
    if st.anim.is_none() {
        r::draw_probe(start, r::c_probe());
    }
    r::end_area();

    let marks: Vec<(f64, bool)> = g.attempts.iter().map(|a| (a.phase, a.success)).collect();
    if let Some(p) = ui::timeline(inp, Rect::new(r::AX, 752.0, r::AW, 28.0), period, ft, &marks, &format!("Field phase {:.1} s of {:.0} s  (triangle = reached, cross = missed)", ft.rem_euclid(period), period)) {
        if !flying {
            g.set_phase(p);
            st.anim = None;
        }
    }
    if !flying {
        if is_key_pressed(KeyCode::Left) {
            g.set_phase(g.clock - 0.25);
        }
        if is_key_pressed(KeyCode::Right) {
            g.set_phase(g.clock + 0.25);
        }
    }
    let mut y = panel_header(&lvl);
    y = wrap("The field changes with time. Watch it, scrub the timeline, then launch at the moment you choose.", PX0, y + 8.0, PW, 17.0, gray());
    y += 6.0;
    text(&format!("Field clock: {:.1} s", g.clock), PX0, y + 14.0, 20.0, WHITE);
    y += 28.0;
    if ui::button(inp, Rect::new(PX0, y, PW, 36.0), if g.playing { "Pause field (P)" } else { "Play field (P)" }, !flying, rgb(120, 130, 160)) || (is_key_pressed(KeyCode::P) && !flying) {
        g.playing = !g.playing;
    }
    y += 46.0;
    text("Left/Right: step the clock", PX0, y + 10.0, 16.0, gray());
    y += 24.0;
    let go = ui::button(inp, Rect::new(PX0, y, PW, 44.0), "Launch now (Space)", !flying, chapter_color(Chapter::Tempo));
    y += 56.0;
    if (go || space()) && !flying {
        let a = g.launch().clone();
        st.anim = Some(Anim::new(vec![(a.traj.clone(), r::c_probe())]));
        st.good = Some(a.success);
        st.msg = if a.success {
            "Success! The timing worked.".to_string()
        } else if a.traj.outcome == Outcome::Reached(0) {
            "The probe reached the target but skipped the gate.".to_string()
        } else {
            format!("Missed: {}. Scrub the timeline and compare how the field looks.", short(&lvl, a.traj.outcome))
        };
    }
    text(&format!("Launches: {}", g.attempts.len()), PX0, y + 8.0, 18.0, gray());
    for (i, a) in g.attempts.iter().rev().take(4).enumerate() {
        let yy = y + 30.0 + i as f32 * 20.0;
        let c = if a.success { good_c() } else { bad_c() };
        text(&format!("{:>5.1} s  {}", a.traj.t0, if a.success { "reached" } else { "missed" }), PX0, yy, 17.0, c);
    }
}

// ------------------------------------------------------------------ EXPERIMENT
fn exp_frame(g: &mut ExperimentGame, st: &mut St, inp: &Input, prefs: &Prefs, dt: f64) {
    let lvl = g.level.clone();
    let (lo, hi) = g.range();
    let start = match &lvl.rules {
        Rules::Experiment { start, .. } => *start,
        _ => unreachable!(),
    };
    r::begin_area();
    r::draw_field(&lvl.field.with_intensity(st.exp_i), 0.0, prefs.grid, prefs.hc);
    for reg in &lvl.regions {
        r::draw_region(reg, false);
    }
    let last = g.runs.len().saturating_sub(1);
    for (i, run) in g.runs.iter().enumerate() {
        if !run.visible {
            continue;
        }
        let col = r::run_color(i);
        let animating = i == last && st.flying();
        if animating {
            let a = st.anim.as_ref().unwrap();
            r::draw_path(&run.traj.upto(a.time), col, 3.5);
            r::draw_probe(run.traj.pos_at(a.time), col);
        } else {
            r::draw_path(&run.traj.points, col, 2.5);
            r::draw_marker_end(run.traj.end(), col, &format!("#{} I={:.2}", i + 1, run.intensity));
        }
    }
    r::draw_start(start);
    if g.runs.is_empty() {
        r::draw_probe(start, r::c_probe());
    }
    r::end_area();

    let flying = st.flying();
    let sp = (hi - lo) * 0.25 * dt;
    if is_key_down(KeyCode::Left) {
        st.exp_i = (st.exp_i - sp).max(lo);
    }
    if is_key_down(KeyCode::Right) {
        st.exp_i = (st.exp_i + sp).min(hi);
    }
    let mut y = panel_header(&lvl);
    y = wrap(&g.question(), PX0, y + 8.0, PW, 17.0, WHITE) + 4.0;
    text(&format!("Intensity {:.2}", st.exp_i), PX0, y + 14.0, 19.0, WHITE);
    y += 22.0;
    ui::slider(inp, Rect::new(PX0, y, PW, 18.0), &mut st.exp_i, lo, hi, chapter_color(Chapter::Experiment));
    let lim = g.rig_limit();
    if lim < hi {
        let x = PX0 + PW * ((lim - lo) / (hi - lo)) as f32;
        draw_rectangle(x, y + 22.0, PX0 + PW - x, 5.0, r::c_hazard());
        text(&format!("rig stops at {:.1}", lim), x - 90.0, y + 40.0, 14.0, r::c_hazard());
    }
    y += 40.0;
    let run = ui::button(inp, Rect::new(PX0, y, 116.0, 34.0), "Run (Space)", !flying, chapter_color(Chapter::Experiment));
    if (run || space()) && !flying {
        match g.run(st.exp_i) {
            Ok(i) => {
                let t = g.runs[i].traj.clone();
                st.msg = format!("Run {}: intensity {:.2} -> {}.", i + 1, g.runs[i].intensity, short(&lvl, t.outcome));
                st.good = None;
                st.anim = Some(Anim::new(vec![(t, r::run_color(i))]));
            }
            Err(e) => {
                st.msg = e;
                st.good = Some(false);
            }
        }
    }
    if ui::button(inp, Rect::new(PX0 + 120.0, y, 116.0, 34.0), "Reset runs", !g.runs.is_empty(), rgb(120, 130, 160)) {
        st.anim = None;
        st.msg.clear();
        st.good = None;
        g.reset();
    }
    y += 44.0;
    text("Runs (click to show or hide)", PX0, y + 8.0, 15.0, gray());
    y += 14.0;
    let shown = g.runs.len().min(4);
    let first = g.runs.len() - shown;
    for k in 0..shown {
        let i = first + k;
        let rr = Rect::new(PX0, y + k as f32 * 21.0, PW, 20.0);
        let col = r::run_color(i);
        if g.runs[i].visible {
            draw_circle(PX0 + 8.0, rr.y + 10.0, 6.0, col);
        } else {
            draw_circle_lines(PX0 + 8.0, rr.y + 10.0, 6.0, 2.0, col);
        }
        text(&format!("#{}  I={:.2}  {}", i + 1, g.runs[i].intensity, short(&lvl, g.runs[i].traj.outcome)), PX0 + 22.0, rr.y + 15.0, 16.0, if g.runs[i].visible { WHITE } else { gray() });
        if inp.pressed && rr.contains(inp.mouse) {
            g.toggle_visible(i);
        }
    }
    y += 4.0 * 21.0 + 6.0;
    text("Evidence needed", PX0, y + 8.0, 15.0, gray());
    y += 12.0;
    for (k, (label, ok)) in g.evidence().iter().enumerate() {
        let yy = y + k as f32 * 17.0;
        draw_rectangle_lines(PX0, yy, 11.0, 11.0, 2.0, if *ok { good_c() } else { gray() });
        if *ok {
            draw_line(PX0 + 2.0, yy + 6.0, PX0 + 5.0, yy + 9.0, 2.0, good_c());
            draw_line(PX0 + 5.0, yy + 9.0, PX0 + 10.0, yy + 1.0, 2.0, good_c());
        }
        text(label, PX0 + 17.0, yy + 11.0, 15.0, if *ok { good_c() } else { gray() });
    }
    y += 3.0 * 17.0 + 8.0;
    text("Your conclusion", PX0, y + 8.0, 15.0, gray());
    y += 14.0;
    let can = g.can_conclude();
    let choices = g.choices();
    let mut submit: Option<Conclusion> = None;
    if !choices.is_empty() {
        let short_label = |s: &str| -> String { s.split([':', '/']).next().unwrap_or(s).trim().to_string() };
        for (k, c) in choices.iter().enumerate() {
            let rect = Rect::new(PX0 + (k % 2) as f32 * 120.0, y + (k / 2) as f32 * 30.0, 116.0, 26.0);
            let sel = st.exp_choice == Some(k);
            if ui::button(inp, rect, &short_label(c), can, if sel { good_c() } else { rgb(120, 130, 160) }) {
                st.exp_choice = Some(k);
            }
            if sel {
                draw_rectangle_lines(rect.x - 2.0, rect.y - 2.0, rect.w + 4.0, rect.h + 4.0, 3.0, good_c());
            }
        }
        let k = choices.len();
        let rect = Rect::new(PX0 + (k % 2) as f32 * 120.0, y + (k / 2) as f32 * 30.0, 116.0, 26.0);
        if ui::button(inp, rect, "Submit", can && st.exp_choice.is_some(), good_c()) {
            submit = Some(Conclusion::Choice(st.exp_choice.unwrap()));
        }
    } else {
        let n = g.estimate_count();
        for k in 0..n {
            text(&format!("Estimate {}: {:.2}", k + 1, st.exp_est[k]), PX0, y + 10.0, 15.0, WHITE);
            ui::slider(inp, Rect::new(PX0, y + 14.0, PW, 12.0), &mut st.exp_est[k], lo, hi, good_c());
            y += 34.0;
        }
        if ui::button(inp, Rect::new(PX0, y, PW, 30.0), "Report finding", can, good_c()) {
            submit = Some(Conclusion::Estimates(st.exp_est[..n].to_vec()));
        }
    }
    if let Some(c) = submit {
        let v = g.conclude(c);
        st.msg = v.message;
        st.good = Some(v.correct);
    }
}
