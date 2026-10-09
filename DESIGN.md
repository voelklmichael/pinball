# Bavarian Pinball: design notes

A pinball game written in Rust, modelled on *Pro Pinball: Timeshock!* (1997) but with a Bavarian theme.

## Decisions (2026-10-08)

1. **Graphics: 2.5D.** Physics runs in 2D (rapier2d) with separate layers for ramps and raised parts. The table is drawn in a tilted perspective view with pre-rendered-style art, close to how Timeshock looks.
2. **Theme: a tour through the 7 Regierungsbezirke** (Bavaria's administrative regions). Each region is one mode or mission, in place of Timeshock's time eras. Finishing all of them unlocks a final wizard mode.
3. **Copyright:** Timeshock is used only for ideas about layout and rules. None of its graphics, sounds or text are reused.

## The 7 regions (first ideas for each mode; all open for change)

| Region | Main city | Mode idea |
|---|---|---|
| Oberbayern | München | Oktoberfest: shoot the beer-tent ramp, or a Neuschwanstein castle shot |
| Niederbayern | Landshut | Landshuter Hochzeit (historic wedding festival): a knights' tournament |
| Oberpfalz | Regensburg | Steinerne Brücke (old stone bridge): cross the bridge via a ramp combo |
| Oberfranken | Bayreuth | Wagner festival, Bamberg smoked beer |
| Mittelfranken | Nürnberg | Christmas market (Christkindlesmarkt), Lebkuchen gingerbread |
| Unterfranken | Würzburg | Wine harvest, the Residenz palace |
| Schwaben | Augsburg | Fuggerei (historic housing estate), Allgäu cows |

- Multiball: "Maßkrug-Multiball" (beer-mug multiball)
- Wizard mode: still to be decided, e.g. "Wiesn-Finale" (Oktoberfest finale)

## Table layout (proposal, 2026-10-09)

The table is a standard wide-body layout. Each main shot is a ramp or lane the player aims at, and each mode tells you which shots to hit.

```
     ___________________________________
    /   B     I     E     R   (rollovers) \
   /     (pop)   (pop)                      \
  |  ←       (pop)        [ UPPER LEVEL      |  |
  |  |   Biergarten          Neuschwanstein  |  |
  |  |                       mini-playfield  |  |
  |  |                       + upper flipper]|  |
  |  |  [ Kegelbahn: 5 drop targets ]        |  |
  |  |                                     |  |  plunger
  |  L.orbit  L.ramp  Scoop   C.ramp R.ramp R.orbit |  lane
  |  (spinner) Zugspitze Rathaus Bierzelt Brücke    |  |
  |                                         |  |
  |        [ map of Bavaria: 7 region lights ]  |
  |   sling                       sling     |  |
  |  |\  inlane              inlane  /|     |  |
  |  |  \___ flipper    flipper ___/  |     |  |
  |  outlane (kickback)        outlane   |__|
   \___________________   ____________________/
                       drain
```

| # | Shot | Name / theme | What it does |
|---|---|---|---|
| 1 | Left orbit + spinner | Autobahn | Loop that feeds the pop bumpers; the spinner adds points |
| 2 | Left ramp (wire ramp) | Zugspitzbahn (cable car) | Feeds the right inlane; fast combo shot |
| 3 | Centre scoop | Rathaus (town hall) | **Starts region modes** and collects awards |
| 4 | Centre ramp | Bierzelt (beer tent) | Ball lock for Maßkrug-Multiball |
| 5 | Right ramp | Steinerne Brücke (stone bridge) | Raised ramp up to the Neuschwanstein upper level |
| 6 | Right orbit | Alpenstraße (Alpine road) | Loop back to the top rollovers |
| 7 | Drop targets (5) | Kegelbahn (bowling alley) | Clearing the bank lights the scoop for a mode start |
| 8 | Pop bumpers (3) | Biergarten | Jackpot values build up here |
| 9 | Top rollovers B-I-E-R | — | Light all four to raise the bonus multiplier; flipper buttons rotate which lanes are lit |

**Physics layers (for the 2.5D engine):** main playfield, wire ramps, and the upper level. The ball switches layer when it passes a sensor at a ramp entrance or exit, which changes its rapier collision group.

## Rules (proposal)

### Region modes
1. Clear the Kegelbahn drop targets: the Rathaus scoop lights.
2. Shoot the scoop: pick a region with the flipper buttons. Only regions you haven't finished can be picked.
3. A mode has no timer. It runs until its goal is reached or the ball drains. Reaching the goal finishes the region and lights it on the Bavaria map. A mode lost to a drain can be started again later.
4. Once all 7 are lit, the scoop starts the wizard mode.

| Region | Mode | Goal |
|---|---|---|
| Oberbayern | Oktoberfest | Hit the Bierzelt ramp 5 times; each hit is worth more |
| Niederbayern | Ritterturnier (knights' tournament) | Alternate left and right orbits 6 times, like charging with a lance |
| Oberpfalz | Steinerne Brücke | Combo: left ramp, then right ramp, 3 times |
| Oberfranken | Festspiele (Bayreuth opera festival) | Lit shots move around the table in a fixed order, Wagner's "Ring"; hit 4 |
| Mittelfranken | Christkindlesmarkt (Christmas market) | Hurry-up: collect "Lebkuchen" at lit shots while the value counts down |
| Unterfranken | Weinlese (grape harvest) | Clear the Kegelbahn twice |
| Schwaben | Fuggerei | Spinner and bumpers build a pot of money; collect it at the scoop |

### Multiball and other features
- **Maßkrug-Multiball:** lock 3 balls on the Bierzelt ramp, then 3-ball multiball. The Rathaus scoop is lit for the jackpot, which gets bigger with each Biergarten bumper hit.
- **Neuschwanstein (upper level):** skill shots with the upper flipper. Gives extra ball and similar awards.
- **Wizard mode: Wiesn-Finale (Oktoberfest finale):** 4-ball multiball where every shot is lit, then a super jackpot at the scoop.
- **Standard features:** ball save for the first 10 s of each ball, left outlane kickback (relit at the drop targets), skill shot when plunging, extra ball, high score table, tilt.

### Scoring (first guess, tune in playtesting)
| Item | Points |
|---|---|
| Bumper / sling / rollover | 1,000 / 100 / 500 |
| Ramp / orbit | 10,000, more in combos |
| Finishing a region mode | 1,000,000 × number of regions finished |
| Multiball jackpot | 2,000,000+ |
| Wiesn-Finale super jackpot | 25,000,000 |
| End-of-ball bonus | (regions + ramps) × multiplier from the B-I-E-R lanes |

### Display
A 128×32 dot-matrix display in the style of the 1990s: score, mode intros, animations, and a small video mode later, maybe "Maßkrug stemmen" (beer-mug holding contest).

## Decisions (2026-10-09)

- Shot names and region modes: kept as above.
- Neuschwanstein upper level: kept.
- Mode length: no timer; a mode runs until its goal is reached or the ball drains.
- Language for display text and voice: English. Bavarian names (Bierzelt, Wiesn, …) stay as proper names.

## Division of work

- **Claude:** physics, table geometry and a level editor, rules engine, dot-matrix display system, rendering, placeholder art and sound.
- **Me:** game design decisions, playtesting, final art, sound and voice (ideally in Bavarian dialect).

## Next steps

1. ~~Expand this document: layout, modes, scoring~~ (done, decisions recorded above).
2. ~~Extend the prototype to the layout above~~ (done 2026-10-09: all shots, layers, upper level, scoop, Kegelbahn, gates, kickback; headless shot tests write `target/svg/*.svg`).
3. Rules engine (modes, multiball) on top of game events: "ramp hit", "target down", and so on.
4. 2.5D view and dot-matrix display.
