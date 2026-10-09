//! Rapier world built from a [`Layout`]: walls, flippers and balls on layers.
//! Reports what happens as [`Timed`] [`Event`]s, which are also logged;
//! scoring and rules live in `game`.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Mutex;

use rapier2d::prelude::*;

use crate::layout::{Button, FlipperDef, Layer, Layout, SensorShape};

pub const PHYSICS_HZ: f32 = 480.0;
const DRAIN_Y: f32 = -0.03;
/// A contact only counts as ended after this many steps (10 ms) without
/// touching, so a ball rolling along a wall doesn't flicker touch/leave.
const LEAVE_DEBOUNCE_STEPS: u64 = 5;

/// Something a ball can touch: an index into the layout's walls, flippers,
/// switches or transitions, or another ball's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Thing {
    Wall(usize),
    Flipper(usize),
    Ball(u32),
    Switch(usize),
    Transition(usize),
    DropTarget(usize),
    Hole(usize),
    Spinner(usize),
}

/// Balls are numbered from 1 in the order they enter play.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Serve { ball: u32, pos: Vector },
    Launch { ball: u32, vel: Vector },
    Touch { ball: u32, thing: Thing },
    Leave { ball: u32, thing: Thing },
    /// A kicking wall (pop bumper, slingshot) pushed the ball away.
    Kick { ball: u32, thing: Thing },
    /// The ball rolled over a switch.
    Switch { ball: u32, switch: usize },
    /// The ball crossed a transition onto another layer (onto or off a ramp,
    /// a level, ...).
    Cross { ball: u32, transition: usize, from: Layer, to: Layer },
    /// A drop target was hit hard enough and dropped.
    Drop { ball: u32, target: usize },
    /// A drop target came back up.
    Reset { target: usize },
    /// A hole (scoop, lock) caught the ball.
    Capture { ball: u32, hole: usize },
    /// A hole let the ball go.
    Eject { ball: u32, hole: usize },
    /// The ball passed a spinner and set it spinning.
    Spin { ball: u32, spinner: usize, spins: u32 },
    FlipperUp { flipper: usize },
    FlipperDown { flipper: usize },
    /// The ball left the bottom of the table.
    Drain { ball: u32 },
}

/// An event and the physics step it happened at (step 0 is before the first step).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed {
    pub step: u64,
    pub event: Event,
}

impl Timed {
    pub fn time(&self) -> f32 {
        self.step as f32 / PHYSICS_HZ
    }
}

pub struct Ball {
    pub number: u32,
    pub body: RigidBodyHandle,
    collider: ColliderHandle,
    pub layer: Layer,
    /// Height above the playfield, for drawing.
    pub height: f32,
    /// Frozen at the launch position until the launch key fires it.
    pub waiting: bool,
    /// Held in this hole.
    pub held: Option<usize>,
}

impl Ball {
    /// Moving under physics: not waiting to be launched and not held in a hole.
    fn in_play(&self) -> bool {
        !self.waiting && self.held.is_none()
    }
}

pub struct Flipper {
    pub def: FlipperDef,
    body: RigidBodyHandle,
    /// Local-space outline, for drawing.
    pub outline: Vec<Vector>,
    angle: f32,
}

fn groups(memberships: u32, filter: u32) -> InteractionGroups {
    InteractionGroups::new(
        Group::from_bits_truncate(memberships),
        Group::from_bits_truncate(filter),
        InteractionTestMode::And,
    )
}

fn layer_groups(layer: Layer) -> InteractionGroups {
    groups(layer.bit(), u32::MAX)
}

/// Lets balls through one-way walls when they move in the wall's direction.
struct GateHooks {
    gates: HashMap<ColliderHandle, Vector>,
}

impl PhysicsHooks for GateHooks {
    fn modify_solver_contacts(&self, ctx: &mut ContactModificationContext) {
        let (pass, ball) = match (self.gates.get(&ctx.collider1), self.gates.get(&ctx.collider2)) {
            (Some(&pass), None) => (pass, ctx.rigid_body2),
            (None, Some(&pass)) => (pass, ctx.rigid_body1),
            _ => return,
        };
        let Some(ball) = ball else { return };
        if ctx.bodies[ball].linvel().dot(pass) > 0.0
            && let Some(m) = ctx.rigid_mut()
        {
            m.solver_contacts.clear();
        }
    }
}

#[derive(Default)]
struct Collector(Mutex<Vec<CollisionEvent>>);

impl EventHandler for Collector {
    fn handle_collision_event(&self, _: &RigidBodySet, _: &ColliderSet, event: CollisionEvent, _: Option<&ContactPair>) {
        self.0.lock().unwrap().push(event);
    }

    fn handle_contact_force_event(&self, _: Real, _: &RigidBodySet, _: &ColliderSet, _: &ContactPair, _: Real) {}

    fn handle_soft_body_tear_event(&self, _: &SoftBodySet, _: &SoftBodyTearEvent) {}
}

pub struct Table {
    world: PhysicsWorld,
    hooks: GateHooks,
    collector: Collector,
    pub layout: Layout,
    pub balls: Vec<Ball>,
    pub flippers: Vec<Flipper>,
    /// What each non-ball collider is.
    things: HashMap<ColliderHandle, Thing>,
    steps: u64,
    next_ball: u32,
    /// Events raised between steps (serve, launch, flipper), returned by the next step.
    pending: Vec<Timed>,
    /// Contacts that stopped, with the step they stopped at, not yet reported.
    leaving: BTreeMap<(u32, Thing), u64>,
    touching: BTreeSet<(u32, Thing)>,
    drop_colliders: Vec<ColliderHandle>,
    /// Which drop targets are down.
    pub drops_down: Vec<bool>,
    /// Balls each hole holds, oldest first, with the step they were caught at.
    held: Vec<VecDeque<(u32, u64)>>,
    /// Balls just ejected from a hole, ignored by it until they've left it.
    ejected: BTreeSet<(u32, usize)>,
    launch_key: bool,
    buttons: [bool; 2],
}

impl Table {
    pub fn new(layout: Layout) -> Self {
        let mut world = PhysicsWorld {
            // Only the component along the tilted playfield acts in 2D.
            gravity: Vector::new(0.0, -layout.gravity * layout.angle.sin()),
            ..Default::default()
        };
        world.integration_parameters.dt = 1.0 / PHYSICS_HZ;
        // Rapier's tolerances are tuned for metre-sized objects; ours are small.
        world.integration_parameters.length_unit = 0.05;

        let mut things = HashMap::new();
        let mut hooks = GateHooks { gates: HashMap::new() };
        let ground = world.insert_body(RigidBodyBuilder::fixed());
        for (i, w) in layout.walls.iter().enumerate() {
            let mut pts = w.pts.clone();
            if w.closed {
                pts.push(pts[0]);
            }
            let mut builder = ColliderBuilder::polyline(pts, None)
                .restitution(w.restitution)
                .friction(w.friction)
                .collision_groups(layer_groups(w.layer));
            if w.one_way.is_some() {
                builder = builder.active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS);
            }
            let h = world.insert_collider(builder, Some(ground));
            things.insert(h, Thing::Wall(i));
            if let Some(pass) = w.one_way {
                hooks.gates.insert(h, pass);
            }
        }

        // Switches and transitions are sensors: they report the ball but don't block it.
        let sensor = |shape: SensorShape| match shape {
            SensorShape::Circle { center, radius } => ColliderBuilder::ball(radius).translation(center),
            SensorShape::Rect { min, max } => {
                let half = (max - min) / 2.0;
                ColliderBuilder::cuboid(half.x, half.y).translation(min + half)
            }
            SensorShape::Segment { a, b } => ColliderBuilder::segment(a, b),
        };
        for (i, sw) in layout.switches.iter().enumerate() {
            let h = world.insert_collider(
                sensor(sw.shape).sensor(true).collision_groups(layer_groups(sw.layer)),
                Some(ground),
            );
            things.insert(h, Thing::Switch(i));
        }
        let mut drop_colliders = Vec::new();
        for (i, d) in layout.drop_targets.iter().enumerate() {
            let h = world.insert_collider(
                ColliderBuilder::segment(d.a, d.b).restitution(d.restitution).collision_groups(layer_groups(d.layer)),
                Some(ground),
            );
            things.insert(h, Thing::DropTarget(i));
            drop_colliders.push(h);
        }
        for (i, hole) in layout.holes.iter().enumerate() {
            let shape = SensorShape::Circle { center: hole.center, radius: hole.radius };
            let h = world
                .insert_collider(sensor(shape).sensor(true).collision_groups(layer_groups(hole.layer)), Some(ground));
            things.insert(h, Thing::Hole(i));
        }
        for (i, sp) in layout.spinners.iter().enumerate() {
            let shape = SensorShape::Segment { a: sp.a, b: sp.b };
            let h = world
                .insert_collider(sensor(shape).sensor(true).collision_groups(layer_groups(sp.layer)), Some(ground));
            things.insert(h, Thing::Spinner(i));
        }
        for (i, t) in layout.transitions.iter().enumerate() {
            // Two-way lines also see balls on the target layer, so they can bring them back.
            let layers = if t.two_way { t.layer.bit() | t.target.bit() } else { t.layer.bit() };
            let h = world.insert_collider(
                sensor(SensorShape::Segment { a: t.a, b: t.b }).sensor(true).collision_groups(groups(layers, u32::MAX)),
                Some(ground),
            );
            things.insert(h, Thing::Transition(i));
        }

        let mut flippers = Vec::new();
        for (i, def) in layout.flippers.iter().enumerate() {
            let (flipper, collider) = make_flipper(&mut world, def);
            things.insert(collider, Thing::Flipper(i));
            flippers.push(flipper);
        }

        let layer_names: Vec<&str> = layout.layers.iter().map(|l| l.name.as_str()).collect();
        log::info!(
            "table \"{}\": layers [{}], {} walls, {} flippers, {} switches, {} transitions",
            layout.name,
            layer_names.join(", "),
            layout.walls.len(),
            layout.flippers.len(),
            layout.switches.len(),
            layout.transitions.len()
        );
        let drops_down = vec![false; layout.drop_targets.len()];
        let held = layout.holes.iter().map(|_| VecDeque::new()).collect();
        Self {
            world,
            hooks,
            collector: Collector::default(),
            layout,
            balls: Vec::new(),
            flippers,
            things,
            steps: 0,
            next_ball: 1,
            pending: Vec::new(),
            leaving: BTreeMap::new(),
            touching: BTreeSet::new(),
            drop_colliders,
            drops_down,
            held,
            ejected: BTreeSet::new(),
            launch_key: false,
            buttons: [false; 2],
        }
    }

    /// Physics steps done so far.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    pub fn ball_pos(&self, ball: &Ball) -> Vector {
        self.world.bodies[ball.body].translation()
    }

    pub fn ball_vel(&self, ball: &Ball) -> Vector {
        self.world.bodies[ball.body].linvel()
    }

    pub fn flipper_pose(&self, f: &Flipper) -> Pose {
        *self.world.bodies[f.body].position()
    }

    /// Flipper buttons. Raises FlipperUp/FlipperDown for each flipper on a
    /// button that changed.
    pub fn set_buttons(&mut self, left: bool, right: bool) {
        let new = [left, right];
        for (i, f) in self.flippers.iter().enumerate() {
            let b = (f.def.button == Button::Right) as usize;
            if new[b] != self.buttons[b] {
                let event = if new[b] { Event::FlipperUp { flipper: i } } else { Event::FlipperDown { flipper: i } };
                self.pending.push(Timed { step: self.steps, event });
            }
        }
        self.buttons = new;
    }

    /// Adds a ball at `pos` on the playfield layer; returns its number.
    pub fn add_ball(&mut self, pos: Vector, vel: Vector) -> u32 {
        let ball = &self.layout.ball;
        let (body, collider) = self.world.insert(
            RigidBodyBuilder::dynamic()
                .translation(pos)
                .linvel(vel)
                .ccd_enabled(true)
                .can_sleep(false),
            // Multiply: a surface's restitution is scaled by the ball's, and the
            // surface's friction is used as is.
            ColliderBuilder::ball(ball.radius)
                .mass(ball.mass)
                .restitution(ball.restitution)
                .restitution_combine_rule(CoefficientCombineRule::Multiply)
                .friction(1.0)
                .friction_combine_rule(CoefficientCombineRule::Multiply)
                .active_events(ActiveEvents::COLLISION_EVENTS),
        );
        let number = self.next_ball;
        self.next_ball += 1;
        let mut ball = Ball { number, body, collider, layer: Layer::PLAYFIELD, height: 0.0, waiting: false, held: None };
        set_layer(&mut self.world, &mut ball, Layer::PLAYFIELD);
        self.balls.push(ball);
        self.pending.push(Timed { step: self.steps, event: Event::Serve { ball: number, pos } });
        number
    }

    /// Puts a new ball at the launch position, frozen until it's launched.
    pub fn serve_ball(&mut self) -> u32 {
        let number = self.add_ball(self.layout.launcher.position, Vector::ZERO);
        let ball = self.balls.last_mut().unwrap();
        ball.waiting = true;
        self.world.bodies[ball.body].set_enabled(false);
        number
    }

    pub fn ball_waiting(&self) -> bool {
        self.balls.iter().any(|b| b.waiting)
    }

    /// Launch key: fires waiting balls at the launcher's start velocity when pressed.
    pub fn launch_key(&mut self, held: bool) {
        let pressed = held && !self.launch_key;
        self.launch_key = held;
        if !pressed {
            return;
        }
        let vel = self.layout.launcher.start_velocity;
        for b in self.balls.iter_mut().filter(|b| b.waiting) {
            b.waiting = false;
            let body = &mut self.world.bodies[b.body];
            body.set_enabled(true);
            body.set_linvel(vel, true);
            self.pending.push(Timed { step: self.steps, event: Event::Launch { ball: b.number, vel } });
        }
    }

    /// The element with this id in the table file.
    pub fn find(&self, id: &str) -> Option<Thing> {
        fn pos<'a>(mut ids: impl Iterator<Item = &'a Option<String>>, id: &str) -> Option<usize> {
            ids.position(|i| i.as_deref() == Some(id))
        }
        let l = &self.layout;
        pos(l.walls.iter().map(|w| &w.id), id)
            .map(Thing::Wall)
            .or_else(|| pos(l.flippers.iter().map(|f| &f.id), id).map(Thing::Flipper))
            .or_else(|| pos(l.switches.iter().map(|s| &s.id), id).map(Thing::Switch))
            .or_else(|| pos(l.transitions.iter().map(|t| &t.id), id).map(Thing::Transition))
            .or_else(|| pos(l.drop_targets.iter().map(|d| &d.id), id).map(Thing::DropTarget))
            .or_else(|| pos(l.holes.iter().map(|h| &h.id), id).map(Thing::Hole))
            .or_else(|| pos(l.spinners.iter().map(|s| &s.id), id).map(Thing::Spinner))
    }

    /// Raises a drop target again (a ball in the way gets pushed aside).
    pub fn reset_drop_target(&mut self, target: usize) {
        if std::mem::replace(&mut self.drops_down[target], false) {
            self.world.colliders[self.drop_colliders[target]].set_enabled(true);
            self.pending.push(Timed { step: self.steps, event: Event::Reset { target } });
        }
    }

    /// Lets the oldest ball in a hole go; false if it was empty.
    pub fn eject(&mut self, hole: usize) -> bool {
        let Some((number, _)) = self.held[hole].pop_front() else { return false };
        let (center, vel) = (self.layout.holes[hole].center, self.layout.holes[hole].eject);
        let b = self.balls.iter_mut().find(|b| b.number == number).unwrap();
        b.held = None;
        let body = &mut self.world.bodies[b.body];
        body.set_enabled(true);
        body.set_translation(center, true);
        body.set_linvel(vel, true);
        self.ejected.insert((number, hole));
        self.pending.push(Timed { step: self.steps, event: Event::Eject { ball: number, hole } });
        true
    }

    /// How many balls a hole holds.
    pub fn held_in(&self, hole: usize) -> usize {
        self.held[hole].len()
    }

    /// Advance one fixed physics step and return what happened since the last one.
    pub fn step(&mut self) -> Vec<Timed> {
        let dt = self.world.integration_parameters.dt;

        // Scoops let their balls go after a while; locks wait for `eject`.
        for hole in 0..self.held.len() {
            if let (Some(after), Some(&(_, caught))) = (self.layout.holes[hole].eject_after, self.held[hole].front())
                && self.steps >= caught + (after * PHYSICS_HZ).round() as u64
            {
                self.eject(hole);
            }
        }

        for f in &mut self.flippers {
            let pressed = self.buttons[(f.def.button == Button::Right) as usize];
            let target = if pressed { f.def.up } else { f.def.rest };
            let max = f.def.speed * dt;
            f.angle += (target - f.angle).clamp(-max, max);
            // Kinematic body: Rapier derives the velocity from the pose change,
            // which is what gives the ball its "hit".
            self.world.bodies[f.body].set_next_kinematic_position(Pose::new(f.def.pivot, f.angle));
        }

        let g = self.layout.gravity;
        for b in self.balls.iter_mut().filter(|b| b.in_play()) {
            let body = &mut self.world.bodies[b.body];
            let layer = &self.layout.layers[b.layer.index()];
            let mut v = body.linvel();
            // Ramps: gravity along the slope and across a banked track, on top
            // of the table tilt.
            b.height = match &layer.ramp {
                Some(ramp) => {
                    let p = ramp.sample(body.translation());
                    let right = Vector::new(p.tangent.y, -p.tangent.x);
                    v += (right * p.bank.sin() - p.tangent * p.slope) * (g * dt);
                    p.height
                }
                None => layer.height,
            };
            // Rolling resistance: a constant slowdown that never reverses the ball.
            let speed = v.length();
            if speed > 0.0 {
                v *= (speed - layer.rolling_resistance * g * dt).max(0.0) / speed;
            }
            body.set_linvel(v, true);
        }

        self.world.step_with_events(&self.hooks, &self.collector);
        self.steps += 1;

        let mut events = std::mem::take(&mut self.pending);
        self.contacts(&mut events);
        self.drains(&mut events);
        // Leaves are reported late (debounce) but stamped with when they happened.
        events.sort_by_key(|e| e.step);
        for e in &events {
            self.log(e);
        }
        events
    }

    fn contacts(&mut self, events: &mut Vec<Timed>) {
        let ball_of = |balls: &[Ball], c: ColliderHandle| balls.iter().find(|b| b.collider == c).map(|b| b.number);
        let collisions = std::mem::take(&mut *self.collector.0.lock().unwrap());
        for ev in collisions {
            let (c1, c2, started) = match ev {
                CollisionEvent::Started(a, b, _) => (a, b, true),
                CollisionEvent::Stopped(a, b, _) => (a, b, false),
            };
            let (ball, thing) = match (ball_of(&self.balls, c1), ball_of(&self.balls, c2)) {
                (Some(a), Some(b)) => (a.min(b), Thing::Ball(a.max(b))),
                (Some(a), None) => match self.things.get(&c2) {
                    Some(&t) => (a, t),
                    None => continue,
                },
                (None, Some(b)) => match self.things.get(&c1) {
                    Some(&t) => (b, t),
                    None => continue,
                },
                (None, None) => continue,
            };
            match thing {
                Thing::Transition(t) => {
                    self.cross(ball, t, started, events);
                    continue;
                }
                Thing::Hole(h) => {
                    if started {
                        self.capture(ball, h, events);
                    } else {
                        self.ejected.remove(&(ball, h));
                    }
                    continue;
                }
                _ => {}
            }
            let key = (ball, thing);
            if started {
                // Touching again within the debounce time continues the old contact.
                if self.leaving.remove(&key).is_none() && self.touching.insert(key) {
                    let event = match thing {
                        Thing::Switch(switch) => Event::Switch { ball, switch },
                        Thing::Spinner(spinner) => Event::Spin { ball, spinner, spins: self.spins(ball, spinner) },
                        _ => Event::Touch { ball, thing },
                    };
                    events.push(Timed { step: self.steps, event });
                    if let Thing::Wall(w) = thing
                        && self.layout.walls[w].kick > 0.0
                    {
                        self.kick(ball, w);
                        events.push(Timed { step: self.steps, event: Event::Kick { ball, thing } });
                    }
                    if let Thing::DropTarget(target) = thing
                        && self.drops(ball, target)
                    {
                        self.drops_down[target] = true;
                        self.world.colliders[self.drop_colliders[target]].set_enabled(false);
                        events.push(Timed { step: self.steps, event: Event::Drop { ball, target } });
                    }
                }
            } else if self.touching.contains(&key) {
                self.leaving.insert(key, self.steps);
            }
        }
        let now = self.steps;
        let done: Vec<_> =
            self.leaving.iter().filter(|&(_, &at)| at + LEAVE_DEBOUNCE_STEPS <= now).map(|(&k, &at)| (k, at)).collect();
        for ((ball, thing), at) in done {
            self.leaving.remove(&(ball, thing));
            self.touching.remove(&(ball, thing));
            if !matches!(thing, Thing::Switch(_) | Thing::Spinner(_)) {
                events.push(Timed { step: at, event: Event::Leave { ball, thing } });
            }
        }
    }

    /// A ball started or stopped overlapping transition `t`. Two-way lines
    /// decide by the direction of travel when the ball arrives, so the ball is
    /// on the right layer while it crosses, and every line decides by the side
    /// the ball's centre is on when it leaves. That also catches a ball that
    /// stalls on the line and rolls back.
    fn cross(&mut self, number: u32, t: usize, started: bool, events: &mut Vec<Timed>) {
        let tr = &self.layout.transitions[t];
        let Some(b) = self.balls.iter().position(|b| b.number == number) else { return };
        let body = &self.world.bodies[self.balls[b].body];
        let to = if started {
            if !tr.two_way {
                return;
            }
            if body.linvel().dot(tr.direction) >= 0.0 { tr.target } else { tr.layer }
        } else if tr.beyond(body.translation()) {
            tr.target
        } else {
            tr.layer
        };
        let from = self.balls[b].layer;
        // A one-way line only acts on balls coming from its own layer.
        if to != from && (tr.two_way || from == tr.layer) {
            set_layer(&mut self.world, &mut self.balls[b], to);
            events.push(Timed { step: self.steps, event: Event::Cross { ball: number, transition: t, from, to } });
        }
    }

    fn ball_velocity(&self, number: u32) -> Vector {
        let b = self.balls.iter().find(|b| b.number == number).unwrap();
        self.world.bodies[b.body].linvel()
    }

    /// Whether this hit drops the target: into its front, hard enough.
    fn drops(&self, number: u32, target: usize) -> bool {
        let d = &self.layout.drop_targets[target];
        let into_front = -self.ball_velocity(number).dot(d.facing);
        into_front >= d.min_speed
    }

    /// Spins from the ball's speed across the spinner (at least one).
    fn spins(&self, number: u32, spinner: usize) -> u32 {
        let s = &self.layout.spinners[spinner];
        let along = (s.b - s.a).normalize();
        let across = Vector::new(-along.y, along.x);
        ((self.ball_velocity(number).dot(across).abs() * s.spins_per_mps).round() as u32).max(1)
    }

    /// A ball rolled into hole `h`: hold it there, unless it was just ejected
    /// from it or is too fast.
    fn capture(&mut self, number: u32, h: usize, events: &mut Vec<Timed>) {
        let hole = &self.layout.holes[h];
        if self.ejected.contains(&(number, h)) || self.ball_velocity(number).length() > hole.max_speed {
            return;
        }
        let b = self.balls.iter_mut().find(|b| b.number == number).unwrap();
        if b.held.is_some() {
            return;
        }
        b.held = Some(h);
        let body = &mut self.world.bodies[b.body];
        body.set_translation(hole.center, false);
        body.set_linvel(Vector::ZERO, false);
        body.set_enabled(false);
        self.held[h].push_back((number, self.steps));
        // Its contacts end while it's held, without leave events.
        self.touching.retain(|&(n, _)| n != number);
        self.leaving.retain(|&(n, _), _| n != number);
        events.push(Timed { step: self.steps, event: Event::Capture { ball: number, hole: h } });
    }

    /// Pushes the ball away from the nearest point of wall `w` at its kick speed,
    /// keeping its speed along the wall.
    fn kick(&mut self, number: u32, w: usize) {
        let wall = &self.layout.walls[w];
        let Some(b) = self.balls.iter().find(|b| b.number == number) else { return };
        let body = &mut self.world.bodies[b.body];
        let p = body.translation();
        let n = wall.pts.len();
        let segs = if wall.closed { n } else { n - 1 };
        let nearest = (0..segs)
            .map(|i| {
                let (a, c) = (wall.pts[i], wall.pts[(i + 1) % n]);
                let ac = c - a;
                a + ac * ((p - a).dot(ac) / ac.length_squared().max(1e-12)).clamp(0.0, 1.0)
            })
            .min_by(|x, y| x.distance_squared(p).total_cmp(&y.distance_squared(p)))
            .unwrap();
        let away = (p - nearest).normalize_or_zero();
        let v = body.linvel();
        body.set_linvel(v - away * v.dot(away) + away * wall.kick, true);
    }

    fn drains(&mut self, events: &mut Vec<Timed>) {
        let mut i = 0;
        while i < self.balls.len() {
            let b = &self.balls[i];
            if b.in_play() && self.world.bodies[b.body].translation().y < DRAIN_Y {
                let number = b.number;
                events.push(Timed { step: self.steps, event: Event::Drain { ball: number } });
                let b = self.balls.swap_remove(i);
                self.world.remove_body_with_colliders(b.body, true);
                // Its contacts end with it, without leave events.
                self.touching.retain(|&(n, t)| n != number && t != Thing::Ball(number));
                self.leaving.retain(|&(n, t), _| n != number && t != Thing::Ball(number));
            } else {
                i += 1;
            }
        }
    }

    /// Name of a thing as the table file calls it, e.g. `outer_wall`, or
    /// `wall#3` / `flipper#1` for elements without an id.
    pub fn name(&self, thing: Thing) -> String {
        match thing {
            Thing::Wall(i) => self.layout.walls[i].id.clone().unwrap_or_else(|| format!("wall#{i}")),
            Thing::Flipper(i) => self.layout.flippers[i].id.clone().unwrap_or_else(|| format!("flipper#{i}")),
            Thing::Ball(n) => format!("ball#{n}"),
            Thing::Switch(i) => self.layout.switches[i].id.clone().unwrap_or_else(|| format!("switch#{i}")),
            Thing::Transition(i) => {
                self.layout.transitions[i].id.clone().unwrap_or_else(|| format!("transition#{i}"))
            }
            Thing::DropTarget(i) => {
                self.layout.drop_targets[i].id.clone().unwrap_or_else(|| format!("drop_target#{i}"))
            }
            Thing::Hole(i) => self.layout.holes[i].id.clone().unwrap_or_else(|| format!("hole#{i}")),
            Thing::Spinner(i) => self.layout.spinners[i].id.clone().unwrap_or_else(|| format!("spinner#{i}")),
        }
    }

    /// One line per event, e.g. `touch ball#1 outer_wall`. Positions are
    /// rounded to millimetres and speeds to cm/s.
    pub fn describe(&self, event: &Event) -> String {
        match *event {
            Event::Serve { ball, pos } => format!("serve ball#{ball} at ({:.3}, {:.3})", pos.x, pos.y),
            Event::Launch { ball, vel } => format!("launch ball#{ball} v=({:.2}, {:.2})", vel.x, vel.y),
            Event::Touch { ball, thing } => format!("touch ball#{ball} {}", self.name(thing)),
            Event::Leave { ball, thing } => format!("leave ball#{ball} {}", self.name(thing)),
            Event::Kick { ball, thing } => format!("kick ball#{ball} {}", self.name(thing)),
            Event::Switch { ball, switch } => format!("switch ball#{ball} {}", self.name(Thing::Switch(switch))),
            Event::Drop { ball, target } => format!("drop ball#{ball} {}", self.name(Thing::DropTarget(target))),
            Event::Reset { target } => format!("reset {}", self.name(Thing::DropTarget(target))),
            Event::Capture { ball, hole } => format!("capture ball#{ball} {}", self.name(Thing::Hole(hole))),
            Event::Eject { ball, hole } => format!("eject ball#{ball} {}", self.name(Thing::Hole(hole))),
            Event::Spin { ball, spinner, spins } => {
                format!("spin ball#{ball} {} x{spins}", self.name(Thing::Spinner(spinner)))
            }
            Event::Cross { ball, transition, from, to } => format!(
                "cross ball#{ball} {}: {} -> {}",
                self.name(Thing::Transition(transition)),
                self.layout.layer_name(from),
                self.layout.layer_name(to)
            ),
            Event::FlipperUp { flipper } => format!("flipper_up {}", self.name(Thing::Flipper(flipper))),
            Event::FlipperDown { flipper } => format!("flipper_down {}", self.name(Thing::Flipper(flipper))),
            Event::Drain { ball } => format!("drain ball#{ball}"),
        }
    }

    fn log(&self, e: &Timed) {
        let level = match e.event {
            Event::Touch { .. }
            | Event::Leave { .. }
            | Event::Kick { .. }
            | Event::FlipperUp { .. }
            | Event::FlipperDown { .. } => log::Level::Debug,
            _ => log::Level::Info,
        };
        log::log!(level, "{:8.3}s {}", e.time(), self.describe(&e.event));
    }
}

/// Balls only collide with their own layer (and other balls on it).
fn set_layer(world: &mut PhysicsWorld, ball: &mut Ball, layer: Layer) {
    ball.layer = layer;
    world.colliders[ball.collider].set_collision_groups(groups(layer.bit(), layer.bit()));
}

fn make_flipper(world: &mut PhysicsWorld, def: &FlipperDef) -> (Flipper, ColliderHandle) {
    let collider = ColliderBuilder::convex_hull(&def.outline)
        .expect("flipper outline has no area")
        .restitution(def.restitution)
        .friction(def.friction)
        .collision_groups(layer_groups(def.layer));
    let outline = match collider.shape.as_convex_polygon() {
        Some(poly) => poly.points().to_vec(),
        None => def.outline.clone(),
    };
    let (body, handle) = world.insert(
        RigidBodyBuilder::kinematic_position_based()
            .pose(Pose::new(def.pivot, def.rest))
            .ccd_enabled(true),
        collider,
    );
    (Flipper { def: def.clone(), body, outline, angle: def.rest }, handle)
}
