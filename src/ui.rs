//! Tiny immediate-mode widgets on a fixed 1280x800 design canvas (letterboxed, mouse mapped back).
use macroquad::prelude::*;

pub const W: f32 = 1280.0;
pub const H: f32 = 800.0;

pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}
pub fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

pub struct Input {
    pub mouse: Vec2,
    pub pressed: bool,
    pub down: bool,
}

/// Installs the letterboxed camera and returns the mouse in design coordinates.
pub fn begin_frame() -> Input {
    let (sw, sh) = (screen_width(), screen_height());
    let s = (sw / W).min(sh / H);
    let (vw, vh) = (sw / s, sh / s);
    let (ox, oy) = (-(vw - W) / 2.0, -(vh - H) / 2.0);
    // macroquad already flips y when drawing to the screen, so use a positive zoom (y-down canvas).
    set_camera(&Camera2D {
        target: vec2(ox + vw / 2.0, oy + vh / 2.0),
        zoom: vec2(2.0 / vw, 2.0 / vh),
        ..Default::default()
    });
    let m = mouse_position();
    Input {
        mouse: vec2(m.0 / s + ox, m.1 / s + oy),
        pressed: is_mouse_button_pressed(MouseButton::Left),
        down: is_mouse_button_down(MouseButton::Left),
    }
}

pub fn text(s: &str, x: f32, y: f32, size: f32, c: Color) {
    draw_text(s, x, y, size, c);
}

pub fn text_c(s: &str, cx: f32, cy: f32, size: f32, c: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, cx - d.width / 2.0, cy + d.height / 2.0, size, c);
}

/// The lines `wrap` would draw for `s` in a column `w` wide.
pub fn wrap_lines(s: &str, w: f32, size: f32) -> Vec<String> {
    let mut lines = vec![];
    let mut line = String::new();
    for word in s.split_whitespace() {
        let trial = if line.is_empty() { word.to_string() } else { format!("{} {}", line, word) };
        if measure_text(&trial, None, size as u16, 1.0).width > w && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = trial;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Height `wrap` will use for `s` (so a box can be sized to its text).
pub fn wrap_height(s: &str, w: f32, size: f32) -> f32 {
    wrap_lines(s, w, size).len() as f32 * size * 1.15
}

/// Word-wrapped text. Returns the y for the next line.
pub fn wrap(s: &str, x: f32, y: f32, w: f32, size: f32, c: Color) -> f32 {
    let mut y = y;
    for line in wrap_lines(s, w, size) {
        draw_text(&line, x, y, size, c);
        y += size * 1.15;
    }
    y
}

pub fn button(inp: &Input, r: Rect, label: &str, enabled: bool, accent: Color) -> bool {
    let hot = enabled && r.contains(inp.mouse);
    let bg = if !enabled {
        rgb(36, 40, 50)
    } else if hot && inp.down {
        rgb(80, 90, 112)
    } else if hot {
        rgb(66, 74, 94)
    } else {
        rgb(48, 54, 70)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if enabled { accent } else { rgb(70, 74, 86) });
    // shrink the font until the label fits inside the button
    let mut size = 20.0;
    while size > 11.0 && measure_text(label, None, size as u16, 1.0).width > r.w - 10.0 {
        size -= 1.0;
    }
    text_c(label, r.x + r.w / 2.0, r.y + r.h / 2.0, size, if enabled { WHITE } else { rgb(110, 114, 126) });
    hot && inp.pressed
}

/// Horizontal slider. Returns true when the value was changed by the mouse.
pub fn slider(inp: &Input, r: Rect, v: &mut f64, lo: f64, hi: f64, c: Color) -> bool {
    let cy = r.y + r.h / 2.0;
    draw_line(r.x, cy, r.x + r.w, cy, 4.0, rgb(70, 76, 92));
    let t = ((*v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
    draw_line(r.x, cy, r.x + r.w * t, cy, 4.0, c);
    draw_circle(r.x + r.w * t, cy, 9.0, c);
    draw_circle_lines(r.x + r.w * t, cy, 9.0, 2.0, WHITE);
    let hit = Rect::new(r.x - 8.0, r.y - 8.0, r.w + 16.0, r.h + 16.0);
    if (inp.down || inp.pressed) && hit.contains(inp.mouse) {
        let nt = ((inp.mouse.x - r.x) / r.w).clamp(0.0, 1.0) as f64;
        let nv = lo + nt * (hi - lo);
        if (nv - *v).abs() > 1e-9 {
            *v = nv;
            return true;
        }
    }
    false
}

/// Phase timeline. `marks` are earlier launches (phase, success). Returns a phase if the player scrubbed.
pub fn timeline(inp: &Input, r: Rect, period: f64, cursor: f64, marks: &[(f64, bool)], label: &str) -> Option<f64> {
    draw_rectangle(r.x, r.y, r.w, r.h, rgb(30, 34, 46));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, rgb(90, 98, 120));
    let n = period.round() as i32;
    for k in 0..=n {
        let x = r.x + r.w * (k as f32 / n as f32);
        draw_line(x, r.y + r.h - 12.0, x, r.y + r.h, 2.0, rgb(120, 128, 150));
        if k < n {
            text(&format!("{}", k), x + 3.0, r.y + r.h - 2.0, 14.0, rgb(130, 138, 160));
        }
    }
    for (ph, ok) in marks {
        let x = r.x + r.w * (*ph / period) as f32;
        if *ok {
            draw_poly(x, r.y + 10.0, 3, 7.0, -90.0, rgb(0, 158, 115));
        } else {
            draw_line(x - 5.0, r.y + 5.0, x + 5.0, r.y + 15.0, 3.0, rgb(213, 94, 0));
            draw_line(x - 5.0, r.y + 15.0, x + 5.0, r.y + 5.0, 3.0, rgb(213, 94, 0));
        }
    }
    let cx = r.x + r.w * (cursor.rem_euclid(period) / period) as f32;
    draw_line(cx, r.y, cx, r.y + r.h, 3.0, rgb(240, 228, 66));
    draw_poly(cx, r.y, 3, 8.0, 90.0, rgb(240, 228, 66));
    text(label, r.x + 6.0, r.y - 6.0, 18.0, rgb(200, 206, 220));
    if (inp.down || inp.pressed) && Rect::new(r.x, r.y - 6.0, r.w, r.h + 12.0).contains(inp.mouse) {
        return Some(((inp.mouse.x - r.x) / r.w).clamp(0.0, 0.9999) as f64 * period);
    }
    None
}
