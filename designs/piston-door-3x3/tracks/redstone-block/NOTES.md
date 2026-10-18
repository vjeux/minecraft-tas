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
