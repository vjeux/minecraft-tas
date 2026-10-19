# The community 7x8 (SacredRedstone), reproduced from vjeux's screenshot (2026-10-19, c70b)

Source: /home/vjeux/agents/community-7x8.png (front view of the door plane, OPEN state, lever
off, the player in the hole). Files here: `open-a.txt` (the screenshot cell by cell + hidden dust
at (3,5) and (3,3), lever on top of the lime block), `closed-b.txt` (its state after one flip),
`closed-c.txt` (the other elevator rest state), `closed-rep4.txt` (c77's fill find), and the
candidate `../community-7x8.txt` (closed-rep4 with the real wall lever and the screenshot's wool).

## Reading (x -2..4 = columns, y -2..5 = rows; the door at x 0..2, y 0..2)

Textures were checked against the client jar (assets/minecraft/textures/block): orange-brown
diamond = redstone lamp (off), teal = cyan wool, lime = lime wool, purple = obsidian, the face at
(4,3) = dropper front (the "smile"; the dispenser has an "O").
- Observers come from a resource pack: the side texture carries red `<<` chevrons, the top/bottom
  texture a triangle plus a red mark. Vanilla `observer_top` (model `up` face, uv flipped) has its
  triangle apex toward the observer's BACK; the red mark sits at the triangle's base (the face).
  So `<<` = facing east (output west), apex up = facing down (output up), apex down = facing up.
  Every observer then watches something that moves and outputs into something it drives; the
  opposite convention would make (4,4) and (-2,4) output into a comparator / repeater from below.
- Repeaters: the fixed torch is at the output end (model: z 2..4 of a facing=south repeater, whose
  input is south); the delay torch sits 4/6/8/10 texels from it. (-2,5) d1 and (-1,5) d2 output
  west, (0,4) d2 outputs east. (4,5) has a rear torch PAIR (seen one behind the other) and a short
  front torch: a comparator, input west, output east (compare mode: unlit front torch).
- Wall lever handle UP = off (blockstate face=wall,powered=false uses the lever_on model, tilt +45
  deg, rotated x90 y180 -> handle up and out): the door rests CLOSED with the lever ON (as c79).
- All 11 pistons are sticky (the slime faces show from the low camera).
- The camera is below the top rows: block tops there are invisible, and the only empty non-door
  cells are (3,5) (on observer (3,4)) and (3,3) (on frame block (3,2)): hidden dust is possible
  there and nowhere else. A x10 dark boost shows both cells empty.

## Mechanism (open: lever off)
- Lever on the lime wool (2,5) holds TR's puller (2,4) and the elevator's top piston (1,5) (its
  head at (1,4), the lower piston at (1,3) at rest, quasi-powered through (1,5)); dust (3,5) ->
  comparator (4,5) (a 2-gt delay and a 1-tick filter) -> observer (4,4) -> dropper (4,3) (strongly
  powered: a conductor, and its `triggered` is what observer (4,2) watches) -> MR/BR pullers
  (4,1) direct + (4,0) by QC, one 2-gt pulse.
- Top-left: observer (0,5) watches the elevator top (1,5) -> repeaters (-1,5) d2, (-2,5) d1 (a
  dead end: the last repeater is only WATCHED) -> observer (-2,4) -> wool (-2,3) -> TL's puller
  (-2,2) + ML's (-2,1) by QC. TL landing in its park -> observer (-1,3) -> wool (-1,4) -> repeater
  (0,4) -> the elevator's lower piston (direct at (1,4), by QC through (1,4) at (1,3)).
- Right -> bottom: BR puller watcher (4,-1) -> lamp (4,-2) -> observers (3,-2), (2,-2) -> bottom
  puller (1,-2) up; its watcher (0,-2) -> wool (-1,-2) -> corner puller (-1,-1) (BM into the floor
  corner (0,-1)) and lamp (-2,-2) -> observer (-2,-1) -> BL's puller. Obsidian (3,-1) stops the
  corner puller from shoving the floor row east on close.
- The center goes down pushed through TM by the lower elevator piston (as c79); the elevator top
  re-pulses (dust (3,5) powered by observer (3,4)) to drop the lower piston back to (1,3) and
  later lift it again; TR's puller shares that power and wiggles TR down and up.
- It is a WIGGLE door: TL/ML get 3 puller pulses (in, out, in: t11, t24, t32 on open), BL 3, TR
  goes up, down, up; it opens in 42 gt and closes in 38.

## The unexplained cell (3,3)
- With hidden dust at (3,3) (cross or dot), the door closes from the screenshot but never reopens
  (redsim and the JVM agree): the dropper's 2-gt pulse gives ONE observer (3,4) re-pulse at t6;
  (1,5) and (2,4) re-pulse together (both on the lime block), so TR is spat back into the door and
  the elevator top moves 3 times (observer (0,5) fires 3 times, the TL/ML pullers twice).
- c77's `door fill --keep` (k <= 2 over 14 top cells of closed-b) finds only (3,3) = repeater d4
  (input from the dropper, output into TR's cell): two re-pulses at t14 and t22 (its on and off
  edges), TR goes down and back up, the left pullers get 3 pulses. 35 finds, all with it.
- `door cycle --vary` (this track) over every hidden-able option at (3,3)/(3,5) (air, dust cross /
  dot / lines, powered and activator rails) x the three repeaters' delays x comparator mode x
  observer (3,4) up/down: 0 cycle. A repeater at (3,3) would be plainly visible in the screenshot.
- Open question: a mechanics difference in the community's game version, or a misread elsewhere.

## Tool bug found
`door cycle` reported CYCLES for open-a: it only compared the rest state with the state after flip
1, and its periodicity test took `states.step_by(2)`, which are the OPEN states when the rest is
open. Fixed (cycle.rs): every flip must alternate open/closed from the rest state's kind, the
periodicity test uses the closed parity; `--vary x,y=a|b` judges every combination of a
transcription's uncertain cells.

## What our tools see (`door audit`, audit.md here)
- Blocks / ticks: 48 blocks with the (3,3) repeater (the community's own count is 47 or 48),
  open 42 / close 38, vs our e78 line: solid e78k-h5 41 blocks open 36 / close 19, sculk 37. It is
  record-eligible (c77: repeater input at the lever works at 32 positions; the e78 doors fail it),
  and c77's stone copy shrinks to 47 (explore/c77 tracks/c77/community-56b-47-elig.txt).
- Mechanism: 10 sticky pistons (3 left pull-type, 2 right pull-type, TR's puller lever-held at
  rest, the elevator top lever-held with the lower piston carried, the bottom puller up, the
  corner puller). IN mechspace with --ext-pullers (g3..g6), OUT of families v3 (the catalog's
  generator) and mechspace's default: TR exits through a puller EXTENDED at rest. The macro solver
  finds a clean schedule for these pistons (open 10 + close 8 events) and calls it WIRABLE in its
  own 7x8 box, so the community's wiggle choreography is not needed for this mechanism.
- Real moves outside every generator: on open TL, ML, BL and TR move 3 times (in, out, in), TM 5
  times (families.rs Gen::paths: 1-2 unit moves that never reverse). Searches that judge in redsim
  (cosearch, pockets, door fill) can still reach such wirings; satwire only wires the ideal plan it
  is given, so it can wire the mechanism but never this choreography.
- Wiring blocks: satwire and cosearch hold every block used (dropper, comparator, lamps, wool,
  lever on the front face, lever ON at rest); boxproof treats wool/concrete as stone (harmless).
- Fixed on this branch: cosearch's `quartz_like` (the FULL / frame check of every redsim search)
  listed 9 materials; a white-concrete door read as circuitry ("not FULL", frame 0/12). Now every
  quartz-like block redsim models natively counts (FULL, frame 12/12, as `door properties` says).
