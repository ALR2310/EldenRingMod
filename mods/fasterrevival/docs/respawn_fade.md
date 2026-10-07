# FasterRevival: bỏ khoảng dừng sau "YOU DIED"

**Status 2026-09-28: HOÀN THÀNH, test ổn trong game.** Code:
`mods/fasterrevival/src/fade.rs`. Kéo mốc "chết" lên sớm đã nằm ở
`death_flow.md`; phần này cắt khoảng dừng ở màn "YOU DIED". Hai phần cộng lại
đưa hồi sinh sau chết thường từ 12.2-14.4s xuống ~4.2-4.7s.

## Nguồn và cách hoạt động

Nguồn: **FasterRespawn** của ImAxel0
([Nexus 501](https://www.nexusmods.com/eldenring/mods/501),
[GitHub](https://github.com/ImAxel0/EldenRing-FasterRespawn-Mod), MIT). DLL C++
15 dòng: AOB `74 ?? F3 0F 10 19`, đổi `74 06` (`jz`) thành `EB 06` (`jmp`);
decompile bản DLL và source công khai khớp hoàn toàn. Tác giả không ghi cú nhảy
đó điều khiển gì; tra trong IDA (2.7.1.0, mẫu duy nhất ở `0x1405A7F23`; 2.6.2.0
ở `0x1405A70D3`):

- Nằm trong `sub_1405A7E00`, một bước của `CSDeathRestartEvent::Start`
  (`sub_14059F670`, có chuỗi tên hàm đó). Bước này hẹn giờ callback
  `sub_1405A7FA0` qua `sub_1405942D0(a2, 4006, cb, delay, 0, 4, 1)`.
- `delay` (xmm3) = `*sub_140D2FE90()`, con trỏ tới param slot 143
  (`MenuCommonParam`), row ID 0, trường đầu tiên =
  **`soloPlayDeath_ToFadeOutTime`** (tên paramdef/Smithbox, hiển thị "[YOU
  DIED] Fade Out Duration", vanilla 3.8; accessor fromsoftware-rs
  `solo_play_death_to_fade_out_time`). Patch ép luôn nhánh `xorps xmm3, xmm3`,
  tức delay = 0.

Vì delay chính là một giá trị param nên không cần patch code: `fade.rs` ghi
thẳng `MenuCommonParam[0].soloPlayDeath_ToFadeOutTime = 0`, **đúng một lần**
(log kèm giá trị cũ), khi `main_player` đã có (gate `SoloParamRepository`) và
`common::params::check` xác nhận slot đúng param. Mod luôn bật nên không cần
khôi phục giá trị gốc. Không đụng `partyGhostDeath_ToFadeOutTime` (co-op,
vanilla 3.3), giống bản gốc.

## Chưa xác định

Bước kế (stage 5, trong `sub_1405A7FA0`) còn một delay khác đọc từ
`GameSystemCommonParam[0]+0x37C`. FasterRespawn không đụng, chưa xác định là
trường nào (giá trị đo được là 0).

## Hồi sinh chậm 8-20s ở một số lần: không phải lỗi

Sau khi thêm phần này, các lần chết thường hồi sinh ~4.4-4.7s (trước đó ~8.4s,
khớp 3.8s vanilla của `soloPlayDeath_ToFadeOutTime`), nhưng một số lần 7.8s /
14s / 20s. Đo từ log: animation đứng yên (fade) ở ~1.7s và khoảng load (không
có frame) ~2.3-2.5s **như nhau** ở mọi lần; phần chênh là thời gian game vẫn
chạy frame sau fade.

Giả thuyết đã thử và loại: stage 5 của `CSDeathRestartEvent` chỉ chờ khi
`is_death_penalty_skip`, với delay từ `GameSystemCommonParam[0]+0x37C`. Log tạm
cho `penalty_skip=false` và `+0x37C = 0`, nên sai.

Nguyên nhân thật (người dùng tự nhận ra): chết gần **Stake of Marika** thì game
hiện menu chọn hồi sinh ở Stake hay ở grace cuối, và thời gian đứng ở menu được
tính vào. Không liên quan tới mod.
