# Track cosearch: a pure-Rust door judge (redsim) + a joint mechanism + wiring search

Brief (coordinator for vjeux, 2026-10-18 23:16Z): SAT (`door satwire`) only places static wiring
around a fixed mechanism with a fixed schedule; it cannot express moving power sources (redstone
blocks or observers carried by pistons), pistons used as relays, or co-designing mechanism +
wiring. Build the complement: a FAST joint search that evaluates candidates in redsim (no JVM in
the loop) and confirms finds with the real judge. Goal: a flat, slime-free, FULL seamless +
hallway-compatible door below 72b (record e89b, 8x9x1). New first target (vjeux 00:02Z): 7 wide x
8 tall = 56b (a community door has it).

Code: `crates/piston-door/src/cosearch.rs` (the evaluator, `agree`, `trace`) and
`crates/piston-door/src/cosearch_search.rs` (the search). Branch `explore/cosearch` (main +
redmodel + pistmodel + satwire + families).

## 1. The evaluator: `mctas door cosearch eval <designs...>`
The judge (`crate::verify_in_spec`) re-done step by step on `redsim::World` (agents redmodel +
pistmodel's exact model): placement like `video.rs::place` (raw writes, a shape update of every
wire, a `self` update of every block), the bridge's `step` semantics (always one tick, then stop
when idle; game time reset per position), settle (400), `canSurvive` of every block, random
ticks, closed at rest (9 identical full non-mechanism blocks, walkway without collision), the
empty-walkway rule over the watch box, an unpowered lever, then 3 lever cycles at the 32 judge
positions (`verify` offsets + `extra_offsets(28)`) with the open (no collision / no moving block
in the opening + walkway), restored door, whole watch box back to rest and no-entity (drops)
checks. Then, at the design's own position:
* FULL seamless = `properties::seamless` reaching FULL: one recorded cycle with the moving blocks,
  the 9 door blocks tracked through every move exactly like `properties::track` (moves, spits,
  re-matching), circuitry = everything but door blocks / the lever / quartz-like blocks resting in
  the ring, QUART both sides + open state clean + the hole rule;
* the hallway check = `properties_dyn::hallway_env` (smooth stone in every ring cell over the depth
  extent + 3 that the build never holds, swept over 2 flips) + `env_run` (placement with the
  hallway: design wires' shapes, note blocks next to the hallway re-shaped, self updates; one cycle
  with the judge's checks);
* the volume (`properties::cheap`'s circuitry bounding box over every frame).
Blocks redsim does not list but that are quartz-like (quartz, concrete...) run as stone.

**Agreement with the real judge** (`door cosearch agree designs/piston-door-3x3/archive/*.txt
--flat-only --mutants 40`): 2049 / 2049 identical (100%): the 50 archived flat doors + 1999 random
near-miss mutants (1-2 edits: remove, replace with a flat-door component, re-face / re-delay,
move, add next to a block), compared on the judge's verdict class (failing offset, cycle and
failure kind; for a pass the open / close ticks of all 3 cycles at the first position), FULL,
hallway and volume; 678 of them work in both. [agree-report.txt](agree-report.txt). The only
disagreements ever seen (14 flat43-family doors: a repeater standing on a down-facing extended
piston popped in redsim) disappeared with pistmodel's flag fix merged on main (0a255b6).

**Speed**: one full evaluation of a working door (32 positions x 3 cycles + FULL + hallway) takes
~31 ms on one thread (e89b: WORKS open 34 close 34, FULL, hallway ok, 72b 8x9x1, 49 blocks, as
archived); failing candidates are faster. `agree` judged 1044 cases/s on 16 threads; the JVM did
1.3 / s on 8 workers. The search's screen (1 position, 2 cycles, partial credit) runs 60-90k
candidates / s on 40 threads.

## 2. The search: `mctas door cosearch search <seeds...> --box=x0,y0:x1,y1`
A candidate is the door plane (z = door.z) inside the target box; the door cells and the lever
port are fixed (the lever may hang OUTSIDE the box: the community volume excludes the lever).
* Moves: set a cell to a palette block or air (stone, smooth stone, glass, dust, redstone block,
  torch, wall torch E/W, repeater E/W x 1-4, comparator E/W x compare/subtract, observer U/D/E/W,
  sticky / normal piston U/D/E/W, lamp, note block, target, waxed copper bulb), re-face, re-delay,
  comparator mode, move one cell, swap with a neighbour, a redstone block or an observer put in
  front of a piston (a power source it carries), a piston toggled retracted / extended (with its
  head). 1-3 moves per step, the next one next to the previous half of the time.
  `--lock-pistons` keeps the seed's pistons (new ones may still be added); `--no-mech` forbids any
  piston change; `--focus` restricts the moves to a sub-box.
* Screen (one position, `--cycles 2`): points per door cell closed at rest, walkable after each
  opening, back after each closing; the 12 ring cells clean in the open state (FULL); nothing of
  the build outside the box; the whole box back to rest after each close; penalties for blocks
  that cannot stand or pop. Max = 11 + 34 per cycle (79). Shaping (optional): every piston
  extension / retraction of a reference run, same cell within `--ref-slack` ticks, earns
  `--ref-weight` points: `--reference <door>` (a working door's own schedule) or
  `--reference-ideal <plan.ideal>` (a mechanism driven by a `door ideal` plan, run in redsim).
* Search: `--enum-k K` first screens EVERY candidate with up to K changed focus cells (k = 2: ~1M
  candidates in 10-20 s), then simulated annealing (`--threads` independent chains, restarts, best
  pool) from the best of them. A perfect screen goes to the redsim judge (32 positions, FULL,
  hallway); a pass is written to `--out-dir` and judged by the JVM (`--confirm`: `verify
  --extra-offsets 28` + `door properties`).

## 3. Runs (no door yet)
* **8 x 8 from e89b** (row -4 dropped; the lever stays below the box, hung under its observer
  (0,-3)): the seed screens 37/79 (center and BR stay). Exhaustive k = 2 over rows -3..-1: 75/79 in
  7 s; annealing: 77/79 in 30 s: BR opens through a new bottom-right trigger, every door block but
  the center leaves. With the mechanism free the annealer drops the lower elevator piston (a trap);
  locked (`--no-mech` / `--lock-pistons`, e89b's schedule as shaping) it plateaus at 77/79; k = 3
  exhaustive over the 9 bottom-right cells: 49/79 (4.0 M candidates); k = 3 exhaustive from the
  77/79 partial over rows -3..-2, x -1..3: 77/79 (9.4 M); lever moves: 77/79.
  Why (e89b's trace): the center needs the elevator's second step, F(1,-3) held 8 gt [t12, t20]
  (lifting F(1,-2) for a2 at t15, lowering it at t20, a3 at t23 through the watcher (2,-2)), which
  e89b gets from the delay-4 repeater (2,-3) standing on row -4. In 8 rows F(1,-3) sits in the
  bottom row: its in-plane feeds are (0,-3) and (2,-3) (no floor: no repeater / comparator), or
  quasi-connectivity through (1,-2), whose every feed also powers F(1,-2) first; an observer gives
  2 gt, a lamp or a bulb does not power a piston, a torch needs a window upstream. So the 8 x 8
  cd19 needs a different elevator or a window source the bottom row can hold.
* **7 x 8** (vjeux's target): seed = wire56's t1 (7x8-028 + real K1 / K0 / K3 wiring) with a lever
  outside the box at (5,4) and wire56's mandatory K2 observers ([seeds/t1-lever.txt](seeds/t1-lever.txt)),
  shaping from wire56's t1.ideal (115 piston events per cycle). **Exhaustive over K2's 4 free cells**
  ((4,4), (2,5), (3,5), (4,5)) x all 56 palette states = 9.8 M candidates in 103 s on 40 threads:
  best 43/79, no door: moving parts inside those cells (pistons, carried blocks, leaves + log) do
  not make K2's odd gap (agrees with wire56's lemma). Annealing 15 min with mechanism changes / lever
  moves: 43/79.
* **Ideal assist** (`--assist`, 18:00Z): a seed whose plan inputs are all plan-driven screens 79/79;
  the annealer releases inputs one by one (a released input must come from real wiring).
  * fm-7x8-000 (pistmodel, 3-signal groups plan, 10 off-plane inputs), lever at (-3,5): 8 of 10
    released at 77/79 in 25 min (left: (-2,0), (-2,2), the left members of the puller group: the
    crossing), the lever wandered to (4,6); continuing from that partial with only those 2 assisted:
    no gain in 10 min. Restart with the lever pinned on the left: (-2,2) wires, (-2,0), (2,-2),
    (2,4), (1,4) remain at 18:45Z.
  * wire56 t1 + K2 plan (6 inputs): 4 released at 75/79 (P5 (1,5) and PTR (4,2) remain).
  * vmid vm1 (ML leaves up, 12 inputs, oc plan of 15 pulses per edge), lever at (-2,-3): 5 of 12
    released with the door still at 79/79 in 3 min, 7 of 12 at 77/79 after 17 min (left: (-2,2),
    (4,2), (1,4), (0,5), (1,5)); swapped for vc1 (vm1 fails `door connect`).
  * vmid vc1 (ML leaves up by a carried puller, the center's elevator below, 12 inputs), lever at
    (-2,-4): 6 of 12 released at 77/79 in 3 min (left: (1,4), (4,2), (-2,3), (1,-2), (3,-1), (1,-3)).
  * wire64b w63 (7x8-014 in 7x9 = 63, bottom pockets wired for real, the top's 7 inputs assisted,
    4 of them in the plane: their cells stay reserved while assisted), lever at (-2,7): 4 of 7
    released at 77/79 in 3 min, 5 of 7 after 7 min (the top elevator (1,4) wired; left: the left stack
    (-2,2) and the right stack (4,2), each needing 3 pulses 6-7 gt apart). Restarted from that
    partial with only the stacks assisted and the focus grown to row 2 (the left stack's own feed
    cell (-2,2) sits in row 2: the first focus, rows 3..6, could never wire it): 15 min, no gain.
    Exhaustive k <= 3 over the left stack's 6 free neighbour cells ((-2,2..6), (-1,5)) with only the
    right stack assisted: best 47/79 (3.4 M candidates): the left stack's 3-pulse train does not fit
    in 3 changes there.
  * fm-7x8-000 with the lever pinned on the left: 7 of 10 released at 77/79 (left: (-2,0), (2,4),
    (4,1)).
  * The update-piston proposal move (vjeux, 01:51Z: a normal / sticky piston facing an empty cell
    next to a quasi cell, an observer's watched cell, a dropper or a note block; 1/8 of the moves):
    1 strict screen improvement credited to it in ~1M improving steps (cell edits: ~200k, assist
    toggles: ~10k) over the vc1 / w63 runs.
  * families 8x8-131 / wire64 f131L (in-plane plans: only 2 inputs could be assisted at the time):
    43-47/79.

## 4. What the assisted partials say
Every assisted run gets the easy inputs wired within minutes and stalls on the same kind of input:
a piston that needs SEVERAL pulses at fixed gaps (w63's side stacks: 3 pulses 6-7 gt apart; the
elevators' P4 / P5 trains) or one signal on the far side of the door (fm-7x8-000's left pullers,
t1's P5 / PTR). Those are the cells where a static wiring has no room; the next moves to add are
macro proposals for them: a delay-line pulse train (repeater chain taps ORed into one cell), a
watcher on a piston of the other side (the crossing), a carried redstone block shuttled by a new
piston (the strip lemma's moving corner source).

## 5. Next
* Finish w63 (63 < 72): the two stacks' 3-pulse trains; seeds and plan in [seeds/](seeds/).
* Run the assist on mechspace's corrected 7x8 list (58 mechanisms, 31 replay-passing plans) in
  parallel, a few minutes each, and keep the ones that release the most inputs.
* Macro moves for multi-pulse inputs (above); scaffolding (redmodel's new 1 gt delay element) in the
  palette once merged.

## Log
* 23:16Z start; 23:46Z evaluator pushed; 00:04Z agreement 2049/2049; 00:15Z palette (coordinator);
  00:38Z docs/coverage.md cosearch column; 00:55Z ideal assist; 01:52Z update-piston move, in-plane
  assist inputs; 02:00Z vc1, w63 runs.
