//! Litematica schematics (`.litematic`) of the repository's designs (doors, clocks, circuits).
//!
//! The format is Litematica's own (checked against its source for Minecraft 26.3,
//! github.com/sakura-ryoko/litematica branch 26.3: `LitematicaSchematic.writeToData`,
//! `SchematicMetadata.writeData`, `LitematicaBlockStateContainer`, `LitematicaBitArray`, and
//! malilib's `DataTypeUtils`): a gzipped NBT compound
//!
//! ```text
//! {MinecraftDataVersion: int, Version: 7, SubVersion: 1,
//!  Metadata: {Name, Author, Description, RegionCount, TotalVolume, TotalBlocks: int,
//!             TimeCreated, TimeModified: long (ms), EnclosingSize: {x, y, z}},
//!  Regions: {<name>: {BlockStatePalette: [{Name, Properties?}], BlockStates: long[],
//!            TileEntities: [{x, y, z, id, ...}], PendingBlockTicks: [], PendingFluidTicks: [],
//!            Entities: [{id, Pos: [d, d, d], ...}], Position: {x, y, z}, Size: {x, y, z}}}}
//! ```
//!
//! Palette entry 0 is air; each cell's palette index takes `max(2, bits(palette size - 1))`
//! bits, packed little-endian into the longs with entries spanning two longs (no padding), cell
//! index `(y * sizeZ + z) * sizeX + x`. Positions in the region are relative to its minimum
//! corner; tile entities carry relative x y z, entities a relative `Pos`.

pub mod cli;
pub mod ingame;
pub mod nbt;

use anyhow::{Context, Result, anyhow, bail};
use mctas::blockstate::BlockState;
use mctas::design::{Design, EntitySpec};
use nbt::Tag;
use std::collections::BTreeMap;
use std::io::{Read, Write};

/// Minecraft 26.3's data version (`world_version` in the server jar's version.json): the game
/// the judges run. Litematica data-fixes schematics with an older one and warns on a newer one.
pub const DATA_VERSION: i32 = 5023;
/// Litematica's schematic format version and sub-version (26.3: `SCHEMATIC_VERSION = 7`,
/// `SCHEMATIC_VERSION_SUB = 1`).
pub const SCHEMATIC_VERSION: i32 = 7;
pub const SCHEMATIC_SUB_VERSION: i32 = 1;
pub const AUTHOR: &str = "minecraft-tas";

type Pos = [i32; 3];

/// What a schematic says about itself besides its blocks.
#[derive(Clone, Debug)]
pub struct Meta {
    pub name: String,
    pub author: String,
    pub description: String,
    /// Milliseconds since 1970 (TimeCreated and TimeModified).
    pub time_ms: i64,
    pub data_version: i32,
}

impl Meta {
    pub fn new(name: &str, description: &str, time_ms: i64) -> Meta {
        Meta { name: name.to_string(), author: AUTHOR.to_string(), description: description.to_string(), time_ms, data_version: DATA_VERSION }
    }
}

/// Milliseconds since 1970, now.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The design's own description: its comment lines without the archive's bookkeeping ones
/// (`# verified ...`, `# archived ...`, judge lines).
pub fn design_comments(text: &str) -> String {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix('#').map(str::trim))
        .filter(|l| {
            !(l.is_empty()
                || l.starts_with("verified")
                || l.starts_with("archived")
                || l.starts_with("CLOCK:")
                || l.starts_with("CIRCUIT:")
                || l.starts_with("FAIL:")
                || l.starts_with("composite"))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The block entity type of a block (its `id` in a tile entity tag): the block's own id, except
/// for the blocks that share one type (signs, beds, banners, heads, shulker boxes...).
pub fn block_entity_id(block: &str) -> String {
    let id = block.split('[').next().unwrap_or(block);
    let short = id.strip_prefix("minecraft:").unwrap_or(id);
    let kind = if short.ends_with("_hanging_sign") || short.ends_with("_wall_hanging_sign") {
        "hanging_sign"
    } else if short.ends_with("_sign") {
        "sign"
    } else if short.ends_with("_bed") {
        "bed"
    } else if short.ends_with("_banner") {
        "banner"
    } else if short.ends_with("_skull") || short.ends_with("_head") {
        "skull"
    } else if short.ends_with("shulker_box") {
        "shulker_box"
    } else if short == "moving_piston" {
        "piston"
    } else if short == "redstone_wall_torch" {
        "redstone_torch"
    } else {
        short
    };
    format!("minecraft:{kind}")
}

/// The region a design fills: its minimum corner and size, over its blocks, its block entities
/// and the cells of its entities.
fn region_box(d: &Design) -> Result<(Pos, Pos)> {
    let cells = d.blocks.keys().copied().chain(d.block_entities.keys().copied()).chain(d.entities.iter().map(EntitySpec::cell));
    let mut lo: Option<Pos> = None;
    let mut hi: Pos = [i32::MIN; 3];
    for p in cells {
        let l = lo.get_or_insert(p);
        for a in 0..3 {
            l[a] = l[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let lo = lo.ok_or_else(|| anyhow!("{}: no blocks", d.name))?;
    Ok((lo, [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]))
}

/// Bits per cell for a palette of `n` states (Litematica: `max(2, 32 - nlz(n - 1))`).
pub fn bits_for(n: usize) -> u32 {
    let need = if n <= 1 { 0 } else { usize::BITS - (n - 1).leading_zeros() };
    need.max(2)
}

/// Pack cell values (each < 2^bits) like Litematica's `LitematicaBitArray`.
pub fn pack(values: &[u32], bits: u32) -> Vec<i64> {
    let total = values.len() as u64 * bits as u64;
    let mut longs = vec![0u64; total.div_ceil(64) as usize];
    let mask = (1u64 << bits) - 1;
    for (i, &v) in values.iter().enumerate() {
        let start = i as u64 * bits as u64;
        let (a, off) = ((start >> 6) as usize, (start & 63) as u32);
        let v = v as u64 & mask;
        longs[a] |= v << off;
        let end = ((i as u64 + 1) * bits as u64 - 1) >> 6;
        if end as usize != a {
            longs[end as usize] |= v >> (64 - off);
        }
    }
    longs.into_iter().map(|l| l as i64).collect()
}

/// Unpack `count` cell values (the reverse of [`pack`], `LitematicaBitArray.getAt`).
pub fn unpack(longs: &[i64], bits: u32, count: usize) -> Result<Vec<u32>> {
    let need = (count as u64 * bits as u64).div_ceil(64) as usize;
    if longs.len() < need {
        bail!("BlockStates has {} longs, {count} cells of {bits} bits need {need}", longs.len());
    }
    let mask = (1u64 << bits) - 1;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let start = i as u64 * bits as u64;
        let (a, off) = ((start >> 6) as usize, (start & 63) as u32);
        let end = (((i as u64 + 1) * bits as u64 - 1) >> 6) as usize;
        let lo = (longs[a] as u64) >> off;
        let v = if end == a { lo & mask } else { (lo | ((longs[end] as u64) << (64 - off))) & mask };
        out.push(v as u32);
    }
    Ok(out)
}

fn pos_tag(p: Pos) -> Tag {
    Tag::compound().with("x", Tag::Int(p[0])).with("y", Tag::Int(p[1])).with("z", Tag::Int(p[2]))
}

/// A palette entry: `{Name}` or `{Name, Properties: {k: "v"}}` (malilib `writeBlockStateToTag`).
fn state_tag(s: &BlockState) -> Tag {
    let mut t = Tag::compound().with("Name", Tag::String(s.name.clone()));
    if !s.props.is_empty() {
        t.put("Properties", Tag::Compound(s.props.iter().map(|(k, v)| (k.clone(), Tag::String(v.clone()))).collect()));
    }
    t
}

/// SNBT of a design line as a compound (`None`: an empty one).
fn snbt_compound(s: Option<&str>, what: &str) -> Result<Tag> {
    let t = match s {
        Some(s) => nbt::parse_snbt(s).with_context(|| format!("{what}: {s}"))?,
        None => Tag::compound(),
    };
    if !matches!(t, Tag::Compound(_)) {
        bail!("{what}: the SNBT is not a compound");
    }
    Ok(t)
}

/// The schematic of a design as an NBT tree: one region named after the design, holding every
/// block, the block entities of its `nbt:` lines and the entities of its `entity:` lines.
pub fn to_nbt(d: &Design, m: &Meta) -> Result<Tag> {
    let (lo, size) = region_box(d)?;
    let volume = size[0] as i64 * size[1] as i64 * size[2] as i64;
    if volume > 64 * 1024 * 1024 {
        bail!("{}: region of {volume} cells is too big", d.name);
    }
    let air = BlockState::parse("minecraft:air")?;
    let mut grid: Vec<Option<BlockState>> = vec![None; volume as usize];
    let mut total_blocks = 0i32;
    for (p, s) in &d.blocks {
        let st = BlockState::parse(s).with_context(|| format!("{}: block {s} at {p:?}", d.name))?;
        if st.name == "minecraft:air" {
            continue;
        }
        let r = [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]];
        grid[((r[1] as i64 * size[2] as i64 + r[2] as i64) * size[0] as i64 + r[0] as i64) as usize] = Some(st);
        total_blocks += 1;
    }
    // palette ids in Litematica's scan order (y, then z, then x), air first
    let mut palette: Vec<BlockState> = vec![air];
    let mut index_of: BTreeMap<BlockState, u32> = BTreeMap::new();
    let cells: Vec<u32> = grid
        .into_iter()
        .map(|c| match c {
            None => 0,
            Some(st) => *index_of.entry(st.clone()).or_insert_with(|| {
                palette.push(st);
                (palette.len() - 1) as u32
            }),
        })
        .collect();
    let bits = bits_for(palette.len());
    let mut tiles = Vec::new();
    for (p, snbt) in &d.block_entities {
        let block = d.blocks.get(p).ok_or_else(|| anyhow!("{}: nbt: at {p:?} has no block", d.name))?;
        let data = snbt_compound(Some(snbt), &format!("nbt: {} {} {}", p[0], p[1], p[2]))?;
        // vanilla's saveWithFullMetadata order: id, x, y, z, then the data
        let mut t = Tag::compound().with("id", Tag::String(block_entity_id(block)));
        for (a, k) in ["x", "y", "z"].iter().enumerate() {
            t.put(k, Tag::Int(p[a] - lo[a]));
        }
        for (k, v) in data.entries() {
            if !matches!(k.as_str(), "x" | "y" | "z" | "id") {
                t.put(k, v.clone());
            }
        }
        tiles.push(t);
    }
    let mut entities = Vec::new();
    for e in &d.entities {
        let data = snbt_compound(e.nbt.as_deref(), &format!("entity: {}", e.kind))?;
        let mut t = Tag::compound();
        for (k, v) in data.entries() {
            if k != "Pos" && k != "id" {
                t.put(k, v.clone());
            }
        }
        t.put("Pos", Tag::List((0..3).map(|a| Tag::Double(e.pos[a] - lo[a] as f64)).collect()));
        t.put("id", Tag::String(e.kind.clone()));
        // what Entity.saveWithoutId always writes: Litematica's placement reads Rotation's two
        // floats unconditionally (ListData.getFloatAt(0) on a missing list throws and the entity
        // never reaches the schematic world, so it is never pasted: `litematic verify-ingame`)
        if t.get("Motion").is_none() {
            t.put("Motion", Tag::List(vec![Tag::Double(0.0); 3]));
        }
        if t.get("Rotation").is_none() {
            t.put("Rotation", Tag::List(vec![Tag::Float(0.0); 2]));
        }
        // and a UUID (deterministic: the design's name and the entity's index): Litematica's
        // schematic world replaces an entity by UUID when it loads a chunk again; without one,
        // every reload adds a copy and the paste summons each copy
        if t.get("UUID").is_none() {
            let h = mctas::net::sha256_hex(format!("{}/entity/{}", d.name, entities.len()).as_bytes());
            let word = |k: usize| u32::from_str_radix(&h[k * 8..k * 8 + 8], 16).unwrap_or(0) as i32;
            t.put("UUID", Tag::IntArray(vec![word(0), word(1), word(2), word(3)]));
        }
        entities.push(t);
    }
    let region = Tag::compound()
        .with("BlockStatePalette", Tag::List(palette.iter().map(state_tag).collect()))
        .with("BlockStates", Tag::LongArray(pack(&cells, bits)))
        .with("TileEntities", Tag::List(tiles))
        .with("PendingBlockTicks", Tag::List(Vec::new()))
        .with("PendingFluidTicks", Tag::List(Vec::new()))
        .with("Entities", Tag::List(entities))
        .with("Position", pos_tag([0, 0, 0]))
        .with("Size", pos_tag(size));
    let region_name = if d.name.is_empty() { "region".to_string() } else { d.name.clone() };
    let metadata = Tag::compound()
        .with("Name", Tag::String(m.name.clone()))
        .with("Author", Tag::String(m.author.clone()))
        .with("Description", Tag::String(truncate_utf8(&m.description, 16_000)))
        .with("RegionCount", Tag::Int(1))
        .with("TotalVolume", Tag::Int(volume as i32))
        .with("TotalBlocks", Tag::Int(total_blocks))
        .with("TimeCreated", Tag::Long(m.time_ms))
        .with("TimeModified", Tag::Long(m.time_ms))
        .with("EnclosingSize", pos_tag(size));
    Ok(Tag::compound()
        .with("MinecraftDataVersion", Tag::Int(m.data_version))
        .with("Version", Tag::Int(SCHEMATIC_VERSION))
        .with("SubVersion", Tag::Int(SCHEMATIC_SUB_VERSION))
        .with("Metadata", metadata)
        .with("Regions", Tag::compound().with(&region_name, region)))
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// The `.litematic` file of a design: [`to_nbt`], written with an empty root name and gzipped.
pub fn export(d: &Design, m: &Meta) -> Result<Vec<u8>> {
    let raw = nbt::write_named("", &to_nbt(d, m)?)?;
    let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(&raw)?;
    Ok(z.finish()?)
}

/// A region read back from a schematic.
#[derive(Clone, Debug, Default)]
pub struct Region {
    pub name: String,
    pub position: Pos,
    /// The size as stored (Litematica allows negative sizes: the region then grows the other way).
    pub size: Pos,
    pub palette: Vec<String>,
    pub bits: u32,
    /// Every non-air block by position relative to the region's minimum corner.
    pub blocks: BTreeMap<Pos, String>,
    /// Tile entities by relative position (their tag, x y z included).
    pub tiles: BTreeMap<Pos, Tag>,
    /// Entities: relative position, id, tag.
    pub entities: Vec<([f64; 3], String, Tag)>,
    /// The cells re-packed with [`pack`] give back the stored BlockStates longs bit for bit
    /// (a check of the packing against files Litematica wrote).
    pub repacks_identically: bool,
}

/// A schematic read back from a `.litematic` file.
#[derive(Clone, Debug, Default)]
pub struct Schematic {
    pub data_version: i64,
    pub version: i64,
    pub sub_version: Option<i64>,
    pub name: String,
    pub author: String,
    pub description: String,
    pub total_volume: Option<i64>,
    pub total_blocks: Option<i64>,
    pub enclosing: Option<Pos>,
    pub regions: Vec<Region>,
    /// The whole tree, for dumps.
    pub root: Tag,
}

fn int_tag(t: &Tag, key: &str) -> Result<i64> {
    t.get(key).and_then(Tag::as_i64).ok_or_else(|| anyhow!("missing int {key}"))
}

fn read_pos(t: &Tag) -> Result<Pos> {
    Ok([int_tag(t, "x")? as i32, int_tag(t, "y")? as i32, int_tag(t, "z")? as i32])
}

/// Parse a `.litematic` (gzipped or raw NBT) and decode every region's blocks.
pub fn read(bytes: &[u8]) -> Result<Schematic> {
    let raw = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut v = Vec::new();
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut v).context("gunzip")?;
        v
    } else {
        bytes.to_vec()
    };
    let (_, root) = nbt::read_named(&raw)?;
    let md = root.get("Metadata").ok_or_else(|| anyhow!("no Metadata"))?;
    let mut s = Schematic {
        data_version: int_tag(&root, "MinecraftDataVersion")?,
        version: int_tag(&root, "Version")?,
        sub_version: root.get("SubVersion").and_then(Tag::as_i64),
        name: md.get("Name").and_then(Tag::as_str).unwrap_or("").to_string(),
        author: md.get("Author").and_then(Tag::as_str).unwrap_or("").to_string(),
        description: md.get("Description").and_then(Tag::as_str).unwrap_or("").to_string(),
        total_volume: md.get("TotalVolume").and_then(Tag::as_i64),
        total_blocks: md.get("TotalBlocks").and_then(Tag::as_i64),
        enclosing: md.get("EnclosingSize").and_then(|t| read_pos(t).ok()),
        regions: Vec::new(),
        root: Tag::compound(),
    };
    for (name, rt) in root.get("Regions").map(Tag::entries).unwrap_or(&[]) {
        let position = read_pos(rt.get("Position").ok_or_else(|| anyhow!("region {name}: no Position"))?)?;
        let size = read_pos(rt.get("Size").ok_or_else(|| anyhow!("region {name}: no Size"))?)?;
        let dims = [size[0].unsigned_abs() as usize, size[1].unsigned_abs() as usize, size[2].unsigned_abs() as usize];
        let palette: Vec<String> = rt
            .get("BlockStatePalette")
            .map(Tag::as_list)
            .unwrap_or(&[])
            .iter()
            .map(|e| {
                let name = e.get("Name").or_else(|| e.get("id")).and_then(Tag::as_str).unwrap_or("minecraft:air");
                let props: Vec<String> = e.get("Properties").or_else(|| e.get("properties")).map(Tag::entries).unwrap_or(&[]).iter().map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("?"))).collect();
                if props.is_empty() { name.to_string() } else { format!("{name}[{}]", props.join(",")) }
            })
            .collect();
        let bits = bits_for(palette.len());
        let air: Vec<bool> = palette.iter().map(|st| BlockState::parse(st).map(|b| b.name == "minecraft:air").unwrap_or(false)).collect();
        let longs: &[i64] = match rt.get("BlockStates") {
            Some(Tag::LongArray(v)) => v,
            _ => bail!("region {name}: BlockStates is not a long array"),
        };
        let count = dims[0] * dims[1] * dims[2];
        let values = unpack(longs, bits, count)?;
        let repacks_identically = pack(&values, bits) == longs;
        let mut blocks = BTreeMap::new();
        for (i, v) in values.iter().enumerate() {
            let st = palette.get(*v as usize).ok_or_else(|| anyhow!("region {name}: cell {i} has palette index {v} of {}", palette.len()))?;
            if air[*v as usize] {
                continue;
            }
            let x = (i % dims[0]) as i32;
            let z = ((i / dims[0]) % dims[2]) as i32;
            let y = (i / (dims[0] * dims[2])) as i32;
            blocks.insert([x, y, z], st.clone());
        }
        let mut tiles = BTreeMap::new();
        for t in rt.get("TileEntities").map(Tag::as_list).unwrap_or(&[]) {
            tiles.insert(read_pos(t)?, t.clone());
        }
        let mut entities = Vec::new();
        // entities are stored relative to the region's Position (its first corner), blocks and
        // tile entities relative to its minimum corner: move entities to the minimum corner
        let offset: [f64; 3] = std::array::from_fn(|a| if size[a] < 0 { (size[a] + 1) as f64 } else { 0.0 });
        for t in rt.get("Entities").map(Tag::as_list).unwrap_or(&[]) {
            let p = t.get("Pos").map(Tag::as_list).unwrap_or(&[]);
            if p.len() != 3 {
                bail!("region {name}: an entity without a Pos of 3 doubles");
            }
            let pos: [f64; 3] = std::array::from_fn(|a| p[a].as_f64().unwrap_or(f64::NAN) - offset[a]);
            entities.push((pos, t.get("id").and_then(Tag::as_str).unwrap_or("?").to_string(), t.clone()));
        }
        s.regions.push(Region { name: name.clone(), position, size, palette, bits, blocks, tiles, entities, repacks_identically });
    }
    s.root = root;
    Ok(s)
}

/// Check that a schematic holds exactly the design: one region, every block (state compared
/// property by property), every block entity's data and every entity at its place; and the
/// metadata Litematica reads (data version, format version, counts, sizes).
pub fn compare(d: &Design, s: &Schematic) -> Result<()> {
    let (lo, size) = region_box(d)?;
    if s.version != SCHEMATIC_VERSION as i64 || s.data_version != DATA_VERSION as i64 {
        bail!("Version {} / MinecraftDataVersion {}", s.version, s.data_version);
    }
    let [r] = s.regions.as_slice() else { bail!("{} regions, want 1", s.regions.len()) };
    if r.size != size || r.position != [0, 0, 0] || s.enclosing != Some(size) {
        bail!("region size {:?} at {:?} (enclosing {:?}), want {size:?} at 0 0 0", r.size, r.position, s.enclosing);
    }
    let want: BTreeMap<Pos, BlockState> = d
        .blocks
        .iter()
        .map(|(p, st)| Ok(([p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]], BlockState::parse(st)?)))
        .collect::<Result<BTreeMap<_, _>>>()?
        .into_iter()
        .filter(|(_, b)| b.name != "minecraft:air")
        .collect();
    let got: BTreeMap<Pos, BlockState> = r.blocks.iter().map(|(p, st)| Ok((*p, BlockState::parse(st)?))).collect::<Result<_>>()?;
    for (p, b) in &want {
        match got.get(p) {
            Some(g) if g == b => {}
            Some(g) => bail!("block at {p:?}: {g}, want {b}"),
            None => bail!("block at {p:?} missing (want {b})"),
        }
    }
    if let Some((p, g)) = got.iter().find(|(p, _)| !want.contains_key(*p)) {
        bail!("extra block {g} at {p:?}");
    }
    if s.total_blocks != Some(want.len() as i64) || s.total_volume != Some(size[0] as i64 * size[1] as i64 * size[2] as i64) {
        bail!("TotalBlocks {:?} / TotalVolume {:?}", s.total_blocks, s.total_volume);
    }
    if r.tiles.len() != d.block_entities.len() {
        bail!("{} tile entities, want {}", r.tiles.len(), d.block_entities.len());
    }
    for (p, snbt) in &d.block_entities {
        let rel = [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]];
        let t = r.tiles.get(&rel).ok_or_else(|| anyhow!("no tile entity at {rel:?}"))?;
        let want_id = block_entity_id(&d.blocks[p]);
        if t.get("id").and_then(Tag::as_str) != Some(want_id.as_str()) {
            bail!("tile entity at {rel:?}: id {:?}, want {want_id}", t.get("id"));
        }
        for (k, v) in nbt::parse_snbt(snbt)?.entries() {
            if t.get(k) != Some(v) {
                bail!("tile entity at {rel:?}: {k} = {:?}, want {}", t.get(k), v.to_snbt());
            }
        }
    }
    if r.entities.len() != d.entities.len() {
        bail!("{} entities, want {}", r.entities.len(), d.entities.len());
    }
    for (e, (pos, id, t)) in d.entities.iter().zip(&r.entities) {
        let want = [e.pos[0] - lo[0] as f64, e.pos[1] - lo[1] as f64, e.pos[2] - lo[2] as f64];
        if *id != e.kind || (0..3).any(|a| (pos[a] - want[a]).abs() > 1e-9) {
            bail!("entity {id} at {pos:?}, want {} at {want:?}", e.kind);
        }
        if let Some(s) = &e.nbt {
            for (k, v) in nbt::parse_snbt(s)?.entries() {
                if k != "Pos" && k != "id" && t.get(k) != Some(v) {
                    bail!("entity {id}: {k} = {:?}, want {}", t.get(k), v.to_snbt());
                }
            }
        }
    }
    Ok(())
}

/// A region as a design (blocks at their relative positions, one palette letter per state):
/// `mctas litematic read --design`.
pub fn region_design(r: &Region, name: &str) -> Design {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    d.blocks = r.blocks.clone();
    for (p, t) in &r.tiles {
        let data: Vec<(String, Tag)> = t.entries().iter().filter(|(k, _)| !matches!(k.as_str(), "x" | "y" | "z" | "id")).cloned().collect();
        if !data.is_empty() {
            d.block_entities.insert(*p, Tag::Compound(data).to_snbt());
        }
    }
    for (pos, id, t) in &r.entities {
        let data: Vec<(String, Tag)> = t.entries().iter().filter(|(k, _)| k != "Pos" && k != "id").cloned().collect();
        d.entities.push(EntitySpec { kind: id.clone(), pos: *pos, nbt: (!data.is_empty()).then(|| Tag::Compound(data).to_snbt()) });
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_like_litematica() {
        // max(2, 32 - nlz(n - 1))
        for (n, b) in [(1, 2), (2, 2), (4, 2), (5, 3), (8, 3), (9, 4), (16, 4), (17, 5), (300, 9)] {
            assert_eq!(bits_for(n), b, "palette {n}");
        }
    }

    #[test]
    fn packing_spans_longs() {
        // 5 bits per entry: entry 12 starts at bit 60 and spans two longs
        let values: Vec<u32> = (0..40).map(|i| (i * 7 % 32) as u32).collect();
        let longs = pack(&values, 5);
        assert_eq!(longs.len(), 4); // 200 bits
        assert_eq!(unpack(&longs, 5, 40).unwrap(), values);
        // hand check against LitematicaBitArray.setAt: entry 0 = 0, entry 1 = 7 at bits 5..10
        assert_eq!(longs[0] as u64 & 0x3ff, 7 << 5);
        // entry 12 = 12*7%32 = 20 = 0b10100: low 4 bits in long 0 bits 60..64, high bit in long 1 bit 0
        assert_eq!((longs[0] as u64) >> 60, 20 & 0xf);
        assert_eq!(longs[1] as u64 & 1, 20 >> 4);
    }

    const DESIGN: &str = "name: t-design\n# A test: a hopper with items, a boat and a lever.\norigin: 10 64 -3\nport lever: 12 65 -3\nentity: minecraft:oak_boat 11.5 65 -2.5 {Rotation:[90f,0f]}\nnbt: 10 64 -3 {Items:[{Slot:0b,id:\"minecraft:stone\",count:3}]}\npalette:\n  H minecraft:hopper[enabled=true,facing=down]\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=north,powered=false]\n  W minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]\nlayer 0:\n  HSS\n  S.S\nlayer 1:\n  .WL\n";

    #[test]
    fn export_reads_back() {
        let d = Design::parse(DESIGN).unwrap();
        let bytes = export(&d, &Meta::new("t-design", &design_comments(DESIGN), 1_792_450_000_000)).unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        let s = read(&bytes).unwrap();
        compare(&d, &s).unwrap();
        assert_eq!(s.name, "t-design");
        assert_eq!(s.author, AUTHOR);
        assert_eq!(s.description, "A test: a hopper with items, a boat and a lever.");
        assert_eq!(s.sub_version, Some(1));
        let r = &s.regions[0];
        assert_eq!(r.size, [3, 2, 2]);
        assert_eq!(r.palette[0], "minecraft:air");
        assert_eq!(r.palette.len(), 5);
        assert_eq!(r.bits, 3);
        assert_eq!(r.blocks[&[0, 0, 0]], "minecraft:hopper[enabled=true,facing=down]");
        assert_eq!(r.tiles[&[0, 0, 0]].get("id").and_then(Tag::as_str), Some("minecraft:hopper"));
        assert_eq!(r.entities[0].0, [1.5, 1.0, 0.5]);
        // the key order Litematica writes
        let keys: Vec<&str> = s.root.entries().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["MinecraftDataVersion", "Version", "SubVersion", "Metadata", "Regions"]);
        // a changed block is caught
        let mut d2 = d.clone();
        d2.blocks.insert([11, 64, -3], "minecraft:dirt".into());
        assert!(compare(&d2, &s).is_err());
        // the repo's independent NBT reader decodes the same palette and longs
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(&bytes[..]).read_to_end(&mut raw).unwrap();
        let other = mctas::vanilla::nbt::parse_named(&raw).unwrap();
        let reg = other.get("Regions").and_then(|t| t.get("t-design")).unwrap();
        assert_eq!(reg.get("BlockStatePalette").map(|t| t.as_list().len()), Some(5));
        assert_eq!(reg.get("BlockStates").map(|t| t.as_long_array().len()), Some(1)); // 12 cells x 3 bits
    }

    #[test]
    fn block_entity_ids() {
        assert_eq!(block_entity_id("minecraft:hopper[facing=down]"), "minecraft:hopper");
        assert_eq!(block_entity_id("minecraft:oak_wall_sign[facing=north]"), "minecraft:sign");
        assert_eq!(block_entity_id("minecraft:spruce_hanging_sign"), "minecraft:hanging_sign");
        assert_eq!(block_entity_id("minecraft:red_bed[part=head]"), "minecraft:bed");
        assert_eq!(block_entity_id("minecraft:skeleton_wall_skull"), "minecraft:skull");
        assert_eq!(block_entity_id("minecraft:player_head"), "minecraft:skull");
        assert_eq!(block_entity_id("minecraft:lime_shulker_box"), "minecraft:shulker_box");
        assert_eq!(block_entity_id("minecraft:chiseled_bookshelf"), "minecraft:chiseled_bookshelf");
    }
}
