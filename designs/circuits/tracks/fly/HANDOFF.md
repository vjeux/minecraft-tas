# fly HANDOFF (study 7, flying machines) — track fly, branch explore/fly

## State
* REAL USE (2026-10-21, coordinator + clk-misc's judge audit): every kind with a real switch at
  minimum volume, game-robust: flat fly-horizontal-start-7b-10gt (3x4x1), up fly-up-start-9b-9gt
  (4x3x1), down fly-down-start-9b-10gt (2x2x3), payload fly-payload-start-1-11b (3x2x3), docking
  fly-docking-15b-78gt (4x2x8). Judges: `flying_start` (one-way start lever/button, one run per
  START_FLIPS gap), `flying_switch` (docking: must shuttle until off), ground clearance
  (robust::fly_clearance). Details, bounds, what failed: designs/circuits/fly/WR.md "Real use";
  tools `circuit fly start-scan [--watcher]` + `circuit compact`; designs in tracks/fly/start.
* WORLD-RECORD CONSOLIDATION (2026-10-20, vjeux "quality over quantity" + the site critique):
  the archive keeps ONE machine per (class, speed, blocks, glue): 13 cards, each caption naming
  its glue (slime / honey / slime + honey), the payload ones what they carry, and how many other
  layouts were folded into designs/circuits/tracks/fly/layouts/ (73 files, `mctas circuit fly
  consolidate`); fly-d1 left as below bar (7 blocks at the 6-block speed). Record targets and
  bounds: designs/circuits/fly/WR.md. The list below is the history of what was found.
* Archive (`designs/circuits/fly/archive/`, `mctas circuit archive`, real game 4 positions x 4
  rotations). The Pareto front (blocks vs speed) per direction/class, no dominated variants
  (coordinator: "we don't need 20 variations of the same design"):
  * horizontal: 6 blocks 10 gt/block (47 structurally different types, all robust: two 3-block
    parts, observer + slime + piston each); 7 blocks **9 gt/block** robust
    (fly-horizontal-7b-9gt-e75c5f: a redstone block riding on the front part beside the cell the
    rear piston lands in, so the pulled piston fires as it lands, 1 gt instead of an observer's 2)
    and fly-horizontal-7b-9gt-aa517c (east-west only: a sticky piston lifts the quasi-connecting
    redstone block away and back); fly-d1 (7 blocks 10 gt/block, the first derivation, the
    calibration's origin; a variant).
  * up: 6 blocks **9 gt/block** robust (5 types: fly-up-6b-9gt-*, a redstone block in place of
    the rear observer fires the pusher as it lands). They took out the up 6-block 10
    gt/block types and fly-up-7b-9gt-ff426b (fly-d1 tilted up: the front observer quasi-connects
    the landing pusher through the slime two above it), now in tracks/fly/vertical and c6all.
  * down: 6 blocks 10 gt/block robust in every orientation (8 types with a redstone block in place
    of the rear observer); 17 observer-only 6-block types, east-west only (rotation-dependent).
  * two-way: fly-two-way-14b (the reversible 6-block engine 01ce95 between two 4-block return
    stations, round trip 98 gt over 6 positions, robust; calibration cal-two-way-machine) and
    fly-two-way-14b-73cdd4 (engine 73cdd4, stations where a station observer and the front
    observer watch each other at the stop; round trip 200 gt over 9 positions, robust).
  * payload: family fly-payload-e2cf52 (base fly-payload-n1): the 6-block engine carrying n =
    1..18 blocks, 6 + n blocks, 10 gt/block (9 per part: the 12-block push limit); members
    fly-payload-9-15b, fly-payload-18-24b. Family fly-train-e2cf52 (base fly-train-n1): the engine
    pulling n trailers (each a move of its own: no size limit), 5 + 3n blocks, 10 gt/block, judged
    up to n = 16 (53 blocks).
* Judge support written here (crates/circuit, reviewed by clockinfra): `port station[.<name>]`
  (return-station blocks: counted, left out of the tracked machine shape), the steady state
  preferring an in-place run that outlasts the moving runs (two-way), flying-machine family cards
  ("speed = 10 gt/block"), the cal-two-way-machine calibration in the selftest.
* Notes, mechanisms, numbers: designs/circuits/tracks/fly/NOTES.md. Track designs:
  tracks/fly/{c6,c6all,vertical,two-way,payload,train,fast,fast-up,rb6,speed,speed9}.

## Tools (crates/circuit-fly, `mctas circuit fly ...`; NOTES.md has the details)
`sim` / `verify` (redsim / real game tracker), `census` (exhaustive boxes, `--procs`), `types`,
`prep` (`--skip-archived`, `--one-per-caption`), `orient` (`--all`), `grow` (`--add 1|2`,
`--remove 1`, `--faster`), `gen payload|train`, `station` (return-station search, `--procs`),
`start-scan` (a real start lever/button: in the kick cell, or `--watcher` W + glue; judged on the
start judge),
`two-way` (reversible engines, station pairs composed; `--reversible-only`), `unarchive`.

## Dead ends (recorded, not archived)
* Slower machines (grow: 8 blocks 11/12/16/17 gt/block, 9 blocks 18/19/24/25): dominated by the
  6-block 10 gt/block ones (tracks/fly/speed, speed9).
* Vertical two-way: not built yet (see Next): no 6-block engine with two sticky pistons flies
  both ways vertically (every orientation and kick cell); 7-block ones do.
* Return stations with <= 3 blocks: none for any of the 7 reversible engines (k <= 2: a wall + 2
  blocks, within TWO cells of the stop, palette obsidian, stone, observers, repeaters, cross dust,
  redstone block, torches); 4 is the minimum (fly-two-way-14b).

## Next
* Faster than 9 gt/block: not found (census k = 6 with redstone blocks, grow/swap around every
  9 gt machine). Why 9: push 2 + observer 2 + reach-pull 2 + pull 2 + landing fire 1, and the
  landing-fired pusher only retracts after the front lands (its own push's neighbour updates come
  before its state turns extended), so the pull cannot start earlier. Ideas: a second piston that
  updates the pusher right after it extends (a sticky pusher retracting at once spits the front:
  1 gt instead of 2); a puller fired on landing with a pulse that ends by itself (an observer in
  the rear part watching the front leave).
* Two-way with other engines: 73cdd4 done (fly-two-way-14b-73cdd4); 91737e and 964ac6 have a
  station ahead but none behind (k = 3, radius 1, obsidian/observer/dust); 02a7fb, 67f769, 873917
  none ahead.
* Vertical two-way: 7-block engines that fly up AND down exist (tracks/fly/vertical-rev, robust
  both ways: grow --both-ways); the top station takes 2 blocks (an obsidian ceiling + an
  observer), the bottom none with <= 3 (NOTES: why): build a 5+ block bottom station (a dust line
  from an observer watching the front's final non-slime cell to the reversing piston or to the
  cell the rear observer watches).
* Payload: trailers that carry their own payload columns (9 per intermediate trailer); a train
  behind the 9 gt engine.
