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

(see the table below; filled in as runs finish)
