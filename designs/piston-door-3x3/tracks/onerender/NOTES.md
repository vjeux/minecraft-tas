# onerender: every door recorded once

vjeux: "Why are we wasting time doing two renderings?" Each door used to be recorded twice in the
real client: `door video` / `video-all` (1280x720, orbiting camera, title card and label baked into
the recording) and `door gallery` (its own clean 960x720 clip per board leader, then the GIF). A new
label (a volume that changed) meant recording again.

## What changed (`crates/piston-door/src/clips.rs`)

- One clean clip per design: still door camera, no card, no label, 1280x720 with the build framed
  for a centered 4:3 crop, x264 crf 16, in `~/videos/clips/<design hash>.clip.mp4` (+ `.clip.png`,
  + the `.clip.txt` sidecar: design hash, recording settings, ticks played, first-cycle open/close
  ticks). Keyed by design content (blocks, ports, entities; not the name or the file name), so a
  track's BEST.txt and its archive copy share one clip. Recorded again only when the design or the
  settings change (`LOOK` in clips.rs), or `--redo`.
- `door video` / `video-all`: the title card (still, fading into the clip) + the corner label
  overlaid, H.264 yuv420p, same names (`<stem>.mp4`) and uploads. Every product has a `.stamp`
  (hash of the clip's sidecar + its card and label lines): `video-all` re-cuts exactly the videos
  whose label changed, without Minecraft; `--relabel` re-cuts all of them and records nothing.
- `door gallery`: the GIFs cut from the same clips (crop 4:3, label overlay); a leader without a
  clip is recorded first; `--gif-only` records nothing. Same names (`<stem>.gif`) and uploads.
- `tick-video`, `montage` (one shared camera per progression) and `--style classic` keep their own
  recordings.

## Measured (devvm55865, flat-b-v7-44b, 2026-10-18)

| | before (main 795ba32) | after |
|---|---|---|
| video of a door | 62 s (record + card) | 61 s once (60 s clip + 1.5 s cut) |
| board leader's GIF | +60 s (second recording) + GIF | 0.6 s from the same clip |
| new label (video) | 62 s (record again) | 1.5 s (cut, no Minecraft) |
| new label (GIF) | 60 s (record again) | 0.6 s |
| published video size | 4.0 MB (orbit) | 0.9 MB (still) |

The GIF cut from the clip has the old GIF's framing, size (640x480) and length (8.4 s).
