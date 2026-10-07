# PassiveRunes: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| trước 2026-08-18 | Bản C++ 1.0.0: một thread `Sleep` cộng rune qua pointer chain tự dò từ DLL cheat khác (`GameDataMan` +0x8 +0x6C), kèm bonus theo mốc thời gian | [docs](docs/startup_and_game_updates.md) |
| 2026-08-18 | 2.0.0: viết lại bằng Rust; đọc/ghi `rune_count` qua fromsoftware-rs, tick là task `FrameBegin`; mốc thời gian đổi sang so ngưỡng (không bỏ sót bonus khi lệch nhịp) | [docs](docs/rune_math.md) |
| 2026-08-24 | Sửa hot reload (trước đó chưa từng đọc `ReloadKey`); đổi ini sang format chung `Rune.Passive.*`/`Rune.Milestone` (đơn vị `Interval` thành mili giây) | [docs](docs/startup_and_game_updates.md) |
| 2026-08-24 | Sửa hai lỗi khởi động: race `CSTaskImp` không retry `InvalidRva`; cộng rune cả khi còn ở màn hình chờ (gate `WorldChrMan.main_player`), và mốc bị bỏ lỡ nếu cộng thất bại | [docs](docs/startup_and_game_updates.md) |
| 2026-08-26 | Pin `fromsoftware-rs` 0.14.0, thêm `wait_for_system_init`, bọc tick bằng `catch_unwind` | [docs](docs/startup_and_game_updates.md) |
| trước 2026-09-23 | 2.0.1 và 2.0.2: cập nhật cho ER 1.17 và 1.17.1 (khi đó mod còn phụ thuộc bảng RVA theo phiên bản của fromsoftware-rs) | [docs](docs/startup_and_game_updates.md) |
| 2026-09-14 | Đổi `[Debug]`/`RuneLog` thành `[Logging]`/`LogFile` cho thống nhất với các mod khác | |
| 2026-09-23 | 2.1.0: thêm `Rune.Passive.Percent` (rune theo % chi phí lên cấp tiếp theo), đã xác minh công thức trong game | [docs](docs/rune_math.md) |
| 2026-09-23 | Bỏ hẳn phụ thuộc `rva::get()`: dùng `common::task` và `WorldChrMan`; `ReloadKey` qua `common::reload` (thêm banner); cache AOB/`CSTaskImp` trong `common` | [docs](docs/startup_and_game_updates.md) |
| 2026-09-23 | Thêm `Rune.Passive.Interest` (lãi kép), cho phép giá trị âm (trừ rune), kẹp giá trị theo khoảng ghi trong ini, đổi mặc định `Amount` và `Percent` | [docs](docs/rune_math.md) |
| 2026-09-23 | Đồng hồ session/interval chỉ chạy khi đã vào game; log chi tiết hơn (từng thành phần, rune đang giữ, level) | [docs](docs/rune_math.md) |
| 2026-09-23 | `LogFile=false` không còn tạo file log (sửa `common::logger`) | |
| 2026-09-24 | 2.1.1: sửa đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini (`common::dll_dir`) | |
| 2026-09-26 | Log: `[INFO ]` thành `[INFO]`; thêm phiên bản game và danh sách DLL đã nạp (`common::diag`) | |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`); thêm key `ReloadBanner` | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/ | |
