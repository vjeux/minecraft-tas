# clk-obs: wiring checks for the sub-56 door (from 2026-10-20 20:00Z)

Coordinator brief: help fly (tracks/fly) wire its sub-56 mechanisms. I take the wiring
sub-problems it hands me (pulse generators, area sequencers) and answer "this wiring" or "no
wiring exists in these cells".

## Method: exhaustive area fillings with a real lever, on redsim
The plans are in [plans/](plans/) (run from the repository root), e.g. the top-right of q69j:
`door ideal-rs designs/piston-door-3x3/tracks/clk-obs/plans/tr-L44.ideal --vary="4,4=lever[face=wall,facing=east,powered=false]|lever[face=wall,facing=east,powered=true]" --vary="3,3=<states>" --vary="3,4=<states>" --vary="3,5=<states>" --ok-only --extra-offsets 0 --cycles 2 --threads 60`.
`door ideal-rs <plan> --vary "x,y=s1|s2|..." --vary ... --ok-only --extra-offsets 0 --cycles 2`
judges every combination of the listed cell states (in memory, milliseconds each):
* the plan keeps the other areas' ideal inputs (shifted later by a variable `s` so a slower
  wiring keeps the order) and drops the area's own inputs;
* the lever is real: a `--vary` on its cell (both rest polarities) and `open 0 use x y z` /
  `close 0 use x y z` in the plan (one plan per lever position);
* first check the harness: the same plan with the area's ideal inputs restored must WORK.
States that work everywhere in the plane: stone, redstone_block, glass, observers (east, west,
up, down), redstone_torch, redstone_wall_torch (east/west), dust as a cross and as a dot,
repeaters and comparators facing east/west, note block, lamp, waxed copper bulb, pistons and
sticky pistons (4 in-plane facings), slime.

## fly's q69j (6x9 = 54): neither top area can be wired (2026-10-20 20:15Z)
* Top-right ((3,3) (3,4) (3,5); pistons Qr (2,3)->(2,4), Br (2,5) extended at rest): 34 states
  per cell x lever at (4,3) / (4,4) / (4,5) / (3,6) x both polarities: 0 of ~140k work. Why: the
  x = 3 column sees nothing of the mechanism (an observer facing west outputs out of the box), and
  no repeater can point along it (repeaters face east/west; their input cell x = 4 is outside). So
  it is lever-driven logic of <= 3 elements: the lever level, a 2 gt pulse per flip (an observer
  with the lever on its face), or the level inverted 2 gt late (one torch). It can't give Qr at
  (2,3) a >= 4 gt pulse nor hold Br >= 8 gt after the flip.
* Top-left ((1,5) (0,5), (-1,5) wall torches only; P_T (1,4) ON at rest, Pl (0,4) OFF at rest;
  trigger: an observer at (1,5) facing east on Br), top-right ideal: 0 of 3468 work. Why: P_T can
  only be fed from (1,5) or by QC from something at (0,5) / (1,6) emitting into (1,5). With the
  trigger observer at (1,5) (output west only), anything at (0,5) feeding that QC also powers Pl
  (which touches (0,5)), except a torch standing on Pl, which never turns off (a piston is not a
  conductor).
* Both at once: the lever must touch the x = 3 column (only it can power Qr at (2,3)), and a
  lever holding P_T at rest (on (1,5)) excludes that.

## Toggle-only top (fly's direction 1, q68f events, 2026-10-20 20:42Z)
With every top pulse 2 gt (`q68f` plan, Pl on at open 16, Pl off at close 68), measured windows:
open P_T + P_TR reach-pull at any time >= 16; close P_TR spit at any time 40..110 (before or
after the slime); close P_T FIRST spit only <= 66, before the slime returns (it clears C out of
(1,3) so the returning slime can push TM back in); close P_T second spit >= 74 (77-78 fail). The
slime can't return before the bottom's close actions (0 of 3751 timings). So a top that sees only
the slime (observer at (-2,4) facing down on (-2,3)) can't do the close: it needs the lever or a
precursor the bottom shows it before the slime returns.

## fly's r69 / r69a / r69t top (2026-10-20 21:20-21:40Z)
* r69 (M elevator above P_T): P_T at (1,4) and M at (1,5) can't be fed apart. P_T's only feed is
  QC through (1,5), i.e. a signal INTO (1,5); M sits at (1,5) and its own power check sees the
  same signals, and M is updated first (the emitter updates its output cell before that cell's
  neighbours), so M spits P_T down. r69 + M pulses at t0/t30: BROKEN. P_T at (1,3) (open t18) has
  no feed at all. fly dropped M (r69a), then found one train for both edges (r69t).
* r69t top windows (plans/r69t-top-windows.ideal, every bottom pulse shifted by x = the bottom's
  start after it sees the slime): P_T first pulse >= 5 gt before Pl; Pl 5..16 (x = 12); P_T
  second >= x + 3 (after Qv@(1,-1)'s spit), tested to 60; P_TR any time 0..60; the bottom may
  start as late as x = 30.
* r69t top with the lever in the top: 0 of 4.5M (plans/r69t-top-L45.ideal and the other lever
  spots: (0,5) (1,5) (2,5) (3,5) (3,4) x 13 states x 6 lever spots x 2 polarities; harness
  checked with the ideal top inputs and a far lever). Why: P_T (2 pulses) and Pl (1 between)
  both draw from the (1,5) hub (Pl is fed through (1,5) into (0,5); P_T by any signal into
  (1,5)), in-plane repeaters can't be locked, and nothing in the top can time a pulse after the
  bottom's Qv spit (its only eye on the bottom, a slime rider at (-1,5) seen from (0,5), fires
  ~3 gt too early).
