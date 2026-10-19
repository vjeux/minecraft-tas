# satwire HANDOFF (satwire3, 2026-10-19 15:01Z; devvm55676) — STOPPED (slot reassigned)

## State
- Branch explore/satwire, pushed. Tool commits this shift: 12659892 batch BROKEN skip, 08e43e39
  `--hold`, 4ee290d2 update-piston relays, 166a0359 tile phase default, 69b8fd46 `--free-order`,
  f6087c25 sculk sensors + wool, c4513044 comparator-through-block + kept-cell supports. The
  coordinator merged up to the sculk model (main 7ffd3378, d7be8736); c4513044 was pinged, maybe not
  merged yet.
- `door satwire` covers: per-piston pulse/hold choice (`--hold auto`), sculk sensors (palette
  `sculk`, `csculk`, `csculk0`) + wool occlusion, update-piston relays, tile-phase plans, free event
  order (`--free-order`, with `--retime`), comparators reading through a block. Known answers in
  NOTES.md (third shift): m79s-sculk-63b-50 and m79s-sculk-8x8 0 piston mismatches; the m79s sculk
  door is re-synthesized with `--hint` (JUDGE OK at 32 positions, 50 blocks).
- NO 7x8 / 7x9 / 8x8 door found by SAT this shift. Results:
  - mechspace 7x8 families, solid palette: s78 450 UNSAT, a78 `--hold` 298 UNSAT, hft78 held-first
    `--hold --free-order` 603 UNSAT + 69 BROKEN of 740. Cores mostly TWO pistons: a corner piston and
    its diagonal side puller (or the bottom puller) sharing their only feed cells; mechspace turned
    that into corner-ok lists (tracks/mechspace/corner-ok-*.txt).
  - t78's 24 hs23 plans with csculk + wool: 23 UNSAT (hs23-0008 core: open (-2,1) + close (-2,2) +
    close (-1,3)); a few with one-piston cores (a piston at the box edge only fed from outside).
  - c78l: L3b UNSAT (core close TP (-1,3) + close s (1,4)); L3d UNSAT (core open L1 (-2,1): its only
    feed is (-2,3)); c78L0 (top-left free) UNSAT, core = 7 close pistons.
  - e78k: r1w (B' ideal, cells (0,-3) (-1,-1) freed + empty cells) UNSAT at once, core open B' (1,-3).
    Model gap: r1's own wiring is REJECTED by --check because its calibrated sensors (-1,-3) (facing
    west) and (0,-3) (facing east) sit back to back: v1 forbids a sensor or dust at a calibrated
    sensor's back (frequency level not modeled). Modeling the back filter level (sensor output =
    distance level, dust level; accept frequency == level) would let e78 be searched.

## Cut batches (all stopped 15:00Z; nothing of value lost)
- /tmp/sw3-cok78/: mechspace corner-ok-hft78 (242) with csculk+wool, --hold auto --free-order,
  30 of 242 done (14 no door). Resume from ~/agents/satwire/ms-wt (origin/explore/mechspace):
  `sed 's|^|designs/piston-door-3x3/tracks/mechspace/mech/|; s|$|-oc.ideal|' designs/piston-door-3x3/tracks/mechspace/corner-ok-hft78.txt | xargs mctas door satwire --jobs 16 --out /tmp/sw3-cok78 --box=-2,-2,0:4,5,0 --full-frame --forbid-ring --lever-oop --lever-on --hold auto --free-order --dmax 8 --skip 8 --sound --update-order --exact-order --retime --palette block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,glass,leaves,upiston,csculk,wool --first --iters 3000 --core-tries 3`
  (batch mode skips plans whose log already has a RESULT? no: point --out at a fresh dir or drop
  the done ids). Same for corner-ok-hf78 (between-ticks: add `--between-ticks`), -hs23 (box
  -2,-3:4,4), -hfh78.
- /tmp/sw3-m79s-scratch.log: m79s-sculk-63b from scratch (no --hint) with csculk+wool: a test of
  sculk synthesis without a hint (would also search below its 50 blocks).
- /tmp/sw3-hs23s: 1 of the 24 hs23 plans was still solving.
- satwire2's leftovers stopped: s78-e88-n6 (e88 in 7x8, cluster warps 6), m79s8-c6 (sensor-free
  8x8). The e88-t7b 42-block hunts were obsolete (the 63b record is 40 blocks).

## Next
1. The calibrated back filter at a level (see e78 above), then e78k's r1w / nb.
2. Corner-ok lists with the sculk palette (resume above); then 7x9 / 8x8 (n79/n88).
3. Other model gaps: strong power of a sensor into a conductor below at its level; comparator
   subtract with a side input (not in flat doors).
