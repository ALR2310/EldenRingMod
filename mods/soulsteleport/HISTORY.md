# SoulsTeleport: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-09-24 | Bản thử `teleporttest`: tìm ra trình tự warp của game (`RequestMoveMap`, `SetCustomSpawn`, `warp_requested`), ghi thẳng cờ để không làm rớt phiên; xác nhận trong game trên 3 block | [docs](docs/warp_mechanism.md) |
| 2026-09-24 | AOB thay cho bảng RVA theo phiên bản (2.7.0.0 panic `Unsupported game version`); đổi tên thành `soulsteleport` | [docs](docs/warp_mechanism.md) |
| 2026-09-24 | Khảo sát Seamless Co-op: vị trí đồng đội ở xa không đọc được, nên gửi vị trí qua Steam P2P; broadcast thử rồi teleport xa thành công, không rớt phiên | [docs](docs/position_exchange.md) |
| 2026-09-24 | Đổi sang hỏi khi cần (`WHERE`/`HERE`), chỉ đồng đội, nhớ vai trò khi `PlayerIns` tạm biến mất, sửa phím lọt sang instance khác | [docs](docs/position_exchange.md) |
| 2026-09-24 | Hook lệnh `/teleport` của SoulsChat v0.1.2 (chạy được lúc Enter) | [docs](docs/menu_ui.md) |
| 2026-09-25 | Bỏ hướng addon SoulsChat, thành mod độc lập có menu ImGui riêng (hudhook); rút gọn cấu hình, tự co giãn theo độ phân giải, theme, font Noto Sans nhúng, hiện mọi người kèm vai trò | [docs](docs/menu_ui.md) |
| 2026-09-25 | Sửa chuột (nhập chuột riêng, hook `SetCursorPos`/`ClipCursor`), chặn chuột trong game (DirectInput), lỗi mất con trỏ ở cửa sổ nhỏ (`ScaleAllSizes` làm tròn); sửa warp tới người đang loading bị văng ra title | [docs](docs/menu_ui.md) |
| 2026-09-25 | Lưu vị trí/kích thước menu vào ini, nút Reset, `MenuScale` (thêm `config::set_values` vào `common`) | [docs](docs/menu_ui.md) |
| 2026-09-25 | Review trước phát hành: chống giả `HERE`/`WHERE` (người gửi do Steam xác thực, request id ngẫu nhiên, lọc tọa độ rác); 1.0.0 phát hành (Nexus 11119) | [docs](docs/position_exchange.md) |
| 2026-09-26 | 1.1.0: chặn teleport giữa người thù địch (invader), log phiên bản game và danh sách DLL, nhãn `[INFO]` | [docs](docs/roles_and_invaders.md) |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`) | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/; kiểm tra bằng IDA: hai mẫu AOB của hàm warp duy nhất trên 2.6.2.0, 2.7.0.0, 2.7.1.0 | [docs](docs/warp_mechanism.md) |
