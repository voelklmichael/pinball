//! Headless checks of physics and table files, on tables/simple.json. Ball
//! paths are written to target/svg/*.svg for eyeballing.

use rapier2d::math::Vector;

use crate::layout::Layout;
use crate::physics::{Event, PHYSICS_HZ, Table};

const SIMPLE: &str = include_str!("../../tables/simple.json");

fn simple() -> Layout {
    crate::tablefile::parse(SIMPLE).unwrap()
}

struct Run {
    trace: Vec<Vector>,
    events: Vec<Event>,
}

fn run(table: &mut Table, secs: f32) -> Run {
    let mut trace = Vec::new();
    let mut events = Vec::new();
    for i in 0..(secs * PHYSICS_HZ) as usize {
        events.extend(table.step().into_iter().map(|t| t.event));
        if i % 4 == 0
            && let Some(b) = table.balls.first()
        {
            trace.push(table.ball_pos(b));
        }
        if table.balls.is_empty() {
            break;
        }
    }
    Run { trace, events }
}

fn write_svg(layout: &Layout, name: &str, traces: &[(Vec<Vector>, &str)]) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/svg");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/{name}.svg"), crate::svg::render(layout, traces)).unwrap();
}

#[test]
fn simple_table_loads_and_plays() {
    let layout = simple();
    assert_eq!(layout.flippers.len(), 2);
    assert_eq!(layout.walls[0].id.as_deref(), Some("outer_wall"));
    let mut table = Table::new(layout);
    table.serve_ball();
    run(&mut table, 0.2);
    assert!(table.ball_waiting());
    table.launch_key(true);
    let r = run(&mut table, 3.0);
    let max_y = r.trace.iter().map(|p| p.y).fold(0.0, f32::max);
    write_svg(&table.layout, "simple", &[(r.trace, "#e41a1c")]);
    assert!(max_y > 0.95, "launch did not go round the arch: max y {max_y}");
}

#[test]
fn served_ball_waits_for_launch() {
    let mut table = Table::new(simple());
    table.serve_ball();
    run(&mut table, 1.0);
    assert!(table.ball_waiting());
    assert_eq!(table.ball_pos(&table.balls[0]), table.layout.launcher.position);
}

#[test]
fn flippers_hit_the_ball() {
    for (i, button) in [(0, (true, false)), (1, (false, true))] {
        let mut table = Table::new(simple());
        let def = table.flippers[i].def.clone();
        // Middle of the flipper, just above it.
        let mid = def.pivot + Vector::new(def.rest.cos(), def.rest.sin()) * 0.04 + Vector::new(0.0, 0.03);
        let ball = table.add_ball(mid, Vector::ZERO);
        run(&mut table, 0.3);
        table.set_buttons(button.0, button.1);
        let mut max_vy: f32 = 0.0;
        for _ in 0..(0.1 * PHYSICS_HZ) as usize {
            table.step();
            let b = table.balls.iter().find(|b| b.number == ball).unwrap();
            max_vy = max_vy.max(table.ball_vel(b).y);
        }
        assert!(max_vy > 1.5, "flipper {i} hit too weak: vy {max_vy}");
    }
}

#[test]
fn ball_drains_between_flippers() {
    let mut table = Table::new(simple());
    table.add_ball(Vector::new(table.layout.size.x / 2.0, 0.30), Vector::ZERO);
    let r = run(&mut table, 2.0);
    assert!(r.events.iter().any(|e| matches!(e, Event::Drain { .. })), "no drain: {:?}", r.events);
}

/// Prints the ball speed off the left flipper for different contact points.
#[test]
#[ignore]
fn measure_flipper_speed() {
    for d in [0.015, 0.025, 0.035, 0.045, 0.055, 0.065] {
        let mut table = Table::new(simple());
        let def = table.flippers[0].def.clone();
        // Let the ball settle against the flipper's top edge, like a cradle.
        let rest_dir = Vector::new(def.rest.cos(), def.rest.sin());
        let normal = Vector::new(-rest_dir.y, rest_dir.x);
        let p = def.pivot + rest_dir * d + normal * (table.layout.ball.radius + 0.011 + 0.001);
        table.add_ball(p, Vector::ZERO);
        table.step();
        table.set_buttons(true, false);
        let mut best = Vector::ZERO;
        for _ in 0..(0.15 * PHYSICS_HZ) as usize {
            table.step();
            if let Some(b) = table.balls.first() {
                let v = table.ball_vel(b);
                if v.length() > best.length() {
                    best = v;
                }
            }
        }
        println!("d={d:.3}  speed={:.2}  angle={:.0}°", best.length(), best.y.atan2(best.x).to_degrees());
    }
}

#[test]
fn table_file_errors_name_the_problem() {
    let err = |json: &str| crate::tablefile::parse(json).err().expect("should fail");
    let base = |elements: &str| {
        format!(
            r#"{{"size": [0.5, 1.0], "angle_deg": 6.5, "ball": {{"radius": 0.0135}},
               "launcher": {{"position": [0.4, 0.3], "start_velocity": [0, 3]}},
               "layers": [{{"id": "playfield", "elements": [{elements}]}}]}}"#
        )
    };
    let e = err(&base(r#"{"type": "line", "from": [0, 0], "too": [1, 1]}"#));
    assert!(e.contains("unknown field `too`"), "{e}");
    let e = err(&base(r#"{"type": "line", "from": [0, 0], "to": [0, 1]}, {"type": "bezier", "points": [[0, 0], [1, 1]]}"#));
    assert!(e.contains("layers[0] (playfield): elements[1] (bezier)") && e.contains("3n+1"), "{e}");
    let e = err(&base(r#"{"type": "spline", "points": []}"#));
    assert!(e.contains("unknown variant `spline`"), "{e}");
    let e = err(&base(
        r#"{"type": "flipper", "pivot": [0.2, 0.2], "rest_deg": -30, "up_deg": 30, "key": "left",
            "shape": [{"type": "line", "from": [0, 0], "to": [0.07, 0], "restitution": 0.5}]}"#,
    ));
    assert!(e.contains("elements[0] (flipper): shape[0] (line)"), "{e}");
    let e = err(&base(r#"{"type": "arc", "center": [0, 0], "radius": [0.1, 0], "start_deg": 0, "end_deg": 90}"#));
    assert!(e.contains("radius must be > 0"), "{e}");
    let e = err(&base(
        r#"{"type": "line", "id": "a", "from": [0, 0], "to": [0, 1]}, {"type": "line", "id": "a", "from": [1, 0], "to": [1, 1]}"#,
    ));
    assert!(e.contains(r#"elements[1] (line): id "a" is used twice"#), "{e}");
    // Layer ids share the namespace with element ids.
    let e = err(&base(r#"{"type": "line", "id": "playfield", "from": [0, 0], "to": [0, 1]}"#));
    assert!(e.contains(r#"id "playfield" is used twice"#), "{e}");
}

const BENCH_FLAP: &str = include_str!("../../tables/bench_flap.json");

/// Runs a ball from `pos` at `vel` on the flap bench. After every step: the
/// flap's angle from closed (degrees, positive is open) and the ball's height
/// (`None` once it's gone).
fn flap_run(pos: Vector, vel: Vector, secs: f32) -> Vec<(f32, Option<f32>)> {
    let mut table = Table::new(crate::tablefile::parse(BENCH_FLAP).unwrap());
    table.add_ball(pos, vel);
    let closed = table.layout.flaps[0].closed;
    (0..(secs * PHYSICS_HZ) as usize)
        .map(|_| {
            table.step();
            let angle = (table.flap_pose(0).rotation.angle() - closed).to_degrees();
            (angle, table.balls.first().map(|b| table.ball_pos(b).y))
        })
        .collect()
}

#[test]
fn flap_opens_for_a_ball_and_closes_behind_it() {
    let run = flap_run(Vector::new(0.47, 0.30), Vector::new(0.0, 3.0), 1.5);
    let (widest, max) = run.iter().enumerate().fold((0, 0.0f32), |m, (i, &(a, _))| if a > m.1 { (i, a) } else { m });
    assert!(max > 45.0, "flap only opened to {max:.1}°");
    let shut = run[widest..].iter().position(|&(a, _)| a < 1.0).expect("flap never closed");
    assert!((shut as f32 / PHYSICS_HZ) < 0.4, "flap took {:.2}s to close", shut as f32 / PHYSICS_HZ);
    // The ball came back down onto the closed flap and stays above it.
    let (end_angle, end_y) = *run.last().unwrap();
    assert!(end_angle.abs() < 1.0 && end_y.unwrap() > 0.6, "end: flap {end_angle:.1}°, ball y {end_y:?}");
}

#[test]
fn flap_holds_from_behind() {
    for (pos, vel) in [
        (Vector::new(0.47, 0.80), Vector::ZERO),
        (Vector::new(0.47, 0.80), Vector::new(0.0, -3.0)),
        (Vector::new(0.50, 0.80), Vector::new(0.0, -6.0)),
        (Vector::new(0.435, 0.80), Vector::new(0.0, -6.0)),
    ] {
        let run = flap_run(pos, vel, 1.0);
        let lowest = run.iter().filter_map(|r| r.1).fold(f32::MAX, f32::min);
        let worst = run.iter().map(|r| r.0.abs()).fold(0.0, f32::max);
        assert!(lowest > 0.6, "ball from {pos} at {vel} got through: y {lowest:.3}");
        assert!(worst < 1.0, "ball from {pos} at {vel} moved the flap {worst:.1}°");
    }
}

#[test]
fn flap_errors_name_the_problem() {
    let err = |angles: &str| {
        let json = BENCH_FLAP.replace(r#""closed_deg": 0, "open_deg": 80"#, angles);
        crate::tablefile::parse(&json).err().expect("should fail")
    };
    let e = err(r#""closed_deg": 0, "open_deg": 180"#);
    assert!(e.contains("elements[2] (flap): open_deg must be 1 to 179 degrees away from closed_deg"), "{e}");
    let e = err(r#""closed_deg": 10, "open_deg": 370"#);
    assert!(e.contains("open_deg must be"), "{e}");
    let e = err(r#""closed_deg": 0, "open_deg": 80, "spring": 3"#);
    assert!(e.contains("unknown field `spring`"), "{e}");
}
