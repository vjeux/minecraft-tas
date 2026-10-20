# Track d67: feeding the pistons of c70b's 6x7 (42) mechanisms (clk-misc session, 2026-10-20)

Brief (coordinator, 20:40 / 20:57 PT): c70b's best 6x7, explore/c70b 254d27d79
`smaller/mm/6x7-in2/mm01-oc` (11 sticky + 1 normal, statics (3,0..2); WORKS at 32 positions, open 129 /
close 39) has two wiring problems: (a) one 2-gt pulse to the piston carried into (0,1) at open t72 (plan
input in the hallway), (b) SU(1,-2) has no feed. Find feeds (door blocks as conductors, quasi-connectivity,
carried observers, block-update tricks), prove them in redsim, hand them to pockets3 / c70b.

Working copies: `in2/mm01-oc.{txt,ideal}` (c70b's, design path rewritten). Tool: `door ideal-rs --planes
LO..HI` draws the door plane after every tick of a window (open and close): what touches a carried piston
when it fires.

## The mechanism, as the planes show it (pistons by rest cell)
a SE(-2,-2), b SU(-1,-2) (pushed by a to (0,-2) at open t37, back at close t7), c SU(1,-2), d SU(2,-2),
e SU(-1,-1) (rides the left column (-1,-1..0)), f SE(-2,1), g SE(-1,2) (rides the left column and row 1:
(-1,1) / (-1,0) / (-1,2) / (0,1) / corner (-1,3) when open), h NE(-2,3) (fires once, open t109),
i SD(-1,4), j SD(0,4), k SD(2,4).
Cells never occupied during a cycle (the only places static wiring can stand): (-2,-1), (-2,0), (-2,2),
(-2,4), (1,4), (3,4), (3,3), (3,-2), (3,-1). Regions: {(-2,-1),(-2,0)}, {(3,-2),(3,-1)}, {(3,3),(3,4)}, and
the single cells (-2,2) (between f and h), (-2,4) (between h and i), (1,4) (between j and k).

## (b) c = SU(1,-2): no feed inside the 6x7 box (proof by cells)
c fires at open t127 (pull BM down) and close t37 (push it back). Direct neighbours: (0,-2) (b's waypoint,
swept open t37 -> close t11: no static block can stand there), d (2,-2), (1,-3) outside the box, (1,-1)
its front. Quasi cell (1,-1): neighbours (0,-1), (2,-1), (1,0), all frame / door cells. The only conductor
there a free cell touches is BR at (2,-1) (from the corner (3,-1)), and only at close:
- a source at (3,-1) powering BR also QC-powers d (its above cell is (2,-1)) and updates it; d's push moves
  BR (moveBlocks makes (2,-1) a moving piston) BEFORE d's base update reaches c, so c sees no power;
- at open BR is still at (2,0) when c must fire; pulling it down first makes d re-fire on the same source.
- the lever on c's front face (1,-2,1) (legal, below the opening's columns): TESTED (`in2/mm01-oc-cl`,
  c extended at rest, the lever drives it on both edges): the lever POPS at the open flip. A retracting
  piston's base becomes a moving piston for 2 gt and every attached block loses its support (same for a
  repeater standing on f, `in2/mm01-oc-q7`). So no lever, button, torch, dust or repeater may hang on or
  stand on any piston that ever retracts: in this mechanism only the statics (3,0..2) can carry the lever.

## (a) g carried into (0,1), 2-gt pulse at open t69..t73: no feed inside the 6x7 box
g lands at (0,1) at t69 (f's spit) and must pull MR (2,1) -> (1,1); f pulls it back at t79. Around it:
(0,0) empty, (-1,1) empty, (1,1) its front, (0,2) TL; quasi cell (0,2) with neighbours ML (-1,2), TM
(1,2), (0,3) (empty ceiling cell). Power can reach g only from (0,3), (-1,1), (0,0), (1,1) (frame / door
cells) or through ML (strongly powered from (-2,2), (-1,3) or (-1,1)) or TM (from (1,3), (1,1), (2,2)).
The corner (-1,3) is ML's / g's waypoint (t49-63, t91-108, g from t115); the only free cell is (-2,2), and
it is sealed: f below, h above, (-3,2) outside the box. Only full blocks survive there (anything standing
on f pops when f retracts, tested), and no full block there can emit east without an input from (-3,2):
an observer facing west watches (-3,2); facing up / down it outputs into f / h; a repeater / comparator
facing east takes its input from (-3,2).
What does work physically: a carried piston FIRES ON ARRIVAL when it lands on a powered spot (onPlace
re-checks its power; no extra update needed), and a strongly powered frame block QC-powers a piston whose
above cell it touches. So with ONE more column (x = -3, a 7x7 = 49), an observer at (-2,2) facing west
(watching (-3,2)) firing into ML at g's landing tick would fire g at (0,1) through QC.

## Also unfeedable in 6x7 (connect is optimistic about output directions)
i = SD(-1,4) (open t61, close t25): neighbours (-2,4) (sealed single cell: h below, i, outside), j, (-1,5)
outside; its quasi cell (-1,5) is outside. A full block at (-2,4) cannot emit east without (-3,4) (as
above); a torch on h pops at h's retraction. h is fed only from the sealed (-2,2) / (-2,4).
Ways out: the lever on a stone at (-2,4) would give i and h the lever's LEVEL (both edges at t0); or the
7th column (x = -3) feeds f, h, i, (-2,2) and (-2,4) like the plan's own (-3, y) inputs; a row below
(y = -3, 6x8 = 48) feeds a, b, c, d like the plan's (x, -3) inputs. The plan was written for those
cells: every non-hallway input of the plan sits at x = -3 or y = -3.
