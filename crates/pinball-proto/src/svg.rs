//! Debug view of the layout (plus optional ball traces) as SVG, for checking
//! geometry without opening a window. Used by the tests.

use std::fmt::Write;

use rapier2d::math::{Pose, Vector};

use crate::layout::{Layer, Layout, Shape};

const SCALE: f32 = 1000.0; // px per metre

fn layer_color(l: Layer) -> &'static str {
    match l {
        Layer::Playfield => "#222",
        Layer::RampL => "#1f77b4",
        Layer::RampC => "#d62728",
        Layer::RampR => "#2ca02c",
        Layer::Upper => "#9467bd",
    }
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
    let poly = |s: &mut String, pts: &[Vector], closed, style: &str| poly(s, top, pts, closed, style);
    let circle = |s: &mut String, c, r, style: &str| circle(s, top, c, r, style);

    poly(&mut s, &layout.upper_outline, true, "fill:#9467bd22;stroke:none");
    for r in &layout.ramps {
        let mut outline = r.left.clone();
        outline.extend(r.right.iter().rev());
        let c = layer_color(r.layer);
        writeln!(s, "<g opacity=\"0.15\">").unwrap();
        poly(&mut s, &outline, true, &format!("fill:{c};stroke:none"));
        writeln!(s, "</g>").unwrap();
    }
    for w in &layout.walls {
        let width = if w.layer == Layer::Playfield { 3 } else { 2 };
        poly(&mut s, &w.pts, w.closed, &format!("stroke:{};stroke-width:{width}", layer_color(w.layer)));
    }
    for sen in &layout.sensors {
        let style = "fill:#ffbf0040;stroke:#e6a000;stroke-width:1";
        match sen.shape {
            Shape::Rect { min, max } => poly(
                &mut s,
                &[min, Vector::new(max.x, min.y), max, Vector::new(min.x, max.y)],
                true,
                style,
            ),
            Shape::Circle { c, r } => circle(&mut s, c, r, style),
        }
    }
    for b in &layout.bumpers {
        circle(&mut s, b.center, b.radius, "fill:#c83c50;stroke:#000");
    }
    for sl in &layout.slings {
        poly(&mut s, &[sl.a, sl.b, sl.c], true, "stroke:#000;stroke-width:2");
        poly(&mut s, &[sl.a, sl.b], false, "stroke:#00a0ff;stroke-width:4");
    }
    for d in &layout.drops {
        let (lo, hi) = (d.center - d.half, d.center + d.half);
        poly(&mut s, &[lo, Vector::new(hi.x, lo.y), hi, Vector::new(lo.x, hi.y)], true, "fill:#f80;stroke:#000");
    }
    for st in &layout.standups {
        poly(&mut s, &[st.a, st.b], false, "stroke:#f0f;stroke-width:5");
    }
    for g in &layout.gates {
        poly(&mut s, &[g.a, g.b], false, "stroke:#f80;stroke-width:3;stroke-dasharray:6 3");
    }
    for h in &layout.holes {
        circle(&mut s, h.center, h.radius, "fill:#000");
    }
    for f in &layout.flippers {
        let pose = Pose::new(f.pivot, f.rest);
        let pts: Vec<Vector> = f.outline.iter().map(|&p| pose * p).collect();
        poly(&mut s, &pts, true, "fill:#f60;stroke:none");
    }
    for (path, color) in traces {
        poly(&mut s, path, false, &format!("stroke:{color};stroke-width:1.5"));
        if let Some(&end) = path.last() {
            circle(&mut s, end, layout.ball.radius, &format!("fill:{color}"));
        }
    }
    writeln!(s, "</svg>").unwrap();
    s
}
