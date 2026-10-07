# RuneMultiplier

> **v1.0.4** · Nexus mod 10630 · **đã phát hành** (1.0.0 đến 1.0.4), cập nhật cho ER 1.17.1

Mod DLL cho Elden Ring: **nhân hệ số cấu hình được lên số rune nhận được từ mọi
nguồn** (giết quái, nhặt/dùng item, bán đồ...). Rune **chi tiêu** (lên cấp, mua
đồ) không bị ảnh hưởng. Chỉ một hook duy nhất, patch thẳng vào hàm cộng rune
thấp nhất của game (`AddSoul_Call`).

## Cấu hình (`RuneMultiplier.ini`)

File mẫu nhúng trong DLL (`include_str!`), là nguồn sự thật duy nhất cho giá trị
mặc định.

| Section | Key | Mặc định | Ý nghĩa |
|---|---|---|---|
| `[General]` | `ReloadKey` | `F5` | Phím nạp lại ini. Nhận `F1`-`F24`, một ký tự/số, hoặc mã virtual-key dạng hex (`0x2D`) / decimal |
| `[General]` | `ReloadBanner` | `true` | Hiện banner "Config reloaded" sau khi reload |
| `[Settings]` | `Multiplier` | `2.0` | Hệ số nhân lên rune **nhận được**; `1.0` = vanilla, nhỏ hơn 1 để nhận ít đi, số âm để bị trừ rune |
| `[Logging]` | `LogFile` | `false` | Ghi `RuneMultiplier.log` cạnh DLL (khác phần lớn mod khác, mặc định **tắt**); `false` thì không tạo file. Bật thì log thêm byte gốc tại điểm patch và byte của stub |

Đổi ini rồi bấm `ReloadKey` là có hiệu lực ngay, kể cả đổi chính `ReloadKey`, vì
phím được đọc lại mỗi frame. Sau khi reload, mod hiện banner "Config reloaded"
(tắt bằng `ReloadBanner=false`) và ghi dòng log tương ứng. Phím và banner do
`common::reload` lo, giống các mod khác.

## Cách hoạt động

- **Hook (`src/hook.rs`):** tìm `AddSoul_Call` bằng một mẫu AOB neo (duy nhất trên
  2.6.2.0, 2.7.0.0 và 2.7.1.0) rồi đọc lệnh `call` tại `anchor+0x11`. Ghi đè 12
  byte đầu của hàm bằng một jump tuyệt đối tới stub tự sinh. Stub nhân `EDX`
  (số rune cộng thêm) bằng fixed-point Q20 rồi chạy lại đúng 3 lệnh gốc.
- **Chỉ nhân khi số cộng thêm dương:** nhờ đó khoản trừ (lên cấp, mua đồ) không bị
  nhân. Sai số tối đa ±1 rune.
- **Hot reload không patch lại:** stub đọc hệ số qua con trỏ lúc chạy, nên reload
  chỉ cập nhật một biến.
- **Hook thất bại thì mod tắt cho phiên đó** (không tìm thấy neo, không giải
  được `call`, `VirtualAlloc`/`VirtualProtect` lỗi): log ghi lỗi và không patch gì.
  Khi đó cũng không có reload. Nếu chỉ không đăng ký được task theo dõi reload thì hook
  vẫn chạy, chỉ mất hot reload.
- **Khởi động:** `DllMain` tạo một thread, nạp/tạo ini, mở log, cài hook, chạy
  watcher `ReloadKey` (`common::reload`), chờ `CSTaskImp`, rồi đăng ký task
  `FrameBegin` theo dõi mỗi lần reload để cập nhật hệ số. Log ghi phiên bản game
  và danh sách DLL đã nạp (`common::diag`, không in đường dẫn đầy đủ).
- Chi tiết kỹ thuật, các hướng đã thử và bỏ, cách kiểm tra tương thích:
  [docs/addsoul_hook.md](docs/addsoul_hook.md).

Code: [src/hook.rs](src/hook.rs) (toàn bộ logic), [src/lib.rs](src/lib.rs) (entry point).

## Giới hạn

- Phụ thuộc mẫu AOB neo: game vá làm đổi bố cục thì mod tự tắt cho phiên đó.
- Mọi nguồn rune **nhận được** đều bị nhân, không có tuỳ chọn tách riêng từng nguồn.
- Với `Multiplier` âm, rune nhận được bị **trừ** thay vì cộng.
- Chỉ dùng ở chế độ offline, tắt EAC.

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/addsoul_hook.md](docs/addsoul_hook.md): hook `AddSoul_Call`, các hướng đã bỏ, tương thích.
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.
