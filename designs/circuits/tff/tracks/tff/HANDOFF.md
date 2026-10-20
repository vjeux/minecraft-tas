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
- `circuit memory ring --n N[,N..] [--max-dust k] [--bulbs K] [--then a,b,..] [--divider] --out DIR`:
  dropper-ring counters around the input cell (cycle search over the cells the input fires
  directly, through dust, dust relays or repeater relays; hopper rules; comparator placement),
  K bulb stages (counter N*2^K), more rings around stones the previous comparators power
  (counter N*a*b..), or a ring + bulbs judged as a toggles-style divider. Parallel over layouts;
  instant up to N=17, minutes for 19-25 (`--max-dust 3-4`).
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
| comparator+crafter+hopper | 7 | tff-crafter7 (constant 1 from a dropper instead of two dust) |
| comparator+redstone_torch+redstone_wire+repeater | 17 | tff-window (redstone only) |
| redstone_torch+redstone_wire+repeater | 24 | tff-masterslave (redstone only) |
| observer+redstone_block+sticky_piston | 5 | tff-observer-flat (second design) |
| observer+piston+redstone_block+sticky_piston | 5 | tff-observer-pistonline |

Learned: two droppers fired together never toggle (the later one passes the item straight back):
every transit must cross a hopper. Dispensers: an empty bucket vs a powder snow bucket read 1 vs 2
on a comparator (stack sizes 16 vs 1); powder snow never flows. 30 more 5-block piston TFFs toggle
on 4 gt pulses but not on 10 gt ones (the judge now fails those; the census demands 12 gt pulses).
No redstone-only (stone/dust/torch/repeater/comparator) TFF of <= 6 blocks in 3x2x2 (3.2e9
candidates per input) nor <= 5 components with auto floors in 3x1x3. The redstone family came from
a hand-built repeater-lock master-slave (24) whose lock wiring a skeleton census refilled; that
showed the slave was only a delay, and the single-latch pulse-window design followed (18, then 17
with the NOT-q torch standing right behind the latch); a
skeleton census over its lock (k <= 5 components, 23 free cells) finds nothing smaller.
Skeleton files: tracks/tff/scratch/ms-skeleton.txt, win-skeleton.txt (`--skeleton --free=...`).

## Memory
RS: rs-dropper 4 (two droppers + stone + comparator), rs-crafter 6, rs-loop 12 (redstone only:
repeater + comparator + four dust ring, r at the comparator's side). D: d-latch-repeater 4 (enable
low), d-latch-high 5 (subtract comparator with a redstone block behind it as the lock). Counters
(level style unless noted): 2 bulb 3 / bulb2 6 (toggles) / ring 8; 3 corner 8 / ring 8; 4 bulb2 6 /
ring9 9 / ring 10; 5 ring11 11 / star 12; 6 corner-bulb 11; 7 ring7 18; 8 bulb3 9; 9 ring3-ring3 17 /
ring9 25; 10 ring5-bulb 14; 11 ring11 29; 12 ring3-bulb2 14 / ring3-ring4 18; 13 ring13 36; 14
ring7-bulb1 21; 15 ring5-ring3 21; 16 bulb4 12 / ring4-ring4 20; 17 ring17 47; 18 ring9-bulb1 28; 20
ring5-bulb2 17 / ring5-ring4 22; 21 ring7-ring3 28; 22 ring11-bulb1 32; 24 ring3-bulb3 17; 26
ring13-bulb1 39; 28 ring7-ring4 29; 32 bulb5 15; then multi-stage chains (`--then a,b`, `--bulbs`
after a chain): 18 ring3-ring3-bulb1 20, 27 ring3x3x3 26, 30 ring5-ring3-bulb1 24, 33 ring11-ring3
39, 34 ring17-bulb1 50, 36 ring3-ring3-bulb2 23, 39 ring13-ring3 46, 40 ring5-bulb3 20, 45 ring5x3x3
30, 48 ring3-bulb4 20; repeater-relay rings 19 (54), 23 (65), 25 (72), 38 ring19-bulb1 57; dividers
(toggles style) for 3, 5, 7 (ring + one bulb). Missing up to 40: 29, 31, 35, 37 (no ring of 29/31
with 4 relays; a ring of 5+ around a chain stone breaks the hopper rules).

Learned: the judge's level style takes any output whose end-of-window level has period N, so a
TFF is a counter 2 and n bulbs make a counter 2^n. Ring rules: droppers of one ring share a grid
parity; hoppers never face up and never sit under a container (they pull); a dust next to the
input connects to it (a line) and fires the cell beyond and its floor; a dust under the input is
a cross; a dust pointing into a stone makes it a relay that fires its own neighbours (rings to 17).
Counters carry tunable-group headers (`circuit memory stamp-groups`, the ring generator stamps its
own): bulb-chain, dropper-ring, ring-bulbs, ring-chain, ring-divider = 5 catalog cards.

Sequential kinds (judge kinds added by this track, crates/circuit/src/seq.rs, docs/circuits.md):
tffr-dropring (tff-reset, 8b: the dropper-ring TFF with r next to the q dropper only), dff-window
(dff, 10b redstone: a repeater latch opened for 2 gt after the clock's falling edge by a comparator
reading a wire powered by the clock while on and by a NOT-clock stone while off), dffe-window
(dff-enable, 13b: + a NOT-e subtract comparator powering the wire), shift2-window (shift 2, 20b:
two latches on one 5 gt window wire, the output latch opened by a comparator 2..7 gt after the
fall, the input latch by a delay-4 repeater 4..9 gt after it; equal windows always race, and a
repeater longer than the window gap swallows it). Calibrations: cal-dropper-tff-reset,
cal-window-dff, cal-window-dff-enable, cal-window-shift1 + 4 refusals in designs/circuits/selftest/.

## Next
- Smaller redstone TFF: a different core (the 17-block one spends 6 blocks on the NOT-q loop and
  8 on the lock window); the auto-floor census over 3x1x3 at k=6 found none on its first input.
- A master-slave dff (9b if the clock cell can feed a master-lock repeater and a slave-lock
  subtract comparator's side at once: no layout found yet), shift N > 2 (staggered windows), a
  copper-bulb tff-reset (needs r AND q).
- Third RS / D latch mechanism (piston RS: none <= 5 blocks in 3x2x2 / 4x2x1).
