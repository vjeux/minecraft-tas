# Track shulker (study 8: shulker loaders and unloaders) — notebook

Agent clk-rt ("shulker"), branch `explore/shulker`. The judge, the reference and always-on
designs, the badges and redsim's shulker port are this track's; clk-item (explore/tff) owns the
unloader badges and the speed / smallest / mixed / combo designs (designs/circuits/shulker/tracks/
shulker-speed/). What builds want and how each variation is judged: ../../VARIATIONS.md. How the
judge works: docs/circuits.md (the shulker kinds). The archive and its catalog:
designs/circuits/shulker/archive/, CATALOG.md.

## Where things are

- Judge: crates/circuit/src/shulker.rs (scenarios, `run_in`, badges, the drop stress, metrics).
- Searches and tools: crates/circuit-shulker (`mctas circuit shulker shrink | trace | compare`).
- redsim: crates/redsim/src/redstone/shulker.rs (box placement by a dispenser, the piston-broken
  drop, `Chests`: the judge's chests, endless for the viewer or with the judge's supply for the
  replay), Block::ShulkerBox / Block::Chest, `Stack::payload` + `World::payloads` (box contents,
  custom names), crates/redsim/src/rng.rs (LegacyRandomSource, `Drop::seeded`).
- Bridge: bridge/src/dev/vjeux/mctas/bridge/Sandbox.java, the "shulker study" block: drops seeded
  per sandbox, entity ids base + n (docs/mechanics.md, "Drops and the bridge's seeds").
- Viewer: crates/mcview/src/session.rs (shulker designs run live with `Chests`), web check's
  `shulker_pair` in crates/web-cli.
- Traces: designs/circuits/shulker/traces (rounded) and traces/exact (`circuit shulker compare`).

## My designs (tracks/shulker/)

| file | what | blocks |
|---|---|---|
| ref-loader-{1,16,64}.txt | the classic loader: lever, dispenser over the box cell, comparator on the box (compare mode, a redstone block on its side: 15 only when full), sticky piston 2 cells away pushing a stone into the box cell | 29 |
| ref-unloader-{1,16,64}.txt | the classic unloader (hopper under the box, comparator says empty) | 47 |
| auto-loader(-1).txt | always-on loader (no lever): the dispenser re-fires on the observer of the piston | 31 |
| auto-unloader(-1).txt | always-on unloader (superseded by clk-item's lock-free small unloader, 41 b) | 49 |
| flush-loader(-1).txt | auto-loader + a flush: a falling edge of the feed hopper's comparator breaks a part-full box when the input stops | 37 |
| filter-loader.txt | a hopper sorter in front of the auto-loader: only the kind's item goes into boxes, the rest to the reject chest (the filter hopper holds 18 cobblestone + 4 renamed paper fillers) | 46 |

## What I learned (each cost a run)

- The dispenser must sit ABOVE the box cell: the drop spawns at the cell center ± 0.25 and rises
  0.2; any open side of the drop cell loses one now and then. The drop cell needs its side walls
  and the box cell its floor: a shrunk loader without them passed 600 plain runs and still lost a
  drop in 6 of 64 burst runs (two drops within the collection hopper's 8 gt cooldown: the second
  waits on the hopper's top and drifts off). Hence the judge's drop stress (64 more runs, every
  other one a burst).
- A powered dispenser next to a piston's quasi-connectivity cell makes a clock: put the sticky
  piston 2 cells away and push a stone.
- A 2 gt observer pulse makes a sticky piston spit its block: stretch it with a delay-2 repeater.
- A hopper's comparator never falls during a backup while the source supplies (pull = push): a
  jam detector needs an upstream hopper lock and an edge, not a level.
- The dispenser picks a random slot (the level random): a top-up box with a spare in the
  dispenser broke the top-up badge; spare = 0 there. redsim takes the first slot only when every
  filled slot holds the same stack.
- A locked hopper can still be pulled from by the hopper below.
- Vanilla drops are not reproducible even in a seeded sandbox (the entity's own random comes
  from nanoTime; entity ids from a JVM-wide counter, and an item at rest moves on
  `(tickCount + id) % 4`): the bridge now reseeds drops and numbers each sandbox's entities. Two
  recordings with the same seed are identical, and redsim mirrors the drop bit for bit.
- `BitRandomSource.nextDouble` is double precision (DOUBLE_MULTIPLIER is a double field);
  decompilers print its float literal. The item's horizontal drag is 0.98F widened, not 0.98.

## The judge's metrics, briefly

items/s: a loader's delivered box items over the run (source outflow overstates buffered
loaders, clk-item); an unloader's steady 10-90 % flow into its items chests. Robust = 16 builds
(4 positions x 4 rotations, seeds from the build's position) within 4 gt and 3 %, then the drop
stress. Badges each at 4 rotations.

## Open

- dropper75 (clk-item's 7.4 items/s loader): its dropper chooses among different stacks, the
  level random. clk-misc is mirroring the level random in redsim (chunk ticks: 3 nextInt(48) per
  forced chunk per tick, piston pitches, dispenser slots, popResource's 3 draws); then set its
  fields in `circuit shulker compare` from the trace header (seed, chunks = the box's chunks).
- shulker-speed-unloader-rail: a minecart pushing entities (clk-misc landed minecart-minecart
  pushing on explore/clk-misc 8d53db39e; re-run `circuit shulker compare` once both are in main).
- The filter loader has no trace (it needs cobblestone: 41k gt). A mixed-item loader (a jam
  detector) is clk-item's now.
- redsim does not judge shulker kinds (`circuit sim` refuses them): the judge stays in the game;
  redsim replays and the viewer runs live.
