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

- `Regen` (hồi HP/FP/Stamina theo thời gian + theo đòn đánh trúng) — xem
  `src/regen.rs` + `src/regen/attack_hook.rs`.
- `Rune Reward` (cộng rune theo thời gian + bonus mốc thời gian, port từ
  [`PassiveRunes`](../passiverunes)) — xem `src/rune_reward.rs`.
- `Rune.Multiplier` (`[Rune Reward]`, nhân hệ số rune nhận được từ mọi
  nguồn, port từ [`RuneMultiplier`](../runemultiplier)) và
  `WeightMultiplier` (`[Misc]`, nhân hệ số Trọng Tải, ảnh hưởng cả số hiển
  thị lẫn roll-type thực tế, port từ
  [`WeightMultiplier`](../weightmultiplier) — **đã test trong game, hoạt
  động đúng**) — 2 hook nhỏ, độc lập nhau, gộp chung vào
  `src/multipliers.rs` (module con `rune_multiplier`/`weight_multiplier`,
  xem mục 2026-08-24 bên dưới).
- `Drop Rate` (`DropRate.Multiplier`/`DropRate.ChancePercent`, chọn 1 trong
  2 qua `DropRate.Mode` - nhân hệ số hoặc ép cứng % tổng cộng "rớt được
  item gì đó" khi giết quái qua `ItemLotParam_enemy`, có hot-reload —
  **đã test `ChancePercent` trong game, hoạt động đúng**: `100` → rớt cùng
  lúc toàn bộ ~6 item gắn với 1 con Godrick Soldier (đúng vì mỗi row lot
  của quái là 1 lượt roll độc lập, `100` ép từng row); `1` → 2/3 lần giết
  có rớt (mẫu quá nhỏ để kết luận % chính xác, nhưng không có dấu hiệu sai)
  — xem `src/drop_rate.rs`.

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
của `fromsoftware-rs`/`libER`), các module còn lại (TorrentAnywhere/Spirit
trong ini hiện chỉ là placeholder). **Đã thử và bỏ:** cho phép dùng Site
of Grace khi cưỡi Torrent - cần viết EMEVD event mới, ngoài phạm vi kiến
trúc hiện tại (xem mục 2026-08-24 "Bỏ hẳn Misc.GraceOnTorrent").

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
