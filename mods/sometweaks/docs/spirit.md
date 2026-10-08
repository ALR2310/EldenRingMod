# [Spirit]: giải mã er10x.dll, Summon.Anywhere và các ngõ cụt

**Status 2026-10-08: phần này đã được viết lại tốt hơn ở `mods/spiritmultiplier` (chain, GhostColor, Regen, MultiSpirit, SummonAnywhere) - dùng đó, không dùng bản trong sometweaks. Tài liệu này giữ kiến thức đã tìm ra về `er10x.dll` và các cách đã thử. Mã: `src/spirit/{color,regen,summon_count,summon_anywhere}.rs`.**

## Giải mã `er10x.dll` ("10x Spirit Summons")

Build MinGW nên còn tên hàm (`hk_summon_gate`, `param_find_rows`, `band_occupancy`, `buddy_mgr`, `strip_ghost_colour`, `sig_hit`). Tự tắt khi thấy EAC, kiểm tra version, quét nhiều signature trong 1 lượt `.text`, VEH, watchdog. Struct `Sig`: pattern ở offset +8, độ dài ở +0x28. Các sig: `kSigGate` (22 byte, "current slot count < max", unique RVA `0x4b6e0d`, entry hàm = match - 0x7D = `0x4b6d90`), `kSigGateEntry` (21 byte, prologue), `kSigDespawn`, `kSigStoneReq`, `kSigPoolStore` (4 phần tử cùng mảng patch khi `summon_anywhere=1`), `kSigCooldown` (cho `unlimited_resummon`). Bản `er10x.dll` cập nhật 17/8 có byte pattern `kSigGate`/`kSigGateEntry` y hệt bản cũ.

"Chain rebuild": đọc `WorldChrMan+0x1e538` (summon buddy manager), dò cây nhị phân MSVC `std::map` (byte `+0x19` = is-nil) tìm entry theo SpEffect kích hoạt, đi linked-list (`data` +0, `next` +8) gom BuddyParam id, cấp phát chain mới dài hơn (không free) rồi ghi đè con trỏ node+0x28. `fromsoftware-rs` có sẵn đúng cấu trúc: `SummonBuddyManager.trigger_speffect_to_buddy_map: ChainingMap<i32, i32>`, `iter_chains_mut()` → nối node mới vào `head.next` không cần offset tay.

## Các tính năng và bài học

- **`Spirit.Summon.Amount`/`Multiplier`** (`summon_count.rs`): cap 10. Lỗi nhân dồn (1→2→4→8→10) vì đọc "gốc" từ chain đã rebuild; sửa bằng snapshot `ORIGINAL_CHAIN_IDS` (cùng pattern `ORIGINAL_BASE_POINTS` của drop rate). **Crash không log** (`SomeTweaks.dll+0xE30D`, `mov rax,[rsi+0x1e538]; mov r8,[rax+8]`) vì thiếu gate "đã vào world": `WorldChrMan::instance_mut()` Ok ngay khi object tồn tại, storage chain còn null; không phải panic nên `catch_unwind` không bắt. Cách tra: Windows Event Log (`Get-WinEvent ... Id=1000,1001`) lấy offset, decompile DLL release bằng Ghidra + `.pdb`.
- **`Spirit.Color`**: gỡ SpEffect 295000-295999 khỏi spirit mỗi frame. Bản tốt hơn (sửa vfx qua param) ở spiritmultiplier.
- **`Spirit.Regen`**: % HP tối đa mỗi giây cho spirit/Torrent trong `summon_buddy_chr_set`.
- **`Spirit.Enabled`**: công tắc tổng; `summon_anywhere` chỉ check lúc khởi động (patch code 1 lần).

## Spirit.Summon.Anywhere: lịch sử (kết cục: nhiều ngõ cụt, cuối cùng chỉ còn gate + `kSigDespawn`)

1. Chỉ bỏ cổng bắt đầu triệu hồi (`kSigGate`, patch 15 byte đầu hàm: `cmp dword [this+0x20],0` "không có bia đá gần đó" → trả thành công): spirit gọi được nhưng sau vài giây tự biến mất, không hiện tên/máu ở góc trái.
2. Port Hook A/B từ `.docs/SummonAnywhere` (`soarqin/ER-EzMod`): A ép "bán kính" `[rax+0x84]` = 1000 cho SpEffect `0x7D0`; B xoá 2 field area-eligibility ở `[rbp-0x68]`. Sig verified unique trên 2.7.0.0 (RVA `0xea1507`, `0xea5860`). **Hook A hiểu sai bản chất:** `+0x84` là `SummonBuddyWarpManager.trigger_dist_to_player` (ngưỡng warp spirit về gần người chơi), không phải bán kính thu hồi. Ép 1000 mở nhánh warp đọc vị trí từ bia đá ảo chưa khởi tạo → spirit "đi về hướng Đông" (toạ độ gốc thế giới). Đã xoá Hook A.
3. Thử ép `is_within_activation_range`/`is_within_warn_range` = true và zero `active_summmon_buddy_stone_entity_id` mỗi tick, và NOP `jne` (`40 84 FF 75 ?? 84 C0 74 ?? 33 C0`, bỏ check "band busy") + zero `item_use_cooldown_timer` (`Spirit.Summon.Unlimited`, sau gộp vào Anywhere): **phá tracking spirit hợp lệ**, hết biến mất khi gỡ; gate hook tự nó đã cho resummon miễn phí. Xoá.
4. **Kết cục:** gate (patch thẳng vào khối 22 byte `kSigGate` thay vì entry hàm, vì Seamless Co-op `ersc.dll` hook đúng entry: `cmp dword[rbx+0x20],0; jl bypass; <replay 22 byte>; jmp resume; bypass: mov al,1`) + `kSigDespawn` (27 byte, `jge` `7D` → `jmp` `EB`, tắt phần thu hồi do đo khoảng cách; hàm cleanup còn gọi từ 2 nơi khác nên thu hồi hợp lệ vẫn chạy) + Hook B. Đã xác nhận hết bug "đi hướng Đông" (2026-09-04).
5. Còn lại: xung đột runtime với `ersc.dll` (thu hồi xong không gọi lại được) chưa điều tra được, cần debugger sống. Cách giải sau này ở spiritmultiplier: chỉ đổi đích `call` ([../../spiritmultiplier/docs/seamless_conflict.md](../../spiritmultiplier/docs/seamless_conflict.md)) và [summon_anywhere.md](../../spiritmultiplier/docs/summon_anywhere.md).

## Bài học quan trọng: cô lập khi test

Nhiều kết luận "đã hoạt động" (Hook A/B, `Unlimited`) bị nhiễu vì `er10x.dll` đang bật song song trong ModEngine2 → thực ra test hành vi của er10x. Luôn kiểm tra không có DLL tham khảo nào (`er10x.dll`, `SummonAnywhere.dll`, ...) bật trong `modengine2/config.toml` trước khi kết luận. Một số DLL tham khảo đặt tên sai chức năng: `zibinha_infinite_summoning.dll` thực chất là "Friendly Invocation Range" (SpEffect 2000 → range 1000, chính là Hook A); `SummonAnywhere.dll` (68KB, PDB `AshesEverywhere.pdb`) chứa cùng AOB Hook A/B.
