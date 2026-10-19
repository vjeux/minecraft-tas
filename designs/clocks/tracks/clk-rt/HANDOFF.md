# clk-rt HANDOFF

Track: redstone-tick clocks (torches, repeaters, comparators, dust). Session
7c07d5cb-1b56-4d72-aecb-946261d96898, box devvm54942 (dedicated), branch explore/clk-rt, clone
~/agents/clk-rt/minecraft-tas. Notes, theory and the result tables: [NOTES.md](NOTES.md).

## State
- Module crates/clock-rt (`mctas clock rt ...`): U-loop generator + sweeps, odd-period kick search,
  burnout sweep, divider chains, shrink (local search). See NOTES.md's command table.
- Verified clocks (all `clock verify` robust): see NOTES.md "Results"; every clock reported to the
  coordinator (f1a42990) in one-line reports.
- Theory: parity (odd periods need a kick at an odd tick), P 1-3 impossible, torch families cannot
  reach P 4-8 (burnout), inverter loops cannot be odd: designs/clocks/impossible.tsv.

## Next steps
1. Smaller dividers (18 blocks per stage now): a stage whose two lock repeaters read the previous
   torch/block directly AND whose latches sit in a U (closure 5 blocks instead of 10).
2. Odd periods below 18 blocks (P5) and more odd periods (P7, P9 ... at 12-18 blocks).
3. Comparator fading clocks (strength decays one per lap, a torch refills), comparator inverter
   loops, 3-edge harmonic modes as waveform variants.
4. P = 6 below 14 blocks; P = 2 mod 4 between 22 and 34 below 12.

## Running processes
None after this session ends (sweeps are foreground or finish within the session).
