//! Minimal pinball prototype: rapier2d for physics, macroquad for drawing/input.
//!
//! Controls: Left Shift / Z = left flipper, Right Shift / M = right flipper,
//! hold and release Space or Down = plunger, R = reset ball, N = new game, Esc = quit.

mod table;

use macroquad::prelude::*;
use table::{BALL_R, KickerKind, LANE_X, PHYSICS_HZ, TABLE_H, TABLE_W, Table};

type PVec = rapier2d::math::Vector;

/// Maps table metres (y up) to screen pixels (y down), fitting the table to the window.
struct View {
    scale: f32,
    ox: f32,
    oy: f32,
}

impl View {
    fn fit() -> Self {
        let margin = 20.0;
        let scale = ((screen_width() - 2.0 * margin) / TABLE_W)
            .min((screen_height() - 2.0 * margin - 30.0) / TABLE_H);
        let ox = (screen_width() - TABLE_W * scale) / 2.0;
        let oy = margin + 30.0 + TABLE_H * scale;
        Self { scale, ox, oy }
    }

    fn p(&self, v: PVec) -> Vec2 {
        vec2(self.ox + v.x * self.scale, self.oy - v.y * self.scale)
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Pinball prototype".into(),
        window_width: 600,
        window_height: 1000,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut table = Table::new();
    let step_dt = 1.0 / PHYSICS_HZ;
    let mut accumulator = 0.0;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::R) {
            table.reset_ball();
        }
        if is_key_pressed(KeyCode::N) {
            table.new_game();
        }

        table.flippers[0].pressed = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::Z);
        table.flippers[1].pressed = is_key_down(KeyCode::RightShift) || is_key_down(KeyCode::M);
        let frame_dt = get_frame_time().min(0.05);
        table.plunger(is_key_down(KeyCode::Space) || is_key_down(KeyCode::Down), frame_dt);

        // Fixed timestep, decoupled from the render frame rate.
        accumulator += frame_dt;
        while accumulator >= step_dt {
            if table.balls_left > 0 {
                table.step();
            }
            accumulator -= step_dt;
        }

        draw(&table);
        next_frame().await;
    }
}

fn draw(table: &Table) {
    clear_background(Color::from_rgba(14, 18, 32, 255));
    let view = View::fit();
    let px = |m: f32| (m * view.scale).max(1.0);

    let top_left = view.p(PVec::new(0.0, TABLE_H));
    draw_rectangle(
        top_left.x,
        top_left.y,
        TABLE_W * view.scale,
        TABLE_H * view.scale,
        Color::from_rgba(24, 40, 64, 255),
    );

    let wall = Color::from_rgba(200, 210, 230, 255);
    for w in &table.walls {
        for seg in w.windows(2) {
            let (a, b) = (view.p(seg[0]), view.p(seg[1]));
            draw_line(a.x, a.y, b.x, b.y, px(0.004), wall);
        }
    }

    for k in &table.kickers {
        let lit = k.flash > 0.0;
        match k.kind {
            KickerKind::Bumper { center, radius } => {
                let c = view.p(center);
                let fill = if lit { YELLOW } else { Color::from_rgba(200, 60, 80, 255) };
                draw_circle(c.x, c.y, radius * view.scale, fill);
                draw_circle_lines(c.x, c.y, radius * view.scale, px(0.003), WHITE);
            }
            KickerKind::Sling { a, b, .. } => {
                let (a, b) = (view.p(a), view.p(b));
                let col = if lit { YELLOW } else { Color::from_rgba(80, 200, 255, 255) };
                draw_line(a.x, a.y, b.x, b.y, px(0.006), col);
            }
        }
    }

    for (i, f) in table.flippers.iter().enumerate() {
        let pose = table.flipper_pose(i);
        let pts: Vec<Vec2> = f.outline.iter().map(|&p| view.p(pose * p)).collect();
        let center = pts.iter().copied().sum::<Vec2>() / pts.len() as f32;
        for j in 0..pts.len() {
            let (a, b) = (pts[j], pts[(j + 1) % pts.len()]);
            draw_triangle(center, a, b, ORANGE);
        }
    }

    // Plunger charge bar at the bottom of the lane.
    if table.plunger_charge > 0.0 {
        let base = view.p(PVec::new(LANE_X + 0.006, 0.025));
        let w = (TABLE_W - LANE_X - 0.012) * view.scale;
        let h = 0.15 * table.plunger_charge * view.scale;
        draw_rectangle(base.x, base.y, w, h, Color::from_rgba(255, 120, 0, 200));
    }

    let ball = view.p(table.ball_pos());
    draw_circle(ball.x, ball.y, BALL_R * view.scale, LIGHTGRAY);
    draw_circle(ball.x - 0.3 * BALL_R * view.scale, ball.y - 0.3 * BALL_R * view.scale,
        0.35 * BALL_R * view.scale, WHITE);

    let hud = format!("SCORE {:>7}    BALLS {}", table.score, table.balls_left);
    draw_text(&hud, 20.0, 34.0, 32.0, WHITE);
    if table.balls_left == 0 {
        let msg = "GAME OVER - press N";
        let size = measure_text(msg, None, 48, 1.0);
        draw_text(msg, (screen_width() - size.width) / 2.0, screen_height() / 2.0, 48.0, YELLOW);
    }
}
