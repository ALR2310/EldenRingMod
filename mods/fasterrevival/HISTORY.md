# FasterRevival: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-09-28 | Tạo mod theo ý tưởng FasterDeathAnimation (0-F), làm bản DLL thay vì patch `c0000.anibnd.dcx` để khỏi xung đột với mod animation khác; có công cụ dò `Debug.DeathProbe` | [docs](docs/death_flow.md) |
| 2026-09-28 | IDA (2.7.1.0): tìm ra hàm kill idempotent và luồng chết của game; thêm `FastDeath` gọi hàm kill sớm | [docs](docs/death_flow.md) |
| 2026-09-28 | Nhận biết chết bằng `hp <= 0` vì `death_flag` không đọc được; thêm `ToggleKey` và log thời gian mỗi lần chết; rơi vực không đổi vì game kill ngay | [docs](docs/death_flow.md) |
| 2026-09-28 | Test bị quái đánh: nhanh hơn ~6s; điều kiện kích hoạt đổi từ "anim đổi / 300ms" sang danh sách animation có event kill (`has_kill_event`) | [docs](docs/death_flow.md) |
| 2026-09-28 | Bỏ khoảng dừng "YOU DIED": ghi `soloPlayDeath_ToFadeOutTime = 0` (cùng hiệu quả FasterRespawn của ImAxel0, không cần patch code) | [docs](docs/respawn_fade.md) |
| 2026-09-28 | Hồi sinh chậm 8-20s ở vài lần là do chết gần Stake of Marika, không phải lỗi | [docs](docs/respawn_fade.md) |
| 2026-09-28 | 1.0.0: `DeathProbe` mặc định `false`, thêm trang Nexus (credit 0-F và ImAxel0); hồi sinh sau chết thường 12.2-14.4s còn ~4.2-4.7s | |
| 2026-09-28 | Bỏ `ReloadKey`, `FastDeath`, `ToggleKey`: mod luôn bật (các key đó chỉ phục vụ so sánh lúc test); `fade.rs` chỉ ghi 1 lần | |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`) | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/ | |
