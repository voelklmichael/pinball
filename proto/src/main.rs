//! Pinball prototype: draws and plays tables with macroquad on top of
//! `engine`.
//!
//! Usage:
//!   proto [table.json]            play a table, by default tables/simple.json
//!   proto --scenario file.json    watch a scenario's scripted inputs
//!
//! Controls: Left Shift / Z = left flippers, Right Shift / M = right flippers,
//! Space or Down = launch, N = new game, F5 = reload the table file,
//! R = restart a scenario, Esc = quit.
//!
//! Logging goes to the terminal; set the level with RUST_LOG, e.g.
//! `RUST_LOG=debug` to see every contact.

mod render;

use std::path::PathBuf;

use macroquad::prelude::*;
use engine::game::Game;
use engine::physics::{PHYSICS_HZ, Table};
use engine::tablefile;
use scenario::{Player, Scenario};

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

/// Exits with the error logged; for problems at startup.
fn fail(e: String) -> ! {
    log::error!("{e}");
    std::process::exit(1);
}

#[macroquad::main(window_conf)]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, path] if flag == "--scenario" => replay(PathBuf::from(path)).await,
        [path] => play(path.clone()).await,
        [] => play(DEFAULT_TABLE.into()).await,
        _ => fail("usage: proto [table.json] | --scenario file.json".into()),
    }
}

/// Fixed timestep, decoupled from the render frame rate: calls `step` for
/// every physics step due this frame.
fn run_steps(accumulator: &mut f32, mut step: impl FnMut()) {
    let step_dt = 1.0 / PHYSICS_HZ;
    *accumulator += get_frame_time().min(0.05);
    while *accumulator >= step_dt {
        step();
        *accumulator -= step_dt;
    }
}

async fn play(path: String) {
    let layout = tablefile::load(&path).unwrap_or_else(|e| fail(e));
    let mut table = Table::new(layout);
    let mut game = Game::new(&mut table);
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
                    log::info!("reloaded {path}");
                    table = Table::new(layout);
                    game = Game::new(&mut table);
                }
                Err(e) => {
                    log::error!("{e}");
                    game.say("TABLE FILE ERROR, SEE TERMINAL");
                }
            }
        }

        let left = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::Z);
        let right = is_key_down(KeyCode::RightShift) || is_key_down(KeyCode::M);
        table.set_buttons(left, right);
        table.launch_key(is_key_down(KeyCode::Space) || is_key_down(KeyCode::Down));

        run_steps(&mut accumulator, || {
            for ev in table.step() {
                game.handle(&mut table, &ev.event);
            }
            game.tick(&mut table, 1.0 / PHYSICS_HZ);
        });

        let (status, message) = game.hud();
        render::draw(&table, &status, message.as_deref());
        next_frame().await;
    }
}

/// Plays a scenario's scripted inputs in the window, exactly as the tests run
/// it, then holds the last frame. No rules: just the physics.
async fn replay(path: PathBuf) {
    let (scenario, layout) = Scenario::load(&path).unwrap_or_else(|e| fail(e));
    log::info!("scenario {}: {}", path.display(), scenario.description);
    let start = |layout| {
        let mut table = Table::new(layout);
        scenario.start(&mut table);
        (table, Player::new(&scenario))
    };
    let (mut table, mut player) = start(layout);
    let mut accumulator = 0.0;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::R) {
            match Scenario::load(&path) {
                Ok((_, layout)) => (table, player) = start(layout),
                Err(e) => log::error!("{e}"),
            }
        }

        run_steps(&mut accumulator, || {
            if table.steps() < scenario.steps() {
                player.apply(&mut table);
                table.step();
            }
        });

        let t = table.steps() as f32 / PHYSICS_HZ;
        let status = format!("SCENARIO  {t:5.2}s / {:.2}s", scenario.duration);
        let done = table.steps() >= scenario.steps();
        let message = done.then_some("DONE - R to replay");
        render::draw(&table, &status, message);
        if !done {
            draw_text(&scenario.description, 16.0, screen_height() - 16.0, 20.0, GRAY);
        }
        next_frame().await;
    }
}
