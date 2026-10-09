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
}
