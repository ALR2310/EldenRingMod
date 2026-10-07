# Tốc độ kẻ địch và boss

**Status 2026-10-07: đã làm (`enemy.rs`), test trong game ổn (kẻ địch thường, Margit, Godrick, Tree Sentinel). Chưa hỗ trợ Seamless Co-op.**

`[Enemy] All` ghi `animation_speed` của kẻ địch giống người chơi và Torrent. Chỉ một key, không chia nhóm hành động: anim id của mỗi con khác nhau (mỗi con một bộ), `group_of()` không dùng được.

## Cách lọc "kẻ địch"

Duyệt `WorldChrMan.chr_sets` (chỉ entry `Active` / `ReadyForActivation`), lấy nhân vật thoả cả ba:

1. `chr_type` là `Npc` hoặc `Unk7`. **Boss là `Unk7`** (lock-on log: Margit `21300014`, Death Bird, Ulcerated Tree Spirit), Tree Sentinel Limgrave là `Npc`. Bản đầu chỉ nhận `Npc` nên Margit và Godrick không chạy; `chr_type` không có cột tương ứng trong param, gần nhất là `NpcParam.npcType` (1 = boss) nhưng không trùng hẳn.
2. `team_type` không thuộc nhóm không-phải-kẻ-địch (enum `TEAM_TYPE` của Smithbox: 0 None, 1 Live, 2-4 Ghost, 5 Wandering Ghost, 8 Ally, 10 Decoy (Torrent), 12 Battle Ally, 13/16-18 Invader, 14 Neutral, 15 Charmed, 19 Host, 20 Co-op, 26 Friendly NPC, 28 Co-op NPC, 30 Object, 31/32 Mad Phantom, 47 Spirit Summon). Còn lại (6 Enemy, 7 Boss, 9 Hostile Ally, 11, 24, 25, 27 Hostile NPC, 33 Arch Enemy, 48/51/54/55/59/60/63/66 "Unknown") là kẻ địch.
3. `NpcParam.threatLv > 0`: là 0 với scarab, hươu, dê, cừu, lợn rừng, chim, rùa, Wandering Noble, Ballista, dummy, Bonfire, Torrent; boss từ 1 trở lên. Không có trên `ChrIns`, nên bảng id -> threatLv được dựng 1 lần bằng `common::params::row_ids` + `get_row_by_index` (không dùng lookup table, xem `shared/src/params.rs`).

`team` không đủ: Goat cùng `team=6` (Enemy) với quái thường.

## Tương tác

- Nạn nhân của backstab/riposte (throw state 2/4/6) do `sync_victims` điều khiển, `enemy::apply` bỏ qua trong lúc đó (xem [critical_victim.md](critical_victim.md)).
- Đặt lại 1.0 thì những con đã ghi được trả về 1.0 (chỉ khi gặp lại trong lúc duyệt).

## Giới hạn

- Seamless Co-op: stub getter `animation_speed` chỉ cho qua người chơi và Torrent, kẻ địch vẫn đọc 1.0.
- Nomadic Merchant (team 27) có thể bị tính là kẻ địch (`threatLv > 0`).
- Boss đổi phase, cutscene hay đòn có hiệu ứng chạy theo đồng hồ riêng có thể lệch nhịp (chưa gặp ở 3 boss đã test).

## Công cụ

`EnemyProbe = true` trong `[Logging]`: khoá mục tiêu thì ghi dòng `LOCKON` (`set`, `npc_param`, `chr_type`, `team`, `hp`, anim). Mục tiêu được ghép bằng vị trí gần `lock_on_target_position` nhất (sai số lớn, 8-25 m): chỉ dùng để đoán, hãy đối chiếu với `npc_param`.
