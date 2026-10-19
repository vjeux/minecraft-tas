# c77 handoff (2026-10-19 19:30Z, final)

## Results (each verified with `door verify --extra-offsets 28`, rotation-safe, FULL seamless)
- **e78st-56b-38-1s.txt** (= BEST.txt): 7x8 (56b), 38 blocks, sculk with ONE calibrated sensor, stone door, open 48 / close 15.
  pockets3's copy without the obsidian (-2,-1) = archive/e78st-56b-37-37b (37, the sculk 7x8 record).
- e78st-56b-41-2s (2 sensors); e78st-56b-40 / -42 / -44 (3 sensors, 51/15; -44 archived as archive/e78st-56b-44-44b).
- **community-56b-47-elig.txt**: the community 7x8 (c70b's transcription + my (3,3) repeater d4) in stone, 47 blocks,
  solid, RECORD-ELIGIBLE (`door eligible`), 42/38. community/community-7x8-rep4.txt = the 48 in the original materials.
- c074-64b-42.txt: 8x8 (64b), 42 blocks, sculk (archive/c77-c074-64b-42-42b).

## Open leads
- community 46 (community/c46-notelig.txt): works and FULL but fails repeater input. e78k's diagnosis (19:35Z): the
  repeater rig delays the support chain by 2 gt (dust (3,5) -> comparator (4,5) -> obs (4,4) -> dropper (4,3) -> rep
  (3,3) -> obs (3,4) -> dust -> support) but the piston chain by only 1 gt (obs (0,5) -> rep (-1,5) -> dropper (-2,4)
  by QC -> obs (-1,4) -> rep (0,4) into S). The lever run's 1-gt margin (right pulse t14, rep (0,4) t15) becomes a tie
  at t16, the repeater ticks first and S blocks (1,5); c47 has a 3-gt margin. No c46 neighbourhood passes both runs
  (e78k: top-left 6 cells, right 7 cells, timing cells of both chains); e78k stopped on this door.
  Rig emulation for ideal-rs --vary (e78k, explore/q78 tracks/q78/e78s/NOTES.md): lever patched off, repeater
  [delay=1,facing=north,powered=true] at (2,5,-1) fed by a redstone block at (2,5,-2) on smooth stone (2,4,-1);
  `open 0 clear 2 5 -2` / `close 0 set 2 5 -2 minecraft:redstone_block`; intersect with the lever run's survivors.
- The stone line's last sensor S2: not replaceable locally (k<=5 over 10 cells, pockets3). The team's solid 7x8 is e78k's h5
  (41, FULL) / h6 (42, 29/15) with a different bottom (BL-wiggle lamp relight).
- Spare row/column ports of h5 (7x9, 8x8): nothing at k<=4; needs a bottom restructure.

## Running processes
None.

## Method notes
- `door fill --keep` re-tunes a broken conversion (wool -> stone door) and replaces one sensor at a time (pre-clear the cell).
- ideal-rs plans are unreliable on the e78 bottom (same-tick event order); screen real wirings.
- door fill's FULL/frame check used to reject door-material frames (white concrete); fixed by pockets3 (e724cae1).
