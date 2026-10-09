//! Static description of the table: pure data that physics and rendering build from.
//! Units are metres, +y points up the playfield (away from the player).
//! See DESIGN.md for the shot names and the sketch this follows.

use rapier2d::math::Vector;

pub const TABLE_W: f32 = 0.52;
pub const TABLE_H: f32 = 1.07;
pub const BALL_R: f32 = 0.0135;
/// Plunger lane separator; the playfield proper spans 0..LANE_X.
pub const LANE_X: f32 = 0.482;
/// Height of the Neuschwanstein upper level above the playfield.
pub const UPPER_HEIGHT: f32 = 0.065;

/// Physics layer: an index into [`Layout::layers`]. A ball only collides with
/// things on its own layer; it switches layer when it crosses a sensor at a
/// ramp entrance or exit.
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

/// The built-in Bavarian table's layers (see [`bavaria`]).
/// Zugspitzbahn (left ramp), returns to the right inlane.
pub const RAMP_L: Layer = Layer(1);
/// Bierzelt (centre ramp), returns to the left inlane.
pub const RAMP_C: Layer = Layer(2);
/// Steinerne Brücke (right ramp), leads up to the upper level.
pub const RAMP_R: Layer = Layer(3);
/// Neuschwanstein upper level.
pub const UPPER: Layer = Layer(4);

pub struct LayerDef {
    #[allow(dead_code)] // read by the rules engine, which is not built yet
    pub name: String,
    /// Height above the playfield. Ramps use their height profile instead.
    pub height: f32,
    /// Rolling resistance coefficient: while rolling on this layer the ball
    /// slows down by `rolling_resistance * gravity` (m/s²).
    pub rolling_resistance: f32,
    /// Area to shade when drawing, e.g. an upper level; may be empty.
    pub outline: Vec<Vector>,
}

/// Switches the rules can react to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Sw {
    /// Top lanes B-I-E-R, 0..4 from the left.
    Rollover(u8),
    Bumper(u8),
    /// 0 = left, 1 = right.
    Sling(u8),
    /// Kegelbahn drop targets, 0..5 from the bottom.
    Drop(u8),
    Spinner,
    LeftOrbit,
    RightOrbit,
    RampEnter(Layer),
    /// The ball reached the end of a ramp (for RAMP_R: arrived on the upper level).
    RampMade(Layer),
    Scoop,
    /// Standup target on the upper level.
    Castle,
    UpperExit,
    Kickback,
    Drain,
}

pub struct Wall {
    /// Name the rules refer to it by.
    #[allow(dead_code)] // read by the rules engine, which is not built yet
    pub id: Option<String>,
    pub layer: Layer,
    pub pts: Vec<Vector>,
    pub closed: bool,
    /// Bounce: the effective value is this times the ball's restitution.
    pub restitution: f32,
    /// Friction along the wall.
    pub friction: f32,
}

pub struct Ramp {
    pub layer: Layer,
    /// Densified centreline and the height above the playfield at each point.
    pub center: Vec<Vector>,
    pub heights: Vec<f32>,
    /// Side rails.
    pub left: Vec<Vector>,
    pub right: Vec<Vector>,
}

impl Ramp {
    /// Height and unit uphill-tangent slope (dh/ds along `tangent`) at the
    /// centreline point nearest to `p`.
    pub fn sample(&self, p: Vector) -> (f32, Vector, f32) {
        let mut best = (f32::MAX, 0.0, Vector::Y, 0.0);
        for i in 0..self.center.len() - 1 {
            let (a, b) = (self.center[i], self.center[i + 1]);
            let ab = b - a;
            let len = ab.length();
            let t = ((p - a).dot(ab) / (len * len)).clamp(0.0, 1.0);
            let d = (a + ab * t).distance_squared(p);
            if d < best.0 {
                let (ha, hb) = (self.heights[i], self.heights[i + 1]);
                best = (d, ha + (hb - ha) * t, ab / len, (hb - ha) / len);
            }
        }
        (best.1, best.2, best.3)
    }
}

#[derive(Clone, Copy)]
pub enum Shape {
    Rect { min: Vector, max: Vector },
    Circle { c: Vector, r: f32 },
}

/// Moves a ball between layers. Crossing along `dir` puts it on `fwd`,
/// crossing against it puts it on `back`.
#[derive(Clone, Copy)]
pub struct LayerSwitch {
    pub dir: Vector,
    pub fwd: Layer,
    pub back: Layer,
}

pub struct Sensor {
    pub shape: Shape,
    /// Layers whose balls trigger it.
    pub layers: u32,
    pub sw: Option<Sw>,
    pub switch: Option<LayerSwitch>,
}

pub struct Bumper {
    pub center: Vector,
    pub radius: f32,
}

/// Slingshot triangle; the a-b face kicks.
pub struct Sling {
    pub a: Vector,
    pub b: Vector,
    pub c: Vector,
    /// +1 left, -1 right.
    pub side: f32,
}

impl Sling {
    pub fn normal(&self) -> Vector {
        let face = (self.b - self.a).normalize();
        Vector::new(-face.y, face.x) * self.side
    }
}

pub struct DropTarget {
    pub center: Vector,
    pub half: Vector,
}

/// Solid target that only reports a switch.
pub struct Standup {
    pub a: Vector,
    pub b: Vector,
    pub layer: Layer,
    pub sw: Sw,
}

/// One-way gate: lets balls through when moving along `pass`.
pub struct Gate {
    pub a: Vector,
    pub b: Vector,
    pub pass: Vector,
    pub layer: Layer,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Left,
    Right,
}

#[derive(Clone)]
pub struct FlipperDef {
    #[allow(dead_code)] // read by the rules engine, which is not built yet
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

/// Classic tapered flipper: a circle at the pivot and a smaller one at the tip,
/// pointing along +x.
pub fn tapered_outline(len: f32, r_base: f32, r_tip: f32) -> Vec<Vector> {
    let mut pts = Vec::new();
    for i in 0..16 {
        let t = i as f32 / 16.0 * std::f32::consts::TAU;
        let (s, c) = t.sin_cos();
        pts.push(Vector::new(c * r_base, s * r_base));
        pts.push(Vector::new(len + c * r_tip, s * r_tip));
    }
    pts
}

pub struct BallDef {
    pub radius: f32,
    /// kg; only matters when balls hit each other.
    pub mass: f32,
    /// Multiplies every surface's restitution: lower is a "softer" ball.
    pub restitution: f32,
}

/// How a new ball enters play: it waits at `position` until the launch key
/// shoots it off.
pub struct Launcher {
    pub position: Vector,
    pub start_velocity: Vector,
    /// Plunger: holding the key longer shoots harder (30% to 100% of
    /// `start_velocity`). Otherwise the key fires at exactly `start_velocity`.
    pub charged: bool,
}

/// Scoop: captures the ball, the rules decide when to eject it.
pub struct Hole {
    pub center: Vector,
    pub radius: f32,
    pub eject: Vector,
    pub sw: Sw,
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
    pub ramps: Vec<Ramp>,
    pub sensors: Vec<Sensor>,
    pub bumpers: Vec<Bumper>,
    pub slings: Vec<Sling>,
    pub drops: Vec<DropTarget>,
    pub standups: Vec<Standup>,
    pub gates: Vec<Gate>,
    pub flippers: Vec<FlipperDef>,
    pub holes: Vec<Hole>,
}

fn layer(name: &str, height: f32, outline: Vec<Vector>) -> LayerDef {
    LayerDef { name: name.into(), height, rolling_resistance: 0.01, outline }
}

fn v(x: f32, y: f32) -> Vector {
    Vector::new(x, y)
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Shape {
    Shape::Rect { min: v(x0, y0), max: v(x1, y1) }
}

/// Mirror an x coordinate across the playfield centre.
fn mx(x: f32) -> f32 {
    LANE_X - x
}

/// Sloped roof over the shot line: the ramp channels end here on the playfield
/// layer, and balls coming out of the pop bumpers roll off it to the right.
const ROOF: (Vector, Vector) = (Vector::new(0.062, 0.76), Vector::new(0.365, 0.70));

fn roof_y(x: f32) -> f32 {
    let (a, b) = ROOF;
    a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x)
}

/// Builds a ramp from control points (x, y, height), smoothed with Catmull-Rom.
fn ramp(layer: Layer, ctrl: &[(f32, f32, f32)], width: f32) -> Ramp {
    let p: Vec<Vector> = ctrl.iter().map(|&(x, y, _)| v(x, y)).collect();
    let h: Vec<f32> = ctrl.iter().map(|c| c.2).collect();
    let n = p.len();
    let mut center = Vec::new();
    let mut heights = Vec::new();
    const SUB: usize = 8;
    for i in 0..n - 1 {
        let p0 = p[i.saturating_sub(1)];
        let (p1, p2) = (p[i], p[i + 1]);
        let p3 = p[(i + 2).min(n - 1)];
        for s in 0..SUB {
            let t = s as f32 / SUB as f32;
            let (t2, t3) = (t * t, t * t * t);
            let q = 0.5
                * ((2.0 * p1)
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3);
            center.push(q);
            heights.push(h[i] + (h[i + 1] - h[i]) * t);
        }
    }
    center.push(p[n - 1]);
    heights.push(h[n - 1]);

    let m = center.len();
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for k in 0..m {
        let t = (center[(k + 1).min(m - 1)] - center[k.saturating_sub(1)]).normalize();
        let nrm = v(-t.y, t.x) * (width / 2.0);
        left.push(center[k] + nrm);
        right.push(center[k] - nrm);
    }
    Ramp { layer, center, heights, left, right }
}

/// The Bavarian table. Coordinates are worked out in DESIGN.md's sketch order:
/// bottom (flippers, slings, lanes), shot line, top (pops, rollovers), ramps, upper level.
pub fn bavaria() -> Layout {
    let pf = Layer::PLAYFIELD;
    let mut walls = Vec::new();
    let mut wall = |layer, pts: Vec<Vector>, closed| {
        // Ramp rails absorb hits; playfield walls are livelier.
        let restitution = if layer == pf { 0.35 } else { 0.1 };
        walls.push(Wall { id: None, layer, pts, closed, restitution, friction: 0.075 })
    };

    // Cabinet: left side, top arch, right side down the plunger lane.
    let mut outer = vec![v(0.0, 0.0), v(0.0, 0.81)];
    let (acx, acy, ar) = (TABLE_W / 2.0, 0.81, TABLE_W / 2.0);
    for i in 1..40 {
        let t = std::f32::consts::PI * (1.0 - i as f32 / 40.0);
        outer.push(v(acx + ar * t.cos(), acy + ar * t.sin()));
    }
    outer.extend([v(TABLE_W, 0.81), v(TABLE_W, 0.03), v(LANE_X, 0.03)]);
    wall(pf, outer, false);
    wall(pf, vec![v(LANE_X, 0.0), v(LANE_X, 0.80)], false);

    // Inlane/outlane dividers that continue into the flipper guides.
    wall(pf, vec![v(0.040, 0.33), v(0.040, 0.24), v(0.142, 0.168)], false);
    wall(pf, vec![v(mx(0.040), 0.33), v(mx(0.040), 0.24), v(mx(0.142), 0.168)], false);

    // Orbit inner walls. The left one is thickened into the left ramp channel's wall.
    wall(pf, vec![v(0.04, 0.82), v(0.04, 0.56), v(0.062, 0.58), v(0.062, roof_y(0.062)), v(0.05, 0.82)], true);
    wall(pf, vec![v(0.44, 0.56), v(0.44, 0.82)], false);

    // Shot line (block bottoms are pointed so the entrances funnel): roof, the block holding the Rathaus scoop, the block between
    // centre and right ramp, and the right ramp's right wall.
    wall(pf, vec![ROOF.0, ROOF.1], false);
    wall(
        pf,
        vec![
            v(0.117, 0.625),
            v(0.131, 0.60),
            v(0.145, 0.625),
            v(0.145, 0.69),
            v(0.193, 0.69),
            v(0.193, 0.625),
            v(0.204, 0.60),
            v(0.215, 0.625),
            v(0.215, roof_y(0.215)),
            v(0.117, roof_y(0.117)),
        ],
        true,
    );
    wall(pf, vec![v(0.27, 0.625), v(0.285, 0.60), v(0.30, 0.625), v(0.30, roof_y(0.30)), v(0.27, roof_y(0.27))], true);
    wall(pf, vec![v(0.37, 0.60), v(0.355, 0.625), v(0.355, roof_y(0.355))], false);

    // Top lane posts (B-I-E-R).
    let lane_xs = [0.09, 0.13, 0.17, 0.21, 0.25];
    for x in lane_xs {
        wall(pf, vec![v(x, 0.885), v(x, 0.925)], false);
    }

    // Ramps. Each starts in its channel in the shot line.
    let ramps = vec![
        ramp(
            RAMP_L,
            &[
                (0.0895, 0.625, 0.0),
                (0.0895, 0.70, 0.015),
                (0.10, 0.80, 0.04),
                (0.15, 0.875, 0.06),
                (0.24, 0.905, 0.07),
                (0.33, 0.89, 0.07),
                (0.39, 0.84, 0.065),
                (0.42, 0.76, 0.055),
                (0.42, 0.55, 0.03),
                (0.42, 0.37, 0.01),
            ],
            0.055,
        ),
        ramp(
            RAMP_C,
            &[
                (0.2425, 0.625, 0.0),
                (0.2425, 0.70, 0.015),
                (0.245, 0.80, 0.04),
                (0.225, 0.88, 0.06),
                (0.16, 0.925, 0.07),
                (0.09, 0.91, 0.07),
                (0.045, 0.85, 0.065),
                (0.04, 0.75, 0.055),
                (0.055, 0.55, 0.03),
                (0.0625, 0.37, 0.01),
            ],
            0.055,
        ),
        ramp(
            RAMP_R,
            &[
                (0.3275, 0.625, 0.0),
                (0.3275, 0.68, 0.012),
                (0.345, 0.74, 0.025),
                (0.385, 0.80, 0.038),
                (0.43, 0.86, 0.05),
                (0.45, 0.91, 0.06),
                (0.435, 0.955, UPPER_HEIGHT),
                (0.39, 0.965, UPPER_HEIGHT),
            ],
            0.055,
        ),
    ];
    for r in &ramps {
        wall(r.layer, r.left.clone(), false);
        wall(r.layer, r.right.clone(), false);
    }

    // Neuschwanstein upper level. The right ramp climbs past it and drops the
    // ball in at the top right; it falls onto the upper flipper, and balls that
    // get past the flipper drop back to the playfield (UpperExit).
    wall(UPPER, vec![v(0.4175, 0.80), v(0.4175, 0.928)], false);
    wall(
        UPPER,
        vec![v(0.405, 0.99), v(0.34, 0.995), v(0.30, 0.975), v(0.285, 0.94), v(0.285, 0.885), v(0.30, 0.872)],
        false,
    );
    let upper_outline =
        vec![v(0.285, 0.80), v(0.4175, 0.80), v(0.4175, 0.99), v(0.34, 0.995), v(0.30, 0.975), v(0.285, 0.94)];

    let both = |a: Layer, b: Layer| a.bit() | b.bit();
    let ramp_entry = |layer: Layer, x0: f32, x1: f32| Sensor {
        shape: rect(x0, 0.63, x1, 0.64),
        layers: both(pf, layer),
        sw: Some(Sw::RampEnter(layer)),
        switch: Some(LayerSwitch { dir: Vector::Y, fwd: layer, back: pf }),
    };
    let ramp_exit = |layer: Layer, x: f32| Sensor {
        shape: rect(x - 0.025, 0.37, x + 0.025, 0.385),
        layers: layer.bit(),
        sw: Some(Sw::RampMade(layer)),
        switch: Some(LayerSwitch { dir: -Vector::Y, fwd: pf, back: layer }),
    };
    let mut sensors = vec![
        ramp_entry(RAMP_L, 0.062, 0.117),
        ramp_entry(RAMP_C, 0.215, 0.27),
        ramp_entry(RAMP_R, 0.30, 0.355),
        ramp_exit(RAMP_L, 0.42),
        ramp_exit(RAMP_C, 0.0625),
        Sensor {
            shape: rect(0.395, 0.93, 0.405, 0.99),
            layers: both(RAMP_R, UPPER),
            sw: Some(Sw::RampMade(RAMP_R)),
            switch: Some(LayerSwitch { dir: -Vector::X, fwd: UPPER, back: RAMP_R }),
        },
        Sensor {
            shape: rect(0.29, 0.78, 0.4175, 0.80),
            layers: UPPER.bit(),
            sw: Some(Sw::UpperExit),
            switch: Some(LayerSwitch { dir: -Vector::Y, fwd: pf, back: pf }),
        },
        Sensor { shape: rect(0.0, 0.62, 0.04, 0.63), layers: pf.bit(), sw: Some(Sw::Spinner), switch: None },
        Sensor { shape: rect(0.0, 0.70, 0.04, 0.71), layers: pf.bit(), sw: Some(Sw::LeftOrbit), switch: None },
        Sensor { shape: rect(0.44, 0.70, LANE_X, 0.71), layers: pf.bit(), sw: Some(Sw::RightOrbit), switch: None },
        Sensor { shape: rect(0.005, 0.10, 0.035, 0.13), layers: pf.bit(), sw: Some(Sw::Kickback), switch: None },
    ];
    for i in 0..4 {
        let c = v((lane_xs[i] + lane_xs[i + 1]) / 2.0, 0.905);
        sensors.push(Sensor {
            shape: Shape::Circle { c, r: 0.008 },
            layers: pf.bit(),
            sw: Some(Sw::Rollover(i as u8)),
            switch: None,
        });
    }

    let bumpers = [(0.10, 0.84), (0.165, 0.795), (0.235, 0.84)]
        .map(|(x, y)| Bumper { center: v(x, y), radius: 0.025 })
        .into();

    let sling = |side: f32| {
        let x = |x: f32| if side > 0.0 { x } else { mx(x) };
        Sling { a: v(x(0.085), 0.335), b: v(x(0.135), 0.215), c: v(x(0.085), 0.255), side }
    };

    // Kegelbahn: five drop targets on the left wall.
    let drops = (0..5)
        .map(|i| DropTarget { center: v(0.006, 0.40 + 0.02 * i as f32), half: v(0.006, 0.008) })
        .collect();

    let flipper_len = 0.075;
    let rest: f32 = -0.52;
    // Tip edges ~35mm apart, so a dead ball drains between the flippers.
    let pivot_dx = 0.0235 + flipper_len * rest.cos();
    let flipper = |id: &str, pivot, rest: f32, up: f32, outline, layer, button| FlipperDef {
        id: Some(id.into()),
        pivot,
        outline,
        rest,
        up,
        speed: 55.0, // full stroke in ~19ms
        restitution: 0.25,
        friction: 0.35,
        layer,
        button,
    };
    let main_outline = tapered_outline(flipper_len, 0.011, 0.006);
    let right_x = LANE_X / 2.0 + pivot_dx;
    let left_x = LANE_X / 2.0 - pivot_dx;
    let pi = std::f32::consts::PI;

    Layout {
        name: "Bavaria".into(),
        size: v(TABLE_W, TABLE_H),
        // 2.25x real gravity on a real 6.5° tilt: plays like a real table sped up 1.5x.
        angle: 6.5f32.to_radians(),
        gravity: 22.1,
        ball: BallDef { radius: BALL_R, mass: 0.08, restitution: 0.9 },
        layers: vec![
            layer("playfield", 0.0, Vec::new()),
            layer("zugspitzbahn", 0.0, Vec::new()),
            layer("bierzelt", 0.0, Vec::new()),
            layer("steinerne_bruecke", 0.0, Vec::new()),
            layer("neuschwanstein", UPPER_HEIGHT, upper_outline),
        ],
        launcher: Launcher {
            position: v((LANE_X + TABLE_W) / 2.0, 0.03 + BALL_R + 0.002),
            start_velocity: v(0.0, 4.3),
            charged: true,
        },
        walls,
        ramps,
        sensors,
        bumpers,
        slings: vec![sling(1.0), sling(-1.0)],
        drops,
        standups: vec![Standup { a: v(0.35, 0.992), b: v(0.39, 0.987), layer: UPPER, sw: Sw::Castle }],
        gates: vec![
            // Top of the plunger lane: out only.
            // Slanted so orbit balls coming down the outer wall roll into the right orbit.
            Gate { a: v(LANE_X, 0.80), b: v(TABLE_W, 0.85), pass: Vector::Y, layer: pf },
            // Orbit exits: balls going up pass, balls coming down are steered to the inlanes.
            Gate { a: v(0.0, 0.53), b: v(0.06, 0.48), pass: Vector::Y, layer: pf },
            Gate { a: v(LANE_X, 0.53), b: v(mx(0.06), 0.48), pass: Vector::Y, layer: pf },
        ],
        flippers: vec![
            flipper("left_flipper", v(left_x, 0.15), rest, -rest, main_outline.clone(), pf, Button::Left),
            // Mirrored: points down-left at rest (180° + 30°).
            flipper("right_flipper", v(right_x, 0.15), pi - rest, pi + rest, main_outline, pf, Button::Right),
            flipper("upper_flipper", v(0.31, 0.865), -0.5, 0.5, tapered_outline(0.05, 0.009, 0.005), UPPER, Button::Left),
        ],
        holes: vec![Hole { center: v(0.169, 0.672), radius: 0.012, eject: v(0.05, -0.6), sw: Sw::Scoop }],
    }
}
