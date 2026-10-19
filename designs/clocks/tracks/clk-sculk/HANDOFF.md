# clk-sculk HANDOFF

Track: vibration clocks (sculk sensors, calibrated sensors, amethyst, every vibration source).
Session 0d1a0fb5-da90-484f-9eb7-c40ac1bc4833. Box devvm55868 (granted by the coordinator after
devvm52946 died at 18:10Z), clone ~/agents/clk-sculk/minecraft-tas, branch explore/clk-sculk.
Notebook: NOTES.md (floor proof, timing rule, mechanisms, output rule). Board: Space memory
`minecraft-tas/clocks/families/sculk.md` and the roster line.

## State
* Floors proven from the source and reached: P >= 21 with a calibrated sensor (5 blocks:
  c21-obs3-disp), P >= 41 plain-only (5 blocks: p41-obs3-disp). Every P >= 21 is reachable
  (a hairpin whose repeater delays sum to D: P = 2D + 3, or 2D + 4 with the note one further).
* Archived by me (`mctas clock archive`, merged into main by the coordinator):
  - `best/`: the smallest clock per period, 94 periods P 21..468 (7f87eafb; README table).
  - five EXTENDABLE cards (0cb06af9; `extend/README.md`): replin P=4+9n, hairpin P=3+16n
    (+ plain), racetrack P=28+32n, racetrack-plain P=12+16n; instances in archive/extend.tsv.
  - non-extendable smallest per (period, family): 34 so far (86013897, the commit before
    aef98d42); the rest of the 619-design selection was archiving when this was written
    (`select --minimal --no-composite` over obs, hyb, hyb3, hyb3b, obsd, two, census: no
    repeater-line or ring generators, those are the cards' territory).
* `composite/`: designs whose state repeats only as an lcm of independent parts (excluded, ruling
  16:10Z). `sim-only/`: circle rings (verifiable now; several archived from best/).

## Tools (crates/clock-sculk, `mctas clock sculk ...`)
* `obs` sensor + observer chains (+ `--conductor`, `--extra` one more block anywhere: sticky,
  piston, observer, stone, sculk, csculk, amethyst, wool: hybrids, two-sensor clocks)
* `chain` sensor + repeater lines + terminals (observer / torch / direct emitter)
* `ring` 3-sensor rings (`--tuples n:lo:hi`, `--kick note|behind`, `--wool N`); `ring-big` n
  sensors on a circle; `climb` long-period hunt
* `family <name> --n <n|a..b> (--out F | --out-dir D) [--card]` the extendable families;
  `family-dupes [--remove]` archived designs that are instances of a family
* `select` smallest per period or per (period, family), `--mechanisms N`, `--minimal`,
  `--no-composite`, `--known`; `outputs [--fix]` (period = the output's period); `caption`
  one-line captions; `table [--md]`
Census (clockinfra's `mctas clock census`) palette for sculk:
csculk,sculk,observer,note,trapdoor,dispenser,stone,sticky,piston (boxes 3x1x2 k5, 4x1x2 k5,
2x2x2/3x2x2/3x3x1 k4: 4-block P23/42/43/90/140, 5-block P66..296).

## Regenerate the corpus (deterministic, ~15 min on 166 cores; one heavy job at a time)
obs --max-k 4 --conductor; chain --max-reps 5; obs --max-k 2 --conductor --emitters
note,trapdoor,sticky --extra sticky,piston,observer,stone; obs/chain --emitters dispenser;
ring --tuples 3:4:16 (--kick note --wool 2 / --kick behind --wool 3); obs --max-k 3 --extra ...
(hyb3: note,trapdoor,sticky; hyb3b: piston,gate,dispenser,irontrapdoor); two: obs --max-k 3
--emitters dispenser,note,trapdoor,sticky --extra sculk,csculk; the census boxes above.

## Next steps
1. Finish archiving the non-extendable selection (rerun `clock archive` on it: ALREADY ones are
   skipped); ping the coordinator with the commit.
2. Long non-composite periods beyond 468 with few blocks (two-sensor P>=360 found so far were all
   composite): census k=6 in 3x1x2, `climb` from the census seeds.
3. Mechanisms still missing: a frequency filter that matters (a back signal of 9 vs the
   emitter's 10: a plain sensor 3.2..3.7 from the emitter behind a calibrated one), a wool
   shutter moved by a piston.
