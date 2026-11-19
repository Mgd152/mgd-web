# Làm video chấm điểm nhạc (TikTok / Shorts) bằng khuôn promo-video

Khuôn này tạo video dọc 9:16 (1080×1920, 60 fps, khoảng 18–23 giây) cho một bài nhạc:
thẻ kính có ảnh bìa, điểm số tăng dần theo nhạc, con dấu "FINAL SCORE" đập xuống ở cuối,
và một nhân vật trồi lên chốt điểm. Mọi chuyển động bám theo biên độ âm thanh của bài.

File này đi kèm thư mục `promo-video/`. Bạn cần cả thư mục đó để chạy.

## 1. Cài đặt (một lần)

```bash
pip install librosa scipy numpy pillow
cd promo-video && npm install
```

Cần thêm `ffmpeg`, Node 18+ và Playwright có Chromium.

## 2. Làm một video

1. Chép nhạc vào `assets/<tên>/track.mp3`. Dùng file mp3 của chính bạn hoặc bản bạn có quyền dùng.
2. Lấy ảnh bìa:
   ```bash
   python3 fetch_cover.py "Tên nghệ sĩ Tên bài"                          # xem danh sách
   python3 fetch_cover.py "Tên nghệ sĩ Tên bài" assets/<tên>/cover.jpg 0  # lưu kết quả số 0
   ```
3. Chép một file trong `songs/` thành `songs/<tên>.json` rồi sửa (xem mục 3).
4. Xem trước 6 khung hình, mất khoảng 15 giây:
   ```bash
   python3 make_video.py songs/<tên>.json --check     # ra out/<tên>/check.jpg
   ```
5. Ưng rồi mới xuất video, mất khoảng 4–5 phút:
   ```bash
   python3 make_video.py songs/<tên>.json
   ```

Kết quả nằm trong `out/<tên>/`:

| File | Dùng để |
|---|---|
| `<tên>_tiktok.mp4` | Đăng TikTok (60 fps cố định, khoảng 12 Mbps) |
| `<tên>_tiktok_noaudio.mp4` | Không tiếng, để ghép sound gốc trong TikTok |
| `<tên>_master.mp4` | YouTube Shorts, Reels, lưu trữ |
| `<tên>_preview.mp4` | Bản nhẹ để xem nhanh |

## 3. Các trường trong `songs/<tên>.json`

### Bài nhạc và đoạn cắt

| Trường | Ý nghĩa |
|---|---|
| `title`, `artist` | Chữ trên thẻ |
| `drop_at` | Giây của drop 1 trong bài. Đo theo cú đập bass lớn đầu tiên, thường ngay sau một khoảng lặng ngắn |
| `bpm` | Tốc độ bài. Với bản slowed, đo theo chu kỳ bass |
| `start`, `target` | Giây bắt đầu cắt và độ dài mong muốn. Intro ngắn thì `start: 0`; intro dài thì bắt đầu 2–4 ô nhịp trước drop |

### Điểm số

| Trường | Ý nghĩa |
|---|---|
| `story` | Diễn biến điểm theo giây của video, ví dụ `"0.9:6.0,4.5:6.6,9.04:8.9"` |
| `score` | Điểm chốt trên con dấu. Dấu ghi SKIP (<6), DECENT (6–7.4), REPLAY (7.5–8.9), PEAK (≥9) |
| `wobble` | `"trước,sau"`: điểm dao động nhiều hay ít trước và sau drop. `"3,0.5"` = trước drop nhảy nhiều, sau drop gần như đứng yên |

### Màu và nền

| Trường | Ý nghĩa |
|---|---|
| `pal` | Bốn màu `"đậm,chính,nhấn,trắng"` lấy từ ảnh bìa. Bỏ trống thì tự lấy |
| `bg_dim` | Độ sáng nền, 0.8 là hơi tối |
| `particles` | Hạt bay: `leaves` (lá), `maple` (lá phong), `ash` (mảnh sáng), `none` |
| `leaf_colors` | Màu hạt `"tối,giữa,sáng"` |
| `atmos` | Độ dày không khí, 1 đến 1.7 |
| `dots` | Lớp chấm tròn halftone. Mặc định 0.55, đặt 0 để tắt |
| `bg_fx` | Vật thể nền, ghép bằng dấu phẩy: `blades` (kiếm trôi), `slash` (vệt chém theo cú đập), `chains` (xích treo võng), `chains-x` (xích bắt chéo), `snakes` |
| `bg_objs` | Ảnh PNG đã tách nền để trôi mờ ở nền, ví dụ một chi tiết cắt từ ảnh bìa |

### Phản ứng theo nhạc

| Trường | Ý nghĩa |
|---|---|
| `energy` | Độ mạnh phản ứng: 1 = nhẹ, khoảng 1.6 = nhạc mạnh |
| `kick` | Cách thẻ nhận cú đập. `bounce` (mặc định) = nảy lò xo kèm rung; `jump` = rung ngẫu nhiên kiểu cũ |
| `smooth` | Độ mềm của cú đập, mặc định 1 |
| `mids` | Mức phản ứng với phần ngoài bass (snare, perc). Mặc định 0.65; bass và kick luôn đủ lực |
| `sway` | Thẻ đung đưa nhẹ như đang treo: 0 hoặc 1 |
| `glitch` | `true` = chớp trắng nhẹ đúng lúc vào drop |

### Nhân vật chấm điểm

| Trường | Ý nghĩa |
|---|---|
| `judge_img` | Ảnh nhân vật PNG đã tách nền |
| `judge_crop` | Vùng lấy `"x0,y0,x1,y1"` (0 đến 1), thường từ đầu đến ngực |
| `judge_line` | Câu chốt ở cuối, ví dụ `"9.2/10. nah, i'd replay."` |
| `judge_at` | Bỏ trống = chỉ trồi lên sau khi đóng dấu. `intro` = trồi lên lúc mở đầu để nói một câu, nói xong hạ xuống, cuối video trồi lên lại |
| `voice`, `voice_at` | File giọng nói cho câu mở đầu và giây bắt đầu phát. Nhạc tự hạ xuống trong lúc nhân vật nói |
| `judge_open` | Chữ trong bong bóng lúc nhân vật nói câu mở đầu |

## 4. Tách nền nhân vật

- Ảnh anime nền phẳng: dùng công cụ tách nền tự động, ví dụ `@imgly/background-removal-node`.
- Ảnh bìa nhiều ánh sáng loé: công cụ tự động thường hỏng. Vẽ một vùng chọn đa giác quanh nhân vật bằng tay rồi làm mềm mép.
- Ảnh tối quá thì kéo sáng vùng tối lên trước khi đưa vào.
- Dùng tranh của hoạ sĩ khác thì ghi nguồn hoạ sĩ trong mô tả khi đăng.

## 5. Mẹo để ra video nhanh và đúng

- Luôn xem `--check` trước, chốt xong mới xuất. Mỗi lần đổi ý sau khi xuất là mất thêm 4–5 phút.
- Xuất từng bài một. Chạy hai bài cùng lúc thì cả hai đều chậm gần gấp đôi.
- Đo drop cẩn thận: nhìn biên độ bass ở độ phân giải 20–40 ms. Cú đập lấy đà ngay trước drop và chỗ nhạc dày thêm về sau đều dễ bị nhầm là drop.
- Vật thể nền nên ít và mờ. Nhiều quá sẽ rối và át mất thẻ.
- Đừng dùng bộ lọc làm mờ (blur) trên canvas cho từng khung hình: nó làm khâu dựng chậm gấp 6–8 lần. Khuôn đã thay bằng cách vẽ ở độ phân giải thấp rồi phóng lên.

## 6. Đăng TikTok sao cho đỡ bị giảm chất lượng

1. Đăng file `_tiktok.mp4`.
2. Chuyển file sang điện thoại bằng AirDrop, Google Drive hoặc cáp. Không gửi qua Zalo hay Messenger vì chúng nén video xuống 30 fps.
3. Trong TikTok, bật "Cho phép tải lên chất lượng cao" ở màn hình đăng và tắt "Tiết kiệm dữ liệu".
4. Hạn chế chỉnh sửa trong app TikTok, vì app có thể dựng lại video.

## 7. Dùng với Claude Code

Nếu bạn dùng Claude Code, chép cả thư mục `promo-video/` và thư mục `.claude/skills/promo-video/` vào dự án của mình.
Sau đó chỉ cần gửi file mp3 và nói, ví dụ:

> làm video chấm điểm bài này, drop lúc 18 giây, điểm chốt 8.9, nhân vật chấm điểm là ảnh này

Claude sẽ lấy ảnh bìa, đo drop, dựng ảnh xem trước cho bạn duyệt rồi xuất video.
