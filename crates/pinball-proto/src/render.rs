//! Top-down drawing with macroquad. Ramps and the upper level are drawn on top
//! of the playfield, and balls on them are drawn bigger with an offset shadow,
//! as a stand-in until the 2.5D view.

use macroquad::prelude::*;

use crate::game::Game;
use crate::layout::{BALL_R, LANE_X, Layer, Shape, Sw, TABLE_H, TABLE_W};
use crate::physics::Table;

type PVec = rapier2d::math::Vector;

/// Maps table metres (y up) to screen pixels (y down), fitting the table to the window.
struct View {
    scale: f32,
    ox: f32,
    oy: f32,
}

impl View {
    fn fit() -> Self {
        let (margin, hud) = (16.0, 40.0);
        let scale =
            ((screen_width() - 2.0 * margin) / TABLE_W).min((screen_height() - 2.0 * margin - hud) / TABLE_H);
        let ox = (screen_width() - TABLE_W * scale) / 2.0;
        let oy = margin + hud + TABLE_H * scale;
        Self { scale, ox, oy }
    }

    fn p(&self, v: PVec) -> Vec2 {
        vec2(self.ox + v.x * self.scale, self.oy - v.y * self.scale)
    }

    fn px(&self, m: f32) -> f32 {
        (m * self.scale).max(1.0)
    }
}

fn layer_color(l: Layer) -> Color {
    match l {
        Layer::Playfield => Color::from_rgba(200, 210, 230, 255),
        Layer::RampL => Color::from_rgba(120, 190, 255, 255),
        Layer::RampC => Color::from_rgba(255, 140, 120, 255),
        Layer::RampR => Color::from_rgba(140, 230, 140, 255),
        Layer::Upper => Color::from_rgba(200, 160, 255, 255),
    }
}

fn polyline(view: &View, pts: &[PVec], closed: bool, width: f32, color: Color) {
    let n = pts.len();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (a, b) = (view.p(pts[i]), view.p(pts[(i + 1) % n]));
        draw_line(a.x, a.y, b.x, b.y, width, color);
    }
}

/// Fills a convex polygon as a fan.
fn fill_convex(pts: &[Vec2], color: Color) {
    for i in 1..pts.len() - 1 {
        draw_triangle(pts[0], pts[i], pts[i + 1], color);
    }
}

fn draw_ball(view: &View, pos: PVec, height: f32) {
    let r = BALL_R * view.scale * (1.0 + height * 4.0);
    let c = view.p(pos);
    if height > 0.0 {
        let off = height * view.scale * 0.4;
        draw_circle(c.x + off, c.y + off, r, Color::new(0.0, 0.0, 0.0, 0.35));
    }
    draw_circle(c.x, c.y, r, LIGHTGRAY);
    draw_circle(c.x - 0.3 * r, c.y - 0.3 * r, 0.35 * r, WHITE);
}

pub fn draw(table: &Table, game: &Game) {
    clear_background(Color::from_rgba(14, 18, 32, 255));
    let view = View::fit();
    let layout = &table.layout;

    let top_left = view.p(PVec::new(0.0, TABLE_H));
    draw_rectangle(top_left.x, top_left.y, TABLE_W * view.scale, TABLE_H * view.scale, Color::from_rgba(24, 40, 64, 255));

    // --- Playfield layer ---
    for w in layout.walls.iter().filter(|w| w.layer == Layer::Playfield) {
        polyline(&view, &w.pts, w.closed, view.px(0.004), layer_color(Layer::Playfield));
    }
    for (i, b) in layout.bumpers.iter().enumerate() {
        let c = view.p(b.center);
        let fill = if table.bumper_flash[i] > 0.0 { YELLOW } else { Color::from_rgba(200, 60, 80, 255) };
        draw_circle(c.x, c.y, b.radius * view.scale, fill);
        draw_circle_lines(c.x, c.y, b.radius * view.scale, view.px(0.003), WHITE);
    }
    for (i, s) in layout.slings.iter().enumerate() {
        fill_convex(&[view.p(s.a), view.p(s.b), view.p(s.c)], Color::from_rgba(60, 70, 100, 255));
        let (a, b) = (view.p(s.a), view.p(s.b));
        let col = if table.sling_flash[i] > 0.0 { YELLOW } else { Color::from_rgba(80, 200, 255, 255) };
        draw_line(a.x, a.y, b.x, b.y, view.px(0.006), col);
    }
    for (i, d) in layout.drops.iter().enumerate() {
        let lo = view.p(d.center + PVec::new(-d.half.x, d.half.y));
        let col = if table.drops_down[i] { Color::from_rgba(80, 50, 20, 255) } else { ORANGE };
        draw_rectangle(lo.x, lo.y, 2.0 * d.half.x * view.scale, 2.0 * d.half.y * view.scale, col);
    }
    for g in &layout.gates {
        let (a, b) = (view.p(g.a), view.p(g.b));
        draw_line(a.x, a.y, b.x, b.y, view.px(0.002), Color::from_rgba(255, 160, 0, 255));
    }
    for (i, h) in layout.holes.iter().enumerate() {
        let c = view.p(h.center);
        let lit = i == 0 && game.scoop_lit;
        draw_circle(c.x, c.y, (h.radius + 0.004) * view.scale, if lit { YELLOW } else { BLACK });
    }
    // B-I-E-R lane lights.
    for sen in &layout.sensors {
        if let (Shape::Circle { c, .. }, Some(Sw::Rollover(k))) = (sen.shape, sen.sw) {
            let p = view.p(c);
            let lit = game.lanes[k as usize];
            draw_circle(p.x, p.y, 0.008 * view.scale, if lit { YELLOW } else { DARKGRAY });
            draw_text(["B", "I", "E", "R"][k as usize], p.x - 5.0, p.y - 12.0, 18.0, WHITE);
        }
    }
    // Kickback light.
    let kb = view.p(PVec::new(0.02, 0.16));
    draw_circle(kb.x, kb.y, 0.006 * view.scale, if game.kickback_lit { GREEN } else { DARKGRAY });

    let draw_flippers = |layer: Layer| {
        for f in table.flippers.iter().filter(|f| f.def.layer == layer) {
            let pose = table.flipper_pose(f);
            let pts: Vec<Vec2> = f.outline.iter().map(|&p| view.p(pose * p)).collect();
            fill_convex(&pts, ORANGE);
        }
    };
    draw_flippers(Layer::Playfield);

    if table.plunger_charge > 0.0 {
        let base = view.p(PVec::new(LANE_X + 0.006, 0.025));
        let w = (TABLE_W - LANE_X - 0.012) * view.scale;
        let h = 0.15 * table.plunger_charge * view.scale;
        draw_rectangle(base.x, base.y, w, h, Color::from_rgba(255, 120, 0, 200));
    }

    let balls_on = |layer: Layer| {
        for b in table.balls.iter().filter(|b| b.layer == layer && b.held.is_none()) {
            draw_ball(&view, table.ball_pos(b), b.height);
        }
    };
    balls_on(Layer::Playfield);

    // --- Ramps, then the upper level, each with the balls on it ---
    for r in &layout.ramps {
        let col = layer_color(r.layer);
        let fill = Color::new(col.r, col.g, col.b, 0.18);
        for k in 0..r.center.len() - 1 {
            let (a, b, c, d) = (view.p(r.left[k]), view.p(r.left[k + 1]), view.p(r.right[k + 1]), view.p(r.right[k]));
            draw_triangle(a, b, c, fill);
            draw_triangle(a, c, d, fill);
        }
        polyline(&view, &r.left, false, view.px(0.003), col);
        polyline(&view, &r.right, false, view.px(0.003), col);
        balls_on(r.layer);
    }
    let upper: Vec<Vec2> = layout.upper_outline.iter().map(|&p| view.p(p)).collect();
    let up_col = layer_color(Layer::Upper);
    fill_convex(&upper, Color::new(up_col.r, up_col.g, up_col.b, 0.25));
    for w in layout.walls.iter().filter(|w| w.layer == Layer::Upper) {
        polyline(&view, &w.pts, w.closed, view.px(0.004), up_col);
    }
    for s in &layout.standups {
        let (a, b) = (view.p(s.a), view.p(s.b));
        draw_line(a.x, a.y, b.x, b.y, view.px(0.006), MAGENTA);
    }
    draw_flippers(Layer::Upper);
    balls_on(Layer::Upper);

    // --- HUD ---
    let hud = format!(
        "SCORE {:>9}   BALL {}   BONUS {}X{}",
        game.score,
        game.ball_no,
        game.bonus_x,
        if game.ball_save > 0.0 { "   SAVE" } else { "" }
    );
    draw_text(&hud, 16.0, 34.0, 30.0, WHITE);
    let msg = if game.game_over {
        Some("GAME OVER - press N")
    } else {
        game.message.as_ref().map(|(m, _)| m.as_str())
    };
    if let Some(msg) = msg {
        let size = measure_text(msg, None, 40, 1.0);
        let y = view.p(PVec::new(0.0, 0.47)).y;
        draw_text(msg, (screen_width() - size.width) / 2.0, y, 40.0, YELLOW);
    }
}
