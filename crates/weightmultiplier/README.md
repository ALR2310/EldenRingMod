# WeightMultiplier

Đổi tên từ **ReductionWeight** (2026-08-18) khi port sang Rust và gộp vào
workspace [`EldenRingMod`](../../README.md), theo cùng quy ước đặt tên với
`RuneMultiplier` (mô tả đúng cơ chế "nhân hệ số", không chỉ "giảm" — ini
key `WeightReductionPercent` âm sẽ *tăng* tải trọng, không chỉ giảm). Sống
ở `crates/weightmultiplier`, không còn là repo Git riêng.

DLL mod cho Elden Ring: giảm % Trọng Tải (equip load) hiện tại theo 1 hệ số
cố định, thay vì ghi đè cứng thành 1 số tùy ý như các mod "NoWeight" thông
thường.

## Bản Rust hiện tại (2026-08-18)

Đã **viết lại hoàn toàn bằng Rust** (`cdylib`), thay cho bản C++ ban đầu.
Giữ nguyên 100% kỹ thuật RE (AOB pattern, offset, điểm hook) — xem mục
"Cơ chế" và "Lịch sử tìm offset" bên dưới, vẫn đúng nguyên bản. Khác biệt
so với bản C++:

- **Không dùng được kỹ thuật absolute-jump của `attack_hook.rs`/
  `runemultiplier`** (patch site chỉ có **7 byte** để ghi đè — không đủ
  chỗ cho `mov reg,imm64; jmp reg` cần ~12 byte). Giữ nguyên kỹ thuật gốc:
  JMP tương đối 5 byte (`E9 rel32`) + tìm vùng nhớ thực thi gần bằng
  `VirtualAlloc` (quét lùi/tiến theo bước `0x10000`) — port thẳng
  `CodePatch.cpp`/`.h` thành `common::codepatch` (dùng chung, có thể tái
  sử dụng cho mod khác gặp đúng tình huống "không đủ 12 byte" này).
- Đọc/ghi ini/log giờ dùng chung crate [`common`](../../shared) thay vì
  Config/Logger riêng.
- **Không có `eldenring`/`fromsoftware-shared` trong dependency** — khác
  mọi mod Rust khác trong workspace. Mod này không đọc/ghi struct nào của
  game, không cần theo dõi hotkey qua `CSTaskImp` (xem mục dưới) — chỉ
  patch bộ nhớ 1 lần từ 1 thread thường, y hệt bản C++.
- **Vẫn cố tình không có hotkey reload** — giữ nguyên quyết định thiết kế
  gốc (tránh trùng phím với mod khác). Sửa `WeightReductionPercent` cần
  khởi động lại game, không đổi so với bản C++.
- Giữ nguyên toàn bộ ini key gốc (`InitialDelaySeconds`,
  `WeightReductionPercent`) — không đổi tên, dù crate/DLL đã đổi tên.

Xem `src/hook.rs` cho code hiện tại; `WeightMultiplier.ini` cho toàn bộ key
cấu hình.

## Cơ chế (không đổi từ bản C++)

Không đọc/viết field trong struct (như `autoregen` làm với HP/FP/Stamina).
Thay vào đó, DLL **patch trực tiếp 1 lệnh CPU** trong code của
`eldenring.exe` — kỹ thuật inline hook (JMP tới 1 đoạn code tự cấp phát,
rồi JMP quay lại).

### Lịch sử tìm offset (3 lần thử)

**Lần 1 (sai, đã bỏ):** suy ra từ decompile `Zibinha_Weight_Control.dll`,
hook **lùi lại 5 byte** trước AOB match (đoán là lệnh
`addss xmm6, [rax+0xc]`). Build/chạy không lỗi, nhưng test thực tế cho
thấy **không có tác dụng gì** — Trọng Tải không đổi dù đổi giáp/vũ khí.
Nguyên nhân: `addss` chỉ chạy có điều kiện (slot có đồ mới cộng), không
nằm cố định cách AOB đúng 5 byte — hook trúng byte giữa 1 lệnh khác, vô
hiệu mà không crash.

**Lần 2 (đúng UI, sai gameplay):** dò trực tiếp qua Cheat Engine trên game
đang chạy — pin địa chỉ "Trọng Tải Hiện Tại" bằng Value Between, rồi
**"Find out what writes to this address"** → bắt được `movss [rsi+1C],xmm0`
(rsi = struct equip-load, xmm0 = tổng cuối). Hook tại đây làm **UI hiện
đúng số đã giảm %**, nhưng **roll-type/tốc độ chạy trong game không đổi** —
tức chỉ ảnh hưởng đường hiển thị, không phải đường gameplay thực tế dùng để
tính vật lý.

**Lần 3 (đúng, đang dùng):** người dùng thực sự cài
`Zibinha_Weight_Control.dll` (set `WeightValue=0`) và xác nhận **roll-type
đổi đúng thật** trong game — DLL này ghi log địa chỉ patch thật của nó
(`target=0x...`), và địa chỉ đó khớp *chính xác* với vị trí AOB pattern ở
Lần 1 (không lùi 5 byte). Vậy AOB pattern ban đầu **đúng** — sai chỉ ở việc
tự lùi 5 byte để "hook sớm hơn".

Zibinha hook thẳng vào `inc ebx` (đầu vòng lặp cộng trọng lượng), ghi đè
`xmm6` = giá trị cố định **mỗi lần lặp** — chỉ có lần ghi cuối (ngay trước
khi loop thoát) là còn tồn tại, nên tổng cuối luôn = số cố định. Cách này
tốt cho "set cứng = 0" nhưng **không dùng được cho %**: nhân `xmm6` theo %
tại chính điểm này sẽ bị **dồn qua từng vòng lặp** (món đồ ở slot đầu bị
giảm nhiều hơn món ở slot cuối, không đều).

**Điểm hook đúng cho %:** ngay sau khi loop cộng xong, có lệnh copy tổng ra
làm kết quả trả về:

```asm
addss xmm6, dword ptr [rax + 0xc]   ; cộng trọng lượng slot này
inc ebx                              ; <-- Zibinha hook đây (set cứng)
cmp ebx, 5
jl <loop start>
lea r11, [rsp+0x70]
movaps xmm0, xmm6                    ; <-- WeightMultiplier hook đây (nhân %, chạy đúng 1 lần)
mov rbx, [r11+0x10]
```

Nhân `xmm6` ngay tại `movaps` chỉ chạy **đúng 1 lần** sau khi tổng đã cộng
xong hoàn chỉnh — không bị dồn qua vòng lặp, và tái dùng đúng vị trí đã
được xác nhận thật qua Zibinha.

### AOB pattern (giữ nguyên từ ban đầu, đã xác nhận qua log thật của
Zibinha là trúng đúng vị trí code, không phải trùng ngẫu nhiên):

```
FF C3 83 FB ?? 7C ?? 4C 8D 5C 24 ??
```

Target = anchor + 12 byte (bỏ qua `inc ebx; cmp ebx,5; jl` = 7 byte +
`lea r11,[rsp+0x70]` = 5 byte), trúng `movaps xmm0,xmm6` + `mov rbx,[r11+0x10]`
(7 byte, đủ chỗ cho JMP tương đối 5 byte).

### Stub được chèn

```asm
mov   rcx, &WEIGHT_FACTOR          ; địa chỉ hệ số nhân (đọc từ .ini)
mulss xmm6, dword ptr [rcx]        ; nhân tổng trọng lượng cuối theo % giảm (1 lần)
movaps xmm0, xmm6                  ; lệnh gốc - copy tổng đã giảm ra làm kết quả
mov   rbx, qword ptr [r11+0x10]    ; lệnh gốc tiếp theo, chạy lại như cũ
jmp   <target + 7>                 ; quay lại code gốc (E9 rel32, qua common::codepatch)
```

`WEIGHT_FACTOR` = `1.0 - WeightReductionPercent/100`, đọc từ
`WeightMultiplier.ini` **một lần khi DLL load**.

## Trạng thái

| Việc | Trạng thái |
| --- | --- |
| RE gốc (AOB pattern, xác nhận chéo qua Zibinha) | Xong (bản C++) |
| Port sang Rust (`common::codepatch` + `hook.rs`) | Xong, build được |
| Test trong game (cả UI và roll-type thực tế) | **Chưa** (bản C++ lẫn bản Rust) |

## Rủi ro cần lưu ý khi test

- Đây là patch code sống (ghi đè lệnh CPU đang thực thi) — nếu bản game đã
  đổi, có thể crash game. Test trước ở nơi an toàn (không giữa 1 trận boss).
- Kiểm tra log `WeightMultiplier.log` xem có dòng
  `ERROR: weight-summing-loop anchor pattern not found` không.
- **Luôn test cả 2 mặt**: số hiển thị trên UI *và* hành vi thực tế (roll,
  chạy) — bài học từ Lần 2 là 1 hook có thể đúng nửa việc (UI) mà sai nửa
  còn lại (gameplay), dù không crash và không báo lỗi gì.
