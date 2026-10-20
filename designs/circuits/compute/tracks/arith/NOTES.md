# Track arith (study 12: adders, subtractors, incrementers, comparators)

Owner: pext (session ec7bcf52). Study owner and judge: clk-obs (crates/circuit/src/compute.rs,
[VARIATIONS.md](../../VARIATIONS.md)). Split agreed 2026-10-20: arith = adders, subtractors,
incrementers, comparators (binary, hex/strength, bit slices); clk-obs = mux, demux, decoders,
encoders, binary counters.

## Tools (crates/circuit-arith, `mctas circuit arith ...`)

* `check <design>...`: the judge's walk on redsim (every state in Gray order, the n-cube Euler
  tour, complement jumps; a hex value counts as N bits), each state held `--gap` gt: every output
  must settle to the ideal (an exact level for hex `s`/`d`) and the judge's lamp must follow;
  latency with the worst transition, per-output latency, glitches, blocks. Agrees with `circuit sim`
  on the calibrations (cal-hex-gt: 4 gt, 4 blocks).
* `synth --kind K --in .. --out .. [--auto-ports] --grid WxD`: SAT synthesis (rustsat + CaDiCaL) of
  a flat design (floor y = 0, components y = 1). Grid cells: air, stone, redstone block, dust,
  repeater (4), comparator (4 x compare/subtract), torch, wall torch (4). Steady-state model per
  input combination, exact levels 0..15 (order encoding), read from the 26.3 classes: conductor
  direct vs strong power (dust hidden while a wire computes), dust connection shapes
  (`getConnectionState`: no connection on an axis -> both sides of the other axis), comparator
  back/side rules (`getControlInputSignal`: dust, redstone blocks, diodes; containers at the back),
  torches on their block's `hasSignal`. Ports in the ring around the grid, fixed or placed by the
  solver (`--auto-ports`); hex inputs are chests read by the design's comparators. Two rules
  outside the steady model: feed-forward ranks (no loop through a torch/diode/block: an in-phase
  torch pair oscillates; a dust's strong/weak split keeps "dust on a block" legal) and no
  update-less redirection (a dust next to a port that changes shape between two tested states
  must change power too). Minimizes grid blocks (totalizer); every model runs the walk on redsim
  (CEGAR): a counterexample state not yet in the model is added (CEGIS over the 256+ states of
  hex kinds, `--sample`), any other failure blocks that structure.

## Results (redsim `circuit sim` unless marked "game")

| design | kind | blocks | latency | notes |
|---|---|---|---|---|
| [hex-ge-6b](hex-ge-6b.txt) | comparator 4 hex, ge | 6 | 6 gt | synthesized (3x3 grid, auto ports): subtract comparator b - a into a block, wall torch = NOT; a = b = 0 handled |

## Log

* 05:10 first synth runs: XOR/AND rejected by redsim until two model fixes: feed-forward ranks
  (in-phase torch pairs oscillate) and the update-less redirection rule (a dust on a redstone
  block pointing away when an input appears: same power, no block update, the torch behind never
  reacts). After them every model of AND/OR passed redsim. AND: 6 blocks flat (auto ports).
* Full adder flat: fixed port layouts (inputs west, outputs east; and a spread layout) have NO
  design in a 4x4 grid (UNSAT, 32 blocks max). Auto ports in 5x4: running.
