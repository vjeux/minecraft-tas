# Track exotic: unusual redstone components vs. the smallest 3x3 door

vjeux's question: which redstone components nobody has tried yet could make doors smaller?
Every component below was read in the decompiled 26.3 source, proven with a gadget
(`designs/gadgets/*.drive`, a regression test), added to the search palettes (synth model,
`resynth`, `grow`/`regrow`), then swept over the record holders.

## Components (source + gadget)

| component | source fact | gadget | palette |
|---|---|---|---|
| iron / waxed copper trapdoor | `TrapDoorBlock.neighborChanged`: `hasNeighborSignal` -> OPEN+POWERED in the same tick, flag 2 (no neighbor updates; the shape update still reaches observers); no `canSurvive` (floats); non-conductor; pushable, slime drags it | `trapdoor-watch` (observer pulses 2 gt after each edge; the piston right above the powered trapdoor never moves) | `trapdoor`, `ctrap` |
| fence gate | `FenceGateBlock.neighborChanged`: POWERED+OPEN, flag 2; floats; non-conductor; open = no collision | `indicators` | `gate` |
| skulls / heads | `AbstractSkullBlock.neighborChanged`: POWERED, flag 2; floats; pistons pop them (POPPED) | `indicators` (observer watching a skull) | `skull` |
| iron / copper doors | `DoorBlock.neighborChanged`: either half's signal opens both halves (flag 2, the other half follows by `updateShape`); needs a sturdy floor; pistons pop doors | `indicators` | (not in palettes: 2 cells) |
| shelves | `ShelfBlock.neighborChanged`: POWERED with `setBlockAndUpdate` (neighbor updates, like a note block) but non-conducting; block entity (immovable) | - | `shelf` |
| powered / activator rail | `PoweredRailBlock.updateState`: powered by any neighbor signal or by a fed rail of its line up to 8 rails away, same tick (setBlockAndUpdate chain); powers NOTHING; needs a rigid floor (a slope also its high side) | `rail-line` (9 rails light in the click's tick, up a slope; the 10th stays dark; the piston under a powered rail never moves) | `rail`, `arail` |
| dispenser / dropper | `DispenserBlock.neighborChanged`: `hasNeighborSignal(pos) \|\| hasNeighborSignal(pos.above())` (quasi-connected like a piston) -> TRIGGERED at once on both edges, flag 2; the dispense 4 gt later finds nothing; conductor; block entity = immovable | `dispenser-qc` (triggers through its quasi cell, observer pulses; a piston cannot push it) | `disp`, `dropper` |
| crafter | `CrafterBlock.neighborChanged`: `hasNeighborSignal(pos)` only (no quasi-connectivity), TRIGGERED on both edges; immovable | `dispenser-qc` (rows 2 and 4) | `crafter` |
| scaffolding | any shape update schedules a tick 1 gt later; the tick recomputes DISTANCE (0 on a sturdy floor) and `setBlockAndUpdate`s; distance 7 breaks it (drops) or makes it fall (entity); pistons pop it | `scaffold-delay` (1 gt per scaffold) | `scaffold` |
| glass / tinted glass / waxed copper grate as DOOR blocks | non-conducting full blocks; main's judge accepts all three as the 9 door blocks (category solid): fc2 and b38 work unchanged with glass, fc2 with tinted glass and with grates (`door patch <design> tracks/exotic/door-glass.patch --out ...`; also door-tinted, door-grate, door-snow) | - | `glass`, `tinted`, `grate` |
| podzol (with snow-block doors) | `SnowyBlock.updateShape`: SNOWY follows "a SNOW-tag block above" at once; podzol has no random ticks; snow blocks are valid door blocks (fc2 with snow doors: WORKS, solid) | `podzol-snow` (arrival read at landing, departure at the move's first tick: an asymmetric shape relay) | full states |
| top slabs, upside-down stairs | non-conductors with a sturdy top: dust stands on them, steps up/down past them, nothing powers through them: exactly glass, already in every palette | - | `slab`, `stairs` |

Synth model (synth.rs): new kinds `Indicator` (reads any neighbor signal, plus the cell above
for dispensers/droppers; observable; powers nothing) and `Rail` (an indicator that also feeds the
rails of its line). Sanity check: `door synth` on the trapdoor harness with the trapdoor cleared
(`designs/gadgets/synth/indicator.spec`) finds exactly the trapdoors, fence gates and the skull,
and rejects note block / lamp / stone / dispenser (they conduct into the piston above).

## Searches

Palettes: `exotic` = iron trapdoor, oak fence gate, flat powered rails (2), dispenser, crafter,
skeleton skull; size-2 regrows use every exotic state (trapdoor, gate, skull, shelf, rail incl.
slopes, disp, crafter, scaffold, slab, stairs). `--require-any` keeps only candidates with at least
one exotic block (the old palettes were already exhausted on these designs).

| design | search | candidates | result |
|---|---|---|---|
| sf-holdA-b38 (38) | regrow --size 2, all exotic states | 5.3k | nothing |
| sf-holdA-b38 | regrow --size 2 --any --margin 2, all exotic states | 288k | nothing |
| sf-holdA-b38 | resynth --max-size 4, block,dust,dot,rep,obs,torch,note,exotic, --require-any | ~60 groups | nothing |
| fc2 (40) | regrow --size 2, all exotic states | 6.6k | nothing |
| fc2 | regrow --size 2 --any --margin 2, all exotic states | 523k | nothing |
| fc2 | resynth --max-size 4 (as above) | ~80 groups | nothing |
| mp8 (43) | resynth --max-size 4 (as above) | | nothing |
| lamp-sweep-flat43 (43, flat) | resynth --max-size 4 (as above) | | nothing |
| gravity2-g33 (33) | resynth --max-size 4 (as above) | 5 removable blocks | nothing |
| pareto-mid BEST (mp8-leafU 43, 15/17) | resynth --max-size 4 (as above) | | nothing |
| **pareto-mid 46 (14/14)** | resynth --max-size 4 (as above) | | **45** (pm45-trap, below) |
| flat-b-v7 (44, smallest box) | resynth --max-size 4 (as above) | | nothing |
| pm45-trap (45) | resynth --max-size 4, full palette + exotic, no filter; minimize --moves all --plateau (36k neutral edits) | | nothing |
| sf-holdA-b38 | regrow --size 3, obs,block,dust,rep,torch,glass,note + trapdoor,gate,disp,crafter,skull,railf, --require-any | 2.61M | nothing passes the screen |
| fc2 | regrow --size 3 (as above) | 2.76M | nothing passes the screen |
| glass-door b38, fc2, mp8, flat43, pm43, flat-b-v7, pm45 | resynth --max-size 4 --only around the doorway, block,dust,dot,rep,obs,torch,note,lamp,target,exotic (no filter) | | nothing |
| 39 more archived designs (solid, sculk, gravity, 33..46 blocks) | resynth sweep --max-size 4, exotic + shelf, --require-any | | fc2f 41 -> 40 (= fc2's count and timing), wire19sf-w45 45 -> 44 (open 26 / close 24), gravity-g10 45 -> 44 (NOT rotation-safe, open 24: useless); the other 36 nothing |
| mp8, flat43, g33, pm43, pm45, flat-b-v7 | regrow --size 2 --any --margin 2, all exotic states | 747k, 534k, 282k, 906k, 841k, 607k | nothing |
| fc2f (41, 22 / 9) | resynth --max-size 4, full palette + exotic + railf, --max-open 22 --max-close 9 | | nothing (no 40 at 22 / 9: the fast B-window feed needs its 3 blocks; no trapdoor next to the lever can reach R_B's input in 2) |
| pm45-trap | minimize --moves all --objective total --plateau (new relay substitutions) | 379k | nothing faster at 45 |
| flat43, flat-b-v7, pm45 | regrow --size 3, obs,block,dust,rep,torch,glass,note + trapdoor,gate,disp,crafter,skull,railf, --require-any | 4.18M, 3.51M, 3.43M | nothing |
| fast-clean-a1c (107, 10 / 7) | resynth --max-size 4, exotic + shelf, --require-any, --max-open 10 --max-close 7 | | nothing in 100 min (timed out) |
| snow-door b38 | regrow --size 3, obs,block,dust,rep,torch,note + podzol, --require-any podzol | 531k | nothing |
| **pareto-mid mp7-repU-pgF (50, 14 / 12)**, mp7-leafC-pgF (50, 15 / 11), mp7-leafC (46), mp8-lampU (44) (new on main) | resynth sweep, exotic + shelf, --require-any ...,note | | **mp7-repU-pgF -> 49** (pm49-trap, same pattern as pm45); the others nothing |
| **pareto-mid mp7-dustU (47, 14 / 13), mp7-dustU-pgF (51, 14 / 11)** (its branch, newest) | resynth sweep (as above) | | **-> 46 at 14 / 13 (pm46-trap) and 50 at 14 / 11 (pm50-trap)**: trapdoor beside the U dust's floor + observer instead of dust + stone + observer |
| fast-clean s4c-91 (12 / 7), c1r-107 (9 / 7) (new on main) | resynth --max-size 3, exotic + shelf + note, --require-any, their speed bounds | | nothing |

## Report (exotic track)

**Best:** `pm45-trap.txt` (= BEST.txt): **45 blocks, solid, open 14 / close 14, rotation-safe**, a
new point on the Pareto frontier (pareto-mid's 46 at the same 14 / 14), and three more frontier
doors, each one block under pareto-mid's door at the same speed: `pm46-trap.txt` **46 at 14 / 13**
(its mp7-dustU 47), `pm49-trap.txt` **49 at 14 / 12** (mp7-repU-pgF 50), `pm50-trap.txt` **50 at
14 / 11** (mp7-dustU-pgF 51); all rotation-safe. Variants:
`fc2f-40-trap.txt` (40, = fc2's count and timing), `w45-44-trap.txt` (44, slime-free, 26 / 24).
No new record on the main boards: b38 (38), fc2 (40), flat43 (43, flat/wall), flat-b-v7 (44,
smallest box), g33 (33, gravity) resist every exotic sweep (table above).

**Which components helped, and why**
* **Iron trapdoor (also fence gate, skull): yes, as a zero-delay, non-conducting watchable
  relay.** It turns a power edge (the lever, an observer's output, a powered block) into a state
  change in its own cell, so an observer can read it from a cell whose output lands straight on
  the target. All six finds are this pattern: pm46 -> 45 and mp7-repU-pgF 50 -> 49 (stone +
  repeater + floor -> trapdoor + observer), mp7-dustU 47 -> 46 and mp7-dustU-pgF 51 -> 50 (dust +
  stone + observer -> a trapdoor beside the powered dust floor + observer), fc2f -> 40 (the
  observer watching the lever for P_L -> a trapdoor beside the lever: P_L fires at t2 instead of
  t4), w45 -> 44 (three observers of a lever chain -> trapdoor + observer). sf-holdA had found the
  same relay with a note block (b38); a note block also works in pm45 and pm49. The trapdoor adds
  non-conduction (a note block next to a strongly powered source powers the pistons beside it:
  `designs/gadgets/synth/indicator.spec` is exactly that case) and no neighbor updates (flag 2):
  it never BUDs a quasi-armed piston.
* **Powered / activator rails: no.** A rail line costs a floor per cell like dust, powers nothing,
  and only an observer reads it (2 gt). The records have no long transport, and no spot where
  "passes next to a piston without powering it" saves a block; for one-cell hops a trapdoor or a
  note block is cheaper.
* **Dispensers / droppers / crafters: no.** A dispenser is a note block with quasi-connectivity
  and immovability. No record needs to read power into a cell above, or an immovable relay next to
  slime at a spot that also needs a watcher.
* **Scaffolding: no (for blocks).** A 1 gt-per-block distance line triggered by its floor moving:
  the leaves trick (pareto-mid) with any sturdy moving block instead of a log, but each sensor
  needs a second scaffold on a fixed floor (distance 7 breaks it into drops) and pistons pop it.
  Only a timing tool (odd delays) for the speed boards.
* **Glass / tinted glass / waxed copper grate doors: accepted, no gain.** main's judge takes all
  three as the 9 door blocks (category solid); fc2 and b38 run unchanged with them (no record
  relies on its door conducting); the door-adjacent resynth on 7 glass-door records found nothing.
* **Slabs / stairs: nothing new.** A top slab or upside-down stairs is redstone-wise exactly glass
  (a non-conducting floor with a sturdy top), already in every palette.
* **Others read in the source:** iron/copper doors (a 2-cell trapdoor that needs a floor and that
  pistons pop: costlier), shelves (non-conducting indicator that DOES update neighbors, immovable;
  in the sweeps, nothing), podzol under snow blocks (gadget `podzol-snow`: a zero-delay shape relay
  with an asymmetric delay, snow blocks being valid door blocks), walls/fences/panes (connections
  follow neighbors), copper golem statues (pose changes only by hand; a comparator reads a constant
  1..4). Shape relays cost as much as the observer or flag they would replace (relay +
  observer = 2, like observer + observer or flag + observer), and slime drags them.

**Why the main records resist:** their control is pinned by geometry and timing (sf-holdA's
analysis of b38's x = 1 cube, side followers and M trigger; fresh-c's of fc2's lever cluster).
There the exotic relays are alternatives at the same count: b38 already relays its lever with a
note block; fc2's P_L chain becomes a trapdoor + observer (fc2-trapL), still 2 blocks.

**Tools (separate commits):** synth model kinds `Indicator` and `Rail`; palette / grow shorthands
`trapdoor, ctrap, gate, skull, shelf, rail, arail, railf, disp, dropper, crafter, scaffold, grate,
tinted, slab, stairs, exotic`; `regrow --require-any`; `resynth A B C ... --out DIR` (sweep);
minimize substitutions observer <-> trapdoor / note block, stone -> note block; drive probes for
gates, skulls, shelves, dispensers, scaffolding; gadgets `trapdoor-watch`, `indicators`,
`rail-line`, `dispenser-qc`, `scaffold-delay`, `podzol-snow` and the synth spec `indicator.spec`.

**Next ideas**
* Speed: an observer whose only job is to relay a lever/observer edge to the next observer can be
  a trapdoor (0 gt instead of 2): same blocks, 2 gt earlier (fc2-trapL's P_L). Worth a
  `minimize --objective open|close|total` pass over the Pareto doors with the new substitutions.
* Trapdoor relays in the big speed doors (loose wiring, many lever watchers).
* Asymmetric relays (podzol: arrival read 2 gt after a departure; scaffolding / leaves: 1 gt) for
  the open/close balance of the Pareto doors.

## The variants

* `fc2f-40-trap.txt` (exotic-fc2-trapL, 40, open 24 / close 9, rotation-safe): fc2 with P_L fed by
  an iron trapdoor beside the lever and the observer under it (P_L fires at t2 instead of t4).
  Same count and timing as fc2: a tie.
* `w45-44-trap.txt` (exotic-w45-44-trap, 44, slime-free, open 26 / close 24, rotation-safe):
  wire19sf's w45 with three observers of its lever chain replaced by a trapdoor + one observer;
  2 ticks faster than wire19sf's own 44 (28 / 26). Not a board best (slime-free leader: b38, 38).

## pm45-trap: 45 blocks, open 14 / close 14 (Pareto frontier), rotation-safe

`pm45-trap.txt` (= BEST.txt, archived). pareto-mid's 46 fired its bottom puller (1,-3,0) from the
flag observer o (3,-2,0) (watching a stone glued under the right structure) through stone
(3,-3,0) (o's output block) and a delay-1 repeater (2,-3,0) on a floor (2,-4,0). resynth replaced
those 3 blocks by 2: an iron trapdoor at (3,-3,0) (o's output opens it in the same tick) and an
observer (2,-3,0) watching it, firing into the puller. Same timing (the repeater turned o's 2 gt
pulse into a 2 gt pulse 2 gt later; the observer does the same off the trapdoor's flip).
A note block in the trapdoor's cell works too (verified at 32 positions): the saving is the
"watchable relay" pattern (sf-holdA's note-block trick) applied where nobody had tried it; the
trapdoor's non-conduction is not needed there.
