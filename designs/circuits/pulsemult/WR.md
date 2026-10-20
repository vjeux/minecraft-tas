# Study 11, pulse multipliers / dividers / trains (fly's half): world-record targets (2026-10-20)

vjeux 12:45Z: world-record quality, quality over quantity. The site critique (C37) set the bar for
bursts and dividers: an item-driven design (N items in a container, a small clock that moves one
item per cycle and pulses, a comparator that stops at empty) gives any N in a constant size.

| variation | record now | bound / target | status |
|---|---|---|---|
| burst N = 2 | 7 blocks (pm-burst2-bulb) | an edge detector is 3 blocks plus a doubling stage: 6-7 | near the bound |
| burst N, any N (tunable) | chase clocks: 9 (N=3), 19 (N=4); the timer-gated chase: 27 + 4 per 2 pulses (burst-gate) | CONSTANT size for every N: N items in a dropper, a clock that fires the dropper once per pulse, a comparator reading the dropper that ends the burst at empty, a hopper (locked while it runs) returning the items for the next input. About 12 blocks for N up to 576 (9 stacks of 64): the first item-driven card is the target | build (item burst) |
| burst with ONE re-trigger class | none | ignore-while-busy falls out of the item design (a pulse during the reset finds the latch powered) | with the item burst |
| divider N (square) | 13 / 16 / 23 blocks for N = 3 / 5 / 7 (dropper ring + bulb) | constant size for every N: an item counter (N items moved one per input edge between two containers, the direction flipped by a bulb at empty) | build (item divider) |
| fast divider (not a power of 2) | pm-div3-fast, 55 blocks, 8 gt clocks | smaller: a two-bulb ring | open |
| pulse train | 3 blocks (observer pair + gate), period 4 | 3 is the bound (two observers and a gate) | RECORD = BOUND |

Why the present (item-free) designs can still win a card: no items to load (nothing to refill or
lose), 4 gt pulse periods (a hopper moves one item per 8 gt), and their re-trigger behaviour is
measured; the item designs will be judged on the same axes and the cards keep whichever is better
on each.

## The item burst: what the design has to solve (worked out, not built yet)

The gated 3-block observer chase (pm-train-observers) is the pulse source; each chase pulse fires a
dropper D holding the N items, which drops one item into a hopper H above it (H points back into D).
What makes it hard is the stop and the reset, not the counting:
* a comparator reading D sees every change, including the refill, so a loop that restarts on
  "D changed" restarts while H refills D; the end of the burst must flip a latch (a copper bulb L
  toggled by the input) whose state blocks the chase, and the empty-D edge must reach L as a rising
  edge (a torch under D's comparator), while L's own state locks H during the burst;
* every part of that loop has its own direction (a comparator reading D outputs away from D, the
  piston must touch the observer it pushes, the torch must touch L), so the latch, the chase and D
  sit on three sides of D and the wiring between them is where the blocks go; a first layout needs
  about 12-14 blocks, constant for N up to 576 (nine stacks of 64);
* without locking H, D drains at one item per 8 gt net (two out per hopper cooldown, one back), so
  the burst gives about 2N pulses for N items: still a deterministic tunable family.
Divider: the same counter with two droppers whose direction a bulb flips at empty.
