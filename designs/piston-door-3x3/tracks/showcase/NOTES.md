# Track showcase: make the night's results easy and fun to look at

Not a door track: tools that turn the archive into things to watch, all rendered by the real
Minecraft client (`door video` pipeline) and cut with the static ffmpeg build, in Rust.

## Commands (branch explore/showcase)
* `mctas door gallery --videos ~/videos/gallery --upload videos`: picks the best design of
  each board from the archive (verdicts and worst-cycle timings from the leaderboard cache,
  new designs verified on the spot): solid, slime-free, sculk, gravity (fewest blocks; ties go
  to the record progression) and fastest solid (open + close ticks). Renders a clean 4:3 clip
  of each (door camera swinging around the door), makes a 640x480 looping GIF with a
  Minecraft-font label (palettegen/paletteuse, 1.5x speed, ~1-2 MB), uploads GIFs and clips to
  the release, writes `designs/piston-door-3x3/GALLERY.md` (GIF + one line per design: blocks,
  category, ticks, how it works from `designs/piston-door-3x3/CAPTIONS.md`, then the archive
  README's progression table).
* `mctas door montage --videos ~/videos/montage --out ~/videos/montage/solid-progression.mp4 --upload videos`:
  the solid record progression (archive README table, plus a newer record if the archive has
  one) as one video: intro card, every design filmed by the SAME camera (aimed at the union of
  all the designs, door at the origin, same floor), so the machine visibly shrinks around a
  door that stays put, each clip at game speed (2x the 10 tps recording) with a label (block
  count, delta, caption, file), outro card on the current record. Also a poster
  (`solid-progression.jpg`: every record's frame with the door open, in a grid with its block
  count), embedded at the top of GALLERY.md and linking to the video. Clips are cached per
  camera in `<videos>/clips-<hash>/` (only new records get rendered next time).
* `mctas door video` (and `door video-all`) default style `card`: a 2 s title card (block
  count, category, timings from one verify cycle, design name) drawn in the game's font on the
  darkened dirt background, fading into the clip; the door camera; a corner label.
  `--style clean` (no card/label), `--style classic` (the original still camera framing the
  whole build), `--orbit N` (degrees each way, 0 = still), `--size WxH`. `video-all --redo`
  re-records existing videos (e.g. to give every archived video the new look).
* `mctas door card <design> --out card.png [--label label.png]`: draw the title card / label
  without recording.

## Door camera (piston-door/src/video.rs::door_shot)
* Direction: front or back of the door, from its left or right, 15 or 30 degrees up, whichever
  hides the 3x3 opening least over the orbit (rays from 36 points of the door face toward the
  camera, counting crossed blocks; door cells excluded). Ties: front-left, lower.
* Aim: the build's screen bounds over the whole orbit are centered (largest picture), then the
  aim moves 25% toward the center of the opening; the camera backs off until every block stays
  inside 90% of the frame for every orbit angle.
* Orbit: yaw offset `orbit * sin(2 pi t)` over the clip (t = 0..1): starts and ends at the
  same pose, so a GIF of the clip loops seamlessly. The camera is moved between ticks (about
  35 Hz: `camera` + `hold`), ticks are paced from Rust (`play 1 tick` each 1/tps s).
* 4:3 for gallery GIFs (door builds are taller than wide), 16:9 for videos.
* Gallery GIFs use a still camera: with the orbit every pixel changes every frame and a 640x480
  GIF of a clip was 12-15 MB; still, 0.65-1 MB.
* Floor: grass out to the client's render distance (sandbox of 14x14 chunks, floor 220x220),
  so the horizon is fog instead of the edge of a small floor. The level only sent viewers 2
  chunks around them (its view distance comes from the PlayerList: 0, clamped to 2); the
  bridge's new `view` op (`Worker::view_distance`) raises it to 8 for videos.

## Minecraft font (mctas/src/mcfont.rs)
The client jar's bitmap providers (`assets/minecraft/font/include/default.json`: ascii.png,
accented, nonlatin_european), glyph widths from the sheet like the game (ink width + 1 px
advance, space = 4), drop shadow at +1,+1 at 1/4 brightness. PNGs are decoded/encoded through
ffmpeg (rawvideo rgba), so no image crate. The static ffmpeg has no `drawtext` (7.0 needs
harfbuzz), which is why text is drawn in Rust and overlaid as PNG.

## Fixed on the way
* `Display::start` found no free X display: 110 stale `/tmp/.X*-lock` files left by Xvfb
  servers killed with SIGKILL (every video so far). Now Xvfb gets SIGTERM first (it removes its
  lock and socket) and locks whose process is dead are reclaimed.
