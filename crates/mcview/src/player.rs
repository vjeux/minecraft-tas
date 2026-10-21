//! The live viewer's state, independent of the platform: a session played at a speed (ticks per
//! second) with the pointer's inputs (orbit, zoom, pan, click to flip a lever), the frame's
//! triangles rebuilt only when something moved.

use crate::atlas::Atlas;
use crate::camera::Camera;
use crate::design::Pos;
use crate::mesh::{Mesh, Vertex};
use crate::scene::{self, Look};
use crate::session::{Mode, Options, Session};

pub struct Player {
    pub session: Session,
    pub look: Look,
    pub camera: Camera,
    /// The camera as framed at load (double click / reset view).
    pub home: Camera,
    pub playing: bool,
    /// Game ticks per second (20 = real time).
    pub tps: f32,
    acc: f32,
    /// Fraction of the next tick shown (moving blocks interpolated; 0 when paused).
    pub partial: f32,
    mesh: Option<Mesh>,
    mesh_key: (u32, u32),
    /// Something changed since the last frame.
    pub dirty: bool,
    /// Seconds spent at a flying machine's end before it starts again.
    hold: f32,
    /// A travelling design's last centre: the camera follows it.
    follow: Option<[f32; 3]>,
    /// Circuit inputs are pulses (a tap = a short pulse) rather than levels (a tap flips).
    pub pulse_inputs: bool,
    /// The checker's phases (`data-phases`): (from, to, label).
    pub phases: Vec<(u32, u32, String)>,
    /// The last [`HISTORY`] ticks of each output (on/off), oldest first.
    pub history: Vec<std::collections::VecDeque<bool>>,
    /// The tick the history last recorded.
    history_tick: Option<u32>,
    /// When each held input went down (tick), for momentary presses.
    pressed: Vec<Option<u32>>,
    /// The door demo's stage (None: not running).
    pub door_demo: Option<DoorDemo>,
    /// The tick the auto-demo is shown from ([`Player::skip_demo_wait`]; 0: from placement).
    pub demo_from: u32,
}

/// Ticks of output history kept.
pub const HISTORY: usize = 40;

/// The door demo's stage ([`Player::start_door_demo`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorDemo {
    /// Closed at the start, the lever flipped at this tick.
    Wait(u32),
    /// The lever flipped on: the door opens.
    Opening,
    /// Open, the lever flipped back at this tick.
    Open(u32),
    /// The lever flipped off: the door closes.
    Closing,
    /// Closed again, the lever flipped at this tick.
    Closed(u32),
}

impl DoorDemo {
    /// The badge's words for the stage.
    pub fn words(self) -> &'static str {
        match self {
            DoorDemo::Wait(_) | DoorDemo::Closed(_) => "demo: the door, closed",
            DoorDemo::Opening => "demo: lever on, the door opens",
            DoorDemo::Open(_) => "demo: the door, open",
            DoorDemo::Closing => "demo: lever off, the door closes",
        }
    }

    /// Every badge text, for its size.
    pub const ALL_WORDS: [&'static str; 4] = ["demo: the door, closed", "demo: lever on, the door opens", "demo: the door, open", "demo: lever off, the door closes"];
}

/// The door demo's holds (`door video`'s: 1.5 s closed, 1.2 s open).
const DOOR_HOLD_CLOSED: u32 = 30;
const DOOR_HOLD_OPEN: u32 = 24;

/// A combination lock's recording when its page gives it no demo (its judge's sweep turns the
/// first dial through every code: tens of thousands of ticks; a lock's page stays paused): each
/// DIAL turned to its CODE frame's turn, one right-click every [`LOCK_STEP`] gt, the lamp lit and
/// held, then the last dial turned once more: shut again. With its phases (the badge names each
/// step). Like a door's recording, which shows its cycle. None: not a lock (or not simulated).
/// (Recording QA at phone size: a lock's video was 10 s of a lock standing still.)
pub fn lock_demo(s: &Session) -> Option<(crate::circuit::Schedule, Vec<(u32, u32, String)>)> {
    let lock = s.design.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("lock"));
    if !lock || !s.simulated() {
        return None;
    }
    let frame = |pre: char, k: u32| s.circuit_inputs.iter().position(|c| c.how == crate::circuit::InputHow::Frame && c.name.starts_with(pre) && c.name[1..].parse::<u32>().ok() == Some(k));
    let mut levels: Vec<(u32, usize, u8)> = Vec::new();
    let mut phases: Vec<(u32, u32, String)> = Vec::new();
    let mut t = s.tick + LOCK_STEP * 2;
    phases.push((s.tick, t, "shut: each DIAL turns to its CODE".to_string()));
    let mut last = None;
    for k in 0.. {
        let (Some(d), Some(c)) = (frame('d', k), frame('k', k)) else { break };
        let steps = (s.frame_turn(c)? + 8 - s.frame_turn(d)?) % 8;
        if steps == 0 {
            continue;
        }
        let from = t;
        for _ in 0..steps {
            levels.push((t, d, 0));
            t += LOCK_STEP;
        }
        // the phase ends with its last turn
        phases.push((from, t - LOCK_STEP + 1, format!("DIAL {} to CODE {}", k + 1, k + 1)));
        last = Some(d);
    }
    let d = last.or_else(|| frame('d', 0))?;
    // open from the last turn (the comparators and the lamp take a few ticks), held; then one
    // more turn: shut
    let open = phases.last().map(|p| p.1).unwrap_or(t);
    phases.push((open, open + LOCK_OPEN, "every DIAL at its CODE: open".to_string()));
    let shut = open + LOCK_OPEN;
    levels.push((shut, d, 0));
    phases.push((shut, shut + LOCK_OPEN, "one more turn: shut".to_string()));
    let ticks = shut + LOCK_OPEN;
    Some((crate::circuit::Schedule { edges: vec![Vec::new(); s.circuit_inputs.len()], ticks, events: Vec::new(), levels }, phases))
}

/// [`lock_demo`]'s pace: a right-click every this many ticks, and the open lock held this long.
pub const LOCK_STEP: u32 = 8;
pub const LOCK_OPEN: u32 = 50;

/// A door demo's whole cycle in ticks from the start (closed, opening, open, closing, and the
/// end of the closed hold after), run on its own copy of the design; None: not a door, or it never
/// settles within 4000 ticks.
pub fn door_demo_cycle(text: &str) -> Option<u32> {
    let mut p = Player::new(text, 1.0).ok()?;
    if !p.session.mode.is_door() || !p.session.simulated() {
        return None;
    }
    p.start_door_demo();
    let start = p.session.tick;
    let mut opened = false;
    for _ in 0..4000 {
        p.session.step();
        p.door_demo_tick();
        match p.door_demo {
            Some(DoorDemo::Open(_)) => opened = true,
            Some(DoorDemo::Closed(at)) if opened => return Some(at - start),
            _ => {}
        }
    }
    None
}

impl Player {
    pub fn new(text: &str, aspect: f32) -> Result<Player, String> {
        let mut session = Session::new(text, &Options::default())?;
        session.find_auto_output(20_000);
        let mut look = scene::viewer_look(&session);
        look.lit_marker = true;
        let camera = scene::default_camera(&session, &look, aspect);
        let pulse_inputs = crate::circuit::pulse_kind(&session.design);
        let n_in = session.circuit_inputs.len();
        let n_out = session.outputs.len();
        let mut p = Player {
            session,
            look,
            home: camera,
            camera,
            playing: false,
            tps: 20.0,
            acc: 0.0,
            partial: 0.0,
            mesh: None,
            mesh_key: (u32::MAX, 0),
            dirty: true,
            hold: 0.0,
            follow: None,
            pulse_inputs,
            phases: Vec::new(),
            history: vec![Default::default(); n_out],
            history_tick: None,
            pressed: vec![None; n_in],
            door_demo: None,
            demo_from: 0,
        };
        p.follow_machine();
        p.record();
        Ok(p)
    }

    /// The auto-demo starts 1 s before its first change when that is more than 3 s away (a BUD
    /// switch's judge waits 12 s before its first event: critique, "demo: waiting" at 4.6 s): the
    /// session runs there at once (every tick simulated; the counter shows the real tick), and a
    /// reset comes back there.
    pub fn skip_demo_wait(&mut self) {
        let Some(d) = &self.session.demo else { return };
        // a driven input's switch goes 2 gt before its edge (`Session::inputs_due`)
        let first = d.edges.iter().flatten().map(|(t, _)| t.saturating_sub(2)).chain(d.events.iter().map(|(t, _, _)| *t)).chain(d.levels.iter().map(|(t, _, _)| *t)).min();
        let Some(first) = first else { return };
        if first > 60 && self.session.tick + 20 < first {
            self.demo_from = first - 20;
            self.session.run(self.demo_from - self.session.tick);
            self.history = vec![Default::default(); self.session.outputs.len()];
            self.history_tick = None;
            self.record();
            self.invalidate();
        }
    }

    /// A travelling design (a flying machine): move the camera with its centre (moving blocks at
    /// their drawn offset, `stop:` blocks left out).
    /// A flying machine's centre: while it plays the camera follows it (it leaves the frame
    /// otherwise); nothing else ever moves the camera by itself. `move_camera` false only updates
    /// the reference (paused stepping), so the next follow does not jump.
    fn follow_machine_with(&mut self, move_camera: bool) {
        if self.session.corridor.is_none() || !self.is_flying_machine() {
            return;
        }
        let blocks = self.session.render_blocks(self.partial);
        let stops: Vec<Pos> = self.session.design.stops.iter().map(|(p, _)| *p).collect();
        let (mut c, mut n) = ([0f32; 3], 0usize);
        for b in blocks.iter().filter(|b| !(b.offset == [0.0; 3] && stops.contains(&b.cell))) {
            for i in 0..3 {
                c[i] += b.cell[i] as f32 + b.offset[i] + 0.5;
            }
            n += 1;
        }
        if n == 0 {
            return;
        }
        let c = c.map(|v| v / n as f32);
        if let Some(prev) = self.follow
            && move_camera
        {
            for i in 0..3 {
                self.camera.target[i] += c[i] - prev[i];
            }
        }
        self.follow = Some(c);
    }

    fn follow_machine(&mut self) {
        self.follow_machine_with(true);
    }

    /// A flying machine (`circuit: flying-machine`, or a `travel:` design without a kind): the
    /// only design the camera follows. A piston extender's payload comes back: the camera stays.
    fn is_flying_machine(&self) -> bool {
        self.session.design.meta.get("circuit").is_none_or(|k| k.trim_start().starts_with("flying-machine"))
    }

    /// Advance the clock by `dt` seconds; true when the picture changed.
    pub fn update(&mut self, dt: f32) -> bool {
        if !self.playing || !self.session.playable() {
            return std::mem::take(&mut self.dirty);
        }
        // a flying machine at its judge's end (the corridor's end), or the auto-demo at the end
        // of the judge's stimulus: a pause, then again
        let end = match (&self.session.demo, self.session.end_tick) {
            (Some(d), _) => Some(d.ticks),
            (None, e) => e,
        };
        if let Some(e) = end
            && self.session.tick >= e
        {
            self.hold += dt.min(0.25);
            if self.hold < 1.0 {
                return std::mem::take(&mut self.dirty);
            }
            self.hold = 0.0;
            self.reset();
            return true;
        }
        self.acc += dt.min(0.25) * self.tps;
        let mut n = 0;
        while self.acc >= 1.0 && n < 5000 && end.is_none_or(|e| self.session.tick < e) {
            self.session.step();
            self.record();
            self.door_demo_tick();
            self.acc -= 1.0;
            n += 1;
        }
        if n >= 5000 {
            self.acc = 0.0;
        }
        // interpolate moving blocks only at watchable speeds
        self.partial = if self.tps <= 40.0 { self.acc.clamp(0.0, 0.999) } else { 0.0 };
        self.follow_machine();
        self.dirty = false;
        true
    }

    /// Append the outputs' levels to the history (once per tick).
    fn record(&mut self) {
        if self.history_tick == Some(self.session.tick) {
            return;
        }
        self.history_tick = Some(self.session.tick);
        let outs = self.session.outputs.clone();
        if self.history.len() != outs.len() {
            self.history = vec![Default::default(); outs.len()];
        }
        for (h, o) in self.history.iter_mut().zip(&outs) {
            h.push_back(self.session.output_powered(*o));
            while h.len() > HISTORY {
                h.pop_front();
            }
        }
    }

    /// How many ticks a recording from tick 0 shows (`max_ticks` at most): the auto-demo's whole
    /// run, a door demo's cycle (`door_cycle`, [`door_demo_cycle`]), a clock's warmup and whole
    /// periods (at least one, about 8 s at its speed), a flying machine's flight; else 10 s.
    pub fn recording_ticks(&self, door_cycle: Option<u32>, max_ticks: u32) -> u32 {
        let s = &self.session;
        let secs = |x: f32| (x * self.tps).round() as u32;
        let t = if let Some(d) = &s.demo {
            // what is left of it (it may start after its long wait: `skip_demo_wait`)
            d.ticks.saturating_sub(s.tick)
        } else if let Some(c) = door_cycle {
            c
        } else if let Some(e) = s.end_tick {
            e + secs(1.0)
        } else if s.circuit_inputs.is_empty() && !s.mode.is_door() && !s.design.meta.contains_key("circuit") && s.simulated() {
            match s.period(20_000) {
                Ok(p) if p.period > 0 => {
                    let n = (secs(8.0) as f32 / p.period as f32).ceil().max(1.0) as u32;
                    p.warmup + n * p.period
                }
                _ => secs(10.0),
            }
        } else {
            secs(10.0)
        };
        t.clamp(1, max_ticks.max(1))
    }

    /// The checker's phase label at this tick (the auto-demo's badge).
    pub fn phase(&self) -> Option<&str> {
        let t = self.session.tick;
        self.phases.iter().find(|(a, b, _)| *a <= t && t < *b).map(|(_, _, l)| l.as_str())
    }

    /// Any input from the person ends the auto-demo (the inputs are theirs from then on).
    pub fn stop_demo(&mut self) {
        self.session.demo = None;
        self.door_demo = None;
    }

    /// Start the door demo (a door viewer with `data-autoplay`): its lever flipped like the
    /// record videos (`door video`: a 1.5 s hold, the door opening until it settles, 1.2 s open,
    /// closing until it settles, 1.5 s closed, again). Lever uses only: what runs is redsim's.
    pub fn start_door_demo(&mut self) {
        if self.session.mode.is_door() && self.session.simulated() {
            self.door_demo = Some(DoorDemo::Wait(self.session.tick + DOOR_HOLD_CLOSED));
        }
    }

    /// The door demo after a tick: flip the lever when its stage is over.
    fn door_demo_tick(&mut self) {
        let Some(d) = self.door_demo else { return };
        let Mode::Door { lever, .. } = self.session.mode else { return };
        let t = self.session.tick;
        let settled = self.session.idle() && self.session.since_input() >= 2;
        let next = match d {
            DoorDemo::Wait(at) | DoorDemo::Closed(at) if t >= at => {
                self.session.toggle(lever);
                DoorDemo::Opening
            }
            DoorDemo::Opening if settled => DoorDemo::Open(t + DOOR_HOLD_OPEN),
            DoorDemo::Open(at) if t >= at => {
                self.session.toggle(lever);
                DoorDemo::Closing
            }
            DoorDemo::Closing if settled => DoorDemo::Closed(t + DOOR_HOLD_CLOSED),
            d => d,
        };
        if next != d {
            self.invalidate();
        }
        self.door_demo = Some(next);
    }

    /// A tap on circuit input `i`: a short pulse (pulse inputs) or a flip (level inputs). A pulse
    /// input the demo left on is let go instead (a pulse cannot start on an input already on:
    /// the tap did nothing at all, and stopped the demo; QA at phone size, dff-lock's CLK).
    pub fn tap_input(&mut self, i: usize) -> bool {
        self.stop_demo();
        let ok = if self.pulse_inputs {
            if self.session.input_on.get(i).copied().unwrap_or(false) { self.session.set_input(i, false) } else { self.session.pulse_input(i, 2) }
        } else {
            self.session.set_input(i, !self.session.input_on[i])
        };
        if ok {
            self.invalidate();
        }
        ok
    }

    /// A press on input `i`'s button (momentary for pulse inputs: on while held, at least 2 gt;
    /// a flip for level inputs). A pulse input the demo left on: the press lets it go (as
    /// [`Player::tap_input`]).
    pub fn input_down(&mut self, i: usize) -> bool {
        if !self.pulse_inputs {
            return self.tap_input(i);
        }
        self.stop_demo();
        if self.session.input_on.get(i).copied().unwrap_or(false) {
            let ok = self.session.set_input(i, false);
            if ok {
                self.invalidate();
            }
            return ok;
        }
        if i < self.pressed.len() && self.session.set_input(i, true) {
            self.pressed[i] = Some(self.session.tick);
            self.invalidate();
            return true;
        }
        false
    }

    /// The release of input `i`'s button.
    pub fn input_up(&mut self, i: usize) {
        if !self.pulse_inputs || i >= self.pressed.len() {
            return;
        }
        if let Some(at) = self.pressed[i].take() {
            if self.session.tick >= at + 2 {
                self.session.set_input(i, false);
            } else {
                // a tap: it stays on for 2 gt
                self.session.release_input_at(i, at + 2);
            }
            self.invalidate();
        }
    }

    /// One tick (paused stepping).
    pub fn step(&mut self) {
        self.session.step();
        self.record();
        self.door_demo_tick();
        self.acc = 0.0;
        self.partial = 0.0;
        // paused: the camera stays (only a playing flying machine is followed)
        self.follow_machine_with(false);
        self.invalidate();
    }

    pub fn reset(&mut self) {
        self.session.reset();
        // the auto-demo comes back where it is shown from (its long wait skipped)
        if self.session.demo.is_some() && self.demo_from > self.session.tick {
            self.session.run(self.demo_from - self.session.tick);
        }
        self.history = vec![Default::default(); self.session.outputs.len()];
        self.history_tick = None;
        self.pressed = vec![None; self.session.circuit_inputs.len()];
        // the door demo starts over, closed
        if self.door_demo.is_some() {
            self.door_demo = Some(DoorDemo::Wait(self.session.tick + DOOR_HOLD_CLOSED));
        }
        self.record();
        self.acc = 0.0;
        self.partial = 0.0;
        self.follow_machine();
        self.invalidate();
    }

    /// The block under a point of the view (x, y in -1..1, y up).
    pub fn pick(&self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        let (o, d) = self.camera.ray(x, y, aspect);
        let blocks = self.session.render_blocks(0.0);
        crate::pick::pick(atlas, &blocks, o, d, self.camera.dist * 4.0 + 50.0)
    }

    /// A click: flips the lever / presses the button under the pointer, paused or playing (the
    /// frame is rebuilt at once: the cached mesh is keyed by tick, which a click does not move).
    /// Returns the cell.
    /// The circuit input under a point of the view (x, y in -1..1): its cell (often empty: the
    /// judge writes a block there) or the device in it, when nothing nearer covers it.
    pub fn input_at(&self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<usize> {
        let (o, d) = self.camera.ray(x, y, aspect);
        let picked = self.pick(atlas, x, y, aspect);
        let hit_dist = |c: Pos| -> Option<f32> { ray_box(o, d, [c[0] as f32, c[1] as f32, c[2] as f32], [c[0] as f32 + 1.0, c[1] as f32 + 1.0, c[2] as f32 + 1.0]) };
        let picked_dist = picked.and_then(hit_dist).unwrap_or(f32::MAX);
        self.session
            .circuit_inputs
            .iter()
            .enumerate()
            .filter_map(|(i, ci)| hit_dist(ci.pos).map(|t| (i, ci.pos, t)))
            .filter(|(_, p, t)| Some(*p) == picked || *t <= picked_dist + 1e-3)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(i, _, _)| i)
    }

    /// Press-and-hold on circuit input `i` in the view: on while held (`on`), off when released
    /// (a pulse input stays on at least 2 gt, like its button).
    pub fn hold_input(&mut self, i: usize, on: bool) -> bool {
        if i >= self.session.circuit_inputs.len() {
            return false;
        }
        self.stop_demo();
        if on {
            if !self.session.set_input(i, true) {
                return false;
            }
            if let Some(p) = self.pressed.get_mut(i) {
                *p = Some(self.session.tick);
            }
        } else {
            let at = self.pressed.get_mut(i).and_then(|p| p.take()).unwrap_or(0);
            if self.pulse_inputs && self.session.tick < at + 2 {
                self.session.release_input_at(i, at + 2);
            } else {
                self.session.set_input(i, false);
            }
        }
        self.invalidate();
        true
    }

    pub fn click(&mut self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        if let Some(i) = self.input_at(atlas, x, y, aspect) {
            let p = self.session.circuit_inputs[i].pos;
            return self.tap_input(i).then_some(p);
        }
        let c = self.pick(atlas, x, y, aspect)?;
        self.stop_demo();
        if self.session.toggle(c) {
            self.invalidate();
            Some(c)
        } else {
            None
        }
    }

    /// Where on the view (x, y in -1..1, y up) a click picks `cell` (tests click there); a
    /// circuit input's cell (a click there drives it, even when empty) at its projected centre.
    pub fn screen_point(&self, atlas: &Atlas, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
        if self.session.circuit_inputs.iter().any(|c| c.pos == cell) {
            let (f, r, u) = self.camera.basis();
            let eye = self.camera.eye();
            let t = (self.camera.vfov.to_radians() / 2.0).tan();
            let v = [cell[0] as f32 + 0.5 - eye[0], cell[1] as f32 + 0.5 - eye[1], cell[2] as f32 + 0.5 - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f);
            if z > 0.05 {
                let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
                if x.abs() < 0.98 && y.abs() < 0.98 {
                    return Some((x, y));
                }
            }
            return None;
        }
        scene::click_point(atlas, &self.session, &self.camera, cell, aspect)
    }

    /// Turn the camera (and its home) to a side every lever and button can be seen and clicked
    /// from, if the pictures' side hides one.
    /// The camera from the side every input can be seen from, its build in `window` (a live
    /// viewer: [`scene::live_window`] of its height); the home view too.
    pub fn show_inputs(&mut self, atlas: &Atlas, aspect: f32, window: (f32, f32, f32)) {
        self.camera = scene::camera_showing_inputs(atlas, &self.session, &self.look, aspect, window);
        self.home = self.camera;
        self.dirty = true;
    }

    /// The live viewer's default camera at its CSS size (`scene::live_camera`: on a phone, clear
    /// of the Record pill when the build would come out under it); returns the window it used.
    pub fn frame_live(&mut self, atlas: &Atlas, w: f32, h: f32) -> (f32, f32, f32) {
        let (cam, window) = scene::live_camera(atlas, &self.session, &self.look, w, h);
        self.camera = cam;
        self.home = cam;
        self.dirty = true;
        window
    }

    /// Flip a door's lever (or the first input).
    pub fn flip(&mut self) -> bool {
        self.stop_demo();
        if !self.session.circuit_inputs.is_empty() {
            return self.tap_input(0);
        }
        let target = match self.session.mode {
            Mode::Door { lever, .. } => Some(lever),
            _ => self.session.inputs.first().copied(),
        };
        let ok = target.is_some_and(|p| self.session.toggle(p));
        if ok {
            self.invalidate();
        }
        ok
    }

    /// The frame's triangles (solid, translucent sorted for the camera).
    pub fn triangles(&mut self, atlas: &Atlas) -> (Vec<Vertex>, Vec<Vertex>) {
        let key = (self.session.tick, (self.partial * 64.0) as u32);
        if self.mesh.is_none() || key != self.mesh_key {
            self.mesh = Some(scene::build(atlas, &self.session, self.partial, &self.look));
            self.mesh_key = key;
        }
        scene::triangles(self.mesh.as_ref().unwrap(), &self.camera)
    }

    /// Forget the cached mesh (the look changed).
    pub fn invalidate(&mut self) {
        self.mesh = None;
        self.dirty = true;
    }

    /// After moving the camera: keep it above the ground (never under the floor): the pictures'
    /// grass, or a TNT duper's real ground.
    pub fn keep_camera_above_ground(&mut self) {
        if let Some(g) = self.look.ground {
            self.camera.keep_above(g);
        } else if let Some(f) = &self.session.floor {
            self.camera.keep_above(f.hi[1] as f32 + 1.0);
        }
    }

    /// The tick counter: `6.15 s · 123 ticks` (plain words: seconds first), plus the ticks since
    /// the last flip for a door.
    pub fn label(&self) -> String {
        let s = &self.session;
        let t = s.tick.saturating_sub(s.rest_tick);
        let mut out = format!("{} · {t} ticks", seconds(t));
        // the person's flip of a door: the ticks since (the door demo's badge says its stage)
        if s.last_input.is_some() && s.mode.is_door() && self.door_demo.is_none() {
            out.push_str(&format!(" · flip +{}", s.since_input()));
        }
        out
    }
}

/// Where a ray (origin, direction) enters a box, if it does (slab test).
fn ray_box(o: [f32; 3], d: [f32; 3], lo: [f32; 3], hi: [f32; 3]) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - o[i]) / d[i], (hi[i] - o[i]) / d[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// `6.15 s`, `1 min 2.50 s` (always two decimals: the counter keeps its width).
pub fn seconds(gt: u32) -> String {
    let s = gt as f32 / 20.0;
    if s < 60.0 {
        format!("{s:.2} s")
    } else {
        let m = (s / 60.0).floor();
        format!("{m} min {:05.2} s", s - m * 60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The door demo flips the lever like the record videos: the door opens (its opening
    /// empties), stays open, closes again (its 9 blocks back), and starts over.
    #[test]
    fn the_door_demo_opens_and_closes() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/piston-door-3x3/archive/b38h-obs7-45b.txt")).unwrap();
        let mut p = Player::new(&text, 1.5).unwrap();
        p.start_door_demo();
        p.playing = true;
        let mut seen: Vec<usize> = Vec::new();
        for _ in 0..12_000 {
            p.update(0.05);
            let n = p.session.door_blocks().unwrap();
            if seen.last() != Some(&n) {
                seen.push(n);
            }
            if seen.len() > 1 && seen.first() == Some(&9) && seen.contains(&0) && seen.last() == Some(&9) {
                break;
            }
        }
        assert_eq!(seen.first(), Some(&9), "closed at first: {seen:?}");
        assert!(seen.contains(&0), "opens: {seen:?}");
        assert_eq!(seen.last(), Some(&9), "closes again: {seen:?}");
        assert!(p.door_demo.is_some(), "still running");
        p.flip();
        assert!(p.door_demo.is_none(), "the person's flip stops it");
    }

    /// A flying machine started by its own lever (site 2026-10-21: the start-lever picks stood at
    /// "0.00 s · 0 ticks", the run's end found at placement undoing every flip): it has no end
    /// before its input; a press makes it play on.
    #[test]
    fn a_lever_started_flying_machine_plays_after_its_flip() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/fly/archive/fly-docking-15b.txt")).unwrap();
        // the docking machine without its kick: only its lever left
        let text: String = text.lines().filter(|l| !l.starts_with("port start:")).map(|l| format!("{l}\n")).collect();
        let mut p = Player::new(&text, 1.5).unwrap();
        assert!(p.session.starts_on_input() && p.session.end_tick.is_none());
        assert!(p.tap_input(0), "the lever flips");
        p.playing = true;
        let t0 = p.session.tick;
        for _ in 0..40 {
            p.update(0.05);
        }
        assert!(p.session.tick > t0 + 20, "it plays on after the flip: tick {} from {t0}", p.session.tick);
        // the kick-started docking machine keeps its judge's end
        let kicked = Player::new(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/fly/archive/fly-docking-15b.txt")).unwrap(), 1.5).unwrap();
        assert!(!kicked.session.starts_on_input() && kicked.session.end_tick.is_some());
    }

    /// Critique: a BUD switch's demo said "waiting" for 12 s (its judge's first event at 240 gt):
    /// the demo is shown from 1 s before its first change, and a reset comes back there; a demo
    /// whose first change comes soon starts at placement.
    #[test]
    fn a_long_demo_wait_is_skipped() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/bud/archive/bud-observer.txt")).unwrap();
        let schedule = |events: &str, ticks: u32, n: usize| crate::circuit::Schedule { edges: vec![Vec::new(); n], ticks, events: crate::circuit::parse_events(events).unwrap(), levels: Vec::new() };
        let mut p = Player::new(&text, 1.5).unwrap();
        let t0 = p.session.tick;
        assert!(t0 < 200, "{t0}");
        let n = p.session.circuit_inputs.len();
        p.session.attach_demo(schedule("240 place 0 1 0 minecraft:oak_log[axis=y]; 340 clear 0 1 0", 380, n));
        p.skip_demo_wait();
        assert_eq!((p.demo_from, p.session.tick), (220, 220));
        assert_eq!(p.recording_ticks(None, 1000), 160, "what is left of the demo");
        p.step();
        p.reset();
        assert_eq!(p.session.tick, 220, "a reset comes back where the demo is shown from");
        let mut q = Player::new(&text, 1.5).unwrap();
        q.session.attach_demo(schedule("50 place 0 1 0 minecraft:oak_log[axis=y]", 120, n));
        q.skip_demo_wait();
        assert_eq!((q.demo_from, q.session.tick), (0, t0));
    }

    /// QA at phone size (dff-lock's CLK, the dividers' IN): a press on a pulse input the demo
    /// left on did nothing at all (a pulse cannot start on an input already on) and stopped the
    /// demo. It lets the input go; the next press is a pulse again.
    #[test]
    fn a_press_on_an_input_the_demo_left_on_lets_it_go() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/memory/archive/dff-lock.txt")).unwrap();
        let mut p = Player::new(&text, 0.8).unwrap();
        p.pulse_inputs = true;
        let names: Vec<String> = p.session.circuit_inputs.iter().map(|c| c.name.clone()).collect();
        let clk = names.iter().position(|n| n == "clk").unwrap();
        let demo = crate::circuit::parse_stimulus("clk 40+ 900-", &names, 40).unwrap();
        p.session.attach_demo(demo);
        while p.session.tick < 60 {
            p.step();
        }
        assert!(p.session.input_on[clk], "the demo holds CLK on");
        assert!(p.input_down(clk), "the press does something");
        assert!(!p.session.input_on[clk] && p.session.demo.is_none(), "CLK let go, the demo stopped");
        p.input_up(clk);
        p.step();
        assert!(!p.session.input_on[clk]);
        // the next press: on while held, off after its release
        assert!(p.input_down(clk));
        assert!(p.session.input_on[clk]);
        for _ in 0..3 {
            p.step();
        }
        p.input_up(clk);
        p.step();
        assert!(!p.session.input_on[clk]);
        // a tap the same way
        p.session.set_input(clk, true);
        assert!(p.tap_input(clk) && !p.session.input_on[clk]);
    }

    /// Recording QA at phone size: a lock's video was 10 s of a lock standing still (its page has
    /// no demo). Its recording turns each dial to its code (the lamp stays off until the last
    /// turn), holds it open, and shuts it with one more turn; every archived lock.
    #[test]
    fn a_locks_recording_opens_it() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/lock/archive");
        let mut locks: Vec<std::path::PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        locks.sort();
        assert!(!locks.is_empty());
        for f in &locks {
            let text = std::fs::read_to_string(f).unwrap();
            let mut p = Player::new(&text, 0.8).unwrap();
            let (demo, phases) = lock_demo(&p.session).unwrap_or_else(|| panic!("{}: a lock demo", f.display()));
            assert_eq!(phases.last().map(|x| x.2.as_str()), Some("one more turn: shut"));
            let open_at = phases[phases.len() - 2].0;
            let shut_at = phases[phases.len() - 1].0;
            let out = p.session.outputs[0];
            p.session.attach_demo(demo.clone());
            p.phases = phases;
            let ticks = p.recording_ticks(None, 2000);
            assert_eq!(ticks, demo.ticks - p.session.tick, "{}", f.display());
            assert!(ticks <= 20 * 20, "{}: {ticks} ticks fit a 20 s recording", f.display());
            let mut lit = Vec::new();
            while p.session.tick < demo.ticks {
                p.session.step();
                if p.session.output_powered(out) {
                    lit.push(p.session.tick);
                }
            }
            assert!(!lit.is_empty(), "{}: it opens", f.display());
            assert!(lit[0] + 1 >= open_at && lit[0] < open_at + 20, "{}: lit at {} (the last turn at {})", f.display(), lit[0], open_at - 1);
            assert!(*lit.last().unwrap() < shut_at + 20, "{}: shut by {} (the extra turn at {shut_at})", f.display(), lit.last().unwrap());
            assert!(!p.session.output_powered(out), "{}: shut at the end", f.display());
        }
    }

    /// The Record button's length (from tick 0): a door's demo cycle (closed, opening, open,
    /// closing, closed again), a clock's warmup and whole periods (at least one), the cap.
    #[test]
    fn recording_lengths() {
        let door = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/piston-door-3x3/archive/b38h-obs7-45b.txt")).unwrap();
        let cycle = door_demo_cycle(&door).expect("a door's demo cycle");
        assert!(cycle > DOOR_HOLD_CLOSED + DOOR_HOLD_OPEN && cycle < 1000, "{cycle}");
        let p = Player::new(&door, 1.5).unwrap();
        assert_eq!(p.recording_ticks(Some(cycle), 400), cycle.min(400));
        let clock = "name: c\nport start: 0 0 0 last\npalette:\n  O minecraft:observer[facing=east,powered=false]\n  W minecraft:waxed_copper_bulb[lit=false,powered=false]\n  o minecraft:observer[facing=west,powered=false]\nlayer 0:\n  OoW\n";
        let p = Player::new(clock, 1.5).unwrap();
        let per = p.session.period(20_000).unwrap();
        let t = p.recording_ticks(None, 400);
        assert!(t >= per.warmup + per.period && (t - per.warmup) % per.period == 0, "{t} ticks for warmup {} period {}", per.warmup, per.period);
        assert_eq!(p.recording_ticks(None, 10), 10, "the cap");
        assert!(door_demo_cycle(clock).is_none(), "not a door");
    }

    #[test]
    fn plays_a_clock_and_flips_a_door() {
        let clock = "name: c\nport start: 0 0 0 last\npalette:\n  O minecraft:observer[facing=east,powered=false]\n  W minecraft:waxed_copper_bulb[lit=false,powered=false]\n  o minecraft:observer[facing=west,powered=false]\nlayer 0:\n  OoW\n";
        let mut p = Player::new(clock, 1.5).unwrap();
        p.playing = true;
        for _ in 0..4 {
            assert!(p.update(0.25));
        }
        assert_eq!(p.session.tick, 20);
        assert_eq!(seconds(123), "6.15 s");
        assert_eq!(seconds(20), "1.00 s");
        assert_eq!(seconds(1250), "1 min 02.50 s");
    }

    #[test]
    fn a_paused_click_shows_at_once() {
        // the lever flips while paused: the next frame shows it (no stale mesh of the same tick)
        let gadget = "name: g\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\nlayer 0:\n  SB\nlayer 1:\n  L\n";
        let atlas = Atlas::build();
        let mut p = Player::new(gadget, 1.5).unwrap();
        assert_eq!(p.session.inputs, vec![[0, 1, 0]]);
        let before = p.triangles(&atlas).0;
        let (x, y) = p.screen_point(&atlas, [0, 1, 0], 1.5).expect("the lever is visible");
        assert_eq!(p.click(&atlas, x, y, 1.5), Some([0, 1, 0]));
        assert!(p.update(0.0), "a paused click asks for a frame");
        let after = p.triangles(&atlas).0;
        assert!(before.len() != after.len() || before.iter().zip(&after).any(|(a, b)| a.pos != b.pos), "the frame changed");
        assert!(p.session.is_on([0, 1, 0]));
    }
}
