# c77 handoff (2026-10-19 16:35Z)

## Results (all verified with `door verify --extra-offsets 28`, rotation-safe, FULL seamless)
- **e78st-56b-38-1s.txt** (= BEST.txt): 7x8 (56b), 38 blocks, sculk (ONE calibrated sensor), stone door, open 48 / close 15.
- e78st-56b-41-2s (2 sensors), e78st-56b-40 / -42 / -44 (3 sensors, open 51 / close 15; -44 archived as archive/e78st-56b-44-44b.txt).
- c074-64b-42.txt: 8x8 (64b), 42 blocks, sculk (archive/c77-c074-64b-42-42b.txt).

## How the stone 7x8 was made
1. e78-56b-45 (wool door, archived) -> wool door/frame turned to stone (breaks it) -> `door fill --keep` k=3 over 17 bottom/top cells re-tuned it (e78st-56b-44).
2. pockets3's row-3 top (observers (-1,3) E / (3,3) W on the ceiling heads) -> 42; minus normal piston (-2,-3) and obsidian (-2,-1) -> 40.
3. Solid conversion, one sensor at a time (`door fill --keep` with that sensor's cell pre-cleared, 42 non-sculk states incl. pistons, --gaps, k<=4):
   S0 (0,-3) -> observer W on a normal piston (-1,-3) facing up (head-pop relay) + obsidian (-2,-1) (41-2s);
   S4 (4,-3) -> lamp (3,-3) lit by S2 + observer (3,-2) facing down on it = K; S4 chain removed (38-1s).

## Open: the LAST sensor S2 (2,-3) facing north = the solid 7x8 (vjeux's ask)
- Seed: e78s/e78st38-s2.txt (the 38 with S2 cleared). S2's jobs: B' power [8,18) (B' moves at t17 open / t13 close when the (0,-3) observer updates it), the lamp for K (10/22/34/46), gate observer (2,-2) facing down -> F2 taps (A2/A3 through BM, open only), and an OPEN-ONLY re-wake at t32 (hears A' re-firing at t30) holding B' [32,42) (= B2).
- Tried: k<=3 over 10 and 13 cells (0). pockets3 (1d42739b, devvm49255) runs k=4 (28 states) and k=5 (24 states, 2.0G) over (2,-3) (2,-2) (3,-3) (3,-2) (4,-3) (4,-2) (4,-1) (-2,-3) (-2,-2) (0,-3).
- ideal-rs plans are unreliable for this bottom: the doors depend on same-tick event order (K's pull vs A''s push at t12); the plan's torch inputs reorder them. Use real-wiring fills.

## Running processes (mine)
- door fill --keep k<=3 from e78st-56b-38-1s (shrink to 37), runs/e78st38-keep3.log.

## Files
- e78s/: seeds and plans (kb1*, e78st*, s0b/s4k/s2all ideals); runs/: fill logs (big logs summarized, raw in /tmp).
