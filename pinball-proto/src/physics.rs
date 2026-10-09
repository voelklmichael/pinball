//! Rapier world built from a [`Layout`]: walls, flippers and balls on layers.
//! Reports [`Event`]s; scoring and rules live in `game`.

use rapier2d::prelude::*;

use crate::layout::{Button, FlipperDef, Layer, Layout};

pub const PHYSICS_HZ: f32 = 480.0;
const DRAIN_Y: f32 = -0.03;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The ball left the bottom of the table.
    Drain(RigidBodyHandle),
}

pub struct Ball {
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

pub struct Table {
    world: PhysicsWorld,
    pub layout: Layout,
    pub balls: Vec<Ball>,
    pub flippers: Vec<Flipper>,
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

        let flippers = layout.flippers.iter().map(|def| make_flipper(&mut world, def)).collect();
        Self { world, layout, balls: Vec::new(), flippers, launch_key: false, buttons: [false; 2] }
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
                .friction_combine_rule(CoefficientCombineRule::Multiply),
        );
        let mut ball = Ball { body, collider, layer: Layer::PLAYFIELD, waiting: false };
        set_layer(&mut self.world, &mut ball, Layer::PLAYFIELD);
        self.balls.push(ball);
        body
    }

    /// Puts a new ball at the launch position, frozen until it's launched.
    pub fn serve_ball(&mut self) -> RigidBodyHandle {
        let body = self.add_ball(self.layout.launcher.position, Vector::ZERO);
        self.world.bodies[body].set_enabled(false);
        self.balls.last_mut().unwrap().waiting = true;
        body
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
        }
    }

    /// Advance one fixed physics step and return what happened.
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

        self.world.step();

        let mut events = Vec::new();
        let mut i = 0;
        while i < self.balls.len() {
            let b = &self.balls[i];
            if !b.waiting && self.world.bodies[b.body].translation().y < DRAIN_Y {
                events.push(Event::Drain(b.body));
                let b = self.balls.swap_remove(i);
                self.world.remove_body_with_colliders(b.body, true);
            } else {
                i += 1;
            }
        }
        events
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
