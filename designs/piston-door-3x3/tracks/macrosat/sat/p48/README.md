# macrosat 48-cell seed (8x6, x -3..4, y -2..3)

The fixed-model sweep (devvm53196 f13-p48: island start with the pulse-visibility fix, `--physics
--observers`, free layout, h 8) found box (-3..4, -2..3) SAT after 18 389 s. Its layout re-solved
with the newest binary on the same flags (`a8.*`, 129 s):

- `door ideal-rs a8.ideal --extra-offsets 28 --cycles 3`: WORKS in redsim (32 positions x 3
  cycles), open 76 / close 74;
- `door ideal a8.ideal`: WORKS in the real game (4 positions x 2 cycles);
- `--extent`: 8x6 = 48 cells.

It is a MECHANISM seed only. On its fixed layout at h 8, `--lever-flip` (the lever on (0,3) is one
level: the pistons it touches switch only at the flips) is UNSAT (114 s), and `--wire-dir` (one
real component per wiring cell, exact island start) is UNSAT (55 s); both together at h 12 are
UNSAT too (370 s).
