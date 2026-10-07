# RiseArcher

> Nexus mod 5807 · **đã phát hành** · đã test trong game

Mod cho Elden Ring: **buff toàn diện Bow / Crossbow / Ballista / Arrow / Bolt**:
sát thương, scaling, mở khoá Ash of War cho Bow, trọng lượng, giá bán, số lượng mang
tối đa của mũi tên và tên nỏ, cùng tốc độ, tầm và số lượng mũi tên của Ash of War
Rain of Arrows.

## Hai cách dùng, cùng tồn tại

- **DLL (`src/`, khuyến nghị):** patch sống param trong bộ nhớ lúc chạy qua
  `SoloParamRepository`, cấu hình bằng `RiseArcher.ini`, không đụng
  `regulation.bin` trên đĩa nên không xung đột với mod `regulation.bin` khác.
- **Sửa tĩnh `regulation.bin` qua Smithbox (`csv/`, `project.json`):** cách làm
  gốc, giữ nguyên. `csv/RiseArcher.MASSEDIT` và các file CSV để dán vào Smithbox
  Mass Edit. Hữu ích khi cần merge tay với một mod `regulation.bin` cụ thể hoặc
  không muốn cài DLL. `.smithbox/` là cache project (machine-specific, bị
  git-ignore).

**Hai cách này độc lập, không tự đồng bộ:** hệ số mặc định của DLL được port từ
giá trị trong `.MASSEDIT` nên hiện khớp nhau, nhưng sửa một bên không cập nhật bên
kia. Đổi hệ số mặc định thì sửa cả hai nếu muốn chúng khớp.

## Cấu hình

Cấu hình của DLL trong `RiseArcher.ini` cạnh DLL (tự tạo nếu thiếu; khi nâng cấp,
key mới được thêm vào file có sẵn mà không ghi đè giá trị đã chỉnh). Có thể chỉnh,
riêng cho từng loại vũ khí khi hợp lý:

- hệ số sát thương và scaling;
- mở khoá Ash of War cho Bow;
- hệ số trọng lượng và giá bán;
- số lượng mang tối đa của mũi tên và tên nỏ;
- tốc độ, tầm và độ loe của đạn, và số mũi tên của Rain of Arrows;
- phím nạp lại cấu hình (áp dụng không cần khởi động lại game), việc hiện banner sau
  khi nạp, và ghi log.

Tên key, giá trị mặc định và ý nghĩa nằm ở chú thích trong
[`RiseArcher.ini`](RiseArcher.ini); file mẫu này được nhúng vào DLL nên là nguồn sự
thật duy nhất, README không lặp lại để khỏi lệch.

## Cách hoạt động

- **Khởi động:** `DllMain` tạo một thread, nạp/tạo ini, mở log, chạy watcher
  `ReloadKey` (`common::reload`), rồi chờ người chơi thật sự vào world
  (`WorldChrMan::main_player`, **không giới hạn thời gian**) trước khi đụng param.
  Sớm hơn thế, param của từng bảng chưa load xong và đọc sẽ panic.
- **Vũ khí (`src/weapon.rs`):** lọc `EquipParamWeapon` theo `weaponCategory` /
  `wepType` (loại trừ dòng chỉ dành cho NPC) rồi nhân hoặc gán các field tương ứng.
- **Đạn (`src/bullet.rs`):** `Bullet` không có field để lọc nên mod dùng danh sách ID
  gốc ghép với bảng offset cố định cho các biến thể Ash of War.
- **Hot reload không cộng dồn:** giá trị gốc của mỗi dòng được lưu lại ngay lần áp
  đầu, mỗi lần reload tính lại từ đó.
- **Log:** ghi phiên bản game và danh sách DLL đã nạp (`common::diag`, không in đường
  dẫn đầy đủ).
- Chi tiết thiết kế: [docs/param_patching.md](docs/param_patching.md). Các sự cố đã
  gặp khi khởi động và đọc ini: [docs/startup_and_ini_bugs.md](docs/startup_and_ini_bugs.md).

## Giới hạn

- Mod đọc param bằng cách duyệt lookup table của fromsoftware-rs; với regulation bị
  lệch header (như Convergence) cách này có thể panic. Mod chưa chuyển sang
  `common::params` và chưa có ghi nhận chạy trên regulation như vậy.
- Danh sách ID đạn gắn với regulation vanilla; game cập nhật hoặc overhaul đổi bố cục
  ID thì mod ghi cảnh báo "không tìm thấy ID" (không crash) và cần dò lại danh sách.
- Phép scale số nguyên (sát thương, số lượng tối đa) làm tròn khác Smithbox Mass Edit
  có thể lệch ±1.
- Chỉ dùng ở chế độ offline, tắt EAC.

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/param_patching.md](docs/param_patching.md): bộ lọc vũ khí, họ đạn, baseline, rủi ro.
- [docs/startup_and_ini_bugs.md](docs/startup_and_ini_bugs.md): crash lặng khi khởi động, ini không tác dụng, bỏ cuộc sau 5 phút.
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.
