# pext notes (study 6, piston extenders)

Tools: `mctas circuit pext sim|trace|verify|grow|minimize` (crates/circuit-pext). The official
judge is `mctas circuit verify` (kind piston-extender); both agree on every design here. My runner
adds the SUPPORT RULE (every block canSurvive at rest: no floating levers).

## Found (N=2, all robust 16/16 in the real game, `work/`)
* rider7 (slime + observer, flat): lever on A; A pushes a 3-slime row carrying B, the payload and
  an observer hanging under B's slime. The observer fires 2 gt after every landing into B's slime
  (A ignores its own front). Out: B spits the payload one more cell. Back: the slime under the
  payload's +1 spot drags it back one, B's landing pulse reach-pulls it home. 7 blocks, 9/9 gt.
* sf7 (slime-free, 3 variants): an observer on the input pulses A (2 gt, both edges) through a
  stone; A spits B + payload out and reach-pulls B back; B is held by the input at its final
  position only; a second pulsed sticky piston carries an observer whose landing pulse makes B
  reach-pull the payload home. 7 blocks, 7/12 gt. Button versions of both work as is.

## N=3 analysis (the rider, archived as pext-n3-rider24)
* Pulse-toggle trains: a 2 gt pulse on a sticky piston pushes+spits when its front is occupied
  and reach-pulls when the front is empty. The SAME train works on both edges: N=2 `B A B`,
  N=3 `C B C A B C` (N(N+1)/2 pulses: rounds k = N..1 of P_k..P_N). Positions per step mirror
  between the edges (step 1: C@2 on / C@4 off ...), so static sources collide with the other
  piston (step 2 on-edge also hits C) unless the collision is harmless (a piston pulsed in the
  same tick as the piston it would pull: the pulled one is extending, so the pull fails).
* Frames cannot nest: a block a stage piston moves relative to its frame cannot be stuck to that
  frame, so the outer frame cannot drag it back; inner returns must be reach-pulls.
* Observers on a moving frame all fire together 2 gt after the landing (no delays on frames).
* One-frame rider (A held, 5 slimes latching C at rel 1,2 and the payload at rel 2..4): after
  each landing the inner train is `C B C` on BOTH edges. C's first pulse is at world x = rest+1
  on both edges (static source works), B's pulse covers world rel0/rel0+1 (conflict-free), C's
  second pulse covers C@rel2+1 (on) / C@rel1 (off) and also hits B on the on-edge, harmless if
  simultaneous. Needs 3 static timed sources (~t5, t9, t13): a big build. grow k<=3 (broad
  palette) and k<=4 (observers/slime, near 1) on base-r3a found nothing.
* Built by hand (n3-rider-v1, 31 blocks: C1 from an observer under the frame, B from an observer
  chain off the lever, C2 through V1/dust/Z to X5 then U 2 gt later through a stone into x=3),
  minimized to 24 (pext-n3-rider24). Dust lines must not run over the pistons (QC).
* The one-frame rider stops at N=4: the frame moves 2N-1 slimes + N blocks (+ carried observers),
  so N=4 is 11-12 blocks (ideal-r4c works with all-static pulses) and N=5 is over the push limit.

## Gravity (sand payload, travel up): pext-n2-sand6
* The payload falls home by itself, so only the pistons need returning. N=2 in 6 blocks: A pulsed
  on both edges by an observer under the lever (spit up / reach-pull down), B held by the lever
  at its top spot. Does not extend: for N>=3 the top piston is PLACED one cell below its final
  spot (by a spit), and a vertical up-stack piston there is QC-powered by anything powering its
  final cell (the cell above it), so it extends early.

## The CASCADE (self-triggered units): pext-n3-cascade22, pext-n4-cascade33
* Unit = slime (in the line, behind the piston) + sticky piston + observer hanging under the slime
  facing down. A moved unit lands, the observer fires into the slime, the slime powers the piston:
  the unit's piston pulses 2 gt after every move of its unit, by itself.
* In the pulse-toggle train T_N (rounds k = N..1: P_k P_k+1 .. P_N) every push or pull moves ONE
  unit, and each step after the first of a round is exactly "the unit just moved fires": the whole
  train runs itself once the N round starts are delivered (P_N, then P_N-1, ..., then A). Nothing
  ever moves more than one unit (3 blocks), so there is no push-limit ceiling.
* Round starts are position-addressed (P_k at its rest cell on the on-edge, at its extended cell
  on the off-edge). Units are 2 cells long, so physical cells differ from the abstract model:
  N=3: C {x5, x7} at once, B {x3 then x4}, A. N=4: round D's on-edge cell x7 is where C sits on
  the off-edge, and round C's on-edge cell x5 / off-edge x7: one LAMP-delayed observer (lamp
  lit at once on the on-edge, dark 4 gt late on the off-edge) pulses x7 at t4 on / t8 off, which
  is D's round on the on-edge and C's round on the off-edge; x10 and x5 come from normal chains.
* Pulsers must never touch a cell a slime passes through (the slime would drag them): they sit
  diagonal to the line, above-beside the piston cell at (x, y+1, z+-1) or two above at (x, y+2),
  facing away, and power the piston by quasi-connectivity (the observer's update reaches the
  piston through the cell above it).
* `mctas circuit pext trains` enumerates pulse-toggle trains in the abstract 1D model and checks
  each step's static delivery (single cell, both cells at once, staggered); `--bfs` searches the
  shortest fully deliverable train: N=3 plain has one (AACBACBC), N=3 rider and N>=4 none, which
  is why the cascade (only round starts addressed) plus edge-delayed sources is the way past N=3.
* Generator `pext cascade --n N` (any N; family cards pext-cascade-r2/r0/r1): one westward
  observer spine at y=3 behind the line, each pulse cell's pulser fired 4 gt after the spine passes
  it, +4 gt towers where a round must wait (gap >= 6 gt between round starts of one edge). The lamp
  is not needed: a static pulse may serve different rounds on the two edges (x7 = D's round on the
  on-edge, C's round on the off-edge), and rounds overlap (cascade waves 6 gt apart never meet).
  Blocks 12N - 7 - 2 floor((N-1)/3), extend 14N - 6, retract 16N - 8.
* FLAT (`--flat`, 1 wide, family cards pext-cascade-flat-*): the pulser of cell x sits two above the
  line at (x, 3, 0) facing up and powers the piston through the air cell above it (QC: its update
  reaches the piston through that cell); it watches the spine node right above it. One observer per
  pulse instead of two: blocks 10N - 5 - floor((N-1)/3). Minimized in the plane (`minimize
  --plane-z 0`) these are the smallest designs for every N >= 3 (N=3 19 blocks ... N=8 54).
* Judge rule (learned the hard way): once extended, the payload must STAY while the input is on;
  a design whose payload reaches the target, leaves and comes back fails (circuit archive refused a
  20-block N=3 that did; pext sim/verify/minimize check it now).

## Slime-free N=3: pext-n3-sf43
* Plain stack A B C + payload; the train A A C B A C B C (the BFS result) with every step from fixed
  pulsers (A three times, x4/x5 and x2/x3 pairs staggered 2 gt: the late pulse finds a piston whose
  base is still a moving block, so its pull fails), 12 pulsers on one observer spine that sweeps the
  line five times (rails behind, in front, below, above; `pext spine` builds a chain along a path),
  62 blocks, minimized to 43 without slime in the palette.

## Slime-free stacks for any N: `pext trains --cells` + `pext stack --cells`
* The most general static schedule: a sequence of SINGLE-CELL pulses, each firing on both edges
  (whatever piston stands on that cell acts, on each edge). Breadth-first over the pair (on-edge
  state, off-edge state) finds the shortest for every N, a clean sweep pattern (abstract cells, A =
  x0): N=2 `x2 x0 x1 x2`; N=3 `x4 x2 x0 x3 x1 x2 x4`; N=4 12 pulses; N=5 17; N=6 24; N=7 31.
* `pext stack --cells` places one observer pulser per pulse beside its cell (sides, top, bottom, two
  quasi-connectivity slots above-beside, A's back) and routes one spine from the lever through the
  watched cells in order (BFS over (cell, entry direction): an observer emits STRAIGHT THROUGH, so
  at a turn its back points ahead, never into the line or the cell above it; future targets keep
  their neighbours free). Generated: N=4 96, N=5 136, N=6 214, N=7 276 blocks; minimized: N=3 38,
  N=4 60. N=8 boxes the router in (one cell needs 5 pulsers there).
  With each cell's preferred sides alternating with its parity (fewer crowded bands): N=6 192, N=7
  254, N=8 356 (N=9 still boxes in); minimized N=5 105.
* `pext trains --interleave` (the cascade's per-edge reuse for a fixed train) also works but needs
  more pulses (16 for N=4).

## World-record push (10-20): `pext topo`, `pext lns` (targets and status: [WR.md](WR.md))
* `pext topo` sweeps N=2 1-wide topologies (the second piston and the payload at 11 placements, k
  grown blocks within 2 cells, `--free-lever` lets the lever go anywhere): no 6-block design; at 7
  only the rider and its mirror (slime or honey frame), 9/9 gt.
* `pext lns` removes every r-subset of a design's blocks and grows back at most r-1 (one subset per
  worker, `--keep-y` keeps the line). From cascade19: 18 blocks. Nothing smaller from 18 (in the
  plane, in 3D, with the `core` palette, removing line blocks too), from 26 (N=4, r=3 and r=4), 33
  (N=5), 39 (N=6). `--free-lever` lets the lever move too (removable, regrown anywhere, never
  counted): from 18 it finds 46 designs of 17 blocks (the lever hangs on the chain's first observer
  and the sticky piston under it shoves the front piston's starter aside and back), and from 17, 24
  designs of 16: the front unit's observer rides ON TOP of its slime facing up and fires when that
  sticky piston pulls back above it, so the front piston needs no starter. Archived
  pext-n3-flat-cascade16 (28/34 gt); 17 and 18 removed (dominated). From 16 nothing smaller (remove
  3 or 4, grow 2).
* `--free-lever --repeat` on the rest of the flat family: N=4 26 -> 23 (35/47 gt), N=5 33 -> 31, N=6
  39 -> 38, N=7 48 -> 46, N=8 54 -> 53, N=9 61 -> 59 (remove 2, grow 1 is enough for N >= 6 and runs
  in seconds); N=10..12 nothing with remove 2. Each record archived, the old one removed.
* Dead ends (redsim traces): a staircase of riders and a single frame carrying both stage pistons
  (`frame3`) fail on simultaneous triggers: one push moves every carried observer, they all fire 2 gt
  later, the block event order lets one piston act and the other fail, and an observer moved while
  powered lands powered and never fires again.
