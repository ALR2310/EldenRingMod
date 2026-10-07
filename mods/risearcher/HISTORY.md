# RiseArcher: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| trước 2026-08-18 | 1.16.1: sửa tĩnh `regulation.bin` bằng Smithbox Mass Edit (`csv/RiseArcher.MASSEDIT`), overhaul bow/crossbow/ballista/arrow/bolt | [docs](docs/param_patching.md) |
| 2026-08-18 | 2.0.0: viết lại thành DLL Rust (patch sống `SoloParamRepository`, cấu hình bằng ini), giữ song song cách `.MASSEDIT`; `weapon.rs` (lọc theo category) và `bullet.rs` (danh sách ID ghép bảng offset) | [docs](docs/param_patching.md) |
| 2026-09-08 | Thêm hot reload (`ReloadKey`) với baseline giá trị gốc để không cộng dồn | [docs](docs/param_patching.md) |
| 2026-09-08 | Sửa crash lặng khi khởi động: chờ `WorldChrMan::main_player` thay vì chỉ chờ `SoloParamRepository`; thêm `apply_with_retry` | [docs](docs/startup_and_ini_bugs.md) |
| 2026-09-08 | Sửa `RiseArcher.ini` không có tác dụng (key không tiền tố); tổ chức lại section theo loại tuỳ chỉnh, tách key trọng lượng/giá bán từng loại, `Bow.UnlockAOW` | [docs](docs/startup_and_ini_bugs.md) |
| 2026-09-17 | Dùng `common::task`/`common::player`/`common::reload` thay ba bản chép riêng; chờ `SoloParamRepository` vô hạn thay vì bỏ cuộc sau 5 phút | [docs](docs/startup_and_ini_bugs.md) |
| 2026-09-23 | `LogFile=false` không còn tạo file log (sửa `common::logger`; mod này vốn đã đúng hành vi đó) | |
| 2026-09-24 | Sửa đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini (`common::dll_dir`) | |
| 2026-09-26 | Log: `[INFO ]` thành `[INFO]`; thêm phiên bản game và danh sách DLL đã nạp (`common::diag`) | |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`); thêm key `ReloadBanner` | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/ | |
