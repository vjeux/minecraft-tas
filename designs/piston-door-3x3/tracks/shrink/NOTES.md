# Track shrink: beat tonight's records by blocks placed, region by region (2026-10-19)

Brief (coordinator): same volume, fewer blocks. Targets: the 63b record (e88-t7b / t7b-rwfast, 43
blocks), the solid 8x8 (s27-bl52, 52), the sculk boards (bridge-p2-sculk-r 49, m79s-sculk-8x8 50).
Box: devvm55865 (shared box1). Tool: `door fill` (new, below).

## Results
- **t7b-nb2: 63b (7x9x1), 43 blocks, solid, FULL seamless, frame 12/12, rotation-safe, open 37 /
  close 25** (t7b-rwfast: 39 / 27): `archive/t7b-nb2-43b.txt` = BEST.txt. The bottom route's
  note block + observer pair becomes stone (2,-4) + note block (1,-4): the observer (2,-3) powers the
  stone, the stone powers the note block (no observer stage), so the row -4 chain is 2 ticks shorter
  on both edges. Same size and blocks as the record, faster: leads by the tie-break. `door verify
  --extra-offsets 28` WORKS (32 positions, rotation-safe), `door sheet --fill` frame 12/12,
  `door properties` FULL. Found by `door fill` re-filling the 3 route cells (11 working 43-block
  fillings in 0.2 s; 6 of them 37/25).

## The tool: `door fill` (crates/piston-door/src/fill.rs)
Exhaustive region filling judged as a REAL door on redsim: clear a region, try every filling of
kmin..=k cells from a palette (+ `--fix` cells always filled, `--cell-palette` per cell), prune
unsupported dust/repeaters/torches and pieces that touch nothing (one piece unless `--gaps`), screen
with cosearch's redsim judge at the first position, judge passes at 4 + `--extra` positions with FULL
seamless + hallway + frame 12/12, write finds. No ideal plan, so no region closure: a region whose
output other real wiring watches (t7b's bottom-right merger feeds the bottom-left chain, which is
why pockets2 had to close BR into a 13-cell region) is searched with the rest of the door real.
Speed: ~35k fillings/s screened on 64 threads (early fails), support pruning removes most fillings
with dust / repeaters.

## t7b's bottom, what it must do (ideal-rs windows on the 14 bottom wiring cells erased)
Plan `plans/nb.ideal` (A' pulse 0 a, P_M pulse pm, B hold b..b+bw, gate pulses g1/g2 open only):
- a (A' pulse 0, both edges): 0..16 with pm 19; pm >= a+3; b >= pm+3; bw >= 7 (8 for d1 >= 3);
  g1 = b+2..b+4 (A' lifted to (1,-1)); g2 >= b+bw+2 (A' back at (1,-2)), up to +20.
- B may also take TWO 2 gt pulses instead of the 8 gt hold (push+spit at b, pull at b2 with
  b2+w2 >= b+7) but then the second gate pulse must come >= w2+2 after B's second pulse: a gate
  watching B's own driver fires 2 too early (`plans/nb-2pulse.ideal`).
- With D fed by the BL puller watcher's double pulse (t9, t13: a d4 repeater absorbs a second input
  pulse 4 gt later), b = 17 and pm must be exactly 14 with a <= 11: no single-pulse source reaches P_M
  that early.
- `--holds`: B can take a calibrated sensor's 10 gt hold starting at 22..23 on both edges; P_M a
  single pulse 14..22.
