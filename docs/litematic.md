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
| `mctas litematic verify-ingame <designs or folders>... [--world server\|single\|both] [--nbt none\|place_data_modify\|place_clone] [--dedupe true\|false] [--timeout S]` | LITEMATICA ITSELF pastes each design's file in the real client, then the world is compared with the design (below); exit 1 on any difference |

## The file

What Litematica for Minecraft 26.3 writes and reads (its source, github.com/sakura-ryoko/litematica
branch 26.3: `LitematicaSchematic.writeToData`, `SchematicMetadata.writeData`,
`LitematicaBlockStateContainer`, `LitematicaBitArray`, malilib's `DataTypeUtils`): a gzipped NBT
compound with an empty root name,

- `MinecraftDataVersion` 5023 (26.3: `world_version` of the server jar's version.json),
  `Version` 7, `SubVersion` 1;
- `Metadata`: `Name` (the design's archive name), `Author` "redstonepedia", `Description` (how it
  works, how to start it, its REDSTONEPEDIA page), `RegionCount` 1, `TotalVolume` and
  `TotalBlocks` (ints), `TimeCreated` / `TimeModified` (longs, ms), `EnclosingSize` {x, y, z};
- `Regions`: one region named after the design: `BlockStatePalette` (`{Name}` or
  `{Name, Properties: {key: "value"}}`, air first, then the states in scan order), `BlockStates`
  (each cell's palette index on `max(2, bits(palette size - 1))` bits, packed little-endian into
  longs, entries spanning two longs, cell index `(y * sizeZ + z) * sizeX + x`), `TileEntities`
  (the design's `nbt:` lines: `{id, x, y, z, ...}` relative to the region), `PendingBlockTicks`
  and `PendingFluidTicks` (empty), `Entities` (the `entity:` lines: `{..., Pos: [x, y, z], id}`
  plus what `Entity.saveWithoutId` always writes when the line leaves it out: `Motion` [0, 0, 0],
  `Rotation` [0f, 0f] and a `UUID` derived from the design's name and the entity's index; see
  below for why), `Position` {0, 0, 0} and `Size` (the box of the blocks, block entities and
  entity cells).

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

## In the real client (`verify-ingame`)

`mctas litematic verify-ingame` installs Fabric Loader for the version (meta.fabricmc.net),
MaLiLib and Litematica (the newest Fabric release for the version on Modrinth: 26.3-0.30.2 and
26.3-0.29.1) and the repository's probe mod (`bridge/litematica-probe`, compiled against them;
code: `crates/mctas/src/fabric.rs`, `crates/litematic/src/ingame.rs`), starts the real client on
an X display (Xvfb) and lays the designs out in a grid. For each design the probe loads the file,
places it and pastes it with Litematica's own code (`LitematicaSchematic.createFromFile`,
`SchematicPlacement.createFor`, `SchematicPlacementManager.pastePlacementToWorld`, after
Litematica's schematic world has the placement), in two worlds:

- `server`: the client is on the bridge's server (a frozen sandbox: nothing ticks) as a flying
  creative operator, and Litematica pastes with commands (`setblock ... strict`, `fill`,
  `data modify` for block entities, `summon <id> x y z`); the verifier reads the area through
  the bridge.
- `single`: the probe creates a flat creative singleplayer world and freezes it (`tick freeze`);
  Litematica pastes directly into the integrated server; the probe dumps the area (blocks, block
  entities and entities with all their saved data) for the verifier.

The world must then equal the design: every cell of the box (the design's state or air; a
property the design leaves out is the game's default), every `nbt:` key in the block entity's
data, every entity of the right type within 1e-6 of its position, nothing extra, and (in the
singleplayer world) every key of the entity's data (`Motion`, `Rotation`, `Items`...).

Run on 20 designs (two doors; the observer, hopper, comparator, sculk and torch calibration
clocks; boat ring, chest-boat lift, minecart loop, hopper minecart, bookshelf cart, dripleaf
armor-stand, water channel and bubble-column clocks; five calibration circuits including a
flying machine): 40 of 40 pastes identical. Found and fixed on the way:

- An entity without `Rotation` was never pasted: Litematica's `placeEntitiesToWorldWithinChunk`
  reads `Rotation`'s two floats unconditionally (malilib's `ListData.getFloatAt(0)` on a missing
  list throws), so the entity never reached the schematic world. The writer now adds the fields
  the game itself always saves.
- An entity without a `UUID` could be pasted twice: Litematica adds a placement chunk's entities
  again when it reloads the chunk into its schematic world; a UUID lets
  `deduplicateSchematicEntities` drop the copy.

What Litematica does with its default settings (none of it is about the file):

- On a server, block entities paste EMPTY (a hopper's item, a chest's contents) unless Generic >
  `pasteNbtRestoreBehavior` is `place_data_modify` (the default here) or `place_clone`.
- On a server, entities may be summoned twice unless Generic > `deduplicateSchematicEntities` is
  on (the default here).
- On a server, entities lose their data: Litematica summons them with `summon <id> x y z`, so a
  minecart's starting motion or a chest boat's items are not pasted (the server run compares type
  and position only). Paste in singleplayer, or add the data by hand.
