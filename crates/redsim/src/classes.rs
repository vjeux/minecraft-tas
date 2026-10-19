//! Block equivalence classes derived from the model: two block states are in one class when
//! every property redsim's behavior reads gives the same answer for both, so swapping one for
//! the other can never change a trace (except the block's name). Search tools can then search
//! one symbol per class and pick any member (cheapest, nicest) afterwards.
//!
//! The signature ([`Signature`]) is everything the engine and the components ask a block:
//! `isRedstoneConductor`; `isFaceSturdy` for each of the 6 faces and each support type (FULL:
//! dust, levers, buttons, wall torches; CENTER: standing torches; RIGID: diodes, rails);
//! `isSolid` (cakes); the judge's collision flags (walkable, full cube); the piston push reaction,
//! stickiness (slime, honey), block entity and unbreakable flags (immovable), item drops (the
//! judge rejects entities), replaceability; signal source, analog output; and the block's own
//! behavior (any block with a neighbor/shape/tick/place/event behavior is its own class, keyed by
//! its block type). The note-block instrument matters only right above or below a note block, so
//! [`classes`] splits on it and [`Class::instrument_free`] groups the classes that differ only by
//! it.

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Instrument, Kind, PushReaction, Simple};
use crate::pos::Dir;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Signature {
    pub conductor: bool,
    /// bit (face * 3 + support): sturdy.
    pub sturdy: u32,
    pub solid: bool,
    pub collides: bool,
    pub full_cube: bool,
    pub push: PushReaction,
    pub sticky: u8,
    pub block_entity: bool,
    pub unbreakable: bool,
    pub drops: bool,
    pub replaceable: bool,
    pub signal_source: bool,
    pub analog: Option<i32>,
    /// Gives persistent leaves distance 0 (logs: the `PREVENTS_NEARBY_LEAF_DECAY` tag).
    pub leaf_source: bool,
    /// Sculk sensors care (`sculk.rs`): bit 0 occludes vibrations (wool), bit 1 dampens them,
    /// bit 2 resonates (amethyst).
    pub vibration: u8,
    /// None: a plain block (no behavior of its own); Some(kind): behaves as that block type.
    pub behavior: Option<Kind>,
    pub instrument: Instrument,
}

const SUPPORTS: [Support; 3] = [Support::Full, Support::Center, Support::Rigid];

/// Does the dispatcher give this block any behavior of its own (neighbor, shape, place,
/// removal, tick, event)? Plain blocks have none.
fn behaves(s: &Block) -> bool {
    !matches!(s, Block::Simple(_) | Block::Glazed { .. } | Block::Log { .. } | Block::Slab { .. })
}

pub fn signature(s: &Block) -> Signature {
    let mut sturdy = 0u32;
    for (i, f) in Dir::ALL.iter().enumerate() {
        for (j, sup) in SUPPORTS.iter().enumerate() {
            if bh::sturdy_of(s, *f, *sup) {
                sturdy |= 1 << (i * 3 + j);
            }
        }
    }
    let sticky = match s {
        Block::Simple(Simple::SlimeBlock) => 1,
        Block::Simple(Simple::HoneyBlock) => 2,
        _ => 0,
    };
    Signature {
        conductor: bh::conductor_of(s),
        sturdy,
        solid: bh::is_solid(s),
        collides: bh::collides(s),
        full_cube: bh::full_cube(s),
        push: bh::push_reaction(s),
        sticky,
        block_entity: bh::has_block_entity(s),
        unbreakable: bh::unbreakable(s),
        drops: bh::drops_item(s),
        replaceable: bh::can_be_replaced(s),
        signal_source: matches!(s, Block::RedstoneBlock | Block::Target { .. }),
        analog: bh::has_analog_output(s).then(|| match s {
            Block::Cake { bites } => (7 - *bites as i32) * 2,
            Block::EndPortalFrame { eye, .. } => {
                if *eye {
                    15
                } else {
                    0
                }
            }
            Block::Composter { level } => *level as i32,
            Block::RespawnAnchor { charges } => (*charges as f32 / 4.0 * 15.0).floor() as i32,
            _ => -1,
        }),
        leaf_source: matches!(s, Block::Log { .. }),
        vibration: (crate::sculk::occludes(s) as u8) | (crate::sculk::dampens(s) as u8) << 1 | (crate::sculk::is_resonator(s) as u8) << 2,
        behavior: behaves(s).then(|| s.kind()),
        instrument: bh::instrument(s),
    }
}

/// One equivalence class.
#[derive(Clone, Debug)]
pub struct Class {
    pub representative: Block,
    pub members: Vec<Block>,
    pub signature: Signature,
}

impl Class {
    /// The signature without the instrument (classes equal under this differ only next to a
    /// note block).
    pub fn instrument_free(&self) -> Signature {
        let mut s = self.signature.clone();
        s.instrument = Instrument::Harp;
        s
    }

    /// One line: what the class is.
    pub fn describe(&self) -> String {
        let g = &self.signature;
        let faces: Vec<String> = Dir::ALL
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                let bits: String = SUPPORTS
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| g.sturdy & (1 << (i * 3 + j)) != 0)
                    .map(|(_, s)| match s {
                        Support::Full => 'F',
                        Support::Center => 'C',
                        Support::Rigid => 'R',
                    })
                    .collect();
                (!bits.is_empty()).then(|| format!("{}:{bits}", &f.name()[..1]))
            })
            .collect();
        let sturdy = if g.sturdy == (1 << 18) - 1 { "sturdy all faces".to_string() } else if faces.is_empty() { "holds nothing".into() } else { format!("sturdy {}", faces.join(" ")) };
        format!(
            "{}{}, {sturdy}, {}{}, push {:?}{}{}{}{}, note {:?}",
            if g.conductor { "conductor" } else { "non-conductor" },
            if g.behavior.is_some() { " (own behavior)" } else { "" },
            if g.full_cube {
                "full cube"
            } else if g.collides {
                "partial collision"
            } else {
                "no collision"
            },
            if g.solid { ", solid" } else { "" },
            g.push,
            match g.sticky {
                1 => ", slime",
                2 => ", honey",
                _ => "",
            },
            if g.block_entity { ", block entity" } else { "" },
            if g.unbreakable { ", unbreakable" } else { "" },
            g.analog.map(|a| format!(", analog {a}")).unwrap_or_default(),
            g.instrument,
        )
    }
}

/// The candidate blocks: every plain block of the model in every in-plane state, and one state
/// per behaving block family (their own classes).
pub fn universe() -> Vec<Block> {
    let mut v = Vec::new();
    for name in Simple::names() {
        v.push(Block::parse(name).unwrap());
    }
    for f in Dir::HORIZONTAL {
        v.push(Block::Glazed { facing: f });
    }
    for axis in 0..3 {
        v.push(Block::Log { axis });
    }
    for kind in 0..3 {
        v.push(Block::Slab { kind });
    }
    v
}

/// The equivalence classes of [`universe`], in table order (the representative is the first
/// member: stone for the plain conductors).
pub fn classes() -> Vec<Class> {
    let mut out: Vec<Class> = Vec::new();
    for b in universe() {
        let sig = signature(&b);
        match out.iter_mut().find(|c| c.signature == sig) {
            Some(c) => c.members.push(b),
            None => out.push(Class { representative: b, members: vec![b], signature: sig }),
        }
    }
    out
}

/// The class representative of a block state (None: not in the universe).
pub fn representative(b: &Block) -> Option<Block> {
    let sig = signature(b);
    classes().into_iter().find(|c| c.signature == sig && c.members.contains(b)).map(|c| c.representative)
}
