# Nguồn gốc: dịch ngược AutoRecovery.dll và bản C++ (tham khảo)

**Status 2026-10-08: lịch sử, code C++ đã bỏ (viết lại bằng Rust từ 2026-08-18, đọc/ghi qua `fromsoftware-rs`). Giữ lại vì AOB/offset tham khảo được. Offset tay bên dưới KHÔNG còn được dùng.**

Mod viết lại từ việc dịch ngược `AutoRecovery.dll` (project gốc "AshesEverywhere" theo PDB còn sót). Kiến trúc Rust lấy mẫu từ `sometweaks` (LifeBetween, cùng tác giả) rồi tách lại thành mod độc lập; `HpOnDamage` thêm lúc port, sau gộp vào `Regen.PerHit` (Mode=2, % sát thương gây ra).

## Phát hiện từ `AutoRecovery.dll`

- AOB `48 8B 05 ?? ?? ?? ?? 48 85 C0 74 0F 48 39 88` (`mov rax,[rip+disp32]; test rax,rax; jz +0xF; cmp [rax+0x88],ecx`) trỏ tới **`WorldChrMan`**.
- Chuỗi con trỏ kiểu CE: `0x0, 0x10EF8, 0x0, 0x190, 0x0, 0x0`.
- Offset struct: HP `+0x138` / max `+0x13C`; FP `+0x148` / max `+0x150` (bản đầu ghi nhầm `0x14C`); Stamina `+0x154` / max `+0x15C` (đối chiếu với 1 DLL cheat bên thứ ba, đọc offset trong lệnh `LEA`/`ADD` thay vì C decompile vì Ghidra có thể nhân scale sai).
- 2026-08-04 từng dò lại bằng Cheat Engine: base `eldenring.exe+0x3B12E30`, chain `0x0, 0xAE8` (offset tĩnh, dễ lệch hơn AOB).
- Config gốc `[recovery] hp/fp/interval` (0.0-1.0, ms); `interval=0` làm vòng lặp `Sleep(0)` tốn CPU.
- Mod gốc không clamp theo max (bản viết lại có clamp).

## Hook `OnAttack` (bản C++, 2026-08-07)

Hook lấy từ CE table công khai Hexinton v6.1 (cheat "NoHitbox+ReflectAttack"): AOB `45 0F B6 CE 4C 8B C3 48 8B D6 48 8B CF E8`. Patch 12 byte đầu thành `mov rax,<trampoline>; jmp rax`, giữ nguyên `call` gốc, trampoline MASM gọi `OnAttackObserved`. Xác định vai trò bằng 2 tình huống đối xứng: địch đánh player → `[ctx+8]==player`, `rsi`=địch; player đánh → `rsi==player`, `[ctx+8]`=địch ⇒ `rsi` = attacker, `[ctx+8]` = target. Player = `WorldChrMan+0x1E508`. AOB `WorldChrManFinder` của CE table lệch version nên không dùng.

Hook này đã được thay hoàn toàn bằng `hit_hook.rs` (xem [per_hit.md](per_hit.md)), cùng các sự cố của nó (Seamless Co-op dùng đúng AOB này, crash Volcano Manor, mất animation đâm lén).

## Quyết định ngôn ngữ

Từng kết luận `AttackHook` không đáng chuyển sang Rust (fromsoftware-rs không phản chiếu call site, vẫn phải tự viết shellcode); đảo ngược vì `std::arch::global_asm!` nhúng trampoline ngay trong file Rust và `#[unsafe(no_mangle)] static AtomicUsize` thay biến toàn cục C++, không cần file `.asm` hay build-customization riêng.
