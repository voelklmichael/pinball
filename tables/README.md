# Table files

A table is a JSON file. Run it with

```
cargo run -p pinball-proto -- tables/simple.json
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
| `gravity` | no | `9.81` | Full gravity. The ball feels `gravity × sin(angle)` along the table. A larger value makes the game faster; the built-in table uses 22 |
| `ball` | yes | | See below |
| `inbound` | yes | | See below |
| `elements` | yes | | List of walls and flippers |

**`ball`**

| Field | Required | Default | Meaning |
|---|---|---|---|
| `radius` | yes | | A real ball is 0.0135 |
| `mass` | no | `0.08` | Only matters when two balls collide |
| `restitution` | no | `0.9` | Softness. Every bounce is `wall restitution × ball restitution` |

**`inbound`:** a new ball waits at `position` and is shot off at `velocity` (`[vx, vy]`, m/s) when you press Space.

## Walls

Every wall has `type`, its geometry, and optionally:

| Field | Default | Meaning |
|---|---|---|
| `restitution` | `0.35` | Bounce, 0 to 1 |
| `friction` | `0.075` | Friction along the wall |

| `type` | Fields |
|---|---|
| `line` | `from`, `to` |
| `arc` | `center`, `radius` (a number, or `[rx, ry]` for an ellipse), `start_deg`, `end_deg`, optional `rotation_deg` (turns the ellipse). Goes counter-clockwise when `end_deg > start_deg`, clockwise otherwise. A sweep of 360° makes a closed circle |
| `bezier` | `points`: start, control, control, end, then two controls and an end for each further segment (4, 7, 10, … points) |
| `polyline` | `points`, optional `closed` (default `false`) |

## Flipper

| Field | Required | Default | Meaning |
|---|---|---|---|
| `pivot` | yes | | Where the flipper turns |
| `rest_deg` | yes | | Angle when the key is released |
| `up_deg` | yes | | Angle when the key is held |
| `key` | yes | | `"left"` or `"right"` |
| `speed_deg_per_s` | no | `3150` | How fast it turns (a 60° stroke in about 19 ms) |
| `restitution` | no | `0.25` | |
| `friction` | no | `0.35` | |
| `shape` | yes | | List of curves (same types as walls, without `restitution`/`friction`), drawn with the pivot at `[0, 0]` and pointing right (+x). The flipper is the convex hull of the outline |

A right flipper uses the same shape as a left one, turned around: for example `rest_deg: 210` and `up_deg: 150` points it down-left at rest. See [simple.json](simple.json).
