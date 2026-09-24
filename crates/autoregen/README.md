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

Xem `src/regen.rs` và `src/hit_hook.rs` (thay `attack_hook.rs` cũ, xem mục
"Viết lại `Regen Per Hit` từ đầu bằng Ghidra" bên dưới) cho code hiện tại;
`AutoRegen.ini` cho toàn bộ key cấu hình.

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

## Thêm `Regen.PerHit.ExcludeAow` - loại trừ Weapon Art/Ash of War (2026-09-03)

Feature request từ Nexus: muốn dùng phép/skill rẻ để hồi FP cho phép/skill
đắt hơn, nên cần 1 tùy chọn loại trừ đòn Weapon Art khỏi Regen Per Hit. Thử
2 hướng trước khi ra được bản dùng được:

1. **Ngưỡng `atkId`** (thử đầu tiên, `atkId >= 700 triệu`): có vẻ đúng qua
   vài mẫu test ban đầu, nhưng khi user tự export toàn bộ `AtkParam` từ
   SmithBox ra `.docs/AtkParam_Pc.csv` (~11000 dòng) rồi đối chiếu, phát
   hiện **không có ngưỡng số nào đúng cho mọi vũ khí** - vd đòn đánh nặng
   *thường* (không phải AoW) của Beast Claw lại nằm chung dải ID với nhiều
   Ash of War khác (`Beast Claw - Default: AttackBothHeavySpecial1End`,
   atkId ~303 triệu). Viết script so sánh **toàn bộ ~200 field** của
   `AtkParam` giữa nhóm tên `[AOW]` và nhóm tên `Default` (dựa theo tên
   trong CSV) - field "tốt nhất" (`finalDamageRateId`) vẫn overlap tới
   44.6%. Kết luận: `AtkParam` (tham số tính sát thương của cú đánh) không
   mang đủ thông tin để phân biệt AoW - đây là thuộc tính của
   *animation/action đang chạy*, không phải của riêng phép tính damage.
2. **Input pad thật** (hướng dùng): `fromsoftware-rs` phản chiếu
   `CSChrActionRequestModule` trên `ChrIns` - đọc trực tiếp bit nút bấm của
   player (`action_requests`/`new_action_presses`, gồm cả `r1`/`r2`/`l1`/
   `l2`). Trong Elden Ring, Weapon Art luôn kích hoạt bằng `L2` (bất kể tay
   nào đang chủ động) - test thật xác nhận `l2=true` đúng lúc các đòn Ash of
   War trúng.

**Vấn đề khi test thật**: chỉ đọc trạng thái `l2` **tại đúng lúc** đòn trúng
thì bị bỏ sót - 1 Ash of War nhiều nhịp, nút `L2` đã buông ra từ lâu (chỉ
giữ trong khoảnh khắc bấm) nhưng animation/damage vẫn còn tiếp diễn vài
giây sau, lúc đó `l2` đã về `false`. **Fix**: không đọc trạng thái tức
thời, mà "chốt" (latch) theo **lần bấm mới nhất** - mỗi frame kiểm tra
`new_action_presses` (bit chỉ bật đúng 1 frame lúc vừa bấm): nếu `R1`/`R2`/
`L1` vừa bấm → chốt "đòn thường"; nếu `L2` vừa bấm → chốt "Weapon Art"; giữ
nguyên trạng thái chốt cho tới lần bấm tiếp theo. Nhờ vậy mọi đòn trúng
trong cùng 1 chuỗi Weapon Art đều được tính đúng, kể cả đòn đến muộn.

`L1` cố tình gộp chung nhóm "đòn thường" (không tách riêng) vì trong Elden
Ring `L1` chỉ dùng cho đánh nhẹ tay trái hoặc đỡ đòn/khiên, không bao giờ
là nút kích hoạt Weapon Art - không phải thiếu sót, chỉ là gộp đúng.

Implementation: `regen.rs` thêm `LAST_ATTACK_WAS_SKILL` (atomic, cập nhật
mỗi frame qua `update_last_attack_input()`) + `is_last_attack_skill()`;
`attack_hook.rs` dùng hàm đó thay cho ngưỡng `atkId` cũ (đã gỡ bỏ hoàn
toàn, kể cả field `atkCategory` trong `RegenLog` - xác nhận qua CSV luôn
bằng 1, vô dụng).

Nhân tiện đổi key `Regen.PerTick.Stamina`/`Regen.PerHit.Stamina` →
`Regen.PerTick.SP`/`Regen.PerHit.SP` cho đồng bộ 2 chữ cái với `HP`/`FP`
(không breaking gì thêm ngoài phạm vi đã breaking sẵn từ đợt đổi sang
`Regen.*` - key cũ đã nằm trong diện phải sửa tay theo README, xem mục
"Đổi ini key sang cấu hình kiểu SomeTweaks" phía trên).

## Thêm `Regen.PerTick.Trigger=3` (Idle) và `=4` (Sitting) (2026-09-10)

Feature request từ Kolagon trên Nexus: muốn thêm điều kiện "chỉ hồi khi đứng
yên không làm gì" (idle) và "chỉ hồi khi đang ngồi qua gesture" (sitting: cầu
nguyện, tuyệt vọng, ngồi khoanh chân, ngủ gật khoanh chân, nghỉ ngơi, ngồi
nghiêng, ủ rũ, cuộn tròn, nằm dang, tư thế ngồi xổm Patches). Trước khi làm,
kiểm tra `fromsoftware-rs` xem có field "đang gesture gì"/animation ID nào
lộ sẵn không - không có (`CSChrTimeActModule.anim_queue` có animation TAE
đang chạy thật, nhưng TAE id không trùng số với GESTURE_ID nên vẫn cần 1
bảng map riêng chưa dò được, không dùng đường này). Tra ra bảng tên↔ID chính
xác (`GESTURE_ID` dropdown) từ CE table công khai của The Grand Archives
(github.com/The-Grand-Archives/Elden-Ring-CT-TGA) - không mod Nexus nào có
sẵn tính năng tương tự để tham khảo code.

**Idle** (`Trigger=3`): đọc thẳng `CSChrActionRequestModule` - không di
chuyển (`movement_request_flags.raw_input`) và không giữ bất kỳ action nào
(`action_requests`: r1/r2/l1/l2/sp_move/jump/use_item/action/guard/rideon/
rideoff/ladderup/ladderdown) - không cần thêm state gì, đọc trực tiếp mỗi
frame.

**Sitting** (`Trigger=4`): dùng lại đúng kỹ thuật latch của
`LAST_ATTACK_WAS_SKILL` (mục "Regen.PerHit.ExcludeAow" phía dưới) vì
`requested_gesture` (Param ID gesture) chỉ có giá trị đúng **1 frame** lúc
`new_action_presses.gesture()` bắn (không phải "đang ngồi suốt animation").
Chốt `IS_SITTING=true` nếu `requested_gesture` khớp 1 trong 10 ID ngồi lúc
gesture mới được bấm, huỷ chốt ngay khi có input "busy" khác (di chuyển/tấn
công/dùng đồ/đỡ đòn/cưỡi ngựa) - không cần tín hiệu "gesture đã kết thúc"
riêng vì mọi input đó đều tự huỷ animation ngồi trong game rồi.

Cả 2 không cần cài `AttackHook` (khác `Trigger=1`/`2` dựa vào
`mark_combat_activity()`) - `update_last_attack_input()` vốn đã chạy mỗi
frame độc lập với `Regen.PerHit` từ trước.

**Đổi tiếp**: ban đầu định hard-code luôn 10 ID vào Rust (`const
SIT_GESTURE_IDS`), nhưng theo góp ý của người dùng, đổi thành đọc từ ini
(`[General] Gesture.SittingId`, danh sách ID cách nhau bởi dấu phẩy, giá trị
mặc định giữ nguyên 10 ID cũ) - lý do: nếu sau này game cập nhật thêm
gesture ngồi mới mà README/code này chưa kịp cập nhật, hoặc người dùng muốn
bớt/thêm gesture theo ý riêng (VD không coi "Balled Up" là ngồi), họ tự sửa
ini được ngay, không cần chờ bản DLL mới. Parse lại từ config mỗi lần có
gesture mới được bấm (`new_action_presses.gesture()`), không phải mỗi frame,
nên không lo chi phí parse string lặp lại.

## Thêm đệm 5s cho Idle, thêm debug log cho Idle/Sitting - đang điều tra bug Sitting không hồi (2026-09-10)

Test thật: `Trigger=3` (Idle) hồi ngay lập tức không có độ trễ nào - theo
yêu cầu người dùng, thêm `IDLE_GRACE_MS=5000` (hằng số, chưa expose ra ini)
- phải đứng yên liên tục ít nhất 5 giây (tính từ lúc `busy` cuối cùng về
false, xem `IDLE_SINCE_MS`) `is_idle()` mới trả `true`. Cùng bug report:
**`Trigger=4` (Sitting) không hồi máu gì cả** khi bấm các gesture ngồi -
chưa xác định được nguyên nhân (nghi ngờ hàng đầu: giá trị thật của
`requested_gesture` không khớp bảng `GESTURE_ID` tra được từ CE table, vì
bảng đó không có gì đảm bảo đúng 1-1 với field `requested_gesture` mà
`fromsoftware-rs` expose - suy luận thuần túy, chưa verify bằng dữ liệu
thật). Vì không có môi trường game để tự test, chưa thể sửa mù - thêm 2
dòng debug log (gate bởi `RegenLog=true` có sẵn):

- `Gesture: requested_gesture=<n> -> is_sitting=<bool>` mỗi lần bấm gesture
  mới - cho biết giá trị `requested_gesture` thật nhận được lúc bấm từng
  gesture ngồi, để đối chiếu với 10 ID đang giả định trong
  `Gesture.SittingId`.
- `Regen.PerTick: trigger=<n> -> condition_met=<bool> (in_combat=.. idle=..
  sitting=..)` mỗi tick khi `Trigger != 0` - cho biết tick có thực sự thấy
  `is_sitting()=true` hay không.

Bước tiếp theo: người dùng bật `RegenLog=true`, vào game bấm lần lượt các
gesture ngồi, đối chiếu `AutoRegen.log` xem `requested_gesture` thật là bao
nhiêu - nếu khác hẳn dải `GESTURE_ID` (160-202) thì bảng ID tra được sai
nguồn, cần dò lại.

**Đã tìm ra (2026-09-10, cùng ngày)**: log thật từ người dùng cho thấy
`requested_gesture` đúng bằng **1 nửa** giá trị `GESTURE_ID` tra được từ CE
table - Dejection (bảng ghi 160) đọc ra `80`, Rest (184) đọc ra `92`, Sitting
Sideways (186) đọc ra `93` - khớp chính xác cả 3 mẫu test thật. Không tài
liệu công khai nào ghi rõ hệ số `/2` này (khác hẳn kiểu sai do lệch version
game như các bug offset trước đây) - chỉ suy ra được từ dữ liệu log thật,
không phải đọc tài liệu. Đã sửa `DEFAULT_SIT_GESTURE_IDS`/`Gesture.SittingId`
thành `80,90,91,92,93,94,95,97,100,101` (chia đôi cả 10 giá trị cũ). Đệm 5s
của Idle (`IDLE_GRACE_MS`) test thật cũng đúng như thiết kế - `idle` chuyển
`false→true` đúng ~5 giây sau lần cuối `busy=true`, xem log
`Regen.PerTick`.

**Bug tiếp theo, phát hiện ngay sau khi sitting hoạt động**: đang ngồi, bấm
lại gesture (cùng gesture hoặc gesture khác) → player đứng dậy thật trong
game (hành vi gốc: bấm gesture lần 2 luôn huỷ/đứng dậy trước, không tự
chuyển thẳng sang gesture mới), nhưng code vẫn đọc `requested_gesture` y hệt
lần 1 nên vẫn set lại `IS_SITTING=true` - hồi máu tiếp dù người chơi đã đứng
dậy. Không có cách phân biệt "bắt đầu" với "huỷ" chỉ từ tín hiệu
`new_action_presses.gesture()` + `requested_gesture` (2 lần bắn ra giống hệt
nhau) - phải dựa ngữ cảnh: nếu đang `IS_SITTING=true` thì lần bấm gesture kế
tiếp (bất kể ID gì) chắc chắn là huỷ. Fix: `is_sit_gesture` giờ luôn `false`
nếu đang sitting từ trước, bất kể `requested_gesture` có khớp danh sách hay
không.

## Thông báo trong game khi bấm `ReloadKey` (2026-09-10)

Yêu cầu người dùng: muốn có xác nhận trực quan trong game khi F5 reload
xong, thay vì chỉ ghi vào `AutoRegen.log` (dòng "Config reloaded (hotkey
pressed)." vẫn giữ nguyên). Tra `fromsoftware-rs` tìm được đúng widget game
tự dùng cho các thông báo kiểu "Autosaving..." - banner cuộn chữ trên đầu
màn hình (`CSMenuManImp::system_announce_view_model`, kiểu
`FeSystemAnnounceViewModel`), khác với `CSMenuManImp::display_status_message`
(chỉ nhận ID cố định như "You Died"/"Great Enemy Felled", không nhận text
tuỳ ý).

Cách hiện: tạo `AnnounceNotification { is_active: true, message: MenuString
{ static_string: null, allocated_string: DLString::from_str(text,
DLAllocator::runtime_heap_allocator()) } }` rồi `push_back` thẳng vào
`notifications: DLDeque<AnnounceNotification>` của
`system_announce_view_model` - không cần tự quản lý hiển thị/ẩn/animation gì
cả, máy trạng thái phát lại banner có sẵn của game (`FeSystemAnnounceView`)
tự nhận thấy hàng đợi không rỗng và tự chạy hết fade-in/scroll/fade-out.
Build sạch trên lần thử đầu (không có môi trường game để tự test thật, dựa
hoàn toàn vào đọc struct - cần người dùng xác nhận banner có thật sự hiện
đúng chữ "AutoRegen: config reloaded" không).

## AOB thay `rva::get()` cho tick đăng ký + allocator (2026-09-11)

Vấn đề gốc rễ mà mục "Fix đâm lén..." (2026-08-26) chưa giải quyết triệt
để: `CSTaskImp::wait_for_instance`/`wait_for_system_init_until_ready` (thêm
hôm đó) vẫn gọi xuống `eldenring::rva::get()` bên trong -  bảng RVA đó chỉ
ghi đúng **1** phiên bản game duy nhất mà `fromsoftware-rs` từng được
publish cho. Test thử chạy mod trên 1 bản game mới hơn bản đã pin thì
panic ngay từ bước này, không chỉ 1 tính năng nhỏ bị tắt - đúng kiểu lỗi
`InvalidRva` đã fix hôm 08-26 nhưng lần này ở 1 lớp sâu hơn, không retry
được nữa vì bảng RVA đơn giản là không có entry cho version đó.

**Đã sửa**: 2 file mới thay 2 chỗ còn phụ thuộc `rva::get()`:

- `task_hook.rs`: thay `fromsoftware_shared::task::SharedTaskImpExt::
  run_recurring` (nội bộ dùng `rva::get().register_task`) bằng quét AOB
  trực tiếp tìm hàm `register_task` trong `.text` - xác nhận (2026-09-09)
  hàm này byte-identical (trừ toán hạng rip-relative tự dịch theo link) trên
  cả exe 1.16.2 (`eldenring` 0.14.0, RVA `0xeb1fe0`) lẫn exe 1.17.0
  (`fromsoftware-rs acb2a19`, RVA `0xeb3de0`) - pattern neo vào phần thân
  hàm ổn định đó, không neo vào RVA. Cần tự dựng lại fake C++ vtable
  (`vtable-rs`, crate mới thêm vào workspace) để đăng ký task qua địa chỉ
  tự quét được, vì cơ chế `RecurringTask`/`self_ref` gốc của
  `fromsoftware-shared` là private, không tái sử dụng được cho địa chỉ khác.
- `alloc_hook.rs`: tương tự cho `DLAllocator::runtime_heap_allocator()` -
  global không phải Dantelion2-reflected singleton nên không tra được theo
  tên như `CSTaskImp`, phải neo AOB vào 1 hàm getter "đọc hoặc lazy-init"
  nhỏ, tự chứa, xác nhận (2026-09-11) byte-identical trên cả exe 1.16.2 lẫn
  1.17.0. **Thay thế lời gọi `DLAllocator::runtime_heap_allocator()` trực
  tiếp trong `regen.rs::show_announcement`** đã mô tả ở mục "Thông báo
  trong game khi bấm `ReloadKey`" (2026-09-10) ở trên - đoạn đó giờ đã lỗi
  thời, `show_announcement` gọi `alloc_hook::runtime_heap_allocator()`
  (trả `Option`, no-op nếu chưa resolve được) thay vì gọi thẳng hàm RVA-gated
  kia.

`wait_for_cs_task` trong `regen.rs` cũng đổi theo: bỏ hẳn
`wait_for_system_init_until_ready`/`CSTaskImp::wait_for_instance`, poll
thẳng `CSTaskImp::instance()` qua `FromStatic` - tra theo tên singleton
Dantelion2 (`#[shared::singleton("CSTask")]`), không đụng `rva::get()` ở
bất kỳ bước nào nữa. Cả 3 thay đổi cùng hướng: mod giờ tự sống sót qua bản
patch game mới mà không cần chờ `fromsoftware-rs` publish lại rồi build lại
DLL - chỉ tính năng nào AOB không tìm thấy pattern mới tắt riêng lẻ (có
log), không còn panic sập cả DLL.

## Viết lại `Regen Per Hit` từ đầu bằng Ghidra, thay `attack_hook.rs` bằng `hit_hook.rs` (2026-09-12)

Báo lỗi từ Nexus (v2.4.0, crash khi vào Volcano Manor) hoá ra **là bug thật
của AutoRegen** - không phải ModEngine2 như nghi ban đầu (điều tra ban đầu
dựa trên crash dump `.dmp` phân tích bằng `cdb.exe`: ntdll
`STATUS_INVALID_HANDLE` trong 1 critical section, thread do
`modengine2.dll`/chính `eldenring.exe` spawn, không có frame nào của
AutoRegen - dẫn tới kết luận sai là bug nằm ngoài mod). Test đối chứng sau
đó (build tạm bỏ đoạn forward "tham số thứ 5" ở trampoline `AttackHook`,
xem code cũ trong lịch sử git commit `44447a2`) cho thấy hết crash ngay -
xác nhận chắc chắn nguồn gốc nằm ở chính trampoline tay viết của
`attack_hook.rs`, cụ thể nghi do thiếu unwind metadata (`.pdata`/`.xdata`)
hợp lệ cho vùng code bị JMP-hijack giữa 1 call-site (`OnAttack` AOB của
Hexinton) - khi engine game raise 1 exception nội bộ (dùng cho control-flow,
không phải lỗi thật) đi qua đúng đoạn đó, unwinder tính sai stack frame và
làm hỏng dữ liệu ở đâu đó khác, muộn hơn nhiều (khớp với crash luôn rơi vào
1 con trỏ hoàn toàn không liên quan).

Thay vì vá tiếp trampoline hijack-giữa-call-site đó, **viết lại toàn bộ
`Regen Per Hit` từ đầu, không dùng lại AOB/CE table của Hexinton nữa**:

1. Lấy offset thật của `hp`/`max_hp`/`recoverable_hp`/... trực tiếp từ
   struct Rust `CSChrDataModule` (`std::mem::offset_of!`, build 1 example
   tạm rồi xoá), không đoán/không copy từ đâu khác.
2. Dùng Ghidra (`D:\Programs\ghidra_12.1.2_PUBLIC`, headless qua
   `analyzeHeadless.bat` - **lưu ý bug thật của bản thân script này**: cờ
   `-analyze` không hợp lệ trong bản 12.1.2, phải bỏ hẳn (phân tích tự chạy
   khi `-import`); cũng tránh đường dẫn có dấu cách khi gọi qua `.bat`)
   phân tích toàn bộ `eldenring.exe` (2.7.1.0, copy ra `D:\tmp` trước để
   né path có dấu cách), quét >1000 chỗ ghi vào offset `hp`, đối chiếu với
   cây gọi hàm (BFS 3 tầng) xuất phát từ đúng hàm hit-resolution mà
   Hexinton's `OnAttack` AOB từng trỏ tới (chỉ dùng làm điểm neo tham chiếu
   độc lập, không tái dùng kỹ thuật hook của họ) - lọc còn đúng 1 hàm khớp
   (ghi 4-byte, không phải 8-byte).
3. Decompile hàm đó (`SetHp`) và các hàm gọi nó, tìm ra `ApplyHpDelta`
   (`module, delta` - delta âm khi bị damage) rồi tới `FUN_140448910(ctx,
   attacker, hit_info, _, flag)` - hàm nhận đúng bộ tham số attacker/hit_info
   mà `attack_hook.rs` cũ vẫn dùng, và **xác nhận độc lập** `hit_info+0x228`
   = damage (khớp 100% `HITINFO_DAMAGE_OFFSET` cũ, nhưng giờ có bằng chứng
   từ decompile thật thay vì suy luận qua AOB Hexinton).
4. `hit_hook.rs` (mới, thay thế hoàn toàn `attack_hook.rs` - đã xoá) hook
   thẳng vào **entry point thật** của `FUN_140448910` (hàm biên dịch độc
   lập, có prologue chuẩn 15 byte kết thúc đúng ranh giới lệnh) thay vì hijack
   giữa call-site: dời nguyên 15 byte prologue gốc ra 1 buffer riêng, nối
   thêm `mov rax, <return_addr>; jmp rax` (12 byte) ngay sau đó để quay lại
   đúng chỗ - quan trọng: **buffer phải đủ chỗ cho cả phần jmp nối thêm**,
   thiếu bước này (bug thật gặp phải lúc code) khiến CPU chạy lố sang vùng
   `.data` rác ngay sau buffer, crash `ACCESS_VIOLATION` tức khắc khi có hit
   đầu tiên - đã sửa bằng cách tăng kích thước buffer và tự ghi thêm đoạn
   jmp đó trong `install()`. Hook mới chỉ "nghe" `rcx/rdx/r8/r9` (lưu ra
   stack, gọi callback Rust, khôi phục lại nguyên vẹn) rồi để hàm gốc chạy
   tiếp y nguyên - không tự gọi lại hàm gốc bằng tay như thiết kế cũ.

Kết quả test trong game (2026-09-12): đâm lén/riposte/chí mạng đều đúng
animation, hồi máu theo hit vẫn hoạt động đúng (log xác nhận đọc damage
chính xác qua nhiều loại đòn), và **không còn crash khi vào Volcano
Manor** dù bật `Regen.PerHit` - cùng máy, cùng save, cùng kịch bản từng
100% tái hiện được crash trước đó.

Ghi chú AOB pattern mới (`HIT_APPLY_PATTERN` trong `hit_hook.rs`): mới
verify trên đúng 1 bản game (2.7.1.0/1.17.1) - chưa cross-check bản khác
như cách `task_hook`/`alloc_hook` đã làm (mục "AOB thay `rva::get()`" ở
trên).

## Fix panic trong fake vtable của `task_hook.rs` - nghi ngờ nguồn gốc giật hình sau bản 2.5.0 (2026-09-12)

Sau khi 2.5.0 (đã bao gồm `task_hook.rs`/`alloc_hook.rs` từ mục "AOB thay
`rva::get()`" ở trên, nhưng **chưa** có `hit_hook.rs` - việc đó publish sau)
lên Nexus, có báo cáo giật hình 1-2 giây liên tục, lặp lại mỗi 15-20 giây,
xảy ra **cả ở menu** (không liên quan gì đến combat/`Regen.PerHit`) - đối
chiếu git log xác nhận đúng thời điểm publish rơi sau commit `task_hook.rs`
(`a7d9138`, 2026-09-11 14:40).

Soát lại `task_hook.rs::Task` (fake vtable dùng để đăng ký tick vào
`CSTaskImp`, xem mục "AOB thay `rva::get()`" ở trên): `get_runtime_class`
và `destructor` đều để `unimplemented!()` (panic) - khác với `execute`
(closure tick mỗi frame), 2 hàm này được **chính engine game gọi trực
tiếp qua vtable**, không đi qua `catch_unwind` nào của `regen.rs`. Nếu
`CSTaskImp` từng gọi 1 trong 2 hàm này vào bất kỳ lúc nào (chưa xác nhận
được chắc chắn có xảy ra hay không, nhưng đây là hành vi không xác định/UB
bất kể có xảy ra hay không - panic unwind thẳng vào call stack C++ của
game, không có landing pad Rust) - đủ khớp để giải thích 1 kiểu lỗi định
kỳ, không phải crash cứng ngay lập tức, xảy ra bất kể `Regen.PerHit`.

**Đã sửa**: `get_runtime_class` trả về `0`, `destructor` no-op - cả 2 đều
an toàn vì `Task` được `Box::leak` (sống suốt vòng đời DLL, không bao giờ
thật sự bị game "huỷ"), nên không cần logic thật đằng sau 2 hàm này, chỉ
cần không panic. Tự test lại (máy tác giả) không thấy giật hình, nhưng
**chưa tái hiện được hiện tượng gốc để xác nhận chắc chắn** đây đúng là
nguyên nhân duy nhất - ghi vào changelog `2.5.1` như 1 fix chứ không khẳng
định chắc 100%.

## Đổi `Regen.PerTick.Unit` → `Regen.PerTick.ValueType`, `Regen.PerHit.Trigger` → `Regen.PerHit.Mode` (2026-09-14)

Đổi tên 2 key cho rõ nghĩa hơn, không đổi hành vi/giá trị mặc định:

- `Regen.PerTick.Unit` → `Regen.PerTick.ValueType` (vẫn 0 = điểm cố định,
  1 = % max stat).
- `Regen.PerHit.Trigger` → `Regen.PerHit.Mode` (vẫn 0/1/2 như mục "Gộp
  `Regen Per Hit`/`Regen Per Damage`" phía trên) - đổi tên vì trùng tên với
  `Regen.PerTick.Trigger` (chọn *điều kiện* áp dụng: always/combat/idle/
  sitting) dù 2 key mang ý nghĩa hoàn toàn khác nhau (`Regen.PerHit.Mode`
  chọn *cách tính giá trị hồi*: điểm cố định/%max/%damage) - dễ gây nhầm lẫn
  khi đọc ini cạnh nhau.

Áp dụng đồng thời cho [`SomeTweaks`](../sometweaks) (cùng module `regen`,
xem README của nó cùng ngày) để 2 mod tiếp tục dùng chung 1 thiết kế ini.
Không tự động migrate giá trị (đổi tên key, không đổi ý nghĩa/giá trị) -
`config::migrate()` tự đẩy `Regen.PerTick.Unit`/`Regen.PerHit.Trigger` cũ
vào `[Legacy]` ở lần chạy đầu sau khi cập nhật DLL, người dùng cần tự copy
giá trị đã tùy chỉnh sang key mới trong `AutoRegen.ini`.

## Thêm `Regen.PerTick.ValueType=2` (% máu đã mất) và `Regen.PerTick.Cap` (2026-09-14)

Feature request từ người dùng, bàn qua nhiều vòng trước khi chốt xuống còn
đúng 2 thứ (ban đầu có bàn thêm `Trigger=5`/`LowHpThreshold` - "chỉ hồi khi
dưới ngưỡng X%" - nhưng bị bỏ vì `Cap` một mình đã đủ tạo hiệu ứng "hồi tới
X% rồi dừng" khi kết hợp đúng, không cần thêm 1 trigger riêng):

- **`Regen.PerTick.ValueType=2`**: giá trị mới cho key `ValueType` đã có
  (0=điểm cố định, 1=% max) - hồi theo **% phần máu/FP/SP đang thiếu**
  (`(max - current) * pct`), khác hẳn `ValueType=1` (luôn hồi cùng 1 lượng
  bất kể current đang bao nhiêu). Hiệu ứng: hồi nhanh lúc máu thấp, chậm dần
  khi gần đầy (đường cong tiệm cận, giống natural regen trong nhiều RPG khác)
  - có sàn tối thiểu 1 điểm/tick (giống `ValueType=1`) nên vẫn bò tới đúng
    max chứ không dừng lửng lơ ở 99%.
- **`Regen.PerTick.Cap`** (mặc định `100` = không giới hạn thêm): trần hồi
  phục riêng, tính theo % **max stat thật** (không phải % của giá trị nào
  khác) - áp dụng cho **cả 3 stat, mọi `Trigger` (0-4)**, độc lập hoàn toàn
  với `ValueType`. Khác với ý tưởng `Trigger=5` đã bỏ: `Cap` không phải là
  điều kiện kích hoạt, mà là **trần** luôn có hiệu lực bất kể tick đang chạy
  vì lý do gì (always/ngoài combat/trong combat/idle/sitting).

Lưu ý tương tác nếu dùng `Cap` cùng combat-based trigger nào đó có "ngưỡng"
riêng của nó (không còn `LowHpThreshold` trong bản này nữa, nhưng ghi lại cho
rõ nguyên tắc chung): `Cap` luôn là trần cuối cùng, tính theo max stat thật -
không phụ thuộc điều kiện tick đang chạy vì lý do gì.

Implementation (`regen.rs`): xoá hẳn `split_by_unit()` (chỉ dùng được cho 2
mode cũ, không đủ cho mode "% missing" vì cần biết `current` ngay lúc tính,
trong khi hàm cũ tính `(flat, percent_fraction)` trước khi resolve player) -
thay bằng `compute_tick_heal(value_type, value, current, max)` (thuần, nhận
đủ 4 tham số). Tách `heal_main_player()` cũ (dùng bởi `hit_hook.rs`/
`Regen.PerHit`, giữ nguyên convention `(flat_amount, percent_fraction)`,
không đổi hành vi) và hàm mới `heal_main_player_tick()` (dùng riêng cho
`Regen.PerTick`, nhận `value_type`/`value`/`cap_pct`) ra 2 hàm độc lập, dùng
chung 1 helper `with_stat_mut()` để resolve player + lấy `&mut current`/`max`
- tránh lặp lại đúng đoạn code resolve `WorldChrMan`/`main_player` vốn đã có
sẵn.

Mới làm ở `autoregen`, **chưa port sang [`sometweaks`](../sometweaks)** -
đợi ổn định rồi mới đưa thiết kế này quay lại đó (khác thứ tự thường lệ mọi
lần trước, lần này cố ý làm 1 bên trước theo yêu cầu người dùng).

## Đổi `[Debug]`/`RegenLog` thành `[Logging]`/`LogFile` (2026-09-14)

Đổi tên cho đúng phạm vi thật: key này chưa bao giờ chỉ dành riêng cho
"Regen" - nó gate luôn log per-hit của `Regen.PerHit` (`hit_hook.rs`) lẫn
log điều kiện tick/gesture debug (`regen.rs`), tức là bật/tắt log chi tiết
cho **toàn bộ mod**, không riêng 1 tính năng nào. Tên `RegenLog` dễ khiến
hiểu lầm là chỉ áp dụng cho `[Regen Per Tick]`. Không đổi hành vi: vẫn chỉ
gate phần log chi tiết (per-hit damage/heal, điều kiện tick/gesture) - file
log `AutoRegen.log` tự nó **luôn ghi** (ghi đè mỗi lần chạy) bất kể key này,
xem `lib.rs`.

`config::migrate()` tự đẩy `RegenLog` cũ (trong `[Debug]` hoặc bất kỳ đâu)
vào `[Legacy]` ở lần chạy đầu sau khi cập nhật DLL - cần tự copy giá trị đã
tùy chỉnh sang `[Logging] LogFile` mới trong `AutoRegen.ini`.

## Tách `task_hook.rs`/`alloc_hook.rs`/`wait_for_cs_task`/`run_recurring_safe` ra crate `engine` (2026-09-14)

Xóa hẳn `src/task_hook.rs`/`src/alloc_hook.rs` khỏi crate này, cùng
`wait_for_cs_task`/`run_recurring_safe` nội bộ trong `regen.rs` và
`main_player_chr_ins_ptr` - chuyển nguyên bản (không đổi logic/AOB pattern)
sang crate share mới **[`engine`](../../engine)** ở gốc workspace, ngang
hàng với [`shared`](../../shared) (`common`). Lý do: lúc port tính năng
`Drop Rate` của [`sometweaks`](../sometweaks) ra mod riêng
([`DropMultiplier`](../dropmultiplier), cùng ngày), nhận ra bộ 4 thứ này
(vốn chỉ ở `autoregen`, viết ra để giải quyết đúng vấn đề `rva::get()`
version-lock - xem mục "AOB thay `rva::get()`..." phía trên) sắp phải copy
lần thứ 2 (`DropMultiplier`) trong khi `sometweaks`/`risearcher` cũng đang
tự mang 2 bản `task.rs`/`player.rs`/`reload.rs` gần như y hệt nhau (chỉ khác
là bản của họ **chưa** có cải tiến AOB, vẫn còn dính `rva::get()`) - tách ra
1 lần trước khi nhân bản thêm, thay vì tiếp tục copy-paste mỗi lần 1 mod mới
cần.

`engine` khác `common`: `common` **cố tình** không phụ thuộc
`eldenring`/`fromsoftware-shared` (xem comment đầu `Cargo.toml` của nó -
plumbing tổng quát, có thể tái dùng cho mod ở game khác); `engine` thì
ngược lại, tồn tại **chính vì** cần phụ thuộc 2 crate đó để nói chuyện với
struct/singleton thật của game, chỉ tách khỏi từng mod chứ không tách khỏi
`eldenring`. Nội dung crate mới (4 module, xem doc comment `engine/src/lib.rs`):

- `task_hook`/`alloc_hook`: y hệt bản cũ của `autoregen`, không đổi 1 dòng
  AOB pattern nào.
- `task`: gộp `wait_for_cs_task()` (bản mới nhất của `autoregen`, tra
  `CSTaskImp::instance()` theo tên, không qua `wait_for_instance()`/RVA) với
  chữ ký `run_recurring_safe(cs_task, tag, group, f)` có tham số `tag` từ
  `sometweaks`/`risearcher`'s `task.rs` (bản `autoregen` cũ không có `tag`,
  chỉ dùng cho 1 tính năng duy nhất nên không cần) - **quan trọng**: đây là
  bản `wait_for_cs_task` **mới hơn, an toàn hơn** bản `sometweaks`/
  `risearcher` đang dùng (bản đó vẫn gọi `wait_for_system_init_until_ready`
  + `CSTaskImp::wait_for_instance` retry `InvalidRva` - còn dính `rva::get()`
  1 lớp sâu hơn), nên 2 mod đó khi migrate sang `engine` sau này sẽ tự động
  được nâng cấp luôn, không chỉ gọn code.
- `player`: `main_player_chr_ins_ptr`/`wait_for_solo_param_repository` -
  giữ nguyên từ `sometweaks::player`/`risearcher::player` (giống hệt nhau ở
  cả 2 nơi), bỏ `NewActionPresses`/`main_player_new_action_presses` (chi
  tiết riêng của `Regen.PerHit.ExcludeAow`, không phải nhu cầu chung).
- `reload`: watch `ReloadKey` + `RELOAD_GENERATION`, giữ nguyên từ
  `sometweaks::reload`/`risearcher::reload`, chỉ đổi để dùng
  `engine::task` nội bộ thay vì tự có bản riêng.

`autoregen` (crate này) migrate xong, dùng thẳng `engine::task_hook`/
`engine::alloc_hook`/`engine::task::{wait_for_cs_task, run_recurring_safe}`/
`engine::player::main_player_chr_ins_ptr` - build lại xác nhận không đổi
hành vi. **`sometweaks`/`risearcher` chưa migrate** (vẫn giữ bản `task.rs`/
`player.rs`/`reload.rs` riêng, cũ hơn) - để dành làm sau, không bắt buộc
ngay; `DropMultiplier` (mod mới) dùng thẳng `engine` ngay từ đầu, không tự
viết bản riêng nào.

## Gộp crate `engine` ngược vào `shared` (`common`) (2026-09-14, cùng ngày)

Đảo ngược quyết định ở mục ngay trên - `engine` (crate riêng, mới tách được
vài giờ) gộp lại vào [`shared`](../../shared) (`common`), không còn tồn tại
độc lập nữa. Lý do: bàn lại với người dùng về việc gọn thư mục (`shared`/
`engine` đều nằm ở gốc workspace, muốn nhóm chung 1 chỗ) - cân nhắc giữa
"gộp thành 1 crate" và "2 crate con trong `shared/`", ban đầu nghiêng về
giữ tách biệt vì lo `weightmultiplier` (mod duy nhất không cần
`eldenring`) sẽ bị kéo thêm dependency không cần thiết nếu gộp.

Test thật (build `WeightMultiplier` cả trước/sau khi gộp, so kích thước
file): **kích thước `.dll` cuối cùng không đổi** (272896 byte cả 2 lần) -
workspace này đã bật `lto = true`/`codegen-units = 1` từ trước, nên code
không dùng tới (toàn bộ `task_hook`/`alloc_hook`/`task`/`player`/`reload`,
với `weightmultiplier` không gọi dòng nào) bị linker cắt bỏ khỏi output
cuối cùng, bất kể có "khai" dependency đó hay không. Cái giá thật sự của
việc gộp chỉ là: build riêng lẻ 1 mod không dùng các module đó (`cargo
build -p weightmultiplier` từ cache sạch) giờ cũng phải compile
`eldenring`/`fromsoftware-shared` một lần - không ảnh hưởng gì tới sản phẩm
cuối, chỉ hơi chậm hơn ở build riêng lẻ lần đầu. Đánh đổi này được người
dùng chấp nhận để đổi lấy cấu trúc thư mục gọn hơn (không cần nhớ "cái nào
ở `shared/`, cái nào ở `engine/`").

Thực hiện: dời nguyên 5 file (`task_hook.rs`/`alloc_hook.rs`/`task.rs`/
`player.rs`/`reload.rs`) từ `engine/src/` vào `shared/src/`, đổi
`use common::X` nội bộ trong các file đó thành `use crate::X` (giờ cùng 1
crate), thêm `eldenring`/`fromsoftware-shared`/`vtable-rs` vào
`[dependencies]` của `shared/Cargo.toml`, xóa hẳn thư mục `engine/` +
entry trong `members` của `Cargo.toml` gốc. `autoregen`/`dropmultiplier`
đổi mọi `engine::X` thành `common::X`, bỏ dòng `engine = { path = ... }`
trong `Cargo.toml` của cả 2. Build + `cargo test --workspace` xác nhận
không đổi hành vi.

## Fix log `Regen.PerTick` spam mỗi frame/interval dù giá trị không đổi (2026-09-14)

Người dùng phát hiện `AutoRegen.log` (khi bật `[Logging] LogFile`) bị dòng
`Regen.PerTick: trigger=... -> condition_met=...` ([regen.rs](src/regen.rs))
ghi lặp lại liên tục mỗi `Regen.PerTick.Interval` (mặc định 1s) kể cả khi
player đứng yên và không có gì thay đổi (`condition_met`/`in_combat`/`idle`/
`sitting` y hệt lần trước) - log này vốn thêm ở mục "Thêm đệm 5s cho Idle..."
(2026-09-10) để debug bug Sitting không hồi, nhưng chưa từng throttle theo
giá trị, chỉ throttle theo `Regen.PerTick.Interval` (vốn để throttle tần
suất *heal*, không phải tần suất log).

Sửa: thêm biến `last_logged_tick_state` (capture trong closure `run()`,
cùng cách với `elapsed_ms`/`attack_hook_installed`) lưu tuple `(trigger,
condition_met, in_combat, idle, sitting)` của lần log gần nhất - chỉ ghi
dòng log mới khi tuple này đổi so với lần trước. Không đổi ini key/hành vi
heal nào, chỉ giảm nhiễu log.

## Revert: `queued_action_inputs.gesture()` làm Sitting mất hẳn tác dụng, quay lại `new_action_presses.gesture()` (2026-09-14)

Bản 2.6.0 (commit `63f980b`) từng đổi tín hiệu chốt gesture từ
`new_action_presses.gesture()` sang `queued_action_inputs.gesture()`, với ý
định chỉ tính là "ngồi" khi animation hiện tại thực sự nhận nút bấm (fix báo
lỗi Kolagon, 2026-09-13: bấm gesture giữa lúc đang cast không nên tính là
ngồi). Người dùng tự phát hiện sau khi deploy: `Trigger=4` (Sitting) **mất
hẳn tác dụng**, không còn hồi máu lúc ngồi nữa.

Nguyên nhân: theo doc của `fromsoftware-rs`
(`CSChrActionRequestModule::queued_action_inputs`), field này chỉ được set
cho action **cũng có mặt trong `possible_action_inputs`** - bitmask này quản
lý luật cancel của các action chiến đấu thường (đánh/né/đỡ/dùng đồ...),
không bao gồm hệ thống gesture wheel (một luồng UI riêng, không đi qua
`possible_action_inputs`). Kết quả `queued_action_inputs.gesture()` gần như
không bao giờ bật, nên `IS_SITTING` không bao giờ thành `true` nữa - lỗi im
lặng, không có log ERROR nào báo vì đây không phải crash/panic, chỉ là điều
kiện luôn `false`.

Đã revert lại đúng `new_action_presses.gesture()` như trước 2.6.0 (xem mục
"Thêm `Regen.PerTick.Trigger=3`..." phía trên) - chấp nhận lại edge-case
hiếm gặp ban đầu (gesture bấm giữa lúc đang cast) cho tới khi tìm được tín
hiệu đúng hơn và **test thực tế trong game trước khi publish**, không suy
đoán từ doc comment của thư viện nữa như lần này.

### 2 lần thử tiếp theo đều sai giả thuyết - test trực tiếp trong game (2026-09-14)

Ngay sau khi revert, thử đúng lời hứa "test thực tế trước khi publish" ở
trên - 2 giả thuyết dựa trên doc comment của `fromsoftware-rs`, **cả 2 đều
bị chính log `LogFile` trong game bác bỏ**:

1. **`TaeCancelFlags::cancel_disable`** ("Global cancel disable... Persistent"
   theo doc) - kỳ vọng `true` lúc đang cast phép (không thể bị ngắt). Log
   thực tế: `cancel_disable=false` **ngay cả khi đang niệm phép** - field
   này không phản ánh "đang trong animation không-cancel-được" như tên gọi
   có vẻ ngụ ý.
2. **`possible_action_inputs.gesture()`** (tự làm lại đúng phép AND mà
   `queued_action_inputs` lẽ ra phải làm, nhưng không qua bước "cleared khi
   `stay_state` active" đã nghi ngờ là nguyên nhân gây lỗi 2.6.0) - kỳ vọng
   `true` lúc ngồi bình thường. Log thực tế: `gesture_allowed=false` **ngay
   cả lúc ngồi thành công bình thường** - phá luôn cả trường hợp đúng, y hệt
   kiểu lỗi của 2.6.0.

Kết luận rút ra: gesture (`ACTION_ARM_GESTURE`, bit 21 của `ChrActions`) tuy
nằm chung struct `ChrActions` với các action chiến đấu, nhưng **không thực
sự tham gia** vào hệ thống gating `possible_action_inputs`/
`queued_action_inputs` của `CSChrActionRequestModule` - cả 2 field này đọc
sai bất kể tình huống nào, không riêng gì lúc cast phép. Cái thực sự chặn
gesture lúc mid-cast nằm ở nơi khác trong bộ nhớ game, chưa tìm ra.

Đã revert cả 2 field này, quay về đúng `new_action_presses.gesture()`
nguyên bản (edge-case Kolagon vẫn còn treo, chưa có hướng mới). Ghi chú
trong code (`ActionSnapshot`'s doc comment, `regen.rs`) để không ai thử lại
2 field này mà không kiểm tra lại điều kiện "phải đọc `true` lúc ngồi
thành công bình thường VÀ `false` lúc mid-cast, cả 2 xác nhận qua
`LogFile` trong game" trước.

### Lần thử thứ 3 - `possible_action_cancels` tưởng đúng qua 11 lần test, hoá ra là artifact chu kỳ animation idle (2026-09-15)

Khác 2 lần trước (đoán từ doc comment), lần này bắt đầu bằng dữ liệu thật:
thêm field debug dump toàn bộ bitfield liên quan vào dòng log `Gesture:`,
nhờ người dùng test trực tiếp trong game và cung cấp log thật để đối chiếu
(không tự suy đoán). Qua nhiều vòng test (đơn lẻ, dồn dập, có nhãn rõ từng
lần bấm), `possible_action_cancels` (OR toàn bộ bit qua hàm
`chr_actions_any_set`, đặt tên `any_cancelable`) cho kết quả **nhất quán
tuyệt đối**: bằng 0 ở cả 4 lần "cast phép + bấm gesture" test riêng biệt,
khác 0 ở cả 7 lần ngồi/đứng dậy bình thường (kể cả bấm dồn dập). Đã áp dụng
làm điều kiện lọc (`&& snapshot.any_cancelable`) và xác nhận hoạt động đúng
qua thêm 1 vòng test nữa.

Nhưng sau đó, theo yêu cầu của người dùng ("muốn chứng minh giá trị này là
gì trước khi bấm gesture"), thêm 1 dòng log riêng in `stay_state`/
`any_cancelable`/giá trị thô `possible_action_cancels` mỗi khi **có thay
đổi**, độc lập hoàn toàn với việc bấm gesture (log liên tục theo từng
frame nền, không cần chờ user bấm nút) - để quan sát giá trị này biến đổi
ra sao khi không hề đụng tới gesture. Kết quả bất ngờ: **đứng yên hoàn toàn
không làm gì** trong hơn 1 phút, `possible_action_cancels` vẫn tự dao động
đều đặn **đúng chu kỳ 3 giây một lần** giữa `0` và giá trị "mở gần hết"
(`31073500911`) - một artifact của chính animation đứng yên (nhiều khả năng
có 1 khung hình "vulnerable"/không-cancel-được cố định lặp lại trong vòng
lặp idle của game), hoàn toàn không liên quan gì đến việc đang cast phép.

Điều này có nghĩa 11 lần test "thành công" trước đó **chỉ là trùng hợp về
thời điểm bấm** rơi đúng pha nào của chu kỳ 3 giây, không phải vì field này
thực sự phân biệt được "đang cast" hay "không cast". Nếu giữ nguyên fix đó,
sẽ có rủi ro thật: người chơi bấm gesture đúng lúc rơi vào khung hình "dip"
tự nhiên của chu kỳ - dù hoàn toàn không cast gì - vẫn bị chặn nhầm, tạo ra
1 bug ngẫu nhiên mới (block sai ngắt quãng) còn khó phát hiện hơn bug gốc.

**Bài học quan trọng nhất từ 3 lần thử**: dù có dữ liệu thật từ log game
(khác hẳn 2 lần đầu chỉ đoán từ doc), **so sánh 2 nhóm dữ liệu (cast vs
không-cast) là chưa đủ** - còn phải xác nhận field đó *ổn định theo thời
gian* khi hoàn toàn không tương tác gì (không chỉ đúng lúc vừa bấm), nếu
không sẽ nhầm 1 chu kỳ nội tại của animation game với 1 tín hiệu nhân-quả
thật.

Đã revert lại lần thứ 3 này, quay về đúng `new_action_presses.gesture()`
nguyên bản không lọc gì thêm (`regen.rs`, mục "Revert" phía trên) - `commit
message`/code comment ghi rõ cả 3 field đã thử và lý do thất bại của từng
field, kèm điều kiện bắt buộc trước khi thử field thứ 4: phải có 1 đoạn
`LogFile` capture **nhiều phút, hoàn toàn không tương tác gì** chứng minh
field đó đứng yên khi không cast, không chỉ so sánh 2 thời điểm bấm riêng
lẻ.

## Lần thử thứ 4 - THÀNH CÔNG: xác nhận trễ qua TAE animation ID thay vì đoán field tại đúng khung hình bấm (2026-09-17)

Sau khi 3 field trong `CSChrActionRequestModule`/`CSChrActionFlagModule`
đều thất bại (xem 2 mục trên), thử dịch ngược `eldenring.exe` bằng Ghidra
headless rồi IDA Professional 9.3 (2 vòng agent riêng, license IDA đã kích
hoạt) để tìm điều kiện game tự kiểm tra trước khi cho gesture chạy - tìm ra
được hàm `PlayGesture` thật (`0x14078a9c0`) và xác định đúng gate của nó là
`GetManipulatorKind() == 1` (chỉ cho gesture khi `ChrCtrl` đang được điều
khiển trực tiếp bởi `PadManipulator`, không phải network/replay/AI/cưỡi
ngựa/đồng hành) - nhưng đối chiếu lại với log thật thì gate này **không
giải thích được bug**: chơi solo thì luôn ở kind `Pad=1` bất kể có đang cast
phép hay không, và field `requested_gesture` vẫn được ghi bình thường ngay
cả lúc bị chặn - mâu thuẫn với giả thuyết. Kết luận: RE tĩnh đã cạn hướng
hợp lý cho đúng bug này (chi tiết đầy đủ quá trình RE không lưu vào README,
chỉ lưu trong lịch sử hội thoại/file tại `D:\tmp` lúc điều tra).

**Hướng thắng cuộc**: đổi hẳn chiến lược - thay vì đọc 1 field *tại đúng
khung hình bấm* để đoán trước kết quả (cách cả 4 lần thử field đều đi),
chuyển sang **xác nhận sau một khoảng trễ** bằng
`CSChrTimeActModule::anim_queue[read_idx].anim_id` (TAE animation ID thật
đang chạy) - field này từng bị gạt bỏ ở mục "Thêm `Regen.PerTick.Trigger=3`"
(2026-09-10) vì cần 1 bảng map TAE-id↔GESTURE_ID chưa dò được, nhưng lần
này **không cần bảng map đó nữa**: chỉ cần biết "animation có đổi khác so
với lúc bấm hay không", không cần biết đổi thành ID gì.

Cơ chế (`confirm_pending_sit`, `regen.rs`): bấm gesture khớp
`sit_gesture_ids()` không còn chốt `IS_SITTING=true` ngay - chỉ ghi nhận
"đang chờ xác nhận" (`PENDING_SIT_SINCE_MS`) kèm `anim_id` tại đúng lúc bấm
(`PENDING_SIT_ANIM_ID`). Sau `SIT_CONFIRM_DELAY_MS` (500ms), đọc lại
`anim_id` và chốt `IS_SITTING=true` chỉ khi **cả 2 điều kiện** đúng: khác
với `anim_id` lúc bấm, VÀ khác `0` (idle bình thường). Đứng dậy (bấm lần 2
khi đang ngồi) vẫn xử lý ngay lập tức như cũ, không qua bước chờ này - đứng
dậy tự nó đã chắc chắn, không cần xác nhận gì thêm.

Lý do cần **cả 2** điều kiện (bản đầu tiên chỉ check "khác `0`" từng bị
confirm sai trong lúc test, trước khi sửa thành check kép này):
- Chỉ "khác `0`" là chưa đủ: lúc bị chặn giữa lúc cast, animation cast
  chưa kịp đổi gì trong 500ms đầu (vẫn y hệt lúc bấm, mà bản thân ID cast
  cũng khác `0`) → sai dương nếu chỉ check khác `0`.
- Chỉ "khác lúc bấm" cũng chưa đủ: nếu delay dài hơn thời gian cast còn
  lại, animation cast tự nhiên kết thúc và về `0` (idle) trong lúc chờ -
  `0` vẫn "khác lúc bấm" nhưng không phải ngồi thật.
- Kết hợp cả 2 xử lý đúng mọi trường hợp đã test.

Xác nhận qua nhiều vòng test trong game bằng `LogFile`: **5/5 lần cast +
bấm gesture bị chặn đúng** (`is_sitting=false`), **2/2 lần ngồi thật đúng**
(`is_sitting=true`), **2/2 lần đứng dậy đúng** ngay lập tức - không còn
false-positive, không phá trường hợp bình thường.

Chi phí hiệu suất: gần như bằng 0 - khi không có gesture nào đang chờ xác
nhận (>99.99% thời gian chơi), `confirm_pending_sit` chỉ đọc 1 biến atomic
rồi `return` ngay, không gọi `WorldChrMan::instance()` hay đọc animation gì
cả.

Đã dọn sạch toàn bộ code debug dùng để điều tra (không giữ lại trong
codebase, khác với những gì mục trước dự tính): field `any_cancelable`/
`stay_state`/`possible_cancels`/`gesture_debug` trong `ActionSnapshot`, hàm
`chr_actions_any_set`, `log_stay_state_changes`, `log_current_anim_changes`
và 2 static latch đi kèm - chỉ giữ lại `current_anim_id()` (dùng thật bởi
`confirm_pending_sit`) và 2 dòng log sản phẩm (`pending confirm`/`confirm
anim_id=... -> is_sitting=...`).

### 2 lần vá thêm sau khi test bấm dồn dập (2026-09-17, cùng ngày)

Test lại sau khi "chốt" bản trên, phát hiện 2 lỗi mới khi bấm gesture nhanh
nhiều lần liên tiếp (không liên quan gì đến cast phép nữa - đây là nhóm bug
"rapid re-press" từng bị gác lại ở mục "Lần thử thứ 4" phía trên khi mới
phát hiện qua test lần đầu):

1. **Chỉ check "khác lúc bấm" chưa đủ khi animation transition đi qua nhiều
   ID trung gian**: bấm ngồi rồi bấm dồn dập lần 2/3 ngay khi animation đứng
   dậy còn đang chuyển tiếp (chưa ổn định) có thể bị "bắt trúng" đúng lúc
   animation đó đang ở 1 ID trung gian khác `0`, khác baseline lúc bấm →
   confirm sai thành `is_sitting=true` dù nhân vật đang đứng dậy, không phải
   ngồi. **Sửa**: thêm yêu cầu `anim_id` phải **giữ nguyên ổn định liên tục
   `ANIM_STABLE_MS`** (300ms → tăng lên 1000ms sau khi cân nhắc animation
   trung gian quan sát được đổi ID mỗi ~0.5-1s) trước khi tin, thay vì chỉ
   đọc 1 lần duy nhất tại mốc delay. Thêm `SIT_CONFIRM_TIMEOUT_MS` (3000ms,
   gấp 3 lần ngưỡng ổn định) để tránh treo "pending" vĩnh viễn nếu animation
   không bao giờ ổn định.
2. **Bấm lần 2 trong lúc lần 1 chưa kịp xác nhận xong bị ghi đè baseline
   sai**: trước đó, 1 lần bấm mới trong lúc đang `pending` bị code hiểu
   nhầm thành "bắt đầu ngồi mới", ghi đè `PENDING_SIT_ANIM_ID` bằng `anim_id`
   đọc được ngay lúc bấm 2 - nhưng lúc đó animation ngồi thật **đã bắt đầu
   chuyển tiếp**, nên khi ngồi ổn định xong, nó không còn "khác" baseline bị
   ghi đè đó nữa → không bao giờ confirm được, dù nhân vật ngồi thật (không
   hồi máu). Rồi 1 lần bấm thứ 3 (ý định đứng dậy) lại bị hiểu nhầm tiếp
   thành "ngồi mới" (vì cờ vẫn `false`) → cuối cùng confirm nhầm đúng lúc
   đang đứng dậy. **Sửa**: 1 lần bấm lặp lại trong lúc còn `pending` giờ bị
   **bỏ qua hoàn toàn** (không đụng gì tới baseline/timer đang chờ), để lần
   bấm đầu tiên tự chạy hết chu trình xác nhận của nó. Nếu `IS_SITTING` sau
   đó thực sự thành `true`, 1 lần bấm tiếp theo sẽ tự nhiên rơi đúng vào
   nhánh "đang ngồi rồi → huỷ ngay lập tức" có sẵn, không cần thêm logic gì
   khác.

Xác nhận lại qua nhiều lần bấm dồn dập (2-3 lần liên tiếp) xen giữa các lần
ngồi bình thường và 1 lần cast+chặn trong cùng phiên test - toàn bộ log thu
được đều đúng: các lần bấm lặp lại lúc pending đều bị `ignored`, mọi lần
ngồi cuối cùng đều `confirm ... -> is_sitting=true` đúng lúc animation ổn
định ở họ ID ngồi thật, huỷ ngồi đúng ngay lập tức, và cast+chặn vẫn đúng
`is_sitting=false` (lần này qua nhánh timeout vì `anim_id=0` chưa kịp ổn
định đủ 1000ms khi hết giờ chờ, nhưng kết luận cuối vẫn đúng).

## Giảm log per-hit trong `hit_hook.rs` và log `Regen.PerTick` - nguồn log lớn nhất trong phiên chơi dài (2026-09-17)

Người dùng chỉ ra: bật `[Logging] LogFile=true` rồi chơi 1 phiên dài (nhiều
combat) sẽ sinh ra log rất lớn. Rà lại toàn bộ log trong crate, tìm ra thủ
phạm chính: dòng `HitHook: player dealt {damage} damage (atkId=...
sourceType=... isSkill=...)` trong `hit_hook.rs::apply_hit_heal` - ghi
**mỗi khi player đánh trúng bất kỳ thứ gì**, kể cả khi `Regen.PerHit` đang
tắt hoàn toàn - nặng hơn nhiều so với các log gesture (chỉ ghi khi bấm nút,
tần suất thấp) hay `Regen.PerTick` (đã dedupe theo thay đổi từ trước). Đây
là log từ thời mới viết lại `hit_hook.rs` (mục "Viết lại `Regen Per Hit` từ
đầu bằng Ghidra", 2026-09-12), dùng để xác minh `damage`/`atkId`/
`sourceType`/`isSkill` đọc đúng lúc đó - không còn giá trị vận hành, dòng
log "player {source} -> +HP +FP +SP" ngay bên dưới (chỉ ghi khi thực sự có
hồi máu xảy ra, tần suất thấp hơn nhiều vì phụ thuộc điều kiện
`Regen.PerHit`) đã đủ cho debug thực tế.

Ban đầu định xoá hẳn, nhưng theo yêu cầu người dùng (muốn giữ khả năng bật
lại để debug sau này mà không cần viết lại từ đầu) - đã **comment lại**
thay vì xoá: dòng log, cùng `read_atk_id()`/`HITINFO_ATK_PARAM_ID_OFFSET`
(chỉ được dùng ở đúng chỗ đó, sẽ thành dead code nếu để active mà không có
log dùng tới) đều bị comment `//` trong `hit_hook.rs`, kèm chỉ dẫn ngay
phía trên - cần debug lại thì chỉ cần bỏ comment cả 3 chỗ rồi build lại,
không cần dò lại offset hay viết lại logic. Vẫn giữ nguyên (active)
`HITINFO_DAMAGE_OFFSET`/`HITINFO_SOURCE_OBJECT_OFFSET` (còn dùng thật bởi
logic hồi máu) và dòng log "player {source} -> +HP +FP +SP" (chỉ ghi khi
thực sự có hồi máu xảy ra).

Cùng lúc đó, người dùng chỉ ra thêm: log `Regen.PerTick` (đã dedupe theo
thay đổi từ mục "Fix log `Regen.PerTick` spam..." phía trên) **vẫn** sinh
ra 1 dòng log mỗi khi player di chuyển rồi đứng yên trở lại - vì bản dedupe
cũ so sánh theo **toàn bộ tuple** `(condition, condition_met, in_combat,
idle, sitting)`, mà chỉ riêng việc đi rồi dừng cũng đủ làm `idle` nhảy
`false`→`true`, dù `condition_met` (kết quả hồi máu bật/tắt) không hề đổi
với `Trigger=1`/`2`. Sửa lại: dedupe chỉ theo `(condition, condition_met)`
- tức là chỉ log khi **kết quả thật sự thay đổi** (bắt đầu/dừng hồi máu),
các chi tiết `in_combat`/`idle`/`gesture_active` vẫn đọc mới và in kèm
trong dòng log đó, chỉ không dùng để quyết định có log hay không nữa.

## Đổi tên tính năng từ "Sitting" sang "Gesture" - tổng quát hoá đúng bản chất (2026-09-17)

Theo yêu cầu người dùng: tính năng `Regen.PerTick.Trigger=4` trước giờ được
đặt tên/mô tả xoay quanh "ngồi" (`IS_SITTING`, `is_sitting()`,
`Gesture.SittingId`...), nhưng bản chất thật của nó là "hồi máu khi 1
gesture cụ thể đang active" - "ngồi" chỉ là **giá trị mặc định** của danh
sách gesture đó (10 gesture ngồi), không phải giới hạn cứng của tính năng.
Đặt tên theo đúng bản chất tổng quát hơn giúp rõ ràng hơn cho người dùng
muốn cấu hình gesture khác (không phải ngồi) cho `Trigger=4`.

Đổi tên (chỉ đổi tên/thuật ngữ, không đổi hành vi mặc định - vẫn hồi máu
khi 1 trong 10 gesture ngồi active, y hệt trước):

- Ini key: `[General] Gesture.SittingId` → **`[Regen Per Tick]
  Regen.PerTick.GestureId`** (dời hẳn qua đúng section `[Regen Per Tick]`
  vì gắn liền với `Trigger=4` ở đó, không còn tách riêng ở `[General]`
  nữa). Key cũ tự động được `config::migrate()` (cơ chế chung, so khớp với
  `AutoRegen.ini` template nhúng sẵn trong DLL - xem `lib.rs`) đẩy vào
  `[Legacy]` như mọi lần đổi key trước đây, không cần code migrate riêng.
- Comment `Regen.PerTick.Trigger=4` trong ini: "Only while sitting via a
  gesture" → "Only while a gesture from `Regen.PerTick.GestureId` below is
  active".
- Code (`regen.rs`): `IS_SITTING`→`IS_GESTURE_ACTIVE`,
  `is_sitting()`→`is_gesture_active()`, `PENDING_SIT_*`→`PENDING_GESTURE_*`,
  `confirm_pending_sit`→`confirm_pending_gesture`,
  `sit_gesture_ids()`→`gesture_trigger_ids()`,
  `DEFAULT_SIT_GESTURE_IDS`→`DEFAULT_GESTURE_IDS`,
  `SIT_CONFIRM_*_MS`→`GESTURE_CONFIRM_*_MS`. Log field `sitting=` trong dòng
  `Regen.PerTick:` đổi thành `gesture_active=`; log `Gesture: ... ->
  is_sitting=...` đổi thành `-> is_gesture_active=...`.

## Gộp `ActionSnapshot` với `sometweaks::player::NewActionPresses` vào `common::player` (2026-09-17)

Xóa hẳn struct `ActionSnapshot` + hàm `main_player_action_snapshot()` nội
bộ trong `regen.rs`, chuyển nguyên bản sang `common::player` (đổi tên
`common::player::ActionSnapshot`/`common::player::main_player_action_snapshot`).
Lý do: nhận ra `sometweaks::player::NewActionPresses` (4 field `r1`/`r2`/
`l1`/`l2`, dùng cho `Regen.PerHit.ExcludeAow`) chỉ là **tập con đúng y hệt**
của `ActionSnapshot` (cùng tên field, cùng đọc từ đúng 1 struct
`CSChrActionRequestModule` của game) - 2 mod đang tự đọc riêng cùng 1 dữ
liệu cho 2 mục đích khác nhau (phân loại đòn Skill/thường ở `sometweaks`;
idle/gesture detection ở `autoregen`), gộp làm 1 để không còn đọc trùng.

`sometweaks::player.rs` sau khi gộp **không còn field/hàm nào riêng nữa** -
xóa hẳn cả file, gọi thẳng `common::player::main_player_action_snapshot()`
rồi chỉ dùng `.r1`/`.r2`/`.l1`/`.l2` (bỏ qua `.new_gesture`/
`.requested_gesture`/`.busy` không cần). Không đổi hành vi gameplay ở cả 2
mod - build + release build (`build-mod.ps1`) xác nhận cho cả `AutoRegen`
lẫn `SomeTweaks`.

## Chuyển `show_announcement` sang `common::announce`, thêm banner cho `common::reload` (2026-09-17)

Xóa hẳn `show_announcement` nội bộ trong `regen.rs` (từng thêm ngày
2026-09-10, mục "Thông báo trong game khi bấm `ReloadKey`") - chuyển
nguyên bản (không đổi logic) sang `common::announce::show_announcement`.
Lý do: `dropmultiplier` muốn có banner xác nhận reload y hệt `AutoRegen`
nhưng chưa có cách nào dùng lại, vì hàm này trước đó chỉ là hàm riêng
(`fn`, không `pub`) trong `regen.rs`.

`AutoRegen` tự gọi `common::announce::show_announcement("AutoRegen: config
reloaded")` tại đúng chỗ cũ (tick loop của nó tự đọc `ReloadKey` riêng,
không qua `common::reload::run()`) - không đổi hành vi. Nhân tiện thêm
banner **"Config reloaded"** (chung, không ghi tên mod) vào chính
`common::reload::run()` - watcher `ReloadKey` dùng chung mà
`dropmultiplier`/`sometweaks`/`risearcher` đều gọi - nên cả 3 mod đó giờ
cũng tự động có banner reload, không cần tự thêm gì riêng.

## Nới `GESTURE_CONFIRM_TIMEOUT_MS` 3000ms → 6000ms - fix regen không hồi sau khi quay gấp trước khi ngồi (2026-09-22)

Kolagon báo trên Nexus: nếu chạy vòng cua hoặc quay gấp ngay trước khi dùng
gesture ngồi, regen thỉnh thoảng không hồi. Nguyên nhân: cơ chế xác nhận ở
`confirm_pending_gesture` (xem mục "Lần thử thứ 4" 2026-09-17) chờ
`current_anim_id()` đổi khác giá trị lúc bấm phím và giữ ổn định
`ANIM_STABLE_MS` (1000ms), rồi mới coi là ngồi thật. Nếu vừa quay gấp/đổi
hướng xong mới bấm gesture, animation xoay người/dừng lại vẫn đang chạy dở
lúc bấm phím - animation đó có thể mất hơn 3000ms để ổn định, nên
`GESTURE_CONFIRM_TIMEOUT_MS` cũ hết hạn trước khi animation ngồi kịp ổn
định, khiến xác nhận bị huỷ và `IS_GESTURE_ACTIVE` không bao giờ thành
`true` cho lần ngồi đó.

Đã nới `GESTURE_CONFIRM_TIMEOUT_MS` từ 3000ms lên 6000ms (giữ nguyên margin
tỉ lệ ~6x so với `ANIM_STABLE_MS` thay vì 3x cũ) để animation xoay/dừng có
đủ thời gian ổn định trước khi bị coi là timeout. Không ảnh hưởng độ trễ
của trường hợp ngồi bình thường (đứng yên rồi bấm) - vẫn xác nhận sau
~`ANIM_STABLE_MS` như cũ, chỉ nới hạn chót cho trường hợp animation chuyển
tiếp dài hơn bình thường.

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
"Write AutoRegen.log next to the DLL (for troubleshooting). Off = no log
file". Các mục cũ hơn trong README nói "file log luôn được tạo bất kể
`LogFile`" giờ đã lỗi thời.


## Đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini - sửa `common::dll_dir` (2026-09-24)

Người dùng (DoctorHigh, Nexus, load qua ME3) báo: sửa `AutoRegen.ini` thế
nào mod cũng dùng mặc định. Profile ME3 của họ nằm ở
`D:\Start\eld-MOD\重回巅峰MOD v1.17.1(2026.9.8 内测)`; Process Monitor cho
thấy game tìm `AutoRegen.ini` ở 1 đường dẫn bị lỗi font. Đây không phải
issue #296 của ME3 (config hardcode), mà là bug của chính mình.

Nguyên nhân: `dll_dir()` gọi `GetModuleFileNameA` - trả đường dẫn theo code
page ANSI của hệ thống (GBK trên máy tiếng Trung, `?` cho ký tự ngoài code
page trên máy Latin) - rồi lại giải mã bằng `String::from_utf8_lossy`. Mọi
tên thư mục không phải ASCII (tiếng Trung, cả chữ tiếng Việt có dấu như
`Tiếng Việt`) đều hỏng, không tìm thấy ini, nên âm thầm dùng mặc định
(file ini/log mặc định cũng không tạo được). Test của mình với ME3 không
lộ bug vì đường dẫn toàn ASCII. Đã tái hiện trên máy dev bằng 1 exe probe
gọi `dll_dir(0)` từ `D:\tmp\重回巅峰MOD 内测 Tiếng Việt\`: trước fix ra
`D:\tmp\????MOD ?? Ti?ng Vi?t`, sau fix ra đúng đường dẫn.

Sửa trong `shared/src/lib.rs`: đổi sang `GetModuleFileNameW` +
`String::from_utf16_lossy`, buffer bắt đầu từ 260 và tăng gấp đôi (tối đa
32768) khi bị cắt, nên đường dẫn dài cũng không bị cắt nữa.
`config`/`logger` đã dùng `std::fs` (Unicode sẵn) nên không cần đổi. Fix
áp dụng cho mọi mod dùng `common::dll_dir`.
