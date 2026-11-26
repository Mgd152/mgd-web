# Báo cáo build và kiểm thử — iOS keymapping cho Mirrorly

Ngày: 2026-11-26. Môi trường chạy: container Linux (không có Windows, không có
PowerShell, không có iPhone). Rust 1.97.0 / cargo 1.97.0, Node v22.23.5,
Chromium qua Playwright. Mọi mục "Đã chạy" dưới đây là lệnh tôi đã chạy trong
phiên này, kèm tóm tắt kết quả thật. Mục nào không có trong "Đã chạy" thì chưa
được kiểm chứng.

## 1. Tóm tắt

| Hạng mục | Trạng thái |
|---|---|
| Crate `keymap` (hình học, hồ sơ, engine, `keymap-sim`) | Đã build và test trên Linux; cross-build `keymap-sim.exe` (Windows GNU) thành công, **chưa chạy** exe đó |
| Trình soạn `editor/index.html` | Đã kiểm thử bằng Chromium headless (139/139 kiểm tra) |
| Hồ sơ mẫu `profiles/*.json` | Đã validate và mô phỏng bằng `keymap-sim` |
| Bản vá Mirrorly | `git apply --check` và `git apply` thành công trên bản sao upstream. Trên GitHub Actions `windows-latest` (lần chạy #1, commit 9cb9931): biên dịch release `app` và `keymap-sim` thành công, `cargo test -p keymap -p input -p device` đạt. Bản vá được sửa thêm một cảnh báo `was_active` (giữ nguyên hành vi), chưa biên dịch lại |
| `apply-to-mirrorly.ps1`, workflow CI | Lần chạy #1: job Linux xanh; job Windows xanh tới bước build, **lỗi ở bước đóng gói** (`expected exactly one avcodec DLL in ...\target\release, found 0`), bước upload artifact bị bỏ qua. Script đã được sửa trong thay đổi này (lấy DLL từ `third_party\ffmpeg\bin` và kiểm SHA-256); **chưa chạy lại**, nên chưa biết đóng gói và tải artifact có hoạt động không |
| Chạy trên Windows thật / iPhone | **Chưa có gì được chạy** trên máy Windows thật hay iPhone |
| Multi-touch trên iPhone | **Chưa kiểm chứng**, cả upstream lẫn chúng tôi chưa từng thử |
| Độ trễ | **Chưa đo** |

## 2. Đã chạy trong container (lệnh và kết quả)

Thư mục làm việc: `ios-keymapping/`.

1. `cargo test`
   - 69 test đơn vị (`src/lib.rs`, gồm `engine::tests::fuzz_invariants`) pass.
   - `tests/samples.rs`: 1 pass (`sample_profiles_validate_and_match_their_names`;
     lúc này `profiles/` đã tồn tại nên test thực sự chạy, không bỏ qua).
   - `tests/sim_cli.rs`: 8 pass. `tests/vectors.rs`: 2 pass.
   - Tổng 80 test, 0 fail, 0 ignored. Doc-test: 0.
2. `cargo test fuzz`: `engine::tests::fuzz_invariants ... ok` (24 seed x 5000 bước,
   xem `crates/keymap/src/engine.rs`, `fuzz_invariants`: `for seed in 0..24u64`,
   `for step in 0..5000`).
3. `cargo clippy --all-targets -- -D warnings`: kết thúc `Finished`, không cảnh
   báo (chạy lại sau khi `touch` để buộc kiểm tra lại).
4. `cargo build --release` (Linux): thành công.
5. `cargo build --release --target x86_64-pc-windows-gnu --bin keymap-sim`: biên
   dịch thành công, tạo `target/x86_64-pc-windows-gnu/release/keymap-sim.exe`
   (1.853.916 byte). Chưa chạy exe này (không có Windows/Wine được thử).
6. `keymap-sim keys | wc -l` = 121 (116 phím bàn phím + 5 nút chuột).
7. `keymap-sim validate` trên hai hồ sơ mẫu, exit 0:
   - `OK: Touch test (portrait) (portrait, 6 controls)`
   - `OK: Touch test (landscape) (landscape, 6 controls)`
8. `keymap-sim simulate` mỗi hồ sơ với script trong `profiles/` (exit 0):
   - Portrait (`touch-test-portrait.sim.txt`): `# frames=19 dropped_presses=1
     look_recenters=1 active=false contacts_down=0`. Quan sát được: bật bằng
     Backquote; joystick W+D tạo ngón 0 ở tâm rồi trượt; look tạo ngón 1;
     Space, MouseLeft, KeyK (hold) tạo ngón 2, 3, 4 (đủ 5 ngón kéo dài); KeyJ
     (tap) là lần nhấn thứ 6 nên bị bỏ qua và đếm (`dropped_presses=1`); nhả lần
     lượt từng ngón; `move 500 0` vượt `reach` gây nhả rồi đặt lại ở tâm (1
     re-center); sau đó KeyJ chạm rồi nhả ở mốc 40 ms; F8 in `# panic` và không
     còn ngón nào.
   - Landscape (`touch-test-landscape.sim.txt`, mặc định xoay R90):
     `# frames=9 dropped_presses=0 look_recenters=0 active=false
     contacts_down=0`; joystick, look, MouseLeft, KeyK (hold) và KeyJ (tap) cho 5
     ngón; ngón tap nhả ở mốc 40 ms; `focus-lost` nhả cả bốn ngón còn lại
     trong một frame (`100 T 0:U 1:U 2:U 3:U`) rồi `# deactivated`.
   Đây là mô phỏng logic, không phải thử trên điện thoại.
9. Trình soạn: `node editor/tests/editor.test.mjs` (Chromium headless, `file://`,
   không mạng): `139/139 checks passed`, exit 0. Gồm: tạo từng loại điều khiển,
   kéo thả, gán phím (kể cả Tab/Space/chuột), xuất rồi `keymap-sim validate`,
   nhập hai hồ sơ mẫu và mọi vector hợp lệ/không hợp lệ (path lỗi đầu tiên phải
   khớp `expected.json`), không request mạng, không lỗi console. Trước khi `profiles/` có đủ file, bản chạy của reviewer cho 132/133; nay đủ file
   nên 139/139.
10. Bản vá: sao chép bản clone upstream (chỉ đọc) vào thư mục tạm, xác nhận
    `HEAD = ac70daa58ee5f24a067e80b6d0ffbcec4c735f1e`, chạy
    `git apply --check integration/mirrorly-keymap.patch` (in `CHECK_OK`) rồi
    `git apply`; `git diff --stat`: 7 file thay đổi, 499 dòng thêm, 8 dòng xóa
    (`Cargo.lock`, `Cargo.toml`, `crates/app/Cargo.toml`, `crates/app/src/main.rs`,
    `crates/device/src/hid.rs`, `crates/input/Cargo.toml`, `crates/input/src/lib.rs`),
    đúng tập file mà SPEC mục 7 cho phép. Sau khi sửa cảnh báo `was_active`,
    `git diff --numstat` của bản vá mới cho tổng +498/-8 (số đã ghi trong
    `integration/NOTES.md`).
    Không chạy cargo hay script nào trong cây upstream hoặc bản sao.

Không do tôi chạy: các kết quả review trước đó (132/133 của editor trước khi
`profiles/` có đủ file, việc reviewer duyệt crate/editor/integration) chỉ được
thuật lại từ báo cáo của reviewer. Kết quả GitHub Actions lần chạy #1 (xem mục 3)
là dữ kiện do người dùng cung cấp, không phải do tôi chạy.

## 3. Chưa kiểm chứng (cần Windows, biên dịch bản vá, hoặc iPhone)

1. Bản Mirrorly đã vá **đã biên dịch** (release, `app` và `keymap-sim`) và
   `cargo test -p keymap -p input -p device` đạt trên `windows-latest` ở lần chạy
   #1 của GitHub Actions (https://github.com/Mgd152/mgd-web/actions/runs/45731622579,
   commit 9cb9931; lần build release mất 2m16s). Chỉ có một cảnh báo
   (`main.rs:639`, giá trị gán cho `was_active` không bao giờ được đọc), đã được
   sửa trong bản vá mới. Việc biên dịch đạt không chứng minh hành vi đúng: chưa
   chạy `Mirrorly.exe` nào, chưa có phép thử trên máy Windows thật.
2. `integration/apply-to-mirrorly.ps1`: các bước clone, chép crate và áp bản vá
   đã chạy được trên CI. Bước `-StageOnly` **lỗi** ở lần chạy #1: "expected
   exactly one avcodec DLL in ...\target\release, found 0" (build script của
   upstream đã chạy, không có cảnh báo thiếu `FFMPEG_DIR`, nhưng DLL không nằm
   trong `target\release`; nghi do bố cục thư mục build mới của cargo, **chưa
   chứng minh**). Script nay lấy DLL từ `third_party\ffmpeg\bin` (bản FFmpeg đã
   ghim và kiểm băm) và kiểm SHA-256 theo `packaging\ffmpeg.txt`. **Bản sửa này
   chưa được chạy**: đóng gói đúng và tải artifact lên được hay không sẽ chỉ biết
   sau lần chạy CI kế tiếp. Nhánh chuẩn hóa CRLF của script chưa được thử (runner
   dùng `autocrlf=false`).
3. **Multi-touch qua `send_multitouch` trên iPhone**: chưa ai thử. iOS có thể
   bỏ qua, đảo thứ tự hoặc xử lý sai báo cáo nhiều điểm chạm.
4. Tần suất báo cáo khi look với chuột 1000-8000 Hz: chưa đo. Bản vá gộp chuyển
   động thô thành một `MouseMotion` mỗi vòng lặp sự kiện, nhưng USB/iOS có chịu
   nổi hay không là chưa biết.
5. Hành vi của iOS khi mất kết nối với ngón còn đặt; khung nhả khi ngắt kết nối
   hoặc thoát là "cố gắng hết sức" (race với việc huỷ tác vụ HID).
6. `CursorGrabMode::Confined` trên Windows ở mép cửa sổ khi look chạy: chưa thử.
7. Chuột nút Back/Forward thật và clipboard thật trong trình soạn: chưa thử
   (Playwright không bấm được các nút này; các nút đó chỉ được thử bằng sự kiện
   tổng hợp theo báo cáo của reviewer). Chưa thử editor trên Edge/Chrome Windows.
8. **Độ trễ chưa đo.** Số 87,5 ms trong tài liệu là số của upstream (Mirrorly,
   LATENCY.md), không phải của chúng tôi; mục tiêu 30-50 ms của bản bàn giao
   nhiều khả năng không đạt vì phía điện thoại chiếm phần lớn.
9. Mọi tính năng cần iPhone thật: không có thiết bị ở đây nên không thử gì.
10. Chạy `keymap-sim.exe` (đã cross-build) trên Windows: chưa.
11. Không có chế độ "chỉ điều khiển, không video": không làm được vì iOS chỉ
    nhận HID khi luồng media đang chạy. Đây là giới hạn đã nêu, không phải lỗi.

## 4. Lỗi và hạn chế đã biết

- Chỉ USB; keymap từ chối bật trên Wi-Fi (iOS bỏ qua báo cáo HID trên Wi-Fi, theo
  đo của upstream), trừ khi đặt `MIRROR_KEYMAP_WIFI=1` để thử nghiệm.
- Phím bật/tắt (mặc định Backquote) không gõ được lên điện thoại khi có `--keymap`.
- Mirrorly single-instance: có cửa sổ cũ đang chạy thì `--keymap` bị bỏ qua.
- Phím F gán trong hồ sơ che phím tắt của Mirrorly khi keymap đang bật; F9 và
  Ctrl+số bị chặn khi bật.
- Reviewer ghi nhận một số điểm nhỏ (minor) không chặn: editor lệch tới ~1 px
  khi kéo vì viền khung; 16 trường hợp biên lệch giữa kiểm tra của editor và
  Rust trong 2.520 hồ sơ fuzz (tên thành viên JSON trùng lặp, số nguyên lớn hơn
  u32, `-0`, giá trị cách biên < 0,00005); một nhãn nhập chứa surrogate lẻ
  không xuất được thành file `keymap-sim` chấp nhận.
- Số dòng patch trong `integration/NOTES.md` đã cập nhật đúng: +498/-8.
- Cargo.lock trong patch sửa tay; nếu không chính xác cargo tự sửa, CI in diff.

## 5. Tái hiện

Trên Linux (hoặc Windows có Rust), tại `ios-keymapping/`:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
cargo build --release --target x86_64-pc-windows-gnu --bin keymap-sim   # cần mingw-w64
for p in portrait landscape; do
  target/release/keymap-sim validate profiles/touch-test-$p.json
  target/release/keymap-sim simulate profiles/touch-test-$p.json profiles/touch-test-$p.sim.txt
done
node editor/tests/editor.test.mjs        # cần playwright + Chromium
```

Kiểm tra bản vá (git, không cần build):

```bash
git clone https://github.com/AnasSAV/mirrorly && cd mirrorly
git checkout ac70daa58ee5f24a067e80b6d0ffbcec4c735f1e
git apply --check ../ios-keymapping/integration/mirrorly-keymap.patch
```

Build thật trên Windows: xem README mục 5
(`powershell -NoProfile -ExecutionPolicy Bypass -File .\apply-to-mirrorly.ps1 -Build`) hoặc
chạy workflow `ios-keymapping`. Kiểm thử trên PC với iPhone: README mục 9 (danh
sách chưa thực hiện) và mục 10 (đo độ trễ: trung vị, p95, FPS, cấu hình máy;
F9/F10/`--timings-csv` của upstream; camera tốc độ cao).

## 6. Phạm vi

Không có macro, lặp, turbo, tự ngắm/bắn, né phát hiện hay giả mạo thiết bị
trong mã, editor, hồ sơ mẫu hay tài liệu. Hành vi có thời gian duy nhất là nút
`tap` nhả sau 40 ms và nhả ngón look sau `release_after_ms` (mặc định tắt).
Hồ sơ mẫu chỉ dành cho ứng dụng kiểm thử multi-touch; không có hồ sơ cho game
cụ thể. Với Free Fire/Garena: không có bảo đảm keymapping được cho phép; cần
kiểm tra quy định chính thức; tài khoản phụ không loại bỏ rủi ro.
