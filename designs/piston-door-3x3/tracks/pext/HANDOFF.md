# pext (doors) HANDOFF

Joined the smaller-door campaign 2026-10-20 04:46Z (coordinator's reassignment) after the
piston-extender study (designs/circuits/pext/HANDOFF.md).

## Tools (crates/piston-door/src/interleave.rs, selffeed.rs; CLI in crates/door-redsim)
* `door interleave <plan.ideal>... --search N`: ONE pulse train for both lever edges. Weighted A*
  over pairs of idle worlds (closed door opening, open door closing); every move pulses input
  cells on both at once; lever-held inputs (redstone-block levels that differ between the rests)
  are set by the lever at t0 and never pulsed. The train is judged timed (4..30 gt spacing), then
  compacted on both edges together, and written (`--out`, batch `--out-dir`). Biases: `--cost
  cell=k`, `--merge a+b` (cells always fired together: one source), `--pocket a+b+c=k` (a pocket's
  inputs fire at most k times in the whole train), `--groups`, `--pairs`, `--weight`.
* `door selffeed <plan.ideal>`: replace inputs by observers watching the mechanism (`--align S`
  prints each input's pulses against every candidate watched cell's changes). Dead end on the
  current candidates (only lever-time inputs replaceable).

## Results (designs/piston-door-3x3/tracks/pext/)
* w1-k2 7x7 (c70b): `w1k2-train.ideal`, 27 pulses, open 37 / close 34 (redsim 32x3 + game).
  Wiring blocked by P / Q / BM-down having no free feed cell (mechanism problem, c70b).
* m69T mm02 6x9 (c70b): right side lever-held + ONE train for the left half: removes w86's
  "halves can't share one lever". Variants for pockets3's satwire: `m69T-mm02-train.ideal` (26
  pulses), `-c3` / `-c6` ((0,4) fired twice / once), `-tl5` (top-left pocket <= 5 moves, 21 pulses,
  open 26 / close 18), `-bl3` (bottom-left {(-1,-2),(-1,-1)} <= 3 ticks), `-blm` (those two merged).
  satwire cores so far: (0,4) close, then the bottom-left (0,-2)/(0,-1).
* Batch over c70b's 53 smaller/ plans: only those two have a train within 300k pairs.

## Next
* Keep producing trains biased to pockets3's satwire cores (pockets3 names the tight pocket and
  the ticks it can make; `--pocket` / `--merge` / `--cost`).
* A train search with timing (moves at ticks, not idle steps) could reach plans the step model
  cannot (6x7-u1 plateaus one block off).
