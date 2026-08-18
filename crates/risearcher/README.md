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
- **`src/lib.rs`** — không cần `CSTaskImp`/theo dõi tick như các mod khác:
  param `regulation.bin` chỉ load 1 lần lúc game khởi động và không reload
  lại giữa phiên chơi, nên chỉ cần chờ `SoloParamRepository::instance_mut()`
  sẵn sàng (poll mỗi 200ms, timeout 60s) rồi patch đúng 1 lần.

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

## Config

Xem `RiseArcher.ini` cho toàn bộ key + comment. Mọi giá trị dùng đúng quy
ước "0 tắt tính năng" ở những chỗ hợp lý (`MaxQuantity`, các multiplier
không có khái niệm "tắt" vì luôn cần 1 hệ số > 0).

## Trạng thái

| Việc | Trạng thái |
|---|---|
| RE gốc (AOB/field/ID pattern qua CSV thật) | Xong (bản `.MASSEDIT`) |
| Port sang Rust DLL (`weapon.rs` + `bullet.rs`) | Xong, build được, không warning |
| Test trong game | **Chưa** |

## Rủi ro cần lưu ý khi test

- `bullet::apply` tự đếm số ID không tìm thấy (`missing`) và log warning
  nếu > 0 — đây là tín hiệu game đã update và danh sách base ID trong
  `bullet.rs` cần dò lại từ CSV export mới (Smithbox → export
  `Bullet.csv` → so khớp ID).
- Chưa xác nhận trong game các phép scale số nguyên (`u16` damage field,
  `u8 max_arrow_quantity`) có làm tròn đúng ý muốn không — công thức dùng
  `.round()` trước khi ép kiểu, khác hành vi chính xác của Smithbox Mass
  Edit (không rõ nó dùng round hay truncate).
