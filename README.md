# Pinball

A pinball engine in Rust: tables are described in JSON files, simulated with the Rapier physics engine and drawn with macroquad; the rules (scoring, modes, multiball) are Rust code on top.
Play the demo table with `cargo run -p proto -- tables/demo.json` (Z / M = flippers, Space = launch).

Crates: `engine` (table files, physics), `rules` (game rules), `scenario` (scripted tests), `proto` (the game).

## Scenarios

A scenario is a small scripted test of one behaviour, e.g. a launched ball following the outer wall. Watch one in the game:

```
cargo run -p proto -- --scenario scenarios/launch_round_arch.json
```

Format and how to update snapshots: [scenarios/README.md](scenarios/README.md).

## Tests

```
cargo test                                    # everything, including all scenarios
cargo test -p scenario                        # only the scenarios
INSTA_UPDATE=always cargo test -p scenario    # accept changed scenario snapshots
```
