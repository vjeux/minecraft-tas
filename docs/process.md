# How a component gets on the site

On 2026-10-21 vjeux judged a day of agent-made circuits "completely useless". Each one optimised
a number nobody asked for: blocks per plant gave a 24-block melon farm making about 50 slices an
hour; exact furnace fairness gave a 149-block, clock-driven row for two furnaces ("Is it an
elaborate joke?"); one lamp per segment gave a display that does not read as digits. Since then
every component goes through the steps below. No new kind, metric or judge rule skips them.

No internet and no one else's designs (vjeux, 2026-10-21): every bar below is something we build
or work out ourselves from the game's mechanics.

## 0. The backlog comes from player needs

`designs/briefs/BACKLOG.md` lists what players need from redstone in the game (food, building
materials, smelting, storage and sorting, getting around, doors and access, ...), the build that
serves each need, what the site has for it, and how far that is from the need's ceiling. Topics
come from its top, never from what an agent finds interesting.

## 1. Design doc first (gate G1)

Before anything is built, write `designs/briefs/<topic>.md` (vjeux: "do a deep dive into the code
to understand the mechanics, what are all the possible tradeoffs, brainstorm different layouts and
constraints... then decide"):

- **Use:** what a player builds it for, in one sentence.
- **The number:** the one or two numbers a player would compare (a farm: items per hour per block
  for a full-size farm; a smelter: minutes to smelt a chest of ore, and blocks).
- **The mechanics, from the game code:** every rule the build relies on, with the class and method
  it comes from.
- **The trade-offs:** what each choice costs against what (blocks, rate, materials, tileability,
  robustness).
- **Four or five genuinely different layouts,** the obvious design among them, each with its
  estimated blocks and number, its percentage of the ceiling, and the constraints it brings.
- **The decision:** which layout to build or search from, and why.
- **Must-haves**, only from real use: inputs and outputs where a player reaches them, survival
  materials, tileable when it is meant to be tiled, safe from stray items when its timing depends
  on items.
- **Not goals:** what the judge must not demand (exact fairness, accounting no player sees).
- **The obvious design's measured numbers** (step 2) and **the ceiling** (step 3).

The reviewer sends back a thin doc (no code citations, fewer than four real alternatives, numbers
not estimated, a decision not argued). It and the coordinator approve the doc before any search
starts.

## 2. The obvious design (gate G2)

Build the straightforward version anyone would derive from the mechanics, without searching: for
a smelter, a chest feeding a hopper line over the furnaces, a fuel line and an output chest; for a
farm, an observer and a piston per plant over a collection line; for a display, a lamp per cell
with each corner lit by its segments. The judge must pass it: a judge that fails the obvious
design is demanding something no player asked for, and gets fixed. Its numbers are the bar.

## 3. The ceiling

Work out the game's own limit for the number from the mechanics (a furnace smelts one item every
10 s; a crop's best growth rate from its random-tick chances; ...). Report every result as a
percentage of it.

## 4. Search against the bar

Searches only try to beat the obvious design on the brief's number, toward the ceiling, keeping
every must-have. A rule found necessary on the way goes back through the brief (G1) before it
changes the judge.

## 5. Review before publishing (gate G3)

The reviewer films the design's page (`mctas site smoke --engine webkit --page <path>`) and judges
from the picture and first principles: is its size reasonable for what it does, does it clearly
beat the obvious design, would a player build it? The coordinator looks at the picture too. A
design that does not clearly beat the obvious design is not published as the pick; when nothing
clearly beats it, the pick is whichever a player would rather build, which may be the obvious
design itself, verified, with an honest caption.

## 6. One topic per agent

Each agent works one topic, time-boxed, reports one line each time it beats the bar, and goes idle
when done. Nobody is kept busy for its own sake.
