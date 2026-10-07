# WeightMultiplier: điểm hook của phép tính Trọng Tải

**Status 2026-10-07: ĐANG PHÁT HÀNH (2.0.1), điểm hook đã chốt.** Mẫu AOB neo
duy nhất trên cả 2.6.2.0, 2.7.0.0 và 2.7.1.0 (kiểm tra bằng IDA ngày
2026-10-07). Tài liệu cũ không ghi lại kết quả test in-game riêng của bản Rust;
khi sửa hook phải kiểm cả hai mặt (xem "Khi test"). Code:
`mods/weightmultiplier/src/hook.rs`.

## Điều mod làm

Patch **một lệnh CPU** trong `eldenring.exe` (inline hook: nhảy tới một đoạn
code tự cấp phát rồi nhảy về), không đọc/ghi struct nào của game. Lệnh được
patch chạy **đúng một lần** sau khi game cộng xong trọng lượng mọi món đồ, nên
nhân hay ghi đè ở đó cho một con số sạch.

## Lịch sử tìm điểm hook (3 lần thử)

Vòng lặp tính tổng trọng lượng (5 slot), dạng:

```asm
addss xmm6, dword ptr [rax + 0xc]   ; cộng trọng lượng slot này
inc ebx                              ; <-- Zibinha hook ở đây (set cứng mỗi vòng)
cmp ebx, 5
jl <loop start>
lea r11, [rsp+0x70]
movaps xmm0, xmm6                    ; <-- WeightMultiplier hook ở đây (chạy đúng 1 lần)
mov rbx, [r11+0x10]
```

1. **Sai, đã bỏ:** suy từ decompile `Zibinha_Weight_Control.dll`, hook lùi 5 byte
   trước chỗ AOB khớp (đoán là `addss xmm6,[rax+0xc]`). Build và chạy không lỗi
   nhưng **không có tác dụng**: `addss` chỉ chạy có điều kiện, không nằm cố định
   cách AOB 5 byte, nên hook trúng byte giữa một lệnh khác, vô hiệu mà không
   crash.
2. **Đúng UI, sai gameplay:** dò bằng Cheat Engine, pin địa chỉ "Trọng Tải Hiện
   Tại" rồi "Find out what writes to this address", bắt được `movss
   [rsi+1C],xmm0` (rsi = struct equip-load). Hook ở đó làm **UI hiện đúng số đã
   giảm** nhưng **roll type và tốc độ chạy không đổi**, tức chỉ ảnh hưởng đường
   hiển thị, không phải đường gameplay.
3. **Đúng (đang dùng):** người dùng cài `Zibinha_Weight_Control.dll` (set
   `WeightValue=0`) và xác nhận roll type đổi thật; log của DLL đó in địa chỉ nó
   patch, khớp **chính xác** vị trí AOB ở lần 1 (không lùi 5 byte). Vậy AOB đúng,
   chỉ sai ở việc tự lùi.

Zibinha hook thẳng vào `inc ebx` và ghi đè `xmm6` bằng giá trị cố định **mỗi
vòng lặp**; chỉ lần ghi cuối còn lại, nên tổng luôn = số cố định. Tốt cho "set
cứng = 0" nhưng **không dùng được cho %**: nhân `xmm6` tại đó sẽ dồn qua từng
vòng (món ở slot đầu bị giảm nhiều hơn món ở slot cuối). `movaps xmm0,xmm6`
chạy một lần sau khi tổng hoàn chỉnh, nên nhân ở đó là một hệ số duy nhất.

## AOB, patch và stub

- **Mẫu neo:** `FF C3 83 FB ?? 7C ?? 4C 8D 5C 24 ??`
  (`inc ebx; cmp ebx,N; jl ..; lea r11,[rsp+..]`).
- **Điểm patch** = neo + 12 byte (bỏ `inc/cmp/jl` 7 byte và `lea r11,[rsp+0x70]` 5
  byte), trúng `movaps xmm0,xmm6` (3) + `mov rbx,[r11+0x10]` (4) = **7 byte**.
- **Jump:** chỉ 7 byte nên không đủ chỗ cho jump tuyệt đối `mov reg,imm64; jmp
  reg` (~12 byte) như `runemultiplier`/`autoregen`. Dùng JMP tương đối 5 byte
  (`E9 rel32`) và tìm vùng nhớ thực thi gần bằng `VirtualAlloc` (quét lùi/tiến
  theo bước `0x10000`), port từ `CodePatch.cpp` của bản C++ thành
  `common::codepatch::install_jmp_hook`.
- **Stub:**
  ```asm
  mov   rcx, &WEIGHT_VALUE
  mulss xmm6, dword ptr [rcx]        ; Mode 0 (Multiplier); Mode 1 dùng movss
  movaps xmm0, xmm6                  ; lệnh gốc
  mov   rbx, qword ptr [r11+0x10]    ; lệnh gốc
  jmp   <target + 7>
  ```
- **Hai chế độ cùng một điểm hook:** `Mode=0` dùng `mulss xmm6,[rcx]` (nhân), `Mode=1`
  dùng `movss xmm6,[rcx]` (ghi đè tổng bằng `FixedValue`). Cùng một static
  `WEIGHT_VALUE` mang nghĩa khác nhau theo mode (hệ số hay số cố định), vì chỉ
  một mode được dùng tại một thời điểm.
- **Hot reload:** đổi `Multiplier`/`FixedValue` chỉ cần ghi lại `WEIGHT_VALUE`
  (stub đọc từ địa chỉ cố định mỗi lần chạy). Đổi **`Mode`** thì opcode
  `mulss`/`movss` đã nằm cứng trong code đang chạy, nên mod tự vá lại đúng 4 byte
  đó qua `codepatch::overwrite_bytes` (`STUB_MODE_INSTR_ADDR`); rủi ro như lúc
  cài hook ban đầu (ghi đè code CPU có thể đang chạy), chỉ lặp lại theo yêu cầu.
  Người dùng đã chấp nhận đánh đổi này khi chọn hỗ trợ đổi cả `Mode` lúc chạy.
  Nếu vá lại thất bại, mod giữ `Mode` cũ và ghi cảnh báo.

## Mod khác hook đè cùng chỗ

Khi không tìm thấy neo, mod quét thêm `FOREIGN_PATCH_PATTERN`
(`E9 ?? ?? ?? ?? 7C ?? 4C 8D 5C 24 ??`), tức `inc ebx; cmp ebx,5` (đúng 5 byte)
đã bị thay bằng `jmp rel32`, chỗ các DLL kiểu "NoWeight" hook vào. Nếu khớp, log
báo đã bị mod khác hook, kèm tên DLL chứa đích của `jmp` (hoặc "unknown" nếu
`jmp` nhảy vào stub cấp phát ngoài mọi module). Chưa test được trong game vì cần
một mod trọng lượng khác để dựng lại tình huống.

Báo lỗi trên Nexus ("anchor pattern not found", 2026-09-26) có nguyên nhân chưa
rõ: nghi có DLL giảm trọng lượng khác đã hook đè lên `inc ebx`, hoặc khác phiên
bản game; đang chờ người báo trả lời, không thấy ghi nhận kết luận.

## Kiểm tra tương thích (IDA, 2026-10-07)

| Bản exe | Neo (RVA) | Điểm patch (neo + 12) | Hai lệnh tại đó |
|---|---|---|---|
| 2.6.2.0 | `0x247C9E` | `0x247CAA` | `movaps xmm0,xmm6` / `mov rbx,[r11+10h]` |
| 2.7.0.0 | `0x247C9E` | `0x247CAA` | như trên |
| 2.7.1.0 | `0x247C9E` | `0x247CAA` | như trên |

Mẫu duy nhất ở cả ba và **cùng RVA** (khác `runemultiplier`, nơi RVA lệch giữa
các bản). Mod vẫn chỉ dùng AOB, không hard-code RVA.

## Khi test

- **Luôn kiểm cả hai mặt:** số hiển thị trên UI **và** hành vi thật (roll, chạy).
  Bài học từ lần thử 2: một hook có thể đúng nửa việc (UI) mà sai nửa còn lại,
  dù không crash và không báo lỗi gì.
- Đây là patch code sống: nếu bản game đã đổi, có thể crash. Test ở nơi an toàn
  (không giữa trận boss).
- Xem log có dòng `Weight-summing-loop anchor pattern not found` không.
