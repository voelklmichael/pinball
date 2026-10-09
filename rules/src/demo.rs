//! Demo rules, to try the rules machinery on `tables/demo.json` (and the
//! bench tables). They expect elements with these ids:
//!
//! - `bumper_*` (100 points per kick), `sling_*` (10)
//! - `spinner` (25 per spin)
//! - `lane_1` .. `lane_3`: rollover lanes; lighting all three raises the
//!   bonus multiplier. The flipper buttons move the lit lanes (lane change).
//! - `kegel_1` .. `kegel_3`: drop targets; all three down ("strike") lights
//!   the scoop, and they reset a second later.
//! - `scoop`: 1,000, or 25,000 when lit; the jackpot during multiball.
//! - `lock`: the first ball is locked; the second starts a 2-ball multiball.

use crate::{Ctx, Kind, Lamp, RuleEvent, TableRules};

const KEGEL: [&str; 3] = ["kegel_1", "kegel_2", "kegel_3"];
const LANES: [&str; 3] = ["lane_1", "lane_2", "lane_3"];
const MAX_BONUS_X: u32 = 5;
const JACKPOT: u64 = 100_000;

#[derive(Default)]
pub struct Demo {
    lanes: [bool; 3],
    multiball: bool,
}

impl Demo {
    fn show_lanes(&self, ctx: &mut Ctx) {
        for (lit, name) in self.lanes.iter().zip(LANES) {
            ctx.lamp(name, if *lit { Lamp::On } else { Lamp::Off });
        }
    }

    fn lane(&mut self, ctx: &mut Ctx, i: usize) {
        ctx.score(500);
        if self.lanes[i] {
            return;
        }
        self.lanes[i] = true;
        if self.lanes.iter().all(|&l| l) {
            self.lanes = [false; 3];
            let x = (ctx.state.bonus_x + 1).min(MAX_BONUS_X);
            ctx.set_bonus_multiplier(x);
            ctx.message(format!("BONUS {x}X"));
        }
        self.show_lanes(ctx);
    }

    fn scoop(&mut self, ctx: &mut Ctx) {
        if self.multiball && ctx.lamp_state("scoop") == Lamp::Blink {
            ctx.score(JACKPOT);
            ctx.message("JACKPOT");
            ctx.sound("jackpot");
        } else if ctx.lamp_state("scoop") == Lamp::On {
            ctx.score(25_000);
            ctx.message("SCOOP 25000");
            ctx.lamp("scoop", Lamp::Off);
        } else {
            ctx.score(1_000);
        }
    }

    fn lock(&mut self, ctx: &mut Ctx) {
        if self.multiball {
            // Locks don't hold balls during multiball.
            ctx.start_timer("release", 0.5);
            return;
        }
        if ctx.held_in("lock") < 2 {
            ctx.message("BALL LOCKED");
            ctx.serve_ball();
            return;
        }
        self.multiball = true;
        ctx.message("MULTIBALL");
        ctx.sound("multiball");
        ctx.ball_save(10.0);
        ctx.lamp("scoop", Lamp::Blink);
        // One ball at a time, so they don't land on top of each other.
        ctx.start_timer("release", 0.0);
    }
}

impl TableRules for Demo {
    fn on_ball_start(&mut self, ctx: &mut Ctx) {
        self.lanes = [false; 3];
        self.show_lanes(ctx);
    }

    fn on_event(&mut self, ctx: &mut Ctx, ev: &RuleEvent) {
        match (ev.kind, ev.id.as_str()) {
            (Kind::Kick, id) if id.starts_with("bumper") => ctx.score(100),
            (Kind::Kick, id) if id.starts_with("sling") => ctx.score(10),
            (Kind::Spin, "spinner") => ctx.score(25 * ev.spins as u64),
            (Kind::Switch, id) => {
                if let Some(i) = LANES.iter().position(|&l| l == id) {
                    self.lane(ctx, i);
                }
            }
            // Lane change: the left button moves the lit lanes left, the right one right.
            (Kind::FlipperUp, "left_flipper") => {
                self.lanes.rotate_left(1);
                self.show_lanes(ctx);
            }
            (Kind::FlipperUp, "right_flipper") => {
                self.lanes.rotate_right(1);
                self.show_lanes(ctx);
            }
            (Kind::Drop, id) if KEGEL.contains(&id) => {
                ctx.score(500);
                ctx.add_bonus(1_000);
                // Several can drop in the same step; the strike counts once.
                if KEGEL.iter().all(|k| ctx.drop_target_down(k)) && !ctx.timer_running("reset_kegel") {
                    ctx.message("STRIKE");
                    if !self.multiball {
                        ctx.lamp("scoop", Lamp::On);
                    }
                    ctx.start_timer("reset_kegel", 1.0);
                }
            }
            (Kind::Capture, "scoop") => self.scoop(ctx),
            (Kind::Capture, "lock") => self.lock(ctx),
            (Kind::Drain, _) if self.multiball && ctx.balls_in_play() <= 1 => {
                self.multiball = false;
                ctx.message("MULTIBALL OVER");
                ctx.lamp("scoop", Lamp::Off);
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, ctx: &mut Ctx, name: &str) {
        match name {
            "reset_kegel" => ctx.reset_drop_targets(&KEGEL),
            "release" => {
                ctx.eject("lock");
                if ctx.held_in("lock") > 0 {
                    ctx.start_timer("release", 0.5);
                }
            }
            _ => {}
        }
    }
}
