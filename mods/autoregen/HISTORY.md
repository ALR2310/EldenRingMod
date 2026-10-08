# AutoRegen: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang file trong `docs/` của mod. Bản nhật ký chi tiết cũ (trước 2026-10-08) nằm trong
lịch sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-08-04 | Bản C++ (dịch ngược `AutoRecovery.dll`): hồi HP/FP/Stamina theo tick, dò lại offset, thêm Stamina; hook `OnAttack` hồi khi đánh trúng (sửa crash lệch RSP, xung đột Seamless Co-op bằng cài hook sau khi vào game) | [docs](docs/cpp_origin.md) |
| 2026-08-11 | Chỉ hồi khi đánh bằng vũ khí (`sourceType` qua vtable của source object), bỏ 3 cách phân loại bằng AtkParam/Stamina | [docs](docs/per_hit.md) |
| 2026-08-18 | Viết lại hoàn toàn bằng Rust (`fromsoftware-rs`, tick qua task `FrameBegin`), thêm hồi theo sát thương | [docs](docs/cpp_origin.md) |
| 2026-08-19 | Gộp `Regen Per Damage` vào `Regen Per Hit` (`Trigger` 0/1), thêm `Condition` (always/ngoài combat/trong combat) | |
| 2026-08-24 | Đổi ini sang kiểu SomeTweaks (`Enabled` riêng, 1 giá trị mỗi stat, `Regen.*`); key cũ vào `[Legacy]` | |
| 2026-08-25 | Thêm `Regen.PerHit.DamageType` (cận chiến / phép / cả hai) | [docs](docs/per_hit.md) |
| 2026-08-26 | Sửa bị vô hiệu hoá vĩnh viễn bởi `InvalidRva`; pin fromsoftware-rs 0.14.0; panic safety cho tick chính | [docs](docs/task_and_startup.md) |
| 2026-09-03 | Sửa mất animation đâm lén/chí mạng khi bật `Regen.PerHit` (tham số thứ 5 qua stack bị trampoline làm hỏng) | [docs](docs/per_hit.md) |
| 2026-09-03 | Thêm `Regen.PerHit.ExcludeAow` (latch theo nút bấm `L2` vs `R1`/`R2`/`L1`); đổi `Stamina` thành `SP` | [docs](docs/per_hit.md) |
| 2026-09-10 | Thêm `Regen.PerTick.Trigger=3` (Idle, đệm 5s) và `=4` (gesture ngồi); tìm ra `requested_gesture` bằng 1/2 `GESTURE_ID`; banner khi bấm `ReloadKey` | [docs](docs/gesture_trigger.md) |
| 2026-09-11 | Thay `rva::get()` bằng quét AOB cho đăng ký tick (`task_hook`) và allocator (`alloc_hook`), mod sống sót qua bản game mới | [docs](docs/task_and_startup.md) |
| 2026-09-12 | Viết lại `Regen Per Hit` bằng Ghidra: `hit_hook.rs` hook entry point thật, hết crash vào Volcano Manor và hết lỗi animation | [docs](docs/per_hit.md) |
| 2026-09-12 | Sửa panic trong fake vtable của `task_hook` (nghi nguồn giật hình sau 2.5.0) | [docs](docs/task_and_startup.md) |
| 2026-09-14 | Đổi tên key `Unit` thành `ValueType`, `PerHit.Trigger` thành `Mode`; thêm `ValueType=2` (% máu đã mất) và `Cap`; đổi `[Debug] RegenLog` thành `[Logging] LogFile` | |
| 2026-09-14 | Tách rồi gộp crate `engine` vào `common`; giảm spam log `Regen.PerTick` | [docs](docs/task_and_startup.md) |
| 2026-09-14 | Thử đổi tín hiệu gesture sang `queued_action_inputs` làm Sitting mất tác dụng, revert; 3 field khác cũng sai | [docs](docs/gesture_trigger.md) |
| 2026-09-17 | Lần thử thứ 4 thành công: xác nhận gesture qua TAE `anim_id` ổn định; vá bấm dồn dập; đổi tên "Sitting" thành "Gesture" (`Regen.PerTick.GestureId`) | [docs](docs/gesture_trigger.md) |
| 2026-09-17 | Giảm log per-hit và dedupe log `Regen.PerTick`; gộp `ActionSnapshot` và `show_announcement` vào `common` | [docs](docs/task_and_startup.md) |
| 2026-09-22 | Nới timeout xác nhận gesture 3000ms thành 6000ms (quay gấp trước khi ngồi làm regen không hồi) | [docs](docs/gesture_trigger.md) |
| 2026-09-23 | `LogFile=false` không còn tạo file log (sửa `common::logger`) | [docs](docs/task_and_startup.md) |
| 2026-09-24 | Sửa đường dẫn DLL có ký tự không ASCII làm mod bỏ qua ini (`common::dll_dir`) | [docs](docs/task_and_startup.md) |
| 2026-09-26 | Log: `[INFO ]` thành `[INFO]`; thêm phiên bản game và danh sách DLL đã nạp (`common::diag`) | [docs](docs/task_and_startup.md) |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`); thêm key `ReloadBanner` | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; đường dẫn script/dump dịch ngược đổi chỗ | |
| 2026-10-08 | Tách README thành README + HISTORY + docs/ | |
