# satwire HANDOFF (satwire3, 2026-10-19 ~14:50Z; devvm55676)

## State
- Branch explore/satwire (tip: see `git log -1`), main merged at de6ecbce. Tool commits this shift
  (all merged by the coordinator up to --hold; the rest pinged): 12659892 batch BROKEN skip,
  08e43e39 `--hold`, 4ee290d2 update-piston relays, 166a0359 tile phase default, 69b8fd46
  `--free-order`, f6087c25 sculk sensors + wool, then the comparator-through-block + kept-support fix.
- `door satwire` now covers: per-piston pulse/hold choice (`--hold auto`), sculk sensors (palette
  `sculk`, `csculk`, `csculk0`) and wool occlusion, update-piston relays, tile-phase plans,
  free event order (`--free-order`, with `--retime`). Known answers: see NOTES.md third shift.
- No 7x8 / 7x9 / 8x8 door found by SAT this shift. Every mechspace 7x8 plan family came back UNSAT
  with a solid palette (s78 450/2016, a78 --hold 298, hft78 held-first --hold --free-order 313 of
  375 done): the cores are mostly TWO pistons = a corner piston and the side puller diagonal to it
  (or the bottom puller), sharing their only feed cells with different timings (told mechspace: it
  now exports corner-ok lists, tracks/mechspace/corner-ok-*.txt). t78's 24 hs23 plans with
  csculk + wool: all UNSAT (hs23-0008 core: open (-2,1) + close (-2,2) + close (-1,3): the corner
  (-1,3) cannot be fed without QC-touching the held stack; sensors near it wake too early on the
  lever / stack vibrations).
- c78l's L3b (L3a kept, TP + L1/L2 to wire): UNSAT, core close TP (-1,3) + close s (1,4).

## Running (devvm55676, /tmp logs)
- /tmp/sw3-hft78/ (nohup batch, mctas-sw3d): hft78 held-first tile plans, --hold auto --free-order.
- /tmp/sw3-c78L0.log: c78l's c78L0 (top-left free: s, M, TP, L1/L2) with csculk/sculk/wool.
- satwire2's leftovers: /tmp/e88-y*.log (t7b 42b hunt: obsolete, 63b record is 40 now: kill them),
  /tmp/s78-e88-n6.log (e88 in 7x8, cluster warps 6), /tmp/m79s8-c6.log (sensor-free 8x8).
  Any `JUDGE OK` past iter 0 = a door to `door verify`.

## Next
1. Run mechspace's corner-ok lists (corner-ok-hft78.txt etc., tile) with `--hold auto` and the
   sculk palette (`csculk,wool`), lever ON; then 7x9 / 8x8 (n79/n88, extra bottom row).
2. Sculk model gaps: a calibrated sensor's frequency filter at levels 9/10 (dust at its back),
   strong power of a sensor into a conductor below at its level, comparator SUBTRACT with a side
   input (not in a flat door).
3. Partials on request (the board): c78l (c78L0/L3), t78 (hs23-0008, r70), c70 (c070x BL).
