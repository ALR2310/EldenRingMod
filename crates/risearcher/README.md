# RiseArcher

Mod cho Elden Ring: buff toàn diện Bow/Crossbow/Ballista/Arrow/Bolt (sát
thương, scaling, trọng lượng, số lượng mang tối đa, tốc độ/tầm/số lượng
mũi tên của Ash of War loại Rain of Arrows...).

## 2 cách dùng song song trong project này

Từ 2026-08-18, project này **giữ cả 2 cách triển khai** thay vì thay thế
hẳn cách cũ:

- **DLL (`src/`, khuyến nghị)** — patch sống qua `SoloParamRepository`,
  cấu hình qua `RiseArcher.ini`, không đụng `regulation.bin` trên đĩa. Xem
  mục "Bản DLL Rust" bên dưới.
- **Static `regulation.bin` qua Smithbox (`csv/`, `.smithbox/`)** — cách
  làm gốc, vẫn giữ nguyên: `csv/RiseArcher.MASSEDIT` + `csv/*.csv` để dán
  vào Smithbox Mass Edit/CSV import, `.smithbox/` là cache project
  (machine-specific, git-ignored). Hữu ích khi cần merge tay với 1 mod
  `regulation.bin` khác cụ thể, hoặc khi không muốn cài thêm DLL nào.

**Lưu ý quan trọng: 2 cách này độc lập, không tự đồng bộ.** Hệ số trong
`RiseArcher.ini` và trong `csv/RiseArcher.MASSEDIT` hiện đang khớp nhau
(cùng giá trị mặc định `1.5`/`2`/`3`/`4`/`0.5`/`255`/`3.5`/`2`/`6`...) vì
bản DLL được port trực tiếp từ giá trị hardcode trong MASSEDIT — nhưng sửa
1 bên (ví dụ đổi `RiseArcher.ini`) sẽ **không** tự cập nhật bên kia. Nếu
sau này đổi hệ số mặc định, nhớ sửa cả 2 chỗ nếu muốn giữ chúng khớp nhau.

## Bản DLL Rust hiện tại (2026-08-18)

Thêm **DLL đọc/ghi param sống**, bổ sung bên cạnh cách sửa tĩnh
`regulation.bin` gốc (`.MASSEDIT`/CSV qua Smithbox) — đúng hướng đi đã chốt
trong README cũ (mục "Quyết định: sẽ chuyển sang DLL + `libER`"), nhưng
dùng [`fromsoftware-rs`](https://github.com/vswarte/fromsoftware-rs) (Rust)
thay vì `libER` (C++) như dự tính ban đầu — không cần nữa vì
`fromsoftware-rs` đã có sẵn `SoloParamRepository::get_mut::<EquipParamWeapon>`/
`rows_mut::<Bullet>` với struct `EQUIP_PARAM_WEAPON_ST`/`BULLET_PARAM_ST`
map đủ mọi field đang dùng, không cần tự dò offset struct nào.

**2 lý do làm bản DLL (đúng như đã ghi trong README cũ, giờ đã làm):**
1. **Người dùng tự chỉnh theo ý thích** — mọi hệ số nhân/gán (trước đây
   hardcode trong `.MASSEDIT`: `* 1.5`, `* 2`, `= 255`...) giờ đọc từ
   `RiseArcher.ini`.
2. **Không còn xung đột `regulation.bin` với mod khác** — DLL patch trực
   tiếp vào bộ nhớ process game lúc chạy, không đụng tới file
   `regulation.bin` trên đĩa nữa. Không cần Smithbox Mass Edit để merge
   với mod khác như hướng dẫn cũ trong `DESCRIPTION.bbcode`.

Sống ở `crates/risearcher` trong workspace [`EldenRingMod`](../../README.md).

### Kiến trúc

- **`src/weapon.rs`** — phần `EquipParamWeapon` (Bow/Crossbow/Ballista/
  Arrow/Bolt), y hệt bộ lọc gốc trong `.MASSEDIT` (`weaponCategory`/
  `wepType`/loại trừ `sortId == 9999999`), lặp qua `rows_mut::<EquipParamWeapon>()`
  1 lần thay vì hàng chục dòng Mass Edit riêng cho từng field.
- **`src/bullet.rs`** — phần `Bullet` (Arrow/Great Arrow/Radahn's Spear/
  Bolt). Đây là phần *không* thể lọc theo field chung (đã xác nhận trong
  RE gốc — xem `csv/RiseArcher.MASSEDIT` cũ), nên vẫn phải liệt kê ID —
  nhưng thay vì ~1000 dòng Mass Edit tay, giờ chỉ còn danh sách base ID
  (63 ID: 32 Arrow + 6 Great Arrow + 1 Radahn's Spear + 24 Bolt) ghép với
  4 bảng offset cố định (`ARROW_OFFSETS`/`GREAT_ARROW_OFFSETS`/
  `RADAHNS_SPEAR_OFFSETS`/`BOLT_OFFSETS`) — đúng cấu trúc đã RE ra trong
  README cũ, chỉ viết lại thành data + vòng lặp thay vì liệt kê tay.
- **`src/lib.rs`** — chờ `player::wait_for_solo_param_repository` sẵn sàng
  (poll mỗi 200ms, timeout 300s) rồi patch lần đầu (bọc `apply_with_retry`,
  tự thử lại nếu panic); sau đó đăng ký 1 task `CSTaskImp::FrameBegin` (qua
  `src/task.rs`) chỉ để theo dõi `reload::RELOAD_GENERATION` và patch lại
  mỗi khi người dùng nhấn `General.ReloadKey` — xem mục "Hot reload" bên
  dưới.
- **`src/player.rs`** — port từ `sometweaks::player`: chờ player thực sự vào
  world (`WorldChrMan::main_player`) trước khi đụng `SoloParamRepository` -
  xem mục "Bug: crash lặng lúc khởi động" bên dưới.
- **`src/reload.rs`** / **`src/task.rs`** — port nguyên khối từ
  `sometweaks::reload`/`sometweaks::task` (xem mục "Hot reload" bên dưới).

### Ánh xạ field (MASSEDIT → Rust, không đổi ý nghĩa)

| MASSEDIT (`weaponCategory`/field) | Rust (`EQUIP_PARAM_WEAPON_ST`) |
|---|---|
| `weaponCategory` | `weapon_category()` |
| `wepType` | `wep_type()` |
| `sortId` | `sort_id()` |
| `attackBasePhysics/Magic/Fire/Thunder/Dark` | `attack_base_physics/magic/fire/thunder/dark()` (u16) |
| `correctStrength/Agility/Magic/Faith/Luck` | `correct_strength/agility/magic/faith/luck()` (f32) |
| `gemMountType` | `gem_mount_type()` |
| `weight` | `weight()` (f32) |
| `sellValue` | `sell_value()` (i32) |
| `maxArrowQuantity` | `max_arrow_quantity()` (u8) |

| MASSEDIT (`Bullet` field) | Rust (`BULLET_PARAM_ST`) |
|---|---|
| `initVellocity`/`maxVellocity` | `init_vellocity/max_vellocity()` (f32) |
| `dist` | `dist()` (f32) |
| `numShoot` | `num_shoot()` (u16) — **gán tuyệt đối** (`RainOfArrowsCount`), không nhân, vì giá trị gốc luôn = 1 |
| `shootAngleYMaxRandom`/`shootAngleXMaxRandom` | `shoot_angle_y/x_max_random()` (f32) |

## Hot reload (2026-09-08)

Trước bản này, `RiseArcher.ini` chỉ đọc **đúng 1 lần** lúc DLL attach —
lý do ghi trong code cũ: "regulation param rows chỉ load 1 lần lúc khởi
động và không bao giờ reload giữa phiên chơi, nên 1 lần patch là đủ". Đúng
với params tự nó, nhưng bỏ sót 1 điều: `SoloParamRepository` vẫn là bộ nhớ
sống suốt phiên chơi, patch lại (ghi đè) hoàn toàn khả thi — chỉ là chưa có
cơ chế trigger. Giờ thêm hot reload giống `sometweaks` (mặc định phím
**F5**, đổi qua `General.ReloadKey`):

- `src/reload.rs`/`src/task.rs` port gần như nguyên khối từ
  `sometweaks::reload`/`sometweaks::task` — theo dõi `ReloadKey` trên task
  `CSTaskImp::FrameBegin`, `config::load()` lại ini rồi tăng biến đếm
  `RELOAD_GENERATION`.
- `weapon::apply`/`bullet::apply` từng scale **trực tiếp trên giá trị đang
  đọc từ row** (`row.attack_base_physics() * factor`) — gọi lại lần 2 sẽ
  compound (`* 1.5` hai lần thành `* 2.25`, không phải vẫn `* 1.5`), y hệt
  vấn đề `drop_rate` trong `sometweaks` đã gặp. Sửa bằng cách cache toàn bộ
  giá trị gốc của từng row (`weapon::Baseline`/`bullet::Baseline`, khoá theo
  row ID) ngay lần `apply` đầu tiên, và mọi lần sau — kể cả reload — luôn
  scale từ baseline đó, không bao giờ từ giá trị đang có trên row.
- Field không bị compound thì giữ nguyên cách gán tuyệt đối, không cần
  baseline: `max_arrow_quantity` (Arrow/Bolt `MaxQuantity`), `num_shoot`
  (`Bullet.RainOfArrowsCount`), `gem_mount_type` (luôn gán `2`).

Đổi tên key debug log: `[Debug] DebugLog=true` → `[Logging] LogFile=true`
(chỉ đổi tên, không đổi hành vi — vẫn gate việc tạo file `RiseArcher.log`).

## Bug: crash lặng lúc khởi động, `SoloParamRepository` ready quá sớm (2026-09-08)

Test in-game đầu tiên (sau khi thêm hot reload ở trên) lộ ra: RiseArcher hoàn
toàn không hoạt động — Black Bow không mở khoá được Ash of War, số lượng mũi
tên vẫn 99 vanilla. `RiseArcher.log` dừng đột ngột ngay sau dòng
`"SoloParamRepository ready, applying weapon buffs..."`, không có dòng
`"Applied to..."` lẫn dòng lỗi nào - dấu hiệu đặc trưng của 1 thread Rust
panic không ai bắt (dưới profile `panic = "unwind"` của workspace, panic ở
1 thread `std::thread::spawn` tự chết trong im lặng, không crash game, không
log gì - xem comment gốc trong `Cargo.toml`).

Đọc thẳng source `fromsoftware-rs` xác nhận: `SoloParamRepository::
instance_mut()` trả `Ok` ngay khi object tồn tại (đúng như code cũ giả định
"regulation.bin chỉ load 1 lần lúc khởi động") - nhưng **sớm hơn** lúc các
file resource của từng param cụ thể (`EquipParamWeapon`, `Bullet`...) load
xong. Gọi `rows_mut::<EquipParamWeapon>()` lúc đó rơi vào
`SoloParamRepository::get_param_file_mut`'s
`.expect("Expected param holder to have exactly one res cap")` → panic,
không phải trả `Err` để retry được.

Y hệt bug đã gặp (và đã fix) trong `sometweaks::drop_rate`
(2026-08-24/25, xem `sometweaks/src/player.rs`'s doc comment) -
`unlock_ashes_of_war` (chính là tính năng tương đương `Bow.UnlockAOW` ở
đây) chạy được trong `sometweaks` chính vì nó chờ qua
`player::wait_for_solo_param_repository` (gate thêm điều kiện
`WorldChrMan::main_player` đã tồn tại), không chỉ chờ
`instance_mut().is_ok()` suông như RiseArcher đang làm.

Sửa bằng cách port nguyên `sometweaks::player`'s 2 hàm
(`main_player_chr_ins_ptr`/`wait_for_solo_param_repository`) sang
`src/player.rs` của RiseArcher, dùng nó thay `wait_for_repository` cũ (bỏ
hẳn hàm này). Thêm lớp phòng hờ thứ 2 (`apply_with_retry` trong `lib.rs`):
bọc lần `apply` đầu bằng `catch_unwind`, tự thử lại tối đa 30s nếu vẫn
panic - đi kèm sửa `weapon::ORIGINALS`/`bullet::ORIGINALS`'s
`.lock().unwrap()` → phục hồi mutex bị "poison" sau 1 lần panic, để lần
retry sau không panic dây chuyền ngay tại bước lock.

## Bug: `RiseArcher.ini` không hề có tác dụng trước bản này (2026-09-08)

Phát hiện khi tổ chức lại section cho ini (theo yêu cầu người dùng, xem
ngay dưới): `weapon.rs`/`bullet.rs` đọc key có tiền tố (`Bow.DamageMultiplier`,
`Crossbow.DamageMultiplier`, `Arrow.MaxQuantity`, `Bullet.SpeedMultiplier`...)
nhưng `RiseArcher.ini` (từ commit đầu tiên của bản DLL, 2026-08-18) lại ghi
key **trần, không tiền tố** (`DamageMultiplier`, `MaxQuantity`...). Vì
`common::config` là map phẳng, bỏ qua section header hoàn toàn (xem
`shared/src/config.rs`), nên **không key nào trong số này từng khớp** —
mọi giá trị nhân/gán luôn chạy bằng default hard-code trong Rust, chỉnh
`RiseArcher.ini` trước bản này không có tác dụng gì. Chỉ `ReloadKey`/
`LogFile` (2 key duy nhất code đọc không tiền tố) là hoạt động đúng.
Đây cũng chính là lý do hợp lý cho mục "Test trong game: Chưa" — chưa ai
kiểm chứng việc chỉnh ini có ăn không, nên bug tồn tại từ đầu mà không ai
phát hiện.

Sửa bằng cách viết lại `RiseArcher.ini` với đúng key có tiền tố khớp code
(xem mục "Tổ chức lại section" ngay dưới). Người dùng nâng cấp từ bản cũ:
`config::migrate` sẽ tự thêm mọi key tiền tố mới (giá trị mặc định) và dời
key trần cũ (nếu người dùng từng chỉnh, dù không có tác dụng) vào
`[Legacy]` — không mất dữ liệu, nhưng giá trị custom cũ (nếu có) cần chỉnh
lại thủ công theo tên key mới.

## Tổ chức lại section theo tính năng thay vì theo loại vũ khí (2026-09-08)

Section cũ đặt tên theo loại vũ khí (`[Bow]`, `[Crossbow]`, `[Ballista]`...),
mỗi section lại trộn nhiều loại tuỳ chỉnh khác nhau (damage, scaling, ash of
war) - khó dò khi muốn sửa "mọi hệ số damage" hay "mọi giới hạn số lượng"
cùng lúc. Đổi sang đặt tên section theo **loại tuỳ chỉnh**: `[General]`,
`[Damage Multipliers]`, `[Common]` (scaling/Ash of War/weight/sell value),
`[Bullet Tuning]` (gồm cả `MaxQuantity` của Arrow/Bolt), `[Logging]`. Mỗi
dòng key vẫn giữ tiền tố (`Bow.`/`Crossbow.`/`Ballista.`/`Bullet.`) kèm 1
dòng comment nói rõ áp dụng cho vũ khí nào - section header vẫn chỉ là nhãn
hiển thị cho người dùng (bị `common::config` bỏ qua hoàn toàn), không ảnh
hưởng logic đọc key.

Cùng đợt, người dùng tự viết lại bộ key theo hướng chi tiết hơn bản đầu của
Claude (không còn multiplier `BowCrossbowBallista.*` dùng chung cho cả 3):

- `Bow.AllowAshOfWar` → **`Bow.UnlockAOW`** (đổi tên, không đổi hành vi -
  vẫn gate `row.set_gem_mount_type(2)`).
- `BowCrossbowBallista.WeightMultiplier`/`SellValueMultiplier` (1 cặp dùng
  chung cho cả 3 loại) → tách riêng theo từng loại:
  `Bow.WeightMultiplier`/`Bow.SellValueMultiplier`,
  `Crossbow.WeightMultiplier`/`Crossbow.SellValueMultiplier`,
  `Ballista.WeightMultiplier`/`Ballista.SellValueMultiplier` - cùng giá trị
  mặc định `0.5`/`2` như cũ, nhưng giờ chỉnh riêng từng loại được.
- `Arrow.MaxQuantity`/`Bolt.MaxQuantity` → **`Bullet.Arrow.MaxQuantity`**/
  **`Bullet.Bolt.MaxQuantity`** (đổi tên cho khớp section `[Bullet Tuning]`
  - field thật vẫn là `max_arrow_quantity` trên `EquipParamWeapon`, do
    `weapon.rs` áp, không phải `bullet.rs`).
- `weapon.rs` được viết lại theo `enum WeaponKind` (`Bow`/`Crossbow`/
  `Ballista`/`Arrow`/`Bolt`) phân loại 1 lần từ `weaponCategory`/`wepType`,
  dùng chung cho cả việc lọc row liên quan (`is_relevant`) lẫn chọn hệ số
  áp dụng trong `apply` - tránh lặp lại cùng 1 khối `match` category/wepType
  2 lần như bản đầu.
- Người dùng gõ nhầm `ShellValueMultiplier` (thay vì `SellValueMultiplier`)
  lúc soạn lại ini - đã xác nhận là gõ nhầm và sửa lại đúng chính tả.

## Config

Xem `RiseArcher.ini` cho toàn bộ key + comment. Mọi giá trị dùng đúng quy
ước "0 tắt tính năng" ở những chỗ hợp lý (`MaxQuantity`, các multiplier
không có khái niệm "tắt" vì luôn cần 1 hệ số > 0).

## Trạng thái

| Việc | Trạng thái |
|---|---|
| RE gốc (AOB/field/ID pattern qua CSV thật) | Xong (bản `.MASSEDIT`) |
| Port sang Rust DLL (`weapon.rs` + `bullet.rs`) | Xong, build được, không warning |
| Hot reload (`ReloadKey`, baseline chống compound) | Xong, build được |
| Test trong game | **Đã test (2026-09-08)** - áp đúng 99 `EquipParamWeapon` + 352 `Bullet` row, xem "Bug: crash lặng lúc khởi động" |

## Rủi ro cần lưu ý khi test

- `bullet::apply` tự đếm số ID không tìm thấy (`missing`) và log warning
  nếu > 0 — đây là tín hiệu game đã update và danh sách base ID trong
  `bullet.rs` cần dò lại từ CSV export mới (Smithbox → export
  `Bullet.csv` → so khớp ID).
- Chưa xác nhận trong game các phép scale số nguyên (`u16` damage field,
  `u8 max_arrow_quantity`) có làm tròn đúng ý muốn không — công thức dùng
  `.round()` trước khi ép kiểu, khác hành vi chính xác của Smithbox Mass
  Edit (không rõ nó dùng round hay truncate).
