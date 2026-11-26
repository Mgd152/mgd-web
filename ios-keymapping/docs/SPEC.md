# iOS keymapping for Mirrorly — implementation spec (v1)

Status: authoritative. Implementers and reviewers follow this document; where
it disagrees with the lead's design notes, this document wins (see section 0).
Language rules: code, comments, identifiers and this spec are English;
user-facing docs (`README.md`, `docs/BAO_CAO.md`) and the editor UI are
Vietnamese; strings added inside the Mirrorly app stay English (upstream's UI
language).

Upstream: <https://github.com/AnasSAV/mirrorly>, MIT (c) 2026 AnasSAV, commit
`ac70daa58ee5f24a067e80b6d0ffbcec4c735f1e` (app 2.0.2). All "upstream" file and
line references below are against that commit.

## 0. Deviations from design notes

Checked against the upstream source; these points of the design notes were
wrong or incomplete and are changed here.

1. **Keymapping is USB-only.** `crates/device/src/session.rs` (~326-334)
   records that over Wi-Fi iOS *ignores* `universalhidservice` touch reports
   (measured upstream 2026-10-18), which is why upstream routes Wi-Fi touches
   through Indigo. Multi-touch exists only as a UHS report, so "always via UHS
   even when touch_path is Indigo" would silently do nothing on Wi-Fi. The app
   therefore refuses to activate the keymap on a Wi-Fi link (override for
   experiments: env `MIRROR_KEYMAP_WIFI=1`). The HID executor still sends
   `MultiTouch` via UHS unconditionally.
2. **Upstream's single touch is identity 2** (`TOUCHSCREEN_STATE_CONTACT =
   0xC2` = touching | identity 2). Keymap frames use identities 0..4, so mouse
   touches, wheel-scroll scripts and the F9 probe must never interleave with
   keymap frames. Activation calls upstream `release_all()` first, is refused
   while a wheel-scroll script is still running (`scroll_busy_until`), and while
   active the gesture path, wheel and F9 probe are blocked (section 7.4).
3. **`Cargo.lock` must change too** (new `keymap` package; `input` and `app`
   depend on it). The patch carries a hand-edited `Cargo.lock`; nothing builds
   with `--locked` (upstream's CI and `build-release.ps1` do not either), so
   cargo repairs it if the hand edit is imperfect. CI prints the resulting
   `Cargo.lock` diff.
4. **`device` does not depend on `keymap`.** `input` re-exports the crate
   (`pub use keymap;`); `device` names the type as `input::keymap::Frame`.
   `app` depends on `keymap` directly.
5. **Name clash:** `main.rs` already imports `device::DeviceEvent`. winit's raw
   device event must be referred to as `winit::event::DeviceEvent` by full path
   (or `as RawDeviceEvent`).
6. **CLI:** upstream parses args with clap derive (`struct Args`), so the flag
   is a new field `#[arg(long, value_name = "FILE")] keymap: Option<PathBuf>`.
7. **`apply-to-mirrorly.ps1 -Build` does not run `build-release.ps1`:** that
   script throws without cargo-about and Inno Setup 6. `-Build` runs
   `get-ffmpeg.ps1` and `cargo build --release -p app` inside vcvars64 itself
   and stages a portable folder. A separate `-Package` switch runs upstream's
   `build-release.ps1` for users who have those tools.
8. **"Control only, no video" is not implemented.** iOS drops HID reports
   unless a displayservice media stream runs (FEASIBILITY §5.1). v1 documents
   this; it adds no mode switch. The other latency items of the handoff
   (separate input task, latest-frame slot, GPU decode) already exist upstream
   and are only documented, not re-implemented.
9. **Engine API additions/precisions:** `Engine::profile()`,
   `Engine::contacts_down()`, `Frame::droppable()`, `Frame::from_contacts()`;
   `handle()` runs `tick(now_ms)` first; `set_view()` with a *changed* view
   lifts all contacts even when the orientation still matches (stale panel
   positions otherwise); callers must filter OS key auto-repeat; structured
   `ProfileError` with a field path; unknown JSON fields are rejected.
10. **Key list is an explicit subset** of winit 0.30.13 `KeyCode` names
    (section 3). Because the patch cannot be compiled in the authoring
    container, the patch adds a test in `crates/input` that proves every name
    equals the `Debug` name of an existing `KeyCode` variant.
11. **Cursor grab/hide happens only when the profile has a `look` control.**
    Without one the cursor stays visible and free (clicks still go to the
    engine, not to the touch path).
12. **While active, Ctrl+1/2/3 shortcuts are suppressed** (a profile may bind
    `ControlLeft`); only the F-key shortcuts remain, minus F9.
13. The handoff's 30-50 ms input-to-photon target is not adopted. Upstream
    measured ~87 ms p50 tap -> changed frame (phone side dominates). No latency
    number may be claimed for this project until measured on hardware.

## 1. Hard scope limits (non-negotiable)

- One key/button press = one human action = at most one contact going down.
  No macros, sequences, recorded input, repeat/turbo, auto-fire, auto-aim,
  recoil compensation, timing assistance, or any input the user did not
  physically produce at that moment. The only timed behaviours are the fixed
  40 ms tap lift and the optional idle lift of the look finger.
- No detection evasion, device spoofing, randomized "humanizing", hiding from
  apps, or anything meant to change how an app/game classifies the device.
- No jailbreak, emulator, IPA modification. Only upstream's CoreDevice path.
- Samples and tests target a touch-test app, never a competitive game.
- Never claim something was tested on an iPhone or on Windows unless it was.
  Multi-touch via `send_multitouch` has never been run against a phone by
  upstream or by us: every doc must label it **unverified**.

Reviewers reject any change that crosses these lines even if asked by a task.

## 2. Repository layout

```
ios-keymapping/
  Cargo.toml  Cargo.lock(committed)  .gitignore  LICENSE  NOTICE  README.md
  crates/keymap/
    Cargo.toml                  deps: serde 1 (derive), serde_json 1. No others, no dev-deps.
    src/lib.rs  geom.rs(existing, API frozen)  keys.rs  profile.rs  engine.rs
    src/bin/keymap-sim.rs
    tests/data/valid/*.json  tests/data/invalid/*.json  tests/data/invalid/expected.json
    tests/vectors.rs  tests/samples.rs  tests/sim_cli.rs
  profiles/touch-test-portrait.json  touch-test-landscape.json
  editor/index.html             single self-contained file
  editor/tests/editor.test.mjs  Playwright script (uses the preinstalled global playwright)
  integration/mirrorly-keymap.patch  apply-to-mirrorly.ps1
  docs/SPEC.md  docs/BAO_CAO.md
../.github/workflows/ios-keymapping.yml   (the only file outside ios-keymapping/)
```

The crate must compile unchanged both in this workspace and when copied to
`mirrorly/crates/keymap` (Mirrorly's workspace has `members = ["crates/*"]`,
`edition = "2024"`, `license = "MIT"` in `[workspace.package]`). Therefore: no
paths leaving the crate directory in `src/` or in tests that must pass there;
`#![forbid(unsafe_code)]`; no warnings under `cargo clippy -- -D warnings`.

`lib.rs`:

```rust
#![forbid(unsafe_code)]
pub mod engine;
pub mod geom;
pub mod keys;
pub mod profile;
pub use engine::{ActivateError, Contact, Engine, EngineEvent, Frame, Input, Output, Stats,
                 MAX_CONTACTS, TAP_HOLD_MS};
pub use geom::{Rotation, View};
pub use profile::{ButtonMode, Control, Orientation, Problem, Profile, ProfileError};
```

## 3. `keys.rs`

```rust
/// Keyboard key names: exactly these winit 0.30 `KeyCode` Debug names.
pub const KEY_NAMES: &[&str];
/// "MouseLeft", "MouseRight", "MouseMiddle", "MouseBack", "MouseForward".
pub const MOUSE_NAMES: &[&str];
pub fn is_known_key(name: &str) -> bool;      // KEY_NAMES or MOUSE_NAMES, case-sensitive
pub fn is_mouse_button(name: &str) -> bool;
pub const DEFAULT_TOGGLE_KEY: &str = "Backquote";
pub const DEFAULT_PANIC_KEY: &str = "F8";
```

`KEY_NAMES` (116 names, no duplicates; a unit test asserts the count and
uniqueness):

- `KeyA`..`KeyZ` (26), `Digit0`..`Digit9` (10), `F1`..`F24` (24)
- `Space Tab Enter Escape Backspace CapsLock ContextMenu` (7)
- `ShiftLeft ShiftRight ControlLeft ControlRight AltLeft AltRight SuperLeft SuperRight` (8)
- `Backquote Minus Equal BracketLeft BracketRight Backslash Semicolon Quote Comma Period Slash IntlBackslash` (12)
- `ArrowUp ArrowDown ArrowLeft ArrowRight Insert Delete Home End PageUp PageDown` (10)
- `NumLock Numpad0`..`Numpad9 NumpadAdd NumpadSubtract NumpadMultiply NumpadDivide NumpadDecimal NumpadEnter` (17)
- `ScrollLock Pause` (2)

(26+10+24+7+8+12+10+17+2 = 116.)

Mouse buttons may be bound to controls but **not** used as `toggle_key` or
`panic_key`.

## 4. `profile.rs` — JSON profile v1

### 4.1 Schema

All coordinates are fractions 0..1 of the **picture as the user sees it** in
the profile's orientation (origin top-left). Lengths (`radius`, `reach`) are
fractions of the picture's **shorter** side.

| Field | Type | Required | Default | Rule |
|---|---|---|---|---|
| `version` | integer | yes | – | must equal `1` |
| `name` | string | no | `""` | ≤ 100 chars |
| `app` | string | no | `""` | free text, ≤ 100 chars |
| `orientation` | `"portrait"` \| `"landscape"` | yes | – | |
| `toggle_key` | key name | no | `"Backquote"` | keyboard key (not mouse) |
| `panic_key` | key name | no | `"F8"` | keyboard key (not mouse), ≠ `toggle_key` |
| `controls` | array | no | `[]` | ≤ 64 entries |

Controls are tagged by `"type"`:

| type | Field | Type | Required | Default | Rule |
|---|---|---|---|---|---|
| all | `label` | string | no | `""` | ≤ 40 chars |
| all | `x`, `y` | number | yes | – | finite, in [0, 1] |
| `joystick` | `radius` | number | yes | – | in (0, 0.5] |
| `joystick` | `up` `down` `left` `right` | key name | yes | – | known key |
| `look` | `sensitivity` | number | no | `1.0` | in (0, 100] |
| `look` | `invert_y` | bool | no | `false` | |
| `look` | `reach` | number | no | `0.3` | in (0, 1] |
| `look` | `release_after_ms` | integer | no | `0` | 0..=60000; 0 = never |
| `button` | `key` | key name | yes | – | known key |
| `button` | `mode` | `"hold"` \| `"tap"` | no | `"hold"` | |

Global rules: at most one `look` control; every key name in the profile
(`toggle_key`, `panic_key`, joystick directions, button keys) is used at most
once. More than 5 controls is valid (only 5 contacts can be down at once).
Unknown fields anywhere are an error (`#[serde(deny_unknown_fields)]`).

### 4.2 Rust API

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation { Portrait, Landscape }
impl Orientation {
    pub fn matches(self, rotation: Rotation) -> bool; // Landscape <=> rotation.is_landscape()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonMode { #[default] Hold, Tap }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Control {
    Joystick { #[serde(default)] label: String, x: f64, y: f64, radius: f64,
               up: String, down: String, left: String, right: String },
    Look     { #[serde(default)] label: String, x: f64, y: f64,
               #[serde(default = ..1.0)] sensitivity: f64, #[serde(default)] invert_y: bool,
               #[serde(default = ..0.3)] reach: f64, #[serde(default)] release_after_ms: u32 },
    Button   { #[serde(default)] label: String, x: f64, y: f64, key: String,
               #[serde(default)] mode: ButtonMode },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    #[serde(default)] pub name: String,
    #[serde(default)] pub app: String,
    pub orientation: Orientation,
    #[serde(default = "Backquote")] pub toggle_key: String,
    #[serde(default = "F8")] pub panic_key: String,
    #[serde(default)] pub controls: Vec<Control>,
}

/// One validation finding. `path` is e.g. "version", "panic_key",
/// "controls[2].radius", "controls[3].up", "controls".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem { pub path: String, pub message: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Not JSON / wrong shape; the serde_json message (with line and column).
    Json(String),
    /// Well-formed but breaks a rule. Display: "{path}: {message}".
    Invalid(Problem),
}
impl std::fmt::Display for ProfileError; impl std::error::Error for ProfileError;

impl Profile {
    pub fn from_json(text: &str) -> Result<Profile, ProfileError>; // parse + validate
    pub fn to_json_pretty(&self) -> String;                         // 2-space, trailing "\n"
    pub fn problems(&self) -> Vec<Problem>;                         // all findings, in order
    pub fn validate(&self) -> Result<(), ProfileError>;             // first of problems()
    pub fn has_look(&self) -> bool;
    pub fn bound_keys(&self) -> Vec<(String /*path*/, &str /*key*/)>; // toggle, panic, then controls in order
}
```

`from_json` strips a leading UTF-8 BOM (Windows editors add one).

### 4.3 Validation order and messages (`problems()`)

Order: `version`, `name`, `app`, `toggle_key`, `panic_key`, `controls` (count),
then each control in array order with fields in this order: `label`, `x`, `y`,
type-specific numeric fields in table order, key fields in table order
(`up`, `down`, `left`, `right` / `key`); then the second-look rule (path
`controls[i]` of the second look, message `only one look control is allowed`);
then duplicates.

Duplicate keys: walk `bound_keys()` in order; the **later** occurrence gets
the problem, e.g. path `controls[1].key`, message
`key "Space" is already used by controls[0].up`. Unknown names are reported
once as `unknown key "Foo"` and take no part in duplicate detection.
`toggle_key`/`panic_key` that are mouse names: `must be a keyboard key`.
Range messages name the range and the value: `must be in (0, 0.5], got 0.7`.

The exact `path` strings are a contract (the editor mirrors them and the
shared test vectors assert them); message wording may vary but must contain
the offending value or key.

## 5. `engine.rs`

### 5.1 Types

```rust
pub const MAX_CONTACTS: usize = 5;
pub const TAP_HOLD_MS: u64 = 40;

/// One finger in HID panel space (0..=65535, native portrait panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Contact { pub id: u8, pub touching: bool, pub x: u16, pub y: u16 }

/// A full snapshot: every contact that is down, plus contacts lifting now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame { contacts: [Contact; MAX_CONTACTS], len: u8, transition: bool }
impl Frame {
    /// None if empty, more than MAX_CONTACTS, an id >= 5, or duplicate ids.
    /// Unused array entries are `Contact::default()` (so `==` is well defined).
    pub fn from_contacts(contacts: &[Contact], transition: bool) -> Option<Frame>;
    pub fn contacts(&self) -> &[Contact];      // ascending id, 1..=5 entries
    pub fn has_transition(&self) -> bool;      // some contact goes down or up in this frame
    /// True for pure-move frames: a sender may drop this frame if a later
    /// frame is already queued. Transition frames are never droppable.
    pub fn droppable(&self) -> bool;           // == !has_transition()
}

pub enum Input<'a> { Press(&'a str), Release(&'a str), MouseMotion { dx: f64, dy: f64 } }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent { Activated, Deactivated, Panic, ActivateFailed(String) }

#[derive(Debug, Default)]
pub struct Output { pub frames: Vec<Frame>, pub consumed: bool, pub event: Option<EngineEvent> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivateError { NoView, OrientationMismatch { profile: Orientation, rotation: Rotation } }
impl Display + Error for ActivateError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats { pub frames: u64, pub dropped_presses: u64, pub look_recenters: u64 }

impl Engine {
    pub fn new(profile: Profile) -> Result<Engine, ProfileError>; // validates
    pub fn profile(&self) -> &Profile;
    pub fn view(&self) -> Option<View>;
    pub fn set_view(&mut self, view: View) -> Vec<Frame>;
    pub fn is_active(&self) -> bool;
    pub fn activate(&mut self) -> Result<(), ActivateError>;
    pub fn deactivate(&mut self) -> Option<Frame>;
    pub fn release_all(&mut self) -> Option<Frame>;
    pub fn handle(&mut self, input: Input<'_>, now_ms: u64) -> Output;
    pub fn tick(&mut self, now_ms: u64) -> Vec<Frame>;
    pub fn next_deadline(&self) -> Option<u64>;
    pub fn contacts_down(&self) -> usize;
    pub fn stats(&self) -> Stats;
}
```

The engine is pure: no I/O, clocks, threads, randomness or globals. Time is
whatever the caller passes; if `now_ms` goes backwards it is treated as the
largest value seen so far.

### 5.2 Frame invariants (tested by the fuzz test)

- I1. A frame has 1..=5 contacts, ids unique, each `< 5`, sorted by id.
- I2. Every contact down after the previous frame appears in this frame,
  either `touching=true` or (exactly once) `touching=false` at its last sent
  position. An id never disappears without its lift.
- I3. An id appears with `touching=false` only if it was down before the frame.
- I4. A lifted id may be reused from the *next* frame on, never in the same
  frame. New contacts take the lowest free id.
- I5. `has_transition()` is true iff some contact in the frame is newly down
  (not down before the frame) or lifting.
- I6. No frame is identical in content to "nothing changed": a move frame is
  emitted only if at least one contact's HID `(x, y)` changed.
- I7. After `deactivate()`, `release_all()` or a panic, `contacts_down() == 0`.
- I8. While inactive the engine emits no frames (except the lift frame of the
  call that deactivates it).

### 5.3 Geometry

With `view` set: `(u, v)` picture point -> `view.to_hid(u, v)` (clamps to
[0,1], rotates into panel space). Short-side lengths:
`(fx, fy) = view.short_side_units(len)`. Joystick target for direction
`d` (unit vector or zero): `(x + d.x * fx(radius), y + d.y * fy(radius))`.
Look position for offset `o` (short-side units):
`(x + o.x * ux, y + o.y * uy)` with `(ux, uy) = view.short_side_units(1.0)`.

### 5.4 State and lifecycle

- New engine: inactive, no view.
- `set_view(v)`: if `v` equals the current view -> no frames. Otherwise store
  it; if contacts are down, emit one lift frame for all of them and forget
  held keys (as `release_all`); if the engine is active and
  `!profile.orientation.matches(v.rotation)` it also deactivates (the caller
  detects this by comparing `is_active()` before and after). Returns 0 or 1
  frames.
- `activate()`: `Err(NoView)` without a view; `Err(OrientationMismatch)` if the
  profile orientation does not match the view rotation; otherwise active (no
  frames; idempotent).
- `deactivate()`: lift all contacts in one frame (`None` if none were down),
  forget held keys and pending deadlines, become inactive. Idempotent.
- `release_all()`: same lift, forget held keys and deadlines, **stay** in the
  current active state.
- "Forget held keys": a key physically still held produces nothing more until
  the engine sees a fresh `Press` for it. Callers MUST NOT forward OS
  auto-repeat presses (winit `KeyEvent::repeat`); the engine additionally
  ignores a `Press` for a key it already considers held.

### 5.5 `handle(input, now_ms)`

Step 0: run `tick(now_ms)`; its frames come first in `Output::frames`.

| State | Input | Result |
|---|---|---|
| any | `Press(panic_key)` | lift-all frame (if any), deactivate, `consumed = true`, `event = Panic` (also when already inactive) |
| any | `Release(panic_key)` / `Release(toggle_key)` | `consumed = true`, nothing else |
| inactive | `Press(toggle_key)` | `activate()`: ok -> `event = Activated`; err -> `event = ActivateFailed(err.to_string())`; `consumed = true` |
| active | `Press(toggle_key)` | lift-all frame (if any), deactivate, `event = Deactivated`, `consumed = true` |
| inactive | anything else | `consumed = false`, no frames |
| active | `Press`/`Release` of a key bound to a control | `consumed = true`; frames per 5.6 |
| active | `Press`/`Release` of any other name (including unknown names) | `consumed = false`, no frames |
| active | `MouseMotion` | `consumed = true`; frames per 5.6 (none if the profile has no look control) |

A `Release` for a bound key the engine does not consider held is consumed
(when active) and otherwise ignored.

### 5.6 Control behaviour and exact frame order

Notation: `[T: 0↓]` transition frame where id 0 goes down, `[M]` move frame,
`[T: 0↑]` lift. Every frame also lists all other contacts currently down.

**Slot exhaustion.** If a control needs a new contact and 5 are down, the
press (or mouse motion) is ignored, `stats.dropped_presses += 1` (once per
press; for look, once per motion event), the key is still recorded as held so
its release is harmless, and for a joystick the direction state is still
tracked but no contact is created by later key changes of that same hold
episode until all its keys were released. Nothing already down is affected.

**Joystick** (`dir = (right − left, down − up)` over held keys, normalized to
length 1 unless zero):

1. No keys held -> first key pressed: `[T: n↓ at center]`, then `[M: n at
   center + dir·radius]` (two frames, same `handle` call).
2. Any later press/release while at least one key stays held: recompute
   `dir`; `[M]` to the new target if its HID position changed (opposite keys
   cancel -> target is the center; the finger stays down).
3. Last key released: `[T: n↑ at last position]`.

**Look** (at most one):

1. First `MouseMotion` with no look contact: `[T: n↓ at anchor (x, y)]`,
   offset = 0; then continue with step 2 using this event's delta.
2. `step = (dx, dy · (invert_y ? −1 : 1)) · sensitivity / 1000`;
   `o' = offset + step`.
   - If `|o'.x| ≤ reach` and `|o'.y| ≤ reach`: `offset = o'`; `[M]` if the HID
     position changed.
   - Otherwise (re-center): `[T: n↑ at last sent position]`, then
     `[T: m↓ at anchor]` (m = lowest free id, normally n again),
     `offset = step` clamped per axis to `±reach`, then `[M]` if that moves the
     HID position. `stats.look_recenters += 1`.
3. Non-finite `dx`/`dy`: the event is consumed and ignored. A zero delta with
   no look contact does not create one.
4. `release_after_ms > 0`: deadline = time of the last accepted motion +
   `release_after_ms`; `tick` at/after it emits `[T: n↑]`. With 0 the finger
   stays down until deactivate/release_all/panic/view change.

**Button, `hold`:** press -> `[T: n↓ at (x, y)]`; release -> `[T: n↑]`.

**Button, `tap`:** press -> `[T: n↓]`, deadline = press time + `TAP_HOLD_MS`;
`tick` at/after the deadline -> `[T: n↑]` whether or not the key is still
held. Releasing the key earlier does nothing. Holding the key never repeats.
A new `Press` (after a `Release`) while the tap contact is still down is
ignored (not counted as dropped).

### 5.7 `tick` and `next_deadline`

`next_deadline()` = the earliest pending deadline (tap lifts, look idle lift)
or `None`. `tick(now_ms)` performs every action whose deadline is `≤ now_ms`,
in deadline order (ties: ascending contact id); lifts that fall due in the
same `tick` call are merged into **one** frame. `tick` on an inactive engine
returns nothing.

## 6. `keymap-sim` (bin)

```
keymap-sim validate <profile.json>
keymap-sim keys
keymap-sim simulate <profile.json> <script.txt> [--rotation R0|R90|R180|R270] [--panel WxH]
```

Exit codes: 0 success; 1 invalid profile/script or I/O error (message on
stderr, prefixed `error: `; script errors include the 1-based line number);
2 usage error (usage text on stderr). Only `std`, `serde_json` and the crate
itself.

- `validate`: prints `OK: <name> (<orientation>, <n> controls)` on stdout, or
  on failure one line per problem `<path>: <message>` on stderr (all
  `problems()`, or the JSON error) and exits 1.
- `keys`: prints every keyboard name then every mouse name, one per line.
- `simulate`: defaults `--panel 828x1792`, `--rotation` R0 for a portrait
  profile and R90 for a landscape one. Time starts at 0 ms. The engine gets
  `set_view` first and is **not** activated automatically: the script must
  press the toggle key.

Script grammar (one command per line, blank lines and `# comments` ignored,
tokens separated by whitespace):

| Line | Effect |
|---|---|
| `press KEY` / `release KEY` | `handle(Press/Release)`; KEY must be a known key name else error |
| `move DX DY` | `handle(MouseMotion)` (floats) |
| `wait MS` | advance time by MS (unsigned integer): call `tick(d)` at each `next_deadline()` `d` that falls inside the interval, in order, then set time to the end |
| `focus-lost` | `deactivate()`; frame printed normally |
| `disconnect` | `deactivate()`; the frame is **not** printed as sent; prints `# disconnected: <n> contacts dropped without a lift frame` |
| `rotate R0\|R90\|R180\|R270` | `set_view(View::from_panel(w, h, r))` |

Output (stdout), one line per frame:

```
<time_ms right-aligned width 8> <T|M> <id>:<D|U>@<x>,<y>[ <id>:<D|U>@<x>,<y>]...
```

e.g. `      40 T 0:D@13107,49151 1:U@55704,52428`. Engine events and state
changes are printed as `<time> # activated`, `# deactivated`, `# panic`,
`# activate-failed: <reason>`, `# not-consumed: press KeyQ`. Last line:
`# frames=<n> dropped_presses=<n> look_recenters=<n> active=<bool> contacts_down=<n>`.

## 7. Mirrorly integration (`integration/mirrorly-keymap.patch`)

Produced by: copy the read-only clone to a scratch directory, edit there,
`git diff ac70daa -- .` with `crates/keymap` not present in the copy. The
patch touches **only** these existing files and nothing else:

`Cargo.toml`, `Cargo.lock`, `crates/input/Cargo.toml`,
`crates/input/src/lib.rs`, `crates/device/src/hid.rs`,
`crates/app/Cargo.toml`, `crates/app/src/main.rs`.

It must pass `git apply --check` on a fresh checkout of `ac70daa` (verified in
the container with git only). It is **not** compiled in the container; the CI
workflow is the first compile. Minimal diff: do not reformat or move upstream
code. LF line endings.

### 7.1 Cargo files

- `Cargo.toml` `[workspace.dependencies]`: add `keymap = { path = "crates/keymap" }`
  next to `input = …`.
- `crates/input/Cargo.toml`: `keymap.workspace = true`.
- `crates/app/Cargo.toml`: `keymap.workspace = true`.
- `Cargo.lock`: add
  ```
  [[package]]
  name = "keymap"
  version = "0.1.0"
  dependencies = [
   "serde",
   "serde_json",
  ]
  ```
  in alphabetical position, and `"keymap",` in the `dependencies` of `app`
  (after `"input"`) and of `input` (before `"winit"`).

### 7.2 `crates/input/src/lib.rs`

- `pub use keymap;`
- New variant (keep the derive list `Debug, Clone, Copy, PartialEq`):
  ```rust
  /// Multi-touch snapshot from the keymap engine (UHS touchscreen report).
  MultiTouch(keymap::Frame),
  ```
- New test `keymap_key_names_match_winit`: a macro lists every name of
  section 3 as `KeyCode::$name`, asserts `format!("{:?}", KeyCode::$name) ==
  stringify!($name)`, `keymap::keys::is_known_key(..)`, and that the number of
  listed names equals `keymap::keys::KEY_NAMES.len()`.

### 7.3 `crates/device/src/hid.rs`

- `Op::Multi(input::keymap::Frame)`.
- `expand`: `HidEvent::MultiTouch(f) => vec![(now, Op::Multi(f))]`.
- `superseded`: add
  `(HidEvent::MultiTouch(f), Some(HidEvent::MultiTouch(_))) => f.droppable()`.
  All existing arms unchanged; the `touching` flag logic is untouched.
- `route_touch`: unchanged (non-`Op::Touch` ops already pass through), so
  multi-touch always goes to UHS.
- `exec`: map contacts to `idevice::core_device::TouchscreenContact
  { identity, touching, x, y }` in a stack array and call
  `uhs.send_multitouch(&contacts[..n], None).await?`.
- Tests: move frame behind another frame is superseded; transition frame is
  not; move frame followed by a non-MultiTouch event is not; `expand` yields
  one zero-delay op.

### 7.4 `crates/app/src/main.rs`

New state on `App`: `keymap: Option<KeymapHost>` where

```rust
struct KeymapHost { path: PathBuf, engine: Option<keymap::Engine>, grabbed: bool }
```

`Args`: `#[arg(long, value_name = "FILE")] keymap: Option<std::path::PathBuf>`
("Keymap profile (JSON) for keyboard/mouse control; see ios-keymapping").
Without `--keymap`, behaviour is byte-for-byte upstream's: every new code path
is behind `self.keymap.is_some()`.

Helpers (all on `App` unless noted):

- `fn keymap_load(path) -> Result<keymap::Engine, String>`: read file,
  `Profile::from_json`, `Engine::new`. Called at startup (failure: log
  `tracing::error!` + toast; app keeps running) and again on every activation
  attempt so editor changes apply without a restart (on failure keep the old
  engine, toast the error, do not activate).
- `fn keymap_view(&self) -> keymap::View`: from `shared.view` — `screen`
  (portrait panel px) and `rotation` mapped variant-by-variant from
  `input::geom::Rotation` to `keymap::Rotation`.
- `fn keymap_sync_view(&mut self)`: `engine.set_view(..)`, send frames; if the
  engine was active and no longer is -> `keymap_after_stop()`.
- `fn keymap_send(&self, frames)`: `self.send(HidEvent::MultiTouch(f))` each.
- `fn keymap_stop(&mut self)`: `engine.deactivate()`, send the frame,
  `keymap_after_stop()`.
- `fn keymap_after_stop(&mut self)`: ungrab (`CursorGrabMode::None`), show the
  cursor, `set_title()`, `update_hover()`.
- `fn keymap_active(&self) -> bool`.
- `fn keymap_input(&mut self, input, pressed_toggle: bool) -> bool` (returns
  `consumed`): gate, feed `engine.handle(input, now_us() / 1000)`, send
  frames, react to the event:
  `Activated` -> `self.release_all()` was already done by the gate; if
  `profile.has_look()`: `set_cursor_grab(CursorGrabMode::Confined)` (ignore an
  error, log it) + `set_cursor_visible(false)`; hide the bar
  (`set_bar(false)`); title; toast `Keymap on: <toggle> to leave, <panic> to stop`.
  `Deactivated` / `Panic` -> `keymap_after_stop()` (+ toast on Panic).
  `ActivateFailed(msg)` -> toast.
- Activation gate (a `Press` of the toggle key while inactive, non-repeat):
  refuse with a toast, consume the key, and do not call the engine when
  (a) not connected (`self.link.is_none()` or chrome `disconnected`),
  (b) `self.link == Some("Wi-Fi")` and env `MIRROR_KEYMAP_WIFI` is unset,
  (c) `self.display.is_some()`, or (d) `Instant::now() < self.scroll_busy_until`.
  Otherwise: reload the profile, `self.release_all()` (lifts the mouse finger
  and phone keys), `keymap_sync_view()`, then feed the press.
  If no engine is loaded (startup load failed), a press of `Backquote` retries
  the load.
- `set_title()`: append ` · Keymap ON` when active, ` · Keymap (<toggle key>)`
  when loaded but inactive.

Event wiring:

| Upstream place | Change |
|---|---|
| `window_event` `Focused(false)` | `keymap_stop()` before the existing `release_all()` |
| `user_event` `DeviceEvent::Disconnected` | `keymap_stop()` (the frame cannot reach the phone; engine state must still reset) |
| `user_event` `DeviceEvent::Connected` | after `v.screen` is updated: `keymap_sync_view()` |
| `rotate()` | after the existing body: `keymap_sync_view()` |
| `quit()` | `keymap_stop()` first |
| `KeyboardInput` | at the top, if a host exists and `!event.repeat`: `consumed = keymap_input(Press/Release(&format!("{code:?}")))`; if consumed `return`. If repeat and keymap active: `return`. If active and not consumed: evaluate only the F-key shortcuts (`F1`-`F7`, `F10`, `F11`, `F12`; **not** F9, not Ctrl+digit) and `return` — nothing is typed on the phone while active. |
| `MouseInput` | if active: map `Left/Right/Middle/Back/Forward` to `MouseLeft/…` (`Other(_)` ignored), feed the engine, `return` (never reaches the gesture path, Home on right-click, pickers). |
| `MouseWheel` | if active: `return` |
| `CursorMoved` | if active: update `self.cursor` only, `return` |
| `CursorLeft` / `CursorEntered` | if active: skip `update_hover()` |
| new `fn device_event(&mut self, _el, _id, event: winit::event::DeviceEvent)` | on `MouseMotion { delta }` and active: `keymap_input(MouseMotion { dx: delta.0, dy: delta.1 })` |
| `about_to_wait` | if a host exists: `engine.tick(now_us()/1000)` -> send frames; add `engine.next_deadline()` converted to an `Instant` (`epoch().instant + Duration::from_millis(d)`) to the array whose minimum picks `ControlFlow::WaitUntil` |
| `start_probe()` | return early with a log line while the keymap is active |

Winit facts relied on (0.30.13, Windows): `CursorGrabMode::Confined` is
supported, `Locked` is not; raw `DeviceEvent::MouseMotion` is delivered while
the window is focused (default `DeviceEvents::WhenFocused`). Focus loss always
deactivates, so "active" implies "focused".

Known, documented limits of the integration: USB only (deviation 1); iOS
behaviour on a dropped connection with fingers down is unknown; the toggle key
(default `` ` ``) can no longer be typed to the phone while `--keymap` is
given; a profile may bind F-keys, which then shadow Mirrorly's shortcuts
while active.

## 8. `integration/apply-to-mirrorly.ps1`

Windows PowerShell 5.1 compatible, `$ErrorActionPreference = 'Stop'`, checks
`$LASTEXITCODE` after every native command.

```
apply-to-mirrorly.ps1 [-Dest <dir>] [-Ref <sha>] [-Build] [-Package] [-Force]
```

- Defaults: `-Dest` = `<ios-keymapping>\build\mirrorly`, `-Ref` =
  `ac70daa58ee5f24a067e80b6d0ffbcec4c735f1e`.
- Steps: require `git`; refuse a non-empty `-Dest` unless `-Force` (then
  delete it); `git init`, `git remote add origin
  https://github.com/AnasSAV/mirrorly.git`, `git fetch --depth 1 origin <Ref>`,
  `git checkout --detach FETCH_HEAD`; verify `git rev-parse HEAD` equals
  `-Ref`; copy `crates\keymap` (without `target`) to `<Dest>\crates\keymap`;
  `git apply --check` then `git apply` the patch; print next steps.
- `-Build`: require `cargo`; run `packaging\get-ffmpeg.ps1` if
  `third_party\ffmpeg` is missing; `cargo test -p keymap -p input`; `cargo
  build --release -p app` (inside vcvars64 found like upstream's
  `build-release.ps1` does, unless `$env:VCINSTALLDIR` is set); stage
  `<ios-keymapping>\dist\Mirrorly-keymap\` with `Mirrorly.exe`, the
  `avcodec-*`, `avutil-*`, `swresample-*` DLLs from `target\release`,
  upstream `LICENSE` (as `LICENSE-Mirrorly.txt`), `packaging\licenses\*.txt`,
  our `LICENSE`, `NOTICE`, `README.md`, `profiles\`, `editor\index.html`.
- `-Package`: additionally run upstream `packaging\build-release.ps1`.

The script cannot be executed in the Linux container (no PowerShell): it is
reviewed by reading and first run by CI. Say so in `BAO_CAO.md`.

## 9. CI: `.github/workflows/ios-keymapping.yml`

- `on`: `workflow_dispatch`; `push` to branch `claude/new-session-l9i0tr` with
  `paths: ["ios-keymapping/**", ".github/workflows/ios-keymapping.yml"]`.
- `permissions: contents: read`; `concurrency` per ref, cancel in progress.
- Job `keymap` (`ubuntu-latest`): `cargo test` and `cargo clippy --all-targets
  -- -D warnings` in `ios-keymapping`; `cargo run --bin keymap-sim -- validate`
  on both sample profiles.
- Job `mirrorly` (`windows-latest`, `needs: keymap`, timeout 60 min), modeled
  on upstream `ci.yml`/`package.yml` (same action versions: `actions/checkout@v7`,
  `ilammy/msvc-dev-cmd@v1`, `actions/cache@v6`, `Swatinem/rust-cache@v2`,
  `actions/upload-artifact@v7`):
  1. checkout; Rust stable (minimal + clippy).
  2. `cargo test` in `ios-keymapping` (Windows run of our crate).
  3. `./ios-keymapping/integration/apply-to-mirrorly.ps1 -Dest
     "$env:GITHUB_WORKSPACE\_mirrorly"` (inside the workspace so `hashFiles`
     works).
  4. MSVC env; FFmpeg cache keyed on `hashFiles('_mirrorly/packaging/ffmpeg.txt')`,
     path `_mirrorly/third_party/ffmpeg`; `get-ffmpeg.ps1` on a miss.
  5. rust-cache with `workspaces: _mirrorly`.
  6. In `_mirrorly`, with `third_party\ffmpeg\bin` on `PATH`:
     `cargo test -p keymap -p input -p device`, then
     `cargo build --release -p app`, then `git diff --stat -- Cargo.lock`
     (informational), then `cargo clippy -p input -p device -p app -- -D
     warnings` with `continue-on-error: true`.
  7. Stage as section 8's `-Build` staging (reuse by calling the script's
     staging logic or duplicating it) plus `keymap-sim.exe`; upload artifact
     `mirrorly-keymap-${{ github.sha }}` with `if-no-files-found: error`.

Until this workflow has actually run green, the Windows build is
**unverified** and every doc says so.

## 10. Editor (`editor/index.html`)

One self-contained HTML file: inline CSS and JS, no external requests, no
frameworks, works from `file://` on Windows (Edge/Chrome). Vietnamese UI
(`<html lang="vi">`). Must not persist anything outside the page except an
optional `localStorage` draft.

Functional requirements:

1. **Canvas/stage** keeps the picture aspect. Background screenshot via file
   picker and drag-and-drop (image files only, read with `FileReader`/object
   URL; Mirrorly's F6 saves to `Pictures\Mirrorly`). Orientation is taken from
   the image aspect (w > h -> landscape) and can be toggled manually; without
   an image a default 9:19.5 placeholder is used. Changing orientation keeps
   control coordinates (fractions) and warns that positions should be checked.
2. **Controls:** add joystick / look / button; drag to move (clamped to
   [0,1]); joystick radius and look reach are resized with a handle; shapes
   are drawn in short-side units exactly as the engine interprets them
   (joystick = circle of `radius`, look = square of half-side `reach`
   around the anchor); select, delete, duplicate (duplicate clears key
   bindings, since keys must be unique). Adding a second look is disabled.
3. **Side panel** for the selection: label, keys, radius, sensitivity,
   invert_y, reach, release_after_ms, hold/tap; profile fields: name, app,
   orientation, toggle_key, panic_key.
4. **Key capture:** "Gán phím" then press a key or click a mouse button.
   `KeyboardEvent.code` -> name unchanged, except `MetaLeft/MetaRight` (and
   legacy `OSLeft/OSRight`) -> `SuperLeft/SuperRight`; mouse `button`
   0/1/2/3/4 -> `MouseLeft/MouseMiddle/MouseRight/MouseBack/MouseForward`;
   `preventDefault()` + `stopPropagation()` during capture (Tab, Space, F-keys,
   context menu must not act); codes outside section 3's list are refused with
   a message; `Escape` cancels capture only when pressed twice or via a
   "Hủy" button (Escape itself is bindable). Mouse names are not offered for
   toggle/panic.
5. **Validation:** the same rules, order and `path` strings as section 4.3,
   shown inline next to the field and in a summary list; export is disabled
   (or clearly marked invalid) while problems exist.
6. **Import/export:** import by file picker, drag-drop of `.json`, or paste;
   import runs validation and reports problems without discarding the data;
   export = download `<name>.json` and copy to clipboard; output is the exact
   schema of section 4.1 (no extra fields, numbers rounded to 4 decimals,
   2-space indent). Exported files must pass `keymap-sim validate`.
7. A visible scope notice (Vietnamese): one key = one touch, no macros, check
   the game's rules, test with a touch-test app.
8. Accessibility basics: all inputs labelled, keyboard reachable, visible
   focus, contrast AA.

`editor/tests/editor.test.mjs` (Node, `require('playwright')`/import, Chromium
headless, `file://` URL): builds a profile through the UI (add each control
type, drag, capture keys incl. Tab/Space and a mouse button), exports, and
runs `keymap-sim validate` on the result; imports both sample profiles and
every `tests/data/valid/*.json` and re-exports them (must validate and be
semantically equal); imports every `tests/data/invalid/*.json` and asserts the
first reported `path` equals `expected.json`; asserts zero network requests
and zero console errors. Exit non-zero on failure.

## 11. Required tests (Rust)

Unit tests live next to the code; all are deterministic.

**keys:** count = 116, no duplicates, mouse names disjoint, defaults known.

**profile:** minimal profile gets every default; full round trip
`from_json(to_json_pretty(p)) == p`; BOM accepted; one test per rule of 4.1 /
4.3 asserting the exact `path` (wrong version, x/y out of range, NaN-free
bounds at 0 and 1 accepted, radius 0 / 0.5 / >0.5, reach, sensitivity,
release_after_ms > 60000, unknown key, mouse toggle key, toggle == panic,
duplicate across controls, duplicate with toggle, second look, unknown field,
unknown type, bad JSON, > 64 controls, 6+ controls valid); `problems()`
returns several findings in the specified order.

**Shared vectors (`tests/vectors.rs`):** every `tests/data/valid/*.json`
parses; every `tests/data/invalid/*.json` fails with the `path` listed in
`tests/data/invalid/expected.json` (`{"file.json": "controls[1].key" | "<json>"}`).
At least 6 valid and 15 invalid vectors.

**Samples (`tests/samples.rs`):** `../../profiles/*.json` validate, and their
orientation matches the file name; skipped (passing) when that directory does
not exist (the crate copied into Mirrorly).

**engine:** activation errors (no view, mismatch); toggle on/off; inactive
input not consumed; unbound key not consumed while active; joystick: W ->
down+move with exact HID coordinates for panel 828x1792 R0, diagonal length
equals radius, opposite keys return to center without lifting, last release
lifts, coordinates under R90/R180/R270 land where `picture_to_panel` says;
look: first motion down+move, sensitivity scaling, invert_y, re-center frame
order and counter, idle release via `tick`/`next_deadline`, zero/NaN motion;
button hold; tap: lift at exactly +40 ms via `tick`, early release changes
nothing, held key does not repeat, `handle` at a later time lifts first
(step 0); two taps due together merge into one frame; slot exhaustion with 6
buttons (6th ignored + counted, its release harmless, others unaffected); id
reuse after lift takes the lowest free id; panic from active and inactive;
`release_all` keeps active and ignores stale releases; `deactivate`
idempotent; `set_view` same view = no frames, rotated same orientation = lift
only, other orientation = lift + inactive; repeat `Press` ignored; time going
backwards.

**Fuzz (`engine::tests::fuzz_invariants`):** tiny in-file LCG (no deps), at
least 20 seeds x 5000 random inputs (press/release of bound, unbound, toggle
and panic keys, motions, time advances with `tick`, `release_all`,
`set_view` rotations, `deactivate`) over a profile with joystick + look + 6
buttons (hold and tap); a reference model replays the emitted frames and
asserts I1-I8 after every call, and that after a final `release_all` nothing
is down.

**CLI (`tests/sim_cli.rs`):** via `env!("CARGO_BIN_EXE_keymap-sim")`:
`validate` ok / fail exit codes and stderr paths; `keys` line count; a
`simulate` golden script (toggle, WASD, tap + wait, move, focus-lost) compared
line by line; bad script line -> exit 1 with the line number; no args -> exit 2.

Commands that must be run in the container and reported verbatim in
`BAO_CAO.md`: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo build --release` (linux), `cargo build --release --target
x86_64-pc-windows-gnu` (keymap-sim.exe only), the editor Playwright test,
`keymap-sim validate` on both sample profiles, and `git apply --check` of the
patch on a fresh copy of the upstream clone. **Never** run cargo or scripts
inside the upstream tree or its copies.

## 12. Sample profiles

`profiles/touch-test-portrait.json`, `profiles/touch-test-landscape.json`:
valid v1 profiles for a generic multi-touch test app (`"app": "multi-touch
test app"`), each with one joystick (WASD), one look, and 3-4 buttons mixing
`hold` and `tap` (e.g. `Space`, `KeyJ`, `KeyK`, `MouseLeft`), default toggle
and panic keys, controls kept ≥ 0.06 away from the edges (system gestures).
No profile for any specific game ships.

## 13. Docs

- `README.md` (Vietnamese): what it is and is not (section 1, in plain
  words); safety section incl. Free Fire/Garena from the handoff (no guarantee
  keymapping is allowed or how the device is classified; check the official
  rules first; a secondary account does not remove the risk; the tool must not
  be used to dodge device classification or gain an unfair advantage);
  architecture (keyboard/mouse -> engine -> `HidEvent::MultiTouch` -> UHS
  report -> iPhone; video path is upstream's); prerequisites (Windows 10/11,
  iOS 27+, Apple Devices app, trust + Developer Mode, Rust MSVC, VS 2022 Build
  Tools C++, LLVM, FFmpeg via `get-ffmpeg.ps1`); apply/build on Windows; usage
  (`Mirrorly.exe --keymap <file>`, toggle, panic, F12 rotation, USB only);
  profile format; editor; PC test checklist with a multi-touch test app (each
  contact count 1-5, joystick directions, look drag and re-center, tap vs
  hold, panic, focus loss, unplug USB, rotation, >5 keys); latency measurement
  method (upstream F9 probe and F10 overlay / `--timings-csv`, report median,
  p95, FPS, machine config, method; high-speed camera for true
  input-to-photon; upstream's own figures quoted as upstream's, not ours);
  limitations (unverified multi-touch, USB only, HID needs the video stream so
  no control-only mode, no hardware testing done, toggle key not typeable).
- `docs/BAO_CAO.md` (Vietnamese): exactly what was run in the container with
  results; what is unverified (everything needing Windows, a compile of the
  patched Mirrorly, or an iPhone); known issues; reproduction steps.
- `LICENSE`: MIT for this project. `NOTICE`: Mirrorly (c) 2026 AnasSAV, MIT;
  idevice by Jackson Coxson (jkcoxson), MIT; statement that the patch modifies
  Mirrorly and that binaries built from it bundle FFmpeg (LGPL) DLLs under
  upstream's terms; not affiliated with Apple or with Mirrorly's author.

## 14. Definition of done / review checklist

- Section 11's commands all pass in the container and their real output is in
  `BAO_CAO.md`; nothing is reported that was not run.
- Public API matches sections 3-5 exactly (names, signatures, derives).
- Patch touches only the seven files of section 7, applies cleanly, and every
  new app code path is gated on `--keymap`.
- No feature violating section 1 exists anywhere (code, editor, docs, samples).
- Every claim about phones, Windows builds or latency is marked unverified
  unless backed by a run that is cited.
