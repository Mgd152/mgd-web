# iOS keymapping cho Mirrorly

Điều khiển một chiếc **iPhone thật** từ PC Windows bằng bàn phím và chuột
(WASD như cần joystick, kéo chuột để "nhìn", phím bấm/giữ thành nút chạm), để
kiểm thử cảm ứng và dùng với các game/ứng dụng cho phép điều khiển ngoài.

Dự án gồm một thư viện Rust thuần logic (`crates/keymap`), trình soạn cấu hình
chạy trong trình duyệt (`editor/index.html`), cấu hình mẫu, và một **bản vá
(patch)** cho [Mirrorly](https://github.com/AnasSAV/mirrorly) (MIT) để Mirrorly
gửi nhiều điểm chạm cùng lúc tới iPhone.

> Các liên kết tới `docs/BAO_CAO.md`, `docs/SPEC.md`, `integration/NOTES.md`
> trỏ tới kho mã nguồn; chúng không có trong thư mục/ZIP portable đóng gói sẵn.

> **Trạng thái trung thực:** phần thư viện `keymap`, trình soạn và công cụ mô
> phỏng đã được chạy thử trong container Linux (xem
> [`docs/BAO_CAO.md`](docs/BAO_CAO.md)). Bản Mirrorly đã vá đã được biên dịch
> và chạy `cargo test -p keymap -p input -p device` thành công trên runner
> `windows-latest` của GitHub Actions (lần chạy #1). **Chưa có gì được chạy trên
> một máy Windows thật hay trên iPhone nào.** Bước đóng gói thư mục portable
> bị lỗi ở lần chạy #1 và đã được sửa trong thay đổi này; việc sửa đó và việc
> tải artifact có hoạt động hay không sẽ chỉ biết sau lần chạy kế tiếp. Gửi
> nhiều điểm chạm (multi-touch) qua `send_multitouch` chưa từng được kiểm chứng
> trên điện thoại, cả bởi Mirrorly lẫn bởi dự án này; độ trễ chưa đo.

## 1. Đây là gì, không phải là gì

Là:

- Một lớp ánh xạ "phím/chuột -> điểm chạm" nằm trên đường điều khiển HID sẵn
  có của Mirrorly. Không giả lập, không jailbreak, không sửa IPA.
- Một phím bấm = một thao tác của người dùng = tối đa một điểm chạm đi xuống.

Không phải là, và sẽ không bao giờ có:

- Né phát hiện, giả mạo thiết bị, giấu công cụ khỏi ứng dụng/game, hay làm thay
  đổi cách ứng dụng phân loại thiết bị.
- Macro, chuỗi thao tác ghi sẵn, lặp/turbo, tự bắn, tự ngắm, bù giật, "ngẫu
  nhiên hóa cho giống người", hay bất kỳ thao tác tự động nào. Hành vi có thời
  gian duy nhất là: nút chế độ `tap` nhả sau đúng 40 ms, và tùy chọn nhả ngón
  "look" sau một khoảng không di chuyển chuột (`release_after_ms`).
- Công cụ để chơi game cạnh tranh có lợi thế không công bằng. Cấu hình mẫu chỉ
  dành cho **ứng dụng kiểm thử multi-touch**.

## 2. An toàn và quy định (đọc trước khi dùng với game)

- Với game như **Free Fire (Garena)**: **không có bất kỳ đảm bảo nào** rằng
  Garena cho phép keymapping, hay rằng iPhone điều khiển qua PC được xếp vào
  nhóm thiết bị di động. Hãy kiểm tra quy định chính thức của nhà phát hành
  **trước khi dùng**.
- **Tài khoản phụ không loại bỏ rủi ro** (khóa tài khoản, mất vật phẩm,
  vi phạm điều khoản vẫn có thể xảy ra).
- Không dùng công cụ này để tránh việc phân loại thiết bị hay để giành lợi
  thế trái quy định. Nếu không chắc game có cho phép điều khiển ngoài hay
  không, đừng dùng với game đó.
- Phần mềm cung cấp "nguyên trạng" (MIT, không bảo hành).

## 3. Kiến trúc

```
bàn phím / chuột (winit, Windows)
        |  Press/Release/MouseMotion
        v
 keymap::Engine  (thuần logic, không I/O, không đồng hồ; chạy theo now_ms do app truyền)
        |  keymap::Frame  (ảnh chụp đầy đủ: <= 5 điểm chạm, id 0..4)
        v
 HidEvent::MultiTouch (crate input)  ->  bộ thực thi HID (crate device)
        |  gộp frame "chỉ di chuyển", KHÔNG BAO GIỜ bỏ frame down/up
        v
 UniversalHidServiceClient::send_multitouch  (báo cáo 0x09 qua đường hầm USB, CoreDevice HID)
        v
      iPhone
```

Đường hình ảnh (nhận/giải mã/vẽ video) là của Mirrorly, không đổi. Nhận xét
quan trọng: iOS chỉ nhận báo cáo HID khi một luồng media của `displayservice`
đang chạy (FEASIBILITY §5.1 của upstream). Vì vậy **không có chế độ "chỉ điều
khiển, không stream video"**; tối đa là nhận video mà không vẽ, và dự án này
không thêm công tắc như vậy.

Chỉ **USB** được hỗ trợ: qua Wi-Fi iOS bỏ qua báo cáo `universalhidservice`
(upstream đã đo), nên app từ chối bật keymap trên Wi-Fi (biến môi trường thử
nghiệm `MIRROR_KEYMAP_WIFI=1`).

Mã trong repo:

| Thư mục | Nội dung |
|---|---|
| `crates/keymap` | Thư viện: hình học xoay màn hình, hồ sơ JSON + kiểm tra hợp lệ, engine, công cụ `keymap-sim` |
| `editor/index.html` | Trình soạn kéo-thả, một file, không cần mạng |
| `profiles/` | Hồ sơ mẫu cho app kiểm thử cảm ứng + script mô phỏng |
| `integration/` | `mirrorly-keymap.patch`, `apply-to-mirrorly.ps1`, `NOTES.md` |
| `docs/` | `SPEC.md` (đặc tả), `BAO_CAO.md` (báo cáo build/test) |
| `../.github/workflows/ios-keymapping.yml` | CI (Windows) |

## 4. Yêu cầu

- Windows 10 (1809) / 11, x64. iPhone chạy iOS 27 trở lên (theo yêu cầu của
  Mirrorly upstream), cắm **cáp USB**.
- Cài [Apple Devices](https://apps.microsoft.com/detail/9np83lwlpz9k) từ
  Microsoft Store; mở khóa iPhone, bấm **Tin cậy (Trust)**, bật **Developer
  Mode** (Cài đặt > Quyền riêng tư & Bảo mật > Chế độ nhà phát triển).
- Để tự build: Rust (MSVC toolchain), Visual Studio 2022 Build Tools với
  workload C++, LLVM (libclang), Git, PowerShell. FFmpeg 9.0 LGPL shared sẽ được
  tải bằng `packaging\get-ffmpeg.ps1` của Mirrorly. Cài bằng winget (lệnh theo
  README của Mirrorly, chưa chạy ở đây):

  ```powershell
  winget install Rustlang.Rustup
  winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  winget install LLVM.LLVM
  winget install Git.Git
  ```

  Script của upstream tự tìm `vcvars64`, không cần mở "Developer Command Prompt".

## 5. Áp dụng bản vá và build trên Windows

Lệnh dưới đây chưa được chạy trên một máy Windows thật. Trên runner CI
(`windows-latest`, lần chạy #1) các bước clone, chép crate, áp bản vá, build và
test đã chạy thành công; bước đóng gói (`-StageOnly`) lỗi và đã được sửa, chưa
được chạy lại.

```powershell
cd ios-keymapping\integration
# chỉ clone Mirrorly đúng commit ac70daa, chép crate keymap vào, áp bản vá
powershell -NoProfile -ExecutionPolicy Bypass -File .\apply-to-mirrorly.ps1

# hoặc kèm build: tải FFmpeg, chạy test, cargo build --release -p app,
# rồi gom vào ios-keymapping\dist\Mirrorly-keymap\ (DLL FFmpeg lấy từ
# third_party\ffmpeg\bin và được kiểm SHA-256 theo packaging\ffmpeg.txt).
# Hãy chạy Mirrorly.exe từ thư mục dist đó, KHÔNG chạy target\release\Mirrorly.exe
# (thiếu các DLL FFmpeg ở đó).
powershell -NoProfile -ExecutionPolicy Bypass -File .\apply-to-mirrorly.ps1 -Build

# thêm -Package nếu đã có cargo-about + Inno Setup 6 (chạy build-release.ps1 của upstream)
# thêm -Force để xóa thư mục đích không rỗng; -Dest <thư mục> để đổi nơi đặt
```

Chính sách thực thi PowerShell mặc định trên Windows (Restricted) và dấu
mark-of-the-web của file tải về dạng ZIP sẽ chặn chạy trực tiếp `.\script.ps1`;
dùng `-ExecutionPolicy Bypass` như trên, hoặc chạy `Unblock-File .\apply-to-mirrorly.ps1`
trước. (Dựa trên hành vi Windows nói chung, chưa thử ở đây.)

Bản vá chỉ sửa 7 file có sẵn của Mirrorly (`Cargo.toml`, `Cargo.lock`,
`crates/input/{Cargo.toml,src/lib.rs}`, `crates/device/src/hid.rs`,
`crates/app/{Cargo.toml,src/main.rs}`); crate `keymap` được chép vào
`crates/keymap` bởi script. Chi tiết từng thay đổi và các điểm dễ lỗi khi biên
dịch lần đầu: [`integration/NOTES.md`](integration/NOTES.md).

Workflow `ios-keymapping.yml` (chạy tay hoặc khi push lên nhánh
`claude/new-session-l9i0tr`) làm đúng các bước trên trên `windows-latest` và
đăng artifact `mirrorly-keymap-<sha>`. Lần chạy #1: biên dịch và test xanh,
nhưng bước đóng gói lỗi (không tìm thấy DLL avcodec trong `target\release`) nên
chưa có artifact; script đã được sửa để lấy DLL từ `third_party\ffmpeg\bin`.
Việc sửa này chưa được chạy lại, nên artifact chưa được xác nhận.

## 6. Cách dùng

1. Đóng mọi cửa sổ Mirrorly đang chạy (Mirrorly là single-instance; nếu còn
   cửa sổ cũ, cờ `--keymap` bị bỏ qua).
2. Cắm iPhone bằng USB, mở app kiểm thử cảm ứng trên điện thoại.
3. Trong thư mục đã giải nén (artifact `mirrorly-keymap-<sha>`, hoặc
   `ios-keymapping\dist\Mirrorly-keymap\`), **bấm đúp** một trong các file:
   - `Chay-keymap-doc.bat`: hồ sơ mẫu màn hình dọc;
   - `Chay-keymap-ngang.bat`: hồ sơ mẫu màn hình ngang (bấm **F12** để xoay
     khung nhìn cho khớp; hướng hồ sơ phải khớp hướng khung nhìn);
   - `Keo-ho-so-vao-day.bat`: kéo file hồ sơ `.json` của bạn (ví dụ xuất từ
     `editor\index.html`) thả vào file này.

   Cách gõ lệnh tương đương:
   `Mirrorly.exe --keymap profiles\touch-test-portrait.json`.
4. Bấm **`` ` ``** (Backquote) để bật/tắt keymap. Khi bật, tiêu đề cửa sổ hiện
   "Keymap ON"; nếu hồ sơ có điều khiển `look`, con trỏ bị nhốt trong cửa sổ
   và ẩn. Hồ sơ được **đọc lại mỗi lần bật**, nên sửa trong editor rồi bật lại
   là có hiệu lực, không cần khởi động lại.
5. **Phím dừng khẩn cấp** (hồ sơ mẫu dùng **Esc**, mặc định của định dạng là F8):
   nhả toàn bộ điểm chạm và tắt keymap ngay. Đổi bằng `panic_key` trong hồ sơ.
6. Tự động nhả hết và tắt keymap khi: mất focus cửa sổ, mất kết nối/rút USB,
   đổi hướng/kích thước màn hình, thoát app.

Giới hạn khi dùng: chỉ USB; khi `--keymap` được đưa vào, phím bật/tắt (mặc định
`` ` ``) không còn gõ được lên điện thoại; khi keymap đang bật chuột/cuộn/F9
không đi vào cử chỉ thường, chỉ các phím F (trừ F9) của Mirrorly còn hoạt động;
nếu hồ sơ gán phím F thì phím đó che phím tắt của Mirrorly lúc đang bật.
Keymap từ chối bật khi chưa kết nối, trên Wi-Fi, khi đang bật iPad display, hoặc
khi đang chạy script cuộn/đo độ trễ F9.

## 7. Định dạng hồ sơ (JSON v1)

```json
{
  "version": 1,
  "name": "Touch test (portrait)",
  "app": "multi-touch test app",
  "orientation": "portrait",
  "toggle_key": "Backquote",
  "panic_key": "F8",
  "controls": [
    { "type": "joystick", "label": "Move", "x": 0.25, "y": 0.75, "radius": 0.12,
      "up": "KeyW", "down": "KeyS", "left": "KeyA", "right": "KeyD" },
    { "type": "look", "label": "Look", "x": 0.7, "y": 0.45, "sensitivity": 1.0,
      "invert_y": false, "reach": 0.25, "release_after_ms": 0 },
    { "type": "button", "label": "Hold A", "x": 0.82, "y": 0.8, "key": "Space", "mode": "hold" },
    { "type": "button", "label": "Hold C", "x": 0.55, "y": 0.62, "key": "KeyK", "mode": "hold" },
    { "type": "button", "label": "Tap D", "x": 0.88, "y": 0.65, "key": "KeyJ", "mode": "tap" }
  ]
}
```

- Toạ độ `x`, `y` là **tỉ lệ 0..1 của bức hình như người dùng thấy** (gốc ở góc
  trên-trái) theo hướng của hồ sơ. `radius`, `reach` là tỉ lệ của **cạnh ngắn**.
- `version` phải bằng 1. `orientation`: `portrait` | `landscape`. `toggle_key`
  mặc định `Backquote`, `panic_key` mặc định `F8` (hai phím phải khác nhau, là
  phím bàn phím, không phải nút chuột, và không được gán cho điều khiển).
- `joystick`: `radius` trong (0, 0.5]; 4 phím hướng. Nhấn phím -> ngón chạm vào
  tâm rồi trượt tới `radius` theo hướng (chéo được chuẩn hóa, không dài hơn).
  Hai phím ngược nhau triệt tiêu (ngón ở lại tâm); nhả hết phím -> nhả ngón.
- `look`: tối đa một cái; `sensitivity` trong (0, 100], `invert_y`, `reach` trong
  (0, 1] (mặc định 0.3), `release_after_ms` 0..60000 (0 = không bao giờ tự nhả).
  Chuột di chuyển -> ngón trượt; vượt `reach` thì nhả và đặt lại ở tâm.
- `button`: `key` + `mode`: `hold` (giữ phím = giữ chạm) hoặc `tap` (chạm 40 ms
  rồi nhả, kể cả khi vẫn giữ phím, không lặp).
- Tên phím theo `KeyCode` của winit 0.30: `KeyA`..`KeyZ`, `Digit0`..`Digit9`,
  `F1`..`F24`, `Space`, `Tab`, `Enter`, `Escape`, `ShiftLeft`, `ArrowUp`,
  `Numpad0`..., kèm `MouseLeft/Right/Middle/Back/Forward` (chỉ cho điều khiển).
  Danh sách đầy đủ: `keymap-sim keys` (121 dòng = 116 phím + 5 nút chuột).
- Quy tắc: tọa độ trong [0,1]; mỗi tên phím chỉ dùng một lần trong cả hồ sơ;
  tối đa 64 điều khiển; trường lạ bị từ chối. Có thể có hơn 5 điều khiển
  nhưng chỉ 5 điểm chạm đồng thời; lần nhấn cần điểm thứ 6 bị bỏ qua và đếm.
- Mọi lỗi nêu rõ đường dẫn, ví dụ `controls[2].radius: must be in (0, 0.5], got 0.7`.

Kiểm tra: `keymap-sim validate profiles\touch-test-portrait.json`.
Mô phỏng không cần điện thoại:
`keymap-sim simulate <hồ sơ> <script> [--rotation R90] [--panel 828x1792]`
(lệnh trong script: `press KEY`, `release KEY`, `move DX DY`, `wait MS`,
`focus-lost`, `disconnect`, `rotate R90`, `# ghi chú`). Script mẫu:
`profiles\*.sim.txt`. Bản `keymap-sim` Linux/Windows được dựng bằng
`cargo build --release` (xem BAO_CAO).

## 8. Trình soạn (editor)

Mở `editor\index.html` bằng Edge/Chrome (chạy từ ổ đĩa, không cần mạng).
Giao diện tiếng Việt:

- Nạp ảnh nền (nút chọn file hoặc kéo-thả). Ảnh chụp bằng **F6** của Mirrorly
  nằm ở `Pictures\Mirrorly`. Hướng lấy theo tỉ lệ ảnh, có thể đổi tay.
- Thêm joystick / look / nút; kéo để di chuyển; kéo tay nắm để đổi `radius` /
  `reach`; nhân đôi (phím được xóa vì phím không được trùng) và xóa.
- Bảng bên: nhãn, phím ("Gán phím" rồi bấm phím hoặc nhấp nút chuột), độ nhạy,
  đảo trục Y, hold/tap, phím bật/tắt, phím dừng khẩn cấp.
- Kiểm tra hợp lệ theo đúng quy tắc và đường dẫn của Rust, hiện ngay cạnh
  trường; xuất bị khóa khi còn lỗi. Nhập/xuất JSON (tải xuống, sao chép, dán).
- Lưu ý: editor chỉ soạn cấu hình. Một phím = một chạm; không có macro.

## 9. Danh sách kiểm thử trên PC (chưa thực hiện)

Dùng **một ứng dụng kiểm thử multi-touch** (hiển thị từng ngón và số điểm
chạm), không dùng game cạnh tranh. Ghi kết quả pass/fail cho từng mục:

1. Kết nối: cắm USB, Trust, Developer Mode; Mirrorly hiện hình ảnh. Bấm `` ` ``
   khi chưa kết nối -> bị từ chối với toast.
2. Chạm đơn: chuột (khi chưa bật keymap) nhấp/kéo vẫn như upstream.
3. Bật keymap: tiêu đề "Keymap ON"; nhấn `` ` `` lần nữa tắt, mọi ngón nhả.
4. Multi-touch 1..5 điểm đồng thời: hồ sơ mẫu có 3 nút `hold` (Space, MouseLeft, KeyK), nên giữ W + kéo chuột (look)
   + Space + MouseLeft + KeyK = 5 ngón kéo dài; app test phải hiện đủ 1, 2, 3, 4,
   5 điểm. Đây là **kiểm chứng đầu tiên** của
   `send_multitouch` trên điện thoại; nếu iOS bỏ qua/trộn ngón, ghi lại và báo.
5. Khi đang giữ 5 ngón như mục 4, nhấn `KeyJ` (nút `tap`, lần nhấn thứ 6): bị bỏ
   qua, các ngón khác không đổi; sau khi nhả bớt một ngón, `KeyJ` chạm ~40 ms.
6. Joystick: 4 hướng + chéo (độ lệch bằng nhau), hai phím ngược nhau (ngón ở
   tâm, không nhả), nhả hết phím (ngón nhả).
7. Look: kéo chuột -> ngón trượt; kéo quá `reach` -> nhả và đặt lại ở tâm, không
   có chạm thừa; thử `release_after_ms` > 0.
8. `tap` vs `hold`: tap nhả sau ~40 ms dù giữ phím, không lặp; hold theo phím.
9. Phím dừng (Esc trong hồ sơ mẫu) khi đang giữ nhiều phím + kéo chuột: mọi ngón nhả, keymap tắt.
10. Mất focus (Alt+Tab, bấm cửa sổ khác) khi đang giữ phím: nhả hết, tắt.
11. Rút cáp USB khi đang giữ: app không treo, engine reset; cắm lại hoạt động.
    Ghi lại điện thoại có kẹt ngón nào không (hành vi iOS chưa rõ).
12. Xoay: F12 với hồ sơ landscape; đổi hướng khi đang bật -> nhả và tắt; kiểm tra
    vị trí điểm chạm trên màn hình điện thoại khớp vị trí trong editor.
13. Wi-Fi: xác nhận app từ chối bật.
14. Hơn 5 phím được gán, nhấn dồn dập: không treo, không ngón kẹt.
15. Tần số chuột cao (1000-8000 Hz) khi look: điện thoại có giật/trễ không;
    nếu có, hạ tần số chuột và ghi lại.

## 10. Đo độ trễ (chưa đo)

Dự án **không tuyên bố con số độ trễ nào**. Số liệu của upstream (đo bởi tác giả
Mirrorly trên máy của họ, không phải của chúng tôi): chạm -> khung hình phản hồi
đầu tiên khoảng 87,5 ms trung vị qua USB, phần của Mirrorly trên PC khoảng 1 ms;
phần lớn là phía điện thoại. Mục tiêu 30-50 ms của bản bàn giao vì thế nhiều
khả năng **không đạt được**.

Cách đo đề xuất: báo cáo **trung vị, p95, FPS, cấu hình máy** (CPU/GPU/RAM,
Windows, kiểu cáp/cổng USB, tần số chuột) và phương pháp, mỗi cấu hình ít nhất
30 mẫu.

- Của upstream (có sẵn, xem `LATENCY.md` của Mirrorly): **F10** bật overlay
  (p50/p95 từng chặng), `--timings-csv FILE` ghi mỗi frame, **F9** chạy 10 lần
  chạm đo cho đến khi khung phản hồi hiện (cần màn hình tĩnh và đích phản hồi
  thấy được). Lưu ý: bản vá này chặn F9 khi keymap đang bật và F9 đo đường chạm
  đơn, không đo đường multi-touch.
- Input-to-photon thật: camera tốc độ cao (240 fps trở lên) quay đồng thời phím
  bấm/đèn LED và màn hình điện thoại, đếm khung; lặp nhiều lần, tính trung vị và p95.
- Tách riêng độ trễ gửi input và độ trễ hình ảnh như bản bàn giao yêu cầu.

## 11. Hạn chế đã biết

- Multi-touch qua UHS **chưa kiểm chứng** trên điện thoại; bản vá đã biên dịch
  và qua test trên CI, nhưng chưa có phép thử nào trên Windows thật hay iPhone.
- Chỉ USB. Không có chế độ chỉ điều khiển không video (iOS yêu cầu luồng media).
- Con trỏ chỉ được `Confined`, không `Locked` (winit trên Windows); hành vi ở mép
  cửa sổ chưa thử.
- Khi ngắt kết nối/thoát, khung nhả có thể không kịp gửi (race với việc huỷ tác
  vụ HID); iOS xử lý ngón còn "đặt" khi mất kết nối chưa rõ.
- Phím bật/tắt mặc định không gõ được lên điện thoại khi dùng `--keymap`.
- Mirrorly là single-instance; cờ `--keymap` bị bỏ qua nếu đã có cửa sổ chạy.
- Không có profile cho game cụ thể nào.

## Giấy phép

MIT, xem [`LICENSE`](LICENSE) và [`NOTICE`](NOTICE) (ghi công Mirrorly của
AnasSAV và idevice của Jackson Coxson, cả hai MIT; bản build đóng gói kèm DLL
FFmpeg LGPL theo điều khoản của upstream). Không liên kết với Apple, Garena hay
tác giả Mirrorly.
