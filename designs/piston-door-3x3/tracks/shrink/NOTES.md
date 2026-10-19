# Track shrink: beat tonight's records by blocks placed, region by region (2026-10-19)

Brief (coordinator): same volume, fewer blocks. Targets: the 63b record (e88-t7b / t7b-rwfast, 43
blocks), the solid 8x8 (s27-bl52, 52), the sculk boards (bridge-p2-sculk-r 49, m79s-sculk-8x8 50).
Box: devvm55865 (shared box1). Tool: `door fill` (new, below).

## Results (all verified by `door verify --extra-offsets 28` on the JVM: 32 positions, rotation-safe)
| design | volume | blocks | open/close | what changed |
|---|---|---|---|---|
| **archive/t7b-ob40-40b.txt** (= BEST.txt) | 63b (7x9x1) | **40** | 41 / 29 | a MOVING OBSERVER replaces t7b's merger + route (below) |
| archive/t7b-pw42-42b.txt | 63b | 42 | 37 / 25 | route stone + note block -> one normal piston whose head pops into the watched cell |
| **archive/t7b-sk40-40b.txt** (SCULK) | 63b | **40** | 47 / 47 | ob40 with the left wall driven by a plain sculk sensor (-2,4) hearing the lever + observer (-2,3) up + wool (-2,2) (sculk board leader was 49) |
| archive/t7b-ob40e-41b.txt (ELIGIBLE) | 70b (7x10x1) | 41 | 41 / 29 | ob40's record-eligible variant (door relever: note block (1,5) under the lever; `door eligible`: ELIGIBLE). A t7b-family door cannot be eligible inside 63b: the lever's support is the T puller, which has no free side for the repeater rig and can only be powered from y=5 |
| archive/t7b-p42-42b.txt | 63b | 42 | 53 / 21 | bottom-right re-filled with 3 wiring sticky pistons |
| archive/t7b-nb2-43b.txt | 63b | 43 | 37 / 25 | route note block + observer -> stone + note block (one stage less) |
| **archive/s27-tlp50-50b.txt** | 64b (8x8x1) | **50** | 39 / 45 | pockets2's s27-trbl51 (51) + top-left A part: sticky piston (-3,3) up replaces observer + stone |
All FULL seamless, frame 12/12 (`door sheet --fill`), hallway ok (redsim), solid, slime-free, flat.
Sculk (not archived, ties only): s27-trbl51 top-left with a plain sculk sensor at (-3,4) = 64b sculk
50 (ties m79s-sculk-8x8's 50 blocks, faster 35/35): finds/s27tlA.

### t7b-ob40: how it works (only the bottom-right differs from t7b)
Top (lever on the T puller holds the ceiling), walls (park watchers), row -4 chain ((0,-4) -> stone
(-1,-4) -> dust (-1,-3) -> D (0,-3) d4 for B's 8 gt hold, observer (-1,-2) -> P_M, gate (0,-2)
watching D into BM's park (0,-1)) are t7b's. The bottom-right, 5 blocks instead of 7-8:
- the BR watcher (4,-1) fires twice (t5, t9: the BR puller extends and retracts);
- it powers a sticky piston (4,-2) facing WEST directly: pulse 1 pushes the observer (3,-2) (facing
  east) to (2,-2) (spit, it stays), where it lands next to A' and fires into it: A's pulse 0 (t9);
- pulse 2 pulls the observer home to (3,-2); landing, it fires into the air cell (2,-2), which
  QC-fires the normal piston (2,-3) facing down: its head pops into (2,-4) (t15), the observer (1,-4)
  facing east sees it and fires the row -4 chain through (0,-4) (dust t19, P_M t21, B t27).
The moving observer is both the merger (one pulse into A' per edge) and the router.

## The tool: `door fill` (crates/piston-door/src/fill.rs, on main)
Exhaustive region filling judged as a REAL door on redsim: clear a region, try every filling of
kmin..=k cells from a palette (+ `--fix` cells always filled, `--cell-palette` per cell), prune
unsupported dust/repeaters/torches (also base blocks standing on region cells) and pieces that touch
nothing (one piece unless `--gaps`), screen with cosearch's redsim judge at the first position, judge
passes at 4 + `--extra` positions with FULL seamless + hallway + frame 12/12, the volume
(`--max-volume`), blocks (`--max-blocks`) and the rotation check (redsim copy of `door verify`'s:
`rot_block` / `rotate_rsdoor` / `rotation_check_rs`, agrees with the JVM on every case tried;
`--any-rotation` to skip, `--rotation-only` to calibrate). Palette shorthands include `sticky` /
`piston` (retracted, 4 facings) and `--allow-pistons` re-grows a region holding a wiring piston.
No ideal plan, so no region closure: a region whose output other real wiring watches is searched
with the rest of the door real (t7b's bottom-right merger feeds the bottom-left chain, which forced
pockets2 into a 13-cell closed region). Speed: 300-700k fillings/s screened on 96 threads.

**Lesson: put pistons in the palette.** Every win after 43 came from pistons as wiring: a normal
piston as a 1-block watched relay (pw42), a sticky piston moving an observer (ob40), a sticky piston
toggling a bulb (s27-tlp50). Pockets/satwire palettes without pistons cannot see these.

## Screens that found nothing (exhaustive for their region, palette and k)
On t7b-rwfast / t7b-nb2 (43), searching 42 (no pistons in the palette):
- bottom-right 12 cells with the left kept: k<=6 (+BR watcher), 13 states; k<=5, 32 states; --gaps k<=6, 13 states.
- left 11 cells with D + gate fixed: k<=4, 32 states. Middle 15 cells (merger, D, gate fixed): k<=6, 17 states.
- sculk: walls k<=2, route k<=2, middle k<=6 (sensors, calibrated, wool): nothing.
On t7b-pw42 (42), searching 41: middle 15 cells with pistons k<=5 connected: nothing.
On t7b-ob40 (40), searching 39: walls k<=2 (pistons, --gaps), left k<=3 (D + gate kept), bottom-right
k<=3 + BR watcher (pistons, --gaps): nothing. Middle 16 cells k<=5 with pistons (connected, 37
states): nothing. Left 10 cells with D and gate FREE, pistons, --gaps, k<=5 (311M screened): nothing.
Middle with --gaps k<=5 (reduced palette with pistons): nothing (1.2G fillings). Coupled bottom-right
+ (0,-4) (-1,-4) (the route's end and the left chain's first cells), k<=5 free + BR watcher, 15
states with pistons, --gaps: nothing. Sculk left (10 cells, D/gate free) k<=5 with --require-any
sculk: nothing.
s27-tlp50 left column ((-3,0..2), (-2,3), (-3,4), (-2,-1)) k<=2, 46 states with pistons + sculk: nothing.
e78 (helper for e78k, 7x8 sculk, K ideal): 5 right cells + (-2,-1), k<=4 with pistons + sculk
(46 states, --gaps): nothing; 7 free cells k<=4: nothing. e78k's r1.txt (K real via a piston
relay, one open flaw at t45): left cells k<=5 and + top cells k<=4: nothing. r2.txt (loops after
the open): 6 cells k<=4, 8 cells k<=4: nothing. nb.txt (7 cells): k<=4 full palette and exactly
k=5 reduced (316M): nothing. r1 right cluster (relay included) k<=5 (78M) and + (2,-3) k<=5
(346M): nothing. e78k handed off at 15:03Z (explore/q78 tracks/q78/HANDOFF.md).
e78k's SOLID 7x8 s2nob.txt (explore/q78 d77fb3d8: c77's stone e78st with the sensors removed, only
B' ideal: a 2 gt pulse at exactly t21 + a 2nd >= t29): --keep k<=3 over the left 8 + top-left 5
cells (18 solid kinds with pistons): nothing (7.3M); the 8 left cells cleared, k<=6 (18 states:
observers, lamp, stone, sticky/normal pistons, note, dust, redstone block, torch), --gaps:
nothing (682M). --keep k<=4 over the 13 cells: stopped at 29% (0 of 158M) when the base moved.
s11.txt (only B2 ideal): --keep k<=3 over 24 wiring cells (93M), 9 cells cleared k<=5 (362M): nothing.
e78k then found the FIRST SOLID 7x8 (h2, 45, archive e78k-h2-45b). On it: top-left 5 cells cleared
k<=3 gave **44** (the TL watcher on the TL head at (-1,3) powers the dust (-2,3) directly; stone
(-2,4) gone): archive/e78-h2t44-44b.txt, verified 32 + rotations; pockets3 had found the same edit
minutes earlier (main: archive/e78s-56b-44-44b.txt). e78k's later FULL bases h4 (42) and h5 (41,
main: e78k-h5-41b): every clear-and-refill region came back empty (FULL filter, frame cells
(0,-1) (-1,2) (3,2) kept): h5 bottom-left 8 cells k<=5 (169M) and k<=4 with 18 kinds (runs/
h5-left8b.log), bottom-right 7 cells k<=5 (95M), B'-feed 6 cells (0,-3) (-1,-3) (2,-3) (3,-3)
(2,-2) (0,-2) k<=5 (29M); h4 top-left and top-right 5 cells k<=2. Split: pockets3 = keep-mode
over all wiring cells, c77 = wider palettes on the bottom-right, e78k = fast `ideal-rs --vary`.
s27-trbl51 top-left A part: no 5-block solid filling without pistons (k<=5, 32 states, --gaps).

## t7b's bottom, what it must do (ideal-rs windows on the 14 bottom wiring cells erased)
Plan `plans/nb.ideal` (A' pulse 0 a, P_M pulse pm, B hold b..b+bw, gate pulses g1/g2 open only):
- a (A' pulse 0, both edges): 0..16 with pm 19; pm >= a+3; b >= pm+3; bw >= 7 (8 for d1 >= 3);
  g1 = b+2..b+4 (A' lifted to (1,-1)); g2 >= b+bw+2 (A' back at (1,-2)), up to +20.
- A' and P_M also accept an ODD pulse train (3 pulses >= 4 apart): `plans/nb-a3.ideal`, `nb-p3.ideal`.
- B may also take TWO 2 gt pulses instead of the 8 gt hold (push+spit at b, pull at b2 with
  b2+w2 >= b+7) but then the second gate pulse must come >= w2+2 after B's second pulse: a gate
  watching B's own driver fires 2 too early (`plans/nb-2pulse.ideal`).
- `--holds`: B can take a calibrated sensor's 10 gt hold starting at 22..23 on both edges.
