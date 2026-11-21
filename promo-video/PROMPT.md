# Prompt: dựng khuôn video chấm điểm nhạc (TikTok / Shorts)

Dán toàn bộ nội dung dưới đây cho Claude Code trong một dự án trống. Sau đó gửi file mp3 và nói bài nào, drop ở giây nào, bao nhiêu điểm.

---

Tôi muốn bạn dựng cho tôi một khuôn làm video quảng bá nhạc kiểu "chấm điểm", rồi dùng khuôn đó làm video cho từng bài tôi gửi. Trả lời tôi bằng tiếng Việt.

## Sản phẩm

Video dọc 9:16, 1080×1920, 60 fps cố định, dài 18–23 giây, H.264, màu BT.709. Mỗi bài xuất bốn file: bản đăng TikTok (khoảng 12 Mbps), bản không tiếng, bản master bitrate cao, bản xem nhanh.

Trong video có:

- **Nền**: tối, màu lấy từ ảnh bìa. Khói và tia sáng chuyển động chậm, bụi sáng, hạt bay theo chủ đề bìa (lá, mảnh sáng...), một lớp chấm tròn halftone mờ (chấm to dần về mép trên và dưới, mờ hẳn sau thẻ). Có thể thêm vài vật thể nền theo bìa (kiếm, xích, một chi tiết cắt từ bìa), số lượng ít và mờ.
- **Thẻ kính** ở giữa, hơi nhỏ và hơi thấp so với tâm: ảnh bìa, ô điểm ở góc ảnh bìa, tên bài, dòng nghệ sĩ, bên dưới là một hàng sóng nhạc 48 cột (trầm bên trái, cao bên phải) sáng dần từ trái sang phải theo tiến trình video. Thẻ có ánh hologram chạy qua và đung đưa nhẹ như đang treo.
- **Điểm số**: số lăn kiểu đồng hồ cơ, có số cộng điểm viết tay mỗi lần tăng, thỉnh thoảng dao động ±0.1. Điểm tăng theo các mốc tôi cho, mốc lớn nhất đúng lúc vào drop.
- **Mở đầu**: một dòng chữ viết tay "rate this drop ↓" trong khoảng 2 giây đầu.
- **Kết**: con dấu mực "FINAL SCORE x.x/10" đập xuống ảnh bìa kèm tiếng đập trầm, dưới dấu ghi hạng: SKIP (<6), DECENT (6–7.4), REPLAY (7.5–8.9), PEAK (≥9). Sau đó hiện nút "send me your funk ↓".
- **Nhân vật chấm điểm** (nếu tôi gửi ảnh hoặc bìa có nhân vật): ảnh đã tách nền, đánh màu ám theo bìa, có viền sáng. Sau khi đóng dấu, nhân vật trồi lên từ góc dưới trái tới ngang ngực, thở và lắc nhẹ, kèm bong bóng thoại ghi điểm và một câu ngắn. Nếu tôi gửi file giọng nói thì nhân vật trồi lên lúc mở đầu để nói, nhạc hạ xuống trong lúc nói, nói xong hạ xuống, cuối video trồi lên lại.

## Phản ứng theo nhạc

- Mọi chuyển động bám theo biên độ âm thanh thật của bài, giống AVEE Player: đường bao bass 30–150 Hz là chính, cộng các cú đập dải trung ở mức khoảng 65%. Không dùng nhận diện nhịp bằng AI.
- Mỗi cú đập: thẻ nảy kiểu lò xo (phóng to và nhích lên, vọt quá một chút rồi lắng), kèm rung ngẫu nhiên vừa phải; nền khói tăng tốc rồi chậm lại; viền sáng mạnh lên; vật thể nền cũng giật và gợn theo.
- Cú đập lên mềm, nhả chậm ở đuôi, không nhảy vọt trong một khung hình.
- Trước drop phản ứng yếu, từ drop trở đi mới đủ lực.

## Những thứ không làm

- Không có đốm sao bốn cánh lấp lánh.
- Không tách màu RGB khi đập.
- Không vòng tròn làm họa tiết, không giấy nhám, không chữ kiểu "wait for it", không dòng nhận xét cuối video, không thanh tiến trình riêng.
- Không quá nhiều vật thể nền.

## Cách dựng

- Một script Python phân tích nhạc bằng librosa: cắt đoạn, tính đường bao bass, cú đập dải trung, phổ 48 dải, lấy bảng màu từ ảnh bìa, ghi ra một file dữ liệu cho từng khung hình.
- Một trang HTML dựng từng khung hình theo số thứ tự khung (không dựa vào đồng hồ thật): nền bằng WebGL, hạt và vật thể bằng canvas, thẻ bằng DOM.
- Một script Node dùng Playwright mở trang đó trong Chromium ẩn, chụp từng khung hình bằng nhiều trang song song.
- ffmpeg ghép khung hình với đoạn nhạc thành các file mp4.
- Mỗi bài là một file cấu hình JSON; một lệnh ra video, thêm một cờ để chỉ ra 6 khung hình xem trước.

Lưu ý quan trọng về tốc độ: không dùng bộ lọc blur của canvas cho từng hạt hay vật thể ở mỗi khung hình. Trên máy không có card đồ họa nó làm chậm gấp 6–8 lần. Muốn mờ thì vẽ lên một canvas phụ nhỏ hơn rồi phóng lên.

## Quy trình cho mỗi bài

1. Tôi gửi file mp3. Không tải nhạc từ các trang rip YouTube.
2. Tìm ảnh bìa chính thức (Apple Music hoặc Deezer), chọn bản khớp tên, nghệ sĩ và ngày phát hành.
3. Tự đo drop 1: cú đập bass lớn đầu tiên sau đoạn dạo, thường ngay sau một khoảng lặng ngắn. Xem biên độ bass ở độ phân giải 20–40 ms. Đừng nhầm cú đập lấy đà ngay trước drop, hay chỗ nhạc dày thêm về sau, là drop. Báo tôi mốc đo được.
4. Chọn đoạn cắt: đoạn dạo ngắn (drop trong 10–12 giây đầu) thì chạy từ 0:00; đoạn dạo dài thì bắt đầu 2–4 ô nhịp trước drop. Đo BPM theo chu kỳ bass.
5. Chọn màu từ ảnh bìa, màu nhấn phải nổi trên nền kính. Chọn hạt bay và vật thể nền theo chủ đề bìa.
6. Tách nền nhân vật nếu có. Bìa nhiều ánh sáng loé thì công cụ tự động hay hỏng: khoanh vùng nhân vật bằng tay rồi làm mềm mép. Ảnh tối thì kéo sáng lên.
7. Gửi tôi ảnh xem trước 6 khung hình và chờ tôi duyệt rồi mới xuất video. Mỗi lần chỉ xuất một bài.
8. Gửi tôi bản đăng TikTok, tóm tắt đoạn cắt, mốc drop, điểm, và nói rõ những chỗ bạn tự quyết.

Nếu tôi không cho điểm thì bạn tự đặt và nói rõ là tự đặt. Nếu tôi dùng tranh của họa sĩ khác làm nhân vật thì nhắc tôi ghi nguồn khi đăng.
