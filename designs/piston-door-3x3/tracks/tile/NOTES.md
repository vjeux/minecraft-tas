# tile track: tileable 3x3 piston doors

## Definitions (judge: `mctas door verify <module> --tile <spec>`, crates/piston-door/src/tile.rs)

A tileable door is ONE module design copied N times with ONE lever for all copies:

* `wide:N` (WIDE): copies every 3 blocks along the wall. The openings touch and form one
  3 x 3N opening. Nothing may stand between two openings, so no side piston: every door block of an
  inner copy leaves up or down (or the copies share the movers).
* `period:P:N` (PERIODIC): copies every P blocks along the wall, each its own 3x3 opening, a
  wall of P - 3 blocks between openings. Smaller P is better.
* `stack:N` (STACK): copies every 3 blocks up: one 3N x 3 opening (a tall door).
* `vperiod:Q:N` (VPERIOD): copies every Q blocks up (a door on every floor).
* `step:dx,dy,dz:N`: any other translation (a series of doors in one corridor is impossible
  by the rules: each door would stand in front of the other).

One lever: copy 0 keeps the module's lever; every other copy gets the module's `relay:` block
(a header line, e.g. `relay: minecraft:redstone_wire[...]`; default: the cell stays empty) in
the lever's cell. So the module carries its own bus: copy k's machine must switch copy k+1.
Cells used by two copies must hold the same block (shared, counted once). A module may hold
several openings (`door`, `door2`, ...: a two-door tile unit).

The judge runs EVERY check of the single-door judge on EVERY opening at once: closed at rest
(9 identical plain full blocks each, walkways clear), nothing at all in the 3x3 columns in front
of or behind any opening at any distance, every block can stand, no random ticks, the lever opens
every opening (door cells and walkways walkable, no entities), the lever closes every opening (the
whole watched box back to its rest state), 3 cycles at every judge position (32 with
`--extra-offsets 28`). It reports blocks total, per opening, and per extra copy (blocks(N) -
blocks(N-1): the per-opening cost of a long row), and open/close ticks for all openings and per
opening. The rotation check turns the whole arrangement.

Tools: `door verify --tile`, `--tile-levers each` (diagnostic: every copy flips its own lever in
the same tick: do copies interfere when they all run?), `--tile-out` (write the arrangement),
`--tile-extra <end piece>` (a shared piece placed once: the lever and a connection at the start of
the row, for definitions that need one), `door tile-scan <designs> --tiles
wide:3,stack:3,period:4..12:3,vperiod:4..10:3`, and `--tile` on `minimize`/`harvest`
(SearchArgs), `synth` and `resynth`.

## Summary

* Real game check: the 3-copy arrangements of both bests (`b38h-obs8b-x3.tiled`,
  `b38h-vobs9m-x3.tiled`, written by `--tile-out`) PASS `door vanilla-check`: the unmodified
  vanilla 26.3 server matches the simulator tick for tick (3080 ticks at 8 positions x 3 cycles,
  plus a real-time 20 tps cycle). (`.tiled`, not `.txt`, so the leaderboard does not judge the
  arrangements as single doors.)
* PERIODIC: **45 blocks per opening at period 8** (`b38h-obs8b.txt` = BEST.txt), one lever, slime-free,
  rotation-safe, checked with 2, 3, 4 and 6 copies. b38 + an 8-block observer bus.
* VPERIOD: **49 blocks per opening every 9 blocks up** (`b38h-vobs9m.txt`), same family.
* WIDE and STACK: no periodic design yet, and none can exist without a shared end piece or a
  door-block channel: the two sides of a wide (tall) row share no cell (see below). The WIDE
  mechanism itself (12 pistons, each column = b38's center column) works with ideal inputs.
* No archived door tiles as is: wide/stack collide at once; periodic needs a bus.

## Results

| tiling | blocks per opening (long row) | open / close (N=3) | module | notes |
|---|---|---|---|---|
| period:8 | **45** | 64 / 60 (copy k: 36+14k / 32+14k) | `b38h-obs8b.txt` (BEST, BEST-period) | b38 (note block -> hopper) + 7 observers (2..8,5,-3) watching the lever's cell and carrying each edge to the next copy's relay dust; slime-free, rotation-safe, N=2, 3, 4 and 6 at 32 positions; a local minimum of `door minimize --tile period:8:3 --moves all --plateau` |
| period:8 | 46 | 60 / 56 (copy k: 36+12k / 32+12k) | `b38h-obs8.txt` | b38 + hopper + an 8-block observer bus (found by `door minimize --tile period:8:3` from b38h-bus8) |
| period:8 | 52 | 40 / 36 (copy k: 36+2k / 32+2k) | `b38h-bus8.txt` | b38 + 14-block dust bus, note block -> hopper |
| vperiod:9 | **49** | 64 / 60 (copy k: 36+14k / 32+14k) | `b38h-vobs9m.txt` (BEST-vperiod) | tile-aware minimize of b38h-vobs9: one dust dot on the tower's stone replaces the 2 observers on O; N=4 at 32 positions, rotation-safe |
| vperiod:9 | 50 | 76 / 72 (copy k: 36+20k / 32+20k) | `b38h-vobs9.txt` | b38 + hopper + 2 observers on O + a 7-observer tower beside the openings + stone + dust |
| wide | none yet | | `wide-mech.txt` + `wide-mech.sched` | mechanism works with ideal inputs (open 24 / close 26); wiring blocked, see below |
| stack | none | | | same obstruction as wide (left and right sides cannot talk) |

## Archived designs: can they tile? (`door tile-scan`, own-lever diagnostic, 4 positions)

* wide / stack: NONE. Every archived door moves its side columns sideways (pistons, slime or
  observers right next to the opening), which collide with the neighbor copy.
* smallest period (own levers): b38, b39: 7; fx1, mp8, st43, g10, gravity-2 g33: 8; fc2, flat43,
  flat-b-v7, logic-min-53: 9; m05: 10.
* smallest vperiod (own levers): 9 (b38, b39, fc2, m05, fx1, mp8, st43, logic-min) or 10 (flat43,
  flat-b-v7, g10, g33).
* With ONE lever none works as archived: none has a bus.

## Log

* b38 + dust bus at P=7: copy 1's center never climbed. Cause 1: the relay dust stands on the
  note block N and strongly powers it; a strongly powered note block (a conductor) emits into
  observer A's cell, which quasi-powers the south piston E all the time (with the lever, N is only
  weakly powered). Fix: N -> hopper (dust can stand on a hopper; not a conductor). Cause 2 (P=7
  only): copy k's right smooth stone S (strongly powered by observer C) touches copy k+1's left S,
  so it quasi-powers copy k+1's upper left piston D at copy k's time; with own levers fired in the
  same tick this is harmless, with a bus the copies run 2 ticks apart and it breaks. P=8 leaves a gap.
* Tile-aware minimize (`door minimize b38h-bus8.txt --tile period:8:3 --moves all --plateau`,
  12 workers, 18 min): 52 -> 46. The dust bus became an observer chain: an observer watches the
  hopper and strongly powers a smooth stone under one dust; 5 observers carry the pulse to the
  next copy's relay dust. b38 reacts to EDGES (the hopper and the lever cell are only watched by
  observers), so one pulse per lever edge is enough, and each copy runs 12 ticks after the
  previous one.
* VPERIOD: the observer tower must stand beside the openings (x = -1): a tower above the lever
  (x = 1) stands in front of the next door. The tower ends in the next copy's smooth stone
  (-1,5,-3), which powers dust (0,5,-3), which feeds the relay dust. Tile-aware minimize: 50 -> 49
  (the dust (0,5,-3) climbs onto the stone, so each copy's tower starts as the pulse arrives).
* 46 -> 45 by hand: the bus can watch the lever's cell itself (the lever in copy 0, the relay dust
  in the others): 7 observers (2..8,5,-3) replace observer-on-hopper + stone + dust + 5 observers.
  Replacing the hopper by an observer watching the lever cell (so the relay could be an observer)
  breaks b38: its top-row pull then comes 2 ticks late against the B window.
* Tile-aware minimize from 45 (`--moves all --plateau`, 11 min, 12 workers): no improvement;
  resynth runs on both bests (see the end).
* P = 7 stays broken with any delayed bus (the side smooth stones of neighbor copies quasi-power
  each other's side pistons; with every copy fired in the same tick it works, see the scan). A
  pure-dust bus (no delay) would make the copies run together, but dust reaches 15 blocks: 3
  copies at most.

## Speed vs blocks in a row

The bus sets how late the far copies open: copy k opens 2k ticks after copy 0 with the dust bus
(b38h-bus8, 52 per opening: one repeater per copy), 12k with the 8-observer bus (46), 14k with
the 7-observer bus (45); 20k / 14k for the vertical towers (50 / 49). A row of 10 openings with
b38h-obs8b opens after 36 + 126 = 162 ticks (8 s). A 2-tick-per-copy observer bus would need
dust instead (2 blocks per cell).

## Next ideas

* WIDE with an end piece (`--tile-extra`): top bus (observer chain, edge pulses) for A/B/E and a
  bottom bus carrying the lever's LEVEL (F needs one close-only pulse pair: gate it with a
  repeater locked by the level). Wiring rule: in a period-3 module, dust at (0,y,z) or (2,y,z),
  not both; B must be powered through the air cell above it (dust on top of B also
  quasi-powers A through B's cell).
* PERIODIC P = 7: needs side pistons whose quasi-connectivity cells do not touch the neighbor
  copy's strongly powered blocks (b38's side smooth stones do).
* Other edge-triggered doors (m05, the sf-holdA family) take the same observer bus; b38 is the
  smallest so far.

## WIDE and STACK: why no archived door tiles, and why wiring is hard

Mechanism (proven with ideal inputs, `door ideal wide-mech.sched`, 4 positions x 2 cycles, open 24,
close 24-26): every column is b38's center column. A_c (c,4,0) pulls T up, E_c (c,3,-2) pulls it
aside to (c,3,-1), B_c (c,5,0) lowers A_c, A_c pulls C up twice, F_c (c,-2,0) pulls M down. 12
pistons + 9 door blocks per opening. Waveforms (event ticks): A 1-3, 13-15 (lowered, QC through
B's head; it must be UPDATED: power it next to its lowered cell), 21-23; E 5-7; B 11-19 (direct
power from behind, not through the cell above B, which A's quasi-connectivity reads); F: 7-9 on
both edges, plus 15-17 and 21/23-25 on CLOSE only (no symmetric F works: a sweep of every pair of
symmetric F pulses 1..34 x 1..34 found none; the close needs the column pushed down by A's
lowered pulse and pushed back up by F).

Obstruction (topological): in a WIDE row the openings cover x = 0..3N-1, rows y = 0..2; at rest
nothing may stand in front of or behind them at any distance and the door plane holds only door
blocks. So for an inner copy the region above the opening (y >= 3) and the region below it
(y <= -1) share NO cell: no dust, observer or block can carry a signal between them. The bottom
row's pistons can only learn about a lever flip through the door blocks themselves (a column
pushed down from the top moves M into the bottom region, where an observer beside (c,-1,0) can
see it) or through a connection at the END of the row (not periodic). On the close edge the
opening is empty, so even the column channel is gone. STACK is the same problem turned 90
degrees: the left side and the right side of a tall opening share no cell. PERIODIC and VPERIOD
do not have it (the wall between openings connects both sides).

Ways out, not built yet: (1) `door verify --tile-extra <end piece>` (implemented): a shared end
piece placed once, e.g. the lever plus a riser down to a second bus under the row; each module
then carries a top bus and a bottom bus. (2) A mechanism whose open state keeps a top-bottom
channel through the door blocks. (3) Synchronous copies (pure dust buses, no repeaters): adjacent
copies' wires may then merge, but dust reaches only ~5 copies (15 blocks).

Period-3 wiring rule found on the way: in a period-3 module, dust may stand at x = 1 anywhere, but
at (0,y,z) and (2,y,z) not both (the copies' dust would touch across the boundary and merge,
which makes a dot a line and leaks one copy's pulses into the next one, which runs later).
