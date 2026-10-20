# Track pulse (contraption study 1: pulse circuits)

Branch `explore/clk-obs` (shared with clk-obs), crate `crates/circuit-pulse`
(`mctas circuit pulse screen|census|family|unarchive`), designs in this folder, archive in
`designs/circuits/pulse/archive/` (the judge: `mctas circuit sim|verify|archive`, docs/circuits.md).

## What is archived (all robust in the game, 4 positions x 4 rotations)

| kind | L (gt) | blocks | mechanism |
|---|---|---|---|
| rising-edge | 2 | 2 | sticky piston + observer: the observer is pushed INTO the output cell and fires 2 gt after it lands; pulled back on the falling edge, it fires where the output cannot see it (min-pulse 4) |
| rising-edge | 2 | 4 | BULB EDGE: the input toggles a copper bulb on every rising edge (even 1 gt), a comparator reads it (one change per rising edge), an observer turns each change into a 2 gt pulse (min-pulse 1) |
| rising-edge | 4, 6, 8 | 6 | the bulb edge + a repeater (min-pulse 1) |
| rising-edge | 18 + 16n | 19 + 4n | card `pulse-oneshot-bulb-rise` (and `-ext` as an extender): the bulb edge triggers the bulb one-shot (min-pulse 1) |
| rising-edge | 4, 6, 8 | 4 | the same with a sideways observer whose back faces a repeater only when pushed; the repeater stretches the 2 gt pulse to L |
| rising-edge | 6..40 (every even L) | 17-25 | BULB ONE-SHOT: the pushed observer lights a copper bulb; a comparator reads the bulb into a U of repeaters back into the bulb, which turns it off a loop later (a bulb toggles only on rising edges of its power, so the loop's own falling edge does nothing) |
| rising-edge | 18 + 16n | 17 + 4n | card `pulse-oneshot-rise` (n 0..10) |
| falling-edge | 2 | 3 | observer between two sticky pistons (census; another layout: min-pulse 2, rotation-dependent) |
| falling-edge | 4, 6, 8 | 4 | the sideways observer faces the repeater when back at rest (min-pulse 6) |
| falling-edge | 6..40, 18 + 16n | 17-25, card | bulb one-shot fed from the rest position (`pulse-oneshot-fall`) |
| dual-edge | 2 | 1 | an observer watching the input (min-pulse 4) |
| dual-edge | 2 | 2 | an observer watching a lamp the input lights: the lamp goes out 4 gt late, so even a 1 gt input gives two pulses (min-pulse 1) |
| dual-edge | 4, 6, 8 | 3 | observer + repeater |
| dual-edge | 18, 34 | 23, 27 | locked bulb one-shot: a comparator reading the bulb locks the trigger repeater, so short inputs cannot latch it (min-pulse 40 / 80) |
| dual-edge | 30 + 16n | 32 + 4n | card `pulse-dual-edge-reset` (n 0..10, L up to 190): the locked one-shot whose loop ends in a second bulb + comparator + observer, so the bulb gets one 2 gt reset pulse per loop instead of staying powered: min-pulse about L + 10 |
| dual-edge | 4, 6, 8 | 4 | lamp + observer + repeater: the lamp's 4 gt fall separates the edges of short inputs (min-pulse 4 / 8 / 15) |
| pulse-limiter | 2 | 2 | sticky piston + observer (another layout) |
| pulse-limiter | 4..38 (every even L) | 10-18 | SUBTRACT U: a subtract comparator's rear sees the input at once, its side gets it back through a U of repeaters: rear - side is on until the side catches up (min-pulse 2) |
| pulse-limiter | 16 + 16n | 10 + 4n | card `pulse-limiter-u4` (n 0..10) |
| pulse-extender | 2, 4, 6, 8 | 2 | a repeater on stone stretches short pulses to its delay |
| pulse-extender | 10..40, 18 + 16n | 17-25, card | the rising-edge bulb one-shot (`pulse-oneshot-ext`) |

## Tools

* `screen <designs>`: a redsim screen that reads EVERY cell around a design with the judge's output
  levels (dust power; repeater, observer, torch own output; comparator strength; else the best
  signal into the cell) over the judge's input lengths, and lists every kind it satisfies (a
  constant-length monostable is a rising-edge detector, a limiter and an extender at once) with
  its min-pulse. Delays agree with the judge's frames. Like the judge it checks the outputs only:
  an inner toggle (a bulb flipping on every rising edge) is allowed (a first version required the
  whole state back after every pulse and missed the bulb edge detectors).
* `census --k K --palette ...`: every polycube of K blocks + an input cell (any cell of it), every
  output cell; a quick 40 gt probe drops inert designs; keeps the best of each (kind, L) (min-pulse
  then delay). k = 3 with the full palette: 1.4 min on 40 threads.
* `family <name> --n N --out F` (+ `--card-max-n K` for a card), `limiter-for --l-range`,
  `oneshot-for --l-range --kind`, `oneshot-mode --mode rise|fall|dual`, `dual-lock`.
* `unarchive <files>`: remove archived designs (file + verdict and caption rows), e.g. the singles a
  card covers (circuit archive skips a card whose base structure is already archived).

## Open

* Odd L: every design so far has even L (observers, repeaters, comparators work in 2 gt units). A
  redstone block that LANDS (block-entity phase) next to the output and is pushed away the next
  tick by a piston it powers would give a 1 gt pulse (L = 1 + 2d with a repeater on the way).
* Min-pulse 1 falling-edge detectors (the best has min-pulse 2): a bulb needs a rising edge, so
  the input must be inverted first, and torches and comparators miss 1 gt inputs.
* The 4-block census (running) for smaller designs per (kind, L).
