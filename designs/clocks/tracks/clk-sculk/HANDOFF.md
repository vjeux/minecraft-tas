# clk-sculk HANDOFF

Track: vibration clocks (sculk sensors, calibrated sensors, amethyst, every vibration source).
Session 0d1a0fb5-da90-484f-9eb7-c40ac1bc4833. Box devvm55868 (granted by the coordinator after
devvm52946 died at 18:10Z), clone ~/agents/clk-sculk/minecraft-tas, branch explore/clk-sculk.
Notebook: NOTES.md (floor proof, timing rule, mechanisms, output rule). Board: Space memory
`minecraft-tas/clocks/families/sculk.md` and the roster line.

## State
* Floors proven from the source and reached: P >= 21 with a calibrated sensor (5 blocks:
  c21-obs3-disp), P >= 41 plain-only (5 blocks: p41-obs3-disp). Every P >= 21 is reachable.
* `best/`: the smallest verified clock per period (94 periods, P 21..468), archived by me with
  `mctas clock archive` (commit 7f87eafb; 3 were already archived). `best/README.md` = the table.
* `composite/`: designs whose state repeats only as an lcm of independent parts (excluded, ruling
  16:10Z). `sim-only/`: big rings (now verifiable since clockinfra's big-box fix).
* In flight when written: the per-(period, family) smallest set (~1500 cells: every generator +
  census), selected with `select --minimal --no-composite`, outputs fixed, captioned, then
  `clock archive` (JVM workers <= half the cores, one heavy job at a time).

## Tools (crates/clock-sculk, `mctas clock sculk ...`)
* `obs` sensor + observer chains (+ `--conductor`, `--extra` one more block anywhere: the hybrids)
* `chain` sensor + repeater lines + terminals (observer / torch / direct emitter)
* `ring` 3-sensor rings (`--tuples n:lo:hi`, `--kick note|behind`, `--wool N`); `ring-big` n
  sensors on a circle (long periods), `--sensor plain`
* `climb` long-period hunt (random walkers mutating seed clocks)
* `select` smallest per period or per (period, family), `--mechanisms N`, `--minimal`,
  `--no-composite`, `--known` to skip what exists
* `outputs [--fix]` the "period = output's period" rule; `caption` one-line captions for the
  archive; `table [--md]` sim summaries
Every generator simulates with `clock::sim` on all cores (`--threads`). The census
(`mctas clock census`, clockinfra's) with the palette
csculk,sculk,observer,note,trapdoor,dispenser,stone,sticky,piston found the 4-block P23/42/43/90/140.

## Regenerate the corpus (deterministic, ~15 min on 166 cores)
obs --max-k 4 --conductor; chain --max-reps 5; obs --max-k 2 --conductor --emitters
note,trapdoor,sticky --extra sticky,piston,observer,stone; obs/chain with --emitters dispenser;
ring --tuples 3:4:16 (--kick note --wool 2 / --kick behind --wool 3); obs --max-k 3 --extra ...
(hyb3, hyb3b); census boxes 3x1x2 k5, 4x1x2 k4/k5, 2x2x2, 3x2x2, 3x3x1 k4. Each with --out-dir.

## Next steps
1. Finish archiving the per-(period, family) set; ping the coordinator with the commit.
2. Long periods beyond 468 with few blocks: census k=6 in 3x1x2, `climb` from the census seeds,
   3-sensor interlocks.
3. Mechanisms still missing: a frequency filter that matters (back signal 9 vs the emitter's 10),
   a wool shutter moved by a piston.
