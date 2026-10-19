# Track m79s: finish m79q (7x9 = 63, u7-m10 mechanism, B pull-type) into a working door

## RESULT (08:00Z): m79s-sculk-63b-50.txt (= BEST.txt): 63b (7x9x1), 50 blocks, sculk, slime-free, flat
- `door verify --extra-offsets 28` WORKS (32 positions x 3 cycles), rotation-safe (32 positions each turn),
  open 43 / close 35; `door properties`: FULL seamless, dentless; redsim: hallway ok, frame 12/12.
- = satwire's m79q-sat (right side, bottom, B + A's last pull from TL's arrival) + the last ideal input
  (L1/L2) wired by SCULK, found by `door cosearch search` (assist sat-noA.ideal, `--palette
  all,sculk,calibrated,wool`, `--lock-pistons`, 64 threads, first finds after ~5 min; seeds: sat + cand/v4):
  calibrated sensor (-2,5) (facing north) hears the right side's t1 moves (activation t11, power 7);
  observer (-2,4) facing up -> WOOL (-2,3) -> L2 direct + L1 by QC: 3 taps per activation (t13 pull, t23
  push, t33 pull on open; push/pull/push on close); the sensor's window also lights the dust (-1,5) ->
  wool (0,5) -> B (held while active); wool (0,4)/(0,5)/(-2,3) occlude the re-arm echo (L2, L1, B, A).

## State (07:50Z)
- satwire's m79q-sat-topright (copied: sat-topright.txt, sat.ideal): its top-right does NOT drive B. B and
  A's last pull are driven by the still-ideal L1/L2 input: observer (-1,3) watches TL's wall cell (-1,2)
  -> stone (-1,4) -> dust (-1,5) -> lamp (0,5): B directly (L2's head arriving, tap a), A by QC through
  (1,5) (TL landing 4 gt later, tap b: B and A land in the same tick, B's push fails, A fires).
  sat-noA.ideal (A's ideal removed) WORKS in redsim: the ONLY input left is L1/L2: one 2 gt pulse per edge,
  open plan tick >= 6 (any later), close any. The pulse must be exactly ~2 gt on BOTH edges (open: the
  B/A coincidence needs TL to land 4 gt after the head; close: spit).
- The top-left is sealed from the lever: nothing it can see moves on open before L2. Only sound (sculk)
  crosses.

## Sculk bridge (cand/)
- s1/s2: sensor on (-2,3) (window into L2): open pulls TL but the long window kills the B/A coincidence;
  close re-pulls. Window = dead.
- s3/s4/v1/v2: sensor (-2,5) + observer (-2,4) up: 3 taps per activation, re-arm echo loops forever.
- v3/v4: calibrated (-2,5) facing west (back to the dust, so no power into the B/A chain) + observer
  (-2,4) + WOOL at (-2,3) (L2/L1 occluded), (0,5) (wool replaces the lamp as B's conductor: B occluded),
  (0,4) (A occluded): the echo is GONE (settles), sensor hears R0 at ~t1 (activation t11). Fails because
  calibrated taps are 10 apart: tap b's L2 push comes at P+10 and its B push at P+12 while A's last
  pull is still landing (needs P+13): one tick short again. A plain sensor (taps 30/10 apart: fits) has
  no legal cell: (-2,5) powers the dust (-1,5) (B for 30 gt), (-2,4) QC-powers L2, (0,4) powers A.
- Copper-bulb T flip-flop (1 tap per activation) needs sensor + bulb + comparator + observer: no room
  next to the fixed B/A chain ((-1,3),(-1,4),(-1,5),(0,5)); the comparator can only read through (-1,4).

## 7x8: m10top's m10f (A's hold + L0 ideal) with sculk (08:05Z-)
- Windows (ideal-rs): A's hold start plan 7..9 for 3..5 gt (start 7: 3-5, 8: 3-4, 9: 3); L0 any 0..40.
- Only 2 free cells ((-1,3), (3,3)): pockets k=3 with the sculk palette: no filling drives the hold or L0.
- cosearch search (sculk + wool palette, pistons locked, 56 threads, 25 min): running.
