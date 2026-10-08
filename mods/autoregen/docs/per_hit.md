# Regen Per Hit: hook, phân loại đòn, ExcludeAow

**Status 2026-10-08: hoạt động (hit_hook.rs, viết lại 2026-09-12), đã test trong game. Còn mở: `ExcludeAow=true` vẫn hồi FP với Unsheathe (xem cuối). Mã: `src/hit_hook.rs`, `src/regen.rs`.**

## Hook hiện tại (`src/hit_hook.rs`)

Hook thẳng vào **entry point thật** của `FUN_140448910(ctx, attacker, hit_info, _, flag)` (hàm hit-resolution, prologue chuẩn 15 byte đúng ranh giới lệnh; AOB `HIT_APPLY_PATTERN`, mới verify trên bản 2.7.1.0/1.17.1). Dời nguyên 15 byte prologue ra buffer riêng, nối `mov rax, <return>; jmp rax` (12 byte; **buffer phải đủ chỗ cho cả đoạn jmp**, thiếu thì CPU chạy lố sang `.data` và crash `ACCESS_VIOLATION` ở hit đầu tiên). Hook chỉ "nghe" `rcx/rdx/r8/r9` (lưu ra stack, gọi callback Rust, khôi phục) rồi để hàm gốc chạy tiếp.

Dữ liệu đọc: `hit_info+0x228` = damage (xác nhận bằng decompile), `hit_info+0x1D8` = source object → vfunc `vtable+0x10` cho `sourceType` (`1` = trực tiếp/cận chiến, `3` = đạn/projectile). `rsi`/attacker so với `main_player`. Offset `hp`/`max_hp`/... lấy bằng `offset_of!` trên `CSChrDataModule` của fromsoftware-rs.

Cách tìm hàm (Ghidra headless trên bản copy `eldenring.exe` ở đường dẫn không dấu cách; cờ `-analyze` không hợp lệ trong Ghidra 12.1.2): quét >1000 chỗ ghi vào offset `hp`, đối chiếu BFS 3 tầng từ hàm hit-resolution → ra `SetHp` → `ApplyHpDelta(module, delta)` → `FUN_140448910`.

## Mode, DamageType

- `Regen.PerHit.Mode`: 0 điểm cố định, 1 % max stat, 2 % sát thương gây ra (lifesteal). Ban đầu `HpOnDamage` bị hiểu sai thành "hồi cố định khi bị đánh"; ý đồ thật là % damage gây ra.
- `DamageType`: 0 chỉ cận chiến (`sourceType==1`), 1 chỉ tầm xa/phép, 2 cả hai - chung cho cả 3 stat (user muốn "cast rẻ hồi FP để cast đắt"). Hạn chế: phép có động tác cận chiến (Carian Greatsword) đọc ra `sourceType==1`.
- 3 cách phân loại vũ khí/phép đã bỏ: (1) ngưỡng `atkParamId` 100.000 (sai Glintstone Nail, dash/jump/backstab); (2) đọc `atkPhys`/`atkMag` của AtkParam (nhiều vũ khí lấy damage qua SpEffect, vũ khí yểm tố dùng chung dòng AtkParam); (3) Stamina vừa giảm (sai với Bestial Incantations).

## ExcludeAow

Không có ngưỡng `atkId` hay field `AtkParam` nào phân biệt được Ash of War (so ~200 field giữa nhóm `[AOW]` và `Default` trong `AtkParam_Pc.csv`, field tốt nhất vẫn overlap 44.6%; Beast Claw heavy thường nằm chung dải ID với nhiều AoW). Hướng dùng: input pad qua `CSChrActionRequestModule` - Weapon Art luôn bằng `L2`. Chỉ đọc `l2` tại lúc trúng thì sót (nút đã nhả từ lâu), nên **chốt (latch) theo lần bấm mới nhất** mỗi frame qua `new_action_presses`: `R1`/`R2`/`L1` → "đòn thường"; `L2` → "Weapon Art"; giữ tới lần bấm kế (`LAST_ATTACK_WAS_SKILL`, `update_last_attack_input()` trong `regen.rs`). `L1` gộp vào đòn thường vì không bao giờ kích hoạt Weapon Art.

**Còn mở (Nexus, Brokensword7, 2026-10-04):** Unsheathe = bấm `L2` vào tư thế rồi `R1`/`R2` để chém; latch thấy `R1`/`R2` → chốt "đòn thường" nên nhát chém bị tính là đòn thường và vẫn hồi FP. Ash 2 bước tương tự có thể bị cùng lỗi (Sacred Blade, Lion's Claw, Bloodhound's Step... cần kiểm tra). Cần log xác nhận (`atkId` + `l2`/`r1` lúc trúng); hướng sửa: giữ latch tới khi anim/action của Ash kết thúc thay vì đổi ngay khi `R1`/`R2` bấm. Chưa có ini đầy đủ của người dùng.

## Lỗi đã gặp (bài học)

- **Mất animation đâm lén/chí mạng khi bật hook (2026-09-03):** hàm hit nhận **5 tham số**, tham số thứ 5 (1 byte, tính từ `hitInfo+0xd9==2`) truyền qua stack `[rsp+0x20]` và quyết định có chạy khối xác định chí mạng/đâm lén hay không. Trampoline cũ chỉ forward 4 tham số rồi `sub rsp,0x20` tạo shadow space mới nên `[rsp+0x20]` là rác. Tắt `Enabled` không hết lỗi vì patch vẫn cài. Sửa: đọc `byte ptr [rsp+0x20]` vào `r10b` làm lệnh đầu tiên trong trampoline rồi ghi lại sau khi tạo shadow space (cũng áp dụng ở sometweaks). Chẩn đoán nhờ log 2 dòng "dealt damage" liên tiếp (0 rồi damage thật).
- **Crash vào Volcano Manor (v2.4.0, 2026-09-12):** lỗi thật của mod, không phải ModEngine2 (crash dump `cdb`: `STATUS_INVALID_HANDLE` trong critical section, không có frame AutoRegen → kết luận sai ban đầu). Build tạm bỏ đoạn forward tham số thứ 5 hết crash → nguyên nhân là trampoline tay hijack giữa call-site (nghi thiếu unwind metadata `.pdata`/`.xdata` cho vùng JMP-hijack, exception nội bộ của engine đi qua tính sai stack frame). Giải pháp: viết lại thành `hit_hook.rs` hook ở entry point hàm (hết crash, cùng máy/save).
- **Xung đột Seamless Co-op (bản C++ cũ, 2026-08-07):** Seamless dùng đúng AOB `OnAttack` (`45 0F B6 CE 4C 8B C3 48 8B D6 48 8B CF E8`) và tự huỷ game khi không khớp ("No such pattern"). Sửa bằng cài hook lần đầu khi đã vào game (sau khi mọi mod khác quét xong). Hook mới ở entry point khác nên không dùng lại AOB đó.
- Trampoline cũ crash do lệch alignment RSP (align rồi `push` làm lệch 8 byte, `movaps` fault); sửa bằng lưu RSP gốc vào `r12`. Hàm chứa `__try` không được gọi `Logger::Log("literal")` (MSVC C2712) - chuyện của bản C++.
- Log `HitHook: player dealt N damage` ghi mỗi đòn trúng kể cả khi `PerHit` tắt → comment lại (cùng `read_atk_id()`/`HITINFO_ATK_PARAM_ID_OFFSET`) thay vì xoá, bỏ comment 3 chỗ để debug lại.
