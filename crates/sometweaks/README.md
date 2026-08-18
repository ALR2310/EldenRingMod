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
mỗi tính năng cũ là 1 module bật/tắt độc lập qua `SomeTweaks.ini` (giá trị
`0` ở key nào thì vô hiệu hoá đúng tính năng đó, không có cờ `Enabled`
riêng).

**Đã triển khai:** module `Regen` (hồi HP/FP/Stamina theo thời gian +
theo đòn đánh trúng) — xem `src/regen/mod.rs` + `src/regen/attack_hook.rs`.

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
của `fromsoftware-rs`/`libER`), các module còn lại (Rune/Spirit/Misc/Debug
trong ini hiện chỉ là placeholder).

## Đã chốt (nhưng chưa triển khai)

- **RiseArcher** sẽ chuyển từ sửa tĩnh `regulation.bin` (`.MASSEDIT`) sang
  đọc/ghi param sống bằng `libER`, hệ số chỉnh qua ini thay vì hardcode —
  lý do: cho người dùng tự chỉnh số theo ý thích, và tránh xung đột file
  `regulation.bin` khi cài chung với mod khác. Xem chi tiết ở
  `d:\MyProjects\EldenRing\RiseArcher\README.md` (mục "Quyết định: sẽ
  chuyển sang DLL + libER") - RiseArcher chưa được đưa vào workspace này.

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
