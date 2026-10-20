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

## N=3 analysis (no design yet)
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
