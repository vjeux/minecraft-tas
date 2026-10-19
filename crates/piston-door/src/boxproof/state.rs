//! The flat-plane state of the whole-box model and its combinational signals (redsim's
//! semantics restricted to one z layer: in-plane facings E W U D, everything outside the box
//! is air except the lever).

use super::alg::Alg;

pub const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const E: usize = 0;
pub const W: usize = 1;
pub const U: usize = 2;
pub const D: usize = 3;
#[inline]
pub fn opp(d: usize) -> usize {
    d ^ 1
}

pub const AIR: usize = 0;
pub const STONE: usize = 1;
pub const OBSI: usize = 2;
pub const RB: usize = 3;
pub const DUST: usize = 4;
pub const REP: usize = 5;
pub const OBSV: usize = 6;
pub const TORCH: usize = 7;
pub const WTORCH: usize = 8;
pub const PISTON: usize = 9;
pub const HEAD: usize = 10;
pub const MOVING: usize = 11;
pub const LEVER: usize = 12;
/// glass: a full sturdy block that pistons move, not a redstone conductor
pub const GLASS: usize = 13;
/// note block: a conductor whose `powered` follows its input at each neighbor update
/// (setBlockAndUpdate) and whose instrument follows the block below (observers see both)
pub const NOTE: usize = 14;
/// redstone lamp: a conductor, lit at once when powered, unlit 4 gt after losing power
pub const LAMP: usize = 15;
pub const NK: usize = 16;
pub const KNAMES: [&str; NK] = ["air", "stone", "obsidian", "rblock", "dust", "repeater", "observer", "torch", "wtorch", "piston", "head", "moving", "lever", "glass", "note", "lamp"];

/// Moving-block contents.
pub const MC_STONE: usize = 0;
pub const MC_RB: usize = 1;
pub const MC_OBSV: usize = 2;
pub const MC_PISTON: usize = 3;
pub const MC_HEAD: usize = 4;
pub const MC_GLASS: usize = 5;
pub const MC_NOTE: usize = 6;
pub const MC_LAMP: usize = 7;
pub const NMK: usize = 8;

/// One cell at one checkpoint. `f` is the facing of repeaters (input side), observers (watched
/// side), wall torches (away from the wall), pistons, heads, and the BE direction of moving
/// blocks. `st` is the sticky type of pistons, heads and moving blocks (the block state's type).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell<B> {
    pub k: [B; NK],
    pub f: [B; 4],
    pub st: B,
    pub ext: B,
    pub on: B,
    pub dl: [B; 4],
    /// unary dust power: lvl[j] = power >= j+1
    pub lvl: [B; 15],
    pub mk: [B; NMK],
    pub mf: [B; 4],
    pub mst: B,
    pub mon: B,
    pub mext: B,
    pub msrc: B,
    pub mold: B,
    pub mnew: B,
    /// pending scheduled tick due in j ticks from this tick (sch[0] = due now)
    pub sch: [B; 8],
    /// block event queued for the next block-event phase: extend, retract, retract-and-drop
    pub ev: [B; 3],
}

pub fn konst_cell<A: Alg>(a: &A, kind: usize) -> Cell<A::B> {
    let f = a.k(false);
    let mut k = [f; NK];
    k[kind] = a.k(true);
    Cell { k, f: [f; 4], st: f, ext: f, on: f, dl: [f; 4], lvl: [f; 15], mk: [f; NMK], mf: [f; 4], mst: f, mon: f, mext: f, msrc: f, mold: f, mnew: f, sch: [f; 8], ev: [f; 3] }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LeverCand {
    /// lever cell (may be outside the box; for `oop` the support's cell)
    pub pos: (i32, i32),
    /// out of the plane (attached to the support's face)
    pub oop: bool,
    pub sup: (i32, i32),
}

#[derive(Clone, Debug)]
pub struct Geo {
    pub w: i32,
    pub h: i32,
    pub door: (i32, i32),
    pub levers: Vec<LeverCand>,
    pub rounds: usize,
}

impl Geo {
    pub fn n(&self) -> usize {
        (self.w * self.h) as usize
    }
    pub fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }
    pub fn xy(&self, i: usize) -> (i32, i32) {
        ((i as i32) % self.w, (i as i32) / self.w)
    }
    pub fn nb(&self, i: usize, d: usize) -> Option<usize> {
        let (x, y) = self.xy(i);
        self.idx(x + DIRS[d].0, y + DIRS[d].1)
    }
    pub fn off(&self, i: usize, d: usize, k: i32) -> Option<usize> {
        let (x, y) = self.xy(i);
        self.idx(x + DIRS[d].0 * k, y + DIRS[d].1 * k)
    }
    pub fn is_door(&self, i: usize) -> bool {
        let (x, y) = self.xy(i);
        x >= self.door.0 && x < self.door.0 + 3 && y >= self.door.1 && y < self.door.1 + 3
    }
    pub fn is_ring(&self, i: usize) -> bool {
        let (x, y) = self.xy(i);
        let (dx, dy) = self.door;
        let inx = x >= dx && x < dx + 3;
        let iny = y >= dy && y < dy + 3;
        (inx && (y == dy - 1 || y == dy + 3)) || (iny && (x == dx - 1 || x == dx + 3))
    }
    /// Cells within Manhattan distance 2 (incl. i).
    pub fn diamond2(&self, i: usize) -> Vec<usize> {
        let (x, y) = self.xy(i);
        let mut v = Vec::new();
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                if dx.abs() + dy.abs() <= 2 {
                    if let Some(j) = self.idx(x + dx, y + dy) {
                        v.push(j);
                    }
                }
            }
        }
        v
    }
    pub fn nbs(&self, i: usize) -> Vec<usize> {
        (0..4).filter_map(|d| self.nb(i, d)).collect()
    }
}

/// Combinational signals of one state.
pub struct Der<B> {
    pub cond: Vec<B>,
    pub strong: Vec<B>,
    pub weak: Vec<B>,
    pub lvl: Vec<[B; 15]>,
}

pub struct Ctx<'g, A: Alg> {
    pub a: A,
    pub g: &'g Geo,
    pub lsel: Vec<A::B>,
    /// compute the race / unmodeled-case flags (off: cheaper circuit, models checked by CEGAR)
    pub flags_on: bool,
}

impl<'g, A: Alg> Ctx<'g, A> {
    pub fn t(&self) -> A::B {
        self.a.k(true)
    }
    pub fn fl(&self) -> A::B {
        self.a.k(false)
    }
    pub fn and(&mut self, xs: &[A::B]) -> A::B {
        self.a.and(xs)
    }
    pub fn or(&mut self, xs: &[A::B]) -> A::B {
        self.a.or(xs)
    }
    pub fn not(&self, x: A::B) -> A::B {
        self.a.not(x)
    }
    pub fn xor(&mut self, x: A::B, y: A::B) -> A::B {
        self.a.xor(x, y)
    }
    pub fn ite(&mut self, c: A::B, x: A::B, y: A::B) -> A::B {
        self.a.ite(c, x, y)
    }
    pub fn eqv(&mut self, x: A::B, y: A::B) -> A::B {
        let d = self.a.xor(x, y);
        self.a.not(d)
    }

    /// A race on an outcome: in nondeterministic mode (`FlagMode::Nondet`) the outcome may take
    /// either value (sound: the game's real order gives one of them); otherwise the race is a
    /// flag (forbidden in prove mode, recorded in find mode) and the parallel value is kept.
    pub fn relax(&mut self, race: A::B, det: A::B, why: &'static str) -> A::B {
        if self.a.val(race) == Some(false) {
            return det;
        }
        if self.a.nondet() {
            let v = self.a.fresh();
            self.ite(race, v, det)
        } else {
            self.a.flag(race, why);
            det
        }
    }

    /// In-plane levers standing at `pos`, powered.
    pub fn levers_at(&mut self, pos: (i32, i32), lon: bool) -> A::B {
        if !lon {
            return self.fl();
        }
        let v: Vec<A::B> = self.g.levers.iter().enumerate().filter(|(_, l)| !l.oop && l.pos == pos).map(|(j, _)| self.lsel[j]).collect();
        self.a.or(&v)
    }
    /// Levers attached to cell i (any kind), powered.
    pub fn levers_sup(&mut self, i: usize, lon: bool) -> A::B {
        if !lon {
            return self.fl();
        }
        let p = self.g.xy(i);
        let v: Vec<A::B> = self.g.levers.iter().enumerate().filter(|(_, l)| l.sup == p).map(|(j, _)| self.lsel[j]).collect();
        self.a.or(&v)
    }
    /// Out-of-plane levers on the face of cell i, powered (their QC reaches the piston below i).
    pub fn levers_oop(&mut self, i: usize, lon: bool) -> A::B {
        if !lon {
            return self.fl();
        }
        let p = self.g.xy(i);
        let v: Vec<A::B> = self.g.levers.iter().enumerate().filter(|(_, l)| l.oop && l.sup == p).map(|(j, _)| self.lsel[j]).collect();
        self.a.or(&v)
    }

    /// The component signal of the block at cell `n` toward its neighbor that lies in direction
    /// opp(d) from it (the receiver looks in direction d), conductors and dust excluded.
    fn comp_sig(&mut self, s: &[Cell<A::B>], n: usize, d: usize) -> A::B {
        let c = s[n];
        let nf = self.not(c.f[d]);
        let mut t = vec![c.k[RB]];
        t.push(self.and(&[c.k[REP], c.f[d], c.on]));
        t.push(self.and(&[c.k[OBSV], c.f[d], c.on]));
        if d != U {
            t.push(self.and(&[c.k[TORCH], c.on]));
        }
        t.push(self.and(&[c.k[WTORCH], c.on, nf]));
        self.or(&t)
    }

    /// Signal into the receiver at (x, y) from its neighbor in direction d (`Level.getSignal` >
    /// 0); `dust_rx`: as a wire computes it (no wire signals, conductors only strongly).
    pub fn sig_at(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, x: i32, y: i32, d: usize, lon: bool, dust_rx: bool) -> A::B {
        let p = (x + DIRS[d].0, y + DIRS[d].1);
        let mut t = vec![self.levers_at(p, lon)];
        if let Some(n) = self.g.idx(p.0, p.1) {
            t.push(self.comp_sig(s, n, d));
            if dust_rx {
                t.push(der.strong[n]);
            } else {
                t.push(der.weak[n]);
                if d != D && !der.lvl.is_empty() {
                    t.push(der.lvl[n][0]);
                }
            }
        }
        self.or(&t)
    }

    /// Signals of `s` with dust levels taken as given (`lvl`), no dust recomputation.
    pub fn derive_lvl(&mut self, s: &[Cell<A::B>], lon: bool, lvl: Vec<[A::B; 15]>) -> Der<A::B> {
        let n = self.g.n();
        let cond: Vec<A::B> = (0..n).map(|i| self.a.or(&[s[i].k[STONE], s[i].k[OBSI], s[i].k[NOTE], s[i].k[LAMP]])).collect();
        let mut strong = Vec::with_capacity(n);
        let mut weak = Vec::with_capacity(n);
        for i in 0..n {
            let mut t = Vec::new();
            for d in 0..4 {
                if let Some(nb) = self.g.nb(i, d) {
                    let c = s[nb];
                    t.push(self.and(&[c.k[REP], c.f[d], c.on]));
                    t.push(self.and(&[c.k[OBSV], c.f[d], c.on]));
                    if d == D {
                        t.push(self.and(&[c.k[TORCH], c.on]));
                        t.push(self.and(&[c.k[WTORCH], c.on]));
                    }
                }
            }
            t.push(self.levers_sup(i, lon));
            let ds = self.or(&t);
            strong.push(self.and(&[cond[i], ds]));
            let mut w = vec![ds];
            for d in [U, E, W] {
                if let Some(nb) = self.g.nb(i, d) {
                    w.push(lvl[nb][0]);
                }
            }
            let any = self.or(&w);
            weak.push(self.and(&[cond[i], any]));
        }
        Der { cond, strong, weak, lvl }
    }

    pub fn derive(&mut self, s: &[Cell<A::B>], lon: bool) -> Der<A::B> {
        let n = self.g.n();
        let cond: Vec<A::B> = (0..n).map(|i| self.a.or(&[s[i].k[STONE], s[i].k[OBSI], s[i].k[NOTE], s[i].k[LAMP]])).collect();
        let mut ds = Vec::with_capacity(n);
        for i in 0..n {
            let mut t = Vec::new();
            for d in 0..4 {
                if let Some(nb) = self.g.nb(i, d) {
                    let c = s[nb];
                    t.push(self.and(&[c.k[REP], c.f[d], c.on]));
                    t.push(self.and(&[c.k[OBSV], c.f[d], c.on]));
                    if d == D {
                        t.push(self.and(&[c.k[TORCH], c.on]));
                        t.push(self.and(&[c.k[WTORCH], c.on]));
                    }
                }
            }
            t.push(self.levers_sup(i, lon));
            ds.push(self.or(&t));
        }
        let strong: Vec<A::B> = (0..n).map(|i| self.a.and(&[cond[i], ds[i]])).collect();
        let mut der = Der { cond, strong, weak: vec![], lvl: vec![] };
        let mut src = Vec::with_capacity(n);
        for i in 0..n {
            if self.a.val(s[i].k[DUST]) == Some(false) {
                src.push(self.fl());
                continue;
            }
            let (x, y) = self.g.xy(i);
            let mut t = Vec::new();
            for d in 0..4 {
                t.push(self.sig_at(s, &der, x, y, d, lon, true));
            }
            let any = self.or(&t);
            src.push(self.and(&[s[i].k[DUST], any]));
        }
        let fl = self.fl();
        let mut lvl = vec![[fl; 15]; n];
        for kk in (1..=15usize).rev() {
            for i in 0..n {
                if self.a.val(s[i].k[DUST]) == Some(false) {
                    continue;
                }
                if kk == 15 {
                    lvl[i][14] = src[i];
                    continue;
                }
                let mut t = vec![src[i]];
                let above = self.g.nb(i, U);
                let nca = match above {
                    Some(a) => self.not(der.cond[a]),
                    None => self.t(),
                };
                for d in [E, W] {
                    if let Some(sd) = self.g.nb(i, d) {
                        t.push(lvl[sd][kk]);
                        if let Some(up) = self.g.nb(sd, U) {
                            let x = self.and(&[der.cond[sd], nca, lvl[up][kk]]);
                            t.push(x);
                        }
                        if let Some(dn) = self.g.nb(sd, D) {
                            let ncs = self.not(der.cond[sd]);
                            let x = self.and(&[ncs, lvl[dn][kk]]);
                            t.push(x);
                        }
                    }
                }
                let any = self.or(&t);
                lvl[i][kk - 1] = self.and(&[s[i].k[DUST], any]);
            }
        }
        let mut weak = Vec::with_capacity(n);
        for i in 0..n {
            let mut t = vec![ds[i]];
            for d in [U, E, W] {
                if let Some(nb) = self.g.nb(i, d) {
                    t.push(lvl[nb][0]);
                }
            }
            let any = self.or(&t);
            weak.push(self.and(&[der.cond[i], any]));
        }
        der.weak = weak;
        der.lvl = lvl;
        der
    }

    /// Piston power at cell p (facing from the cell's `f`): 5 sides + quasi-connectivity.
    pub fn ppow(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, p: usize, lon: bool) -> A::B {
        let (x, y) = self.g.xy(p);
        let mut sides = Vec::with_capacity(4);
        for d in 0..4 {
            sides.push(self.sig_at(s, der, x, y, d, lon, false));
        }
        let mut qc = Vec::new();
        for d in [E, W, U] {
            qc.push(self.sig_at(s, der, x, y + 1, d, lon, false));
        }
        if let Some(up) = self.g.nb(p, U) {
            qc.push(self.levers_oop(up, lon));
        }
        let q = self.or(&qc);
        let mut by = Vec::new();
        for f in 0..4 {
            if self.a.val(s[p].f[f]) == Some(false) {
                continue;
            }
            let mut o: Vec<A::B> = (0..4).filter(|&d| d != f).map(|d| sides[d]).collect();
            o.push(q);
            let any = self.or(&o);
            by.push(self.and(&[s[p].f[f], any]));
        }
        self.or(&by)
    }

    /// Repeater input (> 0) at r.
    pub fn rin(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, r: usize, lon: bool) -> A::B {
        let (x, y) = self.g.xy(r);
        let a = self.sig_at(s, der, x, y, E, lon, false);
        let b = self.sig_at(s, der, x, y, W, lon, false);
        let pa = self.and(&[s[r].f[E], a]);
        let pb = self.and(&[s[r].f[W], b]);
        self.or(&[pa, pb])
    }

    /// Torch input at t (standing: the block below; wall: the block it hangs on).
    pub fn tin(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, t: usize, lon: bool) -> A::B {
        let (x, y) = self.g.xy(t);
        let below = self.sig_at(s, der, x, y, D, lon, false);
        let st = self.and(&[s[t].k[TORCH], below]);
        let mut v = vec![st];
        for f in [E, W] {
            let sup = self.sig_at(s, der, x, y, opp(f), lon, false);
            v.push(self.and(&[s[t].k[WTORCH], s[t].f[f], sup]));
        }
        self.or(&v)
    }

    /// `hasNeighborSignal` of a conductor-like receiver at i (note block, lamp): any neighbor's
    /// signal toward it, an out-of-plane lever on its face included.
    pub fn nin(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, i: usize, lon: bool) -> A::B {
        let (x, y) = self.g.xy(i);
        let mut v = Vec::new();
        for d in 0..4 {
            v.push(self.sig_at(s, der, x, y, d, lon, false));
        }
        v.push(self.levers_oop(i, lon));
        self.or(&v)
    }

    /// Note-block instrument class of the block at `i` (as the block below a note block):
    /// 0 harp, 1 basedrum (stone, obsidian, observer), 2 hat (glass), 3 bass (note block).
    pub fn instr(&mut self, s: &[Cell<A::B>], below: Option<usize>) -> [A::B; 3] {
        match below {
            None => [self.fl(); 3],
            Some(b) => {
                let bd = self.or(&[s[b].k[STONE], s[b].k[OBSI], s[b].k[OBSV]]);
                [bd, s[b].k[GLASS], s[b].k[NOTE]]
            }
        }
    }

    pub fn pushable(&mut self, c: &Cell<A::B>) -> A::B {
        let ne = self.not(c.ext);
        let p = self.and(&[c.k[PISTON], ne]);
        self.or(&[c.k[STONE], c.k[RB], c.k[OBSV], c.k[GLASS], c.k[NOTE], c.k[LAMP], p])
    }
    pub fn popped(&mut self, c: &Cell<A::B>) -> A::B {
        self.or(&[c.k[DUST], c.k[REP], c.k[TORCH], c.k[WTORCH], c.k[LEVER]])
    }
    /// Full sturdy face toward `face` (as seen from the neighbor on that side).
    pub fn sturdy(&mut self, c: &Cell<A::B>, face: usize) -> A::B {
        let ne = self.not(c.ext);
        let rp = self.and(&[c.k[PISTON], ne]);
        // extended base: the back face (opposite the facing) is full
        let eb = self.and(&[c.k[PISTON], c.ext, c.f[opp(face)]]);
        // head: its front plate face is full
        let hd = self.and(&[c.k[HEAD], c.f[face]]);
        self.or(&[c.k[STONE], c.k[OBSI], c.k[RB], c.k[OBSV], c.k[GLASS], c.k[NOTE], c.k[LAMP], rp, eb, hd])
    }

    /// Same block state (what a shape update reports as a change).
    pub fn same(&mut self, a: &Cell<A::B>, b: &Cell<A::B>) -> A::B {
        let mut t = Vec::new();
        for k in 0..NK {
            let both = self.and(&[a.k[k], b.k[k]]);
            if self.a.val(both) == Some(false) {
                continue;
            }
            let mut eqs = vec![both];
            match k {
                DUST => {
                    for j in 0..15 {
                        eqs.push(self.eqv(a.lvl[j], b.lvl[j]));
                    }
                }
                REP | TORCH | WTORCH | NOTE | LAMP => eqs.push(self.eqv(a.on, b.on)),
                OBSV => {
                    eqs.push(self.eqv(a.on, b.on));
                    for f in 0..4 {
                        eqs.push(self.eqv(a.f[f], b.f[f]));
                    }
                }
                PISTON | HEAD | MOVING => {
                    eqs.push(self.eqv(a.st, b.st));
                    if k == PISTON {
                        eqs.push(self.eqv(a.ext, b.ext));
                    }
                    for f in 0..4 {
                        eqs.push(self.eqv(a.f[f], b.f[f]));
                    }
                }
                _ => {}
            }
            t.push(self.and(&eqs));
        }
        self.or(&t)
    }

    pub fn ite_cell(&mut self, c: A::B, x: &Cell<A::B>, y: &Cell<A::B>) -> Cell<A::B> {
        let mut r = *y;
        macro_rules! m {
            ($($f:ident),*) => { $( for j in 0..r.$f.len() { r.$f[j] = self.ite(c, x.$f[j], y.$f[j]); } )* };
        }
        macro_rules! s {
            ($($f:ident),*) => { $( r.$f = self.ite(c, x.$f, y.$f); )* };
        }
        m!(k, f, dl, lvl, mk, mf, sch, ev);
        s!(st, ext, on, mst, mon, mext, msrc, mold, mnew);
        r
    }
}
