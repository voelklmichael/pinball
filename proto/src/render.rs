//! Top-down drawing with macroquad, layer by layer: the playfield first, then
//! each layer above it with the walls, flippers and balls on it.

use macroquad::prelude::*;

use std::collections::BTreeMap;

use engine::layout::{Layer, SensorShape};
use engine::physics::Table;
use rules::Lamp;

type PVec = engine::rapier2d::math::Vector;

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

/// Higher balls are drawn bigger, with a shadow, as a stand-in for the 2.5D view.
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

/// Draws the table with a status line on top and an optional message in the
/// middle. Lamps named like an element make it glow.
pub fn draw(table: &Table, status: &str, message: Option<&str>, lamps: &BTreeMap<String, Lamp>) {
    clear_background(Color::from_rgba(14, 18, 32, 255));
    let layout = &table.layout;
    let size = layout.size;
    let view = View::fit(size);

    let top_left = view.p(PVec::new(0.0, size.y));
    draw_rectangle(top_left.x, top_left.y, size.x * view.scale, size.y * view.scale, Color::from_rgba(24, 40, 64, 255));

    for (i, def) in layout.layers.iter().enumerate() {
        let layer = Layer(i as u8);
        let col = layer_color(layer);
        if let Some(ramp) = &def.ramp {
            let fill = Color::new(col.r, col.g, col.b, 0.18);
            for k in 0..ramp.center.len() - 1 {
                let (a, b) = (view.p(ramp.left[k]), view.p(ramp.left[k + 1]));
                let (c, d) = (view.p(ramp.right[k + 1]), view.p(ramp.right[k]));
                draw_triangle(a, b, c, fill);
                draw_triangle(a, c, d, fill);
            }
        }
        let width = if layer == Layer::PLAYFIELD { 0.004 } else { 0.0035 };
        for w in layout.walls.iter().filter(|w| w.layer == layer) {
            let color = match (w.kick > 0.0, w.one_way.is_some()) {
                (true, _) => Color::from_rgba(230, 70, 90, 255),
                (_, true) => Color::from_rgba(255, 160, 0, 255),
                _ => col,
            };
            polyline(&view, &w.pts, w.closed, view.px(width), color);
        }
        for sw in layout.switches.iter().filter(|s| s.layer == layer) {
            let yellow = Color::from_rgba(255, 220, 60, 200);
            match sw.shape {
                SensorShape::Circle { center, radius } => {
                    let c = view.p(center);
                    draw_circle_lines(c.x, c.y, radius * view.scale, 1.5, yellow);
                }
                SensorShape::Rect { min, max } => {
                    let corners = [min, PVec::new(max.x, min.y), max, PVec::new(min.x, max.y)];
                    polyline(&view, &corners, true, 1.5, yellow);
                }
                SensorShape::Segment { a, b } => polyline(&view, &[a, b], false, 1.5, yellow),
            }
        }
        for t in layout.transitions.iter().filter(|t| t.layer == layer) {
            polyline(&view, &[t.a, t.b], false, 1.0, Color::from_rgba(80, 220, 230, 160));
        }
        for (i, d) in layout.drop_targets.iter().enumerate().filter(|(_, d)| d.layer == layer) {
            let color = if table.drops_down[i] { Color::from_rgba(90, 60, 30, 255) } else { ORANGE };
            polyline(&view, &[d.a, d.b], false, view.px(0.006), color);
        }
        for (i, h) in layout.holes.iter().enumerate().filter(|(_, h)| h.layer == layer) {
            let c = view.p(h.center);
            draw_circle(c.x, c.y, h.radius * view.scale, BLACK);
            if table.held_in(i) > 0 {
                draw_circle_lines(c.x, c.y, h.radius * view.scale + 2.0, 2.0, YELLOW);
            }
        }
        for s in layout.spinners.iter().filter(|s| s.layer == layer) {
            polyline(&view, &[s.a, s.b], false, view.px(0.003), Color::from_rgba(230, 230, 240, 220));
        }
        for (i, f) in layout.flaps.iter().enumerate().filter(|(_, f)| f.layer == layer) {
            let pose = table.flap_pose(i);
            let ends = [pose * PVec::ZERO, pose * PVec::new(f.length, 0.0)];
            polyline(&view, &ends, false, view.px(0.004), Color::from_rgba(255, 160, 0, 255));
            let pivot = view.p(f.pivot);
            draw_circle(pivot.x, pivot.y, view.px(0.003), Color::from_rgba(255, 160, 0, 255));
        }
        for f in table.flippers.iter().filter(|f| f.def.layer == layer) {
            let pose = table.flipper_pose(f);
            let pts: Vec<Vec2> = f.outline.iter().map(|&p| view.p(pose * p)).collect();
            fill_convex(&pts, ORANGE);
        }
        for b in table.balls.iter().filter(|b| b.layer == layer) {
            draw_ball(&view, table.ball_pos(b), layout.ball.radius, b.height);
        }
    }

    // Lamps: a glow on the element of the same name (blinking ones flash).
    let flash = (get_time() * 4.0) as i64 % 2 == 0;
    let glow = Color::from_rgba(255, 230, 80, 140);
    for (name, &lamp) in lamps {
        if lamp == Lamp::Off || (lamp == Lamp::Blink && !flash) {
            continue;
        }
        let at = match table.find(name) {
            Some(engine::physics::Thing::Hole(i)) => Some(layout.holes[i].center),
            Some(engine::physics::Thing::Switch(i)) => Some(match layout.switches[i].shape {
                SensorShape::Circle { center, .. } => center,
                SensorShape::Rect { min, max } => (min + max) / 2.0,
                SensorShape::Segment { a, b } => (a + b) / 2.0,
            }),
            Some(engine::physics::Thing::DropTarget(i)) => Some((layout.drop_targets[i].a + layout.drop_targets[i].b) / 2.0),
            Some(engine::physics::Thing::Spinner(i)) => Some((layout.spinners[i].a + layout.spinners[i].b) / 2.0),
            _ => None,
        };
        if let Some(at) = at {
            let c = view.p(at);
            draw_circle(c.x, c.y, 0.02 * view.scale, glow);
        }
    }

    // --- HUD ---
    draw_text(status, 16.0, 34.0, 30.0, WHITE);
    let name_size = measure_text(&layout.name, None, 20, 1.0);
    draw_text(&layout.name, screen_width() - name_size.width - 16.0, 52.0, 20.0, GRAY);
    if let Some(msg) = message {
        let size = measure_text(msg, None, 40, 1.0);
        let y = view.p(PVec::new(0.0, layout.size.y * 0.45)).y;
        draw_text(msg, (screen_width() - size.width) / 2.0, y, 40.0, YELLOW);
    }
}
