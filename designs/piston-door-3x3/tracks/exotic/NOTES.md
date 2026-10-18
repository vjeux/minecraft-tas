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
| glass / tinted glass / waxed copper grate as DOOR blocks | non-conducting full blocks; main's judge accepts all three as the 9 door blocks (category solid): fc2 and b38 work unchanged with glass, fc2 with tinted glass and with grates | - | `glass`, `tinted`, `grate` |
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
A note block in the trapdoor's cell works too (verified at 4 positions): the saving is the
"watchable relay" pattern (sf-holdA's note-block trick) applied where nobody had tried it; the
trapdoor's non-conduction is not needed there.
