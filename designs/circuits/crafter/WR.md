# Auto-crafters (study 19, `circuit: crafter RESULT`)

A crafter with a set recipe is fed its ingredients from chests. It crafts whenever a complete set
is in its grid, and every result goes to an output chest (coordinator 2026-10-21). The judge:
[docs/circuits.md](../../../docs/circuits.md) ("crafter RESULT"),
[crate::crafter](../../../crates/circuit/src/crafter.rs).

The main board is the PURE supply: only the recipe's ingredients arrive, as players usually feed a
crafter from a sorted line. They come as full and partial stacks at hopper speed. A partial set
must wait without jamming, the craft counts must be exact, and the build must be intrusion-safe
and robust (16 positions x 4 rotations). A mixed supply (other items mixed in: "with a filter")
is a later variant.

## Board

| class | recipe | design | blocks | box (volume) | crafts/min | status |
|---|---|---|---|---|---|---|
| 3x3 | iron block (9 iron ingots) | **ironblock-v24** (archive) | 16 | **4x2x3 (24)** | 16.6 | game robust, sealed |
| 2x2 | crafting table (4 planks) | **table-v24** (archive) | 16 | **4x2x3 (24)** | 37.5 | game robust, sealed |
| 2x2 | sticks (2 planks) | **stick-v24** (archive) | 16 | **4x2x3 (24)** | 75.5 | game robust, sealed |
| multi | TNT (5 gunpowder + 4 sand) | none yet | | | | open (below) |

## Mechanics that set the design (read in the 26.3 source, and in redsim's model)

- **Filling.** A hopper puts each item into the crafter's first EMPTY enabled slot
  (`CrafterBlockEntity.canPlaceItem`). Only once every enabled slot holds something do stacks
  grow, and then evenly: a slot takes one more only if no later slot of the same item holds
  fewer. A different item cannot go into a filled slot at all.
- **The comparator** reads the number of slots that are filled or disabled (0 to 9). A
  disabled slot counts as filled. So a 2x2 recipe with five disabled slots also reads 9 when its
  grid is complete.
- **Crafting.** A crafter crafts on a RISING edge of power, 4 game ticks later. It crafts whatever
  the grid makes then, and every filled slot loses one item. The result goes into the container
  in front, or out into the world as an item entity.
- **A partly filled grid crafts the wrong thing.** One iron ingot makes nine nuggets, two make a
  heavy pressure plate. One plank makes a button, two in a row a pressure plate. So the trigger
  must fire ONLY when the comparator reads 9: a pulse on every new item (the observer trick)
  would craft nuggets. The partial grids of TNT's pattern craft nothing (checked against the
  game's recipes, redsim `auto_crafter_grids`), so a TNT crafter may pulse on every item.
- **The threshold.** Signal 9 versus 8 needs a cut. Dust decay would need nine dust. Instead,
  a comparator in compare mode gets a constant 9 at its side, from a second comparator reading
  a crafter whose nine slots are all disabled; it then outputs 9 only at 9.
- Dust connects to a comparator from any side, so a dust beside the comparator's side never
  points away from it. The signal turns the corner into the stone beside the crafter through a
  repeater instead.

## The record layout (24)

```
y=0 (z = -1, 0, 1 from top; x = -1..2)      y=-1
  . S R r                                     . . # #
  H M K r                                     . o # #      o = the output chest (judge)
  C J D .                                     . # # .
```
- H is the hopper, under the supply chest (judge, at y=1), feeding the crafter M.
- K reads M. J reads C (a crafter, nine slots disabled) into the dust D at K's side.
- K's 9 runs through two dust r into the repeater R, which powers the stone S beside M.
- M crafts into the chest below it.
- The 2x2 recipes disable M's other slots (crafting table: 2, 5, 6, 7, 8; sticks: all but 0 and 3).

## Multi-ingredient (TNT): what it needs

TNT's pattern alternates gunpowder and sand in slot order (G S G / S G S / G S G), and a crafter
fills its empty slots in order. So the items must ARRIVE alternating: G, S, G, S, ..., G.

A template (one of each item left in every slot so that items steer themselves) does not help.
With a template the comparator reads 9 all the time, and nothing shows when a full second layer
is in.

The plan is two lockable hoppers, one per ingredient (per-ingredient supply chests,
`items.gunpowder` / `items.sand`):
- An observer sees every change of M's count and pulses M. Partial TNT grids craft nothing.
- The same pulse toggles a copper bulb; its comparator decides whose turn it is.
- The count changes ten times per set (nine items, one craft). That is even, but between the
  ninth item and the craft's toggle it would be sand's turn. So both hoppers are locked while
  the count is 9, released 4 gt late, exactly when the craft's toggle lands.
