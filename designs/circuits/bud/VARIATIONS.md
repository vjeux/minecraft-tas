# Study 10: BUD switches (block-update detectors): variations

A BUD switch turns a BLOCK UPDATE at some cell (its target) into an output signal, without that
cell powering it. This page lists what can vary between designs, read in the 26.3 source
(`mctas src show <Class>`; no outside knowledge), and how the judge kind `bud` measures each
variation. Designs come after the judge (smallest and fastest per variation, archived with
`mctas circuit archive`).

## 1. The two update channels (Level.setBlock, 26.3)

`Level.setBlock(pos, state, flags)` changes a block and then, only if the state really changed:

* flag 1 (`UPDATE_NEIGHBORS`): `updateNeighborsAt(pos)`: a BLOCK UPDATE (`neighborChanged`) to the
  6 neighbors, in `NeighborUpdater.UPDATE_ORDER` (west, east, down, up, north, south), plus
  comparator updates (`updateNeighbourForOutputSignal`) when the block has an analog output;
* flag 16 (`UPDATE_KNOWN_SHAPE`) NOT set: a SHAPE UPDATE (`updateShape`) to the 6 neighbors
  (`UPDATE_SHAPE_ORDER`: west, east, north, south, down, up) and the indirect ones (redstone wire
  diagonals). Shape updates go out even without flag 1.

So a change can send both (a player placing or breaking a block: flags 3; most redstone and
interaction changes: flags 3), shape updates only (flags 2: `CropBlock`/`CocoaBlock` growth, a
dispenser's `TRIGGERED`, an observer's `POWERED`, ...), or neither (flags with 16 and without 1).
A pure block update without any change also exists: `updateNeighborsAt(pos)` called by a block
whose own state did not change: an observer's back (`ObserverBlock.updateNeighborsInFront`
updates the block behind it and that block's neighbors), redstone torches and dust when they
change (`RedstoneTorchBlock`, `RedstoneWireBlock.checkCornerChangeAt`: the neighbors of each of
their neighbors, 2 cells away). Those reach sensors 2 cells away from the change.

## 2. What each sensor reacts to

| sensor | reacts to | reads | source |
|---|---|---|---|
| observer | a SHAPE update from the cell it faces (any real state change there: placed, removed, property changed, with or without flag 1) | nothing else: no block update, no power | `ObserverBlock.updateShape`: `FACING == direction && !POWERED` -> tick in 2 gt (one pending at a time) |
| piston, sticky piston | a BLOCK update at its own cell (`neighborChanged`), its placement | its power: any side but its front, and QUASI-CONNECTIVITY: any neighbor of the block above it (`getNeighborSignal`) | `PistonBaseBlock.checkIfExtend`; a queued move is cancelled if the power is gone when its block event runs (`triggerEvent`) |
| dispenser, dropper | a block update at its own cell | power at its cell or at the block above (QC) | `DispenserBlock.neighborChanged`: `hasNeighborSignal(pos) || hasNeighborSignal(pos.above())`, fires 4 gt later |
| crafter | a block update | power at its cell only (no QC) | `CrafterBlock.neighborChanged` |
| lamp, note block, door, trapdoor, copper bulb, rails, TNT | a block update | power at their own cell only | (power followers, not BUDs) |

The classic BUD is a QC sensor: a piston (or dispenser / dropper) whose only power comes through
quasi-connectivity is NOT told when that power arrives (nothing updates its own cell), so it sits
armed until any block update reaches its cell; then it re-reads its power and fires. An observer
is the other kind: it watches ONE cell's state, whatever updates that change sends.

## 3. Update classes (what the judge does at a target cell T)

| class | the judge's action at T | sends | observer facing T | QC sensor next to T |
|---|---|---|---|---|
| `place` | air -> a block (stone, or the design's declared state), flags 3 + `onPlace` | block + shape updates | yes | yes |
| `remove` | that block -> air, flags 3 | block + shape updates | yes | yes |
| `state` | the block at T to its other declared state (e.g. a note block's `note`), flags 3 | block + shape updates | yes | yes |
| `shape` | the same change with flags 2 (no block update): how crops grow, how a dispenser marks `TRIGGERED` | shape updates only | yes | no |
| `update` | `updateNeighborsAt(T)`, nothing changes | block updates only | no | yes (if it is one of T's 6 neighbors) |

A design declares its target cells (`port target: x y z [state-a [state-b]]`) and may claim the
classes it detects (`bud: place remove state update`). The judge runs every class at every target
and reports what is detected; a claimed class must be detected, an unclaimed one is reported, not
failed.

## 4. Variations and how the judge measures them

* **Detected classes**: per class, every event (several per class, at irregular times) gives
  exactly one output pulse, in all 16 builds (4 positions x 4 rotations).
* **Latency**: frames from the event (between ticks) to the output's first on frame, per class
  (an observer facing T: 2 gt; a QC piston: its block event runs in the tick after the update,
  then whatever turns its move into the output).
* **Output pulse length**: gt on per detection, per class (fixed lengths only: the same in every
  event and build).
* **Re-arm time**: the shortest gap between two events of one class that are both detected: the
  judge repeats pairs at gaps 2, 4, 6, 8, 10, 15, 20, 30, 40 gt (an observer misses a change
  while its tick is pending or while it is on: re-arm 4 gt).
* **Self-reset vs latched**: self-reset = the output turns off by itself and the next event is
  detected; latched = it stays on: then the design needs a `port reset:` input, which the judge
  drives after each detection (measured: reset-to-armed time); a latched design without a reset
  input fails ("latches").
* **False triggers**: idle periods (no events) must give no output; and the output must be off at
  the end of every gap.
* **Sensitivity (footprint)**: the judge also runs `place` / `remove` / `update` at every cell
  next to the design (not part of it) and lists the cells that trigger it: a 1-cell BUD (only T)
  vs a wide one. Reported, not failed.
* **Observer-free**: no observer in the design (the classic QC BUDs; also dropper / dispenser
  BUDs); observer-based otherwise. From the block list.
* **Silent**: no block that plays a sound when the BUD fires: pistons (extend / retract sounds),
  dispensers and droppers (dispense or fail click), note blocks, bells. From the block list.
* **Size, box, flat** (height of the box).
* **Tileable** (a `tile: dx dy dz` claim): 3 copies side by side along the step, every class at
  every copy's target; each copy's output answers its own target only (`tileable every N`: targets
  every N blocks). Two things in the source decide how dense a BUD can tile:
  * a lamp strongly powered by an observer or a repeater (`getDirectSignal`) makes its neighbors
    see 15 through it (`Level.getSignal` of a conductor is the strong power into it), so two
    judge lamps side by side cross-talk; a lamp lit through a solid block is not strongly powered
    and does not: an observer that must tile every block powers a stone, the lamp beyond it;
  * a piston's own move sets its base block with `UPDATE_NEIGHBORS`, so an armed (quasi-powered)
    piston next to another copy's piston fires when that one moves: QC piston BUDs cannot tile
    every block.

## 5. Variation families to build

1. Observer facing T: 1 block, detects place / remove / state / shape, latency 2, pulse 2,
   re-arm 4, self-reset, silent.
2. QC piston BUD: a piston powered only through the block above it, T one of its neighbors:
   detects place / remove / state / update (not shape). Its output: the piston's own move (an
   observer watching it, or a pushed redstone block). Self-reset needs the piston to lose its QC
   power after firing (moving its own power source away) and get it back without being updated.
3. Dropper / dispenser QC BUD: `TRIGGERED` on the first block update, fires 4 gt later; output
   through an observer watching it (the `TRIGGERED` change is a flags-2 shape update) or a
   comparator; re-arms only when unpowered and updated again.
4. Hybrids: an observer watching a QC sensor (the QC sensor's classes with an observer's clean
   2 gt pulse), or a QC sensor fed by an observer.
5. Relay footprints: blocks whose shape follows their neighbors (redstone wire, fences, walls,
   panes, stairs) let an observer see changes at cells it does not face (a wire's connection
   changes when T changes: an observer facing the wire detects T).

## 6. Which class sets can exist (from the channels above)

* `shape` only, `state` only, `update` only: impossible. A placement, a removal or a flags-3 change
  sends both channels to the same 6 neighbors; a sensor that sees a shape-only change (an
  observer: a real state change of the cell it faces) also sees every placement, removal and
  flags-3 change of that cell, and a sensor that sees a pure update (a QC sensor: any block update
  at its own cell) also sees every placement, removal and flags-3 change beside it. A shape change
  never becomes a block update by itself (`Level.setBlock` passes shape updates on without
  `UPDATE_NEIGHBORS`), except by breaking a block that needs support: one use only.
* `place remove state shape`: an observer (1 block, the smallest possible BUD).
* `place remove state update`: a QC sensor (the census finds no observer-free one under 3 blocks).
* `place remove`: an observer behind a relay whose shape follows the PRESENCE of a solid block, not
  its state (a fence: 2 blocks).
* all five: two sensors (an observer for shape-only changes, a QC sensor for pure updates) with
  their outputs merged. An observer's back is 2 cells from the cell it watches, never beside a
  piston that is beside that cell, so it cannot trigger that piston directly: the merge takes a
  dust run (the union of two archived designs, not built).
* `place` only or `remove` only: a presence LEVEL (a solid block at T conducting power) and an
  edge detector (study 1) behind it (a composition, not built).
