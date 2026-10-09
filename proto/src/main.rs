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
use std::collections::BTreeMap;

use engine::physics::{PHYSICS_HZ, Table};
use engine::tablefile;
use rules::{Game, rules_by_name};
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

/// Loads a table and starts a game with the rules it names.
fn new_game(path: &str) -> Result<(Table, Game), String> {
    let layout = tablefile::load(path)?;
    let rules = rules_by_name(layout.rules.as_deref()).map_err(|e| format!("{path}: {e}"))?;
    let mut table = Table::new(layout);
    let game = Game::new(&mut table, rules);
    Ok((table, game))
}

async fn play(path: String) {
    let (mut table, mut game) = new_game(&path).unwrap_or_else(|e| fail(e));
    let mut accumulator = 0.0;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::N) && game.game_over() {
            match new_game(&path) {
                Ok(fresh) => (table, game) = fresh,
                Err(e) => log::error!("{e}"),
            }
        }
        if is_key_pressed(KeyCode::F5) {
            match new_game(&path) {
                Ok(fresh) => {
                    log::info!("reloaded {path}");
                    (table, game) = fresh;
                }
                Err(e) => log::error!("{e}"),
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
            game.tick(&mut table);
            game.take_outputs(); // already logged
        });

        let (status, message) = game.hud();
        render::draw(&table, &status, message.as_deref(), &game.lamps);
        next_frame().await;
    }
}

/// Plays a scenario's scripted inputs in the window, exactly as the tests run
/// it (with its rules, if it has any), then holds the last frame.
async fn replay(path: PathBuf) {
    let (scenario, layout) = Scenario::load(&path).unwrap_or_else(|e| fail(e));
    log::info!("scenario {}: {}", path.display(), scenario.description);
    let start = |layout| {
        let mut table = Table::new(layout);
        let game = scenario.start(&mut table);
        (table, game, Player::new(&scenario))
    };
    let (mut table, mut game, mut player) = start(layout);
    let mut accumulator = 0.0;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::R) {
            match Scenario::load(&path) {
                Ok((_, layout)) => (table, game, player) = start(layout),
                Err(e) => log::error!("{e}"),
            }
        }

        run_steps(&mut accumulator, || {
            if table.steps() < scenario.steps() {
                player.apply(&mut table);
                for ev in table.step() {
                    if let Some(game) = &mut game {
                        game.handle(&mut table, &ev.event);
                    }
                }
                if let Some(game) = &mut game {
                    game.tick(&mut table);
                    game.take_outputs();
                }
            }
        });

        let t = table.steps() as f32 / PHYSICS_HZ;
        let done = table.steps() >= scenario.steps();
        let (status, message) = match &game {
            Some(game) => {
                let (status, message) = game.hud();
                (format!("{t:5.2}s  {status}"), message)
            }
            None => (format!("SCENARIO  {t:5.2}s / {:.2}s", scenario.duration), None),
        };
        let message = if done { Some("DONE - R to replay".to_string()) } else { message };
        let no_lamps = BTreeMap::new();
        let lamps = game.as_ref().map_or(&no_lamps, |g| &g.lamps);
        render::draw(&table, &status, message.as_deref(), lamps);
        if !done {
            draw_text(&scenario.description, 16.0, screen_height() - 16.0, 20.0, GRAY);
        }
        next_frame().await;
    }
}
