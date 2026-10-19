# e78s: a SOLID 7x8 (no sculk) from the e78 mechanism (e78k, 2026-10-19 from 15:34Z)

## RESULT: the first solid 7x8 doors (all volume 56, solid, slime-free, flat, rotation-safe, verify --extra-offsets 28 WORKS)
| design | blocks | seamless | open/close | what changed |
|---|---|---|---|---|
| archive/e78k-h2-45b | 45 | FULL | 44/15 | THE FIRST (coordinator-verified on main 16:56Z): s11 + B1 fed from the RIGHT + a B'-watcher loop |
| archive/e78k-h3-38b | 38 | QUART (frame 9/12: NOT a record) | 36/19 | frame cells (-1,2) (3,2) (0,-1) changed: do not do this |
| archive/e78k-h4-42b | 42 | FULL | 36/19 | top-left watcher (-1,3) E + bottom-left loop = sticky piston (-1,-3) pushing the lamp |
| archive/e78k-h5-41b | 41 | FULL | 36/19 | right bottom = droppers: BR watcher QC-triggers dropper (4,-3) -> observer (3,-3) E -> dropper (2,-3) (B1 + gate), observer (3,-2) down -> K |

How h2 works (open): A1 t7 (lamp observer), K pulls BM at t13, B1 at t13 = observer (2,-3) facing EAST on a note
block (3,-3) under K's target (K's driver pulses toggle it), 2-gt spit; the gate (2,-2) watches (2,-3) (not B'), so
ONE gate pulse 2 gt after the B' pulse = A2 exactly at A's landing in F1. The late signal is a LOOP: observer (0,-3)
facing EAST on B' -> observer (-1,-3) E -> note (-2,-3) -> observer (-2,-2) down -> stone (-2,-1) re-fires BL's puller
(BL bounces into (0,0) and back) -> the BL-park watcher (-1,-1) keeps the lamp lit -> the lamp observer (0,-2) QC-fires
B' while A' is up (B2, t23) and the lamp's final off-edge fires A' (A3, t42). Found by `door ideal-rs --vary` over 4
cells of h1 (= rr2 + loop stone, which failed only at the final lamp edge) — no ideal inputs: ideal-rs is then a fast
4-position redsim screen, ~100k combos/s at 40 threads. h4/h5 came from the same --vary rounds (6 cells x 11-13 states).
Records need FULL: keep the three static frame stones (-1,2) (3,2) (0,-1).

Goal (coordinator 15:33Z): the first SOLID 7x8 = 56 door. Base mechanism = e78 (ring56's jf/layout J =
t7b mirrored): top row lever-held, sides fired at t3, bottom elevator A' (1,-2) / B' (1,-3), corner piston
K (3,-1) parks BM in F2 (2,-1), C goes down 2 into F1 (1,-1). The 7x9 solid t7b-ob40 times B' with a
row -4 chain; in 7x8 that row is gone.

## Best partial: s11.txt (+ s11.ideal), 39 blocks, ONE ideal input
Every input real except B2 = any 2-gt pulse into B' at t >= 29 on BOTH edges (q = 29..70 all WORK,
open 43 / close 15).
- A1: c77's observer (-1,-1) facing up on BL's landing cell (-1,0) -> lamp (-1,-2) (lit t5..15) -> observer
  (0,-2) facing west -> A' at t7 (pull BM into F1, lands t11) + the LATE PUSH at t17 (lamp off): A'
  extends into the empty F1, lands t21.
- K: right pullers delayed 2 gt (observer (3,3) W on the top-right head -> note block (4,3) -> observer
  (4,2) facing up -> MR/BR pullers at t5); BR watcher observer (4,-1) facing up -> dust (4,-2) on stone
  (4,-3) -> target (3,-2) (conductor) -> K: pulses t7 (harmless) and t11 (pulls BM out of F1 at t13).
  (Found by cosearch cs2 with K assisted.) Glazed terracotta (3,-3) is filler.
- B1: 3-hop chain on the lamp: observer (-2,-2) E -> observer (-2,-3) up -> observer (-1,-3) W -> stone
  (0,-3) -> B'. B' gets [11,13) (absorbed: A' lands at t11 while BM is still moving) and [21,23): the
  lamp-off chain lands EXACTLY on the late push's A' landing (lamp off + 6), so B' extends t22 and
  spits at t23 (1-gt effective) -> ONE gate pulse.
- Gate: p42's observer (2,-3) facing west on B' -> observer (2,-2) facing down into F2: BM (in F2 on
  open only) powers A' at F1 (A2, t26) and, after a 2-gt B2, the gate fires exactly when A' lands at
  (1,-2) (A3 via QC).

## Why B2 is hard
- B' is fed only from (0,-3) (the stone, from (-1,-3)) or (2,-3) (the gate's watcher), and its QC
  neighbours are A1's observer (0,-2), the gate (2,-2) (emits up only) and F1.
- No solid event exists on the left after t23 (the lamp is the only timer; the observers there watch
  inward and emit outward: their outputs leave the box). The late events (B1's moves, gate t26, A2)
  are on the right/center and cannot cross B'/A'/F1 to the left.
- A 2-gt B1 spit makes the (2,-3) observer pulse twice (B' extend, retract end) -> two gate pulses; the
  second re-extends A' at F1 and pushes C back up. So B1 must be 1-gt effective = B' powered exactly at
  an A' landing tick (only t11 with BM in F1, or t21 after the late push).
- A single B' hold does not work; a hold [13, +9..14) works only with a late push >= 24 as A3.

## Ideal windows (s2nob, K real at t7+t12)
- B' 1-gt pulse at t21 + any 2nd >= t29; extra pulses tolerated only at t7..10 or t14.
- K (with the real left chain): power must start at t10..11 (or t7 + t11); (7,12) breaks the close.

## Searches (all 0 unless said)
- door fill s2nob 5 left cells k<=5 (69M); --keep k<=4 left 8 (103M); shrink: left 8 + top-left 5
  --keep k<=3 (7.3M), left 8 cleared k<=6 reduced palette (682M).
- ideal-rs --vary right 5 cells x 16 states (K at t11): 0 of 131k; top-right 5 x 10: 0 of 100k.
- cosearch cs1 (B' assisted, left 9 cells): 0; cs2 (K + B2 assisted, right/top-right): K wired -> s11.
- Running: cs3 (s11, B2 assisted, 18 cells); shrink fills on s11; pockets3 plan-assisted pockets on s11.
