# Tracks tff + memory: handoff (session 90b04d7f "clk-item", box devvm50202, branch explore/tff)

Studies 2 (T flip-flops) and 5 (memory) of ~/agents/CONTRAPTIONS.md. Everything archived is in
designs/circuits/tff/archive/ and designs/circuits/memory/archive/, judged robust (4 positions x 4
rotations) by `mctas circuit archive`. Drafts: designs/circuits/{tff,memory}/tracks/{tff,memory}/.

## Tools (crates/circuit-tff, crates/circuit-memory; Rust, `mctas circuit tff|memory ...`)
- `circuit tff census --box AxBxC [--input x,y,z] --k a..b --palette ... [--auto-floor] --out DIR`:
  every irreducible filling of k cells (redsim), input = a redstone block in an air cell, kept when
  the TFF stimulus (4 pulses of 4 gt every 40, then two 12 gt pulses) toggles an output once per
  pulse; designs + summary.tsv per family. `--auto-floor` puts stone under dust, diodes and standing
  torches (counted) so a one-layer box (`--box 3x1x3`) searches components only. `--skeleton D
  --free box` fills around fixed blocks. Speed: ~10M candidates/s on 160 threads for redstone
  palettes (quiet frames skip the level reads), ~1-2M/s with pistons.
- `circuit memory census --kind rs-latch|d-latch|'counter N' --inputs 'r=..;s=..' ...`: same
  engine with the judge's latch/counter stimuli (specs.rs). `circuit memory check D` prints the
  census view of one design.
- `circuit memory ring --n N[,N..] [--max-dust k] [--bulbs K] --out DIR`: dropper-ring counters
  around the input cell (cycle search, hopper rules, relays, comparator placement, K bulb stages
  -> counter N*2^K). Instant.
- Palette shorthands: clock census names plus `dropper1`/`hopper1`/`dispenser1` (holding 1 stone)
  and `state{snbt}`.

## T flip-flops (axis = family)
| family | blocks | design |
|---|---|---|
| comparator+copper_bulb | 3 | tff-bulb |
| observer+redstone_block+sticky_piston | 4 | tff-observer-piston (census) |
| piston+redstone_block+slime_block+sticky_piston | 5 | tff-piston-slime (census) |
| comparator+dispenser+powder_snow | 6 | tff-snowdisp (+ tff-snowdisp-dust 7) |
| comparator+dropper+hopper+redstone_wire | 8 | tff-dropring |
| comparator+crafter+hopper+redstone_wire | 8 | tff-crafter |

Learned: two droppers fired together never toggle (the later one passes the item straight back):
every transit must cross a hopper. Dispensers: an empty bucket vs a powder snow bucket read 1 vs 2
on a comparator (stack sizes 16 vs 1); powder snow never flows. 30 more 5-block piston TFFs toggle
on 4 gt pulses but not on 10 gt ones (the judge now fails those; the census demands 12 gt pulses).
No redstone-only (stone/dust/torch/repeater/comparator) TFF of <= 6 blocks in 3x2x2 (3.2e9
candidates per input); the auto-floor census over 3x1x3 is the next step for that family.

## Memory
RS: rs-dropper 4 (two droppers + stone + comparator), rs-crafter 6. D: d-latch-repeater 4 (enable
low), d-latch-high 5 (subtract comparator with a redstone block behind it as the lock). Counters
(level style unless noted): 2 bulb 3 / bulb2 6 (toggles) / ring 8; 3 corner 8 / ring 8; 4 bulb2 6 /
ring9 9 / ring 10; 5 ring11 11 / star 12; 6 corner-bulb 11; 7 ring7 18; 8 bulb3 9; 9 ring9 25; 10
ring5-bulb 14; 11 ring11 29; 12 ring3-bulb2 14; 13 ring13 36; 14 ring7-bulb1 21; 16 bulb4 12; 18
ring9-bulb1 28; 20 ring5-bulb2 17; 24 ring3-bulb3 17.

Learned: the judge's level style takes any output whose end-of-window level has period N, so a
TFF is a counter 2 and n bulbs make a counter 2^n. Ring rules: droppers of one ring share a grid
parity; hoppers never face up and never sit under a container (they pull); a dust next to the
input connects to it (a line) and fires the cell beyond and its floor; a dust under the input is
a cross; a dust pointing into a stone makes it a relay that fires its own neighbours (rings to 17).

## Next
- Redstone-only TFF (auto-floor census k=6 over 3x1x3, or a master-slave repeater-lock build:
  M locked by in, S locked by NOT in = subtract comparator with a redstone block, NOT q = another
  subtract comparator).
- Ring -> ring chains (stage 2 around a stone the stage-1 comparator powers): counter 9 = 3x3,
  15 = 3x5, 25 = 5x5 well under the single-ring sizes.
- Third RS / D latch mechanism (piston or torch), extendable cards once `extend:` lands for circuits.
