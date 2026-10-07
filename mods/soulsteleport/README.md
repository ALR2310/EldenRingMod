# SoulsTeleport

> Nexus mod 11119 · **đã phát hành** · đã test trong game (Seamless Co-op)

Mod DLL độc lập cho **Seamless Co-op** (yêu cầu Seamless Co-op, Nexus mod 510): bấm phím mở menu,
menu ImGui liệt kê mọi người trong phiên kèm vai trò (Host, Co-op, Hunter...), bấm một người để
**dịch chuyển tới đúng chỗ họ đang đứng** qua màn hình loading, dù ở xa tới đâu. Vị trí được hỏi trực
tiếp người đó qua Steam P2P, nên **người được tới cũng phải cài mod**. Người chơi thù địch (invader,
Bloody Finger, Recusant) không thể teleport và không ai teleport tới họ được.

## Cấu hình

Cấu hình trong `SoulsTeleport.ini` cạnh DLL (tự tạo nếu thiếu; khi nâng cấp, key mới được thêm vào file
có sẵn mà không ghi đè giá trị đã chỉnh). Có thể chỉnh:

- phím mở/đóng menu;
- hệ số phóng to/thu nhỏ menu (đổi xong phải khởi động lại game);
- vị trí và kích thước menu: mod tự ghi khi người chơi kéo hoặc đổi cỡ menu, và có nút Reset;
- ghi log để chẩn đoán.

Tên key, giá trị mặc định và ý nghĩa nằm ở chú thích trong
[`SoulsTeleport.ini`](SoulsTeleport.ini); file mẫu này được nhúng vào DLL nên là nguồn sự thật duy nhất,
README không lặp lại để khỏi lệch. Mod không có hot reload.

## Cách hoạt động

- **Dịch chuyển:** dùng lại đúng trình tự game tự dùng khi kết thúc phiên multiplayer để trả bạn về
  chỗ đang đứng: yêu cầu chuyển map tới block đích, đặt điểm spawn tuỳ chỉnh, rồi đặt cờ yêu cầu warp.
  Cờ này được ghi thẳng thay vì gọi hàm kích hoạt của game để không làm rớt phiên co-op. Các hàm của
  game tìm bằng AOB (không khoá theo phiên bản; AOB hụt thì mod tự tắt, không ghi sai chỗ). Chi tiết:
  [docs/warp_mechanism.md](docs/warp_mechanism.md).
- **Lấy vị trí đồng đội:** trong Seamless Co-op vị trí đồng đội ở xa không đọc được từ game, nên mod
  hỏi trực tiếp qua Steam P2P: bên hỏi gửi `WHERE`, bên kia trả `HERE` kèm block và tọa độ rồi mới warp.
  Chỉ trả lời khi đã ở trong world thật sự; gói tin được kiểm tra người gửi do Steam xác thực. Chi tiết:
  [docs/position_exchange.md](docs/position_exchange.md).
- **Vai trò và người thù địch:** mỗi người trong phiên hiện kèm vai trò; mod chặn teleport giữa hai bên
  nếu một trong hai là người thù địch, cả khi hỏi lẫn khi trả lời. Chi tiết:
  [docs/roles_and_invaders.md](docs/roles_and_invaders.md).
- **Menu:** Dear ImGui qua hudhook (DirectX 12), tự co giãn theo độ phân giải game; chặn chuột của game
  khi menu mở (bàn phím và tay cầm vẫn tới game); font Noto Sans nhúng trong DLL. Chi tiết:
  [docs/menu_ui.md](docs/menu_ui.md).
- **Khởi động:** `DllMain` tạo một thread, nạp/tạo ini, mở log, cài menu (hook DX12), ghi phiên bản game
  và danh sách DLL đã nạp (`common::diag`, không in đường dẫn đầy đủ), rồi chạy vòng xử lý warp trên task
  của game.

Code: [src/warp.rs](src/warp.rs), [src/net.rs](src/net.rs), [src/steam.rs](src/steam.rs),
[src/party.rs](src/party.rs), [src/ui.rs](src/ui.rs), [src/input_block.rs](src/input_block.rs).

## Giới hạn

- **Cả hai bên phải cài mod**: người được tới là bên trả lời vị trí.
- Ai trong phiên cài mod (và không thù địch) cũng hỏi được vị trí của bạn; nên đặt `cooppassword` riêng
  cho Seamless Co-op để người lạ không vào được phiên.
- Menu chỉ điều khiển bằng bàn phím và chuột (chưa hỗ trợ tay cầm).
- Chưa chặn teleport khi người kia đang cưỡi Torrent trên không, trong trận boss hay đang chết.
- Đọc danh sách phiên, vị trí và trạng thái phiên qua fromsoftware-rs nên phụ thuộc bố cục bộ nhớ từng
  bản game; chưa test phiên từ 3 người trở lên.
- Font nhúng không có chữ Hàn (chữ Hán và kana chỉ hiện khi Windows có sẵn font tương ứng).
- Phím mở menu mặc định trùng phím mở bảng lobby mặc định của SoulsChat, nên ai dùng cả hai mod với mặc
  định cần đổi một trong hai.
- Chỉ dùng ở chế độ offline, tắt EAC.

## Nguồn

Font Noto Sans (SIL OFL 1.1, kèm file giấy phép trong gói phát hành), hudhook, Dear ImGui; tham khảo
cách làm menu và chặn input của QuestPath.

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/warp_mechanism.md](docs/warp_mechanism.md): cơ chế dịch chuyển, AOB, tương thích.
- [docs/position_exchange.md](docs/position_exchange.md): WHERE/HERE qua Steam P2P, chống giả gói tin.
- [docs/roles_and_invaders.md](docs/roles_and_invaders.md): vai trò và chặn teleport giữa người thù địch.
- [docs/menu_ui.md](docs/menu_ui.md): menu ImGui, chuột, input, font.
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.
