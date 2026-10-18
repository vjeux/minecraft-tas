# "Lever on, plus >= 12 ticks after it turns off" at the cell east of the lever's block.
# The lever (0,2,0) sits on X (0,1,0); X itself powers the output cell (1,1,0) while the lever is
# on, so the circuit only has to keep it powered 12 ticks past the off flip, then let it go.
probe out 1 1 0
use 0 2 0
run 30
expect out 1..30 on
use 0 2 0
run 40
expect out 1..12 on
expect out 30..40 off
use 0 2 0
run 30
expect out 1..30 on
use 0 2 0
run 40
expect out 1..12 on
expect out 30..40 off
