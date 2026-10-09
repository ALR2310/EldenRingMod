# SpiritMultiplier: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang file trong `docs/` của mod. Bản nhật ký chi tiết cũ (trước 2026-10-08) nằm trong
lịch sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-09-25 | Tạo mod, thêm `SlotProbe`: đo `summon_buddy_chr_set` có capacity 80, band người chơi bắt đầu ở slot 20, trần 10 nằm ở `sub_140493380` | [docs](docs/slots_and_band.md) |
| 2026-09-25 | 0.2.0: nới band lên 60 slot (patch tìm slot + next-cursor) và `chain.rs` (`Multiplier`/`Amount`); test 30 sói, dismiss sạch | [docs](docs/slots_and_band.md) |
| 2026-09-25 | `MaxSpirits`: nới luôn capacity của ChrSet (test 100 con, offline); chốt mặc định `Multiplier=2`, `Amount=0`, `MaxSpirits=60` là mức tối thiểu | [docs](docs/slots_and_band.md) |
| 2026-09-26 | Seamless Co-op: chia band theo capacity cho người chơi index 1-5 (Seamless nới capacity lên 1000); sửa next-cursor cho band không bắt đầu ở #20 | [docs](docs/slots_and_band.md) |
| 2026-09-26 | Kẻ địch biến mất khi có 60 spirit: spirit chiếm suất trong giới hạn 60 nhân vật active; `activate_limit.rs` nâng giới hạn theo số spirit (luôn bật, bỏ key `KeepEnemySlots`); `EnemyProbe` gây crash nên tắt | [docs](docs/activate_limit.md) |
| 2026-09-27 | Log phiên bản game + danh sách DLL đã nạp (`common::diag`); `GhostColor` ban đầu gỡ SpEffect 295xxx mỗi frame, rồi chuyển sang sửa NpcParam (hot reload 2 chiều) | [docs](docs/ghost_color.md) |
| 2026-09-29 | `Regen` (spirit và Torrent tự hồi máu, copy từ SomeTweaks); tra cứu cờ hành vi spirit (297xxx) | [docs](docs/ghost_color.md) |
| 2026-09-29 | Thử `WarpDistance`/`WarpBlockedTime`: ghi được ngưỡng nhưng spirit không dịch chuyển, gỡ trước 1.0.0, hoãn | [docs](docs/warp_research.md) |
| 2026-09-29 | 1.0.0 phát hành (Nexus mod 11168) | |
| 2026-09-29 | `MultiSpirit`: gọi nhiều Ash khác nhau cùng lúc (patch DoSummon/Update/UI/CanUseItem, giữ bia đá), cho về từng Ash, Ash gọi thêm tốn FP/HP; viết lại từ phân tích Solid Uncapper | [docs](docs/multi_spirit.md) |
| 2026-09-29 | `NoRestResummon` và `SummonAnywhere` (sửa `BuddyStoneParam`) | [docs](docs/summon_anywhere.md) |
| 2026-09-29 | 1.1.0 phát hành (`SummonAnywhere` mặc định `true`) | |
| 2026-09-30 | 1.1.1: `SummonAnywhere` không còn phụ thuộc bia đá được nạp (patch stateInfo 373, in-range, bia dự phòng) | [docs](docs/summon_anywhere.md) |
| 2026-09-30 | 1.1.2: `GhostColor` tắt ở `SpEffectVfx` (`phantomParamOverwriteType = 0`) thay vì gỡ SpEffect ô 26, chạy được trên Reforged; quy tắc tỉ lệ spirit/không-spirit để khỏi tắt vfx của kẻ địch | [docs](docs/ghost_color.md) |
| 2026-10-01 | 1.1.3: Ash đã nâng cấp (+1..+10) cho về đúng thay vì gọi thêm (làm tròn SpEffect `100 * (id / 100)` như `DoSummon`) | [docs](docs/multi_spirit.md) |
| 2026-10-01 | 1.1.4: mọi patch code đổi sang đổi đích `call` (`common::codepatch::redirect_rel32`) vì Seamless Co-op 2.0.1 tự huỷ game khi chữ ký byte không khớp | [docs](docs/seamless_conflict.md) |
| 2026-10-01 | 1.1.4: Reforged - sửa Spirit Ring của Spiritcaller hỏng do `SummonAnywhere` ép cờ in-range mỗi frame; Spirit-Severing Blade không bị `MultiSpirit` chặn | [docs](docs/summon_anywhere.md) |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`); thêm key `ReloadBanner` | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; đường dẫn script/dump dịch ngược đổi chỗ (`.docs/` thành `tmp/`, script IDA ở `scripts/ida/`) | |
| 2026-10-08 | Tách README thành README + HISTORY + docs/; các vấn đề còn mở (Reforged Fury, Seamless mất màu ma, warp) ghi ở `docs/open_issues.md` | [docs](docs/open_issues.md) |
| 2026-10-08 | `MultiSpirit`: spirit đã chết vẫn nằm trong `groups` nên bấm lại Ash của nó (khi Ash khác còn sống) bị coi là "đang out" và chỉ thu hồi; nay kiểm tra HP chr_ins. Đã test in-game (2026-10-08): đúng | |
| 2026-10-08 | Thêm `SpiritThink.ini` (tuỳ chọn, thử nghiệm): chỉnh field NpcThinkParam của spirit khi đang chơi, không cần build lại, để nghiên cứu độ hung hăng | [docs](docs/think_override.md) |
| 2026-10-09 | Sửa `NoRestResummon=false` không có tác dụng khi `SummonAnywhere=true` (báo cáo trên Nexus): stub stateInfo 373 ép "được dùng" nên đè luôn khoá của vanilla; nay mod tự đọc cờ `summonedEventFlagId` gốc của bia và tắt stub khi cờ bật và không còn spirit. Đã test in-game (2026-10-09): đúng | [docs](docs/summon_anywhere.md) |
| 2026-10-09 | Thêm `CloneSpirit` (thử nghiệm, true/false, mặc định false): bấm lại Ash đang out thì gọi thêm thay vì cho về, tới `MaxSpirits` thì mới cho về; theo đề xuất của almasakmal123 trên Nexus. Đã test in-game (2026-10-09): đúng | [docs](docs/multi_spirit.md) |
| 2026-10-09 | Sửa chi phí FP/HP: `cost_state` đọc `refId_default` ở +0 (luôn 0) thay vì +4 nên chỉ lần gọi đầu tốn mana; thêm patch 8 (2 chỗ trong `CanUseItem`) để `CloneSpirit` không gọi được khi thiếu FP/HP. Đã test in-game: đúng | [docs](docs/multi_spirit.md) |
| 2026-10-09 | Dịch chuyển spirit: đọc máy trạng thái `warp_manager` trong IDA; thêm `WarpDistance`/`WarpBlockedTime`/`WarpStuckTime`/`WarpStuckRange` (hạ ngưỡng) và `WarpWhenFar` (yêu cầu dịch chuyển cả khi chỉ ở xa); kiểm AOB bố cục, bỏ spirit của người chơi khác. Đã test in-game bản trước khi thêm kiểm tra an toàn | [docs](docs/warp_research.md) |
| 2026-10-09 | Thêm `ImproveSenses` / `ImproveAggression` / `ImproveFollow` (mặc định true): bộ giá trị AI spirit lấy từ Age of Spirit, ghi vào NpcThinkParam theo kiểu chỉ nâng / chỉ hạ; bỏ log tìm SpEffect rơi. Chưa test từng nhóm | [docs](docs/think_override.md) |
| 2026-10-09 | Thêm `RegenMode` (1 HP cố định, 2 % HP tối đa, 3 % HP đã mất) và `RegenValue` thay cho key `Regen` (tự đổi tên trong ini cũ, giữ nguyên giá trị; mặc định mode 2 = hành vi cũ). Chưa test in-game | |
