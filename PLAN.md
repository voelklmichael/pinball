# Pro Pinball: Timeshock! — Rust Rewrite Plan

Goal: a native Rust reimplementation of Timeshock! that plays like the original
and runs at FullHD (1440×1080 table area, pillarboxed on 16:9). It loads the
original game data from the user's own install, the way ScummVM and OpenMW do.

## Ground rules

- **Never ship original assets or decompiled code.** The engine reads `DAT/` and
  `MUSIC/` from a path the user configures. The repo contains only our own code
  and documentation.
- **Reimplement from understanding, not by transliterating.** Ghidra output
  serves as a reference for behaviour, and the Rust code is written idiomatically.
  Physics numerics are the exception and should be copied exactly (see below).
- **The original is the oracle.** Every subsystem gets validated against the
  original running under Wine or DOSBox: screenshots, traces and recorded games.
- **Fixed-step, deterministic simulation.** The physics and rules run headless
  at the original tick rate. Rendering interpolates on top. Input can be
  recorded and replayed, which gives regression tests for free.

## Targets

| Binary | Use |
|---|---|
| `Timeshock!.exe` (PE32, Watcom C, DDraw/DSound/DInput) | **Primary RE target.** Ghidra loads it natively and the imports have names. |
| `PINBALL.EXE` (LE, DOS4GW) | Secondary, for cross-checking. The DOSBox-X debugger is useful for tracing. |
| `PICN*.DLL` (Pegasus JPEG) | Not needed. Use the `image`/`zune-jpeg` crates. |

The binary contains source paths (`\pinball2\src\pc\display\jpeg.c`,
`\pinball2\src\pc\win\display\ddraw.c`, …). Use them to name functions and
recover the original module layout.

## Known data layout

- `DAT/PC/V{1-3}{1-5}/` and `V0{1-5}`: 4 camera views × 5 resolutions
  (640×480 … **1600×1200**). Each set has `VIEW.BIT` (a `CMP1`-compressed 24-bit
  background), `VIEW.MAP`, `VIEW.SET`, plus the sprites `BALL`, `BALLSHAD`,
  `FLIP*`, `UNFLIP*`, `LIGHTS`, `MOVE` (`.SPR`) and `.ZMP` files.
  **Only the `x5` sets (1600×1200) are needed for FullHD.**
- `DAT/GEN/{COLLIDE,MATRIX,TABLES,FONTS}`: these probably hold the collision
  geometry, DMD content, table/rules data and fonts.
- `DAT/PC/SFX`: `.11C/.22C/.44C` (per sample rate), `.DMS/.2MS`, `.BSF/.BMF`.
- `DAT/PC/SLIDES`, `ZOOM`, `BACKBOX`, `LOGOS`: JPEGs plus `.PAL/.LUT`.
- `MUSIC/Track02-33.ogg`: the CD audio tracks, which come as Ogg files in the
  GOG release.

## Workspace layout

Convert the repo into a Cargo workspace. The existing `decompiler` crate
becomes `ts-tools`.

```
crates/
  ts-formats   # pure parsers: CMP1, BIT, SPR, ZMP, MAP, SET, PAL/LUT, SFX, DMD, COLLIDE, TABLES
  ts-tools     # CLI: dump assets to PNG/WAV/JSON, inspect files, diff traces
  ts-physics   # ball/flipper/collision simulation, no I/O, deterministic
  ts-rules     # switches -> events -> game state machine, scoring, modes
  ts-dmd       # dot-matrix display: fonts, animations, scripted shows
  ts-audio     # SFX mixer, music sequencing, speech priority (kira or cpal)
  ts-render    # wgpu compositor: background, sprites, lights, scaling, side panels
  ts-game      # app: winit window, input (gilrs + keyboard), menus, config, main loop
docs/formats/  # one markdown file per reversed file format
```

`ts-physics` and `ts-rules` must not depend on rendering or audio, so they can
run in tests and in a headless trace runner.

## Phases

### Phase 0: Setup (≈1 week)
- `git init`, set up the workspace and CI (fmt, clippy, tests).
- ✅ Original runs under Wine at 1600×1200 via cnc-ddraw (borderless,
  aspect kept, `inject_resolution=1600x1200`). Launch it with
  `tools/run_timeshock.sh`. The Windows build stores its settings in `wgame.cfg`.
  Never run it with Wine's builtin ddraw on the real display, because it changes
  the monitor's mode.
- Ghidra project: load `Timeshock!.exe`, apply Watcom runtime signatures, label
  the DDraw/DSound/DInput call sites and name functions from the source-path
  strings. Export function lists to `docs/re/` as we go.
- **Exit:** the original is playable locally, and Ghidra has the runtime and
  imports labelled.

### Phase 1: Asset formats (≈2–4 weeks)
Order: `CMP1`/`.BIT` → `.PAL/.LUT` → `.SPR` → `.ZMP` → `.MAP/.SET` → SFX → DMD → `COLLIDE` → `TABLES`.
- Find each loader in Ghidra (search for the file-name strings), document the
  format in `docs/formats/` and write the parser plus a dumper in `ts-tools`.
- Test by round-tripping or decoding every file in `DAT/` without errors. Check
  images visually against screenshots of the original.
- **Exit:** `ts-tools dump <install>` extracts every asset into viewable files.
  The 1600×1200 table view comes out as a PNG.

### Phase 2: Static renderer (≈3–4 weeks)
- wgpu window with the table drawn at 0.9× (1600×1200 → 1440×1080), centred,
  and side panels for backbox art and the DMD.
- Place sprites at their rest positions, toggle light states from `LIGHTS.SPR`,
  switch between the 4 camera views.
- A debug overlay that draws the collision geometry from `COLLIDE` over the
  table. This proves the coordinate mapping between physics space and screen.
- **Exit:** the table looks identical to the original at rest, with lights
  that can be driven by hand.

### Phase 3: Physics (≈2–4 months)
- Find the main loop, the tick rate, the ball state struct and the collision
  routines in Ghidra.
- **Numerics:** work out whether it uses fixed-point or floats. If floats, note
  that Watcom/x87 computes at 80-bit precision. Copy the order of operations
  exactly, and decide early whether we aim for bit-exact or behavioural parity.
- **Tracing harness:** log the ball position and velocity every tick from the
  original (a debugger under Wine, or DOSBox-X), feed the same flipper inputs to
  `ts-physics` and diff the two traces with `ts-tools trace-diff`.
- Covers flippers, ball-to-wall and ball-to-object collisions, slingshots,
  bumpers, ramps, the plunger, nudge and tilt, and moving parts (`MOVE.SPR`/`.ZMP`).
- **Exit:** the ball can be played freely on the table. Recorded traces stay
  within tolerance of the original for N seconds of play.

### Phase 4: Rules, DMD and audio (≈3–6 months, the biggest phase)
- Map switches and lamps first: every switch ID → its physical object, and every
  lamp ID → its `LIGHTS.SPR` frame.
- Rebuild the rules as an explicit state machine in `ts-rules`, one feature at a
  time, each with replay tests:
  1. Attract mode, starting a game, ball drain, end of game, high scores
  2. Scoring basics, bonus, ball save, skill shot
  3. Time-period modes and progression
  4. Multiball(s)
  5. Wizard mode, extra balls, specials, match
- DMD: font rendering, animation playback (`MATRIX` data) and the scripted
  shows the rules trigger.
- Audio: SFX playback, music track selection and transitions, speech
  priority and ducking.
- **Exit:** a full game from attract mode to high-score entry is
  indistinguishable from the original.

### Phase 5: Polish (≈1–2 months)
- Menus, options, key and gamepad remapping, config file, operator settings
  (difficulty, balls per game, and so on).
- Side panels for widescreen, VSync and frame pacing, optional integer or
  smooth scaling, windowed and fullscreen modes.
- Packaging for Linux and Windows. First run asks for the game install path and
  checks the data files.

## Timeline (one person, part-time, with Claude)

| Milestone | Cumulative |
|---|---|
| M0: Tooling and the original runs | ~1 week |
| M1: All assets extracted | ~1–1.5 months |
| M2: Static table on screen in FullHD | ~2–2.5 months |
| M3: Playable ball physics | ~4–6 months |
| M4: Full game rules | ~8–12 months |
| M5: Release-quality | ~9–15 months |

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Physics feel differs from the original | Trace-diff harness from day one. Copy numerics exactly before "cleaning up". |
| Rules are huge and data-driven in opaque ways | Reverse `TABLES` early, since the rules may be table-driven. Build the switch/lamp map first. |
| Undocumented formats (`.SET`, `.ZMP`, `.BSF`, `.DMS`) | The loaders in the binary explain them. Do formats before logic. |
| Original won't run under Wine (DInput error) | Fall back to the DOS build in DOSBox-X, which also has a better debugger. |
| Scope creep (4K, 3D re-render, other Pro Pinball tables) | Out of scope until M5. Keep the formats crate generic enough to try other tables later. |

## Immediate next steps

1. Convert to a workspace, `git init`.
2. Reverse `CMP1` (find the loader that references `VIEW.BIT`) and write
   `ts-formats::cmp1`.
3. `ts-tools dump-bit DAT/PC/V05/VIEW.BIT out.png` and check that the
   1600×1200 table looks right.
