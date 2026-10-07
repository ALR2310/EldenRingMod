# RuneMultiplier: hook vào `AddSoul_Call`

**Status 2026-10-07: ĐANG PHÁT HÀNH (1.0.0 đến 1.0.4), hướng thiết kế đã chốt.**
Test trong game từ bản C++ (2026-08-14) và bản Rust được phát hành, cập nhật
cho ER 1.17 và 1.17.1. Mẫu neo AOB đã được kiểm tra lại bằng IDA ngày
2026-10-07: duy nhất trên cả 2.6.2.0, 2.7.0.0 và 2.7.1.0. Code:
`mods/runemultiplier/src/hook.rs`.

## Vì sao hook ở `AddSoul_Call`

`AddSoul_Call` là hàm cấp thấp nhất cộng rune của game, dùng chung cho **mọi
nguồn** (giết quái, nhặt/dùng item, bán đồ...). Disassemble `eldenring.exe`
thật (Ghidra headless, không full-analysis vì file 87 MB quá chậm):

```
MOV R9D,[RCX+0x6C]        ; R9D = rune hiện tại
LEA R8D,[R9+RDX*1]        ; sum = current + amount (RDX = amount)
CMP R8D,0x3B9AC9FF        ; kẹp về cap 999,999,999
...
MOV [RCX+0x6C],EAX        ; ghi rune mới
RET
```

Offset `[RCX+0x6C]` khớp chính xác với `OFFSET_2 = 0x6C` mà PassiveRunes (bản C++)
tìm ra hoàn toàn độc lập qua pointer-chain từ `GameDataMan` (và khớp field
`GameDataMan::main_player_game_data.rune_count` trong fromsoftware-rs): hai
nỗ lực RE khác nhau cho ra cùng một field.

## Cách tìm `AddSoul_Call` (không cần AOB riêng)

Hàm không có byte pattern riêng đủ đặc trưng, nên mod dùng một mẫu neo
(`ANCHOR_PATTERN` = `F3 0F 59 C1 F3 0F 2C F8 48 8B 8B`) là bước nhân phần thưởng
giết quái của game (`mulss xmm0,xmm1; cvttss2si edi,xmm0; ...`), rồi đọc lệnh
`call` (`E8 rel32`) tại `anchor + 0x11` (4 + 4 + 7 + 2 = 17 byte) và giải rel32
ra địa chỉ tuyệt đối. Nếu byte tại đó không phải `0xE8`, mod coi bố cục khác
dự kiến và không patch.

Kiểm tra bằng IDA (2026-10-07):

| Bản exe | Neo (RVA) | `call` tại `anchor+0x11` | `AddSoul_Call` |
|---|---|---|---|
| 2.6.2.0 | `0x651883` | `0x651894` | `0x25E100` |
| 2.7.0.0 | `0x6526D3` | `0x6526E4` | `0x25E0E0` |
| 2.7.1.0 | `0x6526D3` | `0x6526E4` | `0x25E0E0` |

Mẫu duy nhất ở cả ba. 12 byte đầu của `AddSoul_Call` giống hệt ở cả ba bản
(`44 8B 49 6C` / `45 33 DB` / `44 89 5C 24 10`), đúng với thiết kế bên
dưới. RVA lệch giữa các bản nên mod chỉ dùng AOB, không hard-code RVA.

## Cách patch

Ghi đè **12 byte đầu** của `AddSoul_Call` (3 lệnh `mov r9d,[rcx+0x6c]` +
`xor r11d,r11d` + `mov [rsp+0x10],r11d`, không lệnh nào đọc `RDX/EDX`) bằng
`mov rax,<stub>; jmp rax` (10 + 2 = 12 byte, vừa khít, không cần NOP đệm).
Stub (sinh lúc chạy bằng `VirtualAlloc`) nhân `EDX` (amount), chạy lại đúng 3
lệnh gốc (copy trực tiếp từ bộ nhớ game lúc cài hook, không hard-code tay),
rồi `mov r11,<return>; jmp r11` về ngay sau 12 byte đó.

- **Nhân bằng số nguyên fixed-point Q20**, không dùng XMM: `AddSoul_Call` bị
  gọi từ rất nhiều nơi nên không được giả định thanh ghi XMM nào "chết" ở mọi
  nơi gọi. Hệ số lưu trong `FIXED_Q20 = round(Multiplier * 2^20)`, stub đọc nó
  qua con trỏ lúc chạy, nên hot reload chỉ cần cập nhật biến này, không patch
  lại. Sai số tối đa **±1 rune** (do `SAR` làm tròn xuống), kiểm tra trên ~100
  tổ hợp amount (0 đến 999,999,999) × multiplier (0 đến 100).
- **Chỉ nhân khi `amount > 0`**: stub có `test eax,eax; jle skip`. Xem mục
  "Hướng đã bỏ" bên dưới.
- **Jump tuyệt đối thay `E9 rel32`**: bản C++ (`CodePatch.cpp`) phải quét các
  trang gần target để tìm vùng `VirtualAlloc` được trong tầm 32-bit. Jump tuyệt
  đối cả hai chiều (cùng kỹ thuật `attack_hook.rs` của autoregen) bỏ hẳn ràng
  buộc khoảng cách, nên stub nằm ở đâu cũng được.
- Đã kiểm tra encoding ở mức byte: dump byte của code nhân, nạp vào Ghidra như
  x86-64 và disassemble ngược, khớp 100% ý định.

### Vì sao không cần try/catch quanh code assembly

Không bọc được theo nghĩa thông thường: `try/catch`, SEH hay `catch_unwind`
dựa vào unwind qua call frame có handler table, mà đoạn asm ghép thẳng giữa
một hàm bằng `jmp` thì không có scaffolding đó. Hook này an toàn không nhờ
try/catch mà vì **không dereference con trỏ game nào**: chỉ đụng thanh ghi CPU
(`rax`, `rdx`, `r8`, `r10`, `r11`) và đọc một biến global tĩnh luôn hợp lệ suốt
đời DLL. Khác `AttackHook` của autoregen, hook đó phải đọc `WorldChrMan` / con
trỏ player (có thể null lúc loading), nên mới cần bảo vệ ở đó và dùng
`catch_unwind` quanh **hàm Rust** nó gọi ra, không quanh asm.

## Hướng đã thử và bỏ

1. **Hook thứ hai ở bước nhân thưởng giết quái** (chính điểm neo ở trên):
   nhân đúng enemy-kill nhưng **không phủ rune từ item** (Golden Rune...). Thêm
   hook `AddSoul_Call` để phủ item thì giết quái bị **nhân hai lần** (64 rune
   ra 256 thay vì 128), vì enemy-kill cũng đi qua `AddSoul_Call`. Bỏ hẳn hook
   thứ nhất (2026-08-14); điểm đó chỉ còn dùng làm neo định vị.
2. **Nhân mọi `amount` không điều kiện** (1.0.0): `AddSoul_Call` là hàm
   "current += amount" tổng quát, nên cả khoản **trừ** (lên cấp, mua đồ,
   `amount` âm) cũng bị nhân: giá đồ tăng và rune còn dư về 0 sau khi lên cấp
   (hai người dùng Nexus báo). Script CE "Rune Multiplier" gốc chỉ nhân đúng
   phép tính phần thưởng giết quái, chưa từng hook `AddSoul_Call` trực tiếp.
   Sửa ở 1.0.1: stub bỏ qua khối nhân khi `amount <= 0`.

Hệ quả của `amount > 0`: `Multiplier` vẫn có thể nhỏ hơn 1 hoặc là số âm (rune
nhận được bị giảm hay bị trừ), vì chỉ khoản **nhận** mới bị nhân.

## Nguồn tham khảo ban đầu (cho hook thứ nhất, đã bỏ patch nhưng còn dùng làm neo)

1. CT table cộng đồng `eldenring_all-in-one_Hexinton-v6.1_ce7.5.ct`, entry
   "Rune Multiplier " (không mã hoá, khác phần lớn entry khác trong table đó là
   tính năng trả phí bị `decodeFunction` che):
   ```
   aobscanmodule(rune_multiplier,eldenring.exe,f3 0f 59 c1 f3 0f 2c f8 48 8b 8b)
   ...
   mulss xmm0,xmm1 / cvttss2si edi,xmm0
   // injection point: eldenring.exe+630CB3 (bản game lúc đó)
   ```
2. Một mod DLL cộng đồng build sẵn (tên gốc `法环多倍卢恩.dll`): hàm `Load` AOB-scan
   đúng cùng pattern, patch 8 byte gốc thành `jmp` tới stub tự viết.

Cả hai **thay hẳn `xmm0`** bằng hằng số multiplier trước khi nhân với `xmm1`.
Cách patch cũ của mod này (đã bỏ) từng cải tiến hơn: giữ `mulss xmm0,xmm1` gốc
rồi nhân thêm hệ số lên kết quả; nhưng chồng với hook `AddSoul_Call` thì cộng
dồn sai nên bỏ.

## Giới hạn

- Mẫu neo và cấu trúc `call` tại `anchor+0x11` suy ra từ các bản exe hiện có;
  game vá làm đổi bố cục thì mod ghi `Anchor pattern not found` / `Couldn't
  resolve AddSoul_Call` và **tự tắt cho phiên đó** (không patch gì).
- Chưa kiểm chứng các bản exe ngoài 2.6.2.0, 2.7.0.0, 2.7.1.0.
