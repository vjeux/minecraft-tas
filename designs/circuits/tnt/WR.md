# Study 9, TNT dupers: world-record targets (2026-10-20)

vjeux 12:45Z: "world record kind of implementations for every single one of these... quality over
quantity". Every record is judged with explosions on, standing on the ground (grass that craters
and obsidian that does not, both), with no TNT removed anywhere and at every kick angle; its SAFE
DROP HEIGHT (vjeux 13:31Z) is the fewest air blocks under it at which it never damages itself, and
a lower safe height beats a smaller machine (docs/circuits.md, VARIATIONS.md "On the ground").

## What every duper needs (the lower bound's parts)

A TNT is primed by power + an update, inside one piston push that also moves a copy of it:
1. the TNT block; 2. a pusher (a sticky piston: the cycle needs a pull too); 3. a frame the push
moves (slime or honey: the TNT and the update source must ride it); 4. the silent power: a dust
(the only power source that can reach a TNT inside a push, VARIATIONS "MOVABLE") on an immovable
block (obsidian: the dust must not ride); 5. the power whose connection changes mid-push: a
redstone block riding the frame; 6. the update: a dead coral fan riding the frame that pops when
its support's cell turns moving_piston. The 40M-iteration local search from 12 blocks found
nothing smaller than 12 and nothing faster than 200 TNT/min: 12 is the bound for the bare core.

## Why the ground is hard (measured, `mctas circuit tnt blasts`)

A TNT blast breaks nothing beyond about 7 blocks (a ray starts at <= 5.2 and loses 0.225 per
0.3-block step in air) and pushes entities up to 8 blocks; a TNT exploding inside water (or a
bubble column) breaks nothing at all. A machine making a TNT every 6 ticks has 13 burning at once:
flung 1 block per tick they pile up 10 to 20 blocks away on the ground, and every blast throws
some of the pile back within reach. One TNT at a time (a period longer than the 80-tick fuse) or
landing far away (a drop under the machine) is safe. A 60-minute explosion-screened random search
(the 15b launcher with a foot, plus 2 to 6 blocks of slime, obsidian, observers, pistons and
redstone blocks, every duper screened standing on both grounds at 6 kicks) found no fast duper safe
on the ground. The one that is (clk-misc 2026-10-20, below) holds the engine with a catalog clock:
a 14-tick period throws each TNT before the previous ones land, and they land 9 to 30 blocks away
(7.4 to 27 on unbreakable ground), always past the 6.9-block reach; at a 10-tick period some land
5.3 blocks away and about 1 game build in 30 fails.

## The TNT duper (vjeux 21:47Z: ONE world-best implementation per kind)

**tnt-clocked-22b** (archive): 85.7 TNT/min, 22 blocks, safe on the ground, every blast clear of
water and breaking ground blocks, robust over every kick on both grounds on redsim and in 16 game
builds (5 game runs of 16 builds, all robust). Two catalog parts: the 16-block engine
(tnt-triggered-launch-16b's, without its input block) and the 6-block clock
clk-obs-p14-lamp-obs-rb-stk-6b-b-0e0eb7, whose redstone block holds the engine's piston and steps
away for 4 ticks of every 14 (the engine dupes only on such a release: a pulse on its piston primes
nothing). Found by `mctas circuit tnt hold-scan` over every archived clock. Not precise: an aligner
holds one TNT at a time (its slime launches everything on it) and an apex burst needs the launch
at fuse 16-26, so a precise duper tops out near 20 TNT/min (the air burst, 58 blocks, 13.2/min, is
a step below).

The PRECISE variation (vjeux 00:14Z: "We still need another variation for precise drop location"):
**tnt-airburst** (now 45 blocks in 5x5x10 = 250, the judge's box of every cell the run fills, 22.2/min,
every TNT bit for bit at one point in mid-air, safe on the ground, game-robust; its own card "precise
air burst"; clk-rt's `compact --depth above:5` folded the 50-block version below, 8x6x11 = 528, first
to 8x6x8 = 384, then with the clock behind the engine to a build 4 blocks wide, 5x6x10 = 300, then with
the launcher's sticky piston one block higher, fed by an observer, 5x5x10 = 250). The 50-block step, rebuilt from
catalog parts after vjeux 01:00Z ("massively simplify"): the precise drop's torch-held engine with a
7-block catalog P54 clock (clk-obs-p54-lamp-obs-pis-stk-7b-7b1b02); the 58b's launcher geometry (the
x-stop is a fence post standing on the launcher slime, so every neighbor of the raised slime is air
and nothing is dragged back down); the next engine push, seen by an observer at the frame, goes down
four observers and a delay-2 repeater (a sticky piston answers a bare 2-tick observer pulse by
dropping its block) to the launcher: the launch comes 64 ticks after the TNT was lit (fuse 14), while
the next TNT is still landing two cells away. The 58b (slow engine, hairpin route) was its step.

The PRECISE DROP (vjeux 01:00Z, tree farms; 02:45Z "21 blocks is super far away"): **tnt-precise-drop**
now 41 blocks in a 6x5x8 box (240), 30/min, game-robust, bursting 10.0 blocks under the machine and
13.0 above the ground (`stand: 23`): a clk-rt `compact` squeeze folded the clock into the gap between
the engine and the channel. The 280-volume step (7x5x8, 8.3 below): clk-rt's `circuit tnt graft` moved the clock and its converter
(repeater, stone, torch) as one module so the torch stands under the engine's piston and powers it from
below. The 320-volume step (8x5x8, 9.3 below / 13.7 above) came from clk-rt's `circuit tnt compact
--depth 8..10` (the minimum-volume search, vjeux 02:47Z) on the 45-block version in 2 minutes: the route's last observer became a
delay-4 repeater (the trapdoor opens about 56 ticks after lighting, fuse 24) and the obsidian under the
engine piston went. The 45-block step: 8x5x8, 14.5 below and
8.5 above the ground (`stand: 23`; the card reads both). The 46-block step below (9x4x9, 21.4 below)
used a P33 clock; this one a 7-block catalog P40 clock (clk-obs-p40-lamp-obs-rb-stk-7b-30b78d) whose
moving redstone block stands 6 ticks in front of the converter's repeater (found by `hold-scan --pulse
--rb`; a longer stand lets the torch-held engine run free, 4 TNT). Closer bursts need a later release:
the next-push route takes 12 ticks, so D 8-10 needs P ≈ 45-48, and no catalog clock gives a ≤ 6-tick
pulse there (P46's redstone block stands 20+ ticks); P54 releases at fuse 16-18, 4-5 blocks under the
floor blocks. A block hung under the frame to shorten the route breaks the engine: the extended frame
already moves 12 blocks with the TNT block it grabs, the piston limit. Minimum-volume search (vjeux
02:47Z: "find the minimum volume to still do it") with clk-rt's `circuit tnt compact` / `graft`
(explore/timer), every row game-robust:

| design | box | volume | blocks | burst | rate |
|---|---|---|---|---|---|
| precise drop (P33 step) | 9x4x9 | 324 | 46 | 21.4 below | 36.4/min |
| precise drop (P40) | 8x5x8 | 320 | 45 | 14.5 below | 30/min |
| precise drop (compact, delay-4 route) | 8x5x8 | 320 | 44 | 9.3 below | 30/min |
| precise drop (graft: torch under the piston) | 7x5x8 | 280 | 44 | 8.3 below, 14.7 above ground | 30/min |
| precise drop (compact squeeze: clock between engine and channel) | 6x5x8 | 240 | 44 | 10.0 below, 13.0 above ground | 30/min |
| **precise drop (compact, 3 blocks fewer)** | **6x5x8** | **240** | **41** | **10.0 below, 13.0 above ground** | 30/min |
| precise air burst (catalog parts) | 8x6x11 | 528 | 50 | mid-air, ~9 above the channel | 22.2/min |
| precise air burst (compact) | 8x6x8 | 384 | 45 | mid-air | 22.2/min |
| precise air burst (compact: clock behind the engine, 4 wide; the frame sweeps a 5th column) | 5x6x10 | 300 | 45 | mid-air | 22.2/min |
| **precise air burst (compact: the launcher piston one higher, fed by an observer)** | **5x5x10** | **250** | **45** | mid-air (2048 runs of per-TNT kicks) | 22.2/min |

Squeezing the 240 by one face layer (y-4, x2, x7, z-4, z3): clk-rt, running.

The FAST duper at minimum volume (coordinator 2026-10-21): **tnt-clocked-22b stays the floor found, 4x4x4 =
64** (the judge's box; 22 blocks, 85.7/min). The 16b engine alone fills 3x4x4 = 48 (its frame sweeps one row
north, the foot keeps the slime frame off the ground, the launcher is a fourth layer); the clock takes the
fourth column. `circuit tnt compact --depth fast:85.7` (judged box, 100k moves) found nothing smaller and no
fewer blocks; squeezed into the engine's 3 columns (`--region`, any rate, 160k moves) nothing passes (the free
cells inside the engine's box that touch no slime in either frame position, (1,-1,3), (1,0,2-3), (1,-2,3),
(2,-2,2-3), (1,1,0), (3,1,0), do not hold a working clock); `circuit tnt hold-scan` over every catalog clock of
up to 12 blocks at P 8-24 (291 merges safe on the ground) gives no box under 64. The trade in the same box: 21
blocks at 75/min (designs/circuits/tracks/clk-misc/tnt-clocked-21b-p16.txt, a 5-block P16 clock whose observer
stands under the engine piston and does the foot's job; game-robust).
Earlier step, the 46-block version:
every TNT falls straight down from one point and goes off at one point in mid-air 21.4 blocks under
the machine's lowest block, bit for bit (the judge's flag: `precise: 52 TNT at one point at fuse 1,
in mid-air 21.4 blocks below the machine`, its own card "precise drop"). Catalog parts: the 16b
engine held by a redstone torch, a 7-block catalog clock (clk-obs-p33-lamp-obs-stk-7b-b-9e1f7d,
a 2-tick pulse every 33 ticks into a stone) whose pulse a delay-2 repeater stretches to 4 ticks into
the torch's block (the torch goes dark 4 ticks: one TNT per pulse; `circuit tnt hold-scan --pulse`
placed the clock); the air burst's channel with an observer x-stop and an iron trapdoor floor (the
pinned TNT rests over the trapdoor's cell alone); an observer watching the frame and four more carry
the next push to the x-stop observer, which powers the stone next to the trapdoor: it opens 43 ticks
after the TNT was lit (pinned by 41). It declares `stand: 24` (the judge's ground goes 24 blocks down,
never less than the safe drop, 5; the card says "stands 24 blocks above the ground"); at the safe drop
alone its TNT would land on the ground first and the craters move them. The first precise drop (58
blocks, 13.2/min, the air burst's slow engine and hairpin route) was its step; its "8.0 blocks below"
was the bottom of its own crater on the judge's bedrock, which `stand:` now avoids.

Under 30 blocks for either is out of reach with these parts (coordinator 01:00Z asked): engine 16 +
aligner 10 + launcher or trapdoor, and the one-clock "next push launches the previous TNT" needs a
hold at P 54-64 with a 3+ gt release, which no catalog clock gives (`circuit tnt hold-scan`: the
clk-rt uloops at P 26-52 and hopburn-p65 open for 2 gt only, which primes nothing); a converter (a
catalog 2-gt pulse stretched by a repeater into a sticky piston that moves the hold block) either
releases for pulse + travel (2 TNT) or, through a quasi-connected spot, locks after one TNT
(`hold-scan --pulse` finds the pulse cells). Floor estimate with a working converter: about 40-45.

The other kind, **tnt-duper-triggered**: tnt-triggered-launch-16b (16 blocks, safe on the ground; the
redstone block holds it, without it the engine runs free at 200/min, which needs 4 blocks of air under
it).

Exact game traces (redsim tick-identical every frame, the trace-compare gate): tnt-clocked-22b on the
judge's grass ground (420 gt, 30 TNT, about 1000 ground cells cratered, the dirt drops; recorded with
random_tick_speed 0: grass random ticks draw from the level random at positions chosen by the
level's unseeded randValue, which redsim does not model) and on obsidian ground (420 gt, default
random ticks): designs/clocks/tracks/clk-misc/redsim-tests/traces/tnt-clocked-22b-{grass,obsidian}.trace.

## Steps toward it (taken out of the archive 2026-10-20; their files stay where they were built)

| step | what it was | why it is not the one |
|---|---|---|
| tnt-launch-15b (designs/circuits/tracks/tnt/tnt-launch-15b.txt) | the bare engine running free: 200/min, 15 blocks | safe drop 5-8: on the ground its pile is within blast reach |
| tnt-slow-launch-30b | the engine on fly's crafter-loop clock + a 4-tick repeater hold: 13.2/min, 30 blocks | a bespoke hold; the catalog clock does it in 6 blocks at 6.5 times the rate |
| tnt-airburst (designs/circuits/tracks/clk-misc/tnt-airburst-58b.txt) | precise mid-air burst, 58 blocks, 13.2/min | back in the archive as the precise variation (above) |
| tnt-switch-launch-16b | the free engine with a lever: 200/min, safe drop 4 | same pile problem as the free engine |
| tnt-drop-16b | drops each TNT straight down: safe drop 25-31 | needs a pit |
| tileable | 4 copies, pitch 4 (explosions off) | judged on the ground: open |
| slime-free | none | a honey frame cannot fling; the TNT must drop clear: open |
