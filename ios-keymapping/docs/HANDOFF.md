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
- In-app menu (UI_SPEC WP1-WP4, pure model in `crates/keymap/src/menu/`, commit
  c60a1f2): WP1-WP3 approved in review; WP4 NOT approved yet. `cargo test`:
  307 + 17 + 8 + 2 + 1 pass on Linux. App-side menu (WP5+) is not started.
- WP4 open findings to fix first:
  1. (major) `editor.rs`: prompts (save/revert prompt, capture banner, chips)
     live in the scrolling middle of the dock; when the dock is scrolled they
     are dropped. Pin them outside the scroll region or reset scroll on open.
  2. (major) `editor.rs`: canvas banners are painted under the dock and stay
     hit-testable through it. Lay them out in the free canvas area only and
     stop hit-testing through the dock.
  3. Visual (from rendering the golden SVGs): the "Chưa có hình điện thoại…"
     banner cuts across controls in guide_1..6; guide step 3 highlights the look
     zone with a huge circle (should be a rectangle); guide step 4's ring hides
     the key cap it asks the user to click; in Test the finger dot hides the
     "W" cap; sensitivity slider track is only ~45 px; the toggle key shows as a
     bare "`" (show "` (dưới Esc)"); label "Phím dừng khẩn" should read
     "Phím dừng khẩn cấp"; the dock hides overflow with only a tiny ▾; guide
     step 3 says "chỗ không có nút" but the shooter template puts a button in
     the look zone.
  Golden SVGs: `crates/keymap/tests/data/menu/golden/`. Render to PNG with any
  browser to look at them; show the user the main screens after fixing.
- User decision 2026-11-26: the user has a 60% keyboard (no F row). Sample
  profiles now use `Escape` as panic key (commit 8c8291a). Apply the same to
  the menu: templates/new profiles default to panic `Escape`, and nothing in
  the menu may require an F key (rotate via the bar button, menu via Ctrl+K).
  Remember Mirrorly's own F-key shortcuts need Fn on this keyboard.
- Working locally is now the plan: build with
  `integration\apply-to-mirrorly.ps1 -Build` (needs Rust MSVC, VS Build Tools
  C++ workload, LLVM, Git; FFmpeg is fetched by the script) and test with the
  phone, instead of waiting for CI. The cloud session has stopped launching
  work on this branch, so there is no other writer.

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
