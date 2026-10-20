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
  - `best/`: the smallest clock per period, 177 periods P 21..1512 (README table; every one is
    archived itself or as a design of the same type).
  - five EXTENDABLE cards (0cb06af9; `extend/README.md`): replin P=4+9n, hairpin P=3+16n
    (+ plain), racetrack P=28+32n, racetrack-plain P=12+16n; instances in archive/extend.tsv.
  - the smallest per (period, family) of every non-extendable search: 577 (86013897..d99b80ae),
    57 rings (6da970f1), 237 census 3x1x2 k=6 cells (1e7c9eb4), 40 long-period climb finds
    (89253c23, 725167a8: P300..1512 in 6-7 blocks), and the 3 frequency-filter clocks (P50).
  - four more EXTENDABLE cards (42833245): replin-trapdoor, replin-dispenser, hairpin-trapdoor,
    hairpin-dispenser (the emitter as the variation).
  - `select --minimal --no-composite --known designs/clocks/archive/*.txt` over a search's
    output gives what is still missing; then `outputs --fix`, `caption`, `clock archive`.
* `composite/`: designs whose state repeats only as an lcm of independent parts (excluded, ruling
  16:10Z). `sim-only/`: circle rings (verifiable now; several archived from best/).

## Tools (crates/clock-sculk, `mctas clock sculk ...`)
* `obs` sensor + observer chains (+ `--conductor`, `--extra` one more block anywhere: sticky,
  piston, observer, stone, sculk, csculk, amethyst, wool: hybrids, two-sensor clocks)
* `chain` sensor + repeater lines + terminals (observer / torch / direct emitter)
* `ring` 3-sensor rings (`--tuples n:lo:hi`, `--kick note|behind`, `--wool N`); `ring-big` n
  sensors on a circle; `climb` long-period hunt (refuses composites); `reduce` greedy block
  removal keeping the period and a full-period cell
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
1. Longer non-composite periods: the climb (`climb ... --min-p 1000 --max-blocks 7|8`, then
   `reduce`) saturates near P1512 from the current seeds (8 blocks did not beat it); new seed
   families (filter clocks, rings with dividers) may break through.
2. Mechanisms still missing: a wool shutter clock (NOTES: the piston must be out of range or
   itself behind wool, and the emitter coupled to C).
3. More extendable families: frequency-filter chains, dividers with tunable dead times.
4. Caution: never pipe `caption` into `head` (SIGPIPE stops it after a few files); use
   `caption --archive designs/clocks/archive` to resync captions.tsv after re-captioning.
