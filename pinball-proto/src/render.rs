//! Top-down drawing with macroquad. Ramps and the upper level are drawn on top
//! of the playfield, and balls on them are drawn bigger with an offset shadow,
//! as a stand-in until the 2.5D view.

use macroquad::prelude::*;

use crate::game::Game;
use crate::layout::{Layer, Shape, Sw};
use crate::physics::Table;

type PVec = rapier2d::math::Vector;

/// Maps table metres (y up) to screen pixels (y down), fitting the table to the window.
struct View {
    scale: f32,
    ox: f32,
    oy: f32,
}

impl View {
    fn fit(size: PVec) -> Self {
        let (margin, hud) = (16.0, 40.0);
        let scale = ((screen_width() - 2.0 * margin) / size.x).min((screen_height() - 2.0 * margin - hud) / size.y);
        let ox = (screen_width() - size.x * scale) / 2.0;
        let oy = margin + hud + size.y * scale;
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
    const ABOVE: [(u8, u8, u8); 4] = [(120, 190, 255), (255, 140, 120), (140, 230, 140), (200, 160, 255)];
    if l == Layer::PLAYFIELD {
        return Color::from_rgba(200, 210, 230, 255);
    }
    let (r, g, b) = ABOVE[(l.index() - 1) % ABOVE.len()];
    Color::from_rgba(r, g, b, 255)
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
    for i in 1..pts.len().saturating_sub(1) {
        draw_triangle(pts[0], pts[i], pts[i + 1], color);
    }
}

fn draw_ball(view: &View, pos: PVec, radius: f32, height: f32) {
    let r = radius * view.scale * (1.0 + height * 4.0);
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
    let layout = &table.layout;
    let size = layout.size;
    let view = View::fit(size);

    let top_left = view.p(PVec::new(0.0, size.y));
    draw_rectangle(top_left.x, top_left.y, size.x * view.scale, size.y * view.scale, Color::from_rgba(24, 40, 64, 255));

    // --- Playfield layer ---
    for w in layout.walls.iter().filter(|w| w.layer == Layer::PLAYFIELD) {
        polyline(&view, &w.pts, w.closed, view.px(0.004), layer_color(Layer::PLAYFIELD));
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
    // Kickback light, next to the kickback switch.
    for sen in layout.sensors.iter().filter(|s| s.sw == Some(Sw::Kickback)) {
        if let Shape::Rect { min, max } = sen.shape {
            let kb = view.p(PVec::new((min.x + max.x) / 2.0, max.y + 0.03));
            draw_circle(kb.x, kb.y, 0.006 * view.scale, if game.kickback_lit { GREEN } else { DARKGRAY });
        }
    }

    let draw_flippers = |layer: Layer| {
        for f in table.flippers.iter().filter(|f| f.def.layer == layer) {
            let pose = table.flipper_pose(f);
            let pts: Vec<Vec2> = f.outline.iter().map(|&p| view.p(pose * p)).collect();
            fill_convex(&pts, ORANGE);
        }
    };
    draw_flippers(Layer::PLAYFIELD);

    // Plunger charge, as a bar under the launch position.
    if table.plunger_charge > 0.0 {
        let r = layout.ball.radius;
        let base = view.p(layout.launcher.position - PVec::new(r, r));
        let h = 0.15 * table.plunger_charge * view.scale;
        draw_rectangle(base.x, base.y, 2.0 * r * view.scale, h, Color::from_rgba(255, 120, 0, 200));
    }

    let balls_on = |layer: Layer| {
        for b in table.balls.iter().filter(|b| b.layer == layer && b.held.is_none()) {
            draw_ball(&view, table.ball_pos(b), layout.ball.radius, b.height);
        }
    };
    balls_on(Layer::PLAYFIELD);

    // --- Layers above the playfield in order, each with what's on it ---
    for (i, def) in layout.layers.iter().enumerate().skip(1) {
        let layer = Layer(i as u8);
        let col = layer_color(layer);
        let area: Vec<Vec2> = def.outline.iter().map(|&p| view.p(p)).collect();
        fill_convex(&area, Color::new(col.r, col.g, col.b, 0.25));
        for r in layout.ramps.iter().filter(|r| r.layer == layer) {
            let fill = Color::new(col.r, col.g, col.b, 0.18);
            for k in 0..r.center.len() - 1 {
                let (a, b, c, d) =
                    (view.p(r.left[k]), view.p(r.left[k + 1]), view.p(r.right[k + 1]), view.p(r.right[k]));
                draw_triangle(a, b, c, fill);
                draw_triangle(a, c, d, fill);
            }
        }
        for w in layout.walls.iter().filter(|w| w.layer == layer) {
            polyline(&view, &w.pts, w.closed, view.px(0.0035), col);
        }
        for s in layout.standups.iter().filter(|s| s.layer == layer) {
            let (a, b) = (view.p(s.a), view.p(s.b));
            draw_line(a.x, a.y, b.x, b.y, view.px(0.006), MAGENTA);
        }
        draw_flippers(layer);
        balls_on(layer);
    }

    // --- HUD ---
    let hud = format!(
        "SCORE {:>9}   BALL {}   BONUS {}X{}",
        game.score,
        game.ball_no,
        game.bonus_x,
        if game.ball_save > 0.0 { "   SAVE" } else { "" }
    );
    draw_text(&hud, 16.0, 34.0, 30.0, WHITE);
    let name_size = measure_text(&layout.name, None, 20, 1.0);
    draw_text(&layout.name, screen_width() - name_size.width - 16.0, 52.0, 20.0, GRAY);
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
