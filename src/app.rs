//! Screens, input handling and the front end of each gameplay mode. All rules live in `vfield::modes`.
use crate::render as r;
use crate::ui::{self, alpha, rgb, text, text_c, wrap, Input, H, W};
use macroquad::prelude::*;
use std::collections::HashSet;
use vfield::modes::steer::Run as SteerRun;
use vfield::modes::*;
use vfield::sim::{Outcome, Trajectory};
use vfield::{levels, Chapter, GustDirs, Level, Reveal, Rules, V2};

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
enum Nav {
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
    // Steer
    sel: Option<Action>,
    beat_t: f64,
    replay: Option<SteerReplay>,
}
impl St {
    fn flying(&self) -> bool {
        self.anim.as_ref().is_some_and(|a| !a.done())
    }
    fn msg_visible(&self) -> bool {
        !self.flying()
    }
}

/// Playback of a recorded Steer run (re-simulated from its actions; never touches the live run).
struct SteerReplay {
    run: SteerRun,
    beat: usize,
    t: f64,
    hold: f64,
}

#[allow(clippy::large_enum_variant)]
enum Game {
    Steer(SteerGame),
    Position(PositionGame),
    Predict(PredictGame),
    Tempo(TempoGame),
    Experiment(ExperimentGame),
}
impl Game {
    fn level(&self) -> &Level {
        match self {
            Game::Steer(g) => &g.level,
            Game::Position(g) => &g.level,
            Game::Predict(g) => &g.level,
            Game::Tempo(g) => &g.level,
            Game::Experiment(g) => &g.level,
        }
    }
    fn complete(&self) -> bool {
        match self {
            Game::Steer(g) => g.won,
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
        Rules::Steer { .. } => Game::Steer(SteerGame::new(level)),
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
                    Nav::None => Screen::Play(s),
                    Nav::Menu => Screen::Select(c),
                    Nav::Next => {
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
            let r = Rect::new(40.0 + k as f32 * 202.0, 200.0, 190.0, 300.0);
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
        Chapter::Steer => rgb(213, 94, 0),
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
        Game::Steer(g) => {
            let _ = g.retry();
            st.sel = None;
            st.beat_t = 0.0;
            st.replay = None;
        }
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
        Game::Steer(g) => !g.history.is_empty() && g.phase != Phase::Advancing && st.replay.is_none(),
        _ => st.anim.is_some() || !st.msg.is_empty(),
    }
}

fn play_frame(s: &mut Session, inp: &Input, prefs: &mut Prefs) -> Nav {
    let dt = get_frame_time() as f64;
    if is_key_pressed(KeyCode::G) {
        prefs.grid = (prefs.grid + 1) % 4;
    }
    if is_key_pressed(KeyCode::K) {
        prefs.hc = !prefs.hc;
    }
    if is_key_pressed(KeyCode::Escape) {
        return Nav::Menu;
    }
    if let Some(a) = &mut s.st.anim {
        a.time += dt * a.speed;
    }
    match &mut s.game {
        Game::Steer(g) => steer_frame(g, &mut s.st, inp, prefs, dt),
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
    let can_replay = match &s.game {
        Game::Steer(g) => g.phase != Phase::Advancing && g.replayable().is_some() && s.st.replay.is_none(),
        _ => s.st.anim.is_some(),
    };
    let replay_running = matches!(s.game, Game::Steer(_)) && s.st.replay.is_some();
    let vkey = replay_key(replay_running, can_replay, is_key_pressed(KeyCode::V));
    if vkey == ReplayKey::Stop {
        s.st.replay = None;
    }
    if ui::button(inp, Rect::new(PX0, 742.0, 116.0, 34.0), "Replay (V)", can_replay, acc) || vkey == ReplayKey::Start {
        if let Game::Steer(g) = &s.game {
            if let Some(run) = g.replayable() {
                // re-simulate from the recorded actions: a replay must reproduce, not replay stored pixels
                if let Ok(r) = g.spec.replay(&run.actions()) {
                    s.st.replay = Some(SteerReplay { run: r, beat: 0, t: 0.0, hold: 0.0 });
                }
            }
        } else if let Some(a) = &mut s.st.anim {
            a.time = 0.0;
        }
    }
    let mut action = Nav::None;
    if ui::button(inp, Rect::new(PX0 + 120.0, 742.0, 116.0, 34.0), "Menu (Esc)", true, acc) {
        action = Nav::Menu;
    }
    // completion banner
    let complete = s.game.complete();
    if complete && s.st.msg_visible() && s.st.banner {
        let b = Rect::new(r::AX + r::AW / 2.0 - 240.0, r::AY + 12.0, 480.0, 176.0);
        draw_rectangle(b.x, b.y, b.w, b.h, Color::new(0.06, 0.08, 0.12, 0.96));
        draw_rectangle_lines(b.x, b.y, b.w, b.h, 4.0, good_c());
        text_c("Level complete", b.x + b.w / 2.0, b.y + 36.0, 40.0, good_c());
        let detail = match &s.game {
            Game::Steer(g) => format!("Gusts used: {} of {}   Attempts: {}", g.run().gusts_used(), g.spec.budget, g.attempts + 1),
            Game::Position(g) => format!("Launches used: {}", g.attempts.len()),
            Game::Predict(g) => format!("Forecasts correct: {} of {}", g.correct_count(), g.rounds.len()),
            Game::Tempo(g) => format!("Launches used: {}", g.attempts.len()),
            Game::Experiment(g) => format!("Experiments run: {}", g.runs.len()),
        };
        text_c(&detail, b.x + b.w / 2.0, b.y + 74.0, 22.0, WHITE);
        if ui::button(inp, Rect::new(b.x + 20.0, b.y + 106.0, 200.0, 50.0), "Next (N)", true, good_c()) {
            action = Nav::Next;
        }
        if ui::button(inp, Rect::new(b.x + 260.0, b.y + 106.0, 200.0, 50.0), "Keep playing", true, rgb(120, 130, 160)) {
            s.st.banner = false;
        }
    }
    if complete && is_key_pressed(KeyCode::N) {
        action = Nav::Next;
    }
    action
}

/// What one press of `V` does. It must do exactly one thing: stop a running Steer replay, or start a
/// replay, never both in the same frame (a previous version restarted the replay it had just stopped).
#[derive(Debug, PartialEq, Eq)]
enum ReplayKey {
    None,
    Stop,
    Start,
}
fn replay_key(running: bool, can_start: bool, v_pressed: bool) -> ReplayKey {
    if !v_pressed {
        ReplayKey::None
    } else if running {
        ReplayKey::Stop
    } else if can_start {
        ReplayKey::Start
    } else {
        ReplayKey::None
    }
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

// ------------------------------------------------------------------ STEER
const BEAT_ANIM_SECS: f64 = 0.9;
const FIELD_ARROW: f64 = 0.5; // world units drawn per unit of speed

fn c_field() -> Color {
    r::c_predict()
}
fn c_gust() -> Color {
    r::c_gate()
}

/// Compass layout: grid cell -> (key, direction index in an 8-way set, label). `None` = wait.
const COMPASS: [(KeyCode, Option<usize>, &str); 9] = [
    (KeyCode::Q, Some(3), "Q NW"),
    (KeyCode::W, Some(2), "W N"),
    (KeyCode::E, Some(1), "E NE"),
    (KeyCode::A, Some(4), "A W"),
    (KeyCode::S, None, "S Wait"),
    (KeyCode::D, Some(0), "D E"),
    (KeyCode::Z, Some(5), "Z SW"),
    (KeyCode::X, Some(6), "X S"),
    (KeyCode::C, Some(7), "C SE"),
];

/// Map an 8-way index to the level's own direction index (`GustDirs::Four` only has E, N, W, S).
fn dir_for(dirs: GustDirs, eight: usize) -> Option<usize> {
    match dirs {
        GustDirs::Eight => Some(eight),
        GustDirs::Four => eight.is_multiple_of(2).then_some(eight / 2),
    }
}

fn end_message(e: End, g: &SteerGame) -> String {
    match e {
        End::Success => format!("Reached the target on beat {} using {} gust(s).", g.beat(), g.run().gusts_used()),
        End::Failed(FailReason::Boundary) => "The probe left the field.".into(),
        End::Failed(FailReason::BeatsExhausted) => "Out of beats before reaching the target.".into(),
        End::Failed(FailReason::Invalid) => "Numerical error: the run was stopped.".into(),
        End::Failed(FailReason::Hazard) => "The probe hit a hazard.".into(),
        End::Failed(FailReason::MissedGate) => "Reached the target but skipped the gate.".into(),
        End::Failed(FailReason::WrongRegion) => "The probe ended in the wrong region.".into(),
    }
}

fn draw_run_path(run: &SteerRun, col: Color, th: f32, gust_col: Color, upto_beat: Option<(usize, usize)>) {
    // upto_beat = (beat index in progress, number of its points to show); earlier beats are drawn fully
    for (i, b) in run.beats.iter().enumerate() {
        let pts: Vec<V2> = match upto_beat {
            Some((cur, _)) if i > cur => break,
            Some((cur, n)) if i == cur => b.points[..n.min(b.points.len())].to_vec(),
            _ => b.points.clone(),
        };
        let mut line = vec![b.from];
        line.extend(pts);
        r::draw_path(&line, col, th);
        // beat boundary tick, and a gust marker at the start of every beat that used one
        let bend = r::w2s(b.to());
        if upto_beat.is_none_or(|(cur, _)| i < cur) {
            draw_circle_lines(bend.x, bend.y, 4.0, 1.5, col);
        }
        if let Action::Gust(_) = b.action {
            let dirn = b.gust;
            r::draw_gust_marker(b.from, dirn, gust_col);
            let p = r::w2s(b.from);
            text(&format!("{}", i + 1), p.x + 6.0, p.y - 10.0, 16.0, gust_col);
        }
    }
}

fn steer_frame(g: &mut SteerGame, st: &mut St, inp: &Input, prefs: &Prefs, dt: f64) {
    let lvl = g.level.clone();
    let dirs = g.spec.dirs;
    let nbeats = g.spec.beats;

    // ---- advance the beat animation / replay (time only; decisions are never made for the player)
    if g.phase == Phase::Advancing {
        st.beat_t += dt / BEAT_ANIM_SECS;
        if st.beat_t >= 1.0 {
            if let Some(end) = g.finish_beat() {
                st.msg = end_message(end, g);
                st.good = Some(end == End::Success);
            }
            st.beat_t = 0.0;
        }
    }
    let mut replay_done = false;
    if let Some(rp) = &mut st.replay {
        if rp.beat < rp.run.beats.len() {
            rp.t += dt / BEAT_ANIM_SECS;
            if rp.t >= 1.0 {
                rp.t = 0.0;
                rp.beat += 1;
            }
        } else {
            rp.hold += dt;
            if rp.hold > 1.2 {
                replay_done = true;
            }
        }
    }
    if replay_done {
        st.replay = None;
    }
    let replaying = st.replay.is_some();

    // ---- draw the world
    r::begin_area();
    r::draw_field(&lvl.field, 0.0, prefs.grid, prefs.hc);
    for reg in &lvl.regions {
        r::draw_region(reg, false);
    }
    // ghosts: oldest faintest
    let ng = g.ghosts.len();
    for (i, run) in g.ghosts.iter().enumerate() {
        let a = 0.14 + 0.16 * (i + 1) as f32 / ng.max(1) as f32;
        draw_run_path(run, alpha(rgb(200, 206, 220), a), 2.0, alpha(rgb(200, 206, 220), a + 0.15), None);
    }
    let accepting = g.accepting_input() && !replaying;
    if let Some(rp) = &st.replay {
        let n = rp.run.beats.get(rp.beat).map_or(0, |b| (b.points.len() as f64 * rp.t).ceil() as usize);
        let cur = if rp.beat < rp.run.beats.len() { Some((rp.beat, n)) } else { None };
        draw_run_path(&rp.run, r::c_probe(), 3.5, c_gust(), cur);
        let head = match cur {
            Some((b, n)) if n > 0 => rp.run.beats[b].points[n.min(rp.run.beats[b].points.len()) - 1],
            Some((b, _)) => rp.run.beats[b].from,
            None => rp.run.beats.last().map_or(rp.run.start, |b| b.to()),
        };
        r::draw_probe(head, r::c_probe());
        r::draw_start(rp.run.start);
    } else {
        let run = g.run();
        let cur = if g.phase == Phase::Advancing { g.pending_beat().map(|b| (b.points.len() as f64 * st.beat_t).ceil() as usize) } else { None };
        draw_run_path(&run, r::c_probe(), 3.5, c_gust(), None);
        // the beat in flight
        let mut head = g.pos();
        if let (Some(n), Some(b)) = (cur, g.pending_beat()) {
            let pts = &b.points[..n.min(b.points.len())];
            let mut line = vec![b.from];
            line.extend_from_slice(pts);
            r::draw_path(&line, r::c_probe(), 3.5);
            if let Action::Gust(_) = b.action {
                r::draw_gust_marker(b.from, b.gust, c_gust());
            }
            head = line.last().copied().unwrap_or(b.from);
        }
        r::draw_start(g.spec.start);
        r::draw_probe(head, r::c_probe());
        // ---- decision overlay: field + gust = net velocity, drawn head-to-tail at the probe
        if accepting {
            let a = st.sel.unwrap_or(Action::Wait);
            let p = g.pos();
            let fv = g.field_velocity();
            let gv = g.gust_vector(a);
            let (ps, ft) = (r::w2s(p), r::w2s(p + fv * FIELD_ARROW));
            let nt = r::w2s(p + (fv + gv) * FIELD_ARROW);
            r::draw_arrow(ps, ft, c_field(), 3.0, false);
            text("field", (ps.x + ft.x) / 2.0 - 14.0, (ps.y + ft.y) / 2.0 + 20.0, 17.0, c_field());
            if gv != V2::ZERO {
                r::draw_arrow(ft, nt, c_gust(), 4.0, true);
                text("gust", (ft.x + nt.x) / 2.0 + 10.0, (ft.y + nt.y) / 2.0 + 4.0, 17.0, c_gust());
                r::draw_arrow(ps, nt, WHITE, 2.0, false);
                text("net", nt.x - 34.0, nt.y - 8.0, 17.0, WHITE);
            }
        }
    }
    r::end_area();

    // ---- beat track under the play area: one cell per beat (arrow = gust, dot = wait)
    let track = Rect::new(r::AX, 748.0, r::AW, 32.0);
    let cw = track.w / nbeats as f32;
    let (done_beats, cur_beat) = match &st.replay {
        Some(rp) => (rp.run.beats.iter().take(rp.beat).map(|b| b.action).collect::<Vec<_>>(), rp.beat),
        None => (g.history.iter().map(|b| b.action).collect::<Vec<_>>(), g.history.len()),
    };
    for i in 0..nbeats {
        let c = Rect::new(track.x + i as f32 * cw, track.y, cw - 4.0, track.h);
        draw_rectangle(c.x, c.y, c.w, c.h, rgb(30, 34, 46));
        let live = i == cur_beat && (accepting || g.phase == Phase::Advancing || replaying);
        draw_rectangle_lines(c.x, c.y, c.w, c.h, if live { 3.0 } else { 1.5 }, if live { WHITE } else { rgb(90, 98, 120) });
        text(&format!("{}", i + 1), c.x + 4.0, c.y + 12.0, 14.0, rgb(130, 138, 160));
        if let Some(a) = done_beats.get(i) {
            let mid = vec2(c.x + c.w / 2.0, c.y + c.h / 2.0 + 3.0);
            match a {
                Action::Gust(d) => {
                    let u = dirs.unit(*d);
                    let dv = vec2(u.x as f32, -(u.y as f32));
                    r::draw_arrow(mid - dv * 9.0, mid + dv * 9.0, c_gust(), 3.0, false);
                }
                Action::Wait => draw_circle(mid.x, mid.y, 3.0, rgb(150, 158, 178)),
            }
        }
    }

    // ---- panel
    let mut y = panel_header(&lvl);
    y = wrap("Each beat: choose a gust or wait, then confirm. The field does the rest.", PX0, y + 8.0, PW, 17.0, gray());
    y += 6.0;
    let shown_beat = if matches!(g.phase, Phase::Completed(_)) { g.beat() } else { (g.beat() + 1).min(nbeats) };
    text(&format!("Beat {} of {}", shown_beat, nbeats), PX0, y + 14.0, 20.0, WHITE);
    y += 26.0;
    text("Gusts:", PX0, y + 14.0, 18.0, WHITE);
    for i in 0..g.spec.budget {
        let (cx, cy) = (PX0 + 62.0 + i as f32 * 24.0, y + 8.0);
        if i < g.gusts_left() {
            draw_circle(cx, cy, 8.0, c_gust());
        } else {
            draw_circle_lines(cx, cy, 8.0, 2.0, gray());
        }
    }
    text(&format!("{} left", g.gusts_left()), PX0 + 70.0 + g.spec.budget as f32 * 24.0, y + 14.0, 17.0, gray());
    y += 30.0;
    let sel = st.sel;
    let fv = g.field_velocity();
    let gv = g.gust_vector(sel.unwrap_or(Action::Wait));
    let sel_name = match sel {
        Some(Action::Gust(d)) => format!("gust {}", dirs.name(d)),
        Some(Action::Wait) => "wait".to_string(),
        None => "none yet".to_string(),
    };
    let live_c = |c: Color| if accepting { c } else { rgb(100, 106, 122) };
    text(&format!("Field  ({:+.1}, {:+.1})", fv.x, fv.y), PX0, y + 12.0, 16.0, live_c(c_field()));
    text(&format!("Gust   ({:+.1}, {:+.1})", gv.x, gv.y), PX0, y + 30.0, 16.0, live_c(c_gust()));
    text(&format!("Net    ({:+.1}, {:+.1})", fv.x + gv.x, fv.y + gv.y), PX0, y + 48.0, 16.0, live_c(WHITE));
    text(&format!("Selected: {}", sel_name), PX0, y + 70.0, 17.0, WHITE);
    y += 82.0;

    // compass 3x3 (center = wait). Keys: Q W E / A S D / Z X C, arrows for the cardinals.
    let can_act = accepting;
    for (k, (key, eight, label)) in COMPASS.iter().enumerate() {
        let (col, row) = ((k % 3) as f32, (k / 3) as f32);
        let rect = Rect::new(PX0 + col * 80.0, y + row * 38.0, 76.0, 34.0);
        let action = match eight {
            None => Some(Action::Wait),
            Some(e) => dir_for(dirs, *e).map(Action::Gust),
        };
        let usable = can_act && action.is_some() && (matches!(action, Some(Action::Wait)) || g.gusts_left() > 0);
        let is_sel = sel == action && action.is_some();
        let clicked = ui::button(inp, rect, label, usable, if is_sel { c_gust() } else { rgb(120, 130, 160) });
        if is_sel {
            draw_rectangle_lines(rect.x - 3.0, rect.y - 3.0, rect.w + 6.0, rect.h + 6.0, 3.0, c_gust());
        }
        if usable && (clicked || is_key_pressed(*key)) {
            st.sel = action;
        }
    }
    if can_act {
        for (key, e) in [(KeyCode::Up, 2usize), (KeyCode::Down, 6), (KeyCode::Left, 4), (KeyCode::Right, 0)] {
            if is_key_pressed(key) {
                if let Some(d) = dir_for(dirs, e) {
                    if g.gusts_left() > 0 {
                        st.sel = Some(Action::Gust(d));
                    }
                }
            }
        }
        if is_key_pressed(KeyCode::Key0) || is_key_pressed(KeyCode::Kp0) {
            st.sel = Some(Action::Wait);
        }
    }
    y += 3.0 * 38.0 + 6.0;
    let ready = can_act && st.sel.is_some();
    let go = ui::button(inp, Rect::new(PX0, y, PW, 42.0), "Confirm (Space)", ready, chapter_color(Chapter::Steer));
    if ready && (go || space()) {
        if let Some(a) = st.sel {
            if g.choose(a).is_ok() {
                st.beat_t = 0.0;
                st.sel = None;
                st.msg.clear();
                st.good = None;
            }
        }
    }
    if replaying {
        text("Replay running (V: stop)", PX0, y + 62.0, 16.0, c_gust());
    } else if !g.ghosts.is_empty() {
        text(&format!("Faint paths: {} earlier run(s)", g.ghosts.len()), PX0, y + 62.0, 16.0, gray());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_v_press_does_exactly_one_thing() {
        // running replay: V stops it, and must not also start a new one
        assert_eq!(replay_key(true, false, true), ReplayKey::Stop);
        assert_eq!(replay_key(true, true, true), ReplayKey::Stop);
        // idle with something to replay: V starts it
        assert_eq!(replay_key(false, true, true), ReplayKey::Start);
        // nothing to replay, or no key press: nothing happens
        assert_eq!(replay_key(false, false, true), ReplayKey::None);
        assert_eq!(replay_key(true, true, false), ReplayKey::None);
        assert_eq!(replay_key(false, true, false), ReplayKey::None);
    }
}
