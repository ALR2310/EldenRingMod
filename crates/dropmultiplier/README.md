# DropMultiplier

Mod cho Elden Ring: chỉnh tỉ lệ rớt đồ của quái (`ItemLotParam_enemy`), theo
2 cách loại trừ nhau qua `Mode` - `0` = nhân hệ số lên tỉ lệ mỗi item, `1` =
ép tỉ lệ tổng ("có rớt gì đó") của mỗi dòng về đúng 1 con số cố định, giữ
nguyên tỉ lệ tương đối giữa các item cùng dòng.

## Tách ra từ `sometweaks::drop_rate` (2026-09-14)

Port nguyên bản tính năng `Drop Rate` từ [`sometweaks`](../sometweaks)
(`src/drop_rate/mod.rs`) thành 1 mod độc lập - cùng thuật toán/công thức
hoàn toàn, không đổi hành vi gameplay. `sometweaks` vẫn giữ nguyên tính
năng này song song (không xóa) - ai đang dùng `sometweaks` không cần đổi gì
cả, đây chỉ là thêm 1 lựa chọn dùng riêng lẻ.

Khác biệt so với bản gốc trong `sometweaks`:

- **Bỏ tiền tố `DropRate.`** trong ini: `DropRate.Enabled`/`Mode`/
  `Multiplier`/`ChancePercent` → `Enabled`/`Mode`/`Multiplier`/
  `ChancePercent` (từ 2026-09-28 là `Percentage`, trong `[Drop]`) - không cần tiền tố vì cả file ini của mod này chỉ nói về
  đúng 1 tính năng (khớp quy ước `weightmultiplier`/`runemultiplier`: mod
  đơn tính năng không cần namespace key).
- **Dùng chung [`shared`](../../shared) (`common`)** thay vì tự có bản
  `player.rs`/`task.rs`/`reload.rs` riêng - xem README của `autoregen`, mục
  "Tách `task_hook.rs`.../Gộp crate `engine` ngược vào `shared`" (cùng ngày)
  để biết đầy đủ câu chuyện: các module này (`task_hook`/`alloc_hook`/
  `task`/`player`/`reload`) từng là 1 crate riêng tên `engine` trong vài
  giờ, rồi gộp ngược vào `shared`/`common` cùng ngày - `DropMultiplier` chỉ
  còn biết tới `common::*`, chưa từng thấy dạng `engine` độc lập. Tách ra
  ban đầu vì lúc port mod này nhận ra bộ này sắp bị copy lần thứ 2 (sau
  `sometweaks`/`risearcher`, cả 2 đều tự có bản `task.rs`/`player.rs`/
  `reload.rs` y hệt nhau). `DropMultiplier` là mod **đầu tiên trong
  workspace này khởi tạo mà dùng thẳng bộ này ngay từ đầu**, không phải
  migrate từ code riêng như `autoregen` đã làm.
- Nhờ dùng `common::task`, mod này **không mang theo rủi ro `rva::get()`
  version-lock** mà bản gốc trong `sometweaks` vẫn còn (xem README
  `autoregen`, mục "AOB thay `rva::get()`...") - `common::task::wait_for_cs_task`
  đã dùng cách tra `CSTaskImp::instance()` theo tên (không qua RVA) ngay từ
  đầu.

Không đổi: toàn bộ thuật toán `scale_row`/công thức `ChancePercent`, cách
snapshot `ORIGINAL_BASE_POINTS` để hot-reload không cộng dồn, cách gate
`SoloParamRepository`/`WorldChrMan.main_player` trước khi đọc param - xem
comment đầu `src/drop_rate.rs` cho chi tiết đầy đủ (giữ nguyên từ bản gốc).

## Bỏ prefix "DropMultiplier:" trong log, bỏ timeout 5 phút chờ vào world (2026-09-17)

Test thật trong game phát hiện 2 việc:

- **Log dư thừa**: mọi dòng log đều tự thêm `"DropMultiplier: "` ở đầu - vô
  nghĩa với 1 mod đơn tính năng (log file của chính nó đã là
  `DropMultiplier.log` rồi, không như `sometweaks` gộp nhiều tính năng
  chung 1 log cần tiền tố để phân biệt). Bỏ hết tiền tố này.
- **Race condition thật, tái hiện được**: `wait_for_solo_param_repository`
  (khi đó còn nhận `timeout`) hard-code chờ tối đa 300s (5 phút) rồi bỏ
  cuộc, log `ERROR ... disabled for this session` - nếu người chơi mở game
  xong bận việc khác >5 phút mới thật sự vào world, tính năng chỉnh tỉ lệ
  rớt đồ **không tự áp dụng**, chỉ phục hồi được nếu người dùng tự bấm
  `ReloadKey` (và biết là cần bấm). Sửa tại **hàm dùng chung**
  `common::player::wait_for_solo_param_repository` (không riêng gì
  `DropMultiplier` - `sometweaks`/`risearcher` cũng dính đúng lỗi này, xem
  README của chúng cùng ngày): bỏ hẳn tham số `timeout`, chờ **vô hạn**
  (giống `common::task::wait_for_cs_task` không bao giờ bỏ cuộc), chỉ log
  nhắc nhở mỗi 30s (không phải warning - chờ vài phút để chọn save/load vào
  world là bình thường, khác với `CSTaskImp` chậm bất thường).

## Banner "Config reloaded" trong game khi bấm ReloadKey (2026-09-17)

Hỏi được xác nhận: `AutoRegen` có banner cuộn chữ trên đầu màn hình lúc
reload, `DropMultiplier` thì không. Hàm đó (`show_announcement`, dùng
widget thông báo có sẵn của game, kiểu "Autosaving...") trước đó chỉ là
hàm riêng trong `autoregen/src/regen.rs`, không dùng lại được - chuyển
sang `common::announce::show_announcement` (xem README `autoregen`, mục
cùng ngày) rồi gọi thẳng từ `common::reload::run()` (watcher `ReloadKey`
dùng chung mà mod này đã gọi sẵn) - không cần sửa gì thêm trong
`drop_rate.rs`/`lib.rs`, banner **"Config reloaded"** tự xuất hiện mỗi lần
bấm F5.

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
"Write DropMultiplier.log next to the DLL (for troubleshooting). Off = no log
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


## Nhãn cấp độ log bỏ khoảng trắng thừa: `[INFO ]` → `[INFO]` (2026-09-26)

`common::logger` trước đây căn cột level cho đủ 5 ký tự, nên
mọi dòng INFO/WARN in ra `[INFO ]`/`[WARN ]`. Mục đích là để cột nội dung
thẳng hàng, nhưng khoảng trắng bên trong dấu ngoặc trông như lỗi gõ, nên đã
bỏ: giờ in đúng `[INFO]`, `[WARN]`, `[ERROR]`, `[DEBUG]`. Sửa 1 chỗ trong
`shared/src/logger.rs`, áp dụng cho mọi mod. Không đổi hành vi.


## Log in phiên bản game + danh sách DLL đã nạp (2026-09-26)

Để log gửi kèm báo lỗi tự trả lời được "khác phiên bản game?" và "có mod nào
khác đang chạy cùng?", mod giờ ghi thêm vào log (khi `LogFile` bật) 1 dòng
`Game: eldenring.exe v<version> base=0x.. size=0x.. ts=0x..` và danh sách mọi DLL
không nằm trong thư mục Windows (tên, version, base, size), kiểu header của
MapForGoblins. Code ở module mới `common::diag` (`shared/src/diag.rs`, chi
tiết trong `crates/weightmultiplier/README.md` cùng ngày). Chỗ gọi: `lib.rs`, ngay trước khi chạy tính năng chính, qua `common::diag::log_environment_when_game_ready()` (chờ `CSTaskImp` rồi mới ghi, lúc đó mọi DLL đã nạp xong; lần chờ `CSTaskImp` sau của mod dùng lại kết quả đã cache).
Không đổi hành vi.

Giữ quyền riêng tư để người dùng yên tâm dán log công khai (bình luận
Nexus): danh sách bỏ qua chính exe (đã có ở dòng `Game:`), các DLL đi kèm
game (`bink2w64`, `amd_ags_x64`, `oo2core_6_win64`, `EOSSDK-Win64-Shipping`,
cả `steam_api64` - bản bị thay thế sẽ lộ là bản crack, không nên bắt người
dùng khai ra chỉ để được hỗ trợ; `OnlineFix64` cũng ẩn vì lý do này) và các DLL do Steam client tự chèn vào
(`steamclient64`, `tier0_s64`, `vstdlib_s64`, `gameoverlayrenderer64`);
tiêu đề ghi `Loaded modules (<hiện>/<tổng>):`. DLL trong thư mục game
in theo đường dẫn tương đối (`modengine2\bin\lua.dll` - nhìn là biết thuộc
loader nào); DLL ngoài thư mục game chỉ in tên file, không bao giờ in đường
dẫn đầy đủ (có thể chứa tên tài khoản Windows, `C:\Users\<tên>\...`).


## Sửa panic với regulation của Convergence: bỏ `rows_mut()`, duyệt theo index (2026-09-27)

Báo lỗi trên Nexus (bản 1.0.1, game 1.17 + Convergence 3.0.2.0 + Seamless
Co-op; người báo nói vanilla cũng lỗi nhưng mình không tái tạo được): mod
không có tác dụng, log dừng ngay sau `snapshotting ItemLotParam_enemy...`
bằng panic `param_repository.rs:357:22: called Option::unwrap() on a None
value`. Bấm ReloadKey vẫn hiện banner (banner đến từ `common::reload`, task
riêng), nhưng thread của mod đã chết trước khi kịp đăng ký task reload riêng
và mutex snapshot bị poisoned, nên drop rate không bao giờ đổi.

Tái tạo: vanilla 1.17 chạy bình thường (5135 dòng); bản 1.16.2 có Convergence
panic y hệt. Dòng 357 nằm trong `ParamFile::rows_mut()` của fromsoftware-rs:
`get_row_by_index_mut(lookup.index).unwrap()`. Hàm đó duyệt lookup table
(cặp `(param_id, index)` sắp theo ID) mà game gắn sau mỗi param file, nhưng
định cỡ bảng theo `row_count` trong header file (`u16`).

Đã thử/bỏ: ban đầu nghi tràn `u16` (Convergence có hơn 65535 dòng), bỏ vì
log chẩn đoán cho thấy header 4631 dòng nhưng số dòng runtime (`u32` trong
metadata 0x10 byte trước file, thứ game thật sự dùng để dựng lookup table) là
**4630**. Tức là game dựng bảng thiếu 1 phần tử so với header (nhiều khả năng
file của Convergence có 1 ID dòng bị trùng), fromsoftware-rs đọc lố 1 phần tử
ra vùng nhớ rác, ra index vượt `row_count` rồi panic. Cũng đã loại khả năng
khác bố cục param giữa 1.16.2 và 1.17 (slot 20 đúng là `ItemLotParam_enemy` /
`ITEMLOT_PARAM_ST`).

Sửa:
- Module mới `common::params` (`shared/src/params.rs`):
  `for_each_row_mut::<P>()` duyệt theo index qua row descriptor của chính
  file (`get_row_by_index_mut` tới khi trả `None`), không đụng lookup table;
  `check::<P>()` xác minh slot thật sự chứa đúng param trước khi ghi
  (fromsoftware-rs chỉ kiểm tra bằng `debug_assert!`, bản release không có);
  `describe::<P>()` in tên resource/struct, paramdef version, số dòng header
  và runtime cho log báo lỗi.
- `drop_rate.rs`: snapshot đổi từ `HashMap<u32, Points>` (theo ID) sang
  `Vec<Points>` (theo index, ổn định trong 1 phiên); mutex dùng
  `unwrap_or_else(into_inner)` để không panic dây chuyền; `apply` trả
  `Option<usize>`, `None` (kèm log `[ERROR]`) nếu `check` không qua.

Đã test in-game trên 1.16.2 + Convergence: snapshot + áp 4631 dòng, hotkey
reload chạy bình thường. Dòng log mới:
`snapshotting ItemLotParam_enemy (slot 20): resource 'ItemLotParam_enemy', struct 'ITEMLOT_PARAM_ST' v4, 4631 row(s) (runtime: 4630)...`

Các chỗ khác trong workspace còn dùng `rows()`/`rows_mut()` (risearcher
`EquipParamWeapon`, sometweaks `ShopLineupParam`/`EquipParamGem`/...,
infiniteailment `SpEffectParam`) chưa đổi: chỉ panic nếu đúng param đó bị
lệch header/runtime trong regulation đang dùng; người báo lỗi xác nhận
Infinite Ailments chạy tốt trên Convergence. `sometweaks::drop_rate` (code
gốc của mod này) đã sửa cùng lúc.


## Tính năng mới `[Materials]`: nhân số lượng nguyên liệu; `[Settings]` → `[Drop]`, `ChancePercent` → `Percentage` (2026-09-28)

Đề xuất từ người báo lỗi Convergence trên Nexus (sau khi 1.0.2 sửa xong): cho
multiplier ảnh hưởng cả nguyên liệu craft / đồ hái ngoài thế giới mở. Tìm
trên Nexus không thấy mod DLL nào làm việc này - các mod tương tự
(#2238 "Reasonable Material Drop rate and Amount Increase", #537 "Better
Gathering and Hunting") đều thay nguyên `regulation.bin`, sửa tay từng món,
xung đột với overhaul, không có DLC.

**Thiết kế ini** (chốt cùng người dùng sau vài vòng):
```ini
[Materials]
Crafting=1   ; nguyên liệu craft farm được
Upgrade=1    ; nguyên liệu nâng cấp farm được
Unique=1     ; mọi nguyên liệu chỉ nhặt được 1 lần
```
Đã thử/bỏ: chia 2 key theo nguồn `Farmable`/`Unique` (không tách craft và
nâng cấp) và 1 key duy nhất cho mọi nguyên liệu. Chọn 3 key vì tách được
nhóm ảnh hưởng cân bằng nhiều nhất (Scadutree Fragment, Sacred Tear,
Smithing Stone trên xác - đều nằm ở `Unique`) khỏi nhóm farm vô hại.
Cân nhắc tên `Repeatable`/`OneTime` rồi bỏ: `Multiplier` trong tên key dễ
nhầm với tỉ lệ rơi, "Unique" dùng được vì section `[Materials]` đã giới
hạn ngữ cảnh (không bị hiểu là vũ khí độc nhất).

**Phân loại mỗi ô** (`src/materials.rs`), dựa trên export vanilla 1.17
(`.docs/ItemLotParam_map.csv`, `.docs/EquipParamGoods.csv`):
- ô không phải Goods (`lotItemCategory0N != 1`), hoặc Goods có
  `EquipParamGoods.goodsType` khác 2 (nguyên liệu craft, 106 món) / 14
  (nâng cấp: Smithing/Somber Stone, Glovewort, Golden Seed, Sacred Tear,
  Scadutree Fragment, Revered Spirit Ash) → không đụng. Nhờ vậy vũ khí, key
  item, đồ tiêu hao (Flask, Kukri, Golden Rune - đều `goodsType 0`) và các
  dòng lạ như `ItemLotParam_map` ID `2` (quay ngẫu nhiên bình Crimson Tears,
  không cờ, 60% trắng tay) bị loại. Bài học: "không có cờ" không đồng nghĩa
  với "điểm hái" - phải lọc theo `goodsType` trước;
- có cờ nhặt (`getItemFlagId` của dòng hoặc `getItemFlagId0N` của ô khác 0)
  → `Unique`;
- còn lại → `Crafting`/`Upgrade` theo `goodsType`.

Điểm hái là các dòng `ItemLotParam_map` không cờ ID `9965xx`–`9993xx` (base
game) và `463xxxx` (DLC), vd. `997200` Rowa Fruit ra 1/2/3/5. Smithing Stone
farm được đến từ quái hồi sinh (`ItemLotParam_enemy`: thợ mỏ, golem, lính),
không có "mạch quặng" hồi lại. Mô phỏng trên CSV vanilla: 1602 ô crafting,
1046 upgrade, 1659 unique.

Nhân `lotItemNum0N` (`u8`): làm tròn, tối thiểu 1, tối đa 255; ô số lượng 0
giữ nguyên. Snapshot số lượng gốc + nhóm của từng ô 1 lần (cùng pattern
baseline với `drop_rate`), mỗi lần áp/reload tính lại từ snapshot. Chạy sau
`drop_rate::apply` cả lúc khởi động lẫn khi bấm ReloadKey; 2 tính năng ghi
2 cột khác nhau (`lotItemBasePoint` vs `lotItemNum`) nên độc lập.

`goodsType` đọc từ `EquipParamGoods` lúc chạy (không hardcode ID) để overhaul
như Convergence tự phân loại đúng đồ của nó. Tra theo ID mà không đi qua
`repo.get()` (binary search trên chính lookup table gây panic ở mục trước):
thêm `common::params::row_ids::<P>()` đọc ID thẳng từ row descriptor, tự đối
chiếu data offset với `get_row_by_index` - lệch là trả `None` (log lỗi, bỏ
qua tính năng) chứ không ra ID sai.

**Đổi tên ini** (người dùng tự sửa template): `[Settings]` → `[Drop]`,
`ChancePercent` → `Percentage`. `common::config` bỏ qua section nên chỉ đổi
tên key là có ảnh hưởng: trước đây migrate sẽ đẩy `ChancePercent` cũ vào
`[Legacy]` và đặt lại `Percentage` về mặc định, làm mất giá trị người dùng
đã chỉnh. Thêm `common::config::load_or_create_default_with_renames` (kèm
unit test): key cũ có mà key mới chưa có → chuyển giá trị sang key mới, bỏ
key cũ. `lib.rs` truyền `("ChancePercent", "Percentage")`.

Test in-game do người dùng xác nhận ổn (2026-09-28).
