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

`po16.report` is the macro schedule, `po16.txt` + `po16.ideal` its export (`door ideal-rs
po16.ideal`). The replay is BROKEN on the close: at close step 31 SU(-1,-2) should PULSE (spit the
SE piston it pushed up to (-1,0)), but in redsim it pulls it back to (-1,-1); in the same step the
conductor (0,-1), lever-powered to fire SU(1,-2), also QC-powers SU(-1,-2) (its above cell (-1,-1)
touches (0,-1)): the model counts the co-fire (both act) but the replay's pulses do not give the
spit. So this is a macro seed, not a validated mechanism. An earlier model (before the pulse fix)
put a movable observer at (0,-2) facing west (watching SU(-1,-2), firing SU(1,-2): fly's t7o), whose
replay broke because the observer also fired on SU(-1,-2)'s open pulse, which that model did not
see.
