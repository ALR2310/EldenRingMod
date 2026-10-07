# WindowResize: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-09-29 | Khảo sát: không cần dịch ngược, chỉ cần thêm `WS_THICKFRAME` và subclass WndProc (game vốn tạo cửa sổ không có viền kéo) | [docs](docs/resize_approach.md) |
| 2026-09-29 | Bản đầu: tìm cửa sổ, subclass, `WM_SETCURSOR`/`WM_GETMINMAXINFO`/`WM_SIZING`, watchdog 1 giây | [docs](docs/resize_approach.md) |
| 2026-09-29 | Định đăng Nexus: thêm `nexus_page.bbcode`, version `1.0.0`; test trong game, kéo resize hoạt động | |
| 2026-09-29 | Bỏ hot reload và log môi trường: mod chỉ còn Win32, chạm vào game ít nhất; đổi ini cần khởi động lại game | [docs](docs/resize_approach.md) |
| 2026-09-29 | Thêm `Width`/`Height` (kích thước khởi động, áp 20 giây đầu, dừng khi người chơi kéo), DPI per-monitor | [docs](docs/resize_approach.md) |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`) | |
| 2026-10-07 | Quyết định **không đăng lên Nexus**, chỉ dùng nội bộ | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/ | |
