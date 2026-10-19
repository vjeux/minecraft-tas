# clk-sculk HANDOFF

Track: vibration clocks (sculk sensors, calibrated sensors, amethyst, every vibration source).
Session 0d1a0fb5-da90-484f-9eb7-c40ac1bc4833, box devvm52946, clone ~/agents/clk-sculk/minecraft-tas,
branch explore/clk-sculk. Notebook: NOTES.md (floor proof, timing rule, mechanisms). Board:
Space memory `minecraft-tas/clocks/families/sculk.md`.

## State
* Floors proven from the source and reached: P >= 21 with a calibrated sensor (5 blocks:
  c21-obs3-disp), P >= 41 plain-only (5 blocks: p41-obs3-disp). Every P >= 21 is reachable.
* `best/`: the smallest verified clock per period (P = 21..468), all `clock verify` robust;
  `best/README.md` is the table (regenerate: `mctas clock sculk table --md best/*.txt`).
* Hand-made clocks at the top of this folder (all verified and archived on main by the coordinator).

## Tools (crates/clock-sculk, `mctas clock sculk ...`)
* `obs` sensor + observer chains (+ `--conductor`, `--extra` one more block anywhere: the hybrids)
* `chain` sensor + repeater lines + terminals (observer / torch / direct emitter)
* `ring` 3-sensor rings (`--tuples n:lo:hi`, `--kick note|behind`, `--wool N`)
* `ring-big` n sensors on a circle (long periods), `--sensor plain`
* `select` smallest per period or per (period, family), `--known` to skip what exists
* `table [--md]` sim summaries
Every generator simulates with `clock::sim` on all cores (`--threads`) and writes the best few per
(period, family) with `--out-dir`.

## Next steps
1. `clock census` with a sculk palette on small boxes: prove (or beat) the 5-block P=21/41 and the
   3-4 block entries.
2. Fill the 9-14 block gaps (P 59-83) with richer hybrids (`obs --max-k 3 --extra ...` with more
   emitters, two extra blocks).
3. Mechanism variety still missing: calibrated frequency filter (back input), wool shutter moved by
   a piston, bells and doors as emitters.
4. Rules question pending with the coordinator: independent sub-clocks multiply periods (lcm).
