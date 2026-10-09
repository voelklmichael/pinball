//! Pinball prototype: rapier2d for physics, macroquad for drawing/input.
//!
//! Controls: Left Shift / Z = left flippers, Right Shift / M = right flipper,
//! hold and release Space or Down = plunger, N = new game, Esc = quit.

mod game;
mod layout;
mod physics;
mod render;
#[cfg(test)]
mod svg;
#[cfg(test)]
mod tests;

use macroquad::prelude::*;

use game::Game;
use physics::{PHYSICS_HZ, Table};

fn window_conf() -> Conf {
    Conf {
        window_title: "Bavarian Pinball (prototype)".into(),
        window_width: 600,
        window_height: 1100,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut table = Table::new(layout::bavaria());
    let mut game = Game::new(&mut table);
    let step_dt = 1.0 / PHYSICS_HZ;
    let mut accumulator = 0.0;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::N) && game.game_over {
            game.start(&mut table);
        }

        let left = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::Z);
        let right = is_key_down(KeyCode::RightShift) || is_key_down(KeyCode::M);
        table.set_buttons(left, right);
        game.buttons(left, right);
        let frame_dt = get_frame_time().min(0.05);
        table.plunger(is_key_down(KeyCode::Space) || is_key_down(KeyCode::Down), frame_dt);

        // Fixed timestep, decoupled from the render frame rate.
        accumulator += frame_dt;
        while accumulator >= step_dt {
            for ev in table.step() {
                game.handle(&mut table, &ev);
            }
            game.tick(&mut table, step_dt);
            accumulator -= step_dt;
        }

        render::draw(&table, &game);
        next_frame().await;
    }
}
