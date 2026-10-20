# fly HANDOFF (study 7, flying machines) — track fly, branch explore/fly

## State
* Archive (`designs/circuits/fly/archive/`, `mctas circuit archive`, real game 4 positions x 4
  rotations). The Pareto front (blocks vs speed) per direction/class, no dominated variants
  (coordinator: "we don't need 20 variations of the same design"):
  * horizontal: 6 blocks 10 gt/block (47 structurally different types, all robust: two 3-block
    parts, observer + slime + piston each); 7 blocks **9 gt/block** robust
    (fly-horizontal-7b-9gt-e75c5f: a redstone block riding on the front part beside the cell the
    rear piston lands in, so the pulled piston fires as it lands, 1 gt instead of an observer's 2);
    fly-d1 (7 blocks 10 gt/block, the first derivation, the calibration's origin; a variant).
  * up: 6 blocks **9 gt/block** robust (5 types: fly-up-6b-9gt-*, a redstone block in place of
    the rear observer fires the pusher as it lands). They took out the up 6-block 10
    gt/block types and fly-up-7b-9gt-ff426b (fly-d1 tilted up: the front observer quasi-connects
    the landing pusher through the slime two above it), now in tracks/fly/vertical and c6all.
  * down: 6 blocks 10 gt/block robust in every orientation (8 types with a redstone block in place
    of the rear observer); 17 observer-only 6-block types, east-west only (rotation-dependent).
  * two-way: fly-two-way-14b (the reversible 6-block engine 01ce95 between two 4-block return
    stations, round trip 98 gt over 6 positions, robust; calibration cal-two-way-machine).
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
  tracks/fly/{c6,c6all,vertical,two-way,payload,fast,speed,speed9}.

## Tools (crates/circuit-fly, `mctas circuit fly ...`; NOTES.md has the details)
`sim` / `verify` (redsim / real game tracker), `census` (exhaustive boxes, `--procs`), `types`,
`prep` (`--skip-archived`, `--one-per-caption`), `orient` (`--all`), `grow` (`--add 1|2`,
`--faster`), `gen payload`, `station` (return-station search, `--procs`), `two-way` (reversible
engines, station pairs composed; `--reversible-only`).

## Dead ends (recorded, not archived)
* Slower machines (grow: 8 blocks 11/12/16/17 gt/block, 9 blocks 18/19/24/25): dominated by the
  6-block 10 gt/block ones (tracks/fly/speed, speed9).
* Vertical two-way: no 6-block engine with two sticky pistons flies both up and down in one
  orientation (the reversible horizontal engines tilted fly down only; the up-flyers with two
  sticky pistons break when kicked from the other end), every kick cell tried on redsim.
* Return stations with <= 3 blocks for 01ce95: none within one cell of the stop (k <= 2, the full
  palette); 4 is the minimum there.

## Next
* Faster than 9 gt/block: both pistons firing on landing (the puller needs a pulse that ends by
  itself: an observer in the rear part watching the front part leave, its output reaching the
  puller's landed position), or a sticky pusher retracting early (the pushed part snaps into place:
  1 gt instead of 2); the k = 6 census with redstone blocks (results in NOTES).
* Two-way with fewer blocks: stations for the other reversible engines (02a7fb, 67f769, 73cdd4,
  873917, 91737e, 964ac6), k <= 3.
* Payload: trailers that carry their own payload columns (9 per intermediate trailer); a train
  behind the 9 gt engine.
