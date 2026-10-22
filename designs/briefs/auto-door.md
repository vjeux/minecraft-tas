# An automatic door a player would build

clk-obs, 2026-10-22. Gate G1 (docs/process.md). BACKLOG topic 6. The audit flagged the live pick,
auto-door-lock-v12 (91 blocks), at about 2.3× an estimated 35–40-block obvious door: classic6 plus
plates on both sides.

## Use

A player wants to walk through a hidden 2x2 entrance without pressing anything. The wall opens as they
reach it, from either side, and closes behind them so mobs stay out.

## The number

1. **Blocks** (piston heads not counted, as on the 2x2 board).
2. **Open wait**: the game ticks a walker stands at the closed door. 0 means it opens before you reach
   it, even at a sprint.

Reported only:
- **close delay**: from the last walker leaving the opening to the door closed;
- **stop in doorway**: whether the door stays open for someone standing in the door plane, or closes
  on them.

## Mechanics, from the game code (26.3, `mctas src show <Class>`)

**Pressure plates**
- **Detection box.** `BasePressurePlateBlock.TOUCH_AABB` is `Block.column(14, 0, 4)`: x and z in
  [1/16, 15/16] of the cell, y in [0, 4/16].
- **Pressing.** `entityInside` calls `checkPressed` only while the plate is off. The plate turns on in
  that same tick and schedules a tick in `getPressedTime()` = 20 gt (stone and wood; weighted plates
  use 10).
- **Holding and letting go.** `tick` re-checks, and the plate lets go at the first check that finds
  nobody. So it holds 20 gt after its first press, then lets go on that 20 gt grid.
- **Who presses it.** `PressurePlateBlock.getSignalStrength`: a stone plate counts `LivingEntity`
  only (players, mobs, armor stands that are not markers); a wooden plate counts every entity,
  dropped items included. Entities that are `isIgnoringBlockTriggers` don't count (markers; the
  default is false).
- **Power.** `getDirectSignal` gives power UP only, so a plate strongly powers the block under it.
- **Support.** `canSurvive` needs a supporting block below.
- **Pistons.** `Blocks.STONE_PRESSURE_PLATE` is `PushReaction.POPPED`: a block pushed into it pops it.

**Tripwire**
- **Detection box.** `TripWireBlock.SHAPE_ATTACHED` is `Block.column(16, 1, 2.5)`: the whole cell.
- **Who triggers it.** `checkPressed` counts any entity that is not `isIgnoringBlockTriggers`.
  Dropped items count, so an item lying on the string keeps it pressed until the item despawns.
- **Holding.** It re-checks every `RECHECK_PERIOD` = 10 gt.
- **Hooks.** `TripWireHookBlock.getDirectSignal` strongly powers the hook's attachment block. A string
  needs a hook at each end.
- **Support.** `TripWireBlock` has no `canSurvive`, so string can float.
- **Pistons.** `Blocks.TRIPWIRE` is `PushReaction.POPPED`.

**Torch.** `RedstoneTorchBlock` toggles 2 gt after its base changes (`TOGGLE_DELAY`). It burns out
after 8 toggles in 60 gt (`MAX_RECENT_TOGGLES`, `RECENT_TOGGLE_TIMER`). A plate can only let go on its
20 gt grid, so traffic toggles the torch at most twice per 20 gt (6 per 60): it never burns out.

**Pistons**
- **Timing.** `PistonBaseBlock.checkIfExtend` queues a `level.blockEvent`, run at the end of the tick.
  `triggerEvent` then calls `moveBlocks`: each moved block becomes `moving_piston` at its destination,
  and its old cell turns to air in the same tick. So a door pulled open is clear at once.
- **Motion.** `PistonMovingBlockEntity.tick` adds 0.5 to `progress` per tick: 2 gt of motion, and the
  block lands on the next tick.
- **Quasi-connectivity.** `getNeighborSignal` also checks the neighbours of the cell above the piston,
  so power next to the top piston reaches the bottom one.

**A closing door and the body in it**
- **The push.** `PistonMovingBlockEntity.moveCollidedEntities` moves every entity in a moving block's
  path (`moveEntityByPiston`, `entity.move(MoverType.PISTON, ...)`), stopping at other blocks. Only
  entities with push reaction `IGNORE_ENTITY` are spared, and `ArmorStand` returns that only for
  markers.
- **classic6 closes from both sides,** so a body in the doorway is squeezed. Measured: the judge's
  reported stop, with the armor stand then left alone 40 gt, ends **inside the door's blocks**, at the
  door plane.
- **Damage.** A living entity inside a block takes 1 damage (half a heart) per hurt:
  `LivingEntity.baseTick` calls `isInWall()` and then `hurtServer(inWall, 1.0)`. `Entity.isInWall` tests
  the eye box.
- **Getting out.** A player's own client pushes them out of a block toward open space. That code is
  `LocalPlayer`, client-side, and this box has no client jar, so I have not read it. Pushed out
  0.8 blocks or more from the door plane, the player stands on the plate next to the door
  (|offset| > 0.26), and the door opens again.
- So a player who stands in the doorway is squeezed, takes suffocation damage until they step out,
  and is never trapped. It does not kill an attentive player. This is exactly what the coordinator
  said.

**Iron doors** (layout 5's note).
- `DoorBlock.neighborChanged` sets `OPEN` = powered (either half) in the same tick. Iron doors can't
  be opened by hand (`canOpenByHand`).
- A plate next to the lower half opens it: `getSignal` powers every side.
- A door swinging shut moves nothing (no piston).

**Sculk sensors** (layout 4)
- **Timing.** `SculkSensorBlock.ACTIVE_TICKS` = 30, `COOLDOWN_TICKS` = 10.
- **Steps.** A step is `GameEvent.STEP`, emitted from `Entity.move` (`vibrationAndSoundEffectsFromBlock`).
  A teleported entity makes none.
- **Sneaking.** `VibrationSystem` ignores the event when `isSteppingCarefully()`.
- **Pistons are heard too.** `PistonBaseBlock.triggerEvent` emits `BLOCK_ACTIVATE` and `BLOCK_DEACTIVATE`.

**Walking geometry** (the judge's walker, a 0.5-wide armor stand; a player is 0.6 wide). Offsets are
measured from the middle of the door plane. The plate rows sit right next to the door.
- **Plate zones.** A walker presses the near plate at offset −1.69 (a player at −1.74) and stays on it
  until −0.31 (player −0.26). Walking takes 6.4 gt over that, sprinting 4.9.
- **Blind spot.** In the middle of the 1-deep doorway, |offset| < 0.31 (player 0.26), no plate is
  touched. Walking it takes 2.9 gt, sprinting 2.2.
- **Reaching the door.** From the first plate touch to the door cell is 0.94 block: 4.3 gt walking,
  3.3 sprinting. The obvious door is clear 2 gt after the press (torch, then the block event), so it
  opens before anyone reaches it.

## What a player does at a door: the walks the judge requires

**Required, from real use.** The judge requires each of these; the coordinator named the first three
as clearly real.
- Walking through each way, in the middle and in each column.
- Walking and sprinting.
- Two people in a row.
- Two at every gap one behind the other, and two crossing from opposite sides at every offset (the
  sweeps).
- One walker at every offset across the doorway, 1/32 apart, stand-width and player-width, walking
  and sprinting, both ways (the lateral sweep, added 2026-10-22 08:48; see the search).
- The door must not shut on anyone walking through, must close after the last walker (within 300 gt),
  and must stay closed.

**The two-walker sweeps (asked for yesterday): real, and free.**
- They are what multiplayer traffic looks like: a friend follows you at whatever distance, or someone
  comes the other way.
- The failures they catch are ones a player sees. lock-v1 to lock-v7 passed the simple walks, but for
  some gaps they stayed open with nobody inside, letting mobs in.
- They cost the honest door nothing. The obvious door passes all 142 sweep walks at every position and
  rotation (measured below): a plate pressed on the way in holds 20 gt, and the blind spot takes
  under 3 gt to cross.
- The sweeps only ever made memory doors bigger, and memory exists only for the next item. Keep them.

**Stopping in the door plane: not required, reported.**
- **Why players do it:** a player stops in a doorway now and then, to talk or to look at a map.
- **Why the door can't see them:** a 1-deep doorway is somewhere you pass through, not stand. A player
  in its middle presses no plate, and no detector fits in the door plane: the pistons sweep it, and
  plates and string pop when pushed into (`PushReaction.POPPED`).
- **What it costs to keep the door open for them:** memory, which reads plate order. That is lock-v12
  at 91 blocks against 44, and it still closes on one person standing while another walks through.
- **What it costs the player who stops:** the door closes on them, squeezes them and hurts them until
  they step out. They notice it once.
- That is a lot of blocks to avoid a mild, learnable annoyance. Most players would take the
  44-block door and not stand in the doorway.
- So the judge runs the stop last, as a **reported** walk. The page says
  `stop in doorway: the door closes on them` for the obvious door and `the door stays open` for
  lock-v12, and the player chooses.

## Trade-offs

- **Blocks against open wait.** Plates farther out give the door more time and cost more blocks. The
  obvious door already opens before a sprinter arrives (0 gt), so there is nothing to buy.
- **Plates against tripwire.**
  - Plates need a support each (8 blocks for both sides); string floats but needs two hooks and two
    hook blocks per row (12).
  - Tripwire holds 10 gt, plates 20: string is still enough for the blind spot (a walker reaches the
    far string 9.3 gt after the first touch).
  - Tripwire sees dropped items, so an item on the string holds the door open for 5 minutes (fly's QA
    item 4). Stone plates ignore items.
- **Memory against size.** Memory makes the door safe for someone standing in the doorway, at about
  twice the blocks, and costs open wait (13 gt on lock-v12). A counter that is safe for "one stands
  while another passes" costs more again.
- **Hidden 2x2 against a plain door.** Two iron doors with plates are a hands-free door in about
  10 blocks, and they never push anyone. But they are a visible door, not a flush wall. The 2x2 kind is
  for hidden entrances (BACKLOG need 5 plus 6).
- **Mobs.** No vanilla detector tells a player from a mob: stone plates count any `LivingEntity`,
  tripwire any entity, sculk any vibration. Every automatic door opens for a zombie on its plate. The
  page says so; the judge doesn't test it.

## Five layouts

| # | layout | blocks | open wait | % of floor (≈30–36 b) | constraints |
|---|---|---|---|---|---|
| 1 | **Obvious: one stone-plate row each side, no memory.** classic6's pistons, a torch under each side's pistons (on at rest: closed), the plates' supports feed dust into both torches' bases | **44, measured** | **0 gt, measured** | 68–82% | closes on someone standing in the doorway (squeeze) |
| 2 | Tripwire rows each side instead of plates: hooks in the side columns, string across the walkway, hook blocks feed the torches | 44–50 (est.): +4 for hooks, −4 to −8 if the hook blocks replace the dust under the walkway | 0 gt (est.: 0.7 gt to spare walking) | 60–82% | dropped items hold it open; 10 gt hold is tighter; same stop behaviour |
| 3 | Memory, reading plate order: two rows each side plus a lockable repeater (lock-v8 to lock-v12) | **91, measured** (lock-v12) | 13 gt, measured | 33–40% | safe for one stopper; still closes on one standing while another passes |
| 4 | Sculk sensor: one sensor hears steps on both sides, wool shields it from the pistons | 30–35 (est.) | 0–2 gt (est.) | ~100% | **not judgeable** (teleported walkers make no steps); deaf to sneaking; hears mobs, items, projectiles and its own pistons |
| 5 | Counter memory: plate pairs give direction, a counter holds the door while anyone is between the rows | 120–150 (est.) | ~5 gt (est.) | 20–30% | the only one safe for "one stands while another passes"; far from anything players build |

Iron doors with plates (about 10 blocks) are the no-piston answer to "hands-free". They are outside
this kind (not flush), and I'd note them on the page as the simpler option when hiding doesn't matter.

## Decision

Layout 1 is the bar, and most likely the pick.
- It is the door a player would derive themselves. It beats lock-v12 by 47 blocks, opens before you
  arrive where v12 makes you wait 13 gt, and passes every required walk.
- The search runs from it, on blocks only (open wait is already at its 0 gt ceiling):
  - plates standing on the network's own supports;
  - the network beside the walkway instead of under it;
  - one torch for both sides;
  - tripwire hook blocks as the pickup (layout 2), with its item caveat stated.
- lock-v12 stays on the site as the variant that keeps the door open for someone standing in the
  doorway.

## Must-haves

- The kind:
  - a 2x2 flush door (4 identical full blocks at rest);
  - opens from both sides;
  - people are the input (no lever).
- It never closes on anyone walking or sprinting through: alone, two in a row at any gap, or two
  crossing.
- It closes after the last walker (within 300 gt) and stays closed.
- Detectors sit where a player steps anyway, in the walkway next to the door.
- Stone plates rather than wooden ones, or say so: dropped items must not hold the door open.
- Survival materials, and every block supported (the judge checks nothing pops).

## Not goals

- Staying open for someone standing in the doorway: reported, not required.
- Counting people, i.e. one standing in the doorway while another walks through.
- Telling players from mobs: no vanilla detector can.
- Close delay: reported; anything within the judge's 300 gt passes.

## Judge changes (done on explore/clk-obs, with this doc)

- **`crate::autodoor`:** the two 400 gt stops in the doorway become **reported** walks, run last. If the
  door closes on the walker, the run goes on, and the door must still come to rest closed. The table
  says where the walker ends after 40 gt left alone. The new metric is `stop in doorway`, either
  `the door stays open` or `the door closes on them` (the worst build's).
- **Selftest `circuit:verify-autodoor`, all in the real game:**
  - lock-v8 now shows `stop in doorway the door stays open`;
  - the no-memory calibration passes, robust, with `the door closes on them` (before, it was refused at
    the stop);
  - the plate-order calibration's failing walk is renumbered 91 → 89.
- docs/circuits.md is updated to match.
- **Lateral sweep (08:48, coordinator):** 380 required walks across the doorway, described in the
  search section. The selftest's walk numbers before the sweep are unchanged.

## The search (G1 approved by the reviewer and the coordinator, 2026-10-22 08:35)

**The lateral sweep (coordinator 08:48), a new required walk family.**
- **The gap it closes.** A player a little off centre overlaps one column by less than the plate's
  1/16 margin. They press only the other column's plate, so a door that opens by halves keeps the
  first half shut in their path. The judge's three lanes never land in that window.
- **The walks.** One walker at every offset across the doorway in 1/32 steps, edge to edge:
  - with the stand's 0.5-wide box, 49 offsets;
  - with a player's 0.6-wide box, 46 offsets. The player-width walker is four stands 0.05 off its
    middle each way, whose boxes' union is a player's footprint; the bridge refuses mobs, so a
    villager is not an option.
  - Each offset is walked and sprinted, both ways: 380 walks.
- **Results.** The obvious door passes all 380 with no wait. auto-door-columns fails: "walk 168
  (one walking at lane 0.7812, stand-width): walker 1 waited 100 gt and the door did not open".
  That stand overlaps the second column by 1/32 and presses only the first column's plates.
- **What it means for the search.** Every half must open on all four plates. A sliver-overlapping
  walker presses only the other column's plates, from either side, so the column split is out.

**auto-door-raised-torch, 42 blocks: the candidate, archived**
(`designs/circuits/autodoor/archive/auto-door-raised-torch.txt`, track `tracks/clk-obs/raised-v1.txt`).
- **How it works.** The obvious door's one network for all four plates, with each side's network end
  climbing to y = −1 under the bottom piston and pointing into a torch base one block higher. The torch
  on it powers the bottom piston beside it and strongly powers the stone above, which powers the top
  piston: 3 blocks per side instead of 4.
- **Results.** `circuit verify` in the real game (4 positions × 4 rotations, every walk, the lateral
  sweep included):

```
CIRCUIT: auto-door-raised-torch auto-door door 2x2, open wait 0 gt, close delay 27 gt,
stop in doorway the door closes on them, 42 blocks, 8x5x3, robust
```

- **A judge bug on the way.** One build in 16 first failed: rotation 1, a player-width walker at lane
  0.30, exactly touching the doorway's edge. The judge's box test had an epsilon on its high side
  only, so rounding at that build position counted the touch as an overlap. Both sides have the
  epsilon now; the game's AABB test is strict.
- **The archive rejudge** under the fixed judge keeps lock-v8, v9, v10, v12 and the obvious door
  robust, and removes auto-door-columns.

**auto-door-columns, 38 blocks: rejected by the lateral sweep, removed from the archive.** Track:
`tracks/clk-obs/split-v1.txt`. It passed the judge before the sweep (4 positions × 4 rotations):

```
CIRCUIT: auto-door-columns auto-door door 2x2, open wait 0 gt, close delay 27 gt,
stop in doorway the door closes on them, 38 blocks, 8x4x3, robust
```

**What it changed from the obvious door, and why that fails**
- **Each side's pistons listened only to their own column's two plates.**
  - A walker centred in one column passes only that column's door cells.
  - A walker in the middle (box 3.75–4.25 across) touches both columns' plate zones
    ([1/16, 15/16] of each cell), so both halves open.
  - But a walker overlapping a column by less than 1/16 presses neither of that column's plates
    and walks into it (above).
- **Each collector was three dust at floor depth under the piston-head column**, beside the plate
  supports: (2,−1,−1), (2,−1,0), (2,−1,1).
- **The torch moved up one block**, the trick raised-v1 keeps.

**Below 42, tried on paper and rejected**
- **The count.** With every plate opening both halves, the network must touch all four plate supports
  (one dust beside or under each), join north to south under the opening (1–2 dust), and reach both
  torch bases (2 dust each, with the climb). Every layout I could derive needs at least 10 dust and
  their 10 supports. raised-torch is at that count, and its torch end already saves the one block per
  side that is there to save.
- **The column collectors joined under the opening.** The diagonal dust step down from (2,−1,0) to
  (3,−2,0) is cut by the floor block above (3,−2,0), unless that block is glass. Glass plus the link
  costs 6, which gives 44.
- **Tripwire, one string per side across both columns.**
  - A powered hook weakly powers every neighbour (`TripWireHookBlock.getSignal`), so hooks next to
    the piston columns would hold the pistons closed.
  - Widening the strings to 6 cells keeps the hooks clear, and the hook blocks then feed the torch
    base through two dust crosses: 42, with the dropped-item hold on top.
- **One torch beside the top piston** (it powers the top piston directly, and the bottom one by
  quasi-connectivity through the cell above it). Its base sits at floor height, so the network would
  have to reach it from the side at y = 0, where the piston head blocks the side column.
- **A torch hung on a plate support.** It can only invert one plate; ANDing two torches costs two
  more.

## The obvious design, measured (G2)

`designs/circuits/autodoor/tracks/clk-obs/obvious-v2.txt`, judged by `mctas circuit verify` in the real
game (4 positions × 4 rotations, all 150 required walks):

```
CIRCUIT: auto-door-obvious-v2 auto-door door 2x2, open wait 0 gt, close delay 27 gt,
stop in doorway the door closes on them, 44 blocks, 8x5x3, robust
```

- **How it works.**
  - classic6's four sticky pistons in the door plane.
  - On each side, a torch under the stone beside the bottom piston. The torch strongly powers that
    stone (the bottom piston's neighbour) and the dust cross on it, which points into the top piston.
    At rest: powered, closed.
  - Four stone plates, two in front of the door and two behind. Each strongly powers its support, and
    the dust under the supports is one network running straight into both torches' bases. Any plate
    turns both torches off, and the door opens 2 gt later.
- **Two-walker sweeps.** Walking: the door closes at most 27 gt after the last walker leaves the
  opening, 24 crossing. Sprinting: at most 26, 23 crossing.
- **Reported stop.** The armor stand ends inside the door's blocks at the door plane: squeezed.
- **obvious-v1** is the same door with the naive door's delay-4 repeater into each torch base. The
  walker then waited 6–8 gt at the door, so the repeaters only cost time.
- **Same judge, older doors.**

| design | blocks | open wait | close delay | stop in doorway |
|---|---|---|---|---|
| obvious-v2 (this) | 44 | 0 gt | 27 gt | closes on them |
| naive (selftest no-memory: two plate rows each side) | 60 | 4 gt | 40 gt | closes on them |
| lock-v12 (live pick) | 91 | 13 gt | 40 gt | stays open |
| lock-v8 | 99 | 6 gt | 40 gt | stays open |

## Ceiling

- **Open wait: 0 gt.** Layout 1 reaches it.
- **Blocks: a floor of about 30–36, counted by parts.**
  - The door's 4 sticky pistons and 4 blocks: 8.
  - Power for both piston pairs that a detector can cut: a torch and its base, and a block or dust
    into each pair. That is about 8 with one torch per side; a frame over the opening like classic6's
    costs more.
  - Detectors covering both columns on both sides: 4 plates and 4 supports, 8 (tripwire 12).
  - A link from both sides to the torch bases: at least 6–12 dust and supports.
- Layout 1 at 44 blocks is at 68–82% of that floor.
