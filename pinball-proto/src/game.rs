//! Minimal rules: scoring, B-I-E-R lanes, Kegelbahn reset, scoop, kickback,
//! ball save and ball count. Region modes and multiball come later (DESIGN.md).

use crate::layout::{Layer, Sw};
use crate::physics::{Event, Table};

const BALLS_PER_GAME: u32 = 3;
const BALL_SAVE_SECS: f32 = 10.0;
const SCOOP_HOLD_SECS: f32 = 1.0;
const DROP_RESET_SECS: f32 = 1.5;
const KICKBACK_SPEED: f32 = 2.6;

pub struct Game {
    pub score: u64,
    pub ball_no: u32,
    pub game_over: bool,
    /// B-I-E-R top lanes; all four raise the bonus multiplier.
    pub lanes: [bool; 4],
    pub bonus_x: u32,
    pub bonus: u64,
    pub kickback_lit: bool,
    /// Lit by clearing the Kegelbahn; collected at the Rathaus scoop.
    pub scoop_lit: bool,
    pub ball_save: f32,
    scoop_timer: Option<f32>,
    drop_reset_timer: Option<f32>,
    /// Latest message for the display, with seconds left.
    pub message: Option<(String, f32)>,
    prev_buttons: [bool; 2],
}

impl Game {
    pub fn new(table: &mut Table) -> Self {
        let mut g = Self {
            score: 0,
            ball_no: 0,
            game_over: true,
            lanes: [false; 4],
            bonus_x: 1,
            bonus: 0,
            kickback_lit: true,
            scoop_lit: false,
            ball_save: 0.0,
            scoop_timer: None,
            drop_reset_timer: None,
            message: None,
            prev_buttons: [false; 2],
        };
        g.start(table);
        g
    }

    pub fn start(&mut self, table: &mut Table) {
        self.score = 0;
        self.ball_no = 0;
        self.game_over = false;
        self.bonus_x = 1;
        self.kickback_lit = true;
        self.scoop_lit = false;
        table.reset_drops();
        self.next_ball(table);
    }

    fn next_ball(&mut self, table: &mut Table) {
        self.ball_no += 1;
        self.lanes = [false; 4];
        self.bonus = 0;
        self.ball_save = BALL_SAVE_SECS;
        table.serve_ball();
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), 2.0));
    }

    /// Lane change: the flipper buttons rotate which B-I-E-R lanes are lit.
    pub fn buttons(&mut self, left: bool, right: bool) {
        if left && !self.prev_buttons[0] {
            self.lanes.rotate_left(1);
        }
        if right && !self.prev_buttons[1] {
            self.lanes.rotate_right(1);
        }
        self.prev_buttons = [left, right];
    }

    pub fn handle(&mut self, table: &mut Table, ev: &Event) {
        if self.game_over {
            return;
        }
        let points: u64 = match ev.sw {
            Sw::Bumper(_) => 1_000,
            Sw::Sling(_) => 100,
            Sw::Rollover(_) => 500,
            Sw::Spinner => 100 * (ev.vel.length() * 10.0).round() as u64,
            Sw::Drop(_) => 2_500,
            Sw::RampMade(_) => 10_000,
            Sw::LeftOrbit | Sw::RightOrbit => 2_000,
            Sw::Castle => 25_000,
            _ => 0,
        };
        self.score += points * self.bonus_x as u64;

        match ev.sw {
            Sw::Rollover(i) => {
                self.lanes[i as usize] = true;
                if self.lanes.iter().all(|&l| l) {
                    self.lanes = [false; 4];
                    self.bonus_x = (self.bonus_x + 1).min(5);
                    self.say(format!("BIER! BONUS {}X", self.bonus_x));
                }
            }
            Sw::Drop(_) => {
                if table.drops_down.iter().all(|&d| d) && self.drop_reset_timer.is_none() {
                    self.drop_reset_timer = Some(DROP_RESET_SECS);
                    self.scoop_lit = true;
                    self.kickback_lit = true;
                    self.say("STRIKE! RATHAUS IS LIT");
                }
            }
            Sw::RampMade(layer) => {
                self.bonus += 5_000;
                match layer {
                    Layer::RampL => self.say("ZUGSPITZBAHN"),
                    Layer::RampC => self.say("BIERZELT"),
                    Layer::RampR => self.say("TO NEUSCHWANSTEIN"),
                    _ => {}
                }
            }
            Sw::Scoop => {
                self.scoop_timer = Some(SCOOP_HOLD_SECS);
                if self.scoop_lit {
                    self.scoop_lit = false;
                    self.score += 250_000;
                    self.say("RATHAUS: 250,000");
                }
            }
            Sw::Kickback if self.kickback_lit => {
                self.kickback_lit = false;
                table.set_ball_vel(ev.ball, rapier2d::math::Vector::new(0.0, KICKBACK_SPEED));
                self.say("KICKBACK");
            }
            Sw::Drain => self.on_drain(table),
            _ => {}
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
        self.score += self.bonus * self.bonus_x as u64;
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
        if let Some(t) = &mut self.scoop_timer {
            *t -= dt;
            if *t <= 0.0 {
                self.scoop_timer = None;
                table.eject(0);
            }
        }
        if let Some(t) = &mut self.drop_reset_timer {
            *t -= dt;
            if *t <= 0.0 {
                self.drop_reset_timer = None;
                table.reset_drops();
            }
        }
        if let Some((_, t)) = &mut self.message {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }
    }
}
