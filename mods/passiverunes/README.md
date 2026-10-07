# PassiveRunes

> Nexus mod 10528 · **đã phát hành** · đã test trong game

Mod DLL cho Elden Ring: **tự động cộng rune theo thời gian thực** khi đang chơi, kèm
**bonus theo mốc thời gian chơi**. Mỗi chu kỳ mod cộng một tổng gồm ba thành phần độc
lập, cộng dồn với nhau: một số rune cố định, một phần trăm chi phí lên cấp tiếp theo, và
lãi kép trên số rune đang giữ. Mọi giá trị đều có thể âm để **trừ** rune theo thời gian.

## Cấu hình

Cấu hình trong `PassiveRunes.ini` cạnh DLL (tự tạo nếu thiếu; khi nâng cấp, key mới được
thêm vào file có sẵn mà không ghi đè giá trị đã chỉnh). Có thể chỉnh:

- bật/tắt tính năng và chu kỳ cộng rune;
- số rune cố định mỗi chu kỳ;
- phần trăm chi phí lên cấp tiếp theo cộng mỗi chu kỳ;
- lãi kép theo số rune đang giữ;
- danh sách mốc thời gian chơi và số rune thưởng ở mỗi mốc;
- phím nạp lại cấu hình (áp dụng không cần khởi động lại game) và việc hiện banner sau
  khi nạp;
- ghi log để chẩn đoán.

Tên key, khoảng giá trị, giá trị mặc định và ý nghĩa nằm ở chú thích trong
[`PassiveRunes.ini`](PassiveRunes.ini); file mẫu này được nhúng vào DLL nên là nguồn sự
thật duy nhất, README không lặp lại để khỏi lệch.

## Cách hoạt động

- **Đọc/ghi rune trực tiếp** qua `WorldChrMan.main_player.player_game_data` của
  fromsoftware-rs (field `rune_count`), không AOB, không giải mã pointer chain, không patch
  code.
- **Task trên `FrameBegin`** của chính game, đăng ký bằng `common::task` (tìm `CSTaskImp`
  theo tên và đăng ký task qua AOB). Mod **không dùng bảng RVA theo từng phiên bản game**
  của fromsoftware-rs, nên một bản vá game không làm mod ngừng chạy, khác trước đây khi mỗi
  lần game cập nhật đều phải ra bản mới.
- **Chỉ cộng và chỉ đếm giờ khi người chơi đã vào world.** Ở màn hình tiêu đề hoặc màn hình
  tải thì đồng hồ tạm dừng (không reset), nên thời gian ở ngoài không tính vào mốc.
- **Ba thành phần cộng dồn mỗi chu kỳ**, kết quả luôn nằm trong khoảng từ 0 đến trần rune
  của game. **Mốc thời gian** được cộng khi thời gian chơi vượt qua mốc (không so khớp tuyệt
  đối), nên lệch nhịp tick không làm mất bonus.
- **Hot reload:** phím và banner do `common::reload` lo; mod đọc lại mọi key mỗi tick.
- **Log:** khi bật, mỗi chu kỳ ghi từng thành phần, số rune đang giữ và level; ngoài ra ghi
  phiên bản game và danh sách DLL đã nạp (`common::diag`, không in đường dẫn đầy đủ).
- Công thức từng thành phần, chi phí lên cấp, lãi kép, giá trị âm, mốc thời gian:
  [docs/rune_math.md](docs/rune_math.md). Khởi động, các lỗi đã gặp và chuyện bản vá game:
  [docs/startup_and_game_updates.md](docs/startup_and_game_updates.md).

Code: [src/rune.rs](src/rune.rs) (toàn bộ logic), [src/lib.rs](src/lib.rs) (entry point).

## Giới hạn

- Số rune đang giữ luôn bị kẹp trong khoảng từ 0 đến trần của game; không bao giờ xuống dưới
  0 dù giá trị âm lớn đến đâu.
- Lãi kép tăng theo hàm mũ nên với giá trị lớn sẽ chạm trần rất nhanh (số liệu mô phỏng ở
  `docs/rune_math.md`).
- Mốc thời gian là số rune cố định, chưa hỗ trợ theo phần trăm.
- Chi phí lên cấp là công thức cứng trong exe do cộng đồng giải ra; mod dùng công thức đó,
  đã đối chiếu khớp với màn hình lên cấp trong game.
- Thoát ra màn hình tiêu đề rồi nạp save khác vẫn đếm tiếp thời gian từ số cũ (chỉ không đếm
  phần thời gian ở ngoài game).
- Chỉ dùng ở chế độ offline, tắt EAC.

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/rune_math.md](docs/rune_math.md): cách tính rune mỗi chu kỳ và mốc thời gian.
- [docs/startup_and_game_updates.md](docs/startup_and_game_updates.md): khởi động, lỗi đã gặp, bản vá game.
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.
