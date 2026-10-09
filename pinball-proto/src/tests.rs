//! Headless checks of the table geometry. Each shot test fires a ball from the
//! flipper area at a target and checks the expected switches. The ball paths
//! are written to target/svg/*.svg for eyeballing.

use rapier2d::math::Vector;

use crate::layout::{self, Layer, Sw};
use crate::physics::{Event, PHYSICS_HZ, Table};

struct Run {
    trace: Vec<Vector>,
    events: Vec<Sw>,
}

fn run(table: &mut Table, secs: f32) -> Run {
    let mut trace = Vec::new();
    let mut events = Vec::new();
    for i in 0..(secs * PHYSICS_HZ) as usize {
        let evs: Vec<Event> = table.step();
        events.extend(evs.iter().map(|e| e.sw));
        if i % 4 == 0 {
            if let Some(b) = table.balls.first() {
                trace.push(table.ball_pos(b));
            }
        }
        if table.balls.is_empty() {
            break;
        }
    }
    Run { trace, events }
}

fn shoot(from: (f32, f32), at: (f32, f32), speed: f32, secs: f32) -> Run {
    let mut table = Table::new(layout::bavaria());
    let from = Vector::new(from.0, from.1);
    let dir = (Vector::new(at.0, at.1) - from).normalize();
    table.add_ball(from, dir * speed);
    run(&mut table, secs)
}

fn write_svg(name: &str, traces: &[(Vec<Vector>, &str)]) {
    write_svg_for(&layout::bavaria(), name, traces);
}

fn write_svg_for(layout: &layout::Layout, name: &str, traces: &[(Vec<Vector>, &str)]) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/svg");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/{name}.svg"), crate::svg::render(layout, traces)).unwrap();
}

const COLORS: [&str; 8] = ["#e41a1c", "#377eb8", "#4daf4a", "#984ea3", "#ff7f00", "#a65628", "#f781bf", "#17becf"];

/// Positions just above the flipper tips, where a flipped ball leaves from.
const FROM_LEFT: (f32, f32) = (0.19, 0.17);
const FROM_RIGHT: (f32, f32) = (0.29, 0.17);

#[test]
fn shots_reach_their_targets() {
    let shots: Vec<(&str, (f32, f32), (f32, f32), Vec<Sw>)> = vec![
        ("left ramp", FROM_RIGHT, (0.0895, 0.62), vec![Sw::RampEnter(Layer::RampL), Sw::RampMade(Layer::RampL)]),
        ("centre ramp", FROM_LEFT, (0.2425, 0.62), vec![Sw::RampEnter(Layer::RampC), Sw::RampMade(Layer::RampC)]),
        ("right ramp", FROM_LEFT, (0.3275, 0.62), vec![Sw::RampMade(Layer::RampR), Sw::UpperExit]),
        ("scoop", FROM_RIGHT, (0.169, 0.66), vec![Sw::Scoop]),
        ("left orbit", FROM_RIGHT, (0.02, 0.56), vec![Sw::LeftOrbit, Sw::RightOrbit]),
        ("right orbit", FROM_LEFT, (0.461, 0.56), vec![Sw::RightOrbit, Sw::LeftOrbit]),
        ("kegelbahn", FROM_RIGHT, (0.012, 0.44), vec![]),
    ];
    let mut traces = Vec::new();
    let mut failures = Vec::new();
    for (i, (name, from, at, expect)) in shots.into_iter().enumerate() {
        let r = shoot(from, at, 4.0, 3.0);
        let missing: Vec<_> = expect.iter().filter(|sw| !r.events.contains(sw)).collect();
        let drop_hit = r.events.iter().any(|sw| matches!(sw, Sw::Drop(_)));
        if !missing.is_empty() || (name == "kegelbahn" && !drop_hit) {
            failures.push(format!("{name}: missing {missing:?}, got {:?}", r.events));
        }
        traces.push((r.trace, COLORS[i % COLORS.len()]));
    }
    write_svg("shots", &traces);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn full_plunge_reaches_top_lanes() {
    let mut table = Table::new(layout::bavaria());
    table.serve_ball();
    run(&mut table, 0.3);
    table.plunger(true, 1.0);
    table.plunger(false, 0.0);
    let r = run(&mut table, 3.0);
    write_svg("plunge", &[(r.trace.clone(), COLORS[0])]);
    assert!(r.trace.last().unwrap().x < layout::LANE_X, "ball stayed in the plunger lane: {:?}", r.events);
    assert!(
        r.events.iter().any(|sw| matches!(sw, Sw::Rollover(_) | Sw::Bumper(_) | Sw::LeftOrbit)),
        "plunge reached nothing: {:?}",
        r.events
    );
}

#[test]
fn served_ball_waits_for_launch() {
    let mut table = Table::new(layout::bavaria());
    table.serve_ball();
    run(&mut table, 1.0);
    assert!(table.ball_waiting());
    assert_eq!(table.ball_pos(&table.balls[0]), table.layout.launcher.position);
}

#[test]
fn flipper_launches_ball() {
    let mut table = Table::new(layout::bavaria());
    let pivot = table.flippers[0].def.pivot;
    let ball = table.add_ball(pivot + Vector::new(0.04, 0.03), Vector::ZERO);
    run(&mut table, 0.3);
    table.set_buttons(true, false);
    let mut max_vy: f32 = 0.0;
    for _ in 0..(0.1 * PHYSICS_HZ) as usize {
        table.step();
        let b = table.balls.iter().find(|b| b.body == ball).unwrap();
        max_vy = max_vy.max(table.ball_vel(b).y);
    }
    assert!(max_vy > 1.5, "flipper hit too weak: vy {max_vy}");
}

#[test]
fn ball_drains_between_flippers() {
    let mut table = Table::new(layout::bavaria());
    table.add_ball(Vector::new(layout::LANE_X / 2.0, 0.30), Vector::ZERO);
    let r = run(&mut table, 2.0);
    assert!(r.events.contains(&Sw::Drain), "no drain: {:?}", r.events);
}

/// Prints the ball speed off the left flipper for different contact points.
#[test]
#[ignore]
fn measure_flipper_speed() {
    for d in [0.015, 0.025, 0.035, 0.045, 0.055, 0.065] {
        let mut table = Table::new(layout::bavaria());
        let def = table.flippers[0].def.clone();
        // Hold the flipper up and let the ball settle against it, like a cradle.
        let rest_dir = Vector::new(def.rest.cos(), def.rest.sin());
        let normal = Vector::new(-rest_dir.y, rest_dir.x);
        let p = def.pivot + rest_dir * d + normal * (layout::BALL_R + 0.011 + 0.001);
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

const SIMPLE: &str = include_str!("../../tables/simple.json");

#[test]
fn simple_table_loads_and_plays() {
    let layout = crate::tablefile::parse(SIMPLE).unwrap();
    assert_eq!(layout.flippers.len(), 2);
    assert_eq!(layout.walls[0].id.as_deref(), Some("outer_wall"));
    let mut table = Table::new(layout);
    table.serve_ball();
    run(&mut table, 0.2);
    assert!(table.ball_waiting());
    table.plunger(true, 0.0);
    table.plunger(false, 0.0);
    let r = run(&mut table, 3.0);
    let max_y = r.trace.iter().map(|p| p.y).fold(0.0, f32::max);
    write_svg_for(&table.layout, "simple", &[(r.trace, COLORS[0])]);
    assert!(max_y > 0.95, "launch did not go round the arch: max y {max_y}");
}

#[test]
fn json_flippers_hit_the_ball() {
    for (i, button) in [(0, (true, false)), (1, (false, true))] {
        let mut table = Table::new(crate::tablefile::parse(SIMPLE).unwrap());
        let def = table.flippers[i].def.clone();
        // Middle of the flipper, just above it.
        let mid = def.pivot + Vector::new(def.rest.cos(), def.rest.sin()) * 0.04 + Vector::new(0.0, 0.03);
        let ball = table.add_ball(mid, Vector::ZERO);
        run(&mut table, 0.3);
        table.set_buttons(button.0, button.1);
        let mut max_vy: f32 = 0.0;
        for _ in 0..(0.1 * PHYSICS_HZ) as usize {
            table.step();
            let b = table.balls.iter().find(|b| b.body == ball).unwrap();
            max_vy = max_vy.max(table.ball_vel(b).y);
        }
        assert!(max_vy > 1.5, "flipper {i} hit too weak: vy {max_vy}");
    }
}

#[test]
fn table_file_errors_name_the_problem() {
    let err = |json: &str| crate::tablefile::parse(json).err().expect("should fail");
    let base = |elements: &str| {
        format!(
            r#"{{"size": [0.5, 1.0], "angle_deg": 6.5, "ball": {{"radius": 0.0135}},
               "launcher": {{"position": [0.4, 0.3], "start_velocity": [0, 3]}}, "elements": [{elements}]}}"#
        )
    };
    let e = err(&base(r#"{"type": "line", "from": [0, 0], "too": [1, 1]}"#));
    assert!(e.contains("unknown field `too`"), "{e}");
    let e = err(&base(r#"{"type": "line", "from": [0, 0], "to": [0, 1]}, {"type": "bezier", "points": [[0, 0], [1, 1]]}"#));
    assert!(e.contains("elements[1] (bezier)") && e.contains("3n+1"), "{e}");
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
}
