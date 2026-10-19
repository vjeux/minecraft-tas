# t78 on c070x (ceil's 8x7, sculk wiring attempt), 12:39Z-13:25Z, handed to c70

Files: c070x-orig.txt (ceil wip/c070x.txt), c070x-b1.txt (+ E1/E2 real), c070x-b1m.txt (+ M observer), *.ideal, pockets/ (pocket fillings), pockets-k4-sculk.log.

## Results
1. Joint re-timing (old 'between ticks' input phase): with the real E1 chain, U at u and M at EXACTLY u+3 work for u = 9..20 (c070x-um.ideal sweep); M is tied to U, not to a fixed tick.
2. Pockets --gaps k 4 with sculk/calibrated/wool (E1 assisted, E1 chain cleared): true partial with only U and M assisted (38 blocks).
   - top-left, 3 blocks, drives E1 AND E2 on both edges: plain sensor (-2,4) [lever d 3.16 -> t3] -> waxed copper bulb (-2,3) -> observer (-2,2) up into E. The bulb turns each sensor wake into ONE tap: E1 at t5, E2 from the re-arm wake. Frees (-1,3) and (-1,4). = c070x-b1.txt.
   - BL: pocket0-set0 (observer (-1,-2) west + sticky piston (-2,-1) down, 2 blocks); BM: pocket2-set0 (3 blocks).
   - NO filling drives U (u=13) or M (exact u+3) with k <= 4, sculk included.
3. M through a real observer: c070x-b1m.txt adds an observer (-1,3) facing UP into the static frame stone (-1,2) (E at (-1,1) direct; E at (-2,1) only by QC through (-2,2), never updated by it). Old phase: works when the plan changes (-1,4) at g = u+2 (u 7..24), so M = 'something at (-1,4) changes 2 ticks after U's input'.
4. NEW tile-tick input phase (main de6ecbce): c070x's own plan (ceil's ideal, any u/m) and b1/b1m fail. The watcher's D push and U's retraction land in the same tick: D shoves E AND ML (still at (-1,1)) east. With a REAL observer driving U, U's off-tick is scheduled before the watcher's tick (scheduled at U's extension), so U may retract first. Only real wiring of U can tell; the ideal input can't.
5. c77's trick (plain sensor next to D to hold it): a sensor at (-3,2) powers D directly, so D pushes E at t6 before E1 (0 of 756 u/m combos). Any sensor that QCs D through (-3,2) also updates D when it wakes.
6. --vary search for U's real driver on b1m (sensors/lamp/bulb/observer/wool at (-3,-2),(-2,-2),(-1,-2),(-3,-1),(-2,-1), M by glass at g 1..30): 0 of 12,960.

## Ideas not tried
- U from a plain sensor out of the lever's range ((-3,-2): lever at 8.06 > 8; hears the ceiling at t1, d 6.7 -> t7) via observer (-2,-2) west -> wool (-1,-2), with the 2nd/3rd taps blocked: U's push line (-1,0)..(-1,4) jammed by an immovable block entity at (-1,4) (a sensor) when (-1,1) is occupied.
- (-1,4) = a calibrated sensor waking exactly 2 ticks after U's observer fires (its observer (-1,3) then gives M), e.g. both hearing one event at distances differing by the right amount, with a frequency filter (back signal 10 = extensions only: the open's lever-off and ceiling retraction are 9).
