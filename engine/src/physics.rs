//! Rapier world built from a [`Layout`]: walls, flippers and balls on layers.
//! Reports what happens as [`Timed`] [`Event`]s, which are also logged;
//! scoring and rules live in `game`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use rapier2d::prelude::*;

use crate::layout::{Button, FlipperDef, Layer, Layout};

pub const PHYSICS_HZ: f32 = 480.0;
const DRAIN_Y: f32 = -0.03;
/// A contact only counts as ended after this many steps (10 ms) without
/// touching, so a ball rolling along a wall doesn't flicker touch/leave.
const LEAVE_DEBOUNCE_STEPS: u64 = 5;

/// Something a ball can touch: an index into the layout's walls or flippers,
/// or another ball's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Thing {
    Wall(usize),
    Flipper(usize),
    Ball(u32),
}

/// Balls are numbered from 1 in the order they enter play.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Serve { ball: u32, pos: Vector },
    Launch { ball: u32, vel: Vector },
    Touch { ball: u32, thing: Thing },
    Leave { ball: u32, thing: Thing },
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
    /// Frozen at the launch position until the launch key fires it.
    pub waiting: bool,
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
        let ground = world.insert_body(RigidBodyBuilder::fixed());
        for (i, w) in layout.walls.iter().enumerate() {
            let mut pts = w.pts.clone();
            if w.closed {
                pts.push(pts[0]);
            }
            let h = world.insert_collider(
                ColliderBuilder::polyline(pts, None)
                    .restitution(w.restitution)
                    .friction(w.friction)
                    .collision_groups(layer_groups(w.layer)),
                Some(ground),
            );
            things.insert(h, Thing::Wall(i));
        }

        let mut flippers = Vec::new();
        for (i, def) in layout.flippers.iter().enumerate() {
            let (flipper, collider) = make_flipper(&mut world, def);
            things.insert(collider, Thing::Flipper(i));
            flippers.push(flipper);
        }

        let layer_names: Vec<&str> = layout.layers.iter().map(|l| l.name.as_str()).collect();
        log::info!(
            "table \"{}\": layers [{}], {} walls, {} flippers",
            layout.name,
            layer_names.join(", "),
            layout.walls.len(),
            layout.flippers.len()
        );
        Self {
            world,
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
        let mut ball = Ball { number, body, collider, layer: Layer::PLAYFIELD, waiting: false };
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

    /// Advance one fixed physics step and return what happened since the last one.
    pub fn step(&mut self) -> Vec<Timed> {
        let dt = self.world.integration_parameters.dt;

        for f in &mut self.flippers {
            let pressed = self.buttons[(f.def.button == Button::Right) as usize];
            let target = if pressed { f.def.up } else { f.def.rest };
            let max = f.def.speed * dt;
            f.angle += (target - f.angle).clamp(-max, max);
            // Kinematic body: Rapier derives the velocity from the pose change,
            // which is what gives the ball its "hit".
            self.world.bodies[f.body].set_next_kinematic_position(Pose::new(f.def.pivot, f.angle));
        }

        // Rolling resistance: a constant slowdown that never reverses the ball.
        let g = self.layout.gravity;
        for b in self.balls.iter().filter(|b| !b.waiting) {
            let body = &mut self.world.bodies[b.body];
            let mut v = body.linvel();
            let speed = v.length();
            if speed > 0.0 {
                let rr = self.layout.layers[b.layer.index()].rolling_resistance;
                v *= (speed - rr * g * dt).max(0.0) / speed;
                body.set_linvel(v, true);
            }
        }

        self.world.step_with_events(&(), &self.collector);
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
            let key = (ball, thing);
            if started {
                // Touching again within the debounce time continues the old contact.
                if self.leaving.remove(&key).is_none() && self.touching.insert(key) {
                    events.push(Timed { step: self.steps, event: Event::Touch { ball, thing } });
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
            events.push(Timed { step: at, event: Event::Leave { ball, thing } });
        }
    }

    fn drains(&mut self, events: &mut Vec<Timed>) {
        let mut i = 0;
        while i < self.balls.len() {
            let b = &self.balls[i];
            if !b.waiting && self.world.bodies[b.body].translation().y < DRAIN_Y {
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
            Event::FlipperUp { flipper } => format!("flipper_up {}", self.name(Thing::Flipper(flipper))),
            Event::FlipperDown { flipper } => format!("flipper_down {}", self.name(Thing::Flipper(flipper))),
            Event::Drain { ball } => format!("drain ball#{ball}"),
        }
    }

    fn log(&self, e: &Timed) {
        let level = match e.event {
            Event::Touch { .. } | Event::Leave { .. } | Event::FlipperUp { .. } | Event::FlipperDown { .. } => {
                log::Level::Debug
            }
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
