# SomeTweaks: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang file trong `docs/` của mod. Bản nhật ký chi tiết cũ (trước 2026-10-08) nằm trong
lịch sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-08-18 | Đổi tên LifeBetween thành SomeTweaks khi gộp vào workspace | |
| 2026-08-22 | Gom nhóm lại ini: `Enabled` / `Trigger` / `ValueType` cho `[Regen Per Tick]` và `[Regen Per Hit]`, thêm theo dõi combat | |
| 2026-08-23 | Thêm `[Rune Reward]` (port từ `passiverunes`) | [docs](docs/misc_patches.md) |
| 2026-08-24 | Thêm `Rune.Multiplier`, `WeightMultiplier`, `[Drop Rate]` (2 chế độ nhân hệ số / ép tỉ lệ); sửa `InvalidRva` làm tắt vĩnh viễn, rune cộng trước khi vào world, crash do `SoloParamRepository` resolve sớm, hot-reload `DropRate` không chạy | [docs](docs/misc_patches.md) |
| 2026-08-24 | Thử rồi bỏ `Misc.GraceOnTorrent` (cần EMEVD event mới) | [docs](docs/grace_menu.md) |
| 2026-08-25 | Thêm `TorrentAnywhere`, `UnlockAshesOfWar`, `UnlockEnchantments`, `Rune.KeepOnDeath` (đổi từ cờ `has_dropped_runes` sai sang NOP lệnh `CALL`), `Regen.PerHit.DamageType`; tổ chức lại `src/` theo section ini | [docs](docs/misc_patches.md) |
| 2026-08-25 | Giải mã `er10x.dll`, thêm `[Spirit]` (Color, Regen, Summon.Anywhere) | [docs](docs/spirit.md) |
| 2026-08-26 | Pin fromsoftware-rs 0.14.0, `run_recurring_safe` (bỏ `panic = "abort"`); `Spirit.Enabled`; `Spirit.Summon.Amount`/`Multiplier` qua `ChainingMap`, sửa nhân dồn | [docs](docs/spirit.md) |
| 2026-08-28 | `GraceMenu` (Nâng cấp / Mua / Bán / UnlockShop): tìm ra nguyên nhân bug đứng dậy (điều kiện chờ sai loại menu), state mới thay vì hijack, hook `get_message` cho text tuỳ biến; dọn log (gate `CSTaskImp` 1 lần, level) | [docs](docs/grace_menu.md) |
| 2026-08-28 | Thêm `WarpAnywhere` (port từ `Zibinha_FastTravel.dll`) | [docs](docs/misc_patches.md) |
| 2026-08-29 | Sửa crash `Spirit.Summon` thiếu gate vào world; điều tra giật khi "Mua", chấp nhận giới hạn | [docs](docs/grace_menu.md) |
| 2026-08-30 | Thêm rồi (2026-09-06) xoá `Enemy Scaling` | [docs](docs/misc_patches.md) |
| 2026-09-03 | Sửa mất animation đâm lén (tham số thứ 5 qua stack); thêm `Regen.PerHit.ExcludeAow`; fallback GraceMenu khi hook `get_message` không cài được | [docs](docs/grace_menu.md) |
| 2026-09-03 | `Spirit.Summon.Anywhere`: spirit tự biến mất, port Hook A/B, thử ép cờ in-range (đều không đủ) | [docs](docs/spirit.md) |
| 2026-09-04 | `Spirit.Summon.Anywhere`: phát hiện test bị nhiễu bởi `er10x.dll`, xoá patch NOP + tick ép cờ, đổi điểm patch gate để chung sống với Seamless, thay Hook A (sai bản chất, spirit đi hướng Đông) bằng `kSigDespawn` | [docs](docs/spirit.md) |
| 2026-09-04 | Sửa `common::config::migrate` không dồn key thừa vào `[Legacy]` khi không có key thiếu | |
| 2026-09-14 | Đổi tên key (`Unit` thành `ValueType`, `PerHit.Trigger` thành `Mode`), `[Debug]`/`DebugLog` thành `[Logging]`/`LogFile` | |
| 2026-09-17 | Dùng `common::task`/`reload`/`player`; xóa `player.rs`; sửa `wait_for_solo_param_repository` bỏ cuộc sau 5 phút | |
| 2026-09-27 | Duyệt `ItemLotParam_enemy` qua `common::params::for_each_row_mut` (tránh panic với Convergence) | |
| 2026-09-29 | Bị bỏ: không còn kế hoạch phát triển, không backport sửa lỗi | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; đường dẫn script/dump dịch ngược đổi chỗ | |
| 2026-10-08 | Tách README thành README + HISTORY + docs/ (mod đã bỏ, README ghi trạng thái và nơi từng tính năng sống) | |
