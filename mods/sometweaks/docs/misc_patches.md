# [Misc] và [Rune Reward]: các patch lặt vặt (TorrentAnywhere, WarpAnywhere, UnlockAshesOfWar, KeepOnDeath...)

**Status 2026-10-08: mọi tính năng dưới đây đã test và hoạt động (2026-08/09) nhưng KHÔNG có ở mod nào khác - nếu cần thì port từ đây. Mã: `src/misc/*.rs`, `src/rune/keep_on_death.rs`.**

## TorrentAnywhere (`torrent_anywhere.rs`, port từ `CavaloLivre_V7_Cardoso.dll`)

3 kỹ thuật, không cần EMEVD:
1. **2 patch điều kiện khu vực** (`area_list_check`, `direct_ride_check`): cùng đọc `ptr = [this+0x68]`, check `[ptr+0x36] != 0` ("khu vực cấm cưỡi ngựa"). Không có field trong fromsoftware-rs khớp nên patch code thô: `cmp byte[ptr+0x36],0; setne al` (6 byte) → `mov byte[ptr+0x36],0; xor al,al` (cùng 6 byte, ghi đè tại chỗ).
2. **Bỏ ép xuống ngựa ở Abyssal Woods**: 1 byte `74` → `EB` (`jz` → `jmp`) tại check SpEffect `19995` ("Forced Torrent Dismount Abyssal Woods").
3. **Áp lại SpEffect `19996`** ("Remove Forced Torrent Dismount") mỗi giây qua `ChrInsExt::apply_speffect` trên tick `FrameBegin`.

## WarpAnywhere (`warp_anywhere.rs`, port từ `Zibinha_FastTravel.dll`)

`Zibinha_MapInCombat.dll` là tập con byte-for-byte (2 patch đầu). Đọc log tiếng Bồ Đào Nha (`MAP_CHECK:`, `WARP_BLOCK:`, `FIELD_AREA:`) + Ghidra decompile:
1. `map_check`: 5 byte đầu của 1 `call` (map bị khoá khi combat) → `xor rax,rax; nop; nop`.
2. `warp_block`: `je` `0x74` → `jmp` `0xEB`.
3. `field_area_unlock`: resolve global slot singleton `FieldArea` (`match + 7 + disp32` của `mov rcx,[rip+disp32]`; không dùng công thức từ pseudo-C của Ghidra vì sai), mỗi tick zero offset `+0xA0` của instance (patch gốc dùng vòng `Sleep(100)`).

## UnlockAshesOfWar / UnlockEnchantments (port từ Nexus mod 271 qua diff CSV)

Diff `EquipParamWeapon.csv`/`EquipParamGem.csv` của mod gốc: toàn bộ 3554/3554 dòng weapon đổi `gemMountType=2`, `isEnhance=1`; `disableGemAttr` KHÔNG đụng (357 dòng vẫn =1; suy đoán ban đầu về `disableGemAttr` sai); `EquipParamGem` toàn bộ 44 cột `canMountWep_*` = 1. Tách 2 tính năng: `UnlockAshesOfWar` (`gemMountType` + `canMountWep_*`) và `UnlockEnchantments` (`isEnhance=1`: yểm bùa chú như Bloodflame Blade/Order's Blade/Scholar's Armament, game báo "This weapon cannot be enchanted", KHÔNG phải affinity). Áp 1 lần lúc khởi động (~3800 dòng, không hot-reload; restart với `false` để về vanilla). Chưa xác nhận game có moveset thật cho AoW trên cung/khiên/đuốc.

## Rune.KeepOnDeath (`keep_on_death.rs`, port từ `DisableRuneLoss.dll`)

AOB `b0 01 ? 8b ? e8 ? ? ? ? ? 8b ? ? ? 32 c0 ? 83 ? 28 c3` (RVA `0x594f6c`), verify byte offset+5 = `E8` rồi NOP 5 byte (lệnh `CALL` chuyển rune vào vết máu khi chết). Áp 1 lần lúc khởi động. **Bản dùng `ChrIns.chr_flags1c6.has_dropped_runes` KHÔNG hoạt động** (cờ đó dành cho NPC chết thưởng rune cho người giết, 2 hệ thống khác nhau). Chạy song song `DisableRuneLoss.dll` gốc gây crash (race `VirtualProtect` trên cùng trang). Hàm `overwrite_bytes` đã gộp vào `common::codepatch`.

## Các module port sang mod riêng (xem README)

- `Rune.Multiplier` (hook `AddSoul_Call`, stub nhân fixed-point Q20, chỉ khi amount > 0), `WeightMultiplier` (patch 7 byte `movaps xmm0,xmm6`, `common::codepatch::install_jmp_hook`): mod riêng `runemultiplier`, `weightmultiplier`. Lỗi đáng nhớ: quét AOB 1 lần với delay 5s cố định dễ hỏng trên máy chậm → dùng retry loop (`wait_for_pattern_in_module`).
- `Drop Rate`: `dropmultiplier` (`docs/materials.md`, `docs/convergence_panic.md`). Cơ chế `ItemLotParam_enemy`: xác suất = `lotItemBasePoint0N / tổng 8 ô`; slot rỗng nhận bằng `lotItemId0N == 0` (không phải `category == -1` như wiki DS1); `cumulate_lot_point` luôn 0 ở Elden Ring, không ghi lại. `Zibinha_DropRate_AOB_V2_SAFE.dll` (AOB `41 0F 28 F8 48 85 D2`, RVA `0x68652c`) thực chất **cộng** điểm vào công thức discovery-bonus, không phải nhân.
- `Rune Reward` (cộng rune theo thời gian + milestone): `passiverunes`. Phải gate bằng `main_player` vì `GameDataMan` resolve từ màn title.
- `Regen`: `autoregen` ([../../autoregen/docs/](../../autoregen/docs/per_hit.md)).

## EnemyScaling (đã xoá 2026-09-06, chưa từng test)

`Enemy.Health.Multiplier` nhân `NpcParam.hp` (snapshot gốc, hot-reload); `Enemy.Damage.SpEffectId` áp 1 SpEffect có sẵn lên mọi kẻ thù mỗi giây (không có field hệ số sát thương trong `NpcParam`); kẻ thù lấy từ `WorldChrMan.open_field_chr_set` lọc `chr_type == Npc`. Lưu ý đã rút ra: không dùng ID SpEffect có sẵn mà không kiểm tra chỗ khác trong game có dùng không.

## Ý tưởng đã thử và bỏ: menu cấu hình trong game

Hook DX12 `Present` bằng `hudhook` + `imgui` (slider chỉnh `Regen.*`): chạy được nhưng cursor bị game ẩn mỗi frame (`io.mouse_draw_cursor = true`), toạ độ chuột lệch khi Windows Display Scale ≠ 100%, bù `GetDpiForWindow` chưa đúng, UI không tự scale. Revert. Xem thêm nhánh `feat/modmenu` (TODO.md, mục "Đã dừng").
