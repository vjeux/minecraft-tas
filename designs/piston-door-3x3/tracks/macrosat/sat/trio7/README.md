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
| + `--lever-flip --region-budget '-2,-2,-2,-1:1'` (at most one A2-fed action per edge) | SAT (569 s): `lf16a1.*`; A2 feeds ONE action in all: close step 20, SU(-1,-2) PULSE line 2 from (-2,-2); observer at (3,3) facing west; replay BROKEN like lf16 ((1,1) stone at the open end) |

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
