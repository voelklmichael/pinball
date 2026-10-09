//! Scenarios: a table, scripted key presses and what must happen. They run
//! headless as tests (`tests/scenarios.rs`, format in `scenarios/README.md`)
//! and can be replayed in the game with `--scenario`.

use std::path::Path;

use engine::layout::Layout;
use engine::physics::{Event, PHYSICS_HZ, Table, Thing, Timed};
use engine::rapier2d::math::Vector;
use serde::Deserialize;

type P = [f32; 2];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// What this scenario shows, in one sentence.
    pub description: String,
    /// Table file, relative to the scenario file.
    pub table: String,
    /// Seconds to run.
    pub duration: f32,
    /// Serve a ball at the launcher at the start.
    #[serde(default = "yes")]
    pub serve: bool,
    /// Balls placed directly at the start.
    #[serde(default)]
    pub balls: Vec<BallStart>,
    #[serde(default)]
    pub inputs: Vec<Input>,
    #[serde(default)]
    pub expect: Vec<Expect>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BallStart {
    pub position: P,
    #[serde(default)]
    pub velocity: P,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    Launch,
    Left,
    Right,
}

/// `{"at": 0.2, "press": "launch"}`, `{"at": 1.0, "release": "left"}`, or an
/// action the rules would take: `{"at": 1.0, "reset": "drop_1"}` raises a drop
/// target, `{"at": 1.0, "eject": "lock"}` lets a hole's oldest ball go.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub at: f32,
    pub press: Option<Key>,
    pub release: Option<Key>,
    pub reset: Option<String>,
    pub eject: Option<String>,
}

#[derive(Clone)]
enum Action {
    Key(Key, bool),
    Reset(String),
    Eject(String),
}

/// One check. Exactly one of `event`, `never`, `sequence`, `enters`, `at_end`.
/// Events are written like the trace without ball numbers and times, e.g.
/// `"touch outer_wall"`, `"flipper_up left_flipper"`, `"drain"`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// Happens at least once (within `between`, if given).
    pub event: Option<String>,
    /// Never happens.
    pub never: Option<String>,
    /// These happen in this order (other events may come in between).
    pub sequence: Option<Vec<String>>,
    /// A ball's centre gets into this rectangle (within `between`, if given).
    pub enters: Option<Region>,
    /// A ball is still on the table at the end, as described.
    pub at_end: Option<AtEnd>,
    /// Time window in seconds for `event` and `enters`.
    pub between: Option<[f32; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub min: P,
    pub max: P,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtEnd {
    pub near: P,
    /// Distance from `near`, in metres.
    pub within: f32,
    /// m/s
    #[serde(default = "f32_max")]
    pub max_speed: f32,
    /// The layer the ball must be on.
    pub layer: Option<String>,
}

fn f32_max() -> f32 {
    f32::MAX
}

fn v(p: P) -> Vector {
    Vector::new(p[0], p[1])
}

fn step_of(seconds: f32) -> u64 {
    (seconds * PHYSICS_HZ).round() as u64
}

/// Plays a scenario's inputs into a table, step by step.
pub struct Player {
    /// (step, action), in time order.
    inputs: Vec<(u64, Action)>,
    next: usize,
    keys: [bool; 3],
}

impl Player {
    pub fn new(scenario: &Scenario) -> Self {
        let mut inputs: Vec<_> = scenario
            .inputs
            .iter()
            .map(|i| {
                let action = match (i.press, i.release, &i.reset, &i.eject) {
                    (Some(k), ..) => Action::Key(k, true),
                    (_, Some(k), ..) => Action::Key(k, false),
                    (_, _, Some(id), _) => Action::Reset(id.clone()),
                    (_, _, _, Some(id)) => Action::Eject(id.clone()),
                    _ => unreachable!("checked in Scenario::load"),
                };
                (step_of(i.at), action)
            })
            .collect();
        inputs.sort_by_key(|i| i.0);
        Self { inputs, next: 0, keys: [false; 3] }
    }

    /// Applies the inputs due at the table's current step; call before each step.
    pub fn apply(&mut self, table: &mut Table) {
        while let Some((step, action)) = self.inputs.get(self.next).cloned() {
            if step > table.steps() {
                break;
            }
            match action {
                Action::Key(key, pressed) => self.keys[key as usize] = pressed,
                Action::Reset(id) => {
                    if let Some(Thing::DropTarget(t)) = table.find(&id) {
                        table.reset_drop_target(t);
                    }
                }
                Action::Eject(id) => {
                    if let Some(Thing::Hole(h)) = table.find(&id) {
                        table.eject(h);
                    }
                }
            }
            self.next += 1;
        }
        let [launch, left, right] = self.keys;
        table.set_buttons(left, right);
        table.launch_key(launch);
    }
}

pub struct Recording {
    /// Every event with its trace line and its key (the line without ball
    /// numbers, as used in `expect`).
    pub events: Vec<(Timed, String, String)>,
    /// Ball positions after every step: (step, ball number, position).
    pub positions: Vec<(u64, u32, Vector)>,
    /// Balls on the table at the end: (number, position, velocity, where:
    /// layer name, plus the hole if it's held).
    pub end: Vec<(u32, Vector, Vector, String)>,
}

impl Recording {
    /// Each ball's path, for drawing.
    pub fn paths(&self) -> Vec<Vec<Vector>> {
        let mut paths: std::collections::BTreeMap<u32, Vec<Vector>> = Default::default();
        for &(_, n, p) in &self.positions {
            paths.entry(n).or_default().push(p);
        }
        paths.into_values().collect()
    }

    /// The trace compared by the snapshot tests: one line per event, then the
    /// balls left at the end. Times in ms, positions in mm.
    pub fn trace(&self) -> String {
        let mut out = String::new();
        for (e, line, _) in &self.events {
            out += &format!("{:7.3}  {line}\n", e.time());
        }
        for (n, p, vel, layer) in &self.end {
            out += &format!("    end  ball#{n} at ({:.3}, {:.3}) v=({:.2}, {:.2}) on {layer}\n", p.x, p.y, vel.x, vel.y);
        }
        out
    }
}

/// The event as `expect` writes it: the trace line without the ball number.
fn key(table: &Table, event: &Event) -> String {
    match *event {
        Event::Serve { .. } => "serve".into(),
        Event::Launch { .. } => "launch".into(),
        Event::Touch { thing, .. } => format!("touch {}", table.name(thing)),
        Event::Leave { thing, .. } => format!("leave {}", table.name(thing)),
        Event::Kick { thing, .. } => format!("kick {}", table.name(thing)),
        Event::Switch { switch, .. } => format!("switch {}", table.name(Thing::Switch(switch))),
        Event::Cross { transition, .. } => format!("cross {}", table.name(Thing::Transition(transition))),
        Event::Drop { target, .. } => format!("drop {}", table.name(Thing::DropTarget(target))),
        Event::Reset { target } => format!("reset {}", table.name(Thing::DropTarget(target))),
        Event::Capture { hole, .. } => format!("capture {}", table.name(Thing::Hole(hole))),
        Event::Eject { hole, .. } => format!("eject {}", table.name(Thing::Hole(hole))),
        Event::Spin { spinner, .. } => format!("spin {}", table.name(Thing::Spinner(spinner))),
        Event::FlipperUp { .. } | Event::FlipperDown { .. } => table.describe(event),
        Event::Drain { .. } => "drain".into(),
    }
}

impl Scenario {
    /// Reads a scenario and the table it names.
    pub fn load(path: &Path) -> Result<(Scenario, Layout), String> {
        let ctx = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| ctx(e.to_string()))?;
        let scenario: Scenario = serde_json::from_str(&text).map_err(|e| ctx(e.to_string()))?;
        if scenario.duration <= 0.0 {
            return Err(ctx("duration must be > 0".into()));
        }
        for (i, input) in scenario.inputs.iter().enumerate() {
            let given = [input.press.is_some(), input.release.is_some(), input.reset.is_some(), input.eject.is_some()];
            if given.iter().filter(|&&g| g).count() != 1 {
                return Err(ctx(format!("inputs[{i}]: needs exactly one of press/release/reset/eject")));
            }
        }
        for (i, e) in scenario.expect.iter().enumerate() {
            let kinds = [e.event.is_some(), e.never.is_some(), e.sequence.is_some(), e.enters.is_some(), e.at_end.is_some()];
            if kinds.iter().filter(|&&k| k).count() != 1 {
                return Err(ctx(format!("expect[{i}]: needs exactly one of event/never/sequence/enters/at_end")));
            }
        }
        let table_path = path.parent().unwrap_or(Path::new(".")).join(&scenario.table);
        let layout = engine::tablefile::load(&table_path.to_string_lossy()).map_err(ctx)?;
        let drop_ids: Vec<_> = layout.drop_targets.iter().map(|d| d.id.as_deref()).collect();
        let hole_ids: Vec<_> = layout.holes.iter().map(|h| h.id.as_deref()).collect();
        for (i, input) in scenario.inputs.iter().enumerate() {
            if let Some(id) = &input.reset
                && !drop_ids.contains(&Some(id.as_str()))
            {
                return Err(ctx(format!("inputs[{i}]: no drop target \"{id}\"")));
            }
            if let Some(id) = &input.eject
                && !hole_ids.contains(&Some(id.as_str()))
            {
                return Err(ctx(format!("inputs[{i}]: no hole \"{id}\"")));
            }
        }
        Ok((scenario, layout))
    }

    /// Puts the starting balls on the table.
    pub fn start(&self, table: &mut Table) {
        if self.serve {
            table.serve_ball();
        }
        for b in &self.balls {
            table.add_ball(v(b.position), v(b.velocity));
        }
    }

    pub fn steps(&self) -> u64 {
        step_of(self.duration)
    }

    /// Runs the scenario without a window.
    pub fn run(&self, layout: Layout) -> Recording {
        let mut table = Table::new(layout);
        let mut player = Player::new(self);
        let mut events = Vec::new();
        let mut positions = Vec::new();
        self.start(&mut table);
        for _ in 0..self.steps() {
            player.apply(&mut table);
            for e in table.step() {
                events.push((e, table.describe(&e.event), key(&table, &e.event)));
            }
            for b in &table.balls {
                positions.push((table.steps(), b.number, table.ball_pos(b)));
            }
        }
        let end = table
            .balls
            .iter()
            .map(|b| {
                let mut place = table.layout.layer_name(b.layer).to_owned();
                if let Some(h) = b.held {
                    place += &format!(", held in {}", table.name(Thing::Hole(h)));
                }
                (b.number, table.ball_pos(b), table.ball_vel(b), place)
            })
            .collect();
        Recording { events, positions, end }
    }

    /// Everything in `expect` that didn't hold, one line each.
    pub fn check(&self, rec: &Recording) -> Vec<String> {
        let mut failures = Vec::new();
        for (i, e) in self.expect.iter().enumerate() {
            let window = e.between.unwrap_or([0.0, f32::MAX]);
            let in_window = |step: u64| (step_of(window[0])..=step_of(window[1])).contains(&step);
            let failure = if let Some(want) = &e.event {
                let hit = rec.events.iter().any(|(t, _, k)| k == want && in_window(t.step));
                (!hit).then(|| format!("\"{want}\" did not happen between {}s and {}s", window[0], window[1]))
            } else if let Some(want) = &e.never {
                rec.events
                    .iter()
                    .find(|(_, _, k)| k == want)
                    .map(|(t, _, _)| format!("\"{want}\" happened at {:.3}s", t.time()))
            } else if let Some(seq) = &e.sequence {
                let mut wanted = seq.iter().peekable();
                for (_, _, k) in &rec.events {
                    if wanted.peek() == Some(&k) {
                        wanted.next();
                    }
                }
                wanted.next().map(|missing| format!("sequence stopped before \"{missing}\""))
            } else if let Some(r) = &e.enters {
                let inside = |p: Vector| p.x >= r.min[0] && p.y >= r.min[1] && p.x <= r.max[0] && p.y <= r.max[1];
                let hit = rec.positions.iter().any(|&(step, _, p)| in_window(step) && inside(p));
                (!hit).then(|| format!("no ball entered {:?}..{:?} between {}s and {}s", r.min, r.max, window[0], window[1]))
            } else if let Some(end) = &e.at_end {
                let ok = rec.end.iter().any(|(_, p, vel, layer)| {
                    p.distance(v(end.near)) <= end.within
                        && vel.length() <= end.max_speed
                        && end.layer.as_ref().is_none_or(|l| layer.split(',').next() == Some(l.as_str()))
                });
                let balls: Vec<String> = rec
                    .end
                    .iter()
                    .map(|(n, p, vel, layer)| {
                        format!("ball#{n} at ({:.3}, {:.3}) speed {:.3} on {layer}", p.x, p.y, vel.length())
                    })
                    .collect();
                (!ok).then(|| {
                    format!(
                        "no ball within {} of {:?} slower than {}{} at the end; balls: [{}]",
                        end.within,
                        end.near,
                        end.max_speed,
                        end.layer.as_ref().map(|l| format!(" on {l}")).unwrap_or_default(),
                        balls.join(", ")
                    )
                })
            } else {
                unreachable!("checked in Scenario::load")
            };
            if let Some(f) = failure {
                failures.push(format!("expect[{i}]: {f}"));
            }
        }
        failures
    }
}
