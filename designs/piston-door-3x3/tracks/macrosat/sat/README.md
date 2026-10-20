# macrosat SAT dumps (for c70b's mechmut bridge)

Every SAT instance of the sweeps so far, as `door macrosat sweep` wrote it (`<x0>_<y0>_<x1>_<y1>-h<H>.txt`:
the box corners, H parallel steps per edge), with the `--export` pairs where one was made
(`export/c-<box>-h<H>.txt` + `.ideal`; their `design` line names the path on devvm53196, edit it
before replaying). The first line of each dump gives the box, H and the flags in force; the grids
are y-down rows, x left to right from x0.

Glyphs: `.` air, `#` stone, `O` obsidian, `R` redstone block, pistons `e w u d` (facing east, west,
up, down; uppercase = sticky; `*` = extended), heads `> < ' ,` (normal) and `} { ^ v` (sticky).
**Wiring cells are `w` in these dumps, the same glyph as a retracted normal west piston**: a cell
that is `w` in every grid and never named in an action is wiring. Dumps from 2026-10-20 11:18Z on
print wiring as `%` and add a `wiring: (x,y) ...` line.

| directory | model (all: flat, solid, slime-free, full frame open, box = volume, parallel steps) |
|---|---|
| `7x7-nofeed`, `7x7-nofeed-sym` | no feed rules at all (pure mechanisms; `-sym` with mirror symmetry breaking) |
| `7x7-feed-v3`, `7x7-feed-sym` | the literal feed (any free in-plane cell / QC path / conductor next to a wiring cell; redstone blocks as edge sources), no island start |
| `7x7-private` | v3 + `--private` (a wiring cell feeds one piston cell); its export is cands/m7-a (works with ideal inputs in redsim and the game; fails island start) |
| `48` | v3 + island start; this is m86-a (works with ideal inputs, not wirable: see ../NOTES.md) |
| `v6` | + the lever as a level and the vertical-plane strong-power rule (8e2cced03 without --strict); 6x8 (-1..4, -2..5): fails `--strict` on its layout and its replay breaks |

None of these passes `--strict` (w86's wirability rules); the `--strict` sweeps have no SAT yet.
