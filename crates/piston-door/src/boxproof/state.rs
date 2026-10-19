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
/// waxed copper bulb: a conductor; `ext` = powered, `on` = lit (toggled at once on each rising
/// edge of its input, read by comparators as 15 / 0)
pub const BULB: usize = 16;
/// comparator (compare mode; in the plane its sides face out of it, so its output is its rear
/// input): `f` = facing (input side, like repeaters), `on` = powered, `lvl` = the block
/// entity's output signal, `dl[0]` = its pending tick has HIGH priority (front is a diode not
/// facing back)
pub const COMP: usize = 17;
/// dropper (empty): an immovable conductor and power sensor (its own cell or, by
/// quasi-connectivity, the cell above); `on` = triggered (set with flags 2: observers see it, no
/// neighbor updates), a rising input schedules a dispense tick 4 gt later (empty: no effect)
pub const DROP: usize = 18;
pub const NK: usize = 19;
pub const KNAMES: [&str; NK] = ["air", "stone", "obsidian", "rblock", "dust", "repeater", "observer", "torch", "wtorch", "piston", "head", "moving", "lever", "glass", "note", "lamp", "bulb", "comparator", "dropper"];

/// Moving-block contents.
pub const MC_STONE: usize = 0;
pub const MC_RB: usize = 1;
pub const MC_OBSV: usize = 2;
pub const MC_PISTON: usize = 3;
pub const MC_HEAD: usize = 4;
pub const MC_GLASS: usize = 5;
pub const MC_NOTE: usize = 6;
pub const MC_LAMP: usize = 7;
/// a moving bulb: `mon` = lit, `mst` = powered
pub const MC_BULB: usize = 8;
pub const NMK: usize = 9;

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
    /// strong power level of a conductor (unary), comparators' levels included
    pub slvl: Vec<[B; 15]>,
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

    /// A race between two outcomes: the literal "take the other outcome" (nondeterministic
    /// mode: a free choice where the race occurs; otherwise the race is flagged and false).
    pub fn relax_choice(&mut self, race: A::B, why: &'static str) -> A::B {
        if self.a.val(race) == Some(false) {
            return self.fl();
        }
        if self.a.nondet() {
            let v = self.a.fresh();
            self.and(&[race, v])
        } else {
            self.a.flag(race, why);
            self.fl()
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
        let a = self.comp_sig15(s, n, d);
        let c = s[n];
        let cm = self.and(&[c.k[COMP], c.f[d], c.on]);
        self.or(&[a, cm])
    }

    /// `comp_sig` restricted to the sources of level 15 (comparators excluded).
    pub fn comp_sig15(&mut self, s: &[Cell<A::B>], n: usize, d: usize) -> A::B {
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

    /// Strong (direct) power of every conductor, unary level: 15 from repeaters, observers,
    /// torches below and attached levers; a comparator's own level from a comparator facing in.
    /// Returns (any strong source, cond, level).
    fn strong_lvl(&mut self, s: &[Cell<A::B>], lon: bool) -> (Vec<A::B>, Vec<A::B>, Vec<[A::B; 15]>) {
        let n = self.g.n();
        // copper bulbs are full sturdy blocks but not redstone conductors
        let cond: Vec<A::B> = (0..n).map(|i| self.a.or(&[s[i].k[STONE], s[i].k[OBSI], s[i].k[NOTE], s[i].k[LAMP], s[i].k[DROP]])).collect();
        let mut ds = Vec::with_capacity(n);
        let mut sl = Vec::with_capacity(n);
        for i in 0..n {
            let mut t = Vec::new();
            let mut cm: Vec<(A::B, [A::B; 15])> = Vec::new();
            for d in 0..4 {
                if let Some(nb) = self.g.nb(i, d) {
                    let c = s[nb];
                    t.push(self.and(&[c.k[REP], c.f[d], c.on]));
                    t.push(self.and(&[c.k[OBSV], c.f[d], c.on]));
                    if d == D {
                        t.push(self.and(&[c.k[TORCH], c.on]));
                        t.push(self.and(&[c.k[WTORCH], c.on]));
                    }
                    let x = self.and(&[c.k[COMP], c.f[d], c.on]);
                    if self.a.val(x) != Some(false) {
                        cm.push((x, c.lvl));
                    }
                }
            }
            t.push(self.levers_sup(i, lon));
            let s15 = self.or(&t);
            let mut any = vec![s15];
            any.extend(cm.iter().map(|(x, _)| *x));
            let a = self.or(&any);
            ds.push(a);
            let mut l = [s15; 15];
            if !cm.is_empty() {
                for j in 0..15 {
                    let mut v = vec![s15];
                    for (x, cl) in &cm {
                        v.push(self.and(&[*x, cl[j]]));
                    }
                    l[j] = self.or(&v);
                }
            }
            for j in 0..15 {
                l[j] = self.and(&[cond[i], l[j]]);
            }
            sl.push(l);
        }
        (ds, cond, sl)
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

    /// Unary level of the source signal into the dust at i (no wire signals): levers,
    /// components (15), comparators (their level), strongly powered conductors (their level).
    fn dust_src(&mut self, s: &[Cell<A::B>], sl: &[[A::B; 15]], i: usize, lon: bool) -> [A::B; 15] {
        let (x, y) = self.g.xy(i);
        let mut b15 = Vec::new();
        let mut lv: Vec<[A::B; 15]> = Vec::new();
        for d in 0..4 {
            let p = (x + DIRS[d].0, y + DIRS[d].1);
            b15.push(self.levers_at(p, lon));
            if let Some(n) = self.g.idx(p.0, p.1) {
                b15.push(self.comp_sig15(s, n, d));
                let c = s[n];
                let cm = self.and(&[c.k[COMP], c.f[d], c.on]);
                if self.a.val(cm) != Some(false) {
                    let mut l = c.lvl;
                    for j in 0..15 {
                        l[j] = self.and(&[cm, l[j]]);
                    }
                    lv.push(l);
                }
                lv.push(sl[n]);
            }
        }
        let a15 = self.or(&b15);
        let mut out = [a15; 15];
        for j in 0..15 {
            let mut v = vec![a15];
            for l in &lv {
                v.push(l[j]);
            }
            let o = self.or(&v);
            out[j] = self.and(&[s[i].k[DUST], o]);
        }
        out
    }

    /// Signals of `s` with dust levels taken as given (`lvl`), no dust recomputation.
    pub fn derive_lvl(&mut self, s: &[Cell<A::B>], lon: bool, lvl: Vec<[A::B; 15]>) -> Der<A::B> {
        let n = self.g.n();
        let (ds, cond, slvl) = self.strong_lvl(s, lon);
        let mut strong = Vec::with_capacity(n);
        let mut weak = Vec::with_capacity(n);
        for i in 0..n {
            strong.push(self.and(&[cond[i], ds[i]]));
            let mut w = vec![ds[i]];
            for d in [U, E, W] {
                if let Some(nb) = self.g.nb(i, d) {
                    w.push(lvl[nb][0]);
                }
            }
            let any = self.or(&w);
            weak.push(self.and(&[cond[i], any]));
        }
        Der { cond, strong, weak, lvl, slvl }
    }

    pub fn derive(&mut self, s: &[Cell<A::B>], lon: bool) -> Der<A::B> {
        let n = self.g.n();
        let (ds, cond, slvl) = self.strong_lvl(s, lon);
        let strong: Vec<A::B> = (0..n).map(|i| self.a.and(&[cond[i], ds[i]])).collect();
        let der = Der { cond, strong, weak: vec![], lvl: vec![], slvl };
        let mut src = Vec::with_capacity(n);
        for i in 0..n {
            if self.a.val(s[i].k[DUST]) == Some(false) {
                src.push([self.fl(); 15]);
                continue;
            }
            src.push(self.dust_src(s, &der.slvl, i, lon));
        }
        let mut der = der;
        let fl = self.fl();
        let mut lvl = vec![[fl; 15]; n];
        for kk in (1..=15usize).rev() {
            for i in 0..n {
                if self.a.val(s[i].k[DUST]) == Some(false) {
                    continue;
                }
                if kk == 15 {
                    lvl[i][14] = src[i][14];
                    continue;
                }
                let mut t = vec![src[i][kk - 1]];
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

    /// Comparator rear input at i, unary (`ComparatorBlock.getInputSignal`, compare mode, no
    /// side input in the plane): a bulb behind gives its analog output (15 lit / 0), replacing
    /// any power; dust its level; a source facing in 15; a comparator facing in its level; a
    /// conductor its direct power (strong sources, comparators, dust pointing into it), and
    /// when that is below 15 a bulb right behind the conductor replaces it.
    pub fn crear(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, i: usize, lon: bool) -> [A::B; 15] {
        let fl = self.fl();
        let mut out = [fl; 15];
        if self.a.val(s[i].k[COMP]) == Some(false) {
            return out;
        }
        let (x, y) = self.g.xy(i);
        for f in [E, W] {
            let sel = self.and(&[s[i].k[COMP], s[i].f[f]]);
            if self.a.val(sel) == Some(false) {
                continue;
            }
            let rp = (x + DIRS[f].0, y + DIRS[f].1);
            let lev = self.levers_at(rp, lon);
            let mut v = [lev; 15];
            if let Some(r) = self.g.idx(rp.0, rp.1) {
                let c = s[r];
                let s15 = self.comp_sig15(s, r, f);
                let a15 = self.or(&[lev, s15]);
                let cm = self.and(&[c.k[COMP], c.f[f], c.on]);
                let nb = self.not(c.k[BULB]);
                let cnd = self.and(&[der.cond[r], nb]);
                let dn: Vec<usize> = [U, E, W].iter().filter_map(|&d| self.g.nb(r, d)).collect();
                for j in 0..15 {
                    let mut t = vec![a15, der.lvl[r][j]];
                    t.push(self.and(&[cm, c.lvl[j]]));
                    let mut cl = vec![der.slvl[r][j]];
                    for &q in &dn {
                        cl.push(der.lvl[q][j]);
                    }
                    let clv = self.or(&cl);
                    t.push(self.and(&[cnd, clv]));
                    v[j] = self.or(&t);
                }
                let lit = self.and(&[c.k[BULB], c.on]);
                for j in 0..15 {
                    v[j] = self.ite(c.k[BULB], lit, v[j]);
                    // an (empty) dropper is an analog source too: 0
                    let nd = self.not(c.k[DROP]);
                    v[j] = self.and(&[v[j], nd]);
                }
                if let Some(r2) = self.g.nb(r, f) {
                    let n14 = self.not(v[14]);
                    let ndr = self.not(c.k[DROP]);
                    let behind = self.and(&[cnd, ndr, n14, s[r2].k[BULB]]);
                    if self.a.val(behind) != Some(false) {
                        for j in 0..15 {
                            v[j] = self.ite(behind, s[r2].on, v[j]);
                        }
                    }
                    let behind_d = self.and(&[cnd, ndr, n14, s[r2].k[DROP]]);
                    if self.a.val(behind_d) != Some(false) {
                        let nb = self.not(behind_d);
                        for j in 0..15 {
                            v[j] = self.and(&[v[j], nb]);
                        }
                    }
                }
            }
            for j in 0..15 {
                let x = self.and(&[sel, v[j]]);
                out[j] = self.or(&[out[j], x]);
            }
        }
        out
    }

    /// The comparator's tick would be HIGH priority (front block a diode not facing back).
    pub fn cprio(&mut self, s: &[Cell<A::B>], i: usize) -> A::B {
        let mut v = Vec::new();
        for f in [E, W] {
            if let Some(o) = self.g.nb(i, opp(f)) {
                let dio = self.or(&[s[o].k[REP], s[o].k[COMP]]);
                let back = s[o].f[opp(f)];
                let nb = self.not(back);
                v.push(self.and(&[s[i].f[f], dio, nb]));
            }
        }
        self.or(&v)
    }

    /// Note-block instrument class of the block at `i` (as the block below a note block):
    /// 0 harp, 1 basedrum (stone, obsidian, observer), 2 hat (glass), 3 bass (note block).
    pub fn instr(&mut self, s: &[Cell<A::B>], below: Option<usize>) -> [A::B; 3] {
        match below {
            None => [self.fl(); 3],
            Some(b) => {
                let bd = self.or(&[s[b].k[STONE], s[b].k[OBSI], s[b].k[OBSV], s[b].k[DROP]]);
                [bd, s[b].k[GLASS], s[b].k[NOTE]]
            }
        }
    }

    /// Quasi-connectivity sensor at i: a signal into the cell above (E, W, U sides, an
    /// out-of-plane lever on its face).
    pub fn qcin(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, i: usize, lon: bool) -> A::B {
        let (x, y) = self.g.xy(i);
        let mut qc = Vec::new();
        for d in [E, W, U] {
            qc.push(self.sig_at(s, der, x, y + 1, d, lon, false));
        }
        if let Some(up) = self.g.nb(i, U) {
            qc.push(self.levers_oop(up, lon));
        }
        self.or(&qc)
    }

    pub fn pushable(&mut self, c: &Cell<A::B>) -> A::B {
        let ne = self.not(c.ext);
        let p = self.and(&[c.k[PISTON], ne]);
        self.or(&[c.k[STONE], c.k[RB], c.k[OBSV], c.k[GLASS], c.k[NOTE], c.k[LAMP], c.k[BULB], p])
    }
    pub fn popped(&mut self, c: &Cell<A::B>) -> A::B {
        self.or(&[c.k[DUST], c.k[REP], c.k[TORCH], c.k[WTORCH], c.k[LEVER], c.k[COMP]])
    }
    /// Full sturdy face toward `face` (as seen from the neighbor on that side).
    pub fn sturdy(&mut self, c: &Cell<A::B>, face: usize) -> A::B {
        let ne = self.not(c.ext);
        let rp = self.and(&[c.k[PISTON], ne]);
        // extended base: the back face (opposite the facing) is full
        let eb = self.and(&[c.k[PISTON], c.ext, c.f[opp(face)]]);
        // head: its front plate face is full
        let hd = self.and(&[c.k[HEAD], c.f[face]]);
        self.or(&[c.k[STONE], c.k[OBSI], c.k[RB], c.k[OBSV], c.k[GLASS], c.k[NOTE], c.k[LAMP], c.k[BULB], c.k[DROP], rp, eb, hd])
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
                REP | TORCH | WTORCH | NOTE | LAMP | COMP | DROP => eqs.push(self.eqv(a.on, b.on)),
                BULB => {
                    eqs.push(self.eqv(a.on, b.on));
                    eqs.push(self.eqv(a.ext, b.ext));
                }
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
