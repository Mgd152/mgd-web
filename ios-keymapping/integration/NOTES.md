# Integration notes: what `mirrorly-keymap.patch` changes

Base: Mirrorly `ac70daa58ee5f24a067e80b6d0ffbcec4c735f1e` (app 2.0.2), MIT.
Patch: 7 existing files, +498 / -8 lines (`git diff --numstat` totals), LF endings (`.gitattributes` keeps them LF; the script also normalizes a CRLF copy before `git apply`). The `keymap` crate is
NOT in the patch; `apply-to-mirrorly.ps1` copies `crates/keymap` into the
checkout first.

## Verified here (Linux container)

- `git apply --check` and `git apply` succeed on a fresh copy of the upstream
  checkout; the re-generated diff is byte-identical to the patch file.
- `rustfmt --edition 2024` parses the three patched `.rs` files (syntax only).
- Nothing else was run in this container: it is Linux and the permission
  policy forbids building upstream code.

## Verified by CI (GitHub Actions run #1, commit 9cb9931, `windows-latest`)

- The patched Mirrorly compiled (`cargo build --release -p app` and
  `-p keymap --bin keymap-sim`, 2m16s) and `cargo test -p keymap -p input -p
  device` passed. The only warning was `main.rs:639` "value assigned to
  `was_active` is never read"; this change removes the dead initial assignment
  (behavior identical). It has not been recompiled since.
- The "Stage the portable folder" step FAILED: `expected exactly one avcodec
  DLL in ...\target\release, found 0`. Upstream `crates/app/build.rs` ran but
  left no DLLs in `target\release` (probably a newer cargo build-dir layout;
  not proven). `apply-to-mirrorly.ps1` now takes the DLLs from
  `third_party\ffmpeg\bin` and checks their SHA-256 against
  `packaging\ffmpeg.txt`. That fix has NOT been run yet; whether staging and
  the artifact upload work will be known from the next run.
- Nothing has been run on a real Windows PC or an iPhone. Multi-touch on the
  device, latency, cursor confinement at window edges are unverified.

## File by file

| File | Change |
|---|---|
| `Cargo.toml` | `keymap = { path = "crates/keymap" }` in `[workspace.dependencies]`. (`members = ["crates/*", ...]` already picks the new crate up.) |
| `Cargo.lock` | Hand-added `keymap` package (deps `serde`, `serde_json`, both already in the lock) and `"keymap"` in the dependencies of `app` and `input`. cargo repairs it if imperfect; CI prints the resulting diff. |
| `crates/input/Cargo.toml` | `keymap.workspace = true`. |
| `crates/input/src/lib.rs` | `pub use keymap;`, new `HidEvent::MultiTouch(keymap::Frame)` (derive list unchanged: `Frame` is `Copy + PartialEq + Debug`), test `keymap_key_names_match_winit` (116 names from `keymap::keys::KEY_NAMES` as `KeyCode::X`, checks the Debug string, `is_known_key`, and the count). |
| `crates/device/src/hid.rs` | `Op::Multi(Frame)`; `expand` maps `MultiTouch` to one zero-delay op; `superseded` gets one arm (`MultiTouch` behind `MultiTouch` is dropped only if `Frame::droppable()`); `exec` builds a `[TouchscreenContact; 5]` on the stack and calls `uhs.send_multitouch(&buf[..n], None)`. `route_touch` is untouched, so multi-touch always goes to UHS. Two new unit tests (coalescing rule, expand + not rerouted to Indigo). Existing single-touch logic and `touching` flag untouched. |
| `crates/app/Cargo.toml` | `keymap.workspace = true`. |
| `crates/app/src/main.rs` | See below. |

## `crates/app/src/main.rs`

Everything is behind `self.keymap.is_some()` (set only by `--keymap FILE`);
without the flag the only differences are about ten checks that are
trivially false (`self.keymap.is_none()`, `keymap_active()`), one new
`probe_busy_until` field and a few calls that return immediately. Keep that
in mind when reviewing: it is small, but not literally zero.

- `Args.keymap: Option<PathBuf>` (`--keymap <FILE>`), `App.keymap:
  Option<KeymapHost { path, engine, grabbed }>`.
- Startup: profile loaded after `device::start` (error -> log + toast, app
  keeps running). Re-read on every activation (`keymap_gate`).
- Activation gate (toggle key pressed while inactive): refuses with a toast
  when not connected, on Wi-Fi (override: env `MIRROR_KEYMAP_WIFI=1`), while
  the iPad display is on, or during a wheel-scroll script. Then reload,
  `release_all()`, `keymap_sync_view()`, feed the press to the engine.
- On `Activated`: if the profile has a `look` control, confine (not lock)
  the cursor and hide it; hide the bar; toast. `Deactivated`/`Panic` ->
  `keymap_after_stop()` (ungrab, show cursor, title, hover).
- Keyboard: engine sees the key first (never auto-repeat). Not consumed and
  active: only F-key shortcuts run (Ctrl+digit suppressed through `ctrl &&
  !active`, F9 suppressed), nothing is typed on the phone.
- Mouse: while active, buttons go to the engine as `MouseLeft/...`, wheel
  and cursor moves are swallowed, hover update skipped. Raw motion comes from
  the new `ApplicationHandler::device_event` (`DeviceEvent::MouseMotion`); it
  is only summed there (`KeymapHost.motion`) and delivered to the engine as
  ONE `Input::MouseMotion` per event-loop iteration from `about_to_wait`, so
  a 1000-8000 Hz mouse does not turn into that many HID reports per second.
- Focus loss, `DeviceEvent::Disconnected`, `quit()` and the user's
  Disconnect (`set_connected(false)`) -> `keymap_stop()`. For the user's
  Disconnect and for quit the lift frame is BEST EFFORT only: it is queued
  before `set_connected(false)`, but upstream session.rs calls
  `hid_task.abort()` as soon as the link is no longer wanted, so the frame can
  lose that race. What iOS does with contacts left down when the HID
  connection closes is unknown (unverified).
  `DeviceEvent::Connected` and `rotate()` -> `keymap_sync_view()` (maps
  `input::geom::Rotation` to `keymap::Rotation` variant by variant, screen
  size from `shared.view.screen`).
- `about_to_wait`: `keymap_tick()` runs `engine.tick`, and its
  `next_deadline()` (ms on the app epoch) joins the `ControlFlow::WaitUntil`
  minimum as an `Instant`. The existing control-flow expression only gets one
  more array element.
- `start_probe` returns early while active, and sets `probe_busy_until`
  (`PROBE_TAPS * (PROBE_GAP + PROBE_HOLD)`); `keymap_gate` refuses to activate
  until the probe thread is done, so its single touch (identity 2) never
  interleaves with keymap frames. The wheel-scroll script is guarded the same
  way (`scroll_busy_until`). Window title shows
  `Keymap ON` / `Keymap (<toggle>)` / `Keymap (no profile)`.
- Mirrorly is single-instance (named mutex `Local\Mirrorly.SingleInstance`):
  starting it with `--keymap` while a Mirrorly window already runs just brings
  the old window forward and the flag is silently ignored. Close the running
  Mirrorly first (also from the tray).
- When a new `Connected` event changes the screen size/rotation while the
  keymap is active, the engine leaves active mode; the toast says neutrally
  "Keymap off: the picture changed".
- Side effect to document: with `--keymap`, the toggle key (default
  Backquote) is never typed to the phone.

## Unverified (behavior at run time; the code itself compiled on CI)

1. Whole patch: compiled and unit-tested on CI only (see above); never run.
2. winit 0.30.13 API used: `Window::set_cursor_grab(CursorGrabMode)`,
   `set_cursor_visible`, `ApplicationHandler::device_event(&mut self,
   &ActiveEventLoop, DeviceId, DeviceEvent)`, `MouseButton::{Back, Forward,
   Other(u16)}`. These calls compile on CI (so the signatures are right), but
   their runtime behavior on Windows (e.g. `Confined` at window edges) is untested.
3. `idevice::core_device::TouchscreenContact` is re-exported through
   `pub use hid::*` (checked in `vendor/idevice`), and its fields are public
   and it is `Copy` (checked).
4. Let-chains (`if a && let Some(..) = ..`) are used in two places; upstream
   uses them too (edition 2024).
5. Multi-touch over UHS has never been tried on a phone by Mirrorly or by us:
   iOS may ignore, reorder or mis-handle multi-contact reports.
5b. Rate of multi-touch reports under mouse-look is unmeasured. Raw mouse
   motion is coalesced to one engine input per loop iteration, but that can
   still be one report per frame/iteration; how the USB tunnel and iOS cope is
   unknown. If the phone lags or drops frames, lower the mouse polling rate
   (e.g. 500-1000 Hz) and report it.
6. Cursor `Confined` on Windows with a borderless window: behaviour at the
   window edge while mouse-look runs is untested (raw motion should keep
   flowing, the look control re-centres itself at `reach`).
7. `apply-to-mirrorly.ps1` and the workflow were never run (no PowerShell /
   Windows here). Line endings: `ios-keymapping/.gitattributes` forces LF for
   the patch, the workflow sets `core.autocrlf=false` before checkout, and the
   script itself, if it finds CR bytes in the patch, applies an LF-normalized
   temp copy (a CRLF patch fails `git apply --check` against the LF clone; this
   was reproduced here with a CRLF copy). The CRLF-normalizing code path
   itself has not been executed (no PowerShell). `-Build` / `-StageOnly` now
   fail when LICENSE, NOTICE, README.md, profiles/*.json, editor/index.html or
   keymap-sim.exe is missing.
10. Disconnect/quit lift is a race with `hid_task.abort()` (see above); never
    exercised on a phone.
9. The workflow's sample-profile step needs `profiles/touch-test-*.json` to
   exist and validate; do not push before they do.
8. `cargo test -p keymap` inside Mirrorly: `tests/samples.rs` looks for
   `../../profiles` and skips when it is absent (it is, inside Mirrorly).

Workflow note: the Windows job no longer has `needs: keymap`, so a missing
sample profile only turns the `keymap` job red; the Windows compile still runs.
Staging (`-Build`/`-StageOnly`) still fails without LICENSE, NOTICE, README.md,
profiles and editor, so do not expect a packaged artifact before those exist.
Other fixes: probe margin is now +500 ms; panic key while inactive no longer
shows a toast or runs the stop path.
