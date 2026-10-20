# WR bar — study 2, T flip-flops

What a world-record entry means here, per variation. "Blocks" is `mctas circuit verify`'s count; latency, min-pulse and min-period are the judge's own metrics. Judged in the game or it does not exist.

| variation | record target | ours today | gap |
|---|---|---|---|
| smallest, any speed | 3 blocks: the state block, its reader, the reader's support | tff-bulb 3b (waxed copper bulb + comparator on stone), latency 2 gt | AT THE BAR. A 2-block TFF would need a state block that is its own output; nothing in 26.3 does that |
| fastest | latency 2 gt, min-pulse 1 gt, min-period 2 gt | tff-bulb: exactly that | AT THE BAR: 2 gt is the comparator's own delay, and a 1 gt pulse is the shortest a redstone input can be |
| no copper (bulb-free) | 4-5 blocks | tff-observer-piston 4b (latency 11 gt, min-period 10 gt), tff-piston-slime 5b, tff-observer-flat 5b (falling edge) | 4b is likely the bar; the open question is a 4-block bulb-free TFF faster than 10 gt |
| flat, one block thick | 5 blocks | tff-observer-flat 5b, falling edge, latency 7 gt | OPEN: a flat RISING-edge TFF, and whether 4 blocks is possible flat |
| tileable at pitch 1 (a register row) | a 1-wide slice that repeats with no gap and no interference | none archived | OPEN, the main gap: builders tile TFFs into counters and registers, so a tileable slice is worth more than another one-off |
| item-based (no copper, no pistons) | 6-8 blocks | tff-snowdisp 6b (latency 6 gt), tff-dropring 8b, tff-crafter7 7b | at the bar; the crafter ones are slow (min-period over 30 gt) but are the only crafter-family entries |
| master-slave / windowed | 17-24 blocks | tff-masterslave 24b, tff-window 17b | OPEN: both look like first implementations, and a windowed TFF should not cost 17 blocks when the bulb does it in 3 |

## Priorities

1. A tileable TFF slice at pitch 1 (the registers builders actually make). Start from tff-bulb: bulb + comparator + stone is 1 wide; the question is whether consecutive slices interfere through the comparator's side inputs, and whether the carry into the next slice can be the comparator's own output.
2. Shrink tff-window (17b) and tff-masterslave (24b), or drop them if the bulb dominates them for every use: say on the page why a windowed TFF is kept.
3. A flat rising-edge TFF at 5 blocks or fewer.

## Settled facts (do not re-derive)

- A waxed copper bulb toggles LIT on every rising edge of its power, within the same update, and its lit state is readable by a comparator at 15/0. That is why 3 blocks is the floor: state block, comparator, support.
- A comparator's own delay is 2 gt, so no TFF whose output is read by a comparator can have latency below 2 gt.
- The same 3-block build is archived twice, as `tff-bulb` here and as `counter2-bulb` in study 5 (a TFF is a counter 2). Each page says so; do not count it as two designs.

## ONE design (vjeux 21:47Z, 2026-10-20)

The T flip-flop kind is tff-bulb alone: 3 blocks, 2 ticks, pulses every 4 ticks, tileable every 2. The eight other archived TFFs (observer + piston 4-5 blocks, snow/dispenser 6-7, dropper ring 8, crafter 7-8, the flat falling-edge observer 5) lost to it on size and latency and are removed. The falling-edge toggle is tff-bulb behind the 1-block NOT (a wall torch on the input), which its caption says. Flat is met: the judge's "flat" is height <= 2 (archive.rs variation_signature) and tff-bulb is 2 high; an earlier line here asked for a 1-high TFF, which was my own bar, not the judge's.

T flip-flop WITH RESET (tffr-dropring, 8 blocks, 13 gt) stays open. The judge pulses r and t separately (seq.rs), so a reset only needs to give a LIT copper bulb one extra rising edge. Every gate for "r AND q" priced so far is 9+ blocks (comparator reading the bulb + torch for NOT q + a dust at a comparator's side, or a repeater locked by NOT q), above the 8 already archived. Two droppers holding one stone (the rs-dropper) would toggle if t fired both at once, but two droppers ticking in the same gt run in order, so the stone bounces straight back on one of the two phases — measured reasoning in seq order, not a census.
