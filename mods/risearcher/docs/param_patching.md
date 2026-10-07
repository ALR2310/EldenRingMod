# RiseArcher: patch param sống cho Bow / Crossbow / Ballista / Arrow / Bolt

**Status 2026-10-07: ĐANG PHÁT HÀNH, test trong game (2026-09-08): áp đúng 99
dòng `EquipParamWeapon` và 352 dòng `Bullet`.** Code:
`mods/risearcher/src/weapon.rs`, `src/bullet.rs`, `src/lib.rs`. Hai sự cố khi
khởi động và khi đọc ini nằm ở `startup_and_ini_bugs.md`.

## Hai cách triển khai, cùng tồn tại

Từ 2026-08-18 project giữ **cả hai**:

1. **DLL Rust (`src/`, khuyến nghị):** patch sống `SoloParamRepository` bằng
   `fromsoftware-rs`, không đụng `regulation.bin` trên đĩa nên không xung đột
   với mod `regulation.bin` khác; mọi hệ số đọc từ `RiseArcher.ini`.
2. **Sửa tĩnh `regulation.bin` qua Smithbox:** `csv/RiseArcher.MASSEDIT` cùng
   `csv/Bullet.csv`, `csv/EquipParamWeapon.csv` để dán vào Smithbox Mass Edit /
   CSV import; `.smithbox/` là cache project (machine-specific, git-ignored).
   Hữu ích khi cần merge tay với một mod `regulation.bin` cụ thể hoặc không muốn
   cài DLL.

**Hai cách này độc lập, không tự đồng bộ.** Hệ số mặc định của DLL được port
trực tiếp từ giá trị hardcode trong `.MASSEDIT` nên hiện khớp nhau, nhưng sửa
một bên (vd. ini) không cập nhật bên kia. Đổi hệ số mặc định thì sửa cả hai nếu
muốn chúng khớp.

Hướng DLL dùng `fromsoftware-rs` (Rust) thay vì `libER` (C++) như dự tính ban
đầu, vì `SoloParamRepository` đã có sẵn accessor `EQUIP_PARAM_WEAPON_ST` /
`BULLET_PARAM_ST` map đủ mọi field đang dùng, không cần tự dò offset.

## `EquipParamWeapon` (`weapon.rs`)

Bộ lọc y hệt `.MASSEDIT`, chạy một lần trên mọi dòng thay vì hàng chục lệnh Mass
Edit riêng cho từng field:

| Loại | Điều kiện |
|---|---|
| Bow / Greatbow | `weaponCategory == 10` |
| Crossbow | `weaponCategory == 11` và `wepType == 55` |
| Ballista | `weaponCategory == 11` và `wepType == 56` (Crossbow và Ballista chung category, tách bằng `wepType`) |
| Arrow | `weaponCategory == 13` |
| Bolt | `weaponCategory == 14` |

`sortId == 9999999` là quy ước của FromSoftware cho dòng trùng chỉ dành cho NPC
(ẩn khỏi menu người chơi), luôn bị loại, đúng như `!prop sortId 9999999` trên
mọi lệnh Mass Edit gốc. `enum WeaponKind` phân loại một lần và dùng chung cho cả
việc lọc dòng liên quan lẫn chọn hệ số áp dụng.

Field được chỉnh (tên MASSEDIT → accessor Rust): `attackBase*` (Physics/Magic/
Fire/Thunder/Dark, `u16`), `correct*` (Strength/Agility/Magic/Faith/Luck, `f32`),
`gemMountType`, `weight` (`f32`), `sellValue` (`i32`), `maxArrowQuantity` (`u8`).

## `Bullet` (`bullet.rs`)

`Bullet` không có field nào để lọc như `weaponCategory` (đã xác nhận khi phân
tích CSV gốc), nên tập dòng cần chỉnh **phải là dữ liệu**. Điều đều đặn là bố
cục ID trong mỗi "họ" đạn: một ID gốc cộng bộ offset cố định cho các biến thể
Ash of War (vd. `base+11` Mighty Shot, `base+50` tốc độ của Rain of Arrows,
`base+51` tầm của nó). Mod mã hoá cấu trúc đó một lần (danh sách ID gốc ghép với
4 bảng offset: Arrow, Great Arrow, Radahn's Spear, Bolt) thay cho ~1000 dòng Mass
Edit viết tay. Hiện có 63 ID gốc (32 Arrow, 6 Great Arrow, 1 Radahn's Spear, 24
Bolt).

Mỗi offset có một vai trò: `Speed` (`initVellocity`/`maxVellocity`), và riêng
Rain of Arrows tách thành 3 đạn con vì cần chỉnh độc lập: tầm (`dist`, điểm bắt
đầu suy giảm sát thương chứ không phải quãng bay), số lượng (`numShoot`) và độ
loe (`shootAngleYMaxRandom`/`shootAngleXMaxRandom`).

- **`numShoot` gán tuyệt đối**, không nhân: giá trị vanilla luôn là 1, nên "số
  lượng" là cái tên trung thực hơn một hệ số của giá trị luôn bằng 1.
- Great Arrow không bắn được Mighty Shot/Barrage/Enchanted Shot nên bảng offset
  của nó nhảy thẳng từ đạn gốc sang Rain of Arrows.
- `apply` tự đếm số ID không tìm thấy và ghi cảnh báo nếu lớn hơn 0: đó là dấu hiệu
  game đã cập nhật và danh sách ID gốc cần dò lại (Smithbox, export `Bullet.csv`,
  so khớp ID).

## Hot reload không cộng dồn

`weapon::apply`/`bullet::apply` từng scale **trực tiếp trên giá trị đang đọc từ
dòng**; gọi lại lần hai sẽ cộng dồn (`* 1.5` hai lần ra `* 2.25`). Sửa bằng cách
lưu **giá trị gốc** của từng dòng (`weapon::Baseline`/`bullet::Baseline`, khoá
theo ID dòng) ngay lần `apply` đầu và mọi lần sau (kể cả reload) luôn scale từ
baseline đó. Field gán tuyệt đối thì không cần baseline: `max_arrow_quantity`,
`num_shoot`, `gem_mount_type` (luôn gán `2`).

## Rủi ro đã biết

- **Làm tròn số nguyên:** field `u16` (sát thương) và `u8` (`max_arrow_quantity`)
  dùng `.round()` trước khi ép kiểu. Chưa kiểm chứng khác biệt với Smithbox Mass
  Edit (không rõ dùng round hay truncate).
- **Regulation lệch header (Convergence và overhaul tương tự):** `weapon.rs` dùng
  `repo.rows_mut::<EquipParamWeapon>()` và `bullet.rs` dùng `repo.get_mut::<Bullet>(id)`.
  Cả hai đi qua lookup table của param file, và lookup table có thể thiếu một phần
  tử so với header ở regulation của Convergence, dẫn tới panic (xem
  `mods/dropmultiplier/docs/convergence_panic.md`; cách sửa là
  `common::params::for_each_row_mut` / `row_ids`). Mod này **chưa chuyển** sang
  hàm đó và chưa có ghi nhận chạy trên regulation lệch như vậy.
- **Danh sách ID đạn gắn với regulation vanilla:** overhaul đổi bố cục ID sẽ làm
  một số ID "không tìm thấy" (chỉ cảnh báo, không crash).
