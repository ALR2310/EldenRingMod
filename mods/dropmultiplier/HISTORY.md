# DropMultiplier: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `research/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong
lịch sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-09-14 | Tách `Drop Rate` khỏi `sometweaks` thành mod độc lập (1.0.0): cùng thuật toán, bỏ tiền tố `DropRate.` trong key ini, dùng chung `common` (task/player/reload), không mang theo rủi ro `rva::get()` khóa theo phiên bản của bản gốc | |
| 2026-09-17 | Bỏ tiền tố `DropMultiplier:` trong log; sửa `common::player::wait_for_solo_param_repository` chờ vô hạn thay vì bỏ cuộc sau 5 phút (lỗi thật: vào world muộn thì mod không tự áp) | |
| 2026-09-17 | Banner "Config reloaded" khi bấm `ReloadKey` (`common::announce`) | |
| 2026-09-23 | `LogFile=false` không còn tạo file log (sửa `common::logger`) | |
| 2026-09-24 | Sửa đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini (`common::dll_dir` dùng `GetModuleFileNameW`) | |
| 2026-09-26 | Log: `[INFO ]` thành `[INFO]`; thêm phiên bản game và danh sách DLL đã nạp (`common::diag`) | |
| 2026-09-27 | 1.0.2: sửa panic với Convergence, duyệt param theo index qua `common::params` | [research](../../research/dropmultiplier_convergence_panic.md) |
| 2026-09-28 | 1.1.0: thêm `[Materials]` (Crafting/Upgrade/Unique); đổi `[Settings]` thành `[Drop]` và `ChancePercent` thành `Percentage` (tự chuyển giá trị cũ) | [research](../../research/dropmultiplier_materials.md) |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`); thêm key `ReloadBanner` | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + research | |
