# q78 / e78 HANDOFF (e78k, session da291dc5, devvm47239) — the 7x8 = 56 doors

Board partial: `minecraft-tas/board/partials/e78.md` (Space memory). Everything below is on branch explore/q78
under `designs/piston-door-3x3/tracks/q78/`; details and timings in `e78s/NOTES.md`.

## Results (all volume 56, flat, slime-free, rotation-safe, `door verify --extra-offsets 28` WORKS)
- SCULK: `e78k/p3.txt` = the first 7x8 (pockets3's 3-change fill of e78k's r1; coordinator archive
  `archive/e78-56b-45-45b`), later shrunk by pockets3 (40) and c77 (stone e78st lines, 38 with one sensor).
- SOLID (coordinator goal from 15:33Z), records need FULL seamless = frame 12/12 (the static frame stones
  (-1,2) (3,2) (0,-1) must stay):
  | file | blocks | seamless | open/close | note |
  |---|---|---|---|---|
  | `e78s/archive/e78k-h2-45b.txt` | 45 | FULL | 44/15 | THE FIRST SOLID 7x8 (coordinator-verified + archived on main) |
  | `e78s/archive/e78k-h4-42b.txt` | 42 | FULL | 36/19 | top-left watcher (-1,3) E + lamp-pushing loop |
  | `e78s/archive/e78k-h5-41b.txt` (= BEST.txt) | 41 | FULL | 36/19 | right bottom = dropper chain |
  | `e78s/archive/e78k-h6-42b.txt` | 42 | FULL | 29/15 | pockets3's e78s-56b-43 + the h5 right bottom |
  | `e78s/archive/e78k-h3-38b.txt` | 38 | QUART (frame 9/12) | 36/19 | NOT a record: uses the frame cells |

## How the solid door works (h2; h4/h5 are the same scheme)
c77's stone e78st skeleton without sensors. Open: A1 at t7 (lamp observer (0,-2) on the lamp lit at t5), K
(right pullers delayed 2 gt by observer (3,3) W -> note (4,3) -> observer (4,2) up; BR watcher (4,-1) drives K)
pulls BM into F2 at t13. B1 is fed from the RIGHT: observer (2,-3) facing EAST on a note block (3,-3) (h5: a
dropper) whose state follows K's driver -> B' 2-gt spit at t13; the gate observer (2,-2) facing down watches (2,-3),
not B', so exactly one gate pulse 2 gt after the B' pulse = A2 at A's landing in F1. The late B2/A3 come from a
LOOP: observer (0,-3) facing EAST on B' re-fires the left side (h2: re-fires BL's puller so the BL-park watcher
keeps the lamp lit; h4+: a sticky piston (-1,-3) pushes the lamp up and back), so the lamp observer (0,-2)
QC-fires B' while A' is up at F1 (B2) and its last edge fires A' (A3).

## How it was found (reuse this)
`door ideal-rs <design>.ideal --vary="x,y=keep|state|..." ... --ok-only --threads 64` with an .ideal that has
only the lever uses (no ideal inputs): ideal-rs is then a fast 4-position redsim screen (~100k-250k
combos/s on 64 threads), so 6 cells x 11-13 states (2-5M combos) take 15-60 s. h1 (= rr2 + a lamp-relight loop,
failing only at the final lamp edge) -> 4-cell --vary -> h2; rounds of 6-cell --vary on each group -> h4/h5.
Always verify the winners (`door verify --extra-offsets 28`) and check `door properties` for FULL.

## Searched without a gain from h5 (41)
My --vary rounds: every 6-cell group (bottom-left, A1+lamp, bottom-right, top-left, top-right, mixed) and a
7-cell A1/lamp/loop group (36M): nothing below 41. `door minimize --moves all --plateau --depth 2`: 41.
shrink (7f3f12cf): clear-and-refill k<=5 on the B'-feed cells, bottom-right 7, bottom-left 8: 0. pockets3
(1d42739b): --keep <= 3 over 25 wiring cells: 0; exactly-4/5 runs were still going at 17:40Z.

## Next ideas
- Speed: h6 is 29/15 at 42; `door minimize --objective total` on h5/h6 (started 17:36Z, logs in e78s/min/).
- Fewer blocks needs a structural change: the loop costs 3 blocks (observer (0,-3), sticky (-1,-3) and the
  extra lamp lighter (-2,-2)+(-2,-1) instead of (-1,-1)); a loop that re-uses the BL-park watcher (-1,-1)
  would save one. K + B1 share (4,-1) (4,-3) (3,-3); the top-right driver (3 blocks) is also K's 2-gt delay.
- Older sculk-era notes (r1/r2/nb, satwire3's UNSAT core) are in git history of this file (before 17:40Z).
