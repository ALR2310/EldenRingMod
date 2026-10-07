# RuneMultiplier: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-08-14 | Bản C++: ban đầu có 2 hook, giết quái bị nhân 2 lần (64 ra 256); bỏ hook ở bước nhân thưởng, giữ 1 hook `AddSoul_Call`; test trong game ổn | [docs](docs/addsoul_hook.md) |
| 2026-08-18 | 1.0.0: viết lại bằng Rust; jump tuyệt đối thay `E9 rel32` (bỏ `CodePatch.cpp`); ini/log/hotkey dùng `common`; reload bằng task `FrameBegin` | [docs](docs/addsoul_hook.md) |
| 2026-08-19 | 1.0.1: sửa việc nhân luôn cả rune chi tiêu (stub bỏ qua `amount <= 0`); đổi key ini `RuneMultiplier` thành `Multiplier`, `HotReloadKey` thành `ReloadKey` (mặc định `F9` thành `F5`) | [docs](docs/addsoul_hook.md) |
| 2026-08-26 | Pin `fromsoftware-rs` 0.14.0; chờ `CSTaskImp` có retry khi gặp `InvalidRva` (trước đó lỗi này tắt reload vĩnh viễn); tick reload bọc `catch_unwind` | |
| 2026-08-31 | 1.0.2: cập nhật cho ER 1.17 | |
| 2026-09-09 | 1.0.3: cập nhật cho ER 1.17.1 | |
| 2026-09-14 | Đổi `[Debug]`/`DebugLog` thành `[Logging]`/`LogFile` cho thống nhất với các mod khác | |
| 2026-09-23 | `LogFile` do `common::logger` tự gate (mod này vốn đã đúng hành vi đó, bỏ điều kiện riêng) | |
| 2026-09-24 | 1.0.4: sửa đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini (`common::dll_dir`) | |
| 2026-09-26 | Dòng lỗi `[INFO ] ERROR:` chuyển sang `logger::error`; `[INFO ]` thành `[INFO]`; thêm phiên bản game và danh sách DLL vào log (`common::diag`) | |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`) | |
| 2026-10-07 | Dùng `common::task::wait_for_cs_task` và `run_recurring_safe` thay hai bản chép riêng trong `hook.rs` (-65 dòng); task reload giờ đăng ký bằng AOB của `common::task_hook` thay vì `rva::get()` khoá theo phiên bản game; bỏ dependency `fromsoftware-shared` không còn dùng | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/; kiểm tra bằng IDA: mẫu AOB neo duy nhất trên 2.6.2.0, 2.7.0.0, 2.7.1.0 | [docs](docs/addsoul_hook.md) |
