//! JSON table files. The format is described in `tables/README.md`.
//!
//! Units are metres, kilograms and degrees. The origin is the bottom-left
//! corner of the table, +x points right, +y points up the playfield (away from
//! the player). Angles are counter-clockwise from +x.
//!
//! Every curve is turned into short straight segments for the physics; they
//! stray at most `curve_tolerance` metres from the true curve.

use std::collections::HashMap;
use std::f32::consts::TAU;

use rapier2d::math::Vector;
use serde::Deserialize;

use crate::layout::{
    BallDef, Button, DropTarget, FlipperDef, Hole, Launcher, Layer, LayerDef, Layout, Ramp, SensorShape, Spinner, Switch,
    Transition, Wall,
};

type P = [f32; 2];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableFile {
    #[serde(default)]
    pub name: String,
    /// Name of the rules to play this table with, e.g. "demo".
    pub rules: Option<String>,
    /// Width and height.
    pub size: P,
    /// Tilt of the table.
    pub angle_deg: f32,
    /// Full gravity in m/s²; the ball feels `gravity * sin(angle)` along the table.
    #[serde(default = "default_gravity")]
    pub gravity: f32,
    /// How far (m) the straight segments a curve is turned into may stray
    /// from the true curve.
    #[serde(default = "default_curve_tolerance")]
    pub curve_tolerance: f32,
    pub ball: BallSpec,
    pub launcher: LauncherSpec,
    /// The first layer is the playfield, where new balls start.
    pub layers: Vec<LayerSpec>,
}

/// A surface the ball rolls on, with the elements on it. A ball only touches
/// elements on its own layer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerSpec {
    pub id: String,
    /// The ball slows down by `rolling_resistance * gravity` (m/s²) while
    /// rolling on this layer.
    #[serde(default = "default_rolling_resistance")]
    pub rolling_resistance: f32,
    /// Restitution and friction for walls on this layer that don't set their own.
    #[serde(default)]
    pub defaults: WallDefaults,
    /// Height above the playfield (for drawing), e.g. for an upper level.
    #[serde(default)]
    pub height: f32,
    /// Makes this layer a ramp: generates its rails, slope and the
    /// transitions at both ends.
    pub ramp: Option<RampSpec>,
    #[serde(default)]
    pub elements: Vec<Element>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RampSpec {
    /// Centreline from the entrance to the exit, like a path.
    pub center: CenterSpec,
    /// Width between the rails.
    pub width: f32,
    /// `[position, height]` pairs: position along the centreline from 0
    /// (entrance) to 1 (exit), height above the playfield in metres.
    pub heights: Vec<[f32; 2]>,
    /// `[position, degrees]` pairs: sideways tilt along the ramp. Positive
    /// pushes the ball toward the right rail (as seen going up the ramp).
    #[serde(default)]
    pub bank: Vec<[f32; 2]>,
    /// Layer the entrance is on.
    pub from: String,
    /// Layer the ball lands on at the exit.
    pub to: String,
    #[serde(default)]
    pub rails: Rails,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CenterSpec {
    pub start: P,
    pub segments: Vec<Segment>,
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Rails {
    #[default]
    Both,
    Left,
    Right,
    None,
}

fn default_rolling_resistance() -> f32 {
    0.01
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WallDefaults {
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
}

fn default_gravity() -> f32 {
    9.81
}

fn default_curve_tolerance() -> f32 {
    0.0005
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

/// A new ball waits at `position` and is shot off at `start_velocity` (m/s)
/// when the launch key is pressed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LauncherSpec {
    pub position: P,
    pub start_velocity: P,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Line(Line),
    Arc(Arc),
    Bezier(Bezier),
    Polyline(Polyline),
    Path(Path),
    Flipper(Flipper),
    Switch(SwitchSpec),
    Transition(TransitionSpec),
    DropTarget(DropTargetSpec),
    Hole(HoleSpec),
    Spinner(SpinnerSpec),
}

/// The curves a flipper outline is made of: walls without id or physics settings.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Curve {
    Line(Line),
    Arc(Arc),
    Bezier(Bezier),
    Polyline(Polyline),
    Path(Path),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// Name the rules refer to this element by; unique within the table.
    pub id: Option<String>,
    pub from: P,
    pub to: P,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
    /// Kicks the ball away at this speed (m/s): pop bumpers, slingshots.
    pub kick: Option<f32>,
    /// One-way gate: balls moving in this direction pass through.
    pub one_way: Option<P>,
}

/// Circular or elliptic arc from `start_deg` to `end_deg` (counter-clockwise if
/// end > start). A sweep of 360° or more makes a closed circle/ellipse.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arc {
    /// Name the rules refer to this element by; unique within the table.
    pub id: Option<String>,
    pub center: P,
    pub radius: Radius,
    pub start_deg: f32,
    pub end_deg: f32,
    /// Rotates an ellipse's axes.
    #[serde(default)]
    pub rotation_deg: f32,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
    /// Kicks the ball away at this speed (m/s): pop bumpers, slingshots.
    pub kick: Option<f32>,
    /// One-way gate: balls moving in this direction pass through.
    pub one_way: Option<P>,
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
    /// Name the rules refer to this element by; unique within the table.
    pub id: Option<String>,
    pub points: Vec<P>,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
    /// Kicks the ball away at this speed (m/s): pop bumpers, slingshots.
    pub kick: Option<f32>,
    /// One-way gate: balls moving in this direction pass through.
    pub one_way: Option<P>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Polyline {
    /// Name the rules refer to this element by; unique within the table.
    pub id: Option<String>,
    pub points: Vec<P>,
    #[serde(default)]
    pub closed: bool,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
    /// Kicks the ball away at this speed (m/s): pop bumpers, slingshots.
    pub kick: Option<f32>,
    /// One-way gate: balls moving in this direction pass through.
    pub one_way: Option<P>,
}

/// Connected wall made of segments, each starting where the previous one ended,
/// so there can be no gaps between them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Path {
    /// Name the rules refer to this element by; unique within the table.
    pub id: Option<String>,
    pub start: P,
    pub segments: Vec<Segment>,
    /// Adds a straight line back to `start` if the path doesn't end there.
    #[serde(default)]
    pub closed: bool,
    pub restitution: Option<f32>,
    pub friction: Option<f32>,
    /// Kicks the ball away at this speed (m/s): pop bumpers, slingshots.
    pub kick: Option<f32>,
    /// One-way gate: balls moving in this direction pass through.
    pub one_way: Option<P>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Segment {
    /// Straight line to this point.
    Line(P),
    /// Circular arc around `center`, starting at the current point and turning
    /// `sweep_deg` (counter-clockwise if positive).
    Arc { center: P, sweep_deg: f32 },
    /// Cubic Bézier: two control points, then the end point.
    Bezier([P; 3]),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flipper {
    /// Name the rules refer to this element by; unique within the table.
    pub id: Option<String>,
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

/// Reports a ball rolling over it without blocking it. Give exactly one
/// shape: `center` + `radius`, `min` + `max` (rectangle) or `from` + `to` (line).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchSpec {
    pub id: Option<String>,
    pub center: Option<P>,
    pub radius: Option<f32>,
    pub min: Option<P>,
    pub max: Option<P>,
    pub from: Option<P>,
    pub to: Option<P>,
}

/// Line `from`-`to`: a ball on this layer crossing it in `direction` moves to
/// layer `target`. With `two_way`, balls on `target` crossing back return.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionSpec {
    pub id: Option<String>,
    pub from: P,
    pub to: P,
    pub direction: P,
    pub target: String,
    #[serde(default)]
    pub two_way: bool,
}

/// Line `from`-`to` that blocks balls until hit hard enough into its front,
/// then drops until reset.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DropTargetSpec {
    pub id: Option<String>,
    pub from: P,
    pub to: P,
    /// `[dx, dy]`: the way the front faces, toward where balls come from.
    pub facing: P,
    /// Minimum speed into the front (m/s) that drops it.
    #[serde(default = "default_drop_min_speed")]
    pub min_speed: f32,
    pub restitution: Option<f32>,
}

fn default_drop_min_speed() -> f32 {
    0.3
}

/// Scoop, saucer or ball lock.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoleSpec {
    pub id: Option<String>,
    pub center: P,
    #[serde(default = "default_hole_radius")]
    pub radius: f32,
    /// `[vx, vy]` a ball leaves with.
    pub eject: P,
    /// Seconds until a caught ball is ejected; without it the hole is a lock
    /// that holds balls until the rules eject them.
    pub eject_after: Option<f32>,
    /// Faster balls roll over it (saucers).
    pub max_speed: Option<f32>,
}

fn default_hole_radius() -> f32 {
    0.012
}

/// A flap on line `from`-`to` that spins when balls pass.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpinnerSpec {
    pub id: Option<String>,
    pub from: P,
    pub to: P,
    #[serde(default = "default_spins_per_mps")]
    pub spins_per_mps: f32,
}

fn default_spins_per_mps() -> f32 {
    8.0
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

/// Settings every wall element shares.
#[derive(PartialEq, Default)]
struct Props<'a> {
    id: Option<&'a str>,
    restitution: Option<f32>,
    friction: Option<f32>,
    kick: Option<f32>,
    one_way: Option<P>,
}

/// What every wall curve can do: report its settings and turn into points.
trait WallCurve {
    fn kind(&self) -> &'static str;
    fn props(&self) -> Props<'_>;
    /// Points along the curve, at most `tolerance` away from it between
    /// points; `true` if it is closed.
    fn sample(&self, tolerance: f32) -> Result<(Vec<Vector>, bool), String>;
}

/// Implements `kind` and `props` for a wall struct.
macro_rules! wall_props {
    ($kind:literal) => {
        fn kind(&self) -> &'static str {
            $kind
        }
        fn props(&self) -> Props<'_> {
            Props {
                id: self.id.as_deref(),
                restitution: self.restitution,
                friction: self.friction,
                kick: self.kick,
                one_way: self.one_way,
            }
        }
    };
}

impl Element {
    fn kind(&self) -> &'static str {
        match self {
            Element::Flipper(_) => "flipper",
            Element::Switch(_) => "switch",
            Element::Transition(_) => "transition",
            Element::DropTarget(_) => "drop_target",
            Element::Hole(_) => "hole",
            Element::Spinner(_) => "spinner",
            _ => self.wall().unwrap().kind(),
        }
    }

    fn id(&self) -> Option<&str> {
        match self {
            Element::Flipper(f) => f.id.as_deref(),
            Element::Switch(s) => s.id.as_deref(),
            Element::Transition(t) => t.id.as_deref(),
            Element::DropTarget(d) => d.id.as_deref(),
            Element::Hole(h) => h.id.as_deref(),
            Element::Spinner(s) => s.id.as_deref(),
            _ => self.wall().unwrap().props().id,
        }
    }

    fn wall(&self) -> Option<&dyn WallCurve> {
        match self {
            Element::Line(c) => Some(c),
            Element::Arc(c) => Some(c),
            Element::Bezier(c) => Some(c),
            Element::Polyline(c) => Some(c),
            Element::Path(c) => Some(c),
            Element::Flipper(_)
            | Element::Switch(_)
            | Element::Transition(_)
            | Element::DropTarget(_)
            | Element::Hole(_)
            | Element::Spinner(_) => None,
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
            Curve::Path(c) => c,
        }
    }
}

impl WallCurve for Line {
    wall_props!("line");
    fn sample(&self, _tolerance: f32) -> Result<(Vec<Vector>, bool), String> {
        Ok((vec![v(self.from), v(self.to)], false))
    }
}

impl WallCurve for Arc {
    wall_props!("arc");
    fn sample(&self, tolerance: f32) -> Result<(Vec<Vector>, bool), String> {
        let (rx, ry) = match self.radius {
            Radius::Circle(r) => (r, r),
            Radius::Ellipse([rx, ry]) => (rx, ry),
        };
        check(rx > 0.0 && ry > 0.0, || "radius must be > 0".into())?;
        let sweep = (self.end_deg - self.start_deg).to_radians();
        check(sweep != 0.0, || "start_deg and end_deg are equal".into())?;
        let closed = sweep.abs() >= TAU - 1e-4;
        let shape = Ellipse { center: v(self.center), rx, ry, rotation: self.rotation_deg.to_radians() };
        let mut pts = shape.points(self.start_deg.to_radians(), sweep.clamp(-TAU, TAU), tolerance);
        if closed {
            pts.pop(); // same as the first point
        }
        Ok((pts, closed))
    }
}

impl WallCurve for Bezier {
    wall_props!("bezier");
    fn sample(&self, tolerance: f32) -> Result<(Vec<Vector>, bool), String> {
        let n = self.points.len();
        check(n >= 4 && (n - 1).is_multiple_of(3), || {
            format!("needs 3n+1 points (4, 7, 10, ...): start, 2 controls, end, ...; got {n}")
        })?;
        let mut pts = vec![v(self.points[0])];
        for seg in self.points.windows(4).step_by(3) {
            flatten_cubic([v(seg[0]), v(seg[1]), v(seg[2]), v(seg[3])], tolerance, 0, &mut pts);
        }
        Ok((pts, false))
    }
}

struct Ellipse {
    center: Vector,
    rx: f32,
    ry: f32,
    rotation: f32,
}

impl Ellipse {
    /// Points from angle `start` turning `sweep` (radians), both ends included.
    fn points(&self, start: f32, sweep: f32, tolerance: f32) -> Vec<Vector> {
        // A chord spanning angle a on radius r strays r(1 - cos(a/2)) from the
        // arc. An ellipse is a circle of the larger radius squashed along one
        // axis, which only shrinks that distance.
        let r = self.rx.max(self.ry);
        let max_step = 2.0 * (1.0 - (tolerance / r).min(1.0)).acos();
        let n = ((sweep.abs() / max_step).ceil() as usize).max(4);
        let (rs, rc) = self.rotation.sin_cos();
        (0..=n)
            .map(|i| {
                let t = start + sweep * i as f32 / n as f32;
                let (x, y) = (self.rx * t.cos(), self.ry * t.sin());
                self.center + Vector::new(x * rc - y * rs, x * rs + y * rc)
            })
            .collect()
    }
}

/// Appends points along a cubic Bézier (excluding its start), halving it until
/// each piece is flat enough. The curve stays within 3/4 of the control
/// points' largest distance from the chord, so that bounds the error.
fn flatten_cubic(c: [Vector; 4], tolerance: f32, depth: u32, out: &mut Vec<Vector>) {
    let [p0, p1, p2, p3] = c;
    let chord = p3 - p0;
    let dist = |p: Vector| {
        let len = chord.length();
        if len < 1e-9 { p.distance(p0) } else { (p - p0).perp_dot(chord).abs() / len }
    };
    if depth >= 16 || 0.75 * dist(p1).max(dist(p2)) <= tolerance {
        out.push(p3);
        return;
    }
    // de Casteljau split at t = 0.5.
    let (a, b, c) = ((p0 + p1) * 0.5, (p1 + p2) * 0.5, (p2 + p3) * 0.5);
    let (d, e) = ((a + b) * 0.5, (b + c) * 0.5);
    let mid = (d + e) * 0.5;
    flatten_cubic([p0, a, d, mid], tolerance, depth + 1, out);
    flatten_cubic([mid, e, c, p3], tolerance, depth + 1, out);
}

impl WallCurve for Polyline {
    wall_props!("polyline");
    fn sample(&self, _tolerance: f32) -> Result<(Vec<Vector>, bool), String> {
        check(self.points.len() >= 2, || "needs at least 2 points".into())?;
        Ok((self.points.iter().map(|&q| v(q)).collect(), self.closed))
    }
}

impl WallCurve for Path {
    wall_props!("path");
    fn sample(&self, tolerance: f32) -> Result<(Vec<Vector>, bool), String> {
        let start = v(self.start);
        let mut pts = sample_path(start, &self.segments, tolerance)?;
        if self.closed && pts.last().unwrap().distance(start) < 1e-6 {
            pts.pop(); // the closing line is implied
        }
        Ok((pts, self.closed))
    }
}

/// Points along a path's segments, starting with `start`.
fn sample_path(start: Vector, segments: &[Segment], tolerance: f32) -> Result<Vec<Vector>, String> {
    check(!segments.is_empty(), || "needs at least one segment".into())?;
    {
        let mut pts = vec![start];
        for (k, seg) in segments.iter().enumerate() {
            let cur = *pts.last().unwrap();
            match *seg {
                Segment::Line(to) => pts.push(v(to)),
                Segment::Arc { center, sweep_deg } => {
                    let c = v(center);
                    let r = cur.distance(c);
                    check(r > 1e-6, || format!("segments[{k}] (arc): center is the current point"))?;
                    check(sweep_deg != 0.0, || format!("segments[{k}] (arc): sweep_deg is 0"))?;
                    let start_angle = (cur.y - c.y).atan2(cur.x - c.x);
                    let circle = Ellipse { center: c, rx: r, ry: r, rotation: 0.0 };
                    pts.extend(circle.points(start_angle, sweep_deg.to_radians(), tolerance).into_iter().skip(1));
                }
                Segment::Bezier([c1, c2, end]) => flatten_cubic([cur, v(c1), v(c2), v(end)], tolerance, 0, &mut pts),
            }
        }
        Ok(pts)
    }
}

impl TableFile {
    pub fn into_layout(self) -> Result<Layout, String> {
        let size = v(self.size);
        check(size.x > 0.0 && size.y > 0.0, || "size must be positive".into())?;
        check(self.angle_deg > 0.0 && self.angle_deg < 90.0, || "angle_deg must be between 0 and 90".into())?;
        check(self.gravity > 0.0, || "gravity must be > 0".into())?;
        let tolerance = self.curve_tolerance;
        check(tolerance > 0.0, || "curve_tolerance must be > 0".into())?;
        check(self.ball.radius > 0.0, || "ball.radius must be > 0".into())?;
        check(self.ball.mass > 0.0, || "ball.mass must be > 0".into())?;
        check_coefficient("ball.restitution", self.ball.restitution)?;
        let position = v(self.launcher.position);
        check(position.x >= 0.0 && position.y >= 0.0 && position.x <= size.x && position.y <= size.y, || {
            "launcher.position is outside the table".into()
        })?;

        check(!self.layers.is_empty(), || "needs at least one layer".into())?;
        check(self.layers.len() <= Layer::MAX, || format!("at most {} layers", Layer::MAX))?;

        // Layers can refer to each other in any order, so learn their names first.
        let layer_ids: HashMap<String, Layer> =
            self.layers.iter().enumerate().map(|(i, l)| (l.id.clone(), Layer(i as u8))).collect();
        let lookup = |name: &str, own: Layer| -> Result<Layer, String> {
            let &l = layer_ids.get(name).ok_or_else(|| format!("no layer \"{name}\""))?;
            check(l != own, || format!("\"{name}\" is this layer itself"))?;
            Ok(l)
        };

        let mut layers = Vec::new();
        let mut walls = Vec::new();
        let mut flippers = Vec::new();
        let mut switches = Vec::new();
        let mut transitions = Vec::new();
        let mut drop_targets = Vec::new();
        let mut holes = Vec::new();
        let mut spinners = Vec::new();
        // Layer and element ids share one namespace.
        let mut ids = std::collections::HashSet::new();
        let mut claim = |id: &str| -> Result<(), String> {
            check(!id.is_empty(), || "id is empty".into())?;
            check(ids.insert(id.to_owned()), || format!("id \"{id}\" is used twice"))
        };
        for (li, spec) in self.layers.into_iter().enumerate() {
            let layer = Layer(li as u8);
            let lctx = |e: String| format!("layers[{li}] ({}): {e}", spec.id);
            claim(&spec.id).map_err(lctx)?;
            check(spec.rolling_resistance >= 0.0, || "rolling_resistance must be >= 0".into()).map_err(lctx)?;
            check(spec.height >= 0.0, || "height must be >= 0".into()).map_err(lctx)?;
            let wall_defaults = |props: &Props| {
                let restitution = props.restitution.or(spec.defaults.restitution).unwrap_or(WALL_RESTITUTION);
                let friction = props.friction.or(spec.defaults.friction).unwrap_or(WALL_FRICTION);
                (restitution, friction)
            };

            let ramp = match &spec.ramp {
                None => None,
                Some(r) => {
                    let rctx = |e: String| lctx(format!("ramp: {e}"));
                    let from = lookup(&r.from, layer).map_err(|e| rctx(format!("from: {e}")))?;
                    let to = lookup(&r.to, layer).map_err(|e| rctx(format!("to: {e}")))?;
                    let ramp = ramp(r, tolerance).map_err(rctx)?;
                    let (restitution, friction) = wall_defaults(&Props::default());
                    // One-sided, with the track on the right of the point order.
                    let rail = |side: &str, pts: Vec<Vector>| Wall {
                        id: Some(format!("{}.{side}_rail", spec.id)),
                        layer,
                        pts,
                        closed: false,
                        restitution,
                        friction,
                        kick: 0.0,
                        one_way: None,
                        one_sided: true,
                    };
                    if matches!(r.rails, Rails::Both | Rails::Left) {
                        walls.push(rail("left", ramp.left.clone()));
                    }
                    if matches!(r.rails, Rails::Both | Rails::Right) {
                        walls.push(rail("right", ramp.right.iter().rev().copied().collect()));
                    }
                    let n = ramp.center.len();
                    let tangent = |i: usize, j: usize| (ramp.center[j] - ramp.center[i]).normalize();
                    // Entrance: two-way, so a weak shot rolls back onto `from`.
                    transitions.push(Transition {
                        id: Some(format!("{}.entrance", spec.id)),
                        layer: from,
                        a: ramp.left[0],
                        b: ramp.right[0],
                        direction: tangent(0, 1),
                        target: layer,
                        two_way: true,
                    });
                    // Exit: one-way, so balls under the ramp's end aren't lifted onto it.
                    transitions.push(Transition {
                        id: Some(format!("{}.exit", spec.id)),
                        layer,
                        a: ramp.left[n - 1],
                        b: ramp.right[n - 1],
                        direction: tangent(n - 2, n - 1),
                        target: to,
                        two_way: false,
                    });
                    for id in ["entrance", "exit"].map(|e| format!("{}.{e}", spec.id)) {
                        claim(&id).map_err(rctx)?;
                    }
                    for rail in walls.iter().rev().take_while(|w| w.layer == layer) {
                        claim(rail.id.as_deref().unwrap()).map_err(rctx)?;
                    }
                    Some(ramp)
                }
            };

            for (i, el) in spec.elements.iter().enumerate() {
                let ctx = |e: String| lctx(format!("elements[{i}] ({}): {e}", el.kind()));
                let id = el.id().map(str::to_owned);
                if let Some(id) = &id {
                    claim(id).map_err(ctx)?;
                }
                match el {
                    Element::Flipper(f) => flippers.push(flipper(f, layer, tolerance).map_err(ctx)?),
                    Element::Switch(sw) => {
                        switches.push(Switch { id, layer, shape: switch_shape(sw).map_err(ctx)? })
                    }
                    Element::Transition(t) => {
                        let target = lookup(&t.target, layer).map_err(|e| ctx(format!("target: {e}")))?;
                        let direction = v(t.direction);
                        check(direction.length() > 0.0, || "direction is zero".into()).map_err(ctx)?;
                        check(t.from != t.to, || "from and to are the same point".into()).map_err(ctx)?;
                        let direction = direction.normalize();
                        transitions.push(Transition {
                            id,
                            layer,
                            a: v(t.from),
                            b: v(t.to),
                            direction,
                            target,
                            two_way: t.two_way,
                        });
                    }
                    Element::DropTarget(d) => {
                        let facing = v(d.facing);
                        check(facing.length() > 0.0, || "facing is zero".into()).map_err(ctx)?;
                        check(d.from != d.to, || "from and to are the same point".into()).map_err(ctx)?;
                        check(d.min_speed >= 0.0, || "min_speed must be >= 0".into()).map_err(ctx)?;
                        let (restitution, _) =
                            wall_defaults(&Props { restitution: d.restitution, ..Props::default() });
                        check_coefficient("restitution", restitution).map_err(ctx)?;
                        drop_targets.push(DropTarget {
                            id,
                            layer,
                            a: v(d.from),
                            b: v(d.to),
                            facing: facing.normalize(),
                            min_speed: d.min_speed,
                            restitution,
                        });
                    }
                    Element::Hole(h) => {
                        check(h.radius > 0.0, || "radius must be > 0".into()).map_err(ctx)?;
                        check(h.eject_after.is_none_or(|t| t >= 0.0), || "eject_after must be >= 0".into())
                            .map_err(ctx)?;
                        holes.push(Hole {
                            id,
                            layer,
                            center: v(h.center),
                            radius: h.radius,
                            eject: v(h.eject),
                            eject_after: h.eject_after,
                            max_speed: h.max_speed.unwrap_or(f32::MAX),
                        });
                    }
                    Element::Spinner(sp) => {
                        check(sp.from != sp.to, || "from and to are the same point".into()).map_err(ctx)?;
                        check(sp.spins_per_mps > 0.0, || "spins_per_mps must be > 0".into()).map_err(ctx)?;
                        spinners.push(Spinner {
                            id,
                            layer,
                            a: v(sp.from),
                            b: v(sp.to),
                            spins_per_mps: sp.spins_per_mps,
                        });
                    }
                    _ => {
                        let wall = el.wall().unwrap();
                        let props = wall.props();
                        let (restitution, friction) = wall_defaults(&props);
                        check_coefficient("restitution", restitution).map_err(ctx)?;
                        check(friction >= 0.0, || "friction must be >= 0".into()).map_err(ctx)?;
                        let kick = props.kick.unwrap_or(0.0);
                        check(kick >= 0.0, || "kick must be >= 0".into()).map_err(ctx)?;
                        let one_way = props.one_way.map(v);
                        if let Some(d) = one_way {
                            check(d.length() > 0.0, || "one_way is zero".into()).map_err(ctx)?;
                        }
                        let (pts, closed) = wall.sample(tolerance).map_err(ctx)?;
                        walls.push(Wall {
                            id,
                            layer,
                            pts,
                            closed,
                            restitution,
                            friction,
                            kick,
                            one_way: one_way.map(|d| d.normalize()),
                            one_sided: false,
                        });
                    }
                }
            }
            layers.push(LayerDef {
                name: spec.id,
                rolling_resistance: spec.rolling_resistance,
                height: spec.height,
                ramp,
            });
        }

        Ok(Layout {
            name: self.name,
            rules: self.rules,
            size,
            angle: self.angle_deg.to_radians(),
            gravity: self.gravity,
            ball: BallDef { radius: self.ball.radius, mass: self.ball.mass, restitution: self.ball.restitution },
            launcher: Launcher { position, start_velocity: v(self.launcher.start_velocity) },
            layers,
            walls,
            flippers,
            switches,
            transitions,
            drop_targets,
            holes,
            spinners,
        })
    }
}

fn switch_shape(sw: &SwitchSpec) -> Result<SensorShape, String> {
    match (sw.center, sw.radius, sw.min, sw.max, sw.from, sw.to) {
        (Some(c), Some(r), None, None, None, None) => {
            check(r > 0.0, || "radius must be > 0".into())?;
            Ok(SensorShape::Circle { center: v(c), radius: r })
        }
        (None, None, Some(min), Some(max), None, None) => {
            check(min[0] < max[0] && min[1] < max[1], || "min must be below and left of max".into())?;
            Ok(SensorShape::Rect { min: v(min), max: v(max) })
        }
        (None, None, None, None, Some(a), Some(b)) => {
            check(a != b, || "from and to are the same point".into())?;
            Ok(SensorShape::Segment { a: v(a), b: v(b) })
        }
        _ => Err("needs exactly one shape: center + radius, min + max, or from + to".into()),
    }
}

/// Builds a ramp's track: a densely sampled centreline with heights, and its edges.
fn ramp(r: &RampSpec, tolerance: f32) -> Result<Ramp, String> {
    check(r.width > 0.0, || "width must be > 0".into())?;
    check(!r.heights.is_empty(), || "heights is empty".into())?;
    for w in r.heights.windows(2) {
        check(w[0][0] < w[1][0], || "heights positions must increase".into())?;
    }
    check(r.heights.iter().all(|h| (0.0..=1.0).contains(&h[0]) && h[1] >= 0.0), || {
        "heights must be [position 0..1, height >= 0] pairs".into()
    })?;
    for w in r.bank.windows(2) {
        check(w[0][0] < w[1][0], || "bank positions must increase".into())?;
    }
    check(r.bank.iter().all(|b| (0.0..=1.0).contains(&b[0]) && b[1].abs() < 60.0), || {
        "bank must be [position 0..1, degrees between -60 and 60] pairs".into()
    })?;
    let coarse = sample_path(v(r.center.start), &r.center.segments, tolerance).map_err(|e| format!("center: {e}"))?;

    // Densify so the height profile and the rails follow the curve closely.
    const MAX_STEP: f32 = 0.005;
    let mut center = vec![coarse[0]];
    for w in coarse.windows(2) {
        let n = ((w[0].distance(w[1]) / MAX_STEP).ceil() as usize).max(1);
        center.extend((1..=n).map(|k| w[0] + (w[1] - w[0]) * (k as f32 / n as f32)));
    }
    center.dedup_by(|a, b| a.distance(*b) < 1e-7);
    check(center.len() >= 2, || "center has no length".into())?;

    let mut along = vec![0.0];
    for w in center.windows(2) {
        along.push(along.last().unwrap() + w[0].distance(w[1]));
    }
    let total = *along.last().unwrap();
    let heights = along.iter().map(|s| profile(&r.heights, s / total)).collect();
    let banks = along.iter().map(|s| if r.bank.is_empty() { 0.0 } else { profile(&r.bank, s / total).to_radians() }).collect();

    let m = center.len();
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for k in 0..m {
        let t = (center[(k + 1).min(m - 1)] - center[k.saturating_sub(1)]).normalize();
        let normal = Vector::new(-t.y, t.x) * (r.width / 2.0);
        left.push(center[k] + normal);
        right.push(center[k] - normal);
    }
    Ok(Ramp { center, heights, banks, left, right })
}

/// Linear interpolation in `[position, value]` pairs, flat beyond the ends.
fn profile(points: &[[f32; 2]], f: f32) -> f32 {
    if f <= points[0][0] {
        return points[0][1];
    }
    for w in points.windows(2) {
        if f <= w[1][0] {
            return w[0][1] + (w[1][1] - w[0][1]) * (f - w[0][0]) / (w[1][0] - w[0][0]);
        }
    }
    points[points.len() - 1][1]
}

fn flipper(f: &Flipper, layer: Layer, tolerance: f32) -> Result<FlipperDef, String> {
    check(f.speed_deg_per_s > 0.0, || "speed_deg_per_s must be > 0".into())?;
    check_coefficient("restitution", f.restitution)?;
    check(!f.shape.is_empty(), || "shape is empty".into())?;
    let mut outline = Vec::new();
    for (j, curve) in f.shape.iter().enumerate() {
        let wall = curve.wall();
        let ctx = |e: String| format!("shape[{j}] ({}): {e}", wall.kind());
        // The id and physics settings belong on the flipper itself.
        check(wall.props() == Props::default(), || {
            "set id/restitution/friction on the flipper, not its outline; kick and one_way don't apply".into()
        })
            .map_err(ctx)?;
        outline.extend(wall.sample(tolerance).map_err(ctx)?.0);
    }
    check(
        rapier2d::parry::transformation::convex_hull(&outline).len() >= 3,
        || "shape has no area".into(),
    )?;
    Ok(FlipperDef {
        id: f.id.clone(),
        pivot: v(f.pivot),
        outline,
        rest: f.rest_deg.to_radians(),
        up: f.up_deg.to_radians(),
        speed: f.speed_deg_per_s.to_radians(),
        restitution: f.restitution,
        friction: f.friction,
        layer,
        button: match f.key {
            Key::Left => Button::Left,
            Key::Right => Button::Right,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Largest distance from points on the true curve to the sampled polyline.
    fn max_error(truth: impl Iterator<Item = Vector>, poly: &[Vector]) -> f32 {
        let seg_dist = |p: Vector, a: Vector, b: Vector| {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        };
        truth
            .map(|p| poly.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f32::MAX, f32::min))
            .fold(0.0, f32::max)
    }

    #[test]
    fn arcs_stay_within_tolerance() {
        for (radius, tol) in [(Radius::Circle(0.26), 0.0005), (Radius::Ellipse([0.08, 0.03]), 0.0002), (Radius::Circle(0.01), 0.002)] {
            let (rx, ry) = match radius {
                Radius::Circle(r) => (r, r),
                Radius::Ellipse([a, b]) => (a, b),
            };
            let arc = Arc { id: None, center: [0.0, 0.0], radius, start_deg: 10.0, end_deg: 190.0, rotation_deg: 0.0, restitution: None, friction: None, kick: None, one_way: None };
            let (pts, _) = arc.sample(tol).unwrap();
            let truth = (0..=1000).map(|i| {
                let t = (10.0 + 180.0 * i as f32 / 1000.0).to_radians();
                Vector::new(rx * t.cos(), ry * t.sin())
            });
            let err = max_error(truth, &pts);
            assert!(err <= tol * 1.01, "arc error {err} > {tol} with {} points", pts.len());
        }
    }

    #[test]
    fn beziers_stay_within_tolerance_and_adapt() {
        let curve = |points: Vec<P>| Bezier { id: None, points, restitution: None, friction: None, kick: None, one_way: None };
        let gentle = curve(vec![[0.0, 0.0], [0.1, 0.01], [0.2, 0.01], [0.3, 0.0]]);
        let sharp = curve(vec![[0.0, 0.0], [0.0, 0.3], [0.3, 0.3], [0.3, 0.0]]);
        for b in [&gentle, &sharp] {
            let c = b.points.iter().map(|&p| v(p)).collect::<Vec<_>>();
            let truth = (0..=1000).map(|i| {
                let t = i as f32 / 1000.0;
                let u = 1.0 - t;
                c[0] * (u * u * u) + c[1] * (3.0 * u * u * t) + c[2] * (3.0 * u * t * t) + c[3] * (t * t * t)
            });
            let (pts, _) = b.sample(0.0005).unwrap();
            let err = max_error(truth, &pts);
            assert!(err <= 0.0005 * 1.01, "bezier error {err} with {} points", pts.len());
        }
        let (gentle_pts, _) = gentle.sample(0.0005).unwrap();
        let (sharp_pts, _) = sharp.sample(0.0005).unwrap();
        assert!(gentle_pts.len() < sharp_pts.len(), "{} vs {}", gentle_pts.len(), sharp_pts.len());
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    fn path(json: &str) -> Path {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn path_segments_connect() {
        // Up, a quarter circle to the right, a Bézier down, closed back to the start.
        let p = path(
            r#"{"start": [0, 0], "closed": true, "segments": [
                {"line": [0, 1]},
                {"arc": {"center": [1, 1], "sweep_deg": -90}},
                {"bezier": [[1.5, 2], [2, 1.5], [2, 0]]}
            ]}"#,
        );
        let (pts, closed) = p.sample(0.0005).unwrap();
        assert!(closed);
        assert_eq!(pts[0], Vector::new(0.0, 0.0));
        assert_eq!(pts[1], Vector::new(0.0, 1.0));
        // The arc ends at the top of the circle around (1, 1).
        assert!(pts.iter().any(|q| q.distance(Vector::new(1.0, 2.0)) < 1e-5));
        assert!(pts.last().unwrap().distance(Vector::new(2.0, 0.0)) < 1e-6);
    }

    #[test]
    fn path_errors() {
        let p = path(r#"{"start": [0, 0], "segments": [{"arc": {"center": [0, 0], "sweep_deg": 90}}]}"#);
        assert!(p.sample(0.0005).unwrap_err().contains("segments[0] (arc)"));
        let e = serde_json::from_str::<Path>(r#"{"start": [0, 0], "segments": [{"curve": [1, 1]}]}"#)
            .err()
            .unwrap()
            .to_string();
        assert!(e.contains("unknown variant `curve`"), "{e}");
    }
}
