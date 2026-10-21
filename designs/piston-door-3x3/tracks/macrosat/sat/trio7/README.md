# trio7 (w86, explore/w86 a0c3b1a99 tracks/w86/seven/trio7.txt) under macrosat

Layout fixed (`trio7-input.txt`), box -2..4 x -2..4, the lever forced on (1,4)'s top
(`--lever-at 1,4`: (1,4) directly, (0,4) and (2,4) by QC), wiring free in the empty cells, h 16 per
edge.

| run | result |
|---|---|
| mechanics only | SAT (751 s) |
| feed + lever, no island start | SAT (563 s) |
| feed + lever + island start | SAT (411 s); export replays BROKEN (no --physics) |
| + `--strict --observers` (budget 2, private feeds, frame3) | UNSAT (482 s) |
| + `--physics --observers --budget 2` | SAT (919 s), old model (pulse visibility bug) |
| + `--physics --observers`, fixed model (a pulsing piston counts as a change) | SAT (839 s): `po16.*` |
| + `--lever-flip` (the lever-held trio switches only at the flips, w86) | SAT (518 s): `lf16.*`; a movable observer at (3,3) facing up (watches (3,4), powers the static extra (3,2)) |
| + `--lever-flip --region-budget '-2,-2,-2,-1:0'` (A2 feeds nothing, clk-item) | UNSAT (455 s) |
| + `--lever-flip --region-budget '-2,-2,-2,-1:1'` (at most one A2-fed action per edge) | SAT (569 s): `lf16a1.*`; A2 feeds ONE action in all: close step 20, SU(-1,-2) PULSE line 2 from (-2,-2); observer at (3,3) facing west |

**`lf16a1-L.ideal` WORKS in redsim with ideal inputs (32 positions x 3 cycles, open 156 / close 154
ticks, extent 7x7 = 49).** It is `lf16a1`'s export (fixed exporter: conductor levers toggled) with
the lever written the same way at rest and at the close end: a redstone block on top of (1,4) at
(1,5) (it powers (1,4) and QC-powers (0,4)/(2,4), like the real lever there) plus (1,4,1), removed at
open step 0 (the plain export holds the trio with blocks behind each at rest and with (1,5) at the
close end, so its replay differs only in those input cells). What is NOT done: the wiring. The 15
wiring cells (left as air) must produce the plan's pulses with real components, island start and
budgets as macrosat assumed them; A2 (-2,-2..-1) gives one pulse in the whole cycle.

Then the screens (clk-item, c70b): A2 needs nothing (lf16a1-L still WORKS without its one A2
action), but the sealed C1 (0,-2) must feed SU(-1,-2) to its west and SU(1,-2) to its east at
different ticks, which one component cannot do. Encoded as `--wire-dir` (one fixed-output component
per wiring cell; a directional one needs its input cell inside the box):

| trio7, `--physics --observers --lever-flip`, island start, h16 | verdict |
|---|---|
| `--wire-dir` | UNSAT (458 s); h20: UNSAT (1063 s) (both with the first, stricter MULTI); sound rule (74e837865): h16 UNSAT (468 s), h20 UNSAT (1426 s) |
| `--private` (a wiring cell feeds one piston cell) | UNSAT (524 s) |

`lf16a1-L-assist.*`: the plan rewritten assist-style (`door ideal-rs --assist-dir`); BROKEN, since a
block behind SE(-2,1) also QC-powers SE(-2,0) below it. `lf16a1-L-assist.drives` keys the original
plan's inputs by the piston each moved.

`lf16`'s export replays BROKEN too: at the open end (1,1) is still stone. The model's open step 6
pulse-pulls (1,0) down into (1,-1) with SU(1,-2) and step 8 extends SU(1,-2) into an empty front;
in redsim the pulled block is still in (1,-1) at step 8, so the extend pushes it back up.

`po16.report` is the macro schedule, `po16.txt` + `po16.ideal` its export (`door ideal-rs
po16.ideal`). The replay is BROKEN on the close: at close step 31 SU(-1,-2) should PULSE (spit the
SE piston it pushed up to (-1,0)), but in redsim it pulls it back to (-1,-1); in the same step the
conductor (0,-1), lever-powered to fire SU(1,-2), also QC-powers SU(-1,-2) (its above cell (-1,-1)
touches (0,-1)): the model counts the co-fire (both act) but the replay's pulses do not give the
spit. So this is a macro seed, not a validated mechanism. An earlier model (before the pulse fix)
put a movable observer at (0,-2) facing west (watching SU(-1,-2), firing SU(1,-2): fly's t7o), whose
replay broke because the observer also fired on SU(-1,-2)'s open pulse, which that model did not
see.
