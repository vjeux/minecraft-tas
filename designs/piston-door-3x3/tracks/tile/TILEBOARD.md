# Tileable 3x3 piston doors

Every design with a `tiling:` header on main and the `explore/*` branches, judged by `door verify --tile` (copies of one module, ONE lever, every judge check on every opening, 32 positions). Ranked by blocks per opening in a long row (blocks(N) - blocks(N-1)), then by period. Definitions: designs/piston-door-3x3/tracks/tile/NOTES.md.

## PERIODIC: an opening every P blocks along the wall

| # | blocks per opening | period | tested | open / close | rotations | source |
|---|---|---|---|---|---|---|
| 1 | 45 | 8 | 3 openings, 135 blocks | 64 / 60 | safe | `HEAD:designs/piston-door-3x3/tracks/tile/BEST-period.txt` |
| 2 | 46 | 8 | 3 openings, 138 blocks | 60 / 56 | safe | `HEAD:designs/piston-door-3x3/tracks/tile/archive/b38h-obs8-46b.txt` |
| 3 | 52 | 8 | 3 openings, 156 blocks | 40 / 36 | safe | `HEAD:designs/piston-door-3x3/tracks/tile/archive/b38h-bus8-52b.txt` |

## VPERIOD: a door every Q blocks up

| # | blocks per opening | period | tested | open / close | rotations | source |
|---|---|---|---|---|---|---|
| 1 | 49 | 9 | 3 openings, 147 blocks | 64 / 60 | safe | `HEAD:designs/piston-door-3x3/tracks/tile/BEST-vperiod.txt` |
| 2 | 50 | 9 | 3 openings, 150 blocks | 76 / 72 | safe | `HEAD:designs/piston-door-3x3/tracks/tile/archive/b38h-vobs9-50b.txt` |

## WIDE: openings side by side, one 3 x 3N opening

No working design yet.

## STACK: openings stacked, one 3N x 3 opening

No working design yet.

## Other translations

No working design yet.

