# NoRestResummon và SummonAnywhere (BuddyStoneParam)

**Status 2026-10-08: hoạt động (vanilla, Reforged, Seamless), đã test. Mã: `src/buddy_stone.rs`. `SummonAnywhere` mặc định `true` từ 1.1.0.**

## NoRestResummon

Chỉ sửa param (cách của Solid Uncapper, quét chuỗi trong DLL của họ): `BuddyStoneParam.summonedEventFlagId` (`+0xC`) = 0. Event flag này bật khi triệu hồi ở bia đá và chỉ tắt khi nghỉ → khoá "mỗi lần nghỉ 1 lần". Giá trị gốc chụp 1 lần, áp từ bản chụp (F5 bật/tắt 2 chiều); log `304 summoning pool row(s), 302 with a once-per-rest flag`.

Giới hạn: không gỡ được luật của Reforged (sau Spirit-Severing Blade ô Ash xám tới khi ngồi grace) - xem [open_issues.md](open_issues.md).

**Tương tác với SummonAnywhere (2026-10-09, đã test in-game: đúng):** khoá của vanilla (Ash xám sau khi thu hồi/chết tới khi ngồi grace) chỉ có khi `SummonAnywhere` tắt; bật thì stub stateInfo 373 ép `al = 1` nên đè luôn khoá, `NoRestResummon=false` mất tác dụng (người dùng Nexus báo, bạn xác nhận bằng cách tắt SummonAnywhere). Sửa: khi `NoRestResummon=false` + Anywhere, mỗi frame tick đọc event flag `summonedEventFlagId` gốc của bia hiện tại (`CSEventFlagMan`); cờ bật và không còn spirit thì `ANYWHERE = 0` để game tự trả lời (xám). Khoá theo từng bia, nên sang bia khác (ô map khác) thì mở lại.

## SummonAnywhere

Sửa param như Solid Uncapper (`activateRange +0x1C = 65535`, `overwriteReturnRange +0x1E = -1`, `overwriteActivateRegionEntityId +0x20 = 0`, `eliminateTargetEntityId +0x8 = 0`) **chưa đủ**: bản 1.1.0 báo lỗi ở Shadowlands và ngoài pool. Còn 3 chỗ cần đối tượng bia đá đang được nạp gần người chơi:

1. `GetBuddyState` (`sub_1404B72F0`) chỉ cho dùng Ash khi người chơi mang SpEffect **stateInfo 373** (`sub_1404FA370(player+0x178, 0x175)`, `0x1404B7368`); `sub_1404B7610` (HUD, tail-call cùng check) cũng vậy. Hiệu ứng do vùng pool gắn lên người chơi.
2. Bia hiện tại `+0x38` chỉ do **script ESD của chính bia đá** đặt (lệnh ESD 122 → `sub_1404B8050`), và bị **xoá mỗi frame** bởi `sub_1404B6E80` (từ `sub_140EAFDE0`). Không bia nào nạp → `+0x38 = 0` → `GetBuddyState` = -1 (xám).
3. `sub_1404BD870` (trong `Update`, `0x1404B8B2E`) tính lại `+0xB5` (`is_within_activation_range`) / `+0xB7` (`is_within_warn_range`) mỗi frame bằng cách tìm đối tượng bia `+0x3C` (`sub_1405EEFD0`); bia bị gỡ → cờ false → nhánh "rời khu" của `sub_1404B92B0` cho về hết.

Các patch (chỉ khi `SummonAnywhere` bật, cờ `ANYWHERE` mà stub đọc; từ 1.1.4 đều đổi đích `call`, xem [seamless_conflict.md](seamless_conflict.md)):

- stateInfo 373: bản `call` (AOB `BA 75 01 00 00 48 8B 88 78 01 00 00 E8 ...`, `0x1404B735C`) và bản tail `jmp` (`0x1404B7655`) → `al = 1`. Stub trong `GetBuddyState` (rbx = manager) ghi `FALLBACK_STONE` vào `+0x38` khi = 0, ngay trước khi nó đọc.
- In-range: stub cho `call sub_1404BD870` (AOB `41 0F 28 CA 49 8B CF E8 ...`, `0x1404B8B1B`).
- `FALLBACK_STONE` = bia thật gần nhất vừa thấy, hoặc dòng BuddyStoneParam đầu tiên không có `dopingSpEffectId` (vd. `10000100`); khi có spirit thì ưu tiên bia đang hoạt động `+0x3C` để `+0x38 == +0x3C`.
- Nếu 3 patch không cài được, lùi về chỉ sửa param (log lỗi).

### Sửa lỗi Reforged (2026-10-01)

Stub in-range ép `+0xB5 = 1, +0xB7 = 0` **mỗi frame kể cả khi chưa có spirit** nên `sub_1404B8EB0` (chạy trước, gắn SpEffect "vào vùng triệu hồi": `GameSystemCommonParam.onBuddySummon_inActivateRange_spEffectId_pc` = 9540 lên người chơi, `_buddy` = 9541 lên spirit khi `+0xB5` đổi 0 → 1) không thấy cạnh lên → không SpEffect nào được gắn; Reforged dựa vào chúng cho Spirit Ring và cho phép dùng Ash/Blade. Sửa: chỉ đặt `+0xB5 = 1` khi có spirit (`+0xB4 player_has_alive_summon`) **và** game không tính ra vùng nào (`+0xB5 == 0 && +0xB7 == 0`); còn lại để nguyên giá trị game. Phát hiện bằng cách tách `SummonAnywhere` thành 3 key test (Params/State/Range) và bật từng phần; Range là thủ phạm.

## Quan sát

- Test ở Limgrave: `stones: current=` đổi theo chỗ đứng (`1045380100`, `1046380100`...; 2 cặp số giữa = ô map `m60_XX_YY`), đôi khi về `0` (không có bia).
- Mỗi bia có `dopingSpEffectId` riêng nên buff của spirit có thể khác theo bia được chọn.
- Test cuối: gọi được ở Lands Between và Shadowlands ngoài pool, gọi ở pool rồi cưỡi Torrent ra xa spirit vẫn ở lại, cho về rồi gọi lại vẫn được.
- Param 134 (BuddyStoneParam) tra bằng entity id (`sub_140D28320`) → row id = entity id của bia; `+0x3C` về 0 trong `sub_1404B8EB0` khi hết spirit.
- Cách ghi `+0x38` từ tick FrameBegin vô ích (bị xoá mỗi frame) - đã bỏ; cách patch inline lệnh xoá `mov [rsi+38h],0` cũng bỏ ở 1.1.4 (xung đột Seamless).
