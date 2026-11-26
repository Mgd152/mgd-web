# Integration notes: what `mirrorly-keymap.patch` changes

Base: Mirrorly `ac70daa58ee5f24a067e80b6d0ffbcec4c735f1e` (app 2.0.2), MIT.
Patch: 7 existing files, +457 / -7 lines, LF endings. The `keymap` crate is
NOT in the patch; `apply-to-mirrorly.ps1` copies `crates/keymap` into the
checkout first.

## Verified here (Linux container)

- `git apply --check` and `git apply` succeed on a fresh copy of the upstream
  checkout; the re-generated diff is byte-identical to the patch file.
- `rustfmt --edition 2024` parses the three patched `.rs` files (syntax only).
- Nothing else. **The patched code has never been compiled or run**: the
  container is Linux and the permission policy forbids building upstream
  code. The first compile is the CI workflow
  `.github/workflows/ios-keymapping.yml` on `windows-latest`.

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
without the flag the only differences are two more `if` checks that are
false.

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
  the new `ApplicationHandler::device_event` (`DeviceEvent::MouseMotion`).
- Focus loss, `DeviceEvent::Disconnected`, `quit()` -> `keymap_stop()`.
  `DeviceEvent::Connected` and `rotate()` -> `keymap_sync_view()` (maps
  `input::geom::Rotation` to `keymap::Rotation` variant by variant, screen
  size from `shared.view.screen`).
- `about_to_wait`: `keymap_tick()` runs `engine.tick`, and its
  `next_deadline()` (ms on the app epoch) joins the `ControlFlow::WaitUntil`
  minimum as an `Instant`. The existing control-flow expression only gets one
  more array element.
- `start_probe` returns early while active. Window title shows
  `Keymap ON` / `Keymap (<toggle>)` / `Keymap (no profile)`.
- Side effect to document: with `--keymap`, the toggle key (default
  Backquote) is never typed to the phone.

## Unverified / things most likely to need a fix at first compile

1. Whole patch: never compiled (see above).
2. winit 0.30.13 API used: `Window::set_cursor_grab(CursorGrabMode)`,
   `set_cursor_visible`, `ApplicationHandler::device_event(&mut self,
   &ActiveEventLoop, DeviceId, DeviceEvent)`, `MouseButton::{Back, Forward,
   Other(u16)}`. From memory, not checked against sources (not available
   offline).
3. `idevice::core_device::TouchscreenContact` is re-exported through
   `pub use hid::*` (checked in `vendor/idevice`), and its fields are public
   and it is `Copy` (checked).
4. Let-chains (`if a && let Some(..) = ..`) are used in two places; upstream
   uses them too (edition 2024).
5. Multi-touch over UHS has never been tried on a phone by Mirrorly or by us:
   iOS may ignore, reorder or mis-handle multi-contact reports.
6. Cursor `Confined` on Windows with a borderless window: behaviour at the
   window edge while mouse-look runs is untested (raw motion should keep
   flowing, the look control re-centres itself at `reach`).
7. `apply-to-mirrorly.ps1` and the workflow were never run (no PowerShell /
   Windows here). The script sets `core.autocrlf=false` in the fresh clone so
   `git apply` of the LF patch works under Git for Windows defaults.
8. `cargo test -p keymap` inside Mirrorly: `tests/samples.rs` looks for
   `../../profiles` and skips when it is absent (it is, inside Mirrorly).
