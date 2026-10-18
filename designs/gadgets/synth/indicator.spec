# Sanity spec for the synth model's Indicator kind (exotic track): on the trapdoor-watch harness
# with the trapdoor cleared, the observer must pulse ticks 4..5 after each lever edge and the
# piston above the empty cell must never extend. A note block or stone there would pass the
# repeater's strong power into the piston; a trapdoor, fence gate or skull passes nothing.
# (State probes compare drive's compact probe values: 1/0 powered, E/r extended/retracted.)
probe_state O 3 1 0
probe_state Q 2 2 0
use 0 1 0
run 8
expect O 1..3 0
expect O 4..5 1
expect O 6..8 0
expect Q 1..8 r
use 0 1 0
run 8
expect O 4..5 1
expect Q 1..8 r
