# GhostColor: bật/tắt màu ma của spirit

**Status 2026-10-08: hoạt động trên vanilla và Reforged 2.3.5.3 (đã test), hot reload 2 chiều, áp ngay cho spirit đang ở ngoài. Mã: `src/ghost_color.rs`. Còn mở: Seamless mất màu ma, Reforged mặc định đỏ - xem [open_issues.md](open_issues.md).**

## Chuỗi tham số của màu

`BuddyParam.npcParamId` → `NpcParam` → SpEffect (32 ô `spEffectID`) → `SpEffectParam.vfxId..vfxId7` → `SpEffectVfxParam.phantomParamOverwriteType/Id` → `PhantomParam` (màu RGBA thật).

- Vanilla: NpcParam ô 26 (`spEffectID26`, offset `0x210`) = `295000` "[Spirit Summon] Color" (`stateInfo 432`, vfx `57000`) / `295200` Puppet (vfx `57100`). SpEffectVfx `57000`: `phantomParamOverwriteType 2`, `Id 200` → PhantomParam 200 = màu ma. Puppet còn có `295201`-`295204` (ô 24) chỉ gắn SFX trang trí, không ghi đè phantom.
- Reforged: không có `295000`; màu đến từ `spEffectID4` = SpEffect riêng của từng Ash (`200100`..`292100`) → vfx `70000` (109 spirit) / `70010` (7) / `70020` (5) → PhantomParam `2000`/`2010`/`420`. Các SpEffect này còn mang chỉnh chỉ số + chuỗi `cycleOccurrenceSpEffectId` → gỡ cả SpEffect sẽ mất cân bằng Reforged. 4 spirit không có đường màu nào.

## Cách làm hiện tại

Spirit = NpcParam mà BuddyParam trỏ tới → mọi SpEffect của nó → mọi SpEffectVfx có `phantomParamOverwriteType != 0`. `GhostColor=false` đặt `phantomParamOverwriteType = 0` trên các dòng vfx đó (giữ nguyên SpEffect), lưu giá trị gốc 1 lần, F5 bật lại được. Tra ID bằng `common::params::row_ids`, ghi theo index (không dùng `get_mut`/bảng lookup, có thể ra nhầm hàng trên regulation mod như Convergence).

Quy tắc tỉ lệ: vfx chỉ bị tắt khi **số dòng NpcParam spirit dùng nó > số dòng không phải spirit**. Lý do: `54183` (1 spirit / 16 khác) = SpEffect `14495` → PhantomParam `240` (`isVisibleDeadChr`) của Mausoleum Knight, tắt sẽ mất dáng kẻ địch; còn quy tắc "không NPC thường nào dùng" quá chặt (loại luôn `57000`).

Log vanilla: `106 spirit NpcParam row(s) from 107 BuddyParam NPC id(s); tint vfx 57000 (99/40), 57100 (7/4); left alone 54183 (1/16)`. Log Reforged: `125 spirit NpcParam row(s) from 126 BuddyParam NPC id(s); tint vfx [70000 (109/0), 70010 (7/0), 70020 (5/0)]`.

## Hướng đã bỏ

- Gỡ SpEffect 295000-295999 khỏi spirit mỗi frame (bản SomeTweaks): tốn, chỉ áp dụng cho spirit gọi sau.
- Sửa `NpcParam.spEffectID26 = -1`: chỉ đúng với vanilla, không tác dụng với Reforged.
- Đoán sai trên đường đi: `298091` chỉ là SpEffect số, không phải màu; `stateInfo 432` không phải dấu hiệu màu; màu không đến từ `GameSystemCommonParam`.
- Tên dòng trong Smithbox lấy theo game gốc, modpack có thể sai → luôn đi từ BuddyParam.

## Tra cứu liên quan: cờ hành vi spirit

Mọi spirit có `297000` "Follow & Warp to Player" (ô 28). Kiểu đi theo ở ô 29: `297100` WALK_FOLLOW (Putrid Corpse), `297101` REAR_FOLLOW (23 spirit đánh xa/phép/hỗ trợ), `297102` FLY_FOLLOW (Winged Misbegotten, Jellyfish, Warhawk, Stormhawk), `297103` NO_FOLLOW (Latenna); không cờ = đi theo mặc định. `297200` INTERCEPT_LONGRANGE (ô 24) là cờ cách đánh. Chưa đổi thử trong game.
