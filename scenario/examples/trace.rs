//! Debug helper: runs a scenario and prints the first ball's position and
//! velocity every few steps, plus the events. `cargo run -p scenario --example trace -- scenarios/x.json`
use engine::physics::Table;
use scenario::{Player, Scenario};

fn main() {
    let path = std::env::args().nth(1).expect("scenario file");
    let (sc, layout) = Scenario::load(path.as_ref()).unwrap();
    let mut table = Table::new(layout);
    sc.start(&mut table);
    let mut player = Player::new(&sc);
    for i in 0..sc.steps() {
        player.apply(&mut table);
        let ev = table.step();
        if let Some(b) = table.balls.first() {
            let (p, v) = (table.ball_pos(b), table.ball_vel(b));
            if i % 8 == 0 || !ev.is_empty() {
                let names: Vec<_> = ev.iter().map(|e| table.describe(&e.event)).collect();
                println!("{:.3} x={:.4} y={:.3} vx={:+.3} vy={:+.3} {:?}", table.steps() as f32 / 480.0, p.x, p.y, v.x, v.y, names);
            }
        }
    }
}
