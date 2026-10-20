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
