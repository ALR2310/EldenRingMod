# AutoRegen

Mod tự động hồi HP/FP/Stamina theo thời gian thực cho Elden Ring, cộng thêm
hồi khi chính player đánh trúng địch và/hoặc khi player bị đánh trúng.

## Bản Rust hiện tại (2026-08-18)

Project này đã được **viết lại hoàn toàn bằng Rust** (`cdylib`), thay cho
bản C++/MASM ban đầu — lịch sử dịch ngược AOB/offset/pointer-chain gốc từ
`AutoRecovery.dll` vẫn được giữ nguyên bên dưới vì vẫn còn giá trị tham
khảo (đặc biệt là điểm hook `OnAttack`), nhưng **không còn khớp với code
hiện tại trong `src/`**. Từ 2026-08-18, crate này sống trong workspace
[`EldenRingMod`](../../README.md) (`crates/autoregen`) cùng các mod Rust
khác, không còn là repo Git riêng.

Kiến trúc mẫu lấy từ mod [`SomeTweaks`](../sometweaks) (đổi tên từ
LifeBetween - `src/regen/mod.rs` + `src/regen/attack_hook.rs`) — cùng 1 tác
giả, LifeBetween đã tự làm việc này trước cho phần regen gộp trong DLL
nhiều mod của nó, AutoRegen chỉ tách lại thành 1 mod độc lập với ini key
riêng (không có prefix `Regen.`):

- **Phần hồi HP/FP/Stamina theo tick**: dùng
  [`fromsoftware-rs`](https://github.com/vswarte/fromsoftware-rs) (crate
  `eldenring`) để đọc/ghi `WorldChrMan`/`ChrIns`/`CSChrDataModule` sống trong
  process game — không còn tự AOB-scan/offset tay (`CHR_PATTERN` +
  `WalkPointerChain`) như bản C++ nữa, loại bỏ hẳn rủi ro lệch offset mỗi khi
  game update (xem toàn bộ lịch sử dò lại offset 3-4 lần bên dưới). Tick chạy
  như 1 task đăng ký trên `CSTaskGroupIndex::FrameBegin` của chính game, thay
  vì 1 thread `Sleep` riêng — không còn cần clamp sàn 50ms để tránh busy-spin.
- **Phần `AttackHook` (hồi máu khi đánh trúng / khi bị đánh trúng)**: README
  cũ (mục cuối file) từng kết luận phần này "không có lợi khi chuyển sang
  Rust" vì không có struct/API nào trong `fromsoftware-rs` phản chiếu được
  call site `OnAttack`. Kết luận đó **đã bị đảo ngược** sau khi LifeBetween tự
  chứng minh khả thi: Rust có `std::arch::global_asm!` viết trampoline ASM y
  hệt MASM, và `#[unsafe(no_mangle)] static AtomicUsize` thay cho biến toàn
  cục C++ (`g_ReturnAddr`) — không cần FFI phức tạp gì thêm. AutoRegen giữ
  nguyên hoàn toàn kỹ thuật patch 12-byte + trampoline của bản C++, chỉ đổi
  ngôn ngữ implement.
- **Tính năng mới `HpOnDamage`/`FpOnDamage`/`StaminaOnDamage`**: cùng 1 hook
  `OnAttack` đã có sẵn thông tin cả 2 chiều — `rsi` = attacker (bên gây sát
  thương), `[ctx+8]` = target (bên nhận) — bản C++ trước đây chỉ dùng `rsi`
  để phát hiện "player vừa đánh trúng", còn `[ctx+8]` bỏ không dùng. Bản Rust
  tận dụng nốt để phát hiện "player vừa bị đánh trúng" (`[ctx+8]==player &&
  rsi!=player`), hồi 1 lượng cố định (không có biến thể %) độc lập với
  `HpOnHit`/`FpOnHit`/`StaminaOnHit`.

Xem `src/regen.rs` và `src/attack_hook.rs` cho code hiện tại; `AutoRegen.ini`
cho toàn bộ key cấu hình.

## Gộp `Regen Per Hit`/`Regen Per Damage`, thêm `Condition`/`Trigger` (2026-08-19)

`HpOnDamage`/`FpOnDamage`/`StaminaOnDamage` (thêm lúc port sang Rust, xem mục
"Tính năng mới" ở trên) ban đầu bị hiểu sai ý đồ: code hồi 1 lượng **cố
định** khi player **bị đánh trúng** ("cushion" chống chịu đòn), nhưng ý đồ
thật là hồi theo **% sát thương player vừa gây ra** cho địch (lifesteal thật
sự, ví dụ `0.5` = hồi lại 50% damage vừa gây). Đã sửa lại đúng ý: đọc damage
thật của cú đánh từ `hitInfo+0x228` (offset vốn chỉ dùng cho debug log trước
đó), nhân với hệ số rồi hồi.

Sau khi sửa, `HpOnDamage`/`FpOnDamage`/`StaminaOnDamage` trùng trigger hoàn
toàn với `HpOnHit`/`FpOnHit`/`StaminaOnHit` (cả hai đều chạy khi player đánh
trúng bằng vũ khí) — nên gộp thẳng `[Regen Per Damage]` vào `[Regen Per
Hit]`, chỉ còn 2 nhóm chính (`Regen Per Tick`, `Regen Per Hit`) thay vì 3.
Cách gộp: thêm key `Trigger` (0/1) quyết định `HpPctOnHit`/`FpPctOnHit`/
`StaminaPctOnHit` nghĩa là gì:

- `Trigger=0` (Fixed, hành vi gốc): `HpPctOnHit` = % **max stat**, cộng dồn
  với `HpOnHit` (số cố định).
- `Trigger=1` (damage dealt): `HpPctOnHit` = % **sát thương vừa gây ra** cho
  địch; `HpOnHit`/`FpOnHit`/`StaminaOnHit` bị bỏ qua ở chế độ này.

Cùng đợt, thêm `Condition` (0/1/2) cho `[Regen Per Tick]`: 0 = luôn hồi,
1 = chỉ khi ngoài giao tranh, 2 = chỉ khi đang giao tranh. Game không có sẵn
cờ "đang chiến đấu hay không" nào lộ ra qua `fromsoftware-rs`/engine — xử lý
bằng cách xấp xỉ kiểu nhiều action game khác: `AttackHook` đánh dấu "vừa có
hoạt động combat" (`regen::mark_combat_activity()`) mỗi khi player đánh
trúng **hoặc** bị đánh trúng (hook vẫn cần cài dù không cấu hình heal nào,
nếu `Condition` yêu cầu theo dõi combat), coi là "còn trong combat" trong
15 giây kể từ lần đánh/bị đánh gần nhất (`is_in_combat()`). Không chính xác
100% (ví dụ đứng yên né đòn boss không tính là combat), nhưng đủ dùng cho
mục đích "regen ngoài combat kiểu MMO".

Nhân dịp gộp, dọn luôn cấu trúc thư mục: `src/regen/mod.rs` +
`src/regen/attack_hook.rs` (thư mục con chỉ có 2 file, không cần thiết cho
project nhỏ) → phẳng thành `src/regen.rs` + `src/attack_hook.rs` ngang cấp,
khai báo `mod attack_hook; mod regen;` trong `lib.rs`.

## Đổi ini key sang cấu hình kiểu `SomeTweaks` (2026-08-24)

[`SomeTweaks`](../sometweaks) (cùng module `regen`, port lại từ chính
AutoRegen ở mục "Bản Rust hiện tại" phía trên) sau đó tự phát triển tiếp
thành 1 thiết kế ini gọn hơn: mỗi section có cờ `Enabled` riêng thay vì quy
ước "0 = tắt" rải rác từng key, mỗi stat chỉ còn **1 field giá trị** thay vì
tách riêng flat/percent, và dùng 1 key số (`Trigger`/`Unit`) để chọn field
đó nghĩa là gì. Đợt này đưa thiết kế đó **quay lại** AutoRegen, để 2 mod
dùng chung 1 kiểu cấu hình/code thay vì AutoRegen bị kẹt lại ở bản thiết kế
cũ nó khởi nguồn.

Đổi cụ thể - toàn bộ key cũ **không tương thích ngược**, phải sửa lại
`AutoRegen.ini` thủ công (không tự động migrate vì đổi cả tên lẫn ý nghĩa):

- `[Regen Per Tick]`: `Condition` → `Regen.PerTick.Trigger` (cùng ý nghĩa
  0/1/2), `Interval` → `Regen.PerTick.Interval`. `Hp`/`Fp`/`Stamina` +
  `HpPct`/`FpPct`/`StaminaPct` (6 key) gộp còn 3 key
  `Regen.PerTick.HP`/`FP`/`Stamina`, ý nghĩa (điểm cố định hay % max) do
  `Regen.PerTick.Unit` (0/1) quyết định chung cho cả 3. Thêm
  `Regen.PerTick.Enabled` (mặc định `true`) thay vì dựa vào `Interval=0`.
- `[Regen Per Hit]`: `Trigger` giờ có **3 chế độ** thay vì 2 - `0` = điểm cố
  định, `1` = % max stat, `2` = % sát thương gây ra (lifesteal, thay cho
  `Trigger=1` cũ). `HpOnHit`/`HpPctOnHit` (2 key/stat) gộp còn 1 key
  `Regen.PerHit.HP` (tương tự FP/Stamina), ý nghĩa do `Trigger` quyết định.
  Thêm `Regen.PerHit.Enabled` (mặc định `false`) thay vì dựa vào giá trị
  khác 0.
- `[Debug]`: `DebugLog` → `RegenLog`. Bỏ luôn phần dump `atkCategory`/
  `atkId`/`sourceType` mỗi đòn trúng (dùng để soi lỗi phân loại vũ khí/phép
  hồi lúc trước) - `RegenLog` giờ chỉ log gọn "dealt N damage" + số hồi mỗi
  đòn, đúng những gì người dùng cuối cần theo dõi.

Vì đổi tên/cấu trúc key hoàn toàn, `config::migrate()` (xem
[`shared/src/config.rs`](../../shared/src/config.rs)) sẽ đẩy hết key cũ
(`Condition`, `HpOnHit`, `DebugLog`, ...) vào `[Legacy]` ở lần chạy game đầu
tiên sau khi cập nhật DLL - giá trị cũ không mất, nhưng người dùng cần tự
copy sang key mới nếu muốn giữ cấu hình đã tùy chỉnh.

## Thêm `Regen.PerHit.DamageType` - tùy chọn tính cả đòn phép (2026-08-25)

Từ bản C++ gốc (mục "Chỉ hồi máu khi đánh bằng vũ khí, không tính phép" bên
dưới), Regen Per Hit luôn lọc bỏ đòn từ phép/đạn (`sourceType==3`), chỉ tính
đòn vũ khí cận chiến (`sourceType==1`) - tránh việc spam phép rẻ cũng được
thưởng hồi máu như đánh cận chiến. Có người dùng (phản hồi trên Nexus) muốn
dùng chính cơ chế này để "cycle" FP: cast phép rẻ để hồi FP, dùng FP đó cast
phép đắt hơn - việc này không làm được vì bộ lọc luôn loại phép ra.

Thêm key `Regen.PerHit.DamageType` (0/1/2) để chọn loại sát thương nào được
tính, độc lập với `Trigger`:

- `0` (mặc định, giữ hành vi cũ): chỉ đòn cận chiến.
- `1`: chỉ đòn tầm xa/phép.
- `2`: cả hai.

Đây là 1 điều kiện chung cho cả 3 stat (Hp/Fp/Stamina) trong `[Regen Per
Hit]`, không tách riêng "chỉ Fp hồi từ phép" như yêu cầu gốc - đơn giản hơn,
đủ dùng cho use-case "cast rẻ hồi Fp để cast đắt" nếu người dùng chỉ cấu
hình `Regen.PerHit.FP` (để `HP`/`Stamina` = 0) và đặt `DamageType=1` hoặc
`2`. Implementation: `is_weapon_damage_hit()` cũ đổi tên thành
`matches_damage_type()`, nhận thêm tham số `damage_type` thay vì hard-code
chỉ chấp nhận `sourceType==1`.

## Fix bị vô hiệu hoá vĩnh viễn bởi `InvalidRva` (2026-08-26)

Báo lỗi thật từ bình luận Nexus: `ERROR: CSTaskImp never became available
(InvalidRva) - AutoRegen disabled for this session`. Nguyên nhân:
`CSTaskImp::wait_for_instance` coi `SystemInitError::InvalidRva` là lỗi
chết ngay, **không tự retry** dù truyền `Duration::MAX` (chỉ tự retry case
`Null` bên trong nó) - `InvalidRva` xảy ra khi RVA lookup chạy trước lúc
game giải nén/relocate xong (Arxan), tức là 1 cuộc đua timing với lúc
thread của DLL này khởi động, không liên quan gì đến việc DLL đặt ở đâu
hay máy người dùng yếu/mạnh.

Cùng lỗi [`SomeTweaks`](../sometweaks) đã gặp và fix trước đó (xem README
của nó, mục 2026-08-24) - port nguyên `wait_for_cs_task()` (retry sau 1s
thay vì bỏ cuộc ngay) từ `src/task.rs` của SomeTweaks vào thẳng
`src/regen.rs` (AutoRegen chỉ có 1 chỗ gọi `CSTaskImp::wait_for_instance`
nên không cần tách module riêng như SomeTweaks - crate đó có nhiều feature
module cùng cần dùng lại).

## Pin fromsoftware-rs 0.14.0, thêm wait_for_system_init (2026-08-26)

Áp dụng lại 2 trong 3 cải tiến ổn định đã làm cho `sometweaks` (xem README
của nó cùng ngày) sang crate này - `Regen.PerHit` (panic-safety qua
`run_recurring_safe`) đã có sẵn ở dạng khác từ trước (SEH quanh
`AttackHook`), còn 2 việc dưới đây trước đó chưa có:

- **Version pin**: `eldenring`/`fromsoftware-shared` giờ khai báo 1 lần ở
  workspace `Cargo.toml` gốc, pin về bản crates.io `0.14.0` thay vì tracking
  git HEAD - áp dụng chung cho toàn bộ workspace, không cần sửa gì riêng ở
  crate này.
- **`wait_for_system_init`**: `wait_for_cs_task()` trong `src/regen.rs` giờ
  gọi `wait_for_system_init_until_ready()` (chờ `CSWindow` hInstance - tín
  hiệu "process game còn sống" sớm nhất, ngay sau CRT init) **trước** vòng
  lặp retry `CSTaskImp::wait_for_instance` đã có sẵn (fix `InvalidRva` ngày
  2026-08-26 phía trên) - 2 bước riêng biệt, không thể thay thế cho nhau (xác
  nhận qua đọc `.docs/UltimatePassiveRegeneration` - vẫn giữ cả 2 bước tuần
  tự).
- **Panic safety cho tick chính**: thêm `run_recurring_safe()` bọc quanh
  `cs_task.run_recurring(...)` trong `regen.rs` - bắt panic mỗi frame, log
  rồi bỏ qua thay vì crash cả game. Cần workspace `Cargo.toml` bỏ
  `panic = "abort"` ở `[profile.release]` (mặc định về `"unwind"`) thì
  `catch_unwind` mới có tác dụng trong bản release - đã sửa ở mức workspace.

Không đụng đến ini/hành vi gameplay, chỉ cải thiện độ ổn định lúc khởi động
và chống crash.

## Fix đâm lén/đâm chí mạng mất animation khi bật `Regen.PerHit` (2026-09-03)

Báo lỗi từ Nexus + tự test lại: bật `Regen.PerHit.Enabled=true` rồi đâm sau
lưng địch → đòn chỉ ra như 1 nhát chém thường, không animation "đâm lén",
tắt `Regen.PerHit.Enabled` trong ini **không** hết lỗi (vì hook vẫn còn cài
trong bộ nhớ - `Enabled` chỉ gate phần heal, không gỡ patch), phải thoát
game vào lại mới hết. Riposte sau khi parry thì luôn hoạt động bình thường
- chỉ backstab bị.

**Cách chẩn đoán**: thêm tạm log `atkCategory`/`atkId`/`sourceType` mỗi đòn
trúng (đọc `HITINFO_ATK_PARAM_CATEGORY_OFFSET`/`_ID_OFFSET`, offset cũ đã bỏ
lúc port sang thiết kế `SomeTweaks` - thêm lại). Log cho thấy đòn đâm lén bị
lỗi bắn ra **2 dòng "dealt X damage" liên tiếp** (1 dòng 0 damage rồi 1 dòng
damage thật) thay vì 1 dòng dealt + 1 dòng heal như riposte bình thường -
dấu hiệu chuỗi animation script bị hủy giữa chừng, rơi về xử lý như đòn
thường.

Test cô lập xác nhận: chỉ cần **hook được cài** (`Regen.PerHit.Enabled=true`
tại thời điểm vào game) là đủ gây lỗi - không liên quan tốc độ/ghi log
(đã thử dời hết việc ghi `RegenLog` ra khỏi hook sang 1 hàng đợi, xả ở tick
riêng mỗi frame thay vì ghi file đồng bộ giữa lúc xử lý va chạm - không ăn
thua, chứng minh nguyên nhân không phải do độ trễ).

**Nguyên nhân thật** (đọc lại `.docs/reverse_engineering/AttackFn_decompiled.txt`
- bản decompile cũ của đúng hàm đang hook): hàm xử lý va chạm nhận **5 tham
số**, không phải 4 như hook vẫn tưởng - tham số thứ 5 (1 byte) được truyền
qua **stack** tại `[rsp+0x20]`, không qua register. Code gốc (đoạn ngay
trước call site, không bị patch) tính giá trị này từ `hitInfo+0xd9==2` rồi
ghi vào `[rsp+0x20]` trước khi gọi; bên trong hàm đích, giá trị đó quyết
định **có chạy khối xác định đòn chí mạng/đâm lén hay bỏ qua**
(`CMP byte ptr [RSP+0xc0],SIL; JNZ <bỏ qua>` - offset lệch do 2 hàm có
size frame khác nhau, cùng trỏ về 1 chỗ). Trampoline của mình chỉ forward
đúng 4 tham số qua `rcx/rdx/r8/r9`, rồi tự `sub rsp,0x20` tạo shadow space
**mới** trước khi gọi - `[rsp+0x20]` lúc đó là rác trong shadow space của
chính mình, không phải giá trị game đã tính.

**Fix**: vì patch bằng `jmp` (không phải `call`), `rsp` lúc mới vào
trampoline vẫn y hệt lúc code gốc (chưa bị patch) vừa ghi giá trị đó - chỉ
cần đọc `byte ptr [rsp+0x20]` vào `r10b` (register volatile, không cần lưu)
làm **lệnh đầu tiên trong trampoline, trước khi đụng gì vào rsp**, rồi ghi
lại đúng offset đó sau khi tạo shadow space riêng, trước khi gọi hàm thật.
Không cần sửa gì ở phía Rust (`on_attack_observed` không cần tham số thứ 5
này) - chỉ sửa đúng đoạn `global_asm!` trong `src/attack_hook.rs`.

Cùng 1 trampoline y hệt được dùng ở [`sometweaks`](../sometweaks) (port từ
đây) - đã áp dụng fix tương tự bên đó luôn, không cần chờ báo lỗi riêng.

## Lịch sử dịch ngược (bản C++ gốc, không còn khớp code hiện tại)

Mod ban đầu viết lại từ việc dịch ngược `AutoRecovery.dll` (một mod có sẵn,
tên project gốc là "AshesEverywhere" theo PDB path còn sót lại trong file).
Giữ lại phần dưới đây vì offset/AOB/điểm hook vẫn còn giá trị tham khảo, dù
implementation đã chuyển hẳn sang Rust ở trên.

### Những gì đã dịch ngược được

1. **AOB pattern** để tìm 1 hàm "getter" trong `eldenring.exe`:

   ```
   48 8B 05 ?? ?? ?? ?? 48 85 C0 74 0F 48 39 88
   ```

   Disassembly: `mov rax,[rip+disp32]; test rax,rax; jz +0xF; cmp [rax+0x88],ecx`
   → khác với AOB dùng trong `PassiveRunes` (trỏ tới `GameDataMan`) — pattern
   này trỏ tới **`WorldChrMan`** (singleton quản lý các nhân vật đang tồn tại
   trong world, không phải dữ liệu save-file).

2. **Chuỗi con trỏ** (định dạng "pointer path" kiểu Cheat Engine, đọc từ
   string cứng trong file): `0x0, 0x10EF8, 0x0, 0x190, 0x0, 0x0`
   — cộng offset rồi dereference lần lượt, riêng offset cuối chỉ cộng không
   dereference. Xem giải thích chi tiết từng bước trong lịch sử phân tích
   (đã trao đổi qua chat, không lặp lại ở đây).

3. **Offset HP/FP** trong struct cuối cùng:
   - HP hiện tại: `+0x138`, HP tối đa: `+0x13C`
   - FP hiện tại: `+0x148`, FP tối đa: `+0x14C`

4. **Định dạng config gốc** (`config.ini` cạnh DLL, mod gốc **không kèm sẵn
   file này** — tự tạo mới hoạt động được):

   ```ini
   [recovery]
   hp=...       ; tỉ lệ 0.0-1.0
   fp=...       ; tỉ lệ 0.0-1.0
   interval=... ; mili-giây
   ```

   Mod gốc: nếu thiếu file/section/key, giá trị mặc định về 0 → không hồi gì
   cả (không crash), nhưng `interval=0` khiến vòng lặp gọi `Sleep(0)` liên
   tục → có thể tốn CPU vô ích.

5. Có 1 hàm hiện `MessageBoxA` báo lỗi to nếu AOB pattern không tìm thấy
   (khác với `PassiveRunes` chỉ âm thầm ghi log).

### Những gì đã viết lại (`AutoRegen`, bản C++)

- Kiến trúc code theo đúng style `PassiveRunes`/`SeamlessWrap`: `MemoryScan`
  (bổ sung `WalkPointerChain` mới để tái hiện đúng cơ chế pointer-path ở
  trên), `Config`, `Logger`, `RegenEngine`.
- **Đổi tên key config** cho dễ hiểu: `interval` → `Interval`, `hp`/`fp` →
  `Hp`/`Fp`.
- **Đổi đơn vị HP/FP**: nhập trực tiếp theo % (`1` = 1%, `100` = 100%) thay
  vì tỉ lệ thập phân 0.0-1.0 như bản gốc.
- **Log luôn bật, ghi đè mỗi lần chạy** (`std::ios::trunc`) — không cần
  bật/tắt qua config, không cộng dồn log qua nhiều lần chơi.
- **Có clamp giá trị hồi không vượt quá HP/FP tối đa** — cải tiến nhỏ so với
  bản gốc (bản gốc không clamp).

### Trạng thái bản C++ (lịch sử)

| Việc                                | Trạng thái                                                                  |
| ----------------------------------- | --------------------------------------------------------------------------- |
| Dịch ngược AOB/offset/pointer chain | Xong                                                                        |
| Viết lại logic C++                  | Xong, build thành công (Release\|x64)                                       |
| Test thực tế trong game             | **Chưa** — cần inject thử và xác nhận offset còn đúng với bản game hiện tại |
| Cấu hình `.ini`                     | Xong (`AutoRegen.ini` mẫu đã có sẵn trong project)                          |

### Lưu ý khi test (bản C++)

- Pattern/offset dịch ngược từ 1 bản game cụ thể (file gốc build ngày
  2024-07-10) — nếu game đã vá sau thời điểm đó, offset ban đầu có thể lệch.
- Nếu không hoạt động: mở `AutoRegen.log` (luôn được tạo cạnh DLL) xem có
  dòng `"ERROR: pointer chain hit a null pointer"` không — đó là dấu hiệu
  cần dò lại offset bằng Cheat Engine.

### Cập nhật offset (2026-08-04, dò lại bằng Cheat Engine pointer scan)

Bản game hiện tại không còn khớp với chain reverse-engineer ban đầu từ
`AutoRecovery.dll`. Dò lại bằng Cheat Engine (value scan → pointer scan →
lọc qua nhiều lần restart game) ra được 1 chain khác, **ngắn hơn** và không
cần AOB pattern:

- Base tĩnh: `eldenring.exe+0x3B12E30`
- Pointer chain: `0x0, 0xAE8` (dereference 1 lần, cộng `0xAE8` — không
  dereference bước cuối) → trỏ thẳng tới HP hiện tại, không cần đi qua
  `WorldChrMan` như bản cũ.
- Offset trong struct (tính từ HP hiện tại): HP Max `+0x4`, FP hiện tại
  `+0x10`, FP Max `+0x14` — layout tương đối giữa các field giống bản cũ
  (current/max liền kề), chỉ vị trí struct trong bộ nhớ đổi.

Vì offset tĩnh (`0x3B12E30`) là số cố định tính từ đầu module, khác với AOB
pattern (dò theo byte code), chain này **dễ lệch hơn** nếu bản game thay đổi
kích thước binary ở bất kỳ đâu trước offset đó. Nếu hỏng sau 1 lần update
game, dò lại từ đầu bằng đúng quy trình Cheat Engine (value scan cho địa chỉ
HP → pointer scan → restart game nhiều lần để lọc → chọn chain ngắn nhất,
ưu tiên base dạng `eldenring.exe+offset` chứ không phải `THREADSTACK`).

### Cập nhật offset Stamina + fix offset FP Max (2026-08-04)

Đã quay lại dùng AOB pattern (`CHR_PATTERN` trong `RegenEngine.cpp`) trỏ tới
struct nhân vật. Offset trong struct đó được đối chiếu với 1 DLL cheat của
bên thứ ba (`reverse_engineering/status full dll.dll`, decompile bằng
Ghidra headless — xem `DumpAll.java`/`DumpDisasm.java`) đọc/viết đúng struct
này, nên độ tin cậy cao hơn suy đoán qua Cheat Engine:

- HP Current `0x138`, HP Max `0x13C` — khớp với suy đoán ban đầu.
- FP Current `0x148`, **FP Max `0x150`** — bản cũ ghi nhầm là `0x14C`, đã sửa.
- **Stamina Current `0x154`, Stamina Max `0x15C`** — offset mới, thêm vào
  `RegenEngine.cpp` cùng 2 key config `Stamina`/`StaminaPercent`.

Cách xác nhận: dump raw disassembly của hàm ghi các field này (địa chỉ
`0x1800018b0` trong DLL nói trên) và đọc trực tiếp offset byte trong lệnh
`LEA`/`ADD` — không dựa vào output C decompile vì Ghidra có thể tính sai
kiểu scale con trỏ (`undefined8*` bị nhân 8 nhầm chỗ).

### Hồi máu khi chính player đánh trúng địch (`HpOnHit`/`FpOnHit` + `AttackHook`, 2026-08-07)

Bản đầu tiên của tính năng này dùng heuristic poll theo tick: mỗi tick, so
HP hiện tại của từng địch trong danh sách nhân vật world với HP tick trước,
HP giảm thì tính là "vừa bị đánh". **Cách này sai** — không phân biệt được
đòn đánh của player với sát thương chảy máu/độc theo thời gian, ngã, hay
(co-op) đòn của đồng đội, vì nó chỉ nhìn thấy "HP địch vừa giảm", bất kể lý
do gì. Đã bỏ hoàn toàn cách này, thay bằng hook thẳng vào code game.

**Điểm hook**: tìm được từ 1 CE table công khai
(`eldenring_all-in-one_Hexinton-v6.1_ce7.5.ct`), tái sử dụng 2 AOB pattern
mà cheat **"NoHitbox+ReflectAttack"** của table đó dùng để chặn đúng lệnh
`call` xử lý va chạm đòn đánh — gọi đúng 1 lần mỗi khi có hit thật sự được
tính toán, không chạy cho sát thương DOT:

```
OnAttack: 45 0F B6 CE 4C 8B C3 48 8B D6 48 8B CF E8
```

(AOB `WorldChrManFinder` đi kèm trong CE table không dùng — bị lệch version
với bản game hiện tại, xem mục bên dưới; dùng lại `CHR_PATTERN` sẵn có
trong `RegenEngine.cpp` để lấy `WorldChrMan`, cùng 1 singleton, chỉ khác
code site tìm ra nó.)

**Cơ chế** (`AttackHook.h/.cpp` + `AttackTrampoline.asm`):

- Patch 12 byte đầu của `OnAttack` (4 lệnh `mov`/`movzx` chuẩn bị tham số
  cho lệnh `call` thật) thành `mov rax, <trampoline>; jmp rax`, **giữ
  nguyên hoàn toàn lệnh `call` gốc** — trampoline dựng lại đúng 4 lệnh bị
  ghi đè trước khi nhảy tới đó, nên hàm gốc chạy y hệt bình thường.
- Trampoline (viết tay bằng MASM vì MSVC x64 không hỗ trợ inline asm) gọi 1
  hàm C++ (`OnAttackObserved`) với đúng 4 tham số hàm gốc nhận được, rồi
  tiếp tục luồng gốc.
- `OnAttackObserved` so `rsi` (tham số thứ 2) với con trỏ player
  (`WorldChrMan+0x1E508`, qua `RegenEngine::GetWorldChrMan()`) — nếu khớp,
  gọi `RegenEngine::HealPlayerHp()`/`HealPlayerFp()` với giá trị đọc từ
  `HpOnHit`/`FpOnHit` trong ini.

**Xác định `rsi` = attacker bằng cách nào**: chạy bản chỉ-ghi-log trước
(không hồi máu), rồi đối chiếu 2 tình huống thực tế đối xứng nhau:

- Địch đánh trúng player: `[ctx+8]==player`, `rsi`=địch.
- Player đánh trúng đúng con địch đó (nhiều nhát liên tiếp): `[ctx+8]`=địch
  (đúng bằng giá trị `rsi` ở tình huống trên), `rsi==player`.

Vai trò đảo ngược đối xứng hoàn hảo giữa 2 tình huống → `rsi` = bên gây sát
thương (attacker), `[ctx+8]` = bên nhận (target). Khớp với cách CE script
gốc dùng `[ctx+8]==player` để phát hiện "player đang bị đánh" (dùng cho
reflect). `rbx` (tham số thứ 3) giữ nguyên giá trị ở cả 2 tình huống — một
con trỏ hệ thống không liên quan attacker/target, không dùng tới.

### Sự cố khi test (2026-08-07) — đã tìm ra và sửa

**Lần test đầu**: lỗi `WorldChrManFinder pattern not found` trong log —
pattern độc lập mà CE table dùng để tìm `WorldChrMan` bị lệch version với
bản game hiện tại (trong khi `OnAttack`, dài/đặc thù hơn, vẫn khớp ngay —
xác nhận **không phải do timing/game chưa load** như nghi ngờ ban đầu). Đã
bỏ pattern đó, dùng lại `CHR_PATTERN` đã xác nhận hoạt động.

**Lần test thứ hai**: sau khi sửa lỗi trên, **game văng ngay khi có kẻ địch
tấn công** (đúng lúc hook kích hoạt). Nguyên nhân (lỗi trong chính đoạn asm
viết tay, không phải do đoán sai attacker/target): trampoline align RSP về
bội số 16 rồi `push` thêm 1 giá trị để nhớ số byte đã align — chính `push`
đó làm lệch lại 8 byte, khiến `call OnAttackObserved` chạy với RSP sai
chuẩn Win64 ABI. Hàm C++ có biến cục bộ (`char buf[...]`) khiến trình biên
dịch dùng lệnh SSE (`movaps`, yêu cầu địa chỉ chia hết 16) → lệch alignment
→ crash ngay lập tức. **Đã sửa**: bỏ cách cộng/trừ RSP thủ công dễ sai,
thay bằng lưu "điểm mốc" RSP gốc vào thanh ghi `r12` (non-volatile, tự
lưu/khôi phục), phục hồi bằng `mov rsp, r12` — không còn phép tính nào có
thể sai. Nhân tiện bỏ luôn việc lưu/khôi phục các thanh ghi volatile quanh
`call` của hook (không cần thiết: lệnh `call` thật vốn dĩ đã luôn ghi đè
các thanh ghi này theo chuẩn Win64 ABI).

Sau bản vá thứ hai: test lại không crash, log cho dữ liệu rõ ràng, xác
nhận đúng `rsi`=attacker như trên → đã chuyển từ chế độ log sang hồi máu
thật.

**Còn giữ lại 1 lớp bảo vệ SEH** (`__try`/`__except` trong `OnAttackObserved`,
xem code) đề phòng `WorldChrMan+0x1E508` đọc null/rác trong khoảnh khắc
transition (loading màn/teleport) — nếu xảy ra, chỉ bỏ qua 1 lần hồi máu
thay vì crash. Lưu ý kỹ thuật khi sửa code này: hàm chứa `__try` **không
được** gọi trực tiếp `Logger::Instance().Log("literal")` (tạo `std::string`
tạm có destructor ngay trong khối `__try`/`__except` → MSVC báo lỗi biên
dịch C2712) — phải tách lời gọi log ra hàm `__declspec(noinline)` riêng, và
`AttackHook.cpp` phải build với `/EHa` thay vì `/EHsc` mặc định của
project (xem override trong `AutoRegen.vcxproj`).

### Xung đột với Seamless Co-op (2026-08-07) — đã sửa

Khi bật cả `AutoRegen` và Seamless Co-op (`ersc.dll`) cùng lúc: game crash
ngay khi vào, báo lỗi từ chính Seamless Co-op — `"No such pattern" "45 0F
B6 CE 4C 8B C3 48 8B D6 48 8B CF E8"` (`ersc\signatures\signatures.cpp`).

Nguyên nhân: **Seamless Co-op dùng đúng cùng 1 AOB pattern** (`OnAttack` ở
trên) để tự hook điểm xử lý đòn đánh này cho netcode co-op của nó.
`AttackHook::Install()` trước đây chạy ngay khi DLL vừa load (trước cả khi
vào game) — nếu chạy trước khi Seamless Co-op kịp tự quét pattern gốc để
cài hook riêng, AutoRegen ghi đè mất 12 byte đầu của pattern đó, khiến
Seamless Co-op không tìm thấy nữa → nó tự abort toàn bộ game (fatal error
theo thiết kế của nó, không phải crash không kiểm soát).

**Đã sửa**: dời `AttackHook::Install()` từ lúc DLL vừa load sang **lần đầu
tiên nhân vật resolve thành công trong vòng lặp tick** (`RegenEngine::Run()`)
— tức là chắc chắn đã thực sự vào game, nghĩa là mọi mod khác (bao gồm
Seamless Co-op) đã load và chạy xong bước quét pattern khởi tạo của chúng
từ lâu, không còn có thể bị AutoRegen giành trước nữa. Nếu vì lý do nào đó
pattern vẫn không tìm thấy lúc AutoRegen cài hook (ví dụ mod khác đã hook
đúng chỗ đó trước), `AttackHook::Install()` chỉ ghi log lỗi và tắt tính
năng hồi máu-khi-đánh-trúng, không ảnh hưởng đến các tính năng hồi
HP/FP/Stamina theo tick khác của AutoRegen — và quan trọng nhất, không còn
làm crash mod khác nữa.

### Cách dùng (bản C++, tên key cũ - xem mục "Đổi ini key sang cấu hình kiểu
`SomeTweaks`" ở trên cho tên key/cấu trúc hiện tại)

Trong `AutoRegen.ini`, đặt `HpOnHit`/`FpOnHit` (số HP/FP cố định) và/hoặc
`HpPercentOnHit`/`FpPercentOnHit` (phần trăm HP/FP tối đa, cùng quy ước với
`HpPercent`/`FpPercent`) theo ý muốn — cùng quy ước "0 = tắt" như mọi key
khác trong file, không cần bật thêm cờ riêng: hook chỉ được cài nếu 1 trong
4 giá trị này khác 0 (xem `RegenEngine::Run()`). Config được đọc 1 lần lúc
cài hook, không tra lại mỗi lần đánh trúng. Mỗi lần player đánh trúng địch,
`AutoRegen.log` ghi 1 dòng `AttackHook: player hit landed -> +N HP, +N FP`
(số thực tế đã hồi, sau khi cộng dồn cố định + phần trăm và giới hạn theo
tối đa).

### Chỉ hồi máu khi đánh bằng vũ khí, không tính phép (2026-08-11)

`HpOnHit`/`FpOnHit` ban đầu hồi máu cho **mọi** đòn trúng của player, kể cả
cast phép. Ba cách phân loại "vũ khí vs phép" dựa trên `AtkParam` đã thử và
bỏ (chi tiết xem git history + `reverse_engineering/`):

1. So `atkParamId` (đọc từ `hitInfo+0x40`) với ngưỡng 100.000 (vũ khí luôn
   theo công thức `WeaponType*100000+AtkId`) — sai với Glintstone Nail (một
   sorcery nhưng đâm cận chiến, ID vượt ngưỡng) và với dash/jump/backstab
   (dùng chung 1 ID nhỏ cho mọi vũ khí).
2. Gọi thẳng hàm tra cứu AtkParam của game để đọc field `atkPhys`/`atkMag`
   thật — sai vì nhiều vũ khí lấy sát thương qua SpEffect liên kết thay vì
   lưu trực tiếp trong AtkParam (field luôn = 0), và vũ khí yểm tố (Magic/
   Blood/Fire) dùng chung đúng 1 dòng AtkParam với bản không yểm tố (chuyển
   đổi loại sát thương nằm ở `EquipParamWeapon`/`AttackElementCorrectParam`,
   không phải AtkParam).
3. Theo dõi Stamina của player có vừa giảm ngay trước lúc đòn trúng không
   (vũ khí luôn tốn Stamina, phép thường không) — sai với nhóm "Bestial
   Incantations" (một số phép Faith học từ Gurranq cũng tốn Stamina).

**Cách cuối cùng đang dùng**: đọc con trỏ "source object" tại
`hitInfo+0x1D8`, gọi virtual function tại `vtable+0x10` của nó (đúng pattern
game tự dùng ở nơi khác, thấy được qua decompile) để lấy `sourceType` —
`1` = đòn trực tiếp/cận chiến, `3` = đạn/projectile (đa số phép). Chỉ hồi
máu khi `sourceType==1`. Hạn chế còn biết: phép có động tác cận chiến như
Carian Greatsword đọc ra `sourceType==1` nên vẫn được hồi — chấp nhận được.

Đặt `DebugLog=1` trong ini để log thêm `atkCategory`/`atkId`/`sourceType`/...
cho mỗi đòn trúng của player, dùng khi cần kiểm tra 1 vũ khí/phép cụ thể bị
phân loại sai.

### [ĐÃ LÀM, xem mục đầu README] Ghi chú cũ: từng nghĩ chỉ nửa mod đáng chuyển sang Rust

Mục này giữ lại làm lịch sử quyết định — kết luận bên dưới **đã bị đảo
ngược**, xem "Bản Rust hiện tại" ở đầu file.

Kết luận cũ: `AttackHook` (hồi máu khi đánh trúng) không có lợi khi chuyển
sang Rust, vì không có struct/API nào trong `fromsoftware-rs` phản chiếu
được call site `OnAttack` — lý luận là Rust `unsafe` vẫn phải tự viết đúng
byte shellcode y hệt C++/MASM, không tiết kiệm được gì, có khi phức tạp hơn
vì phải tự lo FFI/calling convention mà 1 file `.asm` (MASM) xử lý tự nhiên
hơn.

Lý do bị đảo ngược: `std::arch::global_asm!` cho phép nhúng thẳng đoạn ASM
trampoline (cú pháp gần như y hệt MASM, chỉ khác dialect) ngay trong file
Rust, và `#[unsafe(no_mangle)] static AtomicUsize` thay thế gọn cho biến
toàn cục C++ kiểu `g_ReturnAddr`/`g_TargetAddr` — không cần file `.asm` +
build-customization riêng (`masm.targets`) như bản C++, không cần lo thêm
FFI phức tạp nào cả. Toàn bộ kỹ thuật patch 12-byte + trampoline + phân loại
nguồn sát thương qua vtable vẫn giữ nguyên logic, chỉ đổi ngôn ngữ viết. Chi
tiết xem `src/regen/attack_hook.rs`.
