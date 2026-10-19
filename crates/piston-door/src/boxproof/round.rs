//! The block-event round, written per target cell: every cell gets at most a handful of
//! candidate new states (its own piston extending or retracting, a head arriving from each
//! side, a block pushed or pulled in from each side, removal, a spit placement), combined as a
//! sum of products. Equivalent to the per-piston formulation, much smaller when every cell
//! may hold a piston (whole-box mode).

use super::alg::Alg;
use super::state::*;
use super::tick::Book;

impl<'g, A: Alg> Ctx<'g, A> {
    fn combine(&mut self, pre: &Cell<A::B>, cands: &[(A::B, Cell<A::B>)]) -> (Cell<A::B>, A::B) {
        if cands.is_empty() {
            return (*pre, self.fl());
        }
        let conds: Vec<A::B> = cands.iter().map(|c| c.0).collect();
        let any = self.or(&conds);
        let none = self.not(any);
        let mut r = *pre;
        macro_rules! arr {
            ($($f:ident),*) => { $( for j in 0..r.$f.len() {
                let mut t = vec![self.and(&[none, pre.$f[j]])];
                for (c, cell) in cands { t.push(self.and(&[*c, cell.$f[j]])); }
                r.$f[j] = self.or(&t);
            } )* };
        }
        macro_rules! one {
            ($($f:ident),*) => { $( {
                let mut t = vec![self.and(&[none, pre.$f])];
                for (c, cell) in cands { t.push(self.and(&[*c, cell.$f])); }
                r.$f = self.or(&t);
            } )* };
        }
        arr!(k, f, dl, lvl, mk, mf, sch);
        one!(st, ext, on, mst, mon, mext, msrc, mold, mnew);
        r.ev = [self.fl(); 3];
        if self.flags_on {
            for a in 0..conds.len() {
                for b in a + 1..conds.len() {
                    let both = self.and(&[conds[a], conds[b]]);
                    self.a.flag(both, "two block events write one cell in the same round");
                }
            }
        }
        (r, any)
    }

    pub fn round2(&mut self, pre: &[Cell<A::B>], lon: bool, q: &[[A::B; 3]], bk: &mut Book<A::B>) -> Vec<Cell<A::B>> {
        let n = self.g.n();
        let fl = self.fl();
        let t = self.t();
        let der = self.derive(pre, lon);
        let mut nc0 = vec![fl; n];
        let mut cands: Vec<Vec<(A::B, Cell<A::B>)>> = vec![Vec::new(); n];
        // per (piston, facing): what it does this round
        let mut dxs: Vec<[A::B; 4]> = vec![[fl; 4]; n];
        let mut push_in: Vec<[Vec<A::B>; 4]> = (0..n).map(|_| [vec![], vec![], vec![], vec![]]).collect();
        let mut execs: Vec<(usize, A::B, A::B, bool)> = Vec::new();
        let mut placed_cells: Vec<(usize, A::B)> = Vec::new();
        let mut air_c: Vec<Vec<A::B>> = vec![Vec::new(); n];
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
            let mut ext_any = Vec::new();
            let mut ret_any = Vec::new();
            for f in 0..4 {
                let face = self.and(&[isp, pre[p].f[f]]);
                if self.a.val(face) == Some(false) {
                    continue;
                }
                let l = self.line(pre, p, f);
                let dx = self.and(&[face, q[p][0], ne, pw, l.resolves]);
                if self.a.val(dx) != Some(false) {
                    let nb = self.not(l.bad);
                    let ok = self.not(dx);
                    let okk = self.or(&[ok, nb]);
                    self.a.require(okk, "a push destroys a block or leaves the box");
                    dxs[p][f] = dx;
                    ext_any.push(dx);
                    self.add_nbs(&mut nc0, p, dx);
                    if let Some(c1) = l.cells[0] {
                        self.add_nbs(&mut nc0, c1, dx);
                    }
                    for k in 0..l.moved.len() {
                        let (Some(ck), Some(Some(cn))) = (l.cells[k], l.cells.get(k + 1).copied()) else { continue };
                        let c = self.and(&[dx, l.moved[k]]);
                        if self.a.val(c) == Some(false) {
                            continue;
                        }
                        push_in[cn][f].push(c);
                        self.add_nbs(&mut nc0, ck, c);
                        let busy = self.or(&[bk.pend[ck], bk.schd[ck]]);
                        let free = self.not(busy);
                        let noff = self.not(pre[ck].on);
                        let st_tick = self.and(&[c, pre[ck].k[OBSV], pre[ck].f[f], noff, free]);
                        self.sched(bk, ck, 2, st_tick);
                        let o = self.and(&[c, pre[ck].k[OBSV], pre[ck].on, busy]);
                        let ff = pre[ck].f;
                        self.add_front(&mut nc0, ck, &ff, o);
                    }
                    execs.push((p, dx, pw, f == U));
                    // the head arrives at the arm cell
                    if let Some(c1) = l.cells[0] {
                        let mut h = konst_cell(&self.a, MOVING);
                        h.sch = pre[c1].sch;
                        h.f[f] = t;
                        h.st = st;
                        h.mk[MC_HEAD] = t;
                        h.mf[f] = t;
                        h.mst = st;
                        h.mext = t;
                        h.msrc = t;
                        h.mnew = t;
                        cands[c1].push((dx, h));
                    }
                }
                let rq = self.or(&[q[p][1], q[p][2]]);
                let dr = self.and(&[face, rq, pre[p].ext, npw]);
                if self.a.val(dr) == Some(false) {
                    continue;
                }
                ret_any.push(dr);
                self.add_nbs(&mut nc0, p, dr);
                execs.push((p, dr, pw, f == U));
                let Some(c1) = self.g.off(p, f, 1) else { continue };
                let c2 = self.g.off(p, f, 2);
                let hm = self.and(&[dr, pre[c1].k[MOVING]]);
                let hm_src = self.and(&[hm, pre[c1].msrc]);
                let nsrc = self.not(pre[c1].msrc);
                let hm_obj = self.and(&[hm, nsrc]);
                self.a.flag(hm_obj, "retraction final-ticks a non-source block in the head cell");
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
                let rm = self.and(&[dr, nsp, npl]);
                let hs = self.and(&[hm_src, spit]);
                air_c[c1].push(rm);
                air_c[c1].push(hs);
                if let Some(c2) = c2 {
                    let m = self.mover(&pre[c2], &pre[c1], f, fl, fl);
                    cands[c1].push((pull, m));
                    air_c[c2].push(pull);
                    self.add_nbs(&mut nc0, c2, pull);
                    let busy = self.or(&[bk.pend[c2], bk.schd[c2]]);
                    let free = self.not(busy);
                    let noff = self.not(pre[c2].on);
                    let st_tick = self.and(&[pull, pre[c2].k[OBSV], pre[c2].f[opp(f)], noff, free]);
                    self.sched(bk, c2, 2, st_tick);
                    let mut pc = self.placed(&pre[c2]);
                    pc.sch = pre[c2].sch;
                    cands[c2].push((spit, pc));
                    self.add_nbs(&mut nc0, c2, spit);
                    self.add_nc(&mut nc0, c2, spit);
                    placed_cells.push((c2, spit));
                }
                let hm_nc = self.or(&[hm, rm]);
                self.add_nbs(&mut nc0, c1, hm_nc);
                self.add_nc(&mut nc0, c1, hm);
            }
            // own base: extends (only `ext` changes) or becomes the retracting moving block
            let ea = self.or(&ext_any);
            if self.a.val(ea) != Some(false) {
                let mut b = pre[p];
                b.ext = t;
                cands[p].push((ea, b));
            }
            let ra = self.or(&ret_any);
            if self.a.val(ra) != Some(false) {
                let mut b = konst_cell(&self.a, MOVING);
                b.sch = pre[p].sch;
                b.f = pre[p].f;
                b.st = st;
                b.mk[MC_PISTON] = t;
                b.mf = pre[p].f;
                b.mst = st;
                b.msrc = t;
                b.mnew = t;
                cands[p].push((ra, b));
            }
        }
        // pushed blocks arrive, one candidate per side
        for c in 0..n {
            for d in 0..4 {
                if push_in[c][d].is_empty() {
                    continue;
                }
                let v = std::mem::take(&mut push_in[c][d]);
                let cond = self.or(&v);
                let Some(src) = self.g.nb(c, opp(d)) else { continue };
                let m = self.mover(&pre[src], &pre[c], d, fl, t);
                cands[c].push((cond, m));
            }
            if !air_c[c].is_empty() {
                let v = std::mem::take(&mut air_c[c]);
                let cond = self.or(&v);
                let mut air = konst_cell(&self.a, AIR);
                air.sch = pre[c].sch;
                cands[c].push((cond, air));
            }
        }
        let mut post = pre.to_vec();
        let mut dchg = vec![fl; n];
        let dbg = std::env::var("BP_DEBUG").is_ok();
        for i in 0..n {
            let cs = std::mem::take(&mut cands[i]);
            if dbg {
                let on: Vec<usize> = cs.iter().enumerate().filter(|(_, c)| self.a.val(c.0) == Some(true)).map(|(j, _)| j).collect();
                if on.len() > 1 {
                    let kinds: Vec<String> = on.iter().map(|&j| format!("{}{}", KNAMES[cs[j].1.k.iter().position(|&b| self.a.val(b) == Some(true)).unwrap_or(0)], if self.a.val(cs[j].1.k[MOVING]) == Some(true) { format!("(mk{:?})", cs[j].1.mk.iter().position(|&b| self.a.val(b) == Some(true))) } else { String::new() })).collect();
                    eprintln!("round write conflict at {:?}: candidates {:?}", self.g.xy(i), kinds);
                }
            }
            let (cell, any) = self.combine(&pre[i], &cs);
            post[i] = cell;
            dchg[i] = any;
        }
        for (i, c) in placed_cells {
            let mut cell = post[i];
            self.place_obs(bk, &mut nc0, i, &mut cell, c);
            post[i] = cell;
        }
        let wtt = vec![fl; n];
        let der2 = self.settle(pre, &mut post, lon, &nc0, &dchg, &[], &wtt, None, super::tick::Phase::Events, bk);
        for (p, c, pw0, up) in execs {
            if up || !self.flags_on {
                continue;
            }
            let pw1 = self.ppow(&post, &der2, p, lon);
            let ch = self.xor(pw0, pw1);
            let race = self.and(&[c, ch]);
            self.a.flag(race, "a block event's power changes within its round");
        }
        let _ = dxs;
        post
    }
}
