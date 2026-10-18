# 3x3 piston door

## The rules (what `mctas door verify` checks)

A design names two ports: `door` (the minimum corner of the 3x3 opening, which lies in the
plane `z = door.z`) and `lever`.

* At rest (lever off) the 9 door cells hold **9 identical plain full blocks** (no slime, honey,
  pistons, redstone or other mechanism blocks), and the walkway (the opening extended 3 blocks
  in front of and behind the door) has no collision.
* After the lever is switched on and everything settles, the 9 door cells and the walkway are
  walkable.
* After the lever is switched off and everything settles, every block of the build is back
  exactly where it was (same states, after canonicalizing wire shapes), with no entities
  (nothing dropped).
* All of that for 3 cycles, at 4 different world offsets (redstone dust update order is
  position dependent, so a design that only works at one position is rejected).

The score is the number of non-air blocks, piston heads excluded.

## v2: the "UP dance"

```
            B        sticky, facing down, retracted
            A        sticky, facing down, extended (head on T)
      .  T  .        top row:    T = top-middle
 s  #  C  #  s       middle row: C = center
 s  #  M  #  s       bottom row: M = bottom-middle
 s  #     #  s
```

* The left and right columns (3 blocks each) are glued to slime backbones and pulled
  sideways by one sticky piston per side.
* M is pulled down by a sticky piston below it.
* T is pulled up by A. A second sticky piston behind the wall (P_T) then pulls T back one
  block, out of A's way.
* C has to climb two cells, and the only piston above it is A. So A rides on B. B pushes A
  down one cell. An observer pulse makes A extend and retract, which pulls C up one cell.
  B pulls A back up, and a second pulse pulls C up again. Both pulses come from one observer
  watching the repeater that powers B, so every B edge fires exactly one A pulse.
* The observer lights a dust cross next to A. The dust powers A directly when A is up. When
  A is down, it powers A through the block under the dust. That path also updates A
  (second-order dust updates), which matters: a quasi-connected source alone would leave A
  stuck extended.
* Closing runs the dance backwards. B pushes A and C down together. The pulse makes A push C
  into the center, and the pulse ends while C is still moving, so A drops it there. B lifts
  A, P_T pushes T back, and A pushes T into the door.

Timing (game ticks after the lever flip):

| | open | close |
|---|---|---|
| sides + M (NOT lever) | 2 | 2 |
| A loses power (lifts T) | ~6 | n/a |
| P_T (NOR of L+8, L+28) | 10 | 30 |
| B window (observer on the lever, stretched to 8gt) | 14..22 | 14..22 |
| A gets power back (late chain) | n/a | 34 |

## Tools

* `mctas door drive <script>` places a design, applies scripted inputs (`set`/`clear`/`use`)
  and prints a plane per tick as ASCII. This is how the mechanism was validated with "ideal"
  inputs (redstone blocks placed by hand) before any wiring existed.
* `mctas door route` is a dust auto-router. It finds the fewest new blocks connecting two
  cells, following the wiring rules: every dust needs a floor, steps need clearance, no new
  block may touch a component it shouldn't power, and nothing goes into a cell the machine
  sweeps while it runs (simulated) or into the walkway.
* `mctas door minimize` removes single blocks greedily, in parallel, while the door still
  verifies.
* `mctas door video` renders the build with the real client, auto-framed so the machine
  fills the frame.

## Block count history

| design | blocks | note |
|---|---|---|
| v1 | 125 → 121 | slime as the top-middle door block: invalid |
| v2 | 151 | first valid design |
| v2 | 143 → 128 | unified late chain, router-laid wires, A torch fed off the sides line |
| v2-min | 126 | greedy single-block removal |
