# Track m79s: finish m79q (7x9 = 63, u7-m10 mechanism, B pull-type) into a working door

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
