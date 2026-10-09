//! Rapier world built from a [`Layout`]. Knows about layers, ramps, kickers and
//! holes, and reports switch hits as [`Event`]s; scoring and rules live in `game`.

use std::collections::HashMap;
use std::sync::Mutex;

use rapier2d::prelude::*;

use crate::layout::{Button, FlipperDef, Layer, Layout, Shape, Sw, UPPER_HEIGHT};

pub const PHYSICS_HZ: f32 = 480.0;
const BUMPER_KICK: f32 = 1.6;
const SLING_KICK: f32 = 1.3;
const KICK_COOLDOWN: f32 = 0.08;
const DRAIN_Y: f32 = -0.03;

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub sw: Sw,
    pub ball: RigidBodyHandle,
    pub vel: Vector,
}

pub struct Ball {
    pub body: RigidBodyHandle,
    collider: ColliderHandle,
    pub layer: Layer,
    /// Height above the playfield, for drawing.
    pub height: f32,
    /// Index of the hole holding this ball, if any.
    pub held: Option<usize>,
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

#[derive(Clone, Copy)]
enum Elem {
    Sensor(usize),
    Hole(usize),
    Bumper(usize),
    Sling(usize),
    Drop(usize),
    Standup(usize),
}

/// Lets balls pass one-way gates in the gate's `pass` direction.
struct GateHooks {
    gates: HashMap<ColliderHandle, Vector>,
}

impl PhysicsHooks for GateHooks {
    fn modify_solver_contacts(&self, ctx: &mut ContactModificationContext) {
        let (gate, ball) = match (self.gates.get(&ctx.collider1), self.gates.get(&ctx.collider2)) {
            (Some(&pass), None) => (pass, ctx.rigid_body2),
            (None, Some(&pass)) => (pass, ctx.rigid_body1),
            _ => return,
        };
        let Some(ball) = ball else { return };
        if ctx.bodies[ball].linvel().dot(gate) > 0.0 {
            if let Some(m) = ctx.rigid_mut() {
                m.solver_contacts.clear();
            }
        }
    }
}

#[derive(Default)]
struct Collector(Mutex<Vec<CollisionEvent>>);

impl EventHandler for Collector {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: CollisionEvent,
        _pair: Option<&ContactPair>,
    ) {
        self.0.lock().unwrap().push(event);
    }

    fn handle_contact_force_event(
        &self,
        _dt: Real,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _pair: &ContactPair,
        _total_force_magnitude: Real,
    ) {
    }

    fn handle_soft_body_tear_event(&self, _soft_bodies: &SoftBodySet, _event: &SoftBodyTearEvent) {}
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

pub struct Table {
    world: PhysicsWorld,
    hooks: GateHooks,
    collector: Collector,
    pub layout: Layout,
    pub balls: Vec<Ball>,
    pub flippers: Vec<Flipper>,
    elems: HashMap<ColliderHandle, Elem>,
    drop_colliders: Vec<ColliderHandle>,
    pub drops_down: Vec<bool>,
    kick_cooldown: HashMap<ColliderHandle, f32>,
    /// Seconds left on each element's "lit" flash, keyed like the layout vectors.
    pub bumper_flash: Vec<f32>,
    pub sling_flash: Vec<f32>,
    pub plunger_charge: f32,
    launch_key: bool,
    buttons: [bool; 2],
}

impl Table {
    pub fn new(layout: Layout) -> Self {
        let mut world = PhysicsWorld::default();
        // Only the component along the tilted playfield acts in 2D.
        world.gravity = Vector::new(0.0, -layout.gravity * layout.angle.sin());
        world.integration_parameters.dt = 1.0 / PHYSICS_HZ;
        // Rapier's tolerances are tuned for metre-sized objects; ours are small.
        world.integration_parameters.length_unit = 0.05;

        let ground = world.insert_body(RigidBodyBuilder::fixed());
        let mut elems = HashMap::new();
        let mut hooks = GateHooks { gates: HashMap::new() };

        for w in &layout.walls {
            let mut pts = w.pts.clone();
            if w.closed {
                pts.push(pts[0]);
            }
            world.insert_collider(
                ColliderBuilder::polyline(pts, None)
                    .restitution(w.restitution)
                    .friction(w.friction)
                    .collision_groups(layer_groups(w.layer)),
                Some(ground),
            );
        }

        for (i, s) in layout.sensors.iter().enumerate() {
            let builder = match s.shape {
                Shape::Rect { min, max } => {
                    let half = (max - min) / 2.0;
                    ColliderBuilder::cuboid(half.x, half.y).translation(min + half)
                }
                Shape::Circle { c, r } => ColliderBuilder::ball(r).translation(c),
            };
            let h = world.insert_collider(
                builder
                    .sensor(true)
                    .active_events(ActiveEvents::COLLISION_EVENTS)
                    .collision_groups(groups(s.layers, u32::MAX)),
                Some(ground),
            );
            elems.insert(h, Elem::Sensor(i));
        }

        for (i, b) in layout.bumpers.iter().enumerate() {
            let h = world.insert_collider(
                ColliderBuilder::ball(b.radius)
                    .translation(b.center)
                    .restitution(0.2)
                    .collision_groups(layer_groups(Layer::Playfield)),
                Some(ground),
            );
            elems.insert(h, Elem::Bumper(i));
        }

        for (i, s) in layout.slings.iter().enumerate() {
            world.insert_collider(
                ColliderBuilder::polyline(vec![s.b, s.c, s.a], None)
                    .restitution(0.3)
                    .collision_groups(layer_groups(Layer::Playfield)),
                Some(ground),
            );
            let h = world.insert_collider(
                ColliderBuilder::segment(s.a, s.b)
                    .restitution(0.3)
                    .collision_groups(layer_groups(Layer::Playfield)),
                Some(ground),
            );
            elems.insert(h, Elem::Sling(i));
        }

        let mut drop_colliders = Vec::new();
        for (i, d) in layout.drops.iter().enumerate() {
            let h = world.insert_collider(
                ColliderBuilder::cuboid(d.half.x, d.half.y)
                    .translation(d.center)
                    .restitution(0.2)
                    .collision_groups(layer_groups(Layer::Playfield)),
                Some(ground),
            );
            elems.insert(h, Elem::Drop(i));
            drop_colliders.push(h);
        }

        for (i, s) in layout.standups.iter().enumerate() {
            let h = world.insert_collider(
                ColliderBuilder::segment(s.a, s.b)
                    .restitution(0.3)
                    .collision_groups(layer_groups(s.layer)),
                Some(ground),
            );
            elems.insert(h, Elem::Standup(i));
        }

        for g in &layout.gates {
            let h = world.insert_collider(
                ColliderBuilder::segment(g.a, g.b)
                    .restitution(0.1)
                    .active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
                    .collision_groups(layer_groups(g.layer)),
                Some(ground),
            );
            hooks.gates.insert(h, g.pass);
        }

        // The scoop is a sensor too: a cup of walls (in the layout) plus this trigger.
        for (i, hole) in layout.holes.iter().enumerate() {
            let h = world.insert_collider(
                ColliderBuilder::ball(hole.radius)
                    .translation(hole.center)
                    .sensor(true)
                    .active_events(ActiveEvents::COLLISION_EVENTS)
                    .collision_groups(groups(Layer::Playfield.bit(), u32::MAX)),
                Some(ground),
            );
            elems.insert(h, Elem::Hole(i));
        }

        let flippers = layout
            .flippers
            .iter()
            .map(|def| make_flipper(&mut world, def))
            .collect();

        let n_bumpers = layout.bumpers.len();
        let n_slings = layout.slings.len();
        let n_drops = layout.drops.len();
        Self {
            world,
            hooks,
            collector: Collector::default(),
            layout,
            balls: Vec::new(),
            flippers,
            elems,
            drop_colliders,
            drops_down: vec![false; n_drops],
            kick_cooldown: HashMap::new(),
            bumper_flash: vec![0.0; n_bumpers],
            sling_flash: vec![0.0; n_slings],
            plunger_charge: 0.0,
            launch_key: false,
            buttons: [false; 2],
        }
    }

    pub fn ball_pos(&self, ball: &Ball) -> Vector {
        self.world.bodies[ball.body].translation()
    }

    #[cfg(test)]
    pub fn ball_vel(&self, ball: &Ball) -> Vector {
        self.world.bodies[ball.body].linvel()
    }

    pub fn flipper_pose(&self, f: &Flipper) -> Pose {
        *self.world.bodies[f.body].position()
    }

    pub fn set_buttons(&mut self, left: bool, right: bool) {
        self.buttons = [left, right];
    }

    /// Adds a ball at `pos` on the playfield layer.
    pub fn add_ball(&mut self, pos: Vector, vel: Vector) -> RigidBodyHandle {
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
        let mut ball = Ball { body, collider, layer: Layer::Playfield, height: 0.0, held: None, waiting: false };
        set_layer(&mut self.world, &mut ball, Layer::Playfield);
        self.balls.push(ball);
        body
    }

    /// Puts a new ball at the launch position, frozen until it's launched.
    pub fn serve_ball(&mut self) -> RigidBodyHandle {
        let body = self.add_ball(self.layout.launch.pos, Vector::ZERO);
        self.world.bodies[body].set_enabled(false);
        self.balls.last_mut().unwrap().waiting = true;
        body
    }

    pub fn ball_waiting(&self) -> bool {
        self.balls.iter().any(|b| b.waiting)
    }

    /// Launch key. A charged launch (plunger) fires on release, harder the
    /// longer it was held; otherwise it fires on press at the launch velocity.
    pub fn plunger(&mut self, held: bool, dt: f32) {
        let pressed = held && !self.launch_key;
        let released = !held && self.launch_key;
        self.launch_key = held;
        let launch = &self.layout.launch;
        let fire = if launch.charged {
            if held {
                self.plunger_charge = (self.plunger_charge + dt * 1.2).min(1.0);
            }
            released.then(|| launch.vel * (0.3 + 0.7 * std::mem::take(&mut self.plunger_charge)))
        } else {
            pressed.then_some(launch.vel)
        };
        let Some(vel) = fire else { return };
        for b in self.balls.iter_mut().filter(|b| b.waiting) {
            b.waiting = false;
            let body = &mut self.world.bodies[b.body];
            body.set_enabled(true);
            body.set_linvel(vel, true);
        }
    }

    pub fn set_ball_vel(&mut self, ball: RigidBodyHandle, vel: Vector) {
        self.world.bodies[ball].set_linvel(vel, true);
    }

    /// Ejects the ball held in hole `i`, if any.
    pub fn eject(&mut self, i: usize) {
        let hole = &self.layout.holes[i];
        let (center, eject) = (hole.center, hole.eject);
        for b in &mut self.balls {
            if b.held == Some(i) {
                b.held = None;
                let body = &mut self.world.bodies[b.body];
                body.set_enabled(true);
                body.set_translation(center, true);
                body.set_linvel(eject, true);
                return;
            }
        }
    }

    pub fn hole_occupied(&self, i: usize) -> bool {
        self.balls.iter().any(|b| b.held == Some(i))
    }

    pub fn reset_drops(&mut self) {
        for (i, &h) in self.drop_colliders.iter().enumerate() {
            self.world.colliders[h].set_enabled(true);
            self.drops_down[i] = false;
        }
    }

    /// Advance one fixed physics step and return the switches hit.
    pub fn step(&mut self) -> Vec<Event> {
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

        // Ramp inclines: gravity along the ramp, on top of the table tilt.
        for b in &mut self.balls {
            b.height = match b.layer {
                Layer::Playfield => 0.0,
                Layer::Upper => UPPER_HEIGHT,
                layer => {
                    let body = &mut self.world.bodies[b.body];
                    let ramp = self.layout.ramps.iter().find(|r| r.layer == layer).unwrap();
                    let (h, tangent, slope) = ramp.sample(body.translation());
                    let v = body.linvel() - tangent * (self.layout.gravity * slope * dt);
                    body.set_linvel(v, true);
                    h
                }
            };
        }

        self.world.step_with_events(&self.hooks, &self.collector);

        let mut events = Vec::new();
        for f in self.bumper_flash.iter_mut().chain(&mut self.sling_flash) {
            *f = (*f - dt).max(0.0);
        }
        self.kick_cooldown.retain(|_, t| {
            *t -= dt;
            *t > 0.0
        });

        let collisions = std::mem::take(&mut *self.collector.0.lock().unwrap());
        for ev in collisions {
            let CollisionEvent::Started(c1, c2, _) = ev else { continue };
            let Some(bi) = self.balls.iter().position(|b| b.collider == c1 || b.collider == c2) else {
                continue;
            };
            let other = if self.balls[bi].collider == c1 { c2 } else { c1 };
            if let Some(&elem) = self.elems.get(&other) {
                self.on_hit(bi, other, elem, &mut events);
            }
        }

        // Drains.
        let mut i = 0;
        while i < self.balls.len() {
            let b = &self.balls[i];
            let body = &self.world.bodies[b.body];
            if b.held.is_none() && !b.waiting && body.translation().y < DRAIN_Y {
                events.push(Event { sw: Sw::Drain, ball: b.body, vel: body.linvel() });
                let b = self.balls.swap_remove(i);
                self.world.remove_body_with_colliders(b.body, true);
            } else {
                i += 1;
            }
        }
        events
    }

    fn on_hit(&mut self, bi: usize, other: ColliderHandle, elem: Elem, events: &mut Vec<Event>) {
        let body = self.balls[bi].body;
        let pos = self.world.bodies[body].translation();
        let vel = self.world.bodies[body].linvel();
        let mut emit = |sw| events.push(Event { sw, ball: body, vel });
        match elem {
            Elem::Hole(hole) => {
                if self.balls[bi].held.is_none() && !self.hole_occupied(hole) {
                    self.balls[bi].held = Some(hole);
                    let rb = &mut self.world.bodies[body];
                    rb.set_linvel(Vector::ZERO, false);
                    rb.set_enabled(false);
                    emit(self.layout.holes[hole].sw);
                }
            }
            Elem::Sensor(i) => {
                let s = &self.layout.sensors[i];
                let (sw, switch) = (s.sw, s.switch);
                if let Some(ls) = switch {
                    let layer = if vel.dot(ls.dir) >= 0.0 { ls.fwd } else { ls.back };
                    set_layer(&mut self.world, &mut self.balls[bi], layer);
                }
                if let Some(sw) = sw {
                    emit(sw);
                }
            }
            Elem::Bumper(i) => {
                let center = self.layout.bumpers[i].center;
                if self.kick(other, body, (pos - center).normalize(), BUMPER_KICK) {
                    self.bumper_flash[i] = 0.12;
                    emit(Sw::Bumper(i as u8));
                }
            }
            Elem::Sling(i) => {
                let normal = self.layout.slings[i].normal();
                if self.kick(other, body, normal, SLING_KICK) {
                    self.sling_flash[i] = 0.12;
                    emit(Sw::Sling(i as u8));
                }
            }
            Elem::Drop(i) => {
                if !self.drops_down[i] {
                    self.drops_down[i] = true;
                    self.world.colliders[other].set_enabled(false);
                    emit(Sw::Drop(i as u8));
                }
            }
            Elem::Standup(i) => emit(self.layout.standups[i].sw),
        }
    }

    /// Active elements add energy: replace the ball's velocity component along `dir`.
    fn kick(&mut self, kicker: ColliderHandle, ball: RigidBodyHandle, dir: Vector, speed: f32) -> bool {
        if self.kick_cooldown.contains_key(&kicker) {
            return false;
        }
        self.kick_cooldown.insert(kicker, KICK_COOLDOWN);
        let body = &mut self.world.bodies[ball];
        let v = body.linvel();
        let tangential = v - dir * v.dot(dir);
        body.set_linvel(tangential * 0.8 + dir * speed, true);
        true
    }
}

/// Balls only collide with their own layer (and other balls on it).
fn set_layer(world: &mut PhysicsWorld, ball: &mut Ball, layer: Layer) {
    ball.layer = layer;
    world.colliders[ball.collider].set_collision_groups(groups(layer.bit(), layer.bit()));
}

fn make_flipper(world: &mut PhysicsWorld, def: &FlipperDef) -> Flipper {
    let collider = ColliderBuilder::convex_hull(&def.outline)
        .expect("flipper outline has no area")
        .restitution(def.restitution)
        .friction(def.friction)
        .collision_groups(layer_groups(def.layer));
    let outline = match collider.shape.as_convex_polygon() {
        Some(poly) => poly.points().to_vec(),
        None => def.outline.clone(),
    };
    let (body, _) = world.insert(
        RigidBodyBuilder::kinematic_position_based()
            .pose(Pose::new(def.pivot, def.rest))
            .ccd_enabled(true),
        collider,
    );
    Flipper { def: def.clone(), body, outline, angle: def.rest }
}
