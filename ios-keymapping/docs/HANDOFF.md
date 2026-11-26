# Handoff: iOS keymapping (for a new Claude Code session on the user's PC)

Written 2026-11-26 by the cloud session that built this. Read this first, then
`README.md` (Vietnamese, user guide), `docs/SPEC.md` (crate spec),
`docs/UI_SPEC.md` (in-app menu spec) and `docs/BAO_CAO.md` (test report).

## The user
- Vietnamese, no coding knowledge. Answer in simple Vietnamese, no jargon.
- Hardware: Dell G15 5530 (Windows), iPhone 17 Pro Max (iOS 27.2.1, USB-C),
  Alienware monitor, Elgato HD60 (exact model unknown; not needed).
- Goal: play games (mainly Free Fire) on the real iPhone with keyboard+mouse.
- Hard limits (keep them): no macros, no turbo/auto-fire/auto-aim/recoil
  control, one physical input = one touch, no detection evasion, no device
  spoofing. Garena's rules may forbid this; say so, promise nothing.
- Quota matters: prefer Sonnet for code, Opus for review; few review rounds.

## Where things are
- Repo `Mgd152/mgd-web`, branch `claude/new-session-l9i0tr`, folder
  `ios-keymapping/` (the rest of the repo is an unrelated website).
- `crates/keymap`: pure Rust logic (profile JSON v1, engine: joystick, look,
  hold/tap buttons, max 5 contacts, toggle `Backquote`, panic `F8`),
  `keymap-sim` CLI. `crates/keymap/src/menu/`: in-app menu model (WIP).
- `editor/index.html`: offline browser editor that exports profile JSON.
- `integration/mirrorly-keymap.patch` + `apply-to-mirrorly.ps1`: patch on
  Mirrorly (github.com/AnasSAV/mirrorly, MIT) at commit ac70daa.
- `.github/workflows/ios-keymapping.yml` (repo root): builds the patched
  Mirrorly on windows-latest and uploads a portable ZIP.

## Status
- CI run #4 green: https://github.com/Mgd152/mgd-web/actions/runs/45734325079
  (artifact `mirrorly-keymap-abdb776…`: Mirrorly.exe, FFmpeg DLLs,
  keymap-sim.exe, profiles\, editor\, `Chay-keymap-doc.bat`,
  `Chay-keymap-ngang.bat`, `Keo-ho-so-vao-day.bat`).
- On the user's PC: Apple Devices (Microsoft Store) installed; the iPhone is
  now recognised over USB. 3uTools is installed too; close it before Mirrorly.
- NOT yet verified on hardware: Mirrorly with the patch running, multi-touch
  (`send_multitouch`, never tested by anyone), joystick/look/buttons, F8,
  focus loss, unplug, latency.
- In-app menu (UI_SPEC WP1-WP4, pure model): WP1 reviewed and approved; WP2-WP4
  were in progress in a cloud workflow. They may be unfinished or unreviewed.
  Check `git log` (WIP commits are tagged `[skip ci]`) and run
  `cargo test` in `ios-keymapping/`. App-side menu (WP5+) is not started.

## Facts that matter
- Keymap works over USB only: over Wi-Fi iOS ignores the touch reports
  (Mirrorly FEASIBILITY.md). So HDMI-out from the iPhone + Wi-Fi control does
  not work.
- iOS accepts injected touch only while the display stream runs, so Mirrorly
  must stay open (it can be small); no "control only" mode.
- Mirrorly's F9 latency probe sends automated taps: the patch disables it while
  the keymap is on. Tell the user never to press F9 in a game.
- Upstream measured ~87 ms tap-to-frame (phone side); the PC adds ~1 ms.
- Running `Mirrorly.exe --console` shows logs, useful when something fails.

## Next steps
1. Hardware test with a multi-touch test app (not a game), following README
   section 9: count simultaneous contacts with W + Space + mouse look held,
   check the look drag for stutter, F8, Alt+Tab, unplug USB.
2. Fix what the test shows (Sonnet codes, review by Opus), push, let CI build.
3. Finish menu WP2-WP4, then WP5+ (app side) per UI_SPEC section 9.
