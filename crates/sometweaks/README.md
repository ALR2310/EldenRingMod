# SomeTweaks

Đổi tên từ **LifeBetween** (2026-08-18) khi gộp vào workspace
[`EldenRingMod`](../../README.md) - lý do đổi tên: tránh nhầm với tên file
`AutoRegen.ini`/`SomeTweaks.ini` chung namespace crate (`autoregen`,
`sometweaks`), tên cũ giữ nguyên trong lịch sử bên dưới vì vẫn phản ánh
đúng ý tưởng gốc của mod. Sống ở `crates/sometweaks` trong workspace, không
còn là repo Git riêng.

Dự định gộp các mod Quality-of-Life hiện có cho Elden Ring (AutoRegen,
RuneMultiplier, PassiveRunes, RiseArcher, ReductionWeight, DropRateTuner,
InventoryDeclutter, SpiritSummonMultiplier, QuestPath...) thành **1 DLL
duy nhất**, thay vì mỗi mod 1 file riêng.

Tên "LifeBetween" chơi chữ nhẹ với "Lands Between" (tên vùng đất trong
Elden Ring) + "Better Life" — đúng tinh thần: mod này không thêm cơ chế
gameplay mới, chỉ tinh chỉnh trải nghiệm (QoL).

## Trạng thái

Crate `cdylib` viết bằng Rust, build ra 1 DLL duy nhất (`SomeTweaks.dll`),
mỗi tính năng cũ là 1 module bật/tắt độc lập qua `SomeTweaks.ini`. Từ
2026-08-22, `[Regen Per Tick]`/`[Regen Per Hit]` mỗi mục có cờ `Enabled`
riêng (không còn chỉ dựa vào giá trị `0` để tắt).

**Đã triển khai:**

- `Regen` (hồi HP/FP/Stamina theo thời gian + theo đòn đánh trúng, cả
  `Regen.PerTick.*`/`Regen.PerHit.*` — **đã test trong game, hoạt động
  đúng**) — xem `src/regen/mod.rs` + `src/regen/attack_hook.rs`.
- `Rune Reward` (cộng rune theo thời gian + bonus mốc thời gian, port từ
  [`PassiveRunes`](../passiverunes) — **đã test trong game, hoạt động
  đúng**) — xem `src/rune/reward.rs`.
- `Rune.Multiplier` (`[Rune Reward]`, nhân hệ số rune nhận được từ mọi
  nguồn, port từ [`RuneMultiplier`](../runemultiplier) — **đã test trong
  game, hoạt động đúng**, xem `src/rune/multiplier.rs`) và
  `WeightMultiplier` (`[Misc]`, nhân hệ số Trọng Tải, ảnh hưởng cả số hiển
  thị lẫn roll-type thực tế, port từ
  [`WeightMultiplier`](../weightmultiplier) — **đã test trong game, hoạt
  động đúng**, xem `src/misc/weight_multiplier.rs`) — 2 hook nhỏ, độc lập
  nhau, tách file riêng theo section ini kể từ 2026-08-25 (xem mục
  "Restructure theo section ini" bên dưới; trước đó gộp chung vào
  `src/multipliers.rs`, xem mục 2026-08-24).
- `Drop Rate` (`DropRate.Multiplier`/`DropRate.ChancePercent`, chọn 1 trong
  2 qua `DropRate.Mode` - nhân hệ số hoặc ép cứng % tổng cộng "rớt được
  item gì đó" khi giết quái qua `ItemLotParam_enemy`, có hot-reload —
  **đã test trong game, hoạt động đúng** (cả `Mode=0` nhân hệ số và
  `Mode=1` ép cứng %, dùng chung `apply`/`scale_row` nên cùng đã xác nhận):
  `ChancePercent=100` → rớt cùng lúc toàn bộ ~6 item gắn với 1 con Godrick
  Soldier (đúng vì mỗi row lot của quái là 1 lượt roll độc lập, `100` ép
  từng row); `ChancePercent=1` → 2/3 lần giết có rớt (mẫu quá nhỏ để kết
  luận % chính xác, nhưng không có dấu hiệu sai) — xem
  `src/drop_rate/mod.rs`.
- `TorrentAnywhere` (`[Misc]`, mặc định `false` - cho phép cưỡi Torrent ở
  mọi nơi kể cả khu vực ép xuống ngựa như Abyssal Woods, port từ
  `.docs/torent_anywhere/CavaloLivre_V7_Cardoso.dll` - **đã test trong
  game, hoạt động đúng**) — xem `src/misc/torrent_anywhere.rs`.
- `Rune.KeepOnDeath` (`[Rune Reward]`, mặc định `false` - giữ nguyên rune
  hiện có khi chết thay vì bị đem vào vết máu - bản đầu dùng
  `ChrIns.has_dropped_runes` KHÔNG hoạt động (rune vẫn rơi bình thường), đã
  đổi sang kỹ thuật AOB-scan-and-NOP port từ `.docs/DisableRuneLoss.dll`
  (xem mục nhật ký bên dưới) - **đã test trong game, hoạt động đúng**) —
  xem `src/rune/keep_on_death.rs`.
- `UnlockAshesOfWar` (`[Misc]`, mặc định `false` - cho phép gắn Ash of War
  bất kỳ lên vũ khí bất kỳ (kể cả loại vốn không hỗ trợ AoW như
  cung/khiên/đuốc), port từ mod Nexus "Unlocked Ashes of War and
  Enchantments" (nexusmods.com/eldenring/mods/271) qua diff trực tiếp file
  CSV của mod đó - **đã test trong game, hoạt động đúng**) và
  `UnlockEnchantments` (`[Misc]`, mặc định `false` - cho phép yểm bùa chú
  (enchant) lên vũ khí bất kỳ, phép tạo hào quang gây thêm sát thương như
  Bloodflame Blade/Order's Blade/Scholar's Armament, KHÔNG phải
  affinity/thuộc tính vũ khí, tách riêng khỏi `UnlockAshesOfWar` vì 2 field
  khác nhau trong param, xem mục nhật ký bên dưới - **đã test trong game,
  hoạt động đúng**) — xem `src/misc/unlock_ashes_of_war.rs` +
  `src/misc/unlock_enchantments.rs`.
- `Spirit.Color` (`[Spirit]`, mặc định `false` - bỏ màu ma trên linh hồn
  triệu hồi, port từ `.docs/x10_summon/er10x.dll` nhưng đơn giản hơn - đọc
  trực tiếp SpEffect đang active trên `WorldChrMan.summon_buddy_chr_set`
  thay vì tự dò `NpcParam` tĩnh - **đã test trong game, hoạt động đúng**)
  — xem `src/spirit/color.rs`.
- `Spirit.Regen` (`[Spirit]`, mặc định `1` = 1%/giây, `0` tắt - hồi máu
  theo % HP tối đa mỗi giây cho linh hồn đang triệu hồi (đổi từ điểm cố
  định sang %, xem mục nhật ký bên dưới), cùng pattern `regen.rs` - **đã
  test trong game, hoạt động đúng**) — xem `src/spirit/regen.rs`.
- `Spirit.Summon.Anywhere` (`[Spirit]`, mặc định `false` - cho phép triệu
  hồi linh hồn không cần bia đá hồi sinh gần đó, port kỹ thuật gate-hook
  từ `er10x.dll`, đã xác nhận đúng AOB thật trong `eldenring.exe` qua
  Ghidra trước khi patch - **đã test trong game, hoạt động đúng**) — xem
  `src/spirit/summon_anywhere.rs`.
- `Spirit.Summon.Amount`/`Spirit.Summon.Multiplier` (`[Spirit]`, mặc định
  `0`/`1` = không đổi - số lượng linh hồn 1 Ash triệu hồi, cap 10, port
  "chain rebuild" của `er10x.dll` nhưng dùng thẳng
  `fromsoftware-rs`'s `ChainingMap`/`ChainingMapBucketEntry` type thay vì
  tự tính offset tay - `Spirit.Summon.Amount` **đã test, hoạt động đúng**;
  `Spirit.Summon.Multiplier` **đã test và lúc đầu SAI** (nhân dồn lên tới
  cap 10 thay vì đúng hệ số, do đọc "gốc" từ chain đã bị sửa ở lần rebuild
  trước thay vì snapshot thật - đã sửa, xem mục nhật ký bên dưới, **chưa
  test lại bản sửa**) — xem `src/spirit/summon_count.rs`.

**Đã chốt kiến trúc:**

- Dùng thư viện [`fromsoftware-rs`](https://github.com/vswarte/fromsoftware-rs)
  (crate `eldenring`) để đọc/ghi `WorldChrMan`/`ChrIns`/`CSChrDataModule`
  (HP/FP/Stamina) sống trong process game — không tự AOB-scan/offset tay
  cho phần này nữa. Lý do chọn Rust thay vì `libER` (C++): `libER` không có
  struct `ChrIns`/`PlayerIns` (không có field HP/FP/Stamina), còn
  `fromsoftware-rs` có đầy đủ.
- Phần **không** được thư viện cover (hook vào call site "hit trúng đòn"
  của game, gốc từ 1 CE table cộng đồng) vẫn tự viết tay bằng AOB scan +
  trampoline ASM (`global_asm!`) — không có lib nào phản chiếu (reflect)
  được call site này.
- Cấu hình đóng gói kiểu `RuneMultiplier`: `SomeTweaks.ini` được nhúng vào
  binary lúc build (`include_str!`), tự ghi lại ra đĩa nếu bị xoá. Phần
  đọc/ghi ini/log giờ dùng chung crate [`common`](../../shared) thay vì code
  riêng.

**Chưa làm:** RiseArcher (đọc/ghi `regulation.bin` sống qua `param_table`
của `fromsoftware-rs`/`libER`), các module còn lại (Spirit trong ini hiện
chỉ là placeholder). **Đã thử và bỏ:** cho phép dùng Site of Grace khi cưỡi
Torrent - cần viết EMEVD event mới, ngoài phạm vi kiến trúc hiện tại (xem
mục 2026-08-24 "Bỏ hẳn Misc.GraceOnTorrent").

## Fix đâm lén mất animation khi bật Regen.PerHit (2026-09-03)

Cùng bug và cùng fix vừa làm bên [`AutoRegen`](../autoregen) (xem README
của nó, mục cùng ngày, có phân tích chi tiết) - `attack_trampoline` ở
`src/regen/attack_hook.rs` dùng chung y hệt đoạn asm với AutoRegen, dính
đúng lỗi tương tự: hàm xử lý va chạm nhận tham số thứ 5 qua stack
(`[rsp+0x20]`, tính từ `hitInfo+0xd9==2`, quyết định có chạy khối xác định
đòn chí mạng/đâm lén hay không), trampoline chỉ forward 4 tham số qua
register nên `[rsp+0x20]` lúc gọi hàm thật là rác từ shadow space của chính
mình. Đâm lén ra animation đòn thường, tắt `Regen.PerHit.Enabled` không hết
lỗi (hook không gỡ patch), phải thoát game vào lại.

Fix: đọc `byte ptr [rsp+0x20]` vào `r10b` làm lệnh đầu tiên trong
trampoline (trước khi đụng `rsp`, vì patch bằng `jmp` nên `rsp` lúc đó vẫn
là giá trị code gốc vừa ghi), ghi lại đúng offset đó sau khi tạo shadow
space riêng, trước khi gọi hàm thật.

## Thêm `Regen.PerHit.ExcludeAow`, đổi key `Stamina` sang `SP` (2026-09-03)

Port từ [`AutoRegen`](../autoregen) (xem README của nó, mục cùng ngày, có
phân tích chi tiết) - cùng module `regen`:

- **`Regen.PerHit.ExcludeAow`**: loại trừ đòn Weapon Art/Ash of War khỏi
  Regen Per Hit (feature request từ Nexus: dùng phép/skill rẻ hồi FP cho
  skill đắt hơn). Thử ngưỡng `atkId` trước, bỏ sau khi đối chiếu toàn bộ
  ~11000 dòng `AtkParam` xuất từ SmithBox - không có field nào phân biệt
  được AoW với đòn thường cho mọi vũ khí. Chuyển sang đọc input pad thật:
  Elden Ring luôn kích hoạt Skill bằng `L2` bất kể tay nào đang chủ động -
  `src/player.rs` thêm `main_player_new_action_presses()`
  (`CSChrActionRequestModule.new_action_presses`), `regen/mod.rs` "chốt"
  (latch) `LAST_ATTACK_WAS_SKILL` mỗi frame theo lần bấm mới nhất (không
  đọc trạng thái tức thời, vì nút thường đã buông trước khi đòn muộn trong
  combo skill trúng).
- Đổi `Regen.PerTick.Stamina`/`Regen.PerHit.Stamina` → `.../SP` cho đồng bộ
  2 chữ cái với `HP`/`FP` - không tương thích ngược với key cũ.

## Xác nhận Rune.KeepOnDeath hoạt động, dọn code debug (2026-08-25)

Test lại bản AOB-scan-and-NOP (không phải bản `has_dropped_runes` cũ) -
**chết thử, rune không bị mất**. Trước đó có 1 khúc quanh: nghi ngờ patch
port có thể sai (vì đọc pseudocode `FUN_1800047f0` của
`DisableRuneLoss.dll` không hoàn toàn rõ ràng) nên đã thêm 1 chế độ debug
(`Rune.KeepOnDeath.DumpOnly`) để đọc byte sống từ `DisableRuneLoss.dll`
gốc chạy song song nhằm so sánh - nhưng chạy 2 DLL cùng lúc (`SomeTweaks.dll`
+ `natives\DisableRuneLoss.dll`, cả 2 đều active sẵn trong
`modengine2/config.toml`) gây crash (nghi do race `VirtualProtect` giữa 2
DLL trên cùng trang bộ nhớ). Hoá ra không cần bước debug đó - bản port ban
đầu đã đúng, chỉ là **chưa được test trực tiếp** trước khi đi vào nhánh
điều tra phức tạp. Đã xoá `dump_for_comparison()`/`hex_dump()`/key
`Rune.KeepOnDeath.DumpOnly` khỏi `keep_on_death.rs` và `SomeTweaks.ini` -
code hiện tại chỉ còn đúng phần patch chính thức.

## Pin fromsoftware-rs bản 0.14.0, thêm wait_for_system_init + panic safety (2026-08-26)

Áp dụng 3 cải thiện từ so sánh với `.docs/UltimatePassiveRegeneration`
(mod tham khảo có source code, không cần Ghidra):

1. **Đổi `eldenring`/`fromsoftware-shared` từ git HEAD sang version cố
   định `0.14.0`** (crates.io, bản mới nhất) trong `Cargo.toml` workspace
   gốc - build reproducible (ai build lại cũng ra cùng 1 mã nguồn upstream,
   không phụ thuộc git HEAD lúc clone). Đã verify trước khi đổi: `0.14.0`
   có đủ mọi struct `sometweaks` đang dùng (kể cả các field mới nhất như
   `SummonBuddyManager`/`ChainingMap` vừa thêm cho `[Spirit]`) - build lại
   toàn bộ workspace sạch, không phải sửa dòng code nào.

2. **Thêm `wait_for_system_init`** (`src/task.rs`) làm bước chờ đầu tiên
   trước `CSTaskImp::wait_for_instance` - hàm này đã có sẵn trong chính
   crate `eldenring` (chờ tín hiệu "process game đã sống" sớm nhất,
   `CSWindow`'s global hInstance, ngay sau CRT init), nhưng `sometweaks`
   trước giờ chưa từng gọi tới, luôn nhảy thẳng vào chờ `CSTaskImp`.

3. **Thêm `crate::task::run_recurring_safe`** - bọc mọi tick định kỳ bằng
   `std::panic::catch_unwind`, nếu 1 frame panic thì chỉ bỏ qua frame đó
   (log lại) thay vì crash cả DLL/game. Thay thế mọi lời gọi
   `cs_task.run_recurring(...)` trực tiếp trong 9 file
   (`regen/mod.rs`, `rune/reward.rs`, `rune/multiplier.rs`,
   `drop_rate/mod.rs`, `misc/weight_multiplier.rs`,
   `misc/torrent_anywhere.rs`, `spirit/color.rs`, `spirit/regen.rs`,
   `spirit/summon_count.rs`, `reload.rs`) - gom logic panic-catch vào 1 chỗ
   duy nhất thay vì lặp lại ở từng module.

   **Phát hiện quan trọng khi làm bước 3:** `Cargo.toml` gốc đang set
   `panic = "abort"` cho `[profile.release]` - với cấu hình này
   `catch_unwind` **hoàn toàn vô dụng ở bản release** (panic abort thẳng
   process, không unwind để catch). Đã bỏ dòng này (mặc định về
   `"unwind"`) để bước 3 thật sự có tác dụng ở bản release, không chỉ lúc
   `cargo build` (dev). Đánh đổi: binary release lớn hơn 1 chút (giữ lại
   unwind table), không đáng kể so với an toàn đạt được.

Build cả `cargo build` (workspace) và `cargo build --release -p sometweaks`
đều sạch, không cần sửa code nào khác ngoài các thay đổi trên.

## Thêm Spirit.Enabled - công tắc tổng cho [Spirit] (2026-08-26)

Người dùng tự thêm `Spirit.Enabled=true` vào ini nhưng chưa có code nào
đọc key này - 4 tính năng `[Spirit]` mỗi cái vẫn tự bật/tắt độc lập qua
cờ riêng. Thêm check `Spirit.Enabled` vào cả 4 module
(`color.rs`/`regen.rs`/`summon_anywhere.rs`/`summon_count.rs`) - `false`
thì bỏ qua toàn bộ nhóm bất kể cờ riêng từng cái là gì. `color.rs`/
`regen.rs`/`summon_count.rs` check lại mỗi tick (đổi được khi đang chơi,
có hot-reload); `summon_anywhere.rs` chỉ check lúc khởi động vì bản chất
là patch code 1 lần, không có tick riêng để re-check.

## Sửa Spirit.Summon.Multiplier nhân dồn (2026-08-26)

Test trong game: `Spirit.Color`, `Spirit.Regen`, `Spirit.Summon.Anywhere`,
`Spirit.Summon.Amount` đều hoạt động đúng. Riêng `Spirit.Summon.Multiplier`
sai: đặt `1 -> 2` (mong đợi x2 số lượng) nhưng ra 10 (kịch trần cap) thay
vì chỉ nhân đôi.

**Nguyên nhân:** `apply()` đọc "số lượng gốc" (`original_ids`) trực tiếp
từ nội dung chain **hiện tại** (`head.iter()`) mỗi lần chạy - nếu chain đó
đã bị rebuild ở lần trước, "hiện tại" đã là bản đã nhân rồi chứ không còn
là bản gốc thật của game. Với `Multiplier`, việc này gây **nhân dồn theo
cấp số nhân** mỗi khi tick chạy lại (1→2→4→8→10, dừng lại vì bị cap) - lý
do `Amount` không bị: nó luôn ép về đúng 1 số cố định bất kể input, tự ổn
định thay vì phóng đại. Đây là đúng loại lỗi `drop_rate.rs` đã gặp và sửa
trước đó (2026-08-24) mà lần này quên áp dụng lại.

**Đã sửa:** thêm `ORIGINAL_CHAIN_IDS` (`Mutex<HashMap<i32, Vec<i32>>>`,
key = SpEffect id) - snapshot nội dung chain thật lần đầu tiên gặp mỗi id
(trước khi có bất kỳ rebuild nào), mọi lần tính `target_count`/cycle ID
sau đó đều đọc từ snapshot này, không bao giờ đọc lại từ chain hiện tại -
cùng pattern `ORIGINAL_BASE_POINTS` của `drop_rate.rs`. **Chưa test lại
bản sửa.**

## Giải mã chain rebuild, thêm Spirit.Summon.Amount/Multiplier (2026-08-26)

Điều tra tiếp phần "chain rebuild" của `er10x.dll` (trước đó bỏ lại vì
chưa hiểu cơ chế). Trước khi bắt đầu, phát hiện file `er10x.dll` người
dùng đang có đã được cập nhật (17/8, project Ghidra cũ phân tích từ
14/8) - re-import sạch từ đầu, dump lại toàn bộ `Sig` struct để so sánh:
byte pattern thật (phần code, bỏ qua con trỏ nội bộ do relink) của
`kSigGate`/`kSigGateEntry` (dùng cho `Spirit.Summon.Anywhere`) **giống hệt
bản cũ**, danh sách toàn bộ ini key cũng không đổi - chỉ có 1 thay đổi nội
bộ: hàm `strip_ghost_colour` bị compiler inline thẳng vào `worker()`,
không ảnh hưởng gì tới các phần đã port.

**Đọc kỹ "chain rebuild"**: `er10x.dll` đọc `WorldChrMan+0x1e538` (con trỏ
tới summon buddy manager) rồi tự dò 1 cây nhị phân (byte `+0x19` làm cờ
"is nil" - đúng layout `_Tree_node` của MSVC `std::map`) để tìm entry ứng
với 1 "trigger id" (SpEffect kích hoạt triệu hồi), đọc con trỏ tại
node+0x28 rồi tự đi bộ 1 linked-list (`node.data` ở +0, `node.next` ở +8)
gom các "creature id", tính số lượng đích theo `count`/`multiply`, cấp
phát bộ nhớ riêng (không bao giờ free) rồi ghi đè con trỏ node+0x28 để
trỏ sang chain mới dài hơn, lặp lại cùng id theo modulo để đạt đủ số
lượng.

**Phát hiện quan trọng:** `fromsoftware-rs` đã có sẵn đúng cấu trúc này ở
dạng typed - `SummonBuddyManager.trigger_speffect_to_buddy_map:
ChainingMap<i32, i32>` (doc comment gốc: "Maps SpEffect IDs to BuddyParam
IDs... this is a chaining tree" - khớp 100% với những gì tự dò được thủ
công). `ChainingMapBucketEntry<T> { pub data: T, pub next: Option<NonNull<Self>> }`
đúng layout node 16-byte đã suy luận, và `iter_chains_mut()` cho thẳng
`&mut ChainingMapBucketEntry` vào head node của mỗi chain - đủ để nối
thêm node mới vào `head.next` mà **không cần tính offset tay hay đụng vào
field private của cây RB-tree** như `er10x.dll` phải làm (nó không có
type thật để dùng).

Thêm `Spirit.Summon.Amount`/`Spirit.Summon.Multiplier`
(`src/spirit/summon_count.rs`): với mỗi chain, đọc các id gốc qua
`head.iter()`, tính số lượng đích (`Amount` ưu tiên nếu >0, không thì
`Multiplier`, cap 10), nếu độ dài hiện tại đã đúng thì bỏ qua (idempotent),
khác thì `Box::leak` 1 mảng node mới (lặp lại id gốc theo modulo) rồi gán
vào `head.next` - giữ nguyên node đầu tiên của game, chỉ nối thêm phía
sau. Chạy trên tick `FrameBegin` (khác `er10x.dll` dùng thread riêng
độc lập + tự kiểm tra an toàn bằng `VirtualQuery`) để khớp quy ước cả
crate: mọi thứ đụng vào `WorldChrMan` sống chỉ an toàn từ thread chính
của game.

**Rủi ro:** đây là tính năng rủi ro cao nhất đã port trong `sometweaks` -
sửa trực tiếp con trỏ bên trong 1 cấu trúc dữ liệu sống (cây + linked-list)
của chính game, không phải chỉnh param tĩnh hay patch code. Nếu sai có
thể làm hỏng cấu trúc `SummonBuddyManager`, không chỉ riêng tính năng này
mà ảnh hưởng luôn hệ thống triệu hồi nói chung. **Chưa test trong game.**

## Triển khai [Spirit], giải mã er10x.dll (2026-08-25)

Giải mã `.docs/x10_summon/er10x.dll` ("10x Spirit Summons") - mod tham
khảo gốc cho phần `[Spirit]` đã để placeholder từ đầu. Khác các mod khác
đã gặp, DLL này build bằng MinGW nên **còn giữ tên hàm gốc** (không bị
strip), dễ đọc hơn hẳn - `hk_summon_gate`, `param_find_rows`,
`world_chr_man`, `band_occupancy`, `buddy_mgr`, `strip_ghost_colour`,
`sig_hit`... Kiến trúc rất chuyên nghiệp: tự tắt nếu phát hiện
EasyAntiCheat, kiểm tra version qua `GetFileVersionInfoA`, quét nhiều
signature trong 1 lượt duyệt `.text` (bitset-trie theo byte đầu), VEH bắt
lỗi truy cập trong vùng code của chính nó, watchdog thread theo dõi
worker có bị treo không.

**Giải mã đúng struct `Sig`** (dùng để lưu byte pattern của từng chữ ký):
đọc `sig_hit(Sig const&)`'s pseudocode xác định layout `pattern bytes ở
offset+8, độ dài ở offset+0x28` - từ đó tách được 2 pattern thật:
`kSigGate` (22 byte, kiểm tra "current slot count < max") và
`kSigGateEntry` (21 byte, prologue hàm gate). Xác nhận cả 2 khớp thật
trong `eldenring.exe` hiện tại qua Ghidra: `kSigGate` chỉ khớp đúng 1 chỗ
(unique) tại RVA `0x4b6e0d`; entry hàm thật nằm ở đúng
`match - 0x7D = 0x4b6d90` (khớp công thức `er10x.dll` tự dùng), và dùng
đúng global `0x143d65f88` + offset `+0x1e508` đã xác nhận độc lập từ đợt
điều tra `rune::keep_on_death` trước đó.

**Đã triển khai:**
- `Spirit.Summon.Anywhere` (`src/spirit/summon_anywhere.rs`): patch 15
  byte đầu hàm gate (`mov [rsp+8],rbx; mov [rsp+0x10],rsi; push rdi; sub
  rsp,0x20`, đúng độ dài `er10x.dll` tự cắt) bằng
  `common::codepatch::install_jmp_hook` - stub chỉ check đúng 1 field
  `er10x.dll` check (`*(i32*)(this+0x20) < 0`, "không có bia đá gần đó")
  rồi trả về thành công ngay, ngược lại replay lại 4 lệnh gốc rồi nhảy về
  code thật. Đơn giản hơn `er10x.dll` (không tự thay thế toàn bộ logic
  hàm gate) - **bỏ qua check "band busy" (đủ 10 slot chưa)** của
  `er10x.dll`, chấp nhận rủi ro nhỏ (request thừa bị driver engine tự bỏ
  qua vì giới hạn cứng 10 slot vẫn còn, không phải lỗi crash).
- `Spirit.Color` (`src/spirit/color.rs`): đơn giản hơn nhiều so với
  `er10x.dll` (không cần dò qua tới 194 bảng `NpcParam`) - `fromsoftware-rs`
  đã có sẵn `WorldChrMan.summon_buddy_chr_set.characters()` (linh hồn +
  Torrent đang active) và `ChrIns.special_effect.entries()` (SpEffect
  đang active) - chỉ cần lọc SpEffect trong khoảng `295000-295999` (dải
  hiệu ứng màu ma theo `er10x.ini`) rồi gọi `remove_speffect` có sẵn.
- `Spirit.Regen` (`src/spirit/regen.rs`): hồi máu theo % HP tối đa mỗi
  giây cho mọi linh hồn trong `summon_buddy_chr_set`, cùng pattern tick
  với `regen.rs`.

**Chưa làm:** `Spirit.Summon.Amount`/`Multiplier` - `er10x.dll`'s "chain
rebuild" (stage 3 trong `worker` thread của nó) không phải field param
tĩnh mà là 1 linked-list được dựng lại trong RAM lúc summon, cơ chế này
chưa được giải mã, cần điều tra riêng.

## Thêm hot-reload cho WeightMultiplier (2026-08-25)

Người dùng hỏi có thể thêm hot-reload cho `WeightMultiplier` không - trước
đó README/comment ghi rõ "no hot-reload" vì lý do "không có tick loop nào
sẵn để tận dụng". Nhưng cơ chế patch của `WeightMultiplier` giống hệt
`Rune.Multiplier`: stub được inject đọc `WEIGHT_FACTOR` qua 1 con trỏ mỗi
lần tính lại trọng tải, nên chỉ cần cập nhật giá trị atomic đó định kỳ là
có hot-reload ngay lập tức, không cần patch lại code lần nào nữa. Thêm tick
trên `CSTaskGroupIndex::FrameBegin` (dùng `crate::task::wait_for_cs_task`,
y hệt pattern `rune::multiplier` đã có) gọi lại `apply_weight_factor()`
(đổi tên từ `init_weight_factor`, giờ chỉ log khi giá trị thực sự đổi) mỗi
frame. Bỏ dòng comment "Applied once at startup (no hot-reload)" khỏi ini
cho `WeightMultiplier`.

## Test kết quả, sửa Rune.KeepOnDeath, tách UnlockEnchantments (2026-08-25)

Test trong game: `TorrentAnywhere` và `UnlockAshesOfWar` hoạt động đúng.
`Rune.KeepOnDeath` KHÔNG hoạt động - rune vẫn rơi bình thường khi chết.

**Nguyên nhân (suy luận):** `ChrIns.chr_flags1c6.has_dropped_runes()` -
theo comment gốc của `fromsoftware-rs`, "prevents dead character from
rewarding runes twice" - gần như chắc chắn là cờ dùng cho việc **NPC/quái
chết thì thưởng rune cho người giết nó** (field tồn tại trên MỌI `ChrIns`,
kể cả quái), không liên quan gì đến việc **rune người chơi đang giữ bị mất
khi chính họ chết** - đây là 2 hệ thống hoàn toàn khác nhau dù tên field
gây hiểu lầm là chung.

**Đã sửa:** bỏ cách tiếp cận field, chuyển sang đúng kỹ thuật đã port từ
`.docs/DisableRuneLoss.dll` (xem mục "Giải mã DisableRuneLoss.dll" bên
dưới) - AOB scan pattern `b0 01 ? 8b ? e8 ? ? ? ? ? 8b ? ? ? 32 c0 ? 83 ?
28 c3` đã xác nhận thật trong `eldenring.exe` (RVA `0x594f6c`), verify byte
tại offset+5 là `0xE8` rồi NOP 5 byte đó (xoá lệnh `CALL` chuyển rune vào
vết máu khi chết). `src/rune/keep_on_death.rs` viết lại hoàn toàn theo
kỹ thuật này, áp dụng 1 lần lúc khởi động (không hot-reload) - **chưa test
lại**.

Nhân tiện gộp `patch_bytes` (đang trùng lặp nếu `keep_on_death.rs` tự viết
lại) vào `common::codepatch::overwrite_bytes` dùng chung, `torrent_anywhere.rs`
cũng đổi sang gọi hàm này.

**Tách `UnlockAshesOfWar` thành 2 tính năng riêng:** người dùng làm rõ
`isEnhance` (trên `EquipParamWeapon`) là khả năng **yểm bùa chú lên vũ khí**
- phép tạo hào quang gây thêm sát thương như Bloodflame Blade, Order's
Blade, Scholar's Armament, đúng từ game gốc dùng là "enchant" (game báo
*"This weapon cannot be enchanted"* với vũ khí không hỗ trợ) - KHÔNG phải
affinity/thuộc tính vũ khí như tôi đoán ban đầu (2 khái niệm dễ nhầm trong
tiếng Anh), khác với việc **gắn Ash of War**
(`gemMountType` + `EquipParamGem`'s `canMountWep_*`). Chia lại:
- `UnlockAshesOfWar`: chỉ còn `EquipParamWeapon.gemMountType=2` +
  `EquipParamGem`'s `canMountWep_*` - xem `src/misc/unlock_ashes_of_war.rs`.
- `UnlockEnchantments` (mới): `EquipParamWeapon.isEnhance=1` - xem
  `src/misc/unlock_enchantments.rs`.

## Thêm UnlockAshesOfWar, port từ Nexus mod #271 qua diff CSV (2026-08-25)

Người dùng hỏi cách "unlock Ashes of War cho bất kỳ vũ khí nào" - lúc đầu
tôi đoán field cần sửa là `EQUIP_PARAM_WEAPON_ST::disableGemAttr` (dựa theo
tên field), nhưng người dùng đưa ra bản mod tham khảo thật:
[Unlocked Ashes of War and Enchantments](https://www.nexusmods.com/eldenring/mods/271)
(SmithBox project của mod này export sẵn 2 CSV, đã diff trực tiếp thay vì
đoán qua tên field):

- `EquipParamWeapon.csv`: **toàn bộ 3554/3554 hàng** (không có ngoại lệ,
  kể cả đạn) đều bị đổi `gemMountType=2`, `isEnhance=1`. `disableGemAttr`
  hoàn toàn KHÔNG bị đụng tới (357/3554 hàng vẫn giữ nguyên `disableGemAttr=1`)
  → chứng minh suy đoán ban đầu của tôi sai: `disableGemAttr` không phải
  cái khoá quyết định việc đổi AoW ở đá mài, `gemMountType`/`isEnhance`
  mới đúng.
- `EquipParamGem.csv` (bảng định nghĩa Ash of War, giữ tên "Gem" từ Dark
  Souls 3): toàn bộ 44 cột `canMountWep_*` (Dagger/SwordNormal/.../Bow/
  Staff/Shield/Torch/...) trên MỌI hàng đều bị set `1` - mọi AoW được coi
  là tương thích với mọi loại vũ khí, kể cả loại chưa từng hỗ trợ AoW
  trong bản gốc (cung, khiên, đuốc...).

Cả 2 param (`EquipParamWeapon`, `EquipParamGem`) đã có sẵn marker trong
`solo_params!` macro của `fromsoftware-rs`, y hệt cách `drop_rate.rs` dùng
`ItemLotParam_enemy` - port thẳng bằng typed API, không cần AOB. Thêm
`UnlockAshesOfWar` (`[Misc]`, mặc định `false`, áp dụng 1 lần lúc khởi
động, không hot-reload - revert lại ~3800 hàng không đáng công sức cache
gốc như `drop_rate` làm cho 1 field, restart game với `false` là quay lại
vanilla) — xem `src/misc/unlock_ashes_of_war.rs`.

Nhân tiện gộp `wait_for_repository` (chờ `SoloParamRepository` sẵn sàng +
người chơi đã vào world) từ `drop_rate/mod.rs` vào
`player::wait_for_solo_param_repository` dùng chung, vì giờ có 2 nơi cần
đúng logic này.

**Chưa xác nhận:** liệu game có animation/moveset thật cho AoW trên vũ khí
vốn không hỗ trợ (cung, khiên...) hay không - việc bỏ check tương thích
không tự tạo ra animation nếu engine chưa có sẵn. Cần test in-game.

## Giải mã DisableRuneLoss.dll, thêm Rune.KeepOnDeath (2026-08-25)

Giải mã `.docs/DisableRuneLoss.dll` bằng Ghidra headless (`DumpAll.java`)
để xem mod này chống mất rune khi chết bằng cách nào. Phát hiện:

- `DllMain` spawn 1 thread, quét AOB `b0 01 ? 8b ? e8 ? ? ? ? ? 8b ? ? ?
  32 c0 ? 83 ? 28 c3` trong `eldenring.exe`, verify byte tại offset+5 là
  `0xE8` rồi NOP 5 byte đó (xoá hẳn 1 lệnh `CALL`).
- Dùng project Ghidra có sẵn cho `eldenring.exe`
  (`D:/tmp/ghidra_eldenring`, script mới
  `.docs/reverse_engineering/DumpRuneLossAOB.java`) để tìm đúng vị trí thật
  trong game (RVA `0x594f6c`) và disassemble thật (không chỉ dựa vào
  pseudocode của DLL): lệnh `CALL` bị NOP gọi tới 1 hàm nhận `(manager,
  player_obj, true)`, hàm đó gọi tiếp nhiều hàm con rồi ghi 2-3 giá trị
  float liên tiếp vào 1 struct - khớp với việc ghi toạ độ 3D vào 1 bản ghi
  (nghi là bước tạo/ghi vết máu tại điểm chết).
- Tra `fromsoftware-rs` (`crates/eldenring/src/cs/chr_ins.rs:453-465`) thì
  thấy hàm đó gần như chắc chắn đọc đúng field mà crate đã có sẵn dạng
  typed: `ChrIns::chr_flags1c6.has_dropped_runes()` - doc comment gốc của
  crate: "Flags that prevents dead character from rewarding runes twice".

**Kết luận:** không cần port kỹ thuật AOB-scan-and-NOP của
`DisableRuneLoss.dll` - `fromsoftware-rs` đã expose đúng field cần dùng.
Thêm `Rune.KeepOnDeath` (`[Rune Reward]`, mặc định `false`): tick trên
`FrameBegin`, khi `chr_flags1c5.death_flag()` bật thì set luôn
`chr_flags1c6.set_has_dropped_runes(true)` (idempotent, set lại mỗi tick
trong lúc chết chứ không chỉ 1 lần) - làm game tưởng bước "đem rune vào vết
máu" đã chạy nên bỏ qua bước đó. Không AOB, không patch byte, không phụ
thuộc phiên bản game.

**Rủi ro chưa xác nhận:** đây là polling theo frame (`FrameBegin`), không
phải hook đồng bộ đúng thời điểm game đọc flag - nếu bước "đem rune đi"
chạy trong đúng frame `death_flag` vừa bật thì có thể trễ 1 frame. Elden
Ring có animation chết kéo dài vài giây trước khi thực sự trừ rune nên dự
đoán vẫn đủ thời gian, nhưng **chưa test trong game** - xem
`src/rune/keep_on_death.rs`.

## Restructure theo section ini + gom helper dùng chung (2026-08-25)

Sau khi thêm `TorrentAnywhere`, `src/` bắt đầu khó nhìn: `multipliers.rs`
gộp 2 tính năng không liên quan (`Rune.Multiplier` + `WeightMultiplier`)
trong 1 file qua `pub mod` lồng nhau, và mỗi feature module lại tự copy 2
đoạn code giống hệt nhau:

- `wait_for_cs_task()` (retry loop cho `SystemInitError::InvalidRva` khi
  `CSTaskImp::wait_for_instance` chưa sẵn sàng) - lặp lại y hệt ở
  `regen.rs`, `multipliers.rs`, `drop_rate.rs`, `torrent_anywhere.rs`.
- `wait_for_pattern`/`wait_for_anchor` (retry loop cho AOB scan) - lặp lại
  giữa `multipliers.rs` (weight_multiplier) và `torrent_anywhere.rs`.

Đồng thời `regen.rs` đang giữ 3 thứ **không phải riêng của Regen** mà các
module khác lại phụ thuộc vào: `main_player_chr_ins_ptr()` (dùng bởi
`rune_reward`, `drop_rate`), `RELOAD_GENERATION` (dùng bởi `drop_rate`), và
việc theo dõi `General.ReloadKey` (chạy trong chính tick của Regen). Điều
này biến Regen - vốn chỉ là 1 tính năng QoL độc lập - thành 1 dependency
ngầm cho toàn bộ crate.

**Đã đổi:**

1. Tách 3 utility dùng chung ra khỏi `regen.rs` thành module riêng ở top
   level (`src/`), không thuộc về feature nào:
   - `src/player.rs`: `main_player_chr_ins_ptr()`.
   - `src/reload.rs`: theo dõi `General.ReloadKey` + `RELOAD_GENERATION`,
     chạy trên thread riêng của chính nó (spawn từ `lib.rs`), không còn
     piggyback vào tick của Regen.
   - `src/task.rs`: `wait_for_cs_task(tag: &str)`, tham số `tag` để log vẫn
     phân biệt được module nào đang chờ.
   - `common::memscan::wait_for_pattern_in_module()` (ở
     [`shared`](../../shared), không phải `sometweaks`): gộp
     `wait_for_pattern`/`wait_for_anchor`. Không đặt `player.rs`/`task.rs`/
     `reload.rs` vào `shared` được vì crate đó cố tình **không** phụ thuộc
     `eldenring` (xem comment đầu `shared/Cargo.toml`) để giữ tính
     engine-agnostic - 3 module này cần `eldenring::cs::*` nên phải ở lại
     trong `sometweaks`. Muốn dùng chung cho cả các mod khác trong workspace
     (`autoregen`, `passiverunes`,...) thì cần 1 crate trung gian mới, chưa
     làm vì ngoài phạm vi lần này.
2. Tổ chức lại `src/` để mỗi section trong `SomeTweaks.ini` ứng với đúng 1
   folder (có `mod.rs`):
   - `src/regen/` (`mod.rs` + `attack_hook.rs`) - `[Regen Per Tick]`/
     `[Regen Per Hit]`.
   - `src/rune/` (`mod.rs`, `reward.rs`, `multiplier.rs`) - `[Rune Reward]`
     (gồm cả `Rune.Multiplier`).
   - `src/misc/` (`mod.rs`, `weight_multiplier.rs`, `torrent_anywhere.rs`) -
     `[Misc]`. Gộp theo section ini, không phải theo code dùng chung - 2
     module này vẫn không gọi vào nhau.
   - `src/drop_rate/mod.rs` - `[Drop Rate]`, không có submodule con nhưng
     đổi thành folder để đồng nhất quy ước với 3 nhóm trên.
3. Xoá `src/multipliers.rs`, `src/rune_reward.rs`, `src/torrent_anywhere.rs`
   (nội dung đã chuyển vào cấu trúc mới ở trên).

Hành vi runtime không đổi - đây thuần là refactor tổ chức code, đã build
lại toàn bộ workspace (`cargo build`) sạch, không warning.

## Thêm Misc.TorrentAnywhere, port từ CavaloLivre_V7_Cardoso.dll (2026-08-25)

Khác `GraceOnTorrent` đã bỏ, tính năng này **không cần EMEVD** - dịch
ngược (Ghidra headless full-analysis, DLL 311KB nên nhanh) 1 mod C++ tham
khảo (`.docs/torent_anywhere/CavaloLivre_V7_Cardoso.dll`, tên nghĩa là
"Ngựa Tự Do" tiếng Bồ Đào Nha) xác nhận toàn bộ 3 kỹ thuật của nó đều là
patch code/gọi lại hàm game - đúng phạm vi kiến trúc hiện tại:

1. **2 patch điều kiện khu vực** (`area_list_check`/`direct_ride_check`):
   cả 2 đều đọc `ptr = [this+0x68]`, check byte tại `[ptr+0x36] != 0`
   (`cmp+setne`) để quyết định "khu vực này có cấm cưỡi ngựa không". Không
   có field nào trong `fromsoftware-rs` khớp offset này (đã tra
   `ChrIns`/`PlayerIns`/`WorldChrMan`/`CSChrDataModule` và mọi struct liên
   quan ride/mount) nên vẫn phải patch code thô. Đọc kỹ byte patch thật từ
   Ghidra (không đoán): `cmp byte[ptr+0x36],0; setne al` (6 byte) đổi thành
   `mov byte[ptr+0x36],0; xor al,al` (cùng 6 byte) - vừa **xoá vĩnh viễn cờ
   cấm** trong bộ nhớ vừa ép kết quả trả về = "không cấm", cùng độ dài nên
   ghi đè tại chỗ, không cần kỹ thuật nhảy/stub như `weight_multiplier`.
2. **Bỏ qua ép xuống ngựa ở Abyssal Woods**: patch đúng 1 byte `74`→`EB`
   (`jz`→`jmp`) tại vị trí kiểm tra SpEffect `19995` ("Forced Torrent
   Dismount Abyssal Woods"), bỏ qua hẳn lệnh gọi ép dismount.
3. **Áp lại SpEffect `19996`** ("Remove Forced Torrent Dismount Abyssal
   Woods") mỗi giây - khác bản C++ (tự AOB tìm hàm `ApplyEffect` gốc của
   game rồi gọi qua con trỏ thô, chạy trên thread `Sleep` riêng),
   `fromsoftware-rs` đã có sẵn `ChrInsExt::apply_speffect` (resolve qua RVA
   table có sẵn trong crate, không cần tự AOB) - gọi trong tick
   `CSTaskImp::run_recurring` trên `FrameBegin`, đúng pattern
   `regen.rs`/ví dụ chính thức `examples/apply-speffect` của thư viện,
   không cần thread riêng.

Mặc định `Misc.TorrentAnywhere=false` (giống `DropRate` lúc đầu) - **chưa
test trong game**, đặc biệt kỹ thuật 1/2 patch code thô nên cần test cẩn
thận (không phải giữa 1 trận boss) trước khi bật mặc định `true`.

## Bỏ hẳn Misc.GraceOnTorrent; fix hot-reload của DropRate không hoạt động (2026-08-24)

**Bỏ hẳn `Misc.GraceOnTorrent`** (`src/torrent_grace.rs` đã xoá, bỏ khỏi
`lib.rs`/ini) sau khi test thực tế cho kết quả xấu hơn dự tính: site đã mở
+ cưỡi ngựa thì bấm không phản ứng gì; site chưa mở + cưỡi ngựa thì bấm
làm tự xuống ngựa rồi **kẹt luôn, site không mở được, không tương tác lại
được nữa**. Xác nhận đúng giả thuyết đã nêu ở mục "Tìm ra nguyên nhân crash"
bên dưới: chỉ xoá 2 bit gate `ActionButtonParam` là không đủ - logic EMEVD
event gắn với action ID `6100` có xử lý "đang cưỡi ngựa" riêng của nó,
không tương thích với việc bị kích hoạt khi đang cưỡi ngựa. Sửa đúng cách
cần viết/patch cả 1 EMEVD event mới (như `EldenConvenienceMod` đã làm) -
đòi hỏi công cụ đọc/ghi EMEVD mà workspace này không có, ngoài phạm vi
kiến trúc "patch bộ nhớ sống"/"sửa param" hiện tại. Không theo đuổi tiếp.

**Fix hot-reload của `DropRate` không hoạt động** (test `WeightMultiplier`
xong, đến `DropRate` thì phát hiện sửa ini xong bấm `ReloadKey` không có
tác dụng, phải thoát game vào lại mới áp dụng): nguyên nhân là
`eldenring::util::input::is_key_pressed` debounce phím bằng 1
`HashMap<VK code, Instant>` **dùng chung cho mọi lời gọi**, không phải
edge-detection riêng theo từng caller (`crates/eldenring/src/util/
input.rs`) - ai gọi hàm này với cùng phím trước trong cửa sổ 250ms thì
nhận `true`, người gọi sau (dù cùng 1 lần bấm phím thật) luôn nhận `false`.
`regen.rs` và `drop_rate.rs` cùng gọi `is_key_pressed(ReloadKey)` độc lập
mỗi tick - `regen.rs` luôn thắng (chạy trước), `drop_rate.rs` không bao
giờ thấy phím được nhấn.

Sửa bằng cách thêm `regen::RELOAD_GENERATION` (đếm số lần reload thành
công) - `regen.rs` là nơi **duy nhất** gọi `is_key_pressed`/`config::load`
cho `ReloadKey`, tăng bộ đếm này mỗi lần reload xong; `drop_rate.rs` chỉ
so sánh bộ đếm có đổi so với lần tick trước không, không tự gọi
`is_key_pressed` nữa. `Rune.Multiplier`/`WeightMultiplier` không dính lỗi
này vì chúng đọc lại config **mỗi tick vô điều kiện** (rẻ, không cần biết
"vừa mới reload hay chưa"), không giống `DropRate` cần biết chính xác thời
điểm reload để tránh duyệt lại toàn bộ ~5000 row mỗi frame.

## Tìm ra nguyên nhân crash: SoloParamRepository resolve sớm trước khi vào world (2026-08-24)

Sau khi fix `DropRate.Enabled=false` (mục bên dưới) để test cô lập đúng
cách: tắt `DropRate` → hết crash khi bật `Misc.GraceOnTorrent` một mình
vẫn crash, dừng ngay tại `repo.get_mut::<ActionButtonParam>(...)` (log:
`SoloParamRepository instance acquired, looking up ActionButtonParam
row...` rồi im bặt). Tắt cả 2 → hết crash hoàn toàn. Xác nhận: cả 2 module
đều crash độc lập, cùng 1 nguyên nhân gốc.

**Nguyên nhân**: `wait_for_repository()` của cả `drop_rate.rs` và
`torrent_grace.rs` chỉ chờ `SoloParamRepository::instance_mut()` trả về
`Ok` - nhưng đây **chỉ là object quản lý tồn tại**, không đảm bảo bảng
param bên trong (`ItemLotParam_enemy`, `ActionButtonParam`) đã load xong
dữ liệu thật. Y hệt lỗi đã gặp và sửa cho `rune_reward::add_runes`
(`GameDataMan` resolve sớm ở title/loading screen, trước khi player thật
sự vào world) - chỉ khác là lần này không phải "cộng rune sớm" (vô hại) mà
là **đọc/ghi vào bảng dữ liệu chưa tồn tại → truy cập bộ nhớ rác → crash
thật**.

**Fix**: cả 2 `wait_for_repository()` giờ chờ thêm điều kiện
`regen::main_player_chr_ins_ptr().is_some()` (đã vào world thật sự, cùng
cổng `Regen`/`Rune Reward` đã dùng) **trước khi** gọi
`SoloParamRepository::instance_mut()` - tăng luôn timeout từ 60s lên 300s
vì giờ phải chờ tới lúc người chơi thực sự chọn save và vào game, không
còn chỉ chờ 1 object khởi tạo sớm ở màn hình title. `drop_rate.rs`'s
reload-key closure cũng được thêm check này (trước đó gọi thẳng
`SoloParamRepository::instance_mut()` không qua `wait_for_repository`,
cùng lỗ hổng nếu bấm `F5` quá sớm).

## Fix DropRate.Enabled=false không thực sự tắt (2026-08-24, đang điều tra crash)

Game crash sau khi build+chạy bản có `Drop Rate`/`Misc.GraceOnTorrent` -
thử tắt cả 2 để cô lập nguyên nhân nhưng vẫn crash. Xem log mới thêm
(`DropRate: SoloParamRepository ready, snapshotting ItemLotParam_enemy...`)
mới phát hiện: `DropRate.Enabled=false` **không hề bỏ qua code** như tưởng
- `build_mode()` chỉ trả `Mode::Multiplier(1.0)` (nhân với 1 = không đổi
giá trị), nhưng `run()` vẫn gọi `wait_for_repository`/`apply` y hệt lúc
bật, tức vẫn snapshot + duyệt toàn bộ `ItemLotParam_enemy` dù tắt. Log dừng
đột ngột ngay tại bước snapshot (`repo.rows_mut::<ItemLotParam_enemy>()`)
- nghi ngờ chính là điểm crash, nhưng **chưa xác nhận được** vì việc "tắt"
trước đó không thực sự tắt gì cả.

Đã sửa `run()`: `DropRate.Enabled=false` giờ bỏ qua hẳn
`wait_for_repository`/`apply` lúc khởi động (không đụng
`SoloParamRepository` chút nào), chỉ còn đăng ký vòng lặp nghe
`ReloadKey` (để vẫn bật lại được sau nếu cần) - vòng lặp `Multiplier(1.0)`
trong `build_mode()` giữ nguyên, chỉ dùng cho trường hợp tắt **giữa
phiên** qua hot-reload (khi đó `apply()` đã chạy ít nhất 1 lần, cần hoàn
tác về snapshot gốc, khác với chưa từng chạy lần nào).

Đã thêm log chốt chặn ở `src/drop_rate.rs` (trước/sau snapshot) và
`src/torrent_grace.rs` (trước/sau lấy row, trước khi sửa bit) để lần chạy
tiếp theo (với fix tắt thật sự này) xác định được chính xác module nào
gây crash - **chưa kết luận được nguyên nhân gốc**, cần test lại.

## Đổi WeightMultiplier từ delay 5s cố định sang retry loop (2026-08-24)

`weight_multiplier`'s `install()` chỉ quét AOB **đúng 1 lần**, không có
vòng lặp thử lại như `wait_for_cs_task` của các module khác - trước đó bù
bằng cách chờ cố định 5 giây (`InitialDelaySeconds` port từ crate độc lập)
trước khi quét, với giả định 5s là đủ để game giải nén/relocate code
(anti-tamper) xong. Rủi ro giống hệt lỗi `InvalidRva` đã gặp (xem mục fix
2 module bên dưới): máy chậm hơn giả định là quét thất bại, tắt hẳn tính
năng cho cả phiên chơi, không có cách nào cứu lại.

Thay `std::thread::sleep(5s)` một lần bằng `wait_for_anchor()` (poll
`memscan::find_pattern_in_module` mỗi 500ms, tối đa 60s) - **nhanh hơn**
khi máy khoẻ (không cần chờ đủ 5s nếu game sẵn sàng sớm hơn) và **an toàn
hơn** khi máy chậm (chịu được tới 60s thay vì bỏ cuộc sau 5s). Không thêm
ini key nào - vẫn hardcode như delay cũ, chỉ đổi từ "chờ 1 lần" sang "thử
lại nhiều lần".

## Thêm Misc.GraceOnTorrent - ngồi Site of Grace không cần xuống ngựa (2026-08-24)

Xuất phát từ việc khám phá `.docs/EldenConvenienceMod` (tool C# của
thefifthmatt, sửa `regulation.bin`/EMEVD offline rồi đóng gói cho ModEngine
2 - kiến trúc hoàn toàn khác `sometweaks`, không port trực tiếp được).
Dịch code `Mods.cs` của nó ra được cơ chế thật: game chặn tương tác grace
khi cưỡi ngựa bằng **2 bit trong `ActionButtonParam`**, không phải EMEVD
event flag nào cả - `isInvalidForRide` (ẩn hẳn nút bấm khi cưỡi ngựa) và
`isGrayoutForRide` (hiện nhưng xám, không cho bấm).

Xác nhận bằng cách người dùng export `ActionButtonParam.csv` (SmithBox,
~400 row) và tra row `6100` ("Touch grace" - action ID chung cho mọi Site
of Grace): `isGrayoutForRide=1` (đúng là bit chặn), `isInvalidForRide=0`,
`overrideActionButtonIdForRide=-1` (**không có row nào khác thay thế khi
cưỡi ngựa** - sửa thẳng row 6100 là đủ, không cần lần theo row khác).

May mắn `fromsoftware-rs` đã tự decode sẵn bitfield này thành accessor có
tên (`is_grayout_for_ride()`/`set_is_grayout_for_ride(bool)`,
`is_invalid_for_ride()`/`set_is_invalid_for_ride(bool)`) - không cần tự
thao tác bit thủ công như dự tính ban đầu.

**Khác biệt so với `EldenConvenienceMod`**: họ phải **clone** row 6100
sang 1 action ID mới toanh rồi tự viết cả 1 EMEVD event mới (vì ID mới
chưa có event logic nào lắng nghe) - lý do họ clone thay vì sửa thẳng có
lẽ là để đồng thời tăng `radius` (1.5→3) mà không ảnh hưởng hành vi
touch-grace bình thường lúc không cưỡi ngựa. `src/torrent_grace.rs` sửa
**thẳng row gốc `6100`** - event logic vanilla đã lắng nghe sẵn ID này,
nên chỉ cần đổi 2 bit, không cần viết event nào cả, đơn giản hơn nhiều.

Chạy 1 lần lúc khởi động, giống `risearcher` - row `ActionButtonParam` chỉ
load 1 lần lúc vào game, không đổi giữa phiên chơi, nên không cần
hot-reload/tick loop nào.

`RuneMultiplier` (đổi tên thành `Rune.Multiplier`) chuyển từ `[General]`
vào `[Rune Reward]` - đứng cùng nhóm với `Rune.Passive.*`/`Rune.Milestone`
thay vì nằm rời rạc ở đầu file. `WeightMultiplier` (giữ nguyên tên) chuyển
sang section `[Misc]` mới tạo - hiện chỉ có mình nó, nhưng để sẵn chỗ cho
các tweak lặt vặt khác sau này (`TorrentAnywhere` đang là placeholder cũng
là ứng viên chuyển vào đây). Cập nhật `src/multipliers.rs` đọc đúng key
`Rune.Multiplier` mới (`WeightMultiplier` không đổi tên nên không cần sửa
code, chỉ đổi vị trí trong ini).

`WeightMultiplier` và `Drop Rate` (`DropRate.*`) **chưa được test trong
game** - cả 2 đều sửa dữ liệu/patch code sống (`WeightMultiplier` patch
ASM, `Drop Rate` sửa `ItemLotParam_enemy`), cần verify thực tế trước khi
xem là hoàn thiện.

## Đã chốt (nhưng chưa triển khai)

- **RiseArcher** sẽ chuyển từ sửa tĩnh `regulation.bin` (`.MASSEDIT`) sang
  đọc/ghi param sống bằng `libER`, hệ số chỉnh qua ini thay vì hardcode —
  lý do: cho người dùng tự chỉnh số theo ý thích, và tránh xung đột file
  `regulation.bin` khi cài chung với mod khác. Xem chi tiết ở
  `d:\MyProjects\EldenRing\RiseArcher\README.md` (mục "Quyết định: sẽ
  chuyển sang DLL + libER") - RiseArcher chưa được đưa vào workspace này.

## Dịch toàn bộ comment trong SomeTweaks.ini sang tiếng Anh (2026-08-24)

Toàn bộ comment trong `SomeTweaks.ini` (trước đó bằng tiếng Việt) đã được
dịch sang tiếng Anh và rút gọn - không đổi bất kỳ key/section/giá trị nào,
chỉ đổi ngôn ngữ và văn phong mô tả.

## Gom Drop Rate vào 1 section riêng, chọn chế độ tường minh thay vì ngầm định (2026-08-24)

`DropRateMultiplier`/`DropChancePercent` ban đầu nằm rời rạc trong
`[General]`, và chế độ nào thắng được quyết định ngầm ("`DropChancePercent
> 0` thì dùng nó, bỏ qua `DropRateMultiplier`") - dễ gây bật nhầm cả 2
cùng lúc mà không nhận ra. Đã gom thành 1 section `[Drop Rate]` với 4 key
`DropRate.Enabled`/`DropRate.Mode`/`DropRate.Multiplier`/
`DropRate.ChancePercent`, trong đó `DropRate.Mode` (0/1) **chọn tường
minh** đúng 1 trong 2 chế độ - không còn khả năng "cả 2 cùng bật" xảy ra
được nữa, vì `build_mode()` chỉ đọc giá trị của chế độ đang được `Mode`
chọn, giá trị kia bị bỏ qua hoàn toàn dù có đặt gì cũng không ảnh hưởng.

`DropRate.Enabled=false` không chỉ "bỏ qua không làm gì" (như
`Regen.PerTick.Enabled=false`) mà chủ động trả `Mode::Multiplier(1.0)` -
tức là **ghi lại đúng giá trị gốc từ snapshot** để hoàn tác mọi lần sửa
trước đó, vì khác `Regen` (tính lại từ đầu mỗi tick, "tắt" tự nhiên = "không
tính"), `drop_rate.rs` sửa dữ liệu param sống - nếu chỉ "bỏ qua" mà không
ghi lại gốc, giá trị đã bị nhân/ép % từ trước vẫn còn nguyên trong bộ nhớ
game dù đã tắt tính năng qua `ReloadKey`.

**Khác biệt triết lý**: `DropRateMultiplier` nhân đều hệ số lên mọi item -
item vốn hiếm vẫn hiếm hơn item vốn thường sau khi nhân (giữ đúng tỉ lệ
tương đối gốc của game). `DropChancePercent` thì **san bằng** - ép **tổng
% "rớt được item gì đó"** của mọi row về đúng 1 con số cố định, bất kể quái
đó vốn dễ hay khó rớt đồ. Nếu 1 row có nhiều item cạnh tranh nhau, tỉ lệ
**giữa các item đó với nhau** vẫn giữ nguyên - chỉ tổng của cả nhóm bị ép.

**Công thức** (áp riêng cho từng row, không cần biết item ID nào): gọi
`itemSum`=tổng weight các slot có item thật, `otherSum`=weight slot "không
rơi gì" (giữ nguyên) - giải phương trình `target = itemSum_new / (itemSum_new
+ otherSum)` ra `itemSum_new = target × otherSum / (1 - target)`, rồi nhân
**mỗi item trong row** với hệ số `itemSum_new / itemSum` (bảo toàn tỉ lệ
tương đối giữa các item, chỉ đổi tổng của cả nhóm).

2 trường hợp biên phải xử lý riêng:
- `target ≥ 100%`: công thức trên chia cho 0 - xử lý bằng cách **đưa hẳn
  weight của slot "không rơi gì" về 0** thay vì cố tính ra 1 "weight vô
  cực" cho item - cách này chắc chắn luôn rớt được gì đó, không cần đụng
  đến weight của item.
- Row không có slot "không rơi gì" nào cả (`otherSum=0`, quái vốn dĩ luôn
  chắc chắn rớt đồ, giữa 5135 row thật đã kiểm tra - chưa xác nhận có row
  nào rơi đúng ca này, nhưng code vẫn cần chống chịu): không thể giảm %
  xuống dưới 100% được nữa (không có chỗ nào để "trả" phần % bớt đi vào) -
  bỏ qua row đó, giữ nguyên.

## Thêm module DropRateMultiplier qua ItemLotParam (2026-08-24)

Kích hoạt `DropRateMultiplier` trong `[General]` (trước đó chỉ là dòng
comment placeholder mô tả sai cơ chế - xem lịch sử điều tra bên dưới),
triển khai `src/drop_rate.rs`. Khác hẳn `RuneMultiplier`/`WeightMultiplier`
- không patch code/AOB, chỉ sửa dữ liệu param sống qua
`SoloParamRepository` (đúng pattern `risearcher/src/weapon.rs` đã dùng cho
`EquipParamWeapon`), nên không có rủi ro crash do ghi đè lệnh CPU.

**Quá trình điều tra cơ chế thật** (trước khi chốt cách làm), vì ini gốc
ghi "hệ số nhân tỉ lệ rơi" nhưng không rõ cơ chế game thật sự hoạt động ra
sao:

1. Dịch ngược `Zibinha_DropRate_AOB_V2_SAFE.dll` (`.docs/droprate/`, Ghidra
   headless full-analysis - file chỉ 14KB nên nhanh) để tham khảo 1 mod
   cộng đồng có sẵn cho cùng tính năng. Tìm ra AOB `41 0F 28 F8 48 85 D2`
   (`movaps xmm7,xmm8; test rdx,rdx`), patch bằng kỹ thuật JMP tương đối +
   NOP đệm giống hệt `WeightMultiplier`.
2. Dò tiếp AOB đó trong `eldenring.exe` thật (Ghidra headless `-noanalysis`
   + `mem.findBytes`, script `DumpDropRateAOB.java` giữ lại trong
   `.docs/reverse_engineering/`) - chỉ 1 match tại RVA `0x68652c`. Giải mã
   ra: DLL này **cộng thẳng** 1 hằng số cấu hình vào 1 giá trị trung gian
   trong công thức tính "discovery-bonus" (`result = max(0, statBonus +
   hookedValue)`) - **không phải phép nhân**, ini key `rate` của nó là số
   điểm cộng thêm, không phải hệ số.
3. Vì mục tiêu là 1 phép nhân **đúng nghĩa**, chuyển hướng sang
   `ITEMLOT_PARAM_ST` (đã có sẵn dạng typed field trong `fromsoftware-rs`,
   đăng ký trong `SoloParamRepository` dưới tên `ItemLotParam_enemy`/
   `ItemLotParam_map`). Cần xác nhận thêm 1 điều trước khi code: mẫu số
   dùng để roll là gì. Tra cứu Souls Modding Wiki (`ItemLotParam` page) +
   dự án `SoulsRandomizers` của thefifthmatt xác nhận: **mẫu số là tổng
   trọng số của chính row đó** (không phải hằng số cố định toàn cục) -
   nhân đều cả 8 slot sẽ **vô tác dụng** vì tỉ lệ giữa các slot không đổi.
4. **[Đã sửa]** Bước 3 ở trên còn 2 chỗ sai, phát hiện được nhờ người dùng
   tự export `ItemLotParam_enemy.csv` bằng SmithBox và đối chiếu số liệu
   thật (xem mục sửa lỗi ngay bên dưới) - dùng đúng lượt trao đổi trong
   phiên làm việc thay vì phải tự dò lại `eldenring.exe`.

## Sửa 2 lỗi trong DropRateMultiplier sau khi đối chiếu dữ liệu thật (2026-08-24)

Người dùng export `ItemLotParam_enemy.csv` qua SmithBox (công cụ sửa param
cộng đồng) và so khớp % hiển thị của SmithBox với công thức đã dùng, phát
hiện 2 điểm sai trong bản đầu tiên của `src/drop_rate.rs`:

1. **Slot "không rơi gì" không dùng `category=-1`** như Souls Modding Wiki
   mô tả (có thể đúng cho Dark Souls 1, không khớp Elden Ring) - dữ liệu
   thật cho thấy slot rỗng có `lotItemCategory=0` ("None"), `lotItemId=0`.
   Cách nhận diện đúng và đơn giản hơn: check thẳng **`lot_item_id0N == 0`**
   (slot rỗng luôn có id=0, bất kể category gì) thay vì so category với
   `-1`. Đã sửa `scale_row`/`apply` dùng `item_ids` thay vì `categories`.
2. **`cumulate_lot_point0N` không cần (và không nên) ghi lại** - dữ liệu
   thật xuất ra cho thấy field này **luôn là `0`** ở mọi slot của mọi row
   mẫu, và chính SmithBox cũng tính "% chance to occur" hiển thị của nó
   **chỉ từ `lot_item_base_point`**, không đọc `cumulate_lot_point` -
   chứng tỏ giả định "game đọc cumulate đã tính sẵn, không tự cộng dồn
   sống" (dựa theo tài liệu game đời cũ) **không đúng với Elden Ring**.
   Đã bỏ hẳn bước tính lại `cumulate_lot_point` (`set_cumulate_points` đã
   xoá) - chỉ còn sửa `lot_item_base_point0N`, để nguyên `cumulate_lot_point`
   như dữ liệu gốc (`0`) - tránh đưa game vào trạng thái chưa từng được
   test (giá trị `cumulate_lot_point` khác `0` không tồn tại trong bất kỳ
   bản `regulation.bin` gốc nào).

Đối chiếu số liệu thật xác nhận công thức `chance_i = weight_i / tổng
weight cả row` là đúng: row `985/15` cho `98.5%/1.5%` (khớp SmithBox);
sửa slot phụ từ `15→30` cho `30/1015=2.96%` và `985/1015=97.04%` (khớp
chính xác số SmithBox hiển thị, kể cả việc % slot còn lại **giảm** - đúng
bản chất mô hình trọng số tương đối, không phải bug).

**Thiết kế cuối cùng** (`src/drop_rate.rs`):

- Chỉ nhân `lot_item_base_point0N` của các slot có `lot_item_id0N != 0`
  (slot có item thật), giữ nguyên slot "không rơi gì" nếu có - đúng ý
  "tăng tỉ lệ rơi", không phải "xáo trộn vô nghĩa tỉ lệ giữa các item".
- Không đụng đến `cumulate_lot_point0N` - giữ nguyên giá trị gốc (xem mục
  sửa lỗi ở trên).
- **Có hot-reload** (khác `risearcher` không hề hỗ trợ, vì `regulation.bin`
  của nó chỉ áp 1 lần) - do người dùng muốn tinh chỉnh nhanh khi test. Để
  tránh compounding (nhân 2 lần liên tiếp thành x4 thay vì giữ x2), lưu lại
  **snapshot giá trị gốc** (`static Mutex<HashMap<u32, [u16; 8]>>`, chụp 1
  lần duy nhất trước khi sửa gì cả) - mỗi lần bấm `ReloadKey`, luôn tính lại
  từ snapshot gốc × hệ số hiện tại, không tính chồng lên giá trị đang sống.
- Không tự đọc lại `SomeTweaks.ini` khi bấm phím (dựa vào `regen.rs` đã lo
  việc đó, cùng cơ chế `rune_multiplier`/`weight_multiplier` đang dùng) -
  chỉ tự poll `ReloadKey` để biết lúc nào cần chạy lại vòng lặp áp hệ số
  (đắt hơn 1 chút vì duyệt cả bảng `ItemLotParam_enemy`, nên phải gate
  đúng lúc bấm phím, không chạy mỗi tick như các module khác).
- Hệ số `DropRateMultiplier=2` chỉ **gần đúng** thành "tăng gấp đôi %",
  không tuyệt đối chính xác - vì công thức `newChance = 2w/(S+w)` (w=trọng
  số gốc của item, S=tổng row) chỉ tiệm cận đúng 2× khi item càng hiếm
  (`w` càng nhỏ so với `S`). Chấp nhận sai số này vì đúng bản chất cơ chế
  game, không có cách nào cho ra kết quả tuyệt đối chính xác mà vẫn giữ
  đúng nghĩa "nhân hệ số" (xem lịch sử trao đổi quyết định giữ tên
  `DropRateMultiplier` thay vì đổi sang kiểu "cộng điểm" của Zibinha).

## Đổi regen/mod.rs thành regen.rs (2026-08-24)

`src/regen/mod.rs` → `src/regen.rs`, giữ nguyên `src/regen/attack_hook.rs`
tại chỗ - dùng convention module edition 2018+ của Rust (file `<tên>.rs`
đặt ngang hàng thư mục `<tên>/` chứa submodule, thay cho `<tên>/mod.rs`
kiểu edition 2015). Lý do: tên file `mod.rs` không tự mô tả được nội dung
- mở nhiều file `mod.rs` từ nhiều module khác nhau (kể cả khác crate) cùng
lúc trong editor khiến tab khó phân biệt hơn hẳn `regen.rs`. Không gộp
`attack_hook.rs` vào cùng file hay đổi tên nó, và không bỏ thư mục con
`regen/` (giữ thư mục để tránh trùng tên `attack_hook.rs` với
`crates/autoregen/src/attack_hook.rs` nếu làm phẳng hoàn toàn) - chỉ đổi
đúng 1 việc: tên file `mod.rs`.

## Thêm `Regen.PerHit.DamageType` - tùy chọn tính cả đòn phép (2026-08-25)

`Regen Per Hit` từ trước tới nay luôn lọc bỏ đòn từ phép/đạn
(`sourceType==3`), chỉ tính đòn vũ khí cận chiến (`sourceType==1`) - tránh
việc spam phép rẻ cũng được thưởng hồi máu như đánh cận chiến. Có người
dùng (phản hồi trên Nexus, bên [`AutoRegen`](../autoregen) - cùng module
`regen`) muốn dùng chính cơ chế này để "cycle" FP: cast phép rẻ để hồi FP,
dùng FP đó cast phép đắt hơn - việc này không làm được vì bộ lọc luôn loại
phép ra.

Thêm key `Regen.PerHit.DamageType` (0/1/2) để chọn loại sát thương nào
được tính, độc lập với `Trigger`:

- `0` (mặc định, giữ hành vi cũ): chỉ đòn cận chiến.
- `1`: chỉ đòn tầm xa/phép.
- `2`: cả hai.

Đây là 1 điều kiện chung cho cả 3 stat (Hp/Fp/Stamina), không tách riêng
"chỉ Fp hồi từ phép" - đơn giản hơn, đủ dùng cho use-case "cast rẻ hồi Fp
để cast đắt" nếu người dùng chỉ cấu hình `Regen.PerHit.FP` (để `HP`/
`Stamina` = 0) và đặt `DamageType=1` hoặc `2`. Đổi cùng lúc và cùng cách
với `AutoRegen` (xem README của mod đó) để 2 mod tiếp tục dùng chung thiết
kế: `is_weapon_damage_hit()` cũ đổi tên thành `matches_damage_type()`,
nhận thêm tham số `damage_type` thay vì hard-code chỉ chấp nhận
`sourceType==1`.

## Gộp RuneMultiplier + WeightMultiplier vào 1 file, thống nhất tên module (2026-08-24)

`src/rune_multiplier.rs` và `src/weight.rs` ban đầu là 2 file riêng - gộp
lại thành `src/multipliers.rs` (2 module con `pub mod rune_multiplier`/
`pub mod weight_multiplier`) vì cả 2 đều là hook 1-lần rất nhỏ, không chia
sẻ code với nhau, và việc tách file riêng chỉ thêm ceremony không cần
thiết cho quy mô này. Đồng thời đổi tên cho nhất quán theo 1 quy ước duy
nhất `<tên>_multiplier` thay vì lẫn lộn `rune_multiplier`/`weight` (module
`weight` ban đầu, trước khi gộp, chưa có hậu tố `_multiplier`) - và đổi
luôn `src/rune.rs` (Rune Reward, module `rune`) thành `src/rune_reward.rs`
(module `rune_reward`) để không còn dễ nhầm với `rune_multiplier`. `lib.rs`
giờ gọi qua `rune_reward::run`/`multipliers::rune_multiplier::run`/
`multipliers::weight_multiplier::run`.

## Thêm module WeightMultiplier, port từ WeightMultiplier crate (2026-08-24)

Kích hoạt `WeightMultiplier` trong `[General]` (trước đó chỉ là dòng
comment placeholder), triển khai `src/multipliers.rs`'s `weight_multiplier` module
port 1:1 kỹ thuật hook của
[`WeightMultiplier`](../weightmultiplier)'s `src/hook.rs`: patch 7 byte lệnh
`movaps xmm0,xmm6` (đúng thời điểm game vừa cộng xong tổng Trọng Tải, chạy
1 lần duy nhất, không dồn qua vòng lặp 5 slot đồ) để nhân hệ số vào `xmm6`
trước khi copy ra kết quả - xem README của crate `WeightMultiplier` cho
toàn bộ lịch sử tìm offset (3 lần thử, xác nhận chéo qua 1 DLL "NoWeight"
cộng đồng).

Khác biệt so với bản crate độc lập:

- Patch không đủ chỗ cho kỹ thuật absolute-jump 12-byte của
  `attack_hook.rs`/module `rune_multiplier` (chỉ 7 byte) - dùng
  `common::codepatch::install_jmp_hook` (JMP tương đối 5 byte + tìm vùng
  nhớ gần bằng `VirtualAlloc`) thay vì viết tay riêng.
- Giữ nguyên tên key `WeightMultiplier` (hệ số nhân trực tiếp, `1` = không
  đổi) - `SomeTweaks.ini` đã có sẵn key này từ trước, không cần đổi thành
  `WeightReductionPercent` (0-100%) như bản crate độc lập.
- Không có `InitialDelaySeconds` riêng trong ini - `SomeTweaks` chưa có
  module nào khác cần delay khởi động nên chưa đáng thêm 1 key ini chỉ cho
  module này. Ban đầu port y hệt delay 5s cố định của bản crate độc lập,
  sau đổi sang retry loop (`wait_for_anchor`) - xem mục ngày hôm nay bên
  dưới.
- Cố tình không có hotkey reload, giống nguyên bản - đổi `WeightMultiplier`
  cần khởi động lại game.

## Thêm module RuneMultiplier, port từ RuneMultiplier crate (2026-08-24)

Kích hoạt `RuneMultiplier` trong `[General]` (trước đó chỉ là dòng comment
placeholder), triển khai `src/multipliers.rs`'s `rune_multiplier` module
port 1:1 kỹ thuật hook của [`RuneMultiplier`](../runemultiplier)'s
`src/hook.rs`: patch 12 byte đầu
`AddSoul_Call` (hàm cộng-rune cấp thấp nhất của game, dùng chung mọi
nguồn) bằng redirect `mov rax,<stub>; jmp rax` sang 1 stub tự sinh nhân
`EDX` (amount) theo fixed-point Q20 integer, chỉ nhân khi `amount > 0` để
không ảnh hưởng rune **chi tiêu** (lên cấp, mua đồ) - xem README của
`RuneMultiplier` crate cho toàn bộ lịch sử dịch ngược/quyết định kỹ thuật.

Khác biệt so với bản crate độc lập:

- Giữ nguyên tên key `RuneMultiplier` (dưới `[General]`, không tách section
  `[Settings]` riêng) thay vì đổi thành `Multiplier` như bản
  `RuneMultiplier` đã làm ngày 2026-08-19 - `SomeTweaks.ini` đã có sẵn key
  này từ trước, không cần đổi tên lần nữa.
- Không tự poll `General.ReloadKey` - đọc lại `RuneMultiplier` mỗi tick
  (`apply_multiplier()`, chỉ log khi giá trị thực sự đổi) thay vì so khớp
  hotkey riêng, vì `regen::run` đã lo việc reload `SomeTweaks.ini` dùng
  chung config map cho cả module này (cùng cách `rune::run` đã làm).
- Hook install không phụ thuộc `WorldChrMan`/in-world (không dereference
  con trỏ player nào), nhưng vòng lặp áp dụng multiplier mỗi tick vẫn cần
  `CSTaskImp` nên áp dụng luôn fix `wait_for_cs_task` (retry `InvalidRva`)
  ở mục ngay bên dưới.

## Fix cả 2 module bị vô hiệu hoá vĩnh viễn bởi InvalidRva (2026-08-24)

Cùng lỗi mà [`PassiveRunes`](../passiverunes) gặp (xem README của nó, mục
2026-08-24): `CSTaskImp::wait_for_instance` coi `SystemInitError::InvalidRva`
là lỗi chết ngay, không tự retry dù truyền `Duration::MAX` (chỉ tự retry
case `Null` bên trong nó) - `InvalidRva` xảy ra khi RVA lookup chạy trước
lúc game giải nén/relocate xong (Arxan), tức là 1 cuộc đua timing với lúc
thread của DLL này khởi động, không liên quan gì đến việc DLL đặt ở đâu.
Log thực tế: cả `regen::run` lẫn `rune_reward::run` đều dính lỗi này cùng
lúc và tắt hẳn tính năng cho session đó. Đã thêm `wait_for_cs_task()`
(retry sau 1s thay vì bỏ cuộc ngay) vào cả `src/regen.rs` (khi đó còn tên
`src/regen/mod.rs`) và `src/rune_reward.rs` (khi đó còn tên `src/rune.rs`,
xem mục đổi tên module bên trên).

## Fix Rune Reward cộng rune trước khi vào world (2026-08-24)

Cùng lỗi mà [`PassiveRunes`](../passiverunes) gặp (xem README của nó, mục
2026-08-24): `GameDataMan` resolve xong ngay khi save slot được chọn, còn
đang ở màn hình title/loading, **trước khi** player thật sự vào world -
`add_runes` trong `src/rune_reward.rs` (khi đó còn tên `src/rune.rs`) trước
đây chỉ check `GameDataMan`, nên cả
tick theo interval lẫn milestone đều bắn sớm hơn dự kiến (milestone dùng
`session_elapsed_ms` tính từ lúc DLL load, không phải lúc vào game). Đã
thêm check `regen::main_player_chr_ins_ptr()` (cùng cổng `WorldChrMan.
main_player` mà `Regen` module đã dùng) vào `add_runes` - milestone loop
cũng đổi từ "bỏ qua vĩnh viễn nếu add_runes fail" sang "giữ nguyên
`next_milestone`, thử lại tick sau" để không mất bonus khi milestone rơi
đúng lúc còn đang loading.

## Gom nhóm lại ini, thêm Enabled/Condition/Trigger cho Regen (2026-08-22)

Cấu trúc key cũ (`Regen.Hp`, `Regen.HpOnHit`, `Regen.HpPctOnHit`...) chỉ có
1 quy ước "0 = tắt", không tách được "tắt hẳn tính năng" khỏi "để giá trị
0 vì đang thử". Đã đổi ini sang namespace rõ ràng hơn theo từng nhóm tính
năng (đúng ý ban đầu khi thêm `[Section]` - xem lịch sử trao đổi lúc cân
nhắc đổi sang TOML rồi quyết định giữ ini vì section + tiền tố đã đủ gom
nhóm):

- `[Regen Per Tick]`: thêm `Regen.PerTick.Enabled` (cờ bật/tắt rõ ràng) và
  `Regen.PerTick.Trigger` (0/1/2 = luôn/ngoài combat/trong combat, đặt tên
  giống `Regen.PerHit.Trigger` cho nhất quán, dù ý nghĩa là "điều kiện combat"
  chứ không phải "loại tính hồi phục" như bên PerHit). `Regen.PerTick.Unit`
  (0=điểm, 1=%) thay cho việc tách riêng `HpPct`/`Hp` như AutoRegen - mỗi
  stat giờ chỉ có 1 giá trị `Regen.PerTick.HP/FP/Stamina`, ý nghĩa đổi theo
  `Unit` chứ không cộng dồn flat+percent như bản AutoRegen.
- `[Regen Per Hit]`: `Regen.PerHit.Enabled` + `Regen.PerHit.Trigger` (0=điểm
  cố định, 1=% chỉ số tối đa, 2=% sát thương vừa gây ra - gộp 3 chế độ độc
  lập thay vì AutoRegen's `Trigger` 2 chế độ có flat+pct cộng dồn ở chế độ
  0). Cùng lý do: mỗi stat 1 giá trị duy nhất `Regen.PerHit.HP/FP/Stamina`.
- Thêm combat-tracking (`mark_combat_activity`/`is_in_combat`, cửa sổ 15s
  kể từ lần đánh/bị đánh gần nhất) vào `src/regen.rs` (khi đó còn tên
  `src/regen/mod.rs`), port từ chính
  cơ chế `Condition` mà AutoRegen từng làm dựa trên bản gốc của SomeTweaks -
  giờ port ngược lại đây vì `Regen.PerTick.Trigger=1/2` cần nó.
  `attack_hook.rs` đọc thêm `[ctx+8]` (target) để phát hiện cả trường hợp
  player **bị** đánh trúng, không chỉ **đánh** trúng - cả 2 đều tính là hoạt
  động combat dù chỉ chiều "đánh trúng" mới có heal-on-hit.
- Debug section thêm `RegenLog` (log riêng cho module Regen, độc lập với
  `DebugLog`) - theo đó `lib.rs` đổi logger sang luôn init (ghi đè mỗi lần
  chạy, không cộng dồn), thay vì trước đây chỉ init khi `DebugLog=true`
  (khiến `RegenLog=true` mặc định trong ini vô nghĩa vì chưa có file log
  nào được tạo).

Phát hiện thêm 1 lỗi copy-paste trong lúc đổi ini: `[Regen Per Hit]` bị gõ
nhầm key `Regen.PerTick.HP/FP/Stamina` (trùng với section trên), đã sửa lại
đúng `Regen.PerHit.HP/FP/Stamina`.

## Thêm module Rune Reward, port từ PassiveRunes (2026-08-23)

Kích hoạt `[Rune Reward]` trong `SomeTweaks.ini` (trước đó chỉ là block
comment placeholder), triển khai `src/rune_reward.rs` (khi đó còn tên
`src/rune.rs`, xem mục đổi tên module ngày 2026-08-24) dựa trên
[`PassiveRunes`](../passiverunes)'s `src/rune.rs` - cùng cơ chế đọc/ghi
`GameDataMan::main_player_game_data.rune_count` qua `fromsoftware-rs` và
threshold-crossing cho milestone (không dùng so khớp tuyệt đối
`elapsed==milestone`, tránh bỏ lỡ mốc nếu tick lệch nhịp), nhưng đổi tên
key và đơn vị cho khớp quy ước của `SomeTweaks.ini`:

- `Rune.Passive.Enabled` (cờ bật/tắt cả nhóm, kể cả milestone) thay vì
  `EnableMilestones` riêng của PassiveRunes.
- `Rune.Passive.Interval` tính bằng **mili giây** (khớp
  `Regen.PerTick.Interval`) thay vì `IntervalSeconds` của PassiveRunes.
- `Rune.Passive.Amount` thay `RunesPerInterval`.
- `Rune.Milestone` (không có tiền tố `Passive.` vì áp dụng độc lập với
  interval) vẫn giữ định dạng chuỗi giây:rune như PassiveRunes's
  `Milestones` - `Rune.Milestone=0` tự nhiên parse ra danh sách rỗng (không
  có dấu `:`) nên không cần case riêng cho "0 = tắt".

Chạy trên 1 thread/task riêng (`std::thread::spawn(rune::run)` trong
`lib.rs`), độc lập với tick của `Regen` - không tự lắng nghe
`General.ReloadKey` vì đọc chung 1 config map với Regen, nên hotkey reload
ở đâu cũng làm mới giá trị cho cả 2 module. Milestone list chỉ parse 1 lần
lúc khởi động (không hot-reload), giống hệt giới hạn PassiveRunes gốc đã
chấp nhận.

Tiện thể sửa 1 lỗi tài liệu trong ini: comment mô tả `Rune.Milestone` ghi
nhầm định dạng `(ms:rune)` trong khi giá trị mặc định dùng đơn vị giây
(`1800:5000` = 30 phút) - đã sửa thành `(giây:rune)`.

## Ý tưởng đã thử, chưa chốt: menu cấu hình trong game

Đã thử nghiệm 1 lần (rồi revert lại) — hook DX12 `Present` bằng
[`hudhook`](https://github.com/veeenu/hudhook) + vẽ UI bằng `imgui`, cho
slider chỉnh trực tiếp các giá trị `Regen.*` (không ghi ra ini, chỉ ảnh
hưởng bộ nhớ live, giống hệt cơ chế `ReloadKey` đang có). Về mặt kỹ thuật
là khả thi — build sạch, hook lên được, slider tương tác được.

**Vấn đề gặp phải khi test thật trong game (chưa giải quyết xong):**

- Con trỏ chuột không hiện — Elden Ring tự ẩn cursor hệ điều hành mỗi
  frame, phải bắt `imgui` tự vẽ cursor riêng (`io.mouse_draw_cursor = true`).
- Toạ độ chuột bị lệch khi Windows Display Scale khác 100% — `hudhook` lấy
  toạ độ thô từ `WM_MOUSEMOVE` (đơn vị bị Windows virtualize theo % scale)
  nhưng lại map vào kích thước buffer thật của swapchain (pixel vật lý).
  Test xác nhận: về 100% thì chuẩn, khác 100% thì lệch.
- Thử bù bằng `GetDpiForWindow` (nhân toạ độ theo `dpi/96`) nhưng **chưa ăn
  đúng** — có thể do giả định "process không DPI-aware" bị sai (Elden Ring
  nhiều khả năng đã tự per-monitor-DPI-aware, nên WM_MOUSEMOVE có khi không
  bị virtualize như giả định, cần đo lại thực tế thay vì suy luận).
- UI (khung/chữ/nút) tự nó cũng không scale theo độ phân giải/DPI — imgui
  vẽ theo pixel cố định, cần tự nhân `font_global_scale`/style scale theo
  màn hình nếu làm tiếp.

**Quyết định hiện tại:** revert toàn bộ phần này, giữ nguyên cấu hình qua
ini + `ReloadKey` hot-reload. Đây mới là thử nghiệm — sẽ quay lại giải
quyết các vấn đề trên nếu thật sự chốt chuyển sang cấu hình bằng menu
trong game (không chỉ để vui).

## Dọn log: level INFO/WARN/ERROR + gate CSTaskImp 1 lần (2026-08-28)

Đọc lại `SomeTweaks.log` của 1 phiên chơi thật thì thấy 3 vấn đề, sửa cả 3
trong lần này.

**1. Mỗi tính năng tự chờ `CSTaskImp` → 10 dòng log nói cùng 1 chuyện.**
Trước đây mỗi module (`Spirit.Summon`, `Spirit.Color`, `Regen`,
`RuneMultiplier`, `Rune Reward`, `Reload`, `TorrentAnywhere`,
`WeightMultiplier`, ...) đều gọi `task::wait_for_cs_task("<tên>")` trên
thread riêng của nó, nên cả ~10 thread cùng đâm vào đúng 1 cửa sổ
`InvalidRva` lúc game chưa unpack xong, và mỗi thread in ra 1 dòng
`"<tên>: CSTaskImp not ready yet (InvalidRva), retrying in 1s..."`. Đó là
thông tin **về game**, không phải về tính năng nào cả — nhân 10 lần là thừa.

Giờ `task::wait_for_cs_task()` (bỏ tham số `tag`) chờ **đúng 1 lần cho cả
DLL** qua `OnceLock`, và `lib.rs` gọi nó **trước khi spawn bất kỳ thread
tính năng nào**. Log rút còn 2 dòng ở đầu file:

```
[...] [INFO ] Engine: waiting for CSTaskImp (game still initializing)...
[...] [WARN ] Engine: CSTaskImp not ready yet (InvalidRva), retrying every 1s...
[...] [INFO ] Engine: CSTaskImp ready after 1.0s (1 retry/retries) - starting features.
```

Các lần retry thứ 2 trở đi không log nữa (chỉ lần đầu mang thông tin: lỗi
gì, tức game đang ở giai đoạn nào). Các module vẫn gọi
`task::wait_for_cs_task()` như cũ nhưng giờ nhận ngay con trỏ đã cache.

Lưu ý kỹ thuật: `CSTaskImp` chứa `DLPlainLightMutex` (bọc
`CRITICAL_SECTION`) nên **không `Sync`**, không nhét thẳng
`&'static CSTaskImp` vào `static` được. Cache bằng địa chỉ (`OnceLock<usize>`)
rồi dựng lại reference khi trả về — không nới rộng phạm vi chia sẻ so với
trước (mỗi module vốn đã giữ `&'static CSTaskImp` riêng trên thread của nó).

Hệ quả phụ: các code patch chạy lúc khởi động (`WeightMultiplier` hook,
`Rune.KeepOnDeath` NOP, `RuneMultiplier` hook, `TorrentAnywhere` patch) giờ
áp muộn hơn ~1s so với trước, vì phải chờ engine. Vô hại — thậm chí an
toàn hơn, vì lúc đó exe chắc chắn đã relocate xong.

**2. Log có level.** `shared/src/logger.rs` thêm `warn`/`error`/`debug` bên
cạnh `log` sẵn có; format 1 dòng giờ là
`[YYYY-MM-DD HH:MM:SS] [LEVEL] message`, với `LEVEL` căn trái 5 ký tự để
cột message của mọi dòng thẳng hàng, dễ lướt mắt. Giữ nguyên tên hàm `log`
(= INFO) thay vì đổi thành `info` để **không phải sửa call site ở các crate
khác** (`autoregen`, `passiverunes`, `runemultiplier`, `risearcher`,
`weightmultiplier` cũng dùng chung logger này) — INFO đúng cho đại đa số
lời gọi hiện có.

Quy ước dùng level trong `sometweaks`:

- `error` — tính năng hỏng hẳn và tắt cả phiên (không tìm thấy pattern,
  `VirtualProtect`/`VirtualAlloc` fail, tick panic). Đã bỏ chuỗi
  `"ERROR - "`/`"ERROR: "` viết tay bên trong message, vì cột level nói rồi.
- `warn` — bất thường nhưng còn cứu được / còn đang retry (engine chưa sẵn
  sàng, `DropRate` reload lúc chưa vào world, module tự tắt sau khi hook fail).
- `debug` — chi tiết chỉ dùng khi soi 1 tính năng cụ thể (hex dump byte của
  stub `RuneMultiplier`, damage từng đòn của `AttackHook`); những chỗ này
  vốn đã nằm sau cờ ini riêng (`DebugLog`/`RegenLog`).

**3. Bỏ log rune mỗi interval.** Dòng
`"Rune Reward: +100 runes (interval tick, session 30s)"` bắn ra mỗi
`Rune.Passive.Interval` ms suốt phiên chơi (~360 dòng/giờ ở mặc định 10s) và
không nói gì mà người chơi không tự thấy trên thanh rune — chỉ làm phình
file log. Đã bỏ hẳn (không phải đưa xuống `debug`, vì kể cả khi debug nó
cũng không giúp gì). **Milestone bonus vẫn log** dưới cờ `RuneLog`: nó hiếm,
1 lần/mốc, và dễ trôi qua mà không để ý trong game.

## Đồng nhất format log giữa các tính năng (2026-08-28)

Review lại toàn bộ `logger::` call site trong `sometweaks` (không đụng
`autoregen`/`passiverunes`/`runemultiplier`/`risearcher`/`weightmultiplier` -
các crate đó dùng chung `shared/src/logger.rs` nhưng là mod độc lập, không
thuộc phạm vi "đồng nhất giữa các tính năng của SomeTweaks"). Trước khi sửa,
mỗi tính năng tự đặt format hơi khác nhau dù cùng chung logger:

- Tag không khớp ini key: log ghi `"RuneMultiplier: ..."` trong khi ini key
  là `Rune.Multiplier`; log ghi `"Rune Reward: ..."` (có dấu cách) trong khi
  mọi key của module này là `Rune.Passive.*`.
- Dòng `"<Tag> tick registered on CSTaskGroupIndex::FrameBegin."` không có
  dấu `:` sau tag ở 5/6 module (`Regen`, `Rune Reward`, `Spirit.Color`,
  `Spirit.Regen`, `Spirit.Summon`), trong khi `TorrentAnywhere` lại có.
- 3 dòng lỗi (`drop_rate`, `unlock_ashes_of_war`, `unlock_enchantments`) khi
  `SoloParamRepository` không bao giờ sẵn sàng thì nhét tên tính năng ở
  **cuối** câu thay vì ở đầu như mọi dòng lỗi khác trong cùng file.
- Vài dòng "disabled for this session (...)" thiếu dấu `:` sau tag
  (`WeightMultiplier`, `Rune.KeepOnDeath`, `RuneMultiplier`,
  `Spirit.Summon.Anywhere`).

Quy tắc chốt cho từ giờ: **mọi dòng log của 1 tính năng phải mở đầu bằng
`<Tag>: `**, với `Tag` luôn khớp đúng tiền tố ini key của tính năng đó
(`Spirit.Summon.Anywhere`, `Rune.KeepOnDeath`, `Rune.Multiplier`,
`Rune.Passive`, ...) - trừ 2 kiểu ngoại lệ có chủ đích, áp dụng đồng nhất
trên toàn bộ crate chứ không phải chỉ 1 module:

1. Dòng "báo giá trị config vừa đổi" tự thân đã là `<IniKey>=<value>` (vd.
   `WeightMultiplier=0.500.`, `Rune.Multiplier=3.000.`,
   `DropRate.Multiplier=3.000 applied to ... row(s).`) - không cần thêm
   `Tag:` phía trước vì chính `IniKey` đã là định danh.
2. Dòng "tắt tại startup vì key=false" cũng ở dạng `<IniKey>=false -
   skipping entirely at startup.` cùng lý do.

`AttackHook` (bên trong `regen/attack_hook.rs`) và `Engine` (bên trong
`task.rs`) là 2 tag không khớp trực tiếp 1 ini key - có chủ đích: `AttackHook`
là 1 cơ chế con của `Regen.PerHit` (một hook code-patch riêng, khác hẳn tick
của `Regen`), còn `Engine` là log về chính game engine (chờ `CSTaskImp`),
không thuộc về tính năng nào cả - xem mục "Gate CSTaskImp 1 lần" phía trên.

Không đổi tên hàm/biến trong code (`rune::multiplier`, `rune::reward` giữ
nguyên tên module) - chỉ đổi **chuỗi hiển thị trong log**, nên không ảnh
hưởng gì tới hành vi, chỉ tới nội dung `SomeTweaks.log`.

## GraceMenu: tìm ra cách fix bug "đứng dậy" khi mở menu tuỳ biến ở Site of Grace (2026-08-28)

Sau phiên điều tra trước (bị dồn vào `git stash@{0}`, xem mục README cũ ở
trên) kết luận tạm "OpenEnhanceShop tự nó ép đứng dậy bất kể ngữ cảnh",
lần này viết lại `crates/sometweaks/src/misc/grace_menu.rs` từ đầu (không
tái dùng code cũ trong stash, cố tình không restore) để kiểm chứng riêng
đúng 1 giả thuyết còn treo: chiếm dụng 1 state vanilla thật ("Tailoring
Shop", bank1 id 142) ngay trong graph ESD đang sống của Site of Grace, chỉ
đổi `entry_events` thành `OpenEnhanceShop` (y hệt `EldenConvenienceMod`),
giữ nguyên 100% `transitions` gốc.

**Vòng test 1** (chỉ đổi `entry_events`, giữ nguyên điều kiện transition
gốc của Tailoring Shop): **vẫn đứng dậy**. Tưởng đây là bằng chứng cuối
cùng khẳng định kết luận cũ.

**Vòng test 2** - phát hiện mảnh ghép còn thiếu: `EldenConvenienceMod`
không chỉ đổi `entry_events`, nó còn tự **ghi đè điều kiện chờ**
(`transitions[0].evaluator`) của chính state đó thành
`CheckSpecificPersonMenuIsOpen(9, 0) == 0 || CheckSpecificPersonGenericDialogIsOpen(0)`
(`MenuCloseExpr(9)` trong `SoulsIds.ESDEdits`, `9` = loại menu riêng của
`OpenEnhanceShop`) - điều kiện GỐC của Tailoring Shop được tinh chỉnh cho
loại menu KHÁC (menu riêng của chính Tailoring Shop), nên rất có thể đánh
giá "đã đóng" (hoặc pass qua) trong khi UI Enhance Shop còn đang khởi tạo,
khiến ESD machine và hệ thống menu lệch pha - kích hoạt logic "hội thoại
đã xong" của engine (đứng dậy).

Đối chiếu byte-code: giải mã ngược `TALK_MENU_CLOSED_EXPRESSION` (biểu
thức đã xác nhận hoạt động ở phiên trước, cho menu loại 1) theo đúng bảng
opcode ESD trong `AST.cs` (`SoulsIds` mới clone) khớp CHÍNH XÁC với
comment gốc của `ESDEdits.cs` cho `MenuCloseExpr(1)` - xác nhận bảng
opcode đúng, từ đó tự mã hoá `MenuCloseExpr(9)` bằng tay
(`MENU_CLOSED_TYPE_9_EXPRESSION`, chỉ khác 1 byte so với bản loại 1).
Thêm luôn lệnh `c1_141(9)` (community đặt tên `Unknown141_PlaylogRelated`,
chưa rõ chức năng thật) ngay trước `OpenEnhanceShop` cho khớp 100%
`EldenConvenienceMod` - không tốn gì nếu hoá ra không quan trọng.

**Test thật (2026-08-28): THÀNH CÔNG** - "Nâng cấp vũ khí" mở đúng menu,
KHÔNG đứng dậy, cả 2 mục (mục mới chèn + đường quay lại sau khi đóng menu)
hoạt động hoàn hảo.

**Kết luận:** kết luận "airtight" của phiên trước là SAI - không phải bản
thân lệnh ép đứng dậy, mà do thiếu đúng điều kiện chờ khớp loại menu.
Công thức khái quát hoá được cho MỌI tính năng muốn thêm vào Site of Grace
kiểu này (không cần sửa file ESD tĩnh, chạy hoàn toàn runtime từ DLL):

1. Chiếm dụng 1 state vanilla ít dùng bất kỳ (tìm bằng
   `find_state_by_first_command` - nội dung, không phải địa chỉ).
2. Đổi `entry_events` của state đó thành chuỗi lệnh ESD mong muốn.
3. Đổi `transitions[0].evaluator` thành `MenuCloseExpr(đúng loại menu của
   lệnh đó)` - **KHÔNG đụng `target_state`** (đường quay lại thật của
   chính state, luôn đúng vì là code gốc của game).

`Bán đồ` (`OpenRegularShop`, loại menu 5) và các tính năng còn lại theo kế
hoạch ban đầu giờ áp dụng được bằng đúng công thức trên, chỉ cần tìm 1
state vanilla ít dùng khác làm "vật chiếm dụng" cho mỗi lệnh.

## GraceMenu: bỏ "Nâng cấp vũ khí" (chỉ để test), thêm thật "Mua đồ (Twin Maiden Husks)" trên 1 slot khác (2026-08-28)

Sau khi xác nhận công thức fix (mục trên) hoạt động đúng, "Nâng cấp vũ
khí" hoàn thành nhiệm vụ của nó (chứng minh kỹ thuật) - không giữ lại làm
tính năng thật. `"Tailoring Shop"` được **khôi phục hoàn toàn về vanilla**
(module không còn đụng tới state này nữa).

Thêm tính năng thật đầu tiên: **"Mua đồ (Twin Maiden Husks)"** -
`OpenRegularShop` với range item lot 101800-101899 (nguyên từ
`EldenConvenienceMod`). Chiếm dụng 1 slot vanilla KHÁC hoàn toàn -
`"Dupe Shop"` (bank1 id 146, `open_dupe_shop` trong `elden-x`/ESDLang -
tên gọi gợi ý đây là nội dung debug/nội bộ chưa từng dùng thật, tương tự
`Tailoring Shop`) - áp dụng đúng công thức đã xác nhận: đổi
`entry_events` thành `OpenRegularShop(101800, 101899)`, đổi
`transitions[0].evaluator` thành `MenuCloseExpr(5)` (loại menu riêng của
`OpenRegularShop`), giữ nguyên `target_state`.

Bỏ luôn phần code specific cho Upgrade (`CombineMenuFlagAndEventFlag`
x4 + lệnh `c1_141` chưa rõ chức năng) - `OpenRegularShop` không cần các
lệnh phụ này, `EldenConvenienceMod` cũng chỉ gọi đúng 1 lệnh
`OpenRegularShop` cho tính năng Purchase.

**Chưa test thật trong game ở bước này** - cần xác nhận "Dupe Shop" cũng
là slot chết an toàn giống Tailoring Shop trước khi coi đây là xong.

## GraceMenu: bỏ hẳn việc chiếm dụng slot vanilla - dùng state hoàn toàn mới (2026-08-28)

Người dùng đặt câu hỏi đúng trọng tâm: tại sao phải chiếm dụng 1 state
vanilla có sẵn (rủi ro: nếu có Site of Grace đặc biệt nào thật sự dùng
"Dupe Shop"/"Tailoring Shop", sẽ mất tính năng gốc của họ)? Lý do trước đó
thuần tuý là bối cảnh phát hiện ra fix (so sánh trực tiếp với
`EldenConvenienceMod` bằng cách chiếm dụng 1 state thật) - không có bằng
chứng nào cho thấy `CheckSpecificPersonMenuIsOpen` bám theo `id`/vị trí
của từng `EzState` cụ thể thay vì bám theo phiên hội thoại (machine
instance) đang sống. Mọi lần thử THẤT BẠI trước đây dùng state mới tự tạo
đều KHÔNG nhúng lệnh ESD thật vào `entry_events` của chính state đó (gọi
qua kênh khác - `ezstate_event`/native - từ 1 thread riêng) và dùng
transition "luôn đúng" thay vì `MenuCloseExpr` đúng loại - tức là công
thức fix thật sự chưa từng được thử trên 1 state hoàn toàn mới.

Đổi `rewrite_as_open_regular_shop` (chiếm dụng "Dupe Shop") thành
`build_purchase_state` - dựng 1 `EzState` hoàn toàn mới (`Box::leak`) với
`entry_events` = `OpenRegularShop(101800, 101899)`,
`transitions[0].evaluator` = `MenuCloseExpr(5)`, `target_state` trỏ về
đúng 1 state vanilla có sẵn KHÔNG BAO GIỜ bị sửa
(`find_menu_rebuild_state` - state mà bản thân vanilla vốn đã coi là
"menu đã đóng, làm mới danh sách", nhận diện bằng nội dung
`entry_events` = đúng 1 lệnh `clear_talk_list_data` không tham số). Không
còn state vanilla nào bị đụng tới - "Dupe Shop" cũng giữ nguyên vanilla y
hệt "Tailoring Shop".

**Test thật (2026-08-28): THÀNH CÔNG** - state hoàn toàn mới cũng tránh
được bug "đứng dậy" y hệt state vanilla bị chiếm dụng trước đó.

## GraceMenu: thêm lại "Nâng cấp vũ khí" bằng state mới (không hijack), cùng lúc với "Mua đồ" (2026-08-28)

Sau khi xác nhận "Mua đồ" (Twin Maiden Husks) hoạt động đúng với kỹ thuật
state hoàn toàn mới (mục trên), thêm lại "Nâng cấp vũ khí" (`OpenEnhanceShop`)
bằng đúng kỹ thuật đó thay vì hijack "Tailoring Shop" như bản test đầu
tiên - `build_upgrade_state` (song song `build_purchase_state`) dựng 1
`EzState` hoàn toàn mới, `entry_events` = 4x `CombineMenuFlagAndEventFlag`
+ `c1_141(9)` + `OpenEnhanceShop(0)` (nguyên xi từ `EldenConvenienceMod`),
`transitions[0].evaluator` = `MenuCloseExpr(9)`, quay về cùng 1 điểm an
toàn `find_menu_rebuild_state` như "Mua đồ".

`patch_state_group` được tổng quát hoá để nhận 1 danh sách insertion thay
vì chỉ 1, chèn cả 2 mục trong cùng 1 lần quét anchor (không phải quét 2
lần). Site of Grace giờ có 2 mục mới: "Mua đồ" và "Nâng cấp vũ khí",
không tính năng vanilla nào bị đụng tới.

**Test thật: THÀNH CÔNG** cho cả 2 mục cùng lúc.

## GraceMenu: text tự soạn thật sự - hook thẳng hàm tra text của game (2026-08-28)

Người dùng hỏi: có dùng được text tự viết (không mượn ID có sẵn) không?
Kiểm tra thêm thì phát hiện `erdGameTools` (`.docs/Elden_Ring_game_tools`,
mã nguồn mở, xác nhận đang chạy đúng bản game hiện tại qua
`erdGameTools.dll` + `Resources/Lang/en-US.txt` cài sẵn trong
`mod_test`) làm được việc này bằng cách **hook thẳng hàm tra bảng text**
(`src/grace_test_messages.cpp`), không phải patch dữ liệu FMG.

Thêm module mới `crates/sometweaks/src/misc/msg_hook.rs`, port lại đúng
kỹ thuật đó:

- Dò 2 AOB y hệt `erdGameTools` dùng để tìm hàm `get_message` thật (nhận
  `(msg_repository, unknown, bnd_id, msg_id)`, trả `const wchar_t*`) -
  xác nhận qua Ghidra (chạy headless bằng `analyzeHeadless` vào project
  `D:/tmp/ghidra_eldenring` có sẵn, script mới
  `.docs/reverse_engineering/DumpGetMessageFn.java`, output
  `D:/tmp/get_message_dump.txt`) đúng trên bản game hiện tại - hàm là 1
  leaf function, prologue 15 byte đầu là `cmp;jae;cmp;jae;mov`, không có
  stack frame.
- Hook bằng kỹ thuật "quan sát rồi tiếp tục" y hệt các hook khác trong
  crate, nhưng có thêm nhánh rẽ: gọi 1 hàm Rust (`get_message_detour`)
  trước - nếu `bnd_id==33` ("event text for talk") và `msg_id` khớp 1 ID
  do module này tự đặt (dải `90000001+`, không đụng ID thật lẫn dải
  `erdGameTools` tự dùng `69010000-69015000`), trả thẳng con trỏ chuỗi
  UTF-16 tự soạn (`"Nâng cấp vũ khí"`/`"Mua đồ (Twin Maiden Husks)"`,
  giữ vĩnh viễn qua `OnceLock<Vec<u16>>`); ngược lại, phát lại (replay)
  đúng 5 byte instruction đầu của hàm gốc rồi nhảy tiếp vào phần còn lại
  - hàm thật chạy hoàn toàn bình thường cho mọi ID khác.
- Điểm khác các hook khác trong crate: 2 lệnh `jae` trong prologue gốc
  **không thể copy nguyên byte** sang stub (địa chỉ đích tính tương đối,
  sai nếu chạy ở vị trí khác) - phải tính lại đích tuyệt đối (đọc sống từ
  byte thật, không hardcode), rồi phát lại dưới dạng `jb` đảo ngược +
  nhảy tuyệt đối qua đích đó (kỹ thuật chuẩn khi relocate short jump ra
  ngoài tầm rel8/rel32).

`grace_menu.rs`'s `UPGRADE_MSG_ID`/`PURCHASE_MSG_ID` giờ trỏ tới
`msg_hook::{UPGRADE_MSG_ID, PURCHASE_MSG_ID}` thay vì 2 ID mượn từ
`EldenConvenienceMod`.

**Test thật: THÀNH CÔNG** - text hiện đúng, không phát sinh crash/lỗi ở
hàm `get_message` cho các lookup khác trong suốt phiên chơi.

## GraceMenu: kết hợp text vanilla thật (đa ngôn ngữ) + hậu tố tự soạn (2026-08-28)

Người dùng hỏi thêm: có ghép được text sẵn có của game (vd "Mua"/"Bán",
tự động đúng ngôn ngữ game đang chạy) với text tự thêm không (vd "Mua"
+ "(Twin Maiden Husks)")? Có - vì hook đã chặn đúng hàm `get_message`,
nên hoàn toàn gọi NGƯỢC LẠI chính hàm đó (qua `call_get_message`) để lấy
text vanilla thật của 1 ID có sẵn (`VANILLA_UPGRADE_MSG_ID=22130001`,
`VANILLA_PURCHASE_MSG_ID=26000010` - đúng 2 ID từng mượn trực tiếp trước
khi có hook này), an toàn không đệ quy vô hạn (ID vanilla không trùng 2
ID tự đặt của module, nên hook tự rơi xuống gọi hàm gốc bình thường).

`combined_text()` build 1 lần duy nhất (cache vĩnh viễn qua `OnceLock`,
vì 1 mục danh sách bị hỏi lại nhiều lần mỗi khi vẽ menu): lấy text
vanilla + nối thêm hậu tố tự soạn. "Nâng cấp vũ khí" giữ nguyên text
vanilla của ID 22130001 (không hậu tố, đã đủ nghĩa); "Mua đồ" đổi thành
vanilla + `" (Twin Maiden Husks)"` - vừa tự đúng ngôn ngữ hiện tại của
game, vừa rõ ràng shop nào.

**Test thật: THÀNH CÔNG.**

## GraceMenu: fix mất mục menu ở Grace khác/quay lại Grace cũ - cờ toàn cục sai (2026-08-28)

Bug do người dùng phát hiện: mục menu mới chỉ xuất hiện ở đúng Grace ĐẦU
TIÊN ngồi trong phiên chơi - Grace khác không có, và quay lại chính Grace
đó sau khi rời đi cũng MẤT LUÔN. Nguyên nhân: `static PATCHED: AtomicBool`
là cờ TOÀN CỤC - hễ patch xong 1 lần là mọi lần vào Grace sau (bất kỳ
Grace nào, kể cả quay lại đúng Grace cũ) đều bị bỏ qua ở
`on_enter_state`, vì bản thân mỗi Site of Grace dựng 1 BẢN COPY ESD graph
MỚI trong bộ nhớ mỗi lần được vào (đúng như ghi trong doc comment của
`is_grace_state_group` từ đầu, nhưng lúc code cờ `PATCHED` lại quên áp
dụng đúng hệ quả của sự thật này).

Bỏ hẳn `PATCHED`, thay bằng kiểm tra NỘI DUNG của đúng graph đang sống
(`already_has_custom_items` - quét `entry_events` tìm sự kiện
`add_talk_list_data` với message ID khớp 1 trong 2 ID tự đặt của module)
- cùng triết lý "neo bằng nội dung, không phải địa chỉ/cờ toàn cục" mà
mọi hàm neo khác trong file này (`is_sort_chest_event`,
`targets_open_repository`, `find_menu_rebuild_state`) đã dùng từ đầu.
Giờ mỗi graph MỚI (mỗi lần vào Grace, ở bất kỳ đâu) đều tự được patch
đúng 1 lần, độc lập với mọi graph khác - không còn phụ thuộc "đã patch
graph nào TRƯỚC ĐÓ trong phiên chơi" nữa.

## GraceMenu: chuyển `msg_hook` vào làm submodule riêng của `grace_menu` (2026-08-28)

Người dùng chỉ ra: `msg_hook` không phải 1 tính năng độc lập (không có
ini key, không tự bật/tắt, chỉ tồn tại để phục vụ `grace_menu`) - để
ngang hàng trong `misc/` (nơi mọi module con đều tương ứng 1 mục ini
riêng) là sai chỗ. Chuyển `misc/msg_hook.rs` thành
`misc/grace_menu/msg_hook.rs` (submodule riêng tư của `grace_menu`,
`misc/grace_menu.rs` → `misc/grace_menu/mod.rs`) - đúng quy ước
`regen/attack_hook.rs` đã dùng cho `regen`. Thuần tuý đổi cấu trúc, không
đổi hành vi.

## GraceMenu: mở rộng thành 3 mục (Nâng cấp/Mua tất cả/Bán) + ini key riêng từng mục (2026-08-28)

Sau khi xác nhận công thức fix hoạt động và kỹ thuật "state hoàn toàn
mới" an toàn, mở rộng theo yêu cầu: thay vì chỉ mua từ Twin Maiden Husks,
tìm thấy bằng chứng cộng đồng trong `Elden-Ring-CT-TGA` (script
`All Shops.cea`): `executeEzStateEvent(EzStateEvent.OpenRegularShop, {0, 9999999})`
- gộp TOÀN BỘ vật phẩm bán được của MỌI thương nhân trong game vào 1
range duy nhất. Đổi `ALL_SHOPS_LOT_START/END = 0/9999999` thay cho range
hẹp của Twin Maiden Husks trước đó.

Thêm tính năng 3 - "Bán" (`OpenSellShop`, bank1 id 46, args `(-1,-1)`
nguyên từ `EldenConvenienceMod`), dùng đúng công thức đã xác nhận
(`entry_events` + `MenuCloseExpr(6)` - loại menu riêng của
`OpenSellShop`).

Thêm `[Grace Menu]` trong `SomeTweaks.ini` với 5 key:
- `GraceMenu.Enabled` - cờ tổng, tắt là bỏ hẳn hook.
- `GraceMenu.Upgrade`/`GraceMenu.Shop`/`GraceMenu.Sell` - bật/tắt riêng
  từng mục, đọc trong `on_enter_state` mỗi lần build insertion cho 1
  instance graph mới.
- `GraceMenu.UnlockShopInventory` - tính năng độc lập MỚI
  (`misc/grace_menu/unlock_shop_inventory.rs`, submodule riêng của
  `grace_menu` cùng `msg_hook`): patch `ShopLineupParam.event_flag_for_release=-1`
  cho mọi dòng (nguyên từ script cộng đồng "Access all shop inventory.cea"),
  để mega-shop hiện cả vật phẩm chưa mở khoá theo tiến trình quest. Tắt
  mặc định - không đụng gì nếu không bật.

Text hiển thị qua `msg_hook`: "Nâng cấp vũ khí" giữ nguyên vanilla (ID
22130001, đủ rõ nghĩa); "Mua" = vanilla (ID 26000010) + hậu tố
`" (tất cả)"` (làm rõ đây là gộp mọi thương nhân, không phải 1 người bán
cụ thể); "Bán" giữ nguyên vanilla (ID 20000011).

**Test thật: THÀNH CÔNG cho cả 5 key** (bao gồm `GraceMenu.UnlockShop`,
đổi tên sau từ `UnlockShopInventory` - xem mục bên dưới).

## GraceMenu: chuyển ra khỏi `misc/`, thành tính năng chính ngang hàng `regen`/`rune`/`spirit` (2026-08-28)

`grace_menu` giờ đã là 1 tính năng đầy đủ (3 mục menu + section `[Grace
Menu]` riêng trong ini), không còn là thử nghiệm nhỏ chia sẻ `[Misc]`
nữa - chuyển `crates/sometweaks/src/misc/grace_menu/` (kèm 2 submodule
`msg_hook`/`unlock_shop_inventory`) thành `crates/sometweaks/src/grace_menu/`,
khai báo `mod grace_menu;` ở `lib.rs` ngang hàng `regen`/`rune`/`spirit`
thay vì nằm trong `misc::`. Thuần tuý đổi cấu trúc, không đổi hành vi.

## GraceMenu: đổi tên `unlock_shop_inventory` → `unlock_shop`, key ini `GraceMenu.UnlockShopInventory` → `GraceMenu.UnlockShop` (2026-08-28)

Ngắn gọn hơn, đủ nghĩa. Đổi tên file/module/ini key, không đổi hành vi.

## GraceMenu: review lại toàn bộ - fix doc-comment cũ, bỏ SHOP_SUFFIX (2026-08-28)

Review theo yêu cầu người dùng. Sửa:
- 3 chỗ doc comment còn trỏ tới `build_purchase_state` (đã đổi tên
  `build_shop_state` từ trước, sót lại link cũ).
- Bỏ hẳn `SHOP_SUFFIX`/tham số `suffix` của `combined_text` (đổi tên
  `vanilla_text`) - không cần thiết, cả 3 mục giờ chỉ hiện đúng text
  vanilla gốc.

Không có bug chức năng nào phát hiện thêm. Các đặc điểm đã biết, chấp
nhận được (không phải bug):
- Mỗi lần vào 1 Grace instance chưa patch sẽ `Box::leak` vài trăm byte
  (state/transition/buffer) - không thể free an toàn vì game có thể vẫn
  giữ tham chiếu; đã là triết lý xuyên suốt file này. Tích luỹ theo số
  LẦN vào Grace trong 1 phiên chơi (không chỉ số Grace vật lý), nhưng quá
  nhỏ để đáng lo (~1KB/lần).
- Nếu 1 biến thể Grace nào đó không có đủ 2 anchor (Sort Chest event /
  OpenRepository transition), Grace đó sẽ leak nhỏ ở MỌI lần ghé thăm
  (không bao giờ "đã patch") - chưa gặp trường hợp này trong test, chỉ là
  khả năng lý thuyết.
- `cargo clippy` hiện fail ngay ở crate `common`/`shared` (lỗi có sẵn từ
  trước, không liên quan `sometweaks`/`grace_menu`) - ngoài phạm vi phiên
  làm việc này.

## Gộp RegenLog/RuneLog vào chung DebugLog (2026-08-28)

Bỏ 2 key `RegenLog`/`RuneLog` - chỉ còn `DebugLog` kiểm soát toàn bộ log
debug/verbose trong crate (đã đổi 3 chỗ gọi `config::get_bool` tương ứng
trong `regen/attack_hook.rs`/`rune/reward.rs`). Ini cũ của người dùng (đã
migrate trước đó) vẫn còn 2 key thừa `RegenLog=true`/`RuneLog=true` nằm
im không dùng tới - không tự xoá (cơ chế migrate chỉ thêm key thiếu,
không xoá key thừa) - có thể tự tay xoá nếu muốn dọn sạch.

## GraceMenu: UnlockShop giờ hỗ trợ hot-reload (2026-08-28)

Đổi `unlock_shop.rs` từ 1-shot (chỉ áp lúc khởi động) sang có
`General.ReloadKey` hot-reload, đúng mẫu `drop_rate.rs` đang dùng: snapshot
`event_flag_for_release` gốc của mọi `ShopLineupParam` row 1 lần (trước
khi sửa bất kỳ gì), rồi mỗi lần `ReloadKey` được bấm, đọc lại
`GraceMenu.UnlockShop` và áp `u32::MAX` (bật) hoặc phục hồi đúng giá trị
gốc (tắt) - không còn bị kẹt vĩnh viễn ở "đã mở khoá" nếu tắt lại giữa
chừng. Nếu `GraceMenu.UnlockShop=false` ngay từ đầu, bỏ qua hẳn việc chờ
`SoloParamRepository` (như `DropRate.Enabled` đã làm) - chỉ chờ khi thật
sự cần.

## Rà soát chú thích hot-reload trong ini (2026-08-28)

Đối chiếu từng key với code thật:
- `Rune.KeepOnDeath` thiếu chú thích "Applied once at startup (no
  hot-reload)" dù patch NOP chỉ chạy đúng 1 lần lúc khởi động, không có
  tick/theo dõi reload nào (giống hệt `TorrentAnywhere`/`UnlockAshesOfWar`
  đã có chú thích này) - đã thêm.
- `GraceMenu.Upgrade`/`GraceMenu.Shop`/`GraceMenu.Sell` không phải "no
  hot-reload" hoàn toàn (đọc lại config mỗi khi vào 1 Site of Grace mới)
  cũng không phải hot-reload tức thời như `Regen.*` (Grace đang đứng
  không đổi) - thêm ghi chú riêng cho đúng hành vi thật.

Mọi key khác đã đối chiếu đều khớp đúng chú thích hiện có (hoặc đúng là
hot-reload tức thời, không cần ghi chú).

## Xác nhận test thật trong game: toàn bộ GraceMenu + Rune.KeepOnDeath (2026-08-28)

Người dùng xác nhận đã test hết:
- **`GraceMenu`**: cả 3 mục (Nâng cấp vũ khí/Mua tất cả/Bán) hoạt động
  đúng, không đứng dậy, không mất mục khi đổi Grace, text hiển thị đúng
  (kể cả sau khi bỏ hậu tố `(All)` khỏi "Mua"), `GraceMenu.UnlockShop`
  hoạt động đúng (bật/tắt + hot-reload).
- **`Rune.KeepOnDeath`**: đã hoạt động đúng (kỹ thuật AOB-scan-and-NOP
  hiện tại trong `src/rune/keep_on_death.rs`, không phải cách tiếp cận
  field `has_dropped_runes` cũ đã bị bỏ trước đó - xem mục README phía
  trên "chưa test trong game" về cách cũ, đã lỗi thời, không áp dụng cho
  code hiện tại nữa).

Không còn tính năng nào của `GraceMenu` ở trạng thái "chưa test".

## Thêm module WarpAnywhere, port từ `.docs/FastTravel/Zibinha_FastTravel.dll` (2026-08-28)

Người dùng đưa 2 tệp DLL tham khảo (`Zibinha_FastTravel.dll` và
`Zibinha_MapInCombat.dll`, cùng thư mục `.docs/FastTravel/`) để giải mã và
port. Cả 2 là C++ MSVC đơn giản (không phải Rust), không strip hết string
nên đọc log message tiếng Bồ Đào Nha trực tiếp được (`MAP_CHECK:`,
`WARP_BLOCK:`, `FIELD_AREA:`...). Dùng Ghidra `analyzeHeadless` decompile
để lấy đúng byte AOB/patch thay vì đoán, cùng cách đã làm với
`torrent_anywhere`/`msg_hook` trước đó.

`Zibinha_MapInCombat.dll` là tập con byte-for-byte của
`Zibinha_FastTravel.dll` (chỉ có 2 patch đầu). File `FastTravel` có thêm
patch thứ 3 cho việc fast travel trong dungeon/hang. Port cả 3 vào
`src/misc/warp_anywhere.rs`, module mới trong `misc/` (dùng chung
`common::memscan`/`common::codepatch`, không có gì đặc thù game engine nên
không cần thư viện `eldenring` cho phần patch chính - xem doc comment của
module để biết vì sao):

1. **`map_check`**: patch 5 byte đầu của 1 lệnh `call` (hàm kiểm tra map có
   bị khoá khi đang combat) thành `xor rax,rax; nop; nop` - hàm không bao
   giờ chạy nữa, kết quả luôn là "không bị khoá".
2. **`warp_block`**: đổi 1 byte `je` (0x74) thành `jmp` (0xEB) - nhánh "cho
   phép warp" luôn được chọn bất kể điều kiện gốc.
3. **`field_area_unlock`**: resolve địa chỉ global slot của singleton
   `FieldArea` 1 lần (từ lệnh `mov rcx,[rip+disp32]` đầu AOB, tính
   `match_addr + 7 + disp32` theo chuẩn RIP-relative, không dùng công thức
   suy ra từ pseudo-C của Ghidra vì nó sai do kiểu con trỏ giả tạo trong
   bản decompile), sau đó mỗi tick (`CSTaskGroupIndex::FrameBegin`) đọc lại
   slot và zero offset `+0xA0` của instance hiện tại - patch gốc dùng vòng
   `Sleep(100)` từ thread riêng vì `FieldArea` chưa tồn tại lúc DLL load,
   ở đây dùng tick trên `CSTaskImp` cho nhất quán với các module khác.

Key ini mới: `WarpAnywhere=true` (mặc định bật, áp dụng 1 lần lúc khởi
động, không hot-reload - giống `TorrentAnywhere`/`UnlockAshesOfWar`, vì
patch code không có "giá trị gốc" để snapshot/restore).

## Fix crash trong `Spirit.Summon` - thiếu gate "đã vào world" (2026-08-29)

Người dùng báo game crash rất dễ (vào world, mở map dịch chuyển qua Grace
khác, hoặc alt-tab), không có log nào in ra để biết nguyên nhân. Cả
`WarpAnywhere` và `GraceMenu` đều đang tắt lúc test nên không phải do 2
tính năng mới - phải tra ngược thật để tìm.

Windows Event Log (`Get-WinEvent -FilterHashtable @{LogName='Application';
Id=1000,1001}`) cho địa chỉ crash chính xác: exception `0xC0000005`
(access violation) tại `SomeTweaks.dll+0xE30D`, lặp lại y hệt mọi lần -
build lại DLL release, decompile bằng Ghidra `analyzeHeadless` (dùng
`.pdb` sẵn có trong `target/release/`) để tra RVA đó ra đúng hàm và đúng
lệnh asm gây crash: `mov rax,[rsi+0x1e538]; mov r8,[rax+8]` - trong
`spirit/summon_count.rs`'s `apply()`.

Nguyên nhân: `WorldChrMan::instance_mut()` trả `Ok` ngay khi object quản
lý tồn tại (màn hình loading, đang chuyển giữa 2 Grace) - CHƯA có nghĩa
`summon_buddy_manager.trigger_speffect_to_buddy_map`'s storage nội bộ đã
được cấp phát. `apply()` đọc thẳng vào đó mà không qua gate "người chơi
đã thực sự vào world chưa" như mọi module khác đã dùng
(`crate::player::main_player_chr_ins_ptr()`, xem doc comment của
`player.rs`) - lúc storage còn null, `iter_chains_mut()` dereference
thẳng vào null pointer → crash im lặng (không phải Rust panic nên
`run_recurring_safe`'s `catch_unwind` không bắt được).

Fix: thêm đúng gate đó vào đầu `apply()`, trước khi chạm
`summon_buddy_manager`. Đây cũng là bài học cho lần port tính năng sau:
khi build ra 1 DLL mới rồi thấy crash không log, luôn tra Windows Event
Log lấy offset trước, đừng đoán mò module nào gây ra.

## Thu hẹp range của "Mua" để giảm giật lúc mở menu (2026-08-29)

Người dùng phản ánh: bấm "Mua" ở Grace menu bị đơ/giật khoảng 1-2s trước
khi menu hiện ra, cảm giác khác hẳn so với game chạy bình thường rồi từ từ
hiện menu.

Đã thử giải mã `.docs/ermerchant.dll` ("Glorious Merchant", mod cộng đồng
cũng cho phép mua mọi thứ) xem có kỹ thuật nào tốt hơn không - phát hiện
nó **không** dùng `ShopLineupParam`/`OpenRegularShop` kiểu range như mình,
mà hook thẳng vào 1 hàm nội bộ của UI shop để tự trả về danh sách item đã
build sẵn (bỏ qua hẳn cơ chế scan range của game). Làm y hệt vậy đòi hỏi
tìm và hook 1 hàm C++ chưa xác định trong game, rủi ro cao hơn nhiều so
với các patch data/ESD đã làm từ trước đến giờ - người dùng chọn hoãn
hướng đó lại, ưu tiên giải pháp an toàn hơn trước.

Nguyên nhân giật: `CMD_OPEN_REGULAR_SHOP` được gọi với range cố định
`0..9999999` (giống hệt "All Shops" của `Elden-Ring-CT-TGA`) - hàm gốc
`OpenRegularShop` của game quét đồng bộ toàn bộ range đó để build danh
sách hiển thị, nên yêu cầu nó quét ~10 triệu ID gần như trống rỗng mỗi lần
bấm "Mua" chính là nguồn gốc độ giật.

Fix: thêm `shop_lot_range()` - tính 1 lần (lần đầu "Mua" được build, tức
lần đầu ngồi Grace trong phiên chơi) min/max ID THẬT có trong
`ShopLineupParam` qua `SoloParamRepository::rows::<ShopLineupParam>()`,
cache lại bằng `OnceLock`, dùng thay cho hằng số `0..9999999` cố định.
Không xoá `ALL_SHOPS_LOT_START`/`ALL_SHOPS_LOT_END` - giữ làm fallback nếu
`SoloParamRepository` chưa sẵn sàng hoặc không có row nào (không nên xảy
ra thực tế, vì menu Grace chỉ tồn tại khi người chơi đã thực sự vào world).

Chưa test lại trong game xem có thực sự hết giật không - range thật vẫn
có thể còn khá rộng (tất cả thương nhân trong game), nên mức cải thiện
cần đo thực tế, không chỉ suy luận lý thuyết.

## Kết luận điều tra độ giật của "Mua": chấp nhận giới hạn hiện tại (2026-08-29)

Người dùng test lại bản thu hẹp range ở trên: **vẫn giật y hệt**, mỗi lần
bấm đều như nhau. Đo trực tiếp dữ liệu thật (`ShopLineupParam`, log tạm
theo từng `equip_type`) xác nhận: các loại item (vũ khí/giáp/bùa/...)
KHÔNG nằm gọn theo dải ID liên tục - chúng trải gần hết cả không gian
`0..9999999` bất kể loại nào, nên thu hẹp range (dù theo loại hay toàn
cục) không giảm được số dòng thật `OpenRegularShop` phải build. Số dòng
thật đo được: 178 (vũ khí) + 453 (giáp) + 12 (bùa) + 498 (vật phẩm) + 135
+ 1 = ~1277 dòng - đây mới là con số quyết định độ giật, không phải độ
rộng dải ID.

Bằng chứng ủng hộ: bản đầu tiên của tính năng này (chỉ mở riêng shop Twin
Maiden Husks, range hẹp 101800-101899, ~100 dòng thật) được xác nhận
KHÔNG giật (xem mục "GraceMenu: bỏ 'Nâng cấp vũ khí'... thêm thật 'Mua đồ
(Twin Maiden Husks)'" phía trên) - chỉ sau khi gộp thêm mọi thương nhân
khác vào 1 range mới bắt đầu giật. Tỉ lệ thuận với số dòng thật, không
phải độ rộng ID - khớp hoàn toàn với phép đo trên.

Đã giải mã thêm `.docs/ermerchant.dll` ("Glorious Merchant") xem có kỹ
thuật nào tránh được vấn đề này không - phát hiện nó **không hề dùng
`OpenRegularShop` cho việc mua bán tổng hợp**: hàm "Hooking shops..." của
nó chỉ build 1 bảng tra giá (`item_id -> giá`, phục vụ `all_items_free`),
còn menu mua/bán theo từng loại (Vũ khí/Giáp/...) mà người dùng quan sát
được trong game nhiều khả năng dựng từ chính cơ chế `AddTalkListData`
(bank1 id19/id149 - tìm thấy y hệt trong dữ liệu tĩnh của nó) - tức là 1
cây hội thoại dạng danh sách chữ (rẻ), mỗi dòng khi chọn chạy 1 lệnh ESD
"đưa item vào túi + trừ rune" trực tiếp, không mở UI shop nào của game
cả. Không xác định được chính xác lệnh ESD đó (bank/id, số tham số) vì nó
ghi đè lên 1 state đã có sẵn trong đồ thị ESD SỐNG lúc chạy game, không
nằm trong dữ liệu tĩnh của file `.dll` - cần debug runtime thật (gắn
debugger vào tiến trình game) mới lần tiếp được, vượt quá phạm vi đọc
tĩnh bằng Ghidra.

**Quyết định cuối**: dừng điều tra thêm, chấp nhận độ giật ~1-2s hiện tại
của "Mua" như 1 giới hạn đã biết của thiết kế "gộp mọi thương nhân vào 1
range `OpenRegularShop`". Giữ nguyên bản thu hẹp range (`shop_lot_range`)
vì vô hại và đúng về mặt kỹ thuật, dù không giải quyết được độ giật. Muốn
hết giật thật sự sau này cần 1 trong 2 hướng, cả 2 đều tốn công lớn hơn
nhiều so với các patch data/ESD đã làm trong repo này:
- Bỏ qua `OpenRegularShop` hoàn toàn, tự dựng menu hội thoại +
  "give item" trực tiếp như `ermerchant` (không cần hook native nguy
  hiểm, chỉ cần thêm 1 lệnh ESD "cho item" mới vào bộ command đã dùng -
  nhưng cần tìm ra lệnh đó là gì, quy mô hàng nghìn state cho hàng nghìn
  item).
- Giảm phạm vi merchant được gộp (chấp nhận không còn "mua hết mọi thứ
  100%") để giảm số dòng thật cần build.

## Thêm module EnemyScaling: tăng máu + sát thương mọi kẻ thù (2026-08-30)

Tính năng mới theo yêu cầu: tăng máu và sát thương của kẻ thù (không phân
biệt boss/lính thường - người dùng chọn áp dụng cho MỌI kẻ thù, đơn giản
hơn việc phải xác định đúng cờ "là boss" trong `NpcParam`).

`src/enemy_scaling.rs` (module mới ở top-level, ngang hàng `drop_rate`),
2 cơ chế độc lập, cùng gate `Enemy.Enabled`:

- **`Enemy.Health.Multiplier`**: nhân `NpcParam.hp` cho MỌI row, hot-reload
  được - dùng đúng công thức snapshot-gốc-rồi-tính-lại của `drop_rate`
  (không compound qua nhiều lần reload).
- **`Enemy.Damage.SpEffectId`**: không có field "hệ số sát thương" đơn
  giản nào trong `NpcParam` (sát thương tính từ `AtkParam` + chỉ số của kẻ
  tấn công lúc va chạm, không phải 1 con số cố định) - nên dùng cách áp
  1 SpEffect có sẵn của game (tăng attack power) lên mọi kẻ thù mỗi giây,
  qua `ChrInsExt::apply_speffect` (giống cách `torrent_anywhere` đang tự
  áp lại SpEffect 19996 mỗi tick). Không đóng gói sẵn 1 ID mặc định nào -
  chọn đại 1 row `SpEffectParam` có sẵn để tái sử dụng mà không kiểm tra
  còn chỗ nào khác trong game cũng dùng row đó có thể vô tình đổi hành vi
  chỗ khác luôn (đúng bài học từ lịch sử `grace_menu` - không bao giờ
  hijack state có sẵn, luôn tạo state mới). Người dùng tự tìm ID hợp lý
  bằng param editor (SmithBox/DSMapStudio) trước khi bật `!=0`.

Kẻ thù được xác định qua `WorldChrMan.open_field_chr_set` (nhân vật theo
map, loại trừ player/summon linh hồn/ghost) lọc `chr_type == Npc`
(`is_enemy`) - không phân biệt boss/thường, khớp đúng lựa chọn "mọi kẻ
thù" của người dùng.

Chưa test trong game.
