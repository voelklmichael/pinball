//! Debug view of the layout (plus optional ball paths) as SVG, for checking
//! geometry and ball paths without opening a window. Used by the tests.

use std::fmt::Write;

use rapier2d::math::{Pose, Vector};

use crate::layout::{Layer, Layout, SensorShape};

const SCALE: f32 = 1000.0; // px per metre

fn layer_color(l: Layer) -> &'static str {
    const ABOVE: [&str; 4] = ["#1f77b4", "#d62728", "#2ca02c", "#9467bd"];
    if l == Layer::PLAYFIELD { "#222" } else { ABOVE[(l.index() - 1) % ABOVE.len()] }
}

/// SVG coordinates have y pointing down; `top` is the table height.
fn pt(top: f32, p: Vector) -> String {
    format!("{:.1},{:.1}", p.x * SCALE, (top - p.y) * SCALE)
}

fn poly(out: &mut String, top: f32, pts: &[Vector], closed: bool, style: &str) {
    if pts.len() < 2 {
        return;
    }
    let tag = if closed { "polygon" } else { "polyline" };
    let pts: Vec<String> = pts.iter().map(|&p| pt(top, p)).collect();
    writeln!(out, r#"<{tag} points="{}" style="fill:none;{style}"/>"#, pts.join(" ")).unwrap();
}

fn circle(out: &mut String, top: f32, c: Vector, r: f32, style: &str) {
    let (x, y) = (c.x * SCALE, (top - c.y) * SCALE);
    writeln!(out, r#"<circle cx="{x:.1}" cy="{y:.1}" r="{:.1}" style="{style}"/>"#, r * SCALE).unwrap();
}

/// `traces`: ball paths with a colour each.
pub fn render(layout: &Layout, traces: &[(Vec<Vector>, &str)]) -> String {
    let mut s = String::new();
    let top = layout.size.y;
    writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" style="background:#fff">"#,
        layout.size.x * SCALE,
        top * SCALE
    )
    .unwrap();

    for (i, def) in layout.layers.iter().enumerate() {
        if let Some(ramp) = &def.ramp {
            let mut outline = ramp.left.clone();
            outline.extend(ramp.right.iter().rev());
            let c = layer_color(Layer(i as u8));
            poly(&mut s, top, &outline, true, &format!("fill:{c};fill-opacity:0.15;stroke:none"));
        }
    }
    for w in &layout.walls {
        let width = if w.layer == Layer::PLAYFIELD { 3 } else { 2 };
        let color = match (w.kick > 0.0, w.one_way.is_some()) {
            (true, _) => "#e6465a",
            (_, true) => "#ff9f00",
            _ => layer_color(w.layer),
        };
        poly(&mut s, top, &w.pts, w.closed, &format!("stroke:{color};stroke-width:{width}"));
    }
    let sensor = "stroke:#e6b800;stroke-width:1.5";
    for sw in &layout.switches {
        match sw.shape {
            SensorShape::Circle { center, radius } => circle(&mut s, top, center, radius, &format!("fill:none;{sensor}")),
            SensorShape::Rect { min, max } => {
                let corners = [min, Vector::new(max.x, min.y), max, Vector::new(min.x, max.y)];
                poly(&mut s, top, &corners, true, sensor);
            }
            SensorShape::Segment { a, b } => poly(&mut s, top, &[a, b], false, sensor),
        }
    }
    for t in &layout.transitions {
        poly(&mut s, top, &[t.a, t.b], false, "stroke:#17becf;stroke-width:1.5;stroke-dasharray:4 2");
    }
    for d in &layout.drop_targets {
        poly(&mut s, top, &[d.a, d.b], false, "stroke:#f80;stroke-width:5");
    }
    for h in &layout.holes {
        circle(&mut s, top, h.center, h.radius, "fill:#000");
    }
    for sp in &layout.spinners {
        poly(&mut s, top, &[sp.a, sp.b], false, "stroke:#888;stroke-width:3;stroke-dasharray:2 2");
    }
    for f in &layout.flaps {
        // Closed, and faintly where it opens to.
        let tip = |angle: f32| f.pivot + Vector::new(angle.cos(), angle.sin()) * f.length;
        poly(&mut s, top, &[f.pivot, tip(f.open)], false, "stroke:#ff9f00;stroke-width:2;stroke-opacity:0.35");
        poly(&mut s, top, &[f.pivot, tip(f.closed)], false, "stroke:#ff9f00;stroke-width:4");
        circle(&mut s, top, f.pivot, 0.003, "fill:#ff9f00");
    }
    for f in &layout.flippers {
        let pose = Pose::new(f.pivot, f.rest);
        let hull = rapier2d::parry::transformation::convex_hull(&f.outline);
        let pts: Vec<Vector> = hull.iter().map(|&p| pose * p).collect();
        poly(&mut s, top, &pts, true, "fill:#f60;stroke:none");
    }
    for (path, color) in traces {
        poly(&mut s, top, path, false, &format!("stroke:{color};stroke-width:1.5"));
        if let Some(&end) = path.last() {
            circle(&mut s, top, end, layout.ball.radius, &format!("fill:{color}"));
        }
    }
    writeln!(s, "</svg>").unwrap();
    s
}
