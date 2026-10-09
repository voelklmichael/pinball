//! Debug view of the layout (plus optional ball paths) as SVG, for checking
//! geometry and ball paths without opening a window. Used by the tests.

use std::fmt::Write;

use rapier2d::math::{Pose, Vector};

use crate::layout::{Layer, Layout};

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

    for w in &layout.walls {
        let width = if w.layer == Layer::PLAYFIELD { 3 } else { 2 };
        poly(&mut s, top, &w.pts, w.closed, &format!("stroke:{};stroke-width:{width}", layer_color(w.layer)));
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
