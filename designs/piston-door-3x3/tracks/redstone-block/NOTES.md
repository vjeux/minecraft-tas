# Track redstone-block: movable power

Direction: move power sources (redstone blocks) with pistons/slime so one actuator switches
several pistons, and let the moving structures sequence each other instead of wires.

## rb1: 73 blocks, category solid (BEST.txt, verified 32 positions x 3 cycles)
Built by `mctas door gen rb1` (`crates/piston-door/src/designs.rs::rb1`). Mechanism = v2's UP
dance for the center column, but:
* **Both side structures are pushed open** (sticky pistons P_L at (0,4,0) facing west and P_R at
  (2,4,0) facing east, on top of the slime backbones). They are OFF at rest: no side torches.
* **P_T is gone**: the right structure has a slime at (2,3,0) next to T's lifted spot (1,3,0),
  so when P_R pushes the right structure east it drags T out of the column to (2,3,0); on close
  it pushes T back into (1,3,0).
* **One redstone block (RB_A at (1,4,-1)) shuttled by one sticky piston Q (0,4,-1)** powers A at
  rest; Q pushes it to (2,4,-1), where it powers P_R. So one actuator does "A off, then P_R on"
  in the right order (RB_A leaves A at once, lands next to P_R 2 ticks later, after T landed).
  On close Q pulls it back: P_R retracts (T pushed back), and A's extension is blocked by the
  moving T until T lands (the landing updates A), so A pushes T down last. No A torch, no A
  early/late nets.
* **BM is powered by a redstone block riding the left structure** (RB_BM at (0,-2,0), slime
  backbone extended down to y=-2): the left structure moving west un-powers BM, coming back
  re-powers it. No BM torch, no sides line.
* Lever on X_L (0,5,-1): X_L strongly powers Q and quasi-powers P_L through (0,5,0) (P_L is
  updated by Q's own state change).
* Q is held after the lever goes off by 3 repeaters (24 gt) into X_L, long enough for the
  closing dance (B window from O2 watching the lever net, stretched by R_B, as in v2).
* A's two pulses: observer O (1,5,1) watching R_B lights dust X at (1,4,1) on (1,3,1): X powers A
  up top directly, and A pushed down through the block under X (dust gives the 2nd-order update
  A needs; a quasi-only source leaves A stuck extended).

Validated first with ideal inputs (`m2/m3/m4-drive.txt` + `*-mech.txt`), then wired with the
router (two routes: lever net -> late chain input, lever net -> O2's watched dust).

## Pitfalls found
* Slime pushes count every block: the right structure with an extra BM redstone block was 13
  blocks (> 12), so P_R silently failed. Keep each structure <= 12 (S1 = 11, S2 = 10).
* A quasi-connected pulse source does not update A when A sits one lower: A stays extended
  until its head lands, 2 ticks too late for B. Use dust (2nd-order updates) or a direct source.
* Levers/dust/repeaters cannot sit on a piston that extends sideways (its top face is no
  longer sturdy): they pop.

## rb2: 57 blocks, category solid (BEST.txt)
`mctas door gen rb2`. Same mechanism as rb1, short nets:
* O2 sits at (0,6,0) right next to the lever, watching it; its back lights dust
  (0,6,1) -> (0,6,2) -> (1,6,2), which feeds R_B (the router's 14-block detour is gone).
* Q's late hold: 3 x 4-tick repeaters at (0,4,-2..-4) pointing into Q, fed by 4 dust from the
  lever ((0,6,-2..-4) then a step down to (0,5,-5), whose floor powers the last repeater).
`door minimize --moves all` finds nothing more (57 is a local minimum for it). rb1 minimized to
67 by a renet (rb1-min.txt).

## rb5: 52 blocks, category solid (BEST.txt)
`mctas door gen rb5`. rb2 with a **silent hold (BUD) for Q** instead of the 24gt late chain:
* H = block (-1,5,-1), next to Q's quasi cell (0,5,-1) (= X_L), strongly powered by L delayed
  16gt (lever -> repeater (0,6,-2) -> dust (0,6,-3) -> dust (-1,5,-3) -> repeater (-1,5,-2) -> H).
* When the lever goes off, Q is updated (X_L) but still quasi-powered by H, so it stays. H turns
  off 16gt later; a repeater switching a solid block only updates that block's neighbors, so Q
  is NOT updated and stays extended unpowered. At the end of the closing dance B lifts A back
  next to Q's head; the head forwards that neighbor update to Q, which retracts: RB_A returns,
  P_R pushes T back, A pushes T down.
* H must outlast every update Q gets during the dance (P_L's retraction, X's 2nd-order updates
  through Q's head at the first A pulse: off at L+12) and drop before A lands back (L+18): 14gt
  failed (H dropped in the same tick as the first pulse ended), 16gt works.
`door minimize --moves all`: no further removal.

## rb6: 48 blocks, category solid (BEST.txt)
`mctas door gen rb6`. rb5 with two shorter nets:
* Silent hold in 7 blocks: lever -> repeater (-1,6,-1) whose floor IS H (-1,5,-1) -> block
  (-2,6,-1) -> dot dust (-3,6,-1) -> its floor powers repeater (-2,5,-1) (facing west) -> H.
  The repeater's floor (-2,4,-1) touches the open left backbone, so it is glazed terracotta
  (slime never drags it). Note: dust/repeaters (PushReaction DESTROY) next to slime are not
  dragged either (only their floors are a problem).
* O2 -> R_B in 5 blocks, borrowed from mechanics' 51: O2 strongly powers (0,6,1), which lights
  the cross dust (0,6,2) (shape must be written as a cross; a lone dust canonicalizes to a dot
  that powers nothing sideways), which strongly powers (1,6,2) behind R_B.

## What did not work / dead ends
* RB_BM at (0,-1,0) (quasi-powering BM through its head cell, saving a slime): opens, but on
  close BM is never updated (the block landing next to the head cell finds M there, and M does
  not forward updates), so M stays down.
* A 14gt silent hold: H dropped in the same tick as the first A pulse's second-order update
  reached Q (through Q's head), so Q released mid-dance. 16gt works; 8gt can't (P_L's
  retraction and the A pulses update Q until ~L+12).
* rb4 bench: the hold repeater's floor next to the open left backbone was dragged and the
  repeater popped (fixed by moving it / glazed terracotta).
* Slime-free attempt (sf1: v2 with 3 pullers per side column): one wall torch at y=1 beside
  the column plus a block above it powers all three pullers (direct, quasi, through the block),
  but feeding that torch with dust on top of its attachment makes a clock (the dust also
  touches the block the torch powers) and the torch burns out; v2's L line at (5,4,0) also
  quasi-powers a right puller at (4,2,0). Not pursued further.

## Ideas not tried yet
* Shorten S1 (7 slimes): BM's redstone block costs 3 blocks; a cheaper BM power source that
  drops when the door opens (and updates BM when it comes back) would save 1-2.
* P_L at (0,3,0) (one slime less) with the lever block at (0,3,-1) powering both Q and P_L
  directly; needs the lever, O2 and the hold re-laid around y=3 (floors at y=2 are corridor).
* Trigger the B window from something already moving instead of O2 (no candidate found: the
  close dance must start right after the lever, and only the lever moves then).
