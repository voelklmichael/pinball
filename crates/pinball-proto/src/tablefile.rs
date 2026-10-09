//! JSON table files. The format is described in `tables/README.md`.
//!
//! Units are metres, kilograms and degrees. The origin is the bottom-left
//! corner of the table, +x points right, +y points up the playfield (away from
//! the player). Angles are counter-clockwise from +x.
//!
//! Every curve is turned into short straight segments for the physics.

use std::f32::consts::TAU;

use rapier2d::math::Vector;
use serde::Deserialize;

use crate::layout::{BallDef, Button, FlipperDef, Launch, Layer, Layout, Wall};

type P = [f32; 2];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableFile {
    #[serde(default)]
    pub name: String,
    /// Width and height.
    pub size: P,
    /// Tilt of the table.
    pub angle_deg: f32,
    /// Full gravity in m/s²; the ball feels `gravity * sin(angle)` along the table.
    #[serde(default = "default_gravity")]
    pub gravity: f32,
    pub ball: BallSpec,
    pub inbound: Inbound,
    pub elements: Vec<Element>,
}

fn default_gravity() -> f32 {
    9.81
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BallSpec {
    pub radius: f32,
    #[serde(default = "default_ball_mass")]
    pub mass: f32,
    /// "Softness": scales every bounce. 1 = as hard as the surfaces allow.
    #[serde(default = "default_ball_restitution")]
    pub restitution: f32,
}

fn default_ball_mass() -> f32 {
    0.08
}

fn default_ball_restitution() -> f32 {
    0.9
}

/// A new ball appears at `position` and is shot off at `velocity` (m/s)
/// when the launch key is pressed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inbound {
    pub position: P,
    pub velocity: P,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Line(Line),
    Arc(Arc),
    Bezier(Bezier),
    Polyline(Polyline),
    Flipper(Flipper),
}

/// The curves a flipper outline is made of: walls without physics settings.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Curve {
    Line(Line),
    Arc(Arc),
    Bezier(Bezier),
    Polyline(Polyline),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub from: P,
    pub to: P,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
}

/// Circular or elliptic arc from `start_deg` to `end_deg` (counter-clockwise if
/// end > start). A sweep of 360° or more makes a closed circle/ellipse.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arc {
    pub center: P,
    pub radius: Radius,
    pub start_deg: f32,
    pub end_deg: f32,
    /// Rotates an ellipse's axes.
    #[serde(default)]
    pub rotation_deg: f32,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Radius {
    Circle(f32),
    /// [x radius, y radius]
    Ellipse(P),
}

/// Chain of cubic Bézier segments: start, control, control, end, control,
/// control, end, … (3n+1 points).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bezier {
    pub points: Vec<P>,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Polyline {
    pub points: Vec<P>,
    #[serde(default)]
    pub closed: bool,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flipper {
    pub pivot: P,
    /// Angle when the key is released / held.
    pub rest_deg: f32,
    pub up_deg: f32,
    pub key: Key,
    #[serde(default = "default_flipper_speed")]
    pub speed_deg_per_s: f32,
    #[serde(default = "default_flipper_restitution")]
    pub restitution: f32,
    #[serde(default = "default_flipper_friction")]
    pub friction: f32,
    /// Outline in flipper coordinates: pivot at (0, 0), pointing along +x.
    /// The flipper is the convex hull of the outline.
    pub shape: Vec<Curve>,
}

fn default_flipper_speed() -> f32 {
    3150.0 // ~19ms for a 60° stroke
}

fn default_flipper_restitution() -> f32 {
    0.25
}

fn default_flipper_friction() -> f32 {
    0.35
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    Left,
    Right,
}

const WALL_RESTITUTION: f32 = 0.35;
const WALL_FRICTION: f32 = 0.075;

pub fn load(path: &str) -> Result<Layout, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    parse(&text).map_err(|e| format!("{path}: {e}"))
}

pub fn parse(json: &str) -> Result<Layout, String> {
    let file: TableFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    file.into_layout()
}

fn v(p: P) -> Vector {
    Vector::new(p[0], p[1])
}

fn check(ok: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what()) }
}

fn check_coefficient(name: &str, value: f32) -> Result<(), String> {
    check((0.0..=1.0).contains(&value), || format!("{name} must be between 0 and 1, got {value}"))
}

/// What every wall curve can do: report its settings and turn into points.
trait WallCurve {
    fn kind(&self) -> &'static str;
    fn physics(&self) -> (Option<f32>, Option<f32>);
    /// Points along the curve; `true` if it is closed.
    fn sample(&self) -> Result<(Vec<Vector>, bool), String>;
}

impl Element {
    fn kind(&self) -> &'static str {
        self.wall().map_or("flipper", |w| w.kind())
    }

    fn wall(&self) -> Option<&dyn WallCurve> {
        match self {
            Element::Line(c) => Some(c),
            Element::Arc(c) => Some(c),
            Element::Bezier(c) => Some(c),
            Element::Polyline(c) => Some(c),
            Element::Flipper(_) => None,
        }
    }
}

impl Curve {
    fn wall(&self) -> &dyn WallCurve {
        match self {
            Curve::Line(c) => c,
            Curve::Arc(c) => c,
            Curve::Bezier(c) => c,
            Curve::Polyline(c) => c,
        }
    }
}

impl WallCurve for Line {
    fn kind(&self) -> &'static str {
        "line"
    }
    fn physics(&self) -> (Option<f32>, Option<f32>) {
        (self.restitution, self.friction)
    }
    fn sample(&self) -> Result<(Vec<Vector>, bool), String> {
        Ok((vec![v(self.from), v(self.to)], false))
    }
}

impl WallCurve for Arc {
    fn kind(&self) -> &'static str {
        "arc"
    }
    fn physics(&self) -> (Option<f32>, Option<f32>) {
        (self.restitution, self.friction)
    }
    fn sample(&self) -> Result<(Vec<Vector>, bool), String> {
        let (rx, ry) = match self.radius {
            Radius::Circle(r) => (r, r),
            Radius::Ellipse([rx, ry]) => (rx, ry),
        };
        check(rx > 0.0 && ry > 0.0, || "radius must be > 0".into())?;
        let sweep = (self.end_deg - self.start_deg).to_radians();
        check(sweep != 0.0, || "start_deg and end_deg are equal".into())?;
        let closed = sweep.abs() >= TAU - 1e-4;
        let sweep = sweep.clamp(-TAU, TAU);
        // Segments of at most 5°.
        let n = ((sweep.abs() / 5f32.to_radians()).ceil() as usize).max(4);
        let (rs, rc) = self.rotation_deg.to_radians().sin_cos();
        let count = if closed { n } else { n + 1 };
        let pts = (0..count)
            .map(|i| {
                let t = self.start_deg.to_radians() + sweep * i as f32 / n as f32;
                let (x, y) = (rx * t.cos(), ry * t.sin());
                v(self.center) + Vector::new(x * rc - y * rs, x * rs + y * rc)
            })
            .collect();
        Ok((pts, closed))
    }
}

impl WallCurve for Bezier {
    fn kind(&self) -> &'static str {
        "bezier"
    }
    fn physics(&self) -> (Option<f32>, Option<f32>) {
        (self.restitution, self.friction)
    }
    fn sample(&self) -> Result<(Vec<Vector>, bool), String> {
        let n = self.points.len();
        check(n >= 4 && (n - 1) % 3 == 0, || {
            format!("needs 3n+1 points (4, 7, 10, ...): start, 2 controls, end, ...; got {n}")
        })?;
        const SUB: usize = 16;
        let mut pts = vec![v(self.points[0])];
        for seg in self.points.windows(4).step_by(3) {
            let [p0, p1, p2, p3] = [v(seg[0]), v(seg[1]), v(seg[2]), v(seg[3])];
            for i in 1..=SUB {
                let t = i as f32 / SUB as f32;
                let u = 1.0 - t;
                pts.push(p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t));
            }
        }
        Ok((pts, false))
    }
}

impl WallCurve for Polyline {
    fn kind(&self) -> &'static str {
        "polyline"
    }
    fn physics(&self) -> (Option<f32>, Option<f32>) {
        (self.restitution, self.friction)
    }
    fn sample(&self) -> Result<(Vec<Vector>, bool), String> {
        check(self.points.len() >= 2, || "needs at least 2 points".into())?;
        Ok((self.points.iter().map(|&q| v(q)).collect(), self.closed))
    }
}

impl TableFile {
    pub fn into_layout(self) -> Result<Layout, String> {
        let size = v(self.size);
        check(size.x > 0.0 && size.y > 0.0, || "size must be positive".into())?;
        check(self.angle_deg > 0.0 && self.angle_deg < 90.0, || "angle_deg must be between 0 and 90".into())?;
        check(self.gravity > 0.0, || "gravity must be > 0".into())?;
        check(self.ball.radius > 0.0, || "ball.radius must be > 0".into())?;
        check(self.ball.mass > 0.0, || "ball.mass must be > 0".into())?;
        check_coefficient("ball.restitution", self.ball.restitution)?;
        let pos = v(self.inbound.position);
        check(pos.x >= 0.0 && pos.y >= 0.0 && pos.x <= size.x && pos.y <= size.y, || {
            "inbound.position is outside the table".into()
        })?;

        let mut walls = Vec::new();
        let mut flippers = Vec::new();
        for (i, el) in self.elements.into_iter().enumerate() {
            let ctx = |e: String| format!("elements[{i}] ({}): {e}", el.kind());
            match (&el, el.wall()) {
                (Element::Flipper(f), _) => flippers.push(flipper(f).map_err(ctx)?),
                (_, Some(wall)) => {
                    let (restitution, friction) = wall.physics();
                    let restitution = restitution.unwrap_or(WALL_RESTITUTION);
                    let friction = friction.unwrap_or(WALL_FRICTION);
                    check_coefficient("restitution", restitution).map_err(ctx)?;
                    check(friction >= 0.0, || "friction must be >= 0".into()).map_err(ctx)?;
                    let (pts, closed) = wall.sample().map_err(ctx)?;
                    walls.push(Wall { layer: Layer::Playfield, pts, closed, restitution, friction });
                }
                (_, None) => unreachable!("only flippers are not walls"),
            }
        }

        Ok(Layout {
            name: self.name,
            size,
            angle: self.angle_deg.to_radians(),
            gravity: self.gravity,
            ball: BallDef { radius: self.ball.radius, mass: self.ball.mass, restitution: self.ball.restitution },
            launch: Launch { pos, vel: v(self.inbound.velocity), charged: false },
            walls,
            flippers,
            ramps: Vec::new(),
            sensors: Vec::new(),
            bumpers: Vec::new(),
            slings: Vec::new(),
            drops: Vec::new(),
            standups: Vec::new(),
            gates: Vec::new(),
            holes: Vec::new(),
            upper_outline: Vec::new(),
        })
    }
}

fn flipper(f: &Flipper) -> Result<FlipperDef, String> {
    check(f.speed_deg_per_s > 0.0, || "speed_deg_per_s must be > 0".into())?;
    check_coefficient("restitution", f.restitution)?;
    check(!f.shape.is_empty(), || "shape is empty".into())?;
    let mut outline = Vec::new();
    for (j, curve) in f.shape.iter().enumerate() {
        let wall = curve.wall();
        let ctx = |e: String| format!("shape[{j}] ({}): {e}", wall.kind());
        // Physics settings belong on the flipper itself.
        check(wall.physics() == (None, None), || "set restitution/friction on the flipper, not its outline".into())
            .map_err(ctx)?;
        outline.extend(wall.sample().map_err(ctx)?.0);
    }
    check(
        rapier2d::parry::transformation::convex_hull(&outline).len() >= 3,
        || "shape has no area".into(),
    )?;
    Ok(FlipperDef {
        pivot: v(f.pivot),
        outline,
        rest: f.rest_deg.to_radians(),
        up: f.up_deg.to_radians(),
        speed: f.speed_deg_per_s.to_radians(),
        restitution: f.restitution,
        friction: f.friction,
        layer: Layer::Playfield,
        button: match f.key {
            Key::Left => Button::Left,
            Key::Right => Button::Right,
        },
    })
}
