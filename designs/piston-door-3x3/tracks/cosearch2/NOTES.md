# Track cosearch2: finish the cosearch partials, then pocket-exhaustive wiring

Brief (coordinator for vjeux, 2026-10-19 02:28Z): continue track cosearch (redsim joint mechanism + wiring
search, `door cosearch search --assist`): a 1-wide, slime-free, FULL seamless + hallway door below 72b;
targets 7x9 = 63 and 7x8 = 56. Later (03:05Z) main line: POCKET-EXHAUSTIVE WIRING. 03:36Z (vjeux): FULL
FRAME: when open, all 12 frame cells around the opening hold plain full blocks (door blocks or static
stone). 03:02Z: one working 56/63 door is the goal; judge at 4 positions while searching.

## 1. The old 77/79 partials are broken doors (read before continuing any of them)

`door cosearch trace --assist <plan> --assist-cells <cells> --grid x0,y0:x1,y1` (new) replays a
partial with its still-assisted inputs and draws the plane tick by tick. On cosearch's
`partials/w63-2as.txt` ("5 of 7 inputs wired, 77/79"): the CENTER NEVER OPENS (it is still at
(1,1) when the open phase ends, both cycles); the elevator (A x3, B x2, P_T) is not wired at all
(A fires once at t2, B and P_T never); 77/79 = 79 minus the center's open point in 2 cycles. The
search traded a door cell (1 point per cycle) for an assist release (`--assist-weight` 4): with
weight 4 every "released at 77/79" count is meaningless. Also: its rest state needs a redstone
TORCH CLOCK that runs ~30 ticks during settling (torch (3,4) toggling the elevator) until it burns
out (8 turn-offs in 60 gt); the real game agrees (door trace: settled in 34 ticks, torch off).

Fix: `--assist-weight 0.45` (< 1 screen point: a release never pays for a broken door cell), and
the search keeps the best FULL-POINTS candidate apart (`partial-full-live.txt`, fewest inputs
still assisted): the only meaningful partial.

## 2. Tools (all `mctas door cosearch ...`, crates/piston-door/src/cosearch_search.rs)

* `search`: `--drive-weight` (shaping: per assisted input, the plan's on/off edges that the REAL
  wiring alone also feeds to its piston, assist blocks taken out for the check, earn points),
  off-plane assist rest blocks (a level input's redstone block behind its piston is placed with the
  design while assisted), `--judge-extra` (default 4 positions), `--offplane` (every in-plane plan
  input the design leaves empty moves behind the pistons it feeds), `--lever x,y` for mechanism
  files without a lever port.
* FULL FRAME (vjeux 03:36Z) in the redsim evaluator: `record_ok` needs frame 12/12 at open, the
  eval line prints `frame N/12` (flat43 reads 10/12); the search screen scores the 12 frame cells
  as plain blocks when open (air no longer counts).
* `pockets` (pocket-exhaustive wiring): windows of every plan pulse / level edge by shifting it
  alone; the free cells the plan never sweeps split into pockets; every filling of up to `--k`
  blocks of each pocket (connected, touching the build; the lever on a candidate cell next to it
  counts as one) is screened with the rest of the door plan-driven; a filling is kept when the door
  still works and every real edge it feeds a still-assisted input falls in the window of one of
  that input's plan groups (pulse width within `--width-tol`); then disjoint fillings are combined
  depth first over the plan groups (cheapest first), each combination verified (screen, then the
  redsim judge at 4 positions + FULL + hallway + frame); true partials written as they improve.
  `--keep-lever` (the seed's real lever stays), `--region-cells` (clear cells to re-wire them),
  `--debug-fill <design>` (one filling's real traces vs the plan, and the groups it matches).
  Receivers: pistons (direct or QC) and trapdoors / lamps / note blocks / bulbs / droppers next to
  the input. Speed: ~18k fillings/s on 24 threads (screen + per-tick real-drive check).

## 3. Results

* f131X (7x8, wire131's in-plane plan): annealing with weight 0.45 reached a TRUE 79/79 partial
  with 4 of 6 inputs left (`runs/f131x-a2-full4.txt`): D2+ held by a wall torch (3,5) on the
  lever's block (4,5) (QC through (2,5)); D1's b shot from a redstone lamp (-1,5) lit by DE's input
  cell (0,5), watched by observer (-1,4) watched by observer (0,4) (emits east: D1 at (1,4)
  directly and lowered at (1,3) by QC through (1,4)); a second run released DE instead
  (`runs/f131x-b1-full4.txt`). Pockets k=3 (all 4 pockets, 47-state palette, 47 s on 150 threads):
  the TR pocket drives D2+ AND the R pair levels on both edges with 3 blocks (with the frame stone
  (3,2) as a conductor); TL (4 cells with the in-plane plan) drives only X's close edge.
* f131N-ff / f131F-b2 (wire131's 63b, frame stones in, one input left: N's train at the trapdoor
  (-2,4)): N has exactly ONE free neighbour, (-2,5) (the others: the lever-watching observer
  (-2,6), the observer (-1,5) that is the repeater (-1,6)'s floor, the box edge). All 47 states of
  that 1-cell pocket screened with everything else real: none gives N's train inside its windows
  (n1 0..1, n2 12..13, n3 20+): no filling exists there with the rest locked; N needs the lever
  region restructured. 25 min of annealing (N assisted, focus rows 3..6 x -2..1, 140 threads,
  65M screens): no release.
* ring56 s1 (frame-12/12 7x8, ring56's hand mechanism): 4 pockets of 5-8 cells (BL 7, TL 5, BR 8,
  TR 5); windows: every top / side input fires once at t0 with window 0..40, the bottom elevator
  train A' 0 [0,3], K 6 [3,10], B' 12 [8,15], A' lifted 16 [13,19], B' 22 [19,24], A' 28 [26,68]
  (open), A' 0, K 6, B' 12, B' 18 (close). k=3 (47 states, 94 s / 150 threads), k=4 (21 states,
  125 s), k=5 (21 states; BL pocket 68M fillings in 427 s): fillings for every top / side input
  and K, and at k=5 a 5-block WATCHER for both of B''s pulses on both edges (observer on BL's
  puller -> stone -> delay-4 repeater -> observer: pulses at t13 and t21, into B' by QC through
  A''s cell: `runs/pk-s1-k5/pocket0-set1.txt` on the dedicated box); no filling drives A'. The top
  pockets are trigger-isolated (history.md "Below 72"): s1 cannot be wired with one lever.
* f131X-ff (`seeds/f131X-ff.txt`: f131X + static frame stones (3,2), (0,-1), (2,-1), level inputs
  moved off the plane by `--offplane`): screens 79/79 with frame 12/12 while assisted; pockets
  k=3: the TR pocket drives D2+ both edges, the TL pocket D1's b shot; annealing (44 threads, 30
  min, drive shaping): a true partial with 5 of 7 inputs left (P3+ and DE released).

## 4. Known-answer calibration on the community 7x9 (c79-closed, main tracks/c79)

`door cosearch plan-from <door> --region <cells>` writes the ideal plan of the pistons a wiring
region feeds: at every tick, a powered piston that loses its power when the region's blocks are
taken out (raw, put back) is fed by it; the plan's redstone block sits behind it (z+1) at exactly
those ticks (the door's other wiring keeps its share), a cell fed like the one above it left to
that block's QC. Then `pockets <door> --assist <plan> --region-cells <cells> --keep-lever
--no-hallway --k <region size>` erases the region and searches it exhaustively.

| region (blocks) | palette | fillings, time | result |
|---|---|---|---|
| top-left (6: dust line + observer on the elevator + stone on TL's puller) | stone, dust, observer (7 states) | 2.9M, 41 s / 44 threads | RE-FOUND: 3 finds, `door verify` WORKS (51 blocks) |
| bottom-right (4: observer on MR's puller, dust, 2 stones) | same | 10k, 1 s | RE-FOUND: 2 finds |
| bottom-left (6: observers / dust / stone into BL's puller, the corner puller, the bottom puller) | same | 249k, 23 s | RE-FOUND, plus a 5-block variant: **c79-pockets-50b.txt, 63b (7x9x1), 50 blocks, `verify --extra-offsets 28` WORKS, rotation-safe, FULL seamless, frame 12/12** (hallway: no, like c79) |
| bottom-middle (3: observer on the signal piston's cell, stone, dropper) | stone, observers, dropper, dust | - | not testable alone: bottom-left's observer watches its dropper (a region must include the blocks other regions sense) |
| top-right (7: lever line repeaters, observers, dust + dropper level) | the 6 used states | 722k, 22 s / 44 threads | PARTLY: the dropper-held right side (2 blocks) and the lever's repeater line into the elevator top (4 blocks) re-found, together (6 blocks); the observer on the dropper -> static stone -> elevator link (shared with top-left's dust line) not matched: (1,3)o2 / (1,4)c4 left |

What it took (all fixed in the pocket search): (1) DOUBLE DRIVE: a filling and the plan's block
on the same piston broke correct fillings (pulses of 3 gt instead of 2, spit vs pull): every input
a filling feeds is screened again with that input RELEASED; (2) a plan block powers EVERY piston
next to it (direct + QC): the real wiring stands in only when it powers all of them (else ML's QC
was missed and the door failed with TL alone pulled); (3) plan-from emits only the region's own
ticks, and a filling's traces are matched with the seed's remaining wiring taken out tick by tick;
(4) the screen accepts a lever on at rest (either polarity), finds without the hallway check
(`--no-hallway`: c79's lever on top of its block fails redsim's hallway check).

## 5. Other wirings of c79 (the coordinator counts them as new doors)

The same region runs with a 25-state palette (stone, dust, observers, note block, repeaters,
torches, wall torches, lamp, redstone block, droppers): bottom-right k=4 (6M fillings, 43 s / 150
threads): 12 finds (`c79var/br/pockets-63b-51-*.txt`, all 63b, 51 blocks, redsim judge + FULL +
hallway + frame 12/12; `door verify` WORKS on the one checked); bottom-left k=5 (46M, 336 s): the
50-block door again (the only 5-block full filling); top-left k=5 (283M fillings) still running on
the dedicated box when this was written (`runs/c79var-tl.log`, out dir `c79var/tl` there).

## 6. Next

* Every region of c79 with the full palette and k = region size (other wirings, fewer blocks);
  regions must include the blocks other regions SENSE (bottom-middle + bottom-left's dropper).
* The periodic rest rule (main 96a62d6) in the screen; an off-plane lever in the grid; powered
  rails in the palette.
* Broad pocket runs on c79-family mechanisms (7x8: mechspace c78-*, w63top u7-m*, seq56 m4/m5,
  ring56 t8/t10; 7x9 variants; 8x8 refits), seeding regions with c79's patterns; whole-mechanism
  runs need the lever's pocket to reach every other pocket through the mechanism (ring56 s1 fails
  this).
