//! `PistonStructureResolver`: which blocks a piston action moves and which it destroys.

use super::{is_pushable, rel_n};
use crate::behavior as bh;
use crate::block::{Block, PushReaction, Simple};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// `MAX_PUSH_DEPTH`.
pub const MAX_PUSH: usize = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    pub to_push: Vec<BlockPos>,
    pub to_destroy: Vec<BlockPos>,
}

/// Slime and honey blocks.
pub fn is_sticky(b: Block) -> bool {
    matches!(b, Block::Simple(Simple::SlimeBlock) | Block::Simple(Simple::HoneyBlock))
}

/// `canStickToEachOther`: slime and honey never stick to each other.
pub fn can_stick(a: Block, b: Block) -> bool {
    let honey = |x: Block| matches!(x, Block::Simple(Simple::HoneyBlock));
    let slime = |x: Block| matches!(x, Block::Simple(Simple::SlimeBlock));
    if (honey(a) && slime(b)) || (slime(a) && honey(b)) {
        return false;
    }
    is_sticky(a) || is_sticky(b)
}

struct R<'a> {
    w: &'a World,
    piston: BlockPos,
    push: Dir,
    out: Resolved,
}

/// `new PistonStructureResolver(level, piston, facing, extending).resolve()`.
pub fn resolve(w: &World, piston: BlockPos, facing: Dir, extending: bool) -> Option<Resolved> {
    let (push, start) = if extending { (facing, piston.rel(facing)) } else { (facing.opposite(), rel_n(piston, facing, 2)) };
    let mut r = R { w, piston, push, out: Resolved::default() };
    let next = w.get(start);
    if !is_pushable(next, push, false, facing) {
        if extending && bh::push_reaction(&next) == PushReaction::Popped {
            r.out.to_destroy.push(start);
            return Some(r.out);
        }
        return None;
    }
    if !r.add_block_line(start, push) {
        return None;
    }
    let mut i = 0;
    while i < r.out.to_push.len() {
        let p = r.out.to_push[i];
        if is_sticky(w.get(p)) && !r.add_branching_blocks(p) {
            return None;
        }
        i += 1;
    }
    Some(r.out)
}

impl R<'_> {
    fn add_block_line(&mut self, start: BlockPos, direction: Dir) -> bool {
        let w = self.w;
        let mut next = w.get(start);
        if next.is_air() {
            return true;
        }
        if !is_pushable(next, self.push, false, direction) {
            return true;
        }
        if start == self.piston {
            return true;
        }
        if self.out.to_push.contains(&start) {
            return true;
        }
        let back = self.push.opposite();
        let mut block_count = 1usize;
        if block_count + self.out.to_push.len() > MAX_PUSH {
            return false;
        }
        while is_sticky(next) {
            let p = rel_n(start, back, block_count as i32);
            let previous = next;
            next = w.get(p);
            if next.is_air() || !can_stick(previous, next) || !is_pushable(next, self.push, false, back) || p == self.piston {
                break;
            }
            block_count += 1;
            if block_count + self.out.to_push.len() > MAX_PUSH {
                return false;
            }
        }
        let mut blocks_added = 0usize;
        for i in (0..block_count).rev() {
            self.out.to_push.push(rel_n(start, back, i as i32));
            blocks_added += 1;
        }
        let mut i = 1;
        loop {
            let p = rel_n(start, self.push, i);
            if let Some(collision) = self.out.to_push.iter().position(|q| *q == p) {
                self.reorder_list_at_collision(blocks_added, collision);
                let mut j = 0;
                while j <= collision + blocks_added {
                    let bp = self.out.to_push[j];
                    if is_sticky(w.get(bp)) && !self.add_branching_blocks(bp) {
                        return false;
                    }
                    j += 1;
                }
                return true;
            }
            next = w.get(p);
            if next.is_air() {
                return true;
            }
            if !is_pushable(next, self.push, true, self.push) || p == self.piston {
                return false;
            }
            if bh::push_reaction(&next) == PushReaction::Popped {
                self.out.to_destroy.push(p);
                return true;
            }
            if self.out.to_push.len() >= MAX_PUSH {
                return false;
            }
            self.out.to_push.push(p);
            blocks_added += 1;
            i += 1;
        }
    }

    fn reorder_list_at_collision(&mut self, blocks_added: usize, collision: usize) {
        let v = &self.out.to_push;
        let n = v.len();
        let mut out = Vec::with_capacity(n);
        out.extend_from_slice(&v[..collision]);
        out.extend_from_slice(&v[n - blocks_added..]);
        out.extend_from_slice(&v[collision..n - blocks_added]);
        self.out.to_push = out;
    }

    fn add_branching_blocks(&mut self, from: BlockPos) -> bool {
        let from_state = self.w.get(from);
        for d in Dir::ALL {
            if d.axis() != self.push.axis() {
                let np = from.rel(d);
                let ns = self.w.get(np);
                if can_stick(ns, from_state) && !self.add_block_line(np, d) {
                    return false;
                }
            }
        }
        true
    }
}
