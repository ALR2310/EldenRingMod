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

## Đổi ini sang `Mode`/`Multiplier`/`FixedValue`, thêm chế độ giá trị cố định (2026-09-14)

Đổi hẳn bộ ini key, không cố giữ tương thích với `WeightReductionPercent`
cũ (theo yêu cầu người dùng - key mới hoàn toàn, không phải chỉ đổi tên):

- `Mode` (0/1) chọn cách tính - `0` = `Multiplier` (hệ số nhân trực tiếp,
  thay cho `WeightReductionPercent`: `Multiplier=0.5` ~ giảm 50% cũ,
  `Multiplier=1.0` = không đổi, `Multiplier=2.0` = tăng gấp đôi, cùng đơn vị
  với key `Multiplier` của [`runemultiplier`](../runemultiplier)), `1` =
  `FixedValue` (đặt cứng tải trọng bằng đúng 1 số, bất kể đang mặc gì).
- **Breaking hoàn toàn** với `WeightReductionPercent` - không phải đổi tên
  suông, đơn vị cũng đổi (% giảm → hệ số nhân trực tiếp), người dùng cũ
  phải tự tính lại giá trị nếu nâng cấp.

Implementation: tận dụng đúng 1 điểm hook đã có (`movaps xmm0,xmm6`, chạy
đúng 1 lần sau khi vòng lặp cộng trọng lượng đã xong - xem mục "Cơ chế" ở
trên) cho cả 2 mode, không cần thêm điểm patch nào khác như DLL "NoWeight"
bên thứ 3 (Zibinha) phải làm (hook giữa vòng lặp để set cứng, xem mục
"Lịch sử tìm offset"). `build_stub()` giờ chọn `mulss xmm6,[rcx]` (mode
`Multiplier`, giữ nguyên như cũ) hay `movss xmm6,[rcx]` (mode `FixedValue`,
ghi đè thẳng tổng đã cộng xong) tùy `Mode`, cùng 1 static giá trị
`WEIGHT_VALUE` (đổi tên từ `WEIGHT_FACTOR`) mang ý nghĩa khác nhau tùy
mode - không cần 2 static riêng vì chỉ 1 trong 2 được dùng tại 1 thời điểm
(chọn lúc `install()`, trước khi bake vào stub).

## Thêm hot reload (`ReloadKey`), `[Logging] LogFile`, đổi `InitialDelaySeconds` → `LoadDelay` (2026-09-14)

Bỏ hẳn quyết định thiết kế gốc "cố tình không có hotkey reload" (từng ghi
trong doc comment đầu `hook.rs`) - theo yêu cầu người dùng, thêm `[General]
ReloadKey=F5` giống mọi mod khác trong workspace, cộng thêm `[Logging]
LogFile=true` (đồng bộ tên section/key logging chung, xem README của
`autoregen`/`sometweaks`/`passiverunes`/`runemultiplier` cùng ngày).

- **Không thêm `fromsoftware-rs`**: mod này vẫn cố tình không phụ thuộc
  crate đó (xem đầu file) - poll `ReloadKey` bằng `GetAsyncKeyState` thô
  (`common::input::is_key_pressed`, mới thêm, khai báo `extern "system"`
  thủ công + `#[link(name = "user32")]`, cùng phong cách
  `codepatch.rs` đã khai báo `VirtualAlloc`/`VirtualProtect` cho
  `kernel32`) thay vì `eldenring::util::input::is_key_pressed` (cần đăng ký
  qua `CSTaskImp`, chỉ mods có `fromsoftware-rs` mới dùng được). Vòng lặp
  reload chạy trên 1 OS thread thường (`std::thread::sleep` 100ms/lần), y
  hệt kiểu polling gốc mà `runemultiplier` từng dùng trước khi nó chuyển
  sang `CSTaskGroupIndex::FrameBegin` (xem README của nó, mục "Bản Rust
  hiện tại").
- **Đổi `Mode` lúc đang chạy phức tạp hơn đổi `Multiplier`/`FixedValue`**:
  giá trị (`Multiplier`/`FixedValue`) hot-reload an toàn vì stub đã cài đọc
  từ 1 địa chỉ cố định (`WEIGHT_VALUE`) mỗi lần chạy - chỉ cần ghi đè giá
  trị đó. Nhưng `Mode` chọn **opcode nào** chạy (`mulss` hay `movss`) đã
  ghi cứng vào code thực thi lúc `install()` - đổi `Mode` lúc runtime cần
  tự vá lại đúng 4 byte đó (`STUB_MODE_INSTR_ADDR`, qua
  `codepatch::overwrite_bytes`) - rủi ro y hệt lúc `install()` ban đầu
  (ghi đè code CPU có thể đang thực thi), chỉ khác là lặp lại theo yêu cầu
  thay vì chỉ 1 lần. Người dùng đã xác nhận chấp nhận đánh đổi này (câu
  hỏi "hỗ trợ đổi Mode luôn hay chỉ đổi số" - chọn hỗ trợ đổi Mode luôn).
- **`catch_unwind` quanh mỗi lần poll** (không phải `run_recurring_safe`
  kiểu `autoregen`/`sometweaks`): vòng lặp reload chạy trên thread riêng
  của chính mod, không phải do game gọi vào qua `CSTaskImp` như các mod
  kia - panic ở đây không có rủi ro phá call stack C++ của game, chỉ cần
  không làm chết luôn thread reload cho phần còn lại của session.
- **`InitialDelaySeconds` → `LoadDelay`, đơn vị giây → mili giây** (giá trị
  mặc định đổi từ `5` thành `5000`, cùng độ trễ thực tế) - khớp quy ước
  "mili giây" mà mọi key thời gian khác trong workspace đang dùng
  (`Regen.PerTick.Interval`, `Rune.Passive.Interval`...).

`WeightMultiplier.ini` đổi section: `[General]` (mới, chứa `ReloadKey` +
`LoadDelay`), `[Features]` (đổi tên từ không-section-cụ-thể, chứa
`Mode`/`Multiplier`/`FixedValue`), `[Logging]` (mới, chứa `LogFile`).

## `LogFile=false` giờ không tạo file log nữa - sửa trong `common::logger` (2026-09-23)

Người dùng phát hiện (qua PassiveRunes): `LogFile=false` nhưng file `.log`
vẫn được tạo. Đúng là trước đó `logger::init` được gọi vô điều kiện trong
`lib.rs` - file luôn tạo, `LogFile` chỉ gate log chi tiết - trái với tên
key và với cách RiseArcher/RuneMultiplier vốn làm. Người dùng xác nhận hành
vi đúng: `LogFile=true` mới tạo file; muốn có log sẵn thì deploy với mặc
định `LogFile=true` (mod này mặc định `true`).

Sửa tập trung trong `shared/src/logger.rs`: `init` chỉ ghi nhớ đường dẫn,
file được tạo (truncate) ở dòng log đầu tiên khi `LogFile=true`, `LogFile`
đọc lại mỗi lần ghi - bật/tắt bằng `ReloadKey` có hiệu lực ngay. Comment
"log is always on" trong `lib.rs` đã bỏ; mô tả key trong ini đổi thành
"Write WeightMultiplier.log next to the DLL (for troubleshooting). Off = no log
file". Các mục cũ hơn trong README nói "file log luôn được tạo bất kể
`LogFile`" giờ đã lỗi thời.


## Đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini - sửa `common::dll_dir` (2026-09-24)

Bug phát hiện qua AutoRegen (người dùng ME3, thư mục profile tên tiếng
Trung): `common::dll_dir()` dùng `GetModuleFileNameA` (code page ANSI) rồi
giải mã như UTF-8, nên đường dẫn có ký tự không phải ASCII (tiếng Trung,
tiếng Việt có dấu...) bị hỏng, không tìm thấy ini/log cạnh DLL, và mod âm
thầm chạy với cấu hình mặc định. Đã đổi sang `GetModuleFileNameW` +
`from_utf16_lossy`, buffer tự tăng cho đường dẫn dài. Mod này dùng chung
`dll_dir` nên cũng được sửa. Chi tiết xem mục cùng ngày trong
`crates/autoregen/README.md`.
