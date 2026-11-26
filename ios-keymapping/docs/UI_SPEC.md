# Keymapping menu for Mirrorly — UI spec (v1)

Status: authoritative for the in-app keymapping menu. It builds on
`docs/SPEC.md` (engine, profile v1, base integration) and never rewrites it;
where this document needs SPEC.md to change, the change is listed in section
0 and must be approved by the lead before the affected work package starts.

Language rules: this spec, code, comments and identifiers are English. Every
string the user can see in the keymap UI is Vietnamese and is given here with
full diacritics; implementers copy the strings verbatim into `menu/msg.rs`.

Who it is for: one Vietnamese player with no coding knowledge, iPhone 17 Pro
Max mirrored by Mirrorly on a Dell G15 + Alienware monitor, mainly Free Fire.
They must get from first launch to playing without seeing JSON, a command
line, or an unexplained word.

Verification status of this document: every upstream file, function and line
reference below was read in the Mirrorly clone at commit `ac70daa`. Nothing
was compiled or run. Everything on the Direct2D/Windows side, on an iPhone,
or in a game is **unverified** until Windows CI and a hardware session.

Design origin: the "novice" draft is the product design (valid by
construction, dry-run Test, Hub with one big Play button, guided first run).
The "pro" draft supplies the engineering skeleton (present-thread hand-off,
`EditDoc`, `Fs`/`MemFs`, exhaustive `Msg`, `TextEdit`, clipboard share,
live sensitivity). The "emulator" draft supplies the parity checklist, SVG
golden scenes, the "ngón N/5" counter and the v1.1 engine ideas.

## 0. Changes this spec needs in SPEC.md (need the lead's sign-off)

| # | SPEC.md text | Needed change | Why |
|---|---|---|---|
| A1 | Header: "strings added inside the Mirrorly app stay English" | Strings of the keymap UI are Vietnamese (`Lang::Vi` default, `Lang::En` complete). Upstream's own strings stay as they are. | The user cannot read English UI. |
| A2 | 7.4: "Without `--keymap`, behaviour is byte-for-byte upstream's" | The keymap host is always built. `--no-keymap` gives byte-for-byte upstream. `--keymap FILE` keeps the exact SPEC 7.4 behaviour (developer/CI path) and disables the menu. | The user must never type a CLI flag. |
| A3 | 7: "The patch touches only these seven files" | Adds `crates/app/src/{layout.rs, bar.rs, ui.rs, state.rs, present.rs, settings.rs, theme.rs}` and one new file `crates/app/src/keymap_ui.rs`. | The menu is drawn by the app. |
| A4 | 2: crate layout and `lib.rs` | Adds `pub mod menu;` and the `src/menu/` tree. No existing item changes. | Pure editor model. |
| A5 | 5.1 `Engine` API | One additive method `set_look_tuning` (section 5.1). Later, optional: schema v2 (5.3). | Sensitivity tuning while playing. |
| A6 | 7.4 toast texts (`Keymap on: ...`) and `set_title()` suffixes | Texts come from `Msg` in Vietnamese when the menu is in charge (not under `--keymap FILE`). | Same as A1. |

**Approved by the lead 2026-11-26:**

- A1-A6 are approved as written. The `lib.rs` line `pub mod menu;` is allowed
  (the SPEC.md team is done with `lib.rs`).
- `recenter: "handoff"` (5.3) is deferred to WP10 and not decided now.
  Templates stay generic by genre (no game-specific profile).
- The keymap stays refused on a Wi-Fi link (SPEC.md: iOS ignores
  universalhidservice touch reports over Wi-Fi); `PlayRefusal::Wifi` stays.

Not needed: any change to profile v1 for the v1 menu; any game-specific
profile (SPEC 1 and 12 stay intact: templates are generic layouts by genre,
see 2.2).

Until A1-A4 are approved, work packages WP1-WP4 (pure crate, section 9) can
proceed, because they only add files under `src/menu/` and tests.

## 1. Goals and non-goals

### 1.1 Goals

1. An in-app menu, like the key-mapping tool of BlueStacks / LDPlayer / MuMu:
   controls are dragged onto the live phone picture, a key is assigned by
   clicking a control and pressing the key, profiles are picked from a list,
   one big "CHƠI" button starts.
2. **The user can never get stuck.** The draft profile is valid at every
   moment (new controls get a free key automatically; a taken key can only be
   swapped or the capture cancelled; values are clamped before they enter the
   draft). There is no "saved but will not start" state.
3. **Everything can be undone:** every edit, delete, template apply, profile
   delete (trash plus an 8 s "Hoàn tác" toast).
4. **Safe to explore:** Hub, Edit and Test never send anything to the phone.
   Only Play does, and only after the one-time notice is accepted.
5. **Every refusal says why, in plain Vietnamese, with a fix button** when a
   fix exists.
6. All editor logic lives in `crates/keymap` as a pure model tested on Linux.
   The Direct2D code draws a display list and forwards events, nothing else.
7. Play-mode latency equals upstream's whenever the overlay is hidden, and any
   overlay cost is measured on Windows, never assumed.

### 1.2 Non-goals for v1

- Power-user surface: sticky tools, multi-select, marquee, typed numeric
  fields, single-letter tool hotkeys.
- Wheel bindings, toggle (latched) buttons, hold-to-free-cursor key.
- File dialogs, opening web links, cloud profiles, per-app auto-switching
  (the app cannot know which iOS app is in front).
- Auto-detecting game HUD positions (we never read the phone's screen to react).
- Schema v2 features (5.3) are specified but scheduled last (WP10).

### 1.3 Hard limits (from SPEC 1; reviewers reject anything that crosses them)

- One physical key/button press = one touch action. No macros, sequences,
  recorded input, repeat/turbo, auto-fire, auto-aim, recoil control, timed
  assistance. The only timed behaviours are SPEC's fixed 40 ms tap lift and
  the optional idle lift of the look finger.
- No latched "toggle" button mode (a touch held with no physical hold).
- No detection evasion, device spoofing, randomising or "humanising".
- No control type, field or setting exists in the UI that could express any
  of the above. The Hub footer states: `1 phím = 1 lần chạm. Không macro.`
- No claim that the tool is allowed by Garena or safe for an account; the
  one-time notice (3.6) is mandatory before the first Play and its wording
  stays hedged because we have not verified Garena's current rules.
- No latency number is claimed until measured on hardware (SPEC 0.13).

### 1.4 Emulator parity checklist

| Emulator convention (public UX) | Here | Version |
|---|---|---|
| Toolbar button opens the key-mapping tool | Bar button "Bàn phím" + gear menu + `Ctrl+K` | v1 |
| Drag control types onto the game picture | Palette: Cần di chuyển / Xoay camera / Nút | v1 |
| Click a control, press the key | Key capture on the key cap | v1 |
| Key caps drawn over the game | Edit canvas; Play overlay (Tự ẩn / Luôn hiện / Tắt) | v1 |
| Scheme list, new / duplicate / rename / delete | Hub | v1 |
| Ready-made layout to start from | Generic templates by genre (2.2) | v1 |
| Mouse sensitivity slider | Độ nhạy chuột + Chậm/Vừa/Nhanh + `[` `]` while playing | v1 |
| Preview / test | Test mode (dry run, nothing sent) | v1 |
| Separate vertical sensitivity (Y ratio) | `look.y_ratio` | v1.1 (5.3) |
| Free look key | `freelook` control | v1.1 (5.3) |
| Suspend key that frees the cursor | deferred | later |
| Wheel = weapon swap | deferred (changes `keys.rs` contract) | later |
| Macros, burst, recoil scripts, combos | never | – |

## 2. User flows

Modes (owned by the pure `Shell`, 6.2):

```
            bar button / gear menu / Ctrl+K
  Off ------------------------------------------> Hub
   ^  \                                          |  [Sửa]        |  [CHƠI]
   |   \  `  (toggle key, profile chosen)        v               v
   |    +------------------------------------> Play  <-----------+
   |         `  /  F8  /  focus lost  /  unplug    (real touches)
   +<---------------------------------------------+
   |
   |   [Xong] / Esc               `  or [Thử]
   +<-------------- Edit <----------------------> Test (dry run)
                     |  Guide card on top
                     +-- Capture (listening for a key)
```

- **Off**: Mirrorly behaves as upstream, plus the bar button and the menu
  entries. `` ` `` starts Play when a profile is chosen.
- **Hub / Edit / Test**: no frame ever reaches the phone; mouse and keyboard
  go to the menu (exceptions: section 4).
- **Play**: SPEC 7.4 exactly. The toggle key has one meaning: Play -> Off.
  (Returning to Off, not to the Hub, keeps one rule: `` ` `` flips between
  "phone controlled by keymap" and "normal Mirrorly".)
- F8 (panic) works in every mode. Focus loss stops Play and Test, cancels a
  capture, and leaves Edit as it is.

### 2.1 First run

1. The user opens Mirrorly. The hover bar has a new button `Bàn phím`
   (tooltip `Chơi bằng bàn phím và chuột`). After the first successful
   connection a toast appears once: `Mới: chơi bằng bàn phím và chuột. Bấm nút Bàn phím trên thanh công cụ.`
2. Clicking it opens the Hub. With no profiles, the Hub shows the empty state
   (3.5) with one primary button `Tạo hồ sơ đầu tiên`.
3. Template picker (2.2). Choosing a template creates the profile and enters
   Edit with the 6-step guide (3.7).
4. The last guide step runs Test, then `Lưu & Chơi`.
5. The first `CHƠI` shows the notice (3.6). After acceptance Play starts, or
   a refusal card explains why not (3.8).

### 2.2 Create a profile

`+ Hồ sơ mới` in the Hub opens the template picker:

| Template (label) | Contents |
|---|---|
| `Bắn súng – màn hình ngang` (marked `đề nghị`) | joystick WASD, look, buttons: Bắn (MouseLeft, hold), Ngắm (MouseRight, tap), Nhảy (Space, tap), Ngồi (KeyC, tap), Nạp đạn (KeyR, tap), Súng 1 / Súng 2 (Digit1/Digit2, tap), Hồi máu (KeyF, tap) |
| `Cơ bản – màn hình ngang` | joystick WASD, look, 3 buttons (Space, KeyJ, KeyK) |
| `Cơ bản – màn hình dọc` | same, portrait positions |
| `Trống` | no controls |

Rules:

- Templates are generic layouts by genre. They carry no game name and
  `app = ""`; this keeps SPEC 1 and 12 intact. Positions are our guess of a
  typical HUD and are **not** a match for any game; the picker says so:
  `Vị trí chỉ là tạm. Bạn sẽ kéo từng nút cho trùng với nút trong game.`
- Coordinates of `Bắn súng – màn hình ngang`: joystick (0.15, 0.68, r 0.11);
  look anchor (0.68, 0.40) reach 0.30 sensitivity 1.0; Bắn (0.87, 0.60);
  Ngắm (0.78, 0.44); Nhảy (0.93, 0.76); Ngồi (0.84, 0.82); Nạp đạn
  (0.72, 0.84); Súng 1 (0.50, 0.90); Súng 2 (0.58, 0.90); Hồi máu
  (0.40, 0.90). All controls of all templates stay >= 0.06 from every edge.
- The picker preselects the template whose orientation equals the current
  picture rotation.
- The new profile is named `Hồ sơ 1`, `Hồ sơ 2`, ... (first free number);
  renaming is optional (typing is never required to finish a flow).

### 2.3 Edit on the live screen

1. Hub -> `Sửa` (or gear menu -> `Sửa nút…`). The picture stays live.
2. Add: drag a palette item onto the picture, or click the item then click
   the picture. A new button immediately listens for its key (capture) and,
   if the user cancels, keeps the free key that was auto-assigned.
3. Move: drag the control. Resize joystick/look: drag the round handle, or
   mouse wheel over the selected control.
4. The dock (3.2) shows the properties of the selection.
5. Every change is written to a crash-safe draft. `Xong` saves the profile
   and returns to the Hub. `Huỷ thay đổi` restores the last saved version
   after a confirmation (and is itself undoable until Edit is left).
6. `Giữ H` hides the dock and all markers to see the bare game HUD.

### 2.4 Bind keys

1. Click the key cap on a control (or on the dock), or select the control and
   press Enter.
2. The cap shows `Bấm phím hoặc nút chuột…` and a `Huỷ` chip.
3. Press the key / mouse button. Outcomes:
   - free key: bound, done;
   - key used elsewhere: `Phím R đang dùng cho “Nạp đạn”.` with exactly two
     choices, `Đổi chỗ` and `Huỷ`. Swap exchanges the two keys, so the draft
     stays valid;
   - unsupported key: `Phím này chưa dùng được. Hãy chọn phím khác.` and the
     capture keeps listening;
   - a mouse button for the on/off or stop key: `Phím bật/tắt và phím dừng phải là phím trên bàn phím.`
4. Esc: first press shows `Bấm Esc lần nữa để gán phím Esc, hoặc bấm Huỷ.`;
   a second Esc within 1.5 s binds Escape. There is no timeout otherwise.
5. Non-blocking advice appears under the cap after binding (6.9), e.g.
   `F4 cũng là phím tắt âm lượng của Mirrorly; khi đang chơi nó sẽ là phím game.`

### 2.5 Tune sensitivity

- In Edit: select the camera control. Slider `Độ nhạy chuột` (logarithmic,
  0.1 to 10) with three presets `Chậm` (0.6) `Vừa` (1.0) `Nhanh` (1.8). These
  are starting points, not recommended values; the right value depends on the
  mouse DPI and the game's own setting, which the dock states:
  `Còn tuỳ vào chuột và cài đặt trong game. Hãy thử rồi chỉnh.`
- In Test: move the mouse and watch the blue finger travel inside the camera
  box; nothing is sent.
- While playing: `[` lowers and `]` raises the sensitivity by 10 % per press
  (clamped 0.05 to 100). The existing toast shows `Độ nhạy chuột: 1,21`. No
  finger is lifted (5.1). On leaving Play the new value is saved to the
  profile (with a backup) and the next Hub visit shows
  `Độ nhạy đã đổi 1,00 → 1,21.` with `Trả về như cũ`.
- If the profile itself binds `[` or `]`, the profile wins and the Hub shows
  `Phím [ ] đang dùng trong hồ sơ nên không chỉnh độ nhạy khi chơi được.`

### 2.6 Switch profile

- Hub: click a row, then `CHƠI`; the row gets `● Đang dùng`.
- Gear / right-click menu -> `Bàn phím và chuột` -> `Hồ sơ` lists up to 9
  valid profiles with a tick on the chosen one (native submenu, costs nothing
  in Play because the bar is hidden then).
- Switching is only possible outside Play. In Play the user presses `` ` ``,
  switches, presses `` ` `` again.

### 2.7 Recover from a stuck touch

Shown as a help card from the Hub footer link `Điện thoại bị kẹt cảm ứng?`
and from the gear menu:

```
Điện thoại cứ như bị chạm mãi, hoặc không điều khiển được?
 1. Bấm F8 (dừng khẩn cấp): nhấc hết các ngón tay.
 2. Vẫn kẹt: bấm F3 để khoá màn hình điện thoại, rồi mở khoá lại.
 3. Vẫn chưa hết: rút cáp USB ra và cắm lại.
```

F8 is printed on every screen that can start touches (Hub footer, Play
toast, Test banner). After a panic the toast says
`Đã dừng khẩn cấp. Tất cả ngón tay đã nhấc.` Step 2 relies on F3 staying
active while the keymap is on (SPEC 7.4). Whether iOS clears a stuck contact
on lock is **unverified**; the card promises nothing beyond the steps.

## 3. Screens

Mockups show a landscape picture. Text in mockups is the real Vietnamese
text. `[ ]` = button, `( )` = key cap, `< >` = text field.

Placement rule for all menu surfaces: they are drawn inside Mirrorly's own
swapchain by the existing Direct2D pass (`ui.rs`). The only other windows
used are native Win32 popup menus (as `settings.rs` already does), and only
outside Play.

### 3.1 Play overlay

```
+----------------------------------------------------------------------+
|                                                                      |
|      (W)                                   + - - - - - - - +         |
|   (A) o (D)     ring 22 % alpha            :  Xoay camera  :         |
|      (S)                                   + - - - - - - - +         |
|                                                         (Chuột trái) |
|                            (R)   (C)   (Space)              Bắn      |
|                                                                      |
|     Bàn phím ĐANG BẬT  ·  ` thoát  ·  F8 dừng khẩn cấp  ·  [ ] độ nhạy |
+----------------------------------------------------------------------+
```

- The bottom line is the **existing upstream toast** (`Chrome.toast`), shown
  for 5 s on activation. No new drawing code.
- Key caps: one static layer. Overlay setting `Hiện phím khi chơi`:
  - `Tự ẩn` (default): caps visible for the first 5 s, then nothing is
    drawn at all (`Chrome.keymap = None`, so `UiFrame::is_empty()` is true and
    the Direct2D pass is skipped exactly as upstream, `present.rs:206`);
  - `Luôn hiện`: caps stay; pressed controls get the accent fill (from one
    atomic bitmask); the menu text beside this option says
    `Có thể làm hình chậm đi một chút. Chưa đo.` until a measurement exists (8.2);
  - `Tắt`: only the toast.
- No finger dots and no pointer are drawn in Play (iOS draws none for
  injected touches; ours would not be what the phone sees). Mouse motion
  never causes a redraw.
- 6th finger: when `Stats::dropped_presses` grows, a 2 s toast:
  `Đã đủ 5 ngón: phím vừa bấm bị bỏ qua.`

### 3.2 Edit mode

There are two layouts, chosen by the pure `layout::dock_placement()`:

**Side layout** (after WP9, when the monitor has room): the window grows to
the right by the dock width and the picture keeps its size.

```
+----------------------------------------------------+-------------------------+
|                                                    | ĐANG SỬA: Hồ sơ 1   [–] |
|      (W)                       + - - - - - - +     | [↶ Hoàn tác] [↷ Làm lại]|
|   (A) o (D) ●  <- handle       :      +      :     |-------------------------|
|      (S)                       + - - - - - - ●     | Thêm:                   |
|                                          (Chuột T) | [Cần di chuyển]         |
|                 (R)  (C)  (Space)                  | [Xoay camera] [Nút]     |
|                                                    |-------------------------|
|  - - - pink dashed guide while a drag aligns - - - | (properties, 3.3)       |
|  //// 6 % edge band, hatched ////                  |-------------------------|
|                                                    | Ngón cùng lúc: 5/5  (i) |
|                                                    | [ Thử ]  [ Xong ]       |
|                                                    | Chuột đang không điều   |
|                                                    | khiển điện thoại.       |
+----------------------------------------------------+-------------------------+
```

**Card layout** (first version, and the fallback whenever the window cannot
grow): the same dock is a floating card, 304 logical px wide, drawn over the
picture on the half **away from the selected control** (it flips when the
selection or a drag crosses the middle; with no selection it sits on the
right). In a portrait picture narrower than 2 x 304 px the dock becomes a
bottom sheet of at most 42 % of the picture height that flips to the top when
the selection is in the lower half. The `[–]` button collapses it to a 44 px
pill (`Bảng sửa ▸`); holding H hides dock and markers entirely.

Nothing else is drawn over the picture: no tool strip, no palette column.
This is the answer to "the editor covers the HUD being edited": one surface,
always on the side away from what is being edited, collapsible, and hidden
with H.

Canvas rules:

- Joystick: ring of `radius * short_side_px`, four key caps on the ring.
  Look: dashed square of half-side `reach * short_side_px` around the anchor
  cross, label `Xoay camera`. Button: key cap of fixed visual size (22
  logical px radius; not stored in the profile) with its name under it.
- Selected control: solid outline and one round resize handle (joystick: on
  the ring at 45 degrees; look: bottom-right corner). Others at 70 % opacity.
- Edge band: a hatched 6 % band along every picture edge. Controls may enter
  it; they then carry the warning `Gần mép màn hình: iPhone có thể hiểu là vuốt hệ thống.`
- `Ngón cùng lúc: N/5` counts the fingers that could be down together (one
  per joystick, look and button, capped display at `nhiều hơn 5`). Clicking
  `(i)` lists the controls counted and explains
  `Điện thoại chỉ nhận 5 ngón cùng lúc. Ngón thứ 6 sẽ bị bỏ qua.`
- Orientation mismatch: a banner across the top of the picture,
  `Hồ sơ này dành cho màn hình NGANG, hình đang DỌC.` with
  `[Xoay hình (F12)]`. Editing still works (coordinates are fractions).
- No phone picture: a neutral background with
  `Chưa có hình điện thoại. Vẫn sửa được, nhưng nên mở game để đặt nút cho đúng chỗ.`

### 3.3 Properties (inside the dock)

Nothing selected (profile properties):

```
HỒ SƠ
Tên            <Hồ sơ 1>
Màn hình       ( Ngang ) ( Dọc )
Phím bật/tắt   ( ` )
Phím dừng khẩn ( F8 )     Nên giữ nguyên F8.
Hiện phím khi chơi  ( Tự ẩn ) ( Luôn hiện ) ( Tắt )
Lưu ý (2)  ▸
[ Huỷ thay đổi ]
```

Button:

```
NÚT BẤM
Tên     <Nhảy>   ▾ gợi ý: Bắn · Ngắm · Nhảy · Ngồi · Nằm · Nạp đạn · Đổi súng · Hồi máu · Lựu đạn · Nhặt đồ · Lên xe
Phím    ( Space )            bấm vào để đổi
Kiểu    (•) Giữ phím = giữ ngón tay
        ( ) Bấm nhanh = chạm 1 lần (0,04 giây)
        Bắn nên dùng “Giữ”. Nhảy, nạp đạn nên dùng “Bấm nhanh”.
[ Nhân bản ]  [ Xoá ]
```

Joystick:

```
CẦN DI CHUYỂN
Tên     <Di chuyển>
Phím        ( W )
         ( A )( S )( D )     bấm vào từng phím để đổi
        [ Dùng W A S D ]  [ Dùng phím mũi tên ]
Cỡ vòng   ───●──────  0,11     (kéo núm tròn trên hình, hoặc lăn chuột)
        Khi giữ phím, ngón tay lệch khỏi tâm đúng bằng cỡ vòng.
[ Nhân bản ]  [ Xoá ]
```

Look:

```
XOAY CAMERA BẰNG CHUỘT
Tên     <Xoay camera>
Độ nhạy chuột   ────●─────  1,00    [ Chậm ] [ Vừa ] [ Nhanh ]
        Còn tuỳ vào chuột và cài đặt trong game. Hãy thử rồi chỉnh.
[ ] Đảo ngược lên / xuống
Cỡ vùng xoay    ───●──────  0,30    (kéo núm ở góc)
        Khi ngón tay chạm mép vùng, nó tự nhấc lên và đặt lại giữa.
        Vùng nhỏ: đặt lại thường xuyên. Vùng lớn: vuốt được dài hơn.
▸ Nâng cao
   Tự nhấc ngón khi chuột đứng yên:  ( Không ) ( 0,3 giây ) ( 1 giây ) ( 3 giây )
[ Xoá ]            Mỗi hồ sơ chỉ có 1 vùng xoay camera.
```

Widget set is closed: label, button, segmented choice, checkbox, slider with
`−`/`+` steppers (Shift = fine), key cap, text field (names only), list row.
No numeric text entry in v1; numbers are display only. `Dùng W A S D` /
`Dùng phím mũi tên` are disabled with a reason when one of those keys is used
elsewhere (`Phím W đang dùng cho “…”.`). `release_after_ms` choices map to
0 / 300 / 1000 / 3000.

### 3.4 Test mode (dry run)

```
+----------------------------------------------------------------------+
|  ĐANG THỬ – không gửi gì tới điện thoại      [ Quay lại sửa ]   `     |
|                                                                      |
|      (W)●lit        blue ring ① on the joystick finger               |
|   (A) o (D)                                + - - - - - - - +         |
|      (S)                                   :      ②→       :         |
|                                            + - - - - - - - +         |
|   Ngón: 2/5      Phím đang giữ: W, Chuột trái         F8 = dừng      |
+----------------------------------------------------------------------+
```

- A second, private `Engine` runs on the draft; its frames are never sent.
  Contacts are drawn as dots through the existing `render.rs` `Scene.dots`
  (`render.rs:127`); held controls light up.
- The camera box flashes once at each re-center so the user sees why the
  finger "jumps back".
- 6th finger: the cap pulses red and the counter reads
  `Đủ 5/5: phím này bị bỏ qua.`
- Works with the phone unplugged (panel size = last known `View.screen`).
- The mouse pointer is confined to the window and hidden only if the profile
  has a look control, as in Play, so the feel is the same. `` ` ``, F8, Esc
  or focus loss returns to Edit.

### 3.5 Hub (profile menu)

A centred card (max 560 x 520 logical px, never wider than the window minus
24 px) over a scrim, over the picture or over the status page.

```
+------------------------------------------------------------------+
|  Bàn phím và chuột                             [ Hướng dẫn ] [ × ] |
|------------------------------------------------------------------|
| ▣ mini   Hồ sơ 1                      NGANG · 10 nút   ● Đang dùng |
|          [   CHƠI   ]  [ Sửa ]  [ ⋯ ]                              |
|------------------------------------------------------------------|
| ▣ mini   Hồ sơ 2                      DỌC · 5 nút                  |
|          [ Chọn ]  [ Sửa ]  [ ⋯ ]                                  |
| ⚠        thu-cam-ung.json                                         |
|          Không dùng được: phím W bị dùng 2 lần.  [ Xem cách sửa ]  |
|------------------------------------------------------------------|
|  [ + Hồ sơ mới ]                          [ Chia sẻ hồ sơ ▾ ]     |
|------------------------------------------------------------------|
|  ` bật/tắt · F8 dừng khẩn cấp · 1 phím = 1 lần chạm. Không macro.  |
|  Điện thoại bị kẹt cảm ứng?          Lưu ý về luật chơi            |
+------------------------------------------------------------------+
```

- `CHƠI` is the only primary button and is on the chosen row. When Play
  would be refused, the reason replaces the button row (3.8).
- Mini preview: the profile drawn with the scene primitives at small scale;
  no screenshots are stored.
- `⋯` (native popup): `Đổi tên`, `Nhân bản`, `Xoá`. Delete moves the file to
  the trash and shows `Đã xoá “Hồ sơ 2”.  [ Hoàn tác ]` for 8 s.
- `Chia sẻ hồ sơ ▾` (native popup): `Sao chép hồ sơ đang chọn` (to the
  clipboard, toast `Đã sao chép. Dán vào Zalo/Messenger để gửi cho bạn bè.`),
  `Dán hồ sơ từ bộ nhớ tạm`, `Mở thư mục hồ sơ`. The words JSON and file
  never appear. Dropping a `.json` on the window while the Hub is open also
  imports it (outside the Hub, drops keep upstream's meaning: send to phone).
- Broken files (hand-edited or from elsewhere) are listed with the first
  problem in plain Vietnamese, can never be chosen, and are never deleted or
  modified automatically. `Xem cách sửa` shows all problems; if every problem
  is a number out of range it offers `Sửa giúp` (clamp only, writes a `.bak`
  first, then reports each change). Keys are never rebound automatically.
- Empty state: `Chưa có hồ sơ nào.` + `[ Tạo hồ sơ đầu tiên ]`.
- Draft recovery, on opening the Hub after a crash:
  `Lần trước bạn sửa “Hồ sơ 1” chưa lưu.  [ Khôi phục ]  [ Bỏ ]`.

### 3.6 First-run notice (once; again if `NOTICE_VERSION` increases)

```
+------------------------------------------------------------------+
|  Trước khi chơi bằng bàn phím và chuột                            |
|                                                                  |
|  • Nhà phát hành game (ví dụ Garena với Free Fire) có thể không   |
|    cho phép phần mềm bên thứ ba. Chúng tôi chưa kiểm chứng luật   |
|    hiện hành của họ. Bạn hãy tự đọc điều khoản của game trước.    |
|  • Không có gì bảo đảm tài khoản an toàn, kể cả tài khoản phụ.    |
|    Game cũng có thể xếp người chơi bàn phím–chuột vào nhóm ghép   |
|    trận riêng; chúng tôi không biết và không can thiệp việc đó.   |
|  • Phần mềm này chỉ đổi 1 phím = 1 lần chạm. Không macro, không   |
|    tự bắn, không tự ngắm, không giảm giật, không che giấu gì.     |
|  • Điều khiển nhiều ngón trên iPhone thật chưa được kiểm chứng.   |
|                                                                  |
|  [ ] Tôi đã đọc và tự chịu trách nhiệm        [ Thoát ] [ Tiếp ] |
+------------------------------------------------------------------+
```

`Tiếp` is disabled until the box is ticked. Acceptance is stored as
`notice_ack = NOTICE_VERSION` in `keymap-ui.json`. No link is opened (we have
no verified URL). The notice can be re-read from the Hub footer and the gear
menu; it must not be removed or softened.

### 3.7 First-run guide (coach card on top of Edit)

The card sits in the dock area (top of the dock); the thing to act on gets a
pulsing ring. `Tiếp` is always enabled; a step also completes by itself when
the user really did it. `Bỏ qua hướng dẫn` ends the guide; `Hướng dẫn` in the
Hub restarts it.

| Step | Card text | Auto-completes when |
|---|---|---|
| 1/6 | `Mở game và vào một trận tập để thấy đủ các nút. Xoay hình cho đúng chiều (F12).` | picture rotation matches the profile orientation and a picture exists |
| 2/6 | `Kéo vòng tròn W A S D trùm lên cần di chuyển trong game. Kéo núm tròn để đổi cỡ cho vừa.` | joystick centre or radius differs from the template by > 0.01 |
| 3/6 | `Ô nét đứt là vùng chuột xoay camera. Kéo nó vào nửa phải màn hình, chỗ không có nút.` | look anchor or reach changed |
| 4/6 | `Kéo nút “Bắn” lên đúng nút bắn trong game. Muốn đổi phím: bấm vào ô phím rồi bấm phím bạn thích.` | the highlighted button moved or was rebound |
| 5/6 | `Làm tương tự với các nút còn lại. Nút nào không cần thì chọn rồi bấm Xoá.` | user presses `Tiếp` (each remaining button is highlighted in turn while hovered) |
| 6/6 | `Bấm [Thử], rồi bấm W A S D, rê chuột, bấm chuột trái. Chấm xanh là chỗ ngón tay sẽ chạm. Chưa có gì gửi tới điện thoại.` then `[ Lưu & Chơi ]` | Test was entered and at least one contact was seen |

With the `Trống` template, steps 2-4 become
`Kéo [Cần di chuyển] / [Xoay camera] / [Nút] từ bảng bên vào hình.` and
complete when a control of that type exists.

### 3.8 Refusals (plain reason + fix)

Shown in the Hub in place of the `CHƠI` row, and as a toast when `` ` `` is
pressed in Off.

| Cause (`PlayRefusal`) | Text | Fix button |
|---|---|---|
| `NoProfile` | `Chưa chọn hồ sơ nào.` | `Mở danh sách hồ sơ` |
| `NoticeNotAccepted` | `Bạn cần đọc lưu ý trước lần chơi đầu.` | `Đọc lưu ý` |
| `NotConnected` | `Điện thoại chưa kết nối.` | – |
| `Wifi` | `Đang nối qua Wi-Fi. Bàn phím chỉ chạy khi cắm cáp USB.` | – |
| `DisplayMode` | `Đang dùng iPad làm màn hình phụ. Hãy tắt chế độ đó trước.` | – |
| `ScrollBusy` | `Đang cuộn dở. Thử lại sau một giây.` | – |
| `Orientation` | `Hồ sơ dành cho màn hình NGANG, hình đang DỌC.` (or the reverse) | `Xoay hình (F12)` |
| `BrokenProfile` | `Hồ sơ này bị lỗi nên chưa dùng được.` | `Xem cách sửa` |

### 3.9 Native menu entries (`settings.rs` popup: gear button or right-click on the bar)

```
Bàn phím và chuột            ▸   Mở bảng hồ sơ…              Ctrl+K
                                 Sửa nút…
                                 Hồ sơ                      ▸  ✓ Hồ sơ 1
                                 ─────────────                   Hồ sơ 2
                                 Hướng dẫn lần đầu
                                 Lưu ý về luật chơi
                                 Điện thoại bị kẹt cảm ứng?
                                 ─────────────
                                 Dừng khẩn cấp: F8          (greyed note)
```

Under `--keymap FILE` the submenu has one greyed line:
`Đang dùng hồ sơ từ dòng lệnh.`

## 4. Input map

Key names are SPEC 3 names (`format!("{code:?}")`). "Upstream" means the
event continues into Mirrorly's unchanged handling.

### 4.1 Mirrorly's F-keys in each mode

| Key | Upstream meaning | Off | Hub | Edit | Test | Play |
|---|---|---|---|---|---|---|
| F1 Home, F2 app switcher, F3 lock, F4/F5 volume | phone keys | upstream | upstream | upstream (so the user can reach the game screen without leaving Edit) | ignored | upstream, unless the profile binds the key (then it is a game key) |
| F6 screenshot, F7 record | PC-side | upstream | upstream | upstream | upstream | as above |
| F8 | free upstream | consumed, toast `Đã dừng khẩn cấp…` | same | cancels drag/capture | stops Test | panic (SPEC 5.5) |
| **F9 latency probe (10 automated taps)** | upstream | upstream | **blocked** | **blocked** | **blocked** | **blocked** (SPEC 7.4) |
| F10 stats | upstream | upstream | upstream | upstream | upstream | upstream (+ overlay cost line, 8.2) |
| F11 fullscreen, F12 rotate | upstream | upstream | upstream | upstream (`set_view` on the model) | F12 ends Test first | upstream; a rotation that breaks the orientation deactivates (SPEC 5.4) |
| Ctrl+1/2/3 | Home/switcher/… | upstream | blocked | blocked | blocked | suppressed (SPEC 0.12) |

F9 is blocked whenever `Shell::mode() != Off`, with a log line; the probe's
automated taps must never mix with keymap frames or run while the user thinks
touches are off. While a capture is listening or a text field is focused,
**every** key including F1-F12 goes to the capture/field first and nothing
else acts.

Reserved by this spec: `Ctrl+K` (open/close Hub, only in Off/Hub; verify at
implementation that upstream's shortcut `match` at `main.rs` ~1608 has no
`KeyK` case — none was seen), F8, and in Play the helper keys `[` `]`.

### 4.2 Off

| Input | Result |
|---|---|
| Bar button `Bàn phím`, gear menu items, `Ctrl+K` | open Hub (or the named screen) |
| `` ` `` (profile's toggle key), non-repeat | `Shell::request_play`: start Play or toast the refusal. Consumed (cannot be typed to the phone; SPEC 7.4 limit) |
| F8 | consumed; toast |
| everything else | upstream |

### 4.3 Hub

| Input | Result |
|---|---|
| Click row / `CHƠI` / `Chọn` / `Sửa` / `⋯` / footer links | as labelled |
| Up / Down | move row selection |
| Enter | `CHƠI` on the selected row |
| Delete | delete selected profile (undo toast) |
| Ctrl+Z | undo the last delete (while the toast is up) |
| Ctrl+V | `Dán hồ sơ từ bộ nhớ tạm` |
| Esc, `×`, `Ctrl+K`, click on the scrim | close Hub (to Off) |
| Drop `.json` | import |
| Mouse elsewhere, wheel, right-click, other keys, typing | swallowed (nothing reaches the phone) |

### 4.4 Edit

Mouse (never reaches the phone; no Home on right-click):

| Input | Result |
|---|---|
| Move | hover highlight; pointer shape hand / move / resize |
| Click control | select |
| Click key cap (canvas or dock) | start capture for that key |
| Click empty picture | deselect; with an armed palette item: place it there |
| Drag control | move; aligns to other controls' x/y within 6 px (pink guide); clamped to the picture; one undo step |
| Drag resize handle | joystick radius (0.03 to 0.5) / look reach (0.05 to 1.0); value shown beside the pointer |
| Wheel over the selected joystick/look | resize by 0.01 (Shift: 0.002); a burst within 400 ms is one undo step |
| Drag a palette item onto the picture, or click it then click the picture | add at that point (a ghost follows the pointer; Esc cancels) |
| Right-click control | native popup: `Đổi phím`, `Nhân bản`, `Xoá` (button also: `Kiểu: Giữ` / `Kiểu: Bấm nhanh`) |
| Right-click empty picture | native popup `Thêm ở đây`: `Cần di chuyển`, `Xoay camera` (greyed if one exists), `Nút` |
| Hold Alt while dragging | no alignment snapping |
| Hold Shift while dragging | move along one axis only |
| Dock widgets | as labelled |

Keyboard (nothing is typed on the phone):

| Key | Result |
|---|---|
| Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z | undo / redo (100 steps) |
| Delete, Backspace | delete the selected control |
| Ctrl+D | duplicate (offset +3 %, fresh free keys) |
| Arrows / Shift+Arrows | nudge 1 px / 10 px of the picture (burst within 500 ms = one undo step) |
| Tab / Shift+Tab | select next / previous control |
| Enter | start capture on the selected control (joystick: Up first) |
| Ctrl+S | save now (stay in Edit) |
| H (held) | hide dock and markers |
| `` ` `` (profile's toggle key) | enter Test — **only when no capture is listening and no text field is focused**; otherwise the key goes to the capture (bindable) or is typed into the field |
| F8 | cancel drag/capture |
| Esc | cancel capture/placement/drag -> else deselect -> else leave Edit (asks `Lưu thay đổi?  [ Lưu ] [ Không lưu ] [ Sửa tiếp ]` when dirty) |
| F-keys | table 4.1 |

Single-letter shortcuts other than H do not exist (they collide with typing
and with capture, and a novice does not need them).

Capture sub-state (model state `Capture`; app forwards **all** keyboard keys
and mouse buttons, key auto-repeat filtered):

| Input | Result |
|---|---|
| Key press with a SPEC 3 name | bind / conflict prompt (2.4) |
| Key not in SPEC 3 list | `Phím này chưa dùng được…`, keep listening |
| Mouse button on the `Huỷ` chip | cancel (hit-tested before binding) |
| Mouse Left/Right/Middle/Back/Forward elsewhere | bind `MouseLeft`… (refused for toggle/panic slots) |
| Escape | arm; second Escape within 1500 ms binds `Escape` |
| In a conflict prompt: `Đổi chỗ` / Enter | swap; `Huỷ` / Esc: cancel |
| Focus loss | cancel |

### 4.5 Test

| Input | Result |
|---|---|
| Keys and mouse buttons bound in the draft, raw mouse motion | fed to the dry-run engine; dots and highlights |
| `` ` ``, Esc, F8, `Quay lại sửa`, focus loss | back to Edit (all dry-run contacts cleared) |
| F1-F5 | ignored |
| F6, F7, F10, F11 | upstream |
| F9 | blocked |
| anything else | swallowed |

### 4.6 Play

Exactly SPEC 7.4, plus:

| Input | Result |
|---|---|
| Profile keys / mouse buttons / raw motion | engine |
| toggle key | Play -> Off |
| panic key | lift everything, Play -> Off, toast |
| `[` / `]` (helper keys from `keymap-ui.json`, only if not bound by the profile) | sensitivity −10 % / +10 %, toast; consumed, never sent to the engine or phone |
| F-keys | table 4.1 (F9 blocked) |
| focus loss, disconnect, rotation that breaks orientation | Play -> Off (SPEC) |

No menu, popup or widget is reachable in Play.

### 4.7 Key namespaces and conflicts

1. Profile keys (`toggle_key`, `panic_key`, control keys) are unique inside
   a profile (SPEC 4.1). The editor enforces it by construction.
2. Helper keys (`sens_down_key`, `sens_up_key`) live in `keymap-ui.json`.
   Rule: **the profile always wins.** A helper key bound by the active
   profile is inactive for that profile (Hub note, 2.5). The auto-assign list
   never proposes `BracketLeft`/`BracketRight`; capturing one shows the
   advice `Phím này đang dùng để chỉnh độ nhạy khi chơi. Nếu gán, bạn sẽ không chỉnh nhanh được nữa.`
3. Mirrorly F-keys bound in a profile shadow the shortcut in Play only
   (SPEC 7.4); the editor shows advice per key (6.9).
4. Windows keys that cannot be captured or are unsafe: `SuperLeft/Right`
   (advice: `Phím Windows sẽ mở menu Start.`), Alt+Tab, Ctrl+Alt+Del
   (not capturable; nothing to do), `AltLeft` (advice: `Alt+F4 sẽ đóng cửa sổ.`).

## 5. Engine and profile changes beyond SPEC.md

Rule: the v1 menu (WP1-WP9) needs **no change to the profile schema** and
no change to existing engine behaviour. Files of profile v1 written by the
menu are byte-compatible with `keymap-sim validate` and the browser editor.

### 5.1 `Engine::set_look_tuning` (additive, WP7)

```rust
impl Engine {
    /// Change the look control's sensitivity in place. Emits no frame, lifts
    /// nothing, keeps the current offset; applies from the next MouseMotion.
    /// Err: no look control, or value outside (0, 100] / non-finite.
    pub fn set_look_tuning(&mut self, sensitivity: f64) -> Result<(), ProfileError>;
}
```

`engine.profile()` reflects the new value afterwards. Tests: no frame is
produced; the next motion is scaled by the new value; out-of-range and
no-look are errors and change nothing; works active and inactive. (With
schema v2 the signature gains nothing: `y_ratio` is edited only in Edit.)

### 5.2 Things done without engine changes

- Pressed-control highlights and the finger dots of Test come from
  `menu::tracker` (6.11), which watches the same `Input`s and the dry-run
  frames. `Stats::dropped_presses` deltas drive the "5 ngón" message.
- Overlay level, helper keys, notice acknowledgement, guide state, chosen
  profile: `keymap-ui.json`, never the profile.

### 5.3 Profile schema v2 (specified now, implemented last in WP10)

Blocked until the SPEC.md team's work on `profile.rs`, `engine.rs`,
`keys.rs`, the vectors and `editor/index.html` has landed; WP10 then extends
those files and SPEC.md in one change.

**Version handling**

- Readers accept `"version": 1` and `"version": 2`. Any other value:
  problem at path `version` (as today).
- A v1 file may not contain v2 fields (unknown-field error, as today).
- Writers (`to_json_pretty`, the menu, the browser editor) emit **the lowest
  version that can express the profile**: `1` when every v2 field has its
  default and no `freelook` control exists, omitting all v2 fields; otherwise
  `2`. Consequence: a profile that does not use v2 features keeps working in
  builds and tools that only know v1; a build that only knows v1 rejects a v2
  file with its existing `version` problem, which the Hub explains as
  `Hồ sơ này làm bằng bản mới hơn. Hãy cập nhật chương trình.`
- In Rust, `Profile.version` stays a field; `Profile::normalize_version(&mut self)`
  sets it by the rule above and is called by every writer. Parsing a v2 file
  uses the same structs (new fields carry `#[serde(default)]` and
  `skip_serializing_if` their default); a post-parse check rejects v2 fields
  in a `version: 1` file at path `controls[i].<field>` with message
  `needs "version": 2`.

**Additions (all inside `controls[]`)**

| type | Field | Type | Default | Rule | Meaning |
|---|---|---|---|---|---|
| `look` | `y_ratio` | number | `1.0` | in (0, 10] | vertical speed relative to horizontal: `step.y = dy * (invert_y ? -1 : 1) * sensitivity * y_ratio / 1000` |
| `look` | `recenter` | `"lift"` \| `"handoff"` | `"lift"` | – | `lift` = SPEC 5.6. `handoff` = see below |
| new type `freelook` | `label`, `x`, `y` | as all controls | – | – | where the game's free-look ("eye") button is |
| `freelook` | `key` | key name | required | known key, unique | hold to free-look |
| `freelook` | `reach` | number | `0.3` | in (0, 1] | how far the finger may travel from (x, y) |
| `freelook` | `sensitivity` | number | `1.0` | in (0, 100] | mouse speed while free-looking |

Global rule added: at most one `freelook` control. Validation order: the new
numeric fields follow the existing ones in table order; the second-`freelook`
rule comes right after the second-look rule (path `controls[i]`, message
`only one freelook control is allowed`).

Example (v2):

```json
{
  "version": 2,
  "orientation": "landscape",
  "controls": [
    { "type": "look", "x": 0.68, "y": 0.4, "sensitivity": 1.2, "y_ratio": 0.8, "recenter": "handoff" },
    { "type": "freelook", "label": "Nhìn quanh", "x": 0.9, "y": 0.3, "key": "AltLeft" }
  ]
}
```

**`freelook` behaviour** (one key = one contact; the mouse only moves it):

1. `Press(key)`: if a look contact is down, it lifts and the free-look
   contact goes down at (x, y) in the same transition frame; offset = 0.
   Slot exhaustion as SPEC 5.6.
2. While held, `MouseMotion` moves this contact:
   `offset += (dx, dy * (look.invert_y ? -1 : 1)) * sensitivity / 1000`,
   clamped per axis to `±reach`. **No re-center, no lift**: at the edge it
   stops. The look control receives no motion while the key is held.
3. `Release(key)`: lift. The next `MouseMotion` starts the look finger
   afresh at its anchor (SPEC 5.6 look step 1).
4. Without a look control in the profile, steps 1-3 hold unchanged.

**`recenter: "handoff"` behaviour** (experimental; default off; UI label
`Đặt lại mượt (thử nghiệm)`): when a step would leave the reach box,
instead of lift-then-down the engine emits `[T: m↓ at anchor]` with the old
contact `n` still down and unmoved, then `[T: n↑]`, in the same `handle`
call; later motion moves `m`. If no contact slot is free it falls back to
`lift` for that re-center. `look_recenters` counts both kinds.

Open points the lead must decide before WP10 implements `handoff`:
(a) for one frame two contacts exist for one mouse movement, which needs an
explicit sentence in SPEC 1 (it is not automation, repetition or assistance,
but "at most one contact going down per press" was written with keys in
mind); (b) whether a game reads the brief second finger as a pinch or
ignores it is **unverified** and can only be learned on hardware. If either
is unresolved, ship `y_ratio` and `freelook` without `handoff`.

**UI for v2** (dock, look control, under `▸ Nâng cao`):
`Tốc độ lên/xuống so với ngang  ───●───  ×1,0`,
`Đặt lại mượt (thử nghiệm)  [ ]`; palette gains `Nhìn quanh (giữ phím)`,
disabled with a tip once one exists.

**Backward compatibility summary**

| File | Old build (v1 only) | New build |
|---|---|---|
| v1 file | works | works, re-saved as v1 unless a v2 feature is turned on |
| v2 file | rejected at `version` with a clear message | works |
| Browser editor before WP10 | opens v1 files only | – |
| Browser editor after WP10 | opens and writes v1 and v2 by the same lowest-version rule | – |

### 5.4 Rejected or deferred engine ideas

- Latched toggle button (`ButtonMode::Toggle`): rejected (1.3).
- Wheel as buttons, hold-to-free-cursor: deferred; both change contracts in
  `keys.rs` / the app's gesture routing and are not needed to play.
- Import auto-repair that rebinds keys: rejected (surprising).

## 6. Pure model in `crates/keymap`

Constraints: `#![forbid(unsafe_code)]`; dependencies stay `serde` +
`serde_json`; no clock, thread, randomness or real file I/O (time comes in as
`now_ms: u64`, files go through the `Fs` trait); no warnings under
`cargo clippy -- -D warnings`; the crate must still compile when copied into
Mirrorly's workspace, so test data lives under `crates/keymap/tests/data/menu/`.
The only edit to an existing file is one line in `lib.rs`: `pub mod menu;`.
Existing modules are used through their public API and not modified
(`keys::KEY_NAMES`, `MOUSE_NAMES`, `is_known_key`, `is_mouse_button`,
`Profile`, `Control`, `Engine`, `View`, `Rotation`).

```
src/menu/
  mod.rs        re-exports
  msg.rs        Msg, Lang, number formatting
  keynames.rs   key_label, key_advice, free_key, label presets
  doc.rs        EditDoc (always-valid profile + stable ids)
  history.rs    undo/redo with coalescing
  hit.rs        EditView, hit_test
  drag.rs       Drag state, placement
  snap.rs       alignment guides, clamping
  capture.rs    key-capture state machine
  text.rs       TextEdit
  lint.rs       advice list, Problem -> Msg
  layout.rs     dock/hub layout, widgets, widget hit-test
  scene.rs      Prim, Scene, to_svg
  guide.rs      first-run steps
  templates.rs  built-in layouts
  store.rs      Fs, MemFs, ProfileStore, slugify, repair_ranges
  prefs.rs      UiPrefs (keymap-ui.json)
  tracker.rs    HeldTracker, TouchDots
  editor.rs     Editor (Edit + Test for one profile)
  shell.rs      Shell (modes, Hub, notice, refusals, effects)
```

### 6.1 Shared small types (`hit.rs`, `mod.rs`)

```rust
pub type CtrlId = u32;                       // stable per editing session, never serialized
#[derive(Clone, Copy, PartialEq, Debug)] pub struct Pt { pub x: f64, pub y: f64 }
#[derive(Clone, Copy, PartialEq, Debug)] pub struct PxRect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Dir { Up, Down, Left, Right }
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)] pub struct Mods { pub shift: bool, pub ctrl: bool, pub alt: bool }

/// Where the picture is inside the window (physical px) and the DPI scale.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EditView { pub window: (f64, f64), pub picture: PxRect, pub scale: f64 }
impl EditView {
    pub fn to_norm(&self, p: Pt) -> Pt;        // window px -> picture fraction (not clamped)
    pub fn to_px(&self, n: Pt) -> Pt;
    pub fn short_px(&self) -> f64;             // min(picture.w, picture.h)
    pub fn lp(&self, logical: f64) -> f64;     // logical px -> physical
}

pub const RADIUS_MIN: f64 = 0.03;  pub const RADIUS_MAX: f64 = 0.5;
pub const REACH_MIN: f64 = 0.05;   pub const REACH_MAX: f64 = 1.0;
pub const SENS_SLIDER_MIN: f64 = 0.1; pub const SENS_SLIDER_MAX: f64 = 10.0;
pub const EDGE_BAND: f64 = 0.06;
pub const NOTICE_VERSION: u32 = 1;
```

All values written into a profile are rounded to 4 decimals (the browser
editor's rule), so both editors produce identical numbers.

### 6.2 `Shell` (`shell.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Mode { Off, Hub, Edit, Test, Play }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Link { None, Usb, Wifi }

/// Facts only the app knows, passed with every event.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Env {
    pub link: Link, pub display_mode: bool, pub scroll_busy: bool,
    pub rotation: Rotation, pub panel: (f64, f64), pub has_picture: bool,
    pub view: EditView, pub can_grow_window: bool,
}

pub enum UiEvent<'a> {
    PointerMove { p: Pt, mods: Mods },
    PointerDown { p: Pt, button: &'a str /* "MouseLeft".. */, mods: Mods },
    PointerUp   { p: Pt, button: &'a str, mods: Mods },
    Wheel { p: Pt, dy: f64, mods: Mods },
    Key { name: &'a str, pressed: bool, mods: Mods },      // auto-repeat already filtered, except for text fields (KeyRepeat)
    KeyRepeat { name: &'a str, mods: Mods },                // only used by a focused text field and arrow nudges
    Text(&'a str),                                          // KeyEvent.text / Ime::Commit
    RawMotion { dx: f64, dy: f64 },                         // Test mode only
    Tick, FocusLost, EnvChanged,
    Command(Command),                                       // bar button, native menu ids, widget-independent
    MenuPick(u32),                                          // id from a MenuSpec the model asked to show
    Pasted(Option<&'a str>),                                // answer to Effect::RequestPaste
    Dropped { name: &'a str, text: Option<&'a str> },       // .json dropped while Hub is open
    PlayEnded { reason: PlayEnd, look_sensitivity: Option<f64> },
    PlayStarted,
}
pub enum Command { ToggleHub, OpenHub, OpenEdit, ChooseProfile(usize), ShowGuide, ShowNotice, ShowStuckHelp, Panic }
pub enum PlayEnd { Toggle, Panic, FocusLost, Disconnected, Orientation }

pub enum Effect {
    Redraw,                                 // scene_version changed
    Toast { msg: Msg, secs: f32 },
    Cursor(CursorKind),                     // Arrow | Hand | Move | ResizeNwse | Text
    StartPlay,                              // app: SPEC 7.4 gate + activate with shell.active_profile()
    GrabPointer(bool),                      // Test with a look control
    ShowMenu(MenuSpec),                     // native popup; answer with MenuPick
    CopyText(String), RequestPaste, OpenFolder,
    ImeAllowed(bool),
    Rotate, ReserveRight(f64),              // logical px; 0 = restore
    Title,                                  // window title needs refreshing
}
pub struct MenuSpec { pub items: Vec<MenuItem> }
pub enum MenuItem { Item { id: u32, label: String, checked: bool, enabled: bool }, Separator, Sub { label: String, items: Vec<MenuItem> } }

#[derive(Clone, PartialEq, Debug)]
pub enum PlayRefusal { NoProfile, NoticeNotAccepted, NotConnected, Wifi, DisplayMode, ScrollBusy,
                       Orientation { profile: Orientation }, BrokenProfile }

pub struct Shell<F: Fs> { /* store, prefs, mode, hub, editor: Option<Editor>, notice, toasts, trash tokens */ }
impl<F: Fs> Shell<F> {
    pub fn new(fs: F, keymaps_dir: &str, prefs_path: &str) -> Shell<F>;   // loads prefs + list; never fails (broken prefs = defaults)
    pub fn mode(&self) -> Mode;
    pub fn event(&mut self, ev: UiEvent<'_>, env: &Env, now_ms: u64) -> Vec<Effect>;
    pub fn next_deadline(&self) -> Option<u64>;            // toast expiry, esc window, overlay auto-hide, autosave
    /// What to draw now; None = draw nothing (keeps the D2D pass skipped).
    pub fn scene(&self, env: &Env, now_ms: u64) -> Option<Scene>;
    pub fn scene_version(&self) -> u64;                    // bumps whenever scene() would differ
    pub fn dots(&self) -> TouchDots;                       // Test mode fingers
    pub fn swallows_input(&self) -> bool;                  // Hub/Edit/Test: app must not run the gesture path
    pub fn can_play(&self, env: &Env) -> Result<(), PlayRefusal>;
    pub fn active_profile(&self) -> Option<&Profile>;      // the chosen, valid profile
    pub fn play_key(&self, name: &str) -> PlayKey;         // Engine | SensDown | SensUp (4.7 rule 2)
    pub fn native_menu(&self) -> MenuItem;                 // the "Bàn phím và chuột" submenu (3.9)
    pub fn title_suffix(&self, lang: Lang) -> String;      // " · Bàn phím BẬT" / " · Bàn phím: bấm ` để chơi" / ""
    pub fn lang(&self) -> Lang;
}
```

Transition table (tested exhaustively):

| From | Event | To | Notes |
|---|---|---|---|
| Off | `ToggleHub` / `OpenHub` | Hub | |
| Off | toggle key press | Play or Off | `can_play` ok -> `Effect::StartPlay`, wait for `PlayStarted`; else toast |
| Hub | `CHƠI` | notice card -> Play | notice only if `notice_ack < NOTICE_VERSION` |
| Hub | `Sửa`, `OpenEdit` | Edit | broken profile -> stays, shows problems |
| Hub | Esc / close | Off | |
| Edit | toggle key (no capture, no field) / `Thử` | Test | |
| Edit | `Xong` | Hub | saves; draft removed |
| Edit | Esc (nothing to cancel) | Hub | via the save prompt when dirty |
| Test | toggle key / Esc / F8 / FocusLost / F12 | Edit | dry-run engine dropped |
| any | `PlayStarted` | Play | scene = overlay per prefs |
| Play | `PlayEnded` | Off | saves a changed sensitivity (2.5) |
| Hub/Edit | `EnvChanged` | same | banners recomputed; never loses the draft |
| any | `Command::Panic` in Off/Hub | same | toast only |

### 6.3 `EditDoc` (`doc.rs`): a profile that is always valid

```rust
pub struct EditDoc { /* profile: Profile, ids: Vec<CtrlId>, next: CtrlId */ }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot { Toggle, Panic, Button(CtrlId), Dir(CtrlId, Dir) }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Kind { Joystick, Look, Button }
#[derive(Clone, PartialEq, Debug)]
pub enum BindOutcome { Bound, Unchanged, Conflict { owner: Slot } }   // Conflict: nothing changed
#[derive(Clone, PartialEq, Debug)]
pub enum EditError { NoSuchControl, UnknownKey, MouseNotAllowed, TooManyControls, SecondLook, NoFreeKey, WrongKind }

impl EditDoc {
    pub fn from_profile(p: Profile) -> Result<EditDoc, ProfileError>;   // validates
    pub fn profile(&self) -> &Profile;
    pub fn ids(&self) -> &[CtrlId];
    pub fn index_of(&self, id: CtrlId) -> Option<usize>;
    pub fn kind(&self, id: CtrlId) -> Option<Kind>;
    pub fn key_of(&self, slot: Slot) -> Option<&str>;
    pub fn owner_of(&self, key: &str) -> Option<Slot>;

    /// Adds a control of `kind` at a picture fraction with default size and
    /// automatically chosen free keys. Err: SecondLook, TooManyControls, NoFreeKey.
    pub fn add(&mut self, kind: Kind, at: Pt) -> Result<CtrlId, EditError>;
    pub fn remove(&mut self, id: CtrlId) -> Result<(), EditError>;
    pub fn duplicate(&mut self, id: CtrlId) -> Result<CtrlId, EditError>;   // +0.03/+0.03, fresh free keys; look -> SecondLook
    pub fn move_to(&mut self, id: CtrlId, at: Pt) -> Result<(), EditError>; // clamps to [0,1], rounds
    pub fn set_radius(&mut self, id: CtrlId, r: f64) -> Result<(), EditError>;       // clamps RADIUS_MIN..=RADIUS_MAX
    pub fn set_reach(&mut self, id: CtrlId, r: f64) -> Result<(), EditError>;        // clamps REACH_MIN..=REACH_MAX
    pub fn set_sensitivity(&mut self, id: CtrlId, s: f64) -> Result<(), EditError>;  // clamps 0.05..=100, non-finite ignored
    pub fn set_invert_y(&mut self, id: CtrlId, on: bool) -> Result<(), EditError>;
    pub fn set_release_after(&mut self, id: CtrlId, ms: u32) -> Result<(), EditError>; // clamps 0..=60000
    pub fn set_mode(&mut self, id: CtrlId, m: ButtonMode) -> Result<(), EditError>;
    pub fn set_label(&mut self, id: CtrlId, s: &str) -> Result<(), EditError>;       // truncates to 40 chars
    pub fn set_name(&mut self, s: &str);                                             // truncates to 100 chars
    pub fn set_orientation(&mut self, o: Orientation);
    pub fn bind(&mut self, slot: Slot, key: &str) -> Result<BindOutcome, EditError>; // UnknownKey, MouseNotAllowed
    pub fn swap(&mut self, a: Slot, b: Slot) -> Result<(), EditError>;               // MouseNotAllowed if a mouse name would land on Toggle/Panic
    pub fn set_joystick_keys(&mut self, id: CtrlId, preset: KeyPreset) -> Result<(), EditError>; // Wasd | Arrows; Err(NoFreeKey) if taken elsewhere
}
```

Invariant (unit + fuzz): after every call, successful or not,
`doc.profile().problems().is_empty()` and `ids.len() == controls.len()` with
unique ids. Mutators never produce an out-of-range number: they clamp.
Defaults for `add`: joystick radius 0.11, keys = first free set of
WASD / arrows / IJKL / TFGH, else four free keys; look sensitivity 1.0,
reach 0.30, no invert, release 0; button mode hold, key from
`keynames::free_key`.

### 6.4 History (`history.rs`)

```rust
pub struct History { /* undo: Vec<Snap>, redo: Vec<Snap>, cap = 100, open gesture */ }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gesture { Drag, Wheel, Slider(u32), Nudge, Typing(u32), Single }
impl History {
    pub fn new(doc: &EditDoc) -> History;
    /// Call before a change. Same-key gestures within `window_ms` coalesce (Wheel 400, Nudge 500, Typing 1000; Drag/Slider until `end`).
    pub fn begin(&mut self, doc: &EditDoc, sel: Option<CtrlId>, g: Gesture, now_ms: u64);
    pub fn end(&mut self, doc: &EditDoc);                   // drops the step if nothing changed
    pub fn undo(&mut self, doc: &mut EditDoc) -> Option<Option<CtrlId>>;   // returns the selection to restore
    pub fn redo(&mut self, doc: &mut EditDoc) -> Option<Option<CtrlId>>;
    pub fn can_undo(&self) -> bool; pub fn can_redo(&self) -> bool;
    pub fn mark_saved(&mut self); pub fn dirty(&self) -> bool;
}
```

Snapshots are whole `EditDoc` clones (<= 64 controls).

### 6.5 Hit-testing (`hit.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit { Handle(CtrlId), Cap(Slot), Body(CtrlId), None }
pub fn hit_test(doc: &EditDoc, view: &EditView, selected: Option<CtrlId>, p: Pt) -> Hit;
pub fn handle_pos(doc: &EditDoc, view: &EditView, id: CtrlId) -> Option<Pt>;
pub fn cap_rects(doc: &EditDoc, view: &EditView, id: CtrlId) -> Vec<(Slot, PxRect)>;
```

Order: (1) resize handle of the selection (slop 10 logical px); (2) key caps
of all controls, topmost = later in the array; (3) bodies, smallest area
first: button = disc of 22 logical px (minimum hit radius 18), joystick =
disc of `radius * short_px`, look = the anchor disc (14 logical px) and an
8 logical px band along the dashed border **only** (its interior is click-
through, so the large box never swallows clicks on buttons inside it);
(4) `None`. Widget and dock hits are tested before the canvas, in
`layout.rs`.

### 6.6 Drag and snap (`drag.rs`, `snap.rs`)

```rust
pub enum Drag {
    Pending { id: CtrlId, down: Pt },                         // becomes Move after 4 logical px
    Move { id: CtrlId, grab: Pt /*px offset centre - pointer*/, start: Pt /*norm*/, axis: Option<Axis> },
    Resize { id: CtrlId },
    Place { kind: Kind },                                     // palette ghost following the pointer
}
pub struct Guide { pub a: Pt, pub b: Pt }                     // window px, drawn pink dashed
pub fn snap_move(target: Pt /*px*/, others: &[Pt] /*px centres*/, view: &EditView, enabled: bool) -> (Pt, Vec<Guide>);
pub fn resize_value(kind: Kind, centre: Pt, pointer: Pt, view: &EditView) -> f64;  // joystick: distance/short_px; look: max(|dx|,|dy|)/short_px
pub fn in_edge_band(n: Pt) -> bool;
```

Snap threshold 6 logical px per axis, independently; no grid. A click that
never passes the 4 px threshold is a select, not a move, and records no undo
step. Esc during a drag restores `start`.

### 6.7 Capture (`capture.rs`)

```rust
pub enum CaptureState {
    Listening { slot: Slot, esc_until: Option<u64>, note: Option<Msg> },
    Conflict { slot: Slot, key: String, owner: Slot },
}
pub enum CaptureResult { Pending, Bound { advice: Option<Msg> }, Swapped { advice: Option<Msg> }, Cancelled }
pub struct Capture { pub state: CaptureState }
impl Capture {
    pub fn start(slot: Slot) -> Capture;
    pub fn slot(&self) -> Slot;
    pub fn key(&mut self, doc: &mut EditDoc, name: &str, now_ms: u64) -> CaptureResult;   // presses only
    pub fn mouse(&mut self, doc: &mut EditDoc, button: &str, on_cancel_chip: bool) -> CaptureResult;
    pub fn confirm_swap(&mut self, doc: &mut EditDoc) -> CaptureResult;
    pub fn cancel(&mut self) -> CaptureResult;
    pub fn tick(&mut self, now_ms: u64);                                                   // clears an expired Esc arm
}
```

Rules: 2.4 and 4.4. Binding a slot to the key it already has is `Bound`
(no history entry). In `Conflict`, a further key press is ignored except
Enter (swap) and Escape (cancel). A swap that would put a mouse name on
Toggle/Panic is refused with the `MouseNotAllowed` message and returns to
`Listening`.

### 6.8 Text fields (`text.rs`)

```rust
pub struct TextEdit { /* text: String, caret: usize (char index), anchor: Option<usize>, max_chars: usize */ }
impl TextEdit {
    pub fn new(text: &str, max_chars: usize) -> TextEdit;
    pub fn text(&self) -> &str; pub fn caret(&self) -> usize; pub fn selection(&self) -> Option<(usize, usize)>;
    pub fn insert(&mut self, s: &str);            // drops control chars, truncates at max_chars
    pub fn backspace(&mut self); pub fn delete(&mut self);
    pub fn left(&mut self, shift: bool); pub fn right(&mut self, shift: bool);
    pub fn home(&mut self, shift: bool); pub fn end(&mut self, shift: bool);
    pub fn select_all(&mut self);
}
```

Used only for the profile name (100) and control label (40). Enter or a click
elsewhere commits; Esc reverts. While focused the model emits
`ImeAllowed(true)` and consumes every key. Vietnamese arrives as text
(`UiEvent::Text`) from `KeyEvent.text` or `Ime::Commit`; Telex/VNI behaviour
is **unverified** until hardware, which is why no flow requires typing
(auto names, label suggestions).

### 6.9 Messages, key names, advice (`msg.rs`, `keynames.rs`, `lint.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)] pub enum Lang { #[default] Vi, En }
#[derive(Clone, PartialEq, Debug)]
pub enum Msg { /* one variant per user-visible sentence, with typed parameters, e.g. */
    KeyTaken { key: String, owner: String }, KeyUnsupported, EscAgain, MouseNotForToggle,
    PlayOn { toggle: String, panic: String }, PanicDone, FiveFingers, Sensitivity(f64),
    Refusal(PlayRefusal), Problem(ProblemMsg), Advice(Advice), Deleted { name: String }, /* … every string of sections 2-4 … */
}
impl Msg { pub fn text(&self, lang: Lang) -> String; }   // one exhaustive match per language: a missing string is a compile error
pub fn fmt_num(v: f64, decimals: usize, lang: Lang) -> String;   // Vi uses a decimal comma: "1,21"

pub fn key_label(name: &str, lang: Lang) -> String;
pub fn free_key(doc: &EditDoc) -> Option<&'static str>;
pub const LABEL_PRESETS_VI: &[&str];   // Bắn, Ngắm, Nhảy, Ngồi, Nằm, Nạp đạn, Đổi súng, Hồi máu, Lựu đạn, Nhặt đồ, Lên xe

#[derive(Clone, PartialEq, Debug)]
pub enum Advice { ShadowsMirrorly { key: String, action: MirrorlyAction }, WindowsKey, AltKey, LockKey, HelperKey,
                  NearEdge, MoreThanFive { count: usize }, NoLook, OverlapsLookAnchor, OrientationMismatch }
pub struct Lint { pub target: Option<CtrlId>, pub advice: Advice }
pub fn key_advice(name: &str) -> Option<Advice>;
pub fn lints(doc: &EditDoc, env: &Env) -> Vec<Lint>;          // warnings only; never block saving or playing
pub fn explain(problem: &Problem, profile: &Profile) -> ProblemMsg;   // SPEC 4.3 path -> friendly sentence (for broken files)
pub fn explain_json_error(e: &ProfileError) -> ProblemMsg;
```

`key_label` (Vi): `KeyW` -> `W`; `Digit1` -> `1`; `Space` -> `Space`;
`Backquote` -> `` ` ``; `BracketLeft` -> `[`; `ShiftLeft` -> `Shift trái`;
`ControlLeft` -> `Ctrl trái`; `AltLeft` -> `Alt trái`; `ArrowUp` -> `↑`;
`Enter` -> `Enter`; `Escape` -> `Esc`; `MouseLeft` -> `Chuột trái`;
`MouseRight` -> `Chuột phải`; `MouseMiddle` -> `Chuột giữa`; `MouseBack` ->
`Chuột bên (lùi)`; `MouseForward` -> `Chuột bên (tới)`; `NumpadN` ->
`Num N`. Labels are unique and non-empty for every name in `KEY_NAMES` and
`MOUSE_NAMES`.

`free_key` order: `Space, KeyR, KeyC, KeyF, KeyG, KeyQ, KeyE, KeyX, KeyZ,
KeyV, KeyB, Digit1..Digit9, Digit0, ShiftLeft, ControlLeft, Tab, KeyT, KeyY,
KeyH, KeyN, KeyM, KeyU, KeyI, KeyO, KeyP, KeyJ, KeyK, KeyL`, then every other
`KEY_NAMES` entry except F-keys, `Escape`, `Super*`, lock keys and the helper
keys; never a mouse name; never a key already bound. (Implementers must check
each listed name exists in `keys::KEY_NAMES`; a test asserts it.)

Vietnamese validation messages for broken files (`ProblemMsg`):

| SPEC 4.3 finding | Vietnamese |
|---|---|
| JSON error | `Tệp này không đọc được (không phải hồ sơ bàn phím). Tệp gốc vẫn còn nguyên.` |
| `version` | `Hồ sơ này làm bằng bản mới hơn. Hãy cập nhật chương trình.` (value > known) / `Hồ sơ thiếu số phiên bản hợp lệ.` |
| `name` / `app` / `label` too long | `Tên “{…}” quá dài (tối đa {n} ký tự).` |
| `toggle_key` / `panic_key` mouse | `Phím bật/tắt và phím dừng phải là phím trên bàn phím.` |
| `panic_key` = toggle | `Phím bật/tắt và phím dừng khẩn cấp đang trùng nhau.` |
| `controls` count | `Hồ sơ có quá nhiều nút (tối đa 64).` |
| `x` / `y` range | `Nút “{label}” nằm ngoài màn hình.` |
| `radius` range | `Vòng của “{label}” quá to hoặc quá nhỏ.` |
| `reach` range | `Vùng xoay camera quá to hoặc quá nhỏ.` |
| `sensitivity` range | `Độ nhạy chuột nằm ngoài khoảng cho phép.` |
| `release_after_ms` range | `Thời gian tự nhấc ngón quá dài (tối đa 60 giây).` |
| unknown key | `Phím “{key}” chưa được hỗ trợ (ở “{label}”).` |
| duplicate key | `Phím {key} bị dùng 2 lần: “{a}” và “{b}”.` |
| second look | `Có hơn một vùng xoay camera. Chỉ được 1.` |
| unknown field / type / orientation | `Hồ sơ có phần chương trình không hiểu. Có thể nó được làm bằng công cụ khác.` |

A control without a label is named by kind and number (`Nút 3`, `Cần di chuyển`).
A test maps every file in `tests/data/invalid/` (first path from
`expected.json`) to a non-empty message in both languages.

Advice strings (Vi): `ShadowsMirrorly` ->
`{key} cũng là phím tắt {action} của Mirrorly; khi đang chơi nó sẽ là phím game.`;
`LockKey` -> `Phím này bật/tắt đèn trên bàn phím mỗi lần bấm.`; `NoLook` ->
`Hồ sơ chưa có vùng xoay camera: chuột sẽ không xoay được màn hình.`;
`MoreThanFive` -> `Có {n} nút nhưng điện thoại chỉ nhận 5 ngón cùng lúc.`;
`OverlapsLookAnchor` -> `Nút này nằm đè lên tâm vùng xoay camera.`; the rest
appear in sections 3-4.

### 6.10 Scene and layout (`scene.rs`, `layout.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone { Marker, MarkerDim, Selected, Pressed, Ghost, Guide, Hatch, Warn, Danger,
                Card, CardLine, Scrim, Text, TextSoft, Accent, OnAccent, Field, FieldFocus, Disabled, Finger }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Font { Body, Small, Title, Cap, Icon }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Layer { Static, Dynamic }
#[derive(Clone, PartialEq, Debug)]
pub enum Prim {
    Disc { c: Pt, r: f64, tone: Tone },
    Ring { c: Pt, r: f64, width: f64, tone: Tone },
    Rect { r: PxRect, radius: f64, tone: Tone },
    Frame { r: PxRect, radius: f64, width: f64, dashed: bool, tone: Tone },
    Line { a: Pt, b: Pt, width: f64, dashed: bool, tone: Tone },
    Hatch { r: PxRect, tone: Tone },
    Text { r: PxRect, text: String, font: Font, align: Align, tone: Tone },   // clipped/ellipsized to r by the drawer
    /// Filled only when bit `control` of the pressed mask is set (Play / Test highlights).
    PressFill { control: u8, shape: Shape, tone: Tone },
}
pub struct Scene { pub version: u64, pub alpha: f32, pub statics: Vec<Prim>, pub dynamics: Vec<Prim> }
impl Scene { pub fn to_svg(&self, window: (f64, f64), pressed_mask: u64) -> String; }
```

- `statics` change only with `version`; the app may cache them in one bitmap
  (Play). `dynamics` in Play contain only `PressFill` entries (no text).
- The model never measures text. Every `Text` has a fixed box; strings are
  written to fit (a test asserts a per-string character budget for both
  languages, e.g. dock rows <= 34 characters per line at 304 logical px).
- `to_svg` renders the same primitives to SVG with a fixed light palette.
  Golden SVG files for every screen of section 3 are committed under
  `tests/data/menu/golden/` and compared byte-for-byte; they are also how
  reviewers look at the screens on Linux. They prove the layout, **not** the
  Direct2D rendering.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockPlacement { Side /*reserved strip right of the picture*/, CardLeft, CardRight, SheetTop, SheetBottom }
pub fn dock_placement(view: &EditView, focus: Option<Pt> /*selected control or drag point, px*/, grown: bool) -> DockPlacement;
pub fn dock_rect(view: &EditView, placement: DockPlacement, collapsed: bool) -> PxRect;
pub const DOCK_W: f64 = 304.0;  // logical px

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)] pub struct WidgetId(pub u32);
pub struct Widget { pub id: WidgetId, pub rect: PxRect, pub kind: WidgetKind, pub enabled: bool }
pub enum WidgetKind {
    Label(Msg), Note(Msg), Banner { msg: Msg, tone: Tone },
    Button { label: Msg, primary: bool }, Check { label: Msg, on: bool },
    Segmented { options: Vec<Msg>, selected: Option<usize> },
    Slider { value: f64, min: f64, max: f64, log: bool, shown: String },
    KeyCap { slot: Slot, label: String, listening: bool },
    Field { edit: TextEdit, focused: bool, placeholder: Msg },
    Row { title: String, sub: String, selected: bool, broken: bool },
}
pub fn hit_widget(ws: &[Widget], p: Pt) -> Option<(WidgetId, f64 /*0..1 along x*/)>;
pub fn slider_value(min: f64, max: f64, log: bool, t: f64) -> f64;   // inverse: slider_t
```

Fixed metrics (logical px): row 36, gap 8, padding 12, title row 44, cap
height 28, slider track 4 with a 16 px knob. Dock content taller than the
dock scrolls with the wheel. Hysteresis for card flipping: the dock flips
only when the focus point is more than 8 % of the picture width past the
middle, so it does not flicker during a drag along the centre.

### 6.11 Tracker (`tracker.rs`)

```rust
pub struct HeldTracker { /* key -> control index, held keys */ }
impl HeldTracker {
    pub fn new(profile: &Profile) -> HeldTracker;
    pub fn on_key(&mut self, name: &str, pressed: bool) -> bool;   // true if the mask changed
    pub fn clear(&mut self);
    pub fn mask(&self) -> u64;                                     // bit i = control i has a held key (<= 64 controls)
}
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TouchDots { pub dots: [Option<(f64, f64)>; MAX_CONTACTS] }   // picture fractions, index = contact id
pub fn dots_from_frame(prev: TouchDots, frame: &Frame, view: &View) -> TouchDots;   // via Rotation::panel_to_picture
```

### 6.12 Store, prefs, templates (`store.rs`, `prefs.rs`, `templates.rs`)

```rust
pub trait Fs {
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError>;        // file names, sorted
    fn read(&self, path: &str) -> Result<String, FsError>;
    fn write_atomic(&mut self, path: &str, data: &str) -> Result<(), FsError>;   // temp file + rename
    fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError>;
    fn remove(&mut self, path: &str) -> Result<(), FsError>;
    fn exists(&self, path: &str) -> bool;
}
pub struct MemFs { /* BTreeMap<String, String>, fail_next: Option<FsOp> */ }    // tests; failure injection per operation

pub enum Health { Ok, Broken(ProblemMsg) }
pub struct Entry { pub file: String, pub name: String, pub orientation: Option<Orientation>, pub controls: usize, pub health: Health }
pub struct TrashToken(/* private */);
pub enum StoreError { Fs(FsError), Invalid(ProblemMsg), NotFound, NameTaken }

pub struct ProfileStore<F: Fs> { /* fs, dir */ }
impl<F: Fs> ProfileStore<F> {
    pub fn new(fs: F, dir: &str) -> Self;
    pub fn list(&self) -> Vec<Entry>;                                   // *.json only; broken ones included, never removed
    pub fn load(&self, file: &str) -> Result<Profile, StoreError>;
    pub fn save(&mut self, file: &str, p: &Profile) -> Result<(), StoreError>;   // validate; previous -> <file>.bak; write_atomic
    pub fn create(&mut self, tpl: Template) -> Result<String, StoreError>;       // "Hồ sơ N" + slug file name
    pub fn rename(&mut self, file: &str, new_name: &str) -> Result<String, StoreError>;
    pub fn duplicate(&mut self, file: &str) -> Result<String, StoreError>;       // "<name> (bản sao)", unique
    pub fn delete(&mut self, file: &str) -> Result<TrashToken, StoreError>;      // moves to .trash/
    pub fn undelete(&mut self, t: TrashToken) -> Result<String, StoreError>;
    pub fn purge_trash(&mut self);                                               // on start-up
    pub fn import_text(&mut self, text: &str, hint: &str) -> Result<String, StoreError>;  // valid only; never overwrites
    pub fn export_text(&self, file: &str) -> Result<String, StoreError>;         // == Profile::to_json_pretty
    pub fn write_draft(&mut self, file: &str, p: &Profile) -> Result<(), StoreError>;     // .draft/<file>
    pub fn draft(&self, file: &str) -> Option<Profile>; pub fn drop_draft(&mut self, file: &str);
    pub fn drafts(&self) -> Vec<String>;
}
pub fn slugify(name: &str) -> String;   // Vietnamese diacritics and đ/Đ -> ASCII, lowercase, other chars -> '-', no leading/trailing '-' or '.',
                                        // Windows-reserved names (CON, PRN, AUX, NUL, COM1-9, LPT1-9) get a "ho-so-" prefix, <= 48 chars, empty -> "ho-so"
pub fn repair_ranges(text: &str) -> Option<(Profile, Vec<Msg>)>;   // Some only if clamping numbers alone makes the file valid

pub enum Template { ShooterLandscape, BasicLandscape, BasicPortrait, Empty(Orientation) }
pub fn template(t: Template) -> Profile;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]   // NOT deny_unknown_fields: forward compatible
pub struct UiPrefs {
    #[serde(default = "1")] pub version: u32,
    #[serde(default)] pub active: Option<String>,            // file name of the chosen profile
    #[serde(default)] pub overlay: Overlay,                  // "auto" (default) | "always" | "off"
    #[serde(default = "BracketLeft")] pub sens_down_key: String,
    #[serde(default = "BracketRight")] pub sens_up_key: String,
    #[serde(default)] pub notice_ack: u32,
    #[serde(default)] pub guide_done: bool,
    #[serde(default)] pub intro_toast_shown: bool,
    #[serde(default)] pub lang: Lang,                         // "vi" (default) | "en"
    #[serde(default)] pub last_sensitivity_change: Option<(String, f64, f64)>,
}
```

Unreadable or invalid `keymap-ui.json` -> defaults (and `notice_ack = 0`, so
the notice shows again); it is rewritten on the next change.

### 6.13 `Editor` (`editor.rs`)

```rust
pub struct Editor { /* doc, history, selection, drag, capture, focus: Option<WidgetId + TextEdit>, armed: Option<Kind>,
                       guide: Option<Guide>, test: Option<TestRun>, dock_collapsed, hide_held, file */ }
impl Editor {
    pub fn open(file: &str, profile: Profile, guide: bool) -> Result<Editor, ProfileError>;
    pub fn event(&mut self, ev: &UiEvent<'_>, env: &Env, now_ms: u64) -> Vec<EditorOut>;  // Effect | Save | Leave | Changed
    pub fn in_test(&self) -> bool;
    pub fn scene(&self, env: &Env, lang: Lang, now_ms: u64) -> Scene;
    pub fn widgets(&self, env: &Env, lang: Lang) -> Vec<Widget>;
    pub fn dots(&self) -> TouchDots;
    pub fn profile(&self) -> &Profile; pub fn dirty(&self) -> bool;
}
```

`TestRun` holds `Engine::new(doc.profile().clone())`, a `View::from_panel`
built from `Env`, a `HeldTracker` and `TouchDots`; its frames are dropped
after `dots_from_frame`. The model has no way to emit a frame to the app:
no `Effect` carries one. That is the structural guarantee that
Hub/Edit/Test send nothing.

### 6.14 Unit tests (all run with `cargo test -p keymap` on Linux)

Numbering is for review; names follow `menu::<module>::tests::<name>`.

**doc** (1) every mutator on every control kind keeps `problems()` empty;
(2) `add` assigns WASD, then arrows, then IJKL when taken; (3) second look
refused, 65th control refused; (4) `bind` free key / same key / conflict
leaves doc unchanged on conflict; (5) `swap` button-button, button-joystick
direction, control-toggle; mouse name onto toggle refused; (6) clamping of x,
y, radius, reach, sensitivity, release; NaN/inf ignored; (7) label 40 / name
100 truncation on char boundaries with Vietnamese text; (8) `duplicate`
offsets, clamps at the edge, gets fresh keys, look refused; (9) ids stable
across remove; (10) numbers rounded to 4 decimals and
`to_json_pretty` -> `from_json` round-trips; (11) `NoFreeKey` when all keys
are taken (constructed profile).

**history** (12) undo/redo exact equality of profile and ids; (13) redo
cleared by a new edit; (14) cap 100; (15) drag = one step, no-change gesture
= no step; (16) wheel burst 399 ms coalesces, 401 ms splits; nudge 500 ms;
(17) selection restored; (18) `dirty` / `mark_saved`.

**hit** (19) handle beats cap beats body; (20) smallest body first (button
inside joystick ring); (21) look interior is click-through, border and
anchor hit; (22) minimum button hit radius at scale 1.0 / 1.5 / 2.0;
(23) `to_norm`/`to_px` round trip for 19.5:9, 9:19.5 and a letterboxed
picture offset; (24) ring radius px = `radius * min(w, h)` and agrees with
`View::short_side_units`.

**drag/snap** (25) click below threshold selects only; (26) move keeps the
grab offset; (27) clamp at all four edges; (28) Shift axis lock, Alt no
snap; (29) snap at 6 px, not at 7 px, guides returned only when snapped;
(30) resize joystick/look values and clamps; (31) Esc restores; (32) palette
placement adds one control in one undo step, Esc adds nothing.

**capture** (33) press binds, release ignored; (34) unknown name keeps
listening with the message; (35) mouse binds for button and direction slots,
refused for toggle/panic; (36) click on the cancel chip cancels;
(37) Esc arm: second Esc at 1499 ms binds Escape, at 1501 ms re-arms;
(38) conflict -> swap -> no duplicates; conflict -> cancel -> unchanged;
(39) Tab, Space, F1-F12, Backquote are bindable (Backquote = conflict with
toggle -> swap); (40) advice returned for F-keys, Super, Alt, lock and
helper keys.

**text** (41) insert/backspace/delete with precomposed and combining
Vietnamese; (42) caret and selection movement; (43) max length; (44) control
characters dropped.

**msg/keynames/lint** (45) `Msg::text` non-empty for every variant in both
languages (a `Msg::all_samples()` test helper enumerates variants);
(46) character budgets; (47) `key_label` unique and non-empty for all
`KEY_NAMES` + `MOUSE_NAMES`; (48) every name in the `free_key` order is a
known key; `free_key` skips bound, helper and excluded keys; (49) `explain`
for every `tests/data/invalid/*.json`; (50) lints: edge band, more than
five, no look, F-key shadow, orientation mismatch, overlap with look anchor;
(51) `fmt_num` decimal comma.

**layout/scene** (52) dock placement for selection left/right/top/bottom,
portrait sheet, collapsed, hysteresis; dock never covers the selected
control's body when the picture is at least 2 x `DOCK_W` wide; (53) widgets
do not overlap and stay inside the dock; (54) `hit_widget`, slider
log mapping round trip, stepper bounds; (55) golden SVG per screen: Hub
(empty, 3 rows with one broken, refusal), template picker, notice (box
unticked/ticked), Edit (nothing / button / joystick / look selected, card
left/right, side, portrait sheet, capture listening, conflict), guide steps
1-6, Test with 2 contacts, Play overlay with a pressed mask, stuck-help
card; (56) Play scene: `dynamics` contain only `PressFill`; `scene()` is
`None` for overlay `off` and for `auto` after 5 s.

**tracker** (57) mask bits for button, joystick (any direction), repeated
press, clear; (58) `dots_from_frame` equals engine contacts mapped by
`panel_to_picture` for R0/R90/R180/R270, lifts clear the dot.

**store/prefs/templates** (59) create numbering, slug collisions get `-2`;
(60) `slugify`: `Hồ sơ Ngang` -> `ho-so-ngang`, `Đấu trường` ->
`dau-truong`, `CON`, trailing dots, empty, very long; (61) save writes
`.bak`, failed `write_atomic` leaves the old file (failure injection on every
`FsOp`); (62) rename updates name and file; (63) delete -> undelete restores
bytes; purge; (64) broken files listed with `Health::Broken`, never removed;
(65) `import_text` of every `tests/data/valid/*.json` succeeds, of every
`invalid/*.json` fails with a message and writes nothing; never overwrites
an existing file; (66) `export_text` equals `to_json_pretty` and re-imports;
(67) drafts write/read/drop; (68) `repair_ranges` only for pure range
problems, result validates, report lists each change; (69) prefs: defaults,
unknown fields tolerated, garbage -> defaults; (70) every template validates,
keeps 0.06 from the edges, and a scripted `Engine` run on it does not panic.

**guide** (71) each step's auto-complete predicate; skip; restart; empty-
template variant.

**shell** (72) every row of the transition table; (73) every `PlayRefusal`
from `Env`; (74) notice gates the first Play only and again after a version
bump; (75) toggle key in Edit goes to Test only without capture/focus;
(76) F8 in each mode; (77) focus loss in each mode; (78) `PlayEnded` with a
changed sensitivity saves it and records the undo offer; (79) `play_key`
shadowing rule; (80) Hub delete + undo within 8 s, expiry after; (81) draft
recovery offer; (82) `swallows_input` per mode; (83) no `Effect` variant can
carry a `Frame` (compile-time: the enum has none) and `Shell` never returns
`StartPlay` from Edit/Test.

**fuzz** (84) `fuzz_editor`: LCG, 20 seeds x 3000 random pointer / key /
text / wheel / tick events in Edit and Test; after every event: profile
valid, ids unique, selection exists, no panic; at the end undo-all returns
the opening profile and redo-all the final one. (85) `fuzz_shell`: 20 seeds
x 2000 events over `MemFs` with random injected failures: no panic, store
never loses a valid file.

**integration, still pure** (86) "pixel placed = HID sent": drag a button so
its centre is at window pixel P; build `Engine` from the result with a
`View` for the same picture; a press produces a contact equal to
`view.to_hid(to_norm(P))` (R0 and R90, with a letterboxed picture);
(87) the same for a joystick: centre and `centre + dir * radius`;
(88) a profile produced through a scripted Editor session passes
`keymap-sim validate` semantics (`Profile::from_json(to_json_pretty)`).

## 7. Mirrorly integration

All line numbers refer to upstream `ac70daa` and were read, not compiled.

### 7.1 Files and functions

| File | Existing anchor | Change |
|---|---|---|
| `crates/app/src/keymap_ui.rs` (**new**, target <= 450 lines) | – | `StdFs` (impl `keymap::menu::Fs` on `std::fs`, `write_atomic` = write `*.tmp` then `rename`); `fn convert_*` winit event -> `UiEvent`; `fn popup(hwnd, &MenuSpec) -> Option<u32>` (same `CreatePopupMenu`/`AppendMenuW`/`TrackPopupMenu` calls as `settings.rs`, plus `MF_POPUP` for `Sub`); `fn run_effects(app, Vec<Effect>)`; `fn env(app) -> Env` |
| `main.rs` | `mod` list | `mod keymap_ui;` |
| `main.rs` | `struct Args` | `#[arg(long)] no_keymap: bool`; `--keymap FILE` unchanged |
| `main.rs` | SPEC 7.4 `KeymapHost` | gains `shell: Option<Shell<StdFs>>` (None under `--keymap FILE`), `tracker: HeldTracker`, `frame_version: u64`. `App.keymap` is `None` only with `--no-keymap` |
| `main.rs` | SPEC 7.4 activation gate | first `shell.can_play(&env)`; a refusal toasts `Msg::Refusal` and returns. The four SPEC checks stay as the authority behind it. On `Activated`: `shell.event(PlayStarted)`; on `Deactivated`/`Panic`/stop paths: `shell.event(PlayEnded{..})` |
| `main.rs` | SPEC 7.4 `keymap_load(path)` | when a shell exists the profile comes from `shell.active_profile()` instead of a path |
| `main.rs` | `window_event`, before SPEC 7.4's wiring | if `shell.swallows_input()`: `CursorMoved`, `MouseInput`, `MouseWheel`, `KeyboardInput`, `Ime`, `DroppedFile` (`.json`, Hub only: read text, `UiEvent::Dropped`), `Focused(false)` are converted, sent to `shell.event`, effects run, and the function returns **before** the gesture path, the right-click Home (`main.rs:1605`), the pickers and typing-to-phone. Exception: F-keys per table 4.1 fall through to the upstream shortcut `match` (~1608-1629) when the shell reports them unhandled |
| `main.rs` | shortcut `match` ~1608 | `KeyK` with Ctrl in Off/Hub -> `Command::ToggleHub` |
| `main.rs` | `device_event` (added by SPEC 7.4) | in Test: `UiEvent::RawMotion` |
| `main.rs` | `keymap_input` (SPEC 7.4), Play | before the engine: `match shell.play_key(name)`; `SensDown/Up` on press -> `engine.set_look_tuning`, toast `Msg::Sensitivity`; consumed. After `engine.handle`: `tracker.on_key`; if the mask changed and the overlay is drawn -> `shared.keymap_mask.store(..)` + redraw request. `stats().dropped_presses` growth -> toast `Msg::FiveFingers` |
| `main.rs` | `start_probe()` (1119) | return early with a log line when `shell.mode() != Off` (SPEC already covers Play) |
| `main.rs` | `about_to_wait` | `shell.event(Tick)` when `shell.next_deadline()` is due; add that deadline to the `WaitUntil` minimum |
| `main.rs` | `show_settings()` (674) | pass `shell.native_menu()`; a returned keymap id -> `shell.event(MenuPick/Command)` |
| `main.rs` | `set_title()` | suffix from `shell.title_suffix()` |
| `main.rs` | `rotate()`, `Connected`/`Disconnected`, resize, DPI change | `shell.event(EnvChanged)` |
| `main.rs` | after each `shell.event` | if `shell.scene_version() != frame_version`: build `shell.scene()`, store `chrome.keymap = scene.map(Arc::new)`, store dots, request a redraw |
| `settings.rs` | `show()` | new parameter `keymap: Option<&MenuItem>`; appends a separator and the submenu; `Choice` gains `Keymap(u32)`; ids start at 1000 |
| `layout.rs` | `enum Control` (16) | variant `Keymap` |
| `layout.rs` | `toolbar()` `wanted` array (88) | `Control::Keymap` inserted **before** `Control::Settings` (icons that do not fit already drop from the right) |
| `bar.rs` | glyph table (167-178), icon `match` (~515), tooltip/click dispatch | `GLYPH_KEYMAP = "\u{E765}"` (Segoe Fluent keyboard glyph; check it renders on the target Windows), click -> `Command::ToggleHub` |
| `state.rs` | `struct Chrome` (106) | `pub keymap: Option<Arc<keymap::menu::Scene>>` |
| `state.rs` | `struct Shared` (131) | `pub keymap_mask: AtomicU64`, `pub keymap_dots: Mutex<keymap::menu::TouchDots>` |
| `state.rs` | `struct View` (22), `mapper()` (71) | WP9 only: `pub reserved_right: f64`; `fit(window.0 - reserved_right - 2b, …)` |
| `main.rs` | `apply_size()` (427) | WP9 only: add `reserved_right` to the requested inner width; skip growth when the result exceeds the monitor work area (then `Env.can_grow_window = false` -> card layout) |
| `present.rs` | chrome snapshot (~178-186), `UiFrame` build (~187) | clone `c.keymap` (an `Arc` clone), read `keymap_mask`; `UiFrame { keymap: keymap.as_deref(), keymap_mask, .. }` |
| `present.rs` | dots vector (~130) | append `keymap_dots` (picture fraction -> back-buffer px through the already computed mapper view), colour = theme accent, radius 14 logical px |
| `present.rs` | around `u.draw` (~206-215) | time the keymap part with `now_us()` and hand it to `metrics` (8.2) |
| `ui.rs` | `struct UiFrame` (64) | `pub keymap: Option<&'a keymap::menu::Scene>`, `pub keymap_mask: u64` |
| `ui.rs` | `is_empty()` (86) | `&& self.keymap.is_none()` |
| `ui.rs` | `Ui::draw` (198) | after the existing chrome and before the toast: `self.keymap(scene, mask)` |
| `ui.rs` | new `fn keymap(&mut self, s: &Scene, mask: u64)` | one `match` over `Prim` using the existing `color()`/`text()` helpers and text formats; `Tone` -> `Theme` colour; static-layer cache (8.1) |
| `theme.rs` | `struct Theme` (12), `DARK` (69), `LIGHT` (93) | new colours: `km_marker`, `km_pressed`, `km_guide`, `km_warn`, `km_danger`, `km_scrim`, `km_card`, `km_finger`; other tones map to existing fields (`text`, `text_soft`, `accent`, `on_accent`, `row`, `row_hover`, `outline`) |
| `metrics.rs` | `text()` | one extra line when the overlay was drawn (8.2) |

Platform effects (`run_effects`): `Toast` -> `self.toast(msg.text(lang), secs)`
(904); `Cursor` -> `window.set_cursor`; `GrabPointer` -> same calls as SPEC
7.4's activation; `ShowMenu` -> `popup`; `CopyText` -> `winclip::write_text`
(357); `RequestPaste` -> `winclip::read` (129), `WinClip::Text` only ->
`UiEvent::Pasted`; `OpenFolder` -> `std::process::Command::new("explorer")`
with the keymaps directory (no new `windows` features); `ImeAllowed` ->
`window.set_ime_allowed`; `Rotate` -> `self.rotate(true)` (1090);
`ReserveRight` -> set `View.reserved_right`, `apply_size()` (no-op before
WP9); `Title` -> `set_title()`.

No file dialog, no URL opening, no new window class, no new crate feature is
planned; if the first Windows CI build shows a missing `windows` feature for
`MF_POPUP` or IME events, that is fixed in `crates/app/Cargo.toml` and noted
in `BAO_CAO.md`.

### 7.2 Threads and hand-off

```
winit main thread                                   present thread
-----------------                                   --------------
WindowEvent -> keymap_ui::convert -> Shell::event   loop:
   -> Vec<Effect> -> run_effects                      lock chrome: clone Arc<Scene> (cheap)
   -> if scene_version changed:                       load keymap_mask (atomic)
        chrome.keymap = Some(Arc::new(scene))         lock keymap_dots: copy 5 dots
        keymap_dots = shell.dots()                    r.draw(&scene)        (dots via Scene.dots)
        request redraw                                if !ui_frame.is_empty(): u.draw(..)
Play: engine.handle -> send (SPEC path)                   -> Ui::keymap(scene, mask)
      tracker mask -> keymap_mask.store
```

- The model runs only on the main thread. The present thread reads an
  immutable `Arc<Scene>`, one atomic and five dots; it runs no model code.
- Test-mode dots change on every mouse motion but do **not** rebuild the
  scene: only `keymap_dots` is updated and a redraw requested.
- In Play nothing of the menu runs per input except `play_key` (two string
  compares) and `HeldTracker::on_key` (one map lookup).

### 7.3 Storage

Location: `device::paths::data_dir()` = `%LOCALAPPDATA%\Mirrorly`
(`crates/device/src/paths.rs:9`), the directory upstream already uses for its
logs, pairings and the `onboarded` marker. (The task brief said `%APPDATA%`;
upstream's convention is `%LOCALAPPDATA%`, and one location is better than
two. On the user's PC this is
`C:\Users\<tên>\AppData\Local\Mirrorly\keymaps`; `Mở thư mục hồ sơ` opens it.)

```
%LOCALAPPDATA%\Mirrorly\
  keymap-ui.json                 UiPrefs (6.12)
  keymaps\
    ho-so-1.json                 profile, exactly SPEC 4.1 (v1; v2 only after WP10)
    ho-so-1.json.bak             previous saved version (one level)
    .draft\ho-so-1.json          crash-safe draft, removed on Xong / discard
    .trash\ho-so-2.json          deleted profiles; purged at start-up
```

Every write is temp file + rename. A failed write shows
`Không lưu được hồ sơ. Kiểm tra ổ đĩa rồi thử lại.` and keeps the draft in
memory; the old file is untouched.

### 7.4 Compatibility with the browser editor

- Profile files are exactly the schema of SPEC 4.1: no extra fields (the
  struct is `deny_unknown_fields`), numbers rounded to 4 decimals, written by
  `Profile::to_json_pretty`. UI-only data never enters a profile.
- A file exported by `editor/index.html` works when dropped on the Hub,
  pasted with `Dán hồ sơ…`, or copied into the `keymaps` folder. A profile
  made in the app opens in the browser editor through `Sao chép hồ sơ…` +
  paste, or from the folder.
- Both editors share the vectors in `crates/keymap/tests/data/` (store test
  65) and the capture rules (SPEC 10.4: Esc twice, mouse names not for
  toggle/panic).
- Difference kept on purpose: the browser editor's duplicate clears keys and
  blocks export while invalid; the app assigns fresh free keys. Both always
  write valid files.
- The browser editor becomes the secondary tool (works without Windows and
  without a phone). It is not changed until WP10, where it learns schema v2
  together with the Rust side.

### 7.5 Patch and CI

- `integration/mirrorly-keymap.patch` grows to cover the files of 7.1; the
  new file is included with `git add -N` before `git diff`. It must still
  pass `git apply --check` on a fresh `ac70daa` checkout.
- The SPEC's CI job stays; with `--keymap FILE` the SPEC 7.4 behaviour and
  its tests are unchanged.
- The pure tests run in both workspaces (ours and inside Mirrorly's).

## 8. Latency and safety rules

### 8.1 Latency rules (hard)

1. **No extra window in Play, ever.** Upstream measured that a window
   covering part of the mirror forces DWM composition (+8-10 ms in
   `LATENCY.md`; `apply_shape`'s comment at `main.rs` ~448 records +9.7 ms for
   a clip region). All menu surfaces are drawn in the existing swapchain
   pass. Native popups are used only in Off/Hub/Edit, where touches are off.
2. **Nothing to draw = upstream cost.** In Play with overlay `Tắt`, and with
   `Tự ẩn` after 5 s, `chrome.keymap` is `None`; with no toast or stats
   `UiFrame::is_empty()` is true and the Direct2D pass is skipped
   (`present.rs:206`), as upstream.
3. **`Luôn hiện` is a cached bitmap.** `Scene.statics` are rendered once into
   an `ID2D1Bitmap1` when `Scene.version`, the picture rectangle, the theme
   or the DPI changes; each frame draws that bitmap once plus the
   `PressFill` shapes whose mask bit is set (no text, no layout, no
   allocation). DirectWrite is used only when the cache is rebuilt.
4. **Mouse motion never redraws in Play.** A redraw is requested only when
   the pressed mask changes and the overlay is visible.
5. **No menu code on the Play input path** beyond 7.2's two cheap calls. The
   engine path (handle, then send) is SPEC's, unchanged.
6. Window growth (`ReserveRight`) happens only on entering/leaving Edit,
   never in Play; leaving Edit restores the size before Play can start.
7. Edit and Test may cost whatever they cost: no touch is being sent.

### 8.2 Measurement (nothing is measured yet)

- `present.rs` times `Ui::keymap` with `now_us()`; `metrics.text()` shows
  `keymap overlay: p50 … / p95 … µs` in the F10 stats whenever the overlay
  was drawn during the window.
- On hardware: record Present-to-scan-out percentiles with upstream's
  `--timings-csv` procedure for overlay `Tắt` vs `Luôn hiện`, same scene,
  F10 hidden during the run. F9 cannot be used while the keymap is on.
- Until those numbers exist, UI and docs say `Chưa đo` and make no claim.
  If `Luôn hiện` shows a visible cost, the option keeps its warning text
  with the measured figure.

### 8.3 Safety rules

1. Hub, Edit and Test cannot emit frames (6.13). The real engine is fed only
   in Play.
2. Play requires: accepted notice, USB link, connected, no display mode, no
   running scroll script, matching orientation (SPEC 7.4 gate stays in the
   app as the authority; `can_play` only explains).
3. F9's automated taps are blocked in every mode except Off (4.1).
4. F8 works in every mode and is printed wherever touches can start. Focus
   loss, disconnect and a breaking rotation stop Play (SPEC).
5. Leaving Edit or Test clears every dry-run contact; entering Play calls
   upstream `release_all()` first (SPEC 0.2).
6. Files: atomic writes, one `.bak`, drafts, trash; broken files are never
   deleted or silently changed.
7. The notice cannot be skipped for the first Play; its text is versioned.
8. No feature, field or widget exists for repetition, delay, sequence,
   combo, latch, aim or recoil help, or randomising (1.3). Review of every
   work package checks this line.
9. Wording: never "an toàn", "không bị phát hiện", "được phép". Multi-touch
   on a real iPhone is labelled unverified in the notice.

### 8.4 Open risks

| Risk | Status | Mitigation |
|---|---|---|
| Direct2D code, event wiring, popup helper | uncompiled | thin `match`; first compile on Windows CI (WP5) |
| Overlay cost | unmeasured | default `Tự ẩn`; F10 line; 8.2 |
| Window growth (`reserved_right`) | unverified | card layout ships first and stays as fallback |
| Vietnamese typing (Telex/VNI via IME) | unverified | no flow needs typing |
| Segoe Fluent glyph, Vietnamese stacked diacritics in "Segoe UI Variable" | unverified | check on the user's Windows; fallback font Segoe UI |
| Freeze frame | not in v1 | later, by copying the frame into an app-owned texture (never holding a decoder surface) |
| Template positions | guesses | labelled; the guide makes the user align them |
| Garena's rules | not verified by us | hedged notice |
| Multi-touch on iPhone | unverified (SPEC 1) | Test mode needs no phone; hardware checklist in WP11 |

## 9. Work packages

Order matters; each package ends with the listed acceptance tests green and
`cargo clippy -- -D warnings` clean. WP1-WP4 touch only new files under
`crates/keymap/src/menu/` and `crates/keymap/tests/` (plus the one-line
`pub mod menu;` in `lib.rs`, which must be coordinated with the SPEC.md team
because they own `lib.rs` right now). All packages use the SPEC API as it
is: `Profile`, `Control`, `Engine`, `View`, `keys::*`. None rewrites it.

| WP | Scope | Depends on | Acceptance |
|---|---|---|---|
| **WP1 Core document** | `mod.rs`, `msg.rs`, `keynames.rs`, `doc.rs`, `history.rs`, `capture.rs`, `text.rs`, `templates.rs` | SPEC crate builds | tests 1-18, 33-48, 51, 70 |
| **WP2 Geometry** | `hit.rs`, `drag.rs`, `snap.rs`, `lint.rs`, `tracker.rs` | WP1 | tests 19-32, 49-50, 57-58, 86-87 |
| **WP3 Store** | `store.rs` (`Fs`, `MemFs`, `ProfileStore`, `slugify`, `repair_ranges`), `prefs.rs` | WP1 | tests 59-69 |
| **WP4 Screens** | `scene.rs` (+ `to_svg`), `layout.rs`, `guide.rs`, `editor.rs`, `shell.rs`; golden SVGs reviewed by a human | WP1-3 | tests 52-56, 71-85, 88; goldens committed |
| **WP5 App: Hub + Play** | A1-A4, A6 approved. `keymap_ui.rs`, host always on (`--no-keymap`), `Chrome.keymap`, `UiFrame.keymap`, `Ui::keymap`, theme colours, bar button, native submenu, Ctrl+K, Hub, notice, refusals, Play via the SPEC path, toast texts, F9 gating, title | WP4, SPEC patch merged | patch applies with `git apply --check`; Windows CI compiles and upstream + SPEC tests pass; `--keymap FILE` and `--no-keymap` paths unchanged (review); manual checklist written for the user |
| **WP6 App: Edit + Test** | event forwarding for pointer/keys/wheel, capture routing, popup helper, `keymap_dots` -> `Scene.dots`, pointer grab in Test, card layout | WP5 | Windows CI compiles; review confirms `Ui::keymap` and `convert_*` contain no state decisions; no code path from Edit/Test reaches `self.send` |
| **WP7 Play overlay + tuning** | static-layer bitmap cache, `keymap_mask`, `Tự ẩn` timer, F10 cost line; `Engine::set_look_tuning` (A5) + `[` `]` | WP6; engine team done with `engine.rs` | 5.1 engine tests; tests 56, 78-79; CI compiles; measurement procedure documented (numbers only after hardware) |
| **WP8 Text, share, recovery** | text field + IME events, clipboard copy/paste, `.json` drop on the Hub, draft recovery prompt, `Sửa giúp` | WP6 | tests 41-44, 65-68, 81; CI compiles |
| **WP9 Window growth** | `View.reserved_right`, `mapper()`, `apply_size()`, side layout; optional freeze frame by texture copy | WP6 | test 52 (`Side`); CI compiles; marked unverified until a hardware session confirms touch mapping still matches (test 86 covers the model side only) |
| **WP10 Schema v2** | 5.3: `y_ratio`, `freelook`, optionally `recenter: handoff` (only with the lead's decision); version rule; SPEC.md update; vectors; `keymap-sim`; browser editor; dock UI | SPEC.md team finished; WP4 | new engine/profile tests for each rule of 5.3; v1 vectors unchanged; writers emit v1 when no v2 feature is used; editor Playwright test extended |
| **WP11 User docs + hardware checklist** | Vietnamese `README` section and `BAO_CAO.md` update: what was run, what is unverified; a step-by-step checklist for the user (touch-test app first: 1-5 fingers, joystick, camera re-center, tap vs hold, F8, focus loss, unplug, rotation; overlay cost measurement) | WP5+ | docs reviewed; no claim without a cited run |

### Implementation notes (WP1, 2026-11-26)

Where the spec was ambiguous or silent, and what the code does. All of it is
additive; nothing in SPEC.md or the existing public API changed.

1. **Where shared types live.** `CtrlId`, `Pt`, `PxRect`, `Dir`, `Mods` and
   the constants of 6.1 are in `menu/mod.rs`. `EditView` (and its methods)
   belongs to `hit.rs` and arrives with WP2.
2. **Types that 6.9 puts in `lint.rs` / 6.2 puts in `shell.rs`** but that
   `Msg` and `capture.rs` need now are defined in `msg.rs`: `Advice`,
   `MirrorlyAction`, `ProblemMsg`, `PlayRefusal` (plus a small `Fix` enum for
   the fix button of 3.8). `key_advice` is in `keynames.rs`. WP2/WP4 re-export
   or use them; they must not define second copies.
3. **`Advice::OrientationMismatch` and `PlayRefusal::Orientation`** carry the
   profile's orientation (`OrientationMismatch { profile }`), because the text
   names both orientations; the picture's is the other one by definition.
4. **`Msg` coverage.** One variant per sentence of sections 2-4 and 6.9, plus
   `Msg::PlayOff` (`Đã tắt bàn phím.`, an invented text for the toast SPEC 7.4
   shows when Play ends) and `Msg::Slot*` / `Msg::ControlName` for names.
   Button captions of later screens (Hub buttons, dock labels) are not in
   `Msg` yet; WP4 adds variants as it draws them. `Msg::all_samples()` is a
   public, non-test function so later modules can reuse it; a test keeps it
   complete (an exhaustive `match` over variants).
5. **Character budgets (test 46)** are `Msg::budget()`: 110 chars for toasts
   and short lines, 150 for advice/problems/long toasts, 200 for guide steps,
   260 for notice bullets; checked in both languages.
6. **Control display names.** An unlabeled control is `Nút N` (N = position
   among buttons), `Cần di chuyển` (with ` N` only if there is more than one
   joystick), `Xoay camera`. `EditDoc::display_name` / `slot_name` build them;
   `Msg::KeyTaken.owner` is that string, so `Capture::message(doc, lang)`
   builds the conflict sentence.
7. **`KeyPreset`** has only `Wasd` and `Arrows`, as specified. The IJKL and
   TFGH sets are used internally by `add`/`duplicate` only.
8. **`EditDoc` details.** Ids are `1..` at load and never reused in a session
   (`next` is part of the cloned state, so undo restores it). `move_to` ignores a
   non-finite coordinate per axis. `duplicate` copies the label unchanged.
   A bind on a slot whose control has the wrong kind is `WrongKind`; on a
   missing control `NoSuchControl`. `from_profile` does not round existing
   numbers (so load -> undo-all is exact); only mutators round.
9. **History.** `begin` pushes the snapshot immediately and `end` drops it if
   the document is unchanged (restoring the redo stack). Coalescing windows
   slide: each event is compared with the previous one (`<= 400 / 500 / 1000`
   ms). `Drag` and `Slider(n)` coalesce only between `begin` and `end`.
   `mark_saved` ends coalescing so edits after a save are a new step. `undo`
   returns the selection from before the step; `redo` returns that same
   selection (the selection after the step is not known to `History`); an id
   that no longer exists is returned as `None`. `History::new` ignores its doc
   argument.
10. **Capture.** `key()` with a mouse name delegates to `mouse()`. A second Esc
    binds when `now <= first + 1500` (1499 binds, 1501 re-arms). In a conflict
    prompt Enter and NumpadEnter swap, Esc cancels, other keys are ignored,
    and a click on the `Huỷ` chip cancels in any state. `Unchanged` is reported
    as `Bound` (the editor's `History::end` then drops the empty step).
    Advice for `F9` is the latency-probe text although F9 is blocked in Play
    anyway; F8 has none.
11. **`free_key`.** Besides the documented order it excludes F-keys, Escape,
    `Super*`, `CapsLock`/`NumLock`/`ScrollLock` and the helper keys. `AltLeft`
    and the other remaining names are eligible in the fallback pass, as the
    spec says. `free_key_excluding` is the same with an extra skip list.
12. **Templates.** `template()` sets `name` to the template label; the store
    (WP3) renames it to `Hồ sơ N`. `Template::Empty(o)` keeps the default
    toggle/panic keys. Basic landscape/portrait button labels are empty (shown
    as `Nút N`); portrait positions are our own choice (spec gives none).
    The 0.06 edge rule is tested on control centres; ring and camera-box
    extents are tested to stay inside the picture for a 19.5:9 phone.
13. **`TextEdit`** works on `char`s, not grapheme clusters: backspace on a
    decomposed `ệ` removes one combining mark at a time; precomposed letters
    are one char. `new` also drops control characters and truncates.
14. **Edition.** The workspace is on edition 2024; the new code avoids
    let-chains so it also builds under older editions.

Cut line if time runs out: WP1-WP6 are the usable product (Hub, guided
editing on the live picture, Test, Play with toast only and overlay `Tắt`).
WP7-WP9 improve it; WP10 is v1.1.

Definition of done for the whole feature: section 6.14's tests pass on
Linux; Windows CI builds the patched app; the golden screens were looked at
by a person; every statement in user docs about Windows, an iPhone, a game or
latency is either backed by a cited run or labelled `chưa kiểm chứng`.
