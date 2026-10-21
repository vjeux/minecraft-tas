# Every published pick against the game (clk-misc, 2026-10-21)

The coordinator (for vjeux: "is it verified against the game?"): each kind's current pick, its judge's
scenario recorded as an exact game trace and replayed in redsim, every frame compared (blocks with their
full states, every entity's position and motion to the bit, the level random). 245 designs: every
published circuit (each pick and every row of a parametric kind's table), the 7 TNT picks on the
judge's ground, the 2 shulker picks, the sorter, the 10 published doors. All identical after two fixes.

| kind | pick (sizes) | frames | identical |
|---|---|---|---|
| BUD | bud-slime-qc | 14316 | yes |
| delay | 9 lengths (delay-rep2-n0 ... delay-piston-13) | 884-1017 | yes |
| seven-segment display | seven-segment-therm-v2 | 21545 | yes |
| flying machine, flat | fly-horizontal-6b-10gt-01ce95 | 292 (to the corridor end) | yes |
| flying machine, tall | fly-payload-n1 | 292 (to the corridor end) | yes |
| flying machine, two-way | fly-docking-15b | 4660 | yes |
| gates | and, imply, nand, nimply, nor, not, or, xnor, xor | 81-281 each | yes |
| combination lock | lock1-hid ... lock4-hid | 773 / 5265 / 41117 / 327849 | yes |
| counter | 39 sizes (2 ... 576) | 209-41537 | yes |
| D latch, DFF, DFF+enable, RS latch, shift, TFF+reset | d-latch-repeater, dff-lock, dffe-lock, rs-dropper, shift2-lock, tffr-ring | 281, 2211, 841, 265, 1161, 618 | yes |
| piston extender | 11 reaches (pext-n2-rider-flat ... n12) | 161-971 | yes |
| sand elevator | pext-n2-sand6 | 173 | yes |
| edge detectors, pulse extender, pulse limiter | 102 sizes over 5 kinds | 425-14543 | yes |
| burst, divider, monostables, off/on delay, pulse train | 38 sizes over 7 kinds | 621-63841 | yes |
| T flip-flop | tff-bulb | 1341 | yes |
| shulker loader / unloader | shulker-loader-1wide / shulker-unloader-1wide-22b | 791 / 711 | yes |
| item sorter | sorter-classic | 4121 | yes (after fix 2) |
| TNT duper (fast, lever) | tnt-engine-lever-17b | 2507 | yes |
| TNT duper (ground variant) | tnt-clocked-22b | 1801 | yes (after fix 1) |
| TNT precise drop / with lever | tnt-precise-drop / tnt-precise-drop-lever-52b | 1801 / 2507 | yes |
| TNT precise air burst / with lever | tnt-airburst / tnt-airburst-lever-56b | 1801 / 2507 | yes |
| one TNT per press | tnt-button-17b | 921 | yes |
| doors (the 10 published) | e78k-h5-41b, e78k-h3-38b, e78st-...-37b, gravity2-g33-33b, gravity-g8-48b, ground-v225b-98b, test-static-armor-39b, fast-clean-s4c-90b, fast-clean-c1r-107b, tick-door-open4-218b | 2201 each (3 open/close cycles) | yes |

The divergences and their causes:

1. tnt-clocked-22b, tick 553: the 9th TNT's motion one ulp off (x -0.012623413035556806 against the game's
   ...804). Java's `Math.sin` is HotSpot's Intel LIBM intrinsic on x86-64, which is not always
   correctly rounded: for this kick angle (`0x400388d384e3cab3`) it is one ulp below `StrictMath.sin`
   and Rust's `f64::sin`. redsim now has the intrinsic's main path operation for operation
   (`crates/redsim/src/entity/jmath.rs`, from OpenJDK's stubGenerator_x86_64_{sin,cos}.cpp and its
   tables), checked bit for bit against the game's JVM on 265536 arguments (every `Mth` sine-table
   argument, 200000 kick angles); the TNT kick and the `Mth` table use it.
2. sorter-classic, frame 0: the game side's container text (`clock::wave::be_extra_from_nbt`) split the
   item list at `},`, so a named item (the filter papers: sorted keys put `components` between `Slot`
   and `count`) was dropped, and an id holding a `b` was cut there (the `b` of `Slot:0b`: `bucket`,
   `cobblestone`); it now reads each item compound's own keys. (The same text is what the clock judge
   compares between frames: a clock whose containers hold such items read part of them wrong.)

The scenes: `designs/clocks/tracks/clk-misc/redsim-tests/traces/` (tnt-clocked-22b-judged with the
judge's ground through tick 553, bud-slime-qc-judged, fly-horizontal-6b-10gt-01ce95-judged, the doors
e78k-h5-41b and test-static-armor-39b; their judged builds in `picks/`), the selftest's
`circuit:sorter-compare`, `jmath`'s unit test.

Again (one game run per design, then redsim; the traces are large, keep them out of the repository):

    mctas clock misc trace-export --exact --circuit --out-dir T --judged-dir T $(mctas site kinds --paths | grep -v '/tnt/\|/shulker/\|/sorter/')
    mctas clock misc trace-export --exact --circuit --ground grass --random-tick-speed 0 --out-dir T --judged-dir T <the TNT picks>
    mctas circuit shulker trace --exact --out-dir S <the shulker picks>; mctas circuit shulker compare S/*.trace
    mctas circuit sorter-run --compare designs/circuits/sorter/archive/sorter-classic.txt
    mctas clock misc trace-export --exact --ticks 2200 --lever-flips 400..1900/300 --out-dir D <the published doors>
    redsim-fuzz trace-compare T/*.trace D/*.trace
