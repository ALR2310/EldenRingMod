# RuneMultiplier

Mod cho Elden Ring: nhân hệ số cấu hình được (`RuneMultiplier` trong ini,
mặc định `2.0`) lên rune nhận được từ **mọi nguồn** — 1 hook duy nhất,
patch thẳng vào hàm cộng-rune cấp thấp nhất của game (`AddSoul_Call`).

## Bản Rust hiện tại (2026-08-18)

Đã **viết lại hoàn toàn bằng Rust** (`cdylib`), thay cho bản C++ ban đầu -
sống ở `crates/runemultiplier` trong workspace
[`EldenRingMod`](../../README.md), không còn là repo Git riêng. Cùng 1 kỹ
thuật hook (patch 12 byte đầu `AddSoul_Call`, xem RE chi tiết bên dưới -
vẫn đúng nguyên bản), chỉ đổi cách redirect:

- **Bỏ hẳn `CodePatch.cpp`** (tìm vùng nhớ thực thi gần target bằng
  `VirtualAlloc` quét lùi/tiến theo bước `0x10000`, để `JMP E9 rel32` vừa
  tầm 32-bit signed). Thay bằng **jump tuyệt đối** cả 2 chiều — đúng kỹ
  thuật `mov rax, imm64; jmp rax` mà `autoregen`/`sometweaks`'s
  `attack_hook.rs` đã dùng: patch site ghi `mov rax,<stub>; jmp rax` (12
  byte, vừa khít không cần NOP đệm), stub tự nó kết thúc bằng
  `mov r11,<return_addr>; jmp r11`. Vì là jump tuyệt đối, `VirtualAlloc`
  giờ gọi thẳng không cần địa chỉ gợi ý (`std::ptr::null_mut()`) — stub có
  thể nằm bất cứ đâu trong không gian địa chỉ 64-bit, không còn ràng buộc
  khoảng cách.
- Đọc/ghi ini/log/parse hotkey giờ dùng chung crate [`common`](../../shared)
  (`common::input::parse_virtual_key` — tách ra từ chính `autoregen`/
  `sometweaks` khi port mod này, vì cả 3 mod cần y hệt 1 hàm) thay vì code
  Config/Logger riêng.
- Vòng lặp theo dõi `HotReloadKey` chuyển từ thread `Sleep`+`GetAsyncKeyState`
  polling mỗi 50ms sang task đăng ký trên `CSTaskGroupIndex::FrameBegin`
  (`eldenring::util::input::is_key_pressed`, cùng cơ chế debounce cạnh lên
  `autoregen`/`sometweaks` đã dùng cho `ReloadKey`) — nhất quán với các mod
  khác trong workspace, dù bản thân việc patch code không bắt buộc phải
  chạy trên main thread của game.
- Giữ nguyên toàn bộ ini key gốc (`RuneMultiplier`, `HotReloadKey`,
  `DebugLog`) — không đổi tên.

Xem `src/hook.rs` cho code hiện tại; `RuneMultiplier.ini` cho toàn bộ key
cấu hình.

## Lịch sử dịch ngược & quyết định kỹ thuật (bản C++ gốc, kỹ thuật RE vẫn đúng)

**Đã test trong game (2026-08-14), hoạt động đúng, không crash** (trên bản
C++). Bản Rust dùng lại đúng anchor/offset/RE bên dưới, chưa test lại
riêng trong game.

### Vì sao chỉ còn 1 hook (từng có 2, gây cộng dồn sai)

**Thiết kế ban đầu có 2 hook:**
1. Hook tại 1 điểm tính riêng cho **enemy-kill reward** (AOB
   `F3 0F 59 C1 F3 0F 2C F8 48 8B 8B`, khớp với entry `"Rune Multiplier "`
   trong CT table cộng đồng `eldenring_all-in-one_Hexinton-v6.1_ce7.5.ct`
   và với 1 mod DLL cộng đồng đã build sẵn — xem lịch sử RE ở cuối file).
   Test trong game: **enemy-kill nhân đúng**, nhưng **item rune (Golden
   Rune...) không đổi** — không đi qua điểm tính này.
2. Để phủ luôn item rune, thêm hook thứ 2 patch thẳng vào `AddSoul_Call`
   — hàm cộng rune cấp thấp nhất, dùng chung cho **mọi** nguồn (xem RE chi
   tiết ở dưới).

**Phát hiện khi test cả 2 cùng lúc (2026-08-14, người dùng report):** kill
enemy 64 rune, đáng lẽ ra `×2 = 128`, nhưng thực tế ra `256` — bị nhân 2
lần. Nguyên nhân: enemy-kill **cũng đi qua `AddSoul_Call`** (đúng như RE
dự đoán), nên hook 1 nhân trước, hook 2 nhân tiếp lên kết quả đã nhân —
cộng dồn sai. **Đã bỏ hoàn toàn hook 1** — không patch điểm đó nữa, chỉ
còn dùng đúng nó để *định vị* `AddSoul_Call` (đọc `call` instruction tại
`anchor+0x11`, không cần AOB riêng cho `AddSoul_Call`). Hook 2 (nay là
hook duy nhất) tự nó đã phủ đúng cả enemy-kill lẫn item rune.

### Cơ chế hook duy nhất — patch thẳng `AddSoul_Call`

**Disassemble `eldenring.exe` thật** (Ghidra headless, không full-analysis
— quá chậm với file 87MB, chỉ disassemble vùng cần) cho `AddSoul_Call`:

```
MOV R9D,[RCX+0x6C]        ; R9D = rune hiện tại
LEA R8D,[R9+RDX*1]        ; sum = current + amount (RDX = amount)
CMP R8D,0x3B9AC9FF          ; clamp về cap 999,999,999
...
MOV [RCX+0x6C],EAX          ; ghi rune mới — điểm cộng rune thật, dùng chung mọi nguồn
RET
```

**Xác nhận chéo bất ngờ**: offset `[RCX+0x6C]` khớp **chính xác** với
`OFFSET_2 = 0x6C` mà `PassiveRunes` (bản C++) đã tìm ra hoàn toàn độc lập
(qua pointer-chain từ `GameDataMan`) — 2 nỗ lực RE khác nhau, khác thời
điểm, ra cùng 1 offset cho cùng 1 field `rune_count`. Bản Rust hiện tại
của `passiverunes` xác nhận offset này gián tiếp qua field
`GameDataMan::main_player_game_data.rune_count` trong `fromsoftware-rs`.

**Tìm địa chỉ `AddSoul_Call` không cần AOB riêng**: dùng lại AOB của điểm
enemy-kill cũ (`ANCHOR_PATTERN`, vẫn còn trong code, chỉ để định vị), đọc
`call` instruction tại `anchor+0x11`, resolve rel32 ra địa chỉ tuyệt đối.

**Cách patch**: ghi đè 12 byte đầu của `AddSoul_Call` (3 instruction:
`mov r9d,[rcx+0x6c]` + `xor r11d,r11d` + `mov [rsp+0x10],r11d` — không cái
nào đọc `RDX`/`EDX`) bằng code nhân `EDX` (amount) theo **fixed-point Q20
integer** (không dùng XMM, vì hàm này bị gọi từ rất nhiều nơi, không nên
giả định `xmm0` "chết" ở mọi nơi gọi), rồi chạy lại đúng 3 instruction gốc
(copy trực tiếp từ bộ nhớ game lúc cài hook, không hardcode tay) trước khi
nhảy về.

### Vì sao không cần try/catch quanh code assembly đã inject

Không thể bọc theo nghĩa thông thường — `try`/`catch` (và cả SEH
`__try`/`__except`, hay `catch_unwind` bên Rust) hoạt động dựa vào unwind
qua call-frame có handler table; code assembly ghép thẳng vào giữa 1 hàm
bằng `jmp` (kỹ thuật splice) không có scaffolding đó. `AutoRegen/AttackHook`
có dùng `catch_unwind`, nhưng bọc quanh **hàm Rust** nó gọi ra
(`on_attack_observed`), không phải quanh đoạn asm chèn trực tiếp.

Hook này an toàn không phải vì có try/catch, mà vì **không dereference bất
kỳ con trỏ game nào** — chỉ đụng thanh ghi CPU (`rax`, `rdx`, `r8`, `r10`,
`r11`) và đọc 1 biến global tĩnh (`FIXED_Q20`) luôn hợp lệ suốt đời DLL,
không bao giờ null. Khác hẳn `AttackHook` phải đọc `WorldChrMan`/player
pointer (có thể null lúc loading/transition, nên mới cần bảo vệ ở đó).

### Đã verify (bản C++)

- **Byte-level encoding**: dump đúng byte multiply-code, import raw vào
  Ghidra như x86-64 rồi disassemble ngược — khớp 100% ý định, không lỗi
  encode tay.
- **Độ chính xác phép tính fixed-point**: test ~100 tổ hợp (amount từ 0
  đến 999,999,999 × multiplier từ 0 đến 100) so với `round(amount×multiplier)`
  bằng double — lệch tối đa **±1 rune** ở mọi mức reward thực tế trong
  game (do `SAR` làm tròn xuống thay vì tròn gần nhất — không đáng kể).
- **Test trong game (2026-08-14)**: hoạt động đúng, không crash, không
  còn cộng dồn sai sau khi bỏ hook 1.

## Config

3 key, giữ nguyên từ bản C++ (`RuneMultiplier.ini`):
- `RuneMultiplier` (float) — hệ số nhân, `1.0` = không đổi.
- `HotReloadKey` (mặc định `F9`) — phím kích hoạt reload config. Nhận
  `F1`-`F24`, 1 ký tự/số (`G`), hoặc **raw virtual-key code** dạng hex
  (`0x2D`) / decimal (`45`) — xem
  [danh sách VK code của Microsoft](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes).
- `DebugLog` (0/1, mặc định `0`) — **`RuneMultiplier.log` chỉ được tạo khi
  giá trị này là `1`**. Bật `1` thì có thêm hex dump byte gốc tại điểm
  patch và byte của stub tự sinh. Đổi qua hotkey reload lúc đang chạy
  cũng có tác dụng.

## Hot reload — sửa ini rồi bấm phím, không cần khởi động lại game

Hot reload **không cần patch lại hook** — stub đã cài luôn đọc
`FIXED_Q20` qua con trỏ tại thời điểm chạy, nên chỉ cần cập nhật giá trị
biến đó (`init_multiplier` chạy lại) là đủ, không đụng gì đến vùng nhớ đã
patch. Đổi `HotReloadKey` cần khởi động lại (phím chỉ đọc 1 lần lúc cài
hook trong bản C++; bản Rust đọc lại config mỗi tick nên **có thể đổi
`HotReloadKey` mà không cần khởi động lại** — khác biệt nhỏ so với bản
C++, do dùng chung cơ chế tick với `autoregen`/`sometweaks` thay vì đọc 1
lần lúc cài hook).

## Chưa test (bản Rust)

- Test thật trong game: enemy-kill, item rune, hot-reload/hotkey.
- Pattern/offset dịch ngược từ 1 bản game cụ thể — nếu game đã vá sau đó,
  cần dò lại `ANCHOR_PATTERN` bằng Cheat Engine/Ghidra như quy trình cũ.

## Nguồn RE tham khảo ban đầu (hook 1, đã bỏ patch nhưng vẫn dùng để định vị)

1. **CT table cộng đồng** (`eldenring_all-in-one_Hexinton-v6.1_ce7.5.ct`),
   entry `"Rune Multiplier "` (không mã hoá — khác phần lớn entry khác
   trong table đó là tính năng trả phí bị `decodeFunction` che):

   ```
   aobscanmodule(rune_multiplier,eldenring.exe,f3 0f 59 c1 f3 0f 2c f8 48 8b 8b)
   newmem:
     movss xmm0, [rn_mult]
   code:
     mulss xmm0,xmm1
     cvttss2si edi,xmm0
   rn_mult:
    dd (float)2
   // ORIGINAL CODE - INJECTION POINT: eldenring.exe+630CB3
   ```

2. **1 mod DLL cộng đồng đã build sẵn** (tên gốc `法环多倍卢恩.dll`). Hàm
   `Load` của nó AOB-scan đúng cùng byte pattern, patch 8 byte gốc thành
   `jmp` tới stub tự viết — cùng kỹ thuật, khác tác giả.

Cả 2 nguồn đều **thay thế hẳn `xmm0`** bằng hằng số multiplier trước khi
nhân với `xmm1` tại điểm này — cách patch cũ (đã bỏ) của mod này từng cải
tiến so với cả 2 (giữ nguyên `mulss xmm0,xmm1` gốc rồi nhân thêm hệ số
riêng lên kết quả), nhưng vấn đề cộng dồn với hook `AddSoul_Call` khiến
toàn bộ điểm patch này bị loại bỏ — chỉ AOB còn giá trị để định vị.
