# Track t78: a 7x8 = 56 door (2026-10-19, from 10:07Z)

Brief: e88-t7b (63b = 7x9, 43 blocks) minus its row -4 -> 7x8, the delay-4 repeater hold of the
lower elevator piston B replaced by a sculk sensor (or a solid hold), then (coordinator 10:59Z)
mechspace's single-U 7x8 layouts with sensor wake-ups.

## 1. e88-t7b in 8 rows: blocked (redsim, ideal inputs + exhaustive --vary)
Files: wip/b0.txt (t7b without row -4 and its bottom chain), wip/br7.txt (bridge-e88-sculk-52b
without its column x=-3), ideal/*.ideal.
- Hold window (ideal/i1, i2): B's hold may be any length >= 8 (tested to 12+), but the gate observer
  (0,-2) must see its watched cell (0,-3) change in the SAME tick B extends (pulse 1: gate on when A
  lands at (1,-1); a tap 1 gt early is lost: A's extend event is cancelled when the gate turns off in
  the next tick's scheduled phase) and in the same tick B retracts (pulse 2, exact: A at (1,-2) is
  only QC-powered by BM, nothing updates it after its landing). D (a repeater: scheduled-phase edges)
  does both.
- A sculk sensor activates in the BLOCK-ENTITY phase: B moves 1 gt later, so the gate's first tap
  misses A by 1 gt (the off edge is a scheduled tick: pulse 2 is fine). A shuttle's redstone block
  is 3 gt early. Both need a boost (A spit to (1,-1) before the tap: bridge's lamp-chain boost).
- Both cells next to B ((0,-3), (2,-3)) are DIAGONAL to A: a hold sensor there always hears A's pulse
  0 (t15, distance 1.41, cannot be occluded: the corner cells are the gate observer and B), wakes at
  t16, B's push fails at t17 (A extended) and B re-extends at t20 when A's base lands, pushing A+BM.
- In 7x8 P_M's only short-pulse driver is an observer (-1,-2) facing down watching (-1,-3): that is
  the shuttle's redstone-block cell (a shuttle fires P_M only after its push, too late) or a sensor
  (then no shuttle). A wall torch at (0,-3) (scheduled edges, aligned like D) needs its attachment
  (-1,-3) strongly powered at rest: no constant source fits (a redstone block does not power through
  a block; a second torch or a moved redstone block has no driver cell).
- Exhaustive: br7 (boost + shuttle bottom; works with ideal P_M at 17..22 and SP hold >= 7 from t24)
  x 6 left cells x 16 states (sensors plain/calibrated x facings, wool, observers, note, lamp,
  rblock, stone, sticky) = 8.4M combos in redsim: 0 work. b0 + calibrated (0,-3): wakes at t8 (lever),
  B lifts at t9, pulse 0 lost; a wool door hides the top but the walls (t7) and A remain audible.

## 2. mechspace single-U 7x8 layouts (sl-*, explore/mechspace) with sensors
Files: mech/ (copied from explore/mechspace), cpt/ (ideal-rs --compact-dir), ideal/s53k*.ideal,
ideal/s49*.ideal, ideal/s79o.ideal (inputs moved off-plane for satwire --keep-ideal).
- --holds 40 on the compacted plans: in sl-049/053/079-sym the bottom is sensor-friendly (SU (1,-3):
  one pulse or 3 taps at ANY start 0..40 on both edges; SE(-2,0) via (-2,-1) and SW(4,0) via (4,-1):
  one pulse almost anywhere); the elevator (SD(1,4), SD(1,5), (1,3,1)) and SE(-1,3) take only their
  exact plan pulses. sl-078's corner SW(3,-1) and SU only work at t3.
- satwire (box -2,-2:4,5, --full-frame, palette block,dust,dot,rep,obs,torch,note,bulb,cmp,lamp,
  trapdoor,dropper,obsidian,glass,upiston,rblock), bottom inputs kept ideal: s53 compacted UNSAT core
  close (-1,3),(4,1),(-2,1),(1,4); s49 compacted UNSAT core close (2,4),(-1,3); --retime UNSAT;
  all six side pullers + SU kept: still UNSAT (core open (-1,3), open (1,4), close (-1,3));
  original 6 gt plans: s53o / s49o UNSAT, core close (-1,3), (1,4), (-2,1).

## 3. a78 batch (explore/mechspace 9ac0f311: 427 7x8 layouts with sym + oc plans)
New tool flags (commit 0f8c4596): `door ideal-rs <plans...> --holds-rank N` (per input, which
sensor modes hold10/hold30/taps3/taps2 work with the SAME start on both edges, starts 0..N; one line
per plan, fewest exact-only inputs first; 427 plans in 13 s) and `--offplane-dir DIR
[--offplane-below Y]` (moves the in-plane inputs of rows < door.y+Y behind the pistons they feed and
prints the `satwire --keep-ideal` list). Results in a78/ (holds-rank30-sym-cpt.txt, offplane-keep.txt).
- sym, compacted: 179 plans with 2 exact-only inputs, 240 with 3; the best are all elevator layouts
  (SE(-1,3)+SD(1,4)+SD(1,5)+static (2,3), or SD(1,4)+SD(1,5)+SW(3,3)+static (0,3)) plus inert extra
  pistons (no plan input). Bottom (SU, second bottom piston, SE(-2,0), SW(4,0)): taps3 at any start.
  Elevator inputs: a single exact start at most (taps3:6, taps2:0).
- satwire with the bottom kept: a78-159 sym UNSAT core close (3,3),(1,4),(4,1); a78-135 sym UNSAT
  core close (1,4),(-2,1),(-1,3),(4,1); a78-159 with all 6 side pullers + SU kept UNSAT core open
  (-2,1),(-2,0) (two KEPT pistons: their off-plane blocks QC each other).
- oc, compacted: 203 plans with 2 exact-only; in oc plans SE(-1,3) and the TL puller SD(0,4) become
  sensor-friendly (taps3 at any start) for some layouts (a78-010).
- Screen artifact (cosearch pockets, br7 + sculk palette): a calibrated sensor at (-2,-2) above SP
  walled by wool "drives" SP's hold only because it hears SP's own assisted extension; the pocket's
  verification rejects it (0 finds), ideal-rs confirms the sensor never wakes without the assist.
