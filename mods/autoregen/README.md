# AutoRegen

> Nexus mod 10548 · **đã phát hành** · đã test trong game

Mod DLL cho Elden Ring tự động hồi HP/FP/Stamina: **hồi theo thời gian** (mỗi
tick, có điều kiện: luôn, ngoài/trong giao tranh, đứng yên, đang làm gesture
như ngồi) và/hoặc **hồi khi đánh trúng** (số cố định, % tối đa hoặc % sát
thương gây ra). Cấu hình trong `AutoRegen.ini`, bấm `ReloadKey` để nạp lại mà
không cần khởi động lại game.

## Cấu hình

Cấu hình trong `AutoRegen.ini` cạnh DLL (tự tạo nếu thiếu; key mới được thêm vào file có sẵn mà không ghi đè giá trị đã chỉnh, key đã đổi tên được dồn vào `[Legacy]`). Có thể chỉnh:

- hồi theo thời gian: điều kiện kích hoạt, chu kỳ, cách tính (điểm cố định, % tối đa, % phần đã mất), trần hồi phục, danh sách gesture;
- hồi khi đánh trúng: cách tính, loại đòn tính (cận chiến / phép / cả hai), loại trừ Weapon Art / Ash of War;
- phím nạp lại cấu hình và việc hiện banner sau khi nạp;
- ghi log để chẩn đoán.

Tên key, giá trị mặc định và ý nghĩa nằm ở chú thích trong [`AutoRegen.ini`](AutoRegen.ini); file mẫu này được nhúng vào DLL nên là nguồn sự thật duy nhất, README không lặp lại để khỏi lệch.

## Cách hoạt động

- **Tick** (`src/regen.rs`): task đăng ký vào `CSTaskGroupIndex::FrameBegin` của game (không phải thread `Sleep` riêng), đọc/ghi `WorldChrMan` → `ChrIns` → `CSChrDataModule` qua `fromsoftware-rs`, gate bằng `common::player` (đã vào game). Việc đăng ký task và lấy allocator dùng quét AOB thay vì bảng RVA của fromsoftware-rs để chạy được trên bản game mới: [docs/task_and_startup.md](docs/task_and_startup.md).
- **Điều kiện combat / idle / gesture**: "đang giao tranh" xấp xỉ bằng 15 giây kể từ lần đánh/bị đánh gần nhất; idle = đứng yên 5 giây; gesture = chốt khi game thật sự chạy animation gesture, xác nhận sau khoảng trễ qua TAE `anim_id` để không tính gesture bị chặn giữa lúc cast: [docs/gesture_trigger.md](docs/gesture_trigger.md).
- **Hồi khi đánh trúng** (`src/hit_hook.rs`): hook entry point hàm hit-resolution của game (AOB), đọc attacker, damage, loại nguồn sát thương; phân biệt Weapon Art bằng nút bấm gần nhất: [docs/per_hit.md](docs/per_hit.md).
- **Hot reload:** watcher `ReloadKey` riêng trong `regen.rs`, banner "AutoRegen: config reloaded".
- **Log:** ghi phiên bản game và danh sách DLL đã nạp (`common::diag`, không in đường dẫn đầy đủ).

Nguồn gốc dịch ngược và bản C++ cũ: [docs/cpp_origin.md](docs/cpp_origin.md).

## Giới hạn

- "Đang giao tranh" chỉ là xấp xỉ (đứng yên né đòn boss không tính là combat).
- Phép có động tác cận chiến (vd. Carian Greatsword) bị tính là đòn cận chiến.
- Loại trừ Ash of War dựa trên nút bấm: Ash 2 bước như Unsheathe (`L2` rồi `R1`/`R2`) có thể bị tính là đòn thường (chưa sửa, xem [docs/per_hit.md](docs/per_hit.md)).
- Hook hit mới verify trên 1 bản game (2.7.1.0 / 1.17.1); nếu AOB không tìm thấy thì chỉ tắt tính năng hồi khi đánh trúng, phần hồi theo thời gian vẫn chạy.
- Chỉ dùng ở chế độ offline hoặc Seamless Co-op, tắt EAC.

## Tài liệu liên quan

- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [docs/per_hit.md](docs/per_hit.md): hook hồi khi đánh trúng, phân loại đòn, ExcludeAow, các lỗi đã gặp.
- [docs/gesture_trigger.md](docs/gesture_trigger.md): trigger Idle và Gesture, các hướng đã thử.
- [docs/task_and_startup.md](docs/task_and_startup.md): khởi động, đăng ký tick bằng AOB, banner, log.
- [docs/cpp_origin.md](docs/cpp_origin.md): dịch ngược `AutoRecovery.dll` và bản C++ (lịch sử).
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.
