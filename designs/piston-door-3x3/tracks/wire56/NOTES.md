# Track wire56: a flat, slime-free, FULL seamless + hallway door in a 7x8 box (56b)

Goal (coordinator, 2026-10-18 22:37Z): 1-wide (flat), slime-free, FULL seamless + hallway-compatible
door in a 7 wide x 8 tall box = volume 56 (record 72b: e89b, 8x9x1, explore/flat-seam). Lead: agent
families' v1 catalog (6b2d389) entry 7x8-028 (= v2 7x8-013), which `door topology` calls wirable
only with MEMORY links (waxed copper bulb T flip-flop + comparator).

## 1. The mechanism: 7x8-028 (mech/m028.txt), box x=-2..4, y=-2..5
`SE(-2,0) SE(-2,1) SD(0,4) SU(1,-2) SD(1,4) SD(1,5) SU(2,-2) SW(3,3) SW(4,1) SW(4,2)`
- Left pair (-2,0)/(-2,1) pull BL/ML into (-1,0)/(-1,1); TL pulled up into (0,3) by (0,4); BM and BR
  pulled down by (1,-2)/(2,-2); MR/TR pulled right by (4,1)/(4,2).
- Core: P4 = (1,4) (sticky, down), P5 = (1,5) (sticky, down, the elevator), PT = (3,3) (sticky, west,
  ring corner). Sym train (13 pulses, the same on both edges): s1 P4, s2 PT, s3 P5, s4 P4 (lifted to
  (1,3)), s5 P5, s6 P4, then the 7 toggles.
  - open: s1 pulls T up into (1,3); s2 pulls T aside into (2,3); s3 pushes P4 down into (1,3) (spit);
    s4 (lifted P4) pulls C up into (1,2); s5 pulls P4 back up to (1,4); s6 pulls C into (1,3).
  - close: s1 pushes C from (1,3) to (1,2); s2 pushes T from (2,3) to (1,3); s3 pushes P4, T, C down
    one (T and C home); s4 (lifted P4) pushes T and C one further down (T (1,1), C (1,0)); s5 pulls
    P4 back up; s6 is a no-op (reach-through pull of an empty cell); BM's push (1,-2) then pushes BM,
    C, T up one: all home. So on close BM's toggle must come after s4.
- `door families --check mech/m028.txt --verify --extra-offsets 28`: oc and sym both WORK with ideal
  inputs at 32 positions (mech/ideal/m028-{oc,sym}.{txt,ideal}).

## 2. Topology (door topology, memory links)
Regions of free cells (stone hops merged): K0 bottom-left (5 cells: feeds (-2,0), (1,-2)), K1
top-left (8: feeds (-2,0) by QC, (-2,1), (0,4), (1,5)), K2 top-right (7: feeds (1,4) at rest AND
lifted (via (2,4), QC through (1,4)), (1,5), (3,3), (4,1) (QC), (4,2)), K3 bottom-right (5: feeds
(2,-2), (4,1)). Links: (-2,0) watched from (-2,-1) (K1 -> K0, 1 change), (4,1) watched from (4,0)
(K2 -> K3, 1 change), P5 watched from (0,5) / (2,5) (K1 <-> K2, 2 changes per edge: even).

Why the lever must be in K2 (not K1, which topology prints): K2 must fire s1 (P4) and s2 (PT)
BEFORE P5 first moves (s3), and P5 is the only thing K2 can see of K1. K1 cannot fire P4 alone: a
source at (0,5) powering P5 also quasi-powers P4, but it updates P5 first (diode/observer output
updates the front block, then its neighbours), so P5 moves first and P4's queued event is dropped
(its cell became a moving block). So K2 holds the lever and runs the core + right side; K1 (top-left)
needs ONE pulse per edge for (0,4) and the left pair from P5's two moves (even): the memory link.

## 3. The memory link: what the bulb + comparator really gives (gadget in progress)
(see designs/gadgets/memory-link*.drive once written)
