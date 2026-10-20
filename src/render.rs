//! Drawing of the play area. Colours follow the Okabe-Ito colour-blind-safe palette and every
//! entity also has a distinct shape, so colour is never the only cue.
use crate::ui::{alpha, rgb, text_c, Input, H, W};
use macroquad::prelude::*;
use vfield::sim::{Region, RegionKind};
use vfield::{Field, Zone, V2};

// Play area on the design canvas; the world is always x in [-6,6], y in [-4,4].
pub const PX: f32 = 80.0;
pub const AX: f32 = 20.0;
pub const AY: f32 = 80.0;
pub const AW: f32 = 960.0;
pub const AH: f32 = 640.0;

pub fn bg() -> Color {
    rgb(19, 22, 30)
}
pub fn c_probe() -> Color {
    rgb(230, 159, 0)
}
pub fn c_target() -> Color {
    rgb(0, 158, 115)
}
pub fn c_hazard() -> Color {
    rgb(213, 94, 0)
}
pub fn c_gate() -> Color {
    rgb(240, 228, 66)
}
pub fn c_predict() -> Color {
    rgb(86, 180, 233)
}
pub fn c_zone() -> Color {
    rgb(204, 121, 167)
}
/// Historical-run colours for Experiment (cycled).
pub fn run_color(i: usize) -> Color {
    [rgb(86, 180, 233), rgb(240, 228, 66), rgb(204, 121, 167), rgb(0, 158, 115), rgb(213, 94, 0), rgb(230, 159, 0), rgb(120, 150, 255), rgb(210, 210, 210)][i % 8]
}

pub fn w2s(p: V2) -> Vec2 {
    vec2(AX + (p.x as f32 + 6.0) * PX, AY + (4.0 - p.y as f32) * PX)
}
pub fn s2w(s: Vec2) -> V2 {
    V2 { x: ((s.x - AX) / PX - 6.0) as f64, y: (4.0 - (s.y - AY) / PX) as f64 }
}
pub fn in_area(s: Vec2) -> bool {
    Rect::new(AX, AY, AW, AH).contains(s)
}

pub fn begin_area() {
    draw_rectangle(AX, AY, AW, AH, bg());
    // faint axes for orientation
    let o = w2s(V2 { x: 0.0, y: 0.0 });
    draw_line(AX, o.y, AX + AW, o.y, 1.0, rgb(34, 38, 50));
    draw_line(o.x, AY, o.x, AY + AH, 1.0, rgb(34, 38, 50));
}

/// Hide anything that spilled outside the play area, then draw the frame.
pub fn end_area() {
    let c = rgb(14, 16, 22);
    draw_rectangle(0.0, 0.0, W, AY, c);
    draw_rectangle(0.0, AY + AH, W, H - (AY + AH), c);
    draw_rectangle(0.0, 0.0, AX, H, c);
    draw_rectangle(AX + AW, 0.0, W - (AX + AW), H, c);
    draw_rectangle_lines(AX, AY, AW, AH, 2.0, rgb(90, 98, 120));
}

pub fn draw_field(field: &Field, t: f64, mode: usize, hc: bool) {
    if mode == 0 {
        return;
    }
    let sp = [0.0, 0.75, 0.5, 0.34][mode];
    let (nx, ny) = ((12.0 / sp) as i32, (8.0 / sp) as i32);
    let cell = sp as f32 * PX;
    for i in 0..nx {
        for j in 0..ny {
            let p = V2 { x: -6.0 + sp * (i as f64 + 0.5), y: -4.0 + sp * (j as f64 + 0.5) };
            let v = field.eval(p, t);
            let s = v.len() as f32;
            if !s.is_finite() {
                continue;
            }
            let c = w2s(p);
            let strength = (s / 3.0).min(1.0);
            let col = if hc { alpha(rgb(210, 222, 245), 0.9) } else { alpha(rgb(120, 140, 175), 0.35 + 0.55 * strength) };
            if s < 0.02 {
                draw_circle(c.x, c.y, 1.5, col);
                continue;
            }
            let d = vec2(v.x as f32 / s, -(v.y as f32) / s); // screen y is flipped
            let len = (s * 20.0).clamp(4.0, cell * 0.92);
            let (tail, tip) = (c - d * (len / 2.0), c + d * (len / 2.0));
            let th = if hc { 2.2 } else { 1.4 };
            draw_line(tail.x, tail.y, tip.x, tip.y, th, col);
            let h = (len * 0.45).min(7.0);
            let n = vec2(-d.y, d.x);
            draw_triangle(tip, tip - d * h + n * (h * 0.45), tip - d * h - n * (h * 0.45), col);
        }
    }
}

pub fn dashed_line(a: Vec2, b: Vec2, col: Color, th: f32) {
    let (dash, gap) = (9.0, 6.0);
    let total = (b - a).length();
    if total < 1.0 {
        return;
    }
    let d = (b - a) / total;
    let mut k = 0.0;
    while k < total {
        let e = (k + dash).min(total);
        draw_line(a.x + d.x * k, a.y + d.y * k, a.x + d.x * e, a.y + d.y * e, th, col);
        k += dash + gap;
    }
}

pub fn draw_path(points: &[V2], col: Color, th: f32) {
    let mut last: Option<Vec2> = None;
    for (i, p) in points.iter().enumerate() {
        if i % 2 != 0 && i + 1 != points.len() {
            continue;
        }
        let s = w2s(*p);
        if let Some(l) = last {
            draw_line(l.x, l.y, s.x, s.y, th, col);
        }
        last = Some(s);
    }
}

pub fn draw_probe(p: V2, col: Color) {
    let s = w2s(p);
    draw_circle(s.x, s.y, 8.0, BLACK);
    draw_circle(s.x, s.y, 6.0, col);
}

pub fn draw_start(p: V2) {
    let s = w2s(p);
    draw_poly(s.x, s.y, 4, 13.0, 0.0, rgb(14, 16, 22));
    draw_poly_lines(s.x, s.y, 4, 13.0, 0.0, 2.5, WHITE);
}

pub fn draw_marker_end(p: V2, col: Color, label: &str) {
    let s = w2s(p);
    draw_circle_lines(s.x, s.y, 6.0, 2.5, col);
    if !label.is_empty() {
        crate::ui::text(label, s.x + 9.0, s.y - 7.0, 18.0, col);
    }
}

pub fn draw_region(r: &Region, selected: bool) {
    let c = w2s(r.center);
    let rad = r.radius as f32 * PX;
    match r.kind {
        RegionKind::Target => {
            draw_circle(c.x, c.y, rad, alpha(c_target(), 0.18));
            draw_circle_lines(c.x, c.y, rad, 3.0, c_target());
            draw_circle_lines(c.x, c.y, rad * 0.5, 2.0, c_target());
            draw_line(c.x - 8.0, c.y, c.x + 8.0, c.y, 2.0, c_target());
            draw_line(c.x, c.y - 8.0, c.x, c.y + 8.0, 2.0, c_target());
        }
        RegionKind::Exit => {
            let col = if selected { c_predict() } else { rgb(200, 206, 220) };
            draw_circle(c.x, c.y, rad, alpha(col, if selected { 0.28 } else { 0.07 }));
            draw_circle_lines(c.x, c.y, rad, if selected { 5.0 } else { 3.0 }, col);
            // label offset towards the play-area centre so it stays visible for corner exits
            let o = w2s(vfield::v2(0.0, 0.0));
            let dir = (o - c).normalize_or_zero();
            let lp = c + dir * (rad * 0.45).min(rad - 20.0);
            let lp = vec2(lp.x.clamp(AX + 20.0, AX + AW - 20.0), lp.y.clamp(AY + 20.0, AY + AH - 20.0));
            text_c(&r.label.to_string(), lp.x, lp.y, 34.0, col);
        }
        RegionKind::Hazard => {
            draw_circle(c.x, c.y, rad, alpha(c_hazard(), 0.3));
            let n = Vec2::new(std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2);
            let d = Vec2::new(std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2);
            let mut off = -rad + 6.0;
            while off < rad {
                let half = (rad * rad - off * off).max(0.0).sqrt();
                let m = Vec2::new(c.x + n.x * off, c.y + n.y * off);
                draw_line(m.x - d.x * half, m.y - d.y * half, m.x + d.x * half, m.y + d.y * half, 1.5, alpha(c_hazard(), 0.8));
                off += 9.0;
            }
            draw_circle_lines(c.x, c.y, rad, 3.0, c_hazard());
        }
        RegionKind::Gate => {
            for k in (0..28).step_by(2) {
                let a0 = k as f32 / 28.0 * std::f32::consts::TAU;
                let a1 = (k + 1) as f32 / 28.0 * std::f32::consts::TAU;
                draw_line(c.x + rad * a0.cos(), c.y + rad * a0.sin(), c.x + rad * a1.cos(), c.y + rad * a1.sin(), 3.0, c_gate());
            }
            text_c("GATE", c.x, c.y, 16.0, c_gate());
        }
    }
}

pub fn draw_zone(z: &Zone) {
    let col = c_zone();
    match z {
        Zone::Circle(c, r) => {
            let s = w2s(*c);
            let rad = *r as f32 * PX;
            draw_circle(s.x, s.y, rad, alpha(col, 0.12));
            for k in (0..40).step_by(2) {
                let a0 = k as f32 / 40.0 * std::f32::consts::TAU;
                let a1 = (k + 1) as f32 / 40.0 * std::f32::consts::TAU;
                draw_line(s.x + rad * a0.cos(), s.y + rad * a0.sin(), s.x + rad * a1.cos(), s.y + rad * a1.sin(), 2.0, col);
            }
        }
        Zone::Rect(r) => {
            let (a, b) = (w2s(r.min), w2s(r.max));
            let (x0, y0, x1, y1) = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
            draw_rectangle(x0, y0, x1 - x0, y1 - y0, alpha(col, 0.12));
            dashed_line(vec2(x0, y0), vec2(x1, y0), col, 2.0);
            dashed_line(vec2(x1, y0), vec2(x1, y1), col, 2.0);
            dashed_line(vec2(x1, y1), vec2(x0, y1), col, 2.0);
            dashed_line(vec2(x0, y1), vec2(x0, y0), col, 2.0);
        }
    }
}

/// Shorthand to check a click on an exit circle.
pub fn hit_region(inp: &Input, r: &Region) -> bool {
    in_area(inp.mouse) && (inp.mouse - w2s(r.center)).length() <= r.radius as f32 * PX
}

/// Straight arrow with a filled head. `dashed` draws the shaft as dashes (used for the gust).
pub fn draw_arrow(a: Vec2, b: Vec2, col: Color, th: f32, dashed: bool) {
    let d = b - a;
    let len = d.length();
    if len < 2.0 {
        return;
    }
    let dir = d / len;
    let n = vec2(-dir.y, dir.x);
    let head = (len * 0.4).clamp(7.0, 14.0);
    let base = b - dir * head;
    if dashed {
        dashed_line(a, base, col, th);
    } else {
        draw_line(a.x, a.y, base.x, base.y, th, col);
    }
    draw_triangle(b, base + n * (head * 0.5), base - n * (head * 0.5), col);
}

/// Small marker for a gust that was applied: a short arrow pointing in the gust direction, anchored at `at`.
pub fn draw_gust_marker(at: V2, dir_world: V2, col: Color) {
    let a = w2s(at);
    let d = vec2(dir_world.x as f32, -(dir_world.y as f32)).normalize_or_zero();
    draw_circle(a.x, a.y, 5.0, col);
    draw_arrow(a, a + d * 26.0, col, 3.0, false);
}
