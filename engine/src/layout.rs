//! The table model that physics and rendering build from. Tables are loaded
//! from JSON files (see `tablefile` and `tables/README.md`).
//! Units are metres, +y points up the playfield (away from the player).

use rapier2d::math::Vector;

/// Physics layer: an index into [`Layout::layers`]. A ball only collides with
/// things on its own layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Layer(pub u8);

impl Layer {
    /// The first layer; new balls start here.
    pub const PLAYFIELD: Layer = Layer(0);
    /// Collision groups are 32-bit masks.
    pub const MAX: usize = 32;

    pub fn bit(self) -> u32 {
        1 << self.0
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

pub struct LayerDef {
    pub name: String,
    /// Rolling resistance coefficient: while rolling on this layer the ball
    /// slows down by `rolling_resistance * gravity` (m/s²).
    pub rolling_resistance: f32,
    /// Height above the playfield, for drawing. Ramps use their profile instead.
    pub height: f32,
    pub ramp: Option<Ramp>,
}

/// A ramp layer's track: a centreline with a height profile and its rails.
pub struct Ramp {
    /// Densely sampled centreline, from the entrance to the exit.
    pub center: Vec<Vector>,
    /// Height above the playfield at each centreline point.
    pub heights: Vec<f32>,
    /// Sideways tilt at each centreline point, in radians; positive pushes the
    /// ball toward the right rail (right as seen going up the ramp).
    pub banks: Vec<f32>,
    /// The track's edges (where the rails are, if it has them).
    pub left: Vec<Vector>,
    pub right: Vec<Vector>,
}

impl Ramp {
    /// Height, uphill unit tangent, slope (dh/ds along the tangent) and bank
    /// at the centreline point nearest to `p`.
    pub fn sample(&self, p: Vector) -> RampPoint {
        let mut best = (f32::MAX, RampPoint { height: 0.0, tangent: Vector::Y, slope: 0.0, bank: 0.0 });
        for i in 0..self.center.len() - 1 {
            let (a, b) = (self.center[i], self.center[i + 1]);
            let ab = b - a;
            let len = ab.length();
            if len < 1e-9 {
                continue;
            }
            let t = ((p - a).dot(ab) / (len * len)).clamp(0.0, 1.0);
            let d = (a + ab * t).distance_squared(p);
            if d < best.0 {
                let (ha, hb) = (self.heights[i], self.heights[i + 1]);
                let (ba, bb) = (self.banks[i], self.banks[i + 1]);
                let point =
                    RampPoint { height: ha + (hb - ha) * t, tangent: ab / len, slope: (hb - ha) / len, bank: ba + (bb - ba) * t };
                best = (d, point);
            }
        }
        best.1
    }
}

pub struct RampPoint {
    pub height: f32,
    /// Unit tangent pointing from the entrance toward the exit.
    pub tangent: Vector,
    /// Height change per metre along `tangent`.
    pub slope: f32,
    /// Sideways tilt in radians, positive toward the right rail.
    pub bank: f32,
}

pub struct Wall {
    /// Name the rules refer to it by.
    pub id: Option<String>,
    pub layer: Layer,
    pub pts: Vec<Vector>,
    pub closed: bool,
    /// Bounce: the effective value is this times the ball's restitution.
    pub restitution: f32,
    /// Friction along the wall.
    pub friction: f32,
    /// Active element (pop bumper, slingshot): on contact the ball is pushed
    /// away from the wall at this speed (m/s); 0 for a plain wall.
    pub kick: f32,
    /// One-way gate: balls moving in this direction pass through.
    pub one_way: Option<Vector>,
    /// Balls are only ever on the right of the point order (ramp rails).
    /// Lets the physics treat the joints between segments correctly, so a
    /// ball rolling along the wall isn't bounced off them.
    pub one_sided: bool,
}

/// Where a switch detects the ball (its centre's shape is irrelevant: the
/// switch fires as soon as the ball overlaps it).
#[derive(Clone, Copy)]
pub enum SensorShape {
    Circle { center: Vector, radius: f32 },
    Rect { min: Vector, max: Vector },
    Segment { a: Vector, b: Vector },
}

/// Doesn't block the ball; reports when a ball rolls over it (rollover lanes,
/// spinners, optos).
pub struct Switch {
    pub id: Option<String>,
    pub layer: Layer,
    pub shape: SensorShape,
}

/// A line that moves balls between layers. A ball on `layer` that crosses
/// from `a`-`b` in `direction` ends up on `target`. If `two_way`, a ball on
/// `target` crossing back returns to `layer` (ramp entrances, where a weak
/// shot rolls back); otherwise only balls on `layer` are affected (ramp exits,
/// holes).
pub struct Transition {
    pub id: Option<String>,
    pub layer: Layer,
    pub a: Vector,
    pub b: Vector,
    pub direction: Vector,
    pub target: Layer,
    pub two_way: bool,
}

/// Stops balls until hit hard enough from the front, then drops out of the way
/// until it's reset.
pub struct DropTarget {
    pub id: Option<String>,
    pub layer: Layer,
    pub a: Vector,
    pub b: Vector,
    /// Unit vector the front faces: balls must come from this side.
    pub facing: Vector,
    /// Minimum speed into the front (m/s) that drops it.
    pub min_speed: f32,
    pub restitution: f32,
}

/// Scoop, saucer or ball lock: captures balls that roll in and ejects them,
/// either automatically after `eject_after` seconds or when told to.
pub struct Hole {
    pub id: Option<String>,
    pub layer: Layer,
    pub center: Vector,
    pub radius: f32,
    /// Velocity a ball leaves with.
    pub eject: Vector,
    pub eject_after: Option<f32>,
    /// Faster balls roll over it (saucers); `f32::MAX` captures everything.
    pub max_speed: f32,
}

/// A flap on a line that balls pass through; it spins depending on their speed.
pub struct Spinner {
    pub id: Option<String>,
    pub layer: Layer,
    pub a: Vector,
    pub b: Vector,
    /// Spins per m/s of the ball's speed across the line.
    pub spins_per_mps: f32,
}

impl Transition {
    /// True if `p` is on the `target` side of the line.
    pub fn beyond(&self, p: Vector) -> bool {
        (p - self.a).dot(self.direction) > 0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Left,
    Right,
}

#[derive(Clone)]
pub struct FlipperDef {
    pub id: Option<String>,
    pub pivot: Vector,
    /// Outline in local coordinates: pivot at the origin, pointing along +x.
    /// The flipper body is the convex hull of these points.
    pub outline: Vec<Vector>,
    /// World angles (radians, counter-clockwise from +x) at rest and fully up.
    pub rest: f32,
    pub up: f32,
    /// Angular speed in rad/s.
    pub speed: f32,
    pub restitution: f32,
    pub friction: f32,
    pub layer: Layer,
    pub button: Button,
}

pub struct BallDef {
    pub radius: f32,
    /// kg; only matters when balls hit each other.
    pub mass: f32,
    /// Multiplies every surface's restitution: lower is a "softer" ball.
    pub restitution: f32,
}

/// How a new ball enters play: it waits at `position` until the launch key
/// shoots it off at `start_velocity`.
pub struct Launcher {
    pub position: Vector,
    pub start_velocity: Vector,
}

pub struct Layout {
    pub name: String,
    /// Name of the rules this table is played with (see the `rules` crate).
    pub rules: Option<String>,
    /// Width and height of the table; the playfield spans (0,0) to `size`.
    pub size: Vector,
    /// Table tilt in radians.
    pub angle: f32,
    /// Full gravity; the ball feels `gravity * sin(angle)` along the playfield.
    pub gravity: f32,
    pub ball: BallDef,
    pub launcher: Launcher,
    /// At most [`Layer::MAX`]; the first is the playfield.
    pub layers: Vec<LayerDef>,
    pub walls: Vec<Wall>,
    pub flippers: Vec<FlipperDef>,
    pub switches: Vec<Switch>,
    pub transitions: Vec<Transition>,
    pub drop_targets: Vec<DropTarget>,
    pub holes: Vec<Hole>,
    pub spinners: Vec<Spinner>,
}

impl Layout {
    pub fn layer_name(&self, layer: Layer) -> &str {
        &self.layers[layer.index()].name
    }
}
