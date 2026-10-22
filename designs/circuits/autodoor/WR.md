# Automatic doors (study 21): board and mechanics

clk-obs, 2026-10-21 (the coordinator's brief: a 2x2 piston door that opens when someone
approaches from either side and closes after they have passed, never on someone standing in the
doorway; judged with a player-sized walker from both sides, walking and sprinting, two in a row,
one stopping in the doorway; minimum volume). The kind and the judge: `auto-door` in
[docs/circuits.md](../../../docs/circuits.md); every run is in the game (`mctas circuit door-run`,
`mctas circuit verify`: 4 positions x 4 rotations).

## Records

| design | blocks | box | open wait | close delay | how |
|---|---|---|---|---|---|
| [auto-door-lock-v5](archive/auto-door-lock-v5.txt) | 87 | 8x9x7 | 12 gt | 34 gt | lock-v3 with the tower's top torch hung on the tower (no frame beside the near piston's frame) and one dust fewer |
| [auto-door-lock-v3](archive/auto-door-lock-v3.txt) | 90 | 8x9x7 | 12 gt | 34 gt | one lock for the whole door: classic6's top frame and dust fed by a 3-torch tower |
| [auto-door-lock-v1](archive/auto-door-lock-v1.txt) | 124 | 12x5x7 | 8 gt | 30 gt | the first door with memory: a torch below each side's pistons, one lock per side |

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

## Limits

The plates' 20 gt grid is a timing rule: with two walkers at some gaps other than the judge's
(2 blocks), the second walker's inner plate can let go after the first one's outer plate, and the
door can then stay open with nobody inside until someone walks through again. The judge's walks
(and the records) are robust in every rotation and position.
