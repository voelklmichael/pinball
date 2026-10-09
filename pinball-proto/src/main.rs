//! Pinball prototype: rapier2d for physics, macroquad for drawing/input.
//!
//! Usage: `pinball-proto [table.json]`, by default `tables/simple.json`.
//!
//! Controls: Left Shift / Z = left flippers, Right Shift / M = right flippers,
//! Space or Down = launch, N = new game, F5 = reload the table file, Esc = quit.

mod game;
mod layout;
mod physics;
mod render;
mod tablefile;
#[cfg(test)]
mod svg;
#[cfg(test)]
mod tests;

use macroquad::prelude::*;

use game::Game;
use physics::{PHYSICS_HZ, Table};

const DEFAULT_TABLE: &str = "tables/simple.json";

fn window_conf() -> Conf {
    Conf {
        window_title: "Pinball (prototype)".into(),
        window_width: 600,
        window_height: 1100,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_TABLE.into());
    let layout = tablefile::load(&path).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let mut table = Table::new(layout);
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
        if is_key_pressed(KeyCode::F5) {
            match tablefile::load(&path) {
                Ok(layout) => {
                    table = Table::new(layout);
                    game = Game::new(&mut table);
                }
                Err(e) => {
                    eprintln!("{e}");
                    game.say("TABLE FILE ERROR, SEE TERMINAL");
                }
            }
        }

        let left = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::Z);
        let right = is_key_down(KeyCode::RightShift) || is_key_down(KeyCode::M);
        table.set_buttons(left, right);
        table.launch_key(is_key_down(KeyCode::Space) || is_key_down(KeyCode::Down));
        let frame_dt = get_frame_time().min(0.05);

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
