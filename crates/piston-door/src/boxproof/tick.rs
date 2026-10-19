//! One game tick of the whole-box model, phase by phase (parallel within a phase, with flags
//! where the game's order could matter): scheduled ticks (repeaters, then observers and
//! torches), block events in rounds (events queued while a round runs execute in the next),
//! moving blocks (age, then place). After every step `settle` recomputes dust, finds which
//! cells changed and which got neighbor updates, and schedules ticks / queues block events.

use super::alg::Alg;
use super::state::*;

pub struct Book<B> {
    pub pend: Vec<B>,
    pub newsch: Vec<[B; 8]>,
    pub schd: Vec<B>,
    pub qn: Vec<[B; 3]>,
}

pub struct Line<B> {
    pub cells: Vec<Option<usize>>,
    pub resolves: B,
    pub bad: B,
    /// moved[k-1]: cell k (1-based) is part of the pushed structure (good case)
    pub moved: Vec<B>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Flip,
    Rep,
    Norm,
    Events,
    Be,
}

impl<'g, A: Alg> Ctx<'g, A> {
    pub fn line(&mut self, s: &[Cell<A::B>], p: usize, f: usize) -> Line<A::B> {
        let mut cells = Vec::new();
        let mut run = self.t();
        let mut ends = Vec::new();
        let mut bads = Vec::new();
        for k in 1..=13i32 {
            let c = self.g.off(p, f, k);
            cells.push(c);
            match c {
                None => {
                    bads.push(run);
                    break;
                }
                Some(c) => {
                    let cc = s[c];
                    let air = self.and(&[run, cc.k[AIR]]);
                    ends.push(air);
                    let pop = self.popped(&cc);
                    bads.push(self.and(&[run, pop]));
                    let pu = self.pushable(&cc);
                    run = self.and(&[run, pu]);
                    if k == 13 {
                        ends.pop();
                        bads.pop();
                    }
                }
            }
        }
        let good = self.or(&ends);
        let bad = self.or(&bads);
        let resolves = self.or(&[good, bad]);
        let mut moved = Vec::new();
        for k in 0..ends.len() {
            let later: Vec<A::B> = ends[k + 1..].to_vec();
            moved.push(self.or(&later));
        }
        Line { cells, resolves, bad, moved }
    }

    fn sched(&mut self, bk: &mut Book<A::B>, i: usize, delay: usize, c: A::B) {
        bk.newsch[i][delay - 1] = self.or(&[bk.newsch[i][delay - 1], c]);
        bk.schd[i] = self.or(&[bk.schd[i], c]);
    }

    fn add_nc(&mut self, nc: &mut [A::B], i: usize, c: A::B) {
        nc[i] = self.or(&[nc[i], c]);
    }
    fn add_nbs(&mut self, nc: &mut [A::B], i: usize, c: A::B) {
        for j in self.g.nbs(i) {
            self.add_nc(nc, j, c);
        }
    }
    /// Repeater/observer at i with facing f toggled: its front cell and that cell's neighbors
    /// (except i) get neighbor updates.
    fn add_front(&mut self, nc: &mut [A::B], i: usize, f: &[A::B; 4], c: A::B) {
        for d in 0..4 {
            if let Some(o) = self.g.nb(i, opp(d)) {
                let cd = self.and(&[c, f[d]]);
                self.add_nc(nc, o, cd);
                for j in self.g.nbs(o) {
                    if j != i {
                        self.add_nc(nc, j, cd);
                    }
                }
            }
        }
    }

    /// The block a finished moving block becomes (non-source contents; heads separately).
    fn placed(&mut self, m: &Cell<A::B>) -> Cell<A::B> {
        let mut c = konst_cell(&self.a, AIR);
        c.sch = m.sch;
        c.k[AIR] = self.fl();
        c.k[STONE] = m.mk[MC_STONE];
        c.k[RB] = m.mk[MC_RB];
        c.k[OBSV] = m.mk[MC_OBSV];
        c.k[PISTON] = m.mk[MC_PISTON];
        c.k[HEAD] = m.mk[MC_HEAD];
        c.f = m.mf;
        c.st = m.mst;
        c.on = self.and(&[m.mk[MC_OBSV], m.mon]);
        c
    }

    /// A cell as the content of a moving block.
    fn mover(&mut self, src: &Cell<A::B>, target: &Cell<A::B>, f: usize, st: A::B, ext: A::B) -> Cell<A::B> {
        let mut c = konst_cell(&self.a, MOVING);
        c.sch = target.sch;
        c.f[f] = self.t();
        c.st = st;
        c.mk[MC_STONE] = src.k[STONE];
        c.mk[MC_RB] = src.k[RB];
        c.mk[MC_OBSV] = src.k[OBSV];
        c.mk[MC_PISTON] = src.k[PISTON];
        c.mf = src.f;
        c.mst = src.st;
        c.mon = self.and(&[src.k[OBSV], src.on]);
        c.mext = ext;
        c.msrc = self.fl();
        c.mnew = self.t();
        c
    }

    /// Placement rules of an observer a moving block puts down at i (`updateFromNeighbourShapes`
    /// fires an unpowered one; `onPlace` turns a powered one off).
    fn place_obs(&mut self, bk: &mut Book<A::B>, nc: &mut [A::B], i: usize, c: &mut Cell<A::B>, placed: A::B) {
        let isobs = self.and(&[placed, c.k[OBSV]]);
        let busy = self.or(&[bk.pend[i], bk.schd[i]]);
        let free = self.not(busy);
        let noff = self.not(c.on);
        let fire = self.and(&[isobs, noff, free]);
        self.sched(bk, i, 2, fire);
        let off = self.and(&[isobs, c.on, free]);
        let f = c.f;
        self.add_front(nc, i, &f, off);
        let noo = self.not(off);
        c.on = self.and(&[c.on, noo]);
    }

    /// Recompute dust, find changes and updates, schedule ticks and queue block events.
    #[allow(clippy::too_many_arguments)]
    pub fn settle(&mut self, pre: &[Cell<A::B>], post: &mut [Cell<A::B>], lon: bool, nc0: &[A::B], chg_x: &[A::B], chg_out: &[((i32, i32), A::B)], wtt: &[A::B], obs_due: Option<&[A::B]>, ph: Phase, bk: &mut Book<A::B>) -> super::state::Der<A::B> {
        let n = self.g.n();
        let der = self.derive(post, lon);
        for i in 0..n {
            post[i].lvl = der.lvl[i];
        }
        let mut chg = Vec::with_capacity(n);
        let mut dlc = Vec::with_capacity(n);
        for i in 0..n {
            let same = self.same(&pre[i], &post[i]);
            let c = self.not(same);
            chg.push(self.or(&[c, chg_x[i]]));
            let both = self.and(&[pre[i].k[DUST], post[i].k[DUST]]);
            let mut df = Vec::new();
            for j in 0..15 {
                df.push(self.xor(pre[i].lvl[j], post[i].lvl[j]));
            }
            let d = self.or(&df);
            dlc.push(self.and(&[both, d]));
        }
        let mut nc = nc0.to_vec();
        let mut ncd = Vec::with_capacity(n);
        for i in 0..n {
            let ds: Vec<A::B> = self.g.diamond2(i).into_iter().map(|j| dlc[j]).collect();
            let any = self.or(&ds);
            ncd.push(any);
            self.add_nc(&mut nc, i, any);
        }
        // components updated only by the step's own actions check before dust settles
        let pre_lvl: Vec<[A::B; 15]> = pre.iter().map(|c| c.lvl).collect();
        let early = if self.flags_on { Some(self.derive_lvl(post, lon, pre_lvl)) } else { None };
        // observers
        for o in 0..n {
            let stay = self.and(&[pre[o].k[OBSV], post[o].k[OBSV]]);
            if self.a.val(stay) == Some(false) {
                continue;
            }
            let (x, y) = self.g.xy(o);
            let mut seen = Vec::new();
            for f in 0..4 {
                let wp = (x + DIRS[f].0, y + DIRS[f].1);
                let cw = match self.g.idx(wp.0, wp.1) {
                    Some(w) => chg[w],
                    None => {
                        let v: Vec<A::B> = chg_out.iter().filter(|(p, _)| *p == wp).map(|(_, c)| *c).collect();
                        self.or(&v)
                    }
                };
                seen.push(self.and(&[post[o].f[f], cw]));
            }
            let sees = self.or(&seen);
            let busy = self.or(&[bk.pend[o], bk.schd[o]]);
            let free = self.not(busy);
            let noff = self.not(pre[o].on);
            let mut fire = self.and(&[stay, sees, noff, free]);
            if let Some(due) = obs_due {
                let nd = self.not(due[o]);
                let race = self.and(&[stay, sees, due[o], pre[o].on]);
                self.a.flag(race, "observer turning off sees a change in the same phase");
                fire = self.and(&[fire, nd]);
            }
            self.sched(bk, o, 2, fire);
        }
        // repeaters and torches
        for i in 0..n {
            let isr = post[i].k[REP];
            let ist = self.or(&[post[i].k[TORCH], post[i].k[WTORCH]]);
            if self.a.val(isr) == Some(false) && self.a.val(ist) == Some(false) {
                continue;
            }
            let busy = self.or(&[bk.pend[i], bk.schd[i], wtt[i]]);
            let free = self.not(busy);
            let rin = self.rin(post, &der, i, lon);
            let mis = self.xor(post[i].on, rin);
            if let Some(early) = &early {
                let nd = self.not(ncd[i]);
                let only0 = self.and(&[nc0[i], nd]);
                let rin_e = self.rin(post, early, i, lon);
                let rch = self.xor(rin, rin_e);
                let rr = self.and(&[only0, isr, rch, free]);
                self.a.flag(rr, "repeater checked before dust settled");
                let tin_e = self.tin(post, early, i, lon);
                let tin_f = self.tin(post, &der, i, lon);
                let tch = self.xor(tin_e, tin_f);
                let tr = self.and(&[only0, ist, tch, free]);
                self.a.flag(tr, "torch checked before dust settled");
            }
            for dl in 0..4 {
                let c = self.and(&[nc[i], isr, post[i].dl[dl], mis, free]);
                self.sched(bk, i, 2 * (dl + 1), c);
            }
            let tin = self.tin(post, &der, i, lon);
            let tm = self.eqv(post[i].on, tin);
            let c = self.and(&[nc[i], ist, tm, free]);
            self.sched(bk, i, 2, c);
        }
        // pistons
        for p in 0..n {
            let isp = post[p].k[PISTON];
            if self.a.val(isp) == Some(false) {
                continue;
            }
            let mut up = vec![nc[p]];
            let mut upd_d = vec![ncd[p]];
            for f in 0..4 {
                if let Some(h) = self.g.nb(p, f) {
                    let fits = self.eqv(post[h].st, post[p].st);
                    let base_ok = self.and(&[post[p].k[PISTON], post[p].ext, post[p].f[f], fits]);
                    let mv = self.and(&[post[p].k[MOVING], post[p].f[f]]);
                    let surv = self.or(&[base_ok, mv]);
                    up.push(self.and(&[nc[h], post[h].k[HEAD], post[h].f[f], surv]));
                    upd_d.push(self.and(&[ncd[h], post[h].k[HEAD], post[h].f[f], surv]));
                }
            }
            let upd = self.or(&up);
            let upd = self.and(&[upd, isp]);
            let anyd = self.or(&upd_d);
            if self.a.val(upd) == Some(false) {
                continue;
            }
            let pw = self.ppow(post, &der, p, lon);
            if let Some(early) = &early {
                let nd = self.not(anyd);
                let only0 = self.and(&[upd, nd]);
                let pe = self.ppow(post, early, p, lon);
                let ch = self.xor(pe, pw);
                let r = self.and(&[only0, ch]);
                self.a.flag(r, "piston checked before dust settled");
            }
            let mut res = Vec::new();
            let mut e2 = Vec::new();
            for f in 0..4 {
                let l = self.line(post, p, f);
                res.push(self.and(&[post[p].f[f], l.resolves]));
                if let Some(c2) = self.g.off(p, f, 2) {
                    let mut t = vec![post[p].f[f], post[c2].k[MOVING], post[c2].f[f], post[c2].mext];
                    if ph == Phase::Be {
                        t.push(self.not(pre[c2].mold));
                    }
                    e2.push(self.and(&t));
                }
            }
            let res = self.or(&res);
            let e2 = self.or(&e2);
            let ne = self.not(post[p].ext);
            let e0 = self.and(&[upd, ne, pw, res]);
            let npw = self.not(pw);
            let rt = self.and(&[upd, post[p].ext, npw]);
            let r2 = self.and(&[rt, e2]);
            let ne2 = self.not(e2);
            let r1 = self.and(&[rt, ne2]);
            bk.qn[p][0] = self.or(&[bk.qn[p][0], e0]);
            bk.qn[p][1] = self.or(&[bk.qn[p][1], r1]);
            bk.qn[p][2] = self.or(&[bk.qn[p][2], r2]);
        }
        der
    }

    fn new_book(&mut self, s: &[Cell<A::B>], flip: bool) -> Book<A::B> {
        let n = self.g.n();
        let f = self.fl();
        let pend = (0..n).map(|i| if flip { self.a.or(&s[i].sch) } else { self.a.or(&s[i].sch[1..]) }).collect();
        Book { pend, newsch: vec![[f; 8]; n], schd: vec![f; n], qn: vec![[f; 3]; n] }
    }

    /// The lever switched to `lon` between ticks (state must already carry levels for !lon).
    pub fn flip(&mut self, s: &[Cell<A::B>], lon: bool) -> Vec<Cell<A::B>> {
        let n = self.g.n();
        let mut bk = self.new_book(s, true);
        let fl = self.fl();
        let mut nc0 = vec![fl; n];
        let mut chg_x = vec![fl; n];
        let mut chg_out = Vec::new();
        let levers = self.g.levers.clone();
        for (j, l) in levers.iter().enumerate() {
            let sel = self.lsel[j];
            if let Some(si) = self.g.idx(l.sup.0, l.sup.1) {
                self.add_nbs(&mut nc0, si, sel);
            }
            if !l.oop {
                match self.g.idx(l.pos.0, l.pos.1) {
                    Some(li) => {
                        self.add_nbs(&mut nc0, li, sel);
                        chg_x[li] = self.or(&[chg_x[li], sel]);
                    }
                    None => chg_out.push((l.pos, sel)),
                }
            }
        }
        let mut post = s.to_vec();
        let wtt = vec![fl; n];
        self.settle(s, &mut post, lon, &nc0, &chg_x, &chg_out, &wtt, None, Phase::Flip, &mut bk);
        for i in 0..n {
            for j in 0..8 {
                post[i].sch[j] = self.or(&[s[i].sch[j], bk.newsch[i][j]]);
            }
            for e in 0..3 {
                post[i].ev[e] = self.or(&[s[i].ev[e], bk.qn[i][e]]);
            }
        }
        post
    }

    pub fn tick(&mut self, s: &[Cell<A::B>], lon: bool) -> Vec<Cell<A::B>> {
        let n = self.g.n();
        let fl = self.fl();
        let mut bk = self.new_book(s, false);
        let due: Vec<A::B> = (0..n).map(|i| s[i].sch[0]).collect();
        let none = vec![fl; n];
        // ---- phase 1a: repeaters
        let der0 = self.derive(s, lon);
        let mut post = s.to_vec();
        let mut nc0 = vec![fl; n];
        let mut rdue = vec![fl; n];
        let mut rin0 = vec![fl; n];
        for i in 0..n {
            let d = self.and(&[due[i], s[i].k[REP]]);
            if self.a.val(d) == Some(false) {
                continue;
            }
            rdue[i] = d;
            let inp = self.rin(s, &der0, i, lon);
            rin0[i] = inp;
            let t = self.t();
            let v = self.ite(s[i].on, inp, t);
            post[i].on = self.ite(d, v, s[i].on);
            let non = self.not(s[i].on);
            let ninp = self.not(inp);
            for dl in 0..4 {
                let c = self.and(&[d, non, ninp, s[i].dl[dl]]);
                self.sched(&mut bk, i, 2 * (dl + 1), c);
            }
            let tg = self.xor(post[i].on, s[i].on);
            let tg = self.and(&[d, tg]);
            let f = s[i].f;
            self.add_front(&mut nc0, i, &f, tg);
        }
        let wtt: Vec<A::B> = (0..n).map(|i| {
            let t = self.a.or(&[s[i].k[REP], s[i].k[TORCH], s[i].k[WTORCH]]);
            self.a.and(&[due[i], t])
        }).collect();
        let der1 = self.settle(s, &mut post, lon, &nc0, &none, &[], &wtt, None, Phase::Rep, &mut bk);
        for i in 0..n {
            if !self.flags_on || self.a.val(rdue[i]) == Some(false) {
                continue;
            }
            let r1 = self.rin(&post, &der1, i, lon);
            let ch = self.xor(r1, rin0[i]);
            let race = self.and(&[rdue[i], ch]);
            self.a.flag(race, "repeater input changes within the repeater phase");
        }
        // ---- phase 1b: observers and torches
        let s1 = post.clone();
        let mut post = s1.clone();
        let mut nc0 = vec![fl; n];
        let mut odue = vec![fl; n];
        let mut tdue = vec![fl; n];
        let mut tin0 = vec![fl; n];
        for i in 0..n {
            let od = self.and(&[due[i], s1[i].k[OBSV]]);
            if self.a.val(od) != Some(false) {
                odue[i] = od;
                post[i].on = self.xor(s1[i].on, od);
                let non = self.not(s1[i].on);
                let busy = self.or(&[bk.pend[i], bk.schd[i]]);
                let free = self.not(busy);
                let c = self.and(&[od, non, free]);
                self.sched(&mut bk, i, 2, c);
                let f = s1[i].f;
                self.add_front(&mut nc0, i, &f, od);
            }
            let tk = self.or(&[s1[i].k[TORCH], s1[i].k[WTORCH]]);
            let td = self.and(&[due[i], tk]);
            if self.a.val(td) != Some(false) {
                tdue[i] = td;
                let inp = self.tin(&s1, &der1, i, lon);
                tin0[i] = inp;
                let ni = self.not(inp);
                post[i].on = self.ite(td, ni, s1[i].on);
                let tg = self.xor(post[i].on, s1[i].on);
                let tg = self.and(&[td, tg]);
                for j in self.g.diamond2(i) {
                    self.add_nc(&mut nc0, j, tg);
                }
            }
        }
        let der2 = self.settle(&s1, &mut post, lon, &nc0, &none, &[], &tdue, Some(&odue), Phase::Norm, &mut bk);
        for i in 0..n {
            if !self.flags_on || self.a.val(tdue[i]) == Some(false) {
                continue;
            }
            let t1 = self.tin(&post, &der2, i, lon);
            let ch = self.xor(t1, tin0[i]);
            let race = self.and(&[tdue[i], ch]);
            self.a.flag(race, "torch input changes within the torch phase");
        }
        // ---- phase 2: block events
        let mut q: Vec<[A::B; 3]> = (0..n).map(|i| {
            let mut e = [fl; 3];
            for k in 0..3 {
                e[k] = self.a.or(&[s[i].ev[k], bk.qn[i][k]]);
            }
            e
        }).collect();
        let mut cur = post;
        for _r in 0..self.g.rounds {
            bk.qn = vec![[fl; 3]; n];
            cur = self.round(&cur, lon, &q, &mut bk);
            q = bk.qn.clone();
        }
        let left: Vec<A::B> = q.iter().flat_map(|e| e.iter().copied()).collect();
        let left = self.or(&left);
        self.a.flag(left, "block events left after the last modeled round");
        // ---- phase 4: moving blocks
        bk.qn = vec![[fl; 3]; n];
        let out = self.be_phase(&cur, lon, &mut bk);
        let mut out = out;
        for i in 0..n {
            for j in 0..8 {
                let old = if j + 1 < 8 { s[i].sch[j + 1] } else { fl };
                out[i].sch[j] = self.or(&[old, bk.newsch[i][j]]);
            }
            out[i].ev = bk.qn[i];
        }
        out
    }

    fn round(&mut self, pre: &[Cell<A::B>], lon: bool, q: &[[A::B; 3]], bk: &mut Book<A::B>) -> Vec<Cell<A::B>> {
        let n = self.g.n();
        let fl = self.fl();
        let der = self.derive(pre, lon);
        let mut writes: Vec<Vec<(A::B, Cell<A::B>)>> = vec![Vec::new(); n];
        let mut nc0 = vec![fl; n];
        let mut execs: Vec<(usize, A::B, A::B, bool)> = Vec::new();
        let mut placed_cells: Vec<(usize, A::B)> = Vec::new();
        for p in 0..n {
            let any_ev = self.or(&q[p]);
            let isp = self.and(&[any_ev, pre[p].k[PISTON]]);
            if self.a.val(isp) == Some(false) {
                continue;
            }
            let pw = self.ppow(pre, &der, p, lon);
            let npw = self.not(pw);
            let ne = self.not(pre[p].ext);
            let st = pre[p].st;
            for f in 0..4 {
                let face = self.and(&[isp, pre[p].f[f]]);
                if self.a.val(face) == Some(false) {
                    continue;
                }
                // extend
                let l = self.line(pre, p, f);
                let dx = self.and(&[face, q[p][0], ne, pw, l.resolves]);
                if self.a.val(dx) != Some(false) {
                    let nb = self.not(l.bad);
                    let ok = self.not(dx);
                    let okk = self.or(&[ok, nb]);
                    self.a.require(okk, "a push destroys a block or leaves the box");
                    let mut base = pre[p];
                    base.ext = self.t();
                    writes[p].push((dx, base));
                    self.add_nbs(&mut nc0, p, dx);
                    if let Some(c1) = l.cells[0] {
                        let mut h = konst_cell(&self.a, MOVING);
                        h.sch = pre[c1].sch;
                        h.f[f] = self.t();
                        h.st = st;
                        h.mk[MC_HEAD] = self.t();
                        h.mf[f] = self.t();
                        h.mst = st;
                        h.mext = self.t();
                        h.msrc = self.t();
                        h.mnew = self.t();
                        writes[c1].push((dx, h));
                        self.add_nbs(&mut nc0, c1, dx);
                    }
                    for k in 0..l.moved.len() {
                        let (Some(ck), Some(Some(cn))) = (l.cells[k], l.cells.get(k + 1).copied()) else { continue };
                        let c = self.and(&[dx, l.moved[k]]);
                        if self.a.val(c) == Some(false) {
                            continue;
                        }
                        let t = self.t();
                        let m = self.mover(&pre[ck], &pre[cn], f, fl, t);
                        writes[cn].push((c, m));
                        self.add_nbs(&mut nc0, ck, c);
                        // a pushed observer facing the push sees its destination change before
                        // it is replaced: a stale tick stays at its old cell
                        let busy = self.or(&[bk.pend[ck], bk.schd[ck]]);
                        let free = self.not(busy);
                        let noff = self.not(pre[ck].on);
                        let st_tick = self.and(&[c, pre[ck].k[OBSV], pre[ck].f[f], noff, free]);
                        self.sched(bk, ck, 2, st_tick);
                        // a powered observer with a pending tick leaving its cell updates its front
                        let busy = self.or(&[bk.pend[ck], bk.schd[ck]]);
                        let o = self.and(&[c, pre[ck].k[OBSV], pre[ck].on, busy]);
                        let ff = pre[ck].f;
                        self.add_front(&mut nc0, ck, &ff, o);
                    }
                    execs.push((p, dx, pw, f == U));
                }
                // retract
                let rq = self.or(&[q[p][1], q[p][2]]);
                let dr = self.and(&[face, rq, pre[p].ext, npw]);
                if self.a.val(dr) == Some(false) {
                    continue;
                }
                let mut b = konst_cell(&self.a, MOVING);
                b.sch = pre[p].sch;
                b.f[f] = self.t();
                b.st = st;
                b.mk[MC_PISTON] = self.t();
                b.mf[f] = self.t();
                b.mst = st;
                b.msrc = self.t();
                b.mnew = self.t();
                writes[p].push((dr, b));
                self.add_nbs(&mut nc0, p, dr);
                execs.push((p, dr, pw, f == U));
                let c1 = self.g.off(p, f, 1);
                let c2 = self.g.off(p, f, 2);
                let Some(c1) = c1 else { continue };
                let hm = self.and(&[dr, pre[c1].k[MOVING]]);
                let hm_src = self.and(&[hm, pre[c1].msrc]);
                let nsrc = self.not(pre[c1].msrc);
                let hm_obj = self.and(&[hm, nsrc]);
                self.a.flag(hm_obj, "retraction final-ticks a non-source block in the head cell");
                let air = konst_cell(&self.a, AIR);
                let mut air1 = air;
                air1.sch = pre[c1].sch;
                // head cell final tick (source head -> air)
                let mut hm_nc = hm;
                let (spit, pull) = match c2 {
                    Some(c2) => {
                        let sp = self.and(&[dr, st, pre[c2].k[MOVING], pre[c2].f[f], pre[c2].mext]);
                        let nsp = self.not(sp);
                        let pu_ok = self.pushable(&pre[c2]);
                        let pl = self.and(&[dr, st, nsp, q[p][1], pu_ok]);
                        (sp, pl)
                    }
                    None => (fl, fl),
                };
                let nsp = self.not(spit);
                let npl = self.not(pull);
                // c1 result
                let rm = self.and(&[dr, nsp, npl]);
                let hs = self.and(&[hm_src, spit]);
                writes[c1].push((rm, air1));
                writes[c1].push((hs, air1));
                if let Some(c2) = c2 {
                    let f_ = fl;
                    let m = self.mover(&pre[c2], &pre[c1], f, f_, fl);
                    writes[c1].push((pull, m));
                    let mut air2 = air;
                    air2.sch = pre[c2].sch;
                    writes[c2].push((pull, air2));
                    self.add_nbs(&mut nc0, c2, pull);
                    // a pulled observer watching the head cell sees it change first
                    let busy = self.or(&[bk.pend[c2], bk.schd[c2]]);
                    let free = self.not(busy);
                    let noff = self.not(pre[c2].on);
                    let st_tick = self.and(&[pull, pre[c2].k[OBSV], pre[c2].f[opp(f)], noff, free]);
                    self.sched(bk, c2, 2, st_tick);
                    // spit: the block two ahead is placed now
                    let mut pc = self.placed(&pre[c2]);
                    pc.sch = pre[c2].sch;
                    writes[c2].push((spit, pc));
                    self.add_nbs(&mut nc0, c2, spit);
                    self.add_nc(&mut nc0, c2, spit);
                    placed_cells.push((c2, spit));
                }
                hm_nc = self.or(&[hm_nc, rm]);
                self.add_nbs(&mut nc0, c1, hm_nc);
                self.add_nc(&mut nc0, c1, hm);
            }
        }
        // combine writes
        let mut post = pre.to_vec();
        for i in 0..n {
            let ws = std::mem::take(&mut writes[i]);
            if ws.is_empty() {
                continue;
            }
            let conds: Vec<A::B> = ws.iter().map(|w| w.0).collect();
            for a in 0..conds.len() {
                for b in a + 1..conds.len() {
                    if !self.flags_on {
                        continue;
                    }
                    let both = self.and(&[conds[a], conds[b]]);
                    self.a.flag(both, "two block events write one cell in the same round");
                }
            }
            let mut c = pre[i];
            for (w, cell) in ws.iter().rev() {
                c = self.ite_cell(*w, cell, &c);
            }
            for e in 0..3 {
                c.ev[e] = fl;
            }
            post[i] = c;
        }
        for (i, c) in placed_cells {
            let mut cell = post[i];
            self.place_obs(bk, &mut nc0, i, &mut cell, c);
            post[i] = cell;
        }
        let wtt = vec![fl; n];
        let none = vec![fl; n];
        let der2 = self.settle(pre, &mut post, lon, &nc0, &none, &[], &wtt, None, Phase::Events, bk);
        // an event whose power differs at the end of the round may have seen another first
        for (p, c, pw0, up) in execs {
            if up || !self.flags_on {
                continue;
            }
            let pw1 = self.ppow(&post, &der2, p, lon);
            let ch = self.xor(pw0, pw1);
            let race = self.and(&[c, ch]);
            self.a.flag(race, "a block event's power changes within its round");
        }
        post
    }

    fn be_phase(&mut self, pre: &[Cell<A::B>], lon: bool, bk: &mut Book<A::B>) -> Vec<Cell<A::B>> {
        let n = self.g.n();
        let fl = self.fl();
        let mut post = pre.to_vec();
        let mut nc0 = vec![fl; n];
        for i in 0..n {
            let mv = pre[i].k[MOVING];
            if self.a.val(mv) == Some(false) {
                continue;
            }
            let fin = self.and(&[mv, pre[i].mold]);
            let nn = self.not(pre[i].mnew);
            let nold = self.not(pre[i].mold);
            let age = self.and(&[mv, nold, nn]);
            let mut aged = pre[i];
            aged.mold = self.or(&[pre[i].mold, age]);
            aged.mnew = fl;
            let mut pc = self.placed(&pre[i]);
            // a source head becomes a head of the moving block's type
            pc.st = pre[i].mst;
            let c = self.ite_cell(fin, &pc, &aged);
            post[i] = c;
            self.add_nbs(&mut nc0, i, fin);
            self.add_nc(&mut nc0, i, fin);
        }
        for i in 0..n {
            let fin = self.and(&[pre[i].k[MOVING], pre[i].mold]);
            if self.a.val(fin) == Some(false) {
                continue;
            }
            let mut cell = post[i];
            self.place_obs(bk, &mut nc0, i, &mut cell, fin);
            post[i] = cell;
        }
        // heads whose base no longer fits vanish
        for h in 0..n {
            let ish = post[h].k[HEAD];
            if self.a.val(ish) == Some(false) {
                continue;
            }
            let mut ok = Vec::new();
            for f in 0..4 {
                match self.g.nb(h, opp(f)) {
                    Some(b) => {
                        let fits = self.eqv(post[b].st, post[h].st);
                        let base = self.and(&[post[b].k[PISTON], post[b].ext, post[b].f[f], fits]);
                        let mv = self.and(&[post[b].k[MOVING], post[b].f[f]]);
                        let o = self.or(&[base, mv]);
                        ok.push(self.and(&[post[h].f[f], o]));
                    }
                    None => {}
                }
            }
            let ok = self.or(&ok);
            let gone = self.and(&[ish, self.a.not(ok)]);
            let mut air = konst_cell(&self.a, AIR);
            air.sch = post[h].sch;
            post[h] = self.ite_cell(gone, &air, &post[h]);
            self.add_nbs(&mut nc0, h, gone);
        }
        let wtt = vec![fl; n];
        let none = vec![fl; n];
        self.settle(pre, &mut post, lon, &nc0, &none, &[], &wtt, None, Phase::Be, bk);
        post
    }
}
