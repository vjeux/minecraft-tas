# Fuel factories (study 23, track `factory`): stopped 2026-10-21

The coordinator's brief: a self-fueling kelp fuel factory (kelp harvested, smelted into dried kelp
in a furnace fuelled by its own dried kelp blocks, crafted into blocks, part fed back, the surplus
to a chest), judged end to end over a long run. Stopped by the coordinator at 21:58 (vjeux: no more
work on unattended circuits); nothing archived.

## What exists
- The judge, `kelp-factory N` (`crates/circuit/src/factory.rs`): plants kelp at every `port crop:`,
  puts ONE dried kelp block in the chest on `port fuel:`, runs 6 h at random tick speed 3 and empties
  `port output:` every 1000 gt. It needs: the starting block taken, only dried kelp blocks out, more
  out than the 1 borrowed, a block in the last third of the run (it never starves), nothing on the
  floor; intrusion drops the sorter's strays on every open hopper top. Game (`mctas circuit verify`,
  `factory-run`) and redsim (`factory-run --sim`, kelp growth mirrored: the same blocks at the same
  ticks as the game on kf2). redsim gained the item `dried_kelp_block` (burn time 4001 gt).
- [kf4.txt](kf4.txt): 101 blocks, 4 kelp-17b plants 3 apart on one hopper chain into the furnace;
  game verify robust (16 builds), no open hopper top; 8 blocks out for the 1 borrowed in 6 h
  (1.17 net blocks/h, 0.0116 blocks/h per block placed).

## How kf4 works
- The furnace is batch-fired: its comparator reads 3 once 28 kelp wait (each block smelts 20), and 3
  dust (the third on a stone beside the fuel dropper, the climb breaks the dust's even-length
  parity) fire the fuel dropper once per rising edge: 1 block per 20 kelp.
- Dried kelp leaves through the hopper under the furnace into two droppers facing up (a lift) into
  the crafter; an observer pair (`port start: ... last`, the second observer placed last) powers the
  top dropper, which powers the bottom one and the crafter: the crafter crafts on the first pulse
  after all 9 slots hold dried kelp.
- The crafter ejects into a hopper facing the output chest; the hopper under it pulls one block into
  the fuel dropper whenever that is empty (a comparator on the dropper, a stone, a dust and a
  repeater lock it while it holds a block). The start chest sits on that hopper.

## Next ideas (not built)
- Four plants around ONE refill water source (a "+": the source refills all four second blocks, one
  hopper under it, the four plant hoppers into one centre hopper): 44 blocks for 4 plants instead
  of 76, about 70 blocks for the factory.
- More plants share the processing (~26 blocks): the metric rises toward a plant's own rate.
