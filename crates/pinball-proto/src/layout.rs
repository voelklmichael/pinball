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

/// Physics layer. A ball only collides with things on its own layer; it switches
/// layer when it crosses a sensor at a ramp entrance or exit.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Layer {
    Playfield,
    /// Zugspitzbahn (left ramp), returns to the right inlane.
    RampL,
    /// Bierzelt (centre ramp), returns to the left inlane.
    RampC,
    /// Steinerne Brücke (right ramp), leads up to the upper level.
    RampR,
    /// Neuschwanstein upper level.
    Upper,
}

impl Layer {
    pub fn bit(self) -> u32 {
        1 << self as u32
    }
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
    /// The ball reached the end of a ramp (for RampR: arrived on the upper level).
    RampMade(Layer),
    Scoop,
    /// Standup target on the upper level.
    Castle,
    UpperExit,
    Kickback,
    Drain,
}

pub struct Wall {
    pub layer: Layer,
    pub pts: Vec<Vector>,
    pub closed: bool,
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

#[derive(Clone, Copy)]
pub struct FlipperDef {
    pub pivot: Vector,
    pub len: f32,
    pub r_base: f32,
    pub r_tip: f32,
    /// +1 points right (left flipper), -1 points left (right flipper).
    pub side: f32,
    /// Angles above horizontal, mirrored by `side`.
    pub rest: f32,
    pub up: f32,
    pub layer: Layer,
    pub button: Button,
}

/// Scoop: captures the ball, the rules decide when to eject it.
pub struct Hole {
    pub center: Vector,
    pub radius: f32,
    pub eject: Vector,
    pub sw: Sw,
}

pub struct Layout {
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
    pub plunger_rest: Vector,
    /// Outline of the upper level, for drawing.
    pub upper_outline: Vec<Vector>,
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
    use Layer::*;
    let pf = Playfield;
    let mut walls = Vec::new();
    let mut wall = |layer, pts: Vec<Vector>, closed| walls.push(Wall { layer, pts, closed });

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
        wall(pf, vec![v(x, 0.91), v(x, 0.955)], false);
    }

    // Ramps. Each starts in its channel in the shot line.
    let ramps = vec![
        ramp(
            RampL,
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
            RampC,
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
            RampR,
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
    wall(Upper, vec![v(0.4175, 0.80), v(0.4175, 0.928)], false);
    wall(
        Upper,
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
        ramp_entry(RampL, 0.062, 0.117),
        ramp_entry(RampC, 0.215, 0.27),
        ramp_entry(RampR, 0.30, 0.355),
        ramp_exit(RampL, 0.42),
        ramp_exit(RampC, 0.0625),
        Sensor {
            shape: rect(0.395, 0.93, 0.405, 0.99),
            layers: both(RampR, Upper),
            sw: Some(Sw::RampMade(RampR)),
            switch: Some(LayerSwitch { dir: -Vector::X, fwd: Upper, back: RampR }),
        },
        Sensor {
            shape: rect(0.29, 0.78, 0.4175, 0.80),
            layers: Upper.bit(),
            sw: Some(Sw::UpperExit),
            switch: Some(LayerSwitch { dir: -Vector::Y, fwd: pf, back: pf }),
        },
        Sensor { shape: rect(0.0, 0.62, 0.04, 0.63), layers: pf.bit(), sw: Some(Sw::Spinner), switch: None },
        Sensor { shape: rect(0.0, 0.70, 0.04, 0.71), layers: pf.bit(), sw: Some(Sw::LeftOrbit), switch: None },
        Sensor { shape: rect(0.44, 0.70, LANE_X, 0.71), layers: pf.bit(), sw: Some(Sw::RightOrbit), switch: None },
        Sensor { shape: rect(0.005, 0.10, 0.035, 0.13), layers: pf.bit(), sw: Some(Sw::Kickback), switch: None },
    ];
    for i in 0..4 {
        let c = v((lane_xs[i] + lane_xs[i + 1]) / 2.0, 0.93);
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
    let main_flipper = |side: f32, button| FlipperDef {
        pivot: v(LANE_X / 2.0 - side * pivot_dx, 0.15),
        len: flipper_len,
        r_base: 0.011,
        r_tip: 0.006,
        side,
        rest,
        up: 0.52,
        layer: pf,
        button,
    };

    Layout {
        walls,
        ramps,
        sensors,
        bumpers,
        slings: vec![sling(1.0), sling(-1.0)],
        drops,
        standups: vec![Standup { a: v(0.35, 0.992), b: v(0.39, 0.987), layer: Upper, sw: Sw::Castle }],
        gates: vec![
            // Top of the plunger lane: out only.
            // Slanted so orbit balls coming down the outer wall roll into the right orbit.
            Gate { a: v(LANE_X, 0.80), b: v(TABLE_W, 0.85), pass: Vector::Y, layer: pf },
            // Orbit exits: balls going up pass, balls coming down are steered to the inlanes.
            Gate { a: v(0.0, 0.53), b: v(0.06, 0.48), pass: Vector::Y, layer: pf },
            Gate { a: v(LANE_X, 0.53), b: v(mx(0.06), 0.48), pass: Vector::Y, layer: pf },
        ],
        flippers: vec![
            main_flipper(1.0, Button::Left),
            main_flipper(-1.0, Button::Right),
            FlipperDef {
                pivot: v(0.31, 0.865),
                len: 0.05,
                r_base: 0.009,
                r_tip: 0.005,
                side: 1.0,
                rest: -0.5,
                up: 0.5,
                layer: Upper,
                button: Button::Left,
            },
        ],
        holes: vec![Hole { center: v(0.169, 0.672), radius: 0.012, eject: v(0.05, -0.6), sw: Sw::Scoop }],
        plunger_rest: v((LANE_X + TABLE_W) / 2.0, 0.03 + BALL_R + 0.002),
        upper_outline,
    }
}
