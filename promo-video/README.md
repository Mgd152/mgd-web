# promo-video

Khuôn video chấm điểm nhạc dọc 9:16 (TikTok / YouTube Shorts), 15–20 giây, 1080×1920, 60 fps.
Mỗi bài chỉ cần một file cấu hình trong `songs/`, chạy một lệnh là ra video.

Những gì có trong video:
- Thẻ kính tông màu ảnh bìa: ảnh bìa lớn, tên bài, nghệ sĩ.
- Ô điểm có số lăn kiểu đồng hồ cơ, số cộng điểm viết tay, thỉnh thoảng dao động nhẹ ±0.1–0.2 quanh mốc.
- Thanh sóng nhạc 48 cột (low bên trái → high bên phải), sáng dần theo tiến trình video.
- Cuối video, **con dấu mực "FINAL SCORE"** đập xuống, rồi hiện nút "send me your funk ↓".
- Thẻ hologram (nghiêng 3D, ánh kim chạy qua ảnh bìa), có thể bật đung đưa nhẹ như vật treo.
- Mọi phản ứng (rung, phát sáng, nền khói tăng tốc rồi chậm lại) đi theo **biên độ âm thanh** của bài: đường bao bass cộng các cú đập ở dải trung. Không dùng nhận diện bằng AI.
- Không khí nhiều lớp:
  - sương khói và vệt sáng,
  - bụi sáng, đốm sáng, tia lấp lánh,
  - họa tiết theo ảnh bìa: lá, lá phong, tro, kèm tàn lửa và sương mờ phía trước khi mức không khí cao.

## Cài đặt (một lần mỗi máy)

```bash
pip install librosa scipy numpy pillow
cd promo-video && npm install          # chỉ cài font
```

Cần thêm `ffmpeg`, Node 18+ và Playwright có Chromium. Môi trường cloud của Claude Code có sẵn Playwright/Chromium.

## Làm video cho bài mới

1. Chép file nhạc vào `assets/<slug>/track.mp3`. Thư mục `assets/` không được đưa lên git.
2. Lấy ảnh bìa:
   ```bash
   python3 fetch_cover.py "ZMAJOR Rampage"                              # xem danh sách
   python3 fetch_cover.py "ZMAJOR Rampage" assets/rampage/cover.jpg 0   # lưu kết quả số 0
   ```
   Nếu nhiều bản phát hành trùng tên, chọn bản có ngày phát hành và hãng (label) khớp với thẻ ID3 của file mp3.
3. Chép một file trong `songs/` thành `songs/<slug>.json` rồi sửa các trường (xem bảng bên dưới).
4. Xem trước 6 khung hình: `python3 make_video.py songs/<slug>.json --check` → tạo ra `out/<slug>/check.jpg`
5. Làm video: `python3 make_video.py songs/<slug>.json` → ra 3 file trong `out/<slug>/`:
   - `<slug>_master.mp4`: có nhạc, để đăng.
   - `<slug>_preview.mp4`: bản nhẹ để xem trên điện thoại.
   - `<slug>_noaudio.mp4`: không tiếng, để ghép sound gốc của bài trên TikTok (lượt xem được tính cho bài).

## Các trường trong `songs/<slug>.json`

| Trường | Ý nghĩa |
|---|---|
| `title`, `artist` | Chữ hiện trên thẻ. Ghi tên nghệ sĩ đúng như trên Apple Music / Spotify. |
| `drop_at` | Thời điểm drop trong bài (giây). |
| `bpm` | Tốc độ bài. Bỏ trống thì máy tự đo. Với bản slowed, nên đo theo chu kỳ bass. |
| `bars_intro`, `bars_main`, `bars_out` | Số ô nhịp trước drop, sau drop, và đoạn chốt điểm. Mặc định 2 / 5 / 2, khoảng 18 giây ở 120 BPM. |
| `start` (+ `target`) | Muốn cắt từ một mốc cố định (ví dụ 42 = 0:42) thì điền vào đây; `target` là độ dài mong muốn, tự làm tròn theo ô nhịp. |
| `story` | Diễn biến điểm theo giây của video, dạng `"0.9:3.1,2.0:6.7,10.96:9.2"`. Bỏ trống thì điểm tự tăng dần theo độ mạnh của từng ô nhịp. |
| `score` | Điểm chốt trên con dấu. Mặc định lấy mốc cuối của `story`. |
| `pal` | Màu tay `"deep,cool,hot,white"`. Bỏ trống thì tự lấy từ ảnh bìa. Nên đặt tay khi ảnh bìa gần như một màu, ví dụ toàn đỏ hay toàn đen. |
| `particles` | Họa tiết: `leaves` (lá), `maple` (lá phong), `ash` (tro), `none`. |
| `leaf_colors` | Màu lá `"dark,mid,bright"` (cho `leaves` / `maple`). |
| `atmos` | Độ dày không khí: 1 = chuẩn; 1.3–1.7 thêm tàn lửa, sương mờ, nhiều hạt hơn. |
| `sway` | Thẻ đung đưa nhẹ như được treo: 0 = tắt, 1 = bật. |
| `energy` | Độ mạnh phản ứng: 1 = funk / nhẹ; khoảng 1.6 = nhạc mạnh, motivation. |
| `glitch` | `true`: chớp trắng lúc drop và tách màu RGB ở những cú đập mạnh nhất (hợp nhạc mạnh). |
| `react` | Độ nhạy bắt nhịp `"th,gain,pre"`. Mặc định `0.22,1.0,0.3`; bài ít bass có thể hạ `th`, tăng `gain`. |
| `cta` | Chữ trên nút cuối video. |

## Ba bài mẫu

- `songs/travessia.json`: funk, tông xanh, lá rơi, cắt quanh drop 1.
- `songs/luz.json`: cắt từ 0:42, màu đỏ đặt tay, lá phong, có `story` và đung đưa.
- `songs/rampage.json`: nhạc mạnh: `energy 1.6`, `glitch`, tro trắng, nền đen đỏ.

## Cấu trúc

- `analyze_clip.py`: cắt nhạc, phân tích tín hiệu (đường bao bass và cú đập dải trung, phổ 48 dải, độ lớn), lấy màu ảnh bìa, rồi ghi ra `work/<slug>/build/`.
- `renderer/template.html`: dựng từng khung hình. Nền khói và vệt sáng vẽ bằng WebGL, phần còn lại bằng canvas và DOM.
- `render.js`: mở template trong Chromium ẩn rồi chụp từng khung hình.
- `make_video.py`: chạy cả ba bước trên rồi xuất 3 file video bằng ffmpeg.
- `fetch_cover.py`: tìm và tải ảnh bìa từ Apple Music, dự phòng bằng Deezer.
- `analyze_overview.py`: in độ mạnh từng giây và vẽ phổ kèm đường bass và treble của cả bài, để tìm drop và chọn đoạn cắt.
