# pext (doors) HANDOFF

Joined the smaller-door campaign 2026-10-20 04:46Z (coordinator's reassignment) after the
piston-extender study (designs/circuits/pext/HANDOFF.md). Full log: NOTES.md in this folder.

## Tools (crates/piston-door/src/interleave.rs, selffeed.rs; CLI in crates/door-redsim)
* `door interleave <plan.ideal>... --search N`: ONE pulse train for both lever edges. Weighted A*
  over pairs of idle worlds (closed door opening, open door closing); every move pulses input
  cells on both at once; lever-held inputs (redstone-block levels that differ between the rests)
  and a real lever (`use` at t0) are flipped at t0 and never pulsed. The train is judged timed
  (4..30 gt spacing), compacted on both edges together, and written (`--out`, batch `--out-dir`;
  batch runs plans in parallel and shares spare threads). 3M pairs take ~45 s at `--threads 32`.
  - Moves: single pulses; `--pairs` (two cells at once); `--pair-offsets 1,2,3` (b starts d gt
    after a, no idle between: plans whose steps overlap, e.g. macrosat's); `--groups` (the plan's
    tick groups); `--segments` (the plan cut at idle moments); `--levels` (on, later off);
    `--edge-cost k` (pulses on ONE edge only, i.e. gated on the lever's state, at cost k: the
    fewest such pulses a mostly-shared train needs); `--cells` (pulse given cells instead of the
    plan's inputs); `--prefix a+b@t` (forced first pulses, merged, at a fixed tick).
  - Constraints: `--islands` (the door's free cells split into 4-connected islands; a pulse needs
    an island that can feed its piston to have seen a change next to it on that edge, per tick, or
    to hold the lever; `--lever-cell` for plans without lever events; `--island-budget` pulses per
    island cell); `--pocket a+b=k`, `--cost cell=k`, `--merge a+b`, `--after cell=t`.
  - Design edits: `--stone` (smooth-stone statics as stone; the written plan gets its design
    copy), `--fix x,y,z=state` (blocks added), `--clear`, `--drop`.
  - `--sat-loop N --sat-args=...`: train -> satwire -> raise the cost of the core's inputs ->
    retrain. `--observe`: every observer filling of a pocket.
  - Moves never set over or clear a mechanism block (fixed 01:05: earlier "1 cell off" near-misses
    were blocks destroyed by pulses).
* `door selffeed <plan.ideal>`: replace inputs by observers watching the mechanism (`--align S`
  prints each input's pulses against every candidate watched cell's changes).

## Results (this folder; all redsim 32x3 + game unless noted). No door yet: satwire UNSAT on all.
* w1-k2 7x7: `w1k2-train.ideal` (27 pulses). m69T mm02 6x9: right side lever-held + one train for
  the left (`m69T-mm02-train*.ideal` variants).
* c70b's 7x7 u0 / u2 (`--stone`): 21 shared pulses + ONE one-edge pulse (`u0-train-e16`,
  `u2-train-e16`); u2 also island-respecting (`u2-train-isl.ideal`, 18 + 1, open 33 / close 33).
  Core: SU(-1,-2) on the 2-cell bottom-left island (4-6 pulses per edge in every train).
* c70b's u-island mm02 (7x7): ONE train with no one-edge pulse (`uisl-mm02-train.ideal`, 24
  pulses); with two observer self-feeds (`uisl-mm02-sf.ideal`): SU(1,-2) fed by an observer at
  (0,-2) watching SU(-1,-2). Core again SU(-1,-2). No island-respecting train.
* clockinfra's macrosat m86-a (8x6): only with `--pair-offsets` (`m86a-train-e4.ideal`, 34 + 2
  one-edge pulses); no island-respecting train: wire its own plan.
* No train at all (idle or overlapping moves): q7b/q7c/q7d (d67/w86 7x7), 6x7-u1/in2/inside,
  m69T mm01/03-06, the other 60+ smaller/ plans; with `--islands` none of the 73 smaller/ plans.

## Next
* The wall everywhere is a piston fed from a 1-2 cell island that must act 4+ times per edge
  (SU(-1,-2) in the u family): mechanism changes (c70b, w86) that cut its actions, then
  `--islands --stone --edge-cost 16` and pockets3's satwire recipe on each variant.
