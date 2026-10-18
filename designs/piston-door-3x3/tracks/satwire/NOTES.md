# Track satwire: SAT synthesis of a door's wiring for a fixed mechanism

`mctas door satwire <mechanism> [--from-wired] --box=x0,y0,z0:x1,y1,z1 [--forbid-ring] [--palette ...]`
(crates/piston-door/src/satwire.rs). The mechanism and each piston's power per tick come from one
recorded run (an ideal schedule `.ideal`, or a wired design with `--from-wired`, wiring stripped);
the wiring is a one-hot placement per free cell of the box (block, dust, repeater 1-4 x E/W,
observer E/W/U/D, torch, wall torch, note block, waxed copper bulb, comparator) plus one lever
among candidates; two traces (lever on from rest, lever off from open) share the placement; the
redstone semantics are unrolled per game tick (exact dust levels, repeater/observer/torch/
comparator scheduling with the game's tick priorities, quasi-connectivity, supports, sticky-pull
cells, stable rest/open states, each trace ends where the other starts) and every piston must get
exactly its recorded power. Solver: CaDiCaL (rustsat). CEGAR: every model is judged by the real
simulator (4 positions, then --extra-offsets 28); a failing design is blocked; the wiring block
count is minimized with a totalizer. `--check` fixes the source design's own wiring and reports
where the model disagrees with the recorded piston power.

## Log
* e89b `--check`: the model reproduces e89b's piston power exactly (0 mismatches / 864
  piston-ticks).
* e89b re-synthesis (mechanism only, 8x9 box, `--forbid-ring`, offsets fixed): the FIRST SAT model
  (52 s) passes the real judge at 32 positions: 42 wiring blocks (62 placed), open 34 / close 34
  (`e89b-sat42.txt`). Minimizing next.
* flat-seam r046x3-plane-levels (11x5 strip door, box -4..6 x -1..3, `--forbid-ring`, palette
  block,dust,rep,obs,torch, offsets 0..10): UNSAT in 4 s = no static wiring in this model
  (agrees with strip's lemma).
