//! Table layout and physics. Units are metres, +y points up the playfield.
//! Gravity is the in-plane component of g on a tilted table (exaggerated a bit
//! for feel; a real 6.5° table gives ~1.1 m/s²).

use rapier2d::prelude::*;

pub const TABLE_W: f32 = 0.505;
pub const TABLE_H: f32 = 1.06;
pub const BALL_R: f32 = 0.0135;
pub const PHYSICS_HZ: f32 = 480.0;
pub const LANE_X: f32 = 0.47; // plunger lane separator

const GRAVITY: f32 = 2.5;
const FLIPPER_LEN: f32 = 0.075;
const FLIPPER_R_BASE: f32 = 0.011;
const FLIPPER_R_TIP: f32 = 0.006;
const FLIPPER_REST: f32 = -0.52; // ~30° below horizontal
const FLIPPER_UP: f32 = 0.52;
const FLIPPER_SPEED: f32 = 35.0; // rad/s, full stroke in ~30ms
const PLUNGER_MAX_SPEED: f32 = 4.5;
const BUMPER_KICK: f32 = 1.6;
const SLING_KICK: f32 = 1.3;
const KICK_COOLDOWN: f32 = 0.08;

pub struct Flipper {
    body: RigidBodyHandle,
    pivot: Vector,
    /// Local-space outline, for drawing.
    pub outline: Vec<Vector>,
    /// +1 for the left flipper, -1 for the right (mirrored).
    side: f32,
    angle: f32,
    pub pressed: bool,
}

pub enum KickerKind {
    /// Round pop bumper: kicks radially away from its centre.
    Bumper { center: Vector, radius: f32 },
    /// Slingshot face: kicks along a fixed normal.
    Sling { a: Vector, b: Vector, normal: Vector },
}

pub struct Kicker {
    collider: ColliderHandle,
    pub kind: KickerKind,
    cooldown: f32,
    /// Seconds left on the "lit" flash, for drawing.
    pub flash: f32,
    score: u32,
}

pub struct Table {
    world: PhysicsWorld,
    ball: RigidBodyHandle,
    ball_collider: ColliderHandle,
    pub flippers: [Flipper; 2],
    pub kickers: Vec<Kicker>,
    /// Static wall polylines, for drawing.
    pub walls: Vec<Vec<Vector>>,
    pub plunger_charge: f32,
    pub score: u32,
    pub balls_left: u32,
}

impl Table {
    pub fn new() -> Self {
        let mut world = PhysicsWorld::default();
        world.gravity = Vector::new(0.0, -GRAVITY);
        world.integration_parameters.dt = 1.0 / PHYSICS_HZ;
        // Rapier's tolerances are tuned for metre-sized objects; tell it ours are small.
        world.integration_parameters.length_unit = 0.05;

        let mut walls = wall_layout();
        let ground = world.insert_body(RigidBodyBuilder::fixed());
        for w in &walls {
            world.insert_collider(
                ColliderBuilder::polyline(w.clone(), None)
                    .restitution(0.45)
                    .friction(0.05),
                Some(ground),
            );
        }

        let (ball, ball_collider) = world.insert(
            RigidBodyBuilder::dynamic()
                .translation(ball_start())
                .ccd_enabled(true)
                .can_sleep(false),
            ColliderBuilder::ball(BALL_R)
                .density(7850.0)
                .restitution(0.3)
                .friction(0.1),
        );

        let center = LANE_X / 2.0;
        let gap = 0.016;
        let pivot_dx = gap + FLIPPER_LEN * FLIPPER_REST.cos();
        let flippers = [
            make_flipper(&mut world, Vector::new(center - pivot_dx, 0.135), 1.0),
            make_flipper(&mut world, Vector::new(center + pivot_dx, 0.135), -1.0),
        ];

        let mut kickers = Vec::new();
        for (x, y) in [(0.15, 0.78), (0.32, 0.78), (0.235, 0.66)] {
            let center = Vector::new(x, y);
            let radius = 0.026;
            let collider = world.insert_collider(
                ColliderBuilder::ball(radius)
                    .translation(center)
                    .restitution(0.2),
                Some(ground),
            );
            kickers.push(Kicker {
                collider,
                kind: KickerKind::Bumper { center, radius },
                cooldown: 0.0,
                flash: 0.0,
                score: 100,
            });
        }
        for side in [1.0f32, -1.0] {
            let mx = |x: f32| if side > 0.0 { x } else { LANE_X - x };
            // Triangle above each flipper; the face towards the centre (a-b) kicks.
            let a = Vector::new(mx(0.075), 0.33);
            let b = Vector::new(mx(0.135), 0.215);
            let c = Vector::new(mx(0.075), 0.235);
            let back = vec![b, c, a];
            world.insert_collider(
                ColliderBuilder::polyline(back.clone(), None).restitution(0.3),
                Some(ground),
            );
            walls.push(back);
            let face = (b - a).normalize();
            let normal = Vector::new(-face.y, face.x) * side;
            let collider = world.insert_collider(
                ColliderBuilder::segment(a, b).restitution(0.3),
                Some(ground),
            );
            kickers.push(Kicker {
                collider,
                kind: KickerKind::Sling { a, b, normal },
                cooldown: 0.0,
                flash: 0.0,
                score: 10,
            });
        }

        Self {
            world,
            ball,
            ball_collider,
            flippers,
            kickers,
            walls,
            plunger_charge: 0.0,
            score: 0,
            balls_left: 3,
        }
    }

    pub fn ball_pos(&self) -> Vector {
        self.world.bodies[self.ball].translation()
    }

    pub fn flipper_pose(&self, i: usize) -> Pose {
        *self.world.bodies[self.flippers[i].body].position()
    }

    /// Hold to charge, release to fire. Only fires if the ball sits in the lane.
    pub fn plunger(&mut self, held: bool, dt: f32) {
        if held {
            self.plunger_charge = (self.plunger_charge + dt * 1.2).min(1.0);
            return;
        }
        if self.plunger_charge > 0.0 {
            let p = self.ball_pos();
            if p.x > LANE_X && p.y < 0.1 {
                let speed = PLUNGER_MAX_SPEED * (0.3 + 0.7 * self.plunger_charge);
                self.world.bodies[self.ball].set_linvel(Vector::new(0.0, speed), true);
            }
            self.plunger_charge = 0.0;
        }
    }

    pub fn reset_ball(&mut self) {
        let b = &mut self.world.bodies[self.ball];
        b.set_translation(ball_start(), true);
        b.set_linvel(Vector::ZERO, true);
        b.set_angvel(0.0, true);
    }

    pub fn new_game(&mut self) {
        self.score = 0;
        self.balls_left = 3;
        self.reset_ball();
    }

    /// Advance one fixed physics step.
    pub fn step(&mut self) {
        let dt = self.world.integration_parameters.dt;

        for f in &mut self.flippers {
            let target = if f.pressed { FLIPPER_UP } else { FLIPPER_REST };
            let max = FLIPPER_SPEED * dt;
            f.angle += (target - f.angle).clamp(-max, max);
            // Kinematic body: Rapier derives the velocity from the pose change,
            // which is what gives the ball its "hit".
            self.world.bodies[f.body]
                .set_next_kinematic_position(Pose::new(f.pivot, f.angle * f.side));
        }

        self.world.step();

        let ball_pos = self.ball_pos();
        for k in &mut self.kickers {
            k.cooldown = (k.cooldown - dt).max(0.0);
            k.flash = (k.flash - dt).max(0.0);
            let touching = self
                .world
                .contact_pair(self.ball_collider, k.collider)
                .is_some_and(|c| c.has_any_active_contact());
            if !touching || k.cooldown > 0.0 {
                continue;
            }
            let (dir, kick) = match k.kind {
                KickerKind::Bumper { center, .. } => ((ball_pos - center).normalize(), BUMPER_KICK),
                KickerKind::Sling { normal, .. } => (normal, SLING_KICK),
            };
            // Active elements add energy: replace the normal velocity component.
            let body = &mut self.world.bodies[self.ball];
            let v = body.linvel();
            let tangential = v - dir * v.dot(dir);
            body.set_linvel(tangential * 0.8 + dir * kick, true);
            k.cooldown = KICK_COOLDOWN;
            k.flash = 0.12;
            self.score += k.score;
        }

        if ball_pos.y < -0.05 && self.balls_left > 0 {
            self.balls_left -= 1;
            if self.balls_left > 0 {
                self.reset_ball();
            }
        }
    }
}

fn ball_start() -> Vector {
    Vector::new((LANE_X + TABLE_W) / 2.0, 0.03 + BALL_R + 0.002)
}

fn make_flipper(world: &mut PhysicsWorld, pivot: Vector, side: f32) -> Flipper {
    // Tapered outline: hull of a large circle at the pivot and a small one at the tip.
    // The left flipper points along +x; the right one is mirrored to point along -x.
    let mut pts = Vec::new();
    for i in 0..16 {
        let t = i as f32 / 16.0 * std::f32::consts::TAU;
        let (s, c) = t.sin_cos();
        pts.push(Vector::new(c * FLIPPER_R_BASE, s * FLIPPER_R_BASE));
        pts.push(Vector::new(side * FLIPPER_LEN + c * FLIPPER_R_TIP, s * FLIPPER_R_TIP));
    }
    let collider = ColliderBuilder::convex_hull(&pts)
        .expect("flipper hull")
        .restitution(0.25)
        .friction(0.6);
    let outline = match collider.shape.as_convex_polygon() {
        Some(poly) => poly.points().to_vec(),
        None => pts,
    };
    let angle = FLIPPER_REST;
    let (body, _) = world.insert(
        RigidBodyBuilder::kinematic_position_based()
            .pose(Pose::new(pivot, angle * side))
            .ccd_enabled(true),
        collider,
    );
    Flipper { body, pivot, outline, side, angle, pressed: false }
}

fn wall_layout() -> Vec<Vec<Vector>> {
    let v = Vector::new;
    let mut outer = vec![v(0.0, 0.245), v(0.0, 0.80)];
    // Top arch, left to right, so the plunged ball is steered into the playfield.
    let (cx, cy, r) = (TABLE_W / 2.0, 0.80, TABLE_W / 2.0);
    for i in 1..32 {
        let t = std::f32::consts::PI * (1.0 - i as f32 / 32.0);
        outer.push(v(cx + r * t.cos(), cy + r * t.sin()));
    }
    outer.extend([v(TABLE_W, 0.80), v(TABLE_W, 0.03), v(LANE_X, 0.03)]);

    let lane_separator = vec![v(LANE_X, 0.0), v(LANE_X, 0.70)];

    // Inlane guides funnel the ball onto the flippers. They stop just short of
    // the pivots so the flipper base covers the joint.
    let left_guide = vec![v(0.0, 0.245), v(0.0, 0.22), v(0.135, 0.15)];
    let right_guide = vec![v(LANE_X, 0.245), v(LANE_X, 0.22), v(LANE_X - 0.135, 0.15)];

    vec![outer, lane_separator, left_guide, right_guide]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(t: &mut Table, secs: f32) {
        for _ in 0..(secs * PHYSICS_HZ) as usize {
            t.step();
        }
    }

    #[test]
    fn ball_rests_in_plunger_lane() {
        let mut t = Table::new();
        run(&mut t, 1.0);
        let p = t.ball_pos();
        assert!(p.x > LANE_X && p.y < 0.1, "ball left the lane: {p:?}");
    }

    #[test]
    fn full_plunge_reaches_playfield() {
        let mut t = Table::new();
        run(&mut t, 0.2);
        t.plunger(true, 1.0);
        t.plunger(false, 0.0);
        let mut max_y: f32 = 0.0;
        for _ in 0..(2.0 * PHYSICS_HZ) as usize {
            t.step();
            max_y = max_y.max(t.ball_pos().y);
        }
        let p = t.ball_pos();
        assert!(max_y > 0.9, "plunge too weak, max y {max_y}");
        assert!(p.x < LANE_X, "ball did not enter playfield: {p:?}");
    }

    #[test]
    fn flipper_launches_ball() {
        let mut t = Table::new();
        // Drop the ball onto the middle of the resting left flipper.
        let f = &t.flippers[0];
        let drop = Vector::new(f.pivot.x + 0.04, f.pivot.y + 0.03);
        t.world.bodies[t.ball].set_translation(drop, true);
        run(&mut t, 0.3);
        t.flippers[0].pressed = true;
        let mut max_vy: f32 = 0.0;
        for _ in 0..(0.1 * PHYSICS_HZ) as usize {
            t.step();
            max_vy = max_vy.max(t.world.bodies[t.ball].linvel().y);
        }
        assert!(max_vy > 1.5, "flipper hit too weak: vy {max_vy}");
    }
}
