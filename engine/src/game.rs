//! Minimal rules: ball count, ball save and game over. Scoring comes with
//! switches and the rules engine (DESIGN.md).

use crate::physics::{Event, Table};

const BALLS_PER_GAME: u32 = 3;
const BALL_SAVE_SECS: f32 = 10.0;

pub struct Game {
    pub score: u64,
    pub ball_no: u32,
    pub game_over: bool,
    pub ball_save: f32,
    /// Latest message for the display, with seconds left.
    pub message: Option<(String, f32)>,
}

impl Game {
    pub fn new(table: &mut Table) -> Self {
        let mut g = Self { score: 0, ball_no: 0, game_over: true, ball_save: 0.0, message: None };
        g.start(table);
        g
    }

    pub fn start(&mut self, table: &mut Table) {
        log::info!("new game on \"{}\"", table.layout.name);
        self.score = 0;
        self.ball_no = 0;
        self.game_over = false;
        self.next_ball(table);
    }

    fn next_ball(&mut self, table: &mut Table) {
        self.ball_no += 1;
        log::info!("ball {} of {BALLS_PER_GAME}", self.ball_no);
        self.ball_save = BALL_SAVE_SECS;
        table.serve_ball();
    }

    pub fn say(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::info!("display: {text}");
        self.message = Some((text, 2.0));
    }

    /// Status line and centre message for the display.
    pub fn hud(&self) -> (String, Option<String>) {
        let status = format!(
            "SCORE {:>9}   BALL {}{}",
            self.score,
            self.ball_no,
            if self.ball_save > 0.0 { "   SAVE" } else { "" }
        );
        let message = if self.game_over {
            Some("GAME OVER - press N".to_string())
        } else {
            self.message.as_ref().map(|(m, _)| m.clone())
        };
        (status, message)
    }

    pub fn handle(&mut self, table: &mut Table, ev: &Event) {
        if self.game_over {
            return;
        }
        if let Event::Drain { .. } = ev {
            self.on_drain(table);
        }
    }

    fn on_drain(&mut self, table: &mut Table) {
        if !table.balls.is_empty() {
            return; // multiball: keep playing
        }
        if self.ball_save > 0.0 {
            self.say("BALL SAVED");
            table.serve_ball();
            return;
        }
        if self.ball_no >= BALLS_PER_GAME {
            self.game_over = true;
            self.say("GAME OVER");
        } else {
            self.next_ball(table);
        }
    }

    pub fn tick(&mut self, table: &mut Table, dt: f32) {
        // Ball save only counts down once the ball has been launched.
        if !table.ball_waiting() {
            self.ball_save = (self.ball_save - dt).max(0.0);
        }
        if let Some((_, t)) = &mut self.message {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }
    }
}
