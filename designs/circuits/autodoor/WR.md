# Automatic doors (study 21): board and mechanics

clk-obs, 2026-10-21 (the coordinator's brief: a 2x2 piston door that opens when someone
approaches from either side and closes after they have passed, never on someone standing in the
doorway; judged with a player-sized walker from both sides, walking and sprinting, two in a row,
one stopping in the doorway; minimum volume). The kind and the judge: `auto-door` in
[docs/circuits.md](../../../docs/circuits.md); every run is in the game (`mctas circuit door-run`,
`mctas circuit verify`: 4 positions x 4 rotations).

## Records

The judge walks 10 single and paired walks and the two-walker sweeps (one behind the other at
every gap from touching to 6 blocks; from opposite sides crossing in the doorway at every offset).

| design | blocks | box | open wait | close delay | how |
|---|---|---|---|---|---|
| [auto-door-lock-v12](archive/auto-door-lock-v12.txt) | 91 | 7x7x9 | 13 gt | 40 gt | the fewest blocks: lock-v10 with the tripwire rows joined at y = -1 along x = 3, right under the right hooks' blocks (no wiring under the floor) |
| [auto-door-lock-v10](archive/auto-door-lock-v10.txt) | 95 | 7x8x9 | 13 gt | 40 gt | tripwire outer rows (a 10 gt re-check) 3 blocks out, the lock 18 gt; the rows 2 blocks out (track lock-v11) let the door close on a second walker |
| [auto-door-lock-v9](archive/auto-door-lock-v9.txt) | 97 | 7x8x11 | 6 gt | 40 gt | lock-v8 with the lock torch's base beside the near outer row (the torch hangs on it; the row's dust feeds it through the 2 gt repeater) |
| [auto-door-lock-v8](archive/auto-door-lock-v8.txt) | 99 | 7x8x11 | 6 gt | 40 gt | robust to two walkers: the outer rows 4 blocks from the inner ones, A 2 gt, the lock 12 gt (below) |

## History: the first doors (the judge's first 10 walks only)

These passed the 10 walks of their day and fail the two-walker sweeps (the door stays open with
nobody inside for some gaps); they left the archive when the sweeps came in. Their files are on
the track; lock-v5 is the selftest's calibration
[selftest/autodoor-plate-order.txt](../selftest/autodoor-plate-order.txt).

| design | blocks | box | how |
|---|---|---|---|
| [lock-v7](tracks/clk-obs/lock-v7.txt) | 87 | 7x8x7 | lock-v6 one level shorter (the far row's dust at y = -3, two blocks stop it climbing into the inner rows' dust) |
| [lock-v6](tracks/clk-obs/lock-v6.txt) | 85 | 7x9x7 | lock-v5 with the tower one row off the door plane (diagonal to the pistons) and the far outer row's spine at x = 2 |
| [lock-v5](tracks/clk-obs/lock-v5.txt) | 87 | 8x9x7 | lock-v3 with the tower's top torch hung on the tower (no frame beside the near piston's frame) and one dust fewer |
| [lock-v3](tracks/clk-obs/lock-v3.txt) | 90 | 8x9x7 | one lock for the whole door: classic6's top frame and dust fed by a 3-torch tower |
| [lock-v1](tracks/clk-obs/lock-v1.txt) | 124 | 12x5x7 | the first door with memory: a torch below each side's pistons, one lock per side |

## Why a door with plates alone fails

The door plane cannot hold a sensor: the pistons' blocks sweep it, and plates or string in it pop.
A pressure plate touches an entity whose box meets x, z in [1/16, 15/16] of its cell and y in
[0, 1/4]. A 0.5-wide walker centred within 0.3125 of the door cell's middle touches no plate in
the next rows, so someone standing in the doorway presses nothing, and a door that opens on "any
plate pressed" closes on them. The judge's calibration
[selftest/autodoor-no-memory.txt](../selftest/autodoor-no-memory.txt) (plates only) is refused at
the first stop: "the door closed on walker 1". The door needs memory.

## The memory: plates let go in the order they were pressed

A stone or wooden plate first pressed at t re-checks at t + 20, t + 40, ... and lets go at the
first check that finds nobody on it. A walker crosses a plate's zone (1.375 blocks) in under
20 gt (6.4 gt walking, 4.9 sprinting), so with every sensor a plate of one kind, a single walker's
plates let go in the order it pressed them. Tripwire breaks that rule: it re-checks every 10 gt,
so an outer row of string lets go before the inner plate on the way out.

Two rows on each side: inner plates (S, next to the door) and outer plates (R). Walking in, R is
pressed, then S; R lets go first, then S. Walking out, S is pressed again, then R; S lets go
first, then R. So the last row to let go tells the story: S last means someone came in and has
not gone out (keep the door open); R last means they went out (close it).

The lock designs read this with a **lockable repeater** A: its input is S (the door opens when A
is on), and it is locked (its side powered by a repeater from a torch) whenever no R plate is
pressed. Walking in, R unlocks A, S turns it on; R lets go and A locks while still on: the door
stays open for someone in the doorway, however long. Walking out, R unlocks A again; S lets go,
A turns off and the door closes; then R lets go and A locks off. The lock takes 4 gt (torch and
repeater) after R lets go, and S lets go 3 gt or more after R for one walker, so A, at 8 gt, stays
on.

## What broke on the way

- **Rotation 2 (lock-v2).** A piston head passes neighbour updates on to its piston: classic6's
  frame blocks beside the pistons' frames sit above heads, so a piston always hears its
  neighbouring frame's dust die. lock-v2 put a frame block (under the tower's dust) beside the
  near piston's frame with air under it. In two of the four rotations, that dust died last and
  the piston never heard it: it stayed out and the door never opened. lock-v3 put the dust on a
  top slab (it holds no power); lock-v5 removed it (the torch, hung on the tower, powers the block
  above it, which feeds the frame line).
- **Two walkers in a row with the rows next to each other (lock-v4 trial).** Plates shared by two
  walkers let go on their first presser's 20 gt grid: the second walker's inner plate and the
  first walker's outer plate let go on the same tick, the lock (4 gt) beat A (4 gt), and the
  door stayed open. A faster A (2 gt) won that tie, but the inner plates' pickup under the door
  left the doorway without a floor: a walker standing there fell into it.
- **Dust climbs every open step (lock-v6 trial).** The far outer row's dust first climbed into
  the near row's pickup at x = 0, from under z = 2; with the block above it open (it must be, for
  that climb), it also climbed into the inner row's dust at z = 1, and the outer plates powered
  the inner network. The climb moved to x = 2, past the inner rows' dust.

## Two walkers: the timing that makes it robust (lock-v8)

Each plate lets go on its own grid (first press + 20k gt). With one walker that grid is the press
order; with two, the last inner plate can let go up to 20 gt after the last walker left it. What
saves the door is distance: that walker leaves the end of the outer row L blocks later (L/v gt),
and the lock engages E gt after the outer plate lets go. Walking out, A (its delay d) must turn off
before the lock catches it: 20 + d < L/v + E. Stopping in the doorway (one walker, so the plates
let go exactly 20 gt after their first press, the outer one L/v gt before the inner one), the lock
must catch A while it is on: E < L/v + d. lock-v8: L = 4, d = 2, E = 12 (a 2 gt repeater into the
lock torch's base, the torch, an 8 gt lock repeater): at a sprint (L/v = 14.3 gt), 22 < 26.3 and
12 < 16.3, both with 4.3 gt to spare. Rows 2 blocks apart (lock-v5, L/v = 7.1 gt) cannot satisfy both.
With tripwire outer rows (lock-v10), a single walker's row lets go 10 gt after its first touch
instead of 20, so stopping needs only E < L/v + 10 + d: L = 3, E = 18 gives 22 < 28.7 walking out
and 18 < 22.7 stopping at a sprint.

## Limits

The two-walker sweeps keep both walkers moving. One walker standing in the doorway while a second
walks through still closes the door on the first when the second has gone: the lock reads plate
order, it does not count people.
