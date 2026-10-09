//! Rules: score, balls, ball save, multiball, bonus, timers, lamps and
//! messages, on top of the engine's events.
//!
//! [`Game`] runs what every table shares; each table's own rules implement
//! [`TableRules`] and react to [`RuleEvent`]s, which name elements by their
//! id in the table file. A table picks its rules with `"rules": "<name>"`
//! (see [`rules_by_name`]).

pub mod demo;

use std::collections::BTreeMap;

use engine::physics::{Event, PHYSICS_HZ, Table, Thing};
use engine::rapier2d::math::Vector;

pub const BALLS_PER_GAME: u32 = 3;
/// Ball save for each new ball, from its launch.
const BALL_SAVE_SECS: f32 = 10.0;
/// How long a message stays on the display.
const MESSAGE_SECS: f32 = 2.0;

/// What kind of thing happened; see the engine's `Event`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Serve,
    Launch,
    Touch,
    Leave,
    Kick,
    Switch,
    Cross,
    Drop,
    Reset,
    Capture,
    Eject,
    Spin,
    FlipperUp,
    FlipperDown,
    Drain,
}

/// An engine event as the rules see it: the element by its id.
#[derive(Clone, Debug)]
pub struct RuleEvent {
    pub kind: Kind,
    /// Id of the element (`""` for events without one, e.g. a drain).
    pub id: String,
    pub ball: Option<u32>,
    /// For spinners.
    pub spins: u32,
}

impl RuleEvent {
    fn from_engine(table: &Table, event: &Event) -> Self {
        let named = |kind, thing: Thing, ball| RuleEvent { kind, id: table.name(thing), ball, spins: 0 };
        let plain = |kind, ball| RuleEvent { kind, id: String::new(), ball, spins: 0 };
        match *event {
            Event::Serve { ball, .. } => plain(Kind::Serve, Some(ball)),
            Event::Launch { ball, .. } => plain(Kind::Launch, Some(ball)),
            Event::Touch { ball, thing } => named(Kind::Touch, thing, Some(ball)),
            Event::Leave { ball, thing } => named(Kind::Leave, thing, Some(ball)),
            Event::Kick { ball, thing } => named(Kind::Kick, thing, Some(ball)),
            Event::Switch { ball, switch } => named(Kind::Switch, Thing::Switch(switch), Some(ball)),
            Event::Cross { ball, transition, .. } => named(Kind::Cross, Thing::Transition(transition), Some(ball)),
            Event::Drop { ball, target } => named(Kind::Drop, Thing::DropTarget(target), Some(ball)),
            Event::Reset { target } => named(Kind::Reset, Thing::DropTarget(target), None),
            Event::Capture { ball, hole } => named(Kind::Capture, Thing::Hole(hole), Some(ball)),
            Event::Eject { ball, hole } => named(Kind::Eject, Thing::Hole(hole), Some(ball)),
            Event::Spin { ball, spinner, spins } => {
                RuleEvent { spins, ..named(Kind::Spin, Thing::Spinner(spinner), Some(ball)) }
            }
            Event::FlipperUp { flipper } => named(Kind::FlipperUp, Thing::Flipper(flipper), None),
            Event::FlipperDown { flipper } => named(Kind::FlipperDown, Thing::Flipper(flipper), None),
            Event::Drain { ball } => plain(Kind::Drain, Some(ball)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lamp {
    #[default]
    Off,
    On,
    Blink,
}

/// What the rules show and play: for the display, the log and scenario traces.
#[derive(Clone, PartialEq, Debug)]
pub enum Output {
    Score { points: u64, total: u64 },
    Message(String),
    Lamp { name: String, state: Lamp },
    Sound(String),
}

impl Output {
    /// One line, e.g. `score +500 = 1500` or `lamp scoop on`.
    pub fn describe(&self) -> String {
        match self {
            Output::Score { points, total } => format!("score +{points} = {total}"),
            Output::Message(text) => format!("message {text}"),
            Output::Lamp { name, state } => format!("lamp {name} {}", format!("{state:?}").to_lowercase()),
            Output::Sound(name) => format!("sound {name}"),
        }
    }

    /// The line as scenario checks write it: without the numbers for scores.
    pub fn key(&self) -> String {
        match self {
            Output::Score { .. } => "score".into(),
            other => other.describe(),
        }
    }
}

/// The game state every table shares.
#[derive(Default)]
pub struct State {
    pub score: u64,
    pub ball_no: u32,
    pub game_over: bool,
    pub extra_balls: u32,
    /// End-of-ball bonus collected so far, and its multiplier.
    pub bonus: u64,
    pub bonus_x: u32,
    /// Step the ball save runs until.
    ball_save_until: u64,
    /// Ball save to start when the next ball is launched, in seconds.
    ball_save_pending: f32,
}

/// What the table's rules can see and do.
pub struct Ctx<'a> {
    pub table: &'a mut Table,
    pub state: &'a mut State,
    timers: &'a mut BTreeMap<String, u64>,
    lamps: &'a mut BTreeMap<String, Lamp>,
    outputs: &'a mut Vec<(u64, Output)>,
    display: &'a mut Option<(String, u64)>,
}

impl Ctx<'_> {
    fn now(&self) -> u64 {
        self.table.steps()
    }

    fn output(&mut self, out: Output) {
        log::info!("{:8.3}s {}", self.now() as f32 / PHYSICS_HZ, out.describe());
        self.outputs.push((self.now(), out));
    }

    pub fn score(&mut self, points: u64) {
        self.state.score += points;
        let total = self.state.score;
        self.output(Output::Score { points, total });
    }

    /// Adds to the end-of-ball bonus.
    pub fn add_bonus(&mut self, points: u64) {
        self.state.bonus += points;
    }

    pub fn set_bonus_multiplier(&mut self, x: u32) {
        self.state.bonus_x = x;
    }

    pub fn message(&mut self, text: impl Into<String>) {
        let text = text.into();
        *self.display = Some((text.clone(), self.now() + (MESSAGE_SECS * PHYSICS_HZ) as u64));
        self.output(Output::Message(text));
    }

    pub fn lamp(&mut self, name: &str, state: Lamp) {
        if self.lamp_state(name) != state {
            self.lamps.insert(name.to_owned(), state);
            self.output(Output::Lamp { name: name.to_owned(), state });
        }
    }

    pub fn lamp_state(&self, name: &str) -> Lamp {
        self.lamps.get(name).copied().unwrap_or_default()
    }

    pub fn sound(&mut self, name: &str) {
        self.output(Output::Sound(name.to_owned()));
    }

    /// Calls `on_timer(name)` after `secs`; restarting a running timer moves it.
    pub fn start_timer(&mut self, name: &str, secs: f32) {
        let due = self.now() + (secs * PHYSICS_HZ).round() as u64;
        self.timers.insert(name.to_owned(), due);
    }

    pub fn cancel_timer(&mut self, name: &str) {
        self.timers.remove(name);
    }

    pub fn timer_running(&self, name: &str) -> bool {
        self.timers.contains_key(name)
    }

    /// Lets the oldest ball in hole `id` go; false if it's empty or unknown.
    pub fn eject(&mut self, id: &str) -> bool {
        match self.table.find(id) {
            Some(Thing::Hole(h)) => self.table.eject(h),
            _ => false,
        }
    }

    pub fn held_in(&self, id: &str) -> usize {
        match self.table.find(id) {
            Some(Thing::Hole(h)) => self.table.held_in(h),
            _ => 0,
        }
    }

    pub fn reset_drop_targets(&mut self, ids: &[&str]) {
        for id in ids {
            if let Some(Thing::DropTarget(t)) = self.table.find(id) {
                self.table.reset_drop_target(t);
            }
        }
    }

    pub fn drop_target_down(&self, id: &str) -> bool {
        matches!(self.table.find(id), Some(Thing::DropTarget(t)) if self.table.drops_down[t])
    }

    /// Puts another ball at the launcher (after a lock, for multiball).
    pub fn serve_ball(&mut self) {
        self.table.serve_ball();
    }

    /// Starts (or extends) a ball save now, e.g. for multiball.
    pub fn ball_save(&mut self, secs: f32) {
        let until = self.now() + (secs * PHYSICS_HZ) as u64;
        self.state.ball_save_until = self.state.ball_save_until.max(until);
    }

    pub fn extra_ball(&mut self) {
        self.state.extra_balls += 1;
    }

    /// Switches element `id` on or off (pins, diverters, gates).
    pub fn set_enabled(&mut self, id: &str, on: bool) {
        if let Some(thing) = self.table.find(id) {
            self.table.set_enabled(thing, on);
        }
    }

    pub fn set_ball_vel(&mut self, ball: u32, vel: Vector) {
        self.table.set_ball_vel(ball, vel);
    }

    pub fn balls_in_play(&self) -> usize {
        self.table.balls_in_play()
    }
}

/// A table's own rules. Every method has a default that does nothing.
pub trait TableRules {
    fn on_game_start(&mut self, _ctx: &mut Ctx) {}
    fn on_ball_start(&mut self, _ctx: &mut Ctx) {}
    fn on_event(&mut self, _ctx: &mut Ctx, _event: &RuleEvent) {}
    fn on_timer(&mut self, _ctx: &mut Ctx, _name: &str) {}
    /// Before the end-of-ball bonus is added.
    fn on_ball_end(&mut self, _ctx: &mut Ctx) {}
}

/// Rules that only count balls: what a table without its own rules gets.
pub struct Basic;

impl TableRules for Basic {}

/// The rules a table names in its `rules` field (`None`: [`Basic`]).
pub fn rules_by_name(name: Option<&str>) -> Result<Box<dyn TableRules>, String> {
    match name {
        None | Some("basic") => Ok(Box::new(Basic)),
        Some("demo") => Ok(Box::new(demo::Demo::default())),
        Some(other) => Err(format!("no rules called \"{other}\" (known: basic, demo)")),
    }
}

/// Runs a game on a table: the shared parts (balls, ball save, multiball,
/// bonus, timers, display) plus the table's rules.
pub struct Game {
    rules: Box<dyn TableRules>,
    pub state: State,
    timers: BTreeMap<String, u64>,
    pub lamps: BTreeMap<String, Lamp>,
    outputs: Vec<(u64, Output)>,
    display: Option<(String, u64)>,
}

/// Lets the shared code and the table's rules use one context.
macro_rules! ctx {
    ($game:expr, $table:expr) => {
        Ctx {
            table: $table,
            state: &mut $game.state,
            timers: &mut $game.timers,
            lamps: &mut $game.lamps,
            outputs: &mut $game.outputs,
            display: &mut $game.display,
        }
    };
}

impl Game {
    /// Starts a game: serves the first ball.
    pub fn new(table: &mut Table, rules: Box<dyn TableRules>) -> Self {
        let mut game = Self {
            rules,
            state: State::default(),
            timers: BTreeMap::new(),
            lamps: BTreeMap::new(),
            outputs: Vec::new(),
            display: None,
        };
        game.start(table);
        game
    }

    pub fn start(&mut self, table: &mut Table) {
        log::info!("new game on \"{}\"", table.layout.name);
        self.state = State { bonus_x: 1, ..State::default() };
        self.timers.clear();
        self.lamps.clear();
        self.rules.on_game_start(&mut ctx!(self, table));
        self.next_ball(table);
    }

    fn next_ball(&mut self, table: &mut Table) {
        self.state.ball_no += 1;
        let ball = self.state.ball_no;
        ctx!(self, table).message(format!("BALL {ball}"));
        self.serve(table);
    }

    /// A new ball for the same player turn: fresh bonus and ball save.
    fn serve(&mut self, table: &mut Table) {
        self.state.bonus = 0;
        self.state.bonus_x = 1;
        self.state.ball_save_pending = BALL_SAVE_SECS;
        table.serve_ball();
        self.rules.on_ball_start(&mut ctx!(self, table));
    }

    pub fn game_over(&self) -> bool {
        self.state.game_over
    }

    /// Reacts to one engine event; call for every event `Table::step` returns.
    pub fn handle(&mut self, table: &mut Table, event: &Event) {
        if self.state.game_over {
            return;
        }
        let ev = RuleEvent::from_engine(table, event);
        if ev.kind == Kind::Launch && self.state.ball_save_pending > 0.0 {
            let secs = std::mem::take(&mut self.state.ball_save_pending);
            ctx!(self, table).ball_save(secs);
        }
        self.rules.on_event(&mut ctx!(self, table), &ev);
        if ev.kind == Kind::Drain {
            self.on_drain(table);
        }
    }

    fn on_drain(&mut self, table: &mut Table) {
        if table.balls_in_play() > 0 {
            return; // multiball: keep playing
        }
        let mut ctx = ctx!(self, table);
        if ctx.now() < ctx.state.ball_save_until {
            ctx.message("BALL SAVED");
            ctx.serve_ball();
            return;
        }
        self.rules.on_ball_end(&mut ctx!(self, table));
        let mut ctx = ctx!(self, table);
        let bonus = ctx.state.bonus * ctx.state.bonus_x as u64;
        if bonus > 0 {
            ctx.message(format!("BONUS {bonus}"));
            ctx.score(bonus);
        }
        if self.state.extra_balls > 0 {
            self.state.extra_balls -= 1;
            ctx!(self, table).message("SHOOT AGAIN");
            self.serve(table);
        } else if self.state.ball_no >= BALLS_PER_GAME {
            self.state.game_over = true;
            ctx!(self, table).message("GAME OVER");
        } else {
            self.next_ball(table);
        }
    }

    /// Runs timers that are due; call once per physics step.
    pub fn tick(&mut self, table: &mut Table) {
        let now = table.steps();
        let due: Vec<String> = self.timers.iter().filter(|&(_, &at)| at <= now).map(|(n, _)| n.clone()).collect();
        for name in due {
            self.timers.remove(&name);
            if !self.state.game_over {
                self.rules.on_timer(&mut ctx!(self, table), &name);
            }
        }
        if self.display.as_ref().is_some_and(|&(_, until)| until <= now) {
            self.display = None;
        }
    }

    /// What the rules showed and played since the last call, with their step.
    pub fn take_outputs(&mut self) -> Vec<(u64, Output)> {
        std::mem::take(&mut self.outputs)
    }

    /// Status line and centre message for the display.
    pub fn hud(&self) -> (String, Option<String>) {
        let s = &self.state;
        let status = format!("SCORE {:>9}   BALL {}   BONUS {}X", s.score, s.ball_no, s.bonus_x);
        let message = if s.game_over {
            Some("GAME OVER - press N".to_string())
        } else {
            self.display.as_ref().map(|(m, _)| m.clone())
        };
        (status, message)
    }
}
