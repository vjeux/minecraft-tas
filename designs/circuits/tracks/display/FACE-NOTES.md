# clk-obs8: the analog decoder after the face rule (2026-10-21 12:00Z)

The judge now wants the 7 lamps as the 3x5 figure-8 on one exterior face (compute::FACE,
compute::display_face). Every stacked design (analog-v1..v5) lights its lamp at a ring centre inside
the stack, so all fail. What was learned, and the plan.

## Carried over
- `mctas circuit display equalize <design> --below Y`: slows the faster inputs' repeaters so all 4
  bits reach the stack in the same tick (complement transitions then move s in one step). v5: 64 ->
  53 glitches on redsim (analog-v5-eq.txt). Use it on any analog design.
- `mctas circuit display glitches <design> [--verbose]`: every single-bit and complement transition,
  which outputs glitch, with waveforms.
- Glitch causes in the rings: a window comparator's dust side changes 2 gt before its offset
  comparator side (O adds one comparator), and bridges inside a ring delay parts of the s network
  by 2 gt. Hand-overs between windows dip; windows that s jumps over pulse.
- Fix designed (colstack.rs, `display column-stack`): 4 lossless columns, mirrored rings; each slot's
  O reads its column cell (time T), its W contact is fed by the slot's own bridge (T + 2 = O's output
  time). Feasible for code [8,2,4,1] neg 0001 (column distances [0,0,4,6]) but the chain (6x5) did
  not fit on the 7x7 top with the s network; now moot.
- A timing model (astack.rs glitch_score, `analog-stack --score`) predicts per-ring glitches within
  ~2 of redsim per segment; choosing ring layouts by it alone made the total worse (the judge counts
  holds where ANY output glitches).

## Why the stack cannot simply feed a face
Each ring ORs its windows AT the lamp cell (4 stones around it). A ring output cannot be extracted:
every neighbour of the centre (and of the block under it) leads radially into a W support, a bridge
or the ring below's centre (dust on the block under a centre also reads the lower ring's centre).

## Mechanics checked for a face design
- A lamp lights from a weakly powered neighbour (a stone with dust on top or pointed into), so dust
  OR nets can feed lamps; dust on top of a lamp lights it (never put dust on a lamp).
- Window comparators facing a lamp from the face plane have their front side in front of the face
  (forbidden): only one usable side, so end runs (top/bottom) only. A window behind the lamp (z = 1,
  facing it) or behind a stone S at z = 1 (z = 2, facing it) has both sides inside.
- Face-plane cells next to exactly one lamp (usable as extra stones P, each lit by a full window
  behind it): above a, left of f and e, right of b and c, below d. g has none.
- Capacity (analog.rs face_capacity test): with S behind each lamp (1 full + 2 half windows) and
  the P stones, NO code fits (g gets one full window). With a 3-window cell behind each lamp's S
  (S <- repeater <- K, K fed by 3 full windows) 304 codes fit.

## Candidate architectures
1. Three column stacks (a/g/d at odd levels, f/e and b/c at even levels), each ring one level
   above its lamp, its output taken under the front axis to the lamp (the front bridge must move).
   Reuses colstack; volume ~1000.
2. Face-centric cells: per lamp a stone S behind it, a repeater, a cell K with 3 full windows, P
   stones for a/f/b/e/c/d; s distributed over 6 levels by lossless comparator columns. Smaller
   (est. 7x7x9) but needs a 3D placer/router.
3. Digital thermometer crossbar: one s ruler, 15 taps (repeaters) give T_t = [s >= t]; a window is one
   subtract comparator (rear T_lo, side T_hi+1, output 15), OR nets are plain dust. Glitch-free by
   construction (every T switches in the same tick), each window is 1 block; needs bus routing.
