# Khởi động, đăng ký tick, tránh `rva::get()`

**Status 2026-10-08: hoạt động. Phần dùng chung đã chuyển lên `common` (`shared/src/{task_hook,alloc_hook,task,player,reload,announce}.rs`); mod chỉ còn `src/lib.rs`, `src/regen.rs`, `src/hit_hook.rs`.**

## Vì sao không dùng `eldenring::rva::get()`

Bảng RVA của fromsoftware-rs chỉ có **1** phiên bản game; chạy trên bản mới hơn thì panic ngay (`InvalidRva`, lần đầu báo từ bình luận Nexus 2026-08-26: "CSTaskImp never became available (InvalidRva)"). `CSTaskImp::wait_for_instance` coi `InvalidRva` là lỗi chết, không retry (đua timing với lúc Arxan giải nén/relocate).

Giải pháp hiện tại (2026-09-11):

- `task_hook`: quét AOB tìm hàm `register_task` trong `.text` (byte-identical trên exe 1.16.2, RVA `0xeb1fe0`, và 1.17.0, `0xeb3de0`; neo vào thân hàm ổn định, không neo RVA). Tự dựng fake C++ vtable (`vtable-rs`) vì cơ chế `RecurringTask` của `fromsoftware-shared` là private.
- `alloc_hook`: AOB cho getter `DLAllocator::runtime_heap_allocator()` (global, không phải Dantelion2 singleton), byte-identical 1.16.2 và 1.17.0. Dùng bởi `common::announce::show_announcement`.
- `wait_for_cs_task`: poll `CSTaskImp::instance()` qua `FromStatic` (tra theo tên singleton `CSTask`), không qua `wait_for_instance()`.
- Mod sống sót qua bản patch game mới mà không chờ fromsoftware-rs; chỉ tính năng nào AOB không tìm thấy mới tắt riêng (có log).

## Panic trong fake vtable (2.5.1)

`get_runtime_class` và `destructor` của `Task` từng là `unimplemented!()`; engine gọi trực tiếp qua vtable (không qua `catch_unwind`), panic unwind vào stack C++ là UB - nghi nguồn giật hình 1-2 giây mỗi 15-20 giây (cả ở menu) sau 2.5.0. Sửa: `get_runtime_class` trả 0, `destructor` no-op (an toàn vì `Task` được `Box::leak`). Chưa tái hiện được hiện tượng gốc nên ghi changelog như fix, không khẳng định chắc.

`run_recurring_safe` bọc `cs_task.run_recurring` bằng `catch_unwind` (workspace bỏ `panic = "abort"`, mặc định `unwind`); version pin `eldenring`/`fromsoftware-shared` 0.14.0 ở workspace.

## Lịch sử tách/gộp crate

2026-09-14: `task_hook`/`alloc_hook`/`wait_for_cs_task`/`run_recurring_safe`/`main_player_chr_ins_ptr` tách ra crate `engine` (vì `DropMultiplier` sắp phải copy lần 2), cùng ngày gộp ngược vào `shared` (`common`) - kích thước DLL không đổi nhờ `lto = true`, cái giá chỉ là build riêng lẻ lần đầu phải compile `eldenring`. 2026-09-17: `ActionSnapshot` chuyển thành `common::player::main_player_action_snapshot` (sometweaks dùng chung `r1/r2/l1/l2`); `show_announcement` sang `common::announce`.

## Banner reload

`CSMenuManImp::system_announce_view_model` (`FeSystemAnnounceViewModel`, banner cuộn "Autosaving..."): tạo `AnnounceNotification { is_active: true, message: MenuString { allocated_string: DLString::from_str(text, runtime_heap_allocator) } }` rồi `push_back` vào `notifications`; game tự chạy fade-in/scroll/fade-out. Khác `display_status_message` (chỉ nhận ID cố định). Mod này vẫn có watcher `ReloadKey` riêng trong `regen.rs` (banner "AutoRegen: config reloaded", key `ReloadBanner`), chưa dùng `common::reload::run`.

## Log

`LogFile=false` không tạo file (sửa tập trung trong `common::logger` 2026-09-23); `dll_dir` dùng `GetModuleFileNameW` (2026-09-24, DoctorHigh: đường dẫn có ký tự không ASCII làm mod bỏ qua ini); nhãn level `[INFO]` không thừa khoảng trắng; `common::diag` ghi phiên bản game + DLL đã nạp (không in đường dẫn đầy đủ). Version thật trong thuộc tính file DLL (`build.rs` + `winresource`).
