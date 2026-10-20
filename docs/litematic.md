# Litematica schematics (`mctas litematic`)

Any design of the repository (a door, a clock, a circuit, a generated family member) becomes a
`.litematic`, the schematic format of [Litematica](https://github.com/maruohon/litematica), to
paste or build in a world. REDSTONEPEDIA offers one on every build page and for every family
member ("Download .litematic"). Code: `crates/litematic` (the writer, a reader, binary NBT and
SNBT).

| command | what |
|---|---|
| `mctas litematic export <designs>... [-o <file.litematic or folder>] [--description T] [--check]` | write each design's schematic (default: next to the design, `<stem>.litematic`); `--check` reads each file back and compares it with the design |
| `mctas litematic read <file> [--design] [--dump]` | metadata and regions of any `.litematic` (also Litematica's own); `--design` prints each region as a design text, `--dump` the whole NBT tree as SNBT; says whether the stored BlockStates re-pack bit for bit |
| `mctas litematic check <designs>...` | round trip in memory: export, read back, compare block for block (states, block entities, entities, sizes, counts); exit 1 on any difference |

## The file

What Litematica for Minecraft 26.3 writes and reads (its source, github.com/sakura-ryoko/litematica
branch 26.3: `LitematicaSchematic.writeToData`, `SchematicMetadata.writeData`,
`LitematicaBlockStateContainer`, `LitematicaBitArray`, malilib's `DataTypeUtils`): a gzipped NBT
compound with an empty root name,

- `MinecraftDataVersion` 5023 (26.3: `world_version` of the server jar's version.json),
  `Version` 7, `SubVersion` 1;
- `Metadata`: `Name` (the design's archive name), `Author` "minecraft-tas", `Description` (how it
  works, how to start it, its REDSTONEPEDIA page), `RegionCount` 1, `TotalVolume` and
  `TotalBlocks` (ints), `TimeCreated` / `TimeModified` (longs, ms), `EnclosingSize` {x, y, z};
- `Regions`: one region named after the design: `BlockStatePalette` (`{Name}` or
  `{Name, Properties: {key: "value"}}`, air first, then the states in scan order), `BlockStates`
  (each cell's palette index on `max(2, bits(palette size - 1))` bits, packed little-endian into
  longs, entries spanning two longs, cell index `(y * sizeZ + z) * sizeX + x`), `TileEntities`
  (the design's `nbt:` lines: `{id, x, y, z, ...}` relative to the region), `PendingBlockTicks`
  and `PendingFluidTicks` (empty), `Entities` (the `entity:` lines: `{..., Pos: [x, y, z], id}`),
  `Position` {0, 0, 0} and `Size` (the box of the blocks, block entities and entity cells).

A design is placed as it is at rest: a kicked clock needs its kick block re-placed after pasting
(the description says which), a circuit's input cell is air (put a lever or a button there).

## How it was checked

- Every archived design (doors, clocks, circuits, calibration circuits) and every generated
  family member, 6334 files: `mctas litematic check` passes (export, read back, equal block for
  block, block entities, entities, sizes, counts).
- Five schematics written by Litematica itself (from the test files of rustmatica and litemapy:
  data versions 2730 to 3465, format versions 5 and 6, negative region sizes, 96-state palettes,
  tile entities, entities) read correctly: the decoded block counts equal their own `TotalBlocks`,
  and re-packing the decoded cells gives their `BlockStates` back bit for bit (`read` prints
  "BlockStates re-packed: identical"). Their keys, tag types and empty lists (element type End)
  match what the writer produces.
- The repository's independent NBT reader (`mctas::vanilla::nbt`) parses the written files.
- `mctas site smoke` fetches a door's `.litematic` from the built site and compares it with the
  design file next to it.
