# mediabunny (vendored)

The viewer's video recorder (the Record button, `crates/mcview-web/src/record.rs`) encodes its
frames with [mediabunny](https://github.com/Vanilagy/mediabunny) (vjeux 2026-10-20: "add a record
video button using mediabunny").

* Version: 1.62.0, from the npm package `mediabunny@1.62.0`
  (`mediabunny-1.62.0.tgz`, sha256 `943c852c707e4bce6afe73d5dec43da894eb964dbf3713460c630368f7393d16`).
* `mediabunny.min.mjs`: the package's `dist/bundles/mediabunny.min.mjs`, unmodified
  (sha256 `9935ec993ac171336b0c097051376bef1298e33882be23bff63c9ae57b5c3064`).
* `LICENSE`: the package's license, Mozilla Public License 2.0 (MPL-2.0). The file is
  distributed unmodified; its source form is the package above.

`mctas web bundle` copies both next to the viewer (`web/viewer/mediabunny.min.js`, renamed:
every static server sends `.js` as JavaScript, which a module import requires, and
`web/viewer/mediabunny.LICENSE.txt`). The viewer imports it only when a recording starts (no
network at page load: the bundle is served with the viewer).
