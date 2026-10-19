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

    pub(super) fn sched(&mut self, bk: &mut Book<A::B>, i: usize, delay: usize, c: A::B) {
        bk.newsch[i][delay - 1] = self.or(&[bk.newsch[i][delay - 1], c]);
        bk.schd[i] = self.or(&[bk.schd[i], c]);
    }

    pub(super) fn add_nc(&mut self, nc: &mut [A::B], i: usize, c: A::B) {
        nc[i] = self.or(&[nc[i], c]);
    }
    pub(super) fn add_nbs(&mut self, nc: &mut [A::B], i: usize, c: A::B) {
        for j in self.g.nbs(i) {
            self.add_nc(nc, j, c);
        }
    }
    /// Repeater/observer at i with facing f toggled: its front cell and that cell's neighbors
    /// (except i) get neighbor updates.
    pub(super) fn add_front(&mut self, nc: &mut [A::B], i: usize, f: &[A::B; 4], c: A::B) {
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
    pub(super) fn placed(&mut self, m: &Cell<A::B>) -> Cell<A::B> {
        let mut c = konst_cell(&self.a, AIR);
        c.sch = m.sch;
        c.k[AIR] = self.fl();
        c.k[STONE] = m.mk[MC_STONE];
        c.k[GLASS] = m.mk[MC_GLASS];
        c.k[RB] = m.mk[MC_RB];
        c.k[OBSV] = m.mk[MC_OBSV];
        c.k[PISTON] = m.mk[MC_PISTON];
        c.k[HEAD] = m.mk[MC_HEAD];
        c.k[NOTE] = m.mk[MC_NOTE];
        c.k[LAMP] = m.mk[MC_LAMP];
        c.k[BULB] = m.mk[MC_BULB];
        c.f = m.mf;
        c.st = m.mst;
        let has_on = self.or(&[m.mk[MC_OBSV], m.mk[MC_NOTE], m.mk[MC_LAMP], m.mk[MC_BULB]]);
        c.on = self.and(&[has_on, m.mon]);
        // a bulb keeps its powered state in `mst` while moving
        c.ext = self.and(&[m.mk[MC_BULB], m.mst]);
        c
    }

    /// A cell as the content of a moving block.
    pub(super) fn mover(&mut self, src: &Cell<A::B>, target: &Cell<A::B>, f: usize, st: A::B, ext: A::B) -> Cell<A::B> {
        let mut c = konst_cell(&self.a, MOVING);
        c.sch = target.sch;
        c.f[f] = self.t();
        c.st = st;
        c.mk[MC_STONE] = src.k[STONE];
        c.mk[MC_GLASS] = src.k[GLASS];
        c.mk[MC_RB] = src.k[RB];
        c.mk[MC_OBSV] = src.k[OBSV];
        c.mk[MC_PISTON] = src.k[PISTON];
        c.mk[MC_NOTE] = src.k[NOTE];
        c.mk[MC_LAMP] = src.k[LAMP];
        c.mk[MC_BULB] = src.k[BULB];
        c.mf = src.f;
        c.mst = self.ite(src.k[BULB], src.ext, src.st);
        let has_on = self.or(&[src.k[OBSV], src.k[NOTE], src.k[LAMP], src.k[BULB]]);
        c.mon = self.and(&[has_on, src.on]);
        c.mext = ext;
        c.msrc = self.fl();
        c.mnew = self.t();
        c
    }

    /// Placement rules of an observer a moving block puts down at i (`updateFromNeighbourShapes`
    /// fires an unpowered one; `onPlace` turns a powered one off).
    pub(super) fn place_obs(&mut self, bk: &mut Book<A::B>, nc: &mut [A::B], i: usize, c: &mut Cell<A::B>, placed: A::B) {
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
        let prof = std::env::var("BP_PROF").is_ok();
        let c0 = self.a.cost();
        let der = self.derive(post, lon);
        let c1 = self.a.cost();
        for i in 0..n {
            // dust levels from the fixpoint; a comparator keeps its block entity's output
            let keep = post[i].k[COMP];
            for j in 0..15 {
                let cl = self.and(&[keep, post[i].lvl[j]]);
                post[i].lvl[j] = self.or(&[der.lvl[i][j], cl]);
            }
        }
        let mut chg = Vec::with_capacity(n);
        let mut dlc = Vec::with_capacity(n);
        for i in 0..n {
            let both = self.and(&[pre[i].k[DUST], post[i].k[DUST]]);
            let d = if self.a.val(both) == Some(false) {
                self.fl()
            } else {
                let mut df = Vec::new();
                for j in 0..15 {
                    df.push(self.xor(pre[i].lvl[j], post[i].lvl[j]));
                }
                let d = self.or(&df);
                self.and(&[both, d])
            };
            dlc.push(d);
            // a cell's state changed: a direct change of the step (`chg_x`) or its dust level
            chg.push(self.or(&[d, chg_x[i]]));
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
        // a reactive block (note, lamp, bulb, dropper) updated only by the step's own actions
        // may read dust that has not settled yet: its input then differs between the stale and
        // the settled dust (a race: flagged, or either outcome)
        let mut react_race = vec![self.fl(); n];
        if let Some(early) = &early {
            for i in 0..n {
                let r = self.or(&[post[i].k[NOTE], post[i].k[LAMP], post[i].k[BULB], post[i].k[DROP]]);
                if self.a.val(r) == Some(false) {
                    continue;
                }
                let a = self.nin(post, &der, i, lon);
                let b = self.nin(post, early, i, lon);
                let mut d = vec![self.xor(a, b)];
                if self.a.val(post[i].k[DROP]) != Some(false) {
                    let qa = self.qcin(post, &der, i, lon);
                    let qb = self.qcin(post, early, i, lon);
                    let x = self.xor(qa, qb);
                    d.push(self.and(&[post[i].k[DROP], x]));
                }
                let ch = self.or(&d);
                let nd = self.not(ncd[i]);
                react_race[i] = self.and(&[nc0[i], nd, r, ch]);
            }
        }
        // note blocks and lamps react at once to their neighbor updates (their own state does
        // not change any signal): a note block re-reads its input and, if it differs, updates
        // its 6 neighbors; a lamp lights at once or schedules its unlighting 4 gt later. Two
        // passes: an update a reacting block sends reaches blocks visited before it.
        for _pass in 0..2 {
        for i in 0..n {
            let isn = post[i].k[NOTE];
            let isl = post[i].k[LAMP];
            let isb = post[i].k[BULB];
            let isd = post[i].k[DROP];
            if self.a.val(isn) == Some(false) && self.a.val(isl) == Some(false) && self.a.val(isb) == Some(false) && self.a.val(isd) == Some(false) {
                continue;
            }
            let inp = self.nin(post, &der, i, lon);
            // a dropper: triggered follows (own input or QC) at once with flags 2 (observers
            // see it, no neighbor updates); a rising input schedules its dispense tick at +4
            if self.a.val(isd) != Some(false) {
                let qc = self.qcin(post, &der, i, lon);
                let di = self.or(&[inp, qc]);
                let noff = self.not(post[i].on);
                let ndi = self.not(di);
                let trig = self.and(&[nc[i], isd, di, noff]);
                let untrig = self.and(&[nc[i], isd, ndi, post[i].on]);
                let nu = self.not(untrig);
                let keep = self.and(&[post[i].on, nu]);
                post[i].on = self.or(&[keep, trig]);
                chg[i] = self.or(&[chg[i], trig, untrig]);
                let busy = self.or(&[bk.pend[i], bk.schd[i]]);
                let free = self.not(busy);
                let c = self.and(&[trig, free]);
                self.sched(bk, i, 4, c);
            }
            // a bulb: `powered` follows its input at once, a rising edge toggles `lit`; the
            // setBlock updates its 6 neighbors and (analog output) comparators behind one
            // conductor east and west
            if self.a.val(isb) != Some(false) {
                let bd = self.xor(inp, post[i].ext);
                let bflip = self.and(&[nc[i], isb, bd]);
                if self.a.val(bflip) != Some(false) {
                    let rise = self.and(&[bflip, inp]);
                    post[i].ext = self.ite(bflip, inp, post[i].ext);
                    post[i].on = self.xor(post[i].on, rise);
                    chg[i] = self.or(&[chg[i], bflip]);
                    for j in self.g.nbs(i) {
                        self.add_nc(&mut nc, j, bflip);
                    }
                    for d in [E, W] {
                        if let Some(n1) = self.g.nb(i, d) {
                            if let Some(n2) = self.g.nb(n1, d) {
                                let x = self.and(&[bflip, der.cond[n1], post[n2].k[COMP]]);
                                self.add_nc(&mut nc, n2, x);
                            }
                        }
                    }
                }
            }
            if self.a.val(isn) == Some(false) && self.a.val(isl) == Some(false) {
                continue;
            }
            let diff = self.xor(inp, post[i].on);
            let flip = self.and(&[nc[i], isn, diff]);
            let noff = self.not(post[i].on);
            let light = self.and(&[nc[i], isl, noff, inp]);
            let ch = self.or(&[flip, light]);
            if self.a.val(ch) != Some(false) {
                post[i].on = self.ite(flip, inp, post[i].on);
                post[i].on = self.or(&[post[i].on, light]);
                chg[i] = self.or(&[chg[i], ch]);
                for j in self.g.nbs(i) {
                    self.add_nc(&mut nc, j, flip);
                }
            }
            let busy = self.or(&[bk.pend[i], bk.schd[i]]);
            let free = self.not(busy);
            let ninp = self.not(inp);
            let off = self.and(&[nc[i], isl, post[i].on, ninp, free]);
            self.sched(bk, i, 4, off);
            // the instrument follows the block below
            let both = self.and(&[pre[i].k[NOTE], isn]);
            if self.a.val(both) != Some(false) {
                let b = self.g.nb(i, D);
                let a0 = self.instr(pre, b);
                let a1 = self.instr(post, b);
                let mut d = Vec::new();
                for j in 0..3 {
                    d.push(self.xor(a0[j], a1[j]));
                }
                let dd = self.or(&d);
                let ic = self.and(&[both, dd]);
                chg[i] = self.or(&[chg[i], ic]);
            }
        }
        }
        if self.flags_on {
            // a third reaction would be needed: the cascade is deeper than modeled
            let mut v = Vec::new();
            for i in 0..n {
                let r = self.or(&[post[i].k[NOTE], post[i].k[BULB], post[i].k[DROP]]);
                if self.a.val(r) == Some(false) {
                    continue;
                }
                let inp = self.nin(post, &der, i, lon);
                let dn = self.xor(inp, post[i].on);
                let db = self.xor(inp, post[i].ext);
                let a = self.and(&[nc[i], post[i].k[NOTE], dn]);
                let b = self.and(&[nc[i], post[i].k[BULB], db]);
                let qc = self.qcin(post, &der, i, lon);
                let di = self.or(&[inp, qc]);
                let dd = self.xor(di, post[i].on);
                let c = self.and(&[nc[i], post[i].k[DROP], dd]);
                v.push(self.or(&[a, b, c]));
            }
            let deep = self.or(&v);
            self.a.flag(deep, "note-block / bulb update cascade deeper than 2");
        }
        for i in 0..n {
            let rr = react_race[i];
            if self.a.val(rr) == Some(false) {
                continue;
            }
            let why = "reactive block checked before dust settled";
            post[i].on = self.relax(rr, post[i].on, why);
            let isb = post[i].k[BULB];
            let bx = self.and(&[rr, isb]);
            post[i].ext = self.relax(bx, post[i].ext, why);
        }
        let c2 = self.a.cost();
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
                fire = self.and(&[fire, nd]);
                fire = self.relax(race, fire, "observer turning off sees a change in the same phase");
            }
            self.sched(bk, o, 2, fire);
        }
        let c3 = self.a.cost();
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
            let mut rr = self.fl();
            let mut tr = self.fl();
            if let Some(early) = &early {
                let nd = self.not(ncd[i]);
                let only0 = self.and(&[nc0[i], nd]);
                let rin_e = self.rin(post, early, i, lon);
                let rch = self.xor(rin, rin_e);
                rr = self.and(&[only0, isr, rch, free]);
                let tin_e = self.tin(post, early, i, lon);
                let tin_f = self.tin(post, &der, i, lon);
                let tch = self.xor(tin_e, tin_f);
                tr = self.and(&[only0, ist, tch, free]);
            }
            for dl in 0..4 {
                let c = self.and(&[nc[i], isr, post[i].dl[dl], mis, free]);
                let c = self.relax(rr, c, "repeater checked before dust settled");
                self.sched(bk, i, 2 * (dl + 1), c);
            }
            let tin = self.tin(post, &der, i, lon);
            let tm = self.eqv(post[i].on, tin);
            let c = self.and(&[nc[i], ist, tm, free]);
            let c = self.relax(tr, c, "torch checked before dust settled");
            self.sched(bk, i, 2, c);
        }
        // comparators: a tick 2 gt later when the output or `powered` would change
        for i in 0..n {
            let isc = post[i].k[COMP];
            if self.a.val(isc) == Some(false) {
                continue;
            }
            let busy = self.or(&[bk.pend[i], bk.schd[i], wtt[i]]);
            let free = self.not(busy);
            let need = |cx: &mut Self, rear: &[A::B; 15], c: &Cell<A::B>| -> A::B {
                let mut dv: Vec<A::B> = (0..15).map(|j| cx.xor(rear[j], c.lvl[j])).collect();
                dv.push(cx.xor(rear[0], c.on));
                cx.or(&dv)
            };
            let rear = self.crear(post, &der, i, lon);
            let cell = post[i];
            let nd_f = need(self, &rear, &cell);
            let c = self.and(&[nc[i], isc, nd_f, free]);
            let mut cr = self.fl();
            if let Some(early) = &early {
                let re = self.crear(post, early, i, lon);
                let nd_e = need(self, &re, &cell);
                let ch = self.xor(nd_f, nd_e);
                let nd = self.not(ncd[i]);
                cr = self.and(&[nc0[i], nd, isc, ch, free]);
            }
            let c = self.relax(cr, c, "comparator checked before dust settled");
            self.sched(bk, i, 2, c);
            let pr = self.cprio(post, i);
            post[i].dl[0] = self.ite(c, pr, post[i].dl[0]);
        }
        let c4 = self.a.cost();
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
            let mut erace = self.fl();
            if let Some(early) = &early {
                let nd = self.not(anyd);
                let only0 = self.and(&[upd, nd]);
                let pe = self.ppow(post, early, p, lon);
                let ch = self.xor(pe, pw);
                erace = self.and(&[only0, ch]);
            }
            let mut res = Vec::new();
            let mut e2 = Vec::new();
            for f in 0..4 {
                if self.a.val(post[p].f[f]) == Some(false) {
                    continue;
                }
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
            let why = "piston checked before dust settled";
            let e0 = self.relax(erace, e0, why);
            let r1 = self.relax(erace, r1, why);
            let r2 = self.relax(erace, r2, why);
            bk.qn[p][0] = self.or(&[bk.qn[p][0], e0]);
            bk.qn[p][1] = self.or(&[bk.qn[p][1], r1]);
            bk.qn[p][2] = self.or(&[bk.qn[p][2], r2]);
        }
        if prof {
            let c5 = self.a.cost();
            eprintln!("settle {:?}: derive {} same+nc {} obs {} rep/torch {} pistons {}", ph as u8, c1 - c0, c2 - c1, c3 - c2, c4 - c3, c5 - c4);
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
                // the lever's own neighbor update reaches its support
                self.add_nc(&mut nc0, si, sel);
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

    /// Comparator ticks due in this phase (`sel`: HIGH-priority ticks in the repeater phase,
    /// NORMAL ones in the observer/torch phase): output := rear input, powered := output > 0
    /// (setBlock flags 2: no neighbor updates of its own), then always `updateNeighborsInFront`
    /// (compare mode). A rear input changing within the phase is a race (either outcome).
    #[allow(clippy::too_many_arguments)]
    fn comp_ticks(&mut self, s: &[Cell<A::B>], der: &Der<A::B>, post: &mut [Cell<A::B>], due: &[A::B], high: bool, lon: bool, nc0: &mut [A::B], dchg: &mut [A::B]) -> Vec<A::B> {
        let n = self.g.n();
        let fl = self.fl();
        let mut cdue = vec![fl; n];
        let mut r0: Vec<[A::B; 15]> = vec![[fl; 15]; n];
        for i in 0..n {
            let pr = if high { s[i].dl[0] } else { self.not(s[i].dl[0]) };
            let d = self.and(&[due[i], s[i].k[COMP], pr]);
            if self.a.val(d) == Some(false) {
                continue;
            }
            cdue[i] = d;
            let r = self.crear(s, der, i, lon);
            r0[i] = r;
            for j in 0..15 {
                post[i].lvl[j] = self.ite(d, r[j], s[i].lvl[j]);
            }
            post[i].on = self.ite(d, r[0], s[i].on);
            let nd = self.not(d);
            post[i].dl[0] = self.and(&[nd, s[i].dl[0]]);
        }
        if self.flags_on && cdue.iter().any(|&d| self.a.val(d) != Some(false)) {
            let dt = self.derive(post, lon);
            for i in 0..n {
                if self.a.val(cdue[i]) == Some(false) {
                    continue;
                }
                let r1 = self.crear(post, &dt, i, lon);
                let dv: Vec<A::B> = (0..15).map(|j| self.xor(r1[j], r0[i][j])).collect();
                let ch = self.or(&dv);
                let race = self.and(&[cdue[i], ch]);
                let alt = self.relax_choice(race, "comparator input changes within its phase");
                if self.a.val(alt) != Some(false) {
                    for j in 0..15 {
                        post[i].lvl[j] = self.ite(alt, r1[j], post[i].lvl[j]);
                    }
                    post[i].on = self.ite(alt, r1[0], post[i].on);
                }
            }
        }
        for i in 0..n {
            let d = cdue[i];
            if self.a.val(d) == Some(false) {
                continue;
            }
            let tg = self.xor(post[i].on, s[i].on);
            let tg = self.and(&[d, tg]);
            dchg[i] = self.or(&[dchg[i], tg]);
            let f = s[i].f;
            self.add_front(nc0, i, &f, d);
        }
        cdue
    }

    pub fn tick(&mut self, s: &[Cell<A::B>], lon: bool) -> Vec<Cell<A::B>> {
        let n = self.g.n();
        let fl = self.fl();
        let mut bk = self.new_book(s, false);
        let due: Vec<A::B> = (0..n).map(|i| s[i].sch[0]).collect();
        let none = vec![fl; n];
        // ---- phase 1a: repeaters (each reads its input when it runs; another repeater of the
        // same phase changing it is a race: flagged, or relaxed to either outcome)
        let der0 = self.derive(s, lon);
        let mut post = s.to_vec();
        let mut nc0 = vec![fl; n];
        let mut rdue = vec![fl; n];
        let mut rin0 = vec![fl; n];
        let mut rrace = vec![fl; n];
        let mut dchg_r = vec![fl; n];
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
        }
        if self.flags_on {
            let dt = self.derive(&post, lon);
            for i in 0..n {
                if self.a.val(rdue[i]) == Some(false) {
                    continue;
                }
                let r1 = self.rin(&post, &dt, i, lon);
                let ch = self.xor(r1, rin0[i]);
                rrace[i] = self.and(&[rdue[i], ch]);
                post[i].on = self.relax(rrace[i], post[i].on, "repeater input changes within the repeater phase");
            }
        }
        for i in 0..n {
            let d = rdue[i];
            if self.a.val(d) == Some(false) {
                continue;
            }
            let non = self.not(s[i].on);
            let ninp = self.not(rin0[i]);
            for dl in 0..4 {
                let c = self.and(&[d, non, ninp, s[i].dl[dl]]);
                let c = self.relax(rrace[i], c, "repeater input changes within the repeater phase");
                self.sched(&mut bk, i, 2 * (dl + 1), c);
            }
            let tg = self.xor(post[i].on, s[i].on);
            let tg = self.and(&[d, tg]);
            dchg_r[i] = tg;
            let f = s[i].f;
            self.add_front(&mut nc0, i, &f, tg);
        }
        self.comp_ticks(s, &der0, &mut post, &due, true, lon, &mut nc0, &mut dchg_r);
        let wtt: Vec<A::B> = (0..n).map(|i| {
            let t = self.a.or(&[s[i].k[REP], s[i].k[TORCH], s[i].k[WTORCH], s[i].k[COMP]]);
            self.a.and(&[due[i], t])
        }).collect();
        let der1 = self.settle(s, &mut post, lon, &nc0, &dchg_r, &[], &wtt, None, Phase::Rep, &mut bk);
        // ---- phase 1b: observers, torches, lamps (torches and lamps read their input when
        // they run: same race treatment)
        let s1 = post.clone();
        let mut post = s1.clone();
        let mut nc0 = vec![fl; n];
        let mut odue = vec![fl; n];
        let mut tdue = vec![fl; n];
        let mut ldue = vec![fl; n];
        let mut tin0 = vec![fl; n];
        let mut lin0 = vec![fl; n];
        let mut dchg_n = vec![fl; n];
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
                dchg_n[i] = od;
            }
            let ld = self.and(&[due[i], s1[i].k[LAMP]]);
            if self.a.val(ld) != Some(false) {
                ldue[i] = ld;
                let inp = self.nin(&s1, &der1, i, lon);
                lin0[i] = inp;
                let ninp = self.not(inp);
                let off = self.and(&[ld, s1[i].on, ninp]);
                let noff = self.not(off);
                post[i].on = self.and(&[post[i].on, noff]);
            }
            let tk = self.or(&[s1[i].k[TORCH], s1[i].k[WTORCH]]);
            let td = self.and(&[due[i], tk]);
            if self.a.val(td) != Some(false) {
                tdue[i] = td;
                let inp = self.tin(&s1, &der1, i, lon);
                tin0[i] = inp;
                let ni = self.not(inp);
                // else-branch: the cell's value so far (a free cell may be an observer that
                // toggled above; `s1` here lost its toggle whenever the cell could be a torch)
                post[i].on = self.ite(td, ni, post[i].on);
            }
        }
        if self.flags_on {
            let dt = self.derive(&post, lon);
            for i in 0..n {
                if self.a.val(tdue[i]) != Some(false) {
                    let t1 = self.tin(&post, &dt, i, lon);
                    let ch = self.xor(t1, tin0[i]);
                    let race = self.and(&[tdue[i], ch]);
                    post[i].on = self.relax(race, post[i].on, "torch input changes within the torch phase");
                }
                if self.a.val(ldue[i]) != Some(false) {
                    let l1 = self.nin(&post, &dt, i, lon);
                    let ch = self.xor(l1, lin0[i]);
                    let race = self.and(&[ldue[i], ch]);
                    post[i].on = self.relax(race, post[i].on, "lamp input changes within the lamp phase");
                }
            }
        }
        for i in 0..n {
            let tl = self.or(&[tdue[i], ldue[i]]);
            if self.a.val(tl) == Some(false) {
                continue;
            }
            let tg = self.xor(post[i].on, s1[i].on);
            let ttg = self.and(&[tdue[i], tg]);
            let ltg = self.and(&[ldue[i], tg]);
            dchg_n[i] = self.or(&[dchg_n[i], ttg, ltg]);
            for j in self.g.diamond2(i) {
                self.add_nc(&mut nc0, j, ttg);
            }
        }
        let cdue_b = self.comp_ticks(&s1, &der1, &mut post, &due, false, lon, &mut nc0, &mut dchg_n);
        for i in 0..n {
            tdue[i] = self.or(&[tdue[i], cdue_b[i]]);
        }
        let _der2 = self.settle(&s1, &mut post, lon, &nc0, &dchg_n, &[], &tdue, Some(&odue), Phase::Norm, &mut bk);
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
            if self.g.split {
                // two sub-rounds: the solver picks which pending events run first (the game's
                // queue order), so two interacting events of one round can go either way
                let mut q1 = q.clone();
                let mut q2 = q.clone();
                for p in 0..n {
                    let any = self.or(&q[p]);
                    if self.a.val(any) == Some(false) {
                        continue;
                    }
                    let first = self.a.fresh();
                    let nf = self.not(first);
                    for k in 0..3 {
                        q1[p][k] = self.and(&[q[p][k], first]);
                        q2[p][k] = self.and(&[q[p][k], nf]);
                    }
                }
                bk.qn = vec![[fl; 3]; n];
                cur = self.round2(&cur, lon, &q1, &mut bk);
                let new1 = bk.qn.clone();
                bk.qn = vec![[fl; 3]; n];
                cur = self.round2(&cur, lon, &q2, &mut bk);
                let new2 = bk.qn.clone();
                q = (0..n).map(|p| {
                    let mut e = [fl; 3];
                    for k in 0..3 {
                        e[k] = self.a.or(&[new1[p][k], new2[p][k]]);
                    }
                    e
                }).collect();
                continue;
            }
            bk.qn = vec![[fl; 3]; n];
            cur = if std::env::var("BP_OLD_ROUND").is_ok() { self.round(&cur, lon, &q, &mut bk) } else { self.round2(&cur, lon, &q, &mut bk) };
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
        let mut dchg = vec![fl; n];
        for i in 0..n {
            let ws = std::mem::take(&mut writes[i]);
            if ws.is_empty() {
                continue;
            }
            let conds: Vec<A::B> = ws.iter().map(|w| w.0).collect();
            dchg[i] = self.or(&conds);
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
        let der2 = self.settle(pre, &mut post, lon, &nc0, &dchg, &[], &wtt, None, Phase::Events, bk);
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
        let mut dchg = vec![fl; n];
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
            dchg[i] = fin;
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
            dchg[h] = self.or(&[dchg[h], gone]);
            self.add_nbs(&mut nc0, h, gone);
        }
        let wtt = vec![fl; n];
        self.settle(pre, &mut post, lon, &nc0, &dchg, &[], &wtt, None, Phase::Be, bk);
        post
    }
}
