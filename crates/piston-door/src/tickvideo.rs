//! `mctas door tick-video`: one open/close cycle of a door, tick by tick, filmed by the real
//! client with a still camera. Captions say when the lever is flipped, count the ticks after the
//! flip until nothing moves anymore (the state the judge checks), and mark the second flip and the
//! settled closed state. A 3x3 map shows the doorway as the server sees it after each tick, and a
//! strip under the picture shows the whole phase at a glance: doorway clear, door closed, or in
//! between, with a cursor on the current tick.

use crate::showcase::Look;
use crate::video::{Facts, design_shot, place};
use crate::{DoorSpec, cell_char, open_problems, watch_box};
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::mcfont::{Image, LINE_HEIGHT};
use mctas::sim::{PlaceMode, Region, SandboxOptions, Worker, WorkerConfig};
use mctas::video::{self, Client, Display};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct TickVideoOptions {
    pub width: u32,
    pub height: u32,
    /// Seconds each tick stays on screen.
    pub secs_per_tick: f64,
    /// Seconds the key moments (at rest, each flip, each settled state) stay on screen.
    pub hold_secs: f64,
    /// Milliseconds the client gets to draw a new tick before the screenshot.
    pub draw_ms: u64,
    /// Give up after this many ticks per flip.
    pub max_ticks: u32,
}

impl Default for TickVideoOptions {
    fn default() -> Self {
        TickVideoOptions { width: 1280, height: 720, secs_per_tick: 0.35, hold_secs: 2.0, draw_ms: 350, max_ticks: 400 }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Rest,
    Opening,
    Closing,
}

/// One frame: a screenshot and what the server said at that moment.
struct Frame {
    shot: PathBuf,
    phase: Phase,
    /// Ticks since the flip (0 = right after the flip, before any tick).
    tick: u32,
    /// Nothing is pending anymore (the judge checks this state).
    settled: bool,
    /// The 3x3 doorway, rows top to bottom, columns left to right as the camera sees them.
    cells: [[char; 3]; 3],
    /// Door cells and walkway walkable (the judge's "open" test).
    clear: bool,
    /// The 9 door cells hold their rest blocks (the judge's "closed" test).
    closed: bool,
    secs: f64,
}

const YELLOW: [u8; 3] = [255, 255, 85];
const WHITE: [u8; 3] = [255, 255, 255];
const GRAY: [u8; 3] = [170, 170, 170];
const GREEN: [u8; 3] = [85, 255, 85];
const RED: [u8; 3] = [255, 85, 85];
const ORANGE: [u8; 3] = [255, 170, 0];

/// Record `d` tick by tick into `out` (mp4).
pub fn record(version: &str, d: &Design, out: &Path, o: &TickVideoOptions) -> Result<()> {
    let spec = DoorSpec::from_design(d)?;
    let look = Look::load(version)?;
    let client_build = video::build_client(version)?;
    let display = Display::start(o.width, o.height)?;
    let port = crate::video::free_port()?;
    let run = mctas::paths::run_dir().join(format!("tick-video-{}", std::process::id()));
    let frames_dir = run.join("frames");
    std::fs::create_dir_all(&frames_dir)?;
    let cfg = WorkerConfig { version: version.to_string(), listen: Some(port), stderr_log: Some(run.join("server.log")), ..Default::default() };
    let mut w = Worker::spawn(&cfg)?;
    let sb = w.create(&SandboxOptions { chunks: [-7, -7, 6, 6], seed: 0 })?;
    place(&mut w, sb, d)?;
    let (lo, _) = d.bounds().context("empty design")?;
    w.fill(sb, [-110, lo[1] - 2, -110], [109, lo[1] - 2, 109], "minecraft:grass_block", PlaceMode::Raw)?;
    w.exec(sb, "time set noon")?;
    w.view_distance(sb, 8)?;

    let mut client = Client::launch(&client_build, &display, port, o.width, o.height, &run.join("client"))?;
    let deadline = Instant::now() + Duration::from_secs(240);
    while w.viewers()? == 0 {
        if !client.alive() {
            bail!("the client exited; see {}", client.log.display());
        }
        if Instant::now() > deadline {
            bail!("the client never joined; see {}", client.log.display());
        }
        w.hold(500)?;
    }
    // A still camera on the door's front: every change from one tick to the next is visible.
    let shot = design_shot(d, 0.0, o.width as f64 / o.height as f64)?;
    let (eye, yaw, pitch) = shot.pose(0.0);
    w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
    w.hold(12_000)?;

    // Door cells as the camera sees them: +x is on the right when the camera looks toward -z.
    let door = spec.door_cells();
    let mirrored = shot.dir[2] < 0.0;
    let grid_pos = |row: usize, col: usize| door[row * 3 + if mirrored { 2 - col } else { col }];
    let (wlo, whi) = watch_box(d, &spec);
    let rest = w.get_region(sb, wlo, whi)?;
    let rest_door: Vec<String> = door.iter().map(|p| rest.get(*p).unwrap_or("?").to_string()).collect();
    let state_of = |r: &Region| -> ([[char; 3]; 3], bool, bool) {
        let mut cells = [['.'; 3]; 3];
        for (row, line) in cells.iter_mut().enumerate() {
            for (col, c) in line.iter_mut().enumerate() {
                let p = grid_pos(row, col);
                *c = cell_char(r.get(p).unwrap_or("?"), r.flags_at(p).is_some_and(|f| f.collides));
            }
        }
        let clear = open_problems(r, &spec).is_empty();
        let closed = door.iter().zip(&rest_door).all(|(p, s)| r.get(*p) == Some(s.as_str()));
        (cells, clear, closed)
    };

    let mut frames: Vec<Frame> = Vec::new();
    let snap = |w: &mut Worker, frames: &mut Vec<Frame>, phase: Phase, tick: u32, settled: bool, secs: f64| -> Result<()> {
        let r = w.get_region(sb, wlo, whi)?;
        let (cells, clear, closed) = state_of(&r);
        // Let the client receive and draw this tick before the screenshot.
        w.hold(o.draw_ms)?;
        let shot = frames_dir.join(format!("s{:04}.png", frames.len()));
        video::screenshot(&display, o.width, o.height, &shot)?;
        frames.push(Frame { shot, phase, tick, settled, cells, clear, closed, secs });
        Ok(())
    };
    snap(&mut w, &mut frames, Phase::Rest, 0, true, o.hold_secs)?;
    for phase in [Phase::Opening, Phase::Closing] {
        w.use_block(sb, spec.lever)?;
        snap(&mut w, &mut frames, phase, 0, false, o.hold_secs * 0.6)?;
        let mut tick = 0;
        loop {
            w.play(sb, 1, 1000.0)?;
            tick += 1;
            let settled = w.status(sb)?.idle;
            snap(&mut w, &mut frames, phase, tick, settled, if settled { o.hold_secs * 1.5 } else { o.secs_per_tick })?;
            if settled {
                break;
            }
            if tick >= o.max_ticks {
                bail!("{phase:?}: still moving after {tick} ticks");
            }
        }
        eprintln!("{phase:?}: settled after {tick} ticks");
    }
    drop(client);

    // Captions, doorway map and phase strip on every screenshot.
    let mut facts = Facts::of(d);
    facts.measure_volume(&mut w, d).context("the title needs the door's volume")?;
    let title = format!("{}: {} ({}), {}", facts.name, facts.size(), facts.placed(), facts.kind());
    let mut list = String::from("ffconcat version 1.0\n");
    let mut last = String::new();
    for (i, f) in frames.iter().enumerate() {
        let phase_frames: Vec<&Frame> = frames.iter().filter(|g| g.phase == f.phase).collect();
        let mut img = Image::load_png(&f.shot)?;
        draw_overlay(&look, &mut img, &title, f, &phase_frames);
        let name = format!("c{i:04}.png");
        img.save_png(&frames_dir.join(&name))?;
        list.push_str(&format!("file '{name}'\nduration {:.3}\n", f.secs));
        last = name;
    }
    // The concat demuxer ignores the last entry's duration unless the file is listed again.
    list.push_str(&format!("file '{last}'\n"));
    let list_path = frames_dir.join("list.txt");
    std::fs::write(&list_path, list)?;
    let status = std::process::Command::new(video::ffmpeg()?)
        .args(["-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path)
        .args(["-vf", "fps=30,format=yuv420p", "-c:v", "libx264", "-preset", "slow", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(out)
        .status()?;
    if !status.success() {
        bail!("ffmpeg failed to encode {}", out.display());
    }
    Ok(())
}

/// The tick of the first frame of `frames` (one phase) where `ok` holds, if any (tick 0 excluded).
fn first_tick(frames: &[&Frame], ok: impl Fn(&Frame) -> bool) -> Option<u32> {
    frames.iter().find(|f| f.tick > 0 && ok(f)).map(|f| f.tick)
}

fn draw_overlay(look: &Look, img: &mut Image, title: &str, f: &Frame, phase: &[&Frame]) {
    let (w, h) = (img.w as i64, img.h as i64);
    let font = &look.font;
    let lh = |s: u32| (LINE_HEIGHT * s) as i64;

    // What is happening, top left.
    let first_clear = first_tick(phase, |g| g.clear);
    let first_closed = first_tick(phase, |g| g.closed);
    let (head, head_color, note, note_color): (String, [u8; 3], String, [u8; 3]) = match f.phase {
        Phase::Rest => ("At rest, lever off".into(), WHITE, "the door is closed and nothing moves".into(), GRAY),
        Phase::Opening | Phase::Closing => {
            let opening = f.phase == Phase::Opening;
            if f.tick == 0 {
                let head = if opening { "Lever flipped ON" } else { "Lever flipped OFF" };
                (head.into(), YELLOW, "nothing has moved yet".into(), GRAY)
            } else if f.settled {
                let good = if opening { f.clear } else { f.closed };
                let head = match (opening, good) {
                    (true, true) => "Settled: OPEN",
                    (true, false) => "Settled: NOT OPEN",
                    (false, true) => "Settled: CLOSED",
                    (false, false) => "Settled: NOT CLOSED",
                };
                (head.into(), if good { GREEN } else { RED }, "nothing moves anymore: this is the only state the judge checks".into(), GRAY)
            } else if opening {
                let head = "Opening".to_string();
                match first_clear {
                    Some(c) if f.clear && c == f.tick => (head, WHITE, "the doorway is clear for the first time".into(), GREEN),
                    Some(_) if f.clear => (head, WHITE, "the doorway is clear".into(), GREEN),
                    Some(c) if c < f.tick => (head, WHITE, format!("blocks moved back into the doorway (it was clear at tick {c})"), RED),
                    _ => (head, WHITE, String::new(), GRAY),
                }
            } else {
                let head = "Closing".to_string();
                match first_closed {
                    Some(c) if f.closed && c == f.tick => (head, WHITE, "the door is closed for the first time".into(), GREEN),
                    Some(_) if f.closed => (head, WHITE, "the door is closed".into(), GREEN),
                    Some(c) if c < f.tick => (head, WHITE, format!("blocks left the doorway again (it was closed at tick {c})"), RED),
                    _ => (head, WHITE, String::new(), GRAY),
                }
            }
        }
    };
    let mut lines: Vec<(String, u32, [u8; 3])> = vec![(title.to_string(), 2, GRAY), (head, 4, head_color)];
    if f.phase != Phase::Rest {
        lines.push((format!("tick {} after the flip", f.tick), 3, WHITE));
    }
    if !note.is_empty() {
        lines.push((note, 2, note_color));
    }
    let pad = 10;
    let box_w = lines.iter().map(|(t, s, _)| font.width(t, *s) as i64).max().unwrap_or(0) + 2 * pad;
    let box_h = lines.iter().map(|(_, s, _)| lh(*s) + 6).sum::<i64>() + 2 * pad;
    img.fill_rect(16, 16, box_w, box_h, [0, 0, 0, 160]);
    let mut y = 16 + pad;
    for (t, s, c) in &lines {
        font.draw(img, 16 + pad, y, t, *s, *c, true);
        y += lh(*s) + 6;
    }

    // The doorway as the server sees it, top right.
    let cell = 34;
    let gap = 4;
    let grid = 3 * cell + 2 * gap;
    let map_w = grid.max(font.width("doorway", 2) as i64) + 2 * pad;
    let map_x = w - 16 - map_w;
    let map_h = lh(2) + 8 + grid + 8 + lh(2) + 2 * pad;
    img.fill_rect(map_x, 16, map_w, map_h, [0, 0, 0, 160]);
    font.draw_centered(img, map_x + map_w / 2, 16 + pad, "doorway", 2, WHITE, true);
    let gx = map_x + (map_w - grid) / 2;
    let gy = 16 + pad + lh(2) + 8;
    for (row, line) in f.cells.iter().enumerate() {
        for (col, c) in line.iter().enumerate() {
            let x = gx + col as i64 * (cell + gap);
            let y = gy + row as i64 * (cell + gap);
            match c {
                '#' => img.fill_rect(x, y, cell, cell, [200, 200, 200, 255]),
                '~' => img.fill_rect(x, y, cell, cell, [255, 170, 0, 255]),
                'h' => img.fill_rect(x, y, cell, cell, [150, 110, 70, 255]),
                _ => {
                    // Air (or a block without collision): an outline only.
                    img.fill_rect(x, y, cell, 2, [255, 255, 255, 120]);
                    img.fill_rect(x, y + cell - 2, cell, 2, [255, 255, 255, 120]);
                    img.fill_rect(x, y, 2, cell, [255, 255, 255, 120]);
                    img.fill_rect(x + cell - 2, y, 2, cell, [255, 255, 255, 120]);
                }
            }
        }
    }
    let (status, color) = if f.clear {
        ("clear", GREEN)
    } else if f.closed {
        ("closed", GRAY)
    } else {
        ("in between", ORANGE)
    };
    font.draw_centered(img, map_x + map_w / 2, gy + grid + 8, status, 2, color, true);

    // The whole phase, one cell per tick, bottom.
    if f.phase == Phase::Rest {
        return;
    }
    let n = phase.len() as i64;
    let strip_h = 26;
    let margin = 16;
    let cw = ((w - 2 * margin - 2 * pad) / n.max(1)).clamp(3, 60);
    let strip_w = cw * n;
    let legend = "each square is one tick:  green = doorway clear,  gray = door closed,  orange = in between";
    let panel_h = lh(2) + 8 + strip_h + 8 + lh(2) + 2 * pad;
    let panel_w = (strip_w + 2 * pad).max(font.width(legend, 2) as i64 + 2 * pad);
    let px = (w - panel_w) / 2;
    let py = h - margin - panel_h;
    img.fill_rect(px, py, panel_w, panel_h, [0, 0, 0, 160]);
    font.draw_centered(img, w / 2, py + pad, legend, 2, WHITE, true);
    let sx = (w - strip_w) / 2;
    let sy = py + pad + lh(2) + 8;
    for (i, g) in phase.iter().enumerate() {
        let c = if g.clear {
            [85, 220, 85, 255]
        } else if g.closed {
            [150, 150, 150, 255]
        } else {
            [255, 170, 0, 255]
        };
        let x = sx + i as i64 * cw;
        img.fill_rect(x + 1, sy, cw - 2, strip_h, c);
        if g.tick == f.tick {
            img.fill_rect(x - 1, sy - 4, cw + 2, 3, [255, 255, 255, 255]);
            img.fill_rect(x - 1, sy + strip_h + 1, cw + 2, 3, [255, 255, 255, 255]);
        }
    }
    let label_y = sy + strip_h + 8;
    let what = if f.phase == Phase::Opening { "opening" } else { "closing" };
    font.draw(img, sx, label_y, &format!("tick 0 ({what} flip)"), 2, GRAY, true);
    let end = format!("tick {} (settled)", phase.last().map(|g| g.tick).unwrap_or(0));
    font.draw(img, sx + strip_w - font.width(&end, 2) as i64, label_y, &end, 2, GRAY, true);
}
