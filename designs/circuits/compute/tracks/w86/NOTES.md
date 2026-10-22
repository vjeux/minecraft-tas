# Track w86 in study 12 (compute): combinational building blocks

Coordinator 2026-10-21 00:11Z: building blocks for redstone computers and locks. A 2→4 decoder, a 4:1
multiplexer, a 2-bit magnitude comparator (lt, eq, gt) and a 4-bit binary up-counter. They are judged
like the adder: real inputs in every transition order, settled outputs, glitches and latency, robust.
The goal is the minimum size of each. The kinds already existed in study 12 (`decoder N`, `mux N`,
`comparator N`, `binary-counter N`; crate::compute, the combinational judge), so the designs use them.

## Designs (all `circuit verify`, 8 positions x 4 rotations)

| design | kind | blocks | box | latency | how |
|---|---|---|---|---|---|
| [bcount4-spiral](bcount4-spiral.txt) | binary-counter 4 | 15 | 4x5x3 | 14 gt | a ripple counter of copper bulbs up a spiral stair; archived |
| [bcount4-bulb](bcount4-bulb.txt) | binary-counter 4 | 18 | 14x2x1 (flat) | 14 gt | the same in a straight line; archived |
| [dec2-24b](dec2-24b.txt) | decoder 2 | 24 | 5x2x4 | 8 gt | arith synth with fixed ports (proved smallest for those ports in a 5x4 flat grid); archived |

## Binary counter

A copper bulb flips (lit / unlit) on every rising edge of its power (26.3 source, CopperBulbBlock:
neighborChanged, no tick). It is not a redstone conductor. A comparator reads it at 15 while it is lit.
Each stage is a bulb, a comparator reading it into a stone (the bit's output beside the stone), and a
torch on that stone (lit while the bulb is unlit) that powers the next bulb. Each bit therefore flips
when the bit below goes from 1 to 0: counting up.

In the spiral, each next bulb stands on the torch (a wall torch powers the block above it), each
comparator stands on the stone of the stage below, and the chain turns 90 degrees per stage. Only
the first comparator needs a floor. Two other foldings conflict:
- a standing torch with the comparator on the bulb below: the next stage's stone lands above a
  torch, which powers it;
- a torch in line with the stage: a torch lands above an earlier bulb and powers it.

The inverse scheme does not work. That scheme starts all bulbs lit, chains on rising edges and reads
inverted. It would need a torch on each bulb, but a bulb is not a conductor, so the comparator's
power does not reach that torch.

## Decoder

The arith synth (`--kind "decoder 2"`, R7 inputs) with auto ports found 30, then 28, 27 and 25 blocks
in a 5x4 flat grid while minimizing (slow: each step proves no smaller design for every port
placement). With the ports of the 27-block design fixed, it found 24 and proved no design of 23 or
fewer for those ports in 7 minutes. Its solutions rely on one-level strength differences: a wire one
block further from a lit torch is one weaker, and a subtract comparator with back 15 and side 14
stays on.

Hand designs: two subtract comparators next to adjacent inputs give q1 = a0 - a1 and q2 = a1 - a0
with 8 blocks. But R7 inputs need a free side for the judge's repeater, outside the build's box, and
that pins the rest of the design between the two inputs. q0 and q3 found no room there.

## Tools

* `circuit arith synth` gains `decoder N` (optional enable `e`) and `mux N` functions.
* CEGIS with `--r7` (2026-10-21 fix): with a partial `--sample`, a design that the circuit judge
  rejects now adds its failing combination to the model. The combination is read from the judge's
  "X rises from <state>" line, or found by the synth's own rig, or else the next 8 unseen ones are
  added. Before the fix, an `--r7` run only forbade the structure, and a model wrong on a combination
  it had not seen came back forever (the 4:1 mux looped 160 rounds on two transitions).
