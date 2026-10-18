//! `PistonHeadBlock`.

use crate::block::{Block, Kind};
use crate::pos::BlockPos;
use crate::world::World;

/// `isFittingBase`: an EXTENDED base of the same type facing the same way.
pub fn is_fitting_base(head: Block, base: Block) -> bool {
    let Block::PistonHead { sticky, facing, .. } = head else { return false };
    matches!(base, Block::Piston { sticky: s, facing: f, extended: true } if s == sticky && f == facing)
}

/// `canSurvive`: a fitting base behind, or a moving piston with the same facing.
pub fn can_survive(w: &World, head: Block, p: BlockPos) -> bool {
    let Block::PistonHead { facing, .. } = head else { return true };
    let base = w.get(p.rel(facing.opposite()));
    is_fitting_base(head, base) || matches!(base, Block::MovingPiston { facing: f, .. } if f == facing)
}

/// `neighborChanged`: forward every neighbor update (with its source block) to the base.
pub fn neighbor_changed(w: &mut World, head: Block, p: BlockPos, src: Kind) {
    let Block::PistonHead { facing, .. } = head else { return };
    if can_survive(w, head, p) {
        w.neighbor_changed(p.rel(facing.opposite()), src);
    }
}

/// `affectNeighborsAfterRemoval`: destroy a fitting base (with drops).
pub fn affect_neighbors_after_removal(w: &mut World, head: Block, p: BlockPos) {
    let Block::PistonHead { facing, .. } = head else { return };
    let base = p.rel(facing.opposite());
    if is_fitting_base(head, w.get(base)) {
        w.destroy_block(base, true, 512);
    }
}
