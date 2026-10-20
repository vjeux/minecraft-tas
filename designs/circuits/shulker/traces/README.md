# Shulker loader / unloader traces

Real-game recordings of the archived designs (designs/circuits/shulker/archive/) for the in-browser
viewer, written by `mctas circuit shulker trace designs/circuits/shulker/archive/*.txt` (and
`--exact` for `exact/`). The format is the clock traces' (designs/clocks/traces/README.md): a
header line, then one line per game tick with the cells that changed and every entity (the box
drops). The header adds `"scenario"`.

Each trace is the judge's first build (rotation 0, position 0, its seeded drop draws) running 3
boxes of iron shovels (27 per box: a short replay; `--stack 16` for designs made for stackable
items, like the speed unloaders). Tick 0 is when the judge's chests are filled and the lever is
on; the trace ends when the judge's run does (the last box delivered). The judge's chests are
blocks of the trace; their contents are not recorded. Since the bridge seeds each drop's own
random (docs/mechanics.md, "Drops and the bridge's seeds"), a re-recording is identical (but for
the entities' `net_id`, numbered by the JVM).
