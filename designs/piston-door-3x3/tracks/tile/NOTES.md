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
`door tile-scan <designs> --tiles wide:3,stack:3,period:4..12:3,vperiod:4..10:3`, and `--tile`
on `minimize`/`harvest` (SearchArgs), `synth` and `resynth`.

## Results

| tiling | blocks per opening (long row) | open / close (N=3) | module | notes |
|---|---|---|---|---|
| period:8 | 52 | 40 / 36 (copy k: 36+2k / 32+2k) | `b38h-bus8.txt` | b38 + 14-block bus, note block -> hopper; slime-free, rotation-safe |

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
