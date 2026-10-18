//! Block states: the [`Block`] enum (one variant per block family, the state's properties as
//! fields), its block type ([`Kind`], the game's `Block` identity used by `state.is(block)`,
//! scheduled ticks and block events), and the conversion from/to `minecraft:name[props]` strings
//! (properties sorted by name, like the game prints them).

use crate::pos::Dir;
use std::fmt;

/// `RedstoneSide`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Side {
    None,
    Side,
    Up,
}
impl Side {
    pub fn connected(self) -> bool {
        self != Side::None
    }
    fn name(self) -> &'static str {
        match self {
            Side::None => "none",
            Side::Side => "side",
            Side::Up => "up",
        }
    }
    fn parse(s: &str) -> Option<Side> {
        Some(match s {
            "none" => Side::None,
            "side" => Side::Side,
            "up" => Side::Up,
            _ => return None,
        })
    }
}

/// `AttachFace` (levers, buttons).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    Floor,
    Wall,
    Ceiling,
}
impl Face {
    fn name(self) -> &'static str {
        match self {
            Face::Floor => "floor",
            Face::Wall => "wall",
            Face::Ceiling => "ceiling",
        }
    }
    fn parse(s: &str) -> Option<Face> {
        Some(match s {
            "floor" => Face::Floor,
            "wall" => Face::Wall,
            "ceiling" => Face::Ceiling,
            _ => return None,
        })
    }
    /// `FaceAttachedHorizontalDirectionalBlock.getConnectedDirection`: from the block toward the
    /// side opposite its support... (the game's "connected direction": the support is at
    /// `pos.relative(connected.getOpposite())`).
    pub fn connected(self, facing: Dir) -> Dir {
        match self {
            Face::Ceiling => Dir::Down,
            Face::Floor => Dir::Up,
            Face::Wall => facing,
        }
    }
}

/// `NoteBlockInstrument`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Instrument {
    Harp,
    Basedrum,
    Snare,
    Hat,
    Bass,
    Flute,
    Bell,
    Guitar,
    Chime,
    Xylophone,
    IronXylophone,
    CowBell,
    Didgeridoo,
    Bit,
    Banjo,
    Pling,
    Trumpet,
    TrumpetExposed,
    TrumpetWeathered,
    TrumpetOxidized,
    Zombie,
    Skeleton,
    Creeper,
    Dragon,
    WitherSkeleton,
    Piglin,
    CustomHead,
}
const INSTRUMENTS: [(Instrument, &str); 27] = [
    (Instrument::Harp, "harp"),
    (Instrument::Basedrum, "basedrum"),
    (Instrument::Snare, "snare"),
    (Instrument::Hat, "hat"),
    (Instrument::Bass, "bass"),
    (Instrument::Flute, "flute"),
    (Instrument::Bell, "bell"),
    (Instrument::Guitar, "guitar"),
    (Instrument::Chime, "chime"),
    (Instrument::Xylophone, "xylophone"),
    (Instrument::IronXylophone, "iron_xylophone"),
    (Instrument::CowBell, "cow_bell"),
    (Instrument::Didgeridoo, "didgeridoo"),
    (Instrument::Bit, "bit"),
    (Instrument::Banjo, "banjo"),
    (Instrument::Pling, "pling"),
    (Instrument::Trumpet, "trumpet"),
    (Instrument::TrumpetExposed, "trumpet_exposed"),
    (Instrument::TrumpetWeathered, "trumpet_weathered"),
    (Instrument::TrumpetOxidized, "trumpet_oxidized"),
    (Instrument::Zombie, "zombie"),
    (Instrument::Skeleton, "skeleton"),
    (Instrument::Creeper, "creeper"),
    (Instrument::Dragon, "dragon"),
    (Instrument::WitherSkeleton, "wither_skeleton"),
    (Instrument::Piglin, "piglin"),
    (Instrument::CustomHead, "custom_head"),
];
impl Instrument {
    fn name(self) -> &'static str {
        INSTRUMENTS.iter().find(|(i, _)| *i == self).map(|(_, n)| *n).unwrap()
    }
    fn parse(s: &str) -> Option<Instrument> {
        INSTRUMENTS.iter().find(|(_, n)| *n == s).map(|(i, _)| *i)
    }
    /// `NoteBlockInstrument.worksAboveNoteBlock`: mob heads (and custom heads).
    pub fn works_above_note_block(self) -> bool {
        matches!(
            self,
            Instrument::Zombie | Instrument::Skeleton | Instrument::Creeper | Instrument::Dragon | Instrument::WitherSkeleton | Instrument::Piglin | Instrument::CustomHead
        )
    }
}

/// `PushReaction`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PushReaction {
    PushPull,
    Push,
    Popped,
    Immoveable,
    IgnoreEntity,
}

/// Plain blocks without redstone behavior (their only roles: conducting or not, holding things
/// up, note block instruments, piston push reactions).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Simple {
    Stone,
    SmoothStone,
    Cobblestone,
    Obsidian,
    Glass,
    TintedGlass,
    WhiteWool,
    OakPlanks,
    GoldBlock,
    IronBlock,
    Clay,
    EmeraldBlock,
    Glowstone,
    PackedIce,
    SoulSand,
    SlimeBlock,
    HoneyBlock,
    WaxedCopperGrate,
    SeaLantern,
    // ---- pistmodel ----
    CryingObsidian,
}

/// (block, id, conductor, full collision cube (sturdy on every face), instrument, push reaction)
const SIMPLES: [(Simple, &str, bool, bool, Instrument, PushReaction); 20] = [
    (Simple::Stone, "stone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::SmoothStone, "smooth_stone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Cobblestone, "cobblestone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Obsidian, "obsidian", true, true, Instrument::Basedrum, PushReaction::Immoveable),
    (Simple::Glass, "glass", false, true, Instrument::Hat, PushReaction::PushPull),
    (Simple::TintedGlass, "tinted_glass", false, true, Instrument::Hat, PushReaction::PushPull),
    (Simple::WhiteWool, "white_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::OakPlanks, "oak_planks", true, true, Instrument::Bass, PushReaction::PushPull),
    (Simple::GoldBlock, "gold_block", true, true, Instrument::Bell, PushReaction::PushPull),
    (Simple::IronBlock, "iron_block", true, true, Instrument::IronXylophone, PushReaction::PushPull),
    (Simple::Clay, "clay", true, true, Instrument::Flute, PushReaction::PushPull),
    (Simple::EmeraldBlock, "emerald_block", true, true, Instrument::Bit, PushReaction::PushPull),
    (Simple::Glowstone, "glowstone", false, true, Instrument::Pling, PushReaction::PushPull),
    (Simple::PackedIce, "packed_ice", true, true, Instrument::Chime, PushReaction::PushPull),
    // Soul sand: `isRedstoneConductor(always)` though its collision box is 14 px tall; its faces
    // are still full for support (block support shape = full cube).
    (Simple::SoulSand, "soul_sand", true, true, Instrument::CowBell, PushReaction::PushPull),
    (Simple::SlimeBlock, "slime_block", true, true, Instrument::Harp, PushReaction::PushPull),
    // Honey: collision 15 px wide: not a conductor; support shape full.
    (Simple::HoneyBlock, "honey_block", false, false, Instrument::Harp, PushReaction::PushPull),
    (Simple::WaxedCopperGrate, "waxed_copper_grate", false, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::SeaLantern, "sea_lantern", false, true, Instrument::Hat, PushReaction::PushPull),
    // ---- pistmodel ----
    (Simple::CryingObsidian, "crying_obsidian", true, true, Instrument::Basedrum, PushReaction::Immoveable),
];

impl Simple {
    fn row(self) -> &'static (Simple, &'static str, bool, bool, Instrument, PushReaction) {
        SIMPLES.iter().find(|r| r.0 == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        self.row().1
    }
    pub fn conductor(self) -> bool {
        self.row().2
    }
    pub fn full(self) -> bool {
        self.row().3
    }
    pub fn instrument(self) -> Instrument {
        self.row().4
    }
    pub fn push_reaction(self) -> PushReaction {
        self.row().5
    }
    fn parse(s: &str) -> Option<Simple> {
        SIMPLES.iter().find(|r| r.1 == s).map(|r| r.0)
    }
}

/// Copper oxidation stages (waxed bulbs).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Oxid {
    Unaffected,
    Exposed,
    Weathered,
    Oxidized,
}
impl Oxid {
    fn bulb_name(self) -> &'static str {
        match self {
            Oxid::Unaffected => "waxed_copper_bulb",
            Oxid::Exposed => "waxed_exposed_copper_bulb",
            Oxid::Weathered => "waxed_weathered_copper_bulb",
            Oxid::Oxidized => "waxed_oxidized_copper_bulb",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Half {
    Top,
    Bottom,
}

/// `RailShape`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum RailShape {
    NorthSouth,
    EastWest,
    AscendingEast,
    AscendingWest,
    AscendingNorth,
    AscendingSouth,
}
const RAIL_SHAPES: [(RailShape, &str); 6] = [
    (RailShape::NorthSouth, "north_south"),
    (RailShape::EastWest, "east_west"),
    (RailShape::AscendingEast, "ascending_east"),
    (RailShape::AscendingWest, "ascending_west"),
    (RailShape::AscendingNorth, "ascending_north"),
    (RailShape::AscendingSouth, "ascending_south"),
];
impl RailShape {
    pub fn name(self) -> &'static str {
        RAIL_SHAPES.iter().find(|(s, _)| *s == self).unwrap().1
    }
    fn parse(s: &str) -> Option<RailShape> {
        RAIL_SHAPES.iter().find(|(_, n)| *n == s).map(|(r, _)| *r)
    }
    pub fn is_ascending(self) -> bool {
        matches!(self, RailShape::AscendingEast | RailShape::AscendingWest | RailShape::AscendingNorth | RailShape::AscendingSouth)
    }
}

/// `FrontAndTop` (crafter orientation).
pub const ORIENTATIONS: [&str; 12] = [
    "down_east", "down_north", "down_south", "down_west", "up_east", "up_north", "up_south", "up_west", "west_up", "east_up", "north_up", "south_up",
];

/// One block state.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Block {
    Air,
    Simple(Simple),
    /// Glazed terracotta (white): a conductor that pistons push but never pull or drag.
    Glazed { facing: Dir },
    Wire { north: Side, east: Side, south: Side, west: Side, power: u8 },
    RedstoneBlock,
    Torch { lit: bool },
    WallTorch { facing: Dir, lit: bool },
    Lever { face: Face, facing: Dir, powered: bool },
    /// Stone (20 gt) or oak (30 gt) button.
    Button { wood: bool, face: Face, facing: Dir, powered: bool },
    Repeater { facing: Dir, delay: u8, locked: bool, powered: bool },
    Comparator { facing: Dir, subtract: bool, powered: bool },
    Observer { facing: Dir, powered: bool },
    Lamp { lit: bool },
    Bulb { oxid: Oxid, lit: bool, powered: bool },
    Note { instrument: Instrument, note: u8, powered: bool },
    Target { power: u8 },
    /// Iron (redstone only) or oak trapdoor.
    Trapdoor { iron: bool, facing: Dir, top: bool, open: bool, powered: bool },
    /// Iron or oak door, one half.
    Door { iron: bool, facing: Dir, upper: bool, hinge_right: bool, open: bool, powered: bool },
    FenceGate { facing: Dir, in_wall: bool, open: bool, powered: bool },
    /// Powered rail or activator rail.
    Rail { activator: bool, shape: RailShape, powered: bool },
    Dispenser { dropper: bool, facing: Dir, triggered: bool },
    Crafter { orientation: u8, triggered: bool, crafting: bool },
    Hopper { facing: Dir, enabled: bool },
    /// Smooth stone slab: 0 bottom, 1 top, 2 double.
    Slab { kind: u8 },
    /// Persistent oak leaves (`persistent=false` ones decay at random: out of scope).
    Leaves { distance: u8, persistent: bool },
    /// Oak log (a leaves distance source).
    Log { axis: u8 },
    /// Skeleton skull (`powered` follows its neighbor signal; pistons pop it).
    Skull { rotation: u8, powered: bool },
    WallSkull { facing: Dir, powered: bool },
    /// Constant comparator sources (analog output from the state alone).
    Cake { bites: u8 },
    EndPortalFrame { facing: Dir, eye: bool },
    Composter { level: u8 },
    RespawnAnchor { charges: u8 },
    // ---- pistons (agent pistmodel's side of the model) ----
    Piston { sticky: bool, facing: Dir, extended: bool },
    PistonHead { sticky: bool, facing: Dir, short: bool },
    MovingPiston { sticky: bool, facing: Dir },
}

/// The block type (`state.getBlock()`): what `state.is(block)`, scheduled ticks and block events
/// compare.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Kind {
    Air,
    Simple(u8),
    Glazed,
    Wire,
    RedstoneBlock,
    Torch,
    WallTorch,
    Lever,
    Button(bool),
    Repeater,
    Comparator,
    Observer,
    Lamp,
    Bulb(u8),
    Note,
    Target,
    Trapdoor(bool),
    Door(bool),
    FenceGate,
    Rail(bool),
    Dispenser(bool),
    Crafter,
    Hopper,
    Slab,
    Leaves,
    Log,
    Skull,
    WallSkull,
    Cake,
    EndPortalFrame,
    Composter,
    RespawnAnchor,
    Piston(bool),
    PistonHead,
    MovingPiston,
}

fn b(s: bool) -> &'static str {
    if s { "true" } else { "false" }
}

impl Block {
    pub fn kind(&self) -> Kind {
        match *self {
            Block::Air => Kind::Air,
            Block::Simple(s) => Kind::Simple(s as u8),
            Block::Glazed { .. } => Kind::Glazed,
            Block::Wire { .. } => Kind::Wire,
            Block::RedstoneBlock => Kind::RedstoneBlock,
            Block::Torch { .. } => Kind::Torch,
            Block::WallTorch { .. } => Kind::WallTorch,
            Block::Lever { .. } => Kind::Lever,
            Block::Button { wood, .. } => Kind::Button(wood),
            Block::Repeater { .. } => Kind::Repeater,
            Block::Comparator { .. } => Kind::Comparator,
            Block::Observer { .. } => Kind::Observer,
            Block::Lamp { .. } => Kind::Lamp,
            Block::Bulb { oxid, .. } => Kind::Bulb(oxid as u8),
            Block::Note { .. } => Kind::Note,
            Block::Target { .. } => Kind::Target,
            Block::Trapdoor { iron, .. } => Kind::Trapdoor(iron),
            Block::Door { iron, .. } => Kind::Door(iron),
            Block::FenceGate { .. } => Kind::FenceGate,
            Block::Rail { activator, .. } => Kind::Rail(activator),
            Block::Dispenser { dropper, .. } => Kind::Dispenser(dropper),
            Block::Crafter { .. } => Kind::Crafter,
            Block::Hopper { .. } => Kind::Hopper,
            Block::Slab { .. } => Kind::Slab,
            Block::Leaves { .. } => Kind::Leaves,
            Block::Log { .. } => Kind::Log,
            Block::Skull { .. } => Kind::Skull,
            Block::WallSkull { .. } => Kind::WallSkull,
            Block::Cake { .. } => Kind::Cake,
            Block::EndPortalFrame { .. } => Kind::EndPortalFrame,
            Block::Composter { .. } => Kind::Composter,
            Block::RespawnAnchor { .. } => Kind::RespawnAnchor,
            Block::Piston { sticky, .. } => Kind::Piston(sticky),
            Block::PistonHead { .. } => Kind::PistonHead,
            Block::MovingPiston { .. } => Kind::MovingPiston,
        }
    }

    pub fn is_air(&self) -> bool {
        matches!(self, Block::Air)
    }

    /// Parse `minecraft:name[k=v,...]` (the prefix is optional; missing properties take the
    /// game's defaults).
    pub fn parse(s: &str) -> Result<Block, String> {
        let s = s.trim();
        let s = s.strip_prefix("minecraft:").unwrap_or(s);
        let (name, props) = match s.find('[') {
            Some(i) => (&s[..i], s[i + 1..].trim_end_matches(']')),
            None => (s, ""),
        };
        let mut kv: Vec<(&str, &str)> = Vec::new();
        for p in props.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = p.split_once('=').ok_or_else(|| format!("bad property {p:?} in {s:?}"))?;
            kv.push((k.trim(), v.trim()));
        }
        let get = |k: &str| kv.iter().find(|(kk, _)| *kk == k).map(|(_, v)| *v);
        let bool_of = |k: &str, d: bool| -> Result<bool, String> {
            match get(k) {
                None => Ok(d),
                Some("true") => Ok(true),
                Some("false") => Ok(false),
                Some(v) => Err(format!("bad {k}={v} in {s:?}")),
            }
        };
        let int_of = |k: &str, d: u8, lo: u8, hi: u8| -> Result<u8, String> {
            match get(k) {
                None => Ok(d),
                Some(v) => v.parse::<u8>().ok().filter(|x| *x >= lo && *x <= hi).ok_or_else(|| format!("bad {k}={v} in {s:?}")),
            }
        };
        let dir_of = |k: &str, d: Dir| -> Result<Dir, String> {
            match get(k) {
                None => Ok(d),
                Some(v) => Dir::from_name(v).ok_or_else(|| format!("bad {k}={v} in {s:?}")),
            }
        };
        let hdir_of = |k: &str, d: Dir| -> Result<Dir, String> {
            let x = dir_of(k, d)?;
            if x.is_horizontal() { Ok(x) } else { Err(format!("bad {k} in {s:?}")) }
        };
        let side_of = |k: &str| -> Result<Side, String> {
            match get(k) {
                None => Ok(Side::None),
                Some(v) => Side::parse(v).ok_or_else(|| format!("bad {k}={v} in {s:?}")),
            }
        };
        let face_of = |d: Face| -> Result<Face, String> {
            match get("face") {
                None => Ok(d),
                Some(v) => Face::parse(v).ok_or_else(|| format!("bad face={v} in {s:?}")),
            }
        };
        let waterlogged_ok = || -> Result<(), String> {
            if bool_of("waterlogged", false)? { Err(format!("waterlogged blocks are out of scope: {s:?}")) } else { Ok(()) }
        };
        if let Some(simple) = Simple::parse(name) {
            if name == "waxed_copper_grate" {
                waterlogged_ok()?;
            }
            return Ok(Block::Simple(simple));
        }
        Ok(match name {
            "air" | "cave_air" | "void_air" => Block::Air,
            "white_glazed_terracotta" => Block::Glazed { facing: hdir_of("facing", Dir::North)? },
            "redstone_wire" => Block::Wire {
                north: side_of("north")?,
                east: side_of("east")?,
                south: side_of("south")?,
                west: side_of("west")?,
                power: int_of("power", 0, 0, 15)?,
            },
            "redstone_block" => Block::RedstoneBlock,
            "redstone_torch" => Block::Torch { lit: bool_of("lit", true)? },
            "redstone_wall_torch" => Block::WallTorch { facing: hdir_of("facing", Dir::North)?, lit: bool_of("lit", true)? },
            "lever" => Block::Lever { face: face_of(Face::Wall)?, facing: hdir_of("facing", Dir::North)?, powered: bool_of("powered", false)? },
            "stone_button" | "oak_button" => Block::Button {
                wood: name == "oak_button",
                face: face_of(Face::Wall)?,
                facing: hdir_of("facing", Dir::North)?,
                powered: bool_of("powered", false)?,
            },
            "repeater" => Block::Repeater {
                facing: hdir_of("facing", Dir::North)?,
                delay: int_of("delay", 1, 1, 4)?,
                locked: bool_of("locked", false)?,
                powered: bool_of("powered", false)?,
            },
            "comparator" => Block::Comparator {
                facing: hdir_of("facing", Dir::North)?,
                subtract: match get("mode") {
                    None | Some("compare") => false,
                    Some("subtract") => true,
                    Some(v) => return Err(format!("bad mode={v}")),
                },
                powered: bool_of("powered", false)?,
            },
            "observer" => Block::Observer { facing: dir_of("facing", Dir::South)?, powered: bool_of("powered", false)? },
            "redstone_lamp" => Block::Lamp { lit: bool_of("lit", false)? },
            "waxed_copper_bulb" | "waxed_exposed_copper_bulb" | "waxed_weathered_copper_bulb" | "waxed_oxidized_copper_bulb" => Block::Bulb {
                oxid: match name {
                    "waxed_copper_bulb" => Oxid::Unaffected,
                    "waxed_exposed_copper_bulb" => Oxid::Exposed,
                    "waxed_weathered_copper_bulb" => Oxid::Weathered,
                    _ => Oxid::Oxidized,
                },
                lit: bool_of("lit", false)?,
                powered: bool_of("powered", false)?,
            },
            "note_block" => Block::Note {
                instrument: match get("instrument") {
                    None => Instrument::Harp,
                    Some(v) => Instrument::parse(v).ok_or_else(|| format!("bad instrument={v}"))?,
                },
                note: int_of("note", 0, 0, 24)?,
                powered: bool_of("powered", false)?,
            },
            "target" => Block::Target { power: int_of("power", 0, 0, 15)? },
            "iron_trapdoor" | "oak_trapdoor" => {
                waterlogged_ok()?;
                Block::Trapdoor {
                    iron: name == "iron_trapdoor",
                    facing: hdir_of("facing", Dir::North)?,
                    top: match get("half") {
                        None | Some("bottom") => false,
                        Some("top") => true,
                        Some(v) => return Err(format!("bad half={v}")),
                    },
                    open: bool_of("open", false)?,
                    powered: bool_of("powered", false)?,
                }
            }
            "iron_door" | "oak_door" => Block::Door {
                iron: name == "iron_door",
                facing: hdir_of("facing", Dir::North)?,
                upper: match get("half") {
                    None | Some("lower") => false,
                    Some("upper") => true,
                    Some(v) => return Err(format!("bad half={v}")),
                },
                hinge_right: match get("hinge") {
                    None | Some("left") => false,
                    Some("right") => true,
                    Some(v) => return Err(format!("bad hinge={v}")),
                },
                open: bool_of("open", false)?,
                powered: bool_of("powered", false)?,
            },
            "oak_fence_gate" => Block::FenceGate {
                facing: hdir_of("facing", Dir::North)?,
                in_wall: bool_of("in_wall", false)?,
                open: bool_of("open", false)?,
                powered: bool_of("powered", false)?,
            },
            "powered_rail" | "activator_rail" => {
                waterlogged_ok()?;
                Block::Rail {
                    activator: name == "activator_rail",
                    shape: match get("shape") {
                        None => RailShape::NorthSouth,
                        Some(v) => RailShape::parse(v).ok_or_else(|| format!("bad shape={v}"))?,
                    },
                    powered: bool_of("powered", false)?,
                }
            }
            "dispenser" | "dropper" => Block::Dispenser { dropper: name == "dropper", facing: dir_of("facing", Dir::North)?, triggered: bool_of("triggered", false)? },
            "crafter" => Block::Crafter {
                orientation: match get("orientation") {
                    None => 10,
                    Some(v) => ORIENTATIONS.iter().position(|o| *o == v).ok_or_else(|| format!("bad orientation={v}"))? as u8,
                },
                triggered: bool_of("triggered", false)?,
                crafting: bool_of("crafting", false)?,
            },
            "hopper" => Block::Hopper {
                facing: {
                    let d = dir_of("facing", Dir::Down)?;
                    if d == Dir::Up {
                        return Err("hopper facing up".into());
                    }
                    d
                },
                enabled: bool_of("enabled", true)?,
            },
            "smooth_stone_slab" => {
                waterlogged_ok()?;
                Block::Slab {
                    kind: match get("type") {
                        None | Some("bottom") => 0,
                        Some("top") => 1,
                        Some("double") => 2,
                        Some(v) => return Err(format!("bad type={v}")),
                    },
                }
            }
            "oak_leaves" => {
                waterlogged_ok()?;
                Block::Leaves { distance: int_of("distance", 7, 1, 7)?, persistent: bool_of("persistent", false)? }
            }
            "oak_log" => Block::Log {
                axis: match get("axis") {
                    None | Some("y") => 1,
                    Some("x") => 0,
                    Some("z") => 2,
                    Some(v) => return Err(format!("bad axis={v}")),
                },
            },
            "skeleton_skull" => Block::Skull { rotation: int_of("rotation", 0, 0, 15)?, powered: bool_of("powered", false)? },
            "skeleton_wall_skull" => Block::WallSkull { facing: hdir_of("facing", Dir::North)?, powered: bool_of("powered", false)? },
            "cake" => Block::Cake { bites: int_of("bites", 0, 0, 6)? },
            "end_portal_frame" => Block::EndPortalFrame { facing: hdir_of("facing", Dir::North)?, eye: bool_of("eye", false)? },
            "composter" => Block::Composter { level: int_of("level", 0, 0, 8)? },
            "respawn_anchor" => Block::RespawnAnchor { charges: int_of("charges", 0, 0, 4)? },
            "piston" | "sticky_piston" => Block::Piston { sticky: name == "sticky_piston", facing: dir_of("facing", Dir::North)?, extended: bool_of("extended", false)? },
            "piston_head" => Block::PistonHead {
                sticky: match get("type") {
                    None | Some("normal") => false,
                    Some("sticky") => true,
                    Some(v) => return Err(format!("bad type={v}")),
                },
                facing: dir_of("facing", Dir::North)?,
                short: bool_of("short", false)?,
            },
            "moving_piston" => Block::MovingPiston {
                sticky: match get("type") {
                    None | Some("normal") => false,
                    Some("sticky") => true,
                    Some(v) => return Err(format!("bad type={v}")),
                },
                facing: dir_of("facing", Dir::North)?,
            },
            _ => return Err(format!("block {name:?} is not modeled by redsim")),
        })
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Block::Air => write!(f, "minecraft:air"),
            Block::Simple(s) => write!(f, "minecraft:{}{}", s.name(), if s == Simple::WaxedCopperGrate { "[waterlogged=false]" } else { "" }),
            Block::Glazed { facing } => write!(f, "minecraft:white_glazed_terracotta[facing={}]", facing.name()),
            Block::Wire { north, east, south, west, power } => write!(
                f,
                "minecraft:redstone_wire[east={},north={},power={power},south={},west={}]",
                east.name(),
                north.name(),
                south.name(),
                west.name()
            ),
            Block::RedstoneBlock => write!(f, "minecraft:redstone_block"),
            Block::Torch { lit } => write!(f, "minecraft:redstone_torch[lit={}]", b(lit)),
            Block::WallTorch { facing, lit } => write!(f, "minecraft:redstone_wall_torch[facing={},lit={}]", facing.name(), b(lit)),
            Block::Lever { face, facing, powered } => write!(f, "minecraft:lever[face={},facing={},powered={}]", face.name(), facing.name(), b(powered)),
            Block::Button { wood, face, facing, powered } => write!(
                f,
                "minecraft:{}[face={},facing={},powered={}]",
                if wood { "oak_button" } else { "stone_button" },
                face.name(),
                facing.name(),
                b(powered)
            ),
            Block::Repeater { facing, delay, locked, powered } => {
                write!(f, "minecraft:repeater[delay={delay},facing={},locked={},powered={}]", facing.name(), b(locked), b(powered))
            }
            Block::Comparator { facing, subtract, powered } => write!(
                f,
                "minecraft:comparator[facing={},mode={},powered={}]",
                facing.name(),
                if subtract { "subtract" } else { "compare" },
                b(powered)
            ),
            Block::Observer { facing, powered } => write!(f, "minecraft:observer[facing={},powered={}]", facing.name(), b(powered)),
            Block::Lamp { lit } => write!(f, "minecraft:redstone_lamp[lit={}]", b(lit)),
            Block::Bulb { oxid, lit, powered } => write!(f, "minecraft:{}[lit={},powered={}]", oxid.bulb_name(), b(lit), b(powered)),
            Block::Note { instrument, note, powered } => write!(f, "minecraft:note_block[instrument={},note={note},powered={}]", instrument.name(), b(powered)),
            Block::Target { power } => write!(f, "minecraft:target[power={power}]"),
            Block::Trapdoor { iron, facing, top, open, powered } => write!(
                f,
                "minecraft:{}[facing={},half={},open={},powered={},waterlogged=false]",
                if iron { "iron_trapdoor" } else { "oak_trapdoor" },
                facing.name(),
                if top { "top" } else { "bottom" },
                b(open),
                b(powered)
            ),
            Block::Door { iron, facing, upper, hinge_right, open, powered } => write!(
                f,
                "minecraft:{}[facing={},half={},hinge={},open={},powered={}]",
                if iron { "iron_door" } else { "oak_door" },
                facing.name(),
                if upper { "upper" } else { "lower" },
                if hinge_right { "right" } else { "left" },
                b(open),
                b(powered)
            ),
            Block::FenceGate { facing, in_wall, open, powered } => {
                write!(f, "minecraft:oak_fence_gate[facing={},in_wall={},open={},powered={}]", facing.name(), b(in_wall), b(open), b(powered))
            }
            Block::Rail { activator, shape, powered } => write!(
                f,
                "minecraft:{}[powered={},shape={},waterlogged=false]",
                if activator { "activator_rail" } else { "powered_rail" },
                b(powered),
                shape.name()
            ),
            Block::Dispenser { dropper, facing, triggered } => {
                write!(f, "minecraft:{}[facing={},triggered={}]", if dropper { "dropper" } else { "dispenser" }, facing.name(), b(triggered))
            }
            Block::Crafter { orientation, triggered, crafting } => {
                write!(f, "minecraft:crafter[crafting={},orientation={},triggered={}]", b(crafting), ORIENTATIONS[orientation as usize], b(triggered))
            }
            Block::Hopper { facing, enabled } => write!(f, "minecraft:hopper[enabled={},facing={}]", b(enabled), facing.name()),
            Block::Slab { kind } => write!(f, "minecraft:smooth_stone_slab[type={},waterlogged=false]", ["bottom", "top", "double"][kind as usize]),
            Block::Leaves { distance, persistent } => write!(f, "minecraft:oak_leaves[distance={distance},persistent={},waterlogged=false]", b(persistent)),
            Block::Log { axis } => write!(f, "minecraft:oak_log[axis={}]", ["x", "y", "z"][axis as usize]),
            Block::Skull { rotation, powered } => write!(f, "minecraft:skeleton_skull[powered={},rotation={rotation}]", b(powered)),
            Block::WallSkull { facing, powered } => write!(f, "minecraft:skeleton_wall_skull[facing={},powered={}]", facing.name(), b(powered)),
            Block::Cake { bites } => write!(f, "minecraft:cake[bites={bites}]"),
            Block::EndPortalFrame { facing, eye } => write!(f, "minecraft:end_portal_frame[eye={},facing={}]", b(eye), facing.name()),
            Block::Composter { level } => write!(f, "minecraft:composter[level={level}]"),
            Block::RespawnAnchor { charges } => write!(f, "minecraft:respawn_anchor[charges={charges}]"),
            Block::Piston { sticky, facing, extended } => {
                write!(f, "minecraft:{}[extended={},facing={}]", if sticky { "sticky_piston" } else { "piston" }, b(extended), facing.name())
            }
            Block::PistonHead { sticky, facing, short } => {
                write!(f, "minecraft:piston_head[facing={},short={},type={}]", facing.name(), b(short), if sticky { "sticky" } else { "normal" })
            }
            Block::MovingPiston { sticky, facing } => write!(f, "minecraft:moving_piston[facing={},type={}]", facing.name(), if sticky { "sticky" } else { "normal" }),
        }
    }
}
