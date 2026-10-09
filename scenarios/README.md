# Scenarios

A scenario checks one behaviour: a table, scripted key presses, and what must happen. Every `*.json` file here is a test, run with

```
cargo test -p scenario
```

and can be watched in the game, playing exactly the same inputs:

```
cargo run -p proto -- --scenario scenarios/launch_round_arch.json
```

(R replays it, Esc quits.)

Each test does two things:

1. **The checks in `expect`.** They state what the scenario is about and survive tuning. If one fails, the behaviour is broken.
2. **The snapshot** in `snapshots/`: the full list of events (see below). It catches any change, intended or not.

The ball paths are also drawn to `target/svg/scenarios/<name>.svg` on every run, for looking at.

## The event trace

```
  0.000  serve ball#1 at (0.495, 0.300)
  0.200  launch ball#1 v=(0.00, 4.00)
  0.356  touch ball#1 outer_wall
  0.373  leave ball#1 outer_wall
  ...
  2.167  drain ball#1
    end  ball#1 at (0.163, 0.163) v=(0.00, 0.00)
```

Times are seconds, positions metres (millimetre precision), speeds m/s. Walls and flippers are named by their `id` in the table file (`wall#3` if they have none). A contact only ends after 10 ms without touching, so a ball rolling along a wall doesn't flicker. `end` lines list the balls still on the table at the end.

The same events are logged when you play: `RUST_LOG=debug cargo run -p proto` shows every contact, the default level (`info`) the rest.

## When a snapshot changes

A changed snapshot fails the test and shows the difference. If the change is intended (you tuned the physics or the table), accept it and look at what changed:

```
INSTA_UPDATE=always cargo test -p scenario
git diff scenarios/snapshots
```

(or `cargo insta review`, if `cargo-insta` is installed). If the change isn't intended, it's a bug.

When fixing a physics bug, first add a scenario that shows it, then fix it, so it can't come back.

## Format

| Field | Required | Default | Meaning |
|---|---|---|---|
| `description` | yes | | What this scenario shows, in one sentence |
| `table` | yes | | Table file, relative to the scenario file |
| `duration` | yes | | Seconds to run |
| `serve` | no | `true` | Serve a ball at the launcher at the start |
| `balls` | no | `[]` | Balls placed directly at the start: `{ "position": [x, y], "velocity": [vx, vy] }` (velocity optional) |
| `inputs` | no | `[]` | Key presses: `{ "at": 0.2, "press": "launch" }` or `{ "at": 0.4, "release": "left" }`. Keys: `launch`, `left`, `right` |
| `expect` | no | `[]` | Checks, see below |

### Checks

Events are written like trace lines without time and ball number: `"launch"`, `"touch outer_wall"`, `"leave left_flipper"`, `"flipper_up left_flipper"`, `"drain"`.

| Check | Passes if |
|---|---|
| `{ "event": "touch center_post", "between": [0.2, 0.3] }` | the event happens at least once (within the time window, if given) |
| `{ "never": "drain" }` | the event never happens |
| `{ "sequence": ["launch", "touch outer_wall", "drain"] }` | the events happen in this order; others may come in between |
| `{ "enters": { "min": [x0, y0], "max": [x1, y1] }, "between": [0.2, 0.8] }` | a ball's centre gets into the rectangle (within the time window, if given) |
| `{ "at_end": { "near": [x, y], "within": 0.01, "max_speed": 0.01 } }` | at the end, a ball is within `within` metres of `near` and slower than `max_speed` m/s |

Prefer checks that describe the behaviour ("the ball goes over the top of the arch") over exact numbers; the snapshot already records the exact numbers.
