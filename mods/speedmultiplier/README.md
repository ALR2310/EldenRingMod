# SpeedMultiplier

> Nexus mod 11173 · **đã phát hành** · đã test trong game

Mod DLL cho Elden Ring: **tăng (hoặc giảm) tốc độ của người chơi và của Torrent** theo từng
nhóm hành động: di chuyển (đi bộ, chạy, đi lén, nhảy, lăn, leo thang), tốc độ tấn công
(đòn thường, đòn chí mạng, kỹ năng vũ khí), tốc độ niệm phép, và dùng item. Có thể đặt tốc
độ khác nhau tuỳ theo hiệu ứng đang có trên người hoặc lớp tải trọng.

## Cấu hình

Mod dùng **TOML** (`SpeedMultiplier.toml` cạnh DLL, tự tạo nếu thiếu), không phải ini như
các mod khác. Khi nâng cấp, key mới được thêm vào file có sẵn mà không ghi đè giá trị đã
chỉnh; key không còn dùng được chuyển thành comment ở cuối file. Có thể chỉnh:

- hệ số tốc độ cho từng nhóm hành động của người chơi, và một hệ số tổng cho tất cả;
- hệ số tốc độ cho các nhóm hành động của Torrent;
- các khối ghi đè tốc độ theo điều kiện: người chơi đang có hiệu ứng (SpEffect) nào đó,
  và/hoặc đang ở một lớp tải trọng nào đó; nhiều khối khớp cùng lúc thì xếp chồng;
- phím nạp lại cấu hình (áp dụng không cần khởi động lại game) và việc hiện banner sau khi
  nạp;
- ghi log, và hai công cụ dò để chẩn đoán (ghi anim đang chạy; ghi SpEffect của người chơi).

Tên key, giá trị mặc định và ý nghĩa nằm ở chú thích trong
[`SpeedMultiplier.toml`](SpeedMultiplier.toml); file mẫu này được nhúng vào DLL nên là nguồn sự
thật duy nhất, README không lặp lại để khỏi lệch. Key gõ sai hoặc sai kiểu được báo kèm số
dòng và **cấu hình đang chạy được giữ nguyên**, không bao giờ bị thay bằng file lỗi.

## Cách hoạt động

- **Một núm duy nhất:** mỗi frame (`ChrIns_PreBehavior`), mod đặt
  `CSChrBehaviorModule.animation_speed` của người chơi theo nhóm của animation đang chạy, và
  của Torrent theo nhóm riêng. Chỉ ghi khi giá trị khác, vì game không tự reset field này.
  Cách tìm ra núm này và thử các hướng khác: [docs/animation_speed_research.md](docs/animation_speed_research.md).
- **Phân nhóm animation:** phép và kỹ năng vũ khí theo prefix của tên TAE; mọi thứ còn lại
  theo phần đuôi của anim id (đi, chạy, đi lén, nhảy, lăn, thang, đòn đánh, item...).
  Bảng phân nhóm và các trường hợp đặc biệt: [docs/action_groups.md](docs/action_groups.md);
  tra cứu tên TAE: [docs/tae-ids.md](docs/tae-ids.md).
- **Ghi đè theo điều kiện:** mỗi frame mod đọc các SpEffect đang có trên người chơi và lớp
  tải trọng, rồi tính tốc độ cuối từ cấu hình gốc cộng các khối ghi đè khớp.
- **Cấu hình TOML xếp lớp:** mặc định nằm trong template nhúng trong DLL, giá trị người dùng
  đặt lên trên; mỗi frame chỉ lấy một snapshot. Thiết kế cấu hình, ghi đè và migration theo
  phiên bản: [docs/config_toml.md](docs/config_toml.md).
- **Tương thích Seamless Co-op:** Seamless hook lại hàm đọc tốc độ animation của game nên
  giá trị mod ghi không được đọc; mod phát hiện điều đó và chuyển hướng hook về phía mình.
  Chi tiết: [docs/seamless.md](docs/seamless.md).
- **Log:** ghi phiên bản game và danh sách DLL đã nạp (`common::diag`, không in đường dẫn đầy
  đủ).

Code: [src/speed.rs](src/speed.rs) (phân nhóm và ghi tốc độ), [src/config.rs](src/config.rs)
(cấu hình), [src/seamless.rs](src/seamless.rs), [src/probe.rs](src/probe.rs) (công cụ dò).

## Giới hạn

- Game chỉ có **một** tốc độ animation cho cả nhân vật, nên không thể tăng riêng animation
  nửa thân trên: uống bình **khi đang chạy** theo nhóm di chuyển, uống khi đứng yên theo nhóm
  item.
- Đòn chí mạng và đòn tóm đi cặp với animation của nạn nhân, mà mod không tăng tốc anim đó;
  nên nhóm đòn chí mạng mặc định giữ tốc độ gốc, và tăng nhiều sẽ lệch nhịp.
- Tia của Placidusax's Ruin chạy theo đồng hồ riêng của game nên không theo tốc độ niệm phép.
- Trong co-op, hình ảnh không đồng bộ giữa hai máy: mỗi máy chỉ tăng tốc nhân vật của chính nó.
- Animation đi bộ khi quá tải nằm ngoài dải của nhóm đi bộ nên không được áp tốc độ đó; tải
  trọng theo % liên tục chưa hỗ trợ (chỉ theo lớp).
- Tăng tốc nhóm "mọi hành động" cũng ảnh hưởng lúc bị đánh, cưỡi ngựa, nghỉ ở grace.
- Chỉ dùng ở chế độ offline, tắt EAC.

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/animation_speed_research.md](docs/animation_speed_research.md): các "núm" tốc độ, SpeedProbe, giới hạn của game.
- [docs/action_groups.md](docs/action_groups.md): phân nhóm hành động.
- [docs/config_toml.md](docs/config_toml.md): cấu hình TOML, ghi đè theo điều kiện, migration.
- [docs/seamless.md](docs/seamless.md): sự cố và cách sửa với Seamless Co-op.
- [docs/tae-ids.md](docs/tae-ids.md): bảng ID TAE của Elden Ring.
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.
