# Table files

A table is a JSON file. Run it with

```
cargo run -p proto -- tables/simple.json
```

and press **F5** in the game to reload the file after editing it.

**Units:** metres, kilograms, degrees, m/s.
**Coordinates:** the origin is the bottom-left corner of the table, +x points right, +y points up the table (away from the player).
**Angles:** counter-clockwise from +x (0° = right, 90° = up the table).

Unknown fields are errors, so a typo is reported instead of silently ignored.

## Table

| Field | Required | Default | Meaning |
|---|---|---|---|
| `name` | no | `""` | Shown in the game |
| `size` | yes | | `[width, height]` |
| `angle_deg` | yes | | Tilt of the table |
| `gravity` | no | `9.81` | Full gravity. The ball feels `gravity × sin(angle)` along the table. A larger value makes the game faster; simple.json uses 22 (2.25× real, plays like a real table sped up 1.5×) |
| `curve_tolerance` | no | `0.0005` | Curves are turned into short straight segments for the physics; this is how far (m) those may stray from the true curve. Anything under ~1 mm is invisible in play |
| `ball` | yes | | See below |
| `launcher` | yes | | See below |
| `layers` | yes | | List of layers, see below. The first one is the playfield, where new balls start |

**`ball`**

| Field | Required | Default | Meaning |
|---|---|---|---|
| `radius` | yes | | A real ball is 0.0135 |
| `mass` | no | `0.08` | Only matters when two balls collide |
| `restitution` | no | `0.9` | Softness. Every bounce is `wall restitution × ball restitution` |

**`launcher`:** a new ball waits at `position` and is shot off at `start_velocity` (`[vx, vy]`, m/s) when you press Space.

## Layers

A layer is a surface the ball rolls on: the playfield, a ramp, an upper level. A ball only touches the elements on its own layer, so ramps can pass over the playfield and over each other. Balls move between layers at transitions (see below); a ramp layer creates its own.

| Field | Required | Default | Meaning |
|---|---|---|---|
| `id` | yes | | Name of the layer. Shares one namespace with element ids |
| `rolling_resistance` | no | `0.01` | The ball slows down by `rolling_resistance × gravity` while rolling on this layer. Wire ramps can differ from plastic or wood |
| `defaults` | no | | `{ "restitution": …, "friction": … }` for walls on this layer that don't set their own |
| `height` | no | `0` | Height above the playfield, e.g. `0.065` for an upper level (used for drawing) |
| `ramp` | no | | Makes the layer a ramp, see below |
| `elements` | no | `[]` | Walls, flippers, switches and transitions on this layer |

### Ramps

A layer with a `ramp` is a raised track. It generates its rails, its slope (the ball slows down climbing and speeds up coming down) and two transitions:

- `<id>.entrance` on the `from` layer, **two-way**: a weak shot that rolls back returns to `from`.
- `<id>.exit`, **one-way**: the ball lands on `to`; balls passing under the ramp's end are not lifted onto it.

| Field | Required | Default | Meaning |
|---|---|---|---|
| `center` | yes | | Centreline from entrance to exit: `{ "start": [x, y], "segments": [...] }`, segments as in `path` |
| `width` | yes | | Width between the rails (a ball is 0.027) |
| `heights` | yes | | `[position, height]` pairs: position along the centreline from 0 (entrance) to 1 (exit), height in metres. Linear in between |
| `from` | yes | | Layer the entrance is on |
| `to` | yes | | Layer the ball lands on at the exit |
| `rails` | no | `"both"` | `"both"`, `"left"`, `"right"` or `"none"` (left/right as seen walking up the ramp). Rails are named `<id>.left_rail` / `<id>.right_rail` and use the layer's `defaults` |

Example: [bench_ramp.json](bench_ramp.json) has a loop ramp, a lift ramp to an upper level, and a hole back down.

At most 32 layers.

## Walls

Every wall has `type`, its geometry, and optionally:

| Field | Default | Meaning |
|---|---|---|
| `id` | none | Name the rules refer to it by, e.g. `"left_ramp"`. Must be unique in the table |
| `restitution` | layer default, else `0.35` | Bounce, 0 to 1 |
| `friction` | layer default, else `0.075` | Friction along the wall |
| `kick` | none | Active element: on contact the ball is pushed away from the wall at this speed (m/s). A closed `arc` with `kick` is a pop bumper (or a pin between the flippers), a `line` with `kick` a slingshot |
| `one_way` | none | `[dx, dy]`: a gate. Balls moving in this direction pass through, others bounce |

A wall with an `id` is also a standup target: touching it is an event the rules can react to.

| `type` | Fields |
|---|---|
| `line` | `from`, `to` |
| `arc` | `center`, `radius` (a number, or `[rx, ry]` for an ellipse), `start_deg`, `end_deg`, optional `rotation_deg` (turns the ellipse). Goes counter-clockwise when `end_deg > start_deg`, clockwise otherwise. A sweep of 360° makes a closed circle |
| `bezier` | `points`: start, control, control, end, then two controls and an end for each further segment (4, 7, 10, … points) |
| `polyline` | `points`, optional `closed` (default `false`) |
| `path` | `start`, `segments`, optional `closed` (adds a line back to `start`). Each segment starts where the previous one ended, so there are no gaps |

**`path` segments**

| Segment | Meaning |
|---|---|
| `{ "line": [x, y] }` | Straight line to this point |
| `{ "arc": { "center": [x, y], "sweep_deg": a } }` | Circular arc around `center`, from the current point, turning `a` degrees (counter-clockwise if positive) |
| `{ "bezier": [[c1x, c1y], [c2x, c2y], [x, y]] }` | Cubic Bézier: two control points, then the end point |

Example: the outer wall of [simple.json](simple.json) is one path, from the left flipper guide up, around the top arch and down to the right flipper guide.

## Switch

Doesn't block the ball; reports when a ball rolls over it (rollover lanes, spinners). Give exactly one shape:

| Field | Meaning |
|---|---|
| `id` | Name of the switch |
| `center` + `radius` | A circle |
| `min` + `max` | A rectangle |
| `from` + `to` | A line |

## Transition

A line that moves balls between layers, e.g. a hole in an upper level. A ball on this layer that crosses the line from `from` to `to` in `direction` moves to `target`.

| Field | Required | Default | Meaning |
|---|---|---|---|
| `id` | no | | Name |
| `from`, `to` | yes | | The line |
| `direction` | yes | | `[dx, dy]`, which way leads to `target` |
| `target` | yes | | Layer id |
| `two_way` | no | `false` | Balls on `target` crossing back return to this layer |

The layer is decided when the ball has crossed the line, by which side its centre is on, so a ball that stops on the line and rolls back stays where it was.

## Flipper

| Field | Required | Default | Meaning |
|---|---|---|---|
| `id` | no | | Name the rules refer to it by |
| `pivot` | yes | | Where the flipper turns |
| `rest_deg` | yes | | Angle when the key is released |
| `up_deg` | yes | | Angle when the key is held |
| `key` | yes | | `"left"` or `"right"` |
| `speed_deg_per_s` | no | `3150` | How fast it turns (a 60° stroke in about 19 ms) |
| `restitution` | no | `0.25` | |
| `friction` | no | `0.35` | |
| `shape` | yes | | List of curves (same types as walls, without `id`/`restitution`/`friction`), drawn with the pivot at `[0, 0]` and pointing right (+x). The flipper is the convex hull of the outline |

A right flipper uses the same shape as a left one, turned around: for example `rest_deg: 210` and `up_deg: 150` points it down-left at rest. See [simple.json](simple.json).
