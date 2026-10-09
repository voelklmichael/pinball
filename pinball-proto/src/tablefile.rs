//! JSON table files. The format is described in `tables/README.md`.
//!
//! Units are metres, kilograms and degrees. The origin is the bottom-left
//! corner of the table, +x points right, +y points up the playfield (away from
//! the player). Angles are counter-clockwise from +x.
//!
//! Every curve is turned into short straight segments for the physics; they
//! stray at most `curve_tolerance` metres from the true curve.

use std::f32::consts::TAU;

use rapier2d::math::Vector;
use serde::Deserialize;

use crate::layout::{BallDef, Button, FlipperDef, Launcher, Layer, LayerDef, Layout, Wall};

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
    pub elements: Vec<Element>,
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
#[derive(PartialEq)]
struct Props<'a> {
    id: Option<&'a str>,
    restitution: Option<f32>,
    friction: Option<f32>,
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
            Props { id: self.id.as_deref(), restitution: self.restitution, friction: self.friction }
        }
    };
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
            Element::Path(c) => Some(c),
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
        check(n >= 4 && (n - 1) % 3 == 0, || {
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
        check(!self.segments.is_empty(), || "needs at least one segment".into())?;
        let start = v(self.start);
        let mut pts = vec![start];
        for (k, seg) in self.segments.iter().enumerate() {
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
        if self.closed && pts.last().unwrap().distance(start) < 1e-6 {
            pts.pop(); // the closing line is implied
        }
        Ok((pts, self.closed))
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

        let mut layers = Vec::new();
        let mut walls = Vec::new();
        let mut flippers = Vec::new();
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
            for (i, el) in spec.elements.iter().enumerate() {
                let ctx = |e: String| lctx(format!("elements[{i}] ({}): {e}", el.kind()));
                let id = match el {
                    Element::Flipper(f) => f.id.clone(),
                    _ => el.wall().unwrap().props().id.map(str::to_owned),
                };
                if let Some(id) = &id {
                    claim(id).map_err(ctx)?;
                }
                match (el, el.wall()) {
                    (Element::Flipper(f), _) => flippers.push(flipper(f, layer, tolerance).map_err(ctx)?),
                    (_, Some(wall)) => {
                        let props = wall.props();
                        let restitution =
                            props.restitution.or(spec.defaults.restitution).unwrap_or(WALL_RESTITUTION);
                        let friction = props.friction.or(spec.defaults.friction).unwrap_or(WALL_FRICTION);
                        check_coefficient("restitution", restitution).map_err(ctx)?;
                        check(friction >= 0.0, || "friction must be >= 0".into()).map_err(ctx)?;
                        let (pts, closed) = wall.sample(tolerance).map_err(ctx)?;
                        walls.push(Wall { id, layer, pts, closed, restitution, friction });
                    }
                    (_, None) => unreachable!("only flippers are not walls"),
                }
            }
            layers.push(LayerDef {
                name: spec.id,
                height: 0.0,
                rolling_resistance: spec.rolling_resistance,
                outline: Vec::new(),
            });
        }

        Ok(Layout {
            name: self.name,
            size,
            angle: self.angle_deg.to_radians(),
            gravity: self.gravity,
            ball: BallDef { radius: self.ball.radius, mass: self.ball.mass, restitution: self.ball.restitution },
            launcher: Launcher { position, start_velocity: v(self.launcher.start_velocity), charged: false },
            layers,
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
        })
    }
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
        let bare = Props { id: None, restitution: None, friction: None };
        check(wall.props() == bare, || "set id/restitution/friction on the flipper, not its outline".into())
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
            let arc = Arc { id: None, center: [0.0, 0.0], radius, start_deg: 10.0, end_deg: 190.0, rotation_deg: 0.0, restitution: None, friction: None };
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
        let curve = |points: Vec<P>| Bezier { id: None, points, restitution: None, friction: None };
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
